pub const MAGIC: u32 = 0x52494646;
pub const VERSION: u8 = 1;
pub const HEADER_SIZE: usize = 10; // magic(4) + version(1) + flags(1) + total_size(4)

pub const FLAGS_NONE: u8 = 0;

pub const FIELD_COUNT_SIZE: usize = 2; // u16
pub const OFFSET_SIZE: usize = 4; // u32
pub const STRING_LEN_SIZE: usize = 4; // u32
pub const VEC_COUNT_SIZE: usize = 4; // u32
pub const OPTION_FLAG_SIZE: usize = 1; // u8
pub const ENUM_TAG_SIZE: usize = 4; // u32
