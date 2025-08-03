#!/bin/bash
set -e

cd "$(dirname "$0")"

echo "Building Rust library..."
(cd .. && cargo build --release)

echo "Copying header..."
cp ../target/release/build/mobiFFI_core-*/out/mobiFFI_core.h .

echo "Compiling Swift test..."
swiftc -I. -L../target/release -lmobiFFI_core test.swift -o test_ffi

echo "Running test..."
DYLD_LIBRARY_PATH=../target/release ./test_ffi
