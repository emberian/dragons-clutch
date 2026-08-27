#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../../.." && pwd)"
sbf_out="$(mktemp -d /tmp/dclutch-affine-program-test.XXXXXX)"

cd "$repo_root"
cargo build-sbf --manifest-path programs/dclutch-claims-sbf/Cargo.toml --sbf-out-dir "$sbf_out"
cargo build-sbf --manifest-path programs/dclutch-registry-sbf/Cargo.toml --sbf-out-dir "$sbf_out"
cargo build-sbf --manifest-path programs/dclutch-core-sbf/Cargo.toml --sbf-out-dir "$sbf_out"
cargo build-sbf \
  --manifest-path programs/dclutch-claims-sbf/test-programs/affine-batch-caller/Cargo.toml \
  --sbf-out-dir "$sbf_out"

SBF_OUT_DIR="$sbf_out" cargo test \
  --locked \
  --manifest-path programs/dclutch-claims-sbf/program-test/affine-batch/Cargo.toml \
  --test affine_batch_v2 \
  -- --nocapture
