#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RUST_DIR="$SCRIPT_DIR/../rust-boltffi"
JNI_SRC="$RUST_DIR/dist/android/kotlin/jni/jni_glue.c"
HEADER_DIR="$RUST_DIR/dist/android/include"
OUTPUT_DIR="$RUST_DIR/target/release"

if [ -z "$JAVA_HOME" ]; then
    JAVA_HOME=$(/usr/libexec/java_home 2>/dev/null || echo "")
fi

if [ -z "$JAVA_HOME" ]; then
    echo "Error: JAVA_HOME not set and cannot be determined"
    exit 1
fi

if [ ! -f "$JNI_SRC" ]; then
    echo "Error: JNI glue not found at $JNI_SRC"
    echo "Run benchmarks/rust-boltffi/build.sh --platform android --skip-bench first"
    exit 1
fi

echo "Compiling JNI glue..."
cd "$OUTPUT_DIR"
cc -c -fPIC \
    -I"$HEADER_DIR" \
    -I"$JAVA_HOME/include" \
    -I"$JAVA_HOME/include/darwin" \
    -o jni_glue.o \
    "$JNI_SRC"

echo "Linking final library..."
cc -shared \
    -o libbench_boltffi_jni.dylib \
    jni_glue.o \
    -L. -lbench_boltffi \
    -Wl,-rpath,@loader_path

rm -f jni_glue.o
echo "Built: $OUTPUT_DIR/libbench_boltffi_jni.dylib"
