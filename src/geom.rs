use std::cmp::{self, Ordering};
use std::f32::consts;
use std::ops::{Add, AddAssign, Sub, SubAssign, Mul, MulAssign, Div, DivAssign};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Dir {
    North,
    East,
    South,
    West,
}

#[derive(Debug, Copy, Clone)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Copy, Clone)]
pub enum CycleDir {
    Next,
    Previous,
}

#[derive(Debug, Copy, Clone)]
pub enum LinearDir {
    Backward,
    Forward,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Copy, Clone)]
pub enum CornerSpec {
    Uniform(i32),
    North(i32),
    East(i32),
    South(i32),
    West(i32),
    Detailed {
        north_west: i32,
        north_east: i32,
        south_east: i32,
        south_west: i32,
    }
}

const HALF_PIXEL_DIAGONAL: f32 = consts::SQRT_2 / 2.0;

// Takes the (signed) distance and angle from the center of a pixel to the closest point on a
// shape's boundary and returns the approximate shape area contained within that pixel (the
// boundary is considered flat at the pixel level).
pub fn surface_area(dist: f32, angle: f32) -> f32 {
    // Clearly {in,out}side of the shape.
    if dist.abs() > HALF_PIXEL_DIAGONAL {
        if dist.is_sign_positive() {
            return 0.0;
        } else {
            return 1.0;
        }
    }
    // If the boundary is parallel to the pixel's diagonals then the area is proportional to `dist²`.
    // If the boundary is parallel to the pixel's sides then the area is proportional to `dist`.
    // Hence we compute an interpolated exponent `expo` (`1 <= expo <= 2`) based on `angle`.
    let expo = 0.5 * (3.0 - (4.0 * angle).cos());
    // The *radius* of the pixel for the given *angle*
    let radius = 0.5 * expo.sqrt();
    if dist.is_sign_positive() {
        (radius - dist).max(0.0).powf(expo)
    } else {
        1.0 - (radius + dist).max(0.0).powf(expo)
    }
}

#[inline]
pub fn halves(n: i32) -> (i32, i32) {
    let small_half = n / 2;
    let big_half = n - small_half;
    (small_half, big_half)
}

#[inline]
pub fn small_half(n: i32) -> i32 {
    n / 2
}

#[inline]
pub fn big_half(n: i32) -> i32 {
    n - small_half(n)
}

pub fn divide(n: i32, p: i32) -> Vec<i32> {
    let k = n.checked_div(p).unwrap_or(0);
    let mut r = n - p * k;
    let e = p.checked_div(r).unwrap_or(0);
    let mut vec = Vec::with_capacity(p as usize);
    for i in 0..p {
        if r > 0 && (i+1) % e == 0 {
            vec.push(k + 1);
            r -= 1;
        } else {
            vec.push(k);
        }
    }
    vec
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

impl Into<Vec2> for Point {
    fn into(self) -> Vec2 {
        Vec2::new(self.x as f32, self.y as f32)
    }
}

#[macro_export]
macro_rules! pt {
    ($x:expr, $y:expr $(,)* ) => ($crate::geom::Point::new($x, $y));
    ($a:expr) => ($crate::geom::Point::new($a, $a));
}

#[derive(Debug, Copy, Clone)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[macro_export]
macro_rules! vec2 {
    ($x:expr, $y:expr $(,)* ) => ($crate::geom::Vec2::new($x, $y));
    ($a:expr) => ($crate::geom::Vec2::new($a, $a));
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Vec2 {
        Vec2 {
            x: x,
            y: y,
        }
    }
    pub fn length(&self) -> f32 {
        self.x.hypot(self.y)
    }
    pub fn angle(&self) -> f32 {
        (-self.y).atan2(self.x)
    }
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

    pub fn from_disk(center: &Point, radius: i32) -> Rectangle {
        Rectangle {
            min: *center - radius,
            max: *center + radius,
        }
    }

    pub fn includes(&self, pt: &Point) -> bool {
        self.min.x <= pt.x && pt.x < self.max.x &&
        self.min.y <= pt.y && pt.y < self.max.y
    }

    pub fn contains(&self, rect: &Rectangle) -> bool {
        rect.min.x >= self.min.x && rect.max.x <= self.max.x &&
        rect.min.y >= self.min.y && rect.max.y <= self.max.y
    }

    pub fn overlaps(&self, rect: &Rectangle) -> bool {
        self.min.x < rect.max.x && rect.min.x < self.max.x &&
        self.min.y < rect.max.y && rect.min.y < self.max.y
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

    pub fn is_empty(&self) -> bool {
        self.max.x <= self.min.x || self.max.y <= self.min.y
    }

    #[inline]
    pub fn width(&self) -> u32 {
        (self.max.x - self.min.x) as u32
    }

    #[inline]
    pub fn height(&self) -> u32 {
        (self.max.y - self.min.y) as u32
    }

    #[inline]
    pub fn ratio(&self) -> f32 {
        self.width() as f32 / self.height() as f32
    }

    #[inline]
    pub fn area(&self) -> u32 {
        self.width() * self.height()
    }
}

impl Default for Rectangle {
    fn default() -> Self {
        Rectangle::new(Point::default(), Point::default())
    }
}

fn rect_cmp(r1: &Rectangle, r2: &Rectangle) -> Ordering {
    if r1.min.y >= r2.max.y {
        Ordering::Greater
    } else if r1.max.y <= r2.min.y {
        Ordering::Less
    } else {
        if r1.min.x >= r2.max.x {
            Ordering::Greater
        } else if r1.max.x <= r2.min.x {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    }
}

#[macro_export]
macro_rules! rect {
    ($x0:expr, $y0:expr, $x1:expr, $y1:expr $(,)* ) => ($crate::geom::Rectangle::new($crate::geom::Point::new($x0, $y0), $crate::geom::Point::new($x1, $y1)));
    ($min:expr, $max:expr $(,)* ) => ($crate::geom::Rectangle::new($min, $max));
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

impl Add<f32> for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: f32) -> Vec2 {
        Vec2 {
            x: self.x + rhs,
            y: self.y + rhs,
        }
    }
}

impl Add<Vec2> for f32 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self + rhs.x,
            y: self + rhs.y,
        }
    }
}

impl AddAssign<f32> for Vec2 {
    fn add_assign(&mut self, rhs: f32) {
        self.x += rhs;
        self.y += rhs;
    }
}

impl Sub<f32> for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: f32) -> Vec2 {
        Vec2 {
            x: self.x - rhs,
            y: self.y - rhs,
        }
    }
}

impl Sub<Vec2> for f32 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self - rhs.x,
            y: self - rhs.y,
        }
    }
}

impl SubAssign<f32> for Vec2 {
    fn sub_assign(&mut self, rhs: f32) {
        self.x -= rhs;
        self.y -= rhs;
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, rhs: f32) -> Vec2 {
        Vec2 {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Mul<Vec2> for f32 {
    type Output = Vec2;
    fn mul(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self * rhs.x,
            y: self * rhs.y,
        }
    }
}

impl MulAssign<f32> for Vec2 {
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl Div<f32> for Vec2 {
    type Output = Vec2;
    fn div(self, rhs: f32) -> Vec2 {
        Vec2 {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

impl Div<Vec2> for f32 {
    type Output = Vec2;
    fn div(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self / rhs.x,
            y: self / rhs.y,
        }
    }
}

impl DivAssign<f32> for Vec2 {
    fn div_assign(&mut self, rhs: f32) {
        self.x /= rhs;
        self.y /= rhs;
    }
}

#[cfg(test)]
mod tests {
    use super::divide;

    #[test]
    fn overlaping_rectangles() {
        let a = rect![2, 2, 10, 10];
        let b = rect![2, 5, 3, 6];
        let c = rect![1, 3, 2, 7];
        let d = rect![9, 9, 12, 12];
        let e = rect![4, 3, 5, 6];
        assert!(b.overlaps(&a));
        assert!(!c.overlaps(&a));
        assert!(d.overlaps(&a));
        assert!(e.overlaps(&a));
        assert!(a.overlaps(&e));
    }

    #[test]
    fn contained_rectangles() {
        let a = rect![2, 2, 10, 10];
        let b = rect![4, 3, 5, 6];
        let c = rect![4, 3, 12, 9];
        assert!(a.contains(&b));
        assert!(!b.contains(&a));
        assert!(!a.contains(&c));
        assert!(c.contains(&b));
    }

    #[test]
    fn divide_integers() {
        let a: i32 = 73;
        let b: i32 = 23;
        let v = divide(a, b);
        let s: i32 = v.iter().sum();
        assert_eq!(v.len(), b as usize);
        assert_eq!(s, a);
        assert_eq!(v.iter().max(), Some(&4));
        assert_eq!(v.iter().min(), Some(&3));
    }
}
