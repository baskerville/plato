use std::fmt;
use serde::{Serialize, Deserialize};
use std::cmp::Ordering;
use std::f32::consts;
use std::ops::{Add, AddAssign, Sub, SubAssign, Mul, MulAssign, Div, DivAssign};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Dir {
    North,
    East,
    South,
    West,
}

impl Dir {
    pub fn opposite(self) -> Dir {
        match self {
            Dir::North => Dir::South,
            Dir::South => Dir::North,
            Dir::East => Dir::West,
            Dir::West => Dir::East,
        }
    }

    pub fn axis(self) -> Axis {
        match self {
            Dir::North | Dir::South => Axis::Vertical,
            Dir::East | Dir::West => Axis::Horizontal,
        }
    }
}

impl fmt::Display for Dir {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Dir::North => write!(f, "north"),
            Dir::East => write!(f, "east"),
            Dir::South => write!(f, "south"),
            Dir::West => write!(f, "west"),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DiagDir {
    NorthWest,
    NorthEast,
    SouthEast,
    SouthWest,
}

impl DiagDir {
    pub fn opposite(self) -> DiagDir {
        match self {
            DiagDir::NorthWest => DiagDir::SouthEast,
            DiagDir::NorthEast => DiagDir::SouthWest,
            DiagDir::SouthEast => DiagDir::NorthWest,
            DiagDir::SouthWest => DiagDir::NorthEast,
        }
    }
}

impl fmt::Display for DiagDir {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DiagDir::NorthWest => write!(f, "northwest"),
            DiagDir::NorthEast => write!(f, "northeast"),
            DiagDir::SouthEast => write!(f, "southeast"),
            DiagDir::SouthWest => write!(f, "southwest"),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Axis {
    Horizontal,
    Vertical,
    Diagonal,
}

impl fmt::Display for Axis {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Axis::Horizontal => write!(f, "horizontal"),
            Axis::Vertical => write!(f, "vertical"),
            Axis::Diagonal => write!(f, "diagonal"),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum CycleDir {
    Next,
    Previous,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum LinearDir {
    Backward,
    Forward,
}

impl LinearDir {
    pub fn opposite(self) -> LinearDir {
        match self {
            LinearDir::Backward => LinearDir::Forward,
            LinearDir::Forward => LinearDir::Backward,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[macro_export]
macro_rules! pt {
    ($x:expr, $y:expr $(,)* ) => ($crate::geom::Point::new($x, $y));
    ($a:expr) => ($crate::geom::Point::new($a, $a));
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Edge {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

impl Edge {
    pub fn uniform(value: i32) -> Edge {
        Edge {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

impl Default for Edge {
    fn default() -> Self {
        Edge {
            top: 0,
            right: 0,
            bottom: 0,
            left: 0,
        }
    }
}

impl Add for Edge {
    type Output = Edge;
    fn add(self, rhs: Edge) -> Edge {
        Edge {
            top: self.top + rhs.top,
            right: self.right + rhs.right,
            bottom: self.bottom + rhs.bottom,
            left: self.left + rhs.left,
        }
    }
}

impl AddAssign for Edge {
    fn add_assign(&mut self, rhs: Edge) {
        self.top += rhs.top;
        self.right += rhs.right;
        self.bottom += rhs.bottom;
        self.left += rhs.left;
    }
}

impl Sub for Edge {
    type Output = Edge;
    fn sub(self, rhs: Edge) -> Edge {
        Edge {
            top: self.top - rhs.top,
            right: self.right - rhs.right,
            bottom: self.bottom - rhs.bottom,
            left: self.left - rhs.left,
        }
    }
}

impl SubAssign for Edge {
    fn sub_assign(&mut self, rhs: Edge) {
        self.top -= rhs.top;
        self.right -= rhs.right;
        self.bottom -= rhs.bottom;
        self.left -= rhs.left;
    }
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

pub trait ColorSource {
    fn color(&self, x: i32, y: i32) -> u8;
}

impl<F> ColorSource for F where F: Fn(i32, i32) -> u8 {
    #[inline]
    fn color(&self, x: i32, y: i32) -> u8 {
        (self)(x, y)
    }
}

impl ColorSource for u8 {
    #[inline]
    fn color(&self, _: i32, _: i32) -> u8 {
        *self
    }
}

#[derive(Debug, Copy, Clone)]
pub struct BorderSpec {
    pub thickness: u16,
    pub color: u8,
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

// Returns the nearest point to p on segment ab
pub fn nearest_segment_point(p: Vec2, a: Vec2, b: Vec2) -> (Vec2, f32) {
    let ab = b - a;
    let ap = p - a;
    let l2 = ab.dot(ab);

    // Will not happen in practice
    if l2 < ::std::f32::EPSILON {
        return (a, 0.0);
    }

    let t = (ap.dot(ab) / l2).clamp(0.0, 1.0);
    (a + t * ab, t)
}

pub fn elbow(sp: &[Point]) -> usize {
    let len = sp.len();
    let a: Vec2 = sp[0].into();
    let b: Vec2 = sp[len - 1].into();
    let i1 = len / 3;
    let i2 = 2 * len / 3;
    let p1: Vec2 = sp[i1].into();
    let p2: Vec2 = sp[i2].into();
    let (n1, _) = nearest_segment_point(p1, a, b);
    let (n2, _) = nearest_segment_point(p2, a, b);
    let d1 = (p1 - n1).length();
    let d2 = (p2 - n2).length();
    if d1 > f32::EPSILON || d2 > f32::EPSILON {
        ((d1 * i1 as f32 + d2 * i2 as f32) / (d1 + d2)) as usize
    } else {
        len / 2
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

// Returns a Vec v, of size p, such that the sum all the elements is n.
// Each element x in v is such that |x - n/p| < 1.
pub fn divide(n: i32, p: i32) -> Vec<i32> {
    let size = n.checked_div(p).unwrap_or(0);
    let mut rem = n - p * size;
    let tick = p.checked_div(rem).unwrap_or(0);
    let mut vec = Vec::with_capacity(p as usize);
    for i in 0..p {
        if rem > 0 && (i+1) % tick == 0 {
            vec.push(size + 1);
            rem -= 1;
        } else {
            vec.push(size);
        }
    }
    vec
}

#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    (1.0 - t) * a + t * b
}

// Returns the clockwise and anti-clockwise modulo p distance from a to b.
#[inline]
pub fn circular_distances(a: u16, mut b: u16, p: u16) -> (u16, u16) {
    if b < a {
        b += p;
    }
    let d0 = b - a;
    let d1 = p - d0;
    (d0, d1)
}

impl Point {
    pub fn new(x: i32, y: i32) -> Point {
        Point { x, y }
    }

    pub fn dist2(self, pt: Point) -> u32 {
        ((pt.x - self.x).pow(2) + (pt.y - self.y).pow(2)) as u32
    }

    pub fn rdist2(self, rect: &Rectangle) -> u32 {
        if rect.includes(self) {
            0
        } else if self.y >= rect.min.y && self.y < rect.max.y {
            if self.x < rect.min.x {
                (rect.min.x - self.x).pow(2) as u32
            } else {
                (self.x - rect.max.x + 1).pow(2) as u32
            }
        } else if self.x >= rect.min.x && self.x < rect.max.x {
            if self.y < rect.min.y {
                (rect.min.y - self.y).pow(2) as u32
            } else {
                (self.y - rect.max.y + 1).pow(2) as u32
            }
        } else if self.x < rect.min.x {
            if self.y < rect.min.y {
                self.dist2(rect.min)
            } else {
                self.dist2(Point::new(rect.min.x, rect.max.y - 1))
            }
        } else {
            if self.y < rect.min.y {
                self.dist2(Point::new(rect.max.x - 1, rect.min.y))
            } else {
                self.dist2(Point::new(rect.max.x - 1, rect.max.y - 1))
            }
        }
    }

    pub fn length(self) -> f32 {
        ((self.x.pow(2) + self.y.pow(2)) as f32).sqrt()
    }

    pub fn angle(self) -> f32 {
        (-self.y as f32).atan2(self.x as f32)
    }

    pub fn dir(self) -> Dir {
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

    pub fn diag_dir(self) -> DiagDir {
        if self.x.is_positive() {
            if self.y.is_positive() {
                DiagDir::SouthEast
            } else {
                DiagDir::NorthEast
            }
        } else {
            if self.y.is_positive() {
                DiagDir::SouthWest
            } else {
                DiagDir::NorthWest
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

impl From<Point> for Vec2 {
    fn from(pt: Point) -> Self {
        Vec2::new(pt.x as f32, pt.y as f32)
    }
}

impl From<Vec2> for Point {
    fn from(pt: Vec2) -> Self {
        Point::new(pt.x as i32, pt.y as i32)
    }
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
        Vec2 { x, y }
    }

    pub fn dot(self, other: Vec2) -> f32 {
        self.x * other.x + self.y * other.y
    }

    pub fn cross(self, other: Vec2) -> f32 {
        self.x * other.y - self.y * other.x
    }

    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn angle(self) -> f32 {
        (-self.y).atan2(self.x)
    }

    pub fn dir(self) -> Dir {
        if self.x.abs() > self.y.abs() {
            if self.x.is_sign_positive() {
                Dir::East
            } else {
                Dir::West
            }
        } else {
            if self.y.is_sign_positive() {
                Dir::South
            } else {
                Dir::North
            }
        }
    }

    pub fn diag_dir(self) -> DiagDir {
        if self.x.is_sign_positive() {
            if self.y.is_sign_positive() {
                DiagDir::SouthEast
            } else {
                DiagDir::NorthEast
            }
        } else {
            if self.y.is_sign_positive() {
                DiagDir::SouthWest
            } else {
                DiagDir::NorthWest
            }
        }
    }
}

// Based on https://golang.org/pkg/image/#Rectangle
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Rectangle {
    pub min: Point,
    pub max: Point,
}

#[macro_export]
macro_rules! rect {
    ($x0:expr, $y0:expr, $x1:expr, $y1:expr $(,)* ) => ($crate::geom::Rectangle::new($crate::geom::Point::new($x0, $y0), $crate::geom::Point::new($x1, $y1)));
    ($min:expr, $max:expr $(,)* ) => ($crate::geom::Rectangle::new($min, $max));
}

impl fmt::Display for Rectangle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}, {}, {}, {}]", self.min.x, self.min.y, self.max.x, self.max.y)
    }
}

impl Rectangle {
    pub fn new(min: Point, max: Point) -> Rectangle {
        Rectangle {
            min,
            max,
        }
    }

    pub fn from_point(pt: Point) -> Rectangle {
        Rectangle {
            min: pt,
            max: pt + 1,
        }
    }

    pub fn from_disk(center: Point, radius: i32) -> Rectangle {
        Rectangle {
            min: center - radius,
            max: center + radius,
        }
    }

    pub fn from_segment(start: Point, end: Point, start_radius: i32, end_radius: i32) -> Rectangle {
        let x_min = (start.x - start_radius).min(end.x - end_radius);
        let x_max = (start.x + start_radius).max(end.x + end_radius);
        let y_min = (start.y - start_radius).min(end.y - end_radius);
        let y_max = (start.y + start_radius).max(end.y + end_radius);
        Rectangle {
            min: pt!(x_min, y_min),
            max: pt!(x_max, y_max),
        }
    }

    pub fn to_boundary(&self) -> Boundary {
        Boundary {
            min: Vec2::new(self.min.x as f32, self.min.y as f32),
            max: Vec2::new(self.max.x as f32, self.max.y as f32),
        }
    }

    pub fn diag2(&self) -> u32 {
        self.min.dist2(self.max)
    }

    pub fn includes(&self, pt: Point) -> bool {
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

    pub fn extends(&self, rect: &Rectangle) -> bool {
        let dmin = [self.width(), self.height(),
                    rect.width(), rect.height()].into_iter().min().unwrap() as i32 / 3;

        // rect is on top of self.
        if self.min.y >= rect.max.y && self.min.x < rect.max.x && rect.min.x < self.max.x {
            (self.min.y - rect.max.y) <= dmin
        // rect is at the right of self.
        } else if rect.min.x >= self.max.x && self.min.y < rect.max.y && rect.min.y < self.max.y {
            (rect.min.x - self.max.x) <= dmin
        // rect is on bottom of self.
        } else if rect.min.y >= self.max.y && self.min.x < rect.max.x && rect.min.x < self.max.x {
            (rect.min.y - self.max.y) <= dmin
        // rect is at the left of self.
        } else if self.min.x >= rect.max.x && self.min.y < rect.max.y && rect.min.y < self.max.y {
            (self.min.x - rect.max.x) <= dmin
        } else {
            false
        }
    }

    pub fn touches(&self, rect: &Rectangle) -> bool {
        ((self.min.x == rect.max.x || self.max.x == rect.min.x ||
          self.min.x == rect.min.x || self.max.x == rect.max.x) &&
         (self.max.y >= rect.min.y && self.min.y <= rect.max.y)) ||
        ((self.min.y == rect.max.y || self.max.y == rect.min.y ||
          self.min.y == rect.min.y || self.max.y == rect.max.y) &&
         (self.max.x >= rect.min.x && self.min.x <= rect.max.x))
    }

    pub fn merge(&mut self, pt: Point) {
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
            Some(Rectangle::new(Point::new(self.min.x.max(rect.min.x),
                                           self.min.y.max(rect.min.y)),
                                Point::new(self.max.x.min(rect.max.x),
                                           self.max.y.min(rect.max.y))))
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

    #[inline]
    pub fn center(&self) -> Point {
        (self.min + self.max) / 2
    }

    pub fn grow(&mut self, edges: &Edge) {
        self.min.x -= edges.left;
        self.min.y -= edges.top;
        self.max.x += edges.right;
        self.max.y += edges.bottom;
    }

    pub fn shrink(&mut self, edges: &Edge) {
        self.min.x += edges.left;
        self.min.y += edges.top;
        self.max.x -= edges.right;
        self.max.y -= edges.bottom;
    }
}

impl Default for Rectangle {
    fn default() -> Self {
        Rectangle::new(Point::default(), Point::default())
    }
}

impl From<(u32, u32)> for Rectangle {
    fn from(dims: (u32, u32)) -> Rectangle {
        Rectangle::new(Point::new(0, 0), Point::new(dims.0 as i32, dims.1 as i32))
    }
}

impl PartialOrd for Rectangle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Rectangle {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.min.y >= other.max.y {
            Ordering::Greater
        } else if self.max.y <= other.min.y {
            Ordering::Less
        } else {
            if self.min.x >= other.max.x {
                Ordering::Greater
            } else if self.max.x <= other.min.x {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        }
    }
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

#[derive(Debug, Copy, Clone)]
pub struct Boundary {
    pub min: Vec2,
    pub max: Vec2,
}

impl Boundary {
    pub fn new(min: Vec2, max: Vec2) -> Boundary {
        Boundary { min, max }
    }

    pub fn to_rect(&self) -> Rectangle {
        Rectangle {
            min: Point::new(self.min.x.floor() as i32, self.min.y.floor() as i32),
            max: Point::new(self.max.x.ceil() as i32, self.max.y.ceil() as i32),
        }
    }

    pub fn overlaps(&self, rect: &Boundary) -> bool {
        self.min.x < rect.max.x && rect.min.x < self.max.x &&
        self.min.y < rect.max.y && rect.min.y < self.max.y
    }

    pub fn contains(&self, rect: &Boundary) -> bool {
        rect.min.x >= self.min.x && rect.max.x <= self.max.x &&
        rect.min.y >= self.min.y && rect.max.y <= self.max.y
    }

    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }
}

#[macro_export]
macro_rules! bndr {
    ($x0:expr, $y0:expr, $x1:expr, $y1:expr $(,)* ) => ($crate::geom::Boundary::new($crate::geom::Vec2::new($x0, $y0), $crate::geom::Vec2::new($x1, $y1)));
    ($min:expr, $max:expr $(,)* ) => ($crate::geom::Boundary::new($min, $max));
}

impl Into<Rectangle> for Boundary {
    fn into(self) -> Rectangle {
        Rectangle {
            min: Point::new(self.min.x.floor() as i32, self.min.y.floor() as i32),
            max: Point::new(self.max.x.ceil() as i32, self.max.y.ceil() as i32),
        }
    }
}

impl Into<Boundary> for Rectangle {
    fn into(self) -> Boundary {
        Boundary {
            min: Vec2::new(self.min.x as f32, self.min.y as f32),
            max: Vec2::new(self.max.x as f32, self.max.y as f32),
        }
    }
}

impl Mul<f32> for Boundary {
    type Output = Boundary;
    fn mul(self, rhs: f32) -> Boundary {
        Boundary {
            min: self.min * rhs,
            max: self.max * rhs,
        }
    }
}

impl Mul<Boundary> for f32 {
    type Output = Boundary;
    fn mul(self, rhs: Boundary) -> Boundary {
        Boundary {
            min: self * rhs.min,
            max: self * rhs.max,
        }
    }
}

impl MulAssign<f32> for Boundary {
    fn mul_assign(&mut self, rhs: f32) {
        self.min *= rhs;
        self.max *= rhs;
    }
}

impl Div<f32> for Boundary {
    type Output = Boundary;
    fn div(self, rhs: f32) -> Boundary {
        Boundary {
            min: self.min / rhs,
            max: self.max / rhs,
        }
    }
}

impl Div<Boundary> for f32 {
    type Output = Boundary;
    fn div(self, rhs: Boundary) -> Boundary {
        Boundary {
            min: rhs.min / self,
            max: rhs.max / self,
        }
    }
}

impl DivAssign<f32> for Boundary {
    fn div_assign(&mut self, rhs: f32) {
        self.min /= rhs;
        self.max /= rhs;
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Region {
    Corner(DiagDir),
    Strip(Dir),
    Center,
}

impl Region {
    // pt ∈ rect
    // 0.0 < {corner,strip}_width < 1.0
    pub fn from_point(pt: Point, rect: Rectangle, strip_width: f32, corner_width: f32) -> Region {
        let w = rect.width() as i32;
        let h = rect.height() as i32;
        let m = w.min(h) as f32 / 2.0;

        let d = (m * corner_width).max(1.0) as i32;
        let x1 = rect.min.x + d - 1;
        let x2 = rect.max.x - d;

        // The four corners are on top of all the other regions.
        if pt.x <= x1 {
            let dx = x1 - pt.x;
            if pt.y <= rect.min.y + dx {
                return Region::Corner(DiagDir::NorthWest);
            } else if pt.y >= rect.max.y - 1 - dx {
                return Region::Corner(DiagDir::SouthWest);
            }
        } else if pt.x >= x2 {
            let dx = pt.x - x2;
            if pt.y <= rect.min.y + dx {
                return Region::Corner(DiagDir::NorthEast);
            } else if pt.y >= rect.max.y - 1 - dx {
                return Region::Corner(DiagDir::SouthEast);
            }
        }

        let d = (m * strip_width).max(1.0) as i32;
        let x1 = rect.min.x + d - 1;
        let x2 = rect.max.x - d;
        let y1 = rect.min.y + d - 1;
        let y2 = rect.max.y - d;

        // The four strips are above the center region.
        // Each of the diagonals between the strips has to belong to one of the strip.
        if pt.x <= x1 {
            let dx = pt.x - rect.min.x;
            if pt.y >= rect.min.y + dx && pt.y < rect.max.y - 1 - dx {
                return Region::Strip(Dir::West);
            }
        } else if pt.x >= x2 {
            let dx = rect.max.x - 1 - pt.x;
            if pt.y > rect.min.y + dx && pt.y <= rect.max.y - 1 - dx {
                return Region::Strip(Dir::East);
            }
        }

        if pt.y <= y1 {
            let dy = pt.y - rect.min.y;
            if pt.x > rect.min.x + dy && pt.y <= rect.max.x - 1 - dy {
                return Region::Strip(Dir::North);
            }
        } else if pt.y >= y2 {
            let dy = rect.max.y - 1 - pt.y;
            if pt.x >= rect.min.x + dy && pt.x < rect.max.x - 1 - dy {
                return Region::Strip(Dir::South);
            }
        }

        // The center rectangle is below everything else.
        Region::Center
    }
}


#[cfg(test)]
mod tests {
    use super::{divide, LinearDir};

    #[test]
    fn test_linear_dir_opposite() {
        assert_eq!(LinearDir::Forward.opposite(), LinearDir::Backward);
        assert_eq!(LinearDir::Backward.opposite(), LinearDir::Forward);
    }

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
    fn extended_rectangles() {
        let a = rect![30, 30, 60, 60];
        let b = rect![23, 0, 67, 28];
        let c = rect![60, 40, 110, 80];
        let d = rect![26, 62, 55, 96];
        let e = rect![0, 25, 29, 60];
        assert!(b.extends(&a));
        assert!(c.extends(&a));
        assert!(d.extends(&a));
        assert!(e.extends(&a));
        assert!(!b.extends(&d));
        assert!(!c.extends(&e));
        assert!(!e.extends(&b));
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

    #[test]
    fn point_rectangle_distance() {
        let pt1 = pt!(4, 5);
        let pt2 = pt!(1, 2);
        let pt3 = pt!(3, 8);
        let pt4 = pt!(8, 6);
        let pt5 = pt!(7, 4);
        let rect = rect![2, 3, 7, 6];
        assert_eq!(pt1.rdist2(&rect), 0);
        assert_eq!(pt2.rdist2(&rect), 2);
        assert_eq!(pt3.rdist2(&rect), 9);
        assert_eq!(pt4.rdist2(&rect), 5);
        assert_eq!(pt5.rdist2(&rect), 1);
    }
}
