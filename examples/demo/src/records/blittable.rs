use boltffi::*;

/// A 2D point with double-precision coordinates.
#[data]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Point {
    /// Horizontal position.
    pub x: f64,
    /// Vertical position.
    pub y: f64,
}

#[export]
pub fn echo_point(p: Point) -> Point {
    p
}

#[export]
pub fn make_point(x: f64, y: f64) -> Point {
    Point { x, y }
}

#[export]
pub fn add_points(a: Point, b: Point) -> Point {
    Point {
        x: a.x + b.x,
        y: a.y + b.y,
    }
}

/// Returns the distance from the origin to the given point.
#[export]
pub fn point_distance(p: Point) -> f64 {
    (p.x * p.x + p.y * p.y).sqrt()
}

#[data]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[export]
pub fn echo_color(c: Color) -> Color {
    c
}

#[export]
pub fn make_color(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color { r, g, b, a }
}
