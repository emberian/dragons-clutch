#!/usr/bin/env bash
# Publish the live dClutch tree into the public subtree host as a single-parent
# content-sync commit.
#
#   tools/cut.sh [--dry-run] [-m <message-file>]
#
# WHAT THIS IS. The public repo `dragons-clutch` carries the protocol under
# `dclutch/`. This does not merge histories and does not use `git subtree`:
# it builds ONE commit whose `dclutch/` tree is byte-identical to the live
# tree's HEAD, parented on the current `origin/main`. History on either side
# stays its own; the public repo gets content, not archaeology.
#
# WHY IT CUTS FROM HEAD, NOT THE WORKING TREE. The live tree is dirty on
# purpose -- lanes stop at honest walls and leave work uncommitted. A cut from
# the working tree would publish a state no commit names, which is
# unreproducible evidence and C-14 forbids it. So: HEAD only, and the commit
# message records which HEAD.
#
# THE GATE. After building the commit, the new commit's `dclutch` tree object
# is compared against the live HEAD tree object. Equal or the push does not
# happen. This is exact and costs nothing -- it is a tree-hash comparison, not
# a file walk, so there is no "mostly synced" outcome to argue about.
#
# THE SWEEP. Credentials are checked as a VALUE test before the push: the sweep
# must find zero, and a nonzero finding aborts. Publication is not reversible
# in the way a local commit is -- a pushed secret is a leaked secret even if
# the next commit removes it.
set -euo pipefail

LIVE="${LIVE:-/Users/ember/dev/dclutch}"
PUB="${PUB:-/Users/ember/dev/dragons-clutch}"
PREFIX="${PREFIX:-dclutch}"
BRANCH="${BRANCH:-main}"

DRY=0
MSGFILE=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY=1; shift ;;
    -m) MSGFILE="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

say() { printf '%s\n' "$*"; }
die() { printf 'cut: %s\n' "$*" >&2; exit 1; }

[ -d "$LIVE/.git" ] || die "live tree is not a git repository: $LIVE"
[ -d "$PUB/.git" ] || die "publication host is not a git repository: $PUB"

LIVE_COMMIT="$(git -C "$LIVE" rev-parse HEAD)"
LIVE_TREE="$(git -C "$LIVE" rev-parse 'HEAD^{tree}')"
LIVE_SHORT="$(git -C "$LIVE" rev-parse --short HEAD)"
LIVE_DIRTY="$(git -C "$LIVE" status --porcelain | wc -l | tr -d ' ')"

say "live      $LIVE (HEAD $LIVE_SHORT, tree $LIVE_TREE)"
say "          $LIVE_DIRTY uncommitted path(s) NOT published -- this cut is HEAD"

# --- transport: make the live tree object reachable inside the public repo ---
git -C "$PUB" fetch --no-tags --quiet "$LIVE" "$LIVE_COMMIT:refs/cut/live"
git -C "$PUB" rev-parse --verify --quiet "$LIVE_TREE^{tree}" >/dev/null \
  || die "live tree object did not arrive in the publication host"

git -C "$PUB" fetch --no-tags --quiet origin "$BRANCH"
BASE="$(git -C "$PUB" rev-parse "origin/$BRANCH")"
BASE_TREE="$(git -C "$PUB" rev-parse "origin/$BRANCH:$PREFIX" 2>/dev/null || echo none)"
say "base      origin/$BRANCH $(git -C "$PUB" rev-parse --short "$BASE") ($PREFIX tree $BASE_TREE)"

if [ "$BASE_TREE" = "$LIVE_TREE" ]; then
  say "nothing to cut: origin/$BRANCH already carries this exact tree"
  exit 0
fi

# --- sweep: value test, must find zero -------------------------------------
say "sweep     scanning the tree about to be published"
SWEEP_OUT="$(mktemp)"
trap 'rm -f "$SWEEP_OUT"' EXIT

# Solana/ed25519 secret keypairs are 64-element byte arrays in .json.
git -C "$PUB" ls-tree -r --name-only "$LIVE_TREE" \
  | while IFS= read -r path; do
      case "$path" in
        *keypair*.json|*/id.json|.env|*/.env|*.pem|*.key|*id_rsa*|*id_ed25519*)
          printf 'NAME\t%s\n' "$path" ;;
      esac
    done >>"$SWEEP_OUT" || true

git -C "$PUB" grep -n -I -E \
  -e 'BEGIN (RSA |EC |OPENSSH |PGP )?PRIVATE KEY' \
  -e 'AKIA[0-9A-Z]{16}' \
  -e 'sk-[A-Za-z0-9]{32,}' \
  -e 'xox[baprs]-[A-Za-z0-9-]{10,}' \
  -e 'gh[pousr]_[A-Za-z0-9]{36,}' \
  "$LIVE_TREE" 2>/dev/null \
  | sed 's/^/CONTENT\t/' >>"$SWEEP_OUT" || true

# A 64-number JSON array is the exact shape of an exported secret key.
git -C "$PUB" grep -n -I -E '^\[( *[0-9]{1,3},){63} *[0-9]{1,3} *\]' "$LIVE_TREE" -- '*.json' 2>/dev/null \
  | sed 's/^/KEYSHAPE\t/' >>"$SWEEP_OUT" || true

FINDINGS="$(grep -c . "$SWEEP_OUT" || true)"
if [ "$FINDINGS" != "0" ]; then
  say "sweep     REFUSED -- $FINDINGS finding(s):"
  cat "$SWEEP_OUT" >&2
  die "credential sweep is a value test and it found something; nothing was pushed"
fi
say "sweep     0 findings"

# --- build the single-parent content-sync commit ---------------------------
if [ -z "$MSGFILE" ]; then
  MSGFILE="$(mktemp)"
  # The previous cut's message names the live commit it published ("live tree
  # at <sha>"), so the range this cut carries is recoverable from the public
  # history alone; the subject and body are the live commits' own subjects,
  # so the public log reads like the work rather than like a counter.
  # Search the last twenty public commits, not only the tip: a workflow merge
  # or a hand commit on the wrapper's main carries no marker of its own.
  PREV_LIVE="$(git -C "$PUB" log -20 --format=%B "$BASE" \
    | sed -n 's/.*live tree at \([0-9a-f]\{40\}\).*/\1/p' | head -1)"
  if [ -n "$PREV_LIVE" ] && git -C "$LIVE" cat-file -e "$PREV_LIVE^{commit}" 2>/dev/null; then
    RANGE="$PREV_LIVE..$LIVE_COMMIT"
  else
    RANGE="$LIVE_COMMIT^..$LIVE_COMMIT"
  fi
  RANGE_COUNT="$(git -C "$LIVE" rev-list --count "$RANGE")"
  SUBJECT="$(git -C "$LIVE" log -1 --format=%s "$LIVE_COMMIT")"
  {
    if [ "$RANGE_COUNT" -gt 1 ]; then
      printf 'dclutch %s (+%s): %s\n\n' "$LIVE_SHORT" "$((RANGE_COUNT - 1))" "$SUBJECT"
      printf 'Carries %s live commits:\n\n' "$RANGE_COUNT"
      git -C "$LIVE" log --reverse --format='  %h %s' "$RANGE"
      printf '\n'
    else
      printf 'dclutch %s: %s\n\n' "$LIVE_SHORT" "$SUBJECT"
    fi
    printf 'Content sync, single parent. The %s/ tree of this commit is\n' "$PREFIX"
    printf 'byte-identical to the live tree at %s.\n' "$LIVE_COMMIT"
  } >"$MSGFILE"
fi

# Compose the new top-level tree directly: every entry of the base tree, with
# the one named PREFIX pointed at the live tree object. Only the root listing
# is touched, so this is a handful of entries rather than a walk of the tree,
# and no index is involved -- an index would carry the base's `dclutch/...`
# file entries and collide with adding `dclutch` as a tree.
NEW_TREE="$(
  git -C "$PUB" ls-tree "$BASE" \
    | awk -v t="$LIVE_TREE" -v p="$PREFIX" '
        $4 == p { print "040000 tree " t "\t" p; seen = 1; next }
        { print }
        END { if (!seen) print "040000 tree " t "\t" p }
      ' \
    | git -C "$PUB" mktree
)"
NEW_COMMIT="$(git -C "$PUB" commit-tree "$NEW_TREE" -p "$BASE" -F "$MSGFILE")"

# --- the gate --------------------------------------------------------------
GOT="$(git -C "$PUB" rev-parse "$NEW_COMMIT:$PREFIX")"
[ "$GOT" = "$LIVE_TREE" ] \
  || die "GATE FAILED: published $PREFIX tree $GOT != live tree $LIVE_TREE"
say "gate      $NEW_COMMIT:$PREFIX == live HEAD tree"

if [ "$DRY" = "1" ]; then
  say "dry-run   built $NEW_COMMIT; not pushed"
  exit 0
fi

git -C "$PUB" push --quiet origin "$NEW_COMMIT:refs/heads/$BRANCH"
say "pushed    $(git -C "$PUB" rev-parse --short "$NEW_COMMIT") -> origin/$BRANCH"
