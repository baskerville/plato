use std::cmp;
use std::ops::{Add, AddAssign, Sub, SubAssign, Mul, MulAssign, Div, DivAssign};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Dir {
    North,
    East,
    South,
    West,
}

#[derive(Debug)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Dir {
    pub fn opposite(&self) -> Dir {
        match *self {
            Dir::North => Dir::South,
            Dir::South => Dir::North,
            Dir::East => Dir::West,
            Dir::West => Dir::East,
        }
    }
    pub fn axis(&self) -> Axis {
        match *self {
            Dir::North | Dir::South => Axis::Vertical,
            Dir::East | Dir::West => Axis::Horizontal,
        }
    }
}

impl Point {
    pub fn new(x: i32, y: i32) -> Point {
        Point {
            x: x,
            y: y,
        }
    }
    pub fn dist2(&self, pt: &Point) -> u32 {
        ((pt.x - self.x).pow(2) + (pt.y - self.y).pow(2)) as u32
    }
    pub fn length(&self) -> f32 {
        ((self.x.pow(2) + self.y.pow(2)) as f32).sqrt()
    }
    pub fn angle(&self) -> f32 {
        (-self.y as f32).atan2(self.x as f32)
    }
    pub fn dir(&self) -> Dir {
        if self.x.abs() > self.y.abs() {
            if self.x.is_positive() {
                Dir::East
            } else {
                Dir::West
            }
        } else {
            if self.y.is_positive() {
                Dir::South
            } else {
                Dir::North
            }
        }
    }
}

impl Default for Point {
    fn default() -> Self {
        Point::new(0, 0)
    }
}

impl Into<(f32, f32)> for Point {
    fn into(self) -> (f32, f32) {
        (self.x as f32, self.y as f32)
    }
}

#[macro_export]
macro_rules! pt {
    ($x:expr, $y:expr) => ($crate::geom::Point::new($x, $y));
    ($a:expr) => ($crate::geom::Point::new($a, $a));
}

// Based on https://golang.org/pkg/image/#Rectangle
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Rectangle {
    pub min: Point,
    pub max: Point,
}

impl Rectangle {
    pub fn new(min: Point, max: Point) -> Rectangle {
        Rectangle {
            min: min,
            max: max,
        }
    }
    pub fn from_point(pt: &Point) -> Rectangle {
        Rectangle {
            min: *pt,
            max: *pt + 1,
        }
    }
    pub fn contains(&self, pt: &Point) -> bool {
        self.min.x <= pt.x && pt.x < self.max.x &&
        self.min.y <= pt.y && pt.y < self.max.y
    }
    pub fn overlaps(&self, rect: &Rectangle) -> bool {
        self.min.x < rect.max.x && self.max.x >= rect.min.x &&
        self.min.y < rect.max.y && self.max.y >= rect.min.y
    }
    pub fn merge(&mut self, pt: &Point) {
        if pt.x < self.min.x {
            self.min.x = pt.x;
        }
        if pt.x >= self.max.x {
            self.max.x = pt.x + 1;
        }
        if pt.y < self.min.y {
            self.min.y = pt.y;
        }
        if pt.y >= self.max.y {
            self.max.y = pt.y + 1;
        }
    }
    pub fn absorb(&mut self, rect: &Rectangle) {
        if self.min.x > rect.min.x {
            self.min.x = rect.min.x;
        }
        if self.max.x < rect.max.x {
            self.max.x = rect.max.x;
        }
        if self.min.y > rect.min.y {
            self.min.y = rect.min.y;
        }
        if self.max.y < rect.max.y {
            self.max.y = rect.max.y;
        }
    }
    pub fn intersection(&self, rect: &Rectangle) -> Option<Rectangle> {
        if self.overlaps(rect) {
            Some(Rectangle::new(Point::new(cmp::max(self.min.x, rect.min.x),
                                           cmp::max(self.min.y, rect.min.y)),
                                Point::new(cmp::min(self.max.x, rect.max.x),
                                           cmp::min(self.max.y, rect.max.y))))
        } else {
            None
        }
    }
    #[inline]
    pub fn width(&self) -> u32 {
        (self.max.x - self.min.x) as u32
    }
    #[inline]
    pub fn height(&self) -> u32 {
        (self.max.y - self.min.y) as u32
    }
}

#[macro_export]
macro_rules! rect {
    ($x0:expr, $y0:expr, $x1:expr, $y1:expr) => ($crate::geom::Rectangle::new($crate::geom::Point::new($x0, $y0), $crate::geom::Point::new($x1, $y1)));
    ($min:expr, $max:expr) => ($crate::geom::Rectangle::new($min, $max));
}

impl Add for Point {
    type Output = Point;
    fn add(self, rhs: Point) -> Point {
        Point {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl AddAssign for Point {
    fn add_assign(&mut self, rhs: Point) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub for Point {
    type Output = Point;
    fn sub(self, rhs: Point) -> Point {
        Point {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl SubAssign for Point {
    fn sub_assign(&mut self, rhs: Point) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl Mul<Point> for Point {
    type Output = Point;
    fn mul(self, rhs: Point) -> Point {
        Point {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
        }
    }
}

impl MulAssign<Point> for Point {
    fn mul_assign(&mut self, rhs: Point) {
        self.x *= rhs.x;
        self.y *= rhs.y;
    }
}

impl Div<Point> for Point {
    type Output = Point;
    fn div(self, rhs: Point) -> Point {
        Point {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
        }
    }
}

impl DivAssign<Point> for Point {
    fn div_assign(&mut self, rhs: Point) {
        self.x /= rhs.x;
        self.y /= rhs.y;
    }
}

impl Add<i32> for Point {
    type Output = Point;
    fn add(self, rhs: i32) -> Point {
        Point {
            x: self.x + rhs,
            y: self.y + rhs,
        }
    }
}

impl Add<Point> for i32 {
    type Output = Point;
    fn add(self, rhs: Point) -> Point {
        Point {
            x: self + rhs.x,
            y: self + rhs.y,
        }
    }
}

impl AddAssign<i32> for Point {
    fn add_assign(&mut self, rhs: i32) {
        self.x += rhs;
        self.y += rhs;
    }
}

impl Sub<i32> for Point {
    type Output = Point;
    fn sub(self, rhs: i32) -> Point {
        Point {
            x: self.x - rhs,
            y: self.y - rhs,
        }
    }
}

impl Sub<Point> for i32 {
    type Output = Point;
    fn sub(self, rhs: Point) -> Point {
        Point {
            x: self - rhs.x,
            y: self - rhs.y,
        }
    }
}

impl SubAssign<i32> for Point {
    fn sub_assign(&mut self, rhs: i32) {
        self.x -= rhs;
        self.y -= rhs;
    }
}

impl Mul<i32> for Point {
    type Output = Point;
    fn mul(self, rhs: i32) -> Point {
        Point {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Mul<Point> for i32 {
    type Output = Point;
    fn mul(self, rhs: Point) -> Point {
        Point {
            x: self * rhs.x,
            y: self * rhs.y,
        }
    }
}

impl MulAssign<i32> for Point {
    fn mul_assign(&mut self, rhs: i32) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl Div<i32> for Point {
    type Output = Point;
    fn div(self, rhs: i32) -> Point {
        Point {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

impl Div<Point> for i32 {
    type Output = Point;
    fn div(self, rhs: Point) -> Point {
        Point {
            x: self / rhs.x,
            y: self / rhs.y,
        }
    }
}

impl DivAssign<i32> for Point {
    fn div_assign(&mut self, rhs: i32) {
        self.x /= rhs;
        self.y /= rhs;
    }
}

impl Add<Point> for Rectangle {
    type Output = Rectangle;
    fn add(self, rhs: Point) -> Rectangle {
        Rectangle {
            min: self.min + rhs,
            max: self.max + rhs,
        }
    }
}

impl AddAssign<Point> for Rectangle {
    fn add_assign(&mut self, rhs: Point) {
        self.min += rhs;
        self.max += rhs;
    }
}

impl Sub<Point> for Rectangle {
    type Output = Rectangle;
    fn sub(self, rhs: Point) -> Rectangle {
        Rectangle {
            min: self.min - rhs,
            max: self.max - rhs,
        }
    }
}

impl SubAssign<Point> for Rectangle {
    fn sub_assign(&mut self, rhs: Point) {
        self.min -= rhs;
        self.max -= rhs;
    }
}
