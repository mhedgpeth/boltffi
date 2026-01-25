use riff_ffi_rules::naming::{self, snake_to_camel as camel_case, to_upper_camel_case as pascal_case};

use crate::ir::LoweredContract;
use crate::ir::codec::{CodecPlan, EnumLayout, RecordLayout, VariantPayloadLayout};
use crate::ir::contract::FfiContract;
use crate::ir::definitions::{EnumRepr, Receiver};
use crate::ir::ids::{ClassId, EnumId, RecordId};
use crate::ir::plan::{
    AbiType, AsyncResult, CallPlanKind, CallTarget, ParamPlan, ParamStrategy, ReturnPlan,
    ReturnValuePlan,
};
use crate::ir::types::TypeExpr;

use super::codec;
use super::plan::{
    SwiftCallback, SwiftCallbackMethod, SwiftCallbackParam, SwiftClass, SwiftConstructor,
    SwiftConversion, SwiftEnum, SwiftField, SwiftFunction, SwiftMethod, SwiftModule, SwiftParam,
    SwiftRecord, SwiftReturn, SwiftVariant, SwiftVariantPayload,
};

pub struct SwiftLowerer<'a> {
    contract: &'a FfiContract,
    lowered: &'a LoweredContract,
    prefix: String,
}

impl<'a> SwiftLowerer<'a> {
    pub fn new(contract: &'a FfiContract, lowered: &'a LoweredContract) -> Self {
        let prefix = contract.package.name.replace('-', "_");
        Self {
            contract,
            lowered,
            prefix,
        }
    }

    pub fn with_prefix(
        contract: &'a FfiContract,
        lowered: &'a LoweredContract,
        prefix: impl Into<String>,
    ) -> Self {
        Self {
            contract,
            lowered,
            prefix: prefix.into(),
        }
    }

    pub fn lower(self) -> SwiftModule {
        let records = self.lower_records();
        let enums = self.lower_enums();
        let classes = self.lower_classes();
        let callbacks = self.lower_callbacks();
        let functions = self.lower_functions();

        SwiftModule {
            imports: vec!["Foundation".to_string()],
            records,
            enums,
            classes,
            callbacks,
            functions,
        }
    }

    fn lower_records(&self) -> Vec<SwiftRecord> {
        self.contract
            .catalog
            .all_records()
            .map(|def| {
                let codec = self
                    .lowered
                    .record_codecs
                    .get(&def.id)
                    .expect("record codec should exist");

                let layout = match codec {
                    CodecPlan::Record { layout, .. } => layout,
                    _ => panic!("expected CodecPlan::Record"),
                };

                let fields = match layout {
                    RecordLayout::Encoded { fields } => fields
                        .iter()
                        .map(|f| SwiftField {
                            swift_name: camel_case(f.name.as_str()),
                            swift_type: codec::swift_type(&f.codec),
                            default_expr: None,
                            codec: f.codec.clone(),
                        })
                        .collect(),
                    RecordLayout::Blittable { fields, .. } => fields
                        .iter()
                        .map(|f| SwiftField {
                            swift_name: camel_case(f.name.as_str()),
                            swift_type: codec::swift_primitive(f.primitive),
                            default_expr: None,
                            codec: CodecPlan::Primitive(f.primitive),
                        })
                        .collect(),
                    RecordLayout::Recursive => vec![],
                };

                SwiftRecord {
                    class_name: self.swift_name_for_record(&def.id),
                    fields,
                    is_blittable: layout.is_blittable(),
                }
            })
            .collect()
    }

    fn lower_enums(&self) -> Vec<SwiftEnum> {
        self.contract
            .catalog
            .all_enums()
            .map(|def| {
                let codec = self
                    .lowered
                    .enum_codecs
                    .get(&def.id)
                    .expect("enum codec should exist");

                let layout = match codec {
                    CodecPlan::Enum { layout, .. } => layout,
                    _ => panic!("expected CodecPlan::Enum"),
                };

                let (is_c_style, variants) = match layout {
                    EnumLayout::CStyle { .. } => (
                        true,
                        match &def.repr {
                            EnumRepr::CStyle { variants, .. } => variants
                                .iter()
                                .map(|v| SwiftVariant {
                                    swift_name: camel_case(v.name.as_str()),
                                    discriminant: v.discriminant,
                                    payload: SwiftVariantPayload::Unit,
                                })
                                .collect(),
                            _ => vec![],
                        },
                    ),
                    EnumLayout::Data { variants, .. } => (
                        false,
                        variants
                            .iter()
                            .map(|v| SwiftVariant {
                                swift_name: camel_case(v.name.as_str()),
                                discriminant: v.discriminant,
                                payload: self.lower_variant_payload_layout(&v.payload),
                            })
                            .collect(),
                    ),
                    EnumLayout::Recursive => (false, vec![]),
                };

                SwiftEnum {
                    name: self.swift_name_for_enum(&def.id),
                    variants,
                    is_c_style,
                    is_error: def.is_error,
                    doc: def.doc.clone(),
                }
            })
            .collect()
    }

    fn lower_variant_payload_layout(&self, payload: &VariantPayloadLayout) -> SwiftVariantPayload {
        match payload {
            VariantPayloadLayout::Unit => SwiftVariantPayload::Unit,
            VariantPayloadLayout::Fields(fields) => SwiftVariantPayload::Struct(
                fields
                    .iter()
                    .map(|f| SwiftField {
                        swift_name: camel_case(f.name.as_str()),
                        swift_type: codec::swift_type(&f.codec),
                        default_expr: None,
                        codec: f.codec.clone(),
                    })
                    .collect(),
            ),
        }
    }

    fn lower_classes(&self) -> Vec<SwiftClass> {
        self.contract
            .catalog
            .all_classes()
            .map(|def| {
                let class_name = self.swift_name_for_class(&def.id);

                let ffi_free = format!(
                    "{}_free",
                    def.id.as_str().to_lowercase().replace("::", "_")
                );

                let constructors = def
                    .constructors
                    .iter()
                    .enumerate()
                    .map(|(idx, ctor)| {
                        let plan = self
                            .lowered
                            .constructors
                            .get(&(def.id.clone(), idx))
                            .expect("constructor plan should exist");

                        let ffi_symbol = match &plan.target {
                            CallTarget::GlobalSymbol(s) => s.clone().into_string(),
                            CallTarget::VtableField(_) => {
                                panic!("constructor should have global symbol")
                            }
                        };

                        SwiftConstructor {
                            name: ctor.name.as_ref().map(|n| camel_case(n.as_str())),
                            ffi_symbol,
                            params: plan
                                .params
                                .iter()
                                .map(|p| self.lower_param_plan(p))
                                .collect(),
                            is_fallible: ctor.is_fallible,
                            doc: ctor.doc.clone(),
                        }
                    })
                    .collect();

                let methods = def
                    .methods
                    .iter()
                    .map(|method| {
                        let plan = self
                            .lowered
                            .methods
                            .get(&(def.id.clone(), method.id.clone()))
                            .expect("method plan should exist");

                        let ffi_symbol = match &plan.target {
                            CallTarget::GlobalSymbol(s) => s.clone().into_string(),
                            CallTarget::VtableField(_) => {
                                panic!("method should have global symbol")
                            }
                        };

                        let (is_async, returns) = match &plan.kind {
                            CallPlanKind::Sync { returns } => {
                                (false, self.lower_return_plan(returns))
                            }
                            CallPlanKind::Async { async_plan } => {
                                (true, self.lower_async_result(&async_plan.result))
                            }
                        };

                        SwiftMethod {
                            name: camel_case(method.id.as_str()),
                            ffi_symbol,
                            params: plan
                                .params
                                .iter()
                                .skip(if method.receiver == Receiver::Static {
                                    0
                                } else {
                                    1
                                })
                                .map(|p| self.lower_param_plan(p))
                                .collect(),
                            returns,
                            is_static: method.receiver == Receiver::Static,
                            is_async,
                            doc: method.doc.clone(),
                        }
                    })
                    .collect();

                SwiftClass {
                    name: class_name,
                    ffi_free,
                    constructors,
                    methods,
                    doc: def.doc.clone(),
                }
            })
            .collect()
    }

    fn lower_callbacks(&self) -> Vec<SwiftCallback> {
        self.contract
            .catalog
            .all_callbacks()
            .map(|def| {
                let protocol_name = pascal_case(def.id.as_str());
                let vtable_var = format!("{}VTableInstance", lower_first_char(&protocol_name));
                let methods = def
                    .methods
                    .iter()
                    .map(|method| {
                        let plans = self
                            .lowered
                            .callbacks
                            .get(&def.id)
                            .expect("callback plans should exist");
                        let expected_field = naming::vtable_field_name(method.id.as_str());
                        let plan = plans
                            .iter()
                            .find(|p| {
                                matches!(&p.target, CallTarget::VtableField(id) if id.as_str() == expected_field.as_str())
                            })
                            .expect("callback method plan should exist");

                        let (is_async, returns) = match &plan.kind {
                            CallPlanKind::Sync { returns } => {
                                (false, self.lower_return_plan(returns))
                            }
                            CallPlanKind::Async { async_plan } => {
                                (true, self.lower_async_result(&async_plan.result))
                            }
                        };

                        let has_out_param = !is_async && !returns.is_void();
                        let wire_encoded_return = returns.is_wire_encoded();

                        SwiftCallbackMethod {
                            name: camel_case(method.id.as_str()),
                            ffi_name: naming::vtable_field_name(method.id.as_str()).into_string(),
                            params: plan
                                .params
                                .iter()
                                .skip(1)
                                .map(|p| self.lower_callback_param_plan(p))
                                .collect(),
                            returns,
                            is_async,
                            has_out_param,
                            wire_encoded_return,
                        }
                    })
                    .collect();

                SwiftCallback {
                    protocol_name: protocol_name.clone(),
                    wrapper_class: format!("{}Wrapper", protocol_name),
                    vtable_var,
                    vtable_type: naming::callback_vtable_name(def.id.as_str()).into_string(),
                    bridge_name: format!("{}Bridge", protocol_name),
                    register_fn: naming::callback_register_fn(def.id.as_str()).into_string(),
                    create_fn: naming::callback_create_fn(def.id.as_str()).into_string(),
                    methods,
                    doc: def.doc.clone(),
                }
            })
            .collect()
    }

    fn lower_functions(&self) -> Vec<SwiftFunction> {
        self.contract
            .functions
            .iter()
            .map(|def| {
                let plan = self
                    .lowered
                    .functions
                    .get(&def.id)
                    .expect("function plan should exist");

                let ffi_symbol = match &plan.target {
                    CallTarget::GlobalSymbol(s) => s.clone().into_string(),
                    CallTarget::VtableField(_) => panic!("function should have global symbol"),
                };

                let (is_async, returns) = match &plan.kind {
                    CallPlanKind::Sync { returns } => (false, self.lower_return_plan(returns)),
                    CallPlanKind::Async { async_plan } => {
                        (true, self.lower_async_result(&async_plan.result))
                    }
                };

                SwiftFunction {
                    name: camel_case(def.id.as_str()),
                    ffi_symbol,
                    params: plan
                        .params
                        .iter()
                        .map(|p| self.lower_param_plan(p))
                        .collect(),
                    returns,
                    is_async,
                    doc: def.doc.clone(),
                }
            })
            .collect()
    }

    fn lower_param_plan(&self, param: &ParamPlan) -> SwiftParam {
        let (swift_type, conversion) = match &param.strategy {
            ParamStrategy::Direct(d) => (self.abi_to_swift(d.abi_type), SwiftConversion::Direct),

            ParamStrategy::Buffer { element_abi, .. } => {
                if *element_abi == AbiType::U8 {
                    ("Data".to_string(), SwiftConversion::ToData)
                } else {
                    (
                        format!("[{}]", self.abi_to_swift(*element_abi)),
                        SwiftConversion::Direct,
                    )
                }
            }

            ParamStrategy::Encoded { codec } => {
                let swift_type = codec::swift_type(codec);
                (
                    swift_type,
                    SwiftConversion::ToWireBuffer {
                        codec: codec.clone(),
                    },
                )
            }

            ParamStrategy::Handle { class_id, nullable } => {
                let class_name = self.swift_name_for_class(class_id);
                let swift_type = if *nullable {
                    format!("{}?", class_name)
                } else {
                    class_name.clone()
                };
                (
                    swift_type,
                    SwiftConversion::PassHandle {
                        class_name,
                        nullable: *nullable,
                    },
                )
            }

            ParamStrategy::Callback {
                callback_id,
                nullable,
                ..
            } => {
                let protocol = pascal_case(callback_id.as_str());
                let swift_type = if *nullable {
                    format!("(any {})?", protocol)
                } else {
                    format!("any {}", protocol)
                };
                (
                    swift_type,
                    SwiftConversion::WrapCallback {
                        protocol: protocol.clone(),
                    },
                )
            }
        };

        SwiftParam {
            label: None,
            name: camel_case(param.name.as_str()),
            swift_type,
            conversion,
        }
    }

    fn lower_callback_param_plan(&self, param: &ParamPlan) -> SwiftCallbackParam {
        let label = camel_case(param.name.as_str());
        let (swift_type, ffi_args, decode_prelude) = match &param.strategy {
            ParamStrategy::Direct(d) => (
                self.abi_to_swift(d.abi_type),
                vec![label.clone()],
                None,
            ),
            ParamStrategy::Buffer { element_abi, .. } => {
                let ptr_name = format!("{}Ptr", label);
                let len_name = format!("{}Len", label);
                if *element_abi == AbiType::U8 {
                    (
                        "Data".to_string(),
                        vec![ptr_name.clone(), len_name.clone()],
                        Some(format!(
                            "let {} = Data(bytes: {}!, count: Int({}))",
                            label, ptr_name, len_name
                        )),
                    )
                } else {
                    let element_type = self.abi_to_swift(*element_abi);
                    (
                        format!("[{}]", element_type),
                        vec![ptr_name.clone(), len_name.clone()],
                        Some(format!(
                            "let {} = Array(UnsafeBufferPointer(start: {}!.assumingMemoryBound(to: {}.self), count: Int({})))",
                            label, ptr_name, element_type, len_name
                        )),
                    )
                }
            }
            ParamStrategy::Encoded { codec } => {
                let ptr_name = format!("{}Ptr", label);
                let len_name = format!("{}Len", label);
                (
                    codec::swift_type(codec),
                    vec![ptr_name.clone(), len_name.clone()],
                    Some(format!(
                        "let {} = {{ let wire = WireBuffer(ptr: {}!, len: Int({})); var pos = 0; return {} }}()",
                        label,
                        ptr_name,
                        len_name,
                        codec::decode_inline(codec)
                    )),
                )
            }
            ParamStrategy::Handle { nullable, .. } => {
                let swift_type = if *nullable {
                    "OpaquePointer?".to_string()
                } else {
                    "OpaquePointer".to_string()
                };
                (swift_type, vec![label.clone()], None)
            }
            ParamStrategy::Callback { nullable, .. } => {
                let swift_type = if *nullable {
                    "RiffCallbackHandle?".to_string()
                } else {
                    "RiffCallbackHandle".to_string()
                };
                (swift_type, vec![label.clone()], None)
            }
        };

        SwiftCallbackParam {
            label: label.clone(),
            swift_type,
            call_arg: label,
            ffi_args,
            decode_prelude,
        }
    }

    fn lower_return_plan(&self, plan: &ReturnPlan) -> SwiftReturn {
        match plan {
            ReturnPlan::Value(v) => self.lower_return_value(v),
            ReturnPlan::Fallible { ok, err_codec } => SwiftReturn::Throws {
                ok: Box::new(self.lower_return_value(ok)),
                err_type: codec::swift_type(err_codec),
            },
        }
    }

    fn lower_return_value(&self, plan: &ReturnValuePlan) -> SwiftReturn {
        match plan {
            ReturnValuePlan::Void => SwiftReturn::Void,

            ReturnValuePlan::Direct(d) => SwiftReturn::Direct {
                swift_type: self.abi_to_swift(d.abi_type),
            },

            ReturnValuePlan::Encoded { codec } => SwiftReturn::FromWireBuffer {
                swift_type: codec::swift_type(codec),
                codec: codec.clone(),
            },

            ReturnValuePlan::Handle { class_id, nullable } => {
                let class_name = self.swift_name_for_class(class_id);
                SwiftReturn::Handle {
                    class_name,
                    nullable: *nullable,
                }
            }

            ReturnValuePlan::Callback {
                callback_id,
                nullable,
            } => {
                let protocol = pascal_case(callback_id.as_str());
                let swift_type = if *nullable {
                    format!("(any {})?", protocol)
                } else {
                    format!("any {}", protocol)
                };
                SwiftReturn::Direct { swift_type }
            }
        }
    }

    fn lower_async_result(&self, result: &AsyncResult) -> SwiftReturn {
        match result {
            AsyncResult::Void => SwiftReturn::Void,
            AsyncResult::Value(v) => self.lower_return_value(v),
            AsyncResult::Fallible { ok, err_codec } => SwiftReturn::Throws {
                ok: Box::new(self.lower_return_value(ok)),
                err_type: codec::swift_type(err_codec),
            },
        }
    }

    fn swift_type(&self, ty: &TypeExpr) -> String {
        match ty {
            TypeExpr::Void => "Void".to_string(),
            TypeExpr::Primitive(p) => codec::swift_primitive(*p),
            TypeExpr::String => "String".to_string(),
            TypeExpr::Bytes => "Data".to_string(),
            TypeExpr::Builtin(id) => codec::swift_builtin(id.as_str()),
            TypeExpr::Vec(inner) => format!("[{}]", self.swift_type(inner)),
            TypeExpr::Option(inner) => format!("{}?", self.swift_type(inner)),
            TypeExpr::Result { ok, err } => {
                format!("Result<{}, {}>", self.swift_type(ok), self.swift_type(err))
            }
            TypeExpr::Record(id) => self.swift_name_for_record(id),
            TypeExpr::Enum(id) => self.swift_name_for_enum(id),
            TypeExpr::Handle(id) => self.swift_name_for_class(id),
            TypeExpr::Callback(id) => format!("any {}", pascal_case(id.as_str())),
            TypeExpr::Custom(id) => pascal_case(id.as_str()),
        }
    }

    fn abi_to_swift(&self, abi: AbiType) -> String {
        match abi {
            AbiType::Void => "Void",
            AbiType::Bool => "Bool",
            AbiType::I8 => "Int8",
            AbiType::U8 => "UInt8",
            AbiType::I16 => "Int16",
            AbiType::U16 => "UInt16",
            AbiType::I32 => "Int32",
            AbiType::U32 => "UInt32",
            AbiType::I64 => "Int64",
            AbiType::U64 => "UInt64",
            AbiType::F32 => "Float",
            AbiType::F64 => "Double",
            AbiType::Pointer => "OpaquePointer",
        }
        .to_string()
    }

    fn swift_name_for_record(&self, id: &RecordId) -> String {
        pascal_case(id.as_str())
    }

    fn swift_name_for_enum(&self, id: &EnumId) -> String {
        pascal_case(id.as_str())
    }

    fn swift_name_for_class(&self, id: &ClassId) -> String {
        pascal_case(id.as_str())
    }
}

fn lower_first_char(name: &str) -> String {
    name.chars()
        .enumerate()
        .map(|(index, ch)| if index == 0 { ch.to_ascii_lowercase() } else { ch })
        .collect()
}
