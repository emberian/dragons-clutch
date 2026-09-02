#!/usr/bin/env bash
# Run the sponsored-push ProgramTest campaign and fold it into the shared
# census ledger.
#
# This is a ProgramTest FAST LANE. Read TIERS.md's fast-lane bar before treating
# any row it produces as validator evidence: nothing here deploys through
# Loader-v3 and ProgramTest has no finalized commitment. It is ALSO not provider
# evidence -- the receiver and push programs are synthetic bootstrap artifacts
# and every price body is a fixture. The honest sentence about the strongest row
# is "the bank accepted a sponsored capture", never "the market observed a
# price".
#
# Five actions, one campaign: capture a sponsored update into an immutable
# candidate, advance the head, settle the best sealed candidate into a terminal
# certificate and receipt, close the candidates and the head, and commit a
# funded failure on a market nobody answered. Nine hostiles run alongside them
# and ARE recorded, because each carries its own label, its own outcome and its
# own refusal code; the two fixture warm-up transactions are not, because they
# drive no protocol route.
#
# usage: run-resolution-sponsored.sh [--work DIR] [--ledger FILE] [--inventory FILE]
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
gauntlet_dir="$repo_root/tools/gauntlet"
tier_dir="$gauntlet_dir/resolution-sponsored"
work="/private/tmp/dclutch-resolution-sponsored-campaign"
gauntlet_out="/private/tmp/dclutch-gauntlet/out"
ledger=""
inventory=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --work) work="${2:?--work needs a directory}"; shift 2 ;;
        --ledger) ledger="${2:?--ledger needs a file}"; shift 2 ;;
        --inventory) inventory="${2:?--inventory needs a file}"; shift 2 ;;
        *) echo "run-resolution-sponsored.sh: unknown argument $1" >&2; exit 1 ;;
    esac
done
: "${ledger:=$gauntlet_out/ledger.json}"
: "${inventory:=$gauntlet_out/inventory.json}"

if [ ! -f "$inventory" ]; then
    echo "run-resolution-sponsored.sh: no inventory at $inventory." >&2
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
    programs/dclutch-resolution-proof-sbf/Cargo.toml
do
    log="$work/build-$(basename "$(dirname "$manifest")").log"
    build "$manifest" > "$log" || { tail -n 40 "$log" >&2; exit 1; }
    count="$(grep -c 'overwrites values in the frame' "$log" || true)"
    diagnostics=$((diagnostics + count))
    printf '  built %-60s %s frame diagnostics\n' "$manifest" "${count:-0}"
done
if [ "$diagnostics" -ne 0 ]; then
    echo "resolution-sponsored: $diagnostics SBF stack-frame-overwrite diagnostics; refusing to run a" >&2
    echo "  campaign on artifacts the toolchain calls potentially-undefined." >&2
    exit 1
fi

evidence_dir="$work/evidence"
rm -rf "$evidence_dir"
mkdir -p "$evidence_dir"
echo "campaign: resolution-sponsored (sponsored_push_lifecycle)"
SBF_OUT_DIR="$sbf_out" DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$evidence_dir" \
    cargo test --manifest-path crates/dclutch-svm-harness/Cargo.toml \
    --test sponsored_push_lifecycle -- --test-threads=1

evidence="$work/resolution-sponsored.evidence.json"
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
echo "resolution-sponsored: folded into $ledger"
echo "resolution-sponsored: render the report with 'tools/gauntlet/run.sh --mode census'"
