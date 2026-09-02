#!/usr/bin/env bash
# Run the NATIVE claim-check campaign and fold it into the census.
#
# WHY THIS EXISTS. `programs/dclutch-claims-sbf/tests/claim_check/mod.rs` has
# driven `claims/claim_check_compaction_v1::process_compaction` and
# `claims/claim_check_redemption_v1::process_redemption#else` against real
# Claims, Custody, Registry, Core, Resolution and Token-2022 artifacts since it
# landed, and both read NEVER-EXECUTED in `docs/reference/routes.md`: the module
# emits evidence, but the tier that runs its binary never bound its labels.
#
# WHY IT IS A SEPARATE TIER. The unfiltered binary belongs to
# `tools/gauntlet/claims-rational-representation-v2/`, whose bindings have
# drifted behind its campaign -- folding the full 273-transaction run against
# them reports 143 problems, including one stale binding, which means
# `run-claims-extended.sh` cannot pass today. Repairing that is the row that owns
# the representation campaign. This tier is scoped to the `claim_check::` filter,
# the same relationship `tools/gauntlet/structured/` has to the same binary, so
# the claim-check rows stand on an instrument that is green.
#
# EMIT, then RUN, then FOLD, and only then author bindings against what the
# ledger OBSERVED.
#
# This is a ProgramTest FAST LANE; `TIERS.md` states the bar.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
gauntlet_dir="$repo_root/tools/gauntlet"
tier_dir="$gauntlet_dir/claims-claim-check"
work="${DCLUTCH_CLAIM_CHECK_WORK:-/private/tmp/dclutch-claim-check-campaign}"
sbf_out="${SBF_OUT_DIR:-$work/sbf-out}"
gauntlet_out="${DCLUTCH_GAUNTLET_OUT:-/private/tmp/dclutch-gauntlet/out}"
inventory="${DCLUTCH_GAUNTLET_INVENTORY:-$gauntlet_out/inventory.json}"
ledger="${DCLUTCH_GAUNTLET_LEDGER:-$gauntlet_out/ledger.json}"

mkdir -p "$work" "$sbf_out" "$gauntlet_out"
cd "$repo_root"

# The tree is shared, and three campaigns on 2026-09-01 died ten minutes into an
# SBF build against another lane's half-applied refactor.
cargo check -p dclutch-claims-sbf --test rational_representation_v2_program_test \
    > "$work/cargo-check.log" 2>&1 || {
    tail -n 30 "$work/cargo-check.log" >&2
    echo "claims-claim-check: the workspace does not check; measure at HEAD in a detached" >&2
    echo "  worktree rather than blaming the campaign." >&2
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
    programs/dclutch-claims-sbf/Cargo.toml \
    programs/dclutch-registry-sbf/Cargo.toml \
    programs/dclutch-core-sbf/Cargo.toml \
    programs/dclutch-custody-sbf/Cargo.toml \
    programs/dclutch-resolution-proof-sbf/Cargo.toml \
    programs/dclutch-rent-sbf/Cargo.toml \
    programs/dclutch-trading-sbf/Cargo.toml \
    programs/dclutch-claims-sbf/test-programs/rational-v2-caller/Cargo.toml
do
    log="$work/build-$(basename "$(dirname "$manifest")").log"
    build "$manifest" > "$log" || { tail -n 40 "$log" >&2; exit 1; }
    count="$(grep -c 'overwrites values in the frame' "$log" || true)"
    diagnostics=$((diagnostics + count))
    printf '  built %-72s %s frame diagnostics\n' "$manifest" "${count:-0}"
done
if [ "$diagnostics" -ne 0 ]; then
    echo "claims-claim-check: $diagnostics SBF stack-frame-overwrite diagnostics; refusing to" >&2
    echo "  run a campaign on artifacts the toolchain calls potentially-undefined." >&2
    exit 1
fi

# Token-2022 v11 is a third-party artifact this tier does not build.
# `claims-extended` owns the provenance requirement; the canonical digest is
# `canonical_elf_sha256` in programs/dclutch-claims-sbf/fixtures/token-2022-v11.provenance
# and the fixture script only reproduces it on a canonical Linux-x86_64 host.
if [ ! -f "$sbf_out/spl_token_2022.so" ]; then
    if [ -n "${TOKEN_2022_V11_ELF:-}" ] && [ -f "${TOKEN_2022_V11_ELF}" ]; then
        cp -- "$TOKEN_2022_V11_ELF" "$sbf_out/spl_token_2022.so"
    else
        "$repo_root/programs/dclutch-claims-sbf/fixtures/prepare-token-2022-v11.sh" \
            "$(find "${CARGO_HOME:-$HOME/.cargo}/registry/cache" -name 'spl-token-2022-11.0.0.crate' -print -quit)" \
            "$sbf_out" > "$work/build-token-2022.log" 2>&1 || {
                tail -n 20 "$work/build-token-2022.log" >&2
                echo "claims-claim-check: could not produce the canonical Token-2022 v11 ELF." >&2
                exit 1
            }
    fi
fi
expected_token_sha="$(sed -n 's/^canonical_elf_sha256=//p' \
    "$repo_root/programs/dclutch-claims-sbf/fixtures/token-2022-v11.provenance")"
actual_token_sha="$(shasum -a 256 "$sbf_out/spl_token_2022.so" | cut -d' ' -f1)"
if [ "$actual_token_sha" != "$expected_token_sha" ]; then
    echo "claims-claim-check: Token-2022 digest $actual_token_sha is not the canonical" >&2
    echo "  $expected_token_sha from token-2022-v11.provenance. A campaign whose token" >&2
    echo "  program has unstated provenance is not evidence about Token-2022." >&2
    exit 1
fi

evidence_dir="$work/evidence"
rm -rf "$evidence_dir"
mkdir -p "$evidence_dir"

echo "campaign: claims-claim-check (rational_representation_v2_program_test, claim_check::)"
SBF_OUT_DIR="$sbf_out" DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$evidence_dir" \
    cargo test -p dclutch-claims-sbf --test rational_representation_v2_program_test \
    claim_check:: -- --test-threads=1

evidence="$work/claim-check.evidence.json"
cargo run --quiet -p dclutch-program-test-evidence \
    --bin fold-program-test-evidence -- "$evidence_dir" "$evidence"

"$gauntlet_dir/tier1/check-witnesses.sh" \
    "$tier_dir/witnesses.json" "$evidence" "$tier_dir/programs.json"

if [ ! -f "$inventory" ]; then
    echo "claims-claim-check: no inventory at $inventory; produce one first with" >&2
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
echo "claims-claim-check: folded into $ledger"
