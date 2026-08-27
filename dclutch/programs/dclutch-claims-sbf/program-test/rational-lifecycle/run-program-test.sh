#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../../.." && pwd)"
sbf_out="$(mktemp -d /tmp/dclutch-rational-lifecycle.XXXXXX)"
: "${TOKEN_2022_V11_CRATE:?set TOKEN_2022_V11_CRATE to the pinned spl-token-2022 11.0.0 crate archive}"
fixture_builder="$repo_root/programs/dclutch-claims-sbf/fixtures/prepare-token-2022-v11.sh"

cleanup() {
  rm -rf -- "$sbf_out"
}
trap cleanup EXIT HUP INT TERM

cd "$repo_root"
for manifest in \
  programs/dclutch-claims-sbf/Cargo.toml \
  programs/dclutch-registry-sbf/Cargo.toml \
  programs/dclutch-core-sbf/Cargo.toml \
  programs/dclutch-claims-sbf/test-programs/rational-lifecycle-caller/Cargo.toml \
  programs/dclutch-rent-sbf/Cargo.toml
do
  cargo build-sbf --manifest-path "$manifest" --sbf-out-dir "$sbf_out"
done
"$fixture_builder" "$TOKEN_2022_V11_CRATE" "$sbf_out"

SBF_OUT_DIR="$sbf_out" cargo test \
  --locked \
  --manifest-path programs/dclutch-claims-sbf/program-test/rational-lifecycle/Cargo.toml \
  --test lifecycle \
  -- --nocapture
