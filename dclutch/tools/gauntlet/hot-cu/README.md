# tools/gauntlet/hot-cu — the Hot tail's compute, measured twenty times

```sh
tools/gauntlet/hot-cu/run-hot-cu.sh                  # working tree, seeds 0..19
tools/gauntlet/hot-cu/run-hot-cu.sh --commit HEAD    # a CLEAN revision — see below
tools/gauntlet/hot-cu/run-hot-cu.sh --seeds 40
tools/gauntlet/hot-cu/run-hot-cu.sh --substrate slot-pinned        # decision 0012's arm
tools/gauntlet/hot-cu/run-hot-cu.sh --elf-dir /somewhere/deploy   # skip the build
tools/gauntlet/hot-cu/run-hot-cu.sh --elf-dir /somewhere/deploy \
  --trading-elf /somewhere/final-dclutch_trading_sbf.so           # replace Direct only
```

**If the number is going to be quoted at a revision, pass `--commit`.** This is
a shared checkout, and under M-61 a changed artifact byte is not a rounding
error — it redraws every seed.

`--trading-elf` is the exact final-Direct handoff. It copies the supplied
regular, non-symlink file into a work-local overlay under the canonical
`dclutch_trading_sbf.so` name and leaves the seven support ELFs unchanged. The
runner prints and records the override digest separately. This does not claim
the file belongs to `--repo` or `--commit`; the digest is its provenance. Use
the all-link checked-release/frame gate on that same ELF before treating its
M-61 sweep as release evidence.

Both SBF builds and the ProgramTest sweep run `--locked --offline`. A stale
root or nested lock is a refusal, not an implicit dependency-graph update.
The bounded option/refusal checks are:

```sh
bash tools/gauntlet/hot-cu/test-run-hot-cu-cli.sh
```

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
MEAN 1,3xx,xxx CU   (over all 20 requested seeds, of 1,400,000)
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

`MEAN`, `MIN` and `MAX` exist only when **every requested seed completed**. A
failed seed has no figure, and averaging only the survivors changes the sample:
`PASS 19/20` therefore prints no mean and writes JSON `null` for all three
statistics. The pass count carries a partial run; its individual completed
figures remain in the logs for diagnosis. This prevents a 19-seed survivor
average from being quoted as the required 20-seed mean.

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

(That reasoning was right, and the arms below now measure the same conclusion
directly rather than bounding it: the `Immutable` arm and the `ExactAuthority`
arm are 73 CU apart.)

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
| `slot-pinned-superseded` | `ExactAuthority` | `[0x9a; 32]` | 531, except Trading's release at 167 | refuses, by name |

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
    the difference of those two   =  an UPPER BOUND on the effect
```

A bound smaller than the redraw is a legitimate result and must be reported as
one: it says the sweep puts 0012's effect below the lottery's width on this path,
which against a ~700,000 CU claim is already an answer. But do not stop there —
these arms share their randomness, and shared randomness cancels.

### Do not stop at the difference of means — pair the seeds

The three arms run the **same twenty seeds against the same ELF**, so seed *k*'s
figure in two arms was drawn with the *same fixture keys* and differs only by
(a) how deep each arm's release identity made the on-chain bump searches walk
and (b) whatever the code paths actually cost. M-61's own decomposition then
applies **per seed**:

```
    delta(seed k)  =  n_k × 1,500  +  c
```

`n_k` is the lottery and varies wildly; `c` is the constant, and it is the
answer. Reporting only the difference of means throws `c` away into the noise it
is not part of. Take each paired delta, round `delta / 1,500` to the nearest
integer, and read the residual — the twenty residuals should agree to within a
few CU, and if they do not, the two arms differ by something that is not a
constant and the comparison needs a harder look.

The paired residual works here **only because both arms share one ELF and one
seed set**. Across two revisions it does not apply: different ELFs are different
lotteries, the pairing is meaningless, and `PASS` and `MEAN` are what you have.

## What the three arms measured (lane POST-0012-EXACTAUTH)

Harness `d20837fd`, one clean `git archive` build, trading ELF
`7facb8e58e45843f46b9d3d572ced5e45507bfcbfb2250e865b5427baa1b9d3c` for all three
arms. Re-run at `57138ba8` against the same ELF: **every figure identical**, so
these are reproducible to the compute unit, not draws that happened once.

| arm | PASS | MEAN | MIN | MAX | spread |
|---|---|---|---|---|---|
| `immutable` | 20/20 | **1,345,302** | 1,324,377 | 1,373,876 | 49,499 |
| `immutable-pinned` | 20/20 | **1,353,477** | 1,333,375 | 1,384,377 | 51,002 |
| `slot-pinned` | 20/20 | **1,355,575** | 1,336,454 | 1,382,950 | 46,496 |

```
    immutable-pinned − immutable  =  + 8,175 CU   REDRAW ALONE
    slot-pinned      − immutable  =  +10,273 CU   REDRAW + the 0012 arm
    ────────────────────────────────────────────────────────────────────
    difference of the differences =  + 2,098 CU   ← an UPPER BOUND, not the cost
```

**+2,098 is an upper bound and stopping there would have been a false null** —
0.16% of the baseline, less than two bump-search iterations, sitting on a
cross-seed spread of 46,000–51,000 in every arm. Reported as the answer it says
"no signal above the lottery", which is not what these arms measured.

**Pair the seeds and the constant comes out exactly.** All three arms ran the
same twenty seeds against the same ELF, so seed *k* used the same fixture keys in
each and `delta = n × 1,500 + c` solves per seed rather than being averaged over:

| paired against `immutable` | constant `c` |
|---|---|
| `immutable-pinned` (same digest arm) | **0** — exactly zero on 18 of 20 seeds, never past 6 CU |
| `slot-pinned` (0012's arm) | **+73 CU** — 67…77 on every one of the twenty |

The control's zero is the method certifying itself: identical code path,
identical constant, and the whole 8,175 CU mean gap accounted for as `n × 1,500`.
Taken against `immutable-pinned` instead — the two arms that share a bank slot —
`slot-pinned`'s constant is **+73.0** again.

**So decision 0012's `ExactAuthority` arm costs a Hot continuation 73 CU more
than the `Immutable` arm — 0.005% of the ceiling.** A mutable substrate runs the
market action at parity with an immutable one.

Say what that is and is not. Post-0012 **neither** arm hashes on this route:
both reach `authenticate_activated_current_deployment`, which reuses the
activation-bound digest, and the ~700,000 CU hash lives in the *uncached*
`authenticate_deployment` (`shadow-accelerator-auth-v4/src/deployment.rs`, the
`hash(programdata_view.elf())` branch). So what these arms measure is the thing
the decision actually needed — *mutable: refused → admitted, at parity* — and
the ~700,000 saving itself is **not** measured and cannot be by this instrument,
because no in-tree fixture constructs the fallback it is a saving against: the
readers refuse a non-pinned release rather than hashing. That figure stays an
argument from ELF size.

The arm is genuinely executing: `slot_pinned_release_elf_digest_v1`'s
`ExactAuthority` branch runs **four times per Hot transaction** — Core and
Trading in `batch_v2::authenticate_request`, then Core and Trading again in
`hot_v3`'s two `authenticate_activated_current_deployment` calls — and
`tests/slot_pin_supersession.rs` reads the staged ProgramData back off the chain
to require a live authority and the pinned slot before it believes any of it.

**Where the rest of the +2,098 went, since the constant is only 73.** Into the
lottery, and one part of it is measured rather than bounded.
`every_substrate_draws_a_different_release_identity_and_activation_bump` (same
test file) prints the bump depth of the activation-cache PDA — the one on-chain
`find_program_address` seeded by the release-set identity and *nothing else*, so
its depth is constant across all twenty seeds and is a fixed per-substrate offset
a 20-seed mean cannot average away. All three swept arms draw **bump 255, depth
0**: that site contributes **0 CU** of difference between them. (The superseded
arm draws 254, one iteration deeper; it is not in the CU comparison.) What
remains is the per-seed admission PDA and the market/root derivations, which the
mean averages only imperfectly over twenty draws — `2,098 − 73 = 2,025 CU`, about
1.35 bump iterations spread across twenty seeds, is residual lottery. That is why
the mean difference is an upper bound and the paired constant is the estimate.

`immutable` runs at bank slot 1 and both pinned arms at 168, because a nonzero
pin is not loadable at slot 1 (see below). That is a second difference inside the
`immutable-pinned − immutable` figure, which is why the comparison is also taken
against `immutable-pinned`: the two pinned arms share the bank slot, the clock,
and the fixture state derived from them. Both pairings return the same paired
constant — `+72.6` against `immutable`, `+73.0` against `immutable-pinned` — so
the bank slot contributes no constant of its own, which is worth knowing rather
than assuming.

### The negative: a pin that no longer holds

`slot-pinned-superseded` does not sweep — a CU mean for a refusal is not the
measurement. `tests/slot_pin_supersession.rs` requires it by name:

| what | observed |
|---|---|
| refusal | `Custom(0x100D)` = `RegistryError::ReleaseSuperseded`, from the program's own enum |
| compute before refusing | 51,574 CU of 1,400,000 |
| Trading invoked | **no** — checked in the program log |
| material state moved | none — every rollback-snapshot account byte-identical |

`0x4007` (`TradingSbfError::ReleaseSuperseded`) is **unreachable on this route**,
and that is a property of the protocol rather than of the fixture: the Registry
authenticates the Core and Trading role deployments before it forwards anything,
and Trading reads the same two ProgramData accounts one CPI later, so any slot
move visible to Trading is visible to the Registry first. The "Trading invoked:
no" row is what makes that an observation instead of an argument.

The substrate stages this as the whole release set redeployed at slot 531 with
every release re-issued and re-pinned **except Trading's**, so four pins hold and
one does not in the same transaction against the same accounts. A substrate where
every pin was stale would also refuse, and would not separate "the pin refuses
the release that moved" from "this fixture refuses".

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
| `summary-<substrate>.json` | `dclutch-hot-cu-sweep-v2`: pass/fail, all-seeds-completed, nullable mean/min/max, the ELF digest, the substrate |

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
