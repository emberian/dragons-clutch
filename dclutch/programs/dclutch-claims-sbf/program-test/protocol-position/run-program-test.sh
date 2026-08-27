#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../../.." && pwd)"
sbf_out="$(mktemp -d /tmp/dclutch-protocol-position.XXXXXX)"

cleanup() {
  rm -rf -- "$sbf_out"
}
trap cleanup EXIT HUP INT TERM

cd "$repo_root"
for manifest in \
  programs/dclutch-claims-sbf/Cargo.toml \
  programs/dclutch-registry-sbf/Cargo.toml \
  programs/dclutch-core-sbf/Cargo.toml \
  programs/dclutch-claims-sbf/test-programs/liability-basis-caller/Cargo.toml \
  programs/dclutch-rent-sbf/Cargo.toml
do
  cargo build-sbf --manifest-path "$manifest" --sbf-out-dir "$sbf_out"
done

SBF_OUT_DIR="$sbf_out" cargo test \
  --locked \
  --manifest-path programs/dclutch-claims-sbf/program-test/protocol-position/Cargo.toml \
  --test lifecycle \
  -- --nocapture
