#!/usr/bin/env bash
# The C-04 fee-completion pair as a CENSUS CAMPAIGN, on five real role ELFs.
#
# This is `programs/dclutch-trading-sbf/program-test/run-fee-pair.sh` with one
# thing added and nothing else changed: `DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR`, so
# the suite that has been executing these two Trading routes for days finally
# emits observations the census can read. Both routes were NEVER-EXECUTED to the
# ledger the whole time -- see `docs/evidence/UNWITNESSED_ROUTES_BY_ROW_2026_09_01.md`
# -- and the gap was this file, never the routes.
#
# Usage: run-direct-fee-pair.sh [<elf-dir>]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
ELF_DIR="${1:-$ROOT/target/direct-fee-pair-elves}"
EVIDENCE="$ROOT/target/direct-fee-pair-evidence"
FOLD="$ROOT/target/direct-fee-pair-evidence.json"
OUT="$ROOT/target/direct-fee-pair.log"

mkdir -p "$ELF_DIR" "$ROOT/target"
# rsync-style: a partial directory from an interrupted run must not be reused,
# and a `test -d` guard would skip the recopy and measure the leftovers.
rm -rf "$EVIDENCE"
mkdir -p "$EVIDENCE"
cd "$ROOT"

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
nice_command=(nice -n 10)
cargo_command=("${nice_command[@]}" cargo)
if command -v swarm-build >/dev/null 2>&1; then
  cargo_command=("${nice_command[@]}" swarm-build cargo)
fi

for package in dclutch-registry-sbf dclutch-trading-sbf dclutch-core-sbf \
               dclutch-claims-sbf dclutch-custody-sbf; do
    echo "=== build $package ===" >&2
    build_log="$ROOT/target/direct-fee-pair-build-$package.log"
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

echo "=== the pair, recording census evidence ===" >&2
SBF_OUT_DIR="$ELF_DIR" \
DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$EVIDENCE" \
    "${cargo_command[@]}" test \
    --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
    --test direct_hot_fee_pair -- --test-threads=1 \
    2>&1 | tee "$OUT" | grep -E "test result|panicked|assertion|error"

echo "=== fold ===" >&2
"${nice_command[@]}" cargo run --quiet \
    --manifest-path tools/gauntlet/program-test-evidence/Cargo.toml \
    --bin fold-program-test-evidence -- "$EVIDENCE" "$FOLD"

count="$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["transactions"]))' "$FOLD")"
echo "  $count observations -> $FOLD" >&2
echo "--- full log: $OUT" >&2
