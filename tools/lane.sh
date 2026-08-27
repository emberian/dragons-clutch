#!/usr/bin/env bash
# tools/lane.sh -- the lane wrapper retiring four recurring accident classes.
#
# WAVE.md's "closing pattern language" (2026-08-27), pattern 7:
#   "LANE WRAPPER: tools/lane.sh -- enforced --only, pinned rustfmt, board
#    helper; retires four recurring accident classes."
#
# The raw git/rustfmt/board commands documented in WAVE.md and
# tools/gauntlet/{TIERS,README}.md remain valid on their own; this script
# just refuses to run them the way that has already cost real lane-hours.
# See tools/lane/README.md for the incident behind each subcommand, or run
# `tools/lane.sh <subcommand> --help`.
#
# Subcommands:
#   commit <msg> -- <paths...>          enforced `git commit --only`
#   fmt [--allow-root] <file.rs>...     pinned rustfmt, named files only
#   board <text...>                     attributed, timestamped board entry
#   guard-script <script> -- <cmd...>   inode/hash-guarded script execution

lane_die() {
  printf 'lane: %s\n' "$1" >&2
  exit "${2:-1}"
}

lane_top_help() {
  cat <<'EOF'
usage: lane.sh <subcommand> [args...]

subcommands:
  commit <msg> -- <paths...>          enforced `git commit --only --no-gpg-sign`
  fmt [--allow-root] <file.rs>...     pinned rustfmt, named files only
  board <text...>                     attributed, timestamped board entry
  guard-script <script> -- <cmd...>   inode/hash-guarded script execution

Run `lane.sh <subcommand> --help` for the specific incident each one exists
to close. See also tools/lane/README.md.
EOF
}

# ---------------------------------------------------------------------------
# commit
# ---------------------------------------------------------------------------

lane_commit_help() {
  cat <<'EOF'
usage: lane.sh commit <message> -- <path> [<path> ...]

Runs exactly:
    git add -- <path> ...
    git commit --only --no-gpg-sign -m <message> -- <path> ...

(the `git add` is scoped to exactly the named paths -- never `-A` -- and
exists only so a brand-new file is visible to `--only` at all; it does not
touch anything else the shared index holds), then reads back the commit's actually-changed paths (`git show --name-only`)
and fails LOUDLY (the commit still exists; this wrapper never rewrites
history) if any path outside the given list was touched. Must be run from
the repository root, so the readback comparison and your pathspecs agree on
what "the given list" means.

Refuses:
  - an empty path list, or a path list of only "." / ".." / "/" / "*" /
    "-A" / "--all" -- these all degrade `git commit --only` back to
    "commit whatever the shared index/working tree happens to hold right
    now", which is the exact hazard below.
  - being run outside the repository root (git rev-parse --show-prefix is
    non-empty).
  - a post-commit readback that names paths you did not.

Incident this prevents: WAVE.md's lane protocol (2026-08-26 -> 2026-08-27)
used to be "named-file staging only; inspect the complete staged path list
before every commit" followed by a plain `git commit`. That inspect-then-act
sequence is a race against every other lane touching the same shared git
index, and WAVE.md records it caused "two collisions on 2026-08-26" before
the protocol changed to `git commit --only -- <paths>` exclusively, because
`--only` takes the named paths' CURRENT WORKING-TREE CONTENT regardless of
what is or is not staged, and leaves every other path exactly as it was in
HEAD -- so a concurrent `git add` anywhere else in the tree cannot leak into
your commit. It only works if the path list is real and non-empty; this
wrapper exists so nobody re-introduces the race by calling `--only` with no
paths (or a wildcard standing in for "all of them") out of habit.
EOF
}

# lane_path_covered <path> <candidate...>
# True if <path> equals one of the candidates, or is nested under one of them.
lane_path_covered() {
  local c="$1"
  shift
  local g
  for g in "$@"; do
    if [[ "$c" == "$g" || "$c" == "$g"/* ]]; then
      return 0
    fi
  done
  return 1
}

lane_cmd_commit() {
  if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    lane_commit_help
    return 0
  fi
  if [[ $# -lt 1 ]]; then
    lane_commit_help >&2
    lane_die "commit: missing <message> (usage: lane.sh commit <message> -- <paths...>)" 2
  fi
  local msg="$1"
  shift
  if [[ "${1:-}" != "--" ]]; then
    lane_commit_help >&2
    lane_die "commit: expected '--' before the path list, got '${1:-<nothing>}'" 2
  fi
  shift

  local -a paths=("$@")
  if [[ ${#paths[@]} -eq 0 ]]; then
    lane_die "commit: refusing an empty path list -- 'git commit --only' with no paths commits whatever the shared index/working tree holds, the exact bare-commit/'git add -A' hazard this wrapper closes (see 'lane.sh commit --help')" 1
  fi

  local p
  for p in "${paths[@]}"; do
    case "$p" in
    "" | "." | ".." | "/" | "*" | "-A" | "--all")
      lane_die "commit: refusing wildcard/whole-tree path '$p' -- name explicit files or directories (see 'lane.sh commit --help')" 1
      ;;
    esac
  done

  git rev-parse --is-inside-work-tree >/dev/null 2>&1 ||
    lane_die "commit: not inside a git working tree" 1
  local prefix
  prefix="$(git rev-parse --show-prefix)"
  if [[ -n "$prefix" ]]; then
    lane_die "commit: run from the repository root, not '$prefix' -- the post-commit readback compares repo-root-relative paths (see 'lane.sh commit --help')" 1
  fi

  # `--only` takes tracked/staged paths' working-tree content regardless of
  # index state, but a brand-new file has to exist in the index at all
  # before `--only` can see it. `git add` on exactly the named paths (never
  # `-A`) is the doctrine AGENTS.md already states ("named-file staging
  # only"); `--only` below still ignores anything else the shared index
  # holds.
  git add -- "${paths[@]}" ||
    lane_die "commit: git add on the named paths failed" 1

  git commit --only --no-gpg-sign -m "$msg" -- "${paths[@]}" ||
    lane_die "commit: git commit --only failed; nothing further to verify" 1

  local -a norm=()
  for p in "${paths[@]}"; do
    p="${p#./}"
    p="${p%/}"
    norm+=("$p")
  done

  local -a committed=()
  local line
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    committed+=("$line")
  done < <(git show --no-color --name-only --pretty=format: HEAD)

  local -a extra=()
  local c
  for c in "${committed[@]}"; do
    if ! lane_path_covered "$c" "${norm[@]}"; then
      extra+=("$c")
    fi
  done

  if [[ ${#extra[@]} -gt 0 ]]; then
    {
      echo "lane commit: POST-COMMIT VERIFICATION FAILED."
      echo "$(git rev-parse HEAD) touched paths outside what you named:"
      printf '  %s\n' "${extra[@]}"
      echo "Named paths were: ${paths[*]}"
      echo "The commit already exists -- this wrapper does not rewrite"
      echo "history. Inspect it by hand ('git show HEAD') before doing"
      echo "anything else; this is the failure mode 'git commit --only'"
      echo "was adopted to close, so a hit here means investigate the"
      echo "pathspec or git behavior itself, not just re-run."
    } >&2
    exit 1
  fi
}

# ---------------------------------------------------------------------------
# fmt
# ---------------------------------------------------------------------------

lane_fmt_help() {
  cat <<'EOF'
usage: lane.sh fmt [--allow-root] <file.rs> [<file.rs> ...]

Runs exactly:
    rustup run 1.97.1 rustfmt --edition 2024 -- <file.rs> ...

Never `cargo fmt -p <crate>` (reformats every file in the crate, including
files another lane currently owns) and never a bare `rustfmt` (whatever
toolchain/edition happens to be ambient on PATH).

Incident this prevents: WAVE.md's cook summary (2026-08-27) --
"Formatting: use `rustup run 1.97.1 rustfmt --edition 2024` -- bare rustfmt
is unpinned and reflows ~178 lines of hot_v3." Commits 3b0c588, d394cd9, and
d7bfb7d each had to hand-untangle a rustfmt version/edition mismatch from
real statement changes sitting in the same diff of a file several lanes
share.

Refuses a bare crate/module root filename (lib.rs, main.rs, mod.rs) unless
--allow-root is given: rustfmt run on a root file follows every `mod`
declaration that file contains and reformats each of those files too --
silently reflowing far more than the one file you named (the mod-following
hazard). This check is filename-based, not a parse of the file's `mod`
statements; pass --allow-root deliberately when a root file really is what
you mean to format.
EOF
}

lane_cmd_fmt() {
  local allow_root=0
  local -a files=()
  local arg
  for arg in "$@"; do
    case "$arg" in
    -h | --help)
      lane_fmt_help
      return 0
      ;;
    --allow-root)
      allow_root=1
      ;;
    -*)
      lane_fmt_help >&2
      lane_die "fmt: unknown flag '$arg'" 2
      ;;
    *)
      files+=("$arg")
      ;;
    esac
  done

  if [[ ${#files[@]} -eq 0 ]]; then
    lane_fmt_help >&2
    lane_die "fmt: no files given" 2
  fi

  local f base
  for f in "${files[@]}"; do
    [[ -f "$f" ]] || lane_die "fmt: not a file: $f" 1
    if [[ $allow_root -eq 0 ]]; then
      base="$(basename -- "$f")"
      case "$base" in
      lib.rs | main.rs | mod.rs)
        lane_die "fmt: refusing crate/module root '$f' (the mod-following hazard) -- pass --allow-root if you mean it (see 'lane.sh fmt --help')" 1
        ;;
      esac
    fi
  done

  command -v rustup >/dev/null 2>&1 ||
    lane_die "fmt: rustup not found -- this wrapper exists specifically to avoid a bare/unpinned rustfmt (see 'lane.sh fmt --help')" 1

  rustup run 1.97.1 rustfmt --edition 2024 -- "${files[@]}"
}

# ---------------------------------------------------------------------------
# board
# ---------------------------------------------------------------------------

lane_board_help() {
  cat <<'EOF'
usage: lane.sh board <text...>

Appends a timestamped, lane-attributed entry to the cross-lane wave board
(default /private/tmp/dclutch-wave-board.md; override with
$DCLUTCH_BOARD_FILE, mainly for tests). Requires $DCLUTCH_LANE to be set;
refuses otherwise, so every entry can be traced back to the lane that wrote
it.

Incident this prevents: the board's own protocol text already asks for this
("append a timestamped entry ... with your lane name"), because the failure
mode when it does not happen is expensive and recorded on the board itself:
the TA-SER entry ("TIER NUMBER COLLISION, my fault") where two lanes
silently clobbered each other's tools/gauntlet/tier2/ files for about
fifteen minutes, and DA2's leaked-validator note that containment on a
shared resource "was a lane remembering to be polite rather than something
structural." An unattributed board entry is the same failure mode: nobody to
ask, nobody who owns the fix.
EOF
}

lane_cmd_board() {
  if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    lane_board_help
    return 0
  fi
  if [[ -z "${DCLUTCH_LANE:-}" ]]; then
    lane_board_help >&2
    lane_die "board: \$DCLUTCH_LANE is unset -- board entries must be attributed to a lane (see 'lane.sh board --help')" 1
  fi
  if [[ $# -eq 0 ]]; then
    lane_die "board: empty entry text" 2
  fi

  local text="$*"
  local board_file="${DCLUTCH_BOARD_FILE:-/private/tmp/dclutch-wave-board.md}"
  local ts
  ts="$(date '+%Y-%m-%d %H:%M %Z')"

  {
    printf '\n## %s -- %s\n\n' "$ts" "$DCLUTCH_LANE"
    printf '%s\n' "$text"
  } >>"$board_file"
}

# ---------------------------------------------------------------------------
# guard-script
# ---------------------------------------------------------------------------

lane_guard_script_help() {
  cat <<'EOF'
usage: lane.sh guard-script <script> -- <cmd...>

Snapshots <script>'s inode + sha256 before running <cmd...>, runs it to
completion (its exit status is this wrapper's exit status), then
re-snapshots and warns LOUDLY on stderr if <script> changed mid-run.

Incident this prevents: tools/gauntlet/TIERS.md / README.md -- "never edit
run.sh while a run is in flight. Bash reads a script incrementally by byte
offset, so an edit mid-run shifts what it reads next and it will re-execute
or skip a block" (the README calls this "a corollary that cost this lane an
hour"). The same hazard is why tools/gauntlet/direct/ lives in its own
directory rather than as a run.sh stage: a --mode full run was already in
flight, editing run.sh mid-run was unsafe, and separately "three lanes
claimed the same two [tier] numbers inside twenty minutes" the day that
landed -- two independent incidents from the one root cause of scripts being
mutated out from under a running interpreter.

This wrapper cannot make a mid-run edit safe -- nothing can, short of not
doing it. It only guarantees you find out.
EOF
}

lane_stat_inode() {
  stat -f '%i' "$1" 2>/dev/null || stat -c '%i' "$1" 2>/dev/null
}

lane_hash_file() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    sha256sum "$1" | awk '{print $1}'
  fi
}

lane_cmd_guard_script() {
  if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    lane_guard_script_help
    return 0
  fi
  if [[ $# -lt 1 ]]; then
    lane_guard_script_help >&2
    lane_die "guard-script: missing <script> (usage: lane.sh guard-script <script> -- <cmd...>)" 2
  fi
  local script="$1"
  shift
  if [[ "${1:-}" != "--" ]]; then
    lane_guard_script_help >&2
    lane_die "guard-script: expected '--' before the command, got '${1:-<nothing>}'" 2
  fi
  shift
  if [[ $# -eq 0 ]]; then
    lane_die "guard-script: empty command" 2
  fi
  [[ -f "$script" ]] || lane_die "guard-script: not a file: $script" 1

  local inode1 hash1 inode2 hash2
  inode1="$(lane_stat_inode "$script")"
  hash1="$(lane_hash_file "$script")"

  local rc=0
  "$@" || rc=$?

  if [[ -f "$script" ]]; then
    inode2="$(lane_stat_inode "$script")"
    hash2="$(lane_hash_file "$script")"
  else
    inode2="<gone>"
    hash2="<gone>"
  fi

  if [[ "$inode1" != "$inode2" || "$hash1" != "$hash2" ]]; then
    {
      echo "lane guard-script: '$script' CHANGED WHILE '$*' WAS RUNNING."
      echo "  inode: $inode1 -> $inode2"
      echo "  sha256: $hash1 -> $hash2"
      echo "Bash reads a script incrementally by byte offset; a mid-run edit"
      echo "can make it re-execute or skip a block (tools/gauntlet/TIERS.md,"
      echo "tools/gauntlet/README.md). Anything '$*' did after the edit landed"
      echo "may not reflect the script you started with. Do not trust this"
      echo "run's output without checking what changed and when."
    } >&2
  fi

  return "$rc"
}

# ---------------------------------------------------------------------------
# dispatch
# ---------------------------------------------------------------------------

lane_main() {
  set -euo pipefail
  local sub="${1:-}"
  case "$sub" in
  commit)
    shift
    lane_cmd_commit "$@"
    ;;
  fmt)
    shift
    lane_cmd_fmt "$@"
    ;;
  board)
    shift
    lane_cmd_board "$@"
    ;;
  guard-script)
    shift
    lane_cmd_guard_script "$@"
    ;;
  -h | --help | help | "")
    lane_top_help
    ;;
  *)
    lane_top_help >&2
    lane_die "unknown subcommand: $sub" 2
    ;;
  esac
}

if [[ "${BASH_SOURCE[0]:-$0}" == "${0}" ]]; then
  lane_main "$@"
fi
