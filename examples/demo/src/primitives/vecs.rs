use boltffi::*;

#[export]
pub fn echo_vec_i32(v: Vec<i32>) -> Vec<i32> {
    v
}

/// Sums all elements in the vector. Uses i64 to avoid overflow
/// on large inputs.
#[export]
pub fn sum_vec_i32(v: Vec<i32>) -> i64 {
    v.iter().map(|&x| x as i64).sum()
}

#[export]
pub fn echo_vec_f64(v: Vec<f64>) -> Vec<f64> {
    v
}

#[export]
pub fn echo_vec_bool(v: Vec<bool>) -> Vec<bool> {
    v
}

#[export]
pub fn echo_vec_string(v: Vec<String>) -> Vec<String> {
    v
}

#[export]
pub fn vec_string_lengths(v: Vec<String>) -> Vec<u32> {
    v.iter().map(|s| s.len() as u32).collect()
}

#[export]
pub fn make_range(start: i32, end: i32) -> Vec<i32> {
    (start..end).collect()
}

#[export]
pub fn reverse_vec_i32(v: Vec<i32>) -> Vec<i32> {
    v.into_iter().rev().collect()
}
