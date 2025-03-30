//! Controls the display of the soldering controller.
use core::fmt::Write;
use embassy_stm32::spi::Spi;
use embassy_stm32::{gpio::Output, mode::Async};
use embassy_time::{Duration, Ticker};
use embedded_graphics::mono_font::iso_8859_1::{FONT_10X20, FONT_5X7, FONT_6X13};
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
use ssd1306::prelude::{Brightness, DisplayRotation, DisplaySize128x64, SPIInterface};
use ssd1306::Ssd1306;

use crate::{
    PERSISTENT, POWER_BARGRAPH_SIG, POWER_MEASUREMENT_W_SIG, TEMPERATURE_MEASUREMENT_DEG_C_SIG, TOOL_NAME_SIG,
};

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
    let spi_interface = SPIInterface::new(
        display_resources.spi,
        display_resources.pin_dc,
        display_resources.pin_cs,
    );
    let mut display =
        Ssd1306::new(spi_interface, DisplaySize128x64, DisplayRotation::Rotate0).into_buffered_graphics_mode();

    display
        .reset(&mut display_resources.pin_reset, &mut embassy_time::Delay {})
        .unwrap();
    display
        .init_with_addr_mode(ssd1306::command::AddrMode::Horizontal)
        .unwrap();
    display.set_brightness(Brightness::BRIGHTEST).unwrap();

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
    let mut tool_name_string: &str = "";

    let mut ticker = Ticker::every(Duration::from_hz(10));

    loop {
        let persistent = PERSISTENT.lock(|x| *x.borrow());
        set_temperature_string.clear();
        write!(&mut set_temperature_string, "{} °C", persistent.set_temperature_deg_c).unwrap();

        if let Some(temperature_deg_c) = TEMPERATURE_MEASUREMENT_DEG_C_SIG.try_take() {
            temperature_string.clear();

            if !temperature_deg_c.is_nan() {
                write!(&mut temperature_string, "{} °C", temperature_deg_c.round() as usize).unwrap();
            }
        }

        if let Some(name) = TOOL_NAME_SIG.try_take() {
            tool_name_string = name;
        }

        if let Some(power) = POWER_MEASUREMENT_W_SIG.try_take() {
            power_string.clear();
            if !(power.is_nan()) {
                write!(&mut power_string, "{} W", power.round() as usize).unwrap();
            }
        }

        if let Some(power_ratio) = POWER_BARGRAPH_SIG.try_take() {
            power_bar_width = ((power_ratio * 112.0) as i32).max(0);
        }

        display.clear_buffer();

        let set_temp_y = 10;
        let set_temperature_triangle = Triangle::new(
            Point::new(15, set_temp_y - 8),
            Point::new(15, set_temp_y),
            Point::new(19, set_temp_y - 4),
        );

        if persistent.set_temperature_pending {
            set_temperature_triangle
                .draw_styled(&outline_style, &mut display)
                .unwrap();
        } else {
            set_temperature_triangle
                .draw_styled(&filled_style, &mut display)
                .unwrap();
        }

        Text::new(
            &temperature_string,
            Point::new(15, 36),
            MonoTextStyle::new(&FONT_10X20, BinaryColor::On),
        )
        .draw(&mut display)
        .unwrap();

        Text::new(
            &set_temperature_string,
            Point::new(23, set_temp_y),
            MonoTextStyle::new(&FONT_6X13, BinaryColor::On),
        )
        .draw(&mut display)
        .unwrap();

        Text::new(
            tool_name_string,
            Point::new(15, 59),
            MonoTextStyle::new(&FONT_5X7, BinaryColor::On),
        )
        .draw(&mut display)
        .unwrap();

        Text::with_alignment(
            &power_string,
            Point::new(112, 59),
            MonoTextStyle::new(&FONT_5X7, BinaryColor::On),
            Alignment::Right,
        )
        .draw(&mut display)
        .unwrap();

        Rectangle::new(Point::new(15, 62), Size::new(power_bar_width as u32, 2))
            .draw_styled(&filled_style, &mut display)
            .unwrap();

        display.flush().unwrap();

        ticker.next().await;
    }
}
