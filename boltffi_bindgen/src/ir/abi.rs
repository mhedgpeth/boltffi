use boltffi_ffi_rules::naming::{
    CreateFn, GlobalSymbol, Name, RegisterFn, VtableField, VtableType,
};

use crate::ir::contract::PackageInfo;
use crate::ir::definitions::StreamMode;
use crate::ir::ids::{
    CallbackId, ClassId, EnumId, FieldName, FunctionId, MethodId, ParamName, RecordId, StreamId,
    VariantName,
};
use crate::ir::ops::{ReadSeq, WriteSeq};
use crate::ir::plan::{AbiType, CallbackStyle, Mutability};
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
    pub discriminant: i64,
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
}

#[derive(Debug, Clone)]
pub struct AbiCall {
    pub id: CallId,
    pub symbol: Name<GlobalSymbol>,
    pub mode: CallMode,
    pub params: Vec<AbiParam>,
    pub output_shape: OutputShape,
    pub error: ErrorTransport,
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
    pub result_shape: OutputShape,
    pub error: ErrorTransport,
}

#[derive(Debug, Clone)]
pub enum ValueShape {
    Scalar(AbiType),
    OptionScalar {
        abi: AbiType,
        read: ReadSeq,
        write: WriteSeq,
    },
    ResultScalar {
        ok: AbiType,
        err: AbiType,
        read: ReadSeq,
        write: WriteSeq,
    },
    PrimitiveVec {
        element_abi: AbiType,
        read: ReadSeq,
        write: WriteSeq,
    },
    BlittableRecord {
        id: RecordId,
        size: u32,
        read: ReadSeq,
        write: WriteSeq,
    },
    WireEncoded {
        read: ReadSeq,
        write: WriteSeq,
    },
}

#[derive(Debug, Clone)]
pub enum InputShape {
    Value(ValueShape),
    Utf8Slice {
        len_param: ParamName,
    },
    PrimitiveSlice {
        len_param: ParamName,
        mutability: Mutability,
        element_abi: AbiType,
    },
    WirePacket {
        len_param: ParamName,
        value: ValueShape,
    },
    OutputBuffer {
        len_param: ParamName,
        value: ValueShape,
    },
    Handle {
        class_id: ClassId,
        nullable: bool,
    },
    Callback {
        callback_id: CallbackId,
        nullable: bool,
        style: CallbackStyle,
    },
    HiddenSyntheticLen {
        for_param: ParamName,
    },
    HiddenOutLen {
        for_param: ParamName,
    },
    HiddenOutDirect,
    HiddenStatusOut,
}

#[derive(Debug, Clone)]
pub enum OutputShape {
    Unit,
    Value(ValueShape),
    Handle {
        class_id: ClassId,
        nullable: bool,
    },
    Callback {
        callback_id: CallbackId,
        nullable: bool,
    },
}

#[derive(Debug, Clone)]
pub struct AbiParam {
    pub name: ParamName,
    pub ffi_type: AbiType,
    pub input_shape: InputShape,
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
    pub output_shape: OutputShape,
    pub error: ErrorTransport,
}

impl AbiContract {
    pub fn assert_shape_consistency(&self) {
        self.calls
            .iter()
            .for_each(AbiCall::assert_shape_consistency);
        self.callbacks.iter().for_each(|callback| {
            callback
                .methods
                .iter()
                .for_each(AbiCallbackMethod::assert_shape_consistency)
        });
    }
}

impl AbiCall {
    fn assert_shape_consistency(&self) {
        self.params
            .iter()
            .for_each(AbiParam::assert_shape_consistency);
        if let OutputShape::Value(value_shape) = &self.output_shape {
            assert!(
                value_shape.has_consistent_wire_metadata(),
                "output_shape has inconsistent wire metadata for call {}",
                self.symbol.as_str()
            );
        }
        if let CallMode::Async(async_call) = &self.mode {
            async_call.assert_shape_consistency();
        }
    }
}

impl AsyncCall {
    fn assert_shape_consistency(&self) {
        if let OutputShape::Value(value_shape) = &self.result_shape {
            assert!(
                value_shape.has_consistent_wire_metadata(),
                "result_shape has inconsistent wire metadata for {}",
                self.complete.as_str()
            );
        }
    }
}

impl AbiCallbackMethod {
    fn assert_shape_consistency(&self) {
        self.params
            .iter()
            .for_each(AbiParam::assert_shape_consistency);
        if let OutputShape::Value(value_shape) = &self.output_shape {
            assert!(
                value_shape.has_consistent_wire_metadata(),
                "output_shape has inconsistent wire metadata for callback method {}",
                self.id.as_str()
            );
        }
    }
}

impl AbiParam {
    fn assert_shape_consistency(&self) {
        if let InputShape::Value(ValueShape::Scalar(abi_type)) = &self.input_shape {
            assert!(
                *abi_type == self.ffi_type,
                "scalar input shape ABI and ffi_type mismatch for param {}",
                self.name.as_str()
            );
        }
        if matches!(
            self.input_shape,
            InputShape::Utf8Slice { .. }
                | InputShape::PrimitiveSlice { .. }
                | InputShape::WirePacket { .. }
                | InputShape::OutputBuffer { .. }
                | InputShape::Handle { .. }
                | InputShape::Callback { .. }
        ) {
            assert!(
                self.ffi_type == AbiType::Pointer,
                "non-scalar input shape must use pointer ffi_type for param {}",
                self.name.as_str()
            );
        }
        if let InputShape::Value(value_shape) = &self.input_shape {
            assert!(
                value_shape.has_consistent_wire_metadata(),
                "input_shape has inconsistent wire metadata for param {}",
                self.name.as_str()
            );
        }
    }
}

impl ValueShape {
    pub fn read_ops(&self) -> Option<&ReadSeq> {
        match self {
            Self::Scalar(_) => None,
            Self::OptionScalar { read, .. }
            | Self::ResultScalar { read, .. }
            | Self::PrimitiveVec { read, .. }
            | Self::BlittableRecord { read, .. }
            | Self::WireEncoded { read, .. } => Some(read),
        }
    }

    pub fn write_ops(&self) -> Option<&WriteSeq> {
        match self {
            Self::Scalar(_) => None,
            Self::OptionScalar { write, .. }
            | Self::ResultScalar { write, .. }
            | Self::PrimitiveVec { write, .. }
            | Self::BlittableRecord { write, .. }
            | Self::WireEncoded { write, .. } => Some(write),
        }
    }

    fn has_consistent_wire_metadata(&self) -> bool {
        match self {
            Self::Scalar(_) => true,
            Self::OptionScalar { .. }
            | Self::ResultScalar { .. }
            | Self::PrimitiveVec { .. }
            | Self::BlittableRecord { .. }
            | Self::WireEncoded { .. } => self.read_ops().is_some() && self.write_ops().is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::ops::{SizeExpr, WireShape};
    use boltffi_ffi_rules::naming;

    fn minimal_contract(call: AbiCall) -> AbiContract {
        AbiContract {
            package: PackageInfo {
                name: "test".to_string(),
                version: None,
            },
            calls: vec![call],
            callbacks: Vec::new(),
            streams: Vec::new(),
            records: Vec::new(),
            enums: Vec::new(),
            free_buf: naming::free_buf_u8(),
            atomic_cas: naming::atomic_u8_cas(),
        }
    }

    fn minimal_call(param: AbiParam, output_shape: OutputShape) -> AbiCall {
        AbiCall {
            id: CallId::Function(FunctionId::new("f")),
            symbol: naming::function_ffi_name("f"),
            mode: CallMode::Sync,
            params: vec![param],
            output_shape,
            error: ErrorTransport::None,
        }
    }

    #[test]
    fn shape_consistency_accepts_matching_contract() {
        let param = AbiParam {
            name: ParamName::new("v"),
            ffi_type: AbiType::I32,
            input_shape: InputShape::Value(ValueShape::Scalar(AbiType::I32)),
        };
        let call = minimal_call(param, OutputShape::Value(ValueShape::Scalar(AbiType::I32)));
        let contract = minimal_contract(call);
        contract.assert_shape_consistency();
    }

    #[test]
    #[should_panic]
    fn shape_consistency_rejects_scalar_input_shape_type_mismatch() {
        let param = AbiParam {
            name: ParamName::new("v"),
            ffi_type: AbiType::I64,
            input_shape: InputShape::Value(ValueShape::Scalar(AbiType::I32)),
        };
        let call = minimal_call(param, OutputShape::Unit);
        minimal_contract(call).assert_shape_consistency();
    }

    #[test]
    fn encoded_shapes_keep_wire_metadata() {
        let read = ReadSeq {
            size: SizeExpr::Fixed(0),
            ops: Vec::new(),
            shape: WireShape::Value,
        };
        let write = WriteSeq {
            size: SizeExpr::Fixed(0),
            ops: Vec::new(),
            shape: WireShape::Value,
        };
        let encoded = ValueShape::WireEncoded {
            read: read.clone(),
            write: write.clone(),
        };
        assert!(encoded.read_ops().is_some());
        assert!(encoded.write_ops().is_some());
    }
}
