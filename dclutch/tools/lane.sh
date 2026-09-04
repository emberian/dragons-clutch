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
#   commit <msg>|-F <file> -- <paths...> enforced `git commit --only`
#   commit-patch <msg> <patch-file>     HEAD plus your hunk, for shared files
#   fmt [--allow-root] <file.rs>...     pinned rustfmt, named files only
#   board <text...>                     attributed, timestamped board entry
#   guard-script <script> -- <cmd...>   inode/hash-guarded script execution

lane_die() {
  printf 'lane: %s\n' "$1" >&2
  exit "${2:-1}"
}

# ---------------------------------------------------------------------------
# The `Lane:` trailer.
#
# Every lane in this tree commits as the same git author, so `git log` can name
# a commit and no instrument can name the lane that wrote it. On 2026-09-02
# three lanes mis-attributed each other's commits in one afternoon, and
# `frameguard.py owed` -- whose whole output is a ledger of WHO owes frame rows
# -- could only ever print "ember arlynx" beside every debtor.
#
# A TRAILER, never message prose: a trailer is a parsed field
# (`%(trailers:key=Lane)`), so a reader gets it without regexing a subject
# line, and a lane cannot lose it to the backtick command-substitution hazard
# that eats code spans out of shell-quoted messages.
#
# `DCLUTCH_LANE` is the lane's own name for itself and AGENTS.md tells every
# prompt to set it. Falling back to a session id rather than refusing is
# deliberate: an un-set variable must not be able to block a commit, and a
# session id is still a discriminator -- two lanes with `unknown` are at least
# known to be two DIFFERENT unknowns when their session ids differ. `unknown`
# is the last resort and is honest about being one.
lane_id() {
  local id="${DCLUTCH_LANE:-}"
  [[ -n "$id" ]] || id="${CLAUDE_CODE_SESSION_ID:-}"
  [[ -n "$id" ]] || id="${TERM_SESSION_ID:-}"
  [[ -n "$id" ]] || id="unknown"
  # A trailer value is one line. Collapse any whitespace the environment
  # carried in rather than emitting a malformed trailer that no parser sees.
  id="$(printf '%s' "$id" | tr '\n\r\t' '   ' | sed 's/  */ /g; s/^ //; s/ $//')"
  [[ -n "$id" ]] || id="unknown"
  printf '%s' "$id"
}

lane_top_help() {
  cat <<'EOF'
usage: lane.sh <subcommand> [args...]

subcommands:
  commit <msg>|-F <file> -- <paths...> enforced `git commit --only --no-gpg-sign`
  commit-patch <msg> <patch-file>     HEAD plus your hunk, for shared files
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
    git commit --only --no-gpg-sign --trailer "Lane: $DCLUTCH_LANE" \
        -m <message> -- <path> ...

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

Every commit carries a `Lane:` trailer naming who made it. Every lane in this
tree commits as the SAME git author, so before this a reader could name a
commit and never its lane -- three lanes mis-attributed each other's commits
in one afternoon, and `frameguard.py owed`, whose entire output is a ledger of
who owes frame rows, printed the same name beside every debtor. Set
`DCLUTCH_LANE` to your lane's name; unset, it falls back to the session id,
and then to `unknown`. It is a trailer and not message prose so that
`git log --format=%(trailers:key=Lane,valueonly)` reads it without parsing a
subject line -- and so it cannot be eaten by the backtick command-substitution
that takes code spans out of shell-quoted messages.
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
  # `-F <file>` reads a multi-paragraph message from a file, so a prose body
  # with blank lines and code spans never has to survive a shell's quoting.
  local msg
  if [[ "$1" == "-F" ]]; then
    [[ -r "${2:-}" ]] || lane_die "commit: -F needs a readable message file, got '${2:-<nothing>}'" 2
    msg="$(cat "$2")"
    shift 2
  else
    msg="$1"
    shift
  fi
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

  git commit --only --no-gpg-sign --trailer "Lane: $(lane_id)" -m "$msg" -- "${paths[@]}" ||
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
# commit-patch
# ---------------------------------------------------------------------------

lane_commit_patch_help() {
  cat <<'EOF'
usage: lane.sh commit-patch <message> <patch-file>

Runs, in order:
    git apply --cached --check <patch-file>
    git apply --cached <patch-file>
    git commit --no-gpg-sign --trailer "Lane: $DCLUTCH_LANE" -m <message>

For a SHARED file that several lanes edit at once -- the workspace `members`
list above all -- where `lane.sh commit` cannot help you.

`commit --only` protects other PATHS; it cannot protect other HUNKS in a path
you name, because it takes that path's whole current WORKING-TREE content. So
committing Cargo.toml while another lane holds a line in it sweeps their line
into your commit. This subcommand commits HEAD's blob plus YOUR hunk instead:
`git apply --cached` writes only your change into the index, on top of the
blob the index already holds from HEAD -- the other lane's line stays exactly
where it was, and after your commit `git diff -- <file>` shows only theirs.

AFTER the commit it brings each path's WORKING TREE forward to match, which is
a no-op when you edited in place and the whole point when your patch was built
somewhere else (a detached worktree, the house pattern for a shared file). It
applies only where every context line matches, so it can add your hunk and can
never overwrite another lane's; where a foreign hunk blocks it, the path is
left alone and named. Without this step every path in a worktree-built patch
reads as a REVERSAL of the commit just made, and the next `--only` on one of
them silently reverts it.

Refuses:
  - a non-empty index (`git diff --cached` shows anything). The whole
    mechanism rests on the index holding HEAD's blob for your path; if
    someone has staged something, applying on top of it commits their work
    too, which is the hazard being closed rather than a variation of it.
  - a staged path set that differs from the patch's own path set, checked
    after the apply.
  - being run outside the repository root, or with a patch file that does
    not exist.

Then it reads the commit back the way `lane.sh commit` does, and prints any
path whose working tree still carries hunks that are NOT yours -- so you can
see, and say, exactly whose work you left behind.

Incidents this prevents: `0b8c377d` swept seven files another lane had staged,
and on 2026-09-02 both a rip lane and the Dealer lane serialised for half an
hour on Cargo.toml because neither could commit it without taking the other's
row. Path-granular protection is not enough for the one file every
crate-lifecycle lane must edit.

And the mirror image, the same day, which the reconciliation step above closes:
`9efc24cf` committed a shared file with `--only` while another lane's call
sites sat in its working-tree copy, carrying them to HEAD without the function
they call -- main stopped compiling. Both tools leave a footgun on the side you
are not looking at, so read `git diff` on your own paths right after either.
EOF
}

lane_cmd_commit_patch() {
  if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    lane_commit_patch_help
    return 0
  fi
  if [[ $# -ne 2 ]]; then
    lane_commit_patch_help >&2
    lane_die "commit-patch: expected exactly <message> <patch-file>" 2
  fi
  local msg="$1"
  local patch="$2"

  git rev-parse --is-inside-work-tree >/dev/null 2>&1 ||
    lane_die "commit-patch: not inside a git working tree" 1
  local prefix
  prefix="$(git rev-parse --show-prefix)"
  if [[ -n "$prefix" ]]; then
    lane_die "commit-patch: run from the repository root, not '$prefix'" 1
  fi
  [[ -f "$patch" ]] || lane_die "commit-patch: no such patch file: $patch" 2

  if [[ -n "$(git diff --cached --name-only)" ]]; then
    {
      echo "lane commit-patch: REFUSING -- the index is not empty."
      echo "Staged paths:"
      git diff --cached --name-only | sed 's/^/  /'
      echo "This subcommand commits HEAD's blob plus your patch. Applying on"
      echo "top of someone else's staged work would commit theirs too, which"
      echo "is the exact hazard it exists to close. Unstage and retry."
    } >&2
    exit 1
  fi

  local -a want=()
  local line
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    want+=("$line")
  done < <(git apply --numstat -z "$patch" 2>/dev/null | tr '\0' '\n' | awk 'NF>=3 {print $3}' | sort -u)
  if [[ ${#want[@]} -eq 0 ]]; then
    lane_die "commit-patch: the patch names no paths; nothing to commit" 2
  fi

  git apply --cached --check "$patch" ||
    lane_die "commit-patch: the patch does not apply to the index (is it against HEAD?)" 1
  git apply --cached "$patch" ||
    lane_die "commit-patch: git apply --cached failed after its own --check passed" 1

  local -a staged=()
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    staged+=("$line")
  done < <(git diff --cached --name-only | sort -u)

  local wanted_list staged_list
  wanted_list="$(printf '%s\n' "${want[@]}" | sort -u)"
  staged_list="$(printf '%s\n' "${staged[@]}" | sort -u)"
  if [[ "$wanted_list" != "$staged_list" ]]; then
    {
      echo "lane commit-patch: REFUSING -- staged paths differ from the patch's."
      echo "patch names:"; printf '  %s\n' "$wanted_list"
      echo "index holds:"; printf '  %s\n' "$staged_list"
      echo "Unstaging what this run applied; nothing was committed."
    } >&2
    git reset -q
    exit 1
  fi

  git commit --no-gpg-sign --trailer "Lane: $(lane_id)" -m "$msg" ||
    lane_die "commit-patch: git commit failed; nothing further to verify" 1

  local -a committed=()
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    committed+=("$line")
  done < <(git show --no-color --name-only --pretty=format: HEAD)

  local -a extra=()
  local c
  for c in "${committed[@]}"; do
    if ! lane_path_covered "$c" "${want[@]}"; then
      extra+=("$c")
    fi
  done
  if [[ ${#extra[@]} -gt 0 ]]; then
    {
      echo "lane commit-patch: POST-COMMIT VERIFICATION FAILED."
      echo "$(git rev-parse HEAD) touched paths the patch did not name:"
      printf '  %s\n' "${extra[@]}"
      echo "The commit already exists -- this wrapper does not rewrite history."
    } >&2
    exit 1
  fi

  # RECONCILE THE WORKING TREE, path by path, and only where it is a no-op or
  # a strict addition.
  #
  # `git apply --cached` writes the index and nothing else, which is the whole
  # mechanism -- and its cost, until 2026-09-02, was that a path whose working
  # tree did NOT already carry the hunk was left reading as a REVERSAL of the
  # commit that had just been made. That is not a cosmetic wart. A patch built
  # in a detached worktree (the house pattern for a shared file) leaves every
  # one of its paths in that state, so the next `git status` invites someone to
  # "restore" them, and the next `lane.sh commit --only` on any of those paths
  # takes the working-tree content and silently reverts the hunk. Measured that
  # day: a lane committed a helper function and its call sites through this
  # subcommand, another lane's `--only` on one of those files then landed the
  # call sites without the helper, and main stopped compiling.
  #
  # So each path is now brought forward, with the same conservatism the index
  # side already has:
  #   - the patch applies to the working tree  -> apply it (the file the next
  #     reader sees is the file that was committed);
  #   - it does not apply, but its REVERSE does -> the hunk is already there;
  #     nothing to do, which is the ordinary case where you edited in place;
  #   - neither                                 -> someone else's hunk sits in
  #     the way. Left ALONE and named, because clobbering it is the one thing
  #     this subcommand exists to make impossible.
  # `git apply` writes nothing unless every context line matches, so the first
  # branch cannot overwrite another lane's line either.
  local w
  local -a carried=() blocked=()
  for w in "${want[@]}"; do
    if git apply --check --include="$w" "$patch" >/dev/null 2>&1; then
      if git apply --include="$w" "$patch" >/dev/null 2>&1; then
        carried+=("$w")
      else
        blocked+=("$w")
      fi
    elif ! git apply --reverse --check --include="$w" "$patch" >/dev/null 2>&1; then
      blocked+=("$w")
    fi
  done
  if [[ ${#carried[@]} -gt 0 ]]; then
    echo "lane commit-patch: carried the committed hunk into the working tree for:"
    printf '  %s\n' "${carried[@]}"
  fi
  if [[ ${#blocked[@]} -gt 0 ]]; then
    echo "lane commit-patch: could NOT reconcile (someone else's hunk is in the way):"
    printf '  %s\n' "${blocked[@]}"
  fi

  for w in "${want[@]}"; do
    if [[ -n "$(git diff --name-only -- "$w")" ]]; then
      echo "lane commit-patch: $w still carries hunks that are not yours (left untouched)."
    fi
  done
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
(default /private/tmp/dclutch-wave2-board.md; override with
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
  local board_file="${DCLUTCH_BOARD_FILE:-/private/tmp/dclutch-wave2-board.md}"
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
  commit-patch)
    shift
    lane_cmd_commit_patch "$@"
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
