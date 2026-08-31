#!/usr/bin/env bash
# CACHEREAD's measurement harness: build the five role ELFs, run the top-level
# Direct margin gate's 32-seed sweep, and print the KEY-INDEPENDENT FLOOR.
#
# The floor -- not the worst seed -- is the statistic, for the reason
# `direct_hot_top_level_margin_gate.rs` gives at length: converting a CPI
# changes the five ELF digests, which redraws `release_set_id`, which redraws
# every bump search on the route. Two worst-seed figures across that boundary
# are not comparable. `min over seeds of (CU(seed) - 1500 * T_known(seed))` is.
#
# Usage: cacheread-floor.sh <label> [<elf-dir>]
set -euo pipefail

LABEL="${1:?usage: cacheread-floor.sh <label> [<elf-dir>]}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
ELF_DIR="${2:-$ROOT/target/cacheread-elves-$LABEL}"
OUT="$ROOT/target/cacheread-$LABEL.log"

mkdir -p "$ELF_DIR"
cd "$ROOT"

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"

for package in dclutch-registry-sbf dclutch-trading-sbf dclutch-core-sbf \
               dclutch-claims-sbf dclutch-custody-sbf; do
    echo "=== build $package ===" >&2
    build_log="$ROOT/target/cacheread-build-$LABEL-$package.log"
    cargo build-sbf --manifest-path "programs/$package/Cargo.toml" \
        --sbf-out-dir "$ELF_DIR" >"$build_log" 2>&1 || {
        tail -n 40 "$build_log" >&2
        echo "BUILD FAILED: $package" >&2
        exit 1
    }
    # `cargo build-sbf` exits ZERO when the SBF backend reports a frame
    # diagnostic. The lane rule is zero frame diagnostics, so read the log.
    if grep -qi "stack offset of\|exceeded max\|Error: Function .* Stack" "$build_log"; then
        echo "SBF FRAME DIAGNOSTIC in $package:" >&2
        grep -i "stack offset of\|exceeded max\|Error: Function .* Stack" "$build_log" >&2
        exit 1
    fi
done

echo "=== sweep ($LABEL) ===" >&2
SBF_OUT_DIR="$ELF_DIR" \
    cargo test --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
    --test direct_hot_top_level_margin_gate -- --nocapture --test-threads=1 \
    2>&1 | tee "$OUT" | grep -E "KEY-INDEPENDENT FLOOR|ANALYTIC WORST|SWEPT|test result|panicked|assertion"

echo "--- full log: $OUT"
