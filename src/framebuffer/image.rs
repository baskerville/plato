extern crate png;

use std::fs::File;
use png::HasParameters;
use framebuffer::{Framebuffer, UpdateMode};
use color::WHITE;
use geom::{Rectangle, lerp};
use failure::{Error, ResultExt};

pub struct ImageFramebuffer {
    width: u32,
    height: u32,
    data: Vec<u8>,
    inverted: bool,
    monochrome: bool,
}

impl ImageFramebuffer {
    pub fn new(width: u32, height: u32) -> ImageFramebuffer {
        let len = (width * height) as usize;
        ImageFramebuffer {
            width,
            height,
            data: vec![WHITE; len],
            inverted: false,
            monochrome: false,
        }
    }
}

#[inline]
fn transform_color(color: u8, inverted: bool, monochrome: bool) -> u8 {
    let color = if inverted {
        255 - color
    } else {
        color
    };
    if monochrome {
        (color > 127) as u8 * 255
    } else {
        color
    }
}

impl Framebuffer for ImageFramebuffer {
    fn set_pixel(&mut self, x: u32, y: u32, color: u8) {
        let addr = (y * self.width + x) as usize;
        self.data[addr] = color;
    }

    fn set_blended_pixel(&mut self, x: u32, y: u32, color: u8, alpha: f32) {
        if alpha == 1.0 {
            self.set_pixel(x, y, color);
            return;
        }
        let addr = (y * self.width + x) as usize;
        let blended_color = lerp(self.data[addr] as f32, color as f32, alpha) as u8;
        self.data[addr] = blended_color;
    }

    fn invert_region(&mut self, rect: &Rectangle) {
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let addr = (y * self.width as i32 + x) as usize;
                let color = 255 - self.data[addr];
                self.data[addr] = color;
            }
        }
    }

    fn update(&mut self, _rect: &Rectangle, _mode: UpdateMode) -> Result<u32, Error> {
        Ok(1)
    }

    fn wait(&self, _: u32) -> Result<i32, Error> {
        Ok(1)
    }

    fn save(&self, path: &str) -> Result<(), Error> {
        let (width, height) = self.dims();
        let file = File::create(path).context("Can't create output file.")?;
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set(png::ColorType::Grayscale).set(png::BitDepth::Eight);
        let mut writer = encoder.write_header().context("Can't write header.")?;
        let data: Vec<u8> = self.data.iter().map(|c| transform_color(*c, self.inverted, self.monochrome)).collect();
        writer.write_image_data(&data).context("Can't write data to file.")?;
        Ok(())
    }

    fn toggle_inverted(&mut self) {
        self.inverted = !self.inverted;
    }

    fn toggle_monochrome(&mut self) {
        self.monochrome = !self.monochrome;
    }

    fn dims(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
