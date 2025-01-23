use lazy_static::lazy_static;
use super::image::Pixmap;
use crate::color::Color;

pub type ColorTransform = fn(u32, u32, Color) -> Color;

const DITHER_PITCH: u32 = 128;

lazy_static! {
    // Tileable blue noise matrix.
    pub static ref DITHER_G16_DRIFTS: Vec<i8> = {
        let pixmap = Pixmap::from_png("resources/blue_noise-128.png").unwrap();
        // The gap between two succesive colors in G16 is 17.
        // Map {0 .. 255} to {-8 .. 8}.
        pixmap.data().iter().map(|&v| {
            match v {
                  0..=119 => v as i8 / 15 - 8,
                      120 => 0,
                121..=255 => ((v - 121) / 15) as i8,
            }
        }).collect()
    };

    // Tileable blue noise matrix.
    pub static ref DITHER_G2_DRIFTS: Vec<i8> = {
        let pixmap = Pixmap::from_png("resources/blue_noise-128.png").unwrap();
        // Map {0 .. 255} to {-128 .. 127}.
        pixmap.data().iter().map(|&v| {
            match v {
                  0..=127 => -128 + (v as i8),
                128..=255 => (v - 128) as i8,
            }
        }).collect()
    };
}

// Ordered dithering.
// The input color is in {0 .. 255}.
// The output color is in G16.
// G16 := {17 * i | i ∈ {0 .. 15}}.
pub fn transform_dither_g16(x: u32, y: u32, color: Color) -> Color {
    let gray = color.gray();
    // Get the address of the drift value.
    let addr = (x % DITHER_PITCH) + (y % DITHER_PITCH) * DITHER_PITCH;
    // Apply the drift to the input color.
    let c = (gray as i16 + DITHER_G16_DRIFTS[addr as usize] as i16).clamp(0, 255);
    // Compute the distance to the previous color in G16.
    let d = c % 17;
    // Return the nearest color in G16.
    Color::Gray(if d < 9 {
        (c - d) as u8
    } else {
        (c + (17 - d)) as u8
    })
}

// Ordered dithering.
// The input color is in {0 .. 255}.
// The output color is in {0, 255}.
pub fn transform_dither_g2(x: u32, y: u32, color: Color) -> Color {
    let gray = color.gray();
    // Get the address of the drift value.
    let addr = (x % DITHER_PITCH) + (y % DITHER_PITCH) * DITHER_PITCH;
    // Apply the drift to the input color.
    let c = (gray as i16 + DITHER_G2_DRIFTS[addr as usize] as i16).clamp(0, 255);
    // Return the nearest color in G2.
    Color::Gray(if c < 128 {
        0
    } else {
        255
    })
}

pub fn transform_identity(_x: u32, _y: u32, color: Color) -> Color {
    color
}
