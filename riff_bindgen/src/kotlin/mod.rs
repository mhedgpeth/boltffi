mod marshal;
mod names;
mod templates;
mod types;

use askama::Template;

pub use marshal::{ParamConversion, ReturnKind};
pub use names::NamingConvention;
pub use templates::{
    CStyleEnumTemplate, FunctionTemplate, PreambleTemplate, RecordTemplate, SealedEnumTemplate,
};
pub use types::TypeMapper;

use crate::model::{Enumeration, Function, Module, Record};

pub struct Kotlin;

impl Kotlin {
    pub fn render_module(module: &Module) -> String {
        let mut sections = Vec::new();

        sections.push(Self::render_preamble(module));

        module
            .enums
            .iter()
            .for_each(|enumeration| sections.push(Self::render_enum(enumeration)));

        module
            .records
            .iter()
            .for_each(|record| sections.push(Self::render_record(record)));

        module
            .functions
            .iter()
            .for_each(|function| sections.push(Self::render_function(function)));

        let mut output = sections
            .into_iter()
            .map(|section| section.trim().to_string())
            .filter(|section| !section.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        output.push('\n');
        output
    }

    pub fn render_preamble(module: &Module) -> String {
        PreambleTemplate::from_module(module)
            .render()
            .expect("preamble template failed")
    }

    pub fn render_enum(enumeration: &Enumeration) -> String {
        if enumeration.is_c_style() {
            CStyleEnumTemplate::from_enum(enumeration)
                .render()
                .expect("c-style enum template failed")
        } else {
            SealedEnumTemplate::from_enum(enumeration)
                .render()
                .expect("sealed enum template failed")
        }
    }

    pub fn render_record(record: &Record) -> String {
        RecordTemplate::from_record(record)
            .render()
            .expect("record template failed")
    }

    pub fn render_function(function: &Function) -> String {
        FunctionTemplate::from_function(function)
            .render()
            .expect("function template failed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Primitive, RecordField, Type, Variant};

    #[test]
    fn test_kotlin_type_mapping() {
        assert_eq!(
            TypeMapper::map_type(&Type::Primitive(Primitive::I32)),
            "Int"
        );
        assert_eq!(
            TypeMapper::map_type(&Type::Primitive(Primitive::I64)),
            "Long"
        );
        assert_eq!(
            TypeMapper::map_type(&Type::Primitive(Primitive::Bool)),
            "Boolean"
        );
        assert_eq!(TypeMapper::map_type(&Type::String), "String");
        assert_eq!(TypeMapper::map_type(&Type::Bytes), "ByteArray");
        assert_eq!(
            TypeMapper::map_type(&Type::Vec(Box::new(Type::Primitive(Primitive::F64)))),
            "List<Double>"
        );
    }

    #[test]
    fn test_kotlin_naming() {
        assert_eq!(
            NamingConvention::class_name("sensor_manager"),
            "SensorManager"
        );
        assert_eq!(NamingConvention::method_name("get_reading"), "getReading");
        assert_eq!(NamingConvention::enum_entry_name("active"), "ACTIVE");
    }

    #[test]
    fn test_kotlin_keyword_escaping() {
        assert_eq!(NamingConvention::escape_keyword("value"), "`value`");
        assert_eq!(NamingConvention::escape_keyword("count"), "count");
    }

    #[test]
    fn test_render_c_style_enum() {
        let status = Enumeration::new("sensor_status")
            .with_variant(Variant::new("idle").with_discriminant(0))
            .with_variant(Variant::new("active").with_discriminant(1))
            .with_variant(Variant::new("error").with_discriminant(2));

        let output = Kotlin::render_enum(&status);
        assert!(output.contains("enum class SensorStatus"));
        assert!(output.contains("IDLE(0)"));
        assert!(output.contains("ACTIVE(1)"));
        assert!(output.contains("fromValue(value: Int)"));
    }

    #[test]
    fn test_render_sealed_class_enum() {
        let result_enum = Enumeration::new("api_result")
            .with_variant(Variant::new("success"))
            .with_variant(
                Variant::new("error")
                    .with_field(RecordField::new("code", Type::Primitive(Primitive::I32))),
            );

        let output = Kotlin::render_enum(&result_enum);
        assert!(output.contains("sealed class ApiResult"));
        assert!(output.contains("data object Success"));
        assert!(output.contains("data class Error"));
        assert!(output.contains("val code: Int"));
    }

    #[test]
    fn test_render_record() {
        let reading = Record::new("sensor_reading")
            .with_field(RecordField::new(
                "timestamp",
                Type::Primitive(Primitive::U64),
            ))
            .with_field(RecordField::new(
                "temperature",
                Type::Primitive(Primitive::F64),
            ));

        let output = Kotlin::render_record(&reading);
        assert!(output.contains("data class SensorReading"));
        assert!(output.contains("val timestamp: ULong"));
        assert!(output.contains("val temperature: Double"));
    }

    #[test]
    fn test_render_function() {
        use crate::model::Parameter;

        let function = Function::new("get_sensor_value")
            .with_param(Parameter::new("sensor_id", Type::Primitive(Primitive::I32)))
            .with_output(Type::Primitive(Primitive::F64));

        let output = Kotlin::render_function(&function);
        assert!(output.contains("fun getSensorValue"));
        assert!(output.contains("sensorId: Int"));
        assert!(output.contains(": Double"));
    }
}
