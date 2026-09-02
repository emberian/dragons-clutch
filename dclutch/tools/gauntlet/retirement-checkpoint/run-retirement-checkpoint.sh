#!/usr/bin/env bash
# Run the checkpointed aggregate-retirement campaign and fold it into the census.
#
# WHY THIS EXISTS. `crates/dclutch-svm-harness/tests/market_retirement_v1_lifecycle.rs`
# has driven the whole aggregate-retirement checkpoint chain against real Core,
# Claims, Custody, Registry, Resolution, Trading and Rent ELFs since it landed,
# and the census could not see a byte of it: the campaign called `record()` for
# nothing, so seven routes -- the entire terminal lifecycle of a market -- read
# NEVER-EXECUTED in `docs/reference/routes.md` while its own suite was green.
# That is the register reporting an absence that was really an instrument gap,
# and it is the same gap `claims-fractional-atomic` closed for the fractional
# claim-check life.
#
# The order is EMIT, then RUN, then FOLD, and only then author bindings against
# what the ledger OBSERVED. `tools/genref/generate.mjs` derives `witnessed` from
# bindings alone and never consults the ledger, so a binding written from what a
# campaign OUGHT to touch manufactures exactly the false green this tier exists
# to remove.
#
# This is a ProgramTest FAST LANE. Read TIERS.md's bar before treating any row
# here as validator evidence: nothing deploys through Loader V3, the ProgramData
# accounts are constructed by the campaign, and ProgramTest has no finalized
# commitment. What it does have is the real compiled ELFs and the real CPI
# graph -- Core into Claims, Core into Custody, Core into Rent.
#
#   tools/gauntlet/retirement-checkpoint/run-retirement-checkpoint.sh
#
# Builds nothing on a chain, signs nothing, contacts no cluster.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
gauntlet_dir="$repo_root/tools/gauntlet"
tier_dir="$gauntlet_dir/retirement-checkpoint"
work="${DCLUTCH_RETIREMENT_CHECKPOINT_WORK:-/private/tmp/dclutch-retirement-checkpoint-campaign}"
sbf_out="${SBF_OUT_DIR:-$work/sbf-out}"
gauntlet_out="${DCLUTCH_GAUNTLET_OUT:-/private/tmp/dclutch-gauntlet/out}"
inventory="${DCLUTCH_GAUNTLET_INVENTORY:-$gauntlet_out/inventory.json}"
ledger="${DCLUTCH_GAUNTLET_LEDGER:-$gauntlet_out/ledger.json}"

mkdir -p "$work" "$sbf_out" "$gauntlet_out"
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
# potentially-undefined has no business entering a campaign unnoticed.
diagnostics=0
for manifest in \
    programs/dclutch-core-sbf/Cargo.toml \
    programs/dclutch-claims-sbf/Cargo.toml \
    programs/dclutch-custody-sbf/Cargo.toml \
    programs/dclutch-registry-sbf/Cargo.toml \
    programs/dclutch-resolution-proof-sbf/Cargo.toml \
    programs/dclutch-trading-sbf/Cargo.toml \
    programs/dclutch-rent-sbf/Cargo.toml
do
    log="$work/build-$(basename "$(dirname "$manifest")").log"
    build "$manifest" > "$log" || { tail -n 40 "$log" >&2; exit 1; }
    count="$(grep -c 'overwrites values in the frame' "$log" || true)"
    diagnostics=$((diagnostics + count))
    printf '  built %-56s %s frame diagnostics\n' "$manifest" "${count:-0}"
done
if [ "$diagnostics" -ne 0 ]; then
    echo "retirement-checkpoint: $diagnostics SBF stack-frame-overwrite diagnostics; refusing" >&2
    echo "  to run a campaign on artifacts the toolchain calls potentially-undefined." >&2
    exit 1
fi

evidence_dir="$work/evidence"
# A re-run must not accumulate records from a previous shape.
rm -rf "$evidence_dir"
mkdir -p "$evidence_dir"

echo "campaign: retirement-checkpoint (market_retirement_v1_lifecycle)"
# Filtered to the checkpointed test on purpose. The file's other test is the
# LEGACY atomic retirement, whose transactions carry no label; recording them
# would demand bindings for a route family this tier does not claim.
SBF_OUT_DIR="$sbf_out" DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$evidence_dir" \
    cargo test --manifest-path crates/dclutch-svm-harness/Cargo.toml \
    --test market_retirement_v1_lifecycle \
    checkpointed_retirement_is_packet_bounded_resumable_and_conserving \
    -- --exact --test-threads=1 --nocapture

# The chain is twelve acts. Fewer files than that means two transactions
# collapsed onto one signature -- the census dedup key -- and the fold would
# silently under-count rather than fail.
recorded="$(find "$evidence_dir" -name '*.json' | wc -l | tr -d ' ')"
if [ "$recorded" -ne 12 ]; then
    echo "retirement-checkpoint: recorded $recorded transactions, expected 12." >&2
    echo "  A missing file is a duplicated signature, not a skipped act." >&2
    exit 1
fi

evidence="$work/retirement-checkpoint.evidence.json"
cargo run --quiet -p dclutch-program-test-evidence \
    --bin fold-program-test-evidence -- "$evidence_dir" "$evidence"

"$gauntlet_dir/tier1/check-witnesses.sh" \
    "$tier_dir/witnesses.json" "$evidence" "$tier_dir/programs.json"

if [ ! -f "$inventory" ]; then
    echo "retirement-checkpoint: no inventory at $inventory; produce one first with" >&2
    echo "  tools/gauntlet/run.sh --mode census" >&2
    exit 1
fi

cargo run --quiet --manifest-path tools/gauntlet/census/Cargo.toml -- observe \
    --inventory "$inventory" \
    --ledger "$ledger" \
    --bindings "$tier_dir/bindings.json" \
    --programs "$tier_dir/programs.json" \
    --evidence "$evidence"

echo
echo "retirement-checkpoint: folded into $ledger"
echo "retirement-checkpoint: render the report with 'tools/gauntlet/run.sh --mode census'"
