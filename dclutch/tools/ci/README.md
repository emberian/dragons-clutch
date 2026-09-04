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
| `census` | ~1s | python3, rustfmt | a generated file arriving with **no** re-emit guard, or losing one; the three two-sided wire vectors still carrying their reviewed digests; and a raw-compared emission that a direct `lane.sh fmt` would red — which `cargo fmt` cannot reach, so the `fmt` tier is not its author |
| `seam` | ~20s | `ast-grep` | six structural seam defect classes, new findings against a triaged baseline |
| `release` | ~5s | python3 | the four release-tooling **refusal** suites: build-freshness admission, the devnet activity and demo-pulse wrappers, the sponsored-market-open stager |
| `clippy` | 22s warm, minutes cold | cargo, clippy, python3 | the deny table at `Cargo.toml:119` and the command `README.md:183` publishes, which **no tier ran**: 105 workspace members judged per package against `tools/ci/clippy-debt.tsv`, and the packages `--keep-going` never *reached* counted separately |
| `web` | ~1 min | node | the web + SDK vitest suites |
| `emission` | **86s warm / 195s cold-target**, measured twice 2026-09-04 | `lake`, `rustfmt`, node | every generated file still byte-matches the emitter that printed it — 77 guards, and its first ever full run found two reds |
| `journey` | ~2 min | `cargo` | the journey campaign still **compiles** |
| `root-targets` | ~4 min | `cargo` | the root-workspace integration tests `--all-targets` compiles and nothing ran |
| `programs` | minutes | `cargo-build-sbf` | the programs build with no SBF stack-frame diagnostic, and the public Direct route holds its compute margin across 32 pinned seeds |
| `suites` | ~15 min | `cargo-build-sbf` | the other SBF program-test suites: custody, core, claims, dealer |
| `workspaces` | slow | `cargo` | **every** tracked Cargo workspace checks from an archived revision |

`cheap` = census + seam + release. `all` = census seam release clippy web emission
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

### `clippy`, and the table nobody ran

`[workspace.lints.clippy]` at `Cargo.toml:119` denies seven lints — `unwrap_used`,
`panic`, `indexing_slicing`, `float_arithmetic`, `cast_possible_truncation`,
`cast_sign_loss`, `checked_conversions` — and `README.md:183` publishes
`cargo clippy --workspace --all-targets -- -D warnings` as a workspace check.

**Nothing ran it.** This runner dispatched fifteen tiers and not one invoked
clippy; `workspaces` runs `cargo check`, which cannot see a lint. The only
`cargo clippy` anywhere in the tree was `tools/direct-translation-validator/check.sh`,
over a different workspace. `1bdf5572f` found a red that had survived a day and
named this as the debt it was evidence of.

**The package is the unit.** `--keep-going` checks everything whose dependencies
compiled and stops at a red library, so *one* red kernel hides every package
above it. The first full census reached **30 of 105** members and was blind to
69 — a fact a per-lint quarantine would not have shown. So `clippy-census.py`
reports three sets on every run: clean, red, and **never reached**. The third is
printed because "we did not look" and "we looked and it was fine" are different
answers, which is the same distinction this runner's exit codes exist for.

`tools/ci/clippy-debt.tsv` is the list of packages that are red today and must
**stay** red. A `debt` package that goes green fails the tier by name — the fix
landed and the row is stale coverage. A red package with no row fails it too.
That is the ratchet in both directions.

**Its own target directory.** `cargo clippy` sets `RUSTC_WORKSPACE_WRAPPER`,
which is part of every workspace member's fingerprint, so alternating clippy and
`cargo check` in one directory rebuilds all 105 members each way — a tax on every
other lane in a shared checkout. The tier uses `target/clippy`
(`DCLUTCH_CI_CLIPPY_TARGET` overrides it).

**The budget** is measured the way `root-targets` measures its own: a minimum of
three rounds, warm dependency cache, every `crates/` and `programs/` source
touched first so the whole workspace is genuinely re-checked. 22s, 25s, 31s → 22.
The assertion is the same loose `DCLUTCH_CI_TIME_SLACK` backstop, and it is
**skipped entirely** when the target directory did not exist before the run: a
cold check of the whole dependency graph is not what 22s measured, and firing
there would be the tier crying wolf on its own first run.

**What a green here does not say.** 68 of 105 members carry `[lints] workspace = true`;
the other 37 do not inherit the deny table at all, and the seven lints above are
allow-by-default restriction lints that `-D warnings` does not reach. The tier
prints that ratio on every run so a green is not read as more than it is.

### `root-targets`, and the ten minutes that were a cold cache

`tools/release/check-all-workspaces.py` builds the root workspace with
`--all-targets`, so all 124 of its `tests/*.rs` **compile** on every release
check. Until this tier, none of them **executed**. Every `cargo test` in this
repository pointed somewhere else — the program-test workspaces, the journey
workspace and `--bins` at that, the census workspace, or `cargo check` on
purpose. `ba96d8527` enumerated the class, named the 80 that need neither
`lake` nor a built ELF, and refused to wire them, because a per-target loop had
"exceeded ten minutes locally" and it would not put an unmeasured multi-minute
row into a tier three other lanes were pushing through. That refusal was the
right call on the information it had.

**The ten minutes was a cold target directory.** Each `cargo test -p X --test Y`
was paying for a build, not for a re-resolve. Measured warm at `0bac7f001`, one
execution each of all 80 is **69.5 s** — three full rounds, per-target minimum,
because this machine is shared and load only adds. Nothing came near the 120 s
timeout and the largest resident set was 766 MB, so neither time nor memory
excludes anything. The build the tier does first is ~2.5 min cold and is the
same build `--all-targets` was already paying.

`tools/ci/root-targets.tsv` is the single authority for what happens to each
target, and it carries each one's measured seconds. `never-run-tests.py
--check` — the census that *found* the class — is the tier's control: it fails
if a cheap target has no row, if a row names a target that no longer exists,
if a `run` row records more than the 8 s budget, or if a `slow` row is not
actually slow. It runs in milliseconds and times nothing.

**Why the budget is checked against a committed number and not a stopwatch.**
The same target measured 39.95 s, 9.80 s and 6.48 s in three consecutive rounds
while other lanes built. A wall-clock gate on that is a gate whose red is
usually somebody else's build, and this file already has a section about what
those cost. So the exact budget is the number in the tsv, which a lane adding a
target has to measure and write down; the tier's own wall-clock assertion is a
deliberately loose backstop (`DCLUTCH_CI_TIME_SLACK`, 4 by default, 1 on a
dedicated runner) and its failure text says which of the two it is.

**The quarantine is the load-bearing part.** Seven targets are red today, and a
tier that simply excluded them would be documenting debt rather than holding
it. A `quarantine` row must **stay** red: if one goes green the tier fails by
name, because the fix has landed and the row is now stale coverage. Deleting it
is one line, in the commit that fixed it. That is `COVERAGE.md`'s ratchet
applied to a failing test — the list only moves when somebody looks at it.

### The `DCLUTCH_WRITE_WIRE_VECTOR` escape hatch, and why a test could not close it

Three checked-in fixtures are written by a Rust encoder and re-derived
independently by a TypeScript one, so a wire that moves goes red on the
authoring side first. Each of the three Rust tests also accepts
`DCLUTCH_WRITE_WIRE_VECTOR=1`, which overwrites the fixture and returned
success. That is the correct way for a deliberate move to land — and it was
also **one environment variable that made a moved wire green on both sides at
once**: regenerate, and the encoder, the fixture and the browser mirror all
agree again about bytes nobody read. The web tier would go green too, because
it compares the mirror against the same regenerated file.

A test cannot close that, because the test is the thing being regenerated. So
there are two halves, in one commit:

- each write branch now **refuses after it writes** — writing is not passing;
- the reviewed digest is pinned in `tools/ci/wire-vector-pins.tsv`, which no
  test can write, and `wire-vector-pins.py` checks it in the `census` tier for
  the price of five `sha256`s.

`--update` re-pins and prints every digest it moved, so the numbers can go in
the commit message. The SDK copies are pinned too: the operator's write branch
writes both and reads back only the `apps/` one, so the SDK copy was a file
this repository generates and never checks.

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

Both of the gaps this section used to name are closed, and the paragraph is
rewritten rather than annotated because a runbook that teaches a wrong answer is
worse than one that says nothing. It claimed
`programs/dclutch-core-sbf/tests/` had five targets with three driven and
`capability_close_alias_program_test` / `retirement_replay_handoff_program_test`
"run by nothing"; that runner has globbed `tests/*.rs` since 2026-08-30 and
drives all seven.

The rows went from seven to fifteen on 2026-09-03, from a census of this file
against the tree: sixty real-ELF integration binaries exist and twenty-eight ran
in no tier at all, eleven of them behind a self-contained runner that nothing
invoked. Eight are wired now, each measured first, and `run.sh --list` prints
the rows **from `SUITE_RUNNERS`** instead of restating them — the table had said
seven while the dispatch ran fifteen, which is this file's signature defect
recurring inside its own runbook.

The three Token-2022 rows (`claims-lifecycle`, `claims-position`,
`claims-fractional`) are wired because their runners now exit **2** when the
pinned v11 artifact is absent rather than 1: nothing was proven, which is not
the same as a failing gate. A wrong DIGEST still exits 1. They are also no
longer Linux-only — `fixtures/prepare-token-2022-v11.sh` short-circuits its host
check for a prepared canonical ELF and still verifies digest and length, so
`TOKEN_2022_V11_ELF` beside `TOKEN_2022_V11_CRATE` runs all six Claims suites on
Darwin (measured 2026-09-03: 46 passed, 0 failed).

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

- **The `emission` tier itself. No job runs it.** The wrapper's
  `.github/workflows/rust.yml` has jobs for `programs`, `journey`, `suites`,
  `workspaces` and `cheap`, and the one named **"seam register and emission
  census"** runs `cheap` — which is the CENSUS. So the tier that takes an
  actual verdict on the 77 byte-identity guards has never run automatically,
  and until 2026-09-04 had never run at all: its first full run found two
  guards that had been red for days, in a tree `COVERAGE.md` was calling `100
  guarded, 0 unguarded` throughout. Neither was a stale emission — one was a
  guard comparing raw emitter stdout against a file its crate had since
  formatted (`ea4c46e02`), the other a pinned line count a correct re-emission
  moved past (`d0c0990fc`). The wiring is one job of the same shape as
  `journey`, and the number that was missing is now measured: **86s warm, 195s
  with a cold cargo target**,
  cheaper than `journey`+`root-targets` and a fifth of `suites`. It needs
  `lake`, `rustfmt` and node for the six web guards. The cheap half of what it
  would have caught is already wired: `census` now runs `--fixpoint`, which
  costs a second and names a raw-compared emission a direct `lane.sh fmt`
  would red, BEFORE anyone runs it.
- **The 33 `lake` and 11 ELF root-workspace targets.** `root-targets` runs the
  80 cheap ones and names these as excluded, with the reason, from the census
  rather than from a list of its own. The `lake` ones re-run a Lean emitter and
  belong beside the `emission` tier; the ELF ones need a built program and
  belong beside `suites`. Both are wiring work, not measurement work — the
  classification already exists.
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
