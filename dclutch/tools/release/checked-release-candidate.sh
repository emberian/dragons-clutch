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
# Release admission deliberately requires a fresh top-package compilation for
# every program under programs/. Use a new --work root for each admitted run.
# A warm target may still be useful while developing, but cargo's silence on a
# fresh unit is not evidence that the SBF backend re-checked that unit's frames.
set -euo pipefail

usage() {
    cat <<'USAGE'
usage: checked-release-candidate.sh [options]

  --repo PATH    source repository (default: this script's repository)
  --work PATH    scratch output root (default: /private/tmp/dclutch-release-candidate)
  --tool PATH    prebuilt dclutch-release-tool binary (never emits an Upgrade gate;
                 default source-pinned build under --work is required for that gate)
  --commit REV   source revision to archive (default: HEAD)
  --keep-elf     legacy option; refused because reused ELFs have no fresh-build proof
  --allow-build-diagnostics
                 admit artifacts whose SBF build emitted a stack-frame
                 diagnostic, recording the exact counts in the summary
  -h, --help     show this message

On hbox, wrap this whole command once; this script never calls swarm-build:
  SWARM_MEM_MAX=32G swarm-build tools/release/checked-release-candidate.sh ...
USAGE
}

REPO=""
WORK="/private/tmp/dclutch-release-candidate"
TOOL=""
PREBUILT_TOOL="false"
COMMIT="HEAD"
KEEP_ELF="false"
ALLOW_DIAGNOSTICS="false"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --repo) REPO="${2:?--repo needs a value}"; shift 2 ;;
        --work) WORK="${2:?--work needs a value}"; shift 2 ;;
        --tool) TOOL="${2:?--tool needs a value}"; PREBUILT_TOOL="true"; shift 2 ;;
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
if [ "$KEEP_ELF" = "true" ]; then
    echo "refusing --keep-elf: a checked release requires a fresh top-package compile marker for every SBF link; use a new --work root" >&2
    exit 2
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRESHNESS_CHECKER="$SCRIPT_DIR/check_sbf_build_freshness.py"
[ -x "$FRESHNESS_CHECKER" ] \
    || { echo "build-freshness checker not executable: $FRESHNESS_CHECKER" >&2; exit 1; }

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
FRAME_DIR="$WORK/frame"
EVIDENCE="$WORK/evidence"
SET_DIR="$WORK/set"
INFRA_DIR="$WORK/infrastructure"
SUMMARY="$WORK/SUMMARY.txt"
BUILD_LOG="$WORK/build.log"
BUILD_LINKS="$WORK/build-links.tsv"
BUILD_RUN="$WORK/build-run.txt"
SOURCE_TREE="$WORK/source-tree.txt"
LOCKS_BEFORE="$WORK/cargo-locks-before.tsv"
LOCKS_AFTER="$WORK/cargo-locks-after.tsv"
UPGRADE_GATE="$WORK/CHECKED_UPGRADE_GATE.json"

sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
sha256_stdin() { shasum -a 256 | cut -d' ' -f1; }
run_tool() { "$TOOL" "$@"; }
write_lock_manifest() {
    local root="$1"
    local out="$2"
    (
        cd "$root"
        find . -type f -name Cargo.lock -print \
            | sed 's#^\./##' \
            | LC_ALL=C sort \
            | while IFS= read -r lock; do
                printf '%s\t%s\n' "$lock" "$(sha256 "$lock")"
            done
    ) > "$out"
}

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
WORK="$(cd "$WORK" && pwd -P)"
SOURCE="$WORK/source"
BUILD_TARGET="$WORK/build-target"
HOST_TARGET="$WORK/host-target"
ELF_DIR="$WORK/elf"
FRAME_DIR="$WORK/frame"
EVIDENCE="$WORK/evidence"
SET_DIR="$WORK/set"
INFRA_DIR="$WORK/infrastructure"
SUMMARY="$WORK/SUMMARY.txt"
BUILD_LOG="$WORK/build.log"
BUILD_LINKS="$WORK/build-links.tsv"
BUILD_RUN="$WORK/build-run.txt"
SOURCE_TREE="$WORK/source-tree.txt"
LOCKS_BEFORE="$WORK/cargo-locks-before.tsv"
LOCKS_AFTER="$WORK/cargo-locks-after.tsv"
UPGRADE_GATE="$WORK/CHECKED_UPGRADE_GATE.json"
rm -f "$UPGRADE_GATE" "$SUMMARY"
rm -rf "$EVIDENCE" "$SET_DIR" "$INFRA_DIR" "$ELF_DIR" "$FRAME_DIR"
mkdir -p "$EVIDENCE" "$SET_DIR" "$INFRA_DIR" "$ELF_DIR" "$FRAME_DIR"

# ---------------------------------------------------------------- source pin
SOURCE_REVISION="$(git -C "$REPO" rev-parse "$COMMIT")"
# Every tracked path, mode, and blob identity at the pinned commit. This covers
# the complete first-party build input set without depending on archive
# framing, file mtimes, or checkout state.
git -C "$REPO" ls-tree -r --full-tree "$SOURCE_REVISION" > "$SOURCE_TREE"
SOURCE_DIGEST="$(sha256 "$SOURCE_TREE")"
echo "commit: $SOURCE_REVISION"

rm -rf "$SOURCE"
mkdir -p "$SOURCE"
git -C "$REPO" archive "$SOURCE_REVISION" | tar -x -C "$SOURCE"

# Cargo's `--locked` refusal is the per-invocation admission. This manifest is
# the repository-wide complement: it proves that no build created, removed, or
# rewrote any Cargo.lock anywhere in the archived source tree. The archive
# contains tracked files only, so a newly created nested lock is visible too.
write_lock_manifest "$SOURCE" "$LOCKS_BEFORE"
LOCK_COUNT="$(wc -l < "$LOCKS_BEFORE" | tr -d ' ')"
[ "$LOCK_COUNT" -gt 0 ] \
    || { echo "refusing: source archive contains no Cargo.lock files" >&2; exit 1; }
LOCK_SET_DIGEST="$(sha256 "$LOCKS_BEFORE")"

# The orchestrator and its two measurement parsers are part of the admitted
# source, not ambient host helpers. Refuse an invocation whose script bytes do
# not equal the pinned revision; otherwise `--commit OLD` could truthfully bind
# OLD source while CURRENT admission code decided what counted as checked.
cmp -s "$SCRIPT_DIR/checked-release-candidate.sh" \
    "$SOURCE/tools/release/checked-release-candidate.sh" \
    || { echo "refusing: invoke the checked-release runner from the exact --commit source revision" >&2; exit 1; }
cmp -s "$SCRIPT_DIR/check_sbf_build_freshness.py" \
    "$SOURCE/tools/release/check_sbf_build_freshness.py" \
    || { echo "refusing: build-freshness checker differs from the exact --commit source revision" >&2; exit 1; }
FRESHNESS_CHECKER="$SOURCE/tools/release/check_sbf_build_freshness.py"

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

# Packages excluded from the root workspace resolve against their own lockfile,
# so they must not share a target directory with the workspace builds: a warm
# rebuild otherwise puts two units of one path dependency in the same graph and
# the excluded package stops compiling ("one version of crate ... used here").
# Observed on dclutch-series-shadow-sbf, whose isolated build is clean.
target_dir_for() {
    if printf '%s\n' "$WORKSPACE_MEMBERS" | grep -qx "programs/$1"; then
        printf '%s\n' "$BUILD_TARGET"
    else
        printf '%s\n' "$BUILD_TARGET-$1"
    fi
}

WORKSPACE_MEMBERS="$(awk '
    /^ *members *= *\[/ { inside = 1; next }
    inside && /^ *\]/ { inside = 0; next }
    inside { gsub(/[",]/, ""); gsub(/^ +| +$/, ""); if (length($0) > 0) print $0 }
' "$SOURCE/Cargo.toml")"
if [ -z "$WORKSPACE_MEMBERS" ]; then
    echo "could not read the root workspace member list" >&2
    exit 1
fi

# The directory is the one semantic owner of the frame-gated link set. ROLES
# maps ten of those packages into release artifacts; it does not decide which
# packages get compiled. The other packages remain frame-gate-only.
: > "$BUILD_LINKS"
for manifest in "$SOURCE"/programs/*/Cargo.toml; do
    [ -f "$manifest" ] || { echo "program manifest enumeration is empty" >&2; exit 1; }
    package="$(basename "$(dirname "$manifest")")"
    label="$package"
    for entry in $ROLES; do
        role="${entry%%:*}"; rest="${entry#*:}"
        role_package="${rest%%:*}"
        if [ "$package" = "$role_package" ]; then
            label="$role"
            break
        fi
    done
    printf '%s\t%s\n' "$label" "$package" >> "$BUILD_LINKS"
done

if ! awk -F '\t' '
    NF != 2 || $1 !~ /^[a-z0-9][a-z0-9_-]*$/ || $2 !~ /^[a-z0-9][a-z0-9_-]*$/ { exit 1 }
    seen_label[$1]++ || seen_package[$2]++ { exit 1 }
    END { if (NR == 0) exit 1 }
' "$BUILD_LINKS"; then
    echo "program manifest enumeration is malformed or duplicated" >&2
    exit 1
fi
for entry in $ROLES; do
    role="${entry%%:*}"; rest="${entry#*:}"
    package="${rest%%:*}"
    expected="$(printf '%s\t%s' "$role" "$package")"
    grep -Fqx "$expected" "$BUILD_LINKS" \
        || { echo "release role $role does not name an enumerated program package: $package" >&2; exit 1; }
done

BUILD_RUN_ID="$(python3 - <<'PY'
import secrets
print(secrets.token_hex(32))
PY
)"
printf 'dclutch-sbf-build-run-v1=%s\n' "$BUILD_RUN_ID" > "$BUILD_RUN"
: > "$BUILD_LOG"
: > "$WORK/build-diagnostics.txt"

while IFS=$'\t' read -r label package; do
    stem=""
    for entry in $ROLES; do
        role="${entry%%:*}"; rest="${entry#*:}"
        role_package="${rest%%:*}"; role_stem="${rest#*:}"
        if [ "$package" = "$role_package" ]; then
            stem="$role_stem"
            break
        fi
    done
    if [ -n "$stem" ]; then
        echo "build: $label ($package)"
    else
        echo "build: $package (frame gate only, not a release artifact)"
    fi
    link_log="$WORK/build-$label.log"
    link_target="$(target_dir_for "$package")"
    printf 'dclutch-sbf-build-run-v1=%s\n' "$BUILD_RUN_ID" > "$link_log"
    (
        cd "$SOURCE"
        CARGO_TERM_COLOR=never CARGO_TARGET_DIR="$link_target" \
            cargo build-sbf --manifest-path "programs/$package/Cargo.toml" -- --locked
    ) >>"$link_log" 2>&1
    cat "$link_log" >> "$BUILD_LOG"
    count="$(grep -c "$DIAGNOSTIC_PATTERN" "$link_log" || true)"
    printf '%s=%s\n' "$label" "$count" >> "$WORK/build-diagnostics.txt"
    if [ "$count" != "0" ]; then
        echo "BUILD DIAGNOSTIC: $label emitted $count SBF stack-frame overwrite reports" >&2
        grep "$DIAGNOSTIC_PATTERN" "$link_log" | sort -u >&2
    fi
    if [ -n "$stem" ]; then
        cp "$link_target/deploy/$stem.so" "$ELF_DIR/$label.so"
    fi
done < "$BUILD_LINKS"

FRESHNESS_RESULT="$("$FRESHNESS_CHECKER" \
    --work "$WORK" \
    --expected "$BUILD_LINKS" \
    --diagnostics "$WORK/build-diagnostics.txt" \
    --run-id "$BUILD_RUN_ID")" \
    || { echo "refusing: SBF build freshness gate failed" >&2; exit 1; }
printf '%s\n' "$FRESHNESS_RESULT"
printf '%s\n' "$FRESHNESS_RESULT" >> "$BUILD_LOG"
BUILD_LINK_COUNT="$(wc -l < "$BUILD_LINKS" | tr -d ' ')"
if [ "$BUILD_LINK_COUNT" != "13" ]; then
    echo "refusing: checked Upgrade admission requires the exact 13-link shipped set; enumerated $BUILD_LINK_COUNT" >&2
    exit 1
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

# --------------------------------------------------------- exact frame gate
# This is a separate measurement build. `-Zemit-stack-sizes` adds measurement
# sections, so its linked artifact is never copied into the shipped ELF set.
# Each link gets a new target root so the top-package compile marker is
# load-bearing here too: a warm object cannot masquerade as a fresh report.
if [ "$DIAGNOSTIC_TOTAL" = "0" ] && [ "$ALLOW_DIAGNOSTICS" = "false" ]; then
    while IFS=$'\t' read -r label package; do
        frame_target="$WORK/frame-target-$label"
        frame_build_log="$WORK/frame-build-$label.log"
        frame_raw="$FRAME_DIR/$label.raw.txt"
        frame_report="$FRAME_DIR/$label.txt"
        rm -rf "$frame_target"
        printf 'dclutch-sbf-frame-run-v1=%s\n' "$BUILD_RUN_ID" > "$frame_build_log"
        (
            cd "$SOURCE"
            RUSTC_BOOTSTRAP=1 RUSTFLAGS="-Zemit-stack-sizes --emit=obj,link" \
                CARGO_TERM_COLOR=never CARGO_TARGET_DIR="$frame_target" \
                cargo build-sbf --manifest-path "programs/$package/Cargo.toml" -- --locked
        ) >> "$frame_build_log" 2>&1
        frame_compile_marker="$(grep -E "^[[:space:]]*Compiling[[:space:]]+$package[[:space:]]+v[^[:space:]]+" "$frame_build_log" | tail -n 1 || true)"
        if [ -z "$frame_compile_marker" ]; then
            echo "refusing: frame build for $label has no fresh top-package compile marker for $package" >&2
            exit 1
        fi
        frame_diagnostics="$(grep -c "$DIAGNOSTIC_PATTERN" "$frame_build_log" || true)"
        if [ "$frame_diagnostics" != "0" ]; then
            echo "refusing: frame measurement build for $label emitted $frame_diagnostics stack-frame overwrite diagnostics" >&2
            exit 1
        fi
        object_stem="$(printf '%s' "$package" | tr '-' '_')"
        frame_object="$frame_target/$TARGET_TRIPLE/release/deps/$object_stem.o"
        [ -f "$frame_object" ] \
            || { echo "refusing: frame measurement object is missing for $label: $frame_object" >&2; exit 1; }
        python3 "$SOURCE/tools/sbf-frame-sizes.py" --top 8 "$frame_object" > "$frame_raw"
        frame_count="$(sed -n 's/^  \([0-9][0-9]*\) measured frames.*/\1/p' "$frame_raw")"
        deepest_frame="$(sed -n '2s/^ *\([0-9][0-9]*\) .*/\1/p' "$frame_raw")"
        if [ -z "$frame_count" ] || [ -z "$deepest_frame" ]; then
            echo "refusing: frame report for $label did not expose canonical count/deepest fields" >&2
            exit 1
        fi
        {
            printf 'dclutch-sbf-frame-report-v1\n'
            printf 'label=%s\n' "$label"
            printf 'package=%s\n' "$package"
            printf 'frame_count=%s\n' "$frame_count"
            printf 'frame_bound_bytes=4096\n'
            printf 'frames_at_or_over_bound=0\n'
            printf 'deepest_frame_bytes=%s\n' "$deepest_frame"
            printf 'object_sha256=%s\n' "$(sha256 "$frame_object")"
            printf 'measurement_output:\n'
            cat "$frame_raw"
        } > "$frame_report"
    done < "$BUILD_LINKS"
else
    echo "checked Upgrade gate: not emitted because zero diagnostics in strict mode were not established" >&2
fi

# --------------------------------------------------------------- release tool
if [ -z "$TOOL" ]; then
    TOOL="$HOST_TARGET/release/dclutch-release-tool"
    ( cd "$SOURCE" && CARGO_TARGET_DIR="$HOST_TARGET" \
        cargo build --release --locked --offline -p dclutch-release-tool ) >>"$BUILD_LOG" 2>&1
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
        printf 'build_command=cargo build-sbf --manifest-path programs/%s/Cargo.toml -- --locked\n' "$package"
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

# Do this after every source-tree Cargo invocation, including the host release
# tool. `--locked` should make mutation impossible; byte-compare the complete
# set anyway so the candidate carries the proof instead of relying on intent.
write_lock_manifest "$SOURCE" "$LOCKS_AFTER"
if ! cmp -s "$LOCKS_BEFORE" "$LOCKS_AFTER"; then
    echo "refusing: Cargo.lock set changed while building the candidate" >&2
    diff -u "$LOCKS_BEFORE" "$LOCKS_AFTER" >&2 || true
    exit 1
fi

# ------------------------------------------------------------------- summary
{
    printf 'format=dclutch-checked-release-candidate-summary-v1\n'
    printf 'evidence_level=local-reproducible-release-candidate\n'
    printf 'not_a_deployment=true\n'
    printf 'source_revision=%s\n' "$SOURCE_REVISION"
    printf 'source_digest=%s\n' "$SOURCE_DIGEST"
    printf 'root_cargo_lock_digest=%s\n' "$ROOT_LOCK_DIGEST"
    printf 'cargo_lock_count=%s\n' "$LOCK_COUNT"
    printf 'cargo_lock_set_sha256=%s\n' "$LOCK_SET_DIGEST"
    printf 'cargo_lock_immutability=passed\n'
    printf 'rustc_version=%s\n' "$RUSTC_VERSION"
    printf 'solana_version=%s\n' "$SOLANA_VERSION"
    printf 'cargo_build_sbf_version=%s\n' "$BUILD_SBF_VERSION"
    printf 'target_triple=%s\n' "$TARGET_TRIPLE"
    printf 'loader_program_id=%s\n' "$LOADER_HEX"
    printf 'sbf_build_freshness=passed\n'
    printf 'sbf_build_freshness_links=%s\n' "$BUILD_LINK_COUNT"
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

# ---------------------------------------------------- checked Upgrade gate
# The Upgrade command accepts this generated receipt, never an operator-written
# boolean. Paths are canonical root-relative names so the complete evidence
# directory may be transferred as a unit; the verifier resolves them, refuses
# symlinks/escapes, and rehashes every named file.
if [ "$DIAGNOSTIC_TOTAL" = "0" ] && [ "$ALLOW_DIAGNOSTICS" = "false" ] \
    && [ "$PREBUILT_TOOL" = "false" ]; then
    GATE_ROOT="$WORK" GATE_ROLES="$ROLES" GATE_SOURCE_REVISION="$SOURCE_REVISION" \
    GATE_SOURCE_TREE_SHA256="$SOURCE_DIGEST" GATE_SOLANA_VERSION="$SOLANA_VERSION" \
    GATE_BUILD_RUN_ID="$BUILD_RUN_ID" python3 - <<'PY'
import hashlib
import json
import os
import re
from pathlib import Path

root = Path(os.environ["GATE_ROOT"]).resolve(strict=True)
roles = {}
for entry in os.environ["GATE_ROLES"].split():
    role, package, _stem = entry.split(":", 2)
    roles[package] = role

def evidence(relative: str) -> dict:
    path = root / relative
    if path.is_symlink() or not path.is_file():
        raise SystemExit(f"gate evidence is not one regular file: {path}")
    resolved = path.resolve(strict=True)
    if resolved.parent != root and root not in resolved.parents:
        raise SystemExit(f"gate evidence escapes root: {path}")
    data = path.read_bytes()
    return {
        "canonical_path": relative,
        "bytes": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
    }

links = []
diagnostics = {}
for line in (root / "build-diagnostics.txt").read_text().splitlines():
    label, count = line.split("=", 1)
    diagnostics[label] = int(count)

for row in (root / "build-links.tsv").read_text().splitlines():
    label, package = row.split("\t")
    build_log_path = root / f"build-{label}.log"
    compile_lines = [
        line for line in build_log_path.read_text().splitlines()
        if re.match(rf"^\s*Compiling\s+{re.escape(package)}\s+v\S+(?:\s|$)", line)
    ]
    if not compile_lines:
        raise SystemExit(f"missing canonical compile marker for {label}")
    frame_build_log_path = root / f"frame-build-{label}.log"
    frame_compile_lines = [
        line for line in frame_build_log_path.read_text().splitlines()
        if re.match(rf"^\s*Compiling\s+{re.escape(package)}\s+v\S+(?:\s|$)", line)
    ]
    if not frame_compile_lines:
        raise SystemExit(f"missing canonical frame compile marker for {label}")
    frame_fields = {}
    for line in (root / "frame" / f"{label}.txt").read_text().splitlines()[1:8]:
        key, value = line.split("=", 1)
        frame_fields[key] = value
    role = roles.get(package)
    links.append({
        "label": label,
        "package": package,
        "build_log": evidence(f"build-{label}.log"),
        "compile_marker": compile_lines[-1],
        "sbf_diagnostics_count": diagnostics[label],
        "frame_build_log": evidence(f"frame-build-{label}.log"),
        "frame_compile_marker": frame_compile_lines[-1],
        "frame_report": evidence(f"frame/{label}.txt"),
        "frame_count": int(frame_fields["frame_count"]),
        "frame_bound_bytes": int(frame_fields["frame_bound_bytes"]),
        "frames_at_or_over_bound": int(frame_fields["frames_at_or_over_bound"]),
        "deepest_frame_bytes": int(frame_fields["deepest_frame_bytes"]),
        "elf": evidence(f"elf/{role}.so") if role else None,
        "checked_manifest": evidence(f"evidence/{role}/checked.bin") if role else None,
    })

gate = {
    "schema": "dclutch-checked-upgrade-gate-v1",
    "source_revision": os.environ["GATE_SOURCE_REVISION"],
    "source_tree_sha256": os.environ["GATE_SOURCE_TREE_SHA256"],
    "solana_cli_version": os.environ["GATE_SOLANA_VERSION"],
    "build_run_id": os.environ["GATE_BUILD_RUN_ID"],
    "link_count": len(links),
    "source_tree_manifest": evidence("source-tree.txt"),
    "build_links_manifest": evidence("build-links.tsv"),
    "build_run_manifest": evidence("build-run.txt"),
    "diagnostics_manifest": evidence("build-diagnostics.txt"),
    "links": links,
}
target = root / "CHECKED_UPGRADE_GATE.json"
temporary = root / ".CHECKED_UPGRADE_GATE.json.tmp"
temporary.write_text(json.dumps(gate, indent=2, sort_keys=True) + "\n")
os.replace(temporary, target)
print(f"checked Upgrade gate sha256={hashlib.sha256(target.read_bytes()).hexdigest()}")
PY
    printf 'checked_upgrade_gate_sha256=%s\n' "$(sha256 "$UPGRADE_GATE")" >> "$SUMMARY"
elif [ "$PREBUILT_TOOL" = "true" ]; then
    echo "checked Upgrade gate: not emitted because --tool is not a source-pinned host-tool build" >&2
fi

echo
echo "summary: $SUMMARY"
if [ -f "$UPGRADE_GATE" ]; then
    echo "checked Upgrade gate: $UPGRADE_GATE"
fi
