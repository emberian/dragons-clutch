#!/usr/bin/env bash
# Compile the checked-release evidence chain over ONE CAMPAIGN'S DEPLOYED
# ARTIFACTS, so the browser's activation un-gate can be exercised against a real
# chain instead of only against itself.
#
# `tools/release/checked-release-candidate.sh` builds the same evidence over
# candidate-local program addresses derived from its own fixed domain. Those
# addresses name no deployed program anywhere, which is exactly right for a
# release candidate and exactly wrong for testing a gate whose whole job is to
# compare a manifest against accounts a chain actually holds. This script keeps
# every construction step and every check identical and changes only the
# BINDINGS: the program addresses, ELFs and semantic release preimages come from
# a completed successor-campaign run directory.
#
# WHAT THIS PRODUCES IS LOCAL-VALIDATOR EVIDENCE. It is not a deployment, not
# devnet, not mainnet, and not an official release. Nothing here signs, submits,
# funds, or publishes.
set -euo pipefail

usage() {
    cat <<'USAGE'
usage: campaign-checked-release.sh --run PATH [options]

  --run PATH     a completed campaign run directory (holds plan.json,
                 spec.json and attestation/)
  --work PATH    scratch output root (default: <run>/checked-release)
  --tool PATH    prebuilt dclutch-release-tool binary
  --repo PATH    source repository (default: this script's repository)
  -h, --help     show this message
USAGE
}

RUN=""
WORK=""
TOOL=""
REPO=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --run) RUN="${2:?--run needs a value}"; shift 2 ;;
        --work) WORK="${2:?--work needs a value}"; shift 2 ;;
        --tool) TOOL="${2:?--tool needs a value}"; shift 2 ;;
        --repo) REPO="${2:?--repo needs a value}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done
[ -n "$RUN" ] || { usage >&2; exit 2; }
[ -f "$RUN/plan.json" ] || { echo "no plan.json under $RUN" >&2; exit 2; }
[ -z "$REPO" ] && REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
[ -z "$WORK" ] && WORK="$RUN/checked-release"

sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
sha256_stdin() { shasum -a 256 | cut -d' ' -f1; }

for tool in jq shasum python3 solana cargo-build-sbf; do
    command -v "$tool" >/dev/null 2>&1 || { echo "required command not found: $tool" >&2; exit 1; }
done

EVIDENCE="$WORK/evidence"
SET_DIR="$WORK/set"
INFRA_DIR="$WORK/infrastructure"
rm -rf "$EVIDENCE" "$SET_DIR" "$INFRA_DIR"
mkdir -p "$EVIDENCE" "$SET_DIR" "$INFRA_DIR"

if [ -z "$TOOL" ]; then
    TOOL="$WORK/host-target/release/dclutch-release-tool"
    ( cd "$REPO" && CARGO_TARGET_DIR="$WORK/host-target" cargo build --release -p dclutch-release-tool ) >"$WORK/build.log" 2>&1
fi
[ -x "$TOOL" ] || { echo "release tool not executable: $TOOL" >&2; exit 1; }

SOURCE_REVISION="$(jq -r '.commit' "$RUN/attestation/core.json")"
ARCHIVE_DIGEST="$(jq -r '.archive_sha256' "$RUN/attestation/core.json")"
SOLANA_VERSION="$(solana --version | head -n 1)"
BUILD_SBF_RAW="$(cargo-build-sbf --version)"
BUILD_SBF_VERSION="$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '1p')"
PLATFORM_TOOLS="$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '2p')"
RUSTC_VERSION="$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '3p') (solana $PLATFORM_TOOLS)"
ROOT_LOCK_DIGEST="$(git -C "$REPO" show "$SOURCE_REVISION:Cargo.lock" | sha256_stdin)"

# Loader V3's canonical address, decoded from its base58 spelling rather than
# pasted as a magic constant.
base58_hex() {
    python3 - "$1" <<'PY'
import sys
ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
value = 0
for character in sys.argv[1]:
    value = value * 58 + ALPHABET.index(character)
print(value.to_bytes(32, "big").hex())
PY
}
LOADER_HEX="$(base58_hex BPFLoaderUpgradeab1e11111111111111111111111)"

# The campaign's Resolution role carries a PROTOCOL semantic preimage; every
# other role carries the gauntlet's own. Both are reproduced here from their
# stated form and then CHECKED against the semantic release id the campaign
# actually published, so a drifted preimage fails loudly instead of silently
# producing a manifest that will never match the chain.
semantic_preimage() {
    if [ "$1" = "resolution" ]; then
        printf 'dclutch/release/source-resolution-controller-core-effects-source-closure-v4'
    else
        printf 'dclutch/gauntlet/semantic-release/v1\nrole=%s\ncommit=%s\n' "$1" "$SOURCE_REVISION"
    fi
}

ROLES="core claims trading resolution custody registry rent"
plan_key() { if [ "$1" = "rent" ]; then printf 'rent_credit'; else printf '%s' "$1"; fi; }

echo "== campaign checked release =="
echo "run:      $RUN"
echo "commit:   $SOURCE_REVISION"
echo "work:     $WORK"

for role in $ROLES; do
    key="$(plan_key "$role")"
    dir="$EVIDENCE/$role"
    mkdir -p "$dir"
    program_id="$(jq -r ".${key}.program_id" "$RUN/plan.json")"
    programdata_id="$(jq -r ".${key}.programdata_id" "$RUN/plan.json")"
    elf="$(jq -r ".${key}.elf_path" "$RUN/plan.json")"
    elf_sha="$(jq -r ".${key}.elf_sha256" "$RUN/plan.json")"
    declared_semantic="$(jq -r ".${key}.semantic_release_id" "$RUN/plan.json")"
    [ -f "$elf" ] || { echo "the campaign's $role ELF is gone: $elf" >&2; exit 1; }
    [ "$(sha256 "$elf")" = "$elf_sha" ] || { echo "$role ELF no longer hashes to the campaign pin" >&2; exit 1; }

    semantic_preimage "$role" > "$dir/semantic.bin"
    observed_semantic="$(sha256 "$dir/semantic.bin")"
    if [ "$observed_semantic" != "$declared_semantic" ]; then
        echo "$role semantic preimage hashes to $observed_semantic, campaign published $declared_semantic" >&2
        exit 1
    fi

    program_hex="$(base58_hex "$program_id")"
    "$TOOL" loader-accounts \
        --program-id "$program_hex" \
        --loader-program-id "$LOADER_HEX" \
        --elf "$elf" \
        --deployment-slot 0 \
        --program-out "$dir/program-account.bin" \
        --programdata-out "$dir/programdata-account.bin" \
        --text-out "$dir/loader-accounts.txt"
    derived_programdata="$(sed -n 's/^programdata_id=//p' "$dir/loader-accounts.txt")"
    [ "$derived_programdata" = "$(base58_hex "$programdata_id")" ] \
        || { echo "$role constructed ProgramData address differs from the campaign plan" >&2; exit 1; }

    {
        printf 'dclutch-release-metadata-v1\n'
        printf 'semantic_kind=unowned\n'
        printf 'program_id=%s\n' "$program_hex"
        printf 'programdata_id=%s\n' "$derived_programdata"
        printf 'loader_program_id=%s\n' "$LOADER_HEX"
        printf 'program_owner=%s\n' "$LOADER_HEX"
        printf 'program_executable=true\n'
        printf 'programdata_owner=%s\n' "$LOADER_HEX"
        printf 'programdata_executable=false\n'
        printf 'source_digest=%s\n' "$ARCHIVE_DIGEST"
        printf 'cargo_lock_digest=%s\n' "$ROOT_LOCK_DIGEST"
        printf 'source_revision=%s\n' "$SOURCE_REVISION"
        printf 'rustc_version=%s\n' "$RUSTC_VERSION"
        printf 'solana_version=%s\n' "$SOLANA_VERSION"
        printf 'cargo_build_sbf_version=%s\n' "$BUILD_SBF_VERSION"
        printf 'target_triple=sbpf-solana-solana\n'
        printf 'build_command=cargo build-sbf --manifest-path programs/dclutch-%s-sbf/Cargo.toml\n' "$role"
        printf 'assumption=Loader V3 Program and ProgramData bytes were constructed offline from the exact ELF; whether a chain agrees is a separate observation\n'
        printf 'assumption=cargo_lock_digest is SHA-256 of the root Cargo.lock at the exact source revision\n'
        printf 'assumption=deployment_slot 0 is the constructed genesis-install value and matches the successor launcher genesis boundary\n'
        printf 'assumption=program_id is the address this campaign deployed the role to on a local validator; no private key exists for it\n'
        printf 'assumption=semantic_kind is unowned because no first-party contract in this tree decodes a role-program release preimage\n'
        printf 'assumption=source_digest is the archive digest the campaign attestation recorded for this revision\n'
    } > "$dir/metadata.txt"

    "$TOOL" create \
        --elf "$elf" \
        --semantic-preimage "$dir/semantic.bin" \
        --metadata "$dir/metadata.txt" \
        --program-account-data "$dir/program-account.bin" \
        --programdata-account-data "$dir/programdata-account.bin" \
        --out "$dir/checked.bin" \
        --text-out "$dir/checked.txt"

    # Construction and verification are separate passes on purpose: verify
    # re-decodes the manifest and rebuilds it from the same evidence.
    "$TOOL" verify \
        --manifest "$dir/checked.bin" \
        --elf "$elf" \
        --semantic-preimage "$dir/semantic.bin" \
        --metadata "$dir/metadata.txt" \
        --program-account-data "$dir/program-account.bin" \
        --programdata-account-data "$dir/programdata-account.bin" \
        --text-out "$dir/verify.txt"
    cmp -s "$dir/checked.txt" "$dir/verify.txt" || { echo "verify projection differs for $role" >&2; exit 1; }
    "$TOOL" inspect --manifest "$dir/checked.bin" --text-out "$dir/inspect.txt"
    cmp -s "$dir/checked.txt" "$dir/inspect.txt" || { echo "inspect projection differs for $role" >&2; exit 1; }
    echo "checked: $role"
done

FIVE_ROLES="--core $EVIDENCE/core/checked.bin \
  --claims $EVIDENCE/claims/checked.bin \
  --trading $EVIDENCE/trading/checked.bin \
  --resolution $EVIDENCE/resolution/checked.bin \
  --custody $EVIDENCE/custody/checked.bin"

# shellcheck disable=SC2086
"$TOOL" derive-set $FIVE_ROLES --out "$SET_DIR/execution-release-set.bin"
# shellcheck disable=SC2086
"$TOOL" create-set --release-set "$SET_DIR/execution-release-set.bin" $FIVE_ROLES \
    --out "$SET_DIR/multiprogram.checked" --text-out "$SET_DIR/multiprogram.txt"
# shellcheck disable=SC2086
"$TOOL" verify-set --manifest "$SET_DIR/multiprogram.checked" $FIVE_ROLES --text-out "$SET_DIR/verify-set.txt"
cmp -s "$SET_DIR/multiprogram.txt" "$SET_DIR/verify-set.txt" || { echo "verify-set projection differs" >&2; exit 1; }
echo "checked: five-role execution release set"

"$TOOL" derive-infrastructure-profile \
    --registry "$EVIDENCE/registry/checked.bin" \
    --rent "$EVIDENCE/rent/checked.bin" \
    --out "$INFRA_DIR/profile.bin"
# shellcheck disable=SC2086
"$TOOL" create-infrastructure \
    --execution "$SET_DIR/multiprogram.checked" \
    --profile "$INFRA_DIR/profile.bin" \
    $FIVE_ROLES \
    --registry "$EVIDENCE/registry/checked.bin" \
    --rent "$EVIDENCE/rent/checked.bin" \
    --out "$INFRA_DIR/infrastructure.checked" \
    --text-out "$INFRA_DIR/infrastructure.txt"
echo "checked: immutable Core/Registry/Rent infrastructure"

# The load-bearing join: the execution release set the manifests derive has to
# be the one the campaign published, or nothing downstream can match.
SET_DIGEST="$(sha256 "$SET_DIR/execution-release-set.bin")"
PUBLISHED_SET="$(jq -r '.release_set_id' "$RUN/plan.json")"
printf 'execution_release_set_preimage_sha256=%s\n' "$SET_DIGEST"
printf 'campaign_published_release_set_id=%s\n' "$PUBLISHED_SET"
if [ "$SET_DIGEST" = "$PUBLISHED_SET" ]; then
    echo "JOIN: the checked release set IS the set this campaign activated"
else
    echo "JOIN: MISMATCH - the checked release set is not the set this campaign activated" >&2
    exit 1
fi

{
    printf 'format=dclutch-campaign-checked-release-summary-v1\n'
    printf 'evidence_level=local-validator-bound-release\n'
    printf 'not_a_deployment_beyond_this_local_validator=true\n'
    printf 'run=%s\n' "$RUN"
    printf 'source_revision=%s\n' "$SOURCE_REVISION"
    printf 'archive_digest=%s\n' "$ARCHIVE_DIGEST"
    printf 'execution_release_set_preimage_sha256=%s\n' "$SET_DIGEST"
    printf 'multiprogram_manifest_sha256=%s\n' "$(sha256 "$SET_DIR/multiprogram.checked")"
    printf 'infrastructure_manifest_sha256=%s\n' "$(sha256 "$INFRA_DIR/infrastructure.checked")"
    for role in $ROLES; do
        printf '%s_checked_manifest_sha256=%s\n' "$role" "$(sha256 "$EVIDENCE/$role/checked.bin")"
    done
} > "$WORK/SUMMARY.txt"
echo
echo "summary: $WORK/SUMMARY.txt"
