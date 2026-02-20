use crate::ir::abi::AbiContract;
use crate::ir::contract::FfiContract;

use super::JavaOptions;
use super::names::NamingConvention;
use super::plan::JavaModule;

pub struct JavaLowerer<'a> {
    ffi: &'a FfiContract,
    abi: &'a AbiContract,
    package_name: String,
    module_name: String,
    options: JavaOptions,
}

impl<'a> JavaLowerer<'a> {
    pub fn new(
        ffi: &'a FfiContract,
        abi: &'a AbiContract,
        package_name: String,
        module_name: String,
        options: JavaOptions,
    ) -> Self {
        Self {
            ffi,
            abi,
            package_name,
            module_name,
            options,
        }
    }

    pub fn module(&self) -> JavaModule {
        let lib_name = self
            .options
            .library_name
            .clone()
            .unwrap_or_else(|| self.module_name.clone());

        JavaModule {
            package_name: self.package_name.clone(),
            class_name: NamingConvention::class_name(&self.module_name),
            lib_name,
            java_version: self.options.min_java_version,
        }
    }

    pub fn prefix(&self) -> String {
        boltffi_ffi_rules::naming::ffi_prefix().to_string()
    }
}
