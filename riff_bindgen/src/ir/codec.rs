use crate::ir::ids::{BuiltinId, CustomTypeId, EnumId, FieldName, RecordId, VariantName};
use crate::ir::types::PrimitiveType;

#[derive(Debug, Clone)]
pub enum CodecPlan {
    Primitive(PrimitiveType),
    String,
    Bytes,
    Builtin(BuiltinId),

    Option(Box<CodecPlan>),
    Vec {
        element: Box<CodecPlan>,
        layout: VecLayout,
    },
    Result {
        ok: Box<CodecPlan>,
        err: Box<CodecPlan>,
    },

    Record {
        id: RecordId,
        layout: RecordLayout,
    },
    Enum {
        id: EnumId,
        layout: EnumLayout,
    },
    Custom {
        id: CustomTypeId,
        underlying: Box<CodecPlan>,
    },
}

#[derive(Debug, Clone)]
pub enum VecLayout {
    Blittable { element_size: usize },
    Encoded,
}

#[derive(Debug, Clone)]
pub enum RecordLayout {
    Blittable {
        size: usize,
        fields: Vec<BlittableField>,
    },
    Encoded {
        fields: Vec<EncodedField>,
    },
}

#[derive(Debug, Clone)]
pub struct BlittableField {
    pub name: FieldName,
    pub offset: usize,
    pub primitive: PrimitiveType,
}

#[derive(Debug, Clone)]
pub struct EncodedField {
    pub name: FieldName,
    pub codec: CodecPlan,
}

#[derive(Debug, Clone)]
pub enum EnumLayout {
    CStyle {
        tag_type: PrimitiveType,
    },
    Data {
        tag_type: PrimitiveType,
        variants: Vec<VariantLayout>,
    },
}

#[derive(Debug, Clone)]
pub struct VariantLayout {
    pub name: VariantName,
    pub discriminant: i64,
    pub payload: VariantPayloadLayout,
}

#[derive(Debug, Clone)]
pub enum VariantPayloadLayout {
    Unit,
    Fields(Vec<EncodedField>),
}
