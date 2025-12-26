use askama::Template;
use riff_ffi_rules::naming;

use crate::model::{CallbackTrait, Class, Enumeration, Function, Module, Record, Type};

use super::marshal::{ParamConversion, ReturnKind};
use super::{NamingConvention, TypeMapper};

#[derive(Template)]
#[template(path = "kotlin/preamble.txt", escape = "none")]
pub struct PreambleTemplate {
    pub package_name: String,
    pub prefix: String,
}

impl PreambleTemplate {
    pub fn from_module(module: &Module) -> Self {
        Self {
            package_name: NamingConvention::class_name(&module.name).to_lowercase(),
            prefix: naming::ffi_prefix().to_string(),
        }
    }

    pub fn with_package(package_name: &str) -> Self {
        Self {
            package_name: package_name.to_string(),
            prefix: naming::ffi_prefix().to_string(),
        }
    }
}

#[derive(Template)]
#[template(path = "kotlin/enum_c_style.txt", escape = "none")]
pub struct CStyleEnumTemplate {
    pub class_name: String,
    pub variants: Vec<EnumVariantView>,
}

pub struct EnumVariantView {
    pub name: String,
    pub value: i64,
}

impl CStyleEnumTemplate {
    pub fn from_enum(enumeration: &Enumeration) -> Self {
        let variants = enumeration
            .variants
            .iter()
            .enumerate()
            .map(|(index, variant)| EnumVariantView {
                name: NamingConvention::enum_entry_name(&variant.name),
                value: variant.discriminant.unwrap_or(index as i64),
            })
            .collect();

        Self {
            class_name: NamingConvention::class_name(&enumeration.name),
            variants,
        }
    }
}

#[derive(Template)]
#[template(path = "kotlin/enum_sealed.txt", escape = "none")]
pub struct SealedEnumTemplate {
    pub class_name: String,
    pub variants: Vec<SealedVariantView>,
}

pub struct SealedVariantView {
    pub name: String,
    pub is_tuple: bool,
    pub fields: Vec<SealedFieldView>,
}

pub struct SealedFieldView {
    pub name: String,
    pub index: usize,
    pub kotlin_type: String,
    pub is_tuple: bool,
}

impl SealedEnumTemplate {
    pub fn from_enum(enumeration: &Enumeration) -> Self {
        let variants = enumeration
            .variants
            .iter()
            .map(|variant| {
                let is_tuple = variant.fields.iter().any(|f| 
                    f.name.starts_with('_') && f.name.chars().nth(1).map_or(false, |c| c.is_ascii_digit())
                );
                SealedVariantView {
                    name: NamingConvention::class_name(&variant.name),
                    is_tuple,
                    fields: variant
                        .fields
                        .iter()
                        .enumerate()
                        .map(|(i, field)| {
                            let field_is_tuple = field.name.starts_with('_') && 
                                field.name.chars().nth(1).map_or(false, |c| c.is_ascii_digit());
                            SealedFieldView {
                                name: NamingConvention::property_name(&field.name),
                                index: i,
                                kotlin_type: TypeMapper::map_type(&field.field_type),
                                is_tuple: field_is_tuple,
                            }
                        })
                        .collect(),
                }
            })
            .collect();

        Self {
            class_name: NamingConvention::class_name(&enumeration.name),
            variants,
        }
    }
}

#[derive(Template)]
#[template(path = "kotlin/record.txt", escape = "none")]
pub struct RecordTemplate {
    pub class_name: String,
    pub fields: Vec<FieldView>,
}

pub struct FieldView {
    pub name: String,
    pub kotlin_type: String,
}

impl RecordTemplate {
    pub fn from_record(record: &Record) -> Self {
        let fields = record
            .fields
            .iter()
            .map(|field| FieldView {
                name: NamingConvention::property_name(&field.name),
                kotlin_type: TypeMapper::map_type(&field.field_type),
            })
            .collect();

        Self {
            class_name: NamingConvention::class_name(&record.name),
            fields,
        }
    }
}

#[derive(Template)]
#[template(path = "kotlin/record_reader.txt", escape = "none")]
pub struct RecordReaderTemplate {
    pub reader_name: String,
    pub class_name: String,
    pub struct_size: usize,
    pub fields: Vec<ReaderFieldView>,
}

pub struct ReaderFieldView {
    pub name: String,
    pub const_name: String,
    pub offset: usize,
    pub getter: String,
    pub conversion: String,
}

impl RecordReaderTemplate {
    pub fn from_record(record: &Record) -> Self {
        use super::layout::KotlinBufferRead;
        use crate::model::Type;
        use heck::ToShoutySnakeCase;

        let offsets = record.field_offsets();
        let fields = record
            .fields
            .iter()
            .zip(offsets)
            .map(|(field, offset)| {
                let (getter, conversion) = match &field.field_type {
                    Type::Primitive(primitive) => (
                        primitive.buffer_getter().to_string(),
                        primitive.buffer_conversion().to_string(),
                    ),
                    _ => ("getLong".to_string(), String::new()),
                };

                ReaderFieldView {
                    name: NamingConvention::property_name(&field.name),
                    const_name: field.name.to_shouty_snake_case(),
                    offset,
                    getter,
                    conversion,
                }
            })
            .collect();

        Self {
            reader_name: format!("{}Reader", NamingConvention::class_name(&record.name)),
            class_name: NamingConvention::class_name(&record.name),
            struct_size: record.struct_size().as_usize(),
            fields,
        }
    }
}

#[derive(Template)]
#[template(path = "kotlin/function.txt", escape = "none")]
pub struct FunctionTemplate {
    pub func_name: String,
    pub ffi_name: String,
    pub prefix: String,
    pub params: Vec<ParamView>,
    pub return_type: Option<String>,
    pub return_kind: ReturnKind,
    pub inner_type: Option<String>,
    pub len_fn: Option<String>,
    pub copy_fn: Option<String>,
    pub reader_name: Option<String>,
    pub is_async: bool,
}

pub struct ParamView {
    pub name: String,
    pub kotlin_type: String,
    pub conversion: String,
}

impl FunctionTemplate {
    pub fn from_function(function: &Function, _module: &Module) -> Self {
        let ffi_name = format!("{}_{}", naming::ffi_prefix(), function.name);
        let return_kind = function
            .output
            .as_ref()
            .map(|ty| ReturnKind::from_type(ty, &ffi_name))
            .unwrap_or(ReturnKind::Void);

        let params: Vec<ParamView> = function
            .inputs
            .iter()
            .map(|param| ParamView {
                name: NamingConvention::param_name(&param.name),
                kotlin_type: TypeMapper::map_type(&param.param_type),
                conversion: ParamConversion::to_ffi(
                    &NamingConvention::param_name(&param.name),
                    &param.param_type,
                ),
            })
            .collect();

        let return_type = function.output.as_ref().map(TypeMapper::map_type);
        let inner_type = return_kind.inner_type().map(String::from);
        let len_fn = return_kind.len_fn().map(String::from);
        let copy_fn = return_kind.copy_fn().map(String::from);
        let reader_name = return_kind.reader_name().map(String::from);

        Self {
            func_name: NamingConvention::method_name(&function.name),
            ffi_name,
            prefix: naming::ffi_prefix().to_string(),
            params,
            return_type,
            return_kind,
            inner_type,
            len_fn,
            copy_fn,
            reader_name,
            is_async: function.is_async,
        }
    }
}

#[derive(Template)]
#[template(path = "kotlin/class.txt", escape = "none")]
pub struct ClassTemplate {
    pub class_name: String,
    pub doc: Option<String>,
    pub ffi_free: String,
    pub constructors: Vec<ConstructorView>,
    pub methods: Vec<MethodView>,
}

pub struct ConstructorView {
    pub ffi_name: String,
    pub params: Vec<ParamView>,
}

pub struct MethodView {
    pub name: String,
    pub params: Vec<ParamView>,
    pub return_type: Option<String>,
    pub body: String,
}

impl ClassTemplate {
    pub fn from_class(class: &Class) -> Self {
        let class_name = NamingConvention::class_name(&class.name);
        let ffi_prefix = naming::class_ffi_prefix(&class.name);

        let constructors: Vec<ConstructorView> = class
            .constructors
            .iter()
            .map(|ctor| ConstructorView {
                ffi_name: format!("{}_new", ffi_prefix),
                params: ctor
                    .inputs
                    .iter()
                    .map(|param| ParamView {
                        name: NamingConvention::param_name(&param.name),
                        kotlin_type: TypeMapper::map_type(&param.param_type),
                        conversion: ParamConversion::to_ffi(
                            &NamingConvention::param_name(&param.name),
                            &param.param_type,
                        ),
                    })
                    .collect(),
            })
            .collect();

        let methods: Vec<MethodView> = class
            .methods
            .iter()
            .map(|method| {
                let method_ffi = naming::method_ffi_name(&class.name, &method.name);
                let return_type = method.output.as_ref().map(TypeMapper::map_type);
                let body = Self::generate_method_body(method, &method_ffi);

                MethodView {
                    name: NamingConvention::method_name(&method.name),
                    params: method
                        .inputs
                        .iter()
                        .map(|param| ParamView {
                            name: NamingConvention::param_name(&param.name),
                            kotlin_type: TypeMapper::map_type(&param.param_type),
                            conversion: ParamConversion::to_ffi(
                                &NamingConvention::param_name(&param.name),
                                &param.param_type,
                            ),
                        })
                        .collect(),
                    return_type,
                    body,
                }
            })
            .collect();

        Self {
            class_name,
            doc: class.doc.clone(),
            ffi_free: format!("{}_free", ffi_prefix),
            constructors,
            methods,
        }
    }

    fn generate_method_body(method: &crate::model::Method, ffi_name: &str) -> String {
        let args = std::iter::once("handle".to_string())
            .chain(method.inputs.iter().map(|p| {
                ParamConversion::to_ffi(
                    &NamingConvention::param_name(&p.name),
                    &p.param_type,
                )
            }))
            .collect::<Vec<_>>()
            .join(", ");

        match &method.output {
            Some(ty) if ty.is_primitive() => format!("return Native.{}({})", ffi_name, args),
            Some(_) => format!("return Native.{}({})", ffi_name, args),
            None => format!(
                "val status = Native.{}({})\n        checkStatus(status)",
                ffi_name, args
            ),
        }
    }
}

#[derive(Template)]
#[template(path = "kotlin/native.txt", escape = "none")]
pub struct NativeTemplate {
    pub lib_name: String,
    pub prefix: String,
    pub functions: Vec<NativeFunctionView>,
    pub classes: Vec<NativeClassView>,
    pub callback_traits: Vec<NativeCallbackTraitView>,
}

pub struct NativeCallbackTraitView {
    pub register_fn: String,
    pub create_fn: String,
}

pub struct NativeFunctionView {
    pub ffi_name: String,
    pub params: Vec<NativeParamView>,
    pub has_out_param: bool,
    pub out_type: String,
    pub return_jni_type: String,
}

pub struct NativeParamView {
    pub name: String,
    pub jni_type: String,
}

pub struct NativeClassView {
    pub ffi_new: String,
    pub ffi_free: String,
    pub ctor_params: Vec<NativeParamView>,
    pub methods: Vec<NativeMethodView>,
}

pub struct NativeMethodView {
    pub ffi_name: String,
    pub params: Vec<NativeParamView>,
    pub has_out_param: bool,
    pub out_type: String,
    pub return_jni_type: String,
}

impl NativeTemplate {
    pub fn from_module(module: &Module) -> Self {
        let prefix = naming::ffi_prefix().to_string();

        let functions: Vec<NativeFunctionView> = module
            .functions
            .iter()
            .map(|func| {
                let ffi_name = naming::function_ffi_name(&func.name);
                let (has_out_param, out_type, return_jni_type) =
                    Self::analyze_return(&func.output);

                NativeFunctionView {
                    ffi_name,
                    params: func
                        .inputs
                        .iter()
                        .map(|p| NativeParamView {
                            name: NamingConvention::param_name(&p.name),
                            jni_type: TypeMapper::jni_type(&p.param_type),
                        })
                        .collect(),
                    has_out_param,
                    out_type,
                    return_jni_type,
                }
            })
            .collect();

        let classes: Vec<NativeClassView> = module
            .classes
            .iter()
            .map(|class| {
                let ffi_prefix = naming::class_ffi_prefix(&class.name);

                let ctor_params: Vec<NativeParamView> = class
                    .constructors
                    .first()
                    .map(|ctor| {
                        ctor.inputs
                            .iter()
                            .map(|p| NativeParamView {
                                name: NamingConvention::param_name(&p.name),
                                jni_type: TypeMapper::jni_type(&p.param_type),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let methods: Vec<NativeMethodView> = class
                    .methods
                    .iter()
                    .map(|method| {
                        let method_ffi = naming::method_ffi_name(&class.name, &method.name);
                        let (has_out_param, out_type, return_jni_type) =
                            Self::analyze_return(&method.output);

                        NativeMethodView {
                            ffi_name: method_ffi,
                            params: method
                                .inputs
                                .iter()
                                .map(|p| NativeParamView {
                                    name: NamingConvention::param_name(&p.name),
                                    jni_type: TypeMapper::jni_type(&p.param_type),
                                })
                                .collect(),
                            has_out_param,
                            out_type,
                            return_jni_type,
                        }
                    })
                    .collect();

                NativeClassView {
                    ffi_new: format!("{}_new", ffi_prefix),
                    ffi_free: format!("{}_free", ffi_prefix),
                    ctor_params,
                    methods,
                }
            })
            .collect();

        let callback_traits: Vec<NativeCallbackTraitView> = module
            .callback_traits
            .iter()
            .map(|cb| {
                let ffi_prefix = format!("{}_{}", naming::ffi_prefix(), cb.name);
                NativeCallbackTraitView {
                    register_fn: format!("{}_register_vtable", ffi_prefix),
                    create_fn: format!("{}_create", ffi_prefix),
                }
            })
            .collect();

        Self {
            lib_name: module.name.clone(),
            prefix,
            functions,
            classes,
            callback_traits,
        }
    }

    fn analyze_return(output: &Option<Type>) -> (bool, String, String) {
        match output {
            None => (false, String::new(), "Int".to_string()),
            Some(ty) => match ty {
                Type::Primitive(_) => (false, String::new(), TypeMapper::jni_type(ty)),
                Type::String => (false, String::new(), "String?".to_string()),
                Type::Bytes => (false, String::new(), "ByteArray?".to_string()),
                Type::Vec(inner) => match inner.as_ref() {
                    Type::Primitive(_) => (false, String::new(), TypeMapper::jni_type(ty)),
                    Type::Record(_) => (false, String::new(), "ByteBuffer".to_string()),
                    _ => (false, String::new(), "Long".to_string()),
                },
                _ => (false, String::new(), TypeMapper::jni_type(ty)),
            },
        }
    }
}

#[derive(Template)]
#[template(path = "kotlin/callback_trait.txt", escape = "none")]
pub struct CallbackTraitTemplate {
    pub doc: Option<String>,
    pub interface_name: String,
    pub wrapper_class: String,
    pub vtable_var: String,
    pub vtable_type: String,
    pub bridge_name: String,
    pub register_fn: String,
    pub create_fn: String,
    pub methods: Vec<TraitMethodView>,
}

pub struct TraitMethodView {
    pub name: String,
    pub ffi_name: String,
    pub params: Vec<TraitParamView>,
    pub return_type: Option<String>,
    pub return_jni_type: String,
    pub has_return: bool,
    pub has_out_param: bool,
    pub out_type: String,
}

pub struct TraitParamView {
    pub name: String,
    pub ffi_name: String,
    pub kotlin_type: String,
    pub jni_type: String,
    pub conversion: String,
}

impl CallbackTraitTemplate {
    pub fn from_trait(callback_trait: &CallbackTrait, _module: &Module) -> Self {
        let trait_name = &callback_trait.name;
        let interface_name = NamingConvention::class_name(trait_name);
        let wrapper_class = format!("{}Wrapper", interface_name);
        let vtable_type = format!("{}VTable", interface_name);
        let ffi_prefix = format!("{}_{}", naming::ffi_prefix(), trait_name);

        let methods: Vec<TraitMethodView> = callback_trait
            .methods
            .iter()
            .map(|method| {
                let has_return = method.has_return();
                let return_jni_type = if has_return {
                    method
                        .output
                        .as_ref()
                        .map(TypeMapper::jni_type)
                        .unwrap_or_else(|| "Unit".to_string())
                } else {
                    "FfiStatus".to_string()
                };

                TraitMethodView {
                    name: NamingConvention::method_name(&method.name),
                    ffi_name: NamingConvention::method_name(&method.name),
                    params: method
                        .inputs
                        .iter()
                        .map(|p| TraitParamView {
                            name: NamingConvention::param_name(&p.name),
                            ffi_name: NamingConvention::param_name(&p.name),
                            kotlin_type: TypeMapper::map_type(&p.param_type),
                            jni_type: TypeMapper::jni_type(&p.param_type),
                            conversion: NamingConvention::param_name(&p.name),
                        })
                        .collect(),
                    return_type: method.output.as_ref().map(TypeMapper::map_type),
                    return_jni_type,
                    has_return,
                    has_out_param: has_return,
                    out_type: method
                        .output
                        .as_ref()
                        .map(|_| "LongByReference".to_string())
                        .unwrap_or_default(),
                }
            })
            .collect();

        Self {
            doc: callback_trait.doc.clone(),
            interface_name,
            wrapper_class,
            vtable_var: format!("{}VTableImpl", NamingConvention::method_name(trait_name)),
            vtable_type,
            bridge_name: format!("{}Bridge", NamingConvention::class_name(trait_name)),
            register_fn: format!("{}_register_vtable", ffi_prefix),
            create_fn: format!("{}_create", ffi_prefix),
            methods,
        }
    }
}
