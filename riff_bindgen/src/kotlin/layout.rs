use crate::model::Primitive;

pub trait KotlinBufferRead {
    fn buffer_getter(&self) -> &'static str;
    fn buffer_conversion(&self) -> &'static str;
}

impl KotlinBufferRead for Primitive {
    fn buffer_getter(&self) -> &'static str {
        match self {
            Self::Bool | Self::I8 | Self::U8 => "get",
            Self::I16 | Self::U16 => "getShort",
            Self::I32 | Self::U32 => "getInt",
            Self::I64 | Self::U64 | Self::Usize | Self::Isize => "getLong",
            Self::F32 => "getFloat",
            Self::F64 => "getDouble",
        }
    }

    fn buffer_conversion(&self) -> &'static str {
        match self {
            Self::Bool => " != 0.toByte()",
            _ => "",
        }
    }
}
