# dClutch

dClutch is a Solana protocol for markets on real-world numbers — where a
price will be at a stated time, for example. A market splits the possible
answers into buckets called **cells**. You buy claims on the cells you
believe in; when the market resolves, each claim on the winning cell pays
out one collateral unit, and every other claim pays zero.

Every claim is backed by collateral locked in the market's vault (its
**Hoard**) before the claim exists. There is no leverage, so there is no
liquidation, no margin call, and no way for a market to owe more than it
holds. The most you can ever lose is what you paid.

The recorded cohort 17 contains eight programs on Solana devnet. A new
**cohort** is a full redeploy from one named commit with fresh program ids;
the previous cohort is abandoned in place. The ids are not permanent. The
site reads markets from its selected deployment
([clutch.dregg.pro](https://clutch.dregg.pro)) and by the SDK's deployment
manifest (`packages/dclutch-sdk/lib/deployments.ts`); each cohort's evidence
is a dated document under [`docs/evidence/`](docs/evidence). Nothing on
devnet is worth money: it is a public test network, and every market's
collateral is a devnet test token. Current source is newer than that recorded
deployment. The local execution work below must finish before the next full
redeploy and public simulator launch.

## What works today

Evidence levels are distinct and the list says which one each item has:
*devnet* means a finalized public-chain transaction named in a cohort
document; *local validator* means transactions executed on an owned test
chain; *SBF program test* means program bytes executed in a test bank. Native
component tests establish only their named semantics. A successful build does
not establish any of these execution results; none is mainnet evidence.

- **Founding, on devnet** — a market is projected, funded, founded and opened,
  by the composed transaction or by the two-stage permit route below.
- **Trading, on devnet** — strangers are admitted, a stranger's fee-bearing
  Direct fill has crossed, and the fee leg was settled permissionlessly by a
  third party in its own transaction.
- **Resolution, on devnet** — the sponsored Pyth SOL/USD feed was captured
  inside a market's window and the market settled on an honest certificate;
  a market whose source went silent took its disclosed fallback by the
  failure walk; a market with a funded recovery ladder is answered on its
  second rung (SBF program test).
- **Payout, on devnet** — winning claims, a stranger's included, were paid
  into ordinary wallet token accounts. A cohort-17 market completed all four
  retirement checkpoints: its Market, Hoard and related accounts closed, and
  the recorded rent refund matched the balances. See the night addendum in
  [the cohort-17 evidence](docs/evidence/COHORT17_SEATED_FILLED_RETIRING_2026_09_06.md).
- **Dealer, on a local validator** — a founded market accepted Quote, a
  nonzero nine-unit Fill and a one-unit Withdraw, with claim supply and
  collateral movements checked. This [accepted campaign](docs/evidence/DEALER_ACCEPTED_LOCAL_VALIDATOR_2026_09_08.md)
  does not establish terminal settlement or a public Dealer market.
- **Optional families, at distinct stages** — General has founding and
  OpenBatch execution, with the first nonempty complete lifecycle still owed.
  Fractional claims have [local-validator Wrap, holder transfer and WholeUnwrap](docs/evidence/claims-fractional-validator-2026-09-08/README.md)
  evidence. Series has [native recurrence tests](docs/evidence/SERIES_NATIVE_RECURRENCE_2026_09_08.md).
  Complete current-source General, Dealer, Series and Structured lifecycles
  remain active work.
- Once the setup transactions have finalized, founding locks the collateral,
  creates the market, and opens it for trading. There are two routes to that
  outcome. The composed route does all of it in a single transaction that
  either commits whole or rolls back, leaving nothing half-made. Because that
  transaction runs at the edge of Solana's compute limit, there is also a
  two-stage route: the first transaction commits the market and escrows a
  one-shot permit, and a second, permissionless transaction consumes the
  permit to open the market. The permit is what makes the outcome
  all-or-nothing across the two transactions — the market cannot open on any
  terms but the ones the first stage already committed to, and the escrow has
  a pinned refund path so no value can strand between the stages.
- Range and tail protection ("pays out if SOL ends below X") is just a
  bundle of cell claims, so its price is exactly the sum of the cell
  prices. No extra machinery, nothing to liquidate.
- The web app ([`apps/dclutch-web`](apps/dclutch-web)) reads markets,
  supplies and portfolios from the chain. Its detailed Market page can join
  an eligible wallet and sign and submit a Direct fill; Console `/trade` is a
  read-only arithmetic inspector. Its Representation console can
  authenticate and transfer an ordinary Token-2022 bearer claim on a compatible
  local or custom chain, including separate transfer-authority and fee-payer
  wallets, one saved send, and a finalized balance check. No current devnet
  market supplies that selected representation route. The app publishes no
  authoritative prices; there is no independent indexer.
- A TypeScript SDK ([`packages/dclutch-sdk`](packages/dclutch-sdk)) and two
  command-line clients build and check the same flows
  ([two clients](docs/guides/two-clients.md)): `dclutch` reads and authors
  tickets and never submits; `dclutch-terminal` founds, joins and redeems
  under a durable journal, while its `buy`, `sell` and failure-walk
  submission still refuse by design.

The current work is to complete every retained family's accepted economic
lifecycle on a local validator, including meaningful refusal and rollback
cases, and then repeat them against one exact build of all eight programs.
General's composed transactions currently encounter Solana's transaction
compute ceiling; removing repeated work is part of that implementation task.
The next deliverable is a fresh devnet cohort, a running load simulator that
visitors can watch and join, and the renovated existing site. An older
successful transaction does not make newer source a checked deployment.
The [development wave](docs/design/DEVELOPMENT_WAVE_2026_09_07.md) and
[completion contract](docs/MASTER_COMPLETION_CONTRACT.md) describe the scope;
[GOAL.md](GOAL.md) indexes the dated execution evidence.

## How a market works

This is the design, end to end. All four steps have run on devnet.

1. **Someone creates it.** The creator fixes everything up front: the
   collateral token, the question and its cells, the price source, the
   resolution time window, and a fallback outcome in case the source goes
   silent. None of it can change afterwards, and the creator keeps no
   special powers over the live market.
2. **People trade claims.** Depositing one collateral unit mints one claim
   on every cell (a **complete set**); returning a complete set redeems
   the unit. Cell prices always sum to exactly one unit.
3. **The source resolves it.** The first valid observation from the pinned
   source inside the market's window settles the market. Every later
   observation is rejected. No committee, no vote, no discretion.
4. **Winners redeem.** Claims on the winning cell pay one unit each, from
   the collateral that was there the whole time.

If the source never publishes inside the window, the market takes the
fallback outcome the creator disclosed before it opened — the **failure
walk**, which anyone may submit. The public CLI still only previews that
transaction; the operator tooling has submitted it on devnet.

A transaction that doesn't check out exactly — wrong account, wrong
authority, stale state, a window that isn't open — is **refused**: the
whole transaction rolls back and your funds stay where they were. Every
refusal carries a code naming the program that refused and why; the full
list is in [the refusal reference](docs/reference/refusals.md).

**Who gets the trading fee: the market, and never the protocol.** A Direct
market's rate is fixed when it is created, immutable after, and charged per
side — 50 basis points takes 50 from each side, so 1% of the gross moves on a
fill. All of it goes to that market's own `fee_recipient`, a pubkey the founder
named at creation, delivered by an ordinary token transfer anybody may submit.
There is no protocol treasury, no protocol beneficiary and no instruction that
lets the protocol sweep a market's fees. The rate may be anything from zero up
to 500 basis points a side and no higher; the ceiling is in the deployed
program, not in a setting. See [the trader guide](docs/guides/trader.md) and
[decision 0014](docs/decisions/0014-the-fee-rate.md).

## The eight programs

The current cohort contains eight on-chain programs, each with a distinct job.
A market names the exact program releases it uses when it is created, and
that set never changes.

| Program | Job |
|---|---|
| [`dclutch-core-sbf`](programs/dclutch-core-sbf) | the market itself: creation, phase, opening |
| [`dclutch-claims-sbf`](programs/dclutch-claims-sbf) | claims: minting, complete sets, settlement |
| [`dclutch-trading-sbf`](programs/dclutch-trading-sbf) | trade execution |
| [`dclutch-custody-sbf`](programs/dclutch-custody-sbf) | collateral custody: the Hoard vault |
| [`dclutch-resolution-proof-sbf`](programs/dclutch-resolution-proof-sbf) | resolution: source observations, windows, the fallback |
| [`dclutch-registry-sbf`](programs/dclutch-registry-sbf) | which program releases a market may use |
| [`dclutch-rent-sbf`](programs/dclutch-rent-sbf) | account rent over a market's life |
| [`dclutch-accelerator-sbf`](programs/dclutch-accelerator-sbf) | stateless General, Dealer and Series computation called by Trading |

Test callers and harnesses live beside the program sources; they are not
additional deployed protocol roles.

## Finding your way around

- [`docs/guides/`](docs/guides) — start here: guides for traders, market
  operators, and anyone deciding what this is.
- [`docs/reference/`](docs/reference) — the protocol reference, generated
  straight from the code: every instruction, every error code with its
  meaning, compute costs, byte layouts.
- [`crates/`](crates) — the Rust contracts and kernels the programs share.
- [`formal/`](formal) — the Lean definitions that generate the record
  layouts and wire formats used by both the chain and the web app.
- [`tools/gauntlet/`](tools/gauntlet) — the campaign runner: builds the
  programs and enumerates every route they accept. Founding a market on a
  local chain and walking it lives in the `ladder` gauntlet tier,
  [`tools/gauntlet/ladder/`](tools/gauntlet/ladder).
- [`apps/dclutch-web`](apps/dclutch-web) — the web app.
- [`docs/decisions/`](docs/decisions) — why the architecture is the way it
  is.
- [`docs/INTENT.md`](docs/INTENT.md) — why the project is the way it is: what
  it is for, the design values and the boundaries, in the founder's own words
  with the provenance of each. A draft awaiting his edit.

## The artifacts, and where they come from

The tools and consoles pass a handful of artifacts between them. Every one
has exactly one producer:

| Artifact | Made by | Lives | Used by |
| --- | --- | --- | --- |
| Checked release (per program + the multiprogram evidence) | `tools/release` (the checked-release pipeline) | `release/` build output | the deploy runbook; the web Console's activation page |
| Deployment plan (`plan.json` + genesis accounts) | the bootstrap producer (`tools/local-validator/bootstrap/successor`) | your work directory | the campaign driver; validator launch |
| Finalized records (products, sources, configs) | published on chain by the campaign driver | **on the chain** — fetch by address, never paste | every program; the web app reads them live |
| Market spec (`run-spec`) | you, via the operator/spec producer | your work directory | founding; `/create` is a read-only preview |
| Keypairs | `solana-keygen` (or the driver's per-role forge) | files you keep | signing; the address a keypair file prints is the one you fund |
| Relay publication log | the relay daemon | [publication_log.jsonl](https://portal.dregg.studio/relay/publication_log.jsonl) | anyone checking the operator is alive |
| Evidence documents | each campaign, as it runs | [`docs/evidence/`](docs/evidence) | humans; the reference site |

If a console asks you to paste something and you don't know where it comes
from, that's a bug in the console — this table is the answer key.

## Try it

```sh
# build the programs and enumerate every route they accept (no chain):
tools/gate census

# found a market on a throwaway local validator and walk it: the `ladder`
# gauntlet tier stands the validator up, founds, opens and completes (the
# private-validator lifecycle runner it replaced was deleted on 2026-09-04;
# SIMPLIFY_DRIVERS.md §3 says why). It needs a checked release gate to bind to:
tools/gauntlet/ladder/run-ladder.sh --checked-release-gate /absolute/checked/CHECKED_UPGRADE_GATE.json

# the web app's test suite:
cd apps/dclutch-web && npm test

# the web app, served locally:
cd apps/dclutch-web && npm run dev

# workspace checks:
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# the clippy line above, as a gate rather than a habit -- it judges every
# workspace member against tools/gates/clippy-debt.tsv and says how many it
# never reached:
tools/gate clippy
```

After anything under `packages/dclutch-sdk` moves — the deployment manifest
above all — delete `apps/dclutch-web/node_modules/.vite` before trusting what
`npm run dev` shows you: Vite pre-bundles the package and a cache from before a
cohort redeploy serves the browser a *dead* cohort's program ids, which the site
then reports as an honest refusal on a market that is perfectly healthy.

Working on the code itself? Read [`AGENTS.md`](AGENTS.md) first — it carries
the rules this repository runs on — and [`GOAL.md`](GOAL.md), the index of
what the project is, the standing goal and every dated delta.

## Where this is going

The dated milestones — the first devnet fill, the first honest resolution, the
first stranger paid — are rows in [`GOAL.md`](GOAL.md), each linking to the
cohort document that carries the signatures. The mechanism agenda
([decision 0031](docs/decisions/0031-the-mechanism-agenda.md)) is the design
layer under construction: the frequent batch as every family's clearing
spine, joint clearing across outcomes, a bounded-loss scoring-rule Dealer,
ensemble resolution, the founder bond, and conditional markets. Pyth's devnet
feeds carry the major prices directly, and a disclosed relayer
(`tools/relayer`) carries mainnet account state for everything else. dClutch
grew out of Dragon's Clutch; the first generation lives in the neighboring
`dragons-clutch` repository as an archive.
