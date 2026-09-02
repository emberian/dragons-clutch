#!/usr/bin/env bash
# Run the retirement-only Trading-to-Core Custody replay handoff campaign and
# fold it into the census.
#
# WHY THIS EXISTS. `programs/dclutch-core-sbf/tests/retirement_replay_handoff_program_test.rs`
# has driven `core/retirement_replay_handoff_v1::process` and, by CPI,
# `custody/retirement_replay_handoff_v1::process` against the real Core and
# Custody ELFs since it landed -- with EXACT refusal codes for all six
# hostilities and the replay, no bare `is_err()` anywhere. Both routes still
# read NEVER-EXECUTED in `docs/reference/routes.md`, because the campaign called
# `record()` for nothing and no binding could be corroborated against it.
#
# EMIT, then RUN, then FOLD, and only then author bindings against what the
# ledger OBSERVED: `tools/genref/generate.mjs` derives `witnessed` from bindings
# alone and never consults the ledger, so a binding written from expectation
# manufactures the false green this tier exists to remove.
#
# This is a ProgramTest FAST LANE. Nothing deploys through Loader V3, the
# ProgramData accounts are constructed by the campaign, and ProgramTest has no
# finalized commitment. `TIERS.md` states the bar.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
gauntlet_dir="$repo_root/tools/gauntlet"
tier_dir="$gauntlet_dir/retirement-replay-handoff"
work="${DCLUTCH_RETIREMENT_HANDOFF_WORK:-/private/tmp/dclutch-handoff-campaign}"
sbf_out="${SBF_OUT_DIR:-$work/sbf-out}"
gauntlet_out="${DCLUTCH_GAUNTLET_OUT:-/private/tmp/dclutch-gauntlet/out}"
inventory="${DCLUTCH_GAUNTLET_INVENTORY:-$gauntlet_out/inventory.json}"
ledger="${DCLUTCH_GAUNTLET_LEDGER:-$gauntlet_out/ledger.json}"

mkdir -p "$work" "$sbf_out" "$gauntlet_out"
cd "$repo_root"

# The tree is shared. Two campaigns on 2026-09-01 died ten minutes into an SBF
# build against another lane's half-applied refactor, each time looking like a
# campaign defect rather than a scheduling one.
cargo check -p dclutch-core-sbf --test retirement_replay_handoff_program_test \
    > "$work/cargo-check.log" 2>&1 || {
    tail -n 30 "$work/cargo-check.log" >&2
    echo "retirement-replay-handoff: the workspace does not check; measure at HEAD in a" >&2
    echo "  detached worktree rather than blaming the campaign." >&2
    exit 1
}

build() {
    if command -v swarm-build >/dev/null 2>&1; then
        swarm-build cargo build-sbf --manifest-path "$1" --sbf-out-dir "$sbf_out" 2>&1
    else
        cargo build-sbf --manifest-path "$1" --sbf-out-dir "$sbf_out" 2>&1
    fi
}

diagnostics=0
for manifest in \
    programs/dclutch-core-sbf/Cargo.toml \
    programs/dclutch-custody-sbf/Cargo.toml
do
    log="$work/build-$(basename "$(dirname "$manifest")").log"
    build "$manifest" > "$log" || { tail -n 40 "$log" >&2; exit 1; }
    count="$(grep -c 'overwrites values in the frame' "$log" || true)"
    diagnostics=$((diagnostics + count))
    printf '  built %-48s %s frame diagnostics\n' "$manifest" "${count:-0}"
done
if [ "$diagnostics" -ne 0 ]; then
    echo "retirement-replay-handoff: $diagnostics SBF stack-frame-overwrite diagnostics;" >&2
    echo "  refusing to run a campaign on artifacts the toolchain calls potentially-undefined." >&2
    exit 1
fi

evidence_dir="$work/evidence"
rm -rf "$evidence_dir"
mkdir -p "$evidence_dir"

echo "campaign: retirement-replay-handoff (retirement_replay_handoff_program_test)"
SBF_OUT_DIR="$sbf_out" DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$evidence_dir" \
    cargo test -p dclutch-core-sbf --test retirement_replay_handoff_program_test \
    -- --test-threads=1

# One accepted handoff, one replay refusal, six hostile faults. Fewer files than
# eight means two transactions collapsed onto one signature -- the census dedup
# key -- and the fold would under-count rather than fail.
recorded="$(find "$evidence_dir" -name '*.json' | wc -l | tr -d ' ')"
if [ "$recorded" -ne 8 ]; then
    echo "retirement-replay-handoff: recorded $recorded transactions, expected 8." >&2
    exit 1
fi

evidence="$work/retirement-replay-handoff.evidence.json"
cargo run --quiet -p dclutch-program-test-evidence \
    --bin fold-program-test-evidence -- "$evidence_dir" "$evidence"

"$gauntlet_dir/tier1/check-witnesses.sh" \
    "$tier_dir/witnesses.json" "$evidence" "$tier_dir/programs.json"

if [ ! -f "$inventory" ]; then
    echo "retirement-replay-handoff: no inventory at $inventory; produce one first with" >&2
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
echo "retirement-replay-handoff: folded into $ledger"
