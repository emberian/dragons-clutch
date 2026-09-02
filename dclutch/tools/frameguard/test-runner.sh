#!/usr/bin/env bash
# Hermetic refusal tests for run.sh. The fake cargo emits the exact object and
# compile-marker shape the runner consumes; the parser stub emits complete JSON.
#
# The fixture is a real git repository because run.sh's central claim is now
# about PROVENANCE, not only about frames: a capture names the commit it
# measured, `--at` measures that commit and not the tree around it, and a
# capture from a dirty tree is refused. None of those can be tested against a
# bare directory.
#
# The fake build carries a real datum: each program's `src/frame.txt` is copied
# into its object, and the parser stub reports what it finds there. So editing
# a program source in the working tree genuinely changes what an unqualified
# measurement would report, and `--at` has something to be right about.

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/dclutch-frameguard-runner.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT
FIXTURE="$SCRATCH/source"
BIN="$SCRATCH/bin"
mkdir -p "$FIXTURE/tools/frameguard" "$FIXTURE/tools" "$FIXTURE/programs" "$BIN"

# The runner and the checker each pin the link count by hand; a fixture that
# disagrees with them tests nothing but its own arithmetic. It was 13 against a
# 12-link tool from `e6b7bf1a` until 2026-09-02, which failed BOTH measurement
# tests and so took the whole frameguard CI tier red before it ever built.
link_count="$(sed -n 's/^readonly EXPECTED_LINK_COUNT=\([0-9]*\)$/\1/p' "$HERE/run.sh")"
[ -n "$link_count" ] || { printf 'cannot read the runner link count\n' >&2; exit 1; }
for index in $(seq -w 1 "$link_count"); do
    package="program-$index"
    mkdir -p "$FIXTURE/programs/$package/src"
    printf '[package]\nname = "%s"\nversion = "0.1.0"\n' "$package" \
        > "$FIXTURE/programs/$package/Cargo.toml"
    printf '128\n' > "$FIXTURE/programs/$package/src/frame.txt"
done

cat > "$FIXTURE/tools/sbf-frame-sizes.py" <<'STUB'
#!/usr/bin/env python3
import json, sys
print(json.dumps({
    "schema": "dclutch-sbf-frame-sizes-v1",
    "bound_bytes": 4096,
    "frame_count": 1,
    "frames": [{"bytes": int(open(sys.argv[-1]).read().strip()), "symbol": "fixture"}],
}))
STUB
chmod +x "$FIXTURE/tools/sbf-frame-sizes.py"

cat > "$BIN/cargo-build-sbf" <<'STUB'
#!/usr/bin/env bash
exit 0
STUB
cat > "$BIN/cargo" <<'STUB'
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
cp "$(dirname "$manifest")/src/frame.txt" \
    "$CARGO_TARGET_DIR/sbpf-solana-solana/release/deps/$stem.o"
printf '   Compiling %s v0.1.0 (%s)\n' "$package" "$(dirname "$manifest")"
if [ "${FRAMEGUARD_INJECT_DIAGNOSTIC:-}" = "$package" ]; then
    printf 'warning: A function call in method fixture overwrites values in the frame\n'
fi
STUB
chmod +x "$BIN/cargo-build-sbf" "$BIN/cargo"

commit_fixture() {
    git -C "$FIXTURE" add -A
    git -C "$FIXTURE" -c user.name=frameguard -c user.email=frameguard@invalid \
        -c commit.gpgsign=false commit -q -m "$1"
    git -C "$FIXTURE" rev-parse HEAD
}

git -C "$FIXTURE" init -q
# The first commit's checker refuses every argument, standing in for any commit
# that predates a flag the current runner passes -- which is the whole of
# history the moment a flag is added. Measuring it is possible only because the
# instrument is NOT read from the tree being measured.
cat > "$FIXTURE/tools/frameguard/frameguard.py" <<'STUB'
#!/usr/bin/env python3
import sys
sys.exit("this checker is from the past and cannot be run")
STUB
OLD_TOOLS="$(commit_fixture 'fixture with a checker from the past')"
cp "$HERE/frameguard.py" "$FIXTURE/tools/frameguard/frameguard.py"
FIXTURE_HEAD="$(commit_fixture 'fixture')"

pass=0
fail=0
ok() { pass=$((pass + 1)); printf 'ok %s - %s\n' "$pass" "$1"; }
not_ok() { fail=$((fail + 1)); printf 'not ok - %s\n' "$1" >&2; }
recorded() { python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("commit"))' "$1"; }
measured() { python3 -c '
import json, sys
value = json.load(open(sys.argv[1]))
print(value["links"][0]["functions"][0]["frames_bytes"][0])' "$1"; }
run() { PATH="$BIN:$PATH" "$HERE/run.sh" --source "$FIXTURE" --tools "$FIXTURE" "$@"; }

capture="$SCRATCH/capture.json"
if run --capture "$capture" >"$SCRATCH/stdout" 2>"$SCRATCH/stderr" \
    && [ "$(recorded "$capture")" = "$FIXTURE_HEAD" ]; then
    ok "a clean tree captures the full link set and records its own HEAD"
else
    sed -n '1,12p' "$SCRATCH/stderr" >&2
    not_ok "a clean tree captures the full link set and records its own HEAD"
fi

set +e
FRAMEGUARD_INJECT_DIAGNOSTIC=program-06 run --capture "$SCRATCH/rejected.json" \
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

# From here the fixture's working tree DISAGREES with its own HEAD: one program
# source claims a bigger frame. That is the whole hazard `--at` exists for.
printf '3712\n' > "$FIXTURE/programs/program-01/src/frame.txt"

set +e
run --capture "$SCRATCH/unnamed.json" >"$SCRATCH/stdout" 2>"$SCRATCH/stderr"
code=$?
set -e
if [ "$code" = 2 ] \
    && grep -Fq 'REFUSING to capture from a dirty tree' "$SCRATCH/stderr" \
    && [ ! -e "$SCRATCH/unnamed.json" ]; then
    ok "a capture from a dirty tree is refused, because it would name no base"
else
    sed -n '1,12p' "$SCRATCH/stderr" >&2
    not_ok "a capture from a dirty tree is refused, because it would name no base"
fi

named="$SCRATCH/named.json"
if run --at "$FIXTURE_HEAD" --capture "$named" \
    >"$SCRATCH/stdout" 2>"$SCRATCH/stderr" \
    && [ "$(recorded "$named")" = "$FIXTURE_HEAD" ] \
    && [ "$(measured "$named")" = 128 ]; then
    ok "--at measures the named commit, not the dirty tree it was run from"
else
    printf 'recorded=%s measured=%s\n' "$(recorded "$named" 2>&1)" \
        "$(measured "$named" 2>&1)" >&2
    sed -n '1,12p' "$SCRATCH/stderr" >&2
    not_ok "--at measures the named commit, not the dirty tree it was run from"
fi

if [ "$(git -C "$FIXTURE" worktree list --porcelain | grep -c '^worktree')" = 1 ]; then
    ok "the detached worktree is removed, leaving only the fixture itself"
else
    git -C "$FIXTURE" worktree list >&2
    not_ok "the detached worktree is removed, leaving only the fixture itself"
fi

set +e
run --at nonesuch --capture "$SCRATCH/nonesuch.json" \
    >"$SCRATCH/stdout" 2>"$SCRATCH/stderr"
code=$?
set -e
if [ "$code" = 2 ] && grep -Fq 'does not name a commit' "$SCRATCH/stderr" \
    && [ ! -e "$SCRATCH/nonesuch.json" ]; then
    ok "--at with a revision that does not exist is exit two, not a measurement"
else
    sed -n '1,12p' "$SCRATCH/stderr" >&2
    not_ok "--at with a revision that does not exist is exit two, not a measurement"
fi

past="$SCRATCH/past.json"
if run --at "$OLD_TOOLS" --capture "$past" >"$SCRATCH/stdout" 2>"$SCRATCH/stderr" \
    && [ "$(recorded "$past")" = "$OLD_TOOLS" ]; then
    ok "a commit whose own checker cannot run is still measurable"
else
    sed -n '1,12p' "$SCRATCH/stderr" >&2
    not_ok "a commit whose own checker cannot run is still measurable"
fi

mkdir "$SCRATCH/no-sbf-bin"
ln -s "$(command -v python3)" "$SCRATCH/no-sbf-bin/python3"
ln -s "$(command -v basename)" "$SCRATCH/no-sbf-bin/basename"
set +e
PATH="$SCRATCH/no-sbf-bin" /bin/bash "$HERE/run.sh" --source "$FIXTURE" \
    --tools "$FIXTURE" --at "$FIXTURE_HEAD" --capture "$SCRATCH/unmeasured.json" \
    >"$SCRATCH/stdout" 2>"$SCRATCH/stderr"
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
