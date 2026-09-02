#!/usr/bin/env bash
# Hermetic refusal tests for run.sh. The fake cargo emits the exact object and
# compile-marker shape the runner consumes; the parser stub emits complete JSON.

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-frameguard-runner.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT
FIXTURE="$SCRATCH/source"
BIN="$SCRATCH/bin"
mkdir -p "$FIXTURE/tools/frameguard" "$FIXTURE/tools" "$FIXTURE/programs" "$BIN"
cp "$HERE/frameguard.py" "$FIXTURE/tools/frameguard/frameguard.py"

for index in $(seq -w 0 12); do
    package="program-$index"
    mkdir -p "$FIXTURE/programs/$package"
    printf '[package]\nname = "%s"\nversion = "0.1.0"\n' "$package" \
        > "$FIXTURE/programs/$package/Cargo.toml"
done

cat > "$FIXTURE/tools/sbf-frame-sizes.py" <<'PY'
#!/usr/bin/env python3
import json
print(json.dumps({
    "schema": "dclutch-sbf-frame-sizes-v1",
    "bound_bytes": 4096,
    "frame_count": 1,
    "frames": [{"bytes": 128, "symbol": "fixture"}],
}))
PY
chmod +x "$FIXTURE/tools/sbf-frame-sizes.py"

cat > "$BIN/cargo-build-sbf" <<'SH'
#!/usr/bin/env bash
exit 0
SH
cat > "$BIN/cargo" <<'SH'
#!/usr/bin/env bash
set -eu
manifest=""
while [ "$#" -gt 0 ]; do
    [ "$1" = "--manifest-path" ] && { shift; manifest="$1"; }
    shift
done
package="$(basename "$(dirname "$manifest")")"
stem="$(printf '%s' "$package" | tr '-' '_')"
mkdir -p "$CARGO_TARGET_DIR/sbpf-solana-solana/release/deps"
: > "$CARGO_TARGET_DIR/sbpf-solana-solana/release/deps/$stem.o"
printf '   Compiling %s v0.1.0 (%s)\n' "$package" "$(dirname "$manifest")"
if [ "${FRAMEGUARD_INJECT_DIAGNOSTIC:-}" = "$package" ]; then
    printf 'warning: A function call in method fixture overwrites values in the frame\n'
fi
SH
chmod +x "$BIN/cargo-build-sbf" "$BIN/cargo"

pass=0
fail=0
ok() { pass=$((pass + 1)); printf 'ok %s - %s\n' "$pass" "$1"; }
not_ok() { fail=$((fail + 1)); printf 'not ok - %s\n' "$1" >&2; }

capture="$SCRATCH/capture.json"
if PATH="$BIN:$PATH" "$HERE/run.sh" --source "$FIXTURE" --capture "$capture" \
    >"$SCRATCH/stdout" 2>"$SCRATCH/stderr"; then
    ok "a fresh exact full-link capture runs"
else
    sed -n '1,12p' "$SCRATCH/stderr" >&2
    not_ok "a fresh exact full-link capture runs"
fi

set +e
PATH="$BIN:$PATH" FRAMEGUARD_INJECT_DIAGNOSTIC=program-06 \
    "$HERE/run.sh" --source "$FIXTURE" --capture "$SCRATCH/rejected.json" \
    >"$SCRATCH/stdout" 2>"$SCRATCH/stderr"
code=$?
set -e
if [ "$code" = 1 ] \
    && grep -Fq 'emitted 1 stack-frame overwrite diagnostics' "$SCRATCH/stderr" \
    && [ ! -e "$SCRATCH/rejected.json" ]; then
    ok "a zero-exit build with an SBF frame diagnostic is still red"
else
    sed -n '1,12p' "$SCRATCH/stderr" >&2
    not_ok "a zero-exit build with an SBF frame diagnostic is still red"
fi

mkdir "$SCRATCH/no-sbf-bin"
ln -s "$(command -v python3)" "$SCRATCH/no-sbf-bin/python3"
ln -s "$(command -v basename)" "$SCRATCH/no-sbf-bin/basename"
set +e
PATH="$SCRATCH/no-sbf-bin" /bin/bash "$HERE/run.sh" --source "$FIXTURE" \
    --capture "$SCRATCH/unmeasured.json" >"$SCRATCH/stdout" 2>"$SCRATCH/stderr"
code=$?
set -e
if [ "$code" = 2 ] && grep -Fq 'cargo-build-sbf is not on PATH' "$SCRATCH/stderr"; then
    ok "missing SBF toolchain is exit two, not a passing or broken tree"
else
    sed -n '1,12p' "$SCRATCH/stderr" >&2
    not_ok "missing SBF toolchain is exit two, not a passing or broken tree"
fi

if [ "$fail" -ne 0 ]; then
    printf '%s tests failed; %s passed\n' "$fail" "$pass" >&2
    exit 1
fi
printf 'all %s frameguard runner tests passed\n' "$pass"
