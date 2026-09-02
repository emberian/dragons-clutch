#!/usr/bin/env bash
# Run the Claims and Custody family campaigns and fold them into the census.
#
# This is a ProgramTest FAST LANE. Read tools/gauntlet/claims-custody/README.md
# for which of the TIERS.md fast-lane conditions it satisfies, and which it does
# not, before treating any row it produces as validator evidence.
#
# Two census campaigns, because a census campaign has ONE program map and the
# two families pin different addresses for `registry`:
#
#   claims-family-programtest   the protocol Position lifecycle and the composed
#                               Admit -> SparseNativeTransfer -> Close chain
#   custody-family-programtest  ordinary and delegated Custody, once per token
#                               profile, against the real SPL Token and
#                               Token-2022 programs
#
# usage: run-claims-custody.sh [--work DIR] [--ledger FILE] [--inventory FILE]
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
tier_dir="$repo_root/tools/gauntlet/claims-custody"
work="/private/tmp/dclutch-claims-custody-campaign"
gauntlet_out="/private/tmp/dclutch-gauntlet/out"
ledger=""
inventory=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --work) work="${2:?--work needs a directory}"; shift 2 ;;
        --ledger) ledger="${2:?--ledger needs a file}"; shift 2 ;;
        --inventory) inventory="${2:?--inventory needs a file}"; shift 2 ;;
        *) echo "run-claims-custody.sh: unknown argument $1" >&2; exit 1 ;;
    esac
done
: "${ledger:=$gauntlet_out/ledger.json}"
: "${inventory:=$gauntlet_out/inventory.json}"

if [ ! -f "$inventory" ]; then
    echo "run-claims-custody.sh: no inventory at $inventory." >&2
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

# Both families run against the real Token-2022, and `protocol-position` reads
# `spl_token_2022.so` out of SBF_OUT_DIR with no digest check of its own. A
# locally built substitute from a non-Linux-x86_64 host is a DIFFERENT artifact
# with the same name, and what it produces is not a campaign failure -- it is
# seven tests failing on content that never had the program the fixture pins.
# Measured 2026-09-02: 0 of 7 on Darwin with the audit build, 7 of 7 with the
# canonical one. So refuse BY NAME, and exit 2, which is this suite's
# missing-prerequisite convention: on such a host this tier has proven nothing,
# which is a different verdict from a gate failing.
provenance="$repo_root/programs/dclutch-claims-sbf/fixtures/token-2022-v11.provenance"
canonical_digest="$(sed -n 's/^canonical_elf_sha256=//p' "$provenance")"
audit_digest="$(sed -n 's/^macos_arm64_audit_elf_sha256=//p' "$provenance")"
if [ -z "$canonical_digest" ] || [ -z "$audit_digest" ]; then
    echo "claims-custody DID NOT RUN: $provenance carries no digest pair to check against." >&2
    exit 2
fi
if [ ! -f "$sbf_out/spl_token_2022.so" ] && [ -n "${TOKEN_2022_V11_ELF:-}" ]; then
    cp -- "$TOKEN_2022_V11_ELF" "$sbf_out/spl_token_2022.so"
fi
if [ ! -f "$sbf_out/spl_token_2022.so" ]; then
    echo "claims-custody DID NOT RUN: no spl_token_2022.so in $sbf_out." >&2
    echo "  Set TOKEN_2022_V11_ELF to the canonical artifact ($canonical_digest)," >&2
    echo "  or run programs/dclutch-claims-sbf/fixtures/prepare-token-2022-v11.sh on a" >&2
    echo "  Linux-x86_64 host; see that script's own host-provenance requirement." >&2
    exit 2
fi
observed_token_digest="$(shasum -a 256 "$sbf_out/spl_token_2022.so" | cut -d" " -f1)"
if [ "$observed_token_digest" != "$canonical_digest" ]; then
    if [ "$observed_token_digest" = "$audit_digest" ]; then
        echo "claims-custody DID NOT RUN: $sbf_out/spl_token_2022.so is the macOS arm64 AUDIT" >&2
        echo "  build ($audit_digest), not the canonical" >&2
        echo "  cross-host-reproducible artifact ($canonical_digest)." >&2
        echo "  The audit build is for reading, not for running a campaign: protocol-position" >&2
        echo "  fails 0 of 7 on content against it. Supply the canonical artifact through" >&2
        echo "  TOKEN_2022_V11_ELF, or build it on a Linux-x86_64 host." >&2
    else
        echo "claims-custody DID NOT RUN: $sbf_out/spl_token_2022.so is neither the canonical" >&2
        echo "  artifact ($canonical_digest) nor the known macOS arm64 audit build; it hashes" >&2
        echo "  to $observed_token_digest and this tier will not guess what it is." >&2
    fi
    exit 2
fi

# `cargo build-sbf` exits zero even when the SBF backend reports that a call
# overwrites its own stack frame and "may cause undefined behavior during
# execution". An artifact the toolchain calls potentially-undefined has no
# business entering a campaign unnoticed, so the count is taken and a nonzero
# one stops the tier.
diagnostics=0
for manifest in \
    programs/dclutch-claims-sbf/Cargo.toml \
    programs/dclutch-registry-sbf/Cargo.toml \
    programs/dclutch-core-sbf/Cargo.toml \
    programs/dclutch-rent-sbf/Cargo.toml \
    programs/dclutch-custody-sbf/Cargo.toml \
    programs/dclutch-claims-sbf/test-programs/liability-basis-caller/Cargo.toml \
    programs/dclutch-claims-sbf/test-programs/sparse-chain-caller/Cargo.toml \
    programs/dclutch-custody-sbf/test-programs/caller/Cargo.toml
do
    log="$work/build-$(basename "$(dirname "$manifest")").log"
    build "$manifest" > "$log" || { tail -n 40 "$log" >&2; exit 1; }
    count="$(grep -c 'overwrites values in the frame' "$log" || true)"
    diagnostics=$((diagnostics + count))
    printf '  built %-56s %s frame diagnostics\n' "$manifest" "${count:-0}"
done
if [ "$diagnostics" -ne 0 ]; then
    echo "claims-custody: $diagnostics SBF stack-frame-overwrite diagnostics; refusing to run a" >&2
    echo "  campaign on artifacts the toolchain calls potentially-undefined." >&2
    exit 1
fi

# campaign group : manifest : test target
campaigns="
claims:programs/dclutch-claims-sbf/program-test/protocol-position/Cargo.toml:lifecycle
claims:programs/dclutch-claims-sbf/program-test/sparse-chain/Cargo.toml:sparse_chain
custody:programs/dclutch-custody-sbf/Cargo.toml:program_test
"

for group in claims custody; do
    evidence_dir="$work/$group-evidence"
    # A campaign re-run must not accumulate records from a previous shape.
    rm -rf "$evidence_dir"
    mkdir -p "$evidence_dir"
    for entry in $campaigns; do
        case "$entry" in "$group":*) ;; *) continue ;; esac
        rest="${entry#*:}"
        manifest="${rest%%:*}"
        target="${rest##*:}"
        echo "campaign: $manifest --test $target"
        SBF_OUT_DIR="$sbf_out" DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR="$evidence_dir" \
            cargo test --manifest-path "$manifest" --test "$target" -- --nocapture
    done

    evidence="$work/$group.evidence.json"
    cargo run --quiet -p dclutch-program-test-evidence \
        --bin fold-program-test-evidence -- "$evidence_dir" "$evidence"

    # The witness evaluator takes three files; this tier has no bootstrap plan,
    # so it is handed the program map as the third. Every witness here is
    # `evidence-jq` and none reads it.
    "$repo_root/tools/gauntlet/tier1/check-witnesses.sh" \
        "$tier_dir/$group-witnesses.json" "$evidence" "$tier_dir/$group-programs.json"

    cargo run --quiet --manifest-path tools/gauntlet/census/Cargo.toml -- observe \
        --inventory "$inventory" \
        --ledger "$ledger" \
        --bindings "$tier_dir/$group-bindings.json" \
        --programs "$tier_dir/$group-programs.json" \
        --evidence "$evidence"
done

echo
echo "claims-custody: folded into $ledger"
echo "claims-custody: render the report with 'tools/gauntlet/run.sh --mode census'"
