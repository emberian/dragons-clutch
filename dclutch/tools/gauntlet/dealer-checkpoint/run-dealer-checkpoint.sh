#!/usr/bin/env bash
# Run the Dealer scenario-checkpoint campaign and fold it into the census ledger.
#
# This is a ProgramTest FAST LANE. Read tools/gauntlet/dealer-checkpoint/README.md
# and the `fast_lane` block this script merges into the evidence document before
# treating any row it produces as validator evidence.
#
# usage: run-dealer-checkpoint.sh [--work DIR] [--ledger FILE] [--inventory FILE]
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
campaign_dir="$repo_root/tools/gauntlet/dealer-checkpoint"
work="/private/tmp/dclutch-dealer-checkpoint-campaign"
gauntlet_out="/private/tmp/dclutch-gauntlet/out"
ledger=""
inventory=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --work) work="${2:?--work needs a directory}"; shift 2 ;;
        --ledger) ledger="${2:?--ledger needs a file}"; shift 2 ;;
        --inventory) inventory="${2:?--inventory needs a file}"; shift 2 ;;
        *) echo "run-dealer-checkpoint.sh: unknown argument $1" >&2; exit 1 ;;
    esac
done
: "${ledger:=$gauntlet_out/ledger.json}"
: "${inventory:=$gauntlet_out/inventory.json}"

if [ ! -f "$inventory" ]; then
    echo "run-dealer-checkpoint.sh: no inventory at $inventory." >&2
    echo "  Run 'tools/gauntlet/run.sh --mode census' first; it takes seconds and needs no chain." >&2
    exit 1
fi
command -v jq >/dev/null 2>&1 || { echo "run-dealer-checkpoint.sh: jq is required" >&2; exit 1; }

sbf_out="$work/sbf"
evidence_dir="$work/evidence"
folded="$work/folded.json"
evidence="$work/evidence.json"
mkdir -p "$sbf_out"
# A campaign re-run must not accumulate records from a previous shape.
rm -rf "$evidence_dir"

cd "$repo_root"

# THE CAMPAIGN GETS ITS OWN TARGET DIRECTORY, gate included.
#
# What the gate reads is the shared SOURCE tree; which target directory it
# compiles into has nothing to do with that. Sharing one only buys a queue
# behind the interactive lanes, and on 2026-09-01 that queue cost more than the
# cold build it was avoiding: a run sat at five of six SBF builds for forty
# minutes without advancing, and then its replacement sat ten more in the gate
# itself -- both indistinguishable from a hang.
export CARGO_TARGET_DIR="$work/target"

# THE COMPILE GATE, AND IT IS NOT BELT-AND-BRACES.
#
# dClutch is developed in one shared working tree with several lanes editing at
# once, and `cargo build-sbf` will happily start against a half-applied refactor
# in a file this campaign does not own. Three runs died that way on 2026-09-01,
# each after ten minutes of building, and the failure looked like a campaign
# defect rather than a scheduling one. Check first, and say so out loud.
for attempt in $(seq 1 40); do
    if cargo check -p dclutch-trading-sbf --lib -q 2>/dev/null; then
        echo "gate: tree compiles (attempt $attempt, HEAD $(git rev-parse --short HEAD))"
        break
    fi
    echo "gate: the shared tree does not compile; waiting (attempt $attempt)"
    [ "$attempt" = 40 ] && { echo "run-dealer-checkpoint.sh: gave up waiting for a compiling tree" >&2; exit 1; }
    sleep 45
done

# `cargo build-sbf` exits ZERO when the SBF backend reports that a call
# overwrites its own stack frame. An artifact the toolchain calls
# potentially-undefined has no business entering a campaign unnoticed.
diagnostics=0
for manifest in \
    programs/dclutch-dealer-accelerator-sbf/Cargo.toml \
    programs/dclutch-dealer-accelerator-sbf/test-programs/dealer-caller/Cargo.toml \
    programs/dclutch-trading-sbf/Cargo.toml \
    programs/dclutch-custody-sbf/Cargo.toml \
    programs/dclutch-claims-sbf/Cargo.toml \
    programs/dclutch-core-sbf/Cargo.toml
do
    log="$work/build-$(basename "$(dirname "$manifest")").log"
    cargo build-sbf --manifest-path "$manifest" --sbf-out-dir "$sbf_out" > "$log" 2>&1 \
        || { tail -n 40 "$log" >&2; exit 1; }
    count="$(grep -c 'overwrites values in the frame' "$log" || true)"
    printf '  %s (%s frame diagnostics)\n' "$(basename "$(dirname "$manifest")")" "${count:-0}"
    diagnostics=$((diagnostics + count))
done
if [ "$diagnostics" -ne 0 ]; then
    echo "run-dealer-checkpoint.sh: refusing to run a campaign on artifacts the toolchain calls potentially-undefined" >&2
    exit 1
fi

echo "elf digests:"
shasum -a 256 "$sbf_out"/*.so | sed 's/^/  /'

SBF_OUT_DIR="$sbf_out" DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$evidence_dir" \
    cargo test \
    --manifest-path programs/dclutch-dealer-accelerator-sbf/program-test/Cargo.toml \
    --test accepted -- --test-threads=1

cargo run --quiet -p dclutch-program-test-evidence \
    --bin fold-program-test-evidence -- "$evidence_dir" "$folded"

# The fast-lane clauses ride the evidence document beside the numbers they
# qualify: a fast-lane claim asserted in aggregate is unfalsifiable.
jq -s '.[0] + .[1]' "$folded" "$campaign_dir/fast-lane.json" > "$evidence"

"$repo_root/tools/gauntlet/tier1/check-witnesses.sh" \
    "$campaign_dir/witnesses.json" "$evidence" "$campaign_dir/programs.json"

# `census observe` is a read-modify-write of one shared file and family lanes run
# concurrently, so this takes the same mkdir lock run.sh uses rather than racing.
lock="$ledger.lock"
waited=0
until mkdir "$lock" 2>/dev/null; do
    if [ "$waited" -ge 300 ]; then
        echo "run-dealer-checkpoint.sh: breaking a ledger lock older than 300s at $lock" >&2
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
echo "dealer-checkpoint: folded into $ledger"
echo "dealer-checkpoint: render the report with 'tools/gauntlet/run.sh --mode census'"
