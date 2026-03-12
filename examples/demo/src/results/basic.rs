use boltffi::*;

use crate::records::blittable::Point;

#[export]
pub fn safe_divide(a: i32, b: i32) -> Result<i32, String> {
    if b == 0 {
        Err("division by zero".to_string())
    } else {
        Ok(a / b)
    }
}

#[export]
pub fn safe_sqrt(x: f64) -> Result<f64, String> {
    if x < 0.0 {
        Err("negative input".to_string())
    } else {
        Ok(x.sqrt())
    }
}

#[export]
pub fn parse_point(s: String) -> Result<Point, String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return Err("expected format: x,y".to_string());
    }
    let x = parts[0]
        .trim()
        .parse::<f64>()
        .map_err(|_| "invalid x coordinate".to_string())?;
    let y = parts[1]
        .trim()
        .parse::<f64>()
        .map_err(|_| "invalid y coordinate".to_string())?;
    Ok(Point { x, y })
}

#[export]
pub fn always_ok(v: i32) -> Result<i32, String> {
    Ok(v * 2)
}

#[export]
pub fn always_err(msg: String) -> Result<i32, String> {
    Err(msg)
}

#[export]
pub fn result_to_string(v: Result<i32, String>) -> String {
    match v {
        Ok(val) => format!("ok: {}", val),
        Err(err) => format!("err: {}", err),
    }
}
