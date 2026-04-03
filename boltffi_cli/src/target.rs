use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    Ios,
    IosSimulator,
    MacOs,
    Android,
    Wasm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Architecture {
    Arm64,
    X86_64,
    Armv7,
    X86,
    Wasm32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppleIosArchitecture {
    #[serde(rename = "arm64")]
    Arm64,
}

impl AppleIosArchitecture {
    pub const ALL: &'static [Self] = &[Self::Arm64];

    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Arm64 => "arm64",
        }
    }

    pub const fn rust_target(self) -> RustTarget {
        match self {
            Self::Arm64 => RustTarget::IOS_ARM64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppleArchitecture {
    #[serde(rename = "arm64")]
    Arm64,
    #[serde(rename = "x86_64")]
    X86_64,
}

impl AppleArchitecture {
    pub const ALL: &'static [Self] = &[Self::Arm64, Self::X86_64];

    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Arm64 => "arm64",
            Self::X86_64 => "x86_64",
        }
    }

    pub const fn simulator_rust_target(self) -> RustTarget {
        match self {
            Self::Arm64 => RustTarget::IOS_SIM_ARM64,
            Self::X86_64 => RustTarget::IOS_SIM_X86_64,
        }
    }

    pub const fn macos_rust_target(self) -> RustTarget {
        match self {
            Self::Arm64 => RustTarget::MACOS_ARM64,
            Self::X86_64 => RustTarget::MACOS_X86_64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AndroidArchitecture {
    #[serde(rename = "arm64")]
    Arm64,
    #[serde(rename = "armv7")]
    Armv7,
    #[serde(rename = "x86_64")]
    X86_64,
    #[serde(rename = "x86")]
    X86,
}

impl AndroidArchitecture {
    pub const ALL: &'static [Self] = &[Self::Arm64, Self::Armv7, Self::X86_64, Self::X86];

    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Arm64 => "arm64",
            Self::Armv7 => "armv7",
            Self::X86_64 => "x86_64",
            Self::X86 => "x86",
        }
    }

    pub const fn rust_target(self) -> RustTarget {
        match self {
            Self::Arm64 => RustTarget::ANDROID_ARM64,
            Self::Armv7 => RustTarget::ANDROID_ARMV7,
            Self::X86_64 => RustTarget::ANDROID_X86_64,
            Self::X86 => RustTarget::ANDROID_X86,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RustTarget {
    triple: &'static str,
    platform: Platform,
    architecture: Architecture,
}

impl RustTarget {
    pub const IOS_ARM64: Self = Self {
        triple: "aarch64-apple-ios",
        platform: Platform::Ios,
        architecture: Architecture::Arm64,
    };

    pub const IOS_SIM_ARM64: Self = Self {
        triple: "aarch64-apple-ios-sim",
        platform: Platform::IosSimulator,
        architecture: Architecture::Arm64,
    };

    pub const IOS_SIM_X86_64: Self = Self {
        triple: "x86_64-apple-ios",
        platform: Platform::IosSimulator,
        architecture: Architecture::X86_64,
    };

    pub const MACOS_ARM64: Self = Self {
        triple: "aarch64-apple-darwin",
        platform: Platform::MacOs,
        architecture: Architecture::Arm64,
    };

    pub const MACOS_X86_64: Self = Self {
        triple: "x86_64-apple-darwin",
        platform: Platform::MacOs,
        architecture: Architecture::X86_64,
    };

    pub const ANDROID_ARM64: Self = Self {
        triple: "aarch64-linux-android",
        platform: Platform::Android,
        architecture: Architecture::Arm64,
    };

    pub const ANDROID_ARMV7: Self = Self {
        triple: "armv7-linux-androideabi",
        platform: Platform::Android,
        architecture: Architecture::Armv7,
    };

    pub const ANDROID_X86_64: Self = Self {
        triple: "x86_64-linux-android",
        platform: Platform::Android,
        architecture: Architecture::X86_64,
    };

    pub const ANDROID_X86: Self = Self {
        triple: "i686-linux-android",
        platform: Platform::Android,
        architecture: Architecture::X86,
    };

    pub const WASM32_UNKNOWN_UNKNOWN: Self = Self {
        triple: "wasm32-unknown-unknown",
        platform: Platform::Wasm,
        architecture: Architecture::Wasm32,
    };

    pub const ALL_IOS: &'static [Self] =
        &[Self::IOS_ARM64, Self::IOS_SIM_ARM64, Self::IOS_SIM_X86_64];

    pub const ALL_MACOS: &'static [Self] = &[Self::MACOS_ARM64, Self::MACOS_X86_64];

    pub const ALL_ANDROID: &'static [Self] = &[
        Self::ANDROID_ARM64,
        Self::ANDROID_ARMV7,
        Self::ANDROID_X86_64,
        Self::ANDROID_X86,
    ];

    pub const ALL_WASM: &'static [Self] = &[Self::WASM32_UNKNOWN_UNKNOWN];

    pub const fn from_android_architecture(architecture: AndroidArchitecture) -> Self {
        architecture.rust_target()
    }

    pub fn triple(&self) -> &'static str {
        self.triple
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    pub fn architecture(&self) -> Architecture {
        self.architecture
    }

    pub fn library_path_for_profile(
        &self,
        target_dir: &Path,
        lib_name: &str,
        profile_directory_name: &str,
    ) -> PathBuf {
        let artifact_name = match self.platform {
            Platform::Wasm => format!("{}.wasm", lib_name),
            Platform::Ios | Platform::IosSimulator | Platform::MacOs => {
                format!("lib{}.a", lib_name)
            }
            // Android packages a JNI-facing shared object by linking the Rust static archive
            // into the generated JNI glue. Using the Rust cdylib here leaves a DT_NEEDED
            // entry on the build-machine path, which breaks on-device loading.
            Platform::Android => format!("lib{}.a", lib_name),
        };

        target_dir
            .join(self.triple)
            .join(profile_directory_name)
            .join(artifact_name)
    }
}

pub fn resolve_android_targets(architectures: &[AndroidArchitecture]) -> Vec<RustTarget> {
    architectures
        .iter()
        .copied()
        .map(RustTarget::from_android_architecture)
        .collect()
}

pub fn resolve_apple_ios_targets(architectures: &[AppleIosArchitecture]) -> Vec<RustTarget> {
    architectures
        .iter()
        .copied()
        .map(AppleIosArchitecture::rust_target)
        .collect()
}

pub fn resolve_apple_simulator_targets(architectures: &[AppleArchitecture]) -> Vec<RustTarget> {
    architectures
        .iter()
        .copied()
        .map(AppleArchitecture::simulator_rust_target)
        .collect()
}

pub fn resolve_apple_macos_targets(architectures: &[AppleArchitecture]) -> Vec<RustTarget> {
    architectures
        .iter()
        .copied()
        .map(AppleArchitecture::macos_rust_target)
        .collect()
}

impl Platform {
    pub fn is_apple(&self) -> bool {
        matches!(
            self,
            Platform::Ios | Platform::IosSimulator | Platform::MacOs
        )
    }
}

impl Architecture {
    pub fn android_abi(&self) -> &'static str {
        match self {
            Architecture::Arm64 => "arm64-v8a",
            Architecture::Armv7 => "armeabi-v7a",
            Architecture::X86_64 => "x86_64",
            Architecture::X86 => "x86",
            Architecture::Wasm32 => unreachable!("wasm targets do not map to android abi"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuiltLibrary {
    pub target: RustTarget,
    pub path: PathBuf,
}

impl BuiltLibrary {
    pub fn discover_for_targets(
        target_dir: &Path,
        lib_name: &str,
        profile_directory_name: &str,
        targets: &[RustTarget],
    ) -> Vec<Self> {
        targets
            .iter()
            .filter_map(|target| {
                let path =
                    target.library_path_for_profile(target_dir, lib_name, profile_directory_name);
                path.exists().then_some(BuiltLibrary {
                    target: *target,
                    path,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AndroidArchitecture, AppleArchitecture, AppleIosArchitecture, BuiltLibrary, Platform,
        RustTarget, resolve_android_targets, resolve_apple_ios_targets,
        resolve_apple_macos_targets, resolve_apple_simulator_targets,
    };
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn apple_targets_use_static_libraries() {
        let library_path =
            RustTarget::IOS_ARM64.library_path_for_profile(Path::new("target"), "demo", "debug");

        assert_eq!(RustTarget::IOS_ARM64.platform(), Platform::Ios);
        assert!(library_path.ends_with("target/aarch64-apple-ios/debug/libdemo.a"));
    }

    #[test]
    fn android_targets_use_static_libraries_for_packaging() {
        let library_path = RustTarget::ANDROID_ARM64.library_path_for_profile(
            Path::new("target"),
            "demo",
            "debug",
        );

        assert_eq!(RustTarget::ANDROID_ARM64.platform(), Platform::Android);
        assert!(library_path.ends_with("target/aarch64-linux-android/debug/libdemo.a"));
    }

    #[test]
    fn resolves_android_architectures_to_targets() {
        let targets = resolve_android_targets(&[
            AndroidArchitecture::Arm64,
            AndroidArchitecture::Armv7,
            AndroidArchitecture::X86_64,
        ]);

        assert_eq!(
            targets
                .iter()
                .map(|target| target.triple())
                .collect::<Vec<_>>(),
            vec![
                "aarch64-linux-android",
                "armv7-linux-androideabi",
                "x86_64-linux-android",
            ]
        );
    }

    #[test]
    fn resolves_apple_ios_architectures_to_targets() {
        let targets = resolve_apple_ios_targets(&[AppleIosArchitecture::Arm64]);

        assert_eq!(
            targets
                .iter()
                .map(|target| target.triple())
                .collect::<Vec<_>>(),
            vec!["aarch64-apple-ios"]
        );
    }

    #[test]
    fn resolves_apple_simulator_architectures_to_targets() {
        let targets =
            resolve_apple_simulator_targets(&[AppleArchitecture::Arm64, AppleArchitecture::X86_64]);

        assert_eq!(
            targets
                .iter()
                .map(|target| target.triple())
                .collect::<Vec<_>>(),
            vec!["aarch64-apple-ios-sim", "x86_64-apple-ios"]
        );
    }

    #[test]
    fn resolves_apple_macos_architectures_to_targets() {
        let targets =
            resolve_apple_macos_targets(&[AppleArchitecture::Arm64, AppleArchitecture::X86_64]);

        assert_eq!(
            targets
                .iter()
                .map(|target| target.triple())
                .collect::<Vec<_>>(),
            vec!["aarch64-apple-darwin", "x86_64-apple-darwin"]
        );
    }

    #[test]
    fn discovers_only_requested_targets() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let temp_root = std::env::temp_dir().join(format!("boltffi-target-test-{unique}"));
        let arm64_path =
            RustTarget::ANDROID_ARM64.library_path_for_profile(&temp_root, "demo", "debug");
        let x86_path =
            RustTarget::ANDROID_X86.library_path_for_profile(&temp_root, "demo", "debug");

        fs::create_dir_all(arm64_path.parent().expect("arm64 parent")).expect("create arm64 dir");
        fs::create_dir_all(x86_path.parent().expect("x86 parent")).expect("create x86 dir");
        fs::write(&arm64_path, []).expect("write arm64 artifact");
        fs::write(&x86_path, []).expect("write x86 artifact");

        let discovered = BuiltLibrary::discover_for_targets(
            &temp_root,
            "demo",
            "debug",
            &[RustTarget::ANDROID_ARM64],
        );

        assert_eq!(discovered.len(), 1);
        assert_eq!(
            discovered[0].target.triple(),
            RustTarget::ANDROID_ARM64.triple()
        );

        fs::remove_dir_all(&temp_root).expect("cleanup temp target dir");
    }
}
