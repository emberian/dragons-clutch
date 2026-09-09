# dClutch

dClutch is a Solana protocol for markets with fully funded payouts. A market
defines possible outcomes, how to select the result, and what each claim pays.
For a price-range market, you buy claims on the range you expect. When it
resolves, winning claims pay one collateral unit each; other claims pay zero.

Collateral is held in the market’s vault, called the **Hoard**. Trading fees,
account rent and resolution funding are kept separate from that collateral.

[Open dClutch](https://clutch.dregg.pro) to browse markets, inspect accounts,
and use the console. Devnet markets use test tokens.
[Read the guide](docs/guides/reader.md) for a worked trade and payout example.

## Development status

The public deployment is cohort 17, with eight programs on Solana devnet.
It has executed market creation, a Direct trade, resolution, payouts and
market retirement. Current development is preparing a fresh deployment and a
running simulator that visitors can watch and join.

Recent local-validator work includes:

- **Direct trading:** three nonzero fills, their fee payments, and recovery
  after restarting the simulator.
- **Dealer liquidity:** a trade, withdrawal, settlement, redemption and closure
  of the Dealer accounts. Multiple-provider liquidity is being connected to
  the CLI and browser.
- **General clearing:** accepted Buy orders at two and three outcomes. Work
  continues on Sell orders, wider markets and the complete clearing cycle.
- **Structured claims:** receipt activation has run; coordinate activation is
  being repaired.
- **Series:** recurring-market setup and transaction drivers are in progress.

The [development index](GOAL.md) links to detailed results and open work.
The next deployment will use fresh program addresses from one committed build.

## Tools

- **[Web app](apps/dclutch-web):** market discovery, explorer, portfolio and
  wallet actions. Open a market to trade or redeem; use the console for setup
  and operations.
- **[TypeScript SDK](packages/dclutch-sdk):** account decoding, transaction
  construction and client helpers.
- **[CLI clients](docs/guides/two-clients.md):** `dclutch` reads markets and
  authors offers; `dclutch-terminal` provides transaction workflows. Each
  command lists its inputs and supported actions.

## How a market works

A price-range market has four main steps.

1. **Someone creates it.** The creator fixes everything up front: the
   collateral token, the question and its cells, the price source, the
   resolution time window, and a fallback outcome in case the source goes
   silent. None of it can change afterwards, and the creator keeps no
   special powers over the live market.
2. **People trade claims.** Depositing one collateral unit mints one claim
   on every cell (a **complete set**); returning a complete set redeems
   the unit. A complete set can be redeemed for one collateral unit.
3. **The source resolves it.** The first valid observation from the pinned
   source inside the market's window settles the market. Every later
   observation is rejected. The market applies the source and comparison rules fixed at creation.
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
