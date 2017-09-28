mod kobo;
mod image;

use std::path::Path;
use geom::{Point, Rectangle, surface_area, lerp};
use geom::{CornerSpec, BorderSpec};
use errors::*;

pub use self::kobo::KoboFramebuffer;
pub use self::image::ImageFramebuffer;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum UpdateMode {
    Fast,
    Partial,
    Gui,
    Full,
    Clear,
}

#[derive(Debug, Clone)]
pub struct Bitmap {
    pub width: i32,
    pub height: i32,
    pub buf: Vec<u8>,
}

pub trait Framebuffer {
    fn set_pixel(&mut self, x: u32, y: u32, color: u8);
    fn set_blended_pixel(&mut self, x: u32, y: u32, color: u8, alpha: f32);
    fn update(&mut self, rect: &Rectangle, mode: UpdateMode) -> Result<u32>;
    fn wait(&self, token: u32) -> Result<i32>;
    fn save(&self, path: &str) -> Result<()>;

    fn width(&self) -> u32 {
        let (width, _) = self.dims();
        width
    }

    fn height(&self) -> u32 {
        let (_, height) = self.dims();
        height
    }

    fn dims(&self) -> (u32, u32) {
        (self.width(), self.height())
    }

    fn rect(&self) -> Rectangle {
        let (width, height) = self.dims();
        rect![0, 0, width as i32, height as i32]
    }

    fn draw_rectangle(&mut self, rect: &Rectangle, color: u8) {
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                self.set_pixel(x as u32, y as u32, color);
            }
        }
    }

    fn draw_blended_bitmap(&mut self, bitmap: &Bitmap, pt: &Point, color: u8) {
        for y in 0..bitmap.height {
            for x in 0..bitmap.width {
                let px = x + pt.x;
                let py = y + pt.y;
                let addr = (y * bitmap.width + x) as usize;
                let alpha = (255.0 - bitmap.buf[addr] as f32) / 255.0;
                self.set_blended_pixel(px as u32, py as u32, color, alpha);
            }
        }
    }

    fn draw_bitmap(&mut self, bitmap: &Bitmap, pt: &Point) {
        for y in 0..bitmap.height {
            for x in 0..bitmap.width {
                let px = x + pt.x;
                let py = y + pt.y;
                let addr = (y * bitmap.width + x) as usize;
                let color = bitmap.buf[addr];
                self.set_pixel(px as u32, py as u32, color);
            }
        }
    }

    fn draw_rounded_rectangle(&mut self, rect: &Rectangle, corners: &CornerSpec, color: u8) {
        let (nw, ne, se, sw) = match *corners {
            CornerSpec::Uniform(v) => (v, v, v, v),
            CornerSpec::North(v) => (v, v, 0, 0),
            CornerSpec::East(v) => (0, v, v, 0),
            CornerSpec::South(v) => (0, 0, v, v),
            CornerSpec::West(v) => (v, 0, 0, v),
            CornerSpec::Detailed {
                north_west,
                north_east,
                south_east,
                south_west
            } => (north_west, north_east, south_east, south_west),
        };
        let nw_c = rect.min + nw;
        let ne_c = pt!(rect.max.x - ne, rect.min.y + ne);
        let se_c = rect.max - se;
        let sw_c = pt!(rect.min.x + sw, rect.max.y - sw);
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let mut alpha = 1.0;
                let mut pole = None;
                if x < nw_c.x && y < nw_c.y {
                    pole = Some((nw_c, nw));
                } else if x >= ne_c.x && y < ne_c.y {
                    pole = Some((ne_c, ne));
                } else if x >= se_c.x && y >= se_c.y {
                    pole = Some((se_c, se));
                } else if x < sw_c.x && y >= sw_c.y {
                    pole = Some((sw_c, sw));
                }
                if let Some((center, radius)) = pole {
                    let v = vec2!((x - center.x) as f32, (y - center.y) as f32) + 0.5;
                    let angle = v.angle();
                    let dist = v.length() - radius as f32;
                    alpha = surface_area(dist, angle);
                }
                self.set_blended_pixel(x as u32, y as u32, color, alpha);
            }
        }
    }

    fn draw_rounded_rectangle_with_border(&mut self, rect: &Rectangle, corners: &CornerSpec, border: &BorderSpec, color: &Fn(i32, i32) -> u8) {
        let (nw, ne, se, sw) = match *corners {
            CornerSpec::Uniform(v) => (v, v, v, v),
            CornerSpec::North(v) => (v, v, 0, 0),
            CornerSpec::East(v) => (0, v, v, 0),
            CornerSpec::South(v) => (0, 0, v, v),
            CornerSpec::West(v) => (v, 0, 0, v),
            CornerSpec::Detailed {
                north_west,
                north_east,
                south_east,
                south_west
            } => (north_west, north_east, south_east, south_west),
        };

        let BorderSpec { thickness: border_thickness,
                         color: border_color } = *border;
        let nw_c = rect.min + nw;
        let ne_c = pt!(rect.max.x - ne, rect.min.y + ne);
        let se_c = rect.max - se;
        let sw_c = pt!(rect.min.x + sw, rect.max.y - sw);

        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let mut alpha = 1.0;
                let mut pole = None;
                let mut color = color(x, y);
                if x < nw_c.x && y < nw_c.y {
                    pole = Some((nw_c, nw));
                } else if x >= ne_c.x && y < ne_c.y {
                    pole = Some((ne_c, ne));
                } else if x >= se_c.x && y >= se_c.y {
                    pole = Some((se_c, se));
                } else if x < sw_c.x && y >= sw_c.y {
                    pole = Some((sw_c, sw));
                }
                if let Some((center, radius)) = pole {
                    let small_radius = radius - border_thickness as i32;
                    let mid_radius = 0.5 * (radius as f32 + small_radius as f32);
                    let v = vec2!((x - center.x) as f32, (y - center.y) as f32) + 0.5;
                    let angle = v.angle();
                    let dist = v.length();
                    if dist < mid_radius {
                        let delta_dist = small_radius as f32 - dist;
                        alpha = surface_area(delta_dist, angle);
                        color = lerp(color, border_color, alpha);
                        alpha = 1.0;
                    } else {
                        let delta_dist = dist - radius as f32;
                        color = border_color;
                        alpha = surface_area(delta_dist, angle);
                    }
                } else {
                    if x < rect.min.x + border_thickness as i32 ||
                       x >= rect.max.x - border_thickness as i32 ||
                       y < rect.min.y + border_thickness as i32 ||
                       y >= rect.max.y - border_thickness as i32 {
                        color = border_color;
                    }
                }
                self.set_blended_pixel(x as u32, y as u32, color, alpha);
            }
        }
    }

    fn draw_disk(&mut self, center: &Point, radius: i32, color: u8) {
        let rect = Rectangle::from_disk(center, radius);

        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let pt = Point::new(x, y);
                let v = vec2!((x - center.x) as f32, (y - center.y) as f32);
                let angle = v.angle();
                let dist = v.length() - radius as f32;
                let area = surface_area(dist, angle);
                self.set_blended_pixel(x as u32, y as u32, color, area);
            }
        }
    }
}
