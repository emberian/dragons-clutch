#!/usr/bin/env bash
# Run the Structured V2 island's ProgramTest campaign and fold it into the census.
#
# This is a ProgramTest FAST LANE. Read TIERS.md's fast-lane bar before treating
# any row it produces as validator evidence: it deploys nothing through
# Loader-v3 and ProgramTest has no finalized commitment.
#
# WHY THIS EXISTS AS ITS OWN DIRECTORY, and why it introduces no route ids.
#
# Decision 0011 §3b/§3c: under Option A, Structured V2 authors NO artifacts and
# has NO program. It LOWERS onto the Rational Representation V2 wire, so every
# route it can execute is a Claims route and every binding here points at an
# existing `claims/*` route id. There is deliberately no census TARGETS row
# (0011 §6). What the directory adds is the island's own witnesses and its own
# CU budgets, so the Structured claims stop being prose in a decision record and
# become rows something checks.
#
# THE CAMPAIGN is programs/dclutch-claims-sbf/tests/rational_representation_v2_program_test.rs
# filtered to its three Structured-specific tests, at the Structured campaign
# basis K = 3, coefficients [2, 3, 5], denominator 7 — pairwise coprime and
# coprime to the denominator, which is what makes a one-atom backing skew at one
# coordinate impossible to present as a legitimate quantity at another. Its
# execution descriptor is DERIVED by `derive_structured_representation_descriptor_v2`
# over real Structured terms, a real composition bundle and the real exposure
# record; nothing here stands on a hand-written descriptor preimage.
#
# The FULL campaign — the four committing open actions, the terminal ladder, the
# custody-namespace hostiles — is bound by tools/gauntlet/claims-rational-representation-v2/,
# which is a strict superset of these rows. Run either, or both; the ledger is
# append-only and a route corroborated twice is still corroborated.
#
# The canonical Token-2022 v11 ELF is required and a locally built substitute
# from a non-Linux-x86_64 host is refused on purpose
# (programs/dclutch-claims-sbf/fixtures/token-2022-v11.provenance). Pass one via
# TOKEN_2022_V11_ELF, or let run-claims-extended.sh's preparation step produce it.
#
# usage: run-structured.sh [--work DIR] [--ledger FILE] [--inventory FILE]
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
tier_dir="$repo_root/tools/gauntlet/structured"
work="/private/tmp/dclutch-structured-campaign"
gauntlet_out="/private/tmp/dclutch-gauntlet/out"
ledger=""
inventory=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --work) work="${2:?--work needs a directory}"; shift 2 ;;
        --ledger) ledger="${2:?--ledger needs a file}"; shift 2 ;;
        --inventory) inventory="${2:?--inventory needs a file}"; shift 2 ;;
        *) echo "run-structured.sh: unknown argument $1" >&2; exit 1 ;;
    esac
done
: "${ledger:=$gauntlet_out/ledger.json}"
: "${inventory:=$gauntlet_out/inventory.json}"

if [ ! -f "$inventory" ]; then
    echo "run-structured.sh: no inventory at $inventory." >&2
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
# potentially-undefined has no business entering a campaign unnoticed.
diagnostics=0
for manifest in \
    programs/dclutch-claims-sbf/Cargo.toml \
    programs/dclutch-registry-sbf/Cargo.toml \
    programs/dclutch-core-sbf/Cargo.toml \
    programs/dclutch-custody-sbf/Cargo.toml \
    programs/dclutch-claims-sbf/test-programs/rational-v2-caller/Cargo.toml
do
    log="$work/build-$(basename "$(dirname "$manifest")").log"
    build "$manifest" > "$log" || { tail -n 40 "$log" >&2; exit 1; }
    count="$(grep -c 'overwrites values in the frame' "$log" || true)"
    diagnostics=$((diagnostics + count))
    printf '  built %-70s %s frame diagnostics\n' "$manifest" "${count:-0}"
done
if [ "$diagnostics" -ne 0 ]; then
    echo "structured: $diagnostics SBF stack-frame-overwrite diagnostics; refusing to run a" >&2
    echo "  campaign on artifacts the toolchain calls potentially-undefined." >&2
    exit 1
fi

if [ ! -f "$sbf_out/spl_token_2022.so" ]; then
    if [ -n "${TOKEN_2022_V11_ELF:-}" ]; then
        cp -- "$TOKEN_2022_V11_ELF" "$sbf_out/spl_token_2022.so"
    else
        "$repo_root/programs/dclutch-claims-sbf/fixtures/prepare-token-2022-v11.sh" \
            "$(find "${CARGO_HOME:-$HOME/.cargo}/registry/cache" -name 'spl-token-2022-11.0.0.crate' -print -quit)" \
            "$sbf_out" \
            > "$work/build-token-2022.log" 2>&1 || {
                tail -n 40 "$work/build-token-2022.log" >&2
                echo "structured: could not produce the canonical Token-2022 v11 ELF." >&2
                echo "  See prepare-token-2022-v11.sh's own host-provenance requirement," >&2
                echo "  or set TOKEN_2022_V11_ELF to a matching pre-built artifact." >&2
                exit 1
            }
    fi
fi

evidence_dir="$work/evidence"
rm -rf "$evidence_dir"
mkdir -p "$evidence_dir"

echo "campaign: structured-v2-programtest"
SBF_OUT_DIR="$sbf_out" DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$evidence_dir" \
    cargo test -p dclutch-claims-sbf --test rational_representation_v2_program_test -- \
        --test-threads=1 \
        the_structured_family_hostiles_refuse_through_the_real_wire \
        a_receipt_mint_missing_its_burn_role_refuses_at_the_first_issue \
        the_full_width_structured_frame_does_not_fit_a_packet_at_k_three

evidence="$work/structured.evidence.json"
cargo run --quiet -p dclutch-program-test-evidence \
    --bin fold-program-test-evidence -- "$evidence_dir" "$evidence"

"$repo_root/tools/gauntlet/tier1/check-witnesses.sh" \
    "$tier_dir/witnesses.json" "$evidence" "$tier_dir/programs.json"

cargo run --quiet --manifest-path tools/gauntlet/census/Cargo.toml -- observe \
    --inventory "$inventory" \
    --ledger "$ledger" \
    --bindings "$tier_dir/bindings.json" \
    --programs "$tier_dir/programs.json" \
    --evidence "$evidence"

echo
echo "structured: folded into $ledger"
echo "structured: render the report with 'tools/gauntlet/run.sh --mode census'"
