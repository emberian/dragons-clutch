#!/usr/bin/env bash
# Run the General accelerator campaign and fold it into the census ledger.
#
# This is a ProgramTest FAST LANE restricted to runtime width 1. Read
# tools/gauntlet/general/README.md, and the `fast_lane` block this script merges
# into the evidence document, before treating any row it produces as validator
# evidence. The campaign exercises every action at N=1 and N=258; only the N=1
# transactions are RECORDED, because at N=258 six of the seven actions do not fit
# a Solana packet and ProgramTest is not in a position to notice.
#
# usage: run-general.sh [--work DIR] [--ledger FILE] [--inventory FILE]
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
campaign_dir="$repo_root/tools/gauntlet/general"
work="/private/tmp/dclutch-general-campaign"
gauntlet_out="/private/tmp/dclutch-gauntlet/out"
ledger=""
inventory=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --work) work="${2:?--work needs a directory}"; shift 2 ;;
        --ledger) ledger="${2:?--ledger needs a file}"; shift 2 ;;
        --inventory) inventory="${2:?--inventory needs a file}"; shift 2 ;;
        *) echo "run-general.sh: unknown argument $1" >&2; exit 1 ;;
    esac
done
: "${ledger:=$gauntlet_out/ledger.json}"
: "${inventory:=$gauntlet_out/inventory.json}"

if [ ! -f "$inventory" ]; then
    echo "run-general.sh: no inventory at $inventory." >&2
    echo "  Run 'tools/gauntlet/run.sh --mode census' first; it takes seconds and needs no chain." >&2
    exit 1
fi

command -v jq >/dev/null 2>&1 || { echo "run-general.sh: jq is required" >&2; exit 1; }

sbf_out="$work/sbf"
evidence_dir="$work/evidence"
folded="$work/folded.json"
evidence="$work/evidence.json"
mkdir -p "$sbf_out"
# A campaign re-run must not accumulate records from a previous shape.
rm -rf "$evidence_dir"

cd "$repo_root"

build() {
    if command -v swarm-build >/dev/null 2>&1; then
        swarm-build cargo build-sbf --manifest-path "$1" --sbf-out-dir "$sbf_out"
    else
        cargo build-sbf --manifest-path "$1" --sbf-out-dir "$sbf_out"
    fi
}

# `cargo build-sbf` exits zero even when the SBF backend reports that a call
# overwrites its own stack frame. An artifact the toolchain calls
# potentially-undefined has no business entering a campaign unnoticed, so the
# count is taken and a nonzero one stops the tier. The General campaign has
# claimed "zero frame diagnostics" on the board since it was written; until this
# script existed, that was a human reading build output.
diagnostics=0
for manifest in \
    programs/dclutch-general-accelerator-sbf/Cargo.toml \
    programs/dclutch-general-accelerator-sbf/test-programs/general-caller/Cargo.toml
do
    log="$work/build-$(basename "$(dirname "$manifest")").log"
    build "$manifest" > "$log" 2>&1 || { tail -n 40 "$log" >&2; exit 1; }
    count="$(grep -c 'overwrites values in the frame' "$log" || true)"
    printf '  %s (%s frame diagnostics)\n' "$(basename "$(dirname "$manifest")")" "${count:-0}"
    diagnostics=$((diagnostics + count))
done
if [ "$diagnostics" -ne 0 ]; then
    echo "run-general.sh: refusing to run a campaign on artifacts the toolchain calls potentially-undefined" >&2
    exit 1
fi

# `--locked` is deliberate: this package declares its own workspace and carries
# its own Cargo.lock, and that lock drifting from the adapter's dependencies is
# what made the campaign unrunnable from a checkout of main until 31eca2fa.
SBF_OUT_DIR="$sbf_out" DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$evidence_dir" \
    cargo test --locked \
    --manifest-path programs/dclutch-general-accelerator-sbf/program-test/Cargo.toml \
    --tests -- --nocapture

cargo run --quiet -p dclutch-program-test-evidence \
    --bin fold-program-test-evidence -- "$evidence_dir" "$folded"

# The four fast-lane clauses ride the evidence document beside the numbers they
# qualify, which is direct/'s habit and the reason TIERS.md names it the worked
# example: a fast-lane claim asserted in aggregate is unfalsifiable.
jq -s '.[0] + .[1]' "$folded" "$campaign_dir/fast-lane.json" > "$evidence"

# The witness evaluator takes three files; this tier has no bootstrap plan, so it
# is handed the program map as the third. Every witness here is `evidence-jq` and
# none reads it -- the argument exists only because the evaluator requires the
# path to be present.
"$repo_root/tools/gauntlet/tier1/check-witnesses.sh" \
    "$campaign_dir/witnesses.json" "$evidence" "$campaign_dir/programs.json"

# `census observe` is a read-modify-write of one shared file and family lanes run
# concurrently, so this one takes the same mkdir lock `run.sh` uses rather than
# racing. A lock older than five minutes is broken, on the argument that a
# campaign that has not finished by then has failed.
lock="$ledger.lock"
waited=0
until mkdir "$lock" 2>/dev/null; do
    if [ "$waited" -ge 300 ]; then
        echo "run-general.sh: breaking a ledger lock older than 300s at $lock" >&2
        rm -rf "$lock"
        continue
    fi
    sleep 2
    waited=$((waited + 2))
done
printf '%s\n' "$$" > "$lock/pid"
trap 'rm -rf "$lock"' EXIT

cargo run --quiet --manifest-path tools/gauntlet/census/Cargo.toml -- observe \
    --inventory "$inventory" \
    --ledger "$ledger" \
    --bindings "$campaign_dir/bindings.json" \
    --programs "$campaign_dir/programs.json" \
    --evidence "$evidence"

rm -rf "$lock"
trap - EXIT

echo
echo "general: folded into $ledger"
echo "general: render the report with 'tools/gauntlet/run.sh --mode census'"
