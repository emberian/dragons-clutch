#!/usr/bin/env bash
# Build every dClutch role and accelerator SBF artifact from one exact source
# commit and compile the complete offline checked-release evidence chain over
# them with dclutch-release-tool.
#
# WHAT THIS PRODUCES IS A LOCAL, REPRODUCIBLE RELEASE CANDIDATE.
# It is not a deployment, not devnet, not mainnet, and not an official release.
# Loader V3 account bytes are CONSTRUCTED from each ELF, never observed on any
# chain, and the program addresses are candidate-local values derived offline
# from a fixed domain. Nothing here signs, submits, funds, or publishes.
#
# The run is idempotent: every derived directory is rebuilt from scratch, while
# the cargo target directory is reused so a re-run after one program changes
# costs a single incremental SBF build.
set -euo pipefail

usage() {
    cat <<'USAGE'
usage: checked-release-candidate.sh [options]

  --repo PATH    source repository (default: this script's repository)
  --work PATH    scratch output root (default: /private/tmp/dclutch-release-candidate)
  --tool PATH    prebuilt dclutch-release-tool binary (default: build one under --work)
  --commit REV   source revision to archive (default: HEAD)
  --keep-elf     reuse the ELFs already under --work instead of rebuilding
  --allow-build-diagnostics
                 admit artifacts whose SBF build emitted a stack-frame
                 diagnostic, recording the exact counts in the summary
  -h, --help     show this message
USAGE
}

REPO=""
WORK="/private/tmp/dclutch-release-candidate"
TOOL=""
COMMIT="HEAD"
KEEP_ELF="false"
ALLOW_DIAGNOSTICS="false"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --repo) REPO="${2:?--repo needs a value}"; shift 2 ;;
        --work) WORK="${2:?--work needs a value}"; shift 2 ;;
        --tool) TOOL="${2:?--tool needs a value}"; shift 2 ;;
        --commit) COMMIT="${2:?--commit needs a value}"; shift 2 ;;
        --keep-elf) KEEP_ELF="true"; shift ;;
        --allow-build-diagnostics) ALLOW_DIAGNOSTICS="true"; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done

if [ -z "$REPO" ]; then
    REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fi
case "$WORK" in /*) ;; *) echo "--work must be absolute" >&2; exit 2 ;; esac

# role label : cargo package : built artifact stem
ROLES="core:dclutch-core-sbf:dclutch_core_sbf
claims:dclutch-claims-sbf:dclutch_claims_sbf
trading:dclutch-trading-sbf:dclutch_trading_sbf
resolution:dclutch-resolution-proof-sbf:dclutch_resolution_proof_sbf
custody:dclutch-custody-sbf:dclutch_custody_sbf
registry:dclutch-registry-sbf:dclutch_registry_sbf
rent:dclutch-rent-sbf:dclutch_rent_sbf
general-accelerator:dclutch-general-accelerator-sbf:dclutch_general_accelerator_sbf
dealer-accelerator:dclutch-dealer-accelerator-sbf:dclutch_dealer_accelerator_sbf
series-shadow:dclutch-series-shadow-sbf:dclutch_series_shadow_sbf"

SOURCE="$WORK/source"
BUILD_TARGET="$WORK/build-target"
HOST_TARGET="$WORK/host-target"
ELF_DIR="$WORK/elf"
EVIDENCE="$WORK/evidence"
SET_DIR="$WORK/set"
INFRA_DIR="$WORK/infrastructure"
SUMMARY="$WORK/SUMMARY.txt"
BUILD_LOG="$WORK/build.log"

sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
sha256_stdin() { shasum -a 256 | cut -d' ' -f1; }
run_tool() { "$TOOL" "$@"; }

# Loader V3's canonical address, decoded from its base58 spelling rather than
# pasted as a magic constant.
LOADER_HEX="$(python3 - <<'PY'
ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
value = 0
for character in "BPFLoaderUpgradeab1e11111111111111111111111":
    value = value * 58 + ALPHABET.index(character)
print(value.to_bytes(32, "big").hex())
PY
)"

echo "== checked release candidate =="
echo "repo:   $REPO"
echo "work:   $WORK"

mkdir -p "$WORK"
rm -rf "$EVIDENCE" "$SET_DIR" "$INFRA_DIR"
mkdir -p "$EVIDENCE" "$SET_DIR" "$INFRA_DIR" "$ELF_DIR"

# ---------------------------------------------------------------- source pin
SOURCE_REVISION="$(git -C "$REPO" rev-parse "$COMMIT")"
# Every tracked path, mode, and blob identity at the pinned commit. This covers
# the complete first-party build input set without depending on archive
# framing, file mtimes, or checkout state.
SOURCE_DIGEST="$(git -C "$REPO" ls-tree -r --full-tree "$SOURCE_REVISION" | sha256_stdin)"
echo "commit: $SOURCE_REVISION"

rm -rf "$SOURCE"
mkdir -p "$SOURCE"
git -C "$REPO" archive "$SOURCE_REVISION" | tar -x -C "$SOURCE"

ROOT_LOCK_DIGEST="$(sha256 "$SOURCE/Cargo.lock")"

# ------------------------------------------------------------------ toolchain
SOLANA_VERSION="$(solana --version | head -n 1)"
BUILD_SBF_RAW="$(cargo-build-sbf --version)"
BUILD_SBF_VERSION="$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '1p')"
PLATFORM_TOOLS="$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '2p')"
SBF_RUSTC="$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '3p')"
RUSTC_VERSION="$SBF_RUSTC (solana $PLATFORM_TOOLS)"

# ----------------------------------------------------------------- SBF builds
# cargo build-sbf exits zero even when the SBF backend reports that a call
# overwrites its own stack frame and "may cause undefined behavior during
# execution". Nothing downstream can see that: the ELF is well-formed and every
# release-tool check passes on it. So count the diagnostics per role here and
# refuse by default, because an artifact the toolchain says may execute as
# undefined behavior has no business entering a release unnoticed.
DIAGNOSTIC_PATTERN='overwrites values in the frame'
if [ "$KEEP_ELF" != "true" ]; then
    : > "$BUILD_LOG"
    : > "$WORK/build-diagnostics.txt"
    for entry in $ROLES; do
        role="${entry%%:*}"; rest="${entry#*:}"
        package="${rest%%:*}"; stem="${rest#*:}"
        echo "build: $role ($package)"
        role_log="$WORK/build-$role.log"
        (
            cd "$SOURCE"
            CARGO_TARGET_DIR="$BUILD_TARGET" \
                cargo build-sbf --manifest-path "programs/$package/Cargo.toml"
        ) >"$role_log" 2>&1
        cat "$role_log" >> "$BUILD_LOG"
        count="$(grep -c "$DIAGNOSTIC_PATTERN" "$role_log" || true)"
        printf '%s=%s\n' "$role" "$count" >> "$WORK/build-diagnostics.txt"
        if [ "$count" != "0" ]; then
            echo "BUILD DIAGNOSTIC: $role emitted $count SBF stack-frame overwrite reports" >&2
            grep "$DIAGNOSTIC_PATTERN" "$role_log" | sort -u >&2
        fi
        cp "$BUILD_TARGET/deploy/$stem.so" "$ELF_DIR/$role.so"
    done
fi

DIAGNOSTIC_TOTAL=0
if [ -f "$WORK/build-diagnostics.txt" ]; then
    DIAGNOSTIC_TOTAL="$(awk -F= '{total += $2} END {print total + 0}' "$WORK/build-diagnostics.txt")"
fi
if [ "$DIAGNOSTIC_TOTAL" != "0" ] && [ "$ALLOW_DIAGNOSTICS" != "true" ]; then
    echo "refusing: $DIAGNOSTIC_TOTAL SBF build diagnostics; fix them at their owner, or re-run with --allow-build-diagnostics to record them explicitly" >&2
    exit 1
fi

TARGET_TRIPLE=""
for candidate in sbpf-solana-solana sbf-solana-solana; do
    if [ -d "$BUILD_TARGET/$candidate" ]; then TARGET_TRIPLE="$candidate"; fi
done
if [ -z "$TARGET_TRIPLE" ]; then
    echo "could not determine the SBF target triple under $BUILD_TARGET" >&2
    exit 1
fi

# --------------------------------------------------------------- release tool
if [ -z "$TOOL" ]; then
    TOOL="$HOST_TARGET/release/dclutch-release-tool"
    ( cd "$REPO" && CARGO_TARGET_DIR="$HOST_TARGET" \
        cargo build --release -p dclutch-release-tool ) >>"$BUILD_LOG" 2>&1
fi
[ -x "$TOOL" ] || { echo "release tool not executable: $TOOL" >&2; exit 1; }

# ------------------------------------------------------- per-artifact evidence
# A candidate-local program address. It is derived offline from a fixed domain
# and the role name, so it is stable across rebuilds and across an artifact
# changing underneath a role. No private key exists for it, it is not registered
# anywhere, and it names no deployed program.
program_id_for() {
    printf 'dclutch/checked-release-candidate/program-id/v1\nrole=%s\n' "$1" | sha256_stdin
}

for entry in $ROLES; do
    role="${entry%%:*}"; rest="${entry#*:}"
    package="${rest%%:*}"
    dir="$EVIDENCE/$role"
    mkdir -p "$dir"
    elf="$ELF_DIR/$role.so"
    [ -f "$elf" ] || { echo "missing ELF for $role: $elf" >&2; exit 1; }

    program_id="$(program_id_for "$role")"

    # Candidate-declared semantic preimage. No first-party contract in this tree
    # decodes a role-program release preimage, which is exactly why the metadata
    # below records semantic_kind=unowned rather than claiming a capability owner.
    printf 'dclutch/checked-release-candidate/unowned-semantic-release/v1\nrole=%s\npackage=%s\nsource_revision=%s\n' \
        "$role" "$package" "$SOURCE_REVISION" > "$dir/semantic.bin"

    run_tool loader-accounts \
        --program-id "$program_id" \
        --loader-program-id "$LOADER_HEX" \
        --elf "$elf" \
        --deployment-slot 0 \
        --program-out "$dir/program-account.bin" \
        --programdata-out "$dir/programdata-account.bin" \
        --text-out "$dir/loader-accounts.txt"
    programdata_id="$(sed -n 's/^programdata_id=//p' "$dir/loader-accounts.txt")"

    lock="$SOURCE/Cargo.lock"
    if [ -f "$SOURCE/programs/$package/Cargo.lock" ]; then
        lock="$SOURCE/programs/$package/Cargo.lock"
    fi
    lock_digest="$(sha256 "$lock")"

    {
        printf 'dclutch-release-metadata-v1\n'
        printf 'semantic_kind=unowned\n'
        printf 'program_id=%s\n' "$program_id"
        printf 'programdata_id=%s\n' "$programdata_id"
        printf 'loader_program_id=%s\n' "$LOADER_HEX"
        printf 'program_owner=%s\n' "$LOADER_HEX"
        printf 'program_executable=true\n'
        printf 'programdata_owner=%s\n' "$LOADER_HEX"
        printf 'programdata_executable=false\n'
        printf 'source_digest=%s\n' "$SOURCE_DIGEST"
        printf 'cargo_lock_digest=%s\n' "$lock_digest"
        printf 'source_revision=%s\n' "$SOURCE_REVISION"
        printf 'rustc_version=%s\n' "$RUSTC_VERSION"
        printf 'solana_version=%s\n' "$SOLANA_VERSION"
        printf 'cargo_build_sbf_version=%s\n' "$BUILD_SBF_VERSION"
        printf 'target_triple=%s\n' "$TARGET_TRIPLE"
        printf 'build_command=cargo build-sbf --manifest-path programs/%s/Cargo.toml\n' "$package"
        # Strictly ascending, unique, and each one load-bearing.
        printf 'assumption=Loader V3 Program and ProgramData bytes were constructed offline from the exact ELF; no chain was observed\n'
        printf 'assumption=cargo_lock_digest is SHA-256 of the exact Cargo.lock that resolved this package\n'
        printf 'assumption=deployment_slot 0 is the constructed genesis-install value, not an observed deployment slot\n'
        printf 'assumption=program_id is a candidate-local address derived offline from a fixed domain and the role name; no private key exists for it\n'
        printf 'assumption=semantic_kind is unowned because no first-party contract in this tree decodes a role-program release preimage\n'
        printf 'assumption=source_digest is SHA-256 of the git ls-tree -r --full-tree listing at the exact source revision\n'
    } > "$dir/metadata.txt"

    run_tool create \
        --elf "$elf" \
        --semantic-preimage "$dir/semantic.bin" \
        --metadata "$dir/metadata.txt" \
        --program-account-data "$dir/program-account.bin" \
        --programdata-account-data "$dir/programdata-account.bin" \
        --out "$dir/checked.bin" \
        --text-out "$dir/checked.txt"

    # Construction and verification are separate passes on purpose: verify
    # re-decodes the manifest and rebuilds it from the same evidence.
    run_tool verify \
        --manifest "$dir/checked.bin" \
        --elf "$elf" \
        --semantic-preimage "$dir/semantic.bin" \
        --metadata "$dir/metadata.txt" \
        --program-account-data "$dir/program-account.bin" \
        --programdata-account-data "$dir/programdata-account.bin" \
        --text-out "$dir/verify.txt"
    cmp -s "$dir/checked.txt" "$dir/verify.txt" \
        || { echo "verify projection differs for $role" >&2; exit 1; }

    run_tool inspect --manifest "$dir/checked.bin" --text-out "$dir/inspect.txt"
    cmp -s "$dir/checked.txt" "$dir/inspect.txt" \
        || { echo "inspect projection differs for $role" >&2; exit 1; }
    echo "checked: $role"
done

FIVE_ROLES="--core $EVIDENCE/core/checked.bin \
  --claims $EVIDENCE/claims/checked.bin \
  --trading $EVIDENCE/trading/checked.bin \
  --resolution $EVIDENCE/resolution/checked.bin \
  --custody $EVIDENCE/custody/checked.bin"

# ------------------------------------------------- five-role execution set
# shellcheck disable=SC2086
run_tool derive-set $FIVE_ROLES --out "$SET_DIR/execution-release-set.bin"
# shellcheck disable=SC2086
run_tool create-set --release-set "$SET_DIR/execution-release-set.bin" $FIVE_ROLES \
    --out "$SET_DIR/multiprogram.checked" --text-out "$SET_DIR/multiprogram.txt"
# shellcheck disable=SC2086
run_tool verify-set --manifest "$SET_DIR/multiprogram.checked" $FIVE_ROLES \
    --text-out "$SET_DIR/verify-set.txt"
cmp -s "$SET_DIR/multiprogram.txt" "$SET_DIR/verify-set.txt" \
    || { echo "verify-set projection differs" >&2; exit 1; }
run_tool inspect-set --manifest "$SET_DIR/multiprogram.checked" \
    --text-out "$SET_DIR/inspect-set.txt"
cmp -s "$SET_DIR/multiprogram.txt" "$SET_DIR/inspect-set.txt" \
    || { echo "inspect-set projection differs" >&2; exit 1; }
echo "checked: five-role execution release set"

# --------------------------------------------- immutable infrastructure join
run_tool derive-infrastructure-profile \
    --registry "$EVIDENCE/registry/checked.bin" \
    --rent "$EVIDENCE/rent/checked.bin" \
    --out "$INFRA_DIR/profile.bin"
# shellcheck disable=SC2086
run_tool create-infrastructure \
    --execution "$SET_DIR/multiprogram.checked" \
    --profile "$INFRA_DIR/profile.bin" \
    $FIVE_ROLES \
    --registry "$EVIDENCE/registry/checked.bin" \
    --rent "$EVIDENCE/rent/checked.bin" \
    --out "$INFRA_DIR/infrastructure.checked" \
    --text-out "$INFRA_DIR/infrastructure.txt"
# shellcheck disable=SC2086
run_tool verify-infrastructure \
    --manifest "$INFRA_DIR/infrastructure.checked" \
    --execution "$SET_DIR/multiprogram.checked" \
    $FIVE_ROLES \
    --registry "$EVIDENCE/registry/checked.bin" \
    --rent "$EVIDENCE/rent/checked.bin" \
    --text-out "$INFRA_DIR/verify-infrastructure.txt"
cmp -s "$INFRA_DIR/infrastructure.txt" "$INFRA_DIR/verify-infrastructure.txt" \
    || { echo "verify-infrastructure projection differs" >&2; exit 1; }
run_tool inspect-infrastructure --manifest "$INFRA_DIR/infrastructure.checked" \
    --text-out "$INFRA_DIR/inspect-infrastructure.txt"
cmp -s "$INFRA_DIR/infrastructure.txt" "$INFRA_DIR/inspect-infrastructure.txt" \
    || { echo "inspect-infrastructure projection differs" >&2; exit 1; }
echo "checked: immutable Core/Registry/Rent infrastructure"

# ------------------------------------------------------------------- summary
{
    printf 'format=dclutch-checked-release-candidate-summary-v1\n'
    printf 'evidence_level=local-reproducible-release-candidate\n'
    printf 'not_a_deployment=true\n'
    printf 'source_revision=%s\n' "$SOURCE_REVISION"
    printf 'source_digest=%s\n' "$SOURCE_DIGEST"
    printf 'root_cargo_lock_digest=%s\n' "$ROOT_LOCK_DIGEST"
    printf 'rustc_version=%s\n' "$RUSTC_VERSION"
    printf 'solana_version=%s\n' "$SOLANA_VERSION"
    printf 'cargo_build_sbf_version=%s\n' "$BUILD_SBF_VERSION"
    printf 'target_triple=%s\n' "$TARGET_TRIPLE"
    printf 'loader_program_id=%s\n' "$LOADER_HEX"
    printf 'sbf_build_diagnostics_total=%s\n' "$DIAGNOSTIC_TOTAL"
    printf 'sbf_build_diagnostics_accepted=%s\n' "$ALLOW_DIAGNOSTICS"
    if [ -f "$WORK/build-diagnostics.txt" ]; then
        sed -n 's/^/sbf_build_diagnostics./p' "$WORK/build-diagnostics.txt"
    fi
    for entry in $ROLES; do
        role="${entry%%:*}"
        dir="$EVIDENCE/$role"
        printf '%s_elf_sha256=%s\n' "$role" "$(sha256 "$ELF_DIR/$role.so")"
        printf '%s_elf_bytes=%s\n' "$role" "$(wc -c < "$ELF_DIR/$role.so" | tr -d ' ')"
        printf '%s_program_id=%s\n' "$role" "$(sed -n 's/^program_id=//p' "$dir/loader-accounts.txt")"
        printf '%s_programdata_id=%s\n' "$role" "$(sed -n 's/^programdata_id=//p' "$dir/loader-accounts.txt")"
        printf '%s_checked_manifest_sha256=%s\n' "$role" "$(sha256 "$dir/checked.bin")"
    done
    printf 'execution_release_set_preimage_sha256=%s\n' "$(sha256 "$SET_DIR/execution-release-set.bin")"
    printf 'multiprogram_manifest_sha256=%s\n' "$(sha256 "$SET_DIR/multiprogram.checked")"
    printf 'infrastructure_profile_sha256=%s\n' "$(sha256 "$INFRA_DIR/profile.bin")"
    printf 'infrastructure_manifest_sha256=%s\n' "$(sha256 "$INFRA_DIR/infrastructure.checked")"
    sed -n 's/^/multiprogram./p' "$SET_DIR/multiprogram.txt"
    sed -n 's/^/infrastructure./p' "$INFRA_DIR/infrastructure.txt"
} > "$SUMMARY"

echo
echo "summary: $SUMMARY"
