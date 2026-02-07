//! Controls the display of the soldering controller.
use biquad::{self, Biquad, DirectForm2Transposed, ToHertz};
use core::cmp::Ordering::{Greater, Less};
use core::fmt::Write;
use defmt::info;
use embassy_stm32::spi::Spi;
use embassy_stm32::{gpio::Output, mode::Async};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Ticker};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::primitives::{PrimitiveStyle, StyledDrawable, Triangle};
use embedded_graphics::text::Alignment;
use embedded_graphics::text::Text;
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
};
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_menu::interaction::{Action, Interaction, Navigation};
use embedded_menu::{Menu, MenuStyle, SelectValue};
use micromath::F32Ext;
use panic_probe as _;
use profont::{PROFONT_9_POINT, PROFONT_24_POINT};
use ssd1306::Ssd1306Async;
use ssd1306::mode::BufferedGraphicsModeAsync;
use ssd1306::prelude::{Brightness, DisplayRotation, DisplaySize102x64, SPIInterface};

use crate::control::tool::Error as ToolError;
use crate::control::tool::ToolState;
use crate::ui::MENU_STEPS_SIG;
use crate::{
    AutoSleep, OPERATIONAL_STATE_MUTEX, OperationalState, PERSISTENT_MUTEX, Persistent,
    STORE_PERSISTENT_SIG, dfu,
};

/// The inner display type (draw target).
type InnerDisplay = Ssd1306Async<
    SPIInterface<
        ExclusiveDevice<Spi<'static, Async>, Output<'static>, embassy_time::Delay>,
        Output<'static>,
    >,
    DisplaySize102x64,
    BufferedGraphicsModeAsync<DisplaySize102x64>,
>;

/// Signals a new tool temperature.
static TEMPERATURE_MEASUREMENT_DEG_C_SIG: Signal<ThreadModeRawMutex, Option<f32>> = Signal::new();

/// Signals a new power bargraph value.
static POWER_RATIO_BARGRAPH_SIG: Signal<ThreadModeRawMutex, Option<f32>> = Signal::new();

/// Signals the new power limit (power/W).
static POWER_LIMIT_W_SIG: Signal<ThreadModeRawMutex, Option<f32>> = Signal::new();

/// Signals a new message to display.
static MESSAGE_SIG: Signal<ThreadModeRawMutex, &str> = Signal::new();

/// Display refresh rate in Hz.
const DISPLAY_REFRESH_RATE_HZ: u64 = 50;

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
struct Display<'d> {
    /// The inner display structure (draw target).
    inner: &'d mut InnerDisplay,
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
    /// The number of draws.
    draw_count: u64,
}

impl<'d> Display<'d> {
    /// Create a new display instance.
    async fn new(inner: &'d mut InnerDisplay) -> Self {
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
            draw_count: 0,
        }
    }

    /// Update the tool set temperature.
    fn update_set_temperature(&mut self, temperature_deg_c: i16) {
        self.set_temperature_string.clear();
        write!(&mut self.set_temperature_string, "{}", temperature_deg_c).unwrap();
    }

    /// Update the current tool temperature.
    ///
    /// If `None`, tool temperature is too low to measure.
    fn update_current_temperature(
        &mut self,
        temperature_deg_c: Option<f32>,
        operational_state: &OperationalState,
    ) {
        self.temperature_string.clear();

        if operational_state.tool.is_err() {
            // Do not write a temperature string, when no tool is present.
        } else if let Some(temperature_deg_c) = temperature_deg_c
            && temperature_deg_c > 40.0
        {
            write!(
                &mut self.temperature_string,
                "{}",
                temperature_deg_c.round() as i16
            )
            .unwrap();
        } else {
            write!(&mut self.temperature_string, "Cold",).unwrap();
        }
    }

    /// Update the available power.
    fn update_power(&mut self, operational_state: &OperationalState, power_limit: Option<f32>) {
        self.power_string.clear();
        let power = match power_limit {
            Some(x) => x,
            None => operational_state.negotiated_power_w,
        };

        if !power.is_nan() {
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
        self.inner
    }

    /// Mutable access to the inner display interface.
    fn inner_mut(&mut self) -> &mut InnerDisplay {
        self.inner
    }

    /// Draw all elements of the main view.
    fn draw_main_view(&mut self, operational_state: &OperationalState, persistent: &Persistent) {
        self.draw_count += 1;

        const SET_TEMP_ARROW_Y: i32 = 7;
        const SET_TEMP_Y: i32 = 7;
        const ARROW_WIDTH: i32 = 3;

        if operational_state.tool.is_err() {
            display_idle_state();
        }

        self.message_string = match operational_state.tool {
            Err(ToolError::NoTool) => "No tool",
            Err(ToolError::NoTip) => "No tip",
            Err(ToolError::UnknownTool) => "Unknown",
            Err(ToolError::ToolMismatch) => "",
            Ok(name) => name,
        };

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
                .draw_styled(&OUTLINE_STYLE, self.inner)
                .unwrap();
        } else {
            set_temperature_triangle
                .draw_styled(&FILLED_STYLE, self.inner)
                .unwrap();
        }

        Text::with_alignment(
            &self.temperature_string,
            Point::new(DISPLAY_WIDTH / 2, 32),
            MonoTextStyle::new(&PROFONT_24_POINT, BinaryColor::On),
            Alignment::Center,
        )
        .draw(self.inner)
        .unwrap();

        Text::new(
            &self.set_temperature_string,
            Point::new(DISPLAY_FIRST_COL_INDEX + 2 * ARROW_WIDTH, SET_TEMP_Y),
            MonoTextStyle::new(&PROFONT_9_POINT, BinaryColor::On),
        )
        .draw(self.inner)
        .unwrap();

        Text::with_alignment(
            &self.power_string,
            Point::new(DISPLAY_FIRST_COL_INDEX, DISPLAY_LAST_ROW_INDEX - 20),
            MonoTextStyle::new(&PROFONT_9_POINT, BinaryColor::On),
            Alignment::Left,
        )
        .draw(self.inner)
        .unwrap();

        let mut tool_state_string = heapless::String::<15>::new();
        let tool_state_str: &str = match (operational_state.tool_state, persistent.auto_sleep) {
            (None, _) | (Some(ToolState::Active), _) => "",
            (Some(ToolState::InStand(_)), AutoSleep::Never) => "Stand",
            (Some(ToolState::InStand(_)), AutoSleep::AfterDurationS(0)) => "Sleep",
            (Some(ToolState::Sleeping), _) => "Sleep",
            (Some(ToolState::InStand(instant)), AutoSleep::AfterDurationS(duration_s)) => {
                let auto_sleep_duration = Duration::from_secs(duration_s as u64);
                if let Some(passed_duration) = Instant::now().checked_duration_since(instant) {
                    let remaining_seconds = auto_sleep_duration
                        .checked_sub(passed_duration)
                        .map(|v| v.as_secs())
                        .unwrap_or_default();

                    let minutes = remaining_seconds / 60;
                    let seconds = remaining_seconds.saturating_sub(minutes * 60);

                    write!(&mut tool_state_string, "Sleep {}:{:02}", minutes, seconds).unwrap();
                    tool_state_string.as_str()
                } else {
                    ""
                }
            }
        };

        Text::with_alignment(
            tool_state_str,
            Point::new(DISPLAY_LAST_COL_INDEX, SET_TEMP_Y),
            MonoTextStyle::new(&PROFONT_9_POINT, BinaryColor::On),
            Alignment::Right,
        )
        .draw(self.inner)
        .unwrap();

        if operational_state.tool_is_off {
            Text::with_alignment(
                "Off",
                Point::new(DISPLAY_LAST_COL_INDEX, DISPLAY_LAST_ROW_INDEX - 20),
                MonoTextStyle::new(&PROFONT_9_POINT, BinaryColor::On),
                Alignment::Right,
            )
            .draw(self.inner)
            .unwrap();
        };

        Rectangle::new(
            Point::new(DISPLAY_FIRST_COL_INDEX, DISPLAY_LAST_ROW_INDEX - 16),
            Size::new(self.power_bar_width as u32, 2),
        )
        .draw_styled(&FILLED_STYLE, self.inner)
        .unwrap();

        Text::with_alignment(
            self.message_string,
            Point::new(DISPLAY_LAST_COL_INDEX / 2, DISPLAY_LAST_ROW_INDEX - 5),
            MonoTextStyle::new(&PROFONT_9_POINT, BinaryColor::On),
            Alignment::Center,
        )
        .draw(self.inner)
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

/// Possible settings for auto-sleep (after the tool is in its stand).
#[derive(SelectValue, PartialEq, PartialOrd, Clone)]
#[allow(clippy::missing_docs_in_private_items)]
enum AutoSleepMenu {
    #[display_as("0")]
    Immediately,
    #[display_as("1")]
    After1Minute,
    #[display_as("5")]
    After5Minutes,
    #[display_as("10")]
    After10Minutes,
    #[display_as("30")]
    After30Minutes,
    #[display_as("60")]
    After60Minutes,
    #[display_as("inf")]
    Never,
}

impl From<AutoSleep> for AutoSleepMenu {
    fn from(value: AutoSleep) -> Self {
        match value {
            AutoSleep::AfterDurationS(0) => AutoSleepMenu::Immediately,
            AutoSleep::AfterDurationS(60) => AutoSleepMenu::After1Minute,
            AutoSleep::AfterDurationS(300) => AutoSleepMenu::After5Minutes,
            AutoSleep::AfterDurationS(600) => AutoSleepMenu::After10Minutes,
            AutoSleep::AfterDurationS(1800) => AutoSleepMenu::After30Minutes,
            AutoSleep::AfterDurationS(3600) => AutoSleepMenu::After60Minutes,
            AutoSleep::Never => AutoSleepMenu::Never,
            _ => unreachable!(),
        }
    }
}

impl From<AutoSleepMenu> for AutoSleep {
    fn from(value: AutoSleepMenu) -> Self {
        match value {
            AutoSleepMenu::Immediately => AutoSleep::AfterDurationS(0),
            AutoSleepMenu::After1Minute => AutoSleep::AfterDurationS(60),
            AutoSleepMenu::After5Minutes => AutoSleep::AfterDurationS(300),
            AutoSleepMenu::After10Minutes => AutoSleep::AfterDurationS(600),
            AutoSleepMenu::After30Minutes => AutoSleep::AfterDurationS(1800),
            AutoSleepMenu::After60Minutes => AutoSleep::AfterDurationS(3600),
            AutoSleepMenu::Never => AutoSleep::Never,
        }
    }
}

/// Possible results from menu item modifications.
enum MenuResult {
    /// Display rotation (normal or inverted).
    DisplayRotation(DisplayRotation),
    /// The current margin setting.
    CurrentMargin(CurrentMarginMenu),
    /// The auto sleep setting.
    AutoSleep(AutoSleepMenu),
    /// Switch off after power-cycle.
    OffOnPower(bool),
    /// Switch off after a tool/tip change occurs.
    OffOnChange(bool),
    /// Go to DFU mode.
    Dfu,
}

/// The selected current margin w.r.t. the supply's current limit in mA.
#[derive(SelectValue, PartialEq, PartialOrd, Clone)]
#[allow(clippy::missing_docs_in_private_items)]
enum CurrentMarginMenu {
    #[display_as("0.1")]
    _100,
    #[display_as("0.2")]
    _200,
    #[display_as("0.5")]
    _500,
    #[display_as("1.0")]
    _1000,
}

impl From<u16> for CurrentMarginMenu {
    fn from(value: u16) -> Self {
        match value {
            100 => CurrentMarginMenu::_100,
            200 => CurrentMarginMenu::_200,
            500 => CurrentMarginMenu::_500,
            1000 => CurrentMarginMenu::_1000,
            _ => unreachable!(),
        }
    }
}

impl From<CurrentMarginMenu> for u16 {
    fn from(value: CurrentMarginMenu) -> Self {
        match value {
            CurrentMarginMenu::_100 => 100,
            CurrentMarginMenu::_200 => 200,
            CurrentMarginMenu::_500 => 500,
            CurrentMarginMenu::_1000 => 1000,
        }
    }
}

/// Handle displaying the UI.
#[embassy_executor::task]
pub async fn display_task(mut display_resources: DisplayResources) {
    const GIT_HASH: &str = env!("GIT_HASH");
    let persistent = PERSISTENT_MUTEX.lock(|x| *x.borrow());

    let spi = embedded_hal_bus::spi::ExclusiveDevice::new(
        display_resources.spi,
        display_resources.pin_cs,
        embassy_time::Delay,
    )
    .unwrap();
    let interface = SPIInterface::new(spi, display_resources.pin_dc);
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
        .reset(
            &mut display_resources.pin_reset,
            &mut embassy_time::Delay {},
        )
        .await
        .unwrap();
    inner
        .init_with_addr_mode(ssd1306::command::AddrMode::Horizontal)
        .await
        .unwrap();
    inner.set_brightness(Brightness::BRIGHTEST).await.unwrap();

    let mut display = Display::new(&mut inner).await;

    // FIXME: Persistent storage in callback?
    let version: heapless::String<13> = heapless::format!("Ver. {}", GIT_HASH).unwrap();
    let mut main_menu = Menu::with_style(
        version,
        MenuStyle::default()
            .with_title_font(&PROFONT_9_POINT)
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
    .add_item("Off on power", persistent.off_on_power, |v| {
        MenuResult::OffOnPower(v)
    })
    .add_item("Off on change", persistent.off_on_change, |v| {
        MenuResult::OffOnChange(v)
    })
    .add_item("Sleep / min", persistent.auto_sleep.into(), |v| {
        MenuResult::AutoSleep(v)
    })
    .add_item("DFU mode", ">", |_| MenuResult::Dfu)
    .build();

    loop {
        display.inner_mut().clear_buffer();

        let operational_state = OPERATIONAL_STATE_MUTEX.lock(|x| *x.borrow());
        let persistent = PERSISTENT_MUTEX.lock(|x| *x.borrow());

        display.update_set_temperature(persistent.set_temperature_deg_c);

        if let Some(temperature_deg_c) = TEMPERATURE_MEASUREMENT_DEG_C_SIG.try_take() {
            display.update_current_temperature(temperature_deg_c, &operational_state);
        }

        if let Some(message) = MESSAGE_SIG.try_take() {
            display.update_message(message);
        }

        if let Some(power_limit) = POWER_LIMIT_W_SIG.try_take() {
            display.update_power(&operational_state, power_limit);
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
                        info!("Set rotation");
                    }
                    Some(MenuResult::CurrentMargin(c)) => {
                        PERSISTENT_MUTEX.lock(|x| x.borrow_mut().current_margin_ma = c.into());
                        STORE_PERSISTENT_SIG.signal(());
                    }
                    Some(MenuResult::OffOnPower(v)) => {
                        PERSISTENT_MUTEX.lock(|x| x.borrow_mut().off_on_power = v);
                        STORE_PERSISTENT_SIG.signal(());
                    }
                    Some(MenuResult::OffOnChange(v)) => {
                        PERSISTENT_MUTEX.lock(|x| x.borrow_mut().off_on_change = v);
                        STORE_PERSISTENT_SIG.signal(());
                    }
                    Some(MenuResult::AutoSleep(v)) => {
                        PERSISTENT_MUTEX.lock(|x| x.borrow_mut().auto_sleep = v.into());
                        STORE_PERSISTENT_SIG.signal(());
                    }
                    Some(MenuResult::Dfu) => unsafe { dfu::jump() },
                    _ => (),
                };
            }

            main_menu.update(display.inner());
            main_menu.draw(display.inner_mut()).unwrap();
        } else {
            display.draw_main_view(&operational_state, &persistent);
        }

        display.show().await;
    }
}

/// Display the current tool temperature.
///
/// Passing `None` hides tool temperature.
pub fn display_current_temperature(tool_temperature_deg_c: Option<f32>) {
    TEMPERATURE_MEASUREMENT_DEG_C_SIG.signal(tool_temperature_deg_c);
}

/// Display a relative power bargraph.
///
/// Passing `None` hides the bargraph.
pub fn display_current_power(power_ratio: Option<f32>) {
    POWER_RATIO_BARGRAPH_SIG.signal(power_ratio);
}

/// Displays the effective power limit (depends on supply minus margin, and tool capabilities).
pub fn display_power_limit(power_limit: Option<f32>) {
    POWER_LIMIT_W_SIG.signal(power_limit);
}

/// Display the idle state, switching off fields that are only present when a tool is present and active.
pub fn display_idle_state() {
    display_power_limit(None);
    display_current_power(None);
    display_current_temperature(None);
}
