use embassy_stm32::spi::Spi;
use embassy_stm32::{gpio::Output, mode::Async};
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Circle, PrimitiveStyleBuilder, Rectangle, Triangle},
};
use panic_probe as _;
use ssd1306::prelude::{DisplayRotation, DisplaySize128x64, SPIInterface};
use ssd1306::Ssd1306;

pub struct DisplayResources {
    pub spi: Spi<'static, Async>,
    pub pin_dc: Output<'static>,
    pub pin_reset: Output<'static>,
    pub pin_cs: Output<'static>,
}

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

    let yoffset = 20;

    let style = PrimitiveStyleBuilder::new()
        .stroke_width(1)
        .stroke_color(BinaryColor::On)
        .build();

    // screen outline
    // default display size is 128x64 if you don't pass a _DisplaySize_
    // enum to the _Builder_ struct
    Rectangle::new(Point::new(0, 0), Size::new(127, 63))
        .into_styled(style)
        .draw(&mut display)
        .unwrap();

    // triangle
    Triangle::new(
        Point::new(16, 16 + yoffset),
        Point::new(16 + 16, 16 + yoffset),
        Point::new(16 + 8, yoffset),
    )
    .into_styled(style)
    .draw(&mut display)
    .unwrap();

    // square
    Rectangle::new(Point::new(52, yoffset), Size::new_equal(16))
        .into_styled(style)
        .draw(&mut display)
        .unwrap();

    // circle
    Circle::new(Point::new(88, yoffset), 16)
        .into_styled(style)
        .draw(&mut display)
        .unwrap();

    display.flush().unwrap();
}
