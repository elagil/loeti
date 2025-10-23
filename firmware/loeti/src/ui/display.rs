//! Controls the display of the soldering controller.
use biquad::{self, Biquad, ToHertz};
use core::cmp::Ordering::{Greater, Less};
use core::fmt::Write;
use defmt::info;
use embassy_stm32::spi::Spi;
use embassy_stm32::{gpio::Output, mode::Async};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Ticker};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::primitives::{StyledDrawable, Triangle};
use embedded_graphics::text::Alignment;
use embedded_graphics::text::Text;
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
};
use embedded_menu::interaction::{Action, Interaction, Navigation};
use embedded_menu::{Menu, MenuStyle, SelectValue};
use micromath::F32Ext;
use panic_probe as _;
use profont::{PROFONT_12_POINT, PROFONT_24_POINT, PROFONT_9_POINT};
use ssd1306::prelude::{Brightness, DisplayRotation, DisplaySize102x64, SPIInterface};
use ssd1306::Ssd1306Async;
use uom::si::f32::Power;
use uom::si::power;

use crate::ui::MENU_STEPS_SIG;
use crate::{OPERATIONAL_STATE_MUTEX, PERSISTENT_MUTEX, STORE_PERSISTENT_SIG};

/// Signals a new tool temperature.
static TEMPERATURE_MEASUREMENT_DEG_C_SIG: Signal<ThreadModeRawMutex, Option<f32>> = Signal::new();

/// Signals a new power bargraph value.
static POWER_RATIO_BARGRAPH_SIG: Signal<ThreadModeRawMutex, Option<f32>> = Signal::new();

/// Signals the new power limit (power/W).
static POWER_LIMIT_SIG: Signal<ThreadModeRawMutex, Power> = Signal::new();

/// Signals a new message to display.
static MESSAGE_SIG: Signal<ThreadModeRawMutex, &str> = Signal::new();

/// Display refresh rate.
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

#[derive(SelectValue, PartialEq, PartialOrd, Clone)]
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
pub async fn display_task(mut display_resources: DisplayResources) {
    const GIT_HASH: &str = env!("GIT_HASH");

    let persistent = PERSISTENT_MUTEX.lock(|x| *x.borrow());

    let mut bargraph_filter = biquad::DirectForm2Transposed::<f32>::new(
        biquad::Coefficients::<f32>::from_params(
            biquad::Type::LowPass,
            (DISPLAY_REFRESH_RATE_HZ as f32).hz(),
            2.5_f32.hz(),
            0.5, // Critically damped
        )
        .unwrap(),
    );

    let spi = embedded_hal_bus::spi::ExclusiveDevice::new_no_delay(
        display_resources.spi,
        display_resources.pin_cs,
    )
    .unwrap();
    let interface = SPIInterface::new(spi, display_resources.pin_dc);
    let mut display = Ssd1306Async::new(
        interface,
        DisplaySize102x64,
        if persistent.display_is_rotated {
            DisplayRotation::Rotate180
        } else {
            DisplayRotation::Rotate0
        },
    )
    .into_buffered_graphics_mode();

    display
        .reset(
            &mut display_resources.pin_reset,
            &mut embassy_time::Delay {},
        )
        .await
        .unwrap();
    display
        .init_with_addr_mode(ssd1306::command::AddrMode::Horizontal)
        .await
        .unwrap();
    display.set_brightness(Brightness::BRIGHTEST).await.unwrap();

    let filled_style = PrimitiveStyleBuilder::new()
        .stroke_width(1)
        .fill_color(BinaryColor::On)
        .stroke_color(BinaryColor::On)
        .build();

    let outline_style = PrimitiveStyleBuilder::new()
        .stroke_width(1)
        .fill_color(BinaryColor::Off)
        .stroke_color(BinaryColor::On)
        .build();

    let mut power_bar_width = 0;
    let mut temperature_string: heapless::String<10> = heapless::String::new();
    let mut set_temperature_string: heapless::String<10> = heapless::String::new();
    let mut power_string: heapless::String<10> = heapless::String::new();
    let mut message_string: &str = "";

    let mut refresh_ticker = Ticker::every(Duration::from_hz(DISPLAY_REFRESH_RATE_HZ));

    // FIXME: Persistent storage in callback?
    let version: heapless::String<13> = heapless::format!("Ver. {}", GIT_HASH).unwrap();
    let mut menu = Menu::with_style(
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
        CurrentMarginMa::_100,
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
        let operational_state = OPERATIONAL_STATE_MUTEX.lock(|x| *x.borrow());
        let persistent = PERSISTENT_MUTEX.lock(|x| *x.borrow());

        set_temperature_string.clear();
        write!(
            &mut set_temperature_string,
            "{}",
            persistent.operational_temperature_deg_c
        )
        .unwrap();

        if let Some(temperature_deg_c) = TEMPERATURE_MEASUREMENT_DEG_C_SIG.try_take() {
            temperature_string.clear();

            if let Some(temperature_deg_c) = temperature_deg_c {
                write!(
                    &mut temperature_string,
                    "{}",
                    temperature_deg_c.round() as usize
                )
                .unwrap();
            } else {
                write!(&mut temperature_string, "?",).unwrap();
            }
        }

        if let Some(message) = MESSAGE_SIG.try_take() {
            message_string = message;
        }

        if let Some(power) = POWER_LIMIT_SIG.try_take() {
            let power = power.get::<power::watt>();

            power_string.clear();
            if !(power.is_nan()) {
                write!(&mut power_string, "{} W", power.round() as usize).unwrap();
            }
        }

        if let Some(power_ratio) = POWER_RATIO_BARGRAPH_SIG.try_take() {
            if let Some(power_ratio) = power_ratio {
                let raw = power_ratio * DISPLAY_WIDTH as f32;
                power_bar_width = (bargraph_filter.run(raw).round() as i32).max(0);
            } else {
                bargraph_filter.reset_state();
                power_bar_width = 0;
            }
        }

        display.clear_buffer();

        if operational_state.menu_state.is_open {
            let steps = MENU_STEPS_SIG.try_take().unwrap_or_default();

            match steps.cmp(&0) {
                Greater => {
                    menu.interact(Interaction::Navigation(Navigation::BackwardWrapping(
                        steps as usize,
                    )));
                }
                Less => {
                    menu.interact(Interaction::Navigation(Navigation::ForwardWrapping(
                        -steps as usize,
                    )));
                }
                _ => (),
            };

            if operational_state.menu_state.toggle_pending {
                OPERATIONAL_STATE_MUTEX.lock(|x| {
                    x.borrow_mut().menu_state.toggle_pending = false;
                });

                match menu.interact(Interaction::Action(Action::Select)) {
                    Some(MenuResult::DisplayRotation(r)) => {
                        PERSISTENT_MUTEX.lock(|x| {
                            x.borrow_mut().display_is_rotated = match r {
                                DisplayRotation::Rotate0 => false,
                                DisplayRotation::Rotate180 => true,
                                _ => unreachable!(),
                            };
                        });
                        STORE_PERSISTENT_SIG.signal(());

                        display.set_rotation(r).await.unwrap();
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

            menu.update(&display);
            menu.draw(&mut display).unwrap();
        } else {
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
                    .draw_styled(&outline_style, &mut display)
                    .unwrap();
            } else {
                set_temperature_triangle
                    .draw_styled(&filled_style, &mut display)
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
                .draw(&mut display)
                .unwrap();
            }

            Text::with_alignment(
                &temperature_string,
                Point::new(DISPLAY_WIDTH / 2, 34),
                MonoTextStyle::new(&PROFONT_24_POINT, BinaryColor::On),
                Alignment::Center,
            )
            .draw(&mut display)
            .unwrap();

            Text::new(
                &set_temperature_string,
                Point::new(DISPLAY_FIRST_COL_INDEX + 2 * ARROW_WIDTH, SET_TEMP_Y),
                MonoTextStyle::new(&PROFONT_12_POINT, BinaryColor::On),
            )
            .draw(&mut display)
            .unwrap();

            Text::with_alignment(
                &power_string,
                Point::new(DISPLAY_FIRST_COL_INDEX, DISPLAY_LAST_ROW_INDEX - 20),
                MonoTextStyle::new(&PROFONT_9_POINT, BinaryColor::On),
                Alignment::Left,
            )
            .draw(&mut display)
            .unwrap();

            if operational_state.tool_in_stand {
                Text::with_alignment(
                    "Stand",
                    Point::new(DISPLAY_LAST_COL_INDEX, DISPLAY_LAST_ROW_INDEX - 20),
                    MonoTextStyle::new(&PROFONT_9_POINT, BinaryColor::On),
                    Alignment::Right,
                )
                .draw(&mut display)
                .unwrap();
            }

            Rectangle::new(
                Point::new(DISPLAY_FIRST_COL_INDEX, DISPLAY_LAST_ROW_INDEX - 16),
                Size::new(power_bar_width as u32, 2),
            )
            .draw_styled(&filled_style, &mut display)
            .unwrap();

            Text::with_alignment(
                message_string,
                Point::new(DISPLAY_LAST_COL_INDEX / 2, DISPLAY_LAST_ROW_INDEX - 5),
                MonoTextStyle::new(&PROFONT_9_POINT, BinaryColor::On),
                Alignment::Center,
            )
            .draw(&mut display)
            .unwrap();
        }

        display.flush().await.unwrap();

        refresh_ticker.next().await;
    }
}

/// Display the current tool temperature.
///
/// Passing `None` hides tool temperature.
pub fn show_current_temperature(tool_temperature_deg_c: Option<f32>) {
    TEMPERATURE_MEASUREMENT_DEG_C_SIG.signal(tool_temperature_deg_c);
}

/// Display a power measurement and relative power bargraph.
///
/// Updates the displayed value. Passing `None` hides the bargraph.
pub fn show_current_power(power_ratio: Option<f32>) {
    POWER_RATIO_BARGRAPH_SIG.signal(power_ratio);
}

/// Displays negotiated power.
pub fn show_power_limit(power_limit: Power) {
    POWER_LIMIT_SIG.signal(power_limit);
}

/// Displays a message.
///
/// Mostly used for displaying the current tool's name, but also (tool) error messages.
pub fn show_message(message: &'static str) {
    MESSAGE_SIG.signal(message);
}
