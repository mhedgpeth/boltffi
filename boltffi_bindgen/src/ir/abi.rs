use boltffi_ffi_rules::naming::{
    CreateFn, GlobalSymbol, Name, RegisterFn, VtableField, VtableType,
};
use boltffi_ffi_rules::transport::{
    ErrorReturnStrategy, ReturnInvocationContext, ReturnPlatform, ScalarReturnStrategy,
    ValueReturnMethod, ValueReturnStrategy,
};

use crate::ir::contract::PackageInfo;
use crate::ir::definitions::StreamMode;
use crate::ir::ids::{
    CallbackId, ClassId, EnumId, FieldName, FunctionId, MethodId, ParamName, RecordId, StreamId,
    VariantName,
};
use crate::ir::ops::{ReadSeq, WriteSeq};
use crate::ir::plan::{AbiType, Mutability, ScalarOrigin, SpanContent, Transport};
use crate::ir::types::TypeExpr;

/// The resolved FFI boundary for the whole crate.
///
/// Each function and method is an [`AbiCall`] with a concrete parameter strategy
/// (wire-encoded buffer vs direct primitive), read/write op sequences for its
/// return type, and for async methods, the polling and completion setup. Backends
/// must read this and transform ops into syntax.
#[derive(Debug, Clone)]
pub struct AbiContract {
    pub package: PackageInfo,
    pub calls: Vec<AbiCall>,
    pub callbacks: Vec<AbiCallbackInvocation>,
    pub streams: Vec<AbiStream>,
    pub records: Vec<AbiRecord>,
    pub enums: Vec<AbiEnum>,
    pub free_buf: Name<GlobalSymbol>,
    pub atomic_cas: Name<GlobalSymbol>,
}

#[derive(Debug, Clone)]
pub struct AbiRecord {
    pub id: RecordId,
    pub decode_ops: ReadSeq,
    pub encode_ops: WriteSeq,
    pub is_blittable: bool,
    pub size: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct AbiEnum {
    pub id: EnumId,
    pub decode_ops: ReadSeq,
    pub encode_ops: WriteSeq,
    pub is_c_style: bool,
    pub variants: Vec<AbiEnumVariant>,
}

#[derive(Debug, Clone)]
pub struct AbiEnumVariant {
    pub name: VariantName,
    pub discriminant: i128,
    pub payload: AbiEnumPayload,
}

#[derive(Debug, Clone)]
pub enum AbiEnumPayload {
    Unit,
    Tuple(Vec<AbiEnumField>),
    Struct(Vec<AbiEnumField>),
}

#[derive(Debug, Clone)]
pub struct AbiEnumField {
    pub name: FieldName,
    pub type_expr: TypeExpr,
    pub decode: ReadSeq,
    pub encode: WriteSeq,
}

#[derive(Debug, Clone)]
pub enum StreamItemTransport {
    WireEncoded { decode_ops: ReadSeq },
}

#[derive(Debug, Clone)]
pub struct AbiStream {
    pub class_id: ClassId,
    pub stream_id: StreamId,
    pub mode: StreamMode,
    pub item: StreamItemTransport,
    pub item_transport: Transport,
    pub item_size: Option<usize>,
    pub subscribe: Name<GlobalSymbol>,
    pub poll: Name<GlobalSymbol>,
    pub pop_batch: Name<GlobalSymbol>,
    pub wait: Name<GlobalSymbol>,
    pub unsubscribe: Name<GlobalSymbol>,
    pub free: Name<GlobalSymbol>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CallId {
    Function(FunctionId),
    Method {
        class_id: ClassId,
        method_id: MethodId,
    },
    Constructor {
        class_id: ClassId,
        index: usize,
    },
    RecordMethod {
        record_id: RecordId,
        method_id: MethodId,
    },
    RecordConstructor {
        record_id: RecordId,
        index: usize,
    },
    EnumMethod {
        enum_id: EnumId,
        method_id: MethodId,
    },
    EnumConstructor {
        enum_id: EnumId,
        index: usize,
    },
}

#[derive(Debug, Clone)]
pub struct AbiCall {
    pub id: CallId,
    pub symbol: Name<GlobalSymbol>,
    pub mode: CallMode,
    pub params: Vec<AbiParam>,
    pub returns: ReturnShape,
    pub error: ErrorTransport,
}

impl AbiCall {
    pub fn is_value_type_call(&self) -> bool {
        matches!(
            self.id,
            CallId::RecordMethod { .. }
                | CallId::RecordConstructor { .. }
                | CallId::EnumMethod { .. }
                | CallId::EnumConstructor { .. }
        )
    }
}

#[derive(Debug, Clone)]
pub enum CallMode {
    Sync,
    Async(Box<AsyncCall>),
}

#[derive(Debug, Clone)]
pub struct AsyncCall {
    pub poll: Name<GlobalSymbol>,
    pub complete: Name<GlobalSymbol>,
    pub cancel: Name<GlobalSymbol>,
    pub free: Name<GlobalSymbol>,
    pub result: ReturnShape,
    pub error: ErrorTransport,
}

#[derive(Debug, Clone)]
pub struct AbiParam {
    pub name: ParamName,
    pub abi_type: AbiType,
    pub role: ParamRole,
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ParamRole {
    Input {
        transport: Transport,
        mutability: Mutability,
        len_param: Option<ParamName>,
        decode_ops: Option<ReadSeq>,
        encode_ops: Option<WriteSeq>,
    },
    SyntheticLen {
        for_param: ParamName,
    },
    CallbackContext {
        for_param: ParamName,
    },
    OutLen {
        for_param: ParamName,
    },
    OutDirect,
    StatusOut,
}

#[derive(Debug, Clone)]
pub struct ReturnShape {
    pub transport: Option<Transport>,
    pub decode_ops: Option<ReadSeq>,
    pub encode_ops: Option<WriteSeq>,
}

impl ReturnShape {
    pub fn void() -> Self {
        Self {
            transport: None,
            decode_ops: None,
            encode_ops: None,
        }
    }

    /// Classifies the returned value into the shared return vocabulary.
    ///
    /// This answers what kind of value comes back across the boundary:
    /// nothing, a scalar, a fixed composite value, a direct element buffer,
    /// an encoded buffer, an object handle, or a callback handle.
    ///
    /// It does not answer where the value is delivered in the ABI surface.
    /// That part belongs to [`Self::value_return_method`].
    ///
    /// # Examples
    ///
    /// - a primitive scalar return becomes [`ValueReturnStrategy::Scalar`]
    /// - a `repr(C)` record by value becomes [`ValueReturnStrategy::CompositeValue`]
    /// - a direct `Vec<u32>` return becomes [`ValueReturnStrategy::DirectBuffer`]
    /// - a wire-encoded enum return becomes [`ValueReturnStrategy::EncodedBuffer`]
    pub fn value_return_strategy(&self) -> ValueReturnStrategy {
        match &self.transport {
            None => ValueReturnStrategy::Void,
            Some(Transport::Scalar(ScalarOrigin::Primitive(_))) => {
                ValueReturnStrategy::Scalar(ScalarReturnStrategy::PrimitiveValue)
            }
            Some(Transport::Scalar(ScalarOrigin::CStyleEnum { .. })) => {
                ValueReturnStrategy::Scalar(ScalarReturnStrategy::CStyleEnumTag)
            }
            Some(Transport::Composite(_)) => ValueReturnStrategy::CompositeValue,
            Some(Transport::Span(SpanContent::Scalar(_)))
            | Some(Transport::Span(SpanContent::Composite(_))) => {
                ValueReturnStrategy::DirectBuffer
            }
            Some(Transport::Span(_)) => ValueReturnStrategy::EncodedBuffer,
            Some(Transport::Handle { .. }) => ValueReturnStrategy::ObjectHandle,
            Some(Transport::Callback { .. }) => {
                ValueReturnStrategy::CallbackHandle
            }
        }
    }

    /// Decides how the already-classified value is delivered to the caller.
    ///
    /// This is about the ABI method, not about the value kind itself.
    /// For example, both a direct element buffer and an encoded buffer are
    /// still buffer-shaped returns, but encoded errors may force them to be
    /// written through out pointer and length outputs instead of coming back
    /// in the native return slot.
    ///
    /// # Examples
    ///
    /// - `u32` stays [`ValueReturnMethod::DirectReturn`]
    /// - `Point` by value stays [`ValueReturnMethod::DirectReturn`]
    /// - an encoded buffer with encoded errors becomes
    ///   [`ValueReturnMethod::WriteToOutBufferParts`]
    pub fn value_return_method(
        &self,
        error: &ErrorTransport,
        context: ReturnInvocationContext,
        platform: ReturnPlatform,
    ) -> ValueReturnMethod {
        self.value_return_strategy()
            .return_method(error.return_strategy(), context, platform)
    }
}

#[derive(Debug, Clone)]
pub enum ErrorTransport {
    None,
    StatusCode,
    Encoded {
        decode_ops: ReadSeq,
        encode_ops: Option<WriteSeq>,
    },
}

impl ErrorTransport {
    /// Projects the bindgen error transport into the shared error return
    /// vocabulary.
    ///
    /// This keeps backends from inventing their own meaning for whether a call
    /// has no distinct failure path, reports failure through a status code, or
    /// returns an encoded error payload.
    pub fn return_strategy(&self) -> ErrorReturnStrategy {
        match self {
            Self::None => ErrorReturnStrategy::None,
            Self::StatusCode => ErrorReturnStrategy::StatusCode,
            Self::Encoded { .. } => ErrorReturnStrategy::Encoded,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AbiCallbackInvocation {
    pub callback_id: CallbackId,
    pub vtable_type: Name<VtableType>,
    pub register_fn: Name<RegisterFn>,
    pub create_fn: Name<CreateFn>,
    pub methods: Vec<AbiCallbackMethod>,
}

#[derive(Debug, Clone)]
pub struct AbiCallbackMethod {
    pub id: MethodId,
    pub vtable_field: Name<VtableField>,
    pub is_async: bool,
    pub params: Vec<AbiParam>,
    pub returns: ReturnShape,
    pub error: ErrorTransport,
}

impl AbiParam {
    pub fn is_hidden(&self) -> bool {
        matches!(
            self.role,
            ParamRole::SyntheticLen { .. }
                | ParamRole::CallbackContext { .. }
                | ParamRole::OutLen { .. }
                | ParamRole::OutDirect
                | ParamRole::StatusOut
        )
    }

    pub fn transport(&self) -> Option<&Transport> {
        match &self.role {
            ParamRole::Input { transport, .. } => Some(transport),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::ids::FunctionId;
    use crate::ir::plan::ScalarOrigin;
    use crate::ir::types::PrimitiveType;
    use boltffi_ffi_rules::naming;

    fn scalar_param(name: &str, abi: AbiType) -> AbiParam {
        AbiParam {
            name: ParamName::new(name),
            abi_type: abi,
            role: ParamRole::Input {
                transport: Transport::Scalar(ScalarOrigin::Primitive(PrimitiveType::I32)),
                mutability: Mutability::Shared,
                len_param: None,
                decode_ops: None,
                encode_ops: None,
            },
        }
    }

    #[test]
    fn scalar_param_exposes_transport() {
        let param = scalar_param("v", AbiType::I32);
        assert!(matches!(
            param.transport(),
            Some(Transport::Scalar(ScalarOrigin::Primitive(
                PrimitiveType::I32
            )))
        ));
    }

    #[test]
    fn hidden_param_has_no_transport() {
        let param = AbiParam {
            name: ParamName::new("len"),
            abi_type: AbiType::U64,
            role: ParamRole::SyntheticLen {
                for_param: ParamName::new("buf"),
            },
        };
        assert!(param.is_hidden());
        assert!(param.transport().is_none());
    }

    #[test]
    fn return_shape_void() {
        let ret = ReturnShape::void();
        assert!(ret.transport.is_none());
        assert!(ret.decode_ops.is_none());
        assert!(ret.encode_ops.is_none());
    }

    #[test]
    fn abi_call_uses_return_shape() {
        let call = AbiCall {
            id: CallId::Function(FunctionId::new("f")),
            symbol: naming::function_ffi_name("f"),
            mode: CallMode::Sync,
            params: vec![scalar_param("x", AbiType::I32)],
            returns: ReturnShape {
                transport: Some(Transport::Scalar(ScalarOrigin::Primitive(
                    PrimitiveType::I32,
                ))),
                decode_ops: None,
                encode_ops: None,
            },
            error: ErrorTransport::None,
        };
        assert!(matches!(
            call.returns.transport,
            Some(Transport::Scalar(ScalarOrigin::Primitive(
                PrimitiveType::I32
            )))
        ));
    }
}
