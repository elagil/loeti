use core::f32::consts::PI;

use crate::ui::display::{DISPLAY_REFRESH_RATE_HZ, FILLED_STYLE};

use super::{DISPLAY_HEIGHT, DISPLAY_WIDTH, Display};
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::Point,
    primitives::{Circle, Line, Polyline, PrimitiveStyle, PrimitiveStyleBuilder, StyledDrawable},
};
use micromath::F32Ext;

/// Style for dark filled objects.
const FILLED_STYLE_DARK: PrimitiveStyle<BinaryColor> = PrimitiveStyleBuilder::new()
    .stroke_width(1)
    .fill_color(BinaryColor::Off)
    .stroke_color(BinaryColor::Off)
    .build();

/// Style for thick objects that only have an outline and no fill.
const OUTLINE_STYLE_THICK: PrimitiveStyle<BinaryColor> = PrimitiveStyleBuilder::new()
    .stroke_width(2)
    .fill_color(BinaryColor::Off)
    .stroke_color(BinaryColor::On)
    .build();

/// Style for thick dark objects that only have an outline and no fill.
const OUTLINE_STYLE_THICK_DARK: PrimitiveStyle<BinaryColor> = PrimitiveStyleBuilder::new()
    .stroke_width(6)
    .fill_color(BinaryColor::Off)
    .stroke_color(BinaryColor::Off)
    .build();

impl<'d> Display<'d> {
    pub(super) fn draw_waiting_animation(&mut self) {
        const PERIOD_S: f32 = 2.0;

        let t: f32 =
            2.0 * PI * (self.draw_count as f32 % (PERIOD_S * DISPLAY_REFRESH_RATE_HZ as f32))
                / (PERIOD_S * DISPLAY_REFRESH_RATE_HZ as f32);
        let top_width = 22.0;
        let top_height = 5.0;

        const X_OFFSET: i32 = DISPLAY_WIDTH / 2;
        const Y_OFFSET: i32 = DISPLAY_HEIGHT / 2 - 10;

        {
            // Bottom of the hat.
            let x = 13.0;
            Polyline::new(&[
                Point::new(
                    -x as i32 + X_OFFSET,
                    (top_height * (1.0 - (x * x) / (top_width * top_width)).sqrt()).round() as i32
                        + Y_OFFSET,
                ),
                Point::new(-x as i32 + X_OFFSET, Y_OFFSET + 20),
                Point::new(X_OFFSET, Y_OFFSET + 22),
                Point::new(x as i32 + X_OFFSET, Y_OFFSET + 20),
                Point::new(
                    x as i32 + X_OFFSET,
                    (top_height * (1.0 - (x * x) / (top_width * top_width)).sqrt()).round() as i32
                        + Y_OFFSET,
                ),
            ])
            .draw_styled(&OUTLINE_STYLE_THICK, self.inner)
            .unwrap();
        }

        {
            Line::new(
                Point::new(
                    (top_width * t.cos()).round() as i32 + X_OFFSET,
                    (top_height * t.sin()).round() as i32 + Y_OFFSET,
                ),
                Point::new(
                    -(top_width * t.sin()).round() as i32 + X_OFFSET,
                    (top_height * t.cos()).round() as i32 + Y_OFFSET,
                ),
            )
            .draw_styled(&OUTLINE_STYLE_THICK_DARK, self.inner)
            .unwrap();

            Line::new(
                Point::new(
                    -(top_width * t.sin()).round() as i32 + X_OFFSET,
                    (top_height * t.cos()).round() as i32 + Y_OFFSET,
                ),
                Point::new(
                    -(top_width * t.cos()).round() as i32 + X_OFFSET,
                    -(top_height * t.sin()).round() as i32 + Y_OFFSET,
                ),
            )
            .draw_styled(&OUTLINE_STYLE_THICK_DARK, self.inner)
            .unwrap();

            Line::new(
                Point::new(
                    -(top_width * t.cos()).round() as i32 + X_OFFSET,
                    -(top_height * t.sin()).round() as i32 + Y_OFFSET,
                ),
                Point::new(
                    (top_width * t.sin()).round() as i32 + X_OFFSET,
                    -(top_height * t.cos()).round() as i32 + Y_OFFSET,
                ),
            )
            .draw_styled(&OUTLINE_STYLE_THICK_DARK, self.inner)
            .unwrap();

            Line::new(
                Point::new(
                    (top_width * t.sin()).round() as i32 + X_OFFSET,
                    -(top_height * t.cos()).round() as i32 + Y_OFFSET,
                ),
                Point::new(
                    (top_width * t.cos()).round() as i32 + X_OFFSET,
                    (top_height * t.sin()).round() as i32 + Y_OFFSET,
                ),
            )
            .draw_styled(&OUTLINE_STYLE_THICK_DARK, self.inner)
            .unwrap();
        }

        {
            // Lines for the top of the hat.
            Line::new(
                Point::new(
                    (top_width * t.cos()).round() as i32 + X_OFFSET,
                    (top_height * t.sin()).round() as i32 + Y_OFFSET,
                ),
                Point::new(
                    -(top_width * t.sin()).round() as i32 + X_OFFSET,
                    (top_height * t.cos()).round() as i32 + Y_OFFSET,
                ),
            )
            .draw_styled(&OUTLINE_STYLE_THICK, self.inner)
            .unwrap();

            Line::new(
                Point::new(
                    -(top_width * t.sin()).round() as i32 + X_OFFSET,
                    (top_height * t.cos()).round() as i32 + Y_OFFSET,
                ),
                Point::new(
                    -(top_width * t.cos()).round() as i32 + X_OFFSET,
                    -(top_height * t.sin()).round() as i32 + Y_OFFSET,
                ),
            )
            .draw_styled(&OUTLINE_STYLE_THICK, self.inner)
            .unwrap();

            Line::new(
                Point::new(
                    -(top_width * t.cos()).round() as i32 + X_OFFSET,
                    -(top_height * t.sin()).round() as i32 + Y_OFFSET,
                ),
                Point::new(
                    (top_width * t.sin()).round() as i32 + X_OFFSET,
                    -(top_height * t.cos()).round() as i32 + Y_OFFSET,
                ),
            )
            .draw_styled(&OUTLINE_STYLE_THICK, self.inner)
            .unwrap();

            Line::new(
                Point::new(
                    (top_width * t.sin()).round() as i32 + X_OFFSET,
                    -(top_height * t.cos()).round() as i32 + Y_OFFSET,
                ),
                Point::new(
                    (top_width * t.cos()).round() as i32 + X_OFFSET,
                    (top_height * t.sin()).round() as i32 + Y_OFFSET,
                ),
            )
            .draw_styled(&OUTLINE_STYLE_THICK, self.inner)
            .unwrap();
        }

        {
            // Straw on top of the hat.
            let straw_start = Point::new(
                (top_width / 4.0 * t.cos()).round() as i32 + X_OFFSET,
                (top_height / 4.0 * t.sin()).round() as i32 + Y_OFFSET - 1,
            );
            let straw_end = Point::new(
                (top_width / 2.0 * t.cos()).round() as i32 + X_OFFSET,
                (top_height / 3.0 * t.sin()).round() as i32 + Y_OFFSET - 15,
            );

            Line::new(straw_start, straw_end)
                .draw_styled(&OUTLINE_STYLE_THICK_DARK, self.inner)
                .unwrap();

            Line::new(straw_start, straw_end)
                .draw_styled(&OUTLINE_STYLE_THICK, self.inner)
                .unwrap();

            Circle::with_center(straw_end, 12)
                .draw_styled(&FILLED_STYLE_DARK, self.inner)
                .unwrap();

            Circle::with_center(straw_end, 8)
                .draw_styled(&FILLED_STYLE, self.inner)
                .unwrap();
        }
    }
}
