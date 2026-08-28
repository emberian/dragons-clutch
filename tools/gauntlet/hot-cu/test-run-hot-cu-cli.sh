#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
RUNNER="$HERE/run-hot-cu.sh"
SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-hot-cu-cli.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT

pass=0
fail=0
ok() { pass=$((pass + 1)); printf 'ok %s - %s\n' "$pass" "$1"; }
not_ok() { fail=$((fail + 1)); printf 'not ok - %s\n' "$1" >&2; }

expect_refusal() {
    local name=$1 needle=$2; shift 2
    if "$@" >"$SCRATCH/stdout" 2>"$SCRATCH/stderr"; then
        not_ok "$name (unexpected success)"
    elif grep -Fq -- "$needle" "$SCRATCH/stderr"; then
        ok "$name"
    else
        sed -n '1,8p' "$SCRATCH/stderr" >&2
        not_ok "$name (wrong refusal)"
    fi
}

expect_refusal "relative Trading override refuses" \
    "--trading-elf must be absolute" \
    "$RUNNER" --work "$SCRATCH/work-relative" --trading-elf relative.so

expect_refusal "missing Trading override refuses" \
    "--trading-elf is not a regular file" \
    "$RUNNER" --work "$SCRATCH/work-missing" --trading-elf "$SCRATCH/missing.so"

printf 'not-an-elf\n' > "$SCRATCH/regular.so"
ln -s "$SCRATCH/regular.so" "$SCRATCH/symlink.so"
expect_refusal "symlink Trading override refuses" \
    "--trading-elf must not be a symlink" \
    "$RUNNER" --work "$SCRATCH/work-symlink" --trading-elf "$SCRATCH/symlink.so"

# These are deliberately literal source seams, not this test script's values.
# shellcheck disable=SC2016
if grep -Fq 'cargo build-sbf --manifest-path "$1" --sbf-out-dir "$ELF_DIR" -- --locked --offline' "$RUNNER" \
    && grep -Fq 'cargo test --locked --offline' "$RUNNER"; then
    ok "SBF builds and ProgramTest require locked offline graphs"
else
    not_ok "Hot CU runner lost locked-offline admission"
fi

if [ "$fail" -ne 0 ]; then
    printf '%s tests failed; %s passed\n' "$fail" "$pass" >&2
    exit 1
fi
printf 'all %s Hot CU CLI tests passed\n' "$pass"
