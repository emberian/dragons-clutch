#!/usr/bin/env bash
# tools/lane/test.sh -- self-test for tools/lane.sh.
#
# Exercises each subcommand's happy path and refusal paths against a scratch
# git repository under /tmp. Never touches the real repository, the real
# wave board, or a real crate.
set -euo pipefail

LANE_SH="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/lane.sh"
[[ -x "$LANE_SH" ]] || {
  echo "test.sh: cannot find executable lane.sh at $LANE_SH" >&2
  exit 1
}

PASS=0
FAIL=0
FAILED_NAMES=()

ok() {
  PASS=$((PASS + 1))
  echo "  ok: $1"
}

bad() {
  FAIL=$((FAIL + 1))
  FAILED_NAMES+=("$1")
  echo "FAIL: $1" >&2
}

# in_dir <dir> <cmd...> -- runs <cmd...> with cwd <dir>, WITHOUT changing the
# calling shell's cwd. Only ever called from inside a `$(...)` capture below,
# so the `cd` here affects only that already-forked subshell.
in_dir() {
  local dir="$1"
  shift
  cd "$dir" || return 1
  "$@"
}

# expect_refusal <name> <cmd...>
# Runs the command; passes if it exits nonzero AND prints something (a
# refusal must say why).
expect_refusal() {
  local name="$1"
  shift
  local out rc=0
  out="$("$@" 2>&1)" || rc=$?
  if [[ $rc -eq 0 ]]; then
    bad "$name (expected nonzero exit, got 0)"
    return
  fi
  if [[ -z "$out" ]]; then
    bad "$name (expected an explanatory message, got none)"
    return
  fi
  ok "$name"
}

expect_success() {
  local name="$1"
  shift
  local out rc=0
  out="$("$@" 2>&1)" || rc=$?
  if [[ $rc -ne 0 ]]; then
    bad "$name (expected exit 0, got $rc; output: $out)"
    return
  fi
  ok "$name"
}

WORK="$(mktemp -d "${TMPDIR:-/tmp}/lane-sh-test.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

REPO="$WORK/repo"
mkdir -p "$REPO"
git -C "$REPO" init -q
git -C "$REPO" config user.email "lane-test@example.invalid"
git -C "$REPO" config user.name "lane test"
git -C "$REPO" config commit.gpgsign false

echo "one" >"$REPO/a.txt"
echo "two" >"$REPO/b.txt"
git -C "$REPO" add a.txt b.txt
git -C "$REPO" commit -q -m "initial"

echo "=== lane.sh commit ==="

echo "changed" >"$REPO/a.txt"
expect_refusal "commit: missing --" in_dir "$REPO" "$LANE_SH" commit "msg" a.txt

expect_refusal "commit: empty path list" in_dir "$REPO" "$LANE_SH" commit "msg" --

expect_refusal "commit: wildcard path '.'" in_dir "$REPO" "$LANE_SH" commit "msg" -- .

expect_refusal "commit: wildcard path '-A'" in_dir "$REPO" "$LANE_SH" commit "msg" -- -A

mkdir -p "$REPO/sub"
echo "x" >"$REPO/sub/c.txt"
git -C "$REPO" add sub/c.txt
git -C "$REPO" commit -q -m "add sub/c.txt"
expect_refusal "commit: refuses outside repo root" in_dir "$REPO/sub" "$LANE_SH" commit "msg" -- c.txt

# a.txt is still dirty ("changed") from above; this is the happy path.
expect_success "commit: happy path commits exactly the named file" \
  in_dir "$REPO" "$LANE_SH" commit "lane test: touch a.txt only" -- a.txt

READBACK="$(git -C "$REPO" show --name-only --pretty=format: HEAD | grep -vc '^$' || true)"
if [[ "$READBACK" -eq 1 ]] && git -C "$REPO" show --name-only --pretty=format: HEAD | grep -qx 'a.txt'; then
  ok "commit: readback shows exactly a.txt"
else
  bad "commit: readback shows exactly a.txt"
fi

# Dirty b.txt AND a.txt at once; --only must still commit only b.txt, proving
# it does not silently fall back to "everything dirty" (the plain
# `git commit`/`git add -A` hazard this wrapper exists to close).
echo "changed once more" >"$REPO/b.txt"
echo "untouched-by-arg" >"$REPO/a.txt"
expect_success "commit: second happy path ignores an unrelated dirty file" \
  in_dir "$REPO" "$LANE_SH" commit "lane test: touch b.txt only" -- b.txt

READBACK2="$(git -C "$REPO" show --name-only --pretty=format: HEAD | grep -vc '^$' || true)"
if [[ "$READBACK2" -eq 1 ]] && git -C "$REPO" show --name-only --pretty=format: HEAD | grep -qx 'b.txt'; then
  ok "commit: readback shows exactly b.txt, not the unrelated dirty a.txt"
else
  bad "commit: readback shows exactly b.txt, not the unrelated dirty a.txt"
fi

# A brand-new, never-added file: `--only` alone can't see it until it's in
# the index, so `lane.sh commit` must `git add` it (named-path only) first.
echo "brand new" >"$REPO/new.txt"
echo "dirty-again" >"$REPO/b.txt"
expect_success "commit: stages and commits a brand-new untracked file" \
  in_dir "$REPO" "$LANE_SH" commit "lane test: add new.txt only" -- new.txt

READBACK3="$(git -C "$REPO" show --name-only --pretty=format: HEAD | grep -vc '^$' || true)"
if [[ "$READBACK3" -eq 1 ]] && git -C "$REPO" show --name-only --pretty=format: HEAD | grep -qx 'new.txt'; then
  ok "commit: readback shows exactly new.txt, not the unrelated dirty b.txt"
else
  bad "commit: readback shows exactly new.txt, not the unrelated dirty b.txt"
fi

echo "--- unit-testing the post-commit coverage check directly (source-only) ---"
if (
  # shellcheck disable=SC1090
  source "$LANE_SH"
  if lane_path_covered "crates/foo/bar.rs" "crates/foo"; then
    echo "  ok(sub): lane_path_covered nested-path case"
  else
    echo "  FAIL(sub): lane_path_covered nested-path case" >&2
    exit 1
  fi
  if lane_path_covered "a.txt" "a.txt"; then
    echo "  ok(sub): lane_path_covered exact-match case"
  else
    echo "  FAIL(sub): lane_path_covered exact-match case" >&2
    exit 1
  fi
  if ! lane_path_covered "other/file.rs" "crates/foo"; then
    echo "  ok(sub): lane_path_covered rejects an unrelated path (this is the mismatch 'lane.sh commit' would refuse the whole commit on)"
  else
    echo "  FAIL(sub): lane_path_covered rejects an unrelated path" >&2
    exit 1
  fi
); then
  ok "lane_path_covered: all three cases correct"
else
  bad "lane_path_covered: all three cases correct"
fi

echo "=== lane.sh fmt ==="

mkdir -p "$WORK/fmtdir"
cat >"$WORK/fmtdir/lib.rs" <<'EOF'
pub fn f(){1}
EOF
cat >"$WORK/fmtdir/leaf.rs" <<'EOF'
pub fn g(){2}
EOF

expect_refusal "fmt: no files" "$LANE_SH" fmt
expect_refusal "fmt: unknown flag" "$LANE_SH" fmt --nope "$WORK/fmtdir/leaf.rs"
expect_refusal "fmt: refuses crate root lib.rs without --allow-root" \
  "$LANE_SH" fmt "$WORK/fmtdir/lib.rs"

if command -v rustup >/dev/null 2>&1 && rustup run 1.97.1 rustfmt --version >/dev/null 2>&1; then
  expect_success "fmt: formats a named leaf file" "$LANE_SH" fmt "$WORK/fmtdir/leaf.rs"
  if grep -q 'pub fn g() {' "$WORK/fmtdir/leaf.rs"; then
    ok "fmt: leaf.rs was actually reformatted"
  else
    bad "fmt: leaf.rs was actually reformatted"
  fi
  if grep -q 'pub fn f(){1}' "$WORK/fmtdir/lib.rs"; then
    ok "fmt: lib.rs untouched by the refused call"
  else
    bad "fmt: lib.rs untouched by the refused call"
  fi
  expect_success "fmt: --allow-root permits a crate root" "$LANE_SH" fmt --allow-root "$WORK/fmtdir/lib.rs"
else
  echo "  skip: rustup toolchain 1.97.1 not available in this environment; skipping the two live-format checks" >&2
fi

echo "=== lane.sh board ==="

BOARD_FILE="$WORK/board.md"
: >"$BOARD_FILE"

expect_refusal "board: refuses unset DCLUTCH_LANE" \
  env -u DCLUTCH_LANE DCLUTCH_BOARD_FILE="$BOARD_FILE" "$LANE_SH" board "hello"

expect_refusal "board: refuses empty text" \
  env DCLUTCH_LANE="LANE-TEST" DCLUTCH_BOARD_FILE="$BOARD_FILE" "$LANE_SH" board

expect_success "board: appends an attributed entry" \
  env DCLUTCH_LANE="LANE-TEST" DCLUTCH_BOARD_FILE="$BOARD_FILE" "$LANE_SH" board "the self-test ran"

if grep -q "LANE-TEST" "$BOARD_FILE" && grep -q "the self-test ran" "$BOARD_FILE"; then
  ok "board: entry contains the lane name and the text"
else
  bad "board: entry contains the lane name and the text"
fi

echo "=== lane.sh guard-script ==="

SCRIPT="$WORK/watched.sh"
cat >"$SCRIPT" <<'EOF'
#!/usr/bin/env bash
echo "ran"
EOF
chmod +x "$SCRIPT"

expect_refusal "guard-script: missing --" "$LANE_SH" guard-script "$SCRIPT" true
expect_refusal "guard-script: empty command" "$LANE_SH" guard-script "$SCRIPT" --

RC=0
OUT="$("$LANE_SH" guard-script "$SCRIPT" -- true 2>&1)" || RC=$?
if [[ $RC -eq 0 && -z "$OUT" ]]; then
  ok "guard-script: unchanged script is silent and exits 0"
else
  bad "guard-script: unchanged script is silent and exits 0 (rc=$RC out=$OUT)"
fi

# A command that edits the watched script mid-run must trigger the loud
# warning, and the wrapper must still surface the wrapped command's own
# (nonzero) exit status. The inner script is single-quoted deliberately: its
# "$1" must expand in the spawned bash -c's own scope, not this one.
# shellcheck disable=SC2016
EDITOR_CMD=(bash -c 'echo "echo mutated" >> "$1"; exit 7' _ "$SCRIPT")
RC=0
OUT="$("$LANE_SH" guard-script "$SCRIPT" -- "${EDITOR_CMD[@]}" 2>&1)" || RC=$?
if [[ $RC -eq 7 ]]; then
  ok "guard-script: preserves the wrapped command's exit status"
else
  bad "guard-script: preserves the wrapped command's exit status (got $RC)"
fi
if echo "$OUT" | grep -q "CHANGED WHILE"; then
  ok "guard-script: warns loudly when the script changes mid-run"
else
  bad "guard-script: warns loudly when the script changes mid-run"
fi

echo
echo "=== summary: $PASS passed, $FAIL failed ==="
if [[ $FAIL -gt 0 ]]; then
  printf 'failed: %s\n' "${FAILED_NAMES[@]}" >&2
  exit 1
fi
