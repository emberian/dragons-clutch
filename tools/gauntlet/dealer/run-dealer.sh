#!/usr/bin/env bash
# Run the Dealer family campaign and fold it into the census ledger.
#
# This is a ProgramTest FAST LANE. Read tools/gauntlet/dealer/README.md for which
# of the TIERS.md fast-lane conditions it satisfies before treating any row it
# produces as validator evidence. Every transaction it submits is a REFUSAL; the
# tier claims no executed row and its witnesses pin that.
#
# usage: run-campaign.sh [--work DIR] [--ledger FILE] [--inventory FILE]
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
campaign_dir="$repo_root/tools/gauntlet/dealer"
work="/private/tmp/dclutch-dealer-campaign"
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

# `cargo build-sbf` exits zero even when the SBF backend reports that a call
# overwrites its own stack frame. An artifact the toolchain calls
# potentially-undefined has no business entering a campaign unnoticed, so the
# count is taken and a nonzero one stops the tier.
diagnostics=0
for manifest in \
    programs/dclutch-dealer-sbf/Cargo.toml \
    programs/dclutch-registry-sbf/Cargo.toml \
    programs/dclutch-core-sbf/Cargo.toml \
    programs/dclutch-custody-sbf/Cargo.toml
do
    log="$work/build-$(basename "$(dirname "$manifest")").log"
    build "$manifest" > "$log" 2>&1 || { tail -n 40 "$log" >&2; exit 1; }
    count="$(grep -c 'overwrites values in the frame' "$log" || true)"
    printf '  %s (%s frame diagnostics)\n' "$(basename "$(dirname "$manifest")")" "${count:-0}"
    diagnostics=$((diagnostics + count))
done
if [ "$diagnostics" -ne 0 ]; then
    echo "run-campaign.sh: refusing to run a campaign on artifacts the toolchain calls potentially-undefined" >&2
    exit 1
fi

SBF_OUT_DIR="$sbf_out" DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$evidence_dir" \
    cargo test \
    --manifest-path programs/dclutch-dealer-sbf/program-test/Cargo.toml \
    --test family -- --nocapture

cargo run --quiet -p dclutch-program-test-evidence \
    --bin fold-program-test-evidence -- "$evidence_dir" "$evidence"

# The witness evaluator takes three files; this tier has no bootstrap plan, so
# it is handed the program map as the third. Every Dealer-campaign witness is
# `evidence-jq` and none reads it -- the argument exists only because the
# evaluator requires the path to be present.
"$repo_root/tools/gauntlet/tier1/check-witnesses.sh" \
    "$campaign_dir/witnesses.json" "$evidence" "$campaign_dir/programs.json"

cargo run --quiet --manifest-path tools/gauntlet/census/Cargo.toml -- observe \
    --inventory "$inventory" \
    --ledger "$ledger" \
    --bindings "$campaign_dir/bindings.json" \
    --programs "$campaign_dir/programs.json" \
    --evidence "$evidence"
