mod linuxfb_sys;
mod ion_sys;
mod mxcfb_sys;
mod sunxi_sys;
mod image;
mod transform;
mod kobo1;
mod kobo2;

use anyhow::Error;
use crate::geom::{Point, Rectangle, surface_area, nearest_segment_point, lerp};
use crate::geom::{CornerSpec, BorderSpec, ColorSource, Vec2};
use crate::color::{BLACK, WHITE};

pub use self::kobo1::KoboFramebuffer1;
pub use self::kobo2::KoboFramebuffer2;
pub use self::image::Pixmap;

#[derive(Debug, Copy, Clone)]
pub struct Display {
    pub dims: (u32, u32),
    pub rotation: i8,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum UpdateMode {
    Gui,
    Partial,
    Full,
    Fast,
    FastMono,
}

pub trait Framebuffer {
    fn set_pixel(&mut self, x: u32, y: u32, color: u8);
    fn set_blended_pixel(&mut self, x: u32, y: u32, color: u8, alpha: f32);
    fn invert_region(&mut self, rect: &Rectangle);
    fn shift_region(&mut self, rect: &Rectangle, drift: u8);
    fn update(&mut self, rect: &Rectangle, mode: UpdateMode) -> Result<u32, Error>;
    fn wait(&self, token: u32) -> Result<i32, Error>;
    fn save(&self, path: &str) -> Result<(), Error>;
    fn set_rotation(&mut self, n: i8) -> Result<(u32, u32), Error>;
    fn set_monochrome(&mut self, enable: bool);
    fn set_dithered(&mut self, enable: bool);
    fn set_inverted(&mut self, enable: bool);
    fn monochrome(&self) -> bool;
    fn dithered(&self) -> bool;
    fn inverted(&self) -> bool;
    fn width(&self) -> u32;
    fn height(&self) -> u32;

    fn toggle_inverted(&mut self) {
        self.set_inverted(!self.inverted());
    }

    fn toggle_monochrome(&mut self) {
        self.set_monochrome(!self.monochrome());
    }

    fn toggle_dithered(&mut self) {
        self.set_dithered(!self.dithered());
    }

    fn rotation(&self) -> i8 {
        0
    }

    fn dims(&self) -> (u32, u32) {
        (self.width(), self.height())
    }

    fn rect(&self) -> Rectangle {
        let (width, height) = self.dims();
        rect![0, 0, width as i32, height as i32]
    }

    fn clear(&mut self, color: u8) {
        let rect = self.rect();
        self.draw_rectangle(&rect, color);
    }

    fn draw_rectangle(&mut self, rect: &Rectangle, color: u8) {
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                self.set_pixel(x as u32, y as u32, color);
            }
        }
    }

    fn draw_blended_rectangle(&mut self, rect: &Rectangle, color: u8, alpha: f32) {
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                self.set_blended_pixel(x as u32, y as u32, color, alpha);
            }
        }
    }

    fn draw_rectangle_outline(&mut self, rect: &Rectangle, border: &BorderSpec) {
        let BorderSpec { thickness: border_thickness,
                         color: border_color } = *border;
        self.draw_rectangle(&rect![rect.min.x, rect.min.y,
                                   rect.max.x - border_thickness as i32,
                                   rect.min.y + border_thickness as i32],
                            border_color);
        self.draw_rectangle(&rect![rect.max.x - border_thickness as i32, rect.min.y,
                                   rect.max.x, rect.max.y - border_thickness as i32],
                            border_color);
        self.draw_rectangle(&rect![rect.min.x + border_thickness as i32,
                                   rect.max.y - border_thickness as i32,
                                   rect.max.x, rect.max.y],
                            border_color);
        self.draw_rectangle(&rect![rect.min.x, rect.min.y + border_thickness as i32,
                                   rect.min.x + border_thickness as i32,
                                   rect.max.y],
                            border_color);
    }

    fn draw_pixmap(&mut self, pixmap: &Pixmap, pt: Point) {
        for y in 0..pixmap.height {
            for x in 0..pixmap.width {
                let px = x + pt.x as u32;
                let py = y + pt.y as u32;
                let color = pixmap.get_pixel(x, y);
                self.set_pixel(px, py, color);
            }
        }
    }

    fn draw_framed_pixmap(&mut self, pixmap: &Pixmap, rect: &Rectangle, pt: Point) {
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let px = x - rect.min.x + pt.x;
                let py = y - rect.min.y + pt.y;
                let color = pixmap.get_pixel(x as u32, y as u32);
                self.set_pixel(px as u32, py as u32, color);
            }
        }
    }

    fn draw_framed_pixmap_contrast(&mut self, pixmap: &Pixmap, rect: &Rectangle, pt: Point, exponent: f32, gray: f32) {
        if (exponent - 1.0).abs() < f32::EPSILON {
            self.draw_framed_pixmap(pixmap, rect, pt);
            return;
        }
        let rem_gray = 255.0 - gray;
        let inv_exponent = 1.0 / exponent;
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let px = x - rect.min.x + pt.x;
                let py = y - rect.min.y + pt.y;
                let raw_color = pixmap.get_pixel(x as u32, y as u32) as f32;
                let color = if raw_color < gray {
                    (gray * (raw_color / gray).powf(exponent)) as u8
                } else if raw_color > gray {
                    (gray + rem_gray * ((raw_color - gray) / rem_gray).powf(inv_exponent)) as u8
                } else {
                    gray as u8
                };
                self.set_pixel(px as u32, py as u32, color);
            }
        }
    }

    fn draw_framed_pixmap_halftone(&mut self, pixmap: &Pixmap, rect: &Rectangle, pt: Point) {
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let px = x - rect.min.x + pt.x;
                let py = y - rect.min.y + pt.y;
                let source_color = pixmap.get_pixel(x as u32, y as u32);
                let color = if source_color == BLACK {
                    BLACK
                } else if source_color == WHITE {
                    WHITE
                } else {
                    transform::transform_dither_g2(x as u32, y as u32, source_color)
                };
                self.set_pixel(px as u32, py as u32, color);
            }
        }
    }

    fn draw_blended_pixmap(&mut self, pixmap: &Pixmap, pt: Point, color: u8) {
        for y in 0..pixmap.height {
            for x in 0..pixmap.width {
                let px = x + pt.x as u32;
                let py = y + pt.y as u32;
                let alpha = (255.0 - pixmap.get_pixel(x, y) as f32) / 255.0;
                self.set_blended_pixel(px as u32, py as u32, color, alpha);
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

    fn draw_rounded_rectangle_with_border(&mut self, rect: &Rectangle, corners: &CornerSpec, border: &BorderSpec, color: &dyn ColorSource) {
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
                let mut color = color.color(x, y);
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
                        color = lerp(color as f32, border_color as f32, alpha) as u8;
                        alpha = 1.0;
                    } else {
                        let delta_dist = dist - radius as f32;
                        color = border_color;
                        alpha = surface_area(delta_dist, angle);
                    }
                } else if x < rect.min.x + border_thickness as i32 ||
                          x >= rect.max.x - border_thickness as i32 ||
                          y < rect.min.y + border_thickness as i32 ||
                          y >= rect.max.y - border_thickness as i32 {
                    color = border_color;
                }
                self.set_blended_pixel(x as u32, y as u32, color, alpha);
            }
        }
    }

    fn draw_triangle(&mut self, triangle: &[Point], color: u8) {
        let mut x_min = ::std::i32::MAX;
        let mut x_max = ::std::i32::MIN;
        let mut y_min = ::std::i32::MAX;
        let mut y_max = ::std::i32::MIN;

        for p in triangle.iter() {
            if p.x < x_min {
                x_min = p.x;
            }
            if p.x > x_max {
                x_max = p.x;
            }
            if p.y < y_min {
                y_min = p.y;
            }
            if p.y > y_max {
                y_max = p.y;
            }
        }

        x_max += 1;
        y_max += 1;

        let mut a: Vec2 = triangle[0].into();
        let mut b: Vec2 = triangle[1].into();
        let mut c: Vec2 = triangle[2].into();

        a += 0.5;
        b += 0.5;
        c += 0.5;

        let ab = b - a;
        let ac = c - a;
        let bc = c - b;

        for y in y_min..y_max {
            for x in x_min..x_max {
                let p = vec2!(x as f32 + 0.5, y as f32 + 0.5);
                let ap = p - a;
                let bp = p - b;

                let s_ab = ab.cross(ap).is_sign_positive();
                let inside = ac.cross(ap).is_sign_positive() != s_ab &&
                             bc.cross(bp).is_sign_positive() == s_ab;

                let mut dmin = ::std::f32::MAX;
                let mut nearest = None;

                for &(u, v) in &[(a, b), (b, c), (a, c)] {
                    let (n, _) = nearest_segment_point(p, u, v);
                    let d = (n - p).length();
                    if d < dmin {
                        dmin = d;
                        nearest = Some(n);
                    }
                }

                if let Some(n) = nearest {
                    let angle = (n - p).angle();
                    let delta_dist = if inside { -dmin } else { dmin };
                    let alpha = surface_area(delta_dist, angle);
                    self.set_blended_pixel(x as u32, y as u32, color, alpha);
                }
            }
        }
    }

    fn draw_disk(&mut self, center: Point, radius: i32, color: u8) {
        let rect = Rectangle::from_disk(center, radius);

        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let v = vec2!((x - center.x) as f32, (y - center.y) as f32) + 0.5;
                let angle = v.angle();
                let delta_dist = v.length() - radius as f32;
                let alpha = surface_area(delta_dist, angle);
                self.set_blended_pixel(x as u32, y as u32, color, alpha);
            }
        }
    }

    fn draw_segment(&mut self, start: Point, end: Point, start_radius: f32, end_radius: f32, color: u8) {
        let rect = Rectangle::from_segment(start, end, start_radius.ceil() as i32, end_radius.ceil() as i32);
        let a = vec2!(start.x as f32, start.y as f32) + 0.5;
        let b = vec2!(end.x as f32, end.y as f32) + 0.5;

        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                let p = vec2!(x as f32, y as f32) + 0.5;
                let (n, t) = nearest_segment_point(p, a, b);
                let radius = lerp(start_radius, end_radius, t);
                if (n - p).length() <= radius {
                    self.set_pixel(x as u32, y as u32, color);
                }
            }
        }
    }
}
