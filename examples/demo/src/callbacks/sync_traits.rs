use boltffi::*;

use crate::enums::c_style::Status;
use crate::records::blittable::Point;

/// A callback trait for transforming integer values.
#[export]
pub trait ValueCallback {
    /// Called with an integer, returns a transformed integer.
    fn on_value(&self, value: i32) -> i32;
}

#[export]
pub fn invoke_value_callback(callback: impl ValueCallback, input: i32) -> i32 {
    callback.on_value(input)
}

#[export]
pub fn invoke_value_callback_twice(callback: impl ValueCallback, a: i32, b: i32) -> i32 {
    callback.on_value(a) + callback.on_value(b)
}

#[export]
pub fn invoke_boxed_value_callback(callback: Box<dyn ValueCallback>, input: i32) -> i32 {
    callback.on_value(input)
}

#[export]
pub trait PointTransformer {
    fn transform(&self, point: Point) -> Point;
}

#[export]
pub fn transform_point(transformer: impl PointTransformer, point: Point) -> Point {
    transformer.transform(point)
}

#[export]
pub fn transform_point_boxed(transformer: Box<dyn PointTransformer>, point: Point) -> Point {
    transformer.transform(point)
}

#[export]
pub trait StatusMapper {
    fn map_status(&self, status: Status) -> Status;
}

#[export]
pub fn map_status(mapper: impl StatusMapper, status: Status) -> Status {
    mapper.map_status(status)
}

#[export]
pub trait VecProcessor {
    fn process(&self, values: Vec<i32>) -> Vec<i32>;
}

#[export]
pub fn process_vec(processor: impl VecProcessor, values: Vec<i32>) -> Vec<i32> {
    processor.process(values)
}

#[export]
pub trait MultiMethodCallback {
    fn method_a(&self, x: i32) -> i32;
    fn method_b(&self, x: i32, y: i32) -> i32;
    fn method_c(&self) -> i32;
}

#[export]
pub fn invoke_multi_method(callback: impl MultiMethodCallback, x: i32, y: i32) -> i32 {
    callback.method_a(x) + callback.method_b(x, y) + callback.method_c()
}

#[export]
pub fn invoke_multi_method_boxed(callback: Box<dyn MultiMethodCallback>, x: i32, y: i32) -> i32 {
    callback.method_a(x) + callback.method_b(x, y) + callback.method_c()
}

#[export]
pub fn invoke_two_callbacks(
    first: impl ValueCallback,
    second: impl ValueCallback,
    value: i32,
) -> i32 {
    first.on_value(value) + second.on_value(value)
}

#[export]
pub trait OptionCallback {
    fn find_value(&self, key: i32) -> Option<i32>;
}

#[export]
pub fn invoke_option_callback(callback: impl OptionCallback, key: i32) -> Option<i32> {
    callback.find_value(key)
}
