use crate::types::FfiBuf;
use crate::types::FfiSpan;
use crate::wire::{WireDecode, WireEncode};

pub unsafe trait Passable: Sized {
    type In;
    type Out;
    unsafe fn unpack(input: Self::In) -> Self;
    fn pack(self) -> Self::Out;
}

unsafe impl Passable for i8 {
    type In = i8;
    type Out = i8;
    unsafe fn unpack(input: i8) -> Self { input }
    fn pack(self) -> i8 { self }
}

unsafe impl Passable for i16 {
    type In = i16;
    type Out = i16;
    unsafe fn unpack(input: i16) -> Self { input }
    fn pack(self) -> i16 { self }
}

unsafe impl Passable for i32 {
    type In = i32;
    type Out = i32;
    unsafe fn unpack(input: i32) -> Self { input }
    fn pack(self) -> i32 { self }
}

unsafe impl Passable for i64 {
    type In = i64;
    type Out = i64;
    unsafe fn unpack(input: i64) -> Self { input }
    fn pack(self) -> i64 { self }
}

unsafe impl Passable for u8 {
    type In = u8;
    type Out = u8;
    unsafe fn unpack(input: u8) -> Self { input }
    fn pack(self) -> u8 { self }
}

unsafe impl Passable for u16 {
    type In = u16;
    type Out = u16;
    unsafe fn unpack(input: u16) -> Self { input }
    fn pack(self) -> u16 { self }
}

unsafe impl Passable for u32 {
    type In = u32;
    type Out = u32;
    unsafe fn unpack(input: u32) -> Self { input }
    fn pack(self) -> u32 { self }
}

unsafe impl Passable for u64 {
    type In = u64;
    type Out = u64;
    unsafe fn unpack(input: u64) -> Self { input }
    fn pack(self) -> u64 { self }
}

unsafe impl Passable for f32 {
    type In = f32;
    type Out = f32;
    unsafe fn unpack(input: f32) -> Self { input }
    fn pack(self) -> f32 { self }
}

unsafe impl Passable for f64 {
    type In = f64;
    type Out = f64;
    unsafe fn unpack(input: f64) -> Self { input }
    fn pack(self) -> f64 { self }
}

unsafe impl Passable for bool {
    type In = bool;
    type Out = bool;
    unsafe fn unpack(input: bool) -> Self { input }
    fn pack(self) -> bool { self }
}

unsafe impl Passable for usize {
    type In = usize;
    type Out = usize;
    unsafe fn unpack(input: usize) -> Self { input }
    fn pack(self) -> usize { self }
}

unsafe impl Passable for isize {
    type In = isize;
    type Out = isize;
    unsafe fn unpack(input: isize) -> Self { input }
    fn pack(self) -> isize { self }
}

unsafe impl Passable for String {
    type In = FfiSpan;
    type Out = FfiBuf<u8>;

    unsafe fn unpack(input: FfiSpan) -> Self {
        let bytes = unsafe { input.as_bytes() };
        core::str::from_utf8(bytes)
            .expect("invalid UTF-8 in FfiSpan")
            .to_string()
    }

    fn pack(self) -> FfiBuf<u8> {
        FfiBuf::from_vec(self.into_bytes())
    }
}

pub unsafe trait WirePassable: WireEncode + WireDecode + Sized {}

unsafe impl<T: WirePassable> Passable for T {
    type In = FfiSpan;
    type Out = FfiBuf<u8>;

    unsafe fn unpack(input: FfiSpan) -> Self {
        let bytes = unsafe { input.as_bytes() };
        crate::wire::decode(bytes).expect("wire decode failed in Passable::unpack")
    }

    fn pack(self) -> FfiBuf<u8> {
        FfiBuf::wire_encode(&self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_roundtrip() {
        let value: i32 = 42;
        let packed = value.pack();
        let unpacked = unsafe { i32::unpack(packed) };
        assert_eq!(unpacked, 42);
    }

    #[test]
    fn bool_roundtrip() {
        assert_eq!(unsafe { bool::unpack(true.pack()) }, true);
        assert_eq!(unsafe { bool::unpack(false.pack()) }, false);
    }

    #[test]
    fn string_pack() {
        let value = String::from("hello");
        let buf = value.pack();
        assert_eq!(buf.len(), 5);
    }

    #[test]
    fn string_roundtrip() {
        let original = String::from("hello world");
        let bytes = original.as_bytes();
        let span = FfiSpan {
            ptr: bytes.as_ptr(),
            len: bytes.len(),
        };
        let recovered = unsafe { String::unpack(span) };
        assert_eq!(recovered, "hello world");
    }
}
