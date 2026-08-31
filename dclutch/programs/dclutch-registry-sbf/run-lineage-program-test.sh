#!/usr/bin/env bash
# Every real-ELF program test this crate owns, against a freshly built Registry.
#
# THE TEST LIST IS DISCOVERED, NOT WRITTEN DOWN, for the reason
# `programs/dclutch-core-sbf/run-open-market-program-test.sh` states at length:
# a hand-maintained target list left two of that crate's five targets run by
# nothing at all for days, and an unrun gate is not a passing gate. The crate
# declares no `[[test]]` table, so Cargo's own autodiscovery rule makes exactly
# `tests/*.rs` the integration targets and this glob agrees with it by
# construction.
#
# The ELF list IS short and stays explicit: `DeclareSuccessor` reads two
# activation caches and nothing else -- it observes no deployment and CPIs only
# the System program -- so the Registry's own link is the whole substrate.
set -euo pipefail

# Resolved from this script's own location rather than from `git rev-parse`.
# This tree is vendored as a `dclutch/` subtree inside another repository, where
# the toplevel is one directory up and every path would silently lose its
# `dclutch/` segment.
workspace="$(cd "$(dirname "$0")/../.." && pwd)"
output="$(mktemp -d)"
# Not EXIT alone: a killed run leaks gigabytes per invocation, and /tmp reached
# 373 GB and filled the volume once.
trap 'rm -rf "$output"' EXIT HUP INT TERM

log="$output/build-registry.log"
(cd "$workspace" && cargo build-sbf \
  --manifest-path programs/dclutch-registry-sbf/Cargo.toml \
  --sbf-out-dir "$output") >"$log" 2>&1 || { tail -n 60 "$log" >&2; exit 1; }

# An SBF stack-frame overwrite is a silent miscompile, not a warning: the
# affected frame's locals are clobbered at runtime and the program misbehaves
# in ways no assertion here would attribute to the build. Refuse rather than
# test something the compiler already said it could not lay out.
diagnostics="$(grep -c 'overwrites values in the frame' "$log" || true)"
if [ "$diagnostics" -ne 0 ]; then
  echo "lineage: refusing -- $diagnostics SBF stack-frame-overwrite diagnostics" >&2
  grep 'overwrites values in the frame' "$log" >&2
  exit 1
fi

tests="$workspace/programs/dclutch-registry-sbf/tests"
found=0
for target in "$tests"/*.rs; do
  [ -e "$target" ] || break
  name="$(basename "$target" .rs)"
  found=$((found + 1))
  echo "== $name"
  SBF_OUT_DIR="$output" cargo test \
    --manifest-path "$workspace/programs/dclutch-registry-sbf/Cargo.toml" \
    --test "$name" -- --nocapture
done

if [ "$found" = 0 ]; then
  echo "no integration test target under $tests -- this script proved nothing" >&2
  exit 1
fi
echo "== $found Registry program-test targets ran"
