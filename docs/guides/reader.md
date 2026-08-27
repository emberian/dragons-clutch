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

dClutch is not deployed anywhere. There is no live market and nothing to
buy. What exists runs on a local test chain, and all of it is in this
repository:

- A market's whole life runs end to end: created, funded, opened, traded,
  resolved against a real Pyth price, redeemed, and retired.
- The web app connects a wallet, lists markets, shows a market's cells
  and prices, and reads your portfolio straight from chain state.
- A TypeScript SDK and a command-line client drive the same flows.

Still to come: the Structured product family, the General and Dealer
trading paths, a market discovery index, and the devnet deployment
itself.

## The plan

The demo is the protocol live on Solana devnet, resolving markets about
the state of Solana mainnet — pool prices, token graduations, the major
feeds. Pyth's devnet feeds already carry mainnet prices for the majors;
everything else arrives through a relayer that publishes signed copies of
mainnet account data, checked and decoded on chain
([`docs/design/MAINNET_STATE_RELAY.md`](../design/MAINNET_STATE_RELAY.md)).
Even live, it will be a devnet demo: unaudited, and not a place to put
money at risk.

## See it run

```sh
# build the programs, boot a local validator, create and open a market
# (about 13 minutes):
tools/gauntlet/run.sh --mode full

# the web app's test suite:
cd apps/dclutch-web && npm test
```

The run shuts down its validator when it finishes. Resume the saved chain
with `tools/gauntlet/frontend/resume-validator.sh` and point the web app
at it to browse the market you just made.

Every instruction, error code, and measured cost is in the
[reference](../reference/README.md), generated from the code itself.
