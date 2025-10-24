//! Controls the display of the soldering controller.
use biquad::{self, Biquad, DirectForm2Transposed, ToHertz};
use core::cmp::Ordering::{Greater, Less};
use core::fmt::Write;
use defmt::info;
use embassy_stm32::spi::Spi;
use embassy_stm32::{gpio::Output, mode::Async};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Ticker};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::primitives::{PrimitiveStyle, StyledDrawable, Triangle};
use embedded_graphics::text::Alignment;
use embedded_graphics::text::Text;
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
};
use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
use embedded_menu::interaction::{Action, Interaction, Navigation};
use embedded_menu::{Menu, MenuStyle, SelectValue};
use micromath::F32Ext;
use panic_probe as _;
use profont::{PROFONT_12_POINT, PROFONT_24_POINT, PROFONT_9_POINT};
use ssd1306::mode::BufferedGraphicsModeAsync;
use ssd1306::prelude::{Brightness, DisplayRotation, DisplaySize102x64, SPIInterface};
use ssd1306::Ssd1306Async;

use crate::ui::MENU_STEPS_SIG;
use crate::{
    OperationalState, Persistent, OPERATIONAL_STATE_MUTEX, PERSISTENT_MUTEX, STORE_PERSISTENT_SIG,
};

/// The inner display type (draw target).
type InnerDisplay = Ssd1306Async<
    SPIInterface<ExclusiveDevice<Spi<'static, Async>, Output<'static>, NoDelay>, Output<'static>>,
    DisplaySize102x64,
    BufferedGraphicsModeAsync<DisplaySize102x64>,
>;

/// Signals a new tool temperature.
static TEMPERATURE_MEASUREMENT_DEG_C_SIG: Signal<ThreadModeRawMutex, Option<f32>> = Signal::new();

/// Signals a new power bargraph value.
static POWER_RATIO_BARGRAPH_SIG: Signal<ThreadModeRawMutex, Option<f32>> = Signal::new();

/// Signals the new power limit (power/W).
static POWER_LIMIT_W_SIG: Signal<ThreadModeRawMutex, f32> = Signal::new();

/// Signals a new message to display.
static MESSAGE_SIG: Signal<ThreadModeRawMutex, &str> = Signal::new();

/// Display refresh rate in Hz.
const DISPLAY_REFRESH_RATE_HZ: u64 = 30;

/// Display height in pixels.
const DISPLAY_HEIGHT: i32 = 64;

/// Display width in pixels (shown).
const DISPLAY_WIDTH: i32 = 102;

/// The first visible column index in pixels.
const DISPLAY_FIRST_COL_INDEX: i32 = 0;

/// The last visible column index in pixels.
const DISPLAY_LAST_COL_INDEX: i32 = DISPLAY_WIDTH - 1;

/// The last visible row index in pixels.
const DISPLAY_LAST_ROW_INDEX: i32 = DISPLAY_HEIGHT - 1;

/// Style for filled objects.
const FILLED_STYLE: PrimitiveStyle<BinaryColor> = PrimitiveStyleBuilder::new()
    .stroke_width(1)
    .fill_color(BinaryColor::On)
    .stroke_color(BinaryColor::On)
    .build();

/// Style for objects that only have an outline and no fill.
const OUTLINE_STYLE: PrimitiveStyle<BinaryColor> = PrimitiveStyleBuilder::new()
    .stroke_width(1)
    .fill_color(BinaryColor::Off)
    .stroke_color(BinaryColor::On)
    .build();

/// Wraps displayed items.
struct Display {
    /// The inner display structure (draw target).
    inner: InnerDisplay,
    /// The current temperature.
    temperature_string: heapless::String<10>,
    /// The set temperature.
    set_temperature_string: heapless::String<10>,
    /// The available power.
    power_string: heapless::String<10>,
    /// A message, e.g. tool type or an error.
    message_string: &'static str,
    /// A low-pass filter for the bargraph width.
    power_bargraph_filter: DirectForm2Transposed<f32>,
    /// The width of the power bar on screen.
    power_bar_width: i32,
    /// The ticker that dictates display refresh rate.
    refresh_ticker: Ticker,
}

impl Display {
    /// Create a new display instance.
    async fn new(mut resources: DisplayResources, persistent: &Persistent) -> Self {
        let spi =
            embedded_hal_bus::spi::ExclusiveDevice::new_no_delay(resources.spi, resources.pin_cs)
                .unwrap();
        let interface = SPIInterface::new(spi, resources.pin_dc);
        let mut inner = Ssd1306Async::new(
            interface,
            DisplaySize102x64,
            if persistent.display_is_rotated {
                DisplayRotation::Rotate180
            } else {
                DisplayRotation::Rotate0
            },
        )
        .into_buffered_graphics_mode();

        inner
            .reset(&mut resources.pin_reset, &mut embassy_time::Delay {})
            .await
            .unwrap();
        inner
            .init_with_addr_mode(ssd1306::command::AddrMode::Horizontal)
            .await
            .unwrap();
        inner.set_brightness(Brightness::BRIGHTEST).await.unwrap();

        Self {
            inner,
            temperature_string: heapless::String::new(),
            set_temperature_string: heapless::String::new(),
            power_string: heapless::String::new(),
            message_string: "",
            power_bargraph_filter: biquad::DirectForm2Transposed::<f32>::new(
                biquad::Coefficients::<f32>::from_params(
                    biquad::Type::LowPass,
                    (DISPLAY_REFRESH_RATE_HZ as f32).hz(),
                    2.5_f32.hz(),
                    0.5, // Critically damped
                )
                .unwrap(),
            ),
            power_bar_width: 0,
            refresh_ticker: Ticker::every(Duration::from_hz(DISPLAY_REFRESH_RATE_HZ)),
        }
    }

    /// Update the tool set temperature.
    fn update_set_temperature(&mut self, temperature_deg_c: i16) {
        self.set_temperature_string.clear();
        write!(&mut self.set_temperature_string, "{}", temperature_deg_c).unwrap();
    }

    /// Update the current tool temperature.
    ///
    /// If `None`, tool temperature is invalid. A question mark is displayed.
    fn update_current_temperature(&mut self, temperature_deg_c: Option<f32>) {
        self.temperature_string.clear();

        if let Some(temperature_deg_c) = temperature_deg_c {
            write!(
                &mut self.temperature_string,
                "{}",
                temperature_deg_c.round() as i16
            )
            .unwrap();
        } else {
            write!(&mut self.temperature_string, "?",).unwrap();
        }
    }

    /// Update the available power.
    fn update_power(&mut self, power: f32) {
        self.power_string.clear();
        if !(power.is_nan()) {
            write!(&mut self.power_string, "{} W", power.round() as i16).unwrap();
        }
    }

    /// Update the power bargraph from a ratio, while applying smoothing.
    ///
    /// If the ratio is `None`, the bargraph is hidden.
    fn update_bargraph(&mut self, power_ratio: Option<f32>) {
        if let Some(power_ratio) = power_ratio {
            let raw = power_ratio * DISPLAY_WIDTH as f32;
            self.power_bar_width = (self.power_bargraph_filter.run(raw).round() as i32).max(0);
        } else {
            self.power_bargraph_filter.reset_state();
            self.power_bar_width = 0;
        }
    }

    /// Update the displayed message.
    fn update_message(&mut self, message: &'static str) {
        self.message_string = message;
    }

    /// Access to the inner display interface.
    fn inner(&self) -> &InnerDisplay {
        &self.inner
    }

    /// Mutable access to the inner display interface.
    fn inner_mut(&mut self) -> &mut InnerDisplay {
        &mut self.inner
    }

    /// Draw all elements of the main view.
    fn draw_main_view(&mut self, operational_state: &OperationalState) {
        const SET_TEMP_ARROW_Y: i32 = 11;
        const SET_TEMP_Y: i32 = 11;
        const ARROW_WIDTH: i32 = 4;

        let set_temperature_triangle = Triangle::new(
            Point::new(DISPLAY_FIRST_COL_INDEX, SET_TEMP_ARROW_Y - 2 * ARROW_WIDTH),
            Point::new(DISPLAY_FIRST_COL_INDEX, SET_TEMP_ARROW_Y),
            Point::new(
                DISPLAY_FIRST_COL_INDEX + ARROW_WIDTH,
                SET_TEMP_ARROW_Y - ARROW_WIDTH,
            ),
        );

        if operational_state.set_temperature_is_pending {
            set_temperature_triangle
                .draw_styled(&OUTLINE_STYLE, &mut self.inner)
                .unwrap();
        } else {
            set_temperature_triangle
                .draw_styled(&FILLED_STYLE, &mut self.inner)
                .unwrap();
        }

        let corner_text = if operational_state.tool_is_off {
            Some("OFF")
        } else {
            None
        };

        if let Some(corner_text) = corner_text {
            Text::with_alignment(
                corner_text,
                Point::new(DISPLAY_LAST_COL_INDEX, SET_TEMP_Y),
                MonoTextStyle::new(&PROFONT_12_POINT, BinaryColor::On),
                Alignment::Right,
            )
            .draw(&mut self.inner)
            .unwrap();
        }

        Text::with_alignment(
            &self.temperature_string,
            Point::new(DISPLAY_WIDTH / 2, 34),
            MonoTextStyle::new(&PROFONT_24_POINT, BinaryColor::On),
            Alignment::Center,
        )
        .draw(&mut self.inner)
        .unwrap();

        Text::new(
            &self.set_temperature_string,
            Point::new(DISPLAY_FIRST_COL_INDEX + 2 * ARROW_WIDTH, SET_TEMP_Y),
            MonoTextStyle::new(&PROFONT_12_POINT, BinaryColor::On),
        )
        .draw(&mut self.inner)
        .unwrap();

        Text::with_alignment(
            &self.power_string,
            Point::new(DISPLAY_FIRST_COL_INDEX, DISPLAY_LAST_ROW_INDEX - 20),
            MonoTextStyle::new(&PROFONT_9_POINT, BinaryColor::On),
            Alignment::Left,
        )
        .draw(&mut self.inner)
        .unwrap();

        if operational_state.tool_in_stand {
            Text::with_alignment(
                "Stand",
                Point::new(DISPLAY_LAST_COL_INDEX, DISPLAY_LAST_ROW_INDEX - 20),
                MonoTextStyle::new(&PROFONT_9_POINT, BinaryColor::On),
                Alignment::Right,
            )
            .draw(&mut self.inner)
            .unwrap();
        }

        Rectangle::new(
            Point::new(DISPLAY_FIRST_COL_INDEX, DISPLAY_LAST_ROW_INDEX - 16),
            Size::new(self.power_bar_width as u32, 2),
        )
        .draw_styled(&FILLED_STYLE, &mut self.inner)
        .unwrap();

        Text::with_alignment(
            self.message_string,
            Point::new(DISPLAY_LAST_COL_INDEX / 2, DISPLAY_LAST_ROW_INDEX - 5),
            MonoTextStyle::new(&PROFONT_9_POINT, BinaryColor::On),
            Alignment::Center,
        )
        .draw(&mut self.inner)
        .unwrap();
    }

    /// Flush content and wait for the end of the refresh period.
    async fn show(&mut self) {
        self.inner_mut().flush().await.unwrap();
        self.refresh_ticker.next().await;
    }
}

/// Resources for driving the display.
pub struct DisplayResources {
    /// The display SPI controller.
    pub spi: Spi<'static, Async>,
    /// The display chip select (for SPI)
    pub pin_cs: Output<'static>,
    /// The display data/control line.
    pub pin_dc: Output<'static>,
    /// The display reset line.
    pub pin_reset: Output<'static>,
}

/// Possible results from menu item modifications.
pub enum MenuResult {
    /// Display rotation was changed.
    DisplayRotation(DisplayRotation),
    /// The current margin in mA.
    CurrentMargin(CurrentMarginMa),
    /// Sleep when powering on.
    SleepOnPower(bool),
    /// Sleep when a tool/tip change occurs.
    SleepOnChange(bool),
}

/// The selected current margin w.r.t. the supply's current limit in mA.
#[derive(SelectValue, PartialEq, PartialOrd, Clone)]
#[allow(missing_docs)]
pub enum CurrentMarginMa {
    #[display_as("0.1")]
    _100,
    #[display_as("0.2")]
    _200,
    #[display_as("0.5")]
    _500,
    #[display_as("1.0")]
    _1000,
}

impl From<u16> for CurrentMarginMa {
    fn from(value: u16) -> Self {
        match value {
            100 => CurrentMarginMa::_100,
            200 => CurrentMarginMa::_200,
            500 => CurrentMarginMa::_500,
            1000 => CurrentMarginMa::_1000,
            _ => unreachable!(),
        }
    }
}

impl From<CurrentMarginMa> for u16 {
    fn from(value: CurrentMarginMa) -> Self {
        match value {
            CurrentMarginMa::_100 => 100,
            CurrentMarginMa::_200 => 200,
            CurrentMarginMa::_500 => 500,
            CurrentMarginMa::_1000 => 1000,
        }
    }
}

/// Handle displaying the UI.
#[embassy_executor::task]
pub async fn display_task(display_resources: DisplayResources) {
    const GIT_HASH: &str = env!("GIT_HASH");

    let persistent = PERSISTENT_MUTEX.lock(|x| *x.borrow());
    let mut display = Display::new(display_resources, &persistent).await;

    // FIXME: Persistent storage in callback?
    let version: heapless::String<13> = heapless::format!("Ver. {}", GIT_HASH).unwrap();
    let mut main_menu = Menu::with_style(
        version,
        MenuStyle::default()
            .with_title_font(&PROFONT_12_POINT)
            .with_font(&PROFONT_9_POINT),
    )
    .add_item("Rotate", persistent.display_is_rotated, |b| {
        if b {
            MenuResult::DisplayRotation(DisplayRotation::Rotate180)
        } else {
            MenuResult::DisplayRotation(DisplayRotation::Rotate0)
        }
    })
    .add_item(
        "Margin / A",
        persistent.current_margin_ma.into(),
        MenuResult::CurrentMargin,
    )
    .add_item("Slp on power", persistent.sleep_on_power, |v| {
        MenuResult::SleepOnPower(v)
    })
    .add_item("Slp on change", persistent.sleep_on_change, |v| {
        MenuResult::SleepOnChange(v)
    })
    .build();

    loop {
        display.inner_mut().clear_buffer();

        let operational_state = OPERATIONAL_STATE_MUTEX.lock(|x| *x.borrow());
        let persistent = PERSISTENT_MUTEX.lock(|x| *x.borrow());

        display.update_set_temperature(persistent.set_temperature_deg_c);

        if let Some(temperature_deg_c) = TEMPERATURE_MEASUREMENT_DEG_C_SIG.try_take() {
            display.update_current_temperature(temperature_deg_c);
        }

        if let Some(message) = MESSAGE_SIG.try_take() {
            display.update_message(message);
        }

        if let Some(power) = POWER_LIMIT_W_SIG.try_take() {
            display.update_power(power);
        }

        if let Some(power_ratio) = POWER_RATIO_BARGRAPH_SIG.try_take() {
            display.update_bargraph(power_ratio);
        }

        if operational_state.menu_state.is_open {
            let steps = MENU_STEPS_SIG.try_take().unwrap_or_default();

            match steps.cmp(&0) {
                Greater => {
                    main_menu.interact(Interaction::Navigation(Navigation::BackwardWrapping(
                        steps as usize,
                    )));
                }
                Less => {
                    main_menu.interact(Interaction::Navigation(Navigation::ForwardWrapping(
                        -steps as usize,
                    )));
                }
                _ => (),
            };

            if operational_state.menu_state.toggle_pending {
                OPERATIONAL_STATE_MUTEX.lock(|x| {
                    x.borrow_mut().menu_state.toggle_pending = false;
                });

                match main_menu.interact(Interaction::Action(Action::Select)) {
                    Some(MenuResult::DisplayRotation(r)) => {
                        PERSISTENT_MUTEX.lock(|x| {
                            x.borrow_mut().display_is_rotated = match r {
                                DisplayRotation::Rotate0 => false,
                                DisplayRotation::Rotate180 => true,
                                _ => unreachable!(),
                            };
                        });
                        STORE_PERSISTENT_SIG.signal(());

                        display.inner_mut().set_rotation(r).await.unwrap();
                        info!("set rotation");
                    }
                    Some(MenuResult::CurrentMargin(c)) => {
                        PERSISTENT_MUTEX.lock(|x| x.borrow_mut().current_margin_ma = c.into());
                        STORE_PERSISTENT_SIG.signal(());
                    }
                    Some(MenuResult::SleepOnPower(v)) => {
                        PERSISTENT_MUTEX.lock(|x| x.borrow_mut().sleep_on_power = v);
                        STORE_PERSISTENT_SIG.signal(());
                    }
                    Some(MenuResult::SleepOnChange(v)) => {
                        PERSISTENT_MUTEX.lock(|x| x.borrow_mut().sleep_on_change = v);
                        STORE_PERSISTENT_SIG.signal(());
                    }
                    _ => (),
                };
            }

            main_menu.update(display.inner());
            main_menu.draw(display.inner_mut()).unwrap();
        } else {
            display.draw_main_view(&operational_state);
        }

        display.show().await;
    }
}

/// Display the current tool temperature.
///
/// Passing `None` hides tool temperature.
pub fn show_current_temperature(tool_temperature_deg_c: Option<f32>) {
    TEMPERATURE_MEASUREMENT_DEG_C_SIG.signal(tool_temperature_deg_c);
}

/// Display a relative power bargraph.
///
/// Passing `None` hides the bargraph.
pub fn show_current_power(power_ratio: Option<f32>) {
    POWER_RATIO_BARGRAPH_SIG.signal(power_ratio);
}

/// Displays the effective power limit (depends on supply minus margin, and tool capabilities).
pub fn show_power_limit(power_limit: f32) {
    POWER_LIMIT_W_SIG.signal(power_limit);
}

/// Displays a status message.
///
/// Mostly used for displaying the current tool's name, but also (tool) error messages.
pub fn show_status_message(message: &'static str) {
    MESSAGE_SIG.signal(message);
}
