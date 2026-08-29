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
    "$RUNNER" --probe --work "$SCRATCH/work-relative" --trading-elf relative.so

expect_refusal "missing Trading override refuses" \
    "--trading-elf is not a regular file" \
    "$RUNNER" --probe --work "$SCRATCH/work-missing" --trading-elf "$SCRATCH/missing.so"

printf 'not-an-elf\n' > "$SCRATCH/regular.so"
ln -s "$SCRATCH/regular.so" "$SCRATCH/symlink.so"
expect_refusal "symlink Trading override refuses" \
    "--trading-elf must not be a symlink" \
    "$RUNNER" --probe --work "$SCRATCH/work-symlink" --trading-elf "$SCRATCH/symlink.so"

expect_refusal "single draw cannot present as M-61" \
    "release M-61 requires --checked-gate" \
    "$RUNNER" --work "$SCRATCH/work-single" --seeds 1

expect_refusal "checked gate requires its out-of-band digest" \
    "are one required pair" \
    "$RUNNER" --work "$SCRATCH/work-gate-pair" --checked-gate "$SCRATCH/gate.json"

expect_refusal "mixed projection requires its out-of-band digest" \
    "are one required pair" \
    "$RUNNER" --work "$SCRATCH/work-mixed-pair" \
        --mixed-gate-selection "$SCRATCH/selection.json"

expect_refusal "mixed projection requires its checked gate" \
    "requires --checked-gate + --checked-gate-sha256" \
    "$RUNNER" --work "$SCRATCH/work-mixed-gate" \
        --mixed-gate-selection "$SCRATCH/selection.json" \
        --mixed-gate-selection-sha256 "$(printf '0%.0s' $(seq 1 64))"

# These are deliberately literal source seams, not this test script's values.
# shellcheck disable=SC2016
if grep -Fq 'cargo build-sbf --manifest-path "$1" --sbf-out-dir "$ELF_DIR" -- --locked --offline' "$RUNNER" \
    && grep -Fq 'cargo test --locked --offline' "$RUNNER"; then
    ok "SBF builds and ProgramTest require locked offline graphs"
else
    not_ok "Hot CU runner lost locked-offline admission"
fi

mkdir -p "$SCRATCH/bin" "$SCRATCH/elf"
cat > "$SCRATCH/bin/cargo" <<'SH'
#!/usr/bin/env bash
printf 'protocol default heap fixture seed 0: 1234567 CU consumed\n'
printf 'test result: ok. 1 passed; 0 failed; 0 ignored\n'
SH
chmod +x "$SCRATCH/bin/cargo"
printf 'base-trading\n' > "$SCRATCH/elf/dclutch_trading_sbf.so"
printf 'final-direct\n' > "$SCRATCH/final-direct.so"
base_sha="$(shasum -a 256 "$SCRATCH/elf/dclutch_trading_sbf.so" | cut -d' ' -f1)"
override_sha="$(shasum -a 256 "$SCRATCH/final-direct.so" | cut -d' ' -f1)"
if PATH="$SCRATCH/bin:$PATH" "$RUNNER" \
    --probe \
    --work "$SCRATCH/work-overlay" \
    --elf-dir "$SCRATCH/elf" \
    --trading-elf "$SCRATCH/final-direct.so" \
    --seeds 1 >"$SCRATCH/overlay-stdout" 2>"$SCRATCH/overlay-stderr" \
    && [ "$(shasum -a 256 "$SCRATCH/elf/dclutch_trading_sbf.so" | cut -d' ' -f1)" = "$base_sha" ] \
    && [ "$(shasum -a 256 "$SCRATCH/work-overlay/elf-with-trading-override/dclutch_trading_sbf.so" | cut -d' ' -f1)" = "$override_sha" ] \
    && python3 - "$SCRATCH/work-overlay/summary-immutable.json" "$override_sha" <<'PY'
import json
import pathlib
import sys

summary = json.loads(pathlib.Path(sys.argv[1]).read_text())
expected = sys.argv[2]
assert summary["trading_elf_sha256"] == expected
assert summary["trading_elf_override_sha256"] == expected
assert summary["pass"] == 1
assert summary["m61_eligible"] is False
assert summary["mean_cu"] is None
assert summary["probe_mean_cu"] == 1_234_567
PY
then
    ok "Trading override uses a work-local exact-digest overlay"
else
    sed -n '1,12p' "$SCRATCH/overlay-stderr" >&2
    not_ok "Trading override overlay or summary was not exact"
fi

if [ "$fail" -ne 0 ]; then
    printf '%s tests failed; %s passed\n' "$fail" "$pass" >&2
    exit 1
fi
printf 'all %s Hot CU CLI tests passed\n' "$pass"
