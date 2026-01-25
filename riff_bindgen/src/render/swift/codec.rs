use riff_ffi_rules::naming::to_upper_camel_case as pascal_case;

use crate::ir::codec::{CodecPlan, EnumLayout, RecordLayout, VecLayout};
use crate::ir::types::PrimitiveType;

const OFFSET_VAR: &str = "pos";

pub fn swift_type(codec: &CodecPlan) -> String {
    match codec {
        CodecPlan::Void => "Void".to_string(),
        CodecPlan::Primitive(p) => swift_primitive(*p),
        CodecPlan::String => "String".to_string(),
        CodecPlan::Bytes => "Data".to_string(),
        CodecPlan::Builtin(id) => swift_builtin(id.as_str()),
        CodecPlan::Option(inner) => format!("{}?", swift_type(inner)),
        CodecPlan::Vec { element, .. } => format!("[{}]", swift_type(element)),
        CodecPlan::Result { ok, err } => {
            format!("Result<{}, {}>", swift_type(ok), swift_type(err))
        }
        CodecPlan::Record { id, .. } => pascal_case(id.as_str()),
        CodecPlan::Enum { id, .. } => pascal_case(id.as_str()),
        CodecPlan::Custom { id, .. } => pascal_case(id.as_str()),
    }
}

pub fn swift_primitive(p: PrimitiveType) -> String {
    match p {
        PrimitiveType::Bool => "Bool",
        PrimitiveType::I8 => "Int8",
        PrimitiveType::U8 => "UInt8",
        PrimitiveType::I16 => "Int16",
        PrimitiveType::U16 => "UInt16",
        PrimitiveType::I32 => "Int32",
        PrimitiveType::U32 => "UInt32",
        PrimitiveType::I64 => "Int64",
        PrimitiveType::U64 => "UInt64",
        PrimitiveType::F32 => "Float",
        PrimitiveType::F64 => "Double",
    }
    .to_string()
}

pub fn swift_builtin(id: &str) -> String {
    match id {
        "Duration" => "TimeInterval",
        "SystemTime" => "Date",
        "Uuid" => "UUID",
        "Url" => "URL",
        other => other,
    }
    .to_string()
}

pub fn decode_inline(codec: &CodecPlan) -> String {
    let (reader, size_kind) = decode_expr(codec);
    match size_kind {
        SizeKind::Fixed(size) => {
            format!("{{ let v = {}; {} += {}; return v }}()", reader, OFFSET_VAR, size)
        }
        SizeKind::Variable => {
            format!("{{ let (v, s) = {}; {} += s; return v }}()", reader, OFFSET_VAR)
        }
    }
}

pub fn size_expr(codec: &CodecPlan, name: &str) -> String {
    encode_info(codec, name).0
}

pub fn encode_data(codec: &CodecPlan, name: &str) -> String {
    encode_info(codec, name).1
}

pub fn encode_bytes(codec: &CodecPlan, name: &str) -> String {
    encode_info(codec, name).2
}

enum SizeKind {
    Fixed(usize),
    Variable,
}

fn decode_expr(codec: &CodecPlan) -> (String, SizeKind) {
    match codec {
        CodecPlan::Void => ("()".to_string(), SizeKind::Fixed(0)),
        CodecPlan::Primitive(p) => decode_primitive(*p),
        CodecPlan::String => (
            format!("wire.readString(at: {})", OFFSET_VAR),
            SizeKind::Variable,
        ),
        CodecPlan::Bytes => (
            format!("wire.readBytesWithSize(at: {})", OFFSET_VAR),
            SizeKind::Variable,
        ),
        CodecPlan::Builtin(id) => decode_builtin(id.as_str()),
        CodecPlan::Option(inner) => decode_option(inner),
        CodecPlan::Vec { element, layout } => decode_vec(element, layout),
        CodecPlan::Result { ok, err } => decode_result(ok, err),
        CodecPlan::Record { id, layout } => decode_record(id.as_str(), layout),
        CodecPlan::Enum { id, layout } => decode_enum(id.as_str(), layout),
        CodecPlan::Custom { underlying, .. } => decode_expr(underlying),
    }
}

fn decode_primitive(p: PrimitiveType) -> (String, SizeKind) {
    let (read_fn, size) = match p {
        PrimitiveType::Bool => ("readBool", 1),
        PrimitiveType::I8 => ("readI8", 1),
        PrimitiveType::U8 => ("readU8", 1),
        PrimitiveType::I16 => ("readI16", 2),
        PrimitiveType::U16 => ("readU16", 2),
        PrimitiveType::I32 => ("readI32", 4),
        PrimitiveType::U32 => ("readU32", 4),
        PrimitiveType::I64 => ("readI64", 8),
        PrimitiveType::U64 => ("readU64", 8),
        PrimitiveType::F32 => ("readF32", 4),
        PrimitiveType::F64 => ("readF64", 8),
    };
    (
        format!("wire.{}(at: {})", read_fn, OFFSET_VAR),
        SizeKind::Fixed(size),
    )
}

fn decode_builtin(id: &str) -> (String, SizeKind) {
    match id {
        "Duration" => (
            format!("wire.readDuration(at: {})", OFFSET_VAR),
            SizeKind::Fixed(12),
        ),
        "SystemTime" => (
            format!("wire.readTimestamp(at: {})", OFFSET_VAR),
            SizeKind::Fixed(12),
        ),
        "Uuid" => (
            format!("wire.readUuid(at: {})", OFFSET_VAR),
            SizeKind::Fixed(16),
        ),
        "Url" => (
            format!("wire.readUrl(at: {})", OFFSET_VAR),
            SizeKind::Variable,
        ),
        _ => (
            format!("wire.read{}(at: {})", pascal_case(id), OFFSET_VAR),
            SizeKind::Variable,
        ),
    }
}

fn decode_record(name: &str, layout: &RecordLayout) -> (String, SizeKind) {
    let class_name = pascal_case(name);
    match layout {
        RecordLayout::Blittable { size, .. } => (
            format!("wire.readBlittable(at: {}, as: {}.self)", OFFSET_VAR, class_name),
            SizeKind::Fixed(*size),
        ),
        RecordLayout::Encoded { .. } | RecordLayout::Recursive => (
            format!("{}.decode(wireBuffer: wire, at: {})", class_name, OFFSET_VAR),
            SizeKind::Variable,
        ),
    }
}

fn decode_enum(name: &str, layout: &EnumLayout) -> (String, SizeKind) {
    let class_name = pascal_case(name);
    match layout {
        EnumLayout::CStyle { .. } => (
            format!("{}(fromC: wire.readI32(at: {}))", class_name, OFFSET_VAR),
            SizeKind::Fixed(4),
        ),
        EnumLayout::Data { .. } | EnumLayout::Recursive => (
            format!("{}.decode(wireBuffer: wire, at: {})", class_name, OFFSET_VAR),
            SizeKind::Variable,
        ),
    }
}

fn decode_vec(element: &CodecPlan, layout: &VecLayout) -> (String, SizeKind) {
    if matches!(element, CodecPlan::Primitive(PrimitiveType::U8)) {
        return (
            format!("wire.readBytesWithSize(at: {})", OFFSET_VAR),
            SizeKind::Variable,
        );
    }

    match layout {
        VecLayout::Blittable { .. } => {
            let element_type = swift_type(element);
            (
                format!("wire.readBlittableArrayWithSize(at: {}, as: {}.self)", OFFSET_VAR, element_type),
                SizeKind::Variable,
            )
        }
        VecLayout::Encoded => {
            let (inner_reader, inner_size) = decode_expr(element);
            let tuple_reader = match inner_size {
                SizeKind::Fixed(size) => format!("({}, {})", inner_reader.replace(OFFSET_VAR, "$0"), size),
                SizeKind::Variable => inner_reader.replace(OFFSET_VAR, "$0"),
            };
            (
                format!("wire.readArray(at: {}, reader: {{ {} }})", OFFSET_VAR, tuple_reader),
                SizeKind::Variable,
            )
        }
    }
}

fn decode_option(inner: &CodecPlan) -> (String, SizeKind) {
    let (inner_reader, inner_size) = decode_expr(inner);
    let tuple_reader = match inner_size {
        SizeKind::Fixed(size) => format!("({}, {})", inner_reader.replace(OFFSET_VAR, "$0"), size),
        SizeKind::Variable => inner_reader.replace(OFFSET_VAR, "$0"),
    };
    (
        format!("wire.readOptional(at: {}, reader: {{ {} }})", OFFSET_VAR, tuple_reader),
        SizeKind::Variable,
    )
}

fn decode_result(ok: &CodecPlan, err: &CodecPlan) -> (String, SizeKind) {
    let (ok_reader, ok_size) = decode_expr(ok);
    let (err_reader, err_size) = decode_expr(err);
    
    let ok_tuple = match ok_size {
        SizeKind::Fixed(size) => format!("({}, {})", ok_reader.replace(OFFSET_VAR, "$0"), size),
        SizeKind::Variable => ok_reader.replace(OFFSET_VAR, "$0"),
    };
    let err_tuple = match err_size {
        SizeKind::Fixed(size) => format!("({}, {})", err_reader.replace(OFFSET_VAR, "$0"), size),
        SizeKind::Variable => err_reader.replace(OFFSET_VAR, "$0"),
    };
    
    (
        format!("wire.readResult(at: {}, okReader: {{ {} }}, errReader: {{ {} }})", OFFSET_VAR, ok_tuple, err_tuple),
        SizeKind::Variable,
    )
}

fn encode_info(codec: &CodecPlan, name: &str) -> (String, String, String) {
    match codec {
        CodecPlan::Void => ("0".to_string(), String::new(), String::new()),
        CodecPlan::Primitive(p) => encode_primitive(*p, name),
        CodecPlan::String => (
            format!("(4 + {}.utf8.count)", name),
            format!("data.appendString({})", name),
            format!("bytes.appendString({})", name),
        ),
        CodecPlan::Bytes => (
            format!("(4 + {}.count)", name),
            format!("data.appendBytes({})", name),
            format!("bytes.appendBytes({})", name),
        ),
        CodecPlan::Builtin(id) => encode_builtin(id.as_str(), name),
        CodecPlan::Option(inner) => encode_option(inner, name),
        CodecPlan::Vec { element, layout } => encode_vec(element, layout, name),
        CodecPlan::Result { ok, err } => encode_result(ok, err, name),
        CodecPlan::Record { layout, .. } => encode_record(layout, name),
        CodecPlan::Enum { layout, .. } => encode_enum(layout, name),
        CodecPlan::Custom { underlying, .. } => encode_info(underlying, name),
    }
}

fn encode_primitive(p: PrimitiveType, name: &str) -> (String, String, String) {
    let (append_fn, size) = match p {
        PrimitiveType::Bool => ("appendBool", 1),
        PrimitiveType::I8 => ("appendI8", 1),
        PrimitiveType::U8 => ("appendU8", 1),
        PrimitiveType::I16 => ("appendI16", 2),
        PrimitiveType::U16 => ("appendU16", 2),
        PrimitiveType::I32 => ("appendI32", 4),
        PrimitiveType::U32 => ("appendU32", 4),
        PrimitiveType::I64 => ("appendI64", 8),
        PrimitiveType::U64 => ("appendU64", 8),
        PrimitiveType::F32 => ("appendF32", 4),
        PrimitiveType::F64 => ("appendF64", 8),
    };
    (
        size.to_string(),
        format!("data.{}({})", append_fn, name),
        format!("bytes.{}({})", append_fn, name),
    )
}

fn encode_builtin(id: &str, name: &str) -> (String, String, String) {
    match id {
        "Duration" => (
            "12".to_string(),
            format!("data.appendDuration({})", name),
            format!("bytes.appendDuration({})", name),
        ),
        "SystemTime" => (
            "12".to_string(),
            format!("data.appendTimestamp({})", name),
            format!("bytes.appendTimestamp({})", name),
        ),
        "Uuid" => (
            "16".to_string(),
            format!("data.appendUuid({})", name),
            format!("bytes.appendUuid({})", name),
        ),
        "Url" => (
            format!("(4 + {}.absoluteString.utf8.count)", name),
            format!("data.appendString({}.absoluteString)", name),
            format!("bytes.appendString({}.absoluteString)", name),
        ),
        _ => (
            format!("{}.wireEncodedSize()", name),
            format!("{}.wireEncodeTo(&data)", name),
            format!("{}.wireEncodeToBytes(&bytes)", name),
        ),
    }
}

fn encode_record(layout: &RecordLayout, name: &str) -> (String, String, String) {
    match layout {
        RecordLayout::Blittable { size, .. } => (
            size.to_string(),
            format!("withUnsafeBytes(of: {}) {{ data.append(contentsOf: $0) }}", name),
            format!("withUnsafeBytes(of: {}) {{ bytes.append(contentsOf: $0) }}", name),
        ),
        RecordLayout::Encoded { .. } | RecordLayout::Recursive => (
            format!("{}.wireEncodedSize()", name),
            format!("{}.wireEncodeTo(&data)", name),
            format!("{}.wireEncodeToBytes(&bytes)", name),
        ),
    }
}

fn encode_enum(layout: &EnumLayout, name: &str) -> (String, String, String) {
    match layout {
        EnumLayout::CStyle { .. } => (
            "4".to_string(),
            format!("data.appendI32({}.rawValue)", name),
            format!("bytes.appendI32({}.rawValue)", name),
        ),
        EnumLayout::Data { .. } | EnumLayout::Recursive => (
            format!("{}.wireEncodedSize()", name),
            format!("{}.wireEncodeTo(&data)", name),
            format!("{}.wireEncodeToBytes(&bytes)", name),
        ),
    }
}

fn encode_vec(element: &CodecPlan, layout: &VecLayout, name: &str) -> (String, String, String) {
    if matches!(element, CodecPlan::Primitive(PrimitiveType::U8)) {
        return (
            format!("(4 + {}.count)", name),
            format!("data.appendBytes({})", name),
            format!("bytes.appendBytes({})", name),
        );
    }

    let (inner_size, inner_data, inner_bytes) = encode_info(element, "item");

    match layout {
        VecLayout::Blittable { element_size } => (
            format!("(4 + {}.count * {})", name, element_size),
            format!("data.appendBlittableArray({})", name),
            format!("bytes.appendBlittableArray({})", name),
        ),
        VecLayout::Encoded => (
            format!("(4 + {}.reduce(0) {{ $0 + {} }})", name, inner_size.replace("item", "$1")),
            format!("data.appendU32(UInt32({}.count)); for item in {} {{ {} }}", name, name, inner_data),
            format!("bytes.appendU32(UInt32({}.count)); for item in {} {{ {} }}", name, name, inner_bytes),
        ),
    }
}

fn encode_option(inner: &CodecPlan, name: &str) -> (String, String, String) {
    let (inner_size, inner_data, inner_bytes) = encode_info(inner, "v");
    (
        format!("({}.map {{ v in 1 + {} }} ?? 1)", name, inner_size),
        format!("if let v = {} {{ data.appendU8(1); {} }} else {{ data.appendU8(0) }}", name, inner_data),
        format!("if let v = {} {{ bytes.appendU8(1); {} }} else {{ bytes.appendU8(0) }}", name, inner_bytes),
    )
}

fn encode_result(ok: &CodecPlan, err: &CodecPlan, name: &str) -> (String, String, String) {
    let (ok_size, ok_data, ok_bytes) = encode_info(ok, "okVal");
    let (err_size, err_data, err_bytes) = encode_info(err, "errVal");
    (
        format!(
            "({{ switch {} {{ case .success(let okVal): return 1 + {}; case .failure(let errVal): return 1 + {} }} }}())",
            name, ok_size, err_size
        ),
        format!(
            "switch {} {{ case .success(let okVal): data.appendU8(0); {}; case .failure(let errVal): data.appendU8(1); {} }}",
            name, ok_data, err_data
        ),
        format!(
            "switch {} {{ case .success(let okVal): bytes.appendU8(0); {}; case .failure(let errVal): bytes.appendU8(1); {} }}",
            name, ok_bytes, err_bytes
        ),
    )
}
