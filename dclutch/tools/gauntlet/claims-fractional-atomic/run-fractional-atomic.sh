#!/usr/bin/env bash
# Run the fractional-atomic ProgramTest campaign and fold it into the census.
#
# WHY THIS EXISTS. The campaign has passed against real ELFs since `8fdcdc56`
# and was invisible to the census the whole time: it took no
# `dclutch-program-test-evidence` dependency, so it called `record()` for
# nothing, emitted no evidence document, and no `bindings.json` could be
# corroborated against it. Its routes therefore read NEVER-EXECUTED in
# `docs/reference/routes.md` while its own suite was green -- which is the
# register reporting an absence that was really an instrument gap.
#
# The order here is the point and it is not negotiable: EMIT, then RUN, then
# FOLD, and only then author bindings against what the ledger OBSERVED. A
# binding is what flips a row to `witnessed` in a register that never consults
# the ledger (`tools/genref/generate.mjs` reads bindings alone), so a binding
# written from what a campaign OUGHT to touch manufactures exactly the false
# green this tier exists to remove.
#
#   tools/gauntlet/claims-fractional-atomic/run-fractional-atomic.sh
#
# Runs an in-process bank with synthetic test signers; contacts no public cluster.

set -eu

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
gauntlet_dir="$repo_root/tools/gauntlet"
tier_dir="$gauntlet_dir/claims-fractional-atomic"
work="${DCLUTCH_FRACTIONAL_ATOMIC_WORK:-/private/tmp/dclutch-fractional-atomic-campaign}"
sbf_out="${SBF_OUT_DIR:-$work/sbf-out}"
out="${DCLUTCH_GAUNTLET_OUT:-/private/tmp/dclutch-gauntlet/out}"
inventory="$out/inventory.json"
ledger="$out/ledger.json"

mkdir -p "$work" "$out"
cd "$repo_root"
# The suite owns builds, Token-2022 provenance and the target inventory. The
# census campaign must actually include the new founding/conservation targets.
evidence_dir="$(mktemp -d "$work/evidence.XXXXXX")"
DCLUTCH_FRACTIONAL_ATOMIC_WORK="$work" SBF_OUT_DIR="$sbf_out" \
TOKEN_2022_SO="${TOKEN_2022_SO:-${TOKEN_2022_ELF:-}}" \
DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$evidence_dir" \
    programs/dclutch-claims-sbf/program-test/fractional-atomic/run-program-test.sh \
        --test claims_founding --test claims_conservation --test fractional_compaction

evidence="$work/fractional-atomic.evidence.json"
cargo run --locked --quiet -p dclutch-program-test-evidence \
    --bin fold-program-test-evidence -- "$evidence_dir" "$evidence"

if [ ! -f "$inventory" ]; then
    echo "fractional-atomic: no inventory at $inventory; produce one first with" >&2
    echo "  cargo run --release -p dclutch-route-census -- inventory --root . --out $inventory" >&2
    exit 1
fi

cargo run --locked --quiet -p dclutch-route-census -- observe \
    --inventory "$inventory" \
    --ledger "$ledger" \
    --bindings "$tier_dir/bindings.json" \
    --programs "$tier_dir/programs.json" \
    --evidence "$evidence"

echo
echo "fractional-atomic: folded into $ledger"
echo "fractional-atomic: render the report with 'tools/gauntlet/run.sh --mode census'"
