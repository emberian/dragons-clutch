#!/usr/bin/env bash
# Build and compare the exact per-function SBF frame manifest, one object per link.
#
# Exit 0: every link freshly compiled, emitted zero overwrite diagnostics, and
#         the complete canonical manifest matches the committed ratchet.
# Exit 1: this tree has a build/diagnostic/frame disagreement.
# Exit 2: a prerequisite or measurement artifact is missing; nothing proved.
#
# A CAPTURE NAMES ITS COMMIT. `--at <commit>` builds a detached worktree at that
# commit and records it in the manifest, so the diff a reviewer reads is between
# two named commits. Without `--at`, a capture is admitted only from a clean
# tree (whose HEAD is then recorded) and REFUSED from a dirty one -- because an
# exact ratchet captured from ambient uncommitted state names no base, and
# nobody can review it afterwards. Measured on 2026-09-02: three correct
# recaptures of this baseline were each invalidated within minutes by a program
# commit that landed while the four-minute double build was still running, and
# the last would have admitted 26 changed rows of which 2 belonged to its
# author. A capture must ride with a commit, or name one.

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
at=""
repo=""
tools=""
worktree=""
measured_commit=""

cleanup() {
    if [ -n "$worktree" ] && [ -n "$repo" ]; then
        git -C "$repo" worktree remove --force "$worktree" >/dev/null 2>&1 || true
        git -C "$repo" worktree prune >/dev/null 2>&1 || true
    fi
    [ -n "${scratch:-}" ] && rm -rf -- "$scratch"
}

usage() {
    cat <<'EOF'
usage: tools/frameguard/run.sh [--source DIR] [--repo DIR] [--at COMMIT]
                               [--baseline FILE] [--capture FILE]

Without --capture, freshly measure the tree and compare it with the baseline.
With --capture, write the canonical candidate manifest without admitting it.

--at COMMIT measures a detached worktree at COMMIT and records it in the
manifest. A capture from a dirty tree with no --at is REFUSED: the manifest
would name no base, and an exact ratchet with no base cannot be reviewed.

--repo DIR names the git repository used for --at and for attributing a red
gate; it defaults to the measured source, which is right unless the source is
an unpacked archive (as `tools/ci/run.sh --commit` builds).

--tools DIR names the tree the CHECKER and FRAME PARSER are read from. It
defaults to the tree this script itself lives in -- the instrument is the one
you invoked -- while --source/--at choose only the program sources compiled.

A baseline must be made from TWO fresh captures of the SAME commit with:

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
    --at)
        shift
        [ "$#" -gt 0 ] || { usage >&2; exit 64; }
        at="$1"
        ;;
    --at=*) at="${1#--at=}" ;;
    --repo)
        shift
        [ "$#" -gt 0 ] || { usage >&2; exit 64; }
        repo="$1"
        ;;
    --repo=*) repo="${1#--repo=}" ;;
    --tools)
        shift
        [ "$#" -gt 0 ] || { usage >&2; exit 64; }
        tools="$1"
        ;;
    --tools=*) tools="${1#--tools=}" ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'frameguard: unknown argument %s\n' "$1" >&2; usage >&2; exit 64 ;;
    esac
    shift
done

source_root="$(cd "$source_root" 2>/dev/null && pwd)" || {
    printf 'frameguard: source directory is missing: %s\n' "$source_root" >&2
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

scratch="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-frameguard.XXXXXX")" || exit "$EXIT_PREREQ_MISSING"
trap cleanup EXIT

# --- which SOURCE, and under what name ---------------------------------------
# Everything below measures `$source_root` and reports `$measured_commit`. The
# only three admitted combinations are: a named commit in a detached worktree,
# a clean repository whose HEAD is therefore its own name, and an unnamed tree
# -- the last of which may be compared but never captured.
[ -n "$repo" ] || repo="$source_root"
repo_top=""
if command -v git >/dev/null 2>&1; then
    repo_top="$(git -C "$repo" rev-parse --show-toplevel 2>/dev/null)" || repo_top=""
fi

if [ -n "$at" ]; then
    if [ -z "$repo_top" ]; then
        printf 'frameguard: --at %s needs a git repository; %s is not one\n' \
            "$at" "$repo" >&2
        exit "$EXIT_PREREQ_MISSING"
    fi
    repo="$repo_top"
    measured_commit="$(git -C "$repo" rev-parse --verify --quiet "$at^{commit}")" || {
        printf 'frameguard: --at %s does not name a commit in %s\n' "$at" "$repo" >&2
        exit "$EXIT_PREREQ_MISSING"
    }
    worktree="$scratch/at-${measured_commit}"
    if ! git -C "$repo" worktree add --detach --quiet "$worktree" "$measured_commit" \
        >"$scratch/worktree.log" 2>&1; then
        printf 'frameguard: could not check out %s into a detached worktree\n' \
            "$measured_commit" >&2
        tail -n 10 "$scratch/worktree.log" >&2
        worktree=""
        exit "$EXIT_PREREQ_MISSING"
    fi
    source_root="$worktree"
    printf 'frameguard: measuring commit %s in a detached worktree\n' "$measured_commit"
elif [ -n "$repo_top" ]; then
    repo="$repo_top"
    dirty="$(git -C "$repo" status --porcelain --untracked-files=no)"
    if [ -n "$dirty" ]; then
        if [ -n "$capture" ]; then
            printf 'frameguard: REFUSING to capture from a dirty tree -- %s tracked path(s) differ from HEAD.\n' \
                "$(printf '%s\n' "$dirty" | wc -l | tr -d ' ')" >&2
            printf 'frameguard: an exact ratchet must name its base. Re-run with --at HEAD (or --at <commit>).\n' >&2
            exit "$EXIT_PREREQ_MISSING"
        fi
        printf 'frameguard: measuring a DIRTY tree; the comparison names no commit\n'
    else
        measured_commit="$(git -C "$repo" rev-parse --verify --quiet 'HEAD^{commit}')" || measured_commit=""
        [ -n "$measured_commit" ] && printf 'frameguard: measuring clean HEAD %s\n' "$measured_commit"
    fi
elif [ -n "$capture" ]; then
    printf 'frameguard: REFUSING to capture from %s, which is not a git repository\n' "$repo" >&2
    printf 'frameguard: a capture must name the commit it measured; use --repo DIR --at <commit>\n' >&2
    exit "$EXIT_PREREQ_MISSING"
fi

# --- the instrument, and the subject -----------------------------------------
# THE INSTRUMENT IS THE ONE YOU INVOKED. `--source`/`--at` choose which program
# sources are compiled; the checker and the frame parser come from this
# script's own tree unless `--tools` says otherwise. Pairing a running run.sh
# with an older sibling checker measures nothing coherent, and it is not
# hypothetical: the first `--at` capture of this baseline built all twelve
# links and then died at the assembler, because the commit it measured predates
# the checker's own `--commit` flag by ONE commit -- so measuring any past
# frame was impossible from the moment the flag was added. A whole archived
# tree measured by its own tools is a different and equally valid thing, and is
# what `tools/ci/run.sh --commit <rev>` builds: there this script IS the
# archived one, so the default already resolves to the archive.
instrument_root="$tools"
[ -n "$instrument_root" ] || instrument_root="$(cd "$here/../.." && pwd)"
instrument_root="$(cd "$instrument_root" 2>/dev/null && pwd)" || {
    printf 'frameguard: tool directory is missing: %s\n' "$tools" >&2
    exit "$EXIT_PREREQ_MISSING"
}
tool="$instrument_root/tools/frameguard/frameguard.py"
parser="$instrument_root/tools/sbf-frame-sizes.py"
[ -f "$tool" ] && [ ! -L "$tool" ] || {
    printf 'frameguard: checker is missing from the measuring tools: %s\n' "$tool" >&2
    exit "$EXIT_PREREQ_MISSING"
}
[ -f "$parser" ] && [ ! -L "$parser" ] || {
    printf 'frameguard: frame parser is missing from the measuring tools: %s\n' "$parser" >&2
    exit "$EXIT_PREREQ_MISSING"
}
[ "$instrument_root" = "$source_root" ] \
    || printf 'frameguard: measuring with the tools in %s\n' "$instrument_root"

[ -n "$baseline" ] || baseline="$source_root/tools/frameguard/baseline.json"
if [ -z "$capture" ] && { [ ! -f "$baseline" ] || [ -L "$baseline" ]; }; then
    printf 'frameguard: baseline is missing or not regular: %s\n' "$baseline" >&2
    exit "$EXIT_PREREQ_MISSING"
fi

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

commit_argument=()
[ -n "$measured_commit" ] && commit_argument=(--commit "$measured_commit")

code=0
python3 "$tool" assemble --inventory "$inventory" --reports "$reports" \
    --output "$candidate" "${commit_argument[@]+"${commit_argument[@]}"}" || code=$?
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
        --output "$capture" "${commit_argument[@]+"${commit_argument[@]}"}" || code=$?
    case "$code" in
    0) printf 'frameguard: captured the complete %s-link manifest of %s at %s\n' \
            "$EXPECTED_LINK_COUNT" "$measured_commit" "$capture" ;;
    1) exit "$EXIT_GATE_FAILED" ;;
    *) exit "$EXIT_PREREQ_MISSING" ;;
    esac
    exit "$EXIT_PASS"
fi

code=0
python3 "$tool" check --baseline "$baseline" --candidate "$candidate" || code=$?
if [ "$code" = 1 ] && [ -n "$repo_top" ]; then
    # Red is the start of the question, not the answer. If the baseline names
    # the commit it was captured at, the range since it can be read back and
    # the commits that moved program sources without carrying frame rows named,
    # so the next reader sees WHO owes the recapture rather than only that the
    # gate disagrees.
    python3 "$tool" owed --repo "$repo_top" --baseline "$baseline" \
        --until "${measured_commit:-HEAD}" >&2 || true
fi
case "$code" in
0) exit "$EXIT_PASS" ;;
1) exit "$EXIT_GATE_FAILED" ;;
*) exit "$EXIT_PREREQ_MISSING" ;;
esac
