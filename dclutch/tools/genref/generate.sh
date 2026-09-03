#!/usr/bin/env bash
# tools/genref/generate.sh -- regenerate (or verify) docs/reference/ from the
# protocol's own authorities. See tools/genref/README.md.
#
#   tools/genref/generate.sh              regenerate docs/reference/
#   tools/genref/generate.sh --check      verify byte-identity, write nothing
#   tools/genref/generate.sh --converge   regenerate the reference AND the client
#                                         mirrors that read it, to a FIXPOINT
#   tools/genref/generate.sh --converge --check
#                                         verify the COMMITTED tree is already
#                                         that fixpoint, writing nothing here
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
#
# ---------------------------------------------------------------------------
# WHY --converge EXISTS, and why one genref pass is not enough.
#
# `generate.mjs` alone is a pure function of sources it does not write, so
# running it twice is byte-identical and a fixpoint over genref by itself is
# trivial. The cycle is one layer out:
#
#   docs/reference/refusals.md  ->  {web,sdk}/lib/generated/refusalRegistryV1.ts
#                               ->  docs/reference/abi/refusalRegistryV1.md
#
# `generate-refusal-registry.mjs` takes its per-code names and meanings FROM
# the reference, and this generator mirrors the module that script emits back
# INTO the reference. So a new refusal code lands in `refusals.md` on pass one,
# reaches the TypeScript only after the emitter runs, and reaches
# `abi/refusalRegistryV1.md` only on pass two. Any other reference-coupled
# emitter (route-census's module is mirrored here too) forces the same second
# pass for the same reason.
#
# That two-pass rule was learned by hand, written into a commit message
# (b0d3978c4 / c036b627b, the TIDY lane), and then had to be RE-learned by
# whoever read the reference next. This flag is that rule made mechanical:
# passes run until nothing moves, bounded at three -- two productive passes and
# one that proves the fixpoint -- and a third pass that still moves a file is
# REFUSED rather than accepted as "close enough". Nobody needs to know the rule
# to get a converged tree; they need to know one flag.
#
# EXIT CODES for --converge (`--check`'s vocabulary, extended, not a new one):
#   0  a fixpoint, and NOTHING moved -- the tree was already converged
#   1  a fixpoint, reached by WRITING -- commit what it wrote
#   3  the working tree is dirty (as for every other mode)
#   4  NO fixpoint within three passes -- refused, with the moving files named
# ---------------------------------------------------------------------------

set -euo pipefail

readonly EXIT_CONVERGED_CLEAN=0
readonly EXIT_CONVERGED_WROTE=1
readonly EXIT_DIRTY=3
readonly EXIT_NO_FIXPOINT=4
readonly MAX_PASSES=3

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
CENSUS_DIR="$REPO/tools/gauntlet/census"

# The emitters that CLOSE the loop: they read `docs/reference/`, and what they
# emit is mirrored back into `docs/reference/abi/`. Declared rather than
# discovered so a reader can see the cycle -- and then CHECKED against
# discovery below, because a hardcoded list that silently misses a new emitter
# would reproduce exactly the defect this flag exists to remove.
REFERENCE_COUPLED=(
  "apps/dclutch-web/scripts/generate-refusal-registry.mjs"
  "apps/dclutch-web/scripts/generate-route-census.mjs"
  "packages/dclutch-sdk/scripts/generate-refusal-registry.mjs"
  "packages/dclutch-sdk/scripts/generate-route-census.mjs"
  "packages/dclutch-sdk/scripts/generate-market-phase-admission.mjs"
)

# Every tree a pass can move. The reference and the two client mirrors of it:
# a fixpoint over the reference alone would be blind to the emitters half of
# the cycle, which is the half that needed the second pass.
CONVERGE_TREES=(
  "docs/reference"
  "apps/dclutch-web/lib/generated"
  "packages/dclutch-sdk/lib/generated"
)

# ---------------------------------------------------------------------------
# Refuse a dirty tree. This generator reads the WORKING TREE, and in a shared
# checkout with live lanes that means emitting reference docs describing code
# that is not in HEAD -- and, measured 2026-09-01, silently deleting another
# lane's landed refusal rows. Regenerate from a detached worktree at HEAD
# (`git worktree add --detach <dir> HEAD`), or pass --allow-dirty when you have
# read the diff and mean it.
# ---------------------------------------------------------------------------
allow_dirty=0
[ "${GENREF_ALLOW_DIRTY:-0}" = "1" ] && allow_dirty=1
converge=0
check=0
forward=()
for argument in "$@"; do
  case "$argument" in
  --allow-dirty) allow_dirty=1 ;;
  --converge) converge=1 ;;
  --check)
    check=1
    forward+=("$argument")
    ;;
  *) forward+=("$argument") ;;
  esac
done

tree_is_dirty() {
  [ -n "$(git -C "$REPO" status --porcelain 2>/dev/null)" ]
}

# `--converge --check` measures an ARCHIVED revision and writes nothing into
# this checkout, so the working tree's state cannot affect its answer. Gating it
# would refuse the one mode built for a shared dirty tree.
if [ "$converge" = 1 ] && [ "$check" = 1 ]; then allow_dirty=1; fi

if [ "$allow_dirty" != "1" ] && tree_is_dirty; then
  echo "genref: refusing to regenerate from a dirty tree; use a detached worktree at HEAD, or --allow-dirty" >&2
  exit $EXIT_DIRTY
fi

command -v node >/dev/null 2>&1 || { echo "genref: node not found" >&2; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo "genref: cargo not found" >&2; exit 1; }

# ---------------------------------------------------------------------------
# --converge --check: ask the question about the COMMITTED tree, not this one.
#
# Convergence is a property of a tree, and the only tree worth gating is the
# one a commit names -- this checkout is dirty with other lanes by default, so
# a working-tree answer measures a state nobody will ever have. So archive the
# revision, run the ordinary `--converge` THERE (`git archive` and not
# `git worktree add`, following tools/seam-audit and tools/ci: it touches no
# repository state, cannot contend on .git locks, and cleaning up is an `rm`),
# and pass its exit code straight back. One implementation, two callers.
# ---------------------------------------------------------------------------
if [ "$converge" = 1 ] && [ "$check" = 1 ]; then
  revision="${GENREF_CONVERGE_REV:-HEAD}"
  scratch="$(mktemp -d "${TMPDIR:-/tmp}/genref-converge.XXXXXX")"
  trap 'rm -rf "$scratch"' EXIT
  echo "genref converge --check: measuring $revision, not this working tree."
  git -C "$REPO" archive "$revision" | tar -x -C "$scratch"
  set +e
  "$scratch/tools/genref/generate.sh" --converge
  code=$?
  set -e
  case "$code" in
  "$EXIT_CONVERGED_CLEAN")
    echo "genref converge --check: $revision is already the fixpoint."
    ;;
  "$EXIT_CONVERGED_WROTE")
    echo "genref converge --check: $revision is NOT the fixpoint -- the files above" >&2
    echo "  are stale at that commit. Run: tools/genref/generate.sh --converge" >&2
    ;;
  *)
    echo "genref converge --check: $revision does not converge (exit $code)." >&2
    ;;
  esac
  exit "$code"
fi

# ---------------------------------------------------------------------------
# The census inventory. A function of the Rust sources, which no pass writes,
# so it is built ONCE and reused across passes rather than rebuilt per pass.
# No --revision: the reference lives in-tree, so "at this commit" is implicit
# and the output stays byte-stable for the check gate.
# ---------------------------------------------------------------------------
INVENTORY="$(mktemp "${TMPDIR:-/tmp}/genref-inventory.XXXXXX")"
trap 'rm -f "$INVENTORY"' EXIT

(
  cd "$CENSUS_DIR"
  cargo run --release --quiet -- inventory \
    --root "$REPO" \
    --out "$INVENTORY" \
    --check-unique
)

if [ "$converge" != 1 ]; then
  exec node "$HERE/generate.mjs" --inventory "$INVENTORY" ${forward+"${forward[@]}"}
fi

# ---------------------------------------------------------------------------
# The convergence loop.
# ---------------------------------------------------------------------------

# Discovery has to agree with the declaration above, or the loop is running a
# list that stopped being the truth. A new emitter that reads the reference is
# a new edge in the cycle; it fails HERE, by name, instead of quietly leaving
# the reference one pass behind forever.
discovered="$(cd "$REPO" && grep -rl 'docs/reference' \
  apps/dclutch-web/scripts packages/dclutch-sdk/scripts 2>/dev/null |
  LC_ALL=C sort || true)"
declared="$(printf '%s\n' "${REFERENCE_COUPLED[@]}" | LC_ALL=C sort)"
if [ "$discovered" != "$declared" ]; then
  echo "genref converge: the reference-coupled emitter list is no longer the truth." >&2
  echo "  declared:" >&2
  printf '    %s\n' $declared >&2
  echo "  found reading docs/reference:" >&2
  printf '    %s\n' ${discovered:-"(none)"} >&2
  echo "  Update REFERENCE_COUPLED in $0 -- an emitter outside it leaves the" >&2
  echo "  reference permanently one pass behind, which is the exact defect" >&2
  echo "  --converge exists to remove." >&2
  exit $EXIT_NO_FIXPOINT
fi

# Every file a pass can move, hashed and path-ordered. Path-ordered so the
# diff between two listings NAMES what moved rather than only saying that
# something did.
converge_digest() {
  local out="$1" dir
  : > "$out"
  for dir in "${CONVERGE_TREES[@]}"; do
    [ -d "$REPO/$dir" ] || continue
    (cd "$REPO" && find "$dir" -type f -exec shasum -a 256 {} +) |
      LC_ALL=C sort -k2 >> "$out"
  done
}

moved_between() { # <before> <after> -- prints each path whose bytes differ
  # `diff` exits 1 for "they differ", which is this function's ORDINARY answer
  # and not a failure; under `set -o pipefail` an unguarded call aborted the
  # pass and exited 1, which reads exactly like an honest "converged, wrote".
  { diff "$1" "$2" 2>/dev/null || true; } |
    awk '/^[<>]/ {print $3}' | LC_ALL=C sort -u
}

before="$(mktemp "${TMPDIR:-/tmp}/genref-before.XXXXXX")"
after="$(mktemp "${TMPDIR:-/tmp}/genref-after.XXXXXX")"
trap 'rm -f "$INVENTORY" "$before" "$after"' EXIT

wrote_anything=0
converged_at=0
pass=1
while [ "$pass" -le "$MAX_PASSES" ]; do
  echo
  echo "=== genref converge: pass $pass of $MAX_PASSES ==="
  converge_digest "$before"

  node "$HERE/generate.mjs" --inventory "$INVENTORY"

  # The emitters, each one loud about having run. A generator that reports
  # nothing is indistinguishable from a generator that did nothing, and this
  # tree has paid for that confusion more than once -- so print the row, and
  # dump the captured output only when it is worth reading.
  for emitter in "${REFERENCE_COUPLED[@]}"; do
    emitter_dir="$REPO/$(dirname "$(dirname "$emitter")")"
    emitter_log="$(mktemp "${TMPDIR:-/tmp}/genref-emitter.XXXXXX")"
    if (cd "$emitter_dir" && node "$REPO/$emitter") > "$emitter_log" 2>&1; then
      echo "    ran $emitter"
    else
      echo "genref converge: $emitter FAILED" >&2
      cat "$emitter_log" >&2
      rm -f "$emitter_log"
      exit $EXIT_NO_FIXPOINT
    fi
    rm -f "$emitter_log"
  done

  converge_digest "$after"
  changed="$(moved_between "$before" "$after")"
  if [ -z "$changed" ]; then
    converged_at="$pass"
    echo "    pass $pass moved nothing."
    break
  fi
  wrote_anything=1
  echo "    pass $pass moved:"
  printf '      %s\n' $changed
  pass=$((pass + 1))
done

echo
if [ "$converged_at" = 0 ]; then
  echo "genref converge: NO FIXPOINT after $MAX_PASSES passes. The files above still" >&2
  echo "  move on the last pass, so the reference and its client mirrors do not" >&2
  echo "  agree with each other under repetition. That is a defect in the" >&2
  echo "  generators, not a tree to commit -- do not commit this output." >&2
  exit $EXIT_NO_FIXPOINT
fi

if [ "$wrote_anything" = 0 ]; then
  echo "genref converge: fixpoint on the first pass -- nothing moved."
  exit $EXIT_CONVERGED_CLEAN
fi

echo "genref converge: fixpoint proved by pass $converged_at (of $MAX_PASSES)."
echo "  Files moved; commit the reference and the client mirrors together."
exit $EXIT_CONVERGED_WROTE
