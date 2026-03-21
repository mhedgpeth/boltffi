#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

PACKAGE="bench_uniffi"
TARGET_DIR="target"
DIST_DIR="dist/kotlin"

cargo build --lib --release

rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

if [ "$(uname)" == "Darwin" ]; then
    # Mac
    LIBRARY_FILE="lib${PACKAGE}.dylib"
elif [ "$(expr substr $(uname -s) 1 5)" == "Linux" ]; then
    # Linux
    LIBRARY_FILE="lib${PACKAGE}.so"
else
    echo "Unknown platform: $(uname)"
    exit 1
fi

cargo run --bin uniffi-bindgen generate \
  --library "${TARGET_DIR}/release/$LIBRARY_FILE" \
  --language kotlin \
  --out-dir "$DIST_DIR"
