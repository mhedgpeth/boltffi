#!/usr/bin/env zsh
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RUST_RIFF_DIR="$SCRIPT_DIR/../rust-boltffi"
RUST_UNIFFI_DIR="$SCRIPT_DIR/../rust-uniffi"
JNI_LIBS_DIR="$SCRIPT_DIR/app/src/main/jniLibs"
JNI_SRC="$RUST_RIFF_DIR/dist/android/kotlin/jni/jni_glue.c"
HEADER_DIR="$RUST_RIFF_DIR/dist/android/include"

ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/28.0.13004108}"
TOOLCHAIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64"

if [ ! -d "$TOOLCHAIN" ]; then
    echo "Error: NDK toolchain not found at $TOOLCHAIN"
    exit 1
fi

build_target() {
    local target="$1"
    local triple="$2"
    local abi="$3"

    echo ""
    echo "--- Building $target ($abi) ---"

    export CC="$TOOLCHAIN/bin/${triple}-clang"
    export AR="$TOOLCHAIN/bin/llvm-ar"

    local env_var="CARGO_TARGET_$(echo "$target" | tr '[:lower:]-' '[:upper:]_')_LINKER"
    export "$env_var"="$CC"

    echo "  Building bench_boltffi..."
    cargo build --manifest-path "$RUST_RIFF_DIR/Cargo.toml" --target "$target" --release

    echo "  Building bench_uniffi..."
    cargo build --manifest-path "$RUST_UNIFFI_DIR/Cargo.toml" --target "$target" --release

    local rust_out="$RUST_RIFF_DIR/target/$target/release"
    local uniffi_out="$RUST_UNIFFI_DIR/target/$target/release"

    echo "  Compiling JNI glue..."
    "$TOOLCHAIN/bin/${triple}-clang" -c -fPIC \
        -I"$HEADER_DIR" \
        -o "/tmp/jni_glue_${abi}.o" \
        "$JNI_SRC"

    echo "  Linking libbench_boltffi.so..."
    mkdir -p "$JNI_LIBS_DIR/$abi"
    "$TOOLCHAIN/bin/${triple}-clang" -shared \
        -o "$JNI_LIBS_DIR/$abi/libbench_boltffi.so" \
        "/tmp/jni_glue_${abi}.o" \
        -Wl,--whole-archive "$rust_out/libbench_boltffi.a" -Wl,--no-whole-archive \
        -lm -llog -ldl

    echo "  Copying libbench_uniffi.so..."
    cp "$uniffi_out/libbench_uniffi.so" "$JNI_LIBS_DIR/$abi/" 2>/dev/null || \
    echo "  Warning: uniffi .so not found for $abi"

    rm -f "/tmp/jni_glue_${abi}.o"
    echo "  Done: $abi"
}

echo "=== Building Rust libraries for Android ==="

build_target "aarch64-linux-android" "aarch64-linux-android35" "arm64-v8a"
build_target "x86_64-linux-android" "x86_64-linux-android35" "x86_64"

echo ""
echo "=== Android native libraries built ==="
find "$JNI_LIBS_DIR" -name "*.so" -exec ls -la {} \;
