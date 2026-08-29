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

token_2022_so="${TOKEN_2022_SO:-}"
if [[ -z "$token_2022_so" ]]; then
  echo "TOKEN_2022_SO is unset; run programs/dclutch-claims-sbf/fixtures/prepare-token-2022-v11.sh" >&2
  exit 1
fi
cp "$token_2022_so" "$sbf_out/spl_token_2022.so"

SBF_OUT_DIR="$sbf_out" cargo test \
  --manifest-path programs/dclutch-claims-sbf/program-test/fractional-atomic/Cargo.toml \
  --test fractional_atomic \
  -- --nocapture
