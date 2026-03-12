use boltffi::*;

#[export]
pub fn echo_optional_i32(v: Option<i32>) -> Option<i32> {
    v
}

#[export]
pub fn echo_optional_f64(v: Option<f64>) -> Option<f64> {
    v
}

#[export]
pub fn echo_optional_bool(v: Option<bool>) -> Option<bool> {
    v
}

#[export]
pub fn unwrap_or_default_i32(v: Option<i32>, fallback: i32) -> i32 {
    v.unwrap_or(fallback)
}

#[export]
pub fn make_some_i32(v: i32) -> Option<i32> {
    Some(v)
}

#[export]
pub fn make_none_i32() -> Option<i32> {
    None
}

#[export]
pub fn double_if_some(v: Option<i32>) -> Option<i32> {
    v.map(|x| x * 2)
}
