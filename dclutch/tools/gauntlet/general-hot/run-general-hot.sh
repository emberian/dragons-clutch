#!/usr/bin/env bash
# The General family's Hot campaign, measured at a NAMED COMMIT.
#
#   archive at <sha> -> build six ELFs -> run the campaign -> print the CU table
#
# usage:
#   tools/gauntlet/general-hot/run-general-hot.sh [--at COMMIT] [--repo DIR]
#                                                 [--work DIR] [--keep-source]
#
# WHY --at EXISTS. Until 2026-09-04 this campaign was run by hand out of
# `docs/LETTER_TO_CLAUDE_2026_09_01.md`, from the SHARED working tree. That tree
# is dirty on purpose and continuously: on the morning this script was written it
# held in-flight edits to `programs/dclutch-claims-sbf/src/lib.rs`,
# `programs/dclutch-trading-sbf/src/hot_v3.rs` and `src/lib.rs` belonging to three
# other lanes. Every CU figure the campaign has ever published was therefore one
# ELF SET's reading and not a commit's, and no two of them are comparable. The
# frameguard runner learned this first (`tools/gate frames --at`, *"a
# capture names its commit"*); this is the same rule for the same reason, and the
# table this prints names the sha it was measured at.
#
# `--at` defaults to HEAD. It never reads the working tree: `git archive` is
# what fills the source, so an uncommitted change is invisible to this script by
# construction rather than by discipline.
#
# WORK ROOTS ARE PER-LANE. The default carries the commit in its name, so two
# lanes at two commits cannot land in one directory; pass --work to place it
# yourself. A work root this script did not create is refused rather than
# deleted -- the same guard `run-direct.sh` carries, for the same incident.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
AT=""
WORK=""
KEEP_SOURCE=false

while [ $# -gt 0 ]; do
    case "$1" in
        --at) AT="${2:?--at needs a commit}"; shift 2 ;;
        --repo) REPO="${2:?--repo needs a directory}"; shift 2 ;;
        --work) WORK="${2:?--work needs a directory}"; shift 2 ;;
        --keep-source) KEEP_SOURCE=true; shift ;;
        -h|--help) sed -n '2,27p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "general-hot: unknown argument: $1" >&2; exit 2 ;;
    esac
done

die() { echo "general-hot: $*" >&2; exit 1; }
say() { printf '\n== %s\n' "$*"; }
sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }

command -v cargo-build-sbf >/dev/null 2>&1 || die "cargo-build-sbf not found"
git -C "$REPO" rev-parse --is-inside-work-tree >/dev/null 2>&1 || die "$REPO is not a git repository"

: "${AT:=HEAD}"
REVISION="$(git -C "$REPO" rev-parse --verify "$AT^{commit}")" || die "not a commit: $AT"
: "${WORK:=/private/tmp/dclutch-general-hot/${REVISION:0:12}}"

SOURCE="$WORK/source"
DEPLOY="$WORK/deploy"
LOGS="$WORK/logs"
MARKER="$WORK/.owned-by-general-hot-tier"

if [ -e "$SOURCE" ] && [ ! -e "$MARKER" ]; then
    die "$WORK already holds a source tree this tier did not create. Refusing to delete it; pick a different --work."
fi
mkdir -p "$WORK" "$DEPLOY" "$LOGS"
: > "$MARKER"

# ------------------------------------------------------------- 1. archive
if [ "$KEEP_SOURCE" = false ] || [ ! -d "$SOURCE" ]; then
    say "archive $REVISION"
    rm -rf "$SOURCE"
    mkdir -p "$SOURCE"
    git -C "$REPO" archive "$REVISION" | tar -x -C "$SOURCE"
fi
printf '%s\n' "$REVISION" > "$WORK/revision.txt"

# ----------------------------------------------------------------- 2. ELFs
#
# `cargo build-sbf` exits ZERO even when the SBF backend reports that a call
# overwrites its own stack frame and "may cause undefined behavior during
# execution". An artifact the toolchain calls potentially-undefined has no
# business producing a CU figure, so the diagnostics are counted and a nonzero
# total stops the campaign -- `run-general.sh`'s rule, applied to the six links
# this campaign actually loads.
DIAGNOSTIC_PATTERN='overwrites values in the frame'
ROLES="registry:dclutch_registry_sbf
trading:dclutch_trading_sbf
core:dclutch_core_sbf
claims:dclutch_claims_sbf
custody:dclutch_custody_sbf
accelerator:dclutch_accelerator_sbf"

say "elves at $REVISION"
diagnostics=0
for role in $ROLES; do
    package="dclutch-${role%%:*}-sbf"
    stem="${role##*:}"
    log="$LOGS/build-$package.log"
    ( cd "$SOURCE" && CARGO_TARGET_DIR="$WORK/build-target" \
        cargo build-sbf --manifest-path "programs/$package/Cargo.toml" --sbf-out-dir "$DEPLOY" ) \
        > "$log" 2>&1 \
        || { tail -n 40 "$log" >&2; die "SBF build failed: $package"; }
    count="$(grep -c "$DIAGNOSTIC_PATTERN" "$log" || true)"
    elf="$DEPLOY/$stem.so"
    [ -f "$elf" ] || die "build produced no $elf"
    printf '  %-28s %s  %8s bytes  (%s frame diagnostics)\n' \
        "$stem" "$(sha256 "$elf")" "$(wc -c < "$elf" | tr -d ' ')" "${count:-0}"
    diagnostics=$((diagnostics + count))
done
[ "$diagnostics" -eq 0 ] \
    || die "the six links built with $diagnostics SBF stack-frame-overwrite diagnostics; refusing to publish CU figures measured on them"

# ------------------------------------------------------------ 3. campaign
#
# From the ARCHIVE, not from $REPO: the test source is part of what the commit
# says, and running the shared tree's tests against the archive's ELFs would be
# the same split authority this script exists to close.
say "campaign (General Hot, real ELFs)"
CAMPAIGN_LOG="$LOGS/campaign.log"
set +e
( cd "$SOURCE" && SBF_OUT_DIR="$DEPLOY" CARGO_TARGET_DIR="$WORK/test-target" \
    cargo test --manifest-path programs/dclutch-trading-sbf/program-test/general-hot/Cargo.toml \
    --test open_batch -- --nocapture --test-threads=1 ) > "$CAMPAIGN_LOG" 2>&1
status=$?
set -e
tail -n 20 "$CAMPAIGN_LOG"
[ "$status" -eq 0 ] || die "campaign failed; full log at $CAMPAIGN_LOG"

# -------------------------------------------------------------- 4. the table
#
# The campaign's own `general-campaign <stage> cu=` lines, printed back beside
# the commit they were measured at. A table with no sha beside it is the thing
# this script was written to stop producing.
say "CU at $REVISION"
grep '^general-campaign ' "$CAMPAIGN_LOG" | sed 's/^/  /' || die "the campaign printed no rows"
printf '\ngeneral-hot: commit %s\ngeneral-hot: ELFs   %s\ngeneral-hot: log    %s\n' \
    "$REVISION" "$DEPLOY" "$CAMPAIGN_LOG"
