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
# EXIT 2, not 1: an absent fixture means this suite DID NOT RUN, which is a
# different fact from a failing gate and the one `tools/ci/run.sh`'s `suites`
# tier reports per row. A `:?` expansion would exit 1 under `set -u` and be read
# as a protocol finding. The digest mismatch below keeps exit 1.
if [ -z "${TOKEN_2022_SO:-}" ] || [ ! -f "${TOKEN_2022_SO:-}" ]; then
  echo "TOKEN_2022_SO is unset or missing, so this suite DID NOT RUN. See" >&2
  echo "programs/dclutch-claims-sbf/fixtures/README.md to prepare it." >&2
  exit 2
fi
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
