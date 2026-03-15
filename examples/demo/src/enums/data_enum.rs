use boltffi::*;

use crate::records::blittable::Point;

/// A geometric shape where each variant carries different data.
#[data]
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
    /// Triangle defined by three vertices.
    Triangle { a: Point, b: Point, c: Point },
    Point,
}

#[export]
pub fn echo_shape(s: Shape) -> Shape {
    s
}

/// Computes the area of the given shape. Returns 0 for points.
#[export]
pub fn shape_area(s: Shape) -> f64 {
    match s {
        Shape::Circle { radius } => std::f64::consts::PI * radius * radius,
        Shape::Rectangle { width, height } => width * height,
        Shape::Triangle { a, b, c } => {
            ((a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y)) / 2.0).abs()
        }
        Shape::Point => 0.0,
    }
}

#[export]
pub fn make_circle(radius: f64) -> Shape {
    Shape::Circle { radius }
}

#[export]
pub fn make_rectangle(width: f64, height: f64) -> Shape {
    Shape::Rectangle { width, height }
}

#[export]
pub fn echo_vec_shape(values: Vec<Shape>) -> Vec<Shape> {
    values
}

#[data]
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    Text { body: String },
    Image { url: String, width: u32, height: u32 },
    Ping,
}

#[export]
pub fn echo_message(m: Message) -> Message {
    m
}

#[export]
pub fn message_summary(m: Message) -> String {
    match m {
        Message::Text { body } => format!("text: {}", body),
        Message::Image { url, width, height } => format!("image: {}x{} at {}", width, height, url),
        Message::Ping => "ping".to_string(),
    }
}

#[data]
#[derive(Clone, Debug, PartialEq)]
pub enum Animal {
    Dog { name: String, breed: String },
    Cat { name: String, indoor: bool },
    Fish { count: u32 },
}

#[export]
pub fn echo_animal(a: Animal) -> Animal {
    a
}

#[export]
pub fn animal_name(a: Animal) -> String {
    match a {
        Animal::Dog { name, .. } | Animal::Cat { name, .. } => name,
        Animal::Fish { count } => format!("{} fish", count),
    }
}
