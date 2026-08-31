# A real campaign, and what the site can honestly draw from one — 2026-08-30

**Lane** CAMPAIGN · step 3 of SIMVIZ's arc
(`docs/evidence/SIMULATOR_SERIES_VIZ_2026_08_30.md`) · **Artifacts**
`apps/dclutch-web/public/campaign-series.json`, `apps/dclutch-web/app/campaign`,
`apps/dclutch-web/scripts/campaign-series.mjs`

SIMVIZ ended with a sized handover: the journey tier needed one `ActivateFund`
step (~2h) and three `tier1/bindings.json` rows (~20min), and then a real
campaign could feed the site a moving series. Neither of those was the blocker.

## The journey tier is parked, and so is tier 1

`run-journey.sh` builds its run spec by shelling out to the journey binary's
`demo-market` subcommand. That subcommand is **retired at HEAD**:

```
demo-market is retired: a standalone registry address cannot authenticate the
checked local Direct deployment. Supply a market compiled by
dclutch-local-successor-bootstrap local-private-validator-market-v1 to `run`
```

`tools/gauntlet/run.sh` dies at the same boundary and says so out loud —
*"successor local campaign is parked at the retired demo-market boundary"* — so
this is not a journey-only gap. Direct is deployment-bound now: a market input
can only be compiled by `DirectMarketCompilerOwnedV1::load_local`, which
authenticates a checked local mutable plan against a gate on disk and observes
a **live** loopback deployment before anything compiles. The order the journey
needs is therefore

```
prepare the checked-mutable substrate -> boot a validator over it ->
run the administration campaign through activation -> compile the market
against the LIVE deployment -> found -> keep going
```

which is exactly what `tools/gauntlet/relayed-vertical/` already implements.
Rebuilding it inside `run-journey.sh` is the work; `ActivateFund` and three
bindings rows are the *rest* of the work, not the whole of it.

What was measured before that wall, at `3d259755`:

- the ELF stage builds **all seven roles with zero SBF frame diagnostics** —
  an independent confirmation of CORESTATE's fix from a gate neither lane
  configured;
- the journey binary compiles and runs. SIMVIZ's `dc4ad5d9` fix holds. The tier
  dies at step 4 of its runner, not at compile.

## What was driven instead

`tools/gauntlet/relayed-vertical` is journey-shaped by construction: it links
`journey/src/ledger.rs` by `#[path]` and threads the same seven-law
conservation ledger through every stage boundary, producing the same
`ObservationV1` census the series artifact already reads. It also closes
SIMVIZ's named gap for free — its founding's post-Open readiness suffix drives
**`CreateFund`, `ActivateFund` and `VerifyFundReady`** in one go, which is the
step the journey does not have.

## The walk refused, three times, and the refusal is the finding

Three independent campaigns were driven on hbox, each with its own freshly
prepared checked-mutable substrate, its own stated seed, and its own port
block — 31500, 31900, 32500. The three standing validators (26900, 27100,
29300) were read once and otherwise untouched; every validator these runs
started is gone.

| run | walk | seed preimage | verdict |
|---|---|---|---|
| 1 | success | `dclutch/campaign/relayed-vertical/2026-08-30/success` | refused |
| 2 | success | `…/success-2` | refused |
| 3 | failure | `…/failure` | refused |

All three at the same instruction, with the same code:

```
found the Market atomically: Lock, Found, Realize, Claims, Open (DCLTGMF3)
  Instruction 3 -> Core custom program error 0x3003
  = CoreSbfError::Reference, "Realm/Product/result-domain/Market identity
    linkage refused", 460,625 CU
```

Three things narrow it. The transaction immediately before — DCLTGMF3's own
hostile probe, *"refuses a substituted Claims request and rolls the whole
founding back"* — **succeeds every time**, so the account frame and the
five-role release set are sound; it is the real Open leg that refuses.
Custody's `TransferChecked` and `CloseAccount` inside the same instruction
succeed and return data; Core refuses after them. And every
`CoreSbfError::Reference` emission in the tree is in
`programs/dclutch-core-sbf/src/series_open.rs` — the occurrence, ticket,
capability-root and product-runtime identity joins.

**RELAY-3 got a green from this same walk, gate and revision 75 minutes
earlier, and that does not contradict this.** The first hypothesis was that
their run reused a warm substrate; it did not. `run2/substrate/plan.json` and
`run3/substrate/plan.json` have different digests and were written 34 minutes
apart, so run 3's substrate was as fresh as any of these. Same gate
(`story2-gate3`, `98fe157d…`, source `53354005`). The one remaining input that
differs is the **seed**, and the seed is what `local-mutable-prepare-v1`
derives the seven program identities from — which points at a key-varying
`find_program_address` path: one seed missed it and three hit it.

`53354005` predates `308c3dff` — *"the last two key-varying searches take a
relayed bump, and the count is zero"* — and `44972b78`, so this may already be
fixed on main. A checked release was therefore built at main HEAD and the
identical walk re-run against it, which answers *already fixed* against *live
red* with one measurement rather than an argument.

### The experiment at main HEAD, and the two things it found instead

A complete checked release was built at main HEAD — 13 fresh SBF links, build
freshness PASS, gate `fb4c3cd1…`, source `3d259755`, tree `11526552…` — and the
identical walk driven against it under a fourth stated seed. It answered a
different question than the one it was asked, twice.

**The relayed vertical does not compile on main.** Same tripwire class SIMVIZ
documented for the journey, one commit later in the day:

```
error[E0433]: cannot find `structured_market` in `crate`
  --> local-validator/bootstrap/successor/src/local_mutable.rs:1399:20
```

`local_mutable.rs` grew a fourth capability branch
(`DCLUTCH_MARKET_CAPABILITY=structured`) and calls `crate::structured_market`
from it; this tier links seventeen of the successor's modules by `#[path]` and
that was not one of them. It went off silently for the same reason the
journey's did — nothing in CI builds this tier either. Fixed in `38b4a73e` by
linking the module: the closure is finite, so it is one `mod` and not a
cascade, and forking the file would defeat the tripwire it exists to be.

**And then it refused before it ever reached a founding.** With the binary
building, the administration campaign stops at the activation stage:

```
Error: activation cache progress: ReleaseSetSelectionMismatch
```

So the `0x3003` question is **not answered**. At `53354005` the vertical founds
and refuses at Open; at `3d259755` it does not get as far as the founding at
all. Two different walls, and the second one hides the first.

## What the site draws, and whose run it is

**No campaign this lane drove produced a series.** The record published at
`apps/dclutch-web/public/campaign-series.json` is **RELAY-3's run 3** — the
relayed graduation market's success walk, on hbox, 2026-08-30, at
`533540056d61…`, on a loopback validator at `127.0.0.1:27300`. It is real
local-validator campaign evidence from this repository's own machinery, and it
is not this lane's run.

That provenance is not a footnote here; it is carried in the artifact
(`campaign.label`, `campaign.source_revision`, `campaign.rpc_origin`,
`campaign.transcript_file`), printed on the page in its own reading sentence,
and pinned by a test that asserts the label reaches the rendered markup. The
generator's `--check` re-derives the whole file from that transcript.

What it draws, all of it measured:

| figure | reading |
|---|---|
| odds, per cell | `5000` / `5000` basis points at all four boundaries — the founding's own distribution, unmoved because no fill has landed |
| claims issued | 500,000,000 on each of two outcomes |
| the vault | Hoard 500,000,000 against 1,000,000,000 tracked against a 1,000,000,000 Mint supply — exactly collateralised for either answer, which is L4 in a picture |
| work, per boundary | 191 → 0 → 7 → 1 transactions; 7,047,284 → 0 → 109,316 → 157,869 CU |
| spend | 0 → 0 → 33,412,560 → 33,487,560 lamports off the fee payer |
| settlement | cell 0 selected; one claim on it worth 1 collateral atom, one claim on cell 1 worth nothing |
| laws | 7 laws × 4 boundaries, none broken |

The flat lines are flat and say so; the moving ones move. That is the actual
answer to SIMVIZ's finding: a campaign record's motion is in the **work and the
phase**, not in the market's quantities, until somebody trades.


## The series schema, v3

`dclutch-simulator-series-v3` is v2 plus the four things a campaign record has
and a poller's census does not. Every field is optional; every v1 and v2
document still decodes, as a record that carries none of them.

| added | why |
|---|---|
| `stage` per point | "cycle 3" is not what happened there; "resolution funding active" is |
| `transactions`, `compute_units`, `fee_lamports`, `payer_lamports` per point | the only volume a market with no fills has is the work its stages cost |
| `claim_unit_atoms` | the price primitive: without it a claim count is a count of nothing in particular |
| `settlement` | which cell the terminal certificate selected — the only price move a market without fills ever makes, and the whole of one |

Three refusals are new and all three are the same species — a figure laid under
the wrong name:

- a `position_totals` list whose length disagrees with `outcome_count`;
- a `settlement` naming a cell the series does not have;
- a `campaign` block that will not say which source revision it ran.

## Two exactness defects this found in its own pipeline

**A u64 was silently rounded on the way in.** The campaign transcript writes
`payer_lamports` as a JSON *number*, and the first boundary of a run is
`500000009955591120` — larger than 2^53. Plain `JSON.parse` hands back
`500000009955591100`: a lamport figure wrong by twenty, in a pipeline whose
whole discipline is that quantities cross exactly. `campaign-series.mjs` now
reads each number's **raw source text** (`context.source`, Node 21+) and keeps
it as a string whenever the double cannot represent it. On a runtime with no
`context` it refuses loudly rather than rounding into the artifact.

**The claim unit was nearly guessed.** The relayed vertical's transcript has no
`claim_unit_atoms` field, and the first draft derived it as
`founding_admission.principal_atoms / worst outcome supply` — which on this
campaign gives exactly the right answer, `1`, because the two happen to
coincide. That coincidence is what would have made a wrong derivation
invisible. The unit is now read from the conservation ledger's own L4 sentence
— `Hoard H >= worst outcome W x unit U = H`, where `U` comes from the
Registry's published `ProductBasisV3.payout_scale` — and a run whose ledger
states more than one unit across its boundaries is refused rather than averaged.

## What the page draws, and the rule it enforces

`/campaign` is a second artifact and a second page on purpose. `/pulse` draws a
devnet census; this draws a local rehearsal. Folding them into one file would
put a reader one merge away from believing a local founding happened on devnet.

- **The odds path** — each cell's share of the market's own issued liability, in
  exact floored basis points on BigInt. It is captioned as what it is: the
  market's liability record, *not a price anyone paid*, because nobody bought
  anything in this run. A boundary with nothing issued draws **no odds line at
  all**, because a share of zero is undefined rather than zero.
- **The vault** — the Hoard against every tracked collateral atom against the
  Mint's whole supply. The Mint line is dropped entirely if any boundary did not
  record it; a line with a hole would be redrawn shorter than its own axis.
- **The work** — transactions, compute units and the fee payer's drawdown, on
  three separate figures because they are three dimensions. A transaction
  belongs to the boundary that could have seen it: at or before that boundary's
  finalized slot and after the previous one's. That is a partition, so nothing
  is double-counted and a boundary censused at the same slot as its predecessor
  honestly gets none.
- **The settlement** — stated per cell in a table, never drawn as a path. Two
  points is a settlement; a line through them would invent the shape between.
- **The law band** — the existing `LawBand`, with the stage each column is
  named underneath it.

**The demo-vs-product rule is enforced in code, not by convention.**
`campaignSeriesOrRefusalV1` refuses to draw any record whose `cluster` is not
`local` and any record that names no campaign, and says why on the page. The
local caveat is printed under **every figure** rather than once at the top,
because a reader who lands mid-page or screenshots one chart must still be told
what chain it came off. Both are pinned by test: the suite counts `<figure>`
elements and asserts the caveat appears at least as many times.

## How to reproduce the artifact

```sh
node apps/dclutch-web/scripts/campaign-series.mjs \
  --transcript ABS/transcript.json --evidence ABS/session-evidence.json \
  --label "…" --source-revision HEX40 --rpc-origin http://127.0.0.1:PORT/
```

`--check` re-derives the artifact from the same transcript and refuses if the
committed file disagrees, ignoring only `captured_at` — a drift check that
always fails is a check nobody runs. `--out` writes somewhere else, so the
script can be exercised against a transcript that is not the one published.

## What was NOT verified

- **No devnet or mainnet write of any kind.** Every transaction in this lane
  was submitted to a loopback validator this lane started and then stopped.
  The three standing hbox validators (26900, 27100, 29300) were read once —
  a directory listing and a `getSlot` — and nothing else.
- **Nothing was published.** The artifacts are committed so `git archive` can
  carry them on the next cut; no deploy happened.
- **No screenshot.** No headless browser here. The page was verified by
  rendering it to static markup and reading the result — every figure, every
  caption, every caveat — not by eye. The first thing worth doing with a
  browser attached is looking at the odds band beside the work bars.
- **The `0x3003` refusal is localised, not diagnosed.** It is pinned to
  DCLTGMF3's Open leg and to `series_open.rs`'s identity joins by the error
  class and by which transactions succeeded around it. Exactly which join
  refuses was not determined: the validators were torn down with their
  campaigns, and reading the ticket, occurrence and capability-root state at
  the refusing slot needs a run held open at that point.
- **The seed hypothesis is the best remaining explanation, not a proof.** Three
  seeds refused and one — another lane's — did not, on the same gate, revision
  and walk, with substrates of equal freshness. That makes the derived program
  identities the differing input; it does not identify the bump search.
- **`sbomVerify` is red and was red before this lane.** Suite after this lane:
  129 files passed, 1 failed (`lib/sbomVerify.test.ts`), 991 tests passed.
- **The journey tier still has never run a campaign.** Its ELF stage is green
  at seven roles and zero frame diagnostics, and its binary builds and runs —
  it dies at `run-journey.sh`'s market-input step, which is upstream of both
  gaps SIMVIZ sized.
- **`ActivateFund` was never driven by this lane either.** The relayed
  vertical's founding readiness suffix drives it, and that is why this tier was
  the right one to pick — but no founding of this lane's ever completed, so
  what this lane executed of that ladder is nothing.
- **The published record is not this lane's run.** Four campaigns were driven
  and none founded. See "whose run it is" above; the artifact names its own
  provenance and the page prints it.
