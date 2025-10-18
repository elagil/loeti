//! Controls the display of the soldering controller.
use biquad::{self, Biquad, ToHertz};
use core::cmp::Ordering::{Greater, Less};
use core::fmt::Write;
use defmt::info;
use embassy_stm32::spi::Spi;
use embassy_stm32::{gpio::Output, mode::Async};
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
use embedded_menu::items::menu_item::SelectValue;
use embedded_menu::{Menu, MenuStyle};
use micromath::F32Ext;
use panic_probe as _;
use profont::{PROFONT_12_POINT, PROFONT_24_POINT, PROFONT_9_POINT};
use ssd1306::prelude::{Brightness, DisplayRotation, DisplaySize102x64, SPIInterface};
use ssd1306::Ssd1306Async;

use crate::ui::MENU_STEPS_SIG;
use crate::{
    DISPLAY_POWER_SIG, MESSAGE_SIG, OPERATIONAL_STATE_MUTEX, PERSISTENT_MUTEX,
    POWER_RATIO_BARGRAPH_SIG, STORE_PERSISTENT_SIG, TEMPERATURE_MEASUREMENT_DEG_C_SIG,
};

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
    CurrentMargin(CurrentMargin),
    /// Sleep when powering on.
    SleepOnPower(bool),
    /// Sleep when an error occurs.
    SleepOnError(bool),
}

/// Adjust current margin (from max. supply current).
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct CurrentMargin {
    /// The margin in units of mA.
    current_ma: u16,
}

impl SelectValue for CurrentMargin {
    fn marker(&self) -> &str {
        match self.current_ma {
            150 => "0.15",
            250 => "0.25",
            500 => "0.5",
            1000 => "1.0",
            _ => "?",
        }
    }

    fn next(&mut self) {
        self.current_ma = match self.current_ma {
            150 => 250,
            250 => 500,
            500 => 1000,
            1000 => 150,
            _ => 150,
        };
    }
}

/// Handle displaying the UI.
#[embassy_executor::task]
pub async fn display_task(mut display_resources: DisplayResources) {
    let persistent = PERSISTENT_MUTEX.lock(|x| *x.borrow());

    let mut bargraph_filter = biquad::DirectForm2Transposed::<f32>::new(
        biquad::Coefficients::<f32>::from_params(
            biquad::Type::LowPass,
            (DISPLAY_REFRESH_RATE_HZ as f32).hz(),
            2.0_f32.hz(),
            biquad::Q_BUTTERWORTH_F32,
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
    let mut menu = Menu::with_style(
        "Setup",
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
        CurrentMargin {
            current_ma: persistent.current_margin_ma,
        },
        MenuResult::CurrentMargin,
    )
    .add_item("Slp on power", persistent.sleep_on_power, |v| {
        MenuResult::SleepOnPower(v)
    })
    .add_item("Slp on error", persistent.sleep_on_error, |v| {
        MenuResult::SleepOnError(v)
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

            if !temperature_deg_c.is_nan() {
                write!(
                    &mut temperature_string,
                    "{}",
                    temperature_deg_c.round() as usize
                )
                .unwrap();
            }
        }

        if let Some(message) = MESSAGE_SIG.try_take() {
            message_string = message;
        }

        if let Some(power) = DISPLAY_POWER_SIG.try_take() {
            power_string.clear();
            if !(power.is_nan()) {
                write!(&mut power_string, "{} W", power.round() as usize).unwrap();
            }
        }

        if let Some(power_ratio) = POWER_RATIO_BARGRAPH_SIG.try_take() {
            let raw = power_ratio * DISPLAY_WIDTH as f32;

            if !raw.is_nan() {
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
                        PERSISTENT_MUTEX.lock(|x| x.borrow_mut().current_margin_ma = c.current_ma);
                        STORE_PERSISTENT_SIG.signal(());
                    }
                    Some(MenuResult::SleepOnPower(v)) => {
                        PERSISTENT_MUTEX.lock(|x| x.borrow_mut().sleep_on_power = v);
                        STORE_PERSISTENT_SIG.signal(());
                    }
                    Some(MenuResult::SleepOnError(v)) => {
                        PERSISTENT_MUTEX.lock(|x| x.borrow_mut().sleep_on_error = v);
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
