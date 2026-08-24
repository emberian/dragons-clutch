# dclutch-market-contract

Provider-neutral fixed-layout ownership for the active categorical Market.

The account contains exactly:

- a 16-byte categorical Market header;
- the 232-byte `MarketRoot`;
- 8 bytes of claimant-backing Hoard atoms;
- `N` eight-byte aggregate native state-claim supplies; and
- one 64-byte optional categorical settlement summary.

The total is `320 + 8N` bytes: 336 bytes for a binary Market and 448 bytes at
the current 16-outcome maximum. The summary is all-zero while empty. Once
resolved, it retains only a status, Product-owned resolution route, winner,
positive terminal sequence, and evidence content ID.

`N = 2..=16` is provisional profile V1, not a mathematical restriction. The
lifting path is a new reviewed profile discriminator plus a kernel
implementation with a larger fixed bound. The elementary basis remains one
native claim per cell in an exhaustive ordered state partition; threshold,
ramp, tent, and other payoff shapes belong in portfolio templates.

No oracle policy, feed profile, provider identity, resolution funding, token
program, venue, or Solana account type is persisted here.

## Migration seam

`dclutch-pyth-contract::market::MarketStateV1` remains temporarily present but
is not the semantic owner of new Markets. Its Pyth policy, feed profile, and
receipt must move behind a Pyth resolution child and SBF authentication seam.
Migration must content-identify accepted provider evidence, map the accepted
route to `ResolutionKind`, and supply a positive terminal sequence before the
old composed Market module can be deleted. No bytewise reinterpretation of the
old layout is valid.
