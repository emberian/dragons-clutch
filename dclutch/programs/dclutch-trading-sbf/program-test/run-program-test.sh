#!/usr/bin/env bash
# Real-ELF evidence for the family-neutral Trading activation outer.
#
# The suite had no runner, which is why it was possible for the seam to create
# an all-zero family root for months without anyone meeting a red test.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
sbf_out="$(mktemp -d /tmp/dclutch-trading-outer.XXXXXX)"

cleanup() {
  rm -rf -- "$sbf_out"
}
trap cleanup EXIT HUP INT TERM

cd "$repo_root"
for manifest in \
  programs/dclutch-trading-sbf/program-test/test-programs/trading-outer/Cargo.toml \
  programs/dclutch-trading-sbf/program-test/test-programs/core-caller/Cargo.toml \
  programs/dclutch-trading-sbf/program-test/test-programs/registry/Cargo.toml
do
  cargo build-sbf --manifest-path "$manifest" --sbf-out-dir "$sbf_out"
done

SBF_OUT_DIR="$sbf_out" cargo test \
  --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
  --test activation \
  -- --nocapture
