use boltffi::*;

/// Adds two numbers asynchronously.
#[export]
pub async fn async_add(a: i32, b: i32) -> i32 {
    a + b
}

#[export]
pub async fn async_echo(message: String) -> String {
    format!("Echo: {}", message)
}

#[export]
pub async fn async_double_all(values: Vec<i32>) -> Vec<i32> {
    values.into_iter().map(|v| v * 2).collect()
}

#[export]
pub async fn async_find_positive(values: Vec<i32>) -> Option<i32> {
    values.into_iter().find(|&v| v > 0)
}

#[export]
pub async fn async_concat(strings: Vec<String>) -> String {
    strings.join(", ")
}
