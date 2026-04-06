use crate::{
    ir::{AbiContract, FfiContract},
    render::dart::lower::DartLowerer,
};

pub struct DartEmitter {}

impl DartEmitter {
    pub fn emit(ffi: &FfiContract, abi: &AbiContract, package_name: &str) -> String {
        let _lowerer = DartLowerer::new(ffi, abi, package_name);

        String::new()
    }
}
