use crate::ir::codec::CodecPlan;
use crate::ir::ids::{CallbackId, MethodId, ParamName, QualifiedName};
use crate::ir::ops::{ReadOp, WriteOp};
use crate::ir::plan::DirectPlan;

#[derive(Debug, Clone)]
pub struct CallbackInvocationPlan {
    pub callback_id: CallbackId,
    pub foreign_type_name: QualifiedName,
    pub methods: Vec<CallbackMethodPlan>,
}

#[derive(Debug, Clone)]
pub struct CallbackMethodPlan {
    pub id: MethodId,
    pub ffi_symbol: String,
    pub params: Vec<CallbackParamPlan>,
    pub returns: CallbackReturnPlan,
    pub is_async: bool,
}

#[derive(Debug, Clone)]
pub struct CallbackParamPlan {
    pub name: ParamName,
    pub strategy: CallbackParamStrategy,
}

#[derive(Debug, Clone)]
pub enum CallbackParamStrategy {
    Direct(DirectPlan),
    Encoded { write_ops: Vec<WriteOp> },
}

#[derive(Debug, Clone)]
pub enum CallbackReturnPlan {
    Void,
    Direct(DirectPlan),
    Decoded { read_ops: Vec<ReadOp> },
    Async { completion_codec: CodecPlan },
}
