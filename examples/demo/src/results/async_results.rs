use boltffi::*;

use super::error_enums::MathError;

#[export]
pub async fn async_safe_divide(a: i32, b: i32) -> Result<i32, MathError> {
    if b == 0 {
        Err(MathError::DivisionByZero)
    } else {
        Ok(a / b)
    }
}

#[export]
pub async fn async_fallible_fetch(key: i32) -> Result<String, String> {
    if key < 0 {
        Err("invalid key".to_string())
    } else {
        Ok(format!("value_{}", key))
    }
}

/// Looks up a value by key. Negative keys are invalid, key 0
/// means "not found" (returns Ok(None)), positive keys return
/// the value multiplied by 10.
#[export]
pub async fn async_find_value(key: i32) -> Result<Option<i32>, String> {
    if key < 0 {
        Err("invalid key".to_string())
    } else if key == 0 {
        Ok(None)
    } else {
        Ok(Some(key * 10))
    }
}
