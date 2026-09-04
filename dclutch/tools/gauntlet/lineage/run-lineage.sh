#!/usr/bin/env bash
# Run the successor-declaration loopback campaign and fold it into the census.
#
# This is a LOCAL VALIDATOR campaign. `tools/lineage-loopback/run-lineage-loopback.sh`
# stages two activation caches and the Registry into a genesis, starts a real
# `solana-test-validator`, and drives `DCLRLND1` through preflight, execute and
# replay. That script is the producer and it is not forked here: this wrapper
# hands it a directory to keep its evidence in, then checks witnesses and calls
# `census observe` on the document it wrote.
#
# WHY IT EXISTS. The run has been landing a real validator transaction through
# `registry/lineage_v1::process` for as long as it has existed, and the route
# read NEVER-EXECUTED the whole time, because the run swept its work directory
# -- evidence and all -- on the way out. Same shape as the two ABORT-WITNESS
# found on 2026-09-03: a green driver with no producer. The delta is a flag, a
# `transactions` array in the document the caller already wrote, and a bindings
# file.
#
# usage: run-lineage.sh [--work DIR] [--ledger FILE] [--inventory FILE]
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
gauntlet_dir="$repo_root/tools/gauntlet"
tier_dir="$gauntlet_dir/lineage"
work="/private/tmp/dclutch-lineage-campaign"
gauntlet_out="/private/tmp/dclutch-gauntlet/out"
ledger=""
inventory=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --work) work="${2:?--work needs a directory}"; shift 2 ;;
        --ledger) ledger="${2:?--ledger needs a file}"; shift 2 ;;
        --inventory) inventory="${2:?--inventory needs a file}"; shift 2 ;;
        *) echo "run-lineage.sh: unknown argument $1" >&2; exit 1 ;;
    esac
done
: "${ledger:=$gauntlet_out/ledger.json}"
: "${inventory:=$gauntlet_out/inventory.json}"

if [ ! -f "$inventory" ]; then
    echo "run-lineage.sh: no inventory at $inventory." >&2
    echo "  Run 'tools/gauntlet/run.sh --mode census' first; it takes seconds and needs no chain." >&2
    exit 1
fi

evidence_dir="$work/evidence"
rm -rf "$evidence_dir"
mkdir -p "$evidence_dir"

echo "campaign: lineage-loopback (DCLRLND1 on a localhost validator)"
"$repo_root/tools/lineage-loopback/run-lineage-loopback.sh" --evidence-dir "$evidence_dir"

evidence="$evidence_dir/execute.json"
[ -f "$evidence" ] || { echo "run-lineage.sh: the loopback kept no execute.json" >&2; exit 1; }

"$gauntlet_dir/tier1/check-witnesses.sh" \
    "$tier_dir/witnesses.json" "$evidence" "$tier_dir/programs.json"

# The Registry address is fixed by the stager's own REGISTRY_SEED, so the
# program map is a constant rather than a per-run capture -- but a genesis that
# moved would make every observation refuse rather than silently bind the wrong
# program, because the census reads the invoked address out of the chain's logs.
observed_registry="$(jq -r '[.transactions[] | .logs[] | select(test("^Program [1-9A-HJ-NP-Za-km-z]+ invoke \\[[0-9]+\\]$")) | split(" ")[1]] | unique - ["ComputeBudget111111111111111111111111111111","11111111111111111111111111111111"] | first' "$evidence")"
declared_registry="$(jq -r '.registry' "$tier_dir/programs.json")"
if [ "$observed_registry" != "$declared_registry" ]; then
    echo "run-lineage.sh: the loopback genesis put the Registry at $observed_registry," >&2
    echo "  and $tier_dir/programs.json declares $declared_registry." >&2
    echo "  Update the map, or find out why the stager's REGISTRY_SEED stopped being fixed." >&2
    exit 1
fi

cargo run --quiet --manifest-path "$gauntlet_dir/census/Cargo.toml" -- observe \
    --inventory "$inventory" \
    --ledger "$ledger" \
    --bindings "$tier_dir/bindings.json" \
    --programs "$tier_dir/programs.json" \
    --evidence "$evidence"

echo
echo "lineage-loopback: folded into $ledger"
echo "lineage-loopback: render the report with 'tools/gauntlet/run.sh --mode census'"
