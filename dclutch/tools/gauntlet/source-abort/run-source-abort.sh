#!/usr/bin/env bash
# Run the expired-source abort ProgramTest campaign and fold it into the shared
# census ledger.
#
# This is a ProgramTest FAST LANE. Read TIERS.md's fast-lane bar before treating
# any row it produces as validator evidence: nothing here deploys through
# Loader-v3 and ProgramTest has no finalized commitment.
#
# WHAT IT RECORDS, and why it exists. The expired-source abort is the only route
# out of a funded projection whose founding expired; before it existed that
# collateral could not be moved by anything. It used to be driven by tier 1's
# DCLTPCA1 lane, which left the campaign when the founding was split (e8591ab67),
# taking with it the ONLY witness `custody/abort_source_and_close#AbortSourceAndClose`
# and `trading/projected_custody_bootstrap_v1::process_projected_custody_abort_v1`
# had in any campaign. The driver never left: `real_custody_source_abort_then_
# controller_suffix_is_exact_and_resumable` has been executing the whole suffix
# against the real Trading and Custody ELFs the entire time. What was missing was
# a producer -- the test recorded nothing, so the census could not see it. This
# campaign is that half.
#
# The suffix is DCLTPCA1 -> DCLTCF1A -> DCLTCF2A, with three hostiles that are
# the boundary the abort is judged on: an unwind route that let anyone empty a
# live founding would be a worse defect than the stranding it was written to fix.
#
# usage: run-source-abort.sh [--work DIR] [--ledger FILE] [--inventory FILE]
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
gauntlet_dir="$repo_root/tools/gauntlet"
tier_dir="$gauntlet_dir/source-abort"
work="/private/tmp/dclutch-source-abort-campaign"
gauntlet_out="/private/tmp/dclutch-gauntlet/out"
ledger=""
inventory=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --work) work="${2:?--work needs a directory}"; shift 2 ;;
        --ledger) ledger="${2:?--ledger needs a file}"; shift 2 ;;
        --inventory) inventory="${2:?--inventory needs a file}"; shift 2 ;;
        *) echo "run-source-abort.sh: unknown argument $1" >&2; exit 1 ;;
    esac
done
: "${ledger:=$gauntlet_out/ledger.json}"
: "${inventory:=$gauntlet_out/inventory.json}"

if [ ! -f "$inventory" ]; then
    echo "run-source-abort.sh: no inventory at $inventory." >&2
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
# overwrites its own stack frame. An artifact the toolchain calls
# potentially-undefined has no business entering a campaign, and this campaign's
# whole subject is a recovery route -- the one you least want to run on an ELF
# whose frames the toolchain would not vouch for.
diagnostics=0
for manifest in \
    programs/dclutch-trading-sbf/Cargo.toml \
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
    echo "source-abort: $diagnostics SBF stack-frame-overwrite diagnostics; refusing to run a" >&2
    echo "  campaign on artifacts the toolchain calls potentially-undefined." >&2
    exit 1
fi

evidence_dir="$work/evidence"
rm -rf "$evidence_dir"
mkdir -p "$evidence_dir"
echo "campaign: source-abort (controller_funding_split_abort)"
# The whole binary runs, not just the recorded case: the abort suffix shares its
# fixture with the split-rollback and slot-pin hostiles, and a campaign that ran
# only its own case would report green on an ELF the neighbouring cases refuse.
# Only the six labelled steps record evidence, so nothing arrives unbound.
# `--test-threads=1` because program logs from one binary interleave and a
# refusal read out of a shared stream belongs to whichever test emitted it.
SBF_OUT_DIR="$sbf_out" DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$evidence_dir" \
    cargo test --manifest-path crates/dclutch-svm-harness/Cargo.toml \
    --test controller_funding_split_abort -- --test-threads=1

evidence="$work/source-abort.evidence.json"
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
echo "source-abort: folded into $ledger"
echo "source-abort: render the report with 'tools/gauntlet/run.sh --mode census'"
