use std::path::PathBuf;

use crate::check::EnvironmentCheck;
use crate::config::Config;
use crate::error::Result;
use crate::target::RustTarget;

pub struct DoctorOptions {
    pub apple: bool,
    pub apple_targets: Vec<RustTarget>,
    pub android: bool,
    pub android_targets: Vec<RustTarget>,
    pub wasm: bool,
}

pub fn run_doctor(options: DoctorOptions) -> Result<()> {
    let required_targets = required_targets(&options);
    let check = EnvironmentCheck::run(&required_targets);

    println!("boltffi doctor");
    println!();
    print_environment(&check, &options);
    println!();
    print_config_summary();

    Ok(())
}

fn required_targets(options: &DoctorOptions) -> Vec<RustTarget> {
    let apple_targets = options
        .apple
        .then(|| options.apple_targets.iter().copied())
        .into_iter()
        .flatten();

    let android_targets = options
        .android
        .then(|| options.android_targets.iter().copied())
        .into_iter()
        .flatten();

    let wasm_targets = options
        .wasm
        .then(|| RustTarget::ALL_WASM.iter().cloned())
        .into_iter()
        .flatten();

    apple_targets
        .chain(android_targets)
        .chain(wasm_targets)
        .collect()
}

fn print_environment(check: &EnvironmentCheck, options: &DoctorOptions) {
    match &check.rust_version {
        Some(version) => println!("Rust: {}", version),
        None => println!("Rust: missing"),
    }

    println!("Installed targets: {}", check.installed_targets.len());
    println!("Missing targets: {}", check.missing_targets.len());
    check
        .missing_targets
        .iter()
        .for_each(|triple| println!("  - {}", triple));

    println!();
    println!("Apple tooling: {}", readiness(check.is_ready_for_apple()));
    if options.apple {
        options.apple_targets.iter().for_each(|target| {
            let installed = check
                .installed_targets
                .iter()
                .any(|triple| triple == target.triple());
            println!("  target {}: {}", target.triple(), readiness(installed));
        });
        println!("  xcode-select: {}", readiness(check.tools.xcode_cli));
        println!("  xcodebuild: {}", readiness(check.tools.xcodebuild));
        println!("  lipo: {}", readiness(check.tools.lipo));
    }

    println!();
    println!(
        "Android tooling: {}",
        readiness(check.is_ready_for_android())
    );
    if options.android {
        options.android_targets.iter().for_each(|target| {
            let installed = check
                .installed_targets
                .iter()
                .any(|triple| triple == target.triple());
            println!("  target {}: {}", target.triple(), readiness(installed));
        });
        match &check.tools.android_ndk {
            Some(path) => println!("  ndk: {}", path),
            None => println!("  ndk: missing (set ANDROID_NDK_HOME)"),
        }
    }

    if options.wasm {
        println!();
        println!(
            "WASM target {}",
            readiness(
                check
                    .installed_targets
                    .iter()
                    .any(|target| target == RustTarget::WASM32_UNKNOWN_UNKNOWN.triple())
            )
        );
    }
}

fn print_config_summary() {
    let config_path = PathBuf::from("boltffi.toml");

    if !config_path.exists() {
        println!("Config: missing (expected ./boltffi.toml)");
        return;
    }

    match Config::load(&config_path) {
        Ok(config) => {
            println!("Config: {}", config_path.display());
            println!("  crate: {}", config.library_name());
            println!(
                "  targets.apple.output: {}",
                config.apple_output().display()
            );
            println!(
                "  targets.apple.include_macos: {}",
                config.apple_include_macos()
            );
            println!(
                "  targets.apple.ios_architectures: {}",
                config
                    .apple_ios_architectures()
                    .iter()
                    .map(|architecture| architecture.canonical_name())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!(
                "  targets.apple.simulator_architectures: {}",
                config
                    .apple_simulator_architectures()
                    .iter()
                    .map(|architecture| architecture.canonical_name())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!(
                "  targets.apple.macos_architectures: {}{}",
                config
                    .apple_macos_architectures()
                    .iter()
                    .map(|architecture| architecture.canonical_name())
                    .collect::<Vec<_>>()
                    .join(", "),
                if config.apple_include_macos() {
                    ""
                } else {
                    " (ignored unless include_macos = true)"
                }
            );
            println!(
                "  targets.apple.swift.output: {}",
                config.apple_swift_output().display()
            );
            println!(
                "  targets.apple.header.output: {}",
                config.apple_header_output().display()
            );
            println!(
                "  targets.apple.xcframework.output: {}",
                config.apple_xcframework_output().display()
            );
            println!(
                "  targets.apple.spm.output: {}",
                config.apple_spm_output().display()
            );
            println!(
                "  targets.android.output: {}",
                config.android_output().display()
            );
            println!(
                "  targets.android.kotlin.output: {}",
                config.android_kotlin_output().display()
            );
            println!(
                "  targets.android.header.output: {}",
                config.android_header_output().display()
            );
            println!(
                "  targets.android.pack.output: {}",
                config.android_pack_output().display()
            );
            println!(
                "  targets.android.architectures: {}",
                config
                    .android_architectures()
                    .iter()
                    .map(|architecture| architecture.canonical_name())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!("  targets.wasm.output: {}", config.wasm_output().display());
            println!(
                "  targets.wasm.typescript.output: {}",
                config.wasm_typescript_output().display()
            );
            println!(
                "  targets.wasm.npm.output: {}",
                config.wasm_npm_output().display()
            );
        }
        Err(error) => {
            println!("Config: {} (invalid: {})", config_path.display(), error);
        }
    }
}

fn readiness(is_ready: bool) -> &'static str {
    if is_ready { "ok" } else { "missing" }
}
