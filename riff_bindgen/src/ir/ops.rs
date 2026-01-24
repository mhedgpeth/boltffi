use crate::ir::codec::{EnumLayout, VecLayout};
use crate::ir::ids::{BuiltinId, CustomTypeId, FieldName};
use crate::ir::types::PrimitiveType;

#[derive(Debug, Clone)]
pub enum ReadOp {
    Primitive {
        primitive: PrimitiveType,
        offset: OffsetExpr,
    },
    String {
        offset: OffsetExpr,
    },
    Bytes {
        offset: OffsetExpr,
    },
    Option {
        tag_offset: OffsetExpr,
        inner: Box<ReadOp>,
    },
    Vec {
        header_offset: OffsetExpr,
        element: Box<ReadOp>,
        layout: VecLayout,
    },
    Record {
        offset: OffsetExpr,
        fields: Vec<FieldReadOp>,
    },
    Enum {
        offset: OffsetExpr,
        layout: EnumLayout,
    },
    Result {
        offset: OffsetExpr,
        ok: Box<ReadOp>,
        err: Box<ReadOp>,
    },
    Builtin {
        id: BuiltinId,
        offset: OffsetExpr,
    },
    Custom {
        id: CustomTypeId,
        underlying: Box<ReadOp>,
    },
}

#[derive(Debug, Clone)]
pub enum WriteOp {
    Primitive {
        primitive: PrimitiveType,
    },
    String,
    Bytes,
    Option {
        inner: Box<WriteOp>,
    },
    Vec {
        element: Box<WriteOp>,
    },
    Record {
        fields: Vec<FieldWriteOp>,
    },
    Enum {
        layout: EnumLayout,
    },
    Result {
        ok: Box<WriteOp>,
        err: Box<WriteOp>,
    },
    Builtin {
        id: BuiltinId,
    },
    Custom {
        id: CustomTypeId,
        underlying: Box<WriteOp>,
    },
}

#[derive(Debug, Clone)]
pub enum OffsetExpr {
    Static(usize),
    Dynamic,
}

#[derive(Debug, Clone)]
pub struct FieldReadOp {
    pub name: FieldName,
    pub op: ReadOp,
}

#[derive(Debug, Clone)]
pub struct FieldWriteOp {
    pub name: FieldName,
    pub op: WriteOp,
}
