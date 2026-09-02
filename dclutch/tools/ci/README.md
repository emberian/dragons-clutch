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
| `release` | ~5s | python3 | the four release-tooling **refusal** suites: build-freshness admission, the devnet activity and demo-pulse wrappers, the sponsored-market-open stager |
| `web` | ~1 min | node | the web + SDK vitest suites |
| `emission` | minutes | `lake` | every generated file still byte-matches the emitter that printed it |
| `journey` | ~2 min | `cargo` | the journey campaign still **compiles** |
| `programs` | minutes | `cargo-build-sbf` | the programs build with no SBF stack-frame diagnostic, and the public Direct route holds its compute margin across 32 pinned seeds |
| `suites` | ~15 min | `cargo-build-sbf` | the other SBF program-test suites: custody, core, claims, dealer |
| `workspaces` | slow | `cargo` | **every** tracked Cargo workspace checks from an archived revision |

`cheap` = census + seam + release. `all` = census seam release web emission
journey programs suites. `workspaces` is deliberately outside `all`: it gives
every workspace a fresh target directory, which is the cut's price to pay and
not a push's.

### Why `release` is a push tier despite three "devnet" names

None of its four suites reaches a chain. Each builds a scratch sandbox, writes
stub `solana`/`solana-keygen`/`spl-token`/`dclutch` executables onto `PATH`, and
points the tool under test at `https://example.invalid` so a real fetch fails
loudly instead of quietly succeeding. All four together are about five seconds
and need only bash, python3 and git.

What they gate is **refusals** — the cases where the release tooling has to say
no. Stale or forged build evidence must not be admitted; a market must not be
founded at a nonzero Direct fee rate (it could never trade) or against a founder
key nobody holds (collateral stranded forever). Both of those are irreversible,
and a refusal test that never runs is indistinguishable from a tool that has
quietly stopped refusing. Before this tier they had **no callers anywhere** —
the same defect `workspaces` was created for, four times over.

One honest limit: the stager suite re-runs its red controls against the last
revision *before* its guards existed, which needs real git history. On a shallow
clone, or in a vendored subtree whose history does not carry that path, it says
so itself and two of its thirteen cases do not run.

### Why `journey` is a compile check and not a campaign

On 2026-08-30 the journey campaign had not built on main for about two days.

It is built to break that way **on purpose**: the tier-1 producer's modules are
compiled into it verbatim by `#[path]`, so the journey cannot drift into a
stale copy of the founding. Its own `Cargo.toml` calls the resulting fragility
"the intended tripwire". That is a good design with exactly one requirement —
*something has to pull the tripwire* — and nothing did, so a deliberate alarm
rang into an empty room while six upstream modules moved out from under it.

A real campaign needs a `solana-test-validator` and is tens of minutes; that
belongs to the cut. `cargo check` catches the whole two-day class for the price
of a type-check. It does **not** tell you the campaign passes. Different claim,
and this tier only makes the first one.

### Why `suites` rows name runners and never ELF lists

Every suite needs a different set of built programs — core wants registry,
rent, custody and a series-consume caller; custody wants its own test caller;
claims wants a resolution-proof link and an audited Token-2022 fixture. Each of
those sets is already written down, correctly, by the lane that owns the suite,
inside the runner beside it.

Copying them here would be a value duplicated instead of read: it would agree
until somebody adds a program to their runner and not to this table. So the
tier runs the owner's script and reports what it said.

Two things those runners do **not** do, named as inherited debt rather than
papered over: they build the working tree (so `--commit` cannot reach them, and
the tier says so), and they do not carry the SBF stack-frame-overwrite refusal
that `programs` and the accelerator links have.

Known gaps in what the rows cover, found while wiring them:
`programs/dclutch-core-sbf/tests/` has five targets and its runner drives
three — `capability_close_alias_program_test` and
`retirement_replay_handoff_program_test` are run by **nothing**. The `claims`
row needs a populated cargo registry to build its audited Token-2022 v11
fixture, and a host without one gets a per-row absence rather than a failure.

If you interrupt this tier, check `/tmp`: each runner cleans up on a normal
exit but a killed one leaks 3-7 GB, and `/tmp/dclutch-*` filled a volume once.

## Where each tier actually runs

| trigger | tiers | why there |
|---|---|---|
| wrapper `checks.yml`, every push | census, seam, web, release, SBOM | seconds to a minute, no toolchain beyond python and node |
| wrapper `rust.yml`, on `programs/**` `crates/**` Cargo files, plus daily | programs, census, seam | an SBF build is minutes; paying it on a README edit teaches everyone to ignore a slow red X |
| a human, before landing something near the hot path | `programs --commit HEAD` | the only gate that can see compute erosion |
| the cut | `all --require` | an unrun gate is not a passing gate |

Both workflows live in the **public wrapper** (`dragons-clutch`), because that
is the only repository in this project with a remote and therefore the only
place an automatic trigger exists at all. They call this script rather than
restating the tiering, so there is one definition and two callers.

As of 2026-08-31 they run on that repository's `main`. Before that they existed
only on one side branch, had run exactly once, and both runs failed.

**A fix in this tree does not turn that CI green.** The workflows check the
vendored `dclutch/` SUBTREE, which is cut in waves and therefore lags on
purpose, so a red X over there is a statement about what is PUBLISHED and a
commit here does not answer it until the next cut lands. Two consequences worth
knowing before you go looking for a bug that is already fixed:

- A tier this tree has but the subtree does not shows up as **NOT RUN** with a
  loud warning, never as a pass. That is what the `release` job says today.
- A red whose fix is already committed here stays red until the cut. Check the
  subtree's revision before concluding the gate is wrong.

Two failures found on the first real runs are worth naming, because both were
the same shape and neither was the defect it reported. The SBOM gate failed
with `failures=50`, every one of them a package that could not be resolved
because a fresh runner has an empty cargo registry — it had classified 3 of 53
manifests and reported the other 50 as license findings. The `programs` tier
failed on `DCLUTCH_CUSTODY_LEG_CALLER_ELF is required`, an unset variable,
while printing its compute-margin advice. **A gate that fails for a reason
unrelated to what it gates is worse than no gate**, because the fix that makes
the red go away is always the wrong one. When a gate here goes red, confirm it
is red about its own subject before you touch a threshold.

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

## `--commit`, and why every compiling tier needs it

**The rule, in its corrected form:** on a shared working tree, *any* tier that
compiles is a tier that needs a revision.

I first wrote this section about compute only, and that framing was too narrow
and I fell into the gap it left. Running the `journey` tier against the working
tree, I watched a red, then watched it turn green an hour later, and concluded
the red had been a neighbour's half-written file. It had not. Measured with
`--commit` afterwards, the journey was genuinely broken at `d38b01b9` and at
`e41d0b20`; what had changed in between was that its owner wrote the fix into
the shared tree *without committing it*.

So the dirty tree had **hidden a real breakage**, not invented a fake one —
the mirror image of the failure I was guarding against, and the one I did not
think to guard against. Worse, I used a lesson I had just learned to dismiss a
true finding and told a colleague to stand down on a live bug.

Two things follow, and they are why `--commit` now reaches `journey` too:

- A red from a working-tree run is not reportable in either direction. It may
  be someone else's edit; it may also be concealing yours.
- Scoping a dirtiness check to the paths you *expect* to matter and then
  treating its silence as proof is the same mistake one level up. I checked
  `git status -- crates programs` and called the tree clean. The file that
  mattered was under `tools/`.

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

- **The successor bootstrap campaign.** No runner script exists, it needs a
  real `solana-test-validator`, and its founding is ~13 minutes with **no
  resume** — `--work` must be a fresh absolute path, which is why the board
  shows `run1..run8`. That is a cut-tier campaign, not a push-tier suite. Its
  *host* tests (`cargo test --manifest-path
  tools/local-validator/bootstrap/successor/Cargo.toml`, 443 of them) are
  ordinary and would drop into `suites` as one more row — call it an hour.
- **`tools/gauntlet/claims-extended` and `tools/gauntlet/dealer-checkpoint`**, which fold
  their campaigns into census evidence and *do* carry the frame-diagnostic
  refusal. Both refuse to start until `tools/gauntlet/run.sh --mode census` has
  run, so wiring them means wiring that ordering too. Half a day, and the payoff
  is real: the retired dealer family test was silently red from 2026-08-27 to
  `33a61576` because a release-path change touched seven programs and zero
  campaigns.
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
