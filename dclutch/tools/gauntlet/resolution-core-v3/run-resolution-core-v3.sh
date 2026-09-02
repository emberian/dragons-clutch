#!/usr/bin/env bash
# Run the Resolution core-v3 lifecycle ProgramTest campaign and fold it into the
# shared census ledger.
#
# This is a ProgramTest FAST LANE. Read TIERS.md's fast-lane bar before treating
# any row it produces as validator evidence: nothing here deploys through
# Loader-v3 and ProgramTest has no finalized commitment. The Pyth receiver and
# router are the provenance-pinned local-validator projection of captured
# artifacts, and every price body is a fixture. The honest sentence about the
# strongest row is "the bank executed the funding lifecycle against the real
# ELFs", never "the protocol resolved a market on chain".
#
# What it records: the Core-composed CreateFund that reaches Resolution at depth
# two, the V7 direct activation and its idempotent replay, the V7 direct
# CloseFund, and both halves of the permissionless provider-abandon reclaim --
# the early refusal and the stranger's successful reclaim. The campaign submits
# well over fifty transactions; only the six that
# tools/gauntlet/resolution-core-v3/bindings.json names are recorded, because a
# campaign that labelled every transaction it happens to send would be claiming
# coverage no binding was written for.
#
# usage: run-resolution-core-v3.sh [--work DIR] [--ledger FILE] [--inventory FILE]
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
gauntlet_dir="$repo_root/tools/gauntlet"
tier_dir="$gauntlet_dir/resolution-core-v3"
work="/private/tmp/dclutch-resolution-core-v3-campaign"
gauntlet_out="/private/tmp/dclutch-gauntlet/out"
ledger=""
inventory=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --work) work="${2:?--work needs a directory}"; shift 2 ;;
        --ledger) ledger="${2:?--ledger needs a file}"; shift 2 ;;
        --inventory) inventory="${2:?--inventory needs a file}"; shift 2 ;;
        *) echo "run-resolution-core-v3.sh: unknown argument $1" >&2; exit 1 ;;
    esac
done
: "${ledger:=$gauntlet_out/ledger.json}"
: "${inventory:=$gauntlet_out/inventory.json}"

if [ ! -f "$inventory" ]; then
    echo "run-resolution-core-v3.sh: no inventory at $inventory." >&2
    echo "  Run 'tools/gauntlet/run.sh --mode census' first; it takes seconds and needs no chain." >&2
    exit 1
fi

sbf_out="$work/sbf"
mkdir -p "$sbf_out"
cd "$repo_root"

build() {
    if command -v swarm-build >/dev/null 2>&1; then
        swarm-build cargo build-sbf --manifest-path "$1" --sbf-out-dir "$sbf_out" 2>&1
    else
        cargo build-sbf --manifest-path "$1" --sbf-out-dir "$sbf_out" 2>&1
    fi
}

# `cargo build-sbf` exits zero even when the SBF backend reports that a call
# overwrites its own stack frame. That is not hypothetical here: the deadline
# walk arrived with NINE such diagnostics against
# `process_commit_deadline_failure` and built green anyway. An artifact the
# toolchain calls potentially-undefined has no business entering a campaign.
diagnostics=0
for manifest in \
    programs/dclutch-core-sbf/Cargo.toml \
    programs/dclutch-custody-sbf/Cargo.toml \
    programs/dclutch-registry-sbf/Cargo.toml \
    programs/dclutch-resolution-proof-sbf/Cargo.toml
do
    log="$work/build-$(basename "$(dirname "$manifest")").log"
    build "$manifest" > "$log" || { tail -n 40 "$log" >&2; exit 1; }
    count="$(grep -c 'overwrites values in the frame' "$log" || true)"
    diagnostics=$((diagnostics + count))
    printf '  built %-60s %s frame diagnostics\n' "$manifest" "${count:-0}"
done
if [ "$diagnostics" -ne 0 ]; then
    echo "resolution-core-v3: $diagnostics SBF stack-frame-overwrite diagnostics; refusing to run a" >&2
    echo "  campaign on artifacts the toolchain calls potentially-undefined." >&2
    exit 1
fi

evidence_dir="$work/evidence"
rm -rf "$evidence_dir"
mkdir -p "$evidence_dir"
echo "campaign: resolution-core-v3 (resolution_core_v3_lifecycle)"
SBF_OUT_DIR="$sbf_out" DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$evidence_dir" \
    cargo test --manifest-path crates/dclutch-svm-harness/Cargo.toml \
    --test resolution_core_v3_lifecycle -- --test-threads=1

evidence="$work/resolution-core-v3.evidence.json"
cargo run --quiet -p dclutch-program-test-evidence \
    --bin fold-program-test-evidence -- "$evidence_dir" "$evidence"

"$gauntlet_dir/tier1/check-witnesses.sh" \
    "$tier_dir/witnesses.json" "$evidence" "$tier_dir/programs.json"

cargo run --quiet --manifest-path tools/gauntlet/census/Cargo.toml -- observe \
    --inventory "$inventory" \
    --ledger "$ledger" \
    --bindings "$tier_dir/bindings.json" \
    --programs "$tier_dir/programs.json" \
    --evidence "$evidence"

echo
echo "resolution-core-v3: folded into $ledger"
echo "resolution-core-v3: render the report with 'tools/gauntlet/run.sh --mode census'"
