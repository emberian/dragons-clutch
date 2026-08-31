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

# The ordered Fractional retirement walk closes a real shard Mint, so it needs
# the real Token-2022 rather than the refusing stand-in the rollback test uses.
# Same audited v11 artifact and same provenance check as the fractional-atomic
# campaign; build it once with fixtures/prepare-token-2022-v11.sh.
: "${TOKEN_2022_SO:?set TOKEN_2022_SO to the prepared spl_token_2022.so (see programs/dclutch-claims-sbf/fixtures/README.md)}"
provenance="programs/dclutch-claims-sbf/fixtures/token-2022-v11.provenance"
observed="$(shasum -a 256 "$TOKEN_2022_SO" | cut -d' ' -f1)"
if ! grep -qE "^(canonical_elf_sha256|macos_arm64_audit_elf_sha256)=${observed}$" "$provenance"; then
  echo "TOKEN_2022_SO sha256 ${observed} is in neither row of ${provenance}" >&2
  exit 1
fi
cp -- "$TOKEN_2022_SO" "$sbf_out/spl_token_2022.so"

SBF_OUT_DIR="$sbf_out" cargo test \
  --locked \
  --manifest-path programs/dclutch-claims-sbf/program-test/protocol-position/Cargo.toml \
  --test lifecycle \
  -- --nocapture
