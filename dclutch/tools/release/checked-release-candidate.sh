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
  --builder NAME execution substrate label: local, persvati, or hbox
                 (default: local; hbox refuses unless inside swarm-build)
  --node PATH    absolute canonical Node v26.4.0 executable extracted from the
                 official platform archive named by --node-archive. REQUIRED.
  --node-archive PATH
                 absolute canonical official Node v26.4.0 .tar.xz archive.
                 REQUIRED; its URL, SHA-256, Node, and sibling npm are pinned.
  --predecessor-profile PATH
                 the dumped 144-byte infrastructure profile account this
                 candidate's succession succeeds. Required for a SUCCESSION
                 candidate, and the one input that cannot be built from source:
                 a succession is not a function of the successor alone, so the
                 predecessor's own two binding ids -- which the ceremony copies
                 into the profile it commits -- have to be read from the chain
                 being succeeded.
  --genesis-cohort
                 build a GENESIS candidate instead: one that succeeds nothing,
                 founding infrastructure on a fresh cohort rather than moving
                 an existing one. Mutually exclusive with --predecessor-profile
                 and exactly one of the two is required, so the lineage is
                 always a stated choice and never a default. A genesis
                 candidate emits the write-once V1 profile that
                 InitializeProtocolInfrastructureV1 commits, records
                 infrastructure_lineage=genesis in its summary, and -- being a
                 function of its own manifests alone -- is the only one of the
                 two a COLD MACHINE with no network can build.
  --keep-elf     legacy option; refused because reused ELFs have no fresh-build proof
  --allow-build-diagnostics
                 admit artifacts whose SBF build emitted a stack-frame
                 diagnostic, recording the exact counts in the summary
  -h, --help     show this message

On hbox, wrap this whole command once; this script never calls swarm-build:
  SWARM_MEM_MAX=32G swarm-build tools/release/checked-release-candidate.sh --builder hbox ...
USAGE
}

REPO=""
WORK="/private/tmp/dclutch-release-candidate"
TOOL=""
PREBUILT_TOOL="false"
COMMIT="HEAD"
BUILDER="local"
PREDECESSOR_PROFILE=""
GENESIS_COHORT="false"
NODE=""
NODE_ARCHIVE=""
KEEP_ELF="false"
ALLOW_DIAGNOSTICS="false"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --repo) REPO="${2:?--repo needs a value}"; shift 2 ;;
        --work) WORK="${2:?--work needs a value}"; shift 2 ;;
        --tool) TOOL="${2:?--tool needs a value}"; PREBUILT_TOOL="true"; shift 2 ;;
        --commit) COMMIT="${2:?--commit needs a value}"; shift 2 ;;
        --builder) BUILDER="${2:?--builder needs a value}"; shift 2 ;;
        --node) NODE="${2:?--node needs a value}"; shift 2 ;;
        --node-archive) NODE_ARCHIVE="${2:?--node-archive needs a value}"; shift 2 ;;
        --predecessor-profile) PREDECESSOR_PROFILE="${2:?--predecessor-profile needs a value}"; shift 2 ;;
        --genesis-cohort) GENESIS_COHORT="true"; shift ;;
        --keep-elf) KEEP_ELF="true"; shift ;;
        --allow-build-diagnostics) ALLOW_DIAGNOSTICS="true"; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NODE_ARCHIVE_LISTER="$SCRIPT_DIR/node_archive_members.py"
[ -x "$NODE_ARCHIVE_LISTER" ] \
    || { echo "Node archive member lister not executable: $NODE_ARCHIVE_LISTER" >&2; exit 1; }

case "$BUILDER" in
    local|persvati) ;;
    hbox)
        [ "${SWARM_BUILD_INNER:-}" = "1" ] \
            || { echo "--builder hbox requires the whole runner to execute inside swarm-build" >&2; exit 2; }
        ;;
    *) echo "--builder must be local, persvati, or hbox" >&2; exit 2 ;;
esac

if [ -z "$REPO" ]; then
    REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fi
case "$WORK" in /*) ;; *) echo "--work must be absolute" >&2; exit 2 ;; esac
if [ "$KEEP_ELF" = "true" ]; then
    echo "refusing --keep-elf: a checked release requires a fresh top-package compile marker for every SBF link; use a new --work root" >&2
    exit 2
fi
# The candidate's infrastructure lineage is a stated choice, never a default,
# and the two states are not interchangeable: a SUCCESSION emits the V2 profile
# that pins the two predecessor artifact-release ids read from the chain being
# succeeded, while a GENESIS emits the write-once V1 profile a cohort that
# succeeds nothing commits. Requiring exactly one means neither can be reached
# by omission, so no summary can describe a founding as a succession or the
# reverse.
if [ "$GENESIS_COHORT" = "true" ] && [ -n "$PREDECESSOR_PROFILE" ]; then
    echo "--genesis-cohort and --predecessor-profile are mutually exclusive: a genesis cohort succeeds nothing, so it has no predecessor account to pin" >&2
    exit 2
fi
if [ "$GENESIS_COHORT" = "false" ] && [ -z "$PREDECESSOR_PROFILE" ]; then
    echo "--predecessor-profile is required for a succession candidate: the checked infrastructure evidence describes a succession, and the predecessor account it succeeds cannot be derived from source. For a cohort that succeeds nothing, pass --genesis-cohort instead" >&2
    exit 2
fi
if [ "$GENESIS_COHORT" = "false" ]; then
case "$PREDECESSOR_PROFILE" in
    /*) ;;
    *) echo "--predecessor-profile must be an absolute canonical path" >&2; exit 2 ;;
esac
[ -f "$PREDECESSOR_PROFILE" ] \
    || { echo "--predecessor-profile is not a readable file: $PREDECESSOR_PROFILE" >&2; exit 2; }
[ ! -L "$PREDECESSOR_PROFILE" ] \
    || { echo "--predecessor-profile must not be a symlink: $PREDECESSOR_PROFILE" >&2; exit 2; }
PREDECESSOR_PARENT="$(cd "$(dirname "$PREDECESSOR_PROFILE")" && pwd -P)"
PREDECESSOR_CANONICAL="$PREDECESSOR_PARENT/$(basename "$PREDECESSOR_PROFILE")"
[ "$PREDECESSOR_PROFILE" = "$PREDECESSOR_CANONICAL" ] \
    || { echo "--predecessor-profile must be an absolute canonical path" >&2; exit 2; }
PREDECESSOR_PROFILE_BYTES="$(wc -c < "$PREDECESSOR_PROFILE" | tr -d ' ')"
[ "$PREDECESSOR_PROFILE_BYTES" = "144" ] \
    || { echo "--predecessor-profile must be exactly 144 bytes, got $PREDECESSOR_PROFILE_BYTES" >&2; exit 2; }
fi

# The public Product handoff is a checked candidate gate, so its JS runtime is
# an admitted build input rather than an ambient PATH choice. Both final Linux
# builders use one official v26.4.0 archive. Local macOS/arm64 remains useful
# for diagnostics but is not a member of the cross-builder release pair.
[ -n "$NODE" ] || { echo "--node is required for the source-pinned Product handoff gate" >&2; exit 2; }
[ -n "$NODE_ARCHIVE" ] || { echo "--node-archive is required for the source-pinned Product handoff gate" >&2; exit 2; }
case "$NODE" in /*) ;; *) echo "--node must be an absolute canonical path" >&2; exit 2 ;; esac
case "$NODE_ARCHIVE" in /*) ;; *) echo "--node-archive must be an absolute canonical path" >&2; exit 2 ;; esac
[ -f "$NODE" ] && [ -x "$NODE" ] && [ ! -L "$NODE" ] \
    || { echo "--node must be an executable regular non-symlink file" >&2; exit 2; }
[ -f "$NODE_ARCHIVE" ] && [ ! -L "$NODE_ARCHIVE" ] \
    || { echo "--node-archive must be a regular non-symlink file" >&2; exit 2; }
NODE_CANONICAL="$(cd "$(dirname "$NODE")" && pwd -P)/$(basename "$NODE")"
NODE_ARCHIVE_CANONICAL="$(cd "$(dirname "$NODE_ARCHIVE")" && pwd -P)/$(basename "$NODE_ARCHIVE")"
[ "$NODE" = "$NODE_CANONICAL" ] \
    || { echo "--node must be an absolute canonical path" >&2; exit 2; }
[ "$NODE_ARCHIVE" = "$NODE_ARCHIVE_CANONICAL" ] \
    || { echo "--node-archive must be an absolute canonical path" >&2; exit 2; }

case "$(uname -s):$(uname -m)" in
    Linux:x86_64)
        NODE_ARCHIVE_NAME="node-v26.4.0-linux-x64.tar.xz"
        NODE_ARCHIVE_EXPECTED_SHA256="5c4286dcd5bbd5acb1ccc7eb0e088bd5eb1e3affad671ee9364004f8f6a4a431"
        ;;
    Darwin:arm64)
        NODE_ARCHIVE_NAME="node-v26.4.0-darwin-arm64.tar.xz"
        NODE_ARCHIVE_EXPECTED_SHA256="bef4c7e75087c029835f519a7ba640eba52fa617fadb3a9049828ff3b45b57dd"
        ;;
    *)
        echo "unsupported Product-handoff Node platform: $(uname -s) $(uname -m)" >&2
        exit 2
        ;;
esac
[ "$(basename "$NODE_ARCHIVE")" = "$NODE_ARCHIVE_NAME" ] \
    || { echo "--node-archive must be the official $NODE_ARCHIVE_NAME archive" >&2; exit 2; }
NODE_ARCHIVE_SOURCE="https://nodejs.org/dist/v26.4.0/$NODE_ARCHIVE_NAME"
NODE_ARCHIVE_SHA256="$(shasum -a 256 "$NODE_ARCHIVE" | cut -d' ' -f1)"
[ "$NODE_ARCHIVE_SHA256" = "$NODE_ARCHIVE_EXPECTED_SHA256" ] \
    || { echo "--node-archive SHA-256 differs from the pinned official v26.4.0 archive" >&2; exit 2; }
NODE_DIST_ROOT="${NODE_ARCHIVE_NAME%.tar.xz}"
NODE_ARCHIVE_MEMBER="$NODE_DIST_ROOT/bin/node"
NPM_ARCHIVE_MEMBER="$NODE_DIST_ROOT/lib/node_modules/npm/bin/npm-cli.js"
NODE_ARCHIVE_LISTING="$(mktemp "${TMPDIR:-/tmp}/dclutch-node-archive-members.XXXXXX")"
cleanup_node_archive_listing() { rm -f "$NODE_ARCHIVE_LISTING"; }
trap cleanup_node_archive_listing EXIT
python3 "$NODE_ARCHIVE_LISTER" \
    --archive "$NODE_ARCHIVE" \
    --required "$NODE_ARCHIVE_MEMBER" \
    --required "$NPM_ARCHIVE_MEMBER" \
    > "$NODE_ARCHIVE_LISTING" \
    || { echo "--node-archive has no bounded canonical member listing" >&2; exit 2; }
[ "$(grep -Fxc "$NODE_ARCHIVE_MEMBER" "$NODE_ARCHIVE_LISTING")" = "1" ] \
    || { echo "--node-archive omitted or repeated $NODE_ARCHIVE_MEMBER" >&2; exit 2; }
[ "$(grep -Fxc "$NPM_ARCHIVE_MEMBER" "$NODE_ARCHIVE_LISTING")" = "1" ] \
    || { echo "--node-archive omitted or repeated $NPM_ARCHIVE_MEMBER" >&2; exit 2; }
NODE_SHA256="$(shasum -a 256 "$NODE" | cut -d' ' -f1)"
NODE_ARCHIVE_BINARY_SHA256="$(tar -xOf "$NODE_ARCHIVE" "$NODE_ARCHIVE_MEMBER" | shasum -a 256 | cut -d' ' -f1)"
[ "$NODE_SHA256" = "$NODE_ARCHIVE_BINARY_SHA256" ] \
    || { echo "--node bytes do not equal the executable in --node-archive" >&2; exit 2; }
NODE_DIR="$(dirname "$NODE")"
NPM_BIN="$NODE_DIR/npm"
[ -x "$NPM_BIN" ] || { echo "--node has no executable sibling npm: $NPM_BIN" >&2; exit 2; }
NPM_RESOLVED="$(python3 - "$NPM_BIN" <<'PY'
import pathlib, sys
print(pathlib.Path(sys.argv[1]).resolve(strict=True))
PY
)"
NPM_EXPECTED="$(cd "$NODE_DIR/.." && pwd -P)/lib/node_modules/npm/bin/npm-cli.js"
[ "$NPM_RESOLVED" = "$NPM_EXPECTED" ] \
    || { echo "sibling npm does not resolve inside the admitted Node distribution" >&2; exit 2; }
NPM_SHA256="$(shasum -a 256 "$NPM_RESOLVED" | cut -d' ' -f1)"
NPM_ARCHIVE_SHA256="$(tar -xOf "$NODE_ARCHIVE" "$NPM_ARCHIVE_MEMBER" | shasum -a 256 | cut -d' ' -f1)"
[ "$NPM_SHA256" = "$NPM_ARCHIVE_SHA256" ] \
    || { echo "sibling npm bytes do not equal npm-cli.js in --node-archive" >&2; exit 2; }
NODE_VERSION="$("$NODE" --version)"
[ "$NODE_VERSION" = "v26.4.0" ] \
    || { echo "--node must report exactly v26.4.0, got $NODE_VERSION" >&2; exit 2; }
NPM_VERSION="$(env PATH="$NODE_DIR:$PATH" "$NPM_BIN" --version)"

FRESHNESS_CHECKER="$SCRIPT_DIR/check_sbf_build_freshness.py"
PROVENANCE_TOOL="$SCRIPT_DIR/artifact_provenance.py"
CAMPAIGN_PACK_TOOL="$SCRIPT_DIR/successor_campaign_pack.py"
[ -x "$FRESHNESS_CHECKER" ] \
    || { echo "build-freshness checker not executable: $FRESHNESS_CHECKER" >&2; exit 1; }
[ -x "$PROVENANCE_TOOL" ] \
    || { echo "artifact-provenance tool not executable: $PROVENANCE_TOOL" >&2; exit 1; }
[ -x "$CAMPAIGN_PACK_TOOL" ] \
    || { echo "successor campaign-pack tool not executable: $CAMPAIGN_PACK_TOOL" >&2; exit 1; }

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

# Diagnostic-only Trading build profile.  Keep this as one package-scoped
# suffix so the command recorded in every evidence surface is the command that
# ran, while the other checked links remain byte-for-byte on their ordinary
# release invocation.
HOT_CU_PROFILE_FEATURE="--features hot-cu-profile"
sbf_feature_suffix() {
    if [ "$1" = "dclutch-trading-sbf" ]; then
        printf ' %s' "$HOT_CU_PROFILE_FEATURE"
    fi
}

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
PROVENANCE_DIR="$WORK/provenance"
PRODUCT_HANDOFF_DIR="$WORK/product-handoff"
PRODUCT_BUILD_DIR="$WORK/product-handoff-build"
TOOLCHAIN_DIR="$WORK/toolchain"

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
echo "builder: $BUILDER"

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
CAMPAIGN_PACK="$WORK/SUCCESSOR_CAMPAIGN_PACK.json"
PROVENANCE_DIR="$WORK/provenance"
PRODUCT_HANDOFF_DIR="$WORK/product-handoff"
PRODUCT_BUILD_DIR="$WORK/product-handoff-build"
TOOLCHAIN_DIR="$WORK/toolchain"
rm -f "$UPGRADE_GATE" "$CAMPAIGN_PACK" "$SUMMARY"
rm -rf "$EVIDENCE" "$SET_DIR" "$INFRA_DIR" "$ELF_DIR" "$FRAME_DIR" "$PROVENANCE_DIR" \
    "$PRODUCT_HANDOFF_DIR" "$PRODUCT_BUILD_DIR" "$TOOLCHAIN_DIR"
mkdir -p "$EVIDENCE" "$SET_DIR" "$INFRA_DIR" "$ELF_DIR" "$FRAME_DIR" "$PROVENANCE_DIR" \
    "$PRODUCT_HANDOFF_DIR" "$PRODUCT_BUILD_DIR" "$TOOLCHAIN_DIR"

# Preserve the external runtime distribution exactly like the predecessor
# profile: downstream verifiers rehash pack-owned bytes, never a host cache.
PINNED_NODE_ARCHIVE="$TOOLCHAIN_DIR/$NODE_ARCHIVE_NAME"
cp "$NODE_ARCHIVE" "$PINNED_NODE_ARCHIVE"
cmp -s "$NODE_ARCHIVE" "$PINNED_NODE_ARCHIVE" \
    || { echo "copied Node archive differs from its admitted input" >&2; exit 1; }

# Succession is a function of both the frozen source and the public predecessor
# account state. Preserve that otherwise-external input inside the candidate
# before deriving anything, then use only the preserved copy downstream.
#
# A genesis candidate has no such external input at all -- which is precisely
# what makes it, and only it, buildable on a cold machine with no network.
PINNED_PREDECESSOR_PROFILE="$INFRA_DIR/predecessor-profile.bin"
if [ "$GENESIS_COHORT" = "false" ]; then
    cp "$PREDECESSOR_PROFILE" "$PINNED_PREDECESSOR_PROFILE"
    cmp -s "$PREDECESSOR_PROFILE" "$PINNED_PREDECESSOR_PROFILE" \
        || { echo "copied predecessor profile differs from its admitted input" >&2; exit 1; }
fi

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
cmp -s "$SCRIPT_DIR/artifact_provenance.py" \
    "$SOURCE/tools/release/artifact_provenance.py" \
    || { echo "refusing: artifact-provenance tool differs from the exact --commit source revision" >&2; exit 1; }
cmp -s "$SCRIPT_DIR/successor_campaign_pack.py" \
    "$SOURCE/tools/release/successor_campaign_pack.py" \
    || { echo "refusing: successor campaign-pack tool differs from the exact --commit source revision" >&2; exit 1; }
cmp -s "$SCRIPT_DIR/node_archive_members.py" \
    "$SOURCE/tools/release/node_archive_members.py" \
    || { echo "refusing: Node archive member lister differs from the exact --commit source revision" >&2; exit 1; }
FRESHNESS_CHECKER="$SOURCE/tools/release/check_sbf_build_freshness.py"
PROVENANCE_TOOL="$SOURCE/tools/release/artifact_provenance.py"
CAMPAIGN_PACK_TOOL="$SOURCE/tools/release/successor_campaign_pack.py"
NODE_ARCHIVE_LISTER="$SOURCE/tools/release/node_archive_members.py"

ROOT_LOCK_DIGEST="$(sha256 "$SOURCE/Cargo.lock")"

# ------------------------------------------------------------------ toolchain
SOLANA_VERSION="$(solana --version | head -n 1)"
BUILD_SBF_RAW="$(cargo-build-sbf --version)"
BUILD_SBF_VERSION="$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '1p')"
PLATFORM_TOOLS="$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '2p')"
SBF_RUSTC="$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '3p')"
RUSTC_VERSION="$SBF_RUSTC (solana $PLATFORM_TOOLS)"
HOST_RUSTC_VERSION="$(cd "$SOURCE" && rustc --version)"
HOST_RUSTC_VERBOSE_SHA256="$(cd "$SOURCE" && rustc -Vv | sha256_stdin)"
HOST_CARGO_VERSION="$(cd "$SOURCE" && cargo --version)"
HOST_CC_VERSION="$(cc --version | sed -n '1p')"
HOST_OS="$(uname -s)"
HOST_ARCH="$(uname -m)"
HOST_KERNEL="$(uname -r)"
case "$HOST_OS" in
    Linux)
        HOST_LINKER_VERSION="$(ld --version | sed -n '1p')"
        HOST_LIBC_VERSION="$(ldd --version | sed -n '1p')"
        ;;
    Darwin)
        HOST_LINKER_VERSION="$(ld -v 2>&1 || true)"
        HOST_LINKER_VERSION="$(printf '%s\n' "$HOST_LINKER_VERSION" | sed -n '1p')"
        HOST_LIBC_VERSION="libSystem (macOS $(sw_vers -productVersion))"
        ;;
    *)
        echo "unsupported checked-candidate host substrate: $HOST_OS $HOST_ARCH" >&2
        exit 1
        ;;
esac
[ -n "$HOST_LINKER_VERSION" ] && [ -n "$HOST_LIBC_VERSION" ] \
    || { echo "could not identify checked-candidate linker/libc substrate" >&2; exit 1; }

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
    build_target_relative="${link_target#"$WORK"/}"
    build_feature_suffix="$(sbf_feature_suffix "$package")"
    build_invocation="CARGO_TERM_COLOR=never CARGO_TARGET_DIR=$build_target_relative cargo build-sbf --manifest-path programs/$package/Cargo.toml$build_feature_suffix -- --locked"
    printf 'dclutch-sbf-build-run-v1=%s\n' "$BUILD_RUN_ID" > "$link_log"
    printf 'dclutch-sbf-build-invocation-v1=%s\n' "$build_invocation" >> "$link_log"
    if [ -n "$stem" ]; then
        rm -f "$link_target/deploy/$stem.so"
    fi
    (
        cd "$SOURCE"
        if [ "$package" = "dclutch-trading-sbf" ]; then
            CARGO_TERM_COLOR=never CARGO_TARGET_DIR="$link_target" \
                cargo build-sbf --manifest-path "programs/$package/Cargo.toml" \
                    --features hot-cu-profile -- --locked
        else
            CARGO_TERM_COLOR=never CARGO_TARGET_DIR="$link_target" \
                cargo build-sbf --manifest-path "programs/$package/Cargo.toml" -- --locked
        fi
    ) >>"$link_log" 2>&1
    cat "$link_log" >> "$BUILD_LOG"
    count="$(grep -c "$DIAGNOSTIC_PATTERN" "$link_log" || true)"
    printf '%s=%s\n' "$label" "$count" >> "$WORK/build-diagnostics.txt"
    if [ "$count" != "0" ]; then
        echo "BUILD DIAGNOSTIC: $label emitted $count SBF stack-frame overwrite reports" >&2
        grep "$DIAGNOSTIC_PATTERN" "$link_log" | sort -u >&2
    fi
    if [ -n "$stem" ]; then
        [ -f "$link_target/deploy/$stem.so" ] && [ ! -L "$link_target/deploy/$stem.so" ] \
            || { echo "refusing: fresh build did not emit one regular $stem.so for $label" >&2; exit 1; }
        cp "$link_target/deploy/$stem.so" "$ELF_DIR/$label.so"
        [ -f "$ELF_DIR/$label.so" ] && [ ! -L "$ELF_DIR/$label.so" ] \
            || { echo "refusing: staged named-role ELF is not regular for $label" >&2; exit 1; }
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
        frame_feature_suffix="$(sbf_feature_suffix "$package")"
        frame_invocation="RUSTC_BOOTSTRAP=1 RUSTFLAGS='-Zemit-stack-sizes --emit=obj,link' CARGO_TERM_COLOR=never CARGO_TARGET_DIR=frame-target-$label cargo build-sbf --manifest-path programs/$package/Cargo.toml$frame_feature_suffix -- --locked"
        printf 'dclutch-sbf-frame-run-v1=%s\n' "$BUILD_RUN_ID" > "$frame_build_log"
        printf 'dclutch-sbf-frame-invocation-v1=%s\n' "$frame_invocation" >> "$frame_build_log"
        (
            cd "$SOURCE"
            if [ "$package" = "dclutch-trading-sbf" ]; then
                RUSTC_BOOTSTRAP=1 RUSTFLAGS="-Zemit-stack-sizes --emit=obj,link" \
                    CARGO_TERM_COLOR=never CARGO_TARGET_DIR="$frame_target" \
                    cargo build-sbf --manifest-path "programs/$package/Cargo.toml" \
                        --features hot-cu-profile -- --locked
            else
                RUSTC_BOOTSTRAP=1 RUSTFLAGS="-Zemit-stack-sizes --emit=obj,link" \
                    CARGO_TERM_COLOR=never CARGO_TARGET_DIR="$frame_target" \
                    cargo build-sbf --manifest-path "programs/$package/Cargo.toml" -- --locked
            fi
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
            printf 'source_tree_sha256=%s\n' "$SOURCE_DIGEST"
            printf 'build_run_id=%s\n' "$BUILD_RUN_ID"
            printf 'frame_count=%s\n' "$frame_count"
            printf 'frame_bound_bytes=4096\n'
            printf 'frames_at_or_over_bound=0\n'
            printf 'deepest_frame_bytes=%s\n' "$deepest_frame"
            printf 'object_sha256=%s\n' "$(sha256 "$frame_object")"
            printf 'measurement_output:\n'
            cat "$frame_raw"
        } > "$frame_report"
    done < "$BUILD_LINKS"

    # One descriptor is the only supported join between the named link, the
    # source/run, the fresh plain build, the shipped ELF, and the independent
    # frame object/report. Downstream gates and CU selectors rehash it instead
    # of rediscovering a same-looking file from an adjacent target directory.
    while IFS=$'\t' read -r label package; do
        stem=""
        for entry in $ROLES; do
            role="${entry%%:*}"; rest="${entry#*:}"
            role_package="${rest%%:*}"; role_stem="${rest#*:}"
            if [ "$package" = "$role_package" ]; then stem="$role_stem"; break; fi
        done
        link_target="$(target_dir_for "$package")"
        build_target_relative="${link_target#"$WORK"/}"
        build_feature_suffix="$(sbf_feature_suffix "$package")"
        build_invocation="CARGO_TERM_COLOR=never CARGO_TARGET_DIR=$build_target_relative cargo build-sbf --manifest-path programs/$package/Cargo.toml$build_feature_suffix -- --locked"
        frame_invocation="RUSTC_BOOTSTRAP=1 RUSTFLAGS='-Zemit-stack-sizes --emit=obj,link' CARGO_TERM_COLOR=never CARGO_TARGET_DIR=frame-target-$label cargo build-sbf --manifest-path programs/$package/Cargo.toml$build_feature_suffix -- --locked"
        build_marker="$(grep -E "^[[:space:]]*Compiling[[:space:]]+$package[[:space:]]+v[^[:space:]]+" "$WORK/build-$label.log" | tail -n 1)"
        frame_marker="$(grep -E "^[[:space:]]*Compiling[[:space:]]+$package[[:space:]]+v[^[:space:]]+" "$WORK/frame-build-$label.log" | tail -n 1)"
        object_stem="$(printf '%s' "$package" | tr '-' '_')"
        set -- emit \
            --root "$WORK" \
            --output "$PROVENANCE_DIR/$label.json" \
            --label "$label" \
            --package "$package" \
            --source-revision "$SOURCE_REVISION" \
            --source-tree-sha256 "$SOURCE_DIGEST" \
            --build-run-id "$BUILD_RUN_ID" \
            --build-invocation "$build_invocation" \
            --build-log "build-$label.log" \
            --build-compile-marker "$build_marker" \
            --diagnostics-count 0 \
            --frame-invocation "$frame_invocation" \
            --frame-build-log "frame-build-$label.log" \
            --frame-compile-marker "$frame_marker" \
            --frame-object "frame-target-$label/$TARGET_TRIPLE/release/deps/$object_stem.o" \
            --frame-report "frame/$label.txt"
        if [ -n "$stem" ]; then
            set -- "$@" --artifact-stem "$stem" --elf "elf/$label.so"
        fi
        python3 "$PROVENANCE_TOOL" "$@"
    done < "$BUILD_LINKS"
    # The emitted descriptor is the durable command surface.  This prevents a
    # diagnostic feature from silently spreading to any of the other 12 links,
    # or from being omitted from Trading's plain/frame provenance.
    profile_provenance_count="$(grep -Fl -- "$HOT_CU_PROFILE_FEATURE" "$PROVENANCE_DIR"/*.json | wc -l | tr -d ' ')"
    [ "$profile_provenance_count" = "1" ] \
        && grep -Fq -- "$HOT_CU_PROFILE_FEATURE" "$PROVENANCE_DIR/trading.json" \
        || { echo "refusing: hot-cu-profile provenance must name Trading alone" >&2; exit 1; }
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

# ----------------------------------------- public spline Product handoff gate
# Build both public layers from the archived source, then make the actual CLI
# delegate the canonical fixture to the actual Rust producer and make the SDK
# inspect every emitted record. The smoke's absolute paths are run evidence;
# its semantic/output digests are selected separately by the cross-builder
# verifier.
PRODUCT_PACKAGES="$PRODUCT_BUILD_DIR/packages"
mkdir -p "$PRODUCT_PACKAGES"
cp -R "$SOURCE/packages/dclutch-sdk" "$PRODUCT_PACKAGES/dclutch-sdk"
cp -R "$SOURCE/packages/dclutch-cli" "$PRODUCT_PACKAGES/dclutch-cli"
SDK_LOCK="$PRODUCT_PACKAGES/dclutch-sdk/package-lock.json"
CLI_LOCK="$PRODUCT_PACKAGES/dclutch-cli/package-lock.json"
SDK_LOCK_BEFORE="$(sha256 "$SDK_LOCK")"
CLI_LOCK_BEFORE="$(sha256 "$CLI_LOCK")"
PRODUCT_BUILD_LOG="$PRODUCT_HANDOFF_DIR/build.log"
: > "$PRODUCT_BUILD_LOG"
(
    cd "$PRODUCT_PACKAGES/dclutch-sdk"
    env PATH="$NODE_DIR:$PATH" "$NPM_BIN" ci --no-audit --no-fund
) >>"$PRODUCT_BUILD_LOG" 2>&1
(
    cd "$PRODUCT_PACKAGES/dclutch-cli"
    env PATH="$NODE_DIR:$PATH" "$NPM_BIN" ci --no-audit --no-fund
    env PATH="$NODE_DIR:$PATH" "$NPM_BIN" run build
) >>"$PRODUCT_BUILD_LOG" 2>&1
[ "$(sha256 "$SDK_LOCK")" = "$SDK_LOCK_BEFORE" ] \
    || { echo "source-pinned SDK package-lock.json changed during Product build" >&2; exit 1; }
[ "$(sha256 "$CLI_LOCK")" = "$CLI_LOCK_BEFORE" ] \
    || { echo "source-pinned CLI package-lock.json changed during Product build" >&2; exit 1; }
CLI_BUNDLE="$PRODUCT_HANDOFF_DIR/dclutch.mjs"
cp "$PRODUCT_PACKAGES/dclutch-cli/dist/dclutch.mjs" "$CLI_BUNDLE"
[ -f "$CLI_BUNDLE" ] && [ ! -L "$CLI_BUNDLE" ] \
    || { echo "source-pinned CLI build did not emit a regular bundle" >&2; exit 1; }
PRODUCT_BOOTSTRAP_TARGET="$PRODUCT_BUILD_DIR/bootstrap-target"
(
    cd "$SOURCE"
    cargo build --release --locked --offline \
        --manifest-path "$SOURCE/tools/local-validator/bootstrap/successor/Cargo.toml" \
        --target-dir "$PRODUCT_BOOTSTRAP_TARGET"
) >>"$PRODUCT_BUILD_LOG" 2>&1
SUCCESSOR_BUILT="$PRODUCT_BOOTSTRAP_TARGET/release/dclutch-local-successor-bootstrap"
SUCCESSOR_BIN="$PRODUCT_HANDOFF_DIR/dclutch-local-successor-bootstrap"
[ -f "$SUCCESSOR_BUILT" ] && [ -x "$SUCCESSOR_BUILT" ] && [ ! -L "$SUCCESSOR_BUILT" ] \
    || { echo "source-pinned Product producer build did not emit a regular executable" >&2; exit 1; }
cp "$SUCCESSOR_BUILT" "$SUCCESSOR_BIN"
chmod 755 "$SUCCESSOR_BIN"
PRODUCT_SMOKE="$PRODUCT_HANDOFF_DIR/smoke"
"$SOURCE/tools/release/spline-product-handoff-smoke.sh" \
    --node "$NODE" \
    --cli "$CLI_BUNDLE" \
    --successor "$SUCCESSOR_BIN" \
    --work "$PRODUCT_SMOKE" >>"$PRODUCT_BUILD_LOG" 2>&1
PRODUCT_SMOKE_REPORT="$PRODUCT_SMOKE/smoke-report.json"
[ -f "$PRODUCT_SMOKE_REPORT" ] && [ ! -L "$PRODUCT_SMOKE_REPORT" ] \
    || { echo "Product handoff gate did not emit its regular machine report" >&2; exit 1; }
echo "checked: public spline Product compiler/SDK handoff"

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
        if [ "$package" = "dclutch-trading-sbf" ]; then
            printf 'build_command=cargo build-sbf --manifest-path programs/%s/Cargo.toml --features hot-cu-profile -- --locked\n' "$package"
        else
            printf 'build_command=cargo build-sbf --manifest-path programs/%s/Cargo.toml -- --locked\n' "$package"
        fi
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
# Two different chain acts, two different profile versions, one stated choice.
# The succession emits V2 and pins the predecessor's two artifact-release ids;
# the genesis emits the write-once V1 that InitializeProtocolInfrastructureV1
# commits at `dclutch:infrastructure:v1` on a cohort whose profile PDA is still
# vacant. Neither command can produce the other's bytes.
if [ "$GENESIS_COHORT" = "true" ]; then
    run_tool derive-genesis-infrastructure-profile \
        --registry "$EVIDENCE/registry/checked.bin" \
        --rent "$EVIDENCE/rent/checked.bin" \
        --out "$INFRA_DIR/profile.bin"
else
    run_tool derive-infrastructure-profile \
        --registry "$EVIDENCE/registry/checked.bin" \
        --rent "$EVIDENCE/rent/checked.bin" \
        --predecessor-profile "$PINNED_PREDECESSOR_PROFILE" \
        --out "$INFRA_DIR/profile.bin"
fi
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
    printf 'builder=%s\n' "$BUILDER"
    if [ "$BUILDER" = "hbox" ]; then
        printf 'builder_scheduler=swarm-build\n'
    else
        printf 'builder_scheduler=direct\n'
    fi
    printf 'source_revision=%s\n' "$SOURCE_REVISION"
    printf 'source_digest=%s\n' "$SOURCE_DIGEST"
    printf 'root_cargo_lock_digest=%s\n' "$ROOT_LOCK_DIGEST"
    printf 'cargo_lock_count=%s\n' "$LOCK_COUNT"
    printf 'cargo_lock_set_sha256=%s\n' "$LOCK_SET_DIGEST"
    printf 'cargo_lock_immutability=passed\n'
    printf 'node_version=%s\n' "$NODE_VERSION"
    printf 'npm_version=%s\n' "$NPM_VERSION"
    printf 'node_archive_source=%s\n' "$NODE_ARCHIVE_SOURCE"
    printf 'node_archive_sha256=%s\n' "$NODE_ARCHIVE_SHA256"
    printf 'node_binary_sha256=%s\n' "$NODE_SHA256"
    printf 'npm_cli_sha256=%s\n' "$NPM_SHA256"
    printf 'host_rustc_version=%s\n' "$HOST_RUSTC_VERSION"
    printf 'host_rustc_verbose_sha256=%s\n' "$HOST_RUSTC_VERBOSE_SHA256"
    printf 'host_cargo_version=%s\n' "$HOST_CARGO_VERSION"
    printf 'host_cc_version=%s\n' "$HOST_CC_VERSION"
    printf 'host_linker_version=%s\n' "$HOST_LINKER_VERSION"
    printf 'host_libc_version=%s\n' "$HOST_LIBC_VERSION"
    printf 'host_os=%s\n' "$HOST_OS"
    printf 'host_arch=%s\n' "$HOST_ARCH"
    printf 'host_kernel=%s\n' "$HOST_KERNEL"
    printf 'spline_product_handoff=passed\n'
    printf 'spline_product_handoff_report_sha256=%s\n' "$(sha256 "$PRODUCT_SMOKE_REPORT")"
    printf 'spline_product_cli_bundle_sha256=%s\n' "$(sha256 "$CLI_BUNDLE")"
    printf 'spline_product_successor_sha256=%s\n' "$(sha256 "$SUCCESSOR_BIN")"
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
    # The lineage is stated unconditionally so no reader has to infer it from
    # the presence or absence of another line, and the two states never share a
    # key: a genesis says `predecessor_infrastructure_profile=none` rather than
    # putting the word "none" where a consumer expects 64 hex digits.
    if [ "$GENESIS_COHORT" = "true" ]; then
        printf 'infrastructure_lineage=genesis\n'
        printf 'infrastructure_profile_version=1\n'
        printf 'predecessor_infrastructure_profile=none\n'
    else
        printf 'infrastructure_lineage=succession\n'
        printf 'infrastructure_profile_version=2\n'
        printf 'predecessor_infrastructure_profile_sha256=%s\n' "$(sha256 "$PINNED_PREDECESSOR_PROFILE")"
    fi
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
    python3 "$PROVENANCE_TOOL" emit-gate \
        --root "$WORK" \
        --source-revision "$SOURCE_REVISION" \
        --source-tree-sha256 "$SOURCE_DIGEST" \
        --solana-cli-version "$SOLANA_VERSION" \
        --build-run-id "$BUILD_RUN_ID"
    printf 'checked_upgrade_gate_sha256=%s\n' "$(sha256 "$UPGRADE_GATE")" >> "$SUMMARY"
elif [ "$PREBUILT_TOOL" = "true" ]; then
    echo "checked Upgrade gate: not emitted because --tool is not a source-pinned host-tool build" >&2
fi

# ------------------------------------------ successor campaign release pack
# This is the campaign-facing projection of the same checked candidate, not a
# second build or a second release authority.  It reauthenticates the complete
# all-link gate, binds source-owned compute/frame/packet and SBOM/licence
# pointers, and emits exact seven-role inputs that `materialize-spec` converts
# into the existing successor runner's `SuccessorRunSpec`.  Without a source-
# pinned Upgrade gate there is deliberately no weaker campaign pack.
if [ -f "$UPGRADE_GATE" ]; then
    python3 "$CAMPAIGN_PACK_TOOL" emit --root "$WORK"
else
    echo "successor campaign release pack: not emitted without a checked Upgrade gate" >&2
fi

echo
echo "summary: $SUMMARY"
if [ -f "$UPGRADE_GATE" ]; then
    echo "checked Upgrade gate: $UPGRADE_GATE"
fi
if [ -f "$CAMPAIGN_PACK" ]; then
    echo "successor campaign release pack: $CAMPAIGN_PACK"
fi
