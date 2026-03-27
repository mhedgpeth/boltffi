#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
apple_dir="$repo_root/examples/platforms/apple"
kotlin_dir="$repo_root/examples/platforms/kotlin"
java_dir="$repo_root/examples/platforms/java"
wasm_dir="$repo_root/examples/platforms/wasm"

run_step() {
    local title="$1"
    shift
    printf '\n=== %s ===\n' "$title"
    "$@"
}

cd "$script_dir"

run_step "install boltffi" cargo install --path "$repo_root/boltffi_cli" --force
run_step "pack apple" boltffi pack apple
run_step "swift test" swift test --package-path "$apple_dir"
run_step "pack android" boltffi pack android
run_step "kotlin test" gradle -p "$kotlin_dir" test
run_step "pack java" boltffi pack java
run_step "java demo" "$java_dir/test-demo.sh" --auto
run_step "pack wasm" boltffi pack wasm
run_step "wasm test" npm test --prefix "$wasm_dir"
