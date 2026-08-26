# Rational Representation V2 lifecycle integration

This contract defines the accepted successor lifecycle without persisting a
parallel lifecycle ledger.

## Physical activation

- `ActivateReceipt` initializes one pre-funded canonical Token-2022 Mint with
  the Claims representation authority as both mint authority and
  `MintCloseAuthority`.
- `ActivateCoordinate` initializes one ordered nonzero descriptor coordinate:
  a closeable shard Mint, the representation authority's canonical Structured
  ATA, and the canonical Claims LBV2 custody Position plus admission record.
- Zero-coefficient coordinates create no physical resource or rent obligation.
- All target accounts are already funded. The adapter rejects funding below the
  current Rent minimum and never accepts an offchain payer or permissioned
  activation authority.

## Physical retirement

- `RetireCoordinate` requires zero shard supply, zero Structured custody, and a
  zero LBV2 Position, then closes Token resources and calls the public protocol
  Position close path directly. Every lamport reaches the immutable RentCredit.
- `RetireReceipt` requires zero receipt supply and an exact ordered vacancy row
  for every nonzero descriptor coordinate before closing the receipt Mint.
- Token-2022 with the exact `MintCloseAuthority` extension is required. Legacy
  SPL Token cannot close Mint accounts and is therefore not a truthful profile
  for this reclaimable lifecycle.

## Open integration seams

1. Protocol Position V2 commit `fe51112` now supplies the exact
   `ClaimsCapability` owner kind and authenticates the descriptor/outcome owner
   with its public `ProtocolPositionClaimsCapabilitySeedsV2`. The adapter must
   consume that public seed API; it must not duplicate a private
   `rational-claims-v2` literal or label the owner as `User`/`TradingRecord`.
2. The current green Rational request path still encodes all Product outcomes
   for Structured actions and infers outcome from row position. Its successor
   must encode an explicit outcome per row and require the exact ordered
   nonzero support before this lifecycle can be enabled end to end.
3. The Claims adapter must accept exactly the Token-2022 base Mint plus one
   `MintCloseAuthority` extension and refuse all other extension layouts. The
   present `dclutch-token-svm` zero-extension parser is intentionally
   insufficient.
4. Shared Claims dispatch/Cargo/module edits wait for the current Core V2 cut.
   The adapter must directly call `protocol_position_v2::process` with its exact
   Admit/Close frame; it must not self-CPI or bypass its authentication.

## Legacy deletion boundary

After the successor adapter, real Token-2022 ProgramTest, rollback campaign,
and operator activation constructors are accepted, delete/disconnect:

- `programs/dclutch-sbf/src/bearer.rs` and its dispatcher magic route;
- `crates/dclutch-bearer-contract/**`;
- `crates/dclutch-operator/src/verticals/bearer.rs`;
- legacy Bearer dependencies and workspace members that have no successor
  consumer.

No legacy path is deleted merely because this pure contract exists.
