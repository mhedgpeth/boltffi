use super::{
    Cargo, CargoMetadata, CargoMetadataPackage, CargoMetadataPackageTarget, JniIncludeDirectories,
    JniLinkerArgs, JvmCargoContext, JvmCrateOutputs, JvmPackagedNativeOutput, JvmPackagingTarget,
    bundled_jvm_shared_library_path, clang_cl_jni_linker_args, clang_native_static_library_flags,
    clang_style_jni_linker_args, ensure_java_no_build_supported,
    ensure_java_pack_cargo_args_supported, existing_jvm_shared_library_path,
    extract_library_filenames, extract_link_search_paths, extract_native_static_libraries,
    link_search_path_flags, missing_built_libraries, msvc_link_search_path_flags,
    msvc_native_static_library_flags, msvc_rustflag_linker_args, parse_native_static_libraries,
    remove_file_if_exists, remove_stale_flat_jvm_outputs_if_current_host_unrequested,
    remove_stale_requested_jvm_shared_library_copies_after_success,
    remove_stale_structured_jvm_outputs, resolve_jni_include_directories_with_overrides,
    resolve_jvm_native_link_input, select_windows_static_library_filename,
    selected_jvm_package_source_directory, target_specific_java_home_env_key,
    target_specific_java_include_env_key,
};
use crate::build::CargoBuildProfile;
use crate::config::{CargoConfig, Config, PackageConfig, TargetsConfig};
use crate::error::CliError;
use crate::target::{BuiltLibrary, JavaHostTarget, RustTarget};
use crate::toolchain::NativeHostToolchain;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn cargo(arguments: &[&str]) -> Cargo {
    Cargo::in_working_directory(
        std::env::current_dir().unwrap_or_default(),
        &arguments
            .iter()
            .map(|argument| argument.to_string())
            .collect::<Vec<_>>(),
    )
}

#[test]
fn parses_target_directory_from_cargo_metadata() {
    let metadata = br#"{
            "packages": [],
            "workspace_members": [],
            "workspace_default_members": [],
            "resolve": null,
            "target_directory": "/tmp/boltffi-target",
            "version": 1,
            "workspace_root": "/tmp/demo"
        }"#;

    let target_directory =
        CargoMetadata::target_directory_from_bytes(metadata).expect("expected target directory");

    assert_eq!(target_directory, PathBuf::from("/tmp/boltffi-target"));
}

#[test]
fn reports_missing_built_libraries_for_unbuilt_configured_targets() {
    let libraries = vec![BuiltLibrary {
        target: RustTarget::ANDROID_ARM64,
        path: PathBuf::from("/tmp/libdemo.a"),
    }];

    let missing = missing_built_libraries(
        &[RustTarget::ANDROID_ARM64, RustTarget::ANDROID_X86_64],
        &libraries,
    );

    assert_eq!(missing, vec!["x86_64-linux-android".to_string()]);
}

#[test]
fn parses_native_static_library_flags_from_cargo_output() {
    let parsed = parse_native_static_libraries(
        "note: native-static-libs: -framework Security -lresolv -lc++",
    )
    .expect("expected static library flags");

    assert_eq!(parsed, vec!["-framework", "Security", "-lresolv", "-lc++"]);
}

#[test]
fn parses_native_static_library_flags_from_ansi_colored_cargo_output() {
    let parsed =
        parse_native_static_libraries("note: native-static-libs: -lSystem -lc -lm\u{1b}[0m")
            .expect("expected static library flags");

    assert_eq!(parsed, vec!["-lSystem", "-lc", "-lm"]);
}

#[test]
fn preserves_repeated_framework_prefixes_in_native_static_library_flags() {
    let parsed = parse_native_static_libraries(
        "note: native-static-libs: -framework Security -framework SystemConfiguration -lobjc",
    )
    .expect("expected static library flags");

    assert_eq!(
        parsed,
        vec![
            "-framework",
            "Security",
            "-framework",
            "SystemConfiguration",
            "-lobjc",
        ]
    );
}

#[test]
fn extracts_last_native_static_library_line_from_combined_output() {
    let parsed = extract_native_static_libraries(
            "Compiling demo\nnote: native-static-libs: -lSystem\nFinished\nnote: native-static-libs: -framework CoreFoundation -lSystem\n",
        )
        .expect("expected static library flags");

    assert_eq!(parsed, vec!["-framework", "CoreFoundation", "-lSystem"]);
}

#[test]
fn extracts_link_search_paths_from_build_script_messages() {
    let linked_paths = extract_link_search_paths(
        r#"{"reason":"compiler-artifact","package_id":"path+file:///tmp/demo#0.1.0"}
{"reason":"build-script-executed","package_id":"path+file:///tmp/dep#0.1.0","linked_paths":["native=/tmp/out","framework=/tmp/frameworks","native=/tmp/out"]}"#,
    );

    assert_eq!(
        linked_paths,
        vec![
            "native=/tmp/out".to_string(),
            "framework=/tmp/frameworks".to_string(),
        ]
    );
}

#[test]
fn converts_link_search_paths_to_clang_flags() {
    let flags = link_search_path_flags(&[
        "native=/tmp/out".to_string(),
        "framework=/tmp/frameworks".to_string(),
        "dependency=/tmp/deps".to_string(),
        "/tmp/plain".to_string(),
        "native=/tmp/out".to_string(),
    ]);

    assert_eq!(
        flags,
        vec![
            "-L/tmp/out".to_string(),
            "-F/tmp/frameworks".to_string(),
            "-L/tmp/deps".to_string(),
            "-L/tmp/plain".to_string(),
        ]
    );
}

#[test]
fn converts_link_search_paths_to_msvc_flags() {
    let flags = msvc_link_search_path_flags(&[
        "native=/tmp/out".to_string(),
        "dependency=/tmp/deps".to_string(),
        "framework=/tmp/frameworks".to_string(),
        "/tmp/plain".to_string(),
        "native=/tmp/out".to_string(),
    ]);

    assert_eq!(
        flags,
        vec![
            "/LIBPATH:/tmp/out".to_string(),
            "/LIBPATH:/tmp/deps".to_string(),
            "/LIBPATH:/tmp/plain".to_string(),
        ]
    );
}

#[test]
fn converts_native_static_libraries_to_msvc_flags() {
    let flags = msvc_native_static_library_flags(&[
        "-l".to_string(),
        "bcrypt".to_string(),
        "-lws2_32".to_string(),
        "-l:custom.lib".to_string(),
        "userenv.lib".to_string(),
        "-framework".to_string(),
        "Security".to_string(),
    ]);

    assert_eq!(
        flags,
        vec![
            "bcrypt.lib".to_string(),
            "ws2_32.lib".to_string(),
            "custom.lib".to_string(),
            "userenv.lib".to_string(),
        ]
    );
}

#[test]
fn strips_implicit_darwin_system_libraries_from_clang_flags() {
    let flags = clang_native_static_library_flags(
        JavaHostTarget::DarwinArm64,
        &[
            "-framework".to_string(),
            "Security".to_string(),
            "-lc".to_string(),
            "-l".to_string(),
            "m".to_string(),
            "-lSystem".to_string(),
            "-liconv".to_string(),
        ],
    );

    assert_eq!(
        flags,
        vec![
            "-framework".to_string(),
            "Security".to_string(),
            "-liconv".to_string(),
        ]
    );
}

#[test]
fn preserves_linux_system_libraries_in_clang_flags() {
    let flags = clang_native_static_library_flags(
        JavaHostTarget::LinuxX86_64,
        &[
            "-ldl".to_string(),
            "-lpthread".to_string(),
            "-lm".to_string(),
        ],
    );

    assert_eq!(
        flags,
        vec![
            "-ldl".to_string(),
            "-lpthread".to_string(),
            "-lm".to_string(),
        ]
    );
}

#[test]
fn converts_msvc_rustflag_linker_args() {
    let flags = msvc_rustflag_linker_args(&[
        "-L/tmp/native".to_string(),
        "-lws2_32".to_string(),
        "userenv.lib".to_string(),
        "/DEBUG".to_string(),
    ])
    .expect("msvc rustflag conversion");

    assert_eq!(
        flags,
        vec![
            "/LIBPATH:/tmp/native".to_string(),
            "ws2_32.lib".to_string(),
            "userenv.lib".to_string(),
            "/DEBUG".to_string(),
        ]
    );
}

#[test]
fn rejects_unsupported_msvc_rustflag_linker_args() {
    let error = msvc_rustflag_linker_args(&["-Wl,--as-needed".to_string()])
        .expect_err("unsupported flag should fail");

    assert!(matches!(
        error,
        CliError::CommandFailed { command, status: None }
            if command.contains("-Wl,--as-needed")
    ));
}

#[test]
fn builds_clang_cl_jni_linker_args_with_msvc_flags() {
    let include_directories = JniIncludeDirectories {
        shared: PathBuf::from("/tmp/jdk/include"),
        platform: PathBuf::from("/tmp/jdk/include/win32"),
    };

    let args = clang_cl_jni_linker_args(&JniLinkerArgs {
        host_target: JavaHostTarget::WindowsX86_64,
        output_lib: Path::new("/tmp/out/demo_jni.dll"),
        jni_glue: Path::new("/tmp/jni/jni_glue.c"),
        link_input: Path::new("/tmp/target/demo.lib"),
        jni_dir: Path::new("/tmp/jni"),
        jni_include_directories: &include_directories,
        rustflag_linker_args: &["-L/tmp/rustflag-native".to_string(), "-luser32".to_string()],
        native_link_search_paths: &["native=/tmp/native".to_string()],
        native_static_libraries: &["-lws2_32".to_string(), "userenv.lib".to_string()],
        rpath_flag: None,
    })
    .expect("msvc jni args");

    assert_eq!(
        args,
        vec![
            "/LD".to_string(),
            "/tmp/jni/jni_glue.c".to_string(),
            "/tmp/target/demo.lib".to_string(),
            "/I/tmp/jni".to_string(),
            "/I/tmp/jdk/include".to_string(),
            "/I/tmp/jdk/include/win32".to_string(),
            "/link".to_string(),
            "/OUT:/tmp/out/demo_jni.dll".to_string(),
            "/LIBPATH:/tmp/rustflag-native".to_string(),
            "user32.lib".to_string(),
            "/LIBPATH:/tmp/native".to_string(),
            "ws2_32.lib".to_string(),
            "userenv.lib".to_string(),
        ]
    );
}

#[test]
fn strips_implicit_darwin_system_libraries_from_clang_jni_args() {
    let include_directories = JniIncludeDirectories {
        shared: PathBuf::from("/tmp/jdk/include"),
        platform: PathBuf::from("/tmp/jdk/include/darwin"),
    };

    let args = clang_style_jni_linker_args(&JniLinkerArgs {
        host_target: JavaHostTarget::DarwinArm64,
        output_lib: Path::new("/tmp/out/libdemo_jni.dylib"),
        jni_glue: Path::new("/tmp/jni/jni_glue.c"),
        link_input: Path::new("/tmp/target/libdemo.a"),
        jni_dir: Path::new("/tmp/jni"),
        jni_include_directories: &include_directories,
        rustflag_linker_args: &[],
        native_link_search_paths: &[],
        native_static_libraries: &[
            "-framework".to_string(),
            "Security".to_string(),
            "-lc".to_string(),
            "-lm".to_string(),
            "-lSystem".to_string(),
            "-liconv".to_string(),
        ],
        rpath_flag: Some("-Wl,-rpath,@loader_path"),
    });

    assert_eq!(
        args,
        vec![
            "-shared".to_string(),
            "-fPIC".to_string(),
            "-o".to_string(),
            "/tmp/out/libdemo_jni.dylib".to_string(),
            "/tmp/jni/jni_glue.c".to_string(),
            "/tmp/target/libdemo.a".to_string(),
            "-I/tmp/jni".to_string(),
            "-I/tmp/jdk/include".to_string(),
            "-I/tmp/jdk/include/darwin".to_string(),
            "-framework".to_string(),
            "Security".to_string(),
            "-liconv".to_string(),
            "-Wl,-rpath,@loader_path".to_string(),
        ]
    );
}

#[test]
fn rejects_pack_all_no_build_when_java_is_enabled() {
    let config = Config {
        experimental: Vec::new(),
        cargo: CargoConfig::default(),
        package: PackageConfig {
            name: "workspace-member".to_string(),
            crate_name: None,
            version: None,
            description: None,
            license: None,
            repository: None,
        },
        targets: TargetsConfig {
            java: crate::config::JavaConfig {
                jvm: crate::config::JavaJvmConfig {
                    enabled: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        },
    };

    let error = ensure_java_no_build_supported(&config, true, false, "pack all")
        .expect_err("expected no-build rejection");

    assert!(matches!(
        error,
        CliError::CommandFailed { command, status: None }
            if command.contains("pack all --no-build is unsupported in Phase 4")
    ));
}

#[test]
fn allows_pack_all_no_build_when_java_is_disabled() {
    let config = Config {
        experimental: Vec::new(),
        cargo: CargoConfig::default(),
        package: PackageConfig {
            name: "workspace-member".to_string(),
            crate_name: None,
            version: None,
            description: None,
            license: None,
            repository: None,
        },
        targets: TargetsConfig::default(),
    };

    ensure_java_no_build_supported(&config, true, false, "pack all")
        .expect("expected no-build to be allowed");
}

#[test]
fn rejects_explicit_cargo_target_for_pack_java() {
    let error = ensure_java_pack_cargo_args_supported(&[
        "--target".to_string(),
        "x86_64-unknown-linux-gnu".to_string(),
    ])
    .expect_err("expected explicit target rejection");

    assert!(matches!(
        error,
        CliError::CommandFailed { command, status: None }
            if command.contains("remove cargo --target 'x86_64-unknown-linux-gnu'")
    ));
}

#[test]
fn extracts_library_filenames_from_print_file_names_output() {
    let filenames = extract_library_filenames(
        "Compiling demo\nlibdemo.a\nlibdemo.dylib\nlibdemo.rlib\nFinished\n",
    );

    assert_eq!(
        filenames,
        vec![
            "libdemo.a".to_string(),
            "libdemo.dylib".to_string(),
            "libdemo.rlib".to_string(),
        ]
    );
}

#[test]
fn selects_windows_static_library_filename_from_reported_outputs() {
    let filename = select_windows_static_library_filename(
        "demo",
        &[
            "demo.lib".to_string(),
            "demo.dll".to_string(),
            "demo.rlib".to_string(),
        ],
    )
    .expect("expected windows staticlib filename");

    assert_eq!(filename, "demo.lib");
}

#[test]
fn selects_windows_gnu_static_library_filename_from_reported_outputs() {
    let filename = select_windows_static_library_filename(
        "demo",
        &[
            "libdemo.a".to_string(),
            "demo.dll".to_string(),
            "demo.rlib".to_string(),
        ],
    )
    .expect("expected windows gnu staticlib filename");

    assert_eq!(filename, "libdemo.a");
}

#[test]
fn splits_toolchain_selector_from_cargo_args() {
    let cargo = cargo(&["--features", "demo", "+nightly", "--locked"]);

    assert_eq!(cargo.toolchain_selector(), Some("+nightly"));
    assert_eq!(
        cargo.command_arguments(),
        vec![
            "--features".to_string(),
            "demo".to_string(),
            "--locked".to_string()
        ]
    );
}

#[test]
fn keeps_metadata_relevant_cargo_args() {
    let metadata_args = cargo(&[
        "+nightly",
        "--target-dir",
        "out/target",
        "--config=build.target-dir=\"other-target\"",
        "--locked",
        "--features",
        "demo",
        "--manifest-path",
        "examples/demo/Cargo.toml",
        "-Zunstable-options",
    ])
    .metadata_passthrough_arguments();

    assert_eq!(
        metadata_args,
        vec![
            "+nightly".to_string(),
            "--target-dir".to_string(),
            "out/target".to_string(),
            "--config=build.target-dir=\"other-target\"".to_string(),
            "--locked".to_string(),
            "--manifest-path".to_string(),
            "examples/demo/Cargo.toml".to_string(),
            "-Zunstable-options".to_string(),
        ]
    );
}

#[test]
fn canonicalizes_manifest_path_from_split_cargo_args() {
    let expected = std::env::current_dir()
        .expect("current dir")
        .join("Cargo.toml")
        .canonicalize()
        .expect("canonical manifest path");

    let manifest_path = cargo(&["--manifest-path", "Cargo.toml"])
        .manifest_path()
        .expect("manifest path");

    assert_eq!(manifest_path, expected);
}

#[test]
fn canonicalizes_manifest_path_from_equals_cargo_arg() {
    let expected = std::env::current_dir()
        .expect("current dir")
        .join("Cargo.toml")
        .canonicalize()
        .expect("canonical manifest path");

    let manifest_path = cargo(&["--manifest-path=Cargo.toml"])
        .manifest_path()
        .expect("manifest path");

    assert_eq!(manifest_path, expected);
}

#[test]
fn canonicalizes_implicit_manifest_path() {
    let expected = std::env::current_dir()
        .expect("current dir")
        .join("Cargo.toml")
        .canonicalize()
        .expect("canonical manifest path");

    let manifest_path = cargo(&[]).manifest_path().expect("manifest path");

    assert_eq!(manifest_path, expected);
}

#[test]
fn extracts_last_package_selector_from_cargo_args() {
    let package_selector = cargo(&[
        "--manifest-path",
        "Cargo.toml",
        "-p",
        "first",
        "--package=second",
    ])
    .package_selector()
    .map(str::to_owned);

    assert_eq!(package_selector.as_deref(), Some("second"));
}

#[test]
fn extracts_package_spec_selector_from_split_cargo_args() {
    let package_selector = cargo(&["--locked", "-p", "workspace-member@1.2.3"])
        .package_selector()
        .map(str::to_owned);

    assert_eq!(package_selector.as_deref(), Some("workspace-member@1.2.3"));
}

#[test]
fn extracts_last_target_selector_from_cargo_args() {
    let target_selector = cargo(&[
        "--target",
        "aarch64-apple-darwin",
        "--target=x86_64-unknown-linux-gnu",
    ])
    .target_selector()
    .map(str::to_owned);

    assert_eq!(target_selector.as_deref(), Some("x86_64-unknown-linux-gnu"));
}

#[test]
fn strips_package_selectors_from_probe_cargo_args() {
    let cargo_args = cargo(&[
        "+nightly",
        "--package",
        "member-a",
        "-pmember-b",
        "-p",
        "member-c@1.2.3",
        "--features",
        "demo",
        "--package=member-d",
        "--release",
    ])
    .command_arguments_without_package_selector();

    assert_eq!(
        cargo_args,
        vec![
            "+nightly".to_string(),
            "--features".to_string(),
            "demo".to_string(),
            "--release".to_string(),
        ]
    );
}

#[test]
fn strips_manifest_path_from_probe_cargo_args() {
    let cargo_args = cargo(&[
        "--locked",
        "--manifest-path",
        "workspace/Cargo.toml",
        "--manifest-path=member/Cargo.toml",
        "--frozen",
    ])
    .command_arguments_without_manifest_path_selector();

    assert_eq!(
        cargo_args,
        vec!["--locked".to_string(), "--frozen".to_string()]
    );
}

#[test]
fn strips_target_selectors_from_probe_cargo_args() {
    let cargo_args = cargo(&[
        "+nightly",
        "--target",
        "aarch64-apple-darwin",
        "--features",
        "demo",
        "--target=x86_64-unknown-linux-gnu",
        "--release",
    ])
    .command_arguments_without_target_selector();

    assert_eq!(
        cargo_args,
        vec![
            "+nightly".to_string(),
            "--features".to_string(),
            "demo".to_string(),
            "--release".to_string(),
        ]
    );
}

#[test]
fn builds_target_specific_java_env_keys() {
    assert_eq!(
        target_specific_java_home_env_key("x86_64-unknown-linux-gnu"),
        "BOLTFFI_JAVA_HOME_X86_64_UNKNOWN_LINUX_GNU"
    );
    assert_eq!(
        target_specific_java_include_env_key("x86_64-unknown-linux-gnu"),
        "BOLTFFI_JAVA_INCLUDE_X86_64_UNKNOWN_LINUX_GNU"
    );
}

#[test]
fn falls_back_to_current_manifest_package_for_effective_package_selector() {
    let config = Config {
        experimental: Vec::new(),
        cargo: CargoConfig::default(),
        package: PackageConfig {
            name: "workspace-member".to_string(),
            crate_name: None,
            version: None,
            description: None,
            license: None,
            repository: None,
        },
        targets: TargetsConfig::default(),
    };

    let metadata = CargoMetadata {
        target_directory: PathBuf::from("/tmp/boltffi-target"),
        packages: vec![CargoMetadataPackage {
            id: "path+file:///tmp/workspace/member#0.1.0".to_string(),
            name: "workspace-member".to_string(),
            manifest_path: PathBuf::from("/tmp/workspace/Cargo.toml"),
            targets: vec![CargoMetadataPackageTarget {
                name: "workspace_member".to_string(),
                crate_types: vec!["cdylib".into()],
            }],
        }],
    };
    let package_selector = cargo(&[]).effective_package_selector(
        &config,
        &metadata,
        Path::new("/tmp/workspace/Cargo.toml"),
    );

    assert_eq!(package_selector.as_deref(), Some("workspace-member"));
}

#[test]
fn falls_back_to_cargo_package_name_when_crate_name_differs() {
    let config = Config {
        experimental: Vec::new(),
        cargo: CargoConfig::default(),
        package: PackageConfig {
            name: "workspace-member".to_string(),
            crate_name: Some("ffi_member".to_string()),
            version: None,
            description: None,
            license: None,
            repository: None,
        },
        targets: TargetsConfig::default(),
    };

    let metadata = CargoMetadata {
        target_directory: PathBuf::from("/tmp/boltffi-target"),
        packages: vec![CargoMetadataPackage {
            id: "path+file:///tmp/workspace/member#0.1.0".to_string(),
            name: "workspace-member".to_string(),
            manifest_path: PathBuf::from("/tmp/workspace/Cargo.toml"),
            targets: vec![CargoMetadataPackageTarget {
                name: "ffi_member".to_string(),
                crate_types: vec!["cdylib".into()],
            }],
        }],
    };
    let package_selector = cargo(&[]).effective_package_selector(
        &config,
        &metadata,
        Path::new("/tmp/workspace/Cargo.toml"),
    );

    assert_eq!(package_selector.as_deref(), Some("workspace-member"));
}

#[test]
fn returns_none_for_effective_package_selector_when_manifest_path_selects_package() {
    let config = Config {
        experimental: Vec::new(),
        cargo: CargoConfig::default(),
        package: PackageConfig {
            name: "workspace-member".to_string(),
            crate_name: None,
            version: None,
            description: None,
            license: None,
            repository: None,
        },
        targets: TargetsConfig::default(),
    };
    let metadata = CargoMetadata {
        target_directory: PathBuf::from("/tmp/boltffi-target"),
        packages: vec![CargoMetadataPackage {
            id: "path+file:///tmp/workspace/member#0.1.0".to_string(),
            name: "workspace-member".to_string(),
            manifest_path: PathBuf::from("/tmp/workspace/member/Cargo.toml"),
            targets: vec![],
        }],
    };

    let package_selector = cargo(&["--manifest-path", "member/Cargo.toml"])
        .effective_package_selector(
            &config,
            &metadata,
            Path::new("/tmp/workspace/member/Cargo.toml"),
        );

    assert_eq!(package_selector, None);
}

#[test]
fn falls_back_to_package_name_for_virtual_workspace_manifest_path() {
    let config = Config {
        experimental: Vec::new(),
        cargo: CargoConfig::default(),
        package: PackageConfig {
            name: "workspace-member".to_string(),
            crate_name: None,
            version: None,
            description: None,
            license: None,
            repository: None,
        },
        targets: TargetsConfig::default(),
    };
    let metadata = CargoMetadata {
        target_directory: PathBuf::from("/tmp/boltffi-target"),
        packages: vec![CargoMetadataPackage {
            id: "path+file:///tmp/workspace/member#0.1.0".to_string(),
            name: "workspace-member".to_string(),
            manifest_path: PathBuf::from("/tmp/workspace/member/Cargo.toml"),
            targets: vec![CargoMetadataPackageTarget {
                name: "workspace_member".to_string(),
                crate_types: vec!["cdylib".into()],
            }],
        }],
    };

    let package_selector = cargo(&["--manifest-path", "/tmp/workspace/Cargo.toml"])
        .effective_package_selector(&config, &metadata, Path::new("/tmp/workspace/Cargo.toml"));

    assert_eq!(package_selector.as_deref(), Some("workspace-member"));
}

#[test]
fn falls_back_to_package_name_when_crate_name_differs_for_virtual_workspace_manifest_path() {
    let config = Config {
        experimental: Vec::new(),
        cargo: CargoConfig::default(),
        package: PackageConfig {
            name: "workspace-member".to_string(),
            crate_name: Some("ffi_member".to_string()),
            version: None,
            description: None,
            license: None,
            repository: None,
        },
        targets: TargetsConfig::default(),
    };
    let metadata = CargoMetadata {
        target_directory: PathBuf::from("/tmp/boltffi-target"),
        packages: vec![CargoMetadataPackage {
            id: "path+file:///tmp/workspace/member#0.1.0".to_string(),
            name: "workspace-member".to_string(),
            manifest_path: PathBuf::from("/tmp/workspace/member/Cargo.toml"),
            targets: vec![CargoMetadataPackageTarget {
                name: "ffi_member".to_string(),
                crate_types: vec!["cdylib".into()],
            }],
        }],
    };

    let package_selector = cargo(&["--manifest-path", "/tmp/workspace/Cargo.toml"])
        .effective_package_selector(&config, &metadata, Path::new("/tmp/workspace/Cargo.toml"));

    assert_eq!(package_selector.as_deref(), Some("workspace-member"));
}

#[test]
fn prefers_explicit_package_selector_over_config_package_name() {
    let config = Config {
        experimental: Vec::new(),
        cargo: CargoConfig::default(),
        package: PackageConfig {
            name: "workspace-member".to_string(),
            crate_name: None,
            version: None,
            description: None,
            license: None,
            repository: None,
        },
        targets: TargetsConfig::default(),
    };
    let metadata = CargoMetadata {
        target_directory: PathBuf::from("/tmp/boltffi-target"),
        packages: vec![],
    };

    let package_selector = cargo(&["--package=selected-member"]).effective_package_selector(
        &config,
        &metadata,
        Path::new("/tmp/workspace/Cargo.toml"),
    );

    assert_eq!(package_selector.as_deref(), Some("selected-member"));
}

#[test]
fn prefers_configured_package_name_over_unique_library_target_match() {
    let config = Config {
        experimental: Vec::new(),
        cargo: CargoConfig::default(),
        package: PackageConfig {
            name: "workspace-member".to_string(),
            crate_name: Some("ffi_member".to_string()),
            version: None,
            description: None,
            license: None,
            repository: None,
        },
        targets: TargetsConfig::default(),
    };
    let metadata = CargoMetadata {
        target_directory: PathBuf::from("/tmp/boltffi-target"),
        packages: vec![
            CargoMetadataPackage {
                id: "path+file:///tmp/workspace/other#0.1.0".to_string(),
                name: "other-member".to_string(),
                manifest_path: PathBuf::from("/tmp/workspace/other/Cargo.toml"),
                targets: vec![CargoMetadataPackageTarget {
                    name: "ffi_member".to_string(),
                    crate_types: vec!["cdylib".into()],
                }],
            },
            CargoMetadataPackage {
                id: "path+file:///tmp/workspace/member#0.1.0".to_string(),
                name: "workspace-member".to_string(),
                manifest_path: PathBuf::from("/tmp/workspace/member/Cargo.toml"),
                targets: vec![CargoMetadataPackageTarget {
                    name: "workspace_member_lib".to_string(),
                    crate_types: vec!["cdylib".into()],
                }],
            },
        ],
    };

    let package_selector = cargo(&["--manifest-path", "/tmp/workspace/Cargo.toml"])
        .effective_package_selector(&config, &metadata, Path::new("/tmp/workspace/Cargo.toml"));

    assert_eq!(package_selector.as_deref(), Some("workspace-member"));
}

#[test]
fn pack_java_no_longer_requires_experimental_gate() {
    let config = Config {
        experimental: Vec::new(),
        cargo: CargoConfig::default(),
        package: PackageConfig {
            name: "workspace-member".to_string(),
            crate_name: None,
            version: None,
            description: None,
            license: None,
            repository: None,
        },
        targets: TargetsConfig {
            java: crate::config::JavaConfig {
                jvm: crate::config::JavaJvmConfig {
                    enabled: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        },
    };

    ensure_java_no_build_supported(&config, false, false, "pack java")
        .expect("expected pack java to proceed without experimental gate");
}

#[test]
fn resolves_selected_jvm_package_source_directory_from_selected_package_manifest() {
    let current_host = JavaHostTarget::current().expect("current host");
    let metadata = CargoMetadata {
        target_directory: PathBuf::from("/tmp/boltffi-target"),
        packages: vec![CargoMetadataPackage {
            id: "path+file:///tmp/workspace/member#0.1.0".to_string(),
            name: "workspace-member".to_string(),
            manifest_path: PathBuf::from("/tmp/workspace/member/Cargo.toml"),
            targets: vec![CargoMetadataPackageTarget {
                name: "workspace_member".to_string(),
                crate_types: vec!["staticlib".into(), "cdylib".into()],
            }],
        }],
    };
    let package = metadata
        .find_package(
            Path::new("/tmp/workspace/Cargo.toml"),
            Some("workspace-member"),
        )
        .expect("selected package");
    let packaging_targets = vec![JvmPackagingTarget {
        cargo_context: JvmCargoContext {
            host_target: current_host,
            rust_target_triple: "x86_64-unknown-linux-gnu".to_string(),
            release: false,
            build_profile: CargoBuildProfile::Debug,
            artifact_name: "workspace_member".to_string(),
            cargo_manifest_path: PathBuf::from("/tmp/workspace/Cargo.toml"),
            manifest_path: package.manifest_path.clone(),
            package_selector: Some("workspace-member".to_string()),
            target_directory: metadata.target_directory.clone(),
            cargo_command_args: Vec::new(),
            toolchain_selector: None,
            crate_outputs: JvmCrateOutputs {
                builds_staticlib: true,
                builds_cdylib: true,
            },
        },
        toolchain: NativeHostToolchain::discover(None, &[], current_host, current_host)
            .expect("desktop toolchain"),
    }];

    let source_directory =
        selected_jvm_package_source_directory(&packaging_targets).expect("source directory");

    assert_eq!(source_directory, PathBuf::from("/tmp/workspace/member"));
}

#[test]
fn remove_file_if_exists_deletes_existing_file() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!("boltffi-remove-file-test-{unique}"));
    fs::create_dir_all(&temp_root).expect("create temp dir");
    let file_path = temp_root.join("stale.dylib");
    fs::write(&file_path, []).expect("write temp file");

    remove_file_if_exists(&file_path).expect("remove stale file");

    assert!(!file_path.exists());

    fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
}

#[test]
fn removes_stale_requested_shared_library_copies_only_after_success() {
    let current_host = JavaHostTarget::current();
    let requested_host = current_host.unwrap_or(JavaHostTarget::DarwinArm64);
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let temp_root =
        std::env::temp_dir().join(format!("boltffi-java-requested-shared-cleanup-{unique}"));
    let native_output = temp_root
        .join("native")
        .join(requested_host.canonical_name());
    fs::create_dir_all(&native_output).expect("create structured output dir");

    let structured_shared = native_output.join(requested_host.shared_library_filename("demo"));
    fs::write(&structured_shared, []).expect("write structured shared copy");

    let flat_shared = temp_root.join(requested_host.shared_library_filename("demo"));
    if current_host == Some(requested_host) {
        fs::write(&flat_shared, []).expect("write flat shared copy");
    }

    remove_stale_requested_jvm_shared_library_copies_after_success(
        &temp_root,
        &[JvmPackagedNativeOutput {
            host_target: requested_host,
            has_shared_library_copy: false,
        }],
        "demo",
    )
    .expect("cleanup stale requested shared copies");

    assert!(!structured_shared.exists());
    if current_host == Some(requested_host) {
        assert!(!flat_shared.exists());
    }

    fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
}

#[test]
fn removes_stale_flat_jvm_outputs_when_current_host_is_not_requested() {
    let current_host = JavaHostTarget::current().expect("supported test host");
    let requested_other_host = [
        JavaHostTarget::DarwinArm64,
        JavaHostTarget::DarwinX86_64,
        JavaHostTarget::LinuxX86_64,
        JavaHostTarget::LinuxAarch64,
        JavaHostTarget::WindowsX86_64,
    ]
    .into_iter()
    .find(|target| *target != current_host)
    .expect("alternate host");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!("boltffi-java-flat-cleanup-{unique}"));
    fs::create_dir_all(&temp_root).expect("create temp dir");

    let jni_copy = temp_root.join(current_host.jni_library_filename("demo"));
    let shared_copy = temp_root.join(current_host.shared_library_filename("demo"));
    fs::write(&jni_copy, []).expect("write stale jni");
    fs::write(&shared_copy, []).expect("write stale shared");

    remove_stale_flat_jvm_outputs_if_current_host_unrequested(
        &temp_root,
        Some(current_host),
        &[requested_other_host],
        "demo",
    )
    .expect("cleanup stale outputs");

    assert!(!jni_copy.exists());
    assert!(!shared_copy.exists());

    fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
}

#[test]
fn removes_stale_structured_jvm_outputs_when_host_matrix_is_narrowed() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!("boltffi-java-structured-cleanup-{unique}"));
    let darwin_dir = temp_root.join(JavaHostTarget::DarwinArm64.canonical_name());
    let linux_dir = temp_root.join(JavaHostTarget::LinuxX86_64.canonical_name());
    fs::create_dir_all(&darwin_dir).expect("create darwin dir");
    fs::create_dir_all(&linux_dir).expect("create linux dir");

    remove_stale_structured_jvm_outputs(&temp_root, &[JavaHostTarget::DarwinArm64])
        .expect("cleanup stale structured outputs");

    assert!(darwin_dir.exists());
    assert!(!linux_dir.exists());

    fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
}

#[test]
fn preserves_requested_structured_jvm_outputs() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!("boltffi-java-structured-preserve-{unique}"));
    let darwin_dir = temp_root.join(JavaHostTarget::DarwinArm64.canonical_name());
    let linux_dir = temp_root.join(JavaHostTarget::LinuxX86_64.canonical_name());
    fs::create_dir_all(&darwin_dir).expect("create darwin dir");
    fs::create_dir_all(&linux_dir).expect("create linux dir");

    remove_stale_structured_jvm_outputs(
        &temp_root,
        &[JavaHostTarget::DarwinArm64, JavaHostTarget::LinuxX86_64],
    )
    .expect("preserve structured outputs");

    assert!(darwin_dir.exists());
    assert!(linux_dir.exists());

    fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
}

#[test]
fn preserves_flat_jvm_outputs_when_current_host_is_requested() {
    let current_host = JavaHostTarget::current().expect("supported test host");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!("boltffi-java-flat-preserve-{unique}"));
    fs::create_dir_all(&temp_root).expect("create temp dir");

    let jni_copy = temp_root.join(current_host.jni_library_filename("demo"));
    let shared_copy = temp_root.join(current_host.shared_library_filename("demo"));
    fs::write(&jni_copy, []).expect("write current jni");
    fs::write(&shared_copy, []).expect("write current shared");

    remove_stale_flat_jvm_outputs_if_current_host_unrequested(
        &temp_root,
        Some(current_host),
        &[current_host],
        "demo",
    )
    .expect("preserve current-host outputs");

    assert!(jni_copy.exists());
    assert!(shared_copy.exists());

    fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
}

#[test]
fn rejects_missing_cross_host_jni_headers_during_validation() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!("boltffi-java-headers-test-{unique}"));
    let java_home = temp_root.join("linux-jdk");
    let shared_include = java_home.join("include");
    fs::create_dir_all(&shared_include).expect("create shared include dir");
    fs::write(shared_include.join("jni.h"), []).expect("write jni.h");

    let cargo_context = JvmCargoContext {
        host_target: JavaHostTarget::LinuxX86_64,
        rust_target_triple: "x86_64-unknown-linux-gnu".to_string(),
        release: false,
        build_profile: CargoBuildProfile::Debug,
        artifact_name: "demo".to_string(),
        cargo_manifest_path: temp_root.join("Cargo.toml"),
        manifest_path: temp_root.join("Cargo.toml"),
        package_selector: None,
        target_directory: temp_root.join("target"),
        cargo_command_args: Vec::new(),
        toolchain_selector: None,
        crate_outputs: JvmCrateOutputs {
            builds_staticlib: true,
            builds_cdylib: false,
        },
    };

    let error =
        resolve_jni_include_directories_with_overrides(&cargo_context, Some(java_home), None, None)
            .expect_err("expected missing target headers error");

    assert!(matches!(
        error,
        CliError::CommandFailed { command, status: None }
            if command.contains("BOLTFFI_JAVA_INCLUDE_X86_64_UNKNOWN_LINUX_GNU")
                && command.contains("BOLTFFI_JAVA_HOME_X86_64_UNKNOWN_LINUX_GNU")
    ));

    fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
}

#[test]
fn rejects_missing_jni_header_files_during_validation() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!("boltffi-java-header-files-test-{unique}"));
    let shared_include = temp_root.join("include");
    let platform_include = shared_include.join("linux");
    fs::create_dir_all(&platform_include).expect("create include dirs");

    let cargo_context = JvmCargoContext {
        host_target: JavaHostTarget::LinuxX86_64,
        rust_target_triple: "x86_64-unknown-linux-gnu".to_string(),
        release: false,
        build_profile: CargoBuildProfile::Debug,
        artifact_name: "demo".to_string(),
        cargo_manifest_path: temp_root.join("Cargo.toml"),
        manifest_path: temp_root.join("Cargo.toml"),
        package_selector: None,
        target_directory: temp_root.join("target"),
        cargo_command_args: Vec::new(),
        toolchain_selector: None,
        crate_outputs: JvmCrateOutputs {
            builds_staticlib: true,
            builds_cdylib: false,
        },
    };

    let error = resolve_jni_include_directories_with_overrides(
        &cargo_context,
        None,
        None,
        Some(platform_include),
    )
    .expect_err("expected missing header file error");

    assert!(matches!(error, CliError::FileNotFound(path) if path.ends_with("jni.h")));

    fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
}

#[test]
fn accepts_target_include_override_without_java_home() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!("boltffi-java-include-only-test-{unique}"));
    let shared_include = temp_root.join("include");
    let platform_include = shared_include.join("linux");
    fs::create_dir_all(&platform_include).expect("create platform include dir");
    fs::write(shared_include.join("jni.h"), []).expect("write jni.h");
    fs::write(platform_include.join("jni_md.h"), []).expect("write jni_md.h");

    let cargo_context = JvmCargoContext {
        host_target: JavaHostTarget::LinuxX86_64,
        rust_target_triple: "x86_64-unknown-linux-gnu".to_string(),
        release: false,
        build_profile: CargoBuildProfile::Debug,
        artifact_name: "demo".to_string(),
        cargo_manifest_path: temp_root.join("Cargo.toml"),
        manifest_path: temp_root.join("Cargo.toml"),
        package_selector: None,
        target_directory: temp_root.join("target"),
        cargo_command_args: Vec::new(),
        toolchain_selector: None,
        crate_outputs: JvmCrateOutputs {
            builds_staticlib: true,
            builds_cdylib: false,
        },
    };

    let include_directories = resolve_jni_include_directories_with_overrides(
        &cargo_context,
        None,
        None,
        Some(platform_include.clone()),
    )
    .expect("include override should be sufficient");

    assert_eq!(include_directories.shared, shared_include);
    assert_eq!(include_directories.platform, platform_include);

    fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
}

#[test]
fn prefers_target_include_override_over_java_home_include() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let temp_root =
        std::env::temp_dir().join(format!("boltffi-java-include-priority-test-{unique}"));
    let host_java_home = temp_root.join("host-jdk");
    let target_include_root = temp_root.join("target-jdk").join("include");
    let target_platform_include = target_include_root.join("linux");
    fs::create_dir_all(host_java_home.join("include").join("darwin"))
        .expect("create host include dir");
    fs::create_dir_all(&target_platform_include).expect("create target include dir");
    fs::write(host_java_home.join("include").join("jni.h"), []).expect("write host jni.h");
    fs::write(target_include_root.join("jni.h"), []).expect("write target jni.h");
    fs::write(target_platform_include.join("jni_md.h"), []).expect("write target jni_md.h");

    let cargo_context = JvmCargoContext {
        host_target: JavaHostTarget::LinuxX86_64,
        rust_target_triple: "x86_64-unknown-linux-gnu".to_string(),
        release: false,
        build_profile: CargoBuildProfile::Debug,
        artifact_name: "demo".to_string(),
        cargo_manifest_path: temp_root.join("Cargo.toml"),
        manifest_path: temp_root.join("Cargo.toml"),
        package_selector: None,
        target_directory: temp_root.join("target"),
        cargo_command_args: Vec::new(),
        toolchain_selector: None,
        crate_outputs: JvmCrateOutputs {
            builds_staticlib: true,
            builds_cdylib: false,
        },
    };

    let include_directories = resolve_jni_include_directories_with_overrides(
        &cargo_context,
        Some(host_java_home),
        None,
        Some(target_platform_include.clone()),
    )
    .expect("target include override should take precedence");

    assert_eq!(include_directories.shared, target_include_root);
    assert_eq!(include_directories.platform, target_platform_include);

    fs::remove_dir_all(&temp_root).expect("cleanup temp dir");
}

#[test]
fn prefers_staticlib_for_jvm_linking_when_available() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!("boltffi-jvm-link-test-{unique}"));
    let profile_dir = temp_root.join("release");
    fs::create_dir_all(&profile_dir).expect("create profile dir");

    let staticlib = profile_dir.join("libdemo.a");
    let cdylib = profile_dir.join("libdemo.dylib");
    fs::write(&staticlib, []).expect("write staticlib");
    fs::write(&cdylib, []).expect("write cdylib");

    let resolved = resolve_jvm_native_link_input(
        &profile_dir,
        JavaHostTarget::DarwinArm64,
        "demo",
        JvmCrateOutputs {
            builds_staticlib: true,
            builds_cdylib: true,
        },
        Some("libdemo.a"),
    )
    .expect("expected link input");

    assert_eq!(resolved.path(), staticlib.as_path());

    fs::remove_dir_all(&temp_root).expect("cleanup temp target dir");
}

#[test]
fn skips_shared_library_compatibility_copy_when_jni_links_staticlib() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!("boltffi-jvm-copy-test-{unique}"));
    let profile_dir = temp_root.join("release");
    fs::create_dir_all(&profile_dir).expect("create profile dir");

    let staticlib = profile_dir.join("libdemo.a");
    let cdylib = profile_dir.join("libdemo.dylib");
    fs::write(&staticlib, []).expect("write staticlib");
    fs::write(&cdylib, []).expect("write cdylib");

    let resolved = resolve_jvm_native_link_input(
        &profile_dir,
        JavaHostTarget::DarwinArm64,
        "demo",
        JvmCrateOutputs {
            builds_staticlib: true,
            builds_cdylib: true,
        },
        Some("libdemo.a"),
    )
    .expect("expected link input");
    let compatibility_shared_library = bundled_jvm_shared_library_path(
        &resolved,
        &profile_dir,
        JavaHostTarget::DarwinArm64,
        "demo",
        JvmCrateOutputs {
            builds_staticlib: true,
            builds_cdylib: true,
        },
    );

    assert_eq!(resolved.path(), staticlib.as_path());
    assert!(compatibility_shared_library.is_none());

    fs::remove_dir_all(&temp_root).expect("cleanup temp target dir");
}

#[test]
fn keeps_shared_library_compatibility_copy_when_jni_links_cdylib() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!("boltffi-jvm-copy-cdylib-test-{unique}"));
    let profile_dir = temp_root.join("release");
    fs::create_dir_all(&profile_dir).expect("create profile dir");

    let cdylib = profile_dir.join("libdemo.dylib");
    fs::write(&cdylib, []).expect("write cdylib");

    let resolved = resolve_jvm_native_link_input(
        &profile_dir,
        JavaHostTarget::DarwinArm64,
        "demo",
        JvmCrateOutputs {
            builds_staticlib: false,
            builds_cdylib: true,
        },
        None,
    )
    .expect("expected link input");
    let compatibility_shared_library = bundled_jvm_shared_library_path(
        &resolved,
        &profile_dir,
        JavaHostTarget::DarwinArm64,
        "demo",
        JvmCrateOutputs {
            builds_staticlib: false,
            builds_cdylib: true,
        },
    )
    .expect("expected shared library compatibility copy");

    assert_eq!(resolved.path(), cdylib.as_path());
    assert_eq!(compatibility_shared_library, cdylib);

    fs::remove_dir_all(&temp_root).expect("cleanup temp target dir");
}

#[test]
fn ignores_stale_staticlib_when_current_crate_is_cdylib_only() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!("boltffi-jvm-stale-static-{unique}"));
    let profile_dir = temp_root.join("release");
    fs::create_dir_all(&profile_dir).expect("create profile dir");

    let staticlib = profile_dir.join("libdemo.a");
    let cdylib = profile_dir.join("libdemo.dylib");
    fs::write(&staticlib, []).expect("write stale staticlib");
    fs::write(&cdylib, []).expect("write current cdylib");

    let resolved = resolve_jvm_native_link_input(
        &profile_dir,
        JavaHostTarget::DarwinArm64,
        "demo",
        JvmCrateOutputs {
            builds_staticlib: false,
            builds_cdylib: true,
        },
        None,
    )
    .expect("expected link input");

    assert_eq!(resolved.path(), cdylib.as_path());

    fs::remove_dir_all(&temp_root).expect("cleanup temp target dir");
}

#[test]
fn ignores_stale_shared_library_when_current_crate_is_staticlib_only() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!("boltffi-jvm-stale-cdylib-{unique}"));
    let profile_dir = temp_root.join("release");
    fs::create_dir_all(&profile_dir).expect("create profile dir");

    let cdylib = profile_dir.join("libdemo.dylib");
    fs::write(&cdylib, []).expect("write stale shared library");

    let compatibility_shared_library = existing_jvm_shared_library_path(
        &profile_dir,
        JavaHostTarget::DarwinArm64,
        "demo",
        JvmCrateOutputs {
            builds_staticlib: true,
            builds_cdylib: false,
        },
    );

    assert!(compatibility_shared_library.is_none());

    fs::remove_dir_all(&temp_root).expect("cleanup temp target dir");
}

#[test]
fn parses_current_jvm_crate_outputs_from_cargo_metadata() {
    let metadata = CargoMetadata {
        target_directory: PathBuf::from("/tmp/boltffi-target"),
        packages: vec![
            CargoMetadataPackage {
                id: "path+file:///tmp/workspace/sibling#0.1.0".to_string(),
                name: "sibling".to_string(),
                manifest_path: PathBuf::from("/tmp/workspace/sibling/Cargo.toml"),
                targets: vec![CargoMetadataPackageTarget {
                    name: "demo".to_string(),
                    crate_types: vec!["cdylib".into()],
                }],
            },
            CargoMetadataPackage {
                id: "path+file:///tmp/workspace/current#0.1.0".to_string(),
                name: "current".to_string(),
                manifest_path: PathBuf::from("/tmp/workspace/current/Cargo.toml"),
                targets: vec![
                    CargoMetadataPackageTarget {
                        name: "demo".to_string(),
                        crate_types: vec!["staticlib".into(), "cdylib".into(), "rlib".into()],
                    },
                    CargoMetadataPackageTarget {
                        name: "demo_cli".to_string(),
                        crate_types: vec!["bin".into()],
                    },
                ],
            },
        ],
    };

    let outputs = JvmCrateOutputs::from_metadata(
        &metadata,
        "demo",
        Path::new("/tmp/workspace/current/Cargo.toml"),
        None,
    )
    .expect("crate outputs");

    assert_eq!(
        outputs,
        JvmCrateOutputs {
            builds_staticlib: true,
            builds_cdylib: true,
        }
    );
}

#[test]
fn scopes_jvm_crate_outputs_to_selected_package_manifest() {
    let metadata = CargoMetadata {
        target_directory: PathBuf::from("/tmp/boltffi-target"),
        packages: vec![
            CargoMetadataPackage {
                id: "path+file:///tmp/workspace/a#0.1.0".to_string(),
                name: "workspace-a".to_string(),
                manifest_path: PathBuf::from("/tmp/workspace/a/Cargo.toml"),
                targets: vec![CargoMetadataPackageTarget {
                    name: "shared_name".to_string(),
                    crate_types: vec!["cdylib".into()],
                }],
            },
            CargoMetadataPackage {
                id: "path+file:///tmp/workspace/b#0.1.0".to_string(),
                name: "workspace-b".to_string(),
                manifest_path: PathBuf::from("/tmp/workspace/b/Cargo.toml"),
                targets: vec![CargoMetadataPackageTarget {
                    name: "shared_name".to_string(),
                    crate_types: vec!["staticlib".into()],
                }],
            },
        ],
    };

    let outputs = JvmCrateOutputs::from_metadata(
        &metadata,
        "shared_name",
        Path::new("/tmp/workspace/b/Cargo.toml"),
        None,
    )
    .expect("crate outputs");

    assert_eq!(
        outputs,
        JvmCrateOutputs {
            builds_staticlib: true,
            builds_cdylib: false,
        }
    );
}

#[test]
fn finds_current_cargo_metadata_package_by_manifest_path() {
    let metadata = CargoMetadata {
        target_directory: PathBuf::from("/tmp/boltffi-target"),
        packages: vec![
            CargoMetadataPackage {
                id: "path+file:///tmp/workspace/a#0.1.0".to_string(),
                name: "workspace-a".to_string(),
                manifest_path: PathBuf::from("/tmp/workspace/a/Cargo.toml"),
                targets: vec![],
            },
            CargoMetadataPackage {
                id: "path+file:///tmp/workspace/b#0.1.0".to_string(),
                name: "workspace-b".to_string(),
                manifest_path: PathBuf::from("/tmp/workspace/b/Cargo.toml"),
                targets: vec![],
            },
        ],
    };

    let package = metadata
        .find_package(Path::new("/tmp/workspace/b/Cargo.toml"), None)
        .expect("package lookup");

    assert_eq!(package.id, "path+file:///tmp/workspace/b#0.1.0");
}

#[test]
fn finds_selected_cargo_metadata_package_by_package_name() {
    let metadata = CargoMetadata {
        target_directory: PathBuf::from("/tmp/boltffi-target"),
        packages: vec![
            CargoMetadataPackage {
                id: "path+file:///tmp/workspace#workspace-a@0.1.0".to_string(),
                name: "workspace-a".to_string(),
                manifest_path: PathBuf::from("/tmp/workspace/Cargo.toml"),
                targets: vec![],
            },
            CargoMetadataPackage {
                id: "path+file:///tmp/workspace#workspace-b@0.1.0".to_string(),
                name: "workspace-b".to_string(),
                manifest_path: PathBuf::from("/tmp/workspace/Cargo.toml"),
                targets: vec![],
            },
        ],
    };

    let package = metadata
        .find_package(Path::new("/tmp/workspace/Cargo.toml"), Some("workspace-b"))
        .expect("package lookup");

    assert_eq!(package.id, "path+file:///tmp/workspace#workspace-b@0.1.0");
}

#[test]
fn finds_selected_cargo_metadata_package_by_package_spec() {
    let metadata = CargoMetadata {
        target_directory: PathBuf::from("/tmp/boltffi-target"),
        packages: vec![
            CargoMetadataPackage {
                id: "path+file:///tmp/workspace#workspace-a@0.1.0".to_string(),
                name: "workspace-a".to_string(),
                manifest_path: PathBuf::from("/tmp/workspace/Cargo.toml"),
                targets: vec![],
            },
            CargoMetadataPackage {
                id: "path+file:///tmp/workspace#workspace-b@1.2.3".to_string(),
                name: "workspace-b".to_string(),
                manifest_path: PathBuf::from("/tmp/workspace/Cargo.toml"),
                targets: vec![],
            },
        ],
    };

    let package = metadata
        .find_package(
            Path::new("/tmp/workspace/Cargo.toml"),
            Some("workspace-b@1.2.3"),
        )
        .expect("package lookup");

    assert_eq!(package.id, "path+file:///tmp/workspace#workspace-b@1.2.3");
}

#[test]
fn scopes_jvm_crate_outputs_to_selected_package_name() {
    let metadata = CargoMetadata {
        target_directory: PathBuf::from("/tmp/boltffi-target"),
        packages: vec![
            CargoMetadataPackage {
                id: "path+file:///tmp/workspace#workspace-a@0.1.0".to_string(),
                name: "workspace-a".to_string(),
                manifest_path: PathBuf::from("/tmp/workspace/Cargo.toml"),
                targets: vec![CargoMetadataPackageTarget {
                    name: "shared_name".to_string(),
                    crate_types: vec!["cdylib".into()],
                }],
            },
            CargoMetadataPackage {
                id: "path+file:///tmp/workspace#workspace-b@0.1.0".to_string(),
                name: "workspace-b".to_string(),
                manifest_path: PathBuf::from("/tmp/workspace/Cargo.toml"),
                targets: vec![CargoMetadataPackageTarget {
                    name: "shared_name".to_string(),
                    crate_types: vec!["staticlib".into()],
                }],
            },
        ],
    };

    let outputs = JvmCrateOutputs::from_metadata(
        &metadata,
        "shared_name",
        Path::new("/tmp/workspace/Cargo.toml"),
        Some("workspace-b"),
    )
    .expect("crate outputs");

    assert_eq!(
        outputs,
        JvmCrateOutputs {
            builds_staticlib: true,
            builds_cdylib: false,
        }
    );
}

#[test]
fn falls_back_to_selected_package_ffi_target_when_preferred_artifact_name_differs() {
    let metadata = CargoMetadata {
        target_directory: PathBuf::from("/tmp/boltffi-target"),
        packages: vec![CargoMetadataPackage {
            id: "path+file:///tmp/workspace/member#0.1.0".to_string(),
            name: "workspace-member".to_string(),
            manifest_path: PathBuf::from("/tmp/workspace/member/Cargo.toml"),
            targets: vec![
                CargoMetadataPackageTarget {
                    name: "workspace_member_lib".to_string(),
                    crate_types: vec!["staticlib".into(), "cdylib".into()],
                },
                CargoMetadataPackageTarget {
                    name: "workspace_member_cli".to_string(),
                    crate_types: vec!["bin".into()],
                },
            ],
        }],
    };

    let outputs = JvmCrateOutputs::from_metadata(
        &metadata,
        "root_config_name",
        Path::new("/tmp/workspace/Cargo.toml"),
        Some("workspace-member"),
    )
    .expect("crate outputs");

    assert_eq!(
        outputs,
        JvmCrateOutputs {
            builds_staticlib: true,
            builds_cdylib: true,
        }
    );
}
