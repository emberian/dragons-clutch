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

**dClutch is not live yet.** There is nothing to buy and no value at
risk anywhere. As of August 27, 2026 the seven programs are deployed to
Solana's devnet — a public test network whose tokens are worthless by
construction — each verified byte-for-byte against its source build
([the deployment record](docs/evidence/DEPLOY_1.md)). The first devnet
markets are being set up now; everything below also runs on a local test
chain you can run yourself.

## What works today

- On a local test validator, a market is created, funded, opened, and
  resolved on chain, after which it begins winding down. Resolution runs
  through the same Pyth and Wormhole programs that are deployed on
  mainnet, and they really do check the signatures — but the price they
  check is a recorded one signed by a test key, not a live Pyth
  publication.
- Trading between two counterparties, moving claims between holders, and
  paying out a winning claim run against those same programs in a test
  harness, at the real compute and memory limits. None of the three has
  been driven on a validator yet, and winding a market all the way down
  to retired has not run anywhere yet.
- Opening a market is all-or-nothing: one atomic transaction locks the
  collateral, creates the market, and opens trading — or rolls the whole
  thing back.
- Range and tail protection ("pays out if SOL ends below X") is just a
  bundle of cell claims, so its price is exactly the sum of the cell
  prices. No extra machinery, nothing to liquidate.
- The web app ([`apps/dclutch-web`](apps/dclutch-web)) connects a wallet,
  lists markets, shows a market's cells and prices, and reads your
  portfolio straight from the chain — no indexer.
- A TypeScript SDK ([`packages/dclutch-sdk`](packages/dclutch-sdk)) and a
  command-line client ([`packages/dclutch-cli`](packages/dclutch-cli))
  drive the same flows from code and from a terminal.

Not done yet: the Structured product family is still being built, the
General and Dealer trading paths have not run their first live trades, and
there is no market discovery index. Trading is also expensive — it runs
close to Solana's per-transaction compute limit, and cutting that cost is
active work.

## How a market works

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
fallback outcome the creator disclosed before it opened, and anyone can
trigger that step for a pre-funded bounty.

A transaction that doesn't check out exactly — wrong account, wrong
authority, stale state, a window that isn't open — is **refused**: the
whole transaction rolls back and your funds stay where they were. Every
refusal carries a code naming the program that refused and why; the full
list is in [the refusal reference](docs/reference/refusals.md).

## The seven programs

The protocol is split across seven on-chain programs, each with one job.
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

The other programs under [`programs/`](programs) are accelerators and test
harnesses.

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
  programs, boots a local validator, and runs markets through their lives.
- [`apps/dclutch-web`](apps/dclutch-web) — the web app.
- [`docs/decisions/`](docs/decisions) — why the architecture is the way it
  is.

## The artifacts, and where they come from

The tools and consoles pass a handful of artifacts between them. Every one
has exactly one producer:

| Artifact | Made by | Lives | Used by |
| --- | --- | --- | --- |
| Checked release (per program + the multiprogram evidence) | `tools/release` (the checked-release pipeline) | `release/` build output | the deploy runbook; the web Console's activation page |
| Deployment plan (`plan.json` + genesis accounts) | the bootstrap producer (`tools/local-validator/bootstrap/successor`) | your work directory | the campaign driver; validator launch |
| Finalized records (products, sources, configs) | published on chain by the campaign driver | **on the chain** — fetch by address, never paste | every program; the web app reads them live |
| Market spec (`run-spec`) | you, via the create wizard or a spec file | your work directory | founding |
| Keypairs | `solana-keygen` (or the driver's per-role forge) | files you keep | signing; the address a keypair file prints is the one you fund |
| Relay publication log | the relay daemon | [portal.dregg.studio/relay/](https://portal.dregg.studio/relay/) | anyone checking the operator is alive |
| Evidence documents | each campaign, as it runs | [`docs/evidence/`](docs/evidence) | humans; the reference site |

If a console asks you to paste something and you don't know where it comes
from, that's a bug in the console — this table is the answer key.

## Try it

```sh
# build the programs, boot a local validator, create and open a market
# (about 13 minutes):
tools/gauntlet/run.sh --mode full

# the web app's test suite:
cd apps/dclutch-web && npm test

# workspace checks:
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Working on the code itself? Read [`AGENTS.md`](AGENTS.md) and
[`WAVE.md`](WAVE.md) first — they carry the working agreements this
repository runs on.

## Where this is going

The next milestone is the protocol live on Solana devnet, resolving
markets about the state of Solana mainnet: Pyth's devnet feeds carry the
major prices directly, and a disclosed relayer carries everything else.
dClutch grew out of Dragon's Clutch; the first generation lives in the
neighboring `dragons-clutch` repository as an archive.
