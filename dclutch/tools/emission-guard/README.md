# tools/emission-guard — what checks the generated files, and what does not

```sh
tools/emission-guard/emission_guard.py            # the census, to stdout
tools/emission-guard/emission_guard.py --write    # regenerate COVERAGE.md
tools/emission-guard/emission_guard.py --verify   # byte-gate it; exit 1 on drift
tools/emission-guard/emission_guard.py --affected BASE..HEAD   # what a range moves
tools/emission-guard/emission_guard.py --run BASE..HEAD        # run those guards
tools/emission-guard/emission_guard.py --run --all             # run every guard
tools/emission-guard/emission_guard.py --fixpoint              # formatting hazards, ~1s
tools/emission-guard/emission_guard.py --fixpoint --write-debt # rewrite that baseline
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

## The third tier: `--fixpoint`, and why a census can be green beside a red guard

The two tiers above answer "does a guard exist" and "does it pass". Nothing
answered the question in between — **can this guard survive an ordinary day in
this repository** — and that gap is what let `100 guarded, 0 unguarded` stand
beside a red guard for a week.

The arithmetic is short. `rustfmt.toml` exists precisely so that every
formatter invocation produces the same bytes, so "formatted" is one thing here
and not two. So:

* A guard that runs `rustfmt` over the emission before comparing holds
  `committed == rustfmt(emission)`. Formatting rewrites the committed file to
  `rustfmt(committed)`, and rustfmt is idempotent, so the equality survives.
  **Such a guard cannot be broken by formatting.**
* A guard that compares RAW emitter stdout holds `committed == emission`.
  Formatting leaves `rustfmt(emission)`, so the guard survives only if the
  emission was already a fixpoint — and reds the first time anyone formats the
  file otherwise.

### The vector is not `cargo fmt`, and that was measured

Every generated module in this tree is declared

```rust
#[rustfmt::skip]
#[path = "generated_x.rs"]
mod generated;
```

or pulled in with `include!`, and **both stop rustfmt's module walk**.
`cargo fmt --check -v` over the eight crates holding these files visits three
generated files and none of the eighteen listed in `fixpoint-debt.tsv` — which
is why that file and `tools/ci/fmt-baseline.txt` share not a single path, and
why the `fmt` tier is not already the author of this question.

What is not stopped is a **direct invocation**. `tools/lane.sh fmt <file>` is
`rustup run 1.97.1 rustfmt --edition 2024 -- <file>`; it refuses crate roots and
nothing else, and the `#[rustfmt::skip]` that would protect the file lives in a
*different* file, so it never enters the picture. The tree's own recommended
formatting command, pointed at a path a lane just touched, reformats a
`do not edit` file.

That is not a reconstruction. `generated_transition_programs_v3.rs` carried
`#[rustfmt::skip]` from its first commit, was twelve bytes per line for six
days, and arrived at `ea4c46e02` reflowed to sixteen — every array moved, not
one byte changed. `#[rustfmt::skip]` on the `mod` is protection against
`cargo fmt` and against nothing else, and four crates are currently relying on
it as though it were more.

`--fixpoint` finds that second pair: a generated `.rs` file no guard normalises
for, that rustfmt does not leave alone. It needs `rustfmt` and NOT `lake`, and
it costs about a second, so it runs on the cheap `census` tier rather than
beside the Lean builds. `fixpoint-debt.tsv` is its committed baseline and moves
in both directions — a new pair reds it, and so does a repaired pair still
listed — which is `COVERAGE.md`'s ratchet applied to formatting.

This is not a hypothetical class. `generated_transition_programs_v3.rs` was
exactly such a pair from `3affdadcb` and went red six days later at `ea4c46e02`
when its file was formatted; `request_profiles_generator_fresh` was the same
defect in the same crate three days earlier. Eighteen pairs were live when this
check was written. Each has two possible repairs, both the owning lane's:
normalise the guard (as forty-two of the sixty-five Rust guards already do), or
make the emitter print what rustfmt would print.

**The verdict tier had never been run.** Measured 2026-09-04: `--run --all` is
**86 seconds for all 77 guards** with a warm Lean build and a warm cargo
target, and **195 seconds** on the first run of the day, where the difference is
entirely `cargo` rebuilding the 32 test binaries. Its first full run found two
reds —
the formatting one above, and a pinned line count in
`dclutch-liability-basis-v2-kernel/check-generated.sh` that a correct
re-emission had moved past at `d0c0990fc`. Neither was a stale emission. No CI
job runs this tier; that is recorded in `tools/ci/README.md` under what is not
wired.

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

- `--fixpoint` does not repair a pair, and deliberately: which repair applies
  is the owning lane's judgment about its own ABI, and a tool that reformatted
  another lane's generated file would be hand-editing a `do not edit` file to
  silence its own gate.
- It does not write check scripts for the unguarded files. Each needs pinned
  literals chosen by someone who knows what a silent width change would look
  like in that ABI, which is judgment, not generation.
- It does not run in CI as a byte check. The Lean build is not free —
  though it is far cheaper than it looks, since `formal/dclutch-semantics`
  pulls no mathlib and no external dependencies at all. The census half is
  the part that belongs on a cheap tier.
