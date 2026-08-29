#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
sbf_out="$(mktemp -d /tmp/dclutch-dealer-accelerator.XXXXXX)"

cleanup() {
  rm -rf -- "$sbf_out"
}
trap cleanup EXIT HUP INT TERM

cd "$repo_root"
# tests/accepted.rs drives the real Trading ELF directly over the lock-bounded
# checkpoint routes, so Trading is staged here alongside the accelerator and its
# caller. Custody joins them because the reservation route authenticates the
# Custody role out of a real activated release set, pinned to that artifact's
# exact deployment. Without these the accepted campaign has no artifact to be
# evidence about.
for manifest in \
  programs/dclutch-dealer-accelerator-sbf/Cargo.toml \
  programs/dclutch-dealer-accelerator-sbf/test-programs/dealer-caller/Cargo.toml \
  programs/dclutch-trading-sbf/Cargo.toml \
  programs/dclutch-custody-sbf/Cargo.toml \
  programs/dclutch-claims-sbf/Cargo.toml \
  programs/dclutch-core-sbf/Cargo.toml
do
  cargo build-sbf --manifest-path "$manifest" --sbf-out-dir "$sbf_out"
done

SBF_OUT_DIR="$sbf_out" cargo test \
  --locked \
  --manifest-path programs/dclutch-dealer-accelerator-sbf/program-test/Cargo.toml \
  --tests \
  -- --nocapture
