# What the site draws from the simulator, and what it was throwing away — 2026-08-30

**Lane** SIMVIZ · **Tree** at this doc's commit · **Artifact measured**
`apps/dclutch-web/public/simulator-series.json` and the live census directory it
is captured from, `/private/tmp/dclutch-sim-devnet-market18/census/`

## Result

The site had two charts drawn from the simulator's record, and both were the
flat line by construction. Across **432 census observations** of the live
devnet run, exactly **one** observed field moves — the slot:

| field | distinct values across 432 observations |
|---|---|
| `slot` | 432 |
| `aggregate_supply` | 1 |
| `hoard_atoms` | 1 |
| `tracked_collateral` | 1 |
| `token_atoms` | 1 |
| `position_balances` | 1 |
| `payer_lamports` | 1 |
| `mint_supply` | 1 |

This is not a defect in the simulator. The devnet run is in **census-only
mode**: it signs nothing, so it spends nothing, so no quantity it observes about
the market has any business changing. The record is honest. The **drawing** was
the problem — it drew only the still quantities and spent the moving one on a
caption.

Two things move the whole time and neither belongs to the simulator: the chain
advanced **36,637 slots** across the drawn window, and the run's own recorded
instants say it took **101 minutes** to do it. `simulatorSeriesSpanV1` already
computed both and used them in one sentence.

And the census has always recorded **which** conservation law it checked — `L1`
through `L7`, defined in `tools/gauntlet/journey/src/ledger.rs` — together with
a sentence saying what it compared. The v1 series schema reduced all of that to
three integers before it reached the browser. That is how

> `L4  Hoard 500000000 >= worst outcome 500000000 x unit 1 = 500000000`

— the protocol demonstrating its own solvency in one line — arrived at the
reader as a `6` on a flat sparkline.

## Inventory: every surface that reads the series, and what it drew

`simulator-series.json` has exactly two consumers in the app.

| surface | mount | drew, before | state |
|---|---|---|---|
| `/pulse` § "over time" | `PulseWorkspace.RecordedCycles` | issued claims per outcome, 4 lines | **flat by construction** — 4 identical bands |
| `/pulse` § "ledger check" | `PulseWorkspace.RecordedCycles` | count of checks that held | **flat at 6** after cycle 1 |
| `/market/<a>` | `MarketDetailWorkspace` → `MarketIssuanceHistory` | issued claims over cycles | flat, and renders only for the one recorded market |
| `/markets` listing | `MarketDiscoveryWorkspace` → `MarketIssuanceHistory` | same | same |
| `/pulse` § holders | `PulseWorkspace.WhoIsHolding` | newest-cycle positions + collateral holders | one founding position, correct, static |

Every other chart in the app (`CellStrip`, `PositionBars`, `SupplyShareStrip`,
`PayoutShape`, `NumberStrip`, `LandingPulse`) draws a **snapshot read live off
a chain**, not the series. Those were never the problem; the series is the
app's only time axis, and the time axis was the dead one.

The status artifact `simulator-status.json` is a separate read and was fine: it
carries the heartbeat deadline, the retention bound, the wallets and the last
reconciliation verdict, and `/pulse` renders all of it.

## What changed

Landed in `f98f491e`.

**1. The heartbeat** (`simulatorHeartbeatV1`, `/pulse` § 02). Slots the chain
advanced between consecutive readings; wall-clock seconds between consecutive
readings; a slot rate measured by dividing the run's own two totals. On the
committed capture that is **6.03 slots/s over 239 intervals**, cadence from 19
to 35 seconds. Two separate figures, never one pair of axes — slots and seconds
are different dimensions, and a shared scale would be a dual-axis chart with the
scale picked to make a shape. The cadence line is drawn only when **every**
interval on it was measured; a line with a hole in it is dropped rather than
silently redrawn shorter than its own x-axis.

**2. Series schema v2** carries the laws' names. A top-level `law_ids` array
plus, on each point, one compact character per law (`h` / `v` / `i`). The
decoder refuses a verdict string whose length disagrees with the names — a
shifted string would report `L2`'s result under `L1`'s name and every reader
downstream would believe it. v1 documents still decode, as a series that
recorded no law names, which is a true thing to say about a capture taken before
this existed.

**3. `LawBand`**, a new chart form. One row per law, one cell per cycle. It is
not a sparkline because a verdict is a **state**, and a line drawn through
states invents an ordering between them that does not exist. The status trio is
the site's existing reserved palette (`.status-chip.pass` / `.fail` plus the
de-emphasis step); the skill validator was re-run against the chart surface and
the reasoning is recorded in `app/charts.css`. Every cell also carries its
verdict in words and every row a glyph, so no state rests on a hue.

On the committed capture the band reads: **7 laws, 240 cycle boundaries, 1,440
held, none broke**, and `L7` inapplicable throughout for a stated reason — the
census did not drive the transactions between boundaries and refuses to guess
their fees.

**4. The band's first draft was 294 KB of markup** — 1,680 rectangles drawing
what 7 could. Consecutive cycles with the same verdict now collapse into one
mark wherever the cells are too thin to carry a 2px separator. That is a pure
rendering change: an unbroken law *is* an unbroken run, and a differing cycle
splits the run where it differs, which is the shape a reader is scanning for.
**56 KB, 7 marks, identical picture.**

## What the aliveness data cannot do, and what would fix it

The heartbeat and the law band make the record read as what it is — a live
chain, checked continuously, holding. What they cannot produce is a **price or
odds path**, because no trade has landed in this run and nothing on this site
may imply one that has not.

Three separate things were checked for a shortcut and none exists:

- **No local validator in this environment has ever completed a Direct trade
  session.** A search across `/var/tmp`, `/tank/dregg-build` and `/private/tmp`
  on hbox found zero `direct-trade-finalized.json` and zero
  `participant-handoff.json`. `build_config_from_probe.py` needs the second one.
- **The three standing hbox validators are founded but not tradeable by this
  lane.** MEMBRANE's `127.0.0.1:29300` holds the Structured market
  `HEanNZ1en…`, STORY-2's `26900` and the relay run's `27100` hold graduation
  markets. `hold-04/runs/seed-01/` carries `founding.json` and `market.json` and
  **no** `participant-handoff.json`, so it was held after founding, not after
  participant admission. They are also other lanes' resources; a census read is
  harmless, an `--execute` run against them is not mine to do.
- **The documented reason a local demo market refuses a trade is on record**
  (`CLI_TRADER_LOOP_2026_08_27.md`): the recipe founded without the Direct
  capability entry, and the spine says so from one read — *"Direct trading was
  never part of this Market's founding, which is the Market's own choice, not an
  outage."*

The machinery that *does* produce movement without any of that is the **journey
campaign**, `tools/gauntlet/journey/run-journey.sh`. It is a superset of the
tier-1 campaign: founding through Open, then `distribute_collateral` (collateral
out to N synthetic holders), `holder_to_holder` (a transfer ring between them),
resolution to Terminal through the Pyth transport, retirement, and rent
recovery — folding every boundary into the same `ObservationV1` census the
series artifact already reads. That arc moves `token_atoms` per holder, moves
the market's phase, and makes `L6` (rent conservation on a closed account)
applicable for the first time. It is self-contained: `--rpc-port auto` takes a
free 42-port block, `--commit REV` archives from any repo holding that commit,
and it never touches an account outside its own fresh loopback ledger.

### Three attempts, and what each one measured

**It did not produce a series, and the reasons are worth more than the series
would have been.**

**Attempt 1, at `f98f491e` (main at the time).** The ELF stage built all seven
programs and then refused:

```
frame-diagnostics: 7 diagnostic(s) match NO entry.
  trading  _ZN19dclutch_trading_sbf22direct_replay_setup_v122invoke_replay_child_v1…
           overwrites values in the frame
journey: refusing to run a journey on artifacts the toolchain calls
         potentially-undefined.
```

Same symbol and same count as the defect CORESTATE was root-causing. Worth
recording because CORESTATE's own note said the deployed link "has a gate
nowhere" — `run-program-test.sh` gates only the two accelerator links and
`run.sh` merely warns. The **journey tier refuses**, and its ELF stage is
therefore a ~90-second yes/no on any candidate build with no validator
involved. No allowlist entry was added to get past it; suppressing another
lane's open investigation to unblock a chart is not a trade worth making.

**Attempt 2, at `1ff6ff45`** (the commit measured at 0 diagnostics). Rolling
back is not an available workaround: the journey **binary** does not compile
there either, for a different reason — the successor modules it links by
`#[path]` had not yet landed. So the tier had a floor at roughly MEMBRANE's
structured landing and a ceiling at the frame regression, and for a while today
the window between them was empty.

**Attempt 3, at `e164feda`**, after CORESTATE landed `557df0d1`. Trading built
**0 frame diagnostics** — an independent confirmation of that fix from a gate
neither lane configured — and then the same 15 compile errors appeared. They
are not about commit age.

### The journey tier does not build on main, and nothing was watching

`main.rs`'s own header says the `#[path]` arrangement is a tripwire: the journey
compiles the successor's founding source verbatim rather than forking it, so
"if those files move or change shape, this build breaks, which is the intended
tripwire." It went off silently, because nothing in CI runs this tier. The
shared files grew call sites into six modules the subset did not link:

| shared file | calls into |
|---|---|
| `market.rs` | `crate::selected_capability` (×3), `crate::funding_readiness` |
| `local_mutable.rs` | `crate::structured_market`, `crate::general_market`, `crate::rational_market` |
| `campaign.rs` | `crate::release_identity` |

plus three crate dependencies the successor has and the journey did not:
`solana-address-lookup-table-interface`, `dclutch-operator`,
`dclutch-general-adapter-contract`.

`e41d0b20` links all six and adds the three. The closure is finite — none of
the six reaches outside the set now linked — so linking them is the fix, not
forking the files or guarding the call sites. **Fifteen errors to one.**

The remaining error is a different species and was deliberately left alone:

```
error[E0063]: missing field `activation_receipt` in initializer of
  ResolutionVerifyFundReadySnapshotV3   — journey/src/resolution.rs:737
```

The operator's snapshot grew an immutable Resolution-owned V7 activation
receipt; the journey's `verify_snapshot` neither observes it nor carries it in
`ResolutionAddressesV1`. The successor gets it from
`FundingReadinessCoordinatesV1::activation_receipt`, supplied by the campaign
layer rather than derived locally. Wiring it is resolution-funding work inside
another lane's open blast radius, and a guessed derivation would build a
snapshot that reads the wrong account and refuses at runtime for a reason
nobody could trace.

**Size of the remainder:** one account wired through
`ResolutionAddressesV1` and into `verify_snapshot`'s observation list at the
position the operator's struct expects — call it an hour for someone already in
the resolution funding path — then one journey run. The run itself is cheap:
the ELF stage took **~90 seconds** with a warm cargo cache and the campaign is
a single fresh loopback validator.

## The devnet activity mode, specified and not built

**No devnet write happens from this lane.** This is the design a later lane
should hold against reality when the cohort-7 cut opens real trading.

### What flips

The simulator already has both halves. `simulator.py run --config C --sustain`
without `--execute` is the census-only preflight that is running today; with
`--execute` it drives admission, Direct session production, per-mutation
session execution, and reconciliation. The devnet flip is entirely a **config**
change, per `tools/load-simulator/README.md`:

- `cluster.label = "devnet"`, an https RPC URL, and `cluster.devnet_genesis`
  spelling the genesis hash in full. Mainnet is refused unconditionally, in
  code, not by convention.
- A `trade.devnet` block naming the sha-pinned producer inputs: plan,
  market-input, campaign report, buyer participant, checked execution release,
  and the per-pair seller/buyer tickets, all from the campaign-open facts.
- The market itself must have been founded **with the Direct capability
  entry**. Without it the spine refuses before anything is signed, and that
  refusal is correct. This is the one precondition that is not a knob.

### Wallet funding

Participants are minted and funded through `tools/release/devnet-activity.sh`
(exact-target envelope funding, signature markers, devnet only). The rule
already in that harness and worth restating because it is the expensive mistake:
**the deployer is never a participant.** A run needs, per participant, rent for
one collateral token account (2,039,280 lamports, measured on market18), rent
for one Claims Position (2,004,480), and a fee float. The existing devnet
wallets sit at ~7,080,000 lamports each, which is the empirical answer to "how
much is enough for a participant that has not yet traded."

### Spend bounds

The knobs that bound spend are the same ones that bound rate, and they should be
set together:

- `cadence.period_seconds` — README says start at "a few seconds minimum, one
  process, no concurrency". The current census run uses 20 s with ±25% jitter.
- `trade.step_pause_seconds` between session pulses.
- The run is **strictly sequential** on devnet by design: the successor already
  paces itself at 250 ms per RPC call, and one busy writer starves the whole
  per-IP budget. Two concurrent writers is the failure, not the throughput.

A spend ceiling is not currently a config field. **Recommendation:** add one —
a `budget.max_lamports_spent` compared against the fee payer's own observed
balance delta, halting the run the way a conservation violation does, because
the payer's lamports are already censused (`payer_lamports`) and `L7` is exactly
the law that watches them. Today the bound is cadence times fee, which is a
bound you have to compute rather than read.

### Kill conditions

Three exist and are load-bearing; the fourth is the gap.

1. **`HALT.json`** — written on any conservation violation, exit 3, and the work
   dir then refuses restart until a human removes it. This is the one that
   matters: a broken law is a fact about the ledger, not about the process.
2. **`EXIT.json`** — how the run ended, from a `finally`, so a crash gets a
   record as readily as a clean stop. Never refuses a restart.
3. **`heartbeat.expected_next_update_by`** — a deadline a reader compares to
   their own clock. A SIGKILL and an ENOSPC leave no record by construction;
   what makes them legible is the deadline passing with no `EXIT.json` beside
   it. `EXIT.json` is cleared at startup precisely so its absence is a claim
   about *this* run.
4. **The gap: no spend-based kill.** See above.

### The demo-vs-product rule, restated for whoever wires this

Nothing on the site may imply devnet trading that has not happened. The series
artifact carries `cluster` (`local` | `devnet`) and the surfaces already
distinguish them in prose — `/pulse` says "a local rehearsal validator, not the
public devnet" when the status says local. A local-validator campaign's series
must be labelled as such **on every chart drawn from it**, not once in a
footnote, and the `cluster` field is the mechanism that already exists for it.

## What was NOT verified

- **No devnet write of any kind from this lane.** The devnet section above is a
  design read out of the simulator's own README and code; none of it was
  executed.
- **The published site.** Nothing was published or deployed. Publishing happens
  through the existing `publish.sh` flow on ember's or ORCH's say-so; the
  artifacts here are committed so `git archive` can carry them.
- **No screenshot.** No headless browser is available in this environment, so
  the charts were verified by rendered markup and geometry (240 columns × 7
  rows, cell width 4.06 px, gap 0, viewBox 1000×109) rather than by eye. The
  first thing worth doing with a browser attached is looking at the band.
- **Two web test files are red and were red before this lane started**:
  `lib/abiVerification.test.ts` (3) and `lib/sbomVerify.test.ts` (1), both ABI
  and SBOM drift from other lanes' in-flight Rust. Baseline 123 files passed /
  2 failed; after this lane, 124 / 2 (924 tests passed, the same 4 failed).
- **The journey tier still does not build**, and no journey campaign was ever
  executed. `e41d0b20` is a link repair measured with `cargo check`, not a run:
  no validator was launched, no transaction was signed, and the campaign's
  behaviour past the founding stage is exactly as unverified as it was before.
- **No local validator was mutated.** The three standing hbox validators were
  read (`getSlot`, a directory listing) and nothing else. MEMBRANE's is theirs
  to stop.
- **The other markets' history.** `MarketIssuanceHistory` renders for exactly
  one market because exactly one has a recorded run. Every other market gets no
  chart at all, which is correct, and it stays correct only while a second
  recorded run does not silently inherit the first one's series.
