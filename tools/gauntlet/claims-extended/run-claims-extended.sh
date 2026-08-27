#!/usr/bin/env bash
# Run the four Claims-family ProgramTest campaigns SN-REC wired into the
# census evidence path (2026-08-27) and fold them into the shared ledger.
#
# This is a ProgramTest FAST LANE. Read TIERS.md's fast-lane bar before
# treating any row it produces as validator evidence -- none of these
# campaigns deploy through Loader-v3, and ProgramTest has no finalized
# commitment.
#
# Four independent campaigns, each in its own tools/gauntlet/claims-<name>/
# directory (own bindings.json/witnesses.json/programs.json, since each pins
# different fixture program addresses):
#
#   claims-affine-batch                the canonical affine LBV2 Claims waist
#   claims-fractional-signed-delta     the N=258 Fractional wrap through SignedDeltaV3
#   claims-rational-representation-v2  structured issuance/unwrap/denominate/
#                                       reconstitute plus terminal redemption
#   claims-rational-lifecycle          ActivateReceipt/ActivateCoordinate/
#                                       RetireCoordinate/RetireReceipt against
#                                       real Token-2022
#
# NOT included: claims-custody (owns its own run-claims-custody.sh already) and
# liability_basis_v2_program_test.rs, whose campaign is NOT green at HEAD --
# see the "claims/liability_basis_v2::process" entry in tools/gauntlet/blocked.json
# for why, and do not add it here until that entry is retired.
#
# Two of the four campaigns (rational-representation-v2, rational-lifecycle)
# require the byte-for-byte CANONICAL spl-token-2022 11.0.0 ELF
# (programs/dclutch-claims-sbf/fixtures/token-2022-v11.provenance) and refuse a
# locally-built substitute from a non-Linux-x86_64 host on purpose. On such a
# host, build it once on a matching machine (hbox satisfies this: Linux x86_64,
# cargo-build-sbf 4.0.0, platform-tools v1.53) and pass it via TOKEN_2022_V11_ELF,
# or run prepare-token-2022-v11.sh there directly and copy the result in.
#
# usage: run-claims-extended.sh [--work DIR] [--ledger FILE] [--inventory FILE]
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
gauntlet_dir="$repo_root/tools/gauntlet"
work="/private/tmp/dclutch-claims-extended-campaign"
gauntlet_out="/private/tmp/dclutch-gauntlet/out"
ledger=""
inventory=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --work) work="${2:?--work needs a directory}"; shift 2 ;;
        --ledger) ledger="${2:?--ledger needs a file}"; shift 2 ;;
        --inventory) inventory="${2:?--inventory needs a file}"; shift 2 ;;
        *) echo "run-claims-extended.sh: unknown argument $1" >&2; exit 1 ;;
    esac
done
: "${ledger:=$gauntlet_out/ledger.json}"
: "${inventory:=$gauntlet_out/inventory.json}"

if [ ! -f "$inventory" ]; then
    echo "run-claims-extended.sh: no inventory at $inventory." >&2
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
    programs/dclutch-rent-sbf/Cargo.toml \
    programs/dclutch-claims-sbf/test-programs/affine-batch-caller/Cargo.toml \
    programs/dclutch-claims-sbf/test-programs/fractional-signed-delta-caller/Cargo.toml \
    programs/dclutch-claims-sbf/test-programs/rational-v2-caller/Cargo.toml \
    programs/dclutch-claims-sbf/test-programs/rational-lifecycle-caller/Cargo.toml
do
    log="$work/build-$(basename "$(dirname "$manifest")").log"
    build "$manifest" > "$log" || { tail -n 40 "$log" >&2; exit 1; }
    count="$(grep -c 'overwrites values in the frame' "$log" || true)"
    diagnostics=$((diagnostics + count))
    printf '  built %-70s %s frame diagnostics\n' "$manifest" "${count:-0}"
done
if [ "$diagnostics" -ne 0 ]; then
    echo "claims-extended: $diagnostics SBF stack-frame-overwrite diagnostics; refusing to run a" >&2
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
                echo "claims-extended: could not produce the canonical Token-2022 v11 ELF." >&2
                echo "  See prepare-token-2022-v11.sh's own host-provenance requirement above," >&2
                echo "  or set TOKEN_2022_V11_ELF to a matching pre-built artifact." >&2
                exit 1
            }
    fi
fi

# campaign group : manifest : test target : gauntlet dir : cargo-invocation flavour
campaigns="
affine-batch:programs/dclutch-claims-sbf/program-test/affine-batch/Cargo.toml:affine_batch_v2:claims-affine-batch:manifest
fractional-signed-delta:programs/dclutch-claims-sbf/program-test/fractional-signed-delta/Cargo.toml:fractional_signed_delta:claims-fractional-signed-delta:manifest
rational-representation-v2:dclutch-claims-sbf:rational_representation_v2_program_test:claims-rational-representation-v2:package
rational-lifecycle:programs/dclutch-claims-sbf/program-test/rational-lifecycle/Cargo.toml:lifecycle:claims-rational-lifecycle:manifest
"

for entry in $campaigns; do
    group="${entry%%:*}"
    rest="${entry#*:}"
    manifest_or_package="${rest%%:*}"
    rest="${rest#*:}"
    target="${rest%%:*}"
    rest="${rest#*:}"
    gauntlet_name="${rest%%:*}"
    flavour="${rest#*:}"

    tier_dir="$gauntlet_dir/$gauntlet_name"
    evidence_dir="$work/$group-evidence"
    rm -rf "$evidence_dir"
    mkdir -p "$evidence_dir"
    echo "campaign: $group ($target)"
    if [ "$flavour" = "package" ]; then
        SBF_OUT_DIR="$sbf_out" DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$evidence_dir" \
            cargo test -p "$manifest_or_package" --test "$target" -- --test-threads=1
    else
        SBF_OUT_DIR="$sbf_out" DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$evidence_dir" \
            cargo test --manifest-path "$manifest_or_package" --test "$target" -- --test-threads=1
    fi

    evidence="$work/$group.evidence.json"
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
done

echo
echo "claims-extended: folded into $ledger"
echo "claims-extended: render the report with 'tools/gauntlet/run.sh --mode census'"
