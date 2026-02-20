use crate::ir::abi::AbiContract;
use crate::ir::contract::FfiContract;

use super::JavaOptions;
use super::lower::JavaLowerer;
use super::templates::{NativeTemplate, PreambleTemplate};
use askama::Template;

pub struct JavaOutput {
    pub source: String,
    pub class_name: String,
    pub package_path: String,
}

pub struct JavaEmitter;

impl JavaEmitter {
    pub fn emit(
        ffi: &FfiContract,
        abi: &AbiContract,
        package_name: String,
        module_name: String,
        options: JavaOptions,
    ) -> JavaOutput {
        let lowerer = JavaLowerer::new(ffi, abi, package_name, module_name, options);
        let module = lowerer.module();
        let prefix = lowerer.prefix();

        let mut source = String::new();

        let preamble = PreambleTemplate { module: &module };
        source.push_str(&preamble.render().expect("preamble template failed"));

        let native = NativeTemplate {
            module: &module,
            prefix: &prefix,
        };
        source.push_str(&native.render().expect("native template failed"));

        JavaOutput {
            source,
            class_name: module.class_name.clone(),
            package_path: module.package_path(),
        }
    }
}
