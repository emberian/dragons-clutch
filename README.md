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

Nothing in this repository is live: no deployment you can use, no live
market, nothing value-bearing. Everything runs on local test chains.

## Layout

- [`dclutch/`](dclutch/) — the current protocol. Everything else here is
  context for it.
- [`programs/`](programs/), [`crates/`](crates/), [`apps/`](apps/),
  [`lean/`](lean/), [`verus/`](verus/), [`rocq/`](rocq/),
  [`research/`](research/) — the retained first-generation implementation
  and its formal work.
- [`docs/`](docs/) — first-generation architecture, protocol, and review
  documents.
- [`site/`](site/) — the first generation's static microsite (no longer
  published; the live site builds from `dclutch/`).

## The first generation

The first version of this project ("Clutch") is kept here as a working
archive. It ran a complete market lifecycle on a local chain — real Pyth
price verification, trading, resolution, redemption, every collateral
atom accounted for — and its requirements, invariants, and counterexamples
shaped the rebuild. It is no longer developed; anything found wrong in it
gets fixed in dClutch instead.

Where its history lives: [`CURRENT_TRUTH.md`](CURRENT_TRUTH.md) for what
it did and didn't do,
[`docs/reviews/ARCHITECTURE_REVIEW_2026-08-22.md`](docs/reviews/ARCHITECTURE_REVIEW_2026-08-22.md)
for why the rebuild happened, and
[`dclutch/COMPOST.md`](dclutch/COMPOST.md) for the rules on what may be
carried forward.

## Security

There is no release, no official deployment, and no live market. Security
policy and threat model: [`SECURITY.md`](SECURITY.md).

## License and provenance

First-party source and documentation are licensed under
[`AGPL-3.0-or-later`](LICENSE). The project is greenfield: it must not
import, copy, or depend on JOSHI, joshibot, leanuweave, minidregg,
breadstuffs, Oracle Pit, or historical DREGG prototypes without an
explicit provenance and license review
([`docs/PROVENANCE.md`](docs/PROVENANCE.md)).
