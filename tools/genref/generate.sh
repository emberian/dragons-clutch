#!/usr/bin/env bash
# tools/genref/generate.sh -- regenerate (or verify) docs/reference/ from the
# protocol's own authorities. See tools/genref/README.md.
#
#   tools/genref/generate.sh            regenerate docs/reference/
#   tools/genref/generate.sh --check    verify byte-identity, write nothing
#
# The census inventory is produced fresh from the tree on every run (it is
# the enumeration authority and is never checked in); everything else the
# generator reads is already in-tree. `--check-unique` rides along, so a
# refusal-code collision fails this gate too.

set -euo pipefail

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

exec node "$HERE/generate.mjs" --inventory "$INVENTORY" "$@"
