use std::fs::File;
use std::path::Path;
use anyhow::{Error, Context, format_err};
use super::{Framebuffer, UpdateMode};
use crate::color::{Color, WHITE};
use crate::geom::{Rectangle, lerp};

#[derive(Debug, Clone)]
pub struct Pixmap {
    pub width: u32,
    pub height: u32,
    pub samples: usize,
    pub data: Vec<u8>,
}

impl Pixmap {
    pub fn new(width: u32, height: u32, samples: usize) -> Pixmap {
        let len = samples * (width * height) as usize;
        Pixmap {
            width,
            height,
            samples,
            data: vec![WHITE.gray(); len],
        }
    }

    pub fn try_new(width: u32, height: u32, samples: usize) -> Option<Pixmap> {
        let mut data = Vec::new();
        let len = samples * (width * height) as usize;
        data.try_reserve_exact(len).ok()?;
        data.resize(len, WHITE.gray());
        Some(Pixmap {
            width,
            height,
            samples,
            data,
        })
    }

    pub fn empty(width: u32, height: u32, samples: usize) -> Pixmap {
        Pixmap {
            width,
            height,
            samples,
            data: Vec::new(),
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn from_png<P: AsRef<Path>>(path: P) -> Result<Pixmap, Error> {
        let file = File::open(path.as_ref())?;
        let decoder = png::Decoder::new(file);
        let mut reader = decoder.read_info()?;
        let info = reader.info();
        let mut pixmap = Pixmap::new(info.width, info.height, info.color_type.samples());
        reader.next_frame(pixmap.data_mut())?;
        Ok(pixmap)
    }

    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> Color {
        if self.data.is_empty() {
            return WHITE;
        }
        let addr = self.samples * (y * self.width + x) as usize;
        if self.samples == 1 {
            Color::Gray(self.data[addr])
        } else {
            Color::from_rgb(&self.data[addr..addr+3])
        }
    }
}

impl Framebuffer for Pixmap {
    fn set_pixel(&mut self, x: u32, y: u32, color: Color) {
        if x >= self.width || y >= self.height {
            return;
        }
        if self.data.is_empty() {
            return;
        }
        let addr = self.samples * (y * self.width + x) as usize;
        if self.samples == 1 {
            self.data[addr] = color.gray();
        } else {
            let rgb = color.rgb();
            self.data[addr..addr+3].copy_from_slice(&rgb);
        }
    }

    fn set_blended_pixel(&mut self, x: u32, y: u32, color: Color, alpha: f32) {
        if alpha >= 1.0 {
            self.set_pixel(x, y, color);
            return;
        }
        if x >= self.width || y >= self.height {
            return;
        }
        if self.data.is_empty() {
            return;
        }
        let addr = self.samples * (y * self.width + x) as usize;
        if self.samples == 1 {
            self.data[addr] = lerp(self.data[addr] as f32, color.gray() as f32, alpha) as u8;
        } else {
            let rgb = color.rgb();
            for (i, c) in self.data[addr..addr+3].iter_mut().enumerate() {
                *c = lerp(*c as f32, rgb[i] as f32, alpha) as u8;
            }
        }
    }

    fn invert_region(&mut self, rect: &Rectangle) {
        if self.data.is_empty() {
            return;
        }
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let addr = self.samples * (y * self.width as i32 + x) as usize;
                if self.samples == 1 {
                    self.data[addr] = 255 - self.data[addr];
                } else {
                    for c in self.data[addr..addr+3].iter_mut() {
                        *c = 255 - *c;
                    }
                }
            }
        }
    }

    fn shift_region(&mut self, rect: &Rectangle, drift: u8) {
        if self.data.is_empty() {
            return;
        }
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let addr = self.samples * (y * self.width as i32 + x) as usize;
                if self.samples == 1 {
                    self.data[addr] = self.data[addr].saturating_sub(drift);
                } else {
                    for c in self.data[addr..addr+3].iter_mut() {
                        *c = c.saturating_sub(drift);
                    }
                }
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
        if self.data.is_empty() {
            return Err(format_err!("nothing to save"));
        }
        let (width, height) = self.dims();
        let file = File::create(path).with_context(|| format!("can't create output file {}", path))?;
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_color(if self.samples == 3 { png::ColorType::Rgb } else { png::ColorType::Grayscale });
        let mut writer = encoder.write_header().with_context(|| format!("can't write PNG header for {}", path))?;
        writer.write_image_data(&self.data).with_context(|| format!("can't write PNG data to {}", path))?;
        Ok(())
    }

    fn set_rotation(&mut self, _n: i8) -> Result<(u32, u32), Error> {
        Err(format_err!("unsupported"))
    }

    fn set_monochrome(&mut self, _enable: bool) {
    }

    fn set_dithered(&mut self, _enable: bool) {
    }

    fn set_inverted(&mut self, _enable: bool) {
    }

    fn monochrome(&self) -> bool {
        false
    }

    fn dithered(&self) -> bool {
        false
    }

    fn inverted(&self) -> bool {
        false
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}
