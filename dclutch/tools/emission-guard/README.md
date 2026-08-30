# tools/emission-guard — what checks the generated files, and what does not

```sh
tools/emission-guard/emission_guard.py            # the census, to stdout
tools/emission-guard/emission_guard.py --write    # regenerate COVERAGE.md
tools/emission-guard/emission_guard.py --verify   # byte-gate it; exit 1 on drift
tools/emission-guard/emission_guard.py --affected BASE..HEAD   # what a range moves
tools/emission-guard/emission_guard.py --run BASE..HEAD        # run those guards
tools/emission-guard/emission_guard.py --run --all             # run every guard
sh tools/emission-guard/install-hooks.sh          # opt-in pre-push hook
```

## Why this exists

This repository generates source from Lean, and the pattern is a good one:
each generated file opens with a provenance header naming the emitter that
printed it, and a hand-written `check-generated.sh` re-runs that emitter and
`cmp`s the result against the committed bytes. Where such a script exists, the
guarantee is real and byte-exact.

The problem was never that the gate is weak. It is that the gate was
**partial and invisible**. Most generated files have no check script, nothing
enumerated which ones, and no CI ran any of them. An unguarded generated file
can be hand-edited, or quietly drift behind the Lean source it claims to come
from, and nothing in the repository notices — the header still says
`do not edit`, and nothing enforces it.

`docs/evidence/ASPIRATION_ARCHAEOLOGY_2026_08_30.md` names the same gap from
the other side ("~54 of 69 generated files have NO re-emit byte-check, and no
CI runs the 21 check scripts that exist"). This tool derives the number
mechanically instead of estimating it, so it disagrees slightly and is
reproducible: run it and see. The point of both numbers is the same.

## The two tiers, and why they are separate

They cost wildly different amounts, so conflating them would put a Lean build
in front of every cheap check.

**The census — milliseconds, no toolchain, no build.** Reads the first line of
every tracked file and the text of every check script. Answers: which
generated files exist, which emitter authored each, and which of them a check
script actually guards. It never runs Lean, so it is safe to put anywhere,
including a cheap CI tier.

`COVERAGE.md` is its committed output and `--verify` byte-gates it. **That is
the ratchet.** A new generated file with no check script changes the census
and reds `--verify` until someone decides, on purpose, whether to write a
guard or record the gap. The unguarded count stops being invisible and starts
being a number that can only move when somebody looks at it.

A green census does **not** mean the bytes match. It means we know which bytes
nobody is checking. Those are different claims and this tool keeps them
apart.

**The byte-identity checks — needs `lake`, `rustfmt`, and a Lean build.** The
existing check scripts, run for real. `--run` scopes them by a git range so a
pre-push hook pays only on pushes that could actually move an emission.

The scoping is conservative in one direction and precise in the other, on
purpose:

- **Any** edit under `formal/dclutch-semantics/` selects **every** guard. Lean
  modules share definitions, so a change to one file can move an emission
  whose own module was never touched. Guessing the import graph here would be
  a guess, and a guess that silently under-selects is worse than a slow hook.
- An edit to a generated file **alone** selects only that file's guard — and
  if the file has *no* guard, that is reported explicitly as `UNGUARDED`
  rather than passing silently. A hand-edited generated file is the exact
  failure this whole pattern exists to prevent, so it gets named.

## The hook

`install-hooks.sh` is opt-in and you run it yourself. It sets one
repository-local git setting (`core.hooksPath`) and touches nothing in your
home directory.

Know the catch before running it: a repository-local `core.hooksPath`
**overrides a global one**, so if you have global hooks (lefthook shims and
similar) they stop running in this checkout. The script detects that case and
asks first. Undo with `git config --unset core.hooksPath`.

The hook always runs the census (cheap) and runs the byte checks only for the
range being pushed. `SKIP_EMISSION_GUARD=1 git push` skips it, deliberately: a
hook that cannot be skipped is a hook that gets uninstalled, and then nothing
is guarded at all.

## What this does not do

- It does not write check scripts for the unguarded files. Each needs pinned
  literals chosen by someone who knows what a silent width change would look
  like in that ABI, which is judgment, not generation.
- It does not run in CI as a byte check. The Lean build is not free —
  though it is far cheaper than it looks, since `formal/dclutch-semantics`
  pulls no mathlib and no external dependencies at all. The census half is
  the part that belongs on a cheap tier.
