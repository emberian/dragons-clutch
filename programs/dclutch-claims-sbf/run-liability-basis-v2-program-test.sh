#!/bin/sh
set -eu

repository=$(git rev-parse --show-toplevel)
output=$(mktemp -d "${TMPDIR:-/tmp}/dclutch-liability-basis-v2-sbf.XXXXXX")

cleanup() {
  rm -rf -- "$output"
}
trap cleanup EXIT HUP INT TERM

for manifest in \
  "$repository/programs/dclutch-claims-sbf/Cargo.toml" \
  "$repository/programs/dclutch-custody-sbf/Cargo.toml" \
  "$repository/programs/dclutch-registry-sbf/Cargo.toml" \
  "$repository/programs/dclutch-core-sbf/Cargo.toml" \
  "$repository/programs/dclutch-claims-sbf/test-programs/liability-basis-caller/Cargo.toml"
do
  cargo build-sbf --manifest-path "$manifest" --sbf-out-dir "$output"
done

SBF_OUT_DIR="$output" cargo test \
  --manifest-path "$repository/programs/dclutch-claims-sbf/Cargo.toml" \
  --test liability_basis_v2_program_test -- --nocapture
