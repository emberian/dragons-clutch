#!/usr/bin/env bash
# Real-ELF evidence for the family-neutral Trading activation outer.
#
# The suite had no runner, which is why it was possible for the seam to create
# an all-zero family root for months without anyone meeting a red test.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
sbf_out="$(mktemp -d /tmp/dclutch-trading-outer.XXXXXX)"

cleanup() {
  rm -rf -- "$sbf_out"
}
trap cleanup EXIT HUP INT TERM

cd "$repo_root"

# ---------------------------------------------------------------------------
# The accelerator link, and why a Trading-seam runner builds it.
#
# `cargo build-sbf` exits ZERO when the SBF backend reports that a call
# overwrites its own stack frame and "may cause undefined behavior during
# execution", so a link nobody builds is a link nobody is told about.
#
# Every other frame gate in this tree -- tools/gauntlet/run.sh, run-journey.sh,
# run-dealer.sh -- builds the seven ROLE programs. Trading at default features
# is one of them and it is not where this class bites: the accelerators link
# `dclutch-trading-sbf` with `default-features = false` and their own feature
# set, and that is a DIFFERENT monomorphization of the same hot path, with
# different inlining and therefore different frames. On 2026-08-27 the dealer
# accelerator carried 82 frame diagnostics on
# `hot_v3::execute_child_routes_v3` -- 5,184 bytes against a 4,096-byte bound --
# while Trading's own ELF reported zero, and it survived a whole wave because
# the only gate that built that link was the checked-release candidate, which
# runs at release time. It went red there, correctly, with a devnet deploy
# already in flight.
#
# So: the seam that owns hot_v3 builds the links that carry hot_v3, every run,
# and refuses on a nonzero count. It costs one incremental SBF build.
# ---------------------------------------------------------------------------
diagnostics=0
for manifest in \
  programs/dclutch-dealer-accelerator-sbf/Cargo.toml \
  programs/dclutch-general-accelerator-sbf/Cargo.toml
do
  link="$(basename "$(dirname "$manifest")")"
  log="$sbf_out/build-$link.log"
  cargo build-sbf --manifest-path "$manifest" --sbf-out-dir "$sbf_out" \
    > "$log" 2>&1 || { tail -n 40 "$log" >&2; exit 1; }
  count="$(grep -c 'overwrites values in the frame' "$log" || true)"
  printf '  %s (%s frame diagnostics)\n' "$link" "${count:-0}"
  if [ "${count:-0}" != "0" ]; then
    grep 'overwrites values in the frame' "$log" | sort -u >&2
  fi
  diagnostics=$((diagnostics + count))
done
if [ "$diagnostics" -ne 0 ]; then
  echo "run-program-test.sh: refusing -- $diagnostics SBF stack-frame-overwrite" \
       "diagnostics on an accelerator link. The toolchain says these calls may" \
       "cause undefined behavior during execution; fix the frame, do not" \
       "measure on top of it." >&2
  exit 1
fi

for manifest in \
  programs/dclutch-trading-sbf/program-test/test-programs/trading-outer/Cargo.toml \
  programs/dclutch-trading-sbf/program-test/test-programs/core-caller/Cargo.toml \
  programs/dclutch-trading-sbf/program-test/test-programs/registry/Cargo.toml
do
  cargo build-sbf --manifest-path "$manifest" --sbf-out-dir "$sbf_out"
done

SBF_OUT_DIR="$sbf_out" cargo test \
  --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
  --test activation \
  -- --nocapture
