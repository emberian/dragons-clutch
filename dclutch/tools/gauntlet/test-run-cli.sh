#!/usr/bin/env bash
# Adversarial CLI checks for the top-level gauntlet runner.
#
# THIS FILE USED TO TEST THE PARK. Every assertion in it was about `--mode
# full` refusing before it built anything: exit 1, the "unavailable" line, an
# untouched `--work` root, and a sentinel proving no staged dependency had been
# invoked. `c9eac1738` unparked the tier on 2026-09-03 and full mode builds and
# campaigns again, so from that commit the file asserted a refusal the runner
# deliberately stopped making -- and it went on "passing" for nobody, because
# NOTHING RUNS IT. No tier calls it and no runner calls it; `tools/ci/run.sh`
# has no row for it. It was red at 837818bc1 (`--mode full exited 97, expected
# 1`) and the tree found out because a lane read it, which is the same way the
# last three defects of this shape were found.
#
# What survives the unpark is the argument boundary: the modes, the port band
# and the stage names are all rejected before any revision is resolved, any
# work root is made, or any build tool is looked for. That is checkable without
# a chain, in under a second, and it is what this file checks now.
#
# STILL OWED: an executor. A test nobody runs is the defect it was written to
# prevent, one level up.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNNER="$ROOT/tools/gauntlet/run.sh"
SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-gauntlet-cli.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT

fail() {
    printf 'test-run-cli: %s\n' "$*" >&2
    exit 1
}

# Each row: a rejected argument list, the exit it owes, and a phrase its
# refusal must contain. The work root is named and must not survive any of
# them -- an argument refused after the scratch tree is built is an argument
# refused too late.
refuses() {
    local expected_status="$1" phrase="$2"; shift 2
    local work="$SCRATCH/work-$RANDOM-must-not-exist" status=0
    set +e
    "$RUNNER" --repo "$ROOT" --work "$work" "$@" \
        > "$SCRATCH/out" 2> "$SCRATCH/err"
    status=$?
    set -e
    [ "$status" -eq "$expected_status" ] \
        || fail "$* exited $status, expected $expected_status"
    grep -F -- "$phrase" "$SCRATCH/err" >/dev/null \
        || fail "$* did not say: $phrase"
    [ ! -e "$work" ] || fail "$* created its --work root before refusing"
    [ ! -s "$SCRATCH/out" ] || fail "$* wrote to stdout before refusing"
}

refuses 2 "--mode must be census or full" --mode neither
refuses 2 "--rpc-port must be 1024-65494" --rpc-port 70000
refuses 2 "--rpc-port must be a decimal port" --rpc-port hello
refuses 2 "unknown argument" --not-an-option
refuses 2 "--from must name a stage" --mode census --from nosuchstage

# `--work` must be absolute: a relative scratch root is resolved against
# whatever directory the caller happened to be in, which in this tree is one
# step from writing a ledger into the repository.
set +e
( cd "$SCRATCH" && "$RUNNER" --repo "$ROOT" --work relative-work --mode census ) \
    > "$SCRATCH/out" 2> "$SCRATCH/err"
status=$?
set -e
[ "$status" -eq 2 ] || fail "--work relative-work exited $status, expected 2"
grep -F -- "--work must be absolute" "$SCRATCH/err" >/dev/null \
    || fail "--work relative-work did not say it must be absolute"
[ ! -e "$SCRATCH/relative-work" ] || fail "--work relative-work made its root anyway"

# `--help` states what full mode costs. It used to state that full mode was
# unavailable; a reader who is told the wrong one of those wastes either
# twenty-five minutes or a campaign.
"$RUNNER" --help > "$SCRATCH/help.stdout"
grep -F -- "census | full" "$SCRATCH/help.stdout" >/dev/null \
    || fail "--help no longer names both modes"
grep -F -- "Budget 25-31 minutes" "$SCRATCH/help.stdout" >/dev/null \
    || fail "--help omits full mode's measured cost"
grep -F -- "unavailable" "$SCRATCH/help.stdout" >/dev/null \
    && fail "--help still advertises full mode as unavailable"

printf 'test-run-cli: ok\n'
