use crate::ir::plan::AbiType;
use crate::ir::types::PrimitiveType;

pub fn primitive_c_type(p: PrimitiveType) -> String {
    match p {
        PrimitiveType::Bool => "bool".to_string(),
        PrimitiveType::I8 => "int8_t".to_string(),
        PrimitiveType::U8 => "uint8_t".to_string(),
        PrimitiveType::I16 => "int16_t".to_string(),
        PrimitiveType::U16 => "uint16_t".to_string(),
        PrimitiveType::I32 => "int32_t".to_string(),
        PrimitiveType::U32 => "uint32_t".to_string(),
        PrimitiveType::I64 => "int64_t".to_string(),
        PrimitiveType::U64 => "uint64_t".to_string(),
        PrimitiveType::F32 => "float".to_string(),
        PrimitiveType::F64 => "double".to_string(),
        PrimitiveType::ISize => "intptr_t".to_string(),
        PrimitiveType::USize => "uintptr_t".to_string(),
    }
}

pub fn abi_type_c(abi_type: &AbiType) -> String {
    match abi_type {
        AbiType::Void => "void".to_string(),
        AbiType::Bool => "bool".to_string(),
        AbiType::I8 => "int8_t".to_string(),
        AbiType::U8 => "uint8_t".to_string(),
        AbiType::I16 => "int16_t".to_string(),
        AbiType::U16 => "uint16_t".to_string(),
        AbiType::I32 => "int32_t".to_string(),
        AbiType::U32 => "uint32_t".to_string(),
        AbiType::I64 => "int64_t".to_string(),
        AbiType::U64 => "uint64_t".to_string(),
        AbiType::F32 => "float".to_string(),
        AbiType::F64 => "double".to_string(),
        AbiType::ISize => "intptr_t".to_string(),
        AbiType::USize => "uintptr_t".to_string(),
        AbiType::Pointer(element) => format!("{}*", primitive_c_type(*element)),
        AbiType::InlineCallbackFn(params) => {
            let mut param_types = vec!["void*".to_string()];
            param_types.extend(params.iter().map(|p| match p {
                AbiType::Pointer(element) => format!("const {}*", primitive_c_type(*element)),
                other => abi_type_c(other),
            }));
            format!("void (*)({})", param_types.join(", "))
        }
        AbiType::Handle(class_id) => format!("const struct {} *", class_id.as_str()),
        AbiType::CallbackHandle => "BoltFFICallbackHandle".to_string(),
        AbiType::Struct(record_id) => format!("___{}", record_id.as_str()),
    }
}
