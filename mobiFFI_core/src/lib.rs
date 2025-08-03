#![allow(unused)]

pub mod handle;
pub mod safety;
pub mod status;
pub mod types;

pub use handle::HandleBox;
pub use safety::catch_ffi_panic;
pub use status::FfiStatus;
pub use types::{FfiBuf, FfiOption, FfiSlice, FfiString};

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

struct Counter {
    value: u64,
}

#[unsafe(no_mangle)]
pub extern "C" fn mffi_counter_new(initial: u64) -> *mut Counter {
    HandleBox::new(Counter { value: initial }).into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mffi_counter_increment(handle: *mut Counter) -> FfiStatus {
    match HandleBox::from_raw(handle) {
        Some(mut counter) => {
            counter.as_mut().value += 1;
            core::mem::forget(counter);
            FfiStatus::OK
        }
        None => FfiStatus::NULL_POINTER,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mffi_counter_get(handle: *mut Counter, out: *mut u64) -> FfiStatus {
    if out.is_null() {
        return FfiStatus::NULL_POINTER;
    }
    match HandleBox::from_raw(handle) {
        Some(counter) => {
            *out = counter.as_ref().value;
            core::mem::forget(counter);
            FfiStatus::OK
        }
        None => FfiStatus::NULL_POINTER,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mffi_counter_free(handle: *mut Counter) {
    if let Some(counter) = HandleBox::from_raw(handle) {
        drop(counter);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DataPoint {
    pub x: f64,
    pub y: f64,
    pub timestamp: i64,
}

struct DataStore {
    items: Vec<DataPoint>,
}

#[unsafe(no_mangle)]
pub extern "C" fn mffi_datastore_new() -> *mut DataStore {
    HandleBox::new(DataStore { items: Vec::new() }).into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mffi_datastore_add(handle: *mut DataStore, point: DataPoint) -> FfiStatus {
    match HandleBox::from_raw(handle) {
        Some(mut store) => {
            store.as_mut().items.push(point);
            core::mem::forget(store);
            FfiStatus::OK
        }
        None => FfiStatus::NULL_POINTER,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mffi_datastore_len(handle: *mut DataStore) -> usize {
    match HandleBox::from_raw(handle) {
        Some(store) => {
            let len = store.as_ref().items.len();
            core::mem::forget(store);
            len
        }
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mffi_datastore_copy_into(
    handle: *mut DataStore,
    dst: *mut DataPoint,
    dst_cap: usize,
    written: *mut usize,
) -> FfiStatus {
    if dst.is_null() || written.is_null() {
        return FfiStatus::NULL_POINTER;
    }

    match HandleBox::from_raw(handle) {
        Some(store) => {
            let items = &store.as_ref().items;
            let items_len = items.len();
            let copy_count = items_len.min(dst_cap);

            core::ptr::copy_nonoverlapping(items.as_ptr(), dst, copy_count);
            *written = copy_count;

            let status = if items_len > dst_cap {
                FfiStatus::BUFFER_TOO_SMALL
            } else {
                FfiStatus::OK
            };

            core::mem::forget(store);
            status
        }
        None => FfiStatus::NULL_POINTER,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mffi_datastore_free(handle: *mut DataStore) {
    if let Some(store) = HandleBox::from_raw(handle) {
        drop(store);
    }
}
