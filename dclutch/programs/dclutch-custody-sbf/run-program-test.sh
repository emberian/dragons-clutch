#!/bin/sh
set -eu

repository=$(git rev-parse --show-toplevel)
output=$(mktemp -d "${TMPDIR:-/tmp}/dclutch-custody-sbf.XXXXXX")

cleanup() {
  rm -rf -- "$output"
}
trap cleanup EXIT HUP INT TERM

cargo build-sbf \
  --manifest-path "$repository/programs/dclutch-custody-sbf/Cargo.toml" \
  --sbf-out-dir "$output"
cargo build-sbf \
  --manifest-path "$repository/programs/dclutch-custody-sbf/test-programs/caller/Cargo.toml" \
  --sbf-out-dir "$output"
cargo build-sbf \
  --manifest-path "$repository/programs/dclutch-registry-sbf/Cargo.toml" \
  --sbf-out-dir "$output"
SBF_OUT_DIR="$output" cargo test \
  --manifest-path "$repository/programs/dclutch-custody-sbf/Cargo.toml" \
  --test program_test -- --nocapture
