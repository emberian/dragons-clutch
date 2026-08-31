# A population with hands: every route with a shipped driver, driven — 2026-08-30

**Lane** SIMLIFE-2 · **Artifacts** `tools/load-simulator/simlife_drivers.py`,
`simlife_drive.py` (`LifecycleSubstrate`), `simlife.py` (the band draw),
`tools/local-validator/bootstrap/successor/src/market.rs`
(`LocalMarketShapeV1`), `apps/dclutch-web/components/PopulationWorkspace.tsx`,
`apps/dclutch-web/app/population/page.tsx` · **Substrate** SIMLIFE's restarted
loopback rehearsal chain at `127.0.0.1:34500`, source revision `53354005`

## What this lane found and what it changed

SIMLIFE built an engine that schedules a whole world and a substrate that could
only watch: every mutation event in its published run was `unattempted` or
`blocked`, and the artifact said so honestly. Three things were in the way and
two of them are gone.

| wall | state |
|---|---|
| `0x5182` at the atomic founding Open leg | fixed by FOUND-5182 (`a7e2f668`) |
| the post-Open funding-readiness suffix | fixed by SUFFIX (`9941a4e4`) |
| the founding compiler emits ONE market shape | **fixed here** (`b68ac693`) |
| only `CategoricalQ1` is foundable | still open, protocol tier, out of scope |
| claim-check compaction has no driver anywhere | still open, **sized** below |

## The compiler's one shape, widened

`demo_market_input_base` hard-coded `cuts = [12_000, 18_000]`,
`cut_denominator = 100`, `coefficients = [1, 0, 1, 0]`,
`initial_collateral_atoms = 1_000_000_000` and a 300-second terminal window, so
every market this repository could found on a local validator was the same
market. `LocalMarketShapeV1` makes those six knobs a parameter with the old
values as its `Default`, and `local-private-validator-market-v1` grows six
optional flags.

**Every existing caller compiles the same market it always did.**
`demo_market_input` and `demo_market_input_base` keep their signatures and
delegate to a shaped form; the four family compilers (Direct, General, Rational,
Structured) take the shape through.

Measured on the live deployment at 34500, compiling only:

| shape | result |
|---|---|
| default (no flags) | 4 outcomes, cuts `[12000, 18000]`, 1,000,000,000 atoms |
| `--cuts 15000 --coefficients 1,1,0` | 3 outcomes |
| `--cuts 9000,14000,21000 --coefficients 1,0,0,1,0` | 5 outcomes |
| `--cuts 5000,9000,14000,21000,30000` + 7 coefficients | 7 outcomes |
| `--cuts ""` + 2 coefficients | **2 outcomes** |
| `--initial-collateral-atoms 2500000000 --generation 7` | collateral and generation move |
| `--cuts 9000,14000,21000 --coefficients 1,0,0` | refused: *"3 cuts describe a 5-outcome market … so it needs 5 coefficients and 3 were given"* |
| `--cuts 21000,9000` | refused: *"cuts must be STRICTLY increasing…"* |

**NO CUTS is legal and is the narrowest market this compiler can reach** — the
whole coordinate domain as one region plus the explicit failure outcome, two
cells. It is the only way to reach a two-cell market here, and it was settled by
compiling and founding one rather than reasoned about.

**The claim unit is deliberately NOT a flag.** It is not a `MarketRunInput`
field at all: `compile_linked_basis_v3` hard-wires `payout_scale: 1` beside the
categorical basis kind, so varying it is the same edit as emitting a graded
basis and belongs to whoever does that one.

## The driver layer, and the rule it exists to keep

`simlife_drivers.py` is one function per route and each one is a **subprocess
calling the shipped driver that owns the route**. Nothing in it builds a
transaction, derives a PDA, or copies a constructor.

> That is FOUND-5182 stated as a module boundary. The founding driver's own
> hand-written copy of a kernel constructor drifted by three bytes —
> `StateBumpsV1::UNRECORDED` against real bumps — and every local founding
> refused `0x5182` for a day while the "independent" control passed. A mirror of
> shipped code is a bug with a delay fuse.

Three things the module computes for itself, each a READ of something already on
disk or on chain, each pinned by test:

- a keypair file's public half is its second thirty-two bytes (a file of the
  wrong width is refused, never truncated);
- base58, with leading zero bytes as ones;
- **the founding's own frozen DCLTGMF3 routing table**, found the way the chain
  describes it.

## Four things this substrate taught, by running

**The admission packet does not fit a legacy message.** It routes through the
founding's OWN frozen DCLTGMF3 lookup table; SEL-SEAM measured that passing all
five founding tables refuses `DuplicateAddress`, and named as residue that the
founding campaign does not record the frozen table's address in its evidence. It
does not have to: a frozen table is one whose **authority is `None`**, and the
founding's own is the frozen table whose **address list contains the market**.
Both facts are already on the chain.

**The position owner's wallet must exist and be funded**, at FINALIZED
commitment rather than confirmed, or the driver refuses `snapshot missing
required account` before it compiles anything. An admission is over a wallet,
and a wallet nobody has ever paid is not one.

**A partially consumed founding key set is unresumable.** A retry is a whole new
key set in its own `attempt-NN/` directory, and the abandoned attempt stays on
disk under its own name. About one founding in three hits a finalization
transient somewhere in its ~85 transactions — *"…did not reach finalized
transaction history"* — and must be re-walked from fresh keys.

**A local market is past its terminal boundary the instant it is founded.** The
window ENDS at the captured Pyth fixture publication, which is in the past on
every local chain, so `--terminal-window-width-seconds` is how far back the
window reaches rather than how long anybody waits.

**And the bootstrap binary SIMLIFE ran was stale against its own source tree.**
Built 12:48 from a tree at `53354005`, it lacked the `--routing-table` flag that
`8f10beb9` added — which is why the first admission attempt refused
`PacketTooLarge` with no flag to fix it. A rebuild of the same tree took 91
seconds. A binary is not a revision.

## The run

Seed `dclutch/simlife2/2026-08-30/hands-144` (`5174e29b146e`), 6 markets over 26 ticks at `slots_per_tick` 900, driven by the `lifecycle` substrate against `http://127.0.0.1:34500/`.

**The world it drew.**

| | |
|---|---|
| archetypes | `tent-band` ×2 · `short-fuse` ×1 · `coin-flip` ×1 · `wide-field` ×1 · `quiet-corner` ×1 |
| destinies | resolves-clean ×5 · founded-then-sleepy ×1 |
| personas | `sleeper` ×6 · `prompt-redeemer` ×5 · `eager-maker` ×3 · `compactor` ×3 · `patient-maker` ×3 · `crank` ×1 |
| outcome widths | 2, 3, 5, 6 |
| collateral | m00 24,957,844 · m01 754,399,349 · m02 566,027,375 · m03 928,858,834 · m04 5,073,807,456 · m05 252,329,312 |
| events | 217 in total: 139 census · 28 fill · 21 admit · 9 redeem · 6 found · 6 compact · 4 resolve · 4 retire |

**What it did.**

| route | executed | refused | unattempted | blocked |
|---|---:|---:|---:|---:|
| found | **4** | 0 | 2 | 0 |
| admit | **12** | 0 | 0 | 9 |
| fill | 0 | 21 | 0 | 7 |
| resolve | 0 | 2 | 0 | 2 |
| redeem | 0 | 0 | 0 | 9 |
| compact | 0 | 0 | 6 | 0 |
| retire | 0 | 0 | 0 | 4 |
| census | **87** | 0 | 0 | 52 |

**Markets this run founded itself: 4** — `m00` (short-fuse, 3 cells, 24,957,844 atoms), `m03` (coin-flip, 2 cells, 928,858,834 atoms), `m04` (wide-field, 6 cells, 5,073,807,456 atoms), `m05` (quiet-corner, 2 cells, 252,329,312 atoms).

**87 censuses executed across 4 markets, 510 conservation checks held, 0 broke**, 99 did not apply.

**Every reason it gave, grouped.**

| route | ending | count | the substrate's own sentence |
|---|---|---:|---|
| census | `blocked` | 26 | m01 asks for a basis this substrate cannot express, so it was never founded and nothing downstream of it happened |
| census | `blocked` | 26 | m02 asks for a basis this substrate cannot express, so it was never founded and nothing downstream of it happened |
| compact | `unattempted` | 6 | claim-check compaction by a stranger has NO CLI anywhere. It is implemented and green in ProgramTest (programs/dclutch-claims-sbf/tests/claim_check/mod.rs, sixteen tests including a_market_retires_a_sleeping_holders_position_and_the_hold… |
| fill | `blocked` | 6 | m01 asks for a basis this substrate cannot express, so it was never founded and nothing downstream of it happened |
| admit | `blocked` | 5 | m02 asks for a basis this substrate cannot express, so it was never founded and nothing downstream of it happened |
| admit | `blocked` | 4 | m01 asks for a basis this substrate cannot express, so it was never founded and nothing downstream of it happened |
| redeem | `blocked` | 3 | m01 asks for a basis this substrate cannot express, so it was never founded and nothing downstream of it happened |
| found | `unattempted` | 2 | a tent is two degree-1 ramps and reaches the wire the same way a ramp does -- through GradedExactComplement, which the local founding compiler never emits (market.rs:1683) |
| resolve | `refused` | 2 | provisioning the resolution tables: Error: Error("authority keypair public key 66LVJypEwxMAMJwsoGAyUSQZDyK5uGH5BBaYUpiddQ4b differs from authenticated input EahMVb2ptYeDKia4uTbmQHsnqT7B8kmGz2xEMBvP1F6N") |
| redeem | `blocked` | 2 | m03 never reached a terminal answer, so there is nothing to collect and nothing to clean up |
| redeem | `blocked` | 2 | m00 never reached a terminal answer, so there is nothing to collect and nothing to clean up |
| redeem | `blocked` | 2 | m02 asks for a basis this substrate cannot express, so it was never founded and nothing downstream of it happened |

The remaining rows are one per refused fill, each naming its own subject; the artifact carries all of them.

**The four executed foundings are four SHAPES, and that is the point of the
widening.** `m03` and `m05` are two-cell markets with no cuts at all — a width
this compiler could not emit yesterday; `m00` is three cells; `m04` is six, over
four cuts and 5,073,807,456 collateral atoms. Before `b68ac693` a run that drew
those four would have founded four identical four-cell markets at
1,000,000,000 atoms.

**And the tick was measured, not assumed.** The plan assumed 900 slots a tick
and the run measured 78, 79, 81 and 84 across its four markets. That gap is
real and is reported rather than reconciled: a tick in this run is one pass of
the conductor, and a conductor that spends five minutes founding a market and
one second censusing four of them does not have a constant tick. The number that
was checked is the one the plan used to decide which markets could reach a
deadline, and it was wrong by an order of magnitude in the direction that makes
FEWER markets terminal — which is why every `redeem` and `retire` below is
`blocked` behind a terminal answer that never came.

### Three walls the run found, in the order it hit them

**Fills: `REFUSED: Direct root owner or width changed`.** The Direct capability
closure a market carries does not follow the widened market's width, so a
six-cell market's Direct root does not match what the trade producer
authenticates. This is the third wall behind the compiler's one shape, after
`0x5182` and the fixture-receipt collateral pin, and it is the next one to fall.
Two fills refused for a different and more ordinary reason —
`signature … has not reached finalized history` — which is the same finalization
transient the foundings hit.

**Resolution: the three typed tables need the FOUNDING FOUNDER, funded.** The
provisioner refuses "authority keypair public key … differs from authenticated
input …", and once given the right identity refuses again with "Attempt to debit
an account but found no record of a prior credit" — a protocol identity the
founding creates and nobody ever pays. With both fixed, and with the local Pyth
update account provisioned by
`local-private-validator-pyth-vaa-provision-v1 --execute` (**nine journaled
actions, executed**), the three typed lookup tables create, extend and freeze.
What has NOT been reached is the producer's second pass: it writes the flagship
input only after a fresh finalized snapshot proves all three tables frozen, and
a re-produce over the same checkpoint refuses "producer checkpoint immutable
Market, authority, or typed table plan changed" while a fresh checkpoint plans
its own new tables. That is where the resolution route stands, four steps
further along than it started.

**Redemption and retirement are `blocked`, not refused**, and the distinction is
the whole vocabulary: nothing terminal happened to any market, so there was
nothing to collect and nothing to clean up. Their drivers were never asked.


## Series v4, and the surface that draws it

SIMLIFE's v4 schema said plainly that no surface drew one: *"the charts that
would use `world.markets` are somebody's next piece of work."*

`/population` is that page, and it draws four things:

1. **Every observed market's odds path**, one small chart each. These markets
   are contemporaries — censused at the same ticks — so the paths share a scale
   and are comparable, and a flat one is a market nobody traded rather than a
   market nobody watched.
2. **The run's own event timeline**, three lines and not one: mutations that
   landed, mutations the chain refused, markets censused. A run that founded
   four markets must not render identically to a run that failed four foundings
   and censused a lot.
3. **The honesty strip**: every route with its four endings in four columns,
   never summed, each row's commonest reason printed in the substrate's own
   words underneath.
4. **What the world drew**, including the archetypes no compiler here can found.

The emitter grows one block, `world.timeline`, and the decoder reads it plus the
`world.tally` the emitter already wrote. **The timeline is optional and its
absence is not a defect**: every capture taken before it existed is still a
complete v4 document and decodes to an empty timeline, which the page says in
words rather than drawing an empty axis. What IS refused is a tick whose four
outcome counts do not add up to its own total — the
caption-disagrees-with-its-chart species, one level down.

`/campaign` is untouched and still draws exactly what it drew.

The published capture is `apps/dclutch-web/public/simlife-series.json`, 114,375
bytes, sha256 `5add286d6757…`, and it is pinned by test: it must decode as a
population, every planned market's `observed` flag must agree with what the
document actually carries, a run that says it founded markets must have observed
every one of them, and no capture may claim a substrate with nothing absent —
compaction has no driver, so something is always absent.

## Claim-check compaction: sized, not built

There is no CLI anywhere — not in the successor binary, not in the gauntlet, not
in the SDK — and this lane did not become the first one, because a compaction it
built by hand would be exactly the mirror the driver layer exists to avoid. It
is implemented and green in ProgramTest
(`programs/dclutch-claims-sbf/tests/claim_check/mod.rs`, sixteen tests including
`a_market_retires_a_sleeping_holders_position_and_the_holder_is_still_paid`) and
named by no gauntlet binding, so it is covered and census-unbound.

**The estimate.** One new `local-private-validator-claim-check-compaction-v1`
subcommand shaped like `…-wallet-terminal-payout-v1`: read the holder's claim
check and the market's terminal receipt, build the one Claims instruction the
ProgramTest already calls, journal the packet before the send, verify the
poststate. The ProgramTest gives it its own oracle.

- **6–10 hours** for the subcommand, its argument parser, its journal domain,
  and one hostile test that a holder cannot compact their own check.
- **plus 1–2 hours** for the gauntlet binding, since an unbound transaction
  fails the census.
- Driver tier, not protocol: **no program changes**.

## What was NOT verified

- **No devnet or mainnet action of any kind, and no devnet read.** Every
  transaction and every read went to the loopback validator SIMLIFE started on
  port block 34500/34502/34503. `~/.helius-key` was not read. Nothing of this
  lane touched 26900, 27100, 29300 or 33500.
- **Nothing was deployed or published.** The engine, the drivers, the compiler
  change, the page and the capture are committed so `git archive` can carry
  them; no site deploy happened and no screenshot was taken. The page is proven
  by its own tests rendering the committed capture, not by a browser.
- **The widened compiler was executed on a `53354005` substrate, not at HEAD.**
  The chain on 34500 carries `53354005` programs, and HEAD's founding driver
  records PDA bumps that a Core from before `e93fe5e9` does not — so a HEAD
  binary against this chain would refuse for a reason that is about the pairing
  rather than about the shape. The binary that ran was built from a copy of that
  tree with two transplants: this lane's `LocalMarketShapeV1` hunks and SUFFIX's
  `9941a4e4`, both extracted from the landed diffs by script and matched
  EXACTLY, never fuzzed. `cargo check` is green over the shipping tree at HEAD;
  what is NOT claimed is that a HEAD binary was run against anything.
- **`--terminal-window-width-seconds` was exercised but its effect was not
  observed.** The window ends at the captured fixture publication either way, so
  what a wider window changes is not visible in a census of an Open market.
- **The resolution was NOT executed.** Its first three prerequisites were: the
  local Pyth update account is provisioned (nine journaled actions), the table
  authority is the founding founder and it is funded, and the three typed
  lookup tables create, extend and freeze. The flagship input itself was never
  emitted, so no certificate, no terminal answer, and therefore no redemption
  and no retirement. Everything downstream of it in the run is `blocked` and
  says why.
- **The Direct trade was NOT executed.** The producer refuses `Direct root owner
  or width changed` over a widened market. That refusal is a measurement of the
  same class as the two walls this lane fixed, and it is not diagnosed here.
- **The graded bases were not executed and were not meant to be.** Two of the
  world's six markets refuse at their own draw site because
  `compile_linked_basis_v3` hard-wires `CategoricalQ1`; that is finding #1 of
  SIMLIFE's evidence and a protocol-tier lane, explicitly out of scope here.
- **Claim-check compaction was neither driven nor built.** It is sized above.
- **No `cargo nextest` suite was run.** The control for the compiler change is
  `cargo check` green plus the compile matrix above, measured against the live
  deployment — the narrowest thing that could refute it. The Python engine is
  67 tests; the web is 1,061 passing against the tree's three pre-existing
  ABI/SBOM reds.
- **RELAY-3's and SIMLIFE's own trees were not modified.** `/var/tmp/relay3` was
  never written; `/tank/dregg-build/story2-src` was copied, never edited; this
  lane's build tree is `/tank/dregg-build/simlife2-src` and its work directories
  are under `/var/tmp/dclutch-simlife2`.
