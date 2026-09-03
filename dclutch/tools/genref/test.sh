#!/usr/bin/env bash
# tools/genref/test.sh -- self-test for tools/genref/generate.sh's dirty-tree
# gate and its argument forwarding.
#
# Runs against a SCRATCH git repository under /tmp with stubbed `cargo` and
# `node` on PATH, so it never regenerates docs/reference/, never builds the
# census, and never reads the real working tree. What it checks is the one
# thing that was wrong: `--allow-dirty` and `GENREF_ALLOW_DIRTY=1` are the same
# escape, and neither reaches `generate.mjs`, which does not accept it.
set -euo pipefail

GENREF_SH="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/generate.sh"
[ -x "$GENREF_SH" ] || { echo "test.sh: no executable generate.sh" >&2; exit 1; }

PASS=0
FAIL=0
ok()  { PASS=$((PASS + 1)); echo "  ok: $1"; }
bad() { FAIL=$((FAIL + 1)); echo "FAIL: $1" >&2; }

SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/genref-test.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT

# A scratch repo laid out like the real one, so `$(dirname "$0")/../..` is the
# scratch root and the gate reads the scratch tree's git status.
mkdir -p "$SCRATCH/repo/tools/genref" "$SCRATCH/repo/tools/gauntlet/census" "$SCRATCH/bin"
cp "$GENREF_SH" "$SCRATCH/repo/tools/genref/generate.sh"
: > "$SCRATCH/repo/tools/genref/generate.mjs"
: > "$SCRATCH/repo/tools/gauntlet/census/Cargo.toml"
git -C "$SCRATCH/repo" init -q
git -C "$SCRATCH/repo" -c user.email=t@t -c user.name=t add -A
git -C "$SCRATCH/repo" -c user.email=t@t -c user.name=t -c commit.gpgsign=false commit -qm base

# `cargo` writes nothing; the inventory file already exists from mktemp.
printf '#!/bin/sh\nexit 0\n' > "$SCRATCH/bin/cargo"
# `node` records the exact argv it was handed, which is the assertion surface.
printf '#!/bin/sh\nprintf "%%s\\n" "$@" > "%s/node-argv"\nexit 0\n' "$SCRATCH" \
  > "$SCRATCH/bin/node"
chmod +x "$SCRATCH/bin/cargo" "$SCRATCH/bin/node"

run() { # run <env-assignments...> -- <args...>; prints exit code, records argv
  rm -f "$SCRATCH/node-argv"
  local envs=() ; while [ "$1" != "--" ]; do envs+=("$1"); shift; done; shift
  set +e
  ( cd "$SCRATCH/repo" && env PATH="$SCRATCH/bin:$PATH" "${envs[@]}" \
      ./tools/genref/generate.sh "$@" >"$SCRATCH/out" 2>&1 )
  local code=$?
  set -e
  echo "$code"
}

# The argv node was handed, with the two paths that are not stable across runs
# -- the script and the mktemp'd inventory -- normalised away.
forwarded() {
  [ -f "$SCRATCH/node-argv" ] || { echo "<node never ran>"; return; }
  awk 'NR==1{print "generate.mjs";next} NR==3{print "<inventory>";next} {print}' \
    "$SCRATCH/node-argv" | tr '\n' ' '
}

echo "genref gate:"

# 1. Clean tree runs, and nothing extra reaches the generator.
[ "$(run -- --check)" = 0 ] && ok "a clean tree runs" || bad "a clean tree runs"
clean_argv="$(forwarded)"
[ "$clean_argv" = "generate.mjs --inventory <inventory> --check " ] \
  && ok "the generator is handed only --inventory and the caller's own arguments" \
  || bad "clean argv is '$clean_argv'"

# 2. A dirty tree refuses, by name and with the documented exit code.
echo dirt > "$SCRATCH/repo/dirt"
[ "$(run -- --check)" = 3 ] && ok "a dirty tree refuses with exit 3" \
  || bad "a dirty tree refuses with exit 3"
grep -q -- '--allow-dirty' "$SCRATCH/out" \
  && ok "the refusal names the escape it offers" || bad "the refusal names the escape"

# 3. THE DEFECT. The flag its own refusal advertises has to work.
[ "$(run -- --check --allow-dirty)" = 0 ] \
  && ok "--allow-dirty admits a dirty tree" || bad "--allow-dirty admits a dirty tree: $(cat "$SCRATCH/out")"
flag_argv="$(forwarded)"
[ "$flag_argv" = "$clean_argv" ] \
  && ok "--allow-dirty is CONSUMED here and never forwarded to generate.mjs" \
  || bad "--allow-dirty leaked to the generator: '$flag_argv'"

# 4. And the environment spelling is the same escape, not a second one.
[ "$(run GENREF_ALLOW_DIRTY=1 -- --check)" = 0 ] \
  && ok "GENREF_ALLOW_DIRTY=1 admits a dirty tree" || bad "GENREF_ALLOW_DIRTY=1 admits a dirty tree"
env_argv="$(forwarded)"
[ "$env_argv" = "$flag_argv" ] \
  && ok "both spellings hand the generator identical arguments" \
  || bad "the two spellings differ: '$env_argv' vs '$flag_argv'"

# 5. Ordinary arguments still pass through, including ones this script has
#    never heard of -- the gate filters exactly one token and nothing else.
run -- --check --allow-dirty --some-future-flag x >/dev/null
pass_argv="$(forwarded)"
[ "$pass_argv" = "generate.mjs --inventory <inventory> --check --some-future-flag x " ] \
  && ok "every other argument passes through in order" || bad "passthrough is '$pass_argv'"


# ---------------------------------------------------------------------------
# --converge: the two-pass rule, made mechanical.
#
# Stubbed the same way and for the same reason: what is under test is the LOOP
# -- that it runs until nothing moves, that it stops at three, and that a third
# pass which still moves a file is REFUSED rather than accepted. Proving that
# against the real generators would cost minutes and would only ever exercise
# the converging case, so the case that matters most (no fixpoint) would never
# run at all. The real tree's fixpoint is proved by `tools/ci/run.sh genref`,
# which runs `--converge --check` against a committed revision.
#
# The stub `node` here is a small state machine: it writes a file under
# docs/reference on its first N invocations of generate.mjs and then stops, so
# N=2 converges on the third pass (two productive passes and one that proves
# it) and N=99 never converges. It derives the repository root from the path it
# is handed, so it works unchanged inside the archive `--converge --check`
# makes.
# ---------------------------------------------------------------------------

CONV="$(mktemp -d "${TMPDIR:-/tmp}/genref-converge-test.XXXXXX")"
trap 'rm -rf "$SCRATCH" "$CONV"' EXIT

mkdir -p "$CONV/repo/tools/genref" "$CONV/repo/tools/gauntlet/census" \
  "$CONV/repo/docs/reference" "$CONV/repo/apps/dclutch-web/scripts" \
  "$CONV/repo/apps/dclutch-web/lib/generated" \
  "$CONV/repo/packages/dclutch-sdk/scripts" \
  "$CONV/repo/packages/dclutch-sdk/lib/generated" "$CONV/bin"
cp "$GENREF_SH" "$CONV/repo/tools/genref/generate.sh"
: > "$CONV/repo/tools/genref/generate.mjs"
: > "$CONV/repo/tools/gauntlet/census/Cargo.toml"
# The five reference-coupled emitters, by the names generate.sh declares. Each
# must contain the string discovery greps for, or the list guard fires.
for e in apps/dclutch-web/scripts/generate-refusal-registry.mjs \
  apps/dclutch-web/scripts/generate-route-census.mjs \
  packages/dclutch-sdk/scripts/generate-refusal-registry.mjs \
  packages/dclutch-sdk/scripts/generate-route-census.mjs \
  packages/dclutch-sdk/scripts/generate-market-phase-admission.mjs; do
  echo "// reads docs/reference" > "$CONV/repo/$e"
done
echo 0 > "$CONV/repo/COUNT"
echo 0 > "$CONV/repo/docs/reference/a.md"

printf '#!/bin/sh\nexit 0\n' > "$CONV/bin/cargo"
cat > "$CONV/bin/node" <<'NODE_EOF'
#!/bin/sh
# Only generate.mjs moves anything; the emitters are inert here.
case "$1" in
*/tools/genref/generate.mjs)
  root=$(dirname "$(dirname "$(dirname "$1")")")
  n=$(cat "$root/COUNT")
  n=$((n + 1))
  echo "$n" > "$root/COUNT"
  [ "$n" -le "${GENREF_TEST_MOVES:-2}" ] && echo "$n" > "$root/docs/reference/a.md"
  ;;
esac
exit 0
NODE_EOF
chmod +x "$CONV/bin/cargo" "$CONV/bin/node"

git -C "$CONV/repo" init -q
git -C "$CONV/repo" -c user.email=t@t -c user.name=t add -A
git -C "$CONV/repo" -c user.email=t@t -c user.name=t -c commit.gpgsign=false \
  commit -qm base

crun() { # crun <env...> -- <args...>; prints the exit code, output in $CONV/out
  local envs=()
  while [ "$1" != "--" ]; do envs+=("$1"); shift; done
  shift
  set +e
  (cd "$CONV/repo" && env PATH="$CONV/bin:$PATH" "${envs[@]}" \
    ./tools/genref/generate.sh "$@" >"$CONV/out" 2>&1)
  local code=$?
  set -e
  echo "$code"
}

reset_conv() { # put the tree back where the committed state is
  echo "${1:-0}" > "$CONV/repo/COUNT"
  echo "${1:-0}" > "$CONV/repo/docs/reference/a.md"
}

echo
echo "genref converge:"

# 6. Two productive passes and a third that proves the fixpoint. Exit 1 means
#    "converged, and it had to write" -- the same vocabulary --check uses for
#    "this tree was not already correct".
reset_conv 0
[ "$(crun GENREF_TEST_MOVES=2 -- --converge --allow-dirty)" = 1 ] \
  && ok "--converge reaches a fixpoint and reports that it wrote" \
  || bad "--converge fixpoint: exit $(cat "$CONV/out")"
grep -q 'fixpoint proved by pass 3' "$CONV/out" \
  && ok "it says WHICH pass proved the fixpoint" \
  || bad "no pass number in: $(cat "$CONV/out")"
grep -q 'docs/reference/a.md' "$CONV/out" \
  && ok "it names the files each pass moved" || bad "no moved file named"

# 7. THE REFUSAL. A generator pair that never settles must not be committed,
#    and "close enough after three passes" is exactly the answer this flag
#    exists to refuse.
reset_conv 0
[ "$(crun GENREF_TEST_MOVES=99 -- --converge --allow-dirty)" = 4 ] \
  && ok "no fixpoint in three passes REFUSES with exit 4" \
  || bad "non-convergence was not refused: $(cat "$CONV/out")"
grep -q 'NO FIXPOINT after 3 passes' "$CONV/out" \
  && ok "the refusal says what it could not prove" || bad "refusal text: $(cat "$CONV/out")"
grep -q 'do not commit this output' "$CONV/out" \
  && ok "and tells the reader not to commit the half-converged tree" \
  || bad "refusal does not warn against committing"

# 8. Already at the fixpoint: nothing moves, and that is exit 0, not 1.
reset_conv 5
[ "$(crun GENREF_TEST_MOVES=2 -- --converge --allow-dirty)" = 0 ] \
  && ok "a tree already at the fixpoint exits 0 having written nothing" \
  || bad "clean converge: $(cat "$CONV/out")"

# 9. The emitter list is checked against discovery, so a new reference-reading
#    emitter cannot silently leave the reference one pass behind.
echo "// also reads docs/reference" \
  > "$CONV/repo/apps/dclutch-web/scripts/generate-newcomer.mjs"
reset_conv 0
[ "$(crun GENREF_TEST_MOVES=2 -- --converge --allow-dirty)" = 4 ] \
  && ok "an undeclared reference-reading emitter is refused" \
  || bad "undeclared emitter was not refused: $(cat "$CONV/out")"
grep -q 'generate-newcomer.mjs' "$CONV/out" \
  && ok "and the refusal names it" || bad "refusal does not name the newcomer"
rm -f "$CONV/repo/apps/dclutch-web/scripts/generate-newcomer.mjs"

# 10. --converge --check measures the COMMITTED revision, never this tree. The
#     working tree is put at the fixpoint and the commit is left stale, so a
#     working-tree answer would be 0 and only the committed answer is 1.
reset_conv 5
[ "$(crun GENREF_TEST_MOVES=2 -- --converge --check)" = 1 ] \
  && ok "--converge --check reports a stale COMMIT past a converged worktree" \
  || bad "--converge --check on a stale commit: $(cat "$CONV/out")"
grep -q 'not this working tree' "$CONV/out" \
  && ok "and says which tree it measured" || bad "no tree named: $(cat "$CONV/out")"

# 11. The same check, green, once the fixpoint is what the commit holds.
reset_conv 5
git -C "$CONV/repo" -c user.email=t@t -c user.name=t add -A
git -C "$CONV/repo" -c user.email=t@t -c user.name=t -c commit.gpgsign=false \
  commit -qm converged
[ "$(crun GENREF_TEST_MOVES=2 -- --converge --check)" = 0 ] \
  && ok "--converge --check passes when the commit IS the fixpoint" \
  || bad "--converge --check on a converged commit: $(cat "$CONV/out")"

echo
echo "genref: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
