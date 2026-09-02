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

echo
echo "genref: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
