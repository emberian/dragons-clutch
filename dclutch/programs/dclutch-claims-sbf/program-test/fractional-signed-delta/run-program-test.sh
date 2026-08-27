#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../../.." && pwd)"
sbf_out="$(mktemp -d /tmp/dclutch-fractional-signed-delta.XXXXXX)"

cd "$repo_root"
cargo build-sbf --manifest-path programs/dclutch-claims-sbf/Cargo.toml --sbf-out-dir "$sbf_out"
cargo build-sbf --manifest-path programs/dclutch-registry-sbf/Cargo.toml --sbf-out-dir "$sbf_out"
cargo build-sbf --manifest-path programs/dclutch-core-sbf/Cargo.toml --sbf-out-dir "$sbf_out"
cargo build-sbf \
  --manifest-path programs/dclutch-claims-sbf/test-programs/fractional-signed-delta-caller/Cargo.toml \
  --sbf-out-dir "$sbf_out"

SBF_OUT_DIR="$sbf_out" cargo test \
  --locked \
  --manifest-path programs/dclutch-claims-sbf/program-test/fractional-signed-delta/Cargo.toml \
  --test fractional_signed_delta \
  -- --nocapture
