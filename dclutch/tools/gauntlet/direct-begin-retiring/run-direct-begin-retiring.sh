#!/usr/bin/env bash
# The Direct BeginRetiring route as a CENSUS CAMPAIGN, on five real role ELFs.
#
# This is `programs/dclutch-trading-sbf/program-test/run-begin-retiring.sh` with
# one thing added and nothing else changed: `DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR`,
# so a suite that has been executing `DCLTDBR1` against the real Trading ELF for
# days finally emits observations the census can read. The route was
# NEVER-EXECUTED to the ledger the whole time. The gap was this file, never the
# route -- exactly the shape `tools/gauntlet/direct-fee-pair/` was written to
# close for its own two routes, and this is the same one-line delta applied to
# the next binary along.
#
# Every submission in this binary already goes through `submit_v0_observed` ->
# `record_campaign_transaction` (programs/dclutch-trading-sbf/program-test/
# direct-hot/src/waist.rs), which emits a labelled record the moment the
# variable is set and stays silent when it is not. So nothing in the producer
# changes and the campaign cannot pick up a transaction the producer did not
# already submit.
#
# Usage: run-direct-begin-retiring.sh [<elf-dir>]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TIER="$ROOT/tools/gauntlet/direct-begin-retiring"
ELF_DIR="${1:-$ROOT/target/direct-begin-retiring-elves}"
EVIDENCE="$ROOT/target/direct-begin-retiring-evidence"
FOLD="$ROOT/target/direct-begin-retiring-evidence.json"
OUT="$ROOT/target/direct-begin-retiring.log"
GAUNTLET_OUT="/private/tmp/dclutch-gauntlet/out"
LEDGER="${DCLUTCH_GAUNTLET_LEDGER:-$GAUNTLET_OUT/ledger.json}"
INVENTORY="${DCLUTCH_GAUNTLET_INVENTORY:-$GAUNTLET_OUT/inventory.json}"

mkdir -p "$ELF_DIR" "$ROOT/target"
# A partial directory from an interrupted run must not be reused, and a `test -d`
# guard would skip the recreate and measure the leftovers.
rm -rf "$EVIDENCE"
mkdir -p "$EVIDENCE"
cd "$ROOT"

if [ ! -f "$INVENTORY" ]; then
    echo "run-direct-begin-retiring.sh: no inventory at $INVENTORY." >&2
    echo "  Run 'tools/gauntlet/run.sh --mode census' first; it takes seconds and needs no chain." >&2
    exit 1
fi

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
nice_command=(nice -n 10)
cargo_command=("${nice_command[@]}" cargo)
if command -v swarm-build >/dev/null 2>&1; then
  cargo_command=("${nice_command[@]}" swarm-build cargo)
fi

for package in dclutch-registry-sbf dclutch-trading-sbf dclutch-core-sbf \
               dclutch-claims-sbf dclutch-custody-sbf; do
    echo "=== build $package ===" >&2
    build_log="$ROOT/target/direct-begin-retiring-build-$package.log"
    "${cargo_command[@]}" build-sbf --manifest-path "programs/$package/Cargo.toml" \
        --sbf-out-dir "$ELF_DIR" >"$build_log" 2>&1 || {
        tail -n 40 "$build_log" >&2
        echo "BUILD FAILED: $package" >&2
        exit 1
    }
    # `cargo build-sbf` exits ZERO when the SBF backend reports that a call
    # overwrites its own stack frame, so a build that only checked the exit
    # status would report success on undefined behaviour. Read the log.
    count="$(grep -c 'overwrites values in the frame' "$build_log" || true)"
    printf '  %-26s %s frame diagnostics\n' "$package" "${count:-0}" >&2
    if [ "${count:-0}" != "0" ]; then
        grep 'overwrites values in the frame' "$build_log" | sort -u >&2
        echo "refusing $package -- the toolchain says these calls may cause undefined" \
             "behavior during execution. Fix the frame; do not measure on top of it." >&2
        exit 1
    fi
done

echo "=== the BeginRetiring binary, recording census evidence ===" >&2
SBF_OUT_DIR="$ELF_DIR" \
DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$EVIDENCE" \
    "${cargo_command[@]}" test \
    --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
    --test direct_begin_retiring_on_chain -- --test-threads=1 \
    2>&1 | tee "$OUT" | grep -E "test result|panicked|assertion|error"

echo "=== fold ===" >&2
"${nice_command[@]}" cargo run --quiet \
    --manifest-path tools/gauntlet/program-test-evidence/Cargo.toml \
    --bin fold-program-test-evidence -- "$EVIDENCE" "$FOLD"

count="$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["transactions"]))' "$FOLD")"
echo "  $count observations -> $FOLD" >&2

echo "=== witnesses ===" >&2
"$ROOT/tools/gauntlet/tier1/check-witnesses.sh" \
    "$TIER/witnesses.json" "$FOLD" "$TIER/programs.json"

echo "=== census ===" >&2
"${nice_command[@]}" cargo run --quiet --manifest-path tools/gauntlet/census/Cargo.toml -- observe \
    --inventory "$INVENTORY" \
    --ledger "$LEDGER" \
    --bindings "$TIER/bindings.json" \
    --programs "$TIER/programs.json" \
    --evidence "$FOLD"

echo
echo "direct-begin-retiring: folded into $LEDGER"
echo "direct-begin-retiring: render the report with 'tools/gauntlet/run.sh --mode census'"
echo "--- full log: $OUT" >&2
