#![allow(unused)]

pub mod handle;
pub mod safety;
pub mod status;
pub mod types;

pub use handle::HandleBox;
pub use mobiFFI_macros::{FfiType, ffi_class, ffi_export};
pub use safety::catch_ffi_panic;
pub use status::{FfiStatus, clear_last_error, set_last_error, take_last_error};
pub use types::{FfiBuf, FfiOption, FfiSlice, FfiString};

unsafe fn read_input_str<'a>(ptr: *const u8, len: usize) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    let bytes = core::slice::from_raw_parts(ptr, len);
    core::str::from_utf8(bytes).ok()
}

pub const VERSION_MAJOR: u32 = 0;
pub const VERSION_MINOR: u32 = 1;
pub const VERSION_PATCH: u32 = 0;

#[unsafe(no_mangle)]
pub extern "C" fn mffi_version_major() -> u32 {
    VERSION_MAJOR
}

#[unsafe(no_mangle)]
pub extern "C" fn mffi_version_minor() -> u32 {
    VERSION_MINOR
}

#[unsafe(no_mangle)]
pub extern "C" fn mffi_version_patch() -> u32 {
    VERSION_PATCH
}

#[unsafe(no_mangle)]
pub extern "C" fn mffi_free_buf_u8(buf: FfiBuf<u8>) {
    drop(buf);
}

#[unsafe(no_mangle)]
pub extern "C" fn mffi_free_string(string: FfiString) {
    drop(string);
}

#[unsafe(no_mangle)]
pub extern "C" fn mffi_last_error_message(out: *mut FfiString) -> FfiStatus {
    if out.is_null() {
        return FfiStatus::NULL_POINTER;
    }

    match take_last_error() {
        Some(message) => {
            unsafe { *out = FfiString::from(message) };
            FfiStatus::OK
        }
        None => {
            unsafe { *out = FfiString::from("") };
            FfiStatus::OK
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mffi_clear_last_error() {
    clear_last_error();
}

fn fail_with_error(status: FfiStatus, message: impl Into<String>) -> FfiStatus {
    set_last_error(message);
    status
}

#[ffi_export]
pub fn greeting(name: &str) -> String {
    format!("Hello, {}!", name)
}

#[ffi_export]
pub fn concat(first: &str, second: &str) -> String {
    format!("{}{}", first, second)
}

#[ffi_export]
pub fn reverse_string(input: String) -> String {
    input.chars().rev().collect()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mffi_copy_bytes(
    src: *const u8,
    src_len: usize,
    dst: *mut u8,
    dst_cap: usize,
    written: *mut usize,
) -> FfiStatus {
    if src.is_null() || dst.is_null() || written.is_null() {
        return FfiStatus::NULL_POINTER;
    }

    if src_len > dst_cap {
        return FfiStatus::BUFFER_TOO_SMALL;
    }

    core::ptr::copy_nonoverlapping(src, dst, src_len);
    *written = src_len;

    FfiStatus::OK
}

pub struct Counter {
    value: u64,
}

#[ffi_class]
impl Counter {
    pub fn new() -> Self {
        Self { value: 0 }
    }

    pub fn set(&mut self, value: u64) {
        self.value = value;
    }

    pub fn increment(&mut self) {
        self.value += 1;
    }

    pub fn get(&self) -> u64 {
        self.value
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DataPoint {
    pub x: f64,
    pub y: f64,
    pub timestamp: i64,
}

pub struct DataStore {
    items: Vec<DataPoint>,
}

#[ffi_class]
impl DataStore {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn add(&mut self, point: DataPoint) {
        self.items.push(point);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn foreach(&self, mut callback: impl FnMut(DataPoint)) {
        self.items.iter().for_each(|p| callback(*p));
    }

    pub fn sum(&self) -> f64 {
        self.items.iter().map(|p| p.x + p.y).sum()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mffi_datastore_copy_into(
    handle: *mut DataStore,
    dst: *mut DataPoint,
    dst_cap: usize,
    written: *mut usize,
) -> FfiStatus {
    if handle.is_null() || dst.is_null() || written.is_null() {
        return FfiStatus::NULL_POINTER;
    }

    let store = &*handle;
    let items = &store.items;
    let copy_count = items.len().min(dst_cap);

    core::ptr::copy_nonoverlapping(items.as_ptr(), dst, copy_count);
    *written = copy_count;

    if items.len() > dst_cap {
        FfiStatus::BUFFER_TOO_SMALL
    } else {
        FfiStatus::OK
    }
}

#[ffi_export]
pub fn add_numbers(first: i32, second: i32) -> i32 {
    first + second
}

#[ffi_export]
pub fn multiply_floats(first: f64, second: f64) -> f64 {
    first * second
}

#[ffi_export]
pub fn make_greeting(name: &str) -> String {
    format!("Hello, {}!", name)
}

#[ffi_export]
pub fn safe_divide(numerator: i32, denominator: i32) -> Result<i32, &'static str> {
    if denominator == 0 {
        Err("division by zero")
    } else {
        Ok(numerator / denominator)
    }
}

#[ffi_export]
pub fn generate_sequence(count: i32) -> Vec<i32> {
    (0..count).collect()
}

#[ffi_export]
pub fn foreach_range(start: i32, end: i32, mut callback: impl FnMut(i32)) {
    (start..end).for_each(|i| callback(i));
}

pub struct Accumulator {
    value: i64,
}

#[ffi_class]
impl Accumulator {
    pub fn new() -> Self {
        Self { value: 0 }
    }

    pub fn add(&mut self, amount: i64) {
        self.value += amount;
    }

    pub fn get(&self) -> i64 {
        self.value
    }

    pub fn reset(&mut self) {
        self.value = 0;
    }
}
