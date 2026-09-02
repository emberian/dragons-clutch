#!/usr/bin/env bash
# Build and compare the exact thirteen-link per-function SBF frame manifest.
#
# Exit 0: every link freshly compiled, emitted zero overwrite diagnostics, and
#         the complete canonical manifest matches the committed ratchet.
# Exit 1: this tree has a build/diagnostic/frame disagreement.
# Exit 2: a prerequisite or measurement artifact is missing; nothing proved.

set -uo pipefail

readonly EXIT_PASS=0
readonly EXIT_GATE_FAILED=1
readonly EXIT_PREREQ_MISSING=2
# Twelve since 2026-09-02, when `dclutch-dealer-sbf` was deleted: a standalone
# measurement prototype its own header disclaimed, `false` in SHIPPED_LINKS,
# whose only consumer was its own program-test. The count is pinned rather than
# discovered on purpose -- a link silently dropping out of the measurement is
# the failure this guard exists to catch -- so it moves by hand, with a reason.
readonly EXPECTED_LINK_COUNT=12
readonly DIAGNOSTIC_PATTERN='overwrites values in the frame'

here="$(cd "$(dirname "$0")" && pwd)"
source_root="$(cd "$here/../.." && pwd)"
baseline=""
capture=""

usage() {
    cat <<'EOF'
usage: tools/frameguard/run.sh [--source DIR] [--baseline FILE]
                               [--capture FILE]

Without --capture, freshly measure DIR and compare it with the baseline. With
--capture, write the canonical candidate manifest without admitting it. A
baseline must be made from TWO such fresh captures with:

  frameguard.py accept --first A --second B --output baseline.json
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
    --source)
        shift
        [ "$#" -gt 0 ] || { usage >&2; exit 64; }
        source_root="$1"
        ;;
    --source=*) source_root="${1#--source=}" ;;
    --baseline)
        shift
        [ "$#" -gt 0 ] || { usage >&2; exit 64; }
        baseline="$1"
        ;;
    --baseline=*) baseline="${1#--baseline=}" ;;
    --capture)
        shift
        [ "$#" -gt 0 ] || { usage >&2; exit 64; }
        capture="$1"
        ;;
    --capture=*) capture="${1#--capture=}" ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'frameguard: unknown argument %s\n' "$1" >&2; usage >&2; exit 64 ;;
    esac
    shift
done

source_root="$(cd "$source_root" 2>/dev/null && pwd)" || {
    printf 'frameguard: source directory is missing: %s\n' "$source_root" >&2
    exit "$EXIT_PREREQ_MISSING"
}
tool="$source_root/tools/frameguard/frameguard.py"
parser="$source_root/tools/sbf-frame-sizes.py"
[ -f "$tool" ] && [ ! -L "$tool" ] || {
    printf 'frameguard: checker is missing from measured source: %s\n' "$tool" >&2
    exit "$EXIT_PREREQ_MISSING"
}
[ -f "$parser" ] && [ ! -L "$parser" ] || {
    printf 'frameguard: frame parser is missing from measured source: %s\n' "$parser" >&2
    exit "$EXIT_PREREQ_MISSING"
}
command -v python3 >/dev/null 2>&1 || {
    printf 'frameguard: python3 is not on PATH\n' >&2
    exit "$EXIT_PREREQ_MISSING"
}
command -v cargo-build-sbf >/dev/null 2>&1 || {
    printf 'frameguard: cargo-build-sbf is not on PATH\n' >&2
    exit "$EXIT_PREREQ_MISSING"
}
command -v cargo >/dev/null 2>&1 || {
    printf 'frameguard: cargo is not on PATH\n' >&2
    exit "$EXIT_PREREQ_MISSING"
}

[ -n "$baseline" ] || baseline="$source_root/tools/frameguard/baseline.json"
if [ -z "$capture" ] && { [ ! -f "$baseline" ] || [ -L "$baseline" ]; }; then
    printf 'frameguard: baseline is missing or not regular: %s\n' "$baseline" >&2
    exit "$EXIT_PREREQ_MISSING"
fi

scratch="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-frameguard.XXXXXX")" || exit "$EXIT_PREREQ_MISSING"
trap 'rm -rf -- "$scratch"' EXIT
inventory="$scratch/inventory.tsv"
reports="$scratch/reports"
candidate="$scratch/candidate.json"
mkdir -p "$reports"

for manifest in "$source_root"/programs/*/Cargo.toml; do
    [ -f "$manifest" ] || {
        printf 'frameguard: program manifest inventory is empty\n' >&2
        exit "$EXIT_PREREQ_MISSING"
    }
    basename "$(dirname "$manifest")"
done | LC_ALL=C sort > "$inventory"

link_count="$(wc -l < "$inventory" | tr -d ' ')"
if [ "$link_count" != "$EXPECTED_LINK_COUNT" ] \
    || [ "$(LC_ALL=C sort -u "$inventory" | wc -l | tr -d ' ')" != "$EXPECTED_LINK_COUNT" ]; then
    printf 'frameguard: program inventory is not the exact %s-link set: %s\n' \
        "$EXPECTED_LINK_COUNT" "$link_count" >&2
    exit "$EXIT_PREREQ_MISSING"
fi

while IFS= read -r package; do
    target="$scratch/target-$package"
    log="$scratch/build-$package.log"
    manifest="$source_root/programs/$package/Cargo.toml"
    printf 'frameguard: build %s\n' "$package"
    if ! (
        cd "$source_root" &&
        RUSTC_BOOTSTRAP=1 \
        RUSTFLAGS="-Zemit-stack-sizes --emit=obj,link" \
        CARGO_TERM_COLOR=never \
        CARGO_TARGET_DIR="$target" \
        cargo build-sbf --manifest-path "$manifest" -- --locked
    ) >"$log" 2>&1; then
        printf 'frameguard: %s measurement build failed\n' "$package" >&2
        tail -n 40 "$log" >&2
        exit "$EXIT_GATE_FAILED"
    fi
    if ! grep -Eq "^[[:space:]]*Compiling[[:space:]]+${package}[[:space:]]+v[^[:space:]]+" "$log"; then
        printf 'frameguard: %s has no fresh top-package compile marker; no measurement\n' \
            "$package" >&2
        exit "$EXIT_PREREQ_MISSING"
    fi
    diagnostics="$(grep -c "$DIAGNOSTIC_PATTERN" "$log" || true)"
    if [ "${diagnostics:-0}" != 0 ]; then
        printf 'frameguard: REFUSING -- %s emitted %s stack-frame overwrite diagnostics\n' \
            "$package" "$diagnostics" >&2
        grep "$DIAGNOSTIC_PATTERN" "$log" | LC_ALL=C sort -u >&2
        exit "$EXIT_GATE_FAILED"
    fi

    target_triple=""
    for candidate_triple in sbpf-solana-solana sbf-solana-solana; do
        [ -d "$target/$candidate_triple" ] && target_triple="$candidate_triple"
    done
    if [ -z "$target_triple" ]; then
        printf 'frameguard: %s emitted no recognizable SBF target directory\n' "$package" >&2
        exit "$EXIT_PREREQ_MISSING"
    fi
    object_stem="$(printf '%s' "$package" | tr '-' '_')"
    object="$target/$target_triple/release/deps/$object_stem.o"
    if [ ! -f "$object" ] || [ -L "$object" ]; then
        printf 'frameguard: %s fresh measurement object is missing: %s\n' \
            "$package" "$object" >&2
        exit "$EXIT_PREREQ_MISSING"
    fi
    code=0
    python3 "$parser" --format json "$object" > "$reports/$package.json" || code=$?
    case "$code" in
    0) ;;
    1) exit "$EXIT_GATE_FAILED" ;;
    *) exit "$EXIT_PREREQ_MISSING" ;;
    esac
done < "$inventory"

code=0
python3 "$tool" assemble --inventory "$inventory" --reports "$reports" \
    --output "$candidate" || code=$?
case "$code" in
0) ;;
1) exit "$EXIT_GATE_FAILED" ;;
*) exit "$EXIT_PREREQ_MISSING" ;;
esac

if [ -n "$capture" ]; then
    # The assembler's same-filesystem temporary + rename is the only writer.
    # Re-assembling at the requested path avoids `cp` becoming a second,
    # non-atomic producer for a file that may later be admitted as baseline.
    code=0
    python3 "$tool" assemble --inventory "$inventory" --reports "$reports" \
        --output "$capture" || code=$?
    case "$code" in
    0) printf 'frameguard: captured complete thirteen-link manifest at %s\n' "$capture" ;;
    1) exit "$EXIT_GATE_FAILED" ;;
    *) exit "$EXIT_PREREQ_MISSING" ;;
    esac
    exit "$EXIT_PASS"
fi

code=0
python3 "$tool" check --baseline "$baseline" --candidate "$candidate" || code=$?
case "$code" in
0) exit "$EXIT_PASS" ;;
1) exit "$EXIT_GATE_FAILED" ;;
*) exit "$EXIT_PREREQ_MISSING" ;;
esac
