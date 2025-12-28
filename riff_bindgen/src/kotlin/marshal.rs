use crate::model::{Primitive, Type};

#[derive(Debug, Clone)]
pub enum ReturnKind {
    Void,
    Primitive,
    String,
    Vec {
        inner: String,
        len_fn: String,
        copy_fn: String,
        primitive: Option<Primitive>,
    },
    VecRecord {
        inner: String,
        reader: String,
    },
    Option {
        inner: String,
    },
    Result {
        ok: String,
    },
    Enum {
        name: String,
    },
    Record {
        name: String,
    },
}

impl ReturnKind {
    pub fn from_type(ty: &Type, ffi_base: &str) -> Self {
        match ty {
            Type::Void => Self::Void,
            Type::Primitive(_) => Self::Primitive,
            Type::String => Self::String,
            Type::Vec(inner) => match inner.as_ref() {
                Type::Record(name) => Self::VecRecord {
                    inner: super::NamingConvention::class_name(name),
                    reader: format!("{}Reader", super::NamingConvention::class_name(name)),
                },
                _ => Self::Vec {
                    inner: super::TypeMapper::map_type(inner),
                    len_fn: format!("{}_len", ffi_base),
                    copy_fn: format!("{}_copy_into", ffi_base),
                    primitive: match inner.as_ref() {
                        Type::Primitive(p) => Some(*p),
                        _ => None,
                    },
                },
            },
            Type::Option(inner) => Self::Option {
                inner: super::TypeMapper::map_type(inner),
            },
            Type::Result { ok, .. } => Self::Result {
                ok: super::TypeMapper::map_type(ok),
            },
            Type::Enum(name) => Self::Enum {
                name: super::NamingConvention::class_name(name),
            },
            Type::Record(name) => Self::Record {
                name: super::NamingConvention::class_name(name),
            },
            Type::Bytes => panic!("Bytes return type not yet supported in Kotlin bindings"),
            Type::Slice(_) => panic!("Slice return type not yet supported in Kotlin bindings"),
            Type::MutSlice(_) => {
                panic!("MutSlice return type not yet supported in Kotlin bindings")
            }
            Type::Object(name) => panic!(
                "Object return type '{}' not yet supported in Kotlin bindings",
                name
            ),
            Type::BoxedTrait(name) => panic!(
                "BoxedTrait return type '{}' not yet supported in Kotlin bindings",
                name
            ),
            Type::Callback(_) => {
                panic!("Callback return type not yet supported in Kotlin bindings")
            }
        }
    }

    pub fn is_primitive(&self) -> bool {
        matches!(self, Self::Primitive)
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Self::String)
    }

    pub fn is_vec(&self) -> bool {
        matches!(self, Self::Vec { .. })
    }

    pub fn is_vec_record(&self) -> bool {
        matches!(self, Self::VecRecord { .. })
    }

    pub fn reader_name(&self) -> Option<&str> {
        match self {
            Self::VecRecord { reader, .. } => Some(reader),
            _ => None,
        }
    }

    pub fn is_option(&self) -> bool {
        matches!(self, Self::Option { .. })
    }

    pub fn is_result(&self) -> bool {
        matches!(self, Self::Result { .. })
    }

    pub fn is_unit(&self) -> bool {
        matches!(self, Self::Void)
    }

    pub fn is_enum(&self) -> bool {
        matches!(self, Self::Enum { .. })
    }

    pub fn inner_type(&self) -> Option<&str> {
        match self {
            Self::Vec { inner, .. } => Some(inner),
            Self::VecRecord { inner, .. } => Some(inner),
            Self::Option { inner } => Some(inner),
            Self::Result { ok } => Some(ok),
            _ => None,
        }
    }

    pub fn len_fn(&self) -> Option<&str> {
        match self {
            Self::Vec { len_fn, .. } => Some(len_fn),
            _ => None,
        }
    }

    pub fn copy_fn(&self) -> Option<&str> {
        match self {
            Self::Vec { copy_fn, .. } => Some(copy_fn),
            _ => None,
        }
    }

    pub fn vec_list_suffix(&self) -> &str {
        match self {
            Self::Vec {
                primitive: Some(Primitive::U8),
                ..
            } => ".map { it.toUByte() }",
            Self::Vec {
                primitive: Some(Primitive::U16),
                ..
            } => ".map { it.toUShort() }",
            Self::Vec {
                primitive: Some(Primitive::U32),
                ..
            } => ".map { it.toUInt() }",
            Self::Vec {
                primitive: Some(Primitive::U64),
                ..
            } => ".map { it.toULong() }",
            Self::Vec { .. } => ".toList()",
            _ => "",
        }
    }
}

pub struct ParamConversion;

impl ParamConversion {
    pub fn to_ffi(param_name: &str, ty: &Type) -> String {
        match ty {
            Type::String => param_name.to_string(),
            Type::Bytes => param_name.to_string(),
            Type::Primitive(primitive) => match primitive {
                Primitive::U8 => format!("{}.toByte()", param_name),
                Primitive::U16 => format!("{}.toShort()", param_name),
                Primitive::U32 => format!("{}.toInt()", param_name),
                Primitive::U64 => format!("{}.toLong()", param_name),
                _ => param_name.to_string(),
            },
            Type::Record(_) => param_name.to_string(),
            Type::Enum(_) => format!("{}.value", param_name),
            Type::Object(_) => format!("{}.handle", param_name),
            Type::Vec(inner) | Type::Slice(inner) => match inner.as_ref() {
                Type::Record(name) => {
                    format!(
                        "{}Writer.pack({})",
                        super::NamingConvention::class_name(name),
                        param_name
                    )
                }
                Type::Primitive(Primitive::I8) => format!("{}.toByteArray()", param_name),
                Type::Primitive(Primitive::U8) => {
                    format!("{}.map {{ it.toByte() }}.toByteArray()", param_name)
                }
                Type::Primitive(Primitive::I16) => format!("{}.toShortArray()", param_name),
                Type::Primitive(Primitive::U16) => {
                    format!("{}.map {{ it.toShort() }}.toShortArray()", param_name)
                }
                Type::Primitive(Primitive::I32) => format!("{}.toIntArray()", param_name),
                Type::Primitive(Primitive::U32) => {
                    format!("{}.map {{ it.toInt() }}.toIntArray()", param_name)
                }
                Type::Primitive(Primitive::I64) => format!("{}.toLongArray()", param_name),
                Type::Primitive(Primitive::U64) => {
                    format!("{}.map {{ it.toLong() }}.toLongArray()", param_name)
                }
                Type::Primitive(Primitive::F32) => format!("{}.toFloatArray()", param_name),
                Type::Primitive(Primitive::F64) => format!("{}.toDoubleArray()", param_name),
                Type::Primitive(Primitive::Bool) => format!("{}.toBooleanArray()", param_name),
                _ => param_name.to_string(),
            },
            _ => param_name.to_string(),
        }
    }
}

// JNI-specific types for C glue generation

#[derive(Debug, Clone)]
pub enum JniReturnKind {
    Void,
    Primitive { jni_type: String },
    String { ffi_name: String },
    Vec { len_fn: String, copy_fn: String },
    CStyleEnum,
    DataEnum { enum_name: String, struct_size: usize },
}

impl JniReturnKind {
    pub fn from_type(ty: Option<&Type>, _func_name: &str) -> Self {
        match ty {
            None | Some(Type::Void) => Self::Void,
            Some(Type::Primitive(primitive)) => Self::Primitive {
                jni_type: super::TypeMapper::c_jni_type(&Type::Primitive(*primitive)),
            },
            Some(Type::String) => Self::String {
                ffi_name: riff_ffi_rules::naming::function_ffi_name(_func_name),
            },
            Some(Type::Vec(_)) => Self::Vec {
                len_fn: riff_ffi_rules::naming::function_ffi_vec_len(_func_name),
                copy_fn: riff_ffi_rules::naming::function_ffi_vec_copy_into(_func_name),
            },
            Some(Type::Enum(_)) => Self::CStyleEnum,
            _ => Self::Void,
        }
    }

    pub fn from_type_with_module(
        ty: Option<&Type>,
        func_name: &str,
        module: &crate::model::Module,
    ) -> Self {
        match ty {
            Some(Type::Enum(enum_name)) => {
                let enumeration = module.enums.iter().find(|e| &e.name == enum_name);
                match enumeration {
                    Some(e) if e.is_data_enum() => {
                        let layout = crate::model::DataEnumLayout::from_enum(e);
                        Self::DataEnum {
                            enum_name: super::NamingConvention::class_name(enum_name),
                            struct_size: layout.map(|l| l.struct_size().as_usize()).unwrap_or(0),
                        }
                    }
                    _ => Self::CStyleEnum,
                }
            }
            _ => Self::from_type(ty, func_name),
        }
    }

    pub fn is_void(&self) -> bool {
        matches!(self, Self::Void)
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Self::String { .. })
    }

    pub fn is_vec(&self) -> bool {
        matches!(self, Self::Vec { .. })
    }

    pub fn is_c_style_enum(&self) -> bool {
        matches!(self, Self::CStyleEnum)
    }

    pub fn is_data_enum(&self) -> bool {
        matches!(self, Self::DataEnum { .. })
    }

    pub fn jni_return_type(&self) -> &str {
        match self {
            Self::Void => "void",
            Self::Primitive { jni_type } => jni_type,
            Self::String { .. } => "jstring",
            Self::Vec { .. } => "jlong",
            Self::CStyleEnum => "jint",
            Self::DataEnum { .. } => "jobject",
        }
    }

    pub fn data_enum_struct_size(&self) -> usize {
        match self {
            Self::DataEnum { struct_size, .. } => *struct_size,
            _ => 0,
        }
    }

    pub fn data_enum_name(&self) -> Option<&str> {
        match self {
            Self::DataEnum { enum_name, .. } => Some(enum_name),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct JniParamInfo {
    pub name: String,
    pub jni_type: String,
    pub is_string: bool,
    pub is_handle: bool,
    pub array_primitive: Option<Primitive>,
    pub array_is_mutable: bool,
    pub record_name: Option<String>,
    pub record_struct_size: usize,
    pub record_is_mutable: bool,
    pub data_enum_name: Option<String>,
    pub data_enum_struct_size: usize,
}

impl JniParamInfo {
    pub fn from_param(name: &str, ty: &Type) -> Self {
        let (array_primitive, array_is_mutable) = match ty {
            Type::Vec(inner) | Type::Slice(inner) | Type::MutSlice(inner) => match inner.as_ref() {
                Type::Primitive(primitive) => (Some(*primitive), matches!(ty, Type::MutSlice(_))),
                _ => (None, false),
            },
            _ => (None, false),
        };

        let (record_name, record_is_mutable) = match ty {
            Type::Vec(inner) | Type::Slice(inner) | Type::MutSlice(inner) => match inner.as_ref() {
                Type::Record(record_name) => {
                    (Some(record_name.clone()), matches!(ty, Type::MutSlice(_)))
                }
                _ => (None, false),
            },
            _ => (None, false),
        };

        let data_enum_name = match ty {
            Type::Enum(enum_name) => Some(enum_name.clone()),
            _ => None,
        };

        Self {
            name: name.to_string(),
            jni_type: super::TypeMapper::c_jni_type(ty),
            is_string: matches!(ty, Type::String),
            is_handle: matches!(ty, Type::Object(_) | Type::BoxedTrait(_)),
            array_primitive,
            array_is_mutable,
            record_name,
            record_struct_size: 0,
            record_is_mutable,
            data_enum_name,
            data_enum_struct_size: 0,
        }
    }

    pub fn jni_param_decl(&self) -> String {
        format!("{} {}", self.jni_type, self.name)
    }

    pub fn ffi_arg(&self) -> String {
        if self.is_string {
            format!(
                "(const uint8_t*)_{}_c, {} ? strlen(_{}_c) : 0",
                self.name, self.name, self.name
            )
        } else if let Some(enum_name) = &self.data_enum_name {
            let c_name = super::NamingConvention::class_name(enum_name);
            format!("*({}*)_{}_ptr", c_name, self.name)
        } else if let Some(record_name) = &self.record_name {
            let c_name = super::NamingConvention::class_name(record_name);
            let ptr_type = if self.record_is_mutable {
                format!("{}*", c_name)
            } else {
                format!("const {}*", c_name)
            };

            format!(
                "({})_{}_ptr, (uintptr_t)_{}_len",
                ptr_type, self.name, self.name
            )
        } else if let Some(primitive) = self.array_primitive {
            let c_type = primitive.c_type_name();
            let ptr_type = if self.array_is_mutable {
                format!("{}*", c_type)
            } else {
                format!("const {}*", c_type)
            };

            format!(
                "({})_{}_ptr, (uintptr_t)_{}_len",
                ptr_type, self.name, self.name
            )
        } else if self.is_handle {
            format!("(void*){}", self.name)
        } else {
            self.name.clone()
        }
    }

    pub fn is_primitive_array(&self) -> bool {
        self.array_primitive.is_some()
    }

    pub fn is_record_buffer(&self) -> bool {
        self.record_name.is_some()
    }

    pub fn is_data_enum(&self) -> bool {
        self.data_enum_name.is_some()
    }

    pub fn data_enum_c_type(&self) -> String {
        self.data_enum_name
            .as_ref()
            .map(|name| super::NamingConvention::class_name(name))
            .unwrap_or_default()
    }

    pub fn array_c_type(&self) -> &'static str {
        self.array_primitive
            .expect("array_c_type called on non-array param")
            .c_type_name()
    }

    pub fn array_release_mode(&self) -> &'static str {
        if self.array_is_mutable {
            "0"
        } else {
            "JNI_ABORT"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Primitive;

    #[test]
    fn test_return_kind_primitives() {
        assert!(ReturnKind::from_type(&Type::Primitive(Primitive::I32), "test").is_primitive());
        assert!(ReturnKind::from_type(&Type::Primitive(Primitive::Bool), "test").is_primitive());
    }

    #[test]
    fn test_return_kind_string() {
        assert!(ReturnKind::from_type(&Type::String, "test").is_string());
    }

    #[test]
    fn test_return_kind_vec() {
        let vec_type = Type::Vec(Box::new(Type::Primitive(Primitive::I32)));
        let kind = ReturnKind::from_type(&vec_type, "test_fn");
        assert!(kind.is_vec());
        assert_eq!(kind.len_fn(), Some("test_fn_len"));
        assert_eq!(kind.copy_fn(), Some("test_fn_copy_into"));
        assert_eq!(kind.inner_type(), Some("Int"));
    }

    #[test]
    fn test_return_kind_void() {
        assert!(ReturnKind::from_type(&Type::Void, "test").is_unit());
    }

    #[test]
    fn test_param_conversion_string() {
        assert_eq!(
            ParamConversion::to_ffi("name", &Type::String),
            "name"
        );
    }

    #[test]
    fn test_param_conversion_enum() {
        assert_eq!(
            ParamConversion::to_ffi("status", &Type::Enum("Status".into())),
            "status.value"
        );
    }

    #[test]
    fn test_param_conversion_object() {
        assert_eq!(
            ParamConversion::to_ffi("sensor", &Type::Object("Sensor".into())),
            "sensor.handle"
        );
    }

    #[test]
    fn test_param_conversion_primitive() {
        assert_eq!(
            ParamConversion::to_ffi("count", &Type::Primitive(Primitive::I32)),
            "count"
        );
    }
}
