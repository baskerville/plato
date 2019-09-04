use std::fs::File;
use failure::{Error, ResultExt, format_err};
use super::{Framebuffer, UpdateMode};
use crate::color::WHITE;
use crate::geom::{Rectangle, lerp};

#[derive(Debug, Clone)]
pub struct Pixmap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Pixmap {
    pub fn new(width: u32, height: u32) -> Pixmap {
        let len = (width * height) as usize;
        Pixmap {
            width,
            height,
            data: vec![WHITE; len],
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

impl Framebuffer for Pixmap {
    fn set_pixel(&mut self, x: u32, y: u32, color: u8) {
        if x >= self.width || y >= self.height {
            return;
        }
        let addr = (y * self.width + x) as usize;
        self.data[addr] = color;
    }

    fn set_blended_pixel(&mut self, x: u32, y: u32, color: u8, alpha: f32) {
        if alpha >= 1.0 {
            self.set_pixel(x, y, color);
            return;
        }
        if x >= self.width || y >= self.height {
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

    fn shift_region(&mut self, rect: &Rectangle, drift: u8) {
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let addr = (y * self.width as i32 + x) as usize;
                let color = self.data[addr].saturating_sub(drift);
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
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_color(png::ColorType::Grayscale);
        let mut writer = encoder.write_header().context("Can't write header.")?;
        writer.write_image_data(&self.data).context("Can't write data to file.")?;
        Ok(())
    }

    fn set_rotation(&mut self, _n: i8) -> Result<(u32, u32), Error> {
        Err(format_err!("Unsupported."))
    }

    fn set_inverted(&mut self, _enable: bool) {
    }

    fn set_monochrome(&mut self, _enable: bool) {
    }

    fn inverted(&self) -> bool {
        false
    }

    fn monochrome(&self) -> bool {
        false
    }

    fn dims(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
