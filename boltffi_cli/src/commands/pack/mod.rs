mod all;
mod request;
#[cfg(test)]
mod tests;

use crate::config::Config;
use crate::error::Result;
use crate::reporter::Reporter;

pub use self::request::{
    PackAllOptions, PackAndroidOptions, PackAppleOptions, PackCommand, PackJavaOptions,
    PackWasmOptions,
};
pub(crate) use crate::pack::android::pack_android;
pub(crate) use crate::pack::apple::pack_apple;
pub(crate) use crate::pack::java::{
    check_java_packaging_prereqs, ensure_java_no_build_supported, pack_java, prepare_java_packaging,
};
pub(crate) use crate::pack::wasm::pack_wasm;

#[cfg(test)]
pub(crate) use crate::cargo::{
    Cargo, CargoMetadata, CargoMetadataPackage, CargoMetadataPackageTarget,
};
#[cfg(test)]
pub(crate) use crate::pack::java::{
    JniIncludeDirectories, JniLinkerArgs, JvmCargoContext, JvmCrateOutputs,
    JvmPackagedNativeOutput, JvmPackagingTarget, bundled_jvm_shared_library_path,
    clang_cl_jni_linker_args, clang_native_static_library_flags, clang_style_jni_linker_args,
    ensure_java_pack_cargo_args_supported, existing_jvm_shared_library_path,
    extract_library_filenames, extract_link_search_paths, extract_native_static_libraries,
    link_search_path_flags, msvc_link_search_path_flags, msvc_native_static_library_flags,
    msvc_rustflag_linker_args, parse_native_static_libraries, remove_file_if_exists,
    remove_stale_flat_jvm_outputs_if_current_host_unrequested,
    remove_stale_requested_jvm_shared_library_copies_after_success,
    remove_stale_structured_jvm_outputs, resolve_jni_include_directories_with_overrides,
    resolve_jvm_native_link_input, select_windows_static_library_filename,
    selected_jvm_package_source_directory, target_specific_java_home_env_key,
    target_specific_java_include_env_key,
};
#[cfg(test)]
pub(crate) use crate::pack::missing_built_libraries;

pub fn run_pack(config: &Config, command: PackCommand, reporter: &Reporter) -> Result<()> {
    match command {
        PackCommand::All(options) => all::pack_all(config, options, reporter),
        PackCommand::Apple(options) => pack_apple(config, options, reporter),
        PackCommand::Android(options) => pack_android(config, options, reporter),
        PackCommand::Wasm(options) => pack_wasm(config, options, reporter),
        PackCommand::Java(options) => pack_java(config, options, None, reporter),
    }
}
