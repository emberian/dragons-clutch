#!/usr/bin/env bash
set -euo pipefail

repository="$(cd "$(dirname "$0")/../.." && pwd)"
sbf_out="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-rational-representation-v2-sbf.XXXXXX")"
token_source="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-token-2022-v11.XXXXXX")"
token_archive="$token_source/spl-token-2022-11.0.0.crate"
fixture_builder="$repository/programs/dclutch-claims-sbf/fixtures/prepare-token-2022-v11.sh"

cleanup() {
  rm -rf -- "$sbf_out" "$token_source"
}
trap cleanup EXIT HUP INT TERM

if [[ -n "${TOKEN_2022_V11_CRATE:-}" ]]; then
  cp -- "$TOKEN_2022_V11_CRATE" "$token_archive"
else
  curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
    https://static.crates.io/crates/spl-token-2022/spl-token-2022-11.0.0.crate \
    --output "$token_archive"
fi

if [[ -n "${CARGO_BUILD_SBF:-}" ]]; then
  builder=("$CARGO_BUILD_SBF")
else
  builder=(cargo build-sbf)
fi

cd "$repository"
for manifest in \
  programs/dclutch-claims-sbf/Cargo.toml \
  programs/dclutch-custody-sbf/Cargo.toml \
  programs/dclutch-registry-sbf/Cargo.toml \
  programs/dclutch-core-sbf/Cargo.toml \
  programs/dclutch-claims-sbf/test-programs/rational-v2-caller/Cargo.toml
do
  "${builder[@]}" --manifest-path "$manifest" --sbf-out-dir "$sbf_out"
done

"$fixture_builder" "$token_archive" "$sbf_out"
shasum -a 256 "$sbf_out"/*.so

SBF_OUT_DIR="$sbf_out" cargo test \
  --locked \
  --manifest-path programs/dclutch-claims-sbf/Cargo.toml \
  --test rational_representation_v2_program_test \
  -- --nocapture
