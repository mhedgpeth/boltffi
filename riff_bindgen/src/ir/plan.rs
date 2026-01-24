use crate::ir::codec::CodecPlan;
use crate::ir::ids::{CallbackId, ClassId, ParamName};

#[derive(Debug, Clone)]
pub struct CallPlan {
    pub ffi_symbol: String,
    pub params: Vec<ParamPlan>,
    pub kind: CallPlanKind,
}

#[derive(Debug, Clone)]
pub enum CallPlanKind {
    Sync { returns: ReturnPlan },
    Async { async_plan: AsyncPlan },
}

#[derive(Debug, Clone)]
pub struct AsyncPlan {
    pub completion_callback: CompletionCallback,
    pub result: AsyncResult,
}

#[derive(Debug, Clone)]
pub enum AsyncResult {
    Void,
    Value {
        codec: CodecPlan,
    },
    Fallible {
        ok_codec: CodecPlan,
        err_codec: CodecPlan,
    },
}

#[derive(Debug, Clone)]
pub struct CompletionCallback {
    pub param_name: ParamName,
    pub ffi_type: AbiType,
}

#[derive(Debug, Clone)]
pub struct ParamPlan {
    pub name: ParamName,
    pub strategy: ParamStrategy,
}

#[derive(Debug, Clone)]
pub enum ParamStrategy {
    Direct(DirectPlan),
    Buffer {
        element_abi: AbiType,
        mutability: Mutability,
    },
    Encoded {
        codec: CodecPlan,
    },
    Handle {
        class_id: ClassId,
        nullable: bool,
    },
    Callback {
        callback_id: CallbackId,
        style: CallbackStyle,
    },
}

#[derive(Debug, Clone)]
pub struct DirectPlan {
    pub abi_type: AbiType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiType {
    Void,
    Bool,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
    Pointer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability {
    Shared,
    Mutable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackStyle {
    ImplTrait,
    BoxedDyn,
}

#[derive(Debug, Clone)]
pub enum ReturnPlan {
    Void,
    Direct(DirectPlan),
    Encoded { codec: CodecPlan },
    Handle { class_id: ClassId },
}
