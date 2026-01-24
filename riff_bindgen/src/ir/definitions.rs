use crate::ir::ids::{
    CallbackId, ClassId, ConverterPath, CustomTypeId, EnumId, FieldName, FunctionId, MethodId,
    ParamName, QualifiedName, RecordId, VariantName,
};
use crate::ir::types::{PrimitiveType, TypeExpr};

#[derive(Debug, Clone)]
pub struct DeprecationInfo {
    pub message: Option<String>,
    pub since: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RecordDef {
    pub id: RecordId,
    pub fields: Vec<FieldDef>,
    pub doc: Option<String>,
    pub deprecated: Option<DeprecationInfo>,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: FieldName,
    pub type_expr: TypeExpr,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub id: EnumId,
    pub repr: EnumRepr,
    pub is_error: bool,
    pub doc: Option<String>,
    pub deprecated: Option<DeprecationInfo>,
}

#[derive(Debug, Clone)]
pub enum EnumRepr {
    CStyle {
        tag_type: PrimitiveType,
        variants: Vec<CStyleVariant>,
    },
    Data {
        tag_type: PrimitiveType,
        variants: Vec<DataVariant>,
    },
}

#[derive(Debug, Clone)]
pub struct CStyleVariant {
    pub name: VariantName,
    pub discriminant: i64,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DataVariant {
    pub name: VariantName,
    pub discriminant: i64,
    pub payload: VariantPayload,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub enum VariantPayload {
    Unit,
    Tuple(Vec<TypeExpr>),
    Struct(Vec<FieldDef>),
}

#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub id: FunctionId,
    pub params: Vec<ParamDef>,
    pub returns: ReturnDef,
    pub is_async: bool,
    pub doc: Option<String>,
    pub deprecated: Option<DeprecationInfo>,
}

#[derive(Debug, Clone)]
pub struct ParamDef {
    pub name: ParamName,
    pub type_expr: TypeExpr,
    pub passing: ParamPassing,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamPassing {
    Value,
    Ref,
    RefMut,
    ImplTrait,
    BoxedDyn,
}

#[derive(Debug, Clone)]
pub enum ReturnDef {
    Void,
    Value(TypeExpr),
    Result { ok: TypeExpr, err: TypeExpr },
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub id: ClassId,
    pub constructors: Vec<ConstructorDef>,
    pub methods: Vec<MethodDef>,
    pub doc: Option<String>,
    pub deprecated: Option<DeprecationInfo>,
}

#[derive(Debug, Clone)]
pub struct ConstructorDef {
    pub name: Option<MethodId>,
    pub params: Vec<ParamDef>,
    pub doc: Option<String>,
    pub deprecated: Option<DeprecationInfo>,
}

#[derive(Debug, Clone)]
pub struct MethodDef {
    pub id: MethodId,
    pub receiver: Receiver,
    pub params: Vec<ParamDef>,
    pub returns: ReturnDef,
    pub is_async: bool,
    pub doc: Option<String>,
    pub deprecated: Option<DeprecationInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Receiver {
    Static,
    RefSelf,
    RefMutSelf,
    OwnedSelf,
}

#[derive(Debug, Clone)]
pub struct CallbackTraitDef {
    pub id: CallbackId,
    pub methods: Vec<CallbackMethodDef>,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CallbackMethodDef {
    pub id: MethodId,
    pub params: Vec<ParamDef>,
    pub returns: ReturnDef,
    pub is_async: bool,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CustomTypeDef {
    pub id: CustomTypeId,
    pub rust_type: QualifiedName,
    pub repr: TypeExpr,
    pub converters: ConverterPath,
    pub doc: Option<String>,
}
