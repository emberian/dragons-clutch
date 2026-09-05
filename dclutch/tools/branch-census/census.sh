#!/usr/bin/env bash
# tools/branch-census/census.sh -- which unmerged branches hold work, and which
# are ghosts the trunk already carries.
#
# THE INCIDENT. On 2026-08-31 `git branch --no-merged main` reported FIFTY
# branches in this tree. Every reasonable reading of that number is "fifty
# things are pending". The true number was one. Forty-seven of the fifty held
# nothing main did not already have -- their work had landed days earlier under
# DIFFERENT COMMIT NAMES, which is exactly the case `--no-merged` cannot see,
# because it asks about ancestry and ancestry is not what re-landing preserves.
#
# So the list rotted into noise, and noise is not free: it hid the branches that
# WERE live, and it made "adjudicate the branches" a job nobody could size.
#
# This script answers the question `--no-merged` is usually being asked to
# answer, which is not "is this branch an ancestor of main" but:
#
#     DOES THIS BRANCH HOLD ANYTHING MAIN DOES NOT?
#
# It answers it three ways per branch, because no one way is sufficient:
#
#   1. PATCH EQUIVALENCE (`git cherry`). Catches a commit that landed with a
#      rewritten message -- the case that produced all forty-seven. Misses a
#      commit that landed reshaped, because reshaping moves the patch-id.
#   2. BRANCH-UNIQUE FILES. Catches whole files main has never seen. Its own
#      blind spot is the mirror image: it says nothing about line-level work,
#      and it reports files main DELETED on purpose as though the branch were
#      holding something (it is holding a corpse; see FILES-MAIN-DELETED).
#   3. AGE. Not evidence, but the prior that decides how hard to look. A branch
#      three days old in this tree is behind hundreds of commits.
#
# A branch that is clean on (1) and (2) is a retirement candidate. A branch that
# is not is a REVIEW -- this script never says "land it", because that judgment
# needs a human reading the diff, and getting it wrong deletes work.
#
# ---------------------------------------------------------------------------
# THE STALE-BASE TRAP, which is the reason this script picks its own base.
#
# Run the same classification in the public wrapper repo against its LOCAL
# `main` and it reports ~1600 unlanded commits per branch and argues to land
# six branches that are, every one of them, already ancestors of `origin/main`.
# The local ref was 1710 commits stale. The classification was not wrong; the
# BASE was, and a wrong base fails in the expensive direction -- it invents work
# rather than hiding it.
#
# So: if a remote-tracking `origin/main` exists and local `main` is behind it,
# this script measures against `origin/main` and SAYS SO. If there is no remote
# at all -- this tree's own case -- local `main` is the trunk and that is fine.
# What it will not do is measure against a base it has not checked.
#
# ---------------------------------------------------------------------------
# EXIT CODES, following tools/gate, which follows seam_audit.py:
#
#   0  the census RAN. Branches may still need review; that is reported in the
#      output, not in the status. This is a REPORT, not a gate -- a tree with
#      live lanes in it is healthy, and a script that failed CI for having
#      work in progress would be uninstallable.
#   2  a PREREQUISITE IS MISSING (not a git repo, no `main`). Nothing was
#      determined, either way.
#
# There is deliberately no exit 1. If you want a gate, wrap this and decide
# your own threshold; the tree does not have one to offer you.

set -eu

usage() {
  cat <<'EOF'
usage: census.sh [--base <ref>] [--live <pattern>] [--quiet]

  --base <ref>       classify against <ref> instead of the auto-detected trunk
  --live <pattern>   egrep pattern of branches known to be live lanes; they are
                     listed under their own heading instead of as REVIEW
  --quiet            summary lines only, no per-branch detail

Classifications:
  RELANDED   every commit is patch-equivalent to one already on the base, and
             the branch adds no file the base lacks. Retirement candidate.
  REVIEW     holds commits or files the base does not. Read the diff.
  LIVE       matched --live. Not adjudicated.
EOF
}

base=""
live_pattern=""
quiet=0
while [ $# -gt 0 ]; do
  case "$1" in
    --base) base="${2:-}"; shift 2 ;;
    --live) live_pattern="${2:-}"; shift 2 ;;
    --quiet) quiet=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'census: unknown argument: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

git rev-parse --git-dir >/dev/null 2>&1 || {
  printf 'census: not a git repository\n' >&2; exit 2; }

# --- pick the base, and justify the pick out loud -------------------------
if [ -z "$base" ]; then
  git rev-parse --verify -q main >/dev/null || {
    printf 'census: no local `main` to measure against; pass --base\n' >&2
    exit 2; }
  base=main
  if git rev-parse --verify -q origin/main >/dev/null 2>&1; then
    behind=$(git rev-list --count main..origin/main)
    if [ "$behind" -gt 0 ]; then
      base=origin/main
      printf 'census: local `main` is %s commits behind `origin/main`;\n' "$behind"
      printf '        measuring against `origin/main` instead. Fetch and\n'
      printf '        fast-forward `main` if you want them to agree.\n\n'
    fi
  fi
fi
git rev-parse --verify -q "$base" >/dev/null || {
  printf 'census: base `%s` does not resolve\n' "$base" >&2; exit 2; }

base_sha=$(git rev-parse --short "$base")
today=$(date +%s)

base_files=$(mktemp "${TMPDIR:-/tmp}/census-base.XXXXXX")
base_deleted=$(mktemp "${TMPDIR:-/tmp}/census-deleted.XXXXXX")
branch_extra=$(mktemp "${TMPDIR:-/tmp}/census-extra.XXXXXX")
trap 'rm -f "$base_files" "$base_deleted" "$branch_extra"' EXIT HUP INT TERM
git ls-tree -r --name-only "$base" | sort > "$base_files"

# Every path the base has ever deleted, computed ONCE.
#
# This used to be a `git log --diff-filter=D -- "$f"` inside the per-file loop
# inside the per-branch loop, which is a process per file per branch. On this
# tree (5 branches) it was imperceptible; pointed at the public wrapper (201
# branches, 190 of them agent/*) it ran for over two minutes without finishing
# a single branch. One pass over the base's history answers the same question
# for every file at once, so the cost stops multiplying by the thing this
# script exists to survey.
git log --diff-filter=D --name-only --format='' "$base" | sed '/^$/d' | sort -u > "$base_deleted"

printf 'branch census against %s (%s)\n' "$base" "$base_sha"
printf '%s\n\n' '---------------------------------------------------------------'

relanded=0; review=0; livecount=0
relanded_names=""; review_names=""

for b in $(git branch --no-merged "$base" --format='%(refname:short)'); do
  if [ -n "$live_pattern" ] && printf '%s' "$b" | grep -qE "$live_pattern"; then
    livecount=$((livecount + 1))
    [ "$quiet" -eq 1 ] || printf 'LIVE      %-52s %s\n' "$b" "$(git rev-parse --short "$b")"
    continue
  fi

  tip=$(git rev-parse --short "$b")
  ahead=$(git rev-list --count "$base..$b")
  ts=$(git log -1 --format='%ct' "$b")
  age=$(( (today - ts) / 86400 ))

  # (1) patch equivalence: how many of this branch's commits are NOT on base
  unique_commits=$(git cherry "$base" "$b" | grep -c '^+' || true)

  # (2) files the branch has that base does not, minus the ones base DELETED
  #     on purpose -- a branch still carrying a deleted file is holding a
  #     corpse, not an asset, and counting it as work is how you resurrect
  #     something the trunk decided to bury.
  git ls-tree -r --name-only "$b" | sort | comm -23 - "$base_files" \
    | grep -v '^\.claude/' > "$branch_extra" || true
  corpses=$(comm -12 "$branch_extra" "$base_deleted" | wc -l | tr -d ' ')
  uniq_files=$(comm -23 "$branch_extra" "$base_deleted" | wc -l | tr -d ' ')

  if [ "$unique_commits" -eq 0 ] && [ "$uniq_files" -eq 0 ]; then
    relanded=$((relanded + 1)); relanded_names="$relanded_names $b"
    [ "$quiet" -eq 1 ] || printf 'RELANDED  %-52s %s  %2dd  %2d commits, all landed; %d file(s) main deleted\n' \
      "$b" "$tip" "$age" "$ahead" "$corpses"
  else
    review=$((review + 1)); review_names="$review_names $b"
    [ "$quiet" -eq 1 ] || printf 'REVIEW    %-52s %s  %2dd  %d unlanded commit(s), %d file(s) main lacks\n' \
      "$b" "$tip" "$age" "$unique_commits" "$uniq_files"
  fi
done

printf '\n%s\n' '---------------------------------------------------------------'
printf 'live %d   relanded %d   review %d\n' "$livecount" "$relanded" "$review"

if [ "$relanded" -gt 0 ]; then
  printf '\n%d branch(es) hold nothing %s lacks. Retiring one means writing its\n' "$relanded" "$base"
  printf 'tombstone first -- name, tip SHA, what it held, and the commit that\n'
  printf 'superseded it -- because the tip SHA is the only way back afterwards.\n'
  printf 'See docs/evidence/BRANCH_ADJUDICATION_2026_08_31.md for the format.\n'
fi

exit 0
