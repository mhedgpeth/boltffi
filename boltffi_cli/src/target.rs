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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    pub fn discover_for_profile(
        target_dir: &Path,
        lib_name: &str,
        profile_directory_name: &str,
    ) -> Vec<Self> {
        let all_targets = RustTarget::ALL_IOS
            .iter()
            .chain(RustTarget::ALL_MACOS)
            .chain(RustTarget::ALL_ANDROID)
            .chain(RustTarget::ALL_WASM);

        all_targets
            .filter_map(|target| {
                let path =
                    target.library_path_for_profile(target_dir, lib_name, profile_directory_name);
                path.exists().then(|| BuiltLibrary {
                    target: target.clone(),
                    path,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{Platform, RustTarget};
    use std::path::Path;

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
}
