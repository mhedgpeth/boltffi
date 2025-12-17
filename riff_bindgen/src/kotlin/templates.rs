use askama::Template;

use crate::model::{Enumeration, Module, Record};

use super::{NamingConvention, TypeMapper};

#[derive(Template)]
#[template(path = "kotlin/preamble.txt", escape = "none")]
pub struct PreambleTemplate {
    pub package_name: String,
}

impl PreambleTemplate {
    pub fn from_module(module: &Module) -> Self {
        Self {
            package_name: NamingConvention::class_name(&module.name).to_lowercase(),
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
    pub fields: Vec<FieldView>,
}

impl SealedEnumTemplate {
    pub fn from_enum(enumeration: &Enumeration) -> Self {
        let variants = enumeration
            .variants
            .iter()
            .map(|variant| SealedVariantView {
                name: NamingConvention::class_name(&variant.name),
                fields: variant
                    .fields
                    .iter()
                    .map(|field| FieldView {
                        name: NamingConvention::property_name(&field.name),
                        kotlin_type: TypeMapper::map_type(&field.field_type),
                    })
                    .collect(),
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
