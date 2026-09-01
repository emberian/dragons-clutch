#!/usr/bin/env bash
# Adversarial CLI checks for the top-level gauntlet runner.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNNER="$ROOT/tools/gauntlet/run.sh"
SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-gauntlet-cli.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT

fail() {
    printf 'test-run-cli: %s\n' "$*" >&2
    exit 1
}

FAKE_BIN="$SCRATCH/fake-bin"
SENTINEL="$SCRATCH/tool-was-invoked"
WORK="$SCRATCH/work-must-not-exist"
mkdir -p "$FAKE_BIN"

# Every dependency used by an archive, build, inventory, or campaign stage is
# hostile here. If the full-mode refusal moves below setup again, the sentinel
# identifies the regression even when the developer already has warm stamps.
for tool in git jq shasum python3 cargo cargo-build-sbf solana-test-validator swarm-build; do
    {
        printf '#!/usr/bin/env sh\n'
        printf 'printf "%%s\\n" "%s" >> "%s"\n' "$tool" "$SENTINEL"
        printf 'exit 97\n'
    } > "$FAKE_BIN/$tool"
    chmod +x "$FAKE_BIN/$tool"
done

set +e
PATH="$FAKE_BIN:$PATH" "$RUNNER" \
    --repo "$ROOT" \
    --work "$WORK" \
    --mode full \
    > "$SCRATCH/full.stdout" 2> "$SCRATCH/full.stderr"
status=$?
set -e

[ "$status" -eq 1 ] || fail "--mode full exited $status, expected 1"
grep -F -- "--mode full is unavailable" "$SCRATCH/full.stderr" >/dev/null \
    || fail "--mode full did not emit its availability refusal"
grep -F -- "No build or campaign was started" "$SCRATCH/full.stderr" >/dev/null \
    || fail "--mode full did not state the pre-build boundary"
[ ! -e "$WORK" ] || fail "--mode full created its --work root before refusing"
[ ! -e "$SENTINEL" ] || fail "--mode full invoked a staged dependency before refusing"
[ ! -s "$SCRATCH/full.stdout" ] || fail "--mode full wrote campaign output before refusing"

# An invocation that relied on the historical default also represented a full
# campaign. It must refuse, not silently become a census run.
DEFAULT_WORK="$SCRATCH/default-work-must-not-exist"
set +e
PATH="$FAKE_BIN:$PATH" "$RUNNER" \
    --repo "$ROOT" \
    --work "$DEFAULT_WORK" \
    > "$SCRATCH/default.stdout" 2> "$SCRATCH/default.stderr"
default_status=$?
set -e

[ "$default_status" -eq 1 ] || fail "default mode exited $default_status, expected full-mode refusal 1"
grep -F -- "--mode full is unavailable" "$SCRATCH/default.stderr" >/dev/null \
    || fail "default mode silently stopped representing the parked full campaign"
[ ! -e "$DEFAULT_WORK" ] || fail "default mode created its --work root before refusing"
[ ! -e "$SENTINEL" ] || fail "default mode invoked a staged dependency before refusing"

"$RUNNER" --help > "$SCRATCH/help.stdout"
grep -F -- "full    unavailable" "$SCRATCH/help.stdout" >/dev/null \
    || fail "--help advertises full without its unavailable status"
grep -F -- 'exits 1 before any build' "$SCRATCH/help.stdout" >/dev/null \
    || fail "--help omits full mode's exit behavior"

printf 'test-run-cli: ok\n'
