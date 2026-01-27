use crate::ir::codec::{EnumLayout, VecLayout};
use crate::ir::ids::{BuiltinId, CustomTypeId, EnumId, FieldName, RecordId};
use crate::ir::types::{PrimitiveType, TypeExpr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireShape {
    Value,
    Optional,
    Sequence,
}

#[derive(Debug, Clone)]
pub enum SizeExpr {
    Fixed(usize),
    Runtime,
    StringLen(String),
    BytesLen(String),
    ValueSize(String),
    WireSize {
        value: String,
    },
    BuiltinSize {
        id: BuiltinId,
        value: String,
    },
    Sum(Vec<SizeExpr>),
    OptionSize {
        value: String,
        inner: Box<SizeExpr>,
    },
    VecSize {
        value: String,
        inner: Box<SizeExpr>,
        layout: VecLayout,
    },
    ResultSize {
        value: String,
        ok: Box<SizeExpr>,
        err: Box<SizeExpr>,
    },
}

#[derive(Debug, Clone)]
pub struct ReadSeq {
    pub size: SizeExpr,
    pub ops: Vec<ReadOp>,
    pub shape: WireShape,
}

#[derive(Debug, Clone)]
pub struct WriteSeq {
    pub size: SizeExpr,
    pub ops: Vec<WriteOp>,
    pub shape: WireShape,
}

#[derive(Debug, Clone)]
pub enum OffsetExpr {
    Fixed(usize),
    Base,
    BasePlus(usize),
    Var(String),
    VarPlus(String, usize),
}

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
        some: Box<ReadSeq>,
    },
    Vec {
        len_offset: OffsetExpr,
        element_type: TypeExpr,
        element: Box<ReadSeq>,
        layout: VecLayout,
    },
    Record {
        id: RecordId,
        offset: OffsetExpr,
        fields: Vec<FieldReadOp>,
    },
    Enum {
        id: EnumId,
        offset: OffsetExpr,
        layout: EnumLayout,
    },
    Result {
        tag_offset: OffsetExpr,
        ok: Box<ReadSeq>,
        err: Box<ReadSeq>,
    },
    Builtin {
        id: BuiltinId,
        offset: OffsetExpr,
    },
    Custom {
        id: CustomTypeId,
        underlying: Box<ReadSeq>,
    },
}

#[derive(Debug, Clone)]
pub enum WriteOp {
    Primitive {
        primitive: PrimitiveType,
        value: String,
    },
    String {
        value: String,
    },
    Bytes {
        value: String,
    },
    Option {
        value: String,
        some: Box<WriteSeq>,
    },
    Vec {
        value: String,
        element_type: TypeExpr,
        element: Box<WriteSeq>,
        layout: VecLayout,
    },
    Record {
        id: RecordId,
        value: String,
        fields: Vec<FieldWriteOp>,
    },
    Enum {
        id: EnumId,
        value: String,
        layout: EnumLayout,
    },
    Result {
        value: String,
        ok: Box<WriteSeq>,
        err: Box<WriteSeq>,
    },
    Builtin {
        id: BuiltinId,
        value: String,
    },
    Custom {
        id: CustomTypeId,
        value: String,
        underlying: Box<WriteSeq>,
    },
}

#[derive(Debug, Clone)]
pub struct FieldReadOp {
    pub name: FieldName,
    pub seq: ReadSeq,
}

#[derive(Debug, Clone)]
pub struct FieldWriteOp {
    pub name: FieldName,
    pub accessor: String,
    pub seq: WriteSeq,
}
