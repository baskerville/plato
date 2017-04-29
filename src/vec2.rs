use std::ops::{Add, AddAssign, Sub, SubAssign, Mul, MulAssign, Div, DivAssign};
use std::f64::consts;
use std::f64;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub fn new(x: f64, y: f64) -> Vec2 {
        Vec2 { x: x, y: y }
    }
    pub fn dot(&self, other: Vec2) -> f64 {
        self.x * other.x + self.y * other.y
    }
    pub fn cross(&self, other: Vec2) -> f64 {
        self.x * other.y - self.y * other.x
    }
    pub fn length(&self) -> f64 {
        self.x.hypot(self.y)
    }
    pub fn normalize(&self) -> Vec2 {
        Self::unit(self.angle())
    }
    pub fn angle(&self) -> f64 {
        let mut a = self.y.atan2(self.x);
        if a < 0.0 {
            a += 2.0 * consts::PI;
        }
        a
    }
    pub fn unit(angle: f64) -> Vec2 {
        Vec2 {
            x: angle.cos(),
            y: angle.sin(),
        }
    }

    pub fn rotate(&self, angle: f64) -> Vec2 {
        self.length() * Self::unit(self.angle() + angle)
    }
}

impl Default for Vec2 {
    fn default() -> Self {
        Vec2 {x: 0.0, y: 0.0}
    }
}

#[macro_export]
macro_rules! vec2 {
    ($x:expr, $y:expr) => (Vec2::new($x, $y));
    ($x:expr) => (Vec2::new($x, $x));
}

impl Into<[f64; 2]> for Vec2 {
    fn into(self) -> [f64; 2] {
        [self.x, self.y]
    }
}

impl Into<[f32; 2]> for Vec2 {
    fn into(self) -> [f32; 2] {
        [self.x as f32, self.y as f32]
    }
}

impl Into<(i32, i32)> for Vec2 {
    fn into(self) -> (i32, i32) {
        (self.x as i32, self.y as i32)
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Vec2) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl SubAssign for Vec2 {
    fn sub_assign(&mut self, rhs: Vec2) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl Mul<Vec2> for Vec2 {
    type Output = Vec2;
    fn mul(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
        }
    }
}

impl MulAssign<Vec2> for Vec2 {
    fn mul_assign(&mut self, rhs: Vec2) {
        self.x *= rhs.x;
        self.y *= rhs.y;
    }
}

impl Div<Vec2> for Vec2 {
    type Output = Vec2;
    fn div(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
        }
    }
}

impl DivAssign<Vec2> for Vec2 {
    fn div_assign(&mut self, rhs: Vec2) {
        self.x /= rhs.x;
        self.y /= rhs.y;
    }
}

impl Add<f64> for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: f64) -> Vec2 {
        Vec2 {
            x: self.x + rhs,
            y: self.y + rhs,
        }
    }
}

impl Add<Vec2> for f64 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self + rhs.x,
            y: self + rhs.y,
        }
    }
}

impl AddAssign<f64> for Vec2 {
    fn add_assign(&mut self, rhs: f64) {
        self.x += rhs;
        self.y += rhs;
    }
}

impl Sub<f64> for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: f64) -> Vec2 {
        Vec2 {
            x: self.x - rhs,
            y: self.y - rhs,
        }
    }
}

impl Sub<Vec2> for f64 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self - rhs.x,
            y: self - rhs.y,
        }
    }
}

impl SubAssign<f64> for Vec2 {
    fn sub_assign(&mut self, rhs: f64) {
        self.x -= rhs;
        self.y -= rhs;
    }
}

impl Mul<f64> for Vec2 {
    type Output = Vec2;
    fn mul(self, rhs: f64) -> Vec2 {
        Vec2 {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Mul<Vec2> for f64 {
    type Output = Vec2;
    fn mul(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self * rhs.x,
            y: self * rhs.y,
        }
    }
}

impl MulAssign<f64> for Vec2 {
    fn mul_assign(&mut self, rhs: f64) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl Div<f64> for Vec2 {
    type Output = Vec2;
    fn div(self, rhs: f64) -> Vec2 {
        Vec2 {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

impl Div<Vec2> for f64 {
    type Output = Vec2;
    fn div(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self / rhs.x,
            y: self / rhs.y,
        }
    }
}

impl DivAssign<f64> for Vec2 {
    fn div_assign(&mut self, rhs: f64) {
        self.x /= rhs;
        self.y /= rhs;
    }
}

#[cfg(test)]
mod tests {
    use std::f64;
    use std::f64::consts;
    use super::*;

    #[test]
    fn test_unit() {
        let v = Vec2::unit(consts::PI / 2.0);
        assert!(v.x.abs() < f64::EPSILON);
        assert!((v.y - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_angle() {
        let v = Vec2::new(0.0, 1.0);
        assert!((v.angle() - consts::PI / 2.0) < f64::EPSILON);
    }
}
