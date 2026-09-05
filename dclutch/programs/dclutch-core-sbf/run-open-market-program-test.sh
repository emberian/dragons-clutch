#!/usr/bin/env bash
# Every real-ELF program test this crate owns, against freshly built links.
#
# THE TEST LIST IS DISCOVERED, NOT WRITTEN DOWN. Until 2026-08-30 this script
# named three targets while `tests/` held five, so `capability_close_alias`
# and `retirement_replay_handoff` were run by nothing at all -- not here, not
# by `tools/gate`, not by the gauntlet. A hand-maintained list is exactly
# the value-duplicated-instead-of-read defect that leaves a target unrun for
# as long as nobody counts the directory. An unrun gate is not a passing gate:
# `67e96e5b` found `open_market_program_test` submitting a frame that had been
# one account short since `2dc53776` four days earlier, with four hostile
# assertions "passing" on a length refusal none of them was about, and it was
# effectively unrun in CI that whole time. So the loop below globs `tests/`;
# adding a target is enough to have it run, and there is nothing to forget.
#
# The ELF list is NOT discoverable and stays explicit. `capability_close_alias`
# needs the real Trading link (`dclutch_trading_sbf.so`) for the Core-to-Trading
# native close; `retirement_replay_handoff` needs Custody; the other three need
# Registry, Rent, Custody and the series-consume caller. Building the union
# once is cheaper than five per-target builds and is what SBF_OUT_DIR means.
set -euo pipefail

workspace="$(cd "$(dirname "$0")/../.." && pwd)"
output="$(mktemp -d)"
# Not EXIT alone: a killed run leaks 3-7 GB per invocation, and /tmp/dclutch-*
# reached 373 GB and filled the volume once.
trap 'rm -rf "$output"' EXIT HUP INT TERM

log="$output/build-core-links.log"
for manifest in \
  programs/dclutch-registry-sbf/Cargo.toml \
  programs/dclutch-rent-sbf/Cargo.toml \
  programs/dclutch-claims-sbf/Cargo.toml \
  programs/dclutch-custody-sbf/Cargo.toml \
  programs/dclutch-resolution-proof-sbf/Cargo.toml \
  programs/dclutch-trading-sbf/Cargo.toml \
  programs/dclutch-core-sbf/test-programs/series-consume-caller/Cargo.toml \
  programs/dclutch-core-sbf/Cargo.toml; do
  (cd "$workspace" && cargo build-sbf --manifest-path "$manifest" --sbf-out-dir "$output") \
    >>"$log" 2>&1 || { tail -n 60 "$log" >&2; exit 1; }
done

# An SBF stack-frame overwrite is a silent miscompile, not a warning: the
# affected frame's locals are clobbered at runtime and the program misbehaves
# in ways no assertion here would attribute to the build. Refuse rather than
# test something the compiler already said it could not lay out. The `suites`
# tier does not check this itself (the gap named in
# docs/evidence/SLIPPED_THROUGH_SWEEP_2026_08_30.md:98), so it has to live in
# the runner -- and this crate's links now carry the succession ceremony, whose
# 21-account frame is exactly the shape that would provoke one.
diagnostics="$(grep -c 'overwrites values in the frame' "$log" || true)"
if [ "$diagnostics" -ne 0 ]; then
  echo "core: refusing -- $diagnostics SBF stack-frame-overwrite diagnostics" >&2
  grep 'overwrites values in the frame' "$log" >&2
  exit 1
fi

# `tests/*.rs` is not a guess at the target list, it is Cargo's own rule: the
# crate declares no `[[test]]` table, so autodiscovery makes exactly these
# files the integration targets. If a `[[test]]` table is ever added, this
# glob stops agreeing with Cargo and must be replaced by reading that table.
tests="$workspace/programs/dclutch-core-sbf/tests"
found=0
for target in "$tests"/*.rs; do
  [ -e "$target" ] || break
  name="$(basename "$target" .rs)"
  found=$((found + 1))
  echo "== $name"
  SBF_OUT_DIR="$output" cargo test \
    --manifest-path "$workspace/programs/dclutch-core-sbf/Cargo.toml" \
    --test "$name" -- --nocapture
done

if [ "$found" = 0 ]; then
  echo "no integration test target under $tests -- this script proved nothing" >&2
  exit 1
fi
echo "== $found core program-test targets ran"
