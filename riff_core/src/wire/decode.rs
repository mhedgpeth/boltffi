use crate::wire::constants::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    BufferTooSmall,
    InvalidMagic,
    UnsupportedVersion,
    InvalidUtf8,
    InvalidBool,
}

pub type DecodeResult<T> = Result<(T, usize), DecodeError>;

pub trait WireDecode: Sized {
    fn decode_from(buf: &[u8]) -> DecodeResult<Self>;
}

macro_rules! impl_wire_decode_primitive {
    ($($ty:ty),*) => {
        $(
            impl WireDecode for $ty {
                #[inline]
                fn decode_from(buf: &[u8]) -> DecodeResult<Self> {
                    const SIZE: usize = core::mem::size_of::<$ty>();
                    if buf.len() < SIZE {
                        return Err(DecodeError::BufferTooSmall);
                    }
                    let bytes: [u8; SIZE] = buf[..SIZE].try_into().unwrap();
                    Ok((<$ty>::from_le_bytes(bytes), SIZE))
                }
            }
        )*
    };
}

impl_wire_decode_primitive!(i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);

impl WireDecode for bool {
    #[inline]
    fn decode_from(buf: &[u8]) -> DecodeResult<Self> {
        if buf.is_empty() {
            return Err(DecodeError::BufferTooSmall);
        }
        match buf[0] {
            0 => Ok((false, 1)),
            1 => Ok((true, 1)),
            _ => Err(DecodeError::InvalidBool),
        }
    }
}

impl WireDecode for isize {
    #[inline]
    fn decode_from(buf: &[u8]) -> DecodeResult<Self> {
        if buf.len() < 8 {
            return Err(DecodeError::BufferTooSmall);
        }
        let bytes: [u8; 8] = buf[..8].try_into().unwrap();
        let value = i64::from_le_bytes(bytes) as isize;
        Ok((value, 8))
    }
}

impl WireDecode for usize {
    #[inline]
    fn decode_from(buf: &[u8]) -> DecodeResult<Self> {
        if buf.len() < 8 {
            return Err(DecodeError::BufferTooSmall);
        }
        let bytes: [u8; 8] = buf[..8].try_into().unwrap();
        let value = u64::from_le_bytes(bytes) as usize;
        Ok((value, 8))
    }
}

impl WireDecode for String {
    fn decode_from(buf: &[u8]) -> DecodeResult<Self> {
        if buf.len() < STRING_LEN_SIZE {
            return Err(DecodeError::BufferTooSmall);
        }

        let len_bytes: [u8; 4] = buf[..STRING_LEN_SIZE].try_into().unwrap();
        let len = u32::from_le_bytes(len_bytes) as usize;

        let total_size = STRING_LEN_SIZE + len;
        if buf.len() < total_size {
            return Err(DecodeError::BufferTooSmall);
        }

        let string_bytes = &buf[STRING_LEN_SIZE..total_size];
        let string = String::from_utf8(string_bytes.to_vec())
            .map_err(|_| DecodeError::InvalidUtf8)?;

        Ok((string, total_size))
    }
}

impl<T: WireDecode> WireDecode for Option<T> {
    fn decode_from(buf: &[u8]) -> DecodeResult<Self> {
        if buf.is_empty() {
            return Err(DecodeError::BufferTooSmall);
        }

        match buf[0] {
            0 => Ok((None, OPTION_FLAG_SIZE)),
            1 => {
                let (value, value_size) = T::decode_from(&buf[OPTION_FLAG_SIZE..])?;
                Ok((Some(value), OPTION_FLAG_SIZE + value_size))
            }
            _ => Err(DecodeError::InvalidBool),
        }
    }
}

pub trait FixedSizeWireDecode: Sized {
    const WIRE_SIZE: usize;
    fn decode_fixed(buf: &[u8]) -> Result<Self, DecodeError>;
}

macro_rules! impl_fixed_size_decode {
    ($($ty:ty),*) => {
        $(
            impl FixedSizeWireDecode for $ty {
                const WIRE_SIZE: usize = core::mem::size_of::<$ty>();

                #[inline]
                fn decode_fixed(buf: &[u8]) -> Result<Self, DecodeError> {
                    if buf.len() < Self::WIRE_SIZE {
                        return Err(DecodeError::BufferTooSmall);
                    }
                    let bytes: [u8; Self::WIRE_SIZE] = buf[..Self::WIRE_SIZE].try_into().unwrap();
                    Ok(<$ty>::from_le_bytes(bytes))
                }
            }
        )*
    };
}

impl_fixed_size_decode!(i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);

impl FixedSizeWireDecode for bool {
    const WIRE_SIZE: usize = 1;

    #[inline]
    fn decode_fixed(buf: &[u8]) -> Result<Self, DecodeError> {
        if buf.is_empty() {
            return Err(DecodeError::BufferTooSmall);
        }
        match buf[0] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(DecodeError::InvalidBool),
        }
    }
}

impl FixedSizeWireDecode for isize {
    const WIRE_SIZE: usize = 8;

    #[inline]
    fn decode_fixed(buf: &[u8]) -> Result<Self, DecodeError> {
        if buf.len() < 8 {
            return Err(DecodeError::BufferTooSmall);
        }
        let bytes: [u8; 8] = buf[..8].try_into().unwrap();
        Ok(i64::from_le_bytes(bytes) as isize)
    }
}

impl FixedSizeWireDecode for usize {
    const WIRE_SIZE: usize = 8;

    #[inline]
    fn decode_fixed(buf: &[u8]) -> Result<Self, DecodeError> {
        if buf.len() < 8 {
            return Err(DecodeError::BufferTooSmall);
        }
        let bytes: [u8; 8] = buf[..8].try_into().unwrap();
        Ok(u64::from_le_bytes(bytes) as usize)
    }
}

impl<T: WireDecode + crate::wire::encode::WireSize> WireDecode for Vec<T> {
    fn decode_from(buf: &[u8]) -> DecodeResult<Self> {
        if buf.len() < VEC_COUNT_SIZE {
            return Err(DecodeError::BufferTooSmall);
        }

        let count_bytes: [u8; 4] = buf[..VEC_COUNT_SIZE].try_into().unwrap();
        let count = u32::from_le_bytes(count_bytes) as usize;

        if count == 0 {
            return Ok((Vec::new(), VEC_COUNT_SIZE));
        }

        let mut result = Vec::with_capacity(count);

        if T::is_fixed_size() {
            let element_size = T::fixed_size().unwrap();
            let mut offset = VEC_COUNT_SIZE;

            for _ in 0..count {
                if buf.len() < offset + element_size {
                    return Err(DecodeError::BufferTooSmall);
                }
                let (element, _) = T::decode_from(&buf[offset..])?;
                result.push(element);
                offset += element_size;
            }
            Ok((result, offset))
        } else {
            let offsets_start = VEC_COUNT_SIZE;
            let offsets_size = count * OFFSET_SIZE;

            if buf.len() < offsets_start + offsets_size {
                return Err(DecodeError::BufferTooSmall);
            }

            let mut max_end = offsets_start + offsets_size;

            for i in 0..count {
                let offset_pos = offsets_start + (i * OFFSET_SIZE);
                let offset_bytes: [u8; 4] = buf[offset_pos..offset_pos + OFFSET_SIZE].try_into().unwrap();
                let relative_offset = u32::from_le_bytes(offset_bytes) as usize;
                let element_offset = offsets_start + relative_offset;

                let (element, element_size) = T::decode_from(&buf[element_offset..])?;
                result.push(element);

                let element_end = element_offset + element_size;
                if element_end > max_end {
                    max_end = element_end;
                }
            }
            Ok((result, max_end))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::encode::WireEncode;

    #[test]
    fn decode_primitives() {
        let mut buf = [0u8; 8];

        42i32.encode_to(&mut buf);
        let (value, size) = i32::decode_from(&buf).unwrap();
        assert_eq!(value, 42);
        assert_eq!(size, 4);

        3.14f64.encode_to(&mut buf);
        let (value, size) = f64::decode_from(&buf).unwrap();
        assert!((value - 3.14).abs() < f64::EPSILON);
        assert_eq!(size, 8);

        true.encode_to(&mut buf);
        let (value, size) = bool::decode_from(&buf).unwrap();
        assert!(value);
        assert_eq!(size, 1);
    }

    #[test]
    fn decode_string() {
        let mut buf = [0u8; 32];
        let original = "hello".to_string();

        original.encode_to(&mut buf);
        let (decoded, size) = String::decode_from(&buf).unwrap();
        assert_eq!(decoded, "hello");
        assert_eq!(size, 9);
    }

    #[test]
    fn decode_option() {
        let mut buf = [0u8; 16];

        Some(42i32).encode_to(&mut buf);
        let (decoded, size) = Option::<i32>::decode_from(&buf).unwrap();
        assert_eq!(decoded, Some(42));
        assert_eq!(size, 5);

        None::<i32>.encode_to(&mut buf);
        let (decoded, size) = Option::<i32>::decode_from(&buf).unwrap();
        assert_eq!(decoded, None);
        assert_eq!(size, 1);
    }

    #[test]
    fn decode_vec_fixed() {
        let mut buf = [0u8; 32];
        let original = vec![1i32, 2, 3];

        original.encode_to(&mut buf);
        let (decoded, size) = Vec::<i32>::decode_from(&buf).unwrap();
        assert_eq!(decoded, vec![1, 2, 3]);
        assert_eq!(size, 16);
    }

    #[test]
    fn decode_vec_variable() {
        let mut buf = [0u8; 64];
        let original = vec!["hi".to_string(), "there".to_string()];

        let written = original.encode_to(&mut buf);
        let (decoded, size) = Vec::<String>::decode_from(&buf).unwrap();
        assert_eq!(decoded, vec!["hi".to_string(), "there".to_string()]);
        assert_eq!(size, written);
    }

    #[test]
    fn roundtrip_complex() {
        let mut buf = [0u8; 128];

        let original: Vec<Option<String>> = vec![
            Some("hello".to_string()),
            None,
            Some("world".to_string()),
        ];

        let written = original.encode_to(&mut buf);
        let (decoded, size) = Vec::<Option<String>>::decode_from(&buf).unwrap();
        assert_eq!(decoded, original);
        assert_eq!(size, written);
    }
}
