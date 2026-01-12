use crate::wire::constants::*;

pub trait WireSize {
    fn is_fixed_size() -> bool where Self: Sized { false }
    fn fixed_size() -> Option<usize> where Self: Sized { None }
    fn wire_size(&self) -> usize;
}

pub trait WireEncode: WireSize {
    fn encode_to(&self, buf: &mut [u8]) -> usize;
}

macro_rules! impl_wire_primitive {
    ($($ty:ty),*) => {
        $(
            impl WireSize for $ty {
                #[inline]
                fn is_fixed_size() -> bool { true }

                #[inline]
                fn fixed_size() -> Option<usize> { Some(core::mem::size_of::<$ty>()) }

                #[inline]
                fn wire_size(&self) -> usize { core::mem::size_of::<$ty>() }
            }

            impl WireEncode for $ty {
                #[inline]
                fn encode_to(&self, buf: &mut [u8]) -> usize {
                    let bytes = self.to_le_bytes();
                    buf[..bytes.len()].copy_from_slice(&bytes);
                    bytes.len()
                }
            }
        )*
    };
}

impl_wire_primitive!(i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);

impl WireSize for bool {
    #[inline]
    fn is_fixed_size() -> bool { true }

    #[inline]
    fn fixed_size() -> Option<usize> { Some(1) }

    #[inline]
    fn wire_size(&self) -> usize { 1 }
}

impl WireEncode for bool {
    #[inline]
    fn encode_to(&self, buf: &mut [u8]) -> usize {
        buf[0] = if *self { 1 } else { 0 };
        1
    }
}

impl WireSize for isize {
    #[inline]
    fn is_fixed_size() -> bool { true }

    #[inline]
    fn fixed_size() -> Option<usize> { Some(8) }

    #[inline]
    fn wire_size(&self) -> usize { 8 }
}

impl WireEncode for isize {
    #[inline]
    fn encode_to(&self, buf: &mut [u8]) -> usize {
        let value = *self as i64;
        let bytes = value.to_le_bytes();
        buf[..8].copy_from_slice(&bytes);
        8
    }
}

impl WireSize for usize {
    #[inline]
    fn is_fixed_size() -> bool { true }

    #[inline]
    fn fixed_size() -> Option<usize> { Some(8) }

    #[inline]
    fn wire_size(&self) -> usize { 8 }
}

impl WireEncode for usize {
    #[inline]
    fn encode_to(&self, buf: &mut [u8]) -> usize {
        let value = *self as u64;
        let bytes = value.to_le_bytes();
        buf[..8].copy_from_slice(&bytes);
        8
    }
}

impl WireSize for str {
    #[inline]
    fn wire_size(&self) -> usize {
        STRING_LEN_SIZE + self.len()
    }
}

impl WireEncode for str {
    #[inline]
    fn encode_to(&self, buf: &mut [u8]) -> usize {
        let len = self.len() as u32;
        buf[..STRING_LEN_SIZE].copy_from_slice(&len.to_le_bytes());
        buf[STRING_LEN_SIZE..STRING_LEN_SIZE + self.len()].copy_from_slice(self.as_bytes());
        STRING_LEN_SIZE + self.len()
    }
}

impl WireSize for String {
    #[inline]
    fn is_fixed_size() -> bool { false }

    #[inline]
    fn fixed_size() -> Option<usize> { None }

    #[inline]
    fn wire_size(&self) -> usize {
        self.as_str().wire_size()
    }
}

impl WireEncode for String {
    #[inline]
    fn encode_to(&self, buf: &mut [u8]) -> usize {
        self.as_str().encode_to(buf)
    }
}

impl<T: WireSize> WireSize for Option<T> {
    #[inline]
    fn is_fixed_size() -> bool { false }

    #[inline]
    fn fixed_size() -> Option<usize> { None }

    #[inline]
    fn wire_size(&self) -> usize {
        match self {
            Some(value) => OPTION_FLAG_SIZE + value.wire_size(),
            None => OPTION_FLAG_SIZE,
        }
    }
}

impl<T: WireEncode> WireEncode for Option<T> {
    #[inline]
    fn encode_to(&self, buf: &mut [u8]) -> usize {
        match self {
            Some(value) => {
                buf[0] = 1;
                OPTION_FLAG_SIZE + value.encode_to(&mut buf[OPTION_FLAG_SIZE..])
            }
            None => {
                buf[0] = 0;
                OPTION_FLAG_SIZE
            }
        }
    }
}

impl<T: WireSize> WireSize for Vec<T> {
    #[inline]
    fn is_fixed_size() -> bool { false }

    #[inline]
    fn fixed_size() -> Option<usize> { None }

    #[inline]
    fn wire_size(&self) -> usize {
        if T::is_fixed_size() {
            let element_size = T::fixed_size().unwrap();
            VEC_COUNT_SIZE + (self.len() * element_size)
        } else {
            let offsets_size = self.len() * OFFSET_SIZE;
            let elements_size: usize = self.iter().map(|e| e.wire_size()).sum();
            VEC_COUNT_SIZE + offsets_size + elements_size
        }
    }
}

impl<T: WireEncode> WireEncode for Vec<T> {
    fn encode_to(&self, buf: &mut [u8]) -> usize {
        let count = self.len() as u32;
        buf[..VEC_COUNT_SIZE].copy_from_slice(&count.to_le_bytes());

        if T::is_fixed_size() {
            let element_size = T::fixed_size().unwrap();
            let mut offset = VEC_COUNT_SIZE;
            for element in self {
                element.encode_to(&mut buf[offset..]);
                offset += element_size;
            }
            offset
        } else {
            let offsets_start = VEC_COUNT_SIZE;
            let offsets_size = self.len() * OFFSET_SIZE;
            let data_start = offsets_start + offsets_size;

            let mut data_offset = data_start;
            for (index, element) in self.iter().enumerate() {
                let relative_offset = (data_offset - offsets_start) as u32;
                let offset_pos = offsets_start + (index * OFFSET_SIZE);
                buf[offset_pos..offset_pos + OFFSET_SIZE].copy_from_slice(&relative_offset.to_le_bytes());

                let written = element.encode_to(&mut buf[data_offset..]);
                data_offset += written;
            }
            data_offset
        }
    }
}

impl<T: WireSize> WireSize for [T] {
    #[inline]
    fn wire_size(&self) -> usize {
        if T::is_fixed_size() {
            let element_size = T::fixed_size().unwrap();
            VEC_COUNT_SIZE + (self.len() * element_size)
        } else {
            let offsets_size = self.len() * OFFSET_SIZE;
            let elements_size: usize = self.iter().map(|e| e.wire_size()).sum();
            VEC_COUNT_SIZE + offsets_size + elements_size
        }
    }
}

impl<T: WireEncode> WireEncode for [T] {
    fn encode_to(&self, buf: &mut [u8]) -> usize {
        let count = self.len() as u32;
        buf[..VEC_COUNT_SIZE].copy_from_slice(&count.to_le_bytes());

        if T::is_fixed_size() {
            let element_size = T::fixed_size().unwrap();
            let mut offset = VEC_COUNT_SIZE;
            for element in self {
                element.encode_to(&mut buf[offset..]);
                offset += element_size;
            }
            offset
        } else {
            let offsets_start = VEC_COUNT_SIZE;
            let offsets_size = self.len() * OFFSET_SIZE;
            let data_start = offsets_start + offsets_size;

            let mut data_offset = data_start;
            for (index, element) in self.iter().enumerate() {
                let relative_offset = (data_offset - offsets_start) as u32;
                let offset_pos = offsets_start + (index * OFFSET_SIZE);
                buf[offset_pos..offset_pos + OFFSET_SIZE].copy_from_slice(&relative_offset.to_le_bytes());

                let written = element.encode_to(&mut buf[data_offset..]);
                data_offset += written;
            }
            data_offset
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_primitives() {
        let mut buf = [0u8; 8];

        let written = 42i32.encode_to(&mut buf);
        assert_eq!(written, 4);
        assert_eq!(&buf[..4], &[42, 0, 0, 0]);

        let written = 3.14f64.encode_to(&mut buf);
        assert_eq!(written, 8);

        let written = true.encode_to(&mut buf);
        assert_eq!(written, 1);
        assert_eq!(buf[0], 1);

        let written = false.encode_to(&mut buf);
        assert_eq!(written, 1);
        assert_eq!(buf[0], 0);
    }

    #[test]
    fn encode_string() {
        let mut buf = [0u8; 32];
        let s = "hello";

        let written = s.encode_to(&mut buf);
        assert_eq!(written, 9); // 4 (len) + 5 (bytes)
        assert_eq!(&buf[..4], &[5, 0, 0, 0]); // len = 5
        assert_eq!(&buf[4..9], b"hello");
    }

    #[test]
    fn encode_option_some() {
        let mut buf = [0u8; 16];
        let opt: Option<i32> = Some(42);

        let written = opt.encode_to(&mut buf);
        assert_eq!(written, 5); // 1 (flag) + 4 (i32)
        assert_eq!(buf[0], 1); // is_some
        assert_eq!(&buf[1..5], &[42, 0, 0, 0]);
    }

    #[test]
    fn encode_option_none() {
        let mut buf = [0u8; 16];
        let opt: Option<i32> = None;

        let written = opt.encode_to(&mut buf);
        assert_eq!(written, 1);
        assert_eq!(buf[0], 0);
    }

    #[test]
    fn encode_vec_fixed_size() {
        let mut buf = [0u8; 32];
        let vec: Vec<i32> = vec![1, 2, 3];

        let written = vec.encode_to(&mut buf);
        assert_eq!(written, 16); // 4 (count) + 3 * 4 (elements)
        assert_eq!(&buf[..4], &[3, 0, 0, 0]); // count = 3
        assert_eq!(&buf[4..8], &[1, 0, 0, 0]);
        assert_eq!(&buf[8..12], &[2, 0, 0, 0]);
        assert_eq!(&buf[12..16], &[3, 0, 0, 0]);
    }

    #[test]
    fn encode_vec_variable_size() {
        let mut buf = [0u8; 64];
        let vec: Vec<String> = vec!["hi".to_string(), "there".to_string()];

        let written = vec.encode_to(&mut buf);
        // count(4) + offsets(2*4) + "hi"(4+2) + "there"(4+5)
        assert_eq!(written, 4 + 8 + 6 + 9);
        assert_eq!(&buf[..4], &[2, 0, 0, 0]); // count = 2
    }

    #[test]
    fn wire_size_calculations() {
        assert_eq!(42i32.wire_size(), 4);
        assert_eq!("hello".wire_size(), 9);
        assert_eq!(Some(42i32).wire_size(), 5);
        assert_eq!(None::<i32>.wire_size(), 1);

        let vec: Vec<i32> = vec![1, 2, 3];
        assert_eq!(vec.wire_size(), 16);

        let vec: Vec<String> = vec!["hi".to_string(), "there".to_string()];
        assert_eq!(vec.wire_size(), 4 + 8 + 6 + 9);
    }
}
