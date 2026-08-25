# dClutch

dClutch is a greenfield Solana protocol for fully collateralized,
liquidation-free claims over bounded objective states.

It is an architectural restart informed by Dragon's Clutch. It keeps the useful
mathematics—canonical state partitions, exact claim bases, complete-set
accounting, coefficient portfolios, checked clearing, and funded resolution—
while replacing the universal account graph and cumulative action maze with a
small Market Core and optional capabilities.

## Status

This repository is active greenfield development. It has no deployed program,
official frontend, live market, wallet integration, production source profile,
or release.

An experimental Lean-owned semantic specializer now emits canonical Direct
Effect bytes and an exact Solana ABI-v1 proof profile. Its isolated 2,232-byte
SBF target executes the example plan in 155 CU, and one exact successful machine
path has a qedsvm-generated, Lean-kernel-checked Hoare triple. This is not a
production successor: controller-PDA authentication, real SPL custody,
high-level-to-machine composition, and complete path coverage remain open.

The implemented first-profile foundation includes exact categorical
complete-set accounting, compact Market/Realm/Position contracts, funded
release-bound Pyth price and permissionless-failure resolution, exact provider
CPI construction, chain-derived unsigned resolution instructions, and a local
SVM test that executes the real dClutch SBF ELF. Exact SDK-free legacy Token and
zero-extension Token-2022 adapter profiles are present, but Market founding,
token custody, split/merge/redemption CPI, trading, product compilation,
liquidity, and frontend workflows are not yet end to end. The categorical Pyth
composition is a cheap first profile, not the protocol's general product
ceiling.

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
