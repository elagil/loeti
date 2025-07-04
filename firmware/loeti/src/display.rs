//! Controls the display of the soldering controller.
use core::fmt::Write;
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
use micromath::F32Ext;
use panic_probe as _;
use profont::{PROFONT_14_POINT, PROFONT_24_POINT, PROFONT_9_POINT};
use ssd1306::prelude::{Brightness, DisplayRotation, DisplaySize128x64, SPIInterface};
use ssd1306::Ssd1306Async;

use crate::{
    MESSAGE_SIG, OPERATIONAL_STATE_MUTEX, PERSISTENT_MUTEX, POWER_BARGRAPH_SIG, POWER_MEASUREMENT_SIG,
    TEMPERATURE_MEASUREMENT_DEG_C_SIG,
};

/// Display width in pixels (internal buffer size).
const DISPLAY_BUFFER_WIDTH: i32 = 128;
/// Display width in pixels (shown).
const DISPLAY_WIDTH: i32 = 102;
/// Empty display area (outside of physical display).
const DISPLAY_MARGIN_X: i32 = (DISPLAY_BUFFER_WIDTH - DISPLAY_WIDTH) / 2;
/// The first visible column's index in pixels.
const DISPLAY_FIRST_COL_INDEX: i32 = DISPLAY_MARGIN_X;
/// The last visible column's index in pixels.
const DISPLAY_LAST_COL_INDEX: i32 = DISPLAY_BUFFER_WIDTH - DISPLAY_MARGIN_X - 1;

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

/// Handle displaying the UI.
#[embassy_executor::task]
pub async fn display_task(mut display_resources: DisplayResources) {
    let spi =
        embedded_hal_bus::spi::ExclusiveDevice::new_no_delay(display_resources.spi, display_resources.pin_cs).unwrap();
    let interface = SPIInterface::new(spi, display_resources.pin_dc);
    let mut display =
        Ssd1306Async::new(interface, DisplaySize128x64, DisplayRotation::Rotate180).into_buffered_graphics_mode();

    display
        .reset(&mut display_resources.pin_reset, &mut embassy_time::Delay {})
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
    let mut voltage_string: heapless::String<10> = heapless::String::new();
    let mut message_string: &str = "";

    let mut refresh_ticker = Ticker::every(Duration::from_hz(20));

    loop {
        let operational_state = OPERATIONAL_STATE_MUTEX.lock(|x| *x.borrow());
        let persistent = PERSISTENT_MUTEX.lock(|x| *x.borrow());

        set_temperature_string.clear();
        write!(&mut set_temperature_string, "{}", persistent.set_temperature_deg_c).unwrap();

        if let Some(temperature_deg_c) = TEMPERATURE_MEASUREMENT_DEG_C_SIG.try_take() {
            temperature_string.clear();

            if !temperature_deg_c.is_nan() {
                if temperature_deg_c < 50.0 {
                    write!(&mut temperature_string, "low").unwrap();
                } else {
                    write!(&mut temperature_string, "{}", temperature_deg_c.round() as usize).unwrap();
                }
            }
        }

        if let Some(message) = MESSAGE_SIG.try_take() {
            message_string = message;
        }

        if let Some((power, voltage)) = POWER_MEASUREMENT_SIG.try_take() {
            power_string.clear();
            if operational_state.manual_sleep {
                write!(&mut power_string, "sleep").unwrap();
            } else {
                if !(power.is_nan()) {
                    write!(&mut power_string, "{} W", power.round() as usize).unwrap();
                }
            }

            voltage_string.clear();
            if !(voltage.is_nan()) {
                write!(&mut voltage_string, "{} V", voltage.round() as usize).unwrap();
            }
        }

        if let Some(power_ratio) = POWER_BARGRAPH_SIG.try_take() {
            power_bar_width = ((power_ratio * DISPLAY_WIDTH as f32) as i32).max(0);
        }

        display.clear_buffer();

        const SET_TEMP_ARROW_Y: i32 = 10;
        const SET_TEMP_Y: i32 = 11;
        const ARROW_WIDTH: i32 = 4;

        let set_temperature_triangle = Triangle::new(
            Point::new(DISPLAY_FIRST_COL_INDEX, SET_TEMP_ARROW_Y - 2 * ARROW_WIDTH),
            Point::new(DISPLAY_FIRST_COL_INDEX, SET_TEMP_ARROW_Y),
            Point::new(DISPLAY_FIRST_COL_INDEX + ARROW_WIDTH, SET_TEMP_ARROW_Y - ARROW_WIDTH),
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

        Text::with_alignment(
            &temperature_string,
            Point::new(DISPLAY_LAST_COL_INDEX, 30),
            MonoTextStyle::new(&PROFONT_24_POINT, BinaryColor::On),
            Alignment::Right,
        )
        .draw(&mut display)
        .unwrap();

        Text::new(
            &set_temperature_string,
            Point::new(DISPLAY_FIRST_COL_INDEX + 2 * ARROW_WIDTH, SET_TEMP_Y),
            MonoTextStyle::new(&PROFONT_14_POINT, BinaryColor::On),
        )
        .draw(&mut display)
        .unwrap();

        Text::new(
            message_string,
            Point::new(DISPLAY_FIRST_COL_INDEX, 45),
            MonoTextStyle::new(&PROFONT_9_POINT, BinaryColor::On),
        )
        .draw(&mut display)
        .unwrap();

        Rectangle::new(
            Point::new(DISPLAY_FIRST_COL_INDEX, 49),
            Size::new(power_bar_width as u32, 2),
        )
        .draw_styled(&filled_style, &mut display)
        .unwrap();

        Text::with_alignment(
            &voltage_string,
            Point::new(DISPLAY_FIRST_COL_INDEX, 60),
            MonoTextStyle::new(&PROFONT_9_POINT, BinaryColor::On),
            Alignment::Left,
        )
        .draw(&mut display)
        .unwrap();

        Text::with_alignment(
            &power_string,
            Point::new(DISPLAY_LAST_COL_INDEX, 60),
            MonoTextStyle::new(&PROFONT_9_POINT, BinaryColor::On),
            Alignment::Right,
        )
        .draw(&mut display)
        .unwrap();

        display.flush().await.unwrap();

        refresh_ticker.next().await;
    }
}
