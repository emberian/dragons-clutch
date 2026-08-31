#!/usr/bin/env bash
# The fee-bearing Direct trade as a PAIR, on the five REAL role ELFs.
#
# `run-fee-second-transaction.sh` beside this one answers a question about
# CUSTODY, and buys its answer by deploying a stand-in program in Trading's
# slot. This runner answers the question about TRADING, so it substitutes
# nothing: the Trading ELF it builds is the one that carries the `DCLTDFS1`
# settlement route, and every other role is real too.
#
# Usage: run-fee-pair.sh [<elf-dir>]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
ELF_DIR="${1:-$ROOT/target/fee-pair-elves}"
OUT="$ROOT/target/fee-pair.log"

mkdir -p "$ELF_DIR" "$ROOT/target"
cd "$ROOT"

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"

# Niced because these five SBF builds have taken the machine into swap-lock
# before. `swarm-build` caps memory where it exists; this is the floor under it.
nice_command=(nice -n 10)

cargo_command=("${nice_command[@]}" cargo)
if command -v swarm-build >/dev/null 2>&1; then
  cargo_command=("${nice_command[@]}" swarm-build cargo)
fi

for package in dclutch-registry-sbf dclutch-trading-sbf dclutch-core-sbf \
               dclutch-claims-sbf dclutch-custody-sbf; do
    echo "=== build $package ===" >&2
    build_log="$ROOT/target/fee-pair-build-$package.log"
    "${cargo_command[@]}" build-sbf --manifest-path "programs/$package/Cargo.toml" \
        --sbf-out-dir "$ELF_DIR" >"$build_log" 2>&1 || {
        tail -n 40 "$build_log" >&2
        echo "BUILD FAILED: $package" >&2
        exit 1
    }
    # `cargo build-sbf` exits ZERO when the SBF backend reports that a call
    # overwrites its own stack frame, so a build that only checked the exit
    # status would report success on undefined behaviour. Read the log.
    #
    # This runner briefly carried ONE named exception:
    # `direct_begin_retiring_v1::process_direct_begin_retiring_v1` emitted 43
    # diagnostics on `lane/fee-core-20260830`, and a control build of that
    # branch confirmed they predated this lane. Main fixed them between
    # `613fa5e1` and `59ecec5f`, so the exception is gone rather than carried:
    # an exception nothing needs is a hole waiting for the next symbol.
    count="$(grep -c 'overwrites values in the frame' "$build_log" || true)"
    printf '  %-26s %s frame diagnostics\n' "$package" "${count:-0}" >&2
    if [ "${count:-0}" != "0" ]; then
        grep 'overwrites values in the frame' "$build_log" | sort -u >&2
        echo "run-fee-pair.sh: refusing $package -- the toolchain says these calls may cause" \
             "undefined behavior during execution. Fix the frame; do not measure on top of it." >&2
        exit 1
    fi
done

echo "=== the pair ===" >&2
SBF_OUT_DIR="$ELF_DIR" \
    "${cargo_command[@]}" test \
    --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
    --test direct_hot_fee_pair -- --nocapture --test-threads=1 \
    2>&1 | tee "$OUT" | grep -E "FEEPAIR|test result|panicked|assertion|error"

echo "--- full log: $OUT" >&2
