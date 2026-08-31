#!/usr/bin/env bash
set -euo pipefail

# Real-ELF campaign for the production Fractional atomic Claims route.
#
# Token-2022 is the audited v11 fixture; build it once with
# programs/dclutch-claims-sbf/fixtures/prepare-token-2022-v11.sh and point
# TOKEN_2022_SO at the result, or leave it unset to reuse a prepared copy.

repo_root="$(cd "$(dirname "$0")/../../../.." && pwd)"
sbf_out="$(mktemp -d /tmp/dclutch-fractional-atomic.XXXXXX)"

cd "$repo_root"
cargo build-sbf --manifest-path programs/dclutch-claims-sbf/Cargo.toml --sbf-out-dir "$sbf_out"
cargo build-sbf --manifest-path programs/dclutch-registry-sbf/Cargo.toml --sbf-out-dir "$sbf_out"
cargo build-sbf --manifest-path programs/dclutch-core-sbf/Cargo.toml --sbf-out-dir "$sbf_out"
# The terminal campaign composes the real Custody program.
cargo build-sbf --manifest-path programs/dclutch-custody-sbf/Cargo.toml --sbf-out-dir "$sbf_out"
cargo build-sbf \
  --manifest-path programs/dclutch-claims-sbf/test-programs/fractional-atomic-caller/Cargo.toml \
  --sbf-out-dir "$sbf_out"

# Token-2022 is the audited v11 fixture. The campaign's Token behaviour is only
# evidence if this is that exact artifact, so the digest is checked against the
# provenance rather than trusting whatever the caller points at.
provenance="programs/dclutch-claims-sbf/fixtures/token-2022-v11.provenance"
token_2022_so="${TOKEN_2022_SO:-}"
if [[ -z "$token_2022_so" ]]; then
  echo "TOKEN_2022_SO is unset. Build the audited fixture once:" >&2
  echo "  programs/dclutch-claims-sbf/fixtures/prepare-token-2022-v11.sh \\" >&2
  echo "    <spl-token-2022-11.0.0.crate> <output dir>" >&2
  echo "then point TOKEN_2022_SO at <output dir>/spl_token_2022.so" >&2
  exit 1
fi
if [[ ! -f "$token_2022_so" ]]; then
  echo "TOKEN_2022_SO does not exist: $token_2022_so" >&2
  exit 1
fi
actual_token_sha="$(shasum -a 256 "$token_2022_so" | awk '{print $1}')"
canonical_token_sha="$(awk -F= '/^canonical_elf_sha256=/{print $2}' "$provenance")"
audit_token_sha="$(awk -F= '/^macos_arm64_audit_elf_sha256=/{print $2}' "$provenance")"
if [[ "$actual_token_sha" != "$canonical_token_sha" \
   && "$actual_token_sha" != "$audit_token_sha" ]]; then
  echo "TOKEN_2022_SO is not the audited v11 fixture." >&2
  echo "  saw       $actual_token_sha" >&2
  echo "  canonical $canonical_token_sha" >&2
  echo "  macos     $audit_token_sha" >&2
  exit 1
fi
cp "$token_2022_so" "$sbf_out/spl_token_2022.so"

SBF_OUT_DIR="$sbf_out" cargo test \
  --manifest-path programs/dclutch-claims-sbf/program-test/fractional-atomic/Cargo.toml \
  --test fractional_atomic \
  -- --nocapture

# The permissioned-burn wall. It loads only Token-2022, so it is cheap, and it
# is what stops the fractional claim-check's redemption route from being
# designed around a burn no shard holder can ever perform.
SBF_OUT_DIR="$sbf_out" cargo test \
  --manifest-path programs/dclutch-claims-sbf/program-test/fractional-atomic/Cargo.toml \
  --test permissioned_burn_wall \
  -- --nocapture
