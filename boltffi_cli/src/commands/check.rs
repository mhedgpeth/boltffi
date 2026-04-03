use crate::check::{EnvironmentCheck, install_missing_targets};
use crate::error::Result;
use crate::target::RustTarget;

pub struct CheckOptions {
    pub fix: bool,
    pub apple: bool,
    pub apple_targets: Vec<RustTarget>,
    pub android: bool,
    pub android_targets: Vec<RustTarget>,
    pub wasm: bool,
    pub wasm_target_triple: Option<String>,
}

impl Default for CheckOptions {
    fn default() -> Self {
        Self {
            fix: false,
            apple: true,
            apple_targets: RustTarget::ALL_IOS.to_vec(),
            android: true,
            android_targets: RustTarget::ALL_ANDROID.to_vec(),
            wasm: true,
            wasm_target_triple: Some(RustTarget::WASM32_UNKNOWN_UNKNOWN.triple().to_string()),
        }
    }
}

pub fn run_check(options: CheckOptions) -> Result<bool> {
    let mut required_triples = Vec::new();

    if options.apple {
        required_triples.extend(
            options
                .apple_targets
                .iter()
                .map(|target| target.triple().to_string()),
        );
    }

    if options.android {
        required_triples.extend(
            options
                .android_targets
                .iter()
                .map(|target| target.triple().to_string()),
        );
    }

    if options.wasm {
        required_triples.push(
            options
                .wasm_target_triple
                .clone()
                .unwrap_or_else(|| RustTarget::WASM32_UNKNOWN_UNKNOWN.triple().to_string()),
        );
    }

    let check = EnvironmentCheck::run_with_required_triples(&required_triples);

    print_environment_status(&check, &options);

    if options.fix && check.has_missing_targets() {
        println!();
        println!("Installing missing targets...");
        install_missing_targets(&check.missing_targets)?;
        println!("Done!");
    }

    let all_good = !check.has_missing_targets()
        && (!options.apple || check.is_ready_for_apple())
        && (!options.android || check.is_ready_for_android());

    Ok(all_good)
}

fn print_environment_status(check: &EnvironmentCheck, options: &CheckOptions) {
    println!("Environment");

    match &check.rust_version {
        Some(version) => println!("  {} {}", status_icon(true), version),
        None => println!("  {} Rust not found", status_icon(false)),
    }

    println!();

    if options.apple {
        print_apple_targets(check, &options.apple_targets);
        println!();

        println!("Apple Tools");
        println!("  {} Xcode CLI tools", status_icon(check.tools.xcode_cli));
        println!("  {} lipo", status_icon(check.tools.lipo));
        println!("  {} xcodebuild", status_icon(check.tools.xcodebuild));
        println!();
    }

    if options.android {
        println!("Android Targets");
        options.android_targets.iter().for_each(|target| {
            let installed = check.installed_targets.iter().any(|t| t == target.triple());
            println!("  {} {}", status_icon(installed), target.triple());
        });
        println!();

        println!("Android Tools");
        match &check.tools.android_ndk {
            Some(path) => println!("  {} Android NDK ({})", status_icon(true), path),
            None => println!("  {} Android NDK not found", status_icon(false)),
        }
        println!();
    }

    if options.wasm {
        println!("WASM Targets");
        let wasm_target = options
            .wasm_target_triple
            .as_deref()
            .unwrap_or(RustTarget::WASM32_UNKNOWN_UNKNOWN.triple());
        let installed = check
            .installed_targets
            .iter()
            .any(|installed| installed == wasm_target);
        println!("  {} {}", status_icon(installed), wasm_target);
        println!();
    }

    if check.has_missing_targets() {
        println!("Missing targets can be installed with:");
        check.fix_commands().iter().for_each(|cmd| {
            println!("  {}", cmd);
        });
        println!();
        println!("Or run: boltffi check --fix");
    }
}

fn status_icon(success: bool) -> &'static str {
    if success { "[ok]" } else { "[missing]" }
}

fn print_apple_targets(check: &EnvironmentCheck, apple_targets: &[RustTarget]) {
    print_apple_target_group(
        check,
        apple_targets,
        "Apple Targets (iOS)",
        crate::target::Platform::Ios,
    );
    print_apple_target_group(
        check,
        apple_targets,
        "Apple Targets (iOS Simulator)",
        crate::target::Platform::IosSimulator,
    );
    print_apple_target_group(
        check,
        apple_targets,
        "Apple Targets (macOS)",
        crate::target::Platform::MacOs,
    );
}

fn print_apple_target_group(
    check: &EnvironmentCheck,
    apple_targets: &[RustTarget],
    label: &str,
    platform: crate::target::Platform,
) {
    let matching_targets: Vec<_> = apple_targets
        .iter()
        .filter(|target| target.platform() == platform)
        .collect();

    if matching_targets.is_empty() {
        return;
    }

    println!("{label}");
    matching_targets.iter().for_each(|target| {
        let installed = check.installed_targets.iter().any(|t| t == target.triple());
        println!("  {} {}", status_icon(installed), target.triple());
    });
    println!();
}
