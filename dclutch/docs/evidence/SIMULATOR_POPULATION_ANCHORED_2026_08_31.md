# A world that resolves into more than one cell — 2026-08-31

**Lane** SIMLIFE-3 · **Substrate** a fresh loopback validator on port block
41480, seven-role successor release set built at `ff2c8e35` ·
**Seed** `dclutch/simlife3/2026-08-31/twelve-and-a-band`

## What this lane found

SIMLIFE-2 built a population that could not trade and did not know its own
answers were constant. Six things were in the way. Five are gone.

| wall | state |
|---|---|
| founding does not create the Direct execution root | **wired here**, `1415a86d` |
| every market was founded at the one fee rate that cannot be filled | **fixed here**, `1415a86d` |
| every market resolved into the same cell, on every chain, always | **fixed here**, `a71814be` |
| `run.py --through participant` could not complete on any chain | **fixed here**, `4365faca` |
| the validator purged the history its own drivers read back | **fixed here**, `ff2c8e35` |
| a fill called a ten-action driver once and called that executed | **fixed here**, `44857bba` |
| claim-check compaction has no driver anywhere | still open, sized by SIMLIFE-2 |

## The band was drawn in the wrong units, and the outcome was a constant

`PythAdapterConfigV1::validate_update` returns `Ok(i128::from(price))` — the raw
signed price atoms, with no rescaling to any denominator — and the committed
local fixture (`fixtures/pyth/local-upgraded-2026-08-22`, PROVENANCE line 99)
carries price `100000000` at exponent `-8`. **So the coordinate every local
market resolves at is 100,000,000, on every chain this repository has ever
started, forever.**

Meanwhile the engine drew its cuts from `BAND_CENTER = IntUniform(4_000,
40_000)` at `BAND_SPACING = IntUniform(400, 6_000)`, under a comment reading
"the coordinate domain is USD cents per SOL" — a devnet SOL/USD framing. Every
cut the old rule could draw was **three to five orders of magnitude below the
observation**, so the observation landed above the top cut in one hundred
percent of markets.

That is not a skew a bigger sample fixes. It is a units mismatch that made the
outcome a **constant**, and the world's own `selected_cell` — drawn uniformly
and independently of the band — hid it, because the plan's expectation and the
chain's answer were never the same question and nothing ever compared them.

**What replaces it.** `WorldSpec.coordinate_anchor` carries the coordinate this
world's substrate will observe, so a world drawn for one chain cannot be run
against another by accident. Spacing is a per-market volatility in basis points
of the anchor, scaled by the **square root** of the market's own window — the
random walk's own scaling, so twenty times the horizon is about four and a half
times the band. Placement is drawn in units of the band's own spacing, wide
enough to put the observation in any cell including both open tails: **that draw
is what makes outcomes distribute.** Gaps vary by profile (`tight-centre`,
`tight-edges`, `ragged`, `uniform`), rescaled by construction so a profile is a
shape over the gaps and never a second scale. And `selected_cell` is now the
cell the anchor falls in, so the plan states a checkable expectation instead of
an unrelated wish; the certificate on chain still decides.

## Outcome spread is a health property of a run

`World.outcome_spread()` counts where every resolving market settles, `plan`
prints it beside the horizon line, `world.json` carries it, the capture publishes
it and `/population` draws it. A world putting more than **70%** of its resolving
markets in one place is flagged `DEGENERATE OUTCOME SPREAD` with the knob to
check named.

**The flag is over POSITION, not over `cell/width`,** and that is the difference
between a metric and a decoration. Keyed by `cell/width`, "every market landed in
its bottom cell" spreads across as many keys as the world has widths and reads as
diverse — the historical defect would have passed. Position normalises the cell
to where in its own market it sits, so "always the bottom" and "always the top"
are each one bucket. A test reproduces the historical mismatch and asserts the
weaker reading misses it.

Markets with a single ordinary cell are counted in the cell histogram and
excluded from the position one: a width-2 market has no cuts, so its whole
coordinate domain is one region, and filing that as "the bottom" would make a
world of deliberately narrow markets read as degenerate for being narrow.

## Zero was the one fee rate that could never be filled

Every archetype drew `Constant(0)` and the engine **refused** any nonzero rate at
its draw site, citing `FEE_SECOND_TRANSACTION_FOUNDATION_2026_08_30.md` as saying
fee-bearing founding does not fit in one transaction. That document is about the
Direct **fill's** fee leg — two Custody CPIs the transition co-enables — and says
nothing about founding; fee-bearing foundings were measured landing on a loopback
validator on 2026-08-30.

The citation was not merely wrong, it was inverted. The owned-loopback Direct
producer has no ticket to read, authors its own terms, and admits exactly
**50 bps** (`direct_trade_producer.rs`, `FEE_BASIS_POINTS_V1`); the chain agrees,
because `direct_token_setup_v1` is the sole creator of the seller and venue
Direct token accounts and refuses at any other rate. **A zero-fee market can
never be filled.** The guard made every market in every world untradeable.

Each archetype now draws a band weighted towards the admitted rate with a
deliberate tail, so the producer's rate clause is exercised rather than
described. The same false guard was mirrored in TypeScript
(`simulatorSeries.ts`) and refused every capture of a world that could trade; it
is renegotiated in the open to the protocol's own `0..10000` domain, with the
test that pinned the old property now pinning the new one and the reason written
into it.

## The activation step a fill cannot skip

**Founding does not create the Direct capability execution root and nothing else
does either.** The root is written by Core's `ActivateCapability` route CPI-ing
Trading's `process_activation`, and
`local-private-validator-direct-capability-activation-v1` is the only command in
this tree that reaches it on a loopback validator. `drive_direct_activation` is
now part of a market's ordinary founding, between the campaign and the first
admission, which is the order the tree documents.

A fill with no activation refuses by naming the **step**, not the root. The
producer's own sentence is about a root, and absence arrives at that check
wearing an owner change's clothes — a finalized snapshot renders a missing
account as a System-owned zero-length placeholder — which is how twenty-one
refused fills were once read as a claim about a widened market's width.

**One taker per market**, and the fixture decides that rather than the world. A
Direct fill debits the buyer 50,250,000 atoms; the participant fixture holds
100,000,000 and the compiler refuses any other nonzero amount, so two fully
funded buyers do not fit. The first admission takes exactly the requirement —
which also satisfies the equality `inline_candidate_v2.rs` requires of the
buyer's delegated allowance, found independently by FINALIZATION and TRADE-4 —
and the rest hold positions they cannot fill with.

## Four driver walls, found by running rather than by reading

**`run.py --through participant` could not complete on any chain.** The admission
message does not fit a legacy transaction: the stage refused `admission message
compilation: PacketTooLarge` after the prefund transfer had already landed, and
no flag on the invocation could fix it. The founding creates five routing tables
and freezes exactly one; the founding's own is the frozen table (authority
`None`) whose address list contains the market, and both facts are already on the
chain. Compared as bytes; ambiguity refuses rather than picks.

**The validator purged the history its own drivers read back.** Every driver
re-verifies its earlier stages from transaction history, and the Direct trade and
the flagship resolution advance one durable action per invocation with minutes
between them. `solana-test-validator` purges root slots in multi-thousand-slot
chunks under its own default, and a purge between two stages strands the journal
**permanently** — no retry recovers it, because the history is gone rather than
late. Measured by FINALIZATION; fixed here.

**A fill called a ten-action driver once.** `…-direct-trade-v1 --execute`
advances exactly one durable ALT, seal or Hot action per invocation and its usage
says so. `drive_fill` called it once, got zero, and recorded the fill as
`executed` over a trade that had barely started. It is driven to completion now,
and completion is the driver's own word for it: once the trade finalizes it
prints its persisted evidence document rather than another journal.

**The resolution's provisioning loop broke on prose it had guessed** —
`"complete" in output and "frozen" in output`, a pattern written from memory
about sentences the driver may or may not print. Completion is read off the
journal now. This one was wasteful rather than wrong, which is exactly why it
would have survived another year.

## The emitter was writing a document this app's own decoder refuses

Running the capture emitter over a real work directory and feeding its output
back through `parseSimulatorSeriesV1` found **four** disagreements at once: the
spend block written beside the substrate instead of on it, histogram counts
rendered as decimal strings where the decoder wants numbers, lamport quantities
rendered as numbers where it wants strings, and the fee guard above.

None of the 79 unit tests on that decoder could have caught any of them, and the
reason generalises: **every fixture in that suite is hand-written to match the
decoder.** A fixture authored against the reader proves the reader parses itself
and says nothing about the writer — and writer/reader pairs here are split across
three languages, which is exactly where they drift.

## The spend ceiling, and why it is cumulative outflow

`budget.max_lamports_spent` is the fourth kill condition and was the missing one:
`HALT.json` catches a broken law, `EXIT.json` catches an ending, the heartbeat
deadline catches a SIGKILL, and nothing caught a run that was merely expensive.
Crossing it halts the way a violated law does — a restart refused until a human
clears it — with its own word (`overspent`, exit 6) because a broken conservation
law is a fact about the **ledger** and a spent budget is a fact about the **run**.

Spend is cumulative **outflow**: every fall in a payer's balance is added, every
rise recorded separately and never subtracted. Under a first-observation delta
one mid-run airdrop forgives every lamport spent before it, and that case is a
test. The numbers come from the census's own `payer_lamports`, the field `L7`
already watches, so this adds no read and invents no source.

## The run

Seed `dclutch/simlife3/2026-08-31/twelve-and-a-band` (`50f2c4ddf517`), plan
digest `a662e592c02c`, 14 markets over 40 ticks against
`http://127.0.0.1:41480/`. The Pyth update account was provisioned once for the
chain — nine journaled actions, all executed — before the world started.

**The world it drew.**

| | |
|---|---|
| archetypes | `ladder` ×5 · `coin-flip` ×2 · `quiet-corner` ×2 · `hairline` ×1 · `long-tail` ×1 · `short-fuse` ×1 · `tent-band` ×1 · `wide-field` ×1 |
| destinies | resolves-clean ×8 · commit-deadline-failure ×3 · founded-then-sleepy ×3 |
| personas | `eager-maker` ×14 · `patient-maker` ×12 · `sleeper` ×9 · `compactor` ×7 · `prompt-redeemer` ×7 · `crank` ×3 |
| outcome widths | 2, 3, 4, 5, 6, 7, 8, 9 |
| fee rates | 0 bps ×2 · 25 bps ×1 · 50 bps ×5, across the foundable markets |
| events | 719 planned |

**Eight markets founded and every one of them activated.** `m00` (long-tail, 6
cells, 50 bps), `m01` (coin-flip, 3, 25 bps), `m04` (quiet-corner, 2, 50 bps),
`m05` (hairline, 3, 50 bps), `m08` (wide-field, 9, 0 bps), `m09` (quiet-corner,
2, 0 bps), `m11` (short-fuse, 4, 50 bps), `m12` (coin-flip, 3, 50 bps). The
other six are ladders and a tent — graded bases `compile_linked_basis_v3` does
not emit — and they are `unattempted` at their own draw site with that sentence.

**288 censuses executed across eight markets and no conservation law was
violated at any drawn tick.**

**The measured tick was 158 to 812 slots against the 900 the plan assumed**, and
the spread across markets is the point rather than the average: a conductor that
spends ten minutes founding one market and one second censusing eight of them
does not have a constant tick. Reported, not reconciled.

### The fee band did exactly what it was added to do

| market | rate | fills | what the chain said |
|---|---:|---:|---|
| `m08` | 0 bps | 16 | *"owned-loopback Direct producer requires the exact 1,000,000 scale and 50-bps config"* |
| `m01` | 25 bps | 9 | the same |
| `m00` | 50 bps | 5 | reached the producer's key check, then the trade |
| `m12` | 50 bps | 7 | the same |

**Twenty-five of the thirty-seven refused fills refused on the RATE, and they
were drawn that way on purpose.** Before tonight every market in every world was
zero-fee, so that refusal was one hundred percent of every run and looked like
"fills do not work". It is now a labelled control arm, and the twelve fills on
the admitted rate refuse somewhere else entirely — which is the whole value of
having drawn both.

### Three walls behind the rate, found by running

The twelve fills on the 50-bps markets refused at *"a private key file did not
expand to its evidence-derived public identity"*. The producer opens exactly
three files out of `--key-dir` and one of them is `participant.json`, **the
buyer** — and a market's own founding key set already contains a
`participant.json`, the fixture-liquidity owner. The founding role was standing
where the admitted buyer belongs. Fixed; the buyer is the admission's own
position owner, written last and unconditionally.

With that fixed, one trade was driven by hand against the live chain and walked
**replay-setup, token-setup, lookup-create and lookup-extend, every one
finalized**, before refusing at the next action with `custom program error
0x4001` from the chain. That is the furthest a Direct trade has gone inside this
driver layer, and the first refusal here that belongs to a program rather than
to a driver.

Getting there took two more driver fixes, both found the same way. A produced
trade could not be resumed — a non-empty output directory was refused, so ten
durable actions with their own journals could never be picked up after an
interruption, and signed work the chain had already accepted was abandoned. And
the stall check called two working trades stuck: keyed on the schema it fired
across `replay-setup` and `token-setup`, keyed on schema plus stage it fired
across the three consecutive `lookup-extend`s. It is the whole document digested
now, because a driver that is genuinely not advancing prints the same report.

### What the resolution said, and the failure walk

Four resolutions refused at *"completed campaign founding_market evidence
differs from the current finalized account"* — the producer re-reads the market
and finds it is not the account the founding sealed, because admissions have
moved collateral into it since. That is a real ordering constraint nobody had
stated: **on this substrate a resolution is produced against a market that has
been admitted to, and the producer authenticates the founding's snapshot.**

Two failure walks refused for the reason SIMLIFE-2 recorded: the local market
resolves through the PULL Pyth family and `sponsored-push` consumes the
SPONSORED family's input, so there is no such document to hand it.

Sixteen compactions are `unattempted`: no CLI exists anywhere.

### The census caught the driver layer lying

A second walk over the same chain halted on `m04`: *"tracked 228894899 atoms
across 6 accounts != Mint supply 178644899; 50250000 atoms are in accounts this
ledger does not name"* — fifty million atoms more than the Mint had ever issued,
in a market where nothing had moved.

The holder's participant token account came back from the prior-accounts scan as
`prior_01` while the admission also named it `holder_m04-p0`. **One address, two
labels, and the conservation ledger sums per account.** The first walk never saw
it because the prior scan runs once per market at binding time and no holder
account exists on a fresh chain; it takes a second walk over the same chain,
which the adopt-rather-than-restart doctrine makes routine.

The census was right and the caption was wrong, which is the entire reason that
law exists. Bindings de-duplicate by address now. The halt discipline worked as
designed throughout: `HALT.json` on disk, exit 3, and the work directory
refusing a restart until a human cleared it.

**Sized, not built:** `MarketCensus.adopt_existing` adopts the newest census file
on disk, including one whose last entry VIOLATED — so a halted run's poisoned
census becomes the next run's baseline and L5 reports the correction as a fresh
violation. Adopting the newest *holding* census instead, and saying so, is about
an hour. This run moved the two affected files into `census/m04/superseded/`
rather than deleting them.

### The second walk, and what it added

The world was walked twice. The first walk built it; the second re-walked the
same plan with the three fill fixes in, adopting every founding and re-censusing
every market — 719 events again, exit 0, **288 censuses and no law violated**,
and this time carrying the substrate revision the first capture was missing.

The published capture is the second walk, and one number in it needs saying out
loud: its `spent_lamports` is **0**. That is true of that walk — it adopted a
world that already existed and moved nothing new — and it is not what the world
cost. The first walk measured **1,630,291,423 lamports** of cumulative outflow
across eight foundings, eight activations, twenty-four admissions and 288
censuses, about **0.2 SOL per market**, all on one campaign payer. The ceiling
was armed at 200 SOL, was never crossed, and the halt path is proven by test
rather than by either walk.

With the buyer identity fixed, the 50-bps fills refused one wall further on, and
it is the one FINALIZATION named in advance:

> *"seller Token-2022 destination was not a System-owned data-empty PDA
> prestate"*

**The producer admits only a VACANT seller and fee token, so no market can be
traded twice.** FINALIZATION recorded that as WALL 7 against devnet and predicted
it would bite this run; it did, on a loopback validator, by an independent path.
One fill on `m12` advanced to its second durable action before hitting it.

A second refusal, *"Direct seal exists but the request-specific ALT journal is
absent"*, is an artifact of this lane rather than a wall: the hand-driven trade
created a capability seal for `m00` under its own slug, and a later fill of the
same market found the seal present with no journal of its own. Recorded so
nobody chases it as a protocol finding.

## The page shows what happened, not our ticket queue

`/population` printed the substrate's own note for every route's commonest
not-done step, verbatim, and those notes are engineering register entries: the
page carried a file path, a Rust test name, an hour estimate and a raw nested
Rust error as public copy. Found by DESIGN-2's audit.

Fixed at the render layer. `publicReasonV1` maps a row to one short descriptive
sentence and the strip prints that; the full note stays on
`world.not_done[].reason` in the capture, which is the record. Every branch is
true of every row it matches, and the fallbacks are keyed on the **outcome word**
alone — those three words are defined, so a sentence built from `unattempted`
cannot be wrong about a reason it did not read.

**The test asserted the opposite and was passing the whole time.** It required
the substrate's own sentence to reach the page, so no amount of running the suite
would ever have found the leak. It is replaced by two tests plus a vocabulary
ratchet rather than a list of the four things found: the rendered page must
contain no file path, no snake_case test name, no hour estimate, no nested Rust
error, no `sha256` and no Rust path separator. A fifth leak fails without anybody
adding a case.

## What was NOT verified

- **No devnet or mainnet action of any kind, and no devnet read.** Every
  transaction and every read went to the loopback validator on port block
  41480. `~/.helius-key` was not read. Nothing of this lane touched 43080,
  34500, 26900, 27100, 29300, market19, market20, or any job directory.
- **Nothing was deployed or published to the site.** The capture is committed so
  `git archive` can carry it; no site deploy happened and no screenshot was
  taken. The page is proven by its tests rendering the committed capture and by
  one render of the live run's own interim capture, not by a browser.
- **The graded bases were not executed and were not meant to be.** Six of the
  world's fourteen markets are ladders and a tent, and
  `compile_linked_basis_v3` hard-wires `CategoricalQ1`; they are `unattempted`
  at their own draw site with that sentence. Protocol tier, out of scope.
- **Claim-check compaction was neither driven nor built.** No CLI exists
  anywhere; it is sized in SIMLIFE-2's evidence and unchanged here.
- **The spend ceiling was armed but never crossed.** It measured real outflow
  throughout and the halt path is proven by test, not by this run.
- **No `cargo nextest` suite was run and no program was changed.** This lane
  touched no Rust under `programs/` or `crates/`. The controls are the Python
  suites, the web suites, and the run itself.
