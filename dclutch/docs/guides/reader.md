# Reader guide

What dClutch is, what works today, and how to see it run. Read this first
if you're not trading or running a market yet — just working out what this
project is.

## What dClutch is

dClutch is a Solana protocol for markets on real-world numbers. A market
asks one question — where will SOL/USD be at noon on Friday? — and splits
the possible answers into buckets. You buy claims on buckets; claims on
the winning bucket pay out. Every claim is fully backed by collateral
deposited before the claim existed, so there is no leverage, no
liquidation, and no way to lose more than you paid.

A pinned price feed resolves each market — no committee, no discretionary
judge. If the feed goes silent, the market takes a fallback outcome that
was published and funded before it opened.

## What works today

Seven protocol programs are deployed at permanent addresses on Solana
devnet, and one market on it is open for trading. That market's collateral is
a devnet test token, so there is nothing to buy with money. You can use the
public app to inspect the deployed programs and the Market accounts they
own; the app labels anything it cannot authenticate instead of filling in
missing facts.

The broader execution evidence is still local:

- On a local test chain, a market is created, funded, opened, and a
  participant is admitted to it, on chain. Resolving that market and paying
  it out are not part of the run that is accepted today. The recorded
  resolution fixtures do exercise the Pyth and Wormhole verification paths,
  but in a test harness, and they are not live devnet price publications.
- Trading between two counterparties, moving claims between holders, and
  paying out a winning claim run in test harnesses at Solana's compute and
  memory limits. Those runs are software evidence, not devnet executions.
- The web app reads the live devnet deployment, lists compatible markets,
  and reads portfolio state from chain. Static browser data remains an
  untrusted view of the onchain accounts.
- A TypeScript SDK and command-line client build and check the same flows.

Still to come: the first public trade on the open devnet market, then its
resolution and wallet payout, followed by the broader product and trading
families.

## The plan

That open market asks a question about the state of Solana mainnet, and the
next public milestone is the first trade made on it. Pyth's devnet feeds
already carry mainnet prices for the majors; everything else can arrive through a
relayer that publishes signed copies of mainnet account data, checked and
decoded on chain
([`docs/design/MAINNET_STATE_RELAY.md`](../design/MAINNET_STATE_RELAY.md)).
It remains an unaudited devnet demonstration, not a place to put money at
risk.

## See it run

```sh
# build the programs and enumerate every route they accept (no chain):
tools/gauntlet/run.sh --mode census

# found a market on a throwaway local validator and join it as a
# participant. It builds its own chain and tears it down afterwards, and it
# needs a clean committed checkout and a checked release root to bind to:
python3 tools/release/private-validator-lifecycle/run.py \
    --repo /absolute/clean/dclutch \
    --release-root /absolute/checked/release \
    --validator "$(command -v solana-test-validator)" \
    --solana "$(command -v solana)" \
    --work /absolute/scratch/outside/the/repo \
    --through participant

# the web app's test suite:
cd apps/dclutch-web && npm test
```

The lifecycle run builds its own local chain, founds one market, admits one
participant, and tears the chain down when it finishes. Resolving that
market and paying it out are not part of it yet.

Every instruction, error code, and measured cost is in the
[reference](../reference/README.md), generated from the code itself.
