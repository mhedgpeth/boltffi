use boltffi_ffi_rules::naming::snake_to_camel as camel_case;

use crate::ir::codec::{EnumLayout, VecLayout};
use crate::ir::ids::BuiltinId;
use crate::ir::ops::{ReadOp, ReadSeq, SizeExpr, ValueExpr, WriteOp, WriteSeq};
use crate::ir::types::{PrimitiveType, TypeExpr};

const TS_KEYWORDS: &[&str] = &[
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "null",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
    "let",
    "static",
    "implements",
    "interface",
    "package",
    "private",
    "protected",
    "public",
    "type",
];

pub fn escape_ts_keyword(name: &str) -> String {
    if TS_KEYWORDS.contains(&name) {
        format!("{}_", name)
    } else {
        name.to_string()
    }
}

pub fn ts_type(type_expr: &TypeExpr) -> String {
    match type_expr {
        TypeExpr::Void => "void".to_string(),
        TypeExpr::Primitive(p) => ts_primitive(*p),
        TypeExpr::String => "string".to_string(),
        TypeExpr::Bytes => "Uint8Array".to_string(),
        TypeExpr::Builtin(id) => ts_builtin(id),
        TypeExpr::Option(inner) => format!("{} | null", ts_type(inner)),
        TypeExpr::Vec(inner) => {
            if matches!(inner.as_ref(), TypeExpr::Primitive(PrimitiveType::U8)) {
                "Uint8Array".to_string()
            } else {
                format!("{}[]", ts_type(inner))
            }
        }
        TypeExpr::Result { ok, .. } => ts_type(ok),
        TypeExpr::Record(id) => to_pascal_case(id.as_str()),
        TypeExpr::Enum(id) => to_pascal_case(id.as_str()),
        TypeExpr::Custom(id) => to_pascal_case(id.as_str()),
        TypeExpr::Handle(id) => to_pascal_case(id.as_str()),
        TypeExpr::Callback(id) => to_pascal_case(id.as_str()),
    }
}

pub fn ts_primitive(primitive: PrimitiveType) -> String {
    match primitive {
        PrimitiveType::Bool => "boolean",
        PrimitiveType::I8 | PrimitiveType::U8 => "number",
        PrimitiveType::I16 | PrimitiveType::U16 => "number",
        PrimitiveType::I32 | PrimitiveType::U32 => "number",
        PrimitiveType::I64 | PrimitiveType::U64 => "bigint",
        PrimitiveType::ISize | PrimitiveType::USize => "number",
        PrimitiveType::F32 | PrimitiveType::F64 => "number",
    }
    .to_string()
}

pub fn ts_builtin(id: &BuiltinId) -> String {
    match id.as_str() {
        "Duration" => "Duration",
        "SystemTime" => "Date",
        "Uuid" => "string",
        "Url" => "string",
        other => other,
    }
    .to_string()
}

pub fn render_value(expr: &ValueExpr) -> String {
    match expr {
        ValueExpr::Instance => "this".to_string(),
        ValueExpr::Var(name) => name.clone(),
        ValueExpr::Named(name) => camel_case(name),
        ValueExpr::Field(parent, field) => {
            format!("{}.{}", render_value(parent), camel_case(field.as_str()))
        }
    }
}

pub fn emit_reader_read(seq: &ReadSeq) -> String {
    seq.ops.first().map(emit_reader_read_op).unwrap_or_default()
}

fn emit_reader_read_op(op: &ReadOp) -> String {
    match op {
        ReadOp::Primitive { primitive, .. } => match primitive {
            PrimitiveType::Bool => "reader.readBool()".into(),
            PrimitiveType::I8 => "reader.readI8()".into(),
            PrimitiveType::U8 => "reader.readU8()".into(),
            PrimitiveType::I16 => "reader.readI16()".into(),
            PrimitiveType::U16 => "reader.readU16()".into(),
            PrimitiveType::I32 => "reader.readI32()".into(),
            PrimitiveType::U32 => "reader.readU32()".into(),
            PrimitiveType::I64 => "reader.readI64()".into(),
            PrimitiveType::U64 => "reader.readU64()".into(),
            PrimitiveType::ISize => "reader.readISize()".into(),
            PrimitiveType::USize => "reader.readUSize()".into(),
            PrimitiveType::F32 => "reader.readF32()".into(),
            PrimitiveType::F64 => "reader.readF64()".into(),
        },
        ReadOp::String { .. } => "reader.readString()".into(),
        ReadOp::Bytes { .. } => "reader.readBytes()".into(),
        ReadOp::Builtin { id, .. } => match id.as_str() {
            "Duration" => "reader.readDuration()".into(),
            "SystemTime" => "reader.readTimestamp()".into(),
            "Uuid" => "reader.readUuid()".into(),
            "Url" => "reader.readUrl()".into(),
            other => format!("reader.read{}()", to_pascal_case(other)),
        },
        ReadOp::Option { some, .. } => {
            let inner = emit_reader_read(some);
            format!("reader.readOptional(() => {})", inner)
        }
        ReadOp::Vec {
            element_type,
            element,
            layout,
            ..
        } => {
            if matches!(element_type, TypeExpr::Primitive(PrimitiveType::U8)) {
                return "reader.readBytes()".into();
            }
            match layout {
                VecLayout::Blittable { element_size } => {
                    format!("reader.readBlittableArray({})", element_size)
                }
                VecLayout::Encoded => {
                    let inner = emit_reader_read(element);
                    format!("reader.readArray(() => {})", inner)
                }
            }
        }
        ReadOp::Record { id, .. } => {
            format!("decode{}(reader)", to_pascal_case(id.as_str()))
        }
        ReadOp::Enum { id, layout, .. } => match layout {
            EnumLayout::CStyle { .. } => {
                format!("decode{}(reader.readI32())", to_pascal_case(id.as_str()))
            }
            EnumLayout::Data { .. } | EnumLayout::Recursive => {
                format!("decode{}(reader)", to_pascal_case(id.as_str()))
            }
        },
        ReadOp::Result { ok, err, .. } => {
            let ok_read = emit_reader_read(ok);
            let err_read = emit_reader_read(err);
            format!(
                "reader.readResult(() => {}, () => {})",
                ok_read, err_read
            )
        }
        ReadOp::Custom { underlying, .. } => emit_reader_read(underlying),
    }
}

pub fn emit_writer_write(seq: &WriteSeq) -> String {
    seq.ops
        .iter()
        .map(emit_writer_write_op)
        .collect::<Vec<_>>()
        .join("; ")
}

fn emit_writer_write_op(op: &WriteOp) -> String {
    match op {
        WriteOp::Primitive { primitive, value } => {
            let val = render_value(value);
            match primitive {
                PrimitiveType::Bool => format!("writer.writeBool({})", val),
                PrimitiveType::I8 => format!("writer.writeI8({})", val),
                PrimitiveType::U8 => format!("writer.writeU8({})", val),
                PrimitiveType::I16 => format!("writer.writeI16({})", val),
                PrimitiveType::U16 => format!("writer.writeU16({})", val),
                PrimitiveType::I32 => format!("writer.writeI32({})", val),
                PrimitiveType::U32 => format!("writer.writeU32({})", val),
                PrimitiveType::I64 => format!("writer.writeI64({})", val),
                PrimitiveType::U64 => format!("writer.writeU64({})", val),
                PrimitiveType::ISize => format!("writer.writeISize({})", val),
                PrimitiveType::USize => format!("writer.writeUSize({})", val),
                PrimitiveType::F32 => format!("writer.writeF32({})", val),
                PrimitiveType::F64 => format!("writer.writeF64({})", val),
            }
        }
        WriteOp::String { value } => format!("writer.writeString({})", render_value(value)),
        WriteOp::Bytes { value } => format!("writer.writeBytes({})", render_value(value)),
        WriteOp::Builtin { id, value } => {
            let val = render_value(value);
            match id.as_str() {
                "Duration" => format!("writer.writeDuration({})", val),
                "SystemTime" => format!("writer.writeTimestamp({})", val),
                "Uuid" => format!("writer.writeUuid({})", val),
                "Url" => format!("writer.writeString({})", val),
                _ => format!("encode{}(writer, {})", to_pascal_case(id.as_str()), val),
            }
        }
        WriteOp::Option { value, some } => {
            let inner = emit_writer_write(some);
            format!(
                "writer.writeOptional({}, (v) => {{ {} }})",
                render_value(value),
                inner
            )
        }
        WriteOp::Vec {
            value,
            element_type,
            element,
            layout,
        } => {
            let val = render_value(value);
            if matches!(element_type, TypeExpr::Primitive(PrimitiveType::U8)) {
                return format!("writer.writeBytes({})", val);
            }
            match layout {
                VecLayout::Blittable { element_size } => {
                    format!("writer.writeBlittableArray({}, {})", val, element_size)
                }
                VecLayout::Encoded => {
                    let inner = emit_writer_write(element);
                    format!(
                        "writer.writeArray({}, (item) => {{ {} }})",
                        val, inner
                    )
                }
            }
        }
        WriteOp::Record { id, value, .. } => {
            format!("encode{}(writer, {})", to_pascal_case(id.as_str()), render_value(value))
        }
        WriteOp::Enum { id, value, layout, .. } => {
            let val = render_value(value);
            match layout {
                EnumLayout::CStyle { .. } => {
                    format!("writer.writeI32({}.value)", val)
                }
                EnumLayout::Data { .. } | EnumLayout::Recursive => {
                    format!("encode{}(writer, {})", to_pascal_case(id.as_str()), val)
                }
            }
        }
        WriteOp::Result { value, ok, err } => {
            let val = render_value(value);
            let ok_write = emit_writer_write(ok);
            let err_write = emit_writer_write(err);
            format!(
                "writer.writeResult({}, () => {{ {} }}, () => {{ {} }})",
                val, ok_write, err_write
            )
        }
        WriteOp::Custom { underlying, .. } => emit_writer_write(underlying),
    }
}

pub fn emit_size_expr(size: &SizeExpr) -> String {
    match size {
        SizeExpr::Fixed(value) => value.to_string(),
        SizeExpr::Runtime => "0".to_string(),
        SizeExpr::StringLen(value) => {
            format!("wireStringSize({})", render_value(value))
        }
        SizeExpr::BytesLen(value) => {
            format!("(4 + {}.byteLength)", render_value(value))
        }
        SizeExpr::ValueSize(expr) => render_value(expr),
        SizeExpr::WireSize { value } => {
            format!("wireSize({})", render_value(value))
        }
        SizeExpr::BuiltinSize { id, value } => {
            let val = render_value(value);
            match id.as_str() {
                "Url" => format!("wireStringSize({})", val),
                "Duration" => "12".to_string(),
                "SystemTime" => "12".to_string(),
                "Uuid" => "16".to_string(),
                _ => format!("wireSize({})", val),
            }
        }
        SizeExpr::Sum(parts) => {
            let rendered = parts
                .iter()
                .map(emit_size_expr)
                .collect::<Vec<_>>()
                .join(" + ");
            format!("({})", rendered)
        }
        SizeExpr::OptionSize { value, inner } => {
            let inner_size = emit_size_expr(inner);
            format!(
                "({} !== null ? 1 + {} : 1)",
                render_value(value),
                inner_size
            )
        }
        SizeExpr::VecSize {
            value,
            inner,
            layout,
        } => {
            let val = render_value(value);
            match layout {
                VecLayout::Blittable { element_size } => {
                    format!("(4 + {}.length * {})", val, element_size)
                }
                VecLayout::Encoded => {
                    let inner_size = emit_size_expr(inner);
                    format!("(4 + {}.length * {})", val, inner_size)
                }
            }
        }
        SizeExpr::ResultSize { value, ok, err } => {
            let val = render_value(value);
            let ok_size = emit_size_expr(ok);
            let err_size = emit_size_expr(err);
            format!("(1 + ({} instanceof Error ? {} : {}))", val, err_size, ok_size)
        }
    }
}

fn to_pascal_case(name: &str) -> String {
    boltffi_ffi_rules::naming::to_upper_camel_case(name)
}
