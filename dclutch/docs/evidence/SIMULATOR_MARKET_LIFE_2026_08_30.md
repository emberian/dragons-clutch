# Market life: a population of markets, and the substrate that can watch one — 2026-08-30

**Lane** SIMLIFE · **Artifacts** `tools/load-simulator/simlife.py`,
`simlife_drive.py`, `test_simlife.py`, `apps/dclutch-web/scripts/simlife-series.mjs`,
`apps/dclutch-web/lib/simulatorSeries.ts` · **Substrate** a loopback rehearsal
chain restarted from RELAY-3's run 3 substrate at source revision
`533540056d61c05faabaae07e9b78e8c90214a8e`, `127.0.0.1:34500`

## What an intricate run produces that today's cannot

Today's simulator polls **one** market through **one** canned walk, so every
quantity it observes is expected to hold still and the site's only time axis
drew four identical bands. A simlife run draws a **population** from a named
seed — markets of six archetypes with different widths, bases, fuses and
destinies, held by participants of six personas who admit at different times,
trade in bursts, redeem promptly, or never come back at all — interleaves their
whole lifecycles into one ordered event schedule, censuses every live market at
every tick through the same conservation ledger, and writes down for **every**
planned event whether it executed, was refused, was not attempted because the
substrate has no such route, or was blocked because a prerequisite never
happened.

**A run was executed**, and what it could execute is one route.

## The line this engine does not cross

> The engine decides **what to attempt and when**. The census decides **what is
> true**.

No number `simlife.py` invents ever reaches a series point. Every market
quantity in the artifact comes from `ledger-census` reading accounts on a chain,
exactly as it did before. What the engine adds is a plan, and a record of what
became of it — and that record has four words, three of which are not "it
worked":

| ending | means |
|---|---|
| `executed` | a transaction landed, or an account was read |
| `refused` | the route exists and the chain said no |
| `unattempted` | this substrate has no such route |
| `blocked` | a prerequisite of this event never executed |

Folding the last three together would turn one wall into a hundred failures.

## Market archetypes, and the distributions they are drawn from

An archetype is a bundle of distributions, never a market: two markets of the
same archetype differ in every number.

| archetype | cells | basis | deadline (slots) | destinies (weights) | participants | stake concentration | fill bursts |
|---|---|---|---|---|---|---|---|
| `coin-flip` | 2 | categorical, degree 0 | log-uniform 2,000–20,000 | resolves 8 · fails 1 · sleepy 1 | 2–4 | 100 | 1–3 × 1–3 |
| `short-fuse` | 2–4 | categorical, degree 0 | log-uniform 120–1,200 | **fails 6** · resolves 3 · sleepy 1 | 2–3 | 60 | 1–2 × 1–2 |
| `ladder` | 4–8 | **ramp, degree 1** | log-uniform 8,000–60,000 | resolves 7 · sleepy 2 · fails 1 | 3–6 | 45 | 2–5 × 1–4 |
| `tent-band` | 3–6 | **tent, degree 1** | log-uniform 4,000–30,000 | resolves 6 · fails 2 · sleepy 2 | 3–5 | 70 | 1–4 × 1–3 |
| `wide-field` | 6–12 | categorical, degree 0 | log-uniform 10,000–80,000 | resolves 5 · sleepy 4 · fails 1 | 4–8 | 35 | 2–6 × 1–5 |
| `quiet-corner` | 2 | categorical, degree 0 | log-uniform 40,000–200,000 | **sleepy always** | 1–2 | 100 | none |

Collateral is log-uniform per archetype (50 M – 6 G atoms) and split across the
cohort by a **Dirichlet** whose symmetric alpha is the `stake concentration`
column: below 100 the mass piles onto one or two participants, at 100 the split
is uniform over the simplex. The split is exact — floored shares plus a
largest-remainder pass — so the parts sum to the founding collateral atom for
atom, and no part is ever zero, because a participant with no stake is not a
participant.

Every archetype is **zero-fee**, and the draw refuses a nonzero rate at the
point it is drawn rather than at a validator: fee-bearing founding does not fit
in one transaction on today's wire
(`docs/evidence/FEE_SECOND_TRANSACTION_FOUNDATION_2026_08_30.md`), so a world
that drew a rate would be a world whose markets cannot be founded.

Deadlines are **log-uniform** and that is load-bearing. A linear draw over
120–1,200 puts nine tenths of its mass above 700 and a world drawn that way has
no genuinely short fuse in it — which is exactly the heterogeneity the whole
module exists to produce.

## Participant personas, and their distributions

| persona | admission delay (ticks) | activity weight | redeem delay | redeems | compacts strangers | cranks |
|---|---|---|---|---|---|---|
| `eager-maker` | 0–1 | 6–10 | 0–2 | 95% | 20% | 30% |
| `patient-maker` | 2–8 | 2–5 | 1–5 | 90% | 10% | 15% |
| `prompt-redeemer` | 1–4 | 1–3 | 0 | 100% | 5% | 5% |
| `sleeper` | 0–6 | 1–4 | — | **0%** | 0% | 0% |
| `crank` | 0–3 | 0 | — | 0% | 60% | 100% |
| `compactor` | 1–5 | 0–2 | 0–1 | 80% | 100% | 70% |

`sleeper` is the persona that earns its keep. A holder who never returns is not
an inactive account: their claim check sits on the chain occupying rent that a
**stranger** can recover by compacting it, and that compaction is permissionless.
A world with no sleepers never exercises it, and a world that models a sleeper
as "did nothing" never notices that somebody else did something to them.

Two rules keep that honest. Compaction is scheduled against every **dormant**
holder rather than only against `sleeper`s — a crank who took a position and
only ever cared about the permissionless steps leaves exactly the same account
behind, and the persona is recorded so a reader can tell them apart. And the
compactor is drawn from the **world**, never from the market, and is required
not to be the holder themselves: compacting your own claim check is redemption
wearing another name.

Cranks are likewise drawn world-wide for the failure walk and the retirement.
The point of a permissionless step is that a stranger can take it, so the
retirement of market 3 is driven by whoever in the world cranks, which is
usually not somebody holding market 3.

## Event scripting

Every market's lifecycle is interleaved into one totally ordered stream, sorted
by `(tick, market id, route rank, subject)` so two runs of a world execute the
same events in the same order and their ledgers compare line by line. Route rank
is the prerequisite order: founded before admitted, admitted before filled,
censused last so the census sees the tick's own work.

Fills arrive in **bursts** rather than at a rate. Real market activity is
clustered — nothing for a while, then several things at once — and a steady
one-per-tick schedule draws a volume chart that is a straight line, which is the
same failure as the flat supply chart this module exists to fix. A fill's
quantity is capped at a quarter of the smaller side's stake, so it is never
larger than what the participant brought.

A market's **settlement cell is drawn from its own width**, and its failure cell
is the protocol's disclosed last cell. What the engine records is the cell it
*expects*; the certificate on chain is what decides.

## Reproducibility: independent streams, and the distribution beside the value

Everything is drawn from one recorded seed **preimage** — a sentence, so a run
can be named and re-run by typing its name — through named, **independent**
streams: each draw site derives its own generator from
`sha256(preimage ‖ 0x00 ‖ domain ‖ 0x00 ‖ index)`. The separators are not
decoration; without them `("market", 12)` and `("market1", 2)` are the same
stream and two draw sites silently share a sequence.

Independence is the property worth paying for, and it is pinned by test:

> market 3 of an eight-market world and market 3 of an eighty-market world are
> **the same market, byte for byte**.

With one shared `Random`, adding a market or reordering a persona reshuffles
every draw after it, and two runs of a slightly edited world are never
comparable again.

A plan records the **distribution** each value came from, not only the value.
`deadline 4096` says nothing about whether that was inevitable; `log-uniform over
512..32768 → 4096` says what the world was actually like.

## Two clocks, and the number that reconciles them

A market's deadline is in **chain slots**. A run's horizon is
`ticks × slots_per_tick`. Those are different clocks and a world is easily drawn
where no market reaches its deadline before the run ends — every resolution,
failure walk, redemption and retirement then falls outside the horizon and the
run watches twelve markets hold still.

Neither clock is bent to flatter the other. The mismatch is **reported**:

- `plan` prints how many markets reach a terminal boundary inside the horizon,
  and says out loud when the answer is none;
- `cadence.period_seconds` paces the conductor between ticks, jittered and
  seeded from the run's own seed so the pauses replay too. **This was found by
  running it**: a census of one market takes under a second, so the first
  sixty-tick run walked its whole schedule in about eight seconds and the
  finalized slot moved by **two**. Every market in it held still because no time
  passed;
- and the run **measures** what a tick was actually worth, reading each market's
  census chain back and reporting the slots that really advanced per tick beside
  the number the plan assumed.

## The substrate: how a chain that founds was found at all

Fresh local foundings are walled at `0x5182 ClaimsFoundingSbfErrorV5::Release`
on the atomic `DCLTGMF3` Open leg, family-neutral, at main HEAD
(`docs/evidence/GENERAL_PUBLICATION_CLOSURE_2026_08_30.md`, wall two). That wall
is downstream of everything this engine wants to drive.

What exists instead is RELAY-3's **run 3**, a relayed-vertical success walk at
`53354005` whose substrate ledger was preserved. Its `substrate/` tree was
**copied** to `/var/tmp/dclutch-simlife/` — `/var/tmp/relay3/` is another lane's
evidence and was read, never written — and restarted on an unshared port block,
`34500` / `34502` / `34503`, dynamic `34510-34541`. Nothing of this lane's
touched `26900`, `27100`, `29300` or `33500`.

One gotcha worth carrying, because it costs a confusing ten minutes: a copied
ledger refuses to boot with

```
failed to load bank from snapshot '.../snapshots/7601/7601':
snapshot dir account paths mismatching
```

because the fastboot snapshot directory records the ledger's **original**
absolute account paths. `rm -rf <ledger>/snapshots/*` makes the validator load
from the `snapshot-NNNN-*.tar.zst` archive beside it and it comes up clean.

It came up with all seven role programs deployed, their release-set roles
activated, and RELAY-3's market `JCWoR8BP9tVbj8XUNDXXhCqjgDYgNNfkn9WcGtxj6QNJ`
standing at its terminal answer.

## What the run executed

Seed `dclutch/simlife/2026-08-30/first-light` (`7e093ae93987`), 12 markets over
45 ticks at a 20-second cadence with ±25% jitter, `slots_per_tick` 176.

**The world it drew.**

| | |
|---|---|
| archetypes | `coin-flip` ×3 · `short-fuse` ×3 · `quiet-corner` ×2 · `wide-field` ×2 · `ladder` ×1 · `tent-band` ×1 |
| destinies | resolves-clean ×8 · founded-then-sleepy ×3 · commit-deadline-failure ×1 |
| personas | `eager-maker` ×10 · `sleeper` ×10 · `patient-maker` ×7 · `compactor` ×6 · `prompt-redeemer` ×5 · `crank` ×2 |
| outcome widths | 2, 3, 6, 7, 11 |
| deadlines | 215 slots (a `short-fuse` drawn to miss it) up to 192,311 (a `quiet-corner`) |
| events | 634 in total: 510 census · 49 fill · 40 admit · 12 found · 9 redeem · 5 retire · 4 resolve · 4 compact · 1 deadline-failure |
| horizon | 45 × 176 = 7,920 slots; **5 of 12** markets reach a terminal boundary inside it |

**What it did.**

| route | executed | refused | unattempted | blocked |
|---|---:|---:|---:|---:|
| census | **41** | 0 | 0 | 469 |
| found | 0 | 0 | 12 | 0 |
| admit | 0 | 0 | 40 | 0 |
| fill | 0 | 0 | 49 | 0 |
| resolve | 0 | 0 | 4 | 0 |
| deadline-failure | 0 | 0 | 1 | 0 |
| redeem | 0 | 0 | 9 | 0 |
| compact | 0 | 0 | 4 | 0 |
| retire | 0 | 0 | 5 | 0 |

41 censuses of the one market a binding named, ticks 4 through 44, **slot 15,404
to 22,500**. Seven laws re-checked at every one of them: **243 checks held, none
broke**, 44 did not apply — the first boundary has no predecessor for L2, L5 and
L6, and `L7` is inapplicable throughout for a stated reason the census writes
itself: *"external census: the transactions between boundaries were not driven by
this ledger, and it refuses to guess their fees."* `L4` reads, at every boundary,
*"Hoard 500000000 >= worst outcome 500000000 x unit 1 = 500000000"*.

469 censuses are `blocked`, and that is the correct word: eleven of the twelve
planned markets have nothing bound to them, so there was nothing to observe.
124 mutations are `unattempted`, each carrying the driver that would perform it.

**And the tick was worth what the plan said it was.** The run measured its own
pace off its census chain: **m06 advanced 7,096 slots across ticks 4..44 = 177
slots/tick measured, against the 176 the plan assumed.** That is the loop
closing — the horizon that decided which markets could reach a deadline was
checked against the chain rather than asserted.

### A second run, and a second market to run it against

The founding wall is a **main-HEAD** wall, not a property of the founding path.
`campaign --founding-only` at `53354005`, against the restarted substrate, with
fresh keypairs for the five protocol-created founding roles and the campaign
payer reused, **opened a new market**:

| | |
|---|---|
| market | `Ge3zi3ojawRLUzMhnZ8WDaKgkoUQX6hGtGQGBT263moW` |
| outcomes | **4** |
| collateral mint | `DRcWEvMDa8jkBVfyzjPBVkDqwng4pWA74EpRRxg2nB83` |
| Hoard | `43xEUnVoXSzCNURZeDdqeeWT8UHRQqh17vkSLLHWNW18` |
| Claims aggregate | `D4W9DZyQarcky4PSJ2SMMkMaUDkRH5pyKJvNXeRTuLhx` |
| founder Position | `FF9gHQ6jgWaNpeH74VdbfceuL1Rjj7ZU5zptkLuMVQQL` |
| census | L1, L3, L4 hold; L2, L5, L6, L7 inapplicable at a first boundary |

Four outcomes is a width **no archetype in the first draft of this table could
describe** — `short-fuse` drew 2–3 and `wide-field` 6–12. It now draws 2–4,
because a market's width has nothing to do with its fuse and because four is a
width this repository actually founds. A table that cannot describe the markets
the chain makes is a table that will quietly file them under the wrong name.

So the published capture is a **second** run, against two real markets of
different widths, censused as contemporaries:

Seed `dclutch/simlife/2026-08-30/second-light-7` (`023394d50391`), same 12
markets / 45 ticks / 20-second cadence.

| | m02 | m06 |
|---|---|---|
| archetype | `coin-flip` | `short-fuse` |
| on chain | RELAY-3's graduation market | the market founded onto this chain |
| cells | 2 | **4** |
| points | 39 | 40 |
| slots | 25,316 → 31,864 | 25,114 → 31,864 |
| checks held / broke | 231 / **0** | 237 / **0** |
| Hoard vs Mint | 500,000,000 of 1,000,000,000 | 500,000,000 of 1,100,000,000 |
| personas the plan gave it | patient-maker, sleeper, compactor, sleeper | patient-maker, compactor |

79 censuses executed, none refused, **468 checks held and none broke** across the
two. 418 censuses blocked (ten planned markets with nothing bound), 129
mutations unattempted. And both paces measured: **m06 173 slots/tick, m02 172,
against the 176 the plan assumed.**

The capture is `apps/dclutch-web/public/simlife-series.json`, 103,225 bytes,
sha256 `47be77a921e7…`, and it is pinned by test: the committed document must
decode, must say it founded nothing itself, must draw only markets it observed,
and must carry chains whose slots actually moved.


### The recipe, and the compiler's one shape

`graduation-market` **refuses loopback** and is not the path:

```
the production Direct planner is devnet-only and refuses loopback;
use the lab fixture compiler for a local validator
```

RELAY-3's two-outcome market was never produced by that subcommand either — it
comes from the *relayed* compiler inside `tools/gauntlet/relayed-vertical`, which
has no standalone entry point. The loopback path is
`local-private-validator-market-v1`, whose bare `MarketRunInput` `campaign`
accepts directly:

```sh
BOOT=/tank/dregg-build/story2-src/.../release/dclutch-local-successor-bootstrap
$BOOT local-private-validator-market-v1 \
  --plan  /var/tmp/dclutch-simlife/substrate/plan.json \
  --rpc-url http://127.0.0.1:34500/ \
  --fee-basis-points 50 --fee-recipient-keypair KEYS/fee-recipient.json > market.json

$BOOT campaign --founding-only --through founding --execute \
  --rpc-url http://127.0.0.1:34500/ --plan .../plan.json \
  --market market.json --evidence founding-evidence.json \
  --founding-founder EahMVb2ptYeDKia4uTbmQHsnqT7B8kmGz2xEMBvP1F6N \
  --substituted-founder 9oNgJsLsj4XKmbr2D6Hv3SxF38rvZwy1gtTfw6RGir7w \
  --keypair-campaign-payer .../prepare/keys/campaign-payer.json \
  --keypair-{collateral-mint,collateral-wallet,founding-beneficiary,\
             founding-projection-witness,founding-source-funder,\
             participant,direct-buyer} KEYS/<role>.json
```

Three things that are not in any README and cost real time to find.

**`participant` and `direct-buyer` are required founding roles here**, and the
relayed tier's own note does not mention them. `campaign.rs:1547` extends
`FOUNDING_REQUIRED_ROLES` with both whenever
`local_participant_fixture_liquidity_atoms != 0` — which is every market this
compiler emits, and none that the relayed one does.

**A census of such a market needs a third `--token`.** These markets carry
100,000,000 atoms of participant fixture liquidity that RELAY-3's does not, so a
census naming only the Hoard and the founder's wallet fails **L1 by
construction**: *"tracked 1000000000 != Mint supply 1100000000; 100000000 atoms
are in accounts this ledger does not name."* That is the law doing its job, and
it is why `--token participant_fixture_source=…` is in the binding.

**A partially-consumed key set is unresumable.** A second attempt with the same
keys refuses before any transaction: *"this founding has STARTED on this chain …
but no compatible durable DCLTPCB2 checkpoint authenticates a safe suffix
resume."* Fresh keys, and it founds first try.

### THE SHAPE THE COMPILER CANNOT VARY, and this is the finding that matters most

Four outcomes was **not a choice**. `demo_market_input_base`
(`market.rs:11504-11555`) hard-codes `cuts = [12_000, 18_000]`,
`cut_denominator = 100`, `coefficients = [1, 0, 1, 0]`, and the outcome count is
`cuts.len() + 2`. Forced identical across **every** market this command can
emit: the outcome count (4), the claim unit (1), the initial collateral
(1,000,000,000 atoms), the fixture liquidity (100,000,000), the display
decimals, the cuts and coefficients, and the terminal window — which is derived
from a captured Pyth fixture publication and has no flag at all.

Exactly three things can vary: the capability family
(`DCLUTCH_MARKET_CAPABILITY` ∈ direct | general | rational), `--fee-basis-points`
and `--fee-recipient-keypair` — and the fee knobs are **silently ignored outside
the Direct family**, because the General and Rational compilers take no bps
argument.

So the heterogeneity this engine draws — twelve markets of five archetypes with
widths 2, 4, 5, 8 and 9, deadlines from 700 slots to 107,666, collateral across
two orders of magnitude — is, today, **unfoundable except at exactly one shape**.
That is not a defect in the engine and it is not a defect in the compiler, which
is a lab fixture and says so. It is the measurement: a load simulator can only
be as heterogeneous as the founding path it drives, and the founding path
currently emits one market.

Two were founded, and their only real difference is structural:

| | m1 | m2b |
|---|---|---|
| capability family | Direct | **General** |
| market | `Ge3zi3ojawRLUzMhnZ8WDaKgkoUQX6hGtGQGBT263moW` | `B1L2dKkaFYt7PX9UgsCSxxUuYX1799VtBudZoDwZzdCo` |
| outcomes | 4 | 4 |
| input size / transactions | 40 KB / 127 | 107 KB / **194** |
| census | L1, L3, L4 hold | L1, L3, L4 hold |

`DCLTGMF3 refuses a substituted Claims request and rolls the whole founding
back` (30,604 CU) appears inside **both** successful foundings. It is a designed
adversarial probe in the ladder, not a failure.

## The one route that is driven, and the eight that are named

`simlife_drive.py` executes exactly one route — `ledger-census`, per market,
chained through `--prior` so the delta laws L2, L5, L6 and L7 evaluate across
ticks — and **names** the driver for every other route rather than composing a
command line for a driver it has never run. A command line built from a README
and never executed is the most expensive kind of code in this repository: it
looks like a route and is a guess.

| route | what drives it, verified against the binary's own dispatch |
|---|---|
| `found` | `local-private-validator-market-v1` compiles against a LIVE deployment, then `DCLTGMF3` opens it. **Blocked: `0x5182`** |
| `admit` | `local-private-validator-user-position-admission-v1 --execute` |
| `fill` | `…-direct-trade-produce-v1` then `…-direct-trade-v1 --session --execute`, one invocation per durable mutation. Needs a market founded **with** the Direct capability entry |
| `resolve` | `local-private-validator-flagship-resolution-v1`, three modes, then the Core terminal admission |
| `deadline-failure` | `local-private-validator-sponsored-push-v1 --action commit-failure --execute`. The bare relay `RelayActionV1::CommitDeadlineFailure` has **no** driver in the successor binary; sponsored-push is the one CLI path to that frame |
| `redeem` | `…-wallet-terminal-payout-input-v1` then `…-wallet-terminal-payout-v1 --execute` |
| `compact` | **no CLI exists anywhere** — see below |
| `retire` | `local-private-validator-aggregate-retirement-v1`, four packets, journaled |
| `census` | `ledger-census …` — **driven here** |

## Two findings that are not this lane's to fix

**Only `CategoricalQ1` is FOUNDABLE.** `BasisKindV3`
(`crates/dclutch-product-payoff-v2-codec/src/runtime_v3.rs:122`) admits
`CategoricalQ1` and `GradedExactComplement`, whose term shapes are `Constant`,
`RampUp`, `RampDown` and `Tent`; degrees 0 and 1 are exempt from the price gate
by proof, so the graded shapes decode, evaluate and settle on today's wire. No
founding driver emits one. `compile_linked_basis_v3`
(`tools/local-validator/bootstrap/successor/src/market.rs:1683`) hard-wires
`kind: CategoricalQ1, payout_scale: 1`, zero knots and zero terms, and founding
refuses any other kind at `market.rs:3487`. All four capability compilers —
Direct, General, Rational, Structured — funnel through the same base, so **every
market this repository can found is a categorical one**, and every graded
construction site outside that path is inside a `#[cfg(test)]` module.

The `ladder` and `tent-band` archetypes are kept anyway, because an archetype
table containing only what today's compiler emits cannot say what is missing. A
substrate declares which basis kinds it can **express**, and a founding it
cannot express is `unattempted` with that sentence — never a failure, and never
quietly redrawn as a categorical market wearing a ladder's name.
`archetype_mix: "foundable-today"` is the same world restricted to what a real
substrate can drive; the day a founding driver emits a graded basis, that preset
should be deleted rather than edited.

**Claim-check compaction by a stranger has no driver anywhere.** It is
implemented and green in ProgramTest —
`programs/dclutch-claims-sbf/tests/claim_check/mod.rs`, sixteen tests including
`a_market_retires_a_sleeping_holders_position_and_the_holder_is_still_paid` and
`a_compacted_claim_check_is_worth_exactly_what_redemption_would_have_paid` — and
**no gauntlet bindings file names any claim-check or compaction label**. It is
covered and census-unbound, which is the same shape as the six `resolution/*`
routes already in `tools/gauntlet/blocked.json`.

## Series schema v4

v1 through v3 all describe a single market, because until now the simulator only
ever watched one. A population's markets are **contemporaries**: read at the same
boundaries, so their lines share an x-axis and can honestly be drawn beside each
other. Splitting them into N files would throw that away.

Nothing about the old shape changes. The top level of a v4 document still
describes exactly one market — the primary, chosen as the longest-observed, ties
broken on id so the choice is stable across captures — so every surface written
against v1, v2 or v3 keeps drawing without knowing v4 exists. Two blocks sit
beside it: `world` (the seed sentence, the plan digest, the substrate with its
routes and basis kinds both present and absent, every market that was PLANNED
whether or not it was observed, and — route by route, grouped so a reader sees
one sentence per reason rather than four hundred copies of it — what the run
could not do) and `markets` (one sub-series per OBSERVED market).

That last part of `world` is the artifact's conscience. A world plans nine kinds
of thing and a census-only substrate can do one of them; a page that draws such
a run without saying so reads as a trading record, which it is not.

The decoder refuses five new ways for a capture to lie, and they are all one
species — a caption that disagrees with its own charts:

- a planned market flagged `observed` that no series carries;
- a world counting more observed markets than the document holds;
- two markets sharing one id, which would draw one over the other;
- a planned market carrying a nonzero fee, which describes a market this
  protocol cannot found in one transaction today;
- a `not_done` row whose outcome is not one of `refused` / `unattempted` /
  `blocked`.

The per-market parse is **factored, not copied**: a market nested inside a
population is held to exactly the same length checks, ordering rule and
settlement bound as a market on its own, so no figure becomes admissible by
being nested. And a planned market that was never observed appears in
`world.planned` and **never** in `markets`, because a market with no points must
not be drawn as a market whose line is flat at zero.

## Exactly what unblocks the rest

In the order a lane would pick them up.

1. **`0x5182`, and this lane narrowed it.** Until the atomic founding leg
   passes at HEAD, no world can create its own markets and every route
   downstream of `found` is `blocked` by construction. GENPUB eliminated the
   activation-cache bump reader and the writability merge from chain state;
   what remains is the per-role deployment authentication inside
   `authenticate_releases`, where the 142,018 CU goes.

   **What this lane adds is a green control.** The same recipe at `53354005`,
   against a substrate prepared at that revision, founds a market today — so
   `0x5182` is a property of what changed between `53354005` and HEAD, not of
   the founding path. That is a bisect anchor rather than a diagnosis, and it
   is the cheapest one available: the substrate, the gate, the binaries and a
   worked founding are all on disk.
2. **A founding path that can emit more than one market.** This is the second
   gate and it is independent of the first: even with `0x5182` gone, the local
   compiler hard-codes the outcome count, the collateral, the claim unit and the
   window, so a world of twelve different markets founds twelve identical ones.
   The knobs a load simulator needs are the ones already in the struct —
   `cuts`, `initial_collateral_atoms`, `claim_unit_atoms`, the window — exposed
   as arguments rather than as constants in `demo_market_input_base`. That is a
   lab fixture growing a parameter list, not new protocol.

3. **A graded founding path.** One driver that emits a `GradedExactComplement`
   basis — the operator already has `compile_graded_basis_admission_v3` and
   `validate_finalized_graded_basis_admission_v3`
   (`crates/dclutch-product-runtime-v2-operator/src/graded_basis_v3.rs:55,151`)
   and no CLI reaches either. Two of six archetypes become foundable the day it
   lands.
4. **A claim-check CLI**, or a gauntlet binding for the ProgramTest coverage.
   Without one, the sleeper persona's whole point — that a stranger recovers the
   rent — is a plan and never an observation.
5. **Wiring the remaining eight routes into `simlife_drive.py`.** Each is one
   `Substrate.execute` branch around a driver that already exists and already
   owns its journal; the table above names them. They are deliberately not
   written yet, because a command line for a driver this lane has never run
   against a market it has never founded is a guess that looks like a route.

## What was NOT verified

- **No devnet or mainnet write of any kind, and no devnet read either.** Every
  transaction and every read in this lane went to a loopback validator this lane
  started, on a port block nobody else holds. `~/.helius-key` was not read.
- **No market was founded by the simlife ENGINE.** The published run's
  `markets_founded_by_this_run` is empty and the artifact says so: both markets
  it observed existed on the chain before it started, and every founding event
  is `unattempted` with that sentence. The second market was founded by the
  successor bootstrap's own `campaign --founding-only` driver, by hand, before
  the run — which is a different claim and is made separately above.
- **No mutation of any kind was executed.** The only route driven is read-only.
  Everything the world planned — foundings, admissions, fills, resolutions, the
  failure walk, redemptions, compactions, retirements — is `unattempted` or
  `blocked`, each with the driver that owns it named.
- **The graded bases were not executed.** That degree-0 and degree-1 decode and
  settle is read off the codec and its tests, not driven; what was measured is
  that the local founding compiler emits neither.
- **Nothing was published.** The engine, the emitter, the decoder and the
  capture are committed so `git archive` can carry them; no deploy happened.

- **The founded market was never traded, resolved or retired.** It was opened
  and censused. Its four cells each carry 500,000,000 issued claims and its
  Hoard covers the worst of them exactly, which is L4 in a picture and is the
  whole of what happened to it.
- **No screenshot, no browser.** The v4 decoder is proven by its own tests, not
  by a rendered page. No surface draws a v4 document yet: the schema and the
  emitter exist, and the charts that would use `world.markets` are somebody's
  next piece of work.
- **RELAY-3's own tree was not modified.** `/var/tmp/relay3/` was read and copied
  from; the validator this lane ran is over a copy, on its own ports, and
  RELAY-3's preserved ledgers are byte-unchanged.
