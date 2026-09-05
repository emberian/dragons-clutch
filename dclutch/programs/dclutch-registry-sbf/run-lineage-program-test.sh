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

# A TARGET THAT DRIVES LEAN AND A HOST WITH NO LEAN ARE NOT A FAILING TEST.
# `release_finalization_corpus_generator_fresh` shells out to `lake` to re-emit
# the release-finalization corpus and byte-compare it, and on a host without the
# toolchain it panicked with "launch Lean build: No such file or directory".
# That is a MISSING PREREQUISITE wearing a failing gate's clothes, and it made
# the wrapper's SBF suites job red on every cut for a fact about the runner
# rather than about this tree -- exactly the confusion tools/gate's exit
# codes exist to prevent, and which that tier already honours per row.
#
# WHICH TARGETS NEED LEAN IS READ OFF THE TARGET, for the same reason the target
# list itself is discovered rather than written down: a second list here would
# be a value duplicated instead of read, and it would go stale the first time
# somebody adds an emitter check.
have_lake=0
command -v lake >/dev/null 2>&1 && have_lake=1

tests="$workspace/programs/dclutch-registry-sbf/tests"
found=0
ran=0
failed=0
deferred=""
for target in "$tests"/*.rs; do
  [ -e "$target" ] || break
  name="$(basename "$target" .rs)"
  found=$((found + 1))
  if [ "$have_lake" = 0 ] && grep -q 'Command::new("lake")' "$target"; then
    echo "== $name SKIPPED -- it drives lake, and lake is not on PATH"
    deferred="$deferred $name"
    continue
  fi
  echo "== $name"
  # EVERY ROW RUNS AND EVERY ROW IS REPORTED. Bare invocations under `set -e`
  # stop at the first failure, so the rows after it never run while the summary
  # states one number -- measured on run-postjoin-hostiles.sh, which reported
  # one failing case when the true figure was ten.
  if SBF_OUT_DIR="$output" cargo test \
    --manifest-path "$workspace/programs/dclutch-registry-sbf/Cargo.toml" \
    --test "$name" -- --nocapture; then
    ran=$((ran + 1))
  else
    echo "== $name FAILED" >&2
    failed=$((failed + 1))
  fi
done

if [ "$found" = 0 ]; then
  echo "no integration test target under $tests -- this script proved nothing" >&2
  exit 1
fi
echo "== $ran of $found Registry program-test targets ran"
if [ "$failed" -gt 0 ]; then
  echo "== $failed of $found Registry program-test targets FAILED" >&2
  exit 1
fi
if [ -n "$deferred" ]; then
  echo "== NOT RUN, this host has no Lean toolchain:$deferred" >&2
  echo "   Nothing is claimed about the checked-in corpus either way. Install" >&2
  echo "   elan/lake, or run the emission guard in the live tree, to gate it." >&2
  exit 2
fi
