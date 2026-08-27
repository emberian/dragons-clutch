# tools/gauntlet/hot-cu — the Hot tail's compute, measured twenty times

```sh
tools/gauntlet/hot-cu/run-hot-cu.sh                  # working tree, seeds 0..19
tools/gauntlet/hot-cu/run-hot-cu.sh --commit HEAD    # a CLEAN revision — see below
tools/gauntlet/hot-cu/run-hot-cu.sh --seeds 40
tools/gauntlet/hot-cu/run-hot-cu.sh --substrate slot-pinned        # decision 0012's arm
tools/gauntlet/hot-cu/run-hot-cu.sh --elf-dir /somewhere/deploy   # skip the build
```

**If the number is going to be quoted at a revision, pass `--commit`.** This is
a shared checkout, and under M-61 a changed artifact byte is not a rounding
error — it redraws every seed.

The first run of this script found fifteen files dirty from other lanes, two of
them (`core-sbf/src/resolution.rs` and a `Cargo.lock` dependency addition)
inside the fixture's dependency closure. Building the same commit from a clean
archive then produced **byte-identical** artifacts, so on that occasion nothing
moved. That is the useful shape of the lesson: the dirty tree is not
automatically a wrong measurement, and it is not automatically a right one
either. **The digest settles it, and only the digest.** Sweep with `--commit`,
or build both ways and compare `shasum -a 256` before quoting.

## What it measures

One number: the compute a Hot continuation consumes at the **protocol default
32 KiB heap**, against the runtime's 1,400,000 per-transaction maximum. The
witness is `hot_heap_frame_is_inert`, in
`programs/dclutch-trading-sbf/program-test/tests/`, run once per value of
`DCLUTCH_FIXTURE_SEED`.

There is no headroom to buy here. 1,400,000 is the runtime's ceiling, so the
transaction already requests the maximum, and exhaustion is a hard refusal with
no partial result. That is why this tier exists and why the statistic it reports
is a **pass count** and not a margin.

## The reporting rule — ledger M-61

**Report `PASS n/20` and `MEAN`. Never a worst margin. Never one seed's number
as a bound.**

The Hot path derives program addresses whose seeds include the artifact release
identity, and that identity is `hash(elf)` — see `hash(elf)` in
`programs/dclutch-trading-sbf/program-test/direct-hot/src/waist.rs`.
`try_find_program_address` costs 1,500 CU per rejected bump and walks up to 31
of them, so every per-seed total carries `n × 1,500` of pure draw: a swing of
±46,000 CU.

The consequence is the one lanes keep getting wrong:

> **Changing one byte of the trading ELF redraws every seed's bump search.**

DIAG-82 measured exactly that across a pure out-of-line refactor whose real cost
was one extra call. Every per-seed delta decomposed as `n × 1,500 + ~50` — the
`~50` was the call, the rest was the lottery. "Worst margin 8,238" was never a
property of the code: the same tip with a 440-byte-larger ELF measured 3,689, on
a seed that had not been the worst before, at 20/20 either way.

So the script prints, itself, at the end of every run:

```
PASS 20/20
MEAN 1,3xx,xxx CU   (over the 20 seeds that completed, of 1,400,000)
MIN  …
MAX  …
SPREAD …  ~ n bump-search iterations at 1,500 CU each
ELF  <sha256>  dclutch_trading_sbf.so
SRC  <revision> (clean git archive)
SUB  <substrate>  (DCLUTCH_FIXTURE_SUBSTRATE)
HARN <revision> (clean|DIRTY) programs/dclutch-trading-sbf/program-test
```

`HARN` is a second cleanliness fact and not a duplicate of `SRC`. `--commit`
archives the **build**; the harness — the fixture keys, the staged ProgramData,
the substrate arms — is compiled out of `--repo` on every run, committed or not.
A clean `SRC` beside a `DIRTY` harness is the ordinary state of a lane measuring
its own uncommitted fixture, and it is not automatically wrong; it is a fact the
number has to be quoted with, which it could not be while only the build's
cleanliness was reported.

`SRC` describes **where the artifacts came from**, not where the checkout is
standing. Under `--elf-dir` it says so and names no revision, because printing
`--repo`'s HEAD beside a figure drawn from somebody else's ELF is the mispairing
M-61 exists to stop — and it is not hypothetical: during this tier's own first
day, another lane committed between a build and its sweep.

`MIN` and `MAX` are printed as the observed **spread**, which is what they are.
`MIN` is not a margin. The `ELF` line is not decoration: a CU figure quoted
without the digest it was drawn against does not mean anything, and M-61 asks
for the digest beside any margin precisely because the pairing is what makes the
number checkable.

`MEAN`, `MIN` and `MAX` are over the seeds that **completed**. A seed that
exhausts the meter has no figure to average — the pass count is what carries it.
That split is the other half of why the rule asks for two numbers.

Pinning `waist::fixture_keypair` to the seed makes a single figure
**reproducible**. It does not make it **meaningful**: on a real chain the makers
are whoever they are, so the spread is a property of the protocol, not of the
fixture.

### Comparing two revisions

Sweep both, compare `PASS` and `MEAN`. Do not diff per-seed figures across
revisions — different ELFs are different lotteries, and a per-seed diff will
hand you ±46,000 CU of noise with a commit's name attached to it. M-46 tells the
next CU lane to bisect against this sweep; the bisect statistic has to be the
pass count and the mean.

## What this tier is not

It is **not a census campaign**. It submits nothing to a validator, binds no
route, carries no `bindings.json` or `witnesses.json`, and folds nothing into
`out/ledger.json`. Nothing it prints is evidence that a route executed or that a
refusal refused; `run.sh --mode census` will not show a row because of it. It is
an instrument, and `DESIGN.md`'s admissibility rules are about evidence.

Two consequences of that, both deliberate:

- **It builds from the working tree by default**, where the campaign tiers
  always archive a revision first. A CU lane's question is normally about an
  edit that is not committed yet, so measuring the uncommitted tree is the
  useful default — but it is a default, not the only mode. The run prints the
  revision and a `DIRTY` flag, `--commit` archives a clean revision instead, and
  the ELF digest is what the numbers are really keyed to either way.

  Under `--commit` the **build** is archived; the harness still runs from
  `--repo`, because the host side consumes no compute. What the figure depends
  on is the ELFs plus the fixture keys, and the fixture derivation lives in
  `program-test/direct-hot/src/waist.rs` — so when quoting, check that the
  harness paths are clean, which the `DIRTY` flag lets a reader do.
- **It warns on SBF frame diagnostics rather than refusing.** Every campaign
  tier refuses a nonzero count, because an artifact the toolchain calls
  potentially-undefined has no business producing evidence. DIAG-82 was an
  82-diagnostic regression whose CU cost is exactly what someone would come here
  to measure, and an instrument that refuses to measure the regression is
  useless on the day it matters.

## Decision 0012's fast path, and why the default sweep says nothing about it

Decision 0012 (`docs/decisions/0012-devnet-iteration-substrate.md`, implemented
in `0e34c036`) claims a large CU saving: re-hashing a megabyte ELF on every
action "was ~700k CU on Trading alone". **A default run of this sweep cannot
measure that claim, and a lane must not cite it as though it had.** `--substrate`
is what makes it measurable; the rest of this section is why the option had to
exist, and the section after it is how to read what it produces.

The reason is in the fixture. `waist::release` constructs every artifact release
with `ArtifactUpgradePolicyV1::Immutable` and no bound authority, and the
ProgramData it stages comes from `waist::immutable_programdata`, which writes
the authority option as `0` — `None`. `slot_pinned_release_elf_digest_v1`
branches on exactly that:

- `Immutable` → `immutable_release_elf_digest_v1`, which decision 0012's own doc
  comment describes as "delegated **unchanged**" — it returns the bound digest
  and never hashed anything, before or after;
- `ExactAuthority` → the new slot-pin arm, which is the whole of what 0012 added.

So the Hot tail measured here took the `Immutable` arm before `0e34c036` and
takes it after. It never paid the hash, so it had nothing to save.

This is a real, checkable negative rather than an absence of evidence, because
the lottery has a known width and a ~700k swing is more than an order of
magnitude outside it. Two sweeps straddling `0e34c036`, both on this fixture:

| | trading ELF | seeds with a figure | mean |
|---|---|---|---|
| W2p, before | `12b9ec5687aa9b9c` | 18 | 1,366,177 CU |
| this tier, after | `7facb8e58e45843f` | 20 | 1,345,302 CU |

The means differ by **20,875 CU** — about fourteen bump-search iterations, well
inside the ±46,000 draw, and in the *cheaper* direction only by luck of the
redraw. That is lottery scale, not fast-path scale. If the Hot tail had been
paying a megabyte hash before `0e34c036`, this table could not look like this.

**To measure 0012 end to end, the fixture needs an `ExactAuthority` variant** —
a release with a bound authority and a ProgramData observation carrying the
pinned slot — swept the same way, against the same ELF. That variant now exists:
`waist::FixtureSubstrateV1`, selected by `--substrate`.

## The three substrate arms, and the only number that is a signal

```sh
tools/gauntlet/hot-cu/run-hot-cu.sh --commit <rev> --substrate immutable
tools/gauntlet/hot-cu/run-hot-cu.sh --commit <rev> --substrate immutable-pinned
tools/gauntlet/hot-cu/run-hot-cu.sh --commit <rev> --substrate slot-pinned
```

Pass the **same `--commit`** to all three. The second and third runs reuse the
first one's completed build — a stamp under the ELF directory records the
revision — so the three arms are drawn against one ELF *byte for byte* rather
than against three builds that ought to agree. That is a requirement, not a
speedup: under M-61 a one-byte difference between two builds would redraw every
seed by more than any substrate effect could.

| arm | policy | bound authority | bound slot | digest arm taken |
|---|---|---|---|---|
| `immutable` | `Immutable` | none | 0 | `immutable_release_elf_digest_v1` |
| `immutable-pinned` | `Immutable` | none | 167 | `immutable_release_elf_digest_v1` |
| `slot-pinned` | `ExactAuthority` | `[0x9a; 32]` | 167 | the 0012 slot-pin arm |
| `slot-pinned-superseded` | `ExactAuthority` | `[0x9a; 32]` | 167, observed 531 | refuses |

**`slot-pinned` minus `immutable` is not 0012's cost.** The policy byte, the
bound authority and the bound slot all live inside `ArtifactReleaseV1::to_bytes`,
so changing the arm moves the artifact id, the release-set identity, and every
PDA seeded by it — and the Registry derives its activation cache and its Hot
admission address with `find_program_address` **on chain**. Switching arms
therefore redraws the same lottery M-61 is about, before any code path differs.

`immutable-pinned` is the control that separates them. It keeps the `Immutable`
policy and the absent authority, so it takes the **same** digest arm as the
default and executes the same code; it binds the same nonzero slot, so it has a
**different** release identity. Its distance from `immutable` is therefore pure
redraw, measured rather than assumed:

```
    immutable-pinned − immutable  =  REDRAW ALONE
    slot-pinned      − immutable  =  REDRAW + whatever 0012 costs or saves
    ────────────────────────────────────────────────────────────────────
    the difference of those two   =  the signal, and the only real number
```

A signal smaller than the redraw is a legitimate result and must be reported as
one. It says the sweep bounds 0012's effect below the lottery's width on this
path — which, given the claim under test is ~700,000 CU, is itself an answer.

The fourth arm does not sweep. `slot-pinned-superseded` moves the Trading
ProgramData to a later slot and is exercised by
`tests/slot_pin_supersession.rs`, which requires the refusal by its declared
discriminant rather than a CU figure.

### What a pinned arm needs from the runtime

Two facts, both of which produce failures that say nothing about the pin if you
miss them, and both handled by `waist::start_with_substrate`:

- a Loader V3 program is visible from `deployment_slot + 1`, and the program
  cache admits an entry only when its deployment slot is an **ancestor** of the
  executing slot. A pin at 167 on a bank at slot 1 fails inside
  `ProgramCache::assign_program` with *"Unexpected replacement of an entry"*;
- `warp_to_slot(T)` roots `T − 1`, so only one nonzero deployment generation is
  visible at a time. The superseded arm therefore redeploys the whole set and
  leaves the staleness in Trading's **release**, not in a second live generation.

The maker replays the Direct campaign signs are valid for `clock_slot ± 1`, so
the bank slot and the fixture clock are read from one place and must not be set
independently.

## Outputs

Everything lands under `--work` (default `/private/tmp/dclutch-hot-cu`), never
under the shared `target/`: parallel lanes share this working tree.

| path | what |
|---|---|
| `elf/*.so` | the eight artifacts the fixture installs (working-tree build) |
| `elf-<rev>/*.so` | the same, from a `--commit` archive |
| `elf-<rev>/.hot-cu-built` | the completed-build stamp a later `--commit` run reuses |
| `logs/build-*.log` | per-program build logs, where the frame-diagnostic count is read |
| `sweep/<substrate>/seed<N>.log` | the full `--nocapture` log for one seed |
| `sweep/<substrate>/observed-cu.txt` | one CU figure per completed seed |
| `summary-<substrate>.json` | `dclutch-hot-cu-sweep-v1`: pass/fail, mean/min/max, the ELF digest, the substrate |

Logs and summaries are keyed by substrate because comparing arms means holding
three sweeps at once, and a shared directory would blend them — the `rm -f` that
keeps a re-run from mixing shapes would otherwise delete the control.

The script exits nonzero if any seed failed.

## `hot-tail-table.py`

The sibling renderer, for the *other* Hot measurement: a per-phase spend + heap
table from a `hot_tail_profile` log, which lifts the heap diagnostically so the
phases separate. Its docstring carries the invocation.

It lives here rather than beside `tools/sbf-footprint.py` and
`tools/sbf-frame-sizes.py` because those two read any ELF, while this one parses
one test's `dclutch-hot-cu:` / `dclutch-hot-heap:` log marks and has nothing to
say about any other input. It follows their one real convention: standard
library only, so a measurement does not depend on what happens to be installed.

## Provenance

This is W2p's measurement, unchanged in what it runs. It lived in
`/private/tmp/w2q/sweep.sh` while board entries quoted pass counts from it, one
`rm -rf /tmp` from gone. Its companion `build-gate.sh` is the build stage above;
`table.py` is `hot-tail-table.py`.

One defect was fixed on the way in: the extraction was
`grep -oE '…: [0-9]+ CU' | tail -1 | grep -oE '[0-9]+'`, and because the line
reads `… fixture seed 7: 1376260 CU …` the bare digit grep returned the **seed**
as well as the figure, so the CU column printed two lines per seed. The
replacement uses one `sed` capture group.
