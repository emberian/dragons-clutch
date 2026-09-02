#!/usr/bin/env bash
# tools/genref/generate.sh -- regenerate (or verify) docs/reference/ from the
# protocol's own authorities. See tools/genref/README.md.
#
#   tools/genref/generate.sh            regenerate docs/reference/
#   tools/genref/generate.sh --check    verify byte-identity, write nothing
#
# `--allow-dirty` and `GENREF_ALLOW_DIRTY=1` are the same escape spelled two
# ways, and this script is its ONE author: the dirty-tree gate is a fact about
# the working tree, which `generate.mjs` knows nothing about and must not be
# handed. Until 2026-09-02 the flag was recognised here and then FORWARDED, so
# `--allow-dirty` died in the generator as an unknown argument while the
# environment variable worked -- an escape its own header documented and that
# nobody could take. Every other argument still passes through untouched.
#
# The census inventory is produced fresh from the tree on every run (it is
# the enumeration authority and is never checked in); everything else the
# generator reads is already in-tree. `--check-unique` rides along, so a
# refusal-code collision fails this gate too.

set -euo pipefail

# Refuse a dirty tree. This generator reads the WORKING TREE, and in a shared
# checkout with live lanes that means emitting reference docs describing code
# that is not in HEAD -- and, measured 2026-09-01, silently deleting another
# lane's landed refusal rows. Regenerate from a detached worktree at HEAD
# (`git worktree add --detach <dir> HEAD`), or pass --allow-dirty when you have
# read the diff and mean it.
allow_dirty=0
[ "${GENREF_ALLOW_DIRTY:-0}" = "1" ] && allow_dirty=1
forward=()
for argument in "$@"; do
  if [ "$argument" = "--allow-dirty" ]; then
    allow_dirty=1
  else
    forward+=("$argument")
  fi
done

if [ "$allow_dirty" != "1" ]; then
  if [ -n "$(git -C "$(dirname "$0")/../.." status --porcelain 2>/dev/null)" ]; then
    echo "genref: refusing to regenerate from a dirty tree; use a detached worktree at HEAD, or --allow-dirty" >&2
    exit 3
  fi
fi

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
CENSUS_DIR="$REPO/tools/gauntlet/census"

command -v node >/dev/null 2>&1 || { echo "genref: node not found" >&2; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo "genref: cargo not found" >&2; exit 1; }

INVENTORY="$(mktemp "${TMPDIR:-/tmp}/genref-inventory.XXXXXX")"
trap 'rm -f "$INVENTORY"' EXIT

# The census tool is its own small workspace; build quietly and enumerate.
# No --revision: the reference lives in-tree, so "at this commit" is implicit
# and the output stays byte-stable for the check gate.
(
  cd "$CENSUS_DIR"
  cargo run --release --quiet -- inventory \
    --root "$REPO" \
    --out "$INVENTORY" \
    --check-unique
)

exec node "$HERE/generate.mjs" --inventory "$INVENTORY" ${forward+"${forward[@]}"}
