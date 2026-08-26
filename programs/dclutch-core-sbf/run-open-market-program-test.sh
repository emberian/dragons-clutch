#!/usr/bin/env bash
set -euo pipefail

workspace="$(cd "$(dirname "$0")/../.." && pwd)"
output="$(mktemp -d)"
trap 'rm -rf "$output"' EXIT

(cd "$workspace" && cargo build-sbf --manifest-path programs/dclutch-registry-sbf/Cargo.toml --sbf-out-dir "$output")
(cd "$workspace" && cargo build-sbf --manifest-path programs/dclutch-rent-sbf/Cargo.toml --sbf-out-dir "$output")
(cd "$workspace" && cargo build-sbf --manifest-path programs/dclutch-custody-sbf/Cargo.toml --sbf-out-dir "$output")
(cd "$workspace" && cargo build-sbf --manifest-path programs/dclutch-core-sbf/test-programs/series-consume-caller/Cargo.toml --sbf-out-dir "$output")
(cd "$workspace" && cargo build-sbf --manifest-path programs/dclutch-core-sbf/Cargo.toml --sbf-out-dir "$output")
SBF_OUT_DIR="$output" cargo test --manifest-path "$workspace/programs/dclutch-core-sbf/Cargo.toml" --test infrastructure_program_test -- --nocapture
SBF_OUT_DIR="$output" cargo test --manifest-path "$workspace/programs/dclutch-core-sbf/Cargo.toml" --test found_program_test -- --nocapture
SBF_OUT_DIR="$output" cargo test --manifest-path "$workspace/programs/dclutch-core-sbf/Cargo.toml" --test open_market_program_test -- --nocapture
