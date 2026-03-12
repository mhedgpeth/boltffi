use boltffi::*;

use crate::records::blittable::Point;

#[data]
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Line {
    pub start: Point,
    pub end: Point,
}

#[export]
pub fn echo_line(l: Line) -> Line {
    l
}

#[export]
pub fn make_line(x1: f64, y1: f64, x2: f64, y2: f64) -> Line {
    Line {
        start: Point { x: x1, y: y1 },
        end: Point { x: x2, y: y2 },
    }
}

#[export]
pub fn line_length(l: Line) -> f64 {
    let dx = l.end.x - l.start.x;
    let dy = l.end.y - l.start.y;
    (dx * dx + dy * dy).sqrt()
}

#[data]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Dimensions {
    pub width: f64,
    pub height: f64,
}

#[data]
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Rect {
    pub origin: Point,
    pub dimensions: Dimensions,
}

#[export]
pub fn echo_rect(r: Rect) -> Rect {
    r
}

#[export]
pub fn rect_area(r: Rect) -> f64 {
    r.dimensions.width * r.dimensions.height
}
