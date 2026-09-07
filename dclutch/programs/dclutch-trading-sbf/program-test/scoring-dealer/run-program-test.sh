#!/usr/bin/env bash
# Real-ELF evidence for the scoring Dealer's DealerFill route.
#
# The route reached `main` with a 4,288-byte stack frame against SBPF v0's
# 4,096 because nothing in the tree ever executed it. Against that ELF the
# accepted case below dies with
#
#   Access violation reading 8 bytes at address 0xd3 (in unallocated region)
#
# and every hostile still refuses by its correct name -- which is exactly why a
# refusal-only campaign would have stayed green. Driving the route to its
# receipt is what catches a frame that overruns.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../../.." && pwd)"
sbf_out="$(mktemp -d /tmp/dclutch-scoring-dealer.XXXXXX)"

cleanup() {
  rm -rf -- "$sbf_out"
}
trap cleanup EXIT HUP INT TERM

cd "$repo_root"

log="$sbf_out/build-trading.log"
cargo build-sbf --manifest-path programs/dclutch-trading-sbf/Cargo.toml \
  --sbf-out-dir "$sbf_out" > "$log" 2>&1 || { tail -n 40 "$log" >&2; exit 1; }
count="$(grep -Ec 'overwrites values in the frame|overflows the maximum allowed frame space' "$log" || true)"
if [ "${count:-0}" != "0" ]; then
  grep -E 'overwrites values in the frame|overflows the maximum allowed frame space' "$log" | sort -u >&2
  echo "run-program-test.sh: refusing -- ${count} SBF stack-frame-overwrite" \
       "diagnostics on the Trading link. The campaign below would measure a" \
       "route the toolchain says may execute as undefined behavior." >&2
  exit 1
fi

SBF_OUT_DIR="$sbf_out" cargo test \
  --locked \
  --manifest-path programs/dclutch-trading-sbf/program-test/scoring-dealer/Cargo.toml \
  --tests \
  -- --nocapture --test-threads=1
