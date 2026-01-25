use heck::{ToLowerCamelCase, ToUpperCamelCase};
use riff_ffi_rules::naming;

pub struct NamingConvention;

impl NamingConvention {
    pub fn class_name(name: &str) -> String {
        name.to_upper_camel_case()
    }

    pub fn method_name(name: &str) -> String {
        let converted = name.to_lower_camel_case();
        Self::escape_keyword(&converted)
    }

    pub fn param_name(name: &str) -> String {
        let converted = Self::tuple_field_name(name).unwrap_or_else(|| name.to_lower_camel_case());
        Self::escape_keyword(&converted)
    }

    pub fn property_name(name: &str) -> String {
        let converted = name.to_lower_camel_case();
        Self::escape_keyword(&converted)
    }

    pub fn enum_case_name(name: &str) -> String {
        let converted = name.to_lower_camel_case();
        Self::escape_keyword(&converted)
    }

    fn tuple_field_name(name: &str) -> Option<String> {
        name.strip_prefix('_')
            .and_then(|rest| rest.parse::<usize>().ok())
            .map(|index| format!("value{}", index))
    }

    pub fn escape_keyword(name: &str) -> String {
        if Self::is_swift_keyword(name) {
            format!("`{}`", name)
        } else {
            name.to_string()
        }
    }

    pub fn is_swift_keyword(name: &str) -> bool {
        matches!(
            name,
            "associatedtype"
                | "class"
                | "deinit"
                | "enum"
                | "extension"
                | "fileprivate"
                | "func"
                | "import"
                | "init"
                | "inout"
                | "internal"
                | "let"
                | "open"
                | "operator"
                | "private"
                | "precedencegroup"
                | "protocol"
                | "public"
                | "rethrows"
                | "static"
                | "struct"
                | "subscript"
                | "typealias"
                | "var"
                | "break"
                | "case"
                | "catch"
                | "continue"
                | "default"
                | "defer"
                | "do"
                | "else"
                | "fallthrough"
                | "for"
                | "guard"
                | "if"
                | "in"
                | "repeat"
                | "return"
                | "throw"
                | "switch"
                | "where"
                | "while"
                | "Any"
                | "as"
                | "await"
                | "false"
                | "is"
                | "nil"
                | "self"
                | "Self"
                | "super"
                | "throws"
                | "true"
                | "try"
                | "Type"
        )
    }

    pub fn ffi_prefix(_module_name: &str) -> String {
        naming::ffi_prefix().to_string()
    }

    pub fn class_ffi_prefix(_module_prefix: &str, class_name: &str) -> String {
        naming::class_ffi_prefix(class_name).into_string()
    }

    pub fn module_name(crate_name: &str) -> String {
        naming::module_name(crate_name)
    }

    pub fn ffi_module_name(crate_name: &str) -> String {
        naming::ffi_module_name(crate_name)
    }
}
