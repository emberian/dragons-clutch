#!/usr/bin/env bash
# Run the tier-2 Series occurrence campaign and fold it into the census ledger.
#
# This is a ProgramTest FAST LANE. Read tools/gauntlet/tier2/README.md for which
# of the four TIERS.md fast-lane conditions it satisfies before treating any row
# it produces as validator evidence.
#
# usage: run-campaign.sh [--work DIR] [--ledger FILE] [--inventory FILE]
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
tier_dir="$repo_root/tools/gauntlet/tier2"
work="/private/tmp/dclutch-tier2"
gauntlet_out="/private/tmp/dclutch-gauntlet/out"
ledger=""
inventory=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --work) work="${2:?--work needs a directory}"; shift 2 ;;
        --ledger) ledger="${2:?--ledger needs a file}"; shift 2 ;;
        --inventory) inventory="${2:?--inventory needs a file}"; shift 2 ;;
        *) echo "run-campaign.sh: unknown argument $1" >&2; exit 1 ;;
    esac
done
: "${ledger:=$gauntlet_out/ledger.json}"
: "${inventory:=$gauntlet_out/inventory.json}"

if [ ! -f "$inventory" ]; then
    echo "run-campaign.sh: no inventory at $inventory." >&2
    echo "  Run 'tools/gauntlet/run.sh --mode census' first; it takes seconds and needs no chain." >&2
    exit 1
fi

sbf_out="$work/sbf"
evidence_dir="$work/evidence"
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

for manifest in \
    programs/dclutch-registry-sbf/Cargo.toml \
    programs/dclutch-rent-sbf/Cargo.toml \
    programs/dclutch-custody-sbf/Cargo.toml \
    programs/dclutch-core-sbf/test-programs/series-consume-caller/Cargo.toml \
    programs/dclutch-core-sbf/Cargo.toml
do
    build "$manifest"
done

SBF_OUT_DIR="$sbf_out" DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$evidence_dir" \
    cargo test \
    --manifest-path programs/dclutch-core-sbf/Cargo.toml \
    --test found_program_test \
    series_consume

cargo run --quiet -p dclutch-program-test-evidence \
    --bin fold-program-test-evidence -- "$evidence_dir" "$evidence"

# The witness evaluator takes three files; this tier has no bootstrap plan, so
# it is handed the program map as the third. Every tier-2 witness is
# `evidence-jq` and none reads it -- the argument exists only because the
# evaluator requires the path to be present.
"$repo_root/tools/gauntlet/tier1/check-witnesses.sh" \
    "$tier_dir/witnesses.json" "$evidence" "$tier_dir/programs.json"

cargo run --quiet --manifest-path tools/gauntlet/census/Cargo.toml -- observe \
    --inventory "$inventory" \
    --ledger "$ledger" \
    --bindings "$tier_dir/bindings.json" \
    --programs "$tier_dir/programs.json" \
    --evidence "$evidence"

echo
echo "tier2: folded into $ledger"
echo "tier2: render the report with 'tools/gauntlet/run.sh --mode census'"
