# Dragon's Clutch

This is the home of **dClutch**, a Solana protocol for markets on
real-world numbers — where a price will be at a stated time, for example.
You buy claims on the outcome you believe in; if you're right, each claim
pays out one collateral unit. Every claim is fully backed by collateral
locked up before the claim exists, so there is no leverage, no
liquidation, and no way to lose more than you paid.

The active work lives in the [`dclutch/`](dclutch/) subtree, vendored here
from its working tree in waves. **Start at
[`dclutch/README.md`](dclutch/README.md).**

Seven upgradeable program identities are parked on Solana devnet, and the
public app reads that development deployment. There is not yet an open devnet
market you can trade, and nothing here is deployed on mainnet or value-bearing.
Local validators remain the primary place where the complete lifecycle is
exercised while the next devnet upgrade is prepared.

## Layout

- [`dclutch/`](dclutch/) — the current protocol. Everything else here is
  context for it.
- [`archive/`](archive/) — the first generation, superseded in August 2026.
  [`archive/gen1/`](archive/gen1/) holds its implementation and formal work
  ([`programs/`](archive/gen1/programs/), [`crates/`](archive/gen1/crates/),
  [`apps/`](archive/gen1/apps/), [`lean/`](archive/gen1/lean/),
  [`verus/`](archive/gen1/verus/), [`rocq/`](archive/gen1/rocq/),
  [`research/`](archive/gen1/research/)), its architecture, protocol, and
  review documents ([`docs/`](archive/gen1/docs/)), and its static microsite
  ([`site/`](archive/gen1/site/) — no longer published; the live site builds
  from `dclutch/`). [`archive/handoffs/`](archive/handoffs/) holds the
  planning and status documents that drove it.

## The first generation

The first version of this project ("Clutch") is kept here as a working
archive. It ran a complete market lifecycle on a local chain — real Pyth
price verification, trading, resolution, redemption, every collateral
atom accounted for — and its requirements, invariants, and counterexamples
shaped the rebuild. It is no longer developed; anything found wrong in it
gets fixed in dClutch instead.

Where its history lives:
[`archive/handoffs/CURRENT_TRUTH.md`](archive/handoffs/CURRENT_TRUTH.md) for
what it did and didn't do,
[`archive/gen1/docs/reviews/ARCHITECTURE_REVIEW_2026-08-22.md`](archive/gen1/docs/reviews/ARCHITECTURE_REVIEW_2026-08-22.md)
for why the rebuild happened, and
[`dclutch/COMPOST.md`](dclutch/COMPOST.md) for the rules on what may be
carried forward.

## Security

There is no mainnet release and no live value-bearing market. Devnet execution
and local-validator evidence are development evidence, not mainnet assurance.
Security policy and threat model: [`SECURITY.md`](SECURITY.md).

## License and provenance

First-party source and documentation are licensed under
[`AGPL-3.0-or-later`](LICENSE). The project is greenfield: it must not
import, copy, or depend on JOSHI, joshibot, leanuweave, minidregg,
breadstuffs, Oracle Pit, or historical DREGG prototypes without an
explicit provenance and license review
([`archive/gen1/docs/PROVENANCE.md`](archive/gen1/docs/PROVENANCE.md)).
