# dClutch

dClutch is a greenfield Solana protocol for fully collateralized,
liquidation-free claims over bounded objective states.

It is an architectural restart informed by Dragon's Clutch. It keeps the useful
mathematics—canonical state partitions, exact claim bases, complete-set
accounting, coefficient portfolios, checked clearing, and funded resolution—
while replacing the universal account graph and cumulative action maze with a
small Market Core and optional capabilities.

## Status

This repository is a new implementation scaffold. It has no deployed program,
official frontend, live market, wallet integration, production source profile,
or release. The initial kernel implements only categorical complete-set
liability transitions. Smooth claims, Solana adapters, trading, Pyth resolution,
and operator workflows remain to be built as vertical slices.

## Architecture in one picture

```text
                         Product compiler
                                |
                                v
Resolution adapter ------> Market Core <------ verified execution
  Pyth / others          /      |      \          receipts
                        /       |       \
                   Positions   Hoard   Supply
                                  ^
                                  |
                     +------------+-------------+
                     |            |             |
                  Direct       General        Dealer
               signed intents  batch venue  covered liquidity
```

The Market Core owns liabilities and collateral. A venue owns its order and
pricing semantics. A resolution adapter owns authenticated observation
transport. Product compiles immutable identities and capabilities. Operator
software derives transactions from finalized chain state but owns no protocol
truth.

Read [ARCHITECTURE.md](ARCHITECTURE.md), [PROJECT_METHOD.md](PROJECT_METHOD.md),
and [COMPOST.md](COMPOST.md) before adding a subsystem.

## Bootstrap checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
