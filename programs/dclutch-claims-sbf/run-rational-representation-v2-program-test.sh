#!/usr/bin/env bash
set -euo pipefail

repository="$(cd "$(dirname "$0")/../.." && pwd)"
sbf_out="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-rational-representation-v2-sbf.XXXXXX")"
token_source="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-token-2022-v11.XXXXXX")"
token_archive="$token_source/spl-token-2022-11.0.0.crate"
token_package="$token_source/source"

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

expected_crate_sha=2f0e045d23300c8c9f57e52fa7a1103a20a707cc02080db929c2ff09044aa06a
actual_crate_sha="$(shasum -a 256 "$token_archive" | awk '{print $1}')"
if [[ "$actual_crate_sha" != "$expected_crate_sha" ]]; then
  echo "unexpected spl-token-2022 11.0.0 crate digest: $actual_crate_sha" >&2
  exit 1
fi
mkdir "$token_package"
tar -xzf "$token_archive" -C "$token_package" --strip-components=1
grep -Fq 'd9a5ce37c018981b6823746856ff9fe1268837cf' "$token_package/.cargo_vcs_info.json"
grep -Fq '"path_in_vcs":"program"' <(tr -d '[:space:]' < "$token_package/.cargo_vcs_info.json")

if [[ -n "${CARGO_BUILD_SBF:-}" ]]; then
  builder=("$CARGO_BUILD_SBF")
else
  builder=(cargo build-sbf)
fi
expected_toolchain=$'cargo-build-sbf 4.0.0\nplatform-tools v1.53\nrustc 1.89.0'
actual_toolchain="$("${builder[@]}" --version)"
if [[ "$actual_toolchain" != "$expected_toolchain" ]]; then
  echo "unexpected SBF toolchain:" >&2
  echo "$actual_toolchain" >&2
  exit 1
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
"${builder[@]}" --manifest-path "$token_package/Cargo.toml" --sbf-out-dir "$sbf_out"

expected_token_elf_sha=e2acdfb750881462ad613a15cc9c54ae17ce066580e867e1e635fbdfe01f5697
actual_token_elf_sha="$(shasum -a 256 "$sbf_out/spl_token_2022.so" | awk '{print $1}')"
if [[ "$actual_token_elf_sha" != "$expected_token_elf_sha" ]]; then
  echo "unexpected spl-token-2022 ELF digest: $actual_token_elf_sha" >&2
  exit 1
fi
shasum -a 256 "$sbf_out"/*.so

SBF_OUT_DIR="$sbf_out" cargo test \
  --locked \
  --manifest-path programs/dclutch-claims-sbf/Cargo.toml \
  --test rational_representation_v2_program_test \
  -- --nocapture
