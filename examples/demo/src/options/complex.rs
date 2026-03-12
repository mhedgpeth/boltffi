use boltffi::*;

use crate::enums::c_style::Status;
use crate::records::blittable::Point;

#[export]
pub fn echo_optional_string(v: Option<String>) -> Option<String> {
    v
}

#[export]
pub fn is_some_string(v: Option<String>) -> bool {
    v.is_some()
}

#[export]
pub fn echo_optional_point(v: Option<Point>) -> Option<Point> {
    v
}

/// Returns a Point if both coordinates are valid, None otherwise.
#[export]
pub fn make_some_point(x: f64, y: f64) -> Option<Point> {
    Some(Point { x, y })
}

#[export]
pub fn make_none_point() -> Option<Point> {
    None
}

#[export]
pub fn echo_optional_status(v: Option<Status>) -> Option<Status> {
    v
}

#[export]
pub fn echo_optional_vec(v: Option<Vec<i32>>) -> Option<Vec<i32>> {
    v
}

#[export]
pub fn optional_vec_length(v: Option<Vec<i32>>) -> Option<u32> {
    v.map(|vec| vec.len() as u32)
}
