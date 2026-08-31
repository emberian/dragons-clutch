#!/usr/bin/env bash
# `DCLTDBR1` executed on a real bank, on the five REAL role ELFs.
#
# `direct_begin_retiring_v1` was the only Direct top-level route with no
# on-chain execution test anywhere: twelve unit tests across three files, none
# of which called `process_direct_begin_retiring_v1`. This runner is the gate
# that keeps that closed.
#
# It builds all five roles rather than the one under examination, and that is
# not thoroughness for its own sake: `add_release_waist` binds a release SET,
# whose identity hashes all five ELF digests, and the activation cache the route
# reads is a complete five-role projection. A substituted role would produce a
# different release-set identity, a different activation address, and therefore
# a different root PDA -- a fixture measuring a market nobody deploys.
#
# Usage: run-begin-retiring.sh [<elf-dir>]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
ELF_DIR="${1:-$ROOT/target/begin-retiring-elves}"
OUT="$ROOT/target/begin-retiring.log"

mkdir -p "$ELF_DIR" "$ROOT/target"
cd "$ROOT"

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"

# Niced because five SBF builds have taken this machine into swap-lock before.
# `swarm-build` caps memory where it exists; this is the floor under it.
nice_command=(nice -n 10)

cargo_command=("${nice_command[@]}" cargo)
if command -v swarm-build >/dev/null 2>&1; then
  cargo_command=("${nice_command[@]}" swarm-build cargo)
fi

for package in dclutch-registry-sbf dclutch-trading-sbf dclutch-core-sbf \
               dclutch-claims-sbf dclutch-custody-sbf; do
    echo "=== build $package ===" >&2
    build_log="$ROOT/target/begin-retiring-build-$package.log"
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
    # This gate is the reason this route needs a runner of its own rather than
    # a line in somebody else's. `process_direct_begin_retiring_v1` is the
    # symbol that HIT this wall: the activation-cache conversion left LLVM one
    # call site for `reauthenticate_roles`, it inlined it, and the caller went
    # to exactly 4,096 of the 4,096 bytes an SBPF v0 frame gets -- 43
    # diagnostics. Two `#[inline(never)]` attributes are the whole of the fix,
    # and an attribute is a request the type system does not hold.
    count="$(grep -c 'overwrites values in the frame' "$build_log" || true)"
    printf '  %-26s %s frame diagnostics\n' "$package" "${count:-0}" >&2
    if [ "${count:-0}" != "0" ]; then
        grep 'overwrites values in the frame' "$build_log" | sort -u >&2
        echo "run-begin-retiring.sh: refusing $package -- the toolchain says these calls may" \
             "cause undefined behavior during execution. Fix the frame; do not measure on" \
             "top of it." >&2
        exit 1
    fi
done

echo "=== the route ===" >&2
SBF_OUT_DIR="$ELF_DIR" \
    "${cargo_command[@]}" test \
    --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
    --test direct_begin_retiring_on_chain -- --nocapture --test-threads=1 \
    2>&1 | tee "$OUT" | grep -E "BEGINRETIRING|test result|panicked|assertion|error"

echo "--- full log: $OUT" >&2
