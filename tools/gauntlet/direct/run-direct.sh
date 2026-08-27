#!/usr/bin/env bash
# The Direct family's stateless AOT gauntlet campaign.
#
#   archive -> build the ELF -> run the campaign -> observe -> witnesses -> report
#
# Unlike `run.sh --mode full`, this stage needs NO validator and NO port. It is
# a ProgramTest FAST LANE, and TIERS.md is explicit that a fast lane is always
# ADDITIONAL evidence, never a substitute: the census records the campaign name
# with every observation it admits, and the report prints it. The tier's four
# fast-lane answers ride inside the evidence document itself so that a reader
# never has to take the claim on trust.
#
# By default this writes to its OWN work root and its OWN ledger, so it can run
# beside a `run.sh --mode full` campaign without racing it. Point --work at the
# shared root only when nothing else holds it.
#
# usage:
#   tools/gauntlet/direct/run-direct.sh [--repo DIR] [--work DIR] [--keep-source]
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
WORK="/private/tmp/dclutch-gauntlet-direct"
KEEP_SOURCE=false

while [ $# -gt 0 ]; do
    case "$1" in
        --repo) REPO="$2"; shift 2 ;;
        --work) WORK="$2"; shift 2 ;;
        --keep-source) KEEP_SOURCE=true; shift ;;
        *) echo "direct: unknown argument: $1" >&2; exit 2 ;;
    esac
done

GAUNTLET="$REPO/tools/gauntlet"
TIER="$GAUNTLET/direct"
SOURCE="$WORK/source"
DEPLOY="$WORK/deploy"
LOGS="$WORK/logs"
OUT="$WORK/out"
LEDGER="$OUT/ledger.json"
INVENTORY="$OUT/inventory.json"
REPORT="$OUT/CENSUS.md"
CENSUS_TARGET="$WORK/census-target"
PRODUCER_TARGET="$WORK/producer-target"
CENSUS_BIN="$CENSUS_TARGET/release/dclutch-route-census"
PRODUCER_BIN="$PRODUCER_TARGET/release/dclutch-gauntlet-direct-campaign"

# `cargo build-sbf` exits ZERO even when the SBF backend reports that a call
# overwrites its own stack frame and "may cause undefined behavior during
# execution". Count them and refuse the campaign if there are any: an artifact
# the toolchain calls potentially-undefined has no business producing evidence.
DIAGNOSTIC_PATTERN='overwrites values in the frame'
PACKAGE="dclutch-direct-aot-sbf"
STEM="dclutch_direct_aot_sbf"

die() { echo "direct: $*" >&2; exit 1; }
say() { printf '\n== %s\n' "$*"; }
sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }

command -v jq >/dev/null 2>&1 || die "jq not found"
command -v cargo-build-sbf >/dev/null 2>&1 || die "cargo-build-sbf not found"

mkdir -p "$WORK" "$LOGS" "$OUT" "$DEPLOY"

# ------------------------------------------------------------- 1. archive
# The campaign measures a REVISION, never the shared working tree. Other lanes
# edit that tree continuously; building from it would silently attribute their
# in-flight work to this tier's numbers.
REVISION="$(cd "$REPO" && git rev-parse HEAD)"
MARKER="$WORK/.owned-by-direct-tier"
# Refuse to delete a work root this tier did not create. `run.sh --mode full`
# keeps its own `source/` under /private/tmp/dclutch-gauntlet, and a --work
# pointed there would otherwise `rm -rf` another campaign's archive out from
# under it. Fold into a shared LEDGER with `census observe` directly instead;
# never by pointing this script's work root at somebody else's.
if [ -e "$SOURCE" ] && [ ! -e "$MARKER" ]; then
    die "$WORK already holds a source tree this tier did not create. Refusing to delete it. Pick a different --work, or fold into a shared ledger with \`census observe\` rather than by re-rooting this script."
fi
: > "$MARKER"
if [ "$KEEP_SOURCE" = false ] || [ ! -d "$SOURCE" ]; then
    say "stage archive ($REVISION)"
    rm -rf "$SOURCE"
    mkdir -p "$SOURCE"
    ( cd "$REPO" && git archive HEAD ) | tar -x -C "$SOURCE"
fi
printf '%s\n' "$REVISION" > "$WORK/revision.txt"

# ----------------------------------------------------------------- 2. elf
say "stage elf"
( cd "$SOURCE" && CARGO_TARGET_DIR="$WORK/build-target" \
    cargo build-sbf --manifest-path "programs/$PACKAGE/Cargo.toml" --sbf-out-dir "$DEPLOY" ) \
    > "$LOGS/build-$PACKAGE.log" 2>&1 \
    || { tail -n 40 "$LOGS/build-$PACKAGE.log" >&2; die "SBF build failed: $PACKAGE"; }
DIAGNOSTICS="$(grep -c "$DIAGNOSTIC_PATTERN" "$LOGS/build-$PACKAGE.log" || true)"
ELF="$DEPLOY/$STEM.so"
[ -f "$ELF" ] || die "build produced no $ELF"
ELF_SHA256="$(sha256 "$ELF")"
ELF_BYTES="$(wc -c < "$ELF" | tr -d ' ')"
printf '  %s  %s bytes  (%s frame diagnostics)\n' "$ELF_SHA256" "$ELF_BYTES" "$DIAGNOSTICS"
[ "$DIAGNOSTICS" -eq 0 ] \
    || die "$PACKAGE built with $DIAGNOSTICS SBF stack-frame-overwrite diagnostics; the toolchain says those calls may cause undefined behavior. Refusing to produce evidence for them."

BUILD_SBF_RAW="$(cargo-build-sbf --version)"
jq -n \
    --arg elf_sha256 "$ELF_SHA256" \
    --argjson elf_bytes "$ELF_BYTES" \
    --arg commit "$REVISION" \
    --arg cargo_build_sbf_version "$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '1p')" \
    --arg platform_tools_version "$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '2p')" \
    --arg rustc_version "$(printf '%s\n' "$BUILD_SBF_RAW" | sed -n '3p')" \
    --arg build_command "cargo build-sbf --manifest-path programs/$PACKAGE/Cargo.toml" \
    --argjson sbf_backend_frame_diagnostics "$DIAGNOSTICS" \
    '{ schema: "dclutch-gauntlet-artifact-attestation-v1", role: "direct-aot",
       elf_sha256: $elf_sha256, elf_bytes: $elf_bytes, commit: $commit,
       cargo_build_sbf_version: $cargo_build_sbf_version,
       platform_tools_version: $platform_tools_version, rustc_version: $rustc_version,
       build_command: $build_command,
       sbf_backend_frame_diagnostics: $sbf_backend_frame_diagnostics }' \
    > "$WORK/artifact.json"

# --------------------------------------------------------------- 3. tools
say "stage tools"
( cd "$GAUNTLET/census" && CARGO_TARGET_DIR="$CENSUS_TARGET" cargo build --release ) \
    > "$LOGS/build-census.log" 2>&1 \
    || { tail -n 40 "$LOGS/build-census.log" >&2; die "census tool build failed"; }
# The census's own adversarial tests: each one fails against a deliberately
# weakened fold. They are what stands between this suite and being a mirror
# one level up.
( cd "$GAUNTLET/census" && CARGO_TARGET_DIR="$CENSUS_TARGET" cargo test --release ) \
    > "$LOGS/test-census.log" 2>&1 \
    || { tail -n 40 "$LOGS/test-census.log" >&2; die "census tool tests failed"; }
( cd "$TIER/producer" && CARGO_TARGET_DIR="$PRODUCER_TARGET" cargo build --release ) \
    > "$LOGS/build-producer.log" 2>&1 \
    || { tail -n 40 "$LOGS/build-producer.log" >&2; die "campaign producer build failed"; }
[ -x "$CENSUS_BIN" ] || die "census binary missing: $CENSUS_BIN"
[ -x "$PRODUCER_BIN" ] || die "producer binary missing: $PRODUCER_BIN"

# ----------------------------------------------------------- 4. inventory
say "stage inventory"
"$CENSUS_BIN" inventory --root "$SOURCE" --out "$INVENTORY" --revision "$REVISION"

# ------------------------------------------------------------ 5. campaign
say "stage campaign (Direct stateless AOT)"
SBF_OUT_DIR="$DEPLOY" "$PRODUCER_BIN" --out "$OUT" > "$LOGS/campaign.stdout" 2> "$LOGS/campaign.stderr" \
    || { tail -n 40 "$LOGS/campaign.stderr" >&2; die "campaign failed; logs under $LOGS"; }
tail -n 1 "$LOGS/campaign.stderr"
EVIDENCE="$OUT/evidence.json"
[ -f "$EVIDENCE" ] || die "campaign evidence missing: $EVIDENCE"

# -------------------------------------------------------------- 6. census
say "stage census"
PROBLEMS=0
"$CENSUS_BIN" observe \
    --inventory "$INVENTORY" \
    --ledger "$LEDGER" \
    --bindings "$TIER/bindings.json" \
    --programs "$OUT/programs.json" \
    --evidence "$EVIDENCE" || PROBLEMS=1

# ----------------------------------------------------------- 7. witnesses
# The witness context is the hand-derived expectations file merged with the
# build stage's artifact record. Neither half reads the campaign output, which
# is what makes `expect_from` a cross-check rather than a mirror.
say "witnesses"
jq -s '.[0] + {artifact: .[1]}' "$TIER/expectations.json" "$WORK/artifact.json" \
    > "$OUT/witness-context.json"
"$GAUNTLET/tier1/check-witnesses.sh" "$TIER/witnesses.json" "$EVIDENCE" "$OUT/witness-context.json" \
    || PROBLEMS=1

# -------------------------------------------------------------- 8. report
"$CENSUS_BIN" report \
    --inventory "$INVENTORY" \
    --ledger "$LEDGER" \
    --blocked "$GAUNTLET/blocked.json" \
    --out "$REPORT"

say "done"
echo "evidence:  $EVIDENCE"
echo "ledger:    $LEDGER"
echo "report:    $REPORT"
if [ "$PROBLEMS" != "0" ]; then
    echo "direct: the census or the witnesses reported problems (above); NOT green" >&2
    exit 1
fi
