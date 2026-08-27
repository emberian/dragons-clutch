#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
sbf_out="$(mktemp -d /tmp/dclutch-general-accelerator.XXXXXX)"

cleanup() {
  rm -rf -- "$sbf_out"
}
trap cleanup EXIT HUP INT TERM

cd "$repo_root"
for manifest in \
  programs/dclutch-general-accelerator-sbf/Cargo.toml \
  programs/dclutch-general-accelerator-sbf/test-programs/general-caller/Cargo.toml
do
  cargo build-sbf --manifest-path "$manifest" --sbf-out-dir "$sbf_out"
done

SBF_OUT_DIR="$sbf_out" cargo test \
  --locked \
  --manifest-path programs/dclutch-general-accelerator-sbf/program-test/Cargo.toml \
  --tests \
  -- --nocapture
