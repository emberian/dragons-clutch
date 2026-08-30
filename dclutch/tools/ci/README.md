# tools/ci — what runs automatically, when, and why

Until this directory existed, the answer to "which of this project's gates run
automatically?" was **none of them**, and the answer to "where would I find
out?" was a YAML file in a different repository.

This is the answer, in the tree the gates live in.

```sh
tools/ci/run.sh --list                    # the table, with costs
tools/ci/run.sh cheap                     # census + seam, ~20s
tools/ci/run.sh programs --commit HEAD    # SBF build + the compute margin gate
tools/ci/run.sh all --require             # the release answer
```

## The tiers

| tier | cost | needs | what it gates |
|---|---|---|---|
| `census` | milliseconds | python3 | a generated file arriving with **no** re-emit guard, or losing one |
| `seam` | ~20s | `ast-grep` | six structural seam defect classes, new findings against a triaged baseline |
| `web` | ~1 min | node | the web + SDK vitest suites |
| `emission` | minutes | `lake` | every generated file still byte-matches the emitter that printed it |
| `programs` | minutes | `cargo-build-sbf` | the programs build, and the public Direct route holds its compute margin across 32 pinned seeds |

`cheap` = census + seam. `all` = everything.

## Where each tier actually runs

| trigger | tiers | why there |
|---|---|---|
| wrapper `checks.yml`, every push | census, seam, web, SBOM | seconds to a minute, no toolchain beyond python and node |
| wrapper `rust.yml`, on `programs/**` `crates/**` Cargo files, plus daily | programs, census, seam | an SBF build is minutes; paying it on a README edit teaches everyone to ignore a slow red X |
| a human, before landing something near the hot path | `programs --commit HEAD` | the only gate that can see compute erosion |
| the cut | `all --require` | an unrun gate is not a passing gate |

Both workflows live in the **public wrapper** (`dragons-clutch`), because that
is the only repository in this project with a remote and therefore the only
place an automatic trigger exists at all. They call this script rather than
restating the tiering, so there is one definition and two callers.

## Exit codes, and why there are three

```
0  every requested tier RAN and PASSED
1  a gate FAILED — this tree has the defect that gate detects
2  a PREREQUISITE IS MISSING — nothing was proven, either way
64 you typed something wrong
```

**2 is the one that earns its keep.** These are `tools/seam-audit`'s codes,
adopted rather than invented so the tree has one convention. That tool used to
exit 1 when `ast-grep` was absent — the same code it uses for "this tree has a
seam defect" — because its "install it" message sat behind a `returncode` check
that an absent binary never reaches, since `subprocess.run` *raises* instead.

The same hole was open in `tools/emission-guard`: its census reads
`git ls-files` under `check=True`, so on a tree that is not a git checkout it
raised, Python exited 1, and a runner reading only the status reported **"the
census FAILED"** about a tree the census had never looked at. That is not
hypothetical — it is how it was found, in a `git archive` export, which is the
normal shape of a release candidate and of a vendored subtree.

So: a gate that cannot run has not passed and has not failed, and it says so.
`--require` turns 2 into 1 for the places where an unverified answer is not an
acceptable one.

Where a gate already reports its own 2, **this script reads that answer rather
than re-deriving it** — it does not check for `ast-grep` itself. A second
detector for a condition the gate already detects is a second author who can
disagree with the first, which is this project's named signature defect.

## Two things this script deliberately does not contain

**The compute margin number.** `programs` runs
`direct_hot_top_level_margin_gate.rs`; that file owns its constant. The number
is a **ratchet** — when a lane makes the route cheaper it *lowers* it — and a
second copy here would turn an ordinary act of progress into a two-file chore
that somebody eventually gets half-right.

**Any gate's logic.** Every gate was written by the lane that owns it and lives
in that lane's directory. This is a dispatcher.

## `--commit`, and why a compute number needs it

`cargo build-sbf` compiles **what is on disk**. This repository's working tree
is shared by a dozen concurrent lanes, so a default build routinely measures a
franken-tree of committed HEAD plus whatever three or four other people have
half-written — a revision nobody committed and nobody ever will.

This is not a rounding concern. Under Ledger M-61 a **one-byte** ELF difference
redraws every fixture seed by up to ±46,000 CU, which is more than twice the
public Direct route's entire margin. Measured: the same gate, on the same
machine, minutes apart, failed at `seed 20 … ProgramFailedToComplete` from the
working tree and at `seed 13 … ComputationalBudgetExceeded` from a clean
archive of HEAD. Two different seeds, two different failure modes, one of them
a claim about a tree that does not exist.

`--commit REV` exports that revision with `git archive` and builds there.
`git archive` and not `git worktree add`, following `tools/seam-audit`'s fix for
the same class: it touches no repository state, so it cannot contend on `.git`
locks with the other lanes, and cleanup is an `rm`.

With no `--commit` it builds the working tree — the right default for the author
of an edit, who wants to be told about the defect they just wrote. It says
loudly which tree it measured, and counts the uncommitted files if there are any.

## What is not wired, and what it would take

- **The other program-test suites** (claims-sbf, core-sbf, dealer campaign,
  successor bootstrap) are not in the `programs` tier. Each needs its own ELF
  set and fixture staging, and several have bespoke runners under
  `tools/gauntlet/`. Roughly an afternoon to fold in behind one flag, and the
  work is mechanical rather than uncertain — the runners already exist.
- **The gauntlet's validator campaigns** are not here and should not be: they
  need a real `solana-test-validator` and are tens of minutes. They belong to
  the cut, not to a push.
- **`tools/release/checked-release-candidate.sh` has no seam-audit call yet.**
  The one-line, already-tested wire-in is written down in
  `tools/seam-audit/README.md`; it is fenced behind the trading lane's
  ownership of `tools/release/` and is that lane's to land.
- **There is no pre-commit or pre-push hook running any of this**, and a
  pre-push one cannot work: this repository has no git remote, so there is no
  push event to hang a hook on. `tools/emission-guard/install-hooks.sh`
  installs a `pre-push`, which is why it has never fired here. A `pre-commit`
  hook is the shape that would work, and `tools/ci/run.sh census` is the only
  tier cheap enough for one — but a hook that blocks a commit for twelve
  concurrent lanes is a decision with a blast radius, not a tidy-up, so it is
  named here rather than switched on.
