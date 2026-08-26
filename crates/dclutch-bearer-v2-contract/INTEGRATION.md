# Bearer V2 convergence manifest

This successor is accepted only after the canonical Rational Representation V2
Claims route physically executes all three selected actions and the following
legacy paths are removed in the same convergence cycle:

- `crates/dclutch-bearer-contract/**`: superseded private Bearer state and wire.
- `crates/dclutch-operator/src/verticals/bearer.rs`: superseded legacy operator.
- legacy DescriptorV1/StateV1 representation handling in
  `programs/dclutch-claims-sbf/src/representation.rs`: superseded by immutable
  `RepresentationDescriptorV2` plus the exact ephemeral projection.
- any Bearer branch in `programs/dclutch-sbf/src/structured.rs` that persists or
  trusts parallel holder/supply/quantity authority.

Integration gates before deletion:

1. Claims rederives the generic Rational V2 representation authority, shard
   Mint, Claims custody owner, actor token-account owner+Mint, and structured
   custody ATA from the descriptor and selected outcome.
2. Claims composes the exact `RepresentationRequestV2` for Denominate,
   Reconstitute, and RedeemTerminal; Token and Claims postconditions finalize the
   shared replay receipt atomically.
3. Real Token-2022 SBF tests cover two transferable holders, replay refusal,
   release/graph/root substitution, terminal winner/refusal, and deliberately
   late rollback across Token, Claims, Custody, and representation replay state.
4. The operator below is grafted into the canonical operator surface without a
   second request encoding.

No legacy file is deleted by this disjoint crate-only commit while the Claims V2
physical route is still under construction.
