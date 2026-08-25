# dClutch Market Core codec

This standalone crate is the generated, fixed-memory interpreter for
`DClutchSemantics.MarketCore`. It is intentionally not wired into the workspace
or an SBF adapter yet.

`DClutchSemantics.MarketCoreAbi` owns the canonical field order, widths, and
offsets. `EmitMarketCoreRust.lean` emits both those constants and the safe Rust
interpreter. `MarketCorePhysicalAbi` and `EmitMarketCorePhysicalRust` own the
cross-program wire boundary. The fixed Market header is 1,416 bytes and the
request is 72 bytes.
Claim vectors are one exact borrowed account-data tail of `7 * N * 8` bytes,
with canonical little-endian `u64` values and `N` equal to the Product's runtime
`outcome_count`; the ABI imposes no width-specialized Market semantics or
provisional maximum N. This representation requires neither alignment casts nor
an adapter heap.

The physical boundary is deliberately factored rather than padded into one
maximum account or request shape:

- `CoreEffectEnvelopeV1` is a 280-byte routing/replay prefix. It binds a single
  role-owned request by exact byte length and SHA-256 digest. Claims, Custody,
  and Resolution remain the sole owners of their token, position, funding, and
  certificate facts.
- `CoreEffectAckV1` is a 240-byte normalized acknowledgement. It binds the full
  effect digest, current role program, release set, Market, context, exact
  pre-revisions, monotonic post-revisions, and the role-owned poststate digest.
- `SeriesCoreRequestV1` is a 336-byte Core-owned child request. Its
  Prepare/Consume/Expire shape binds the exact release set, template, ticket,
  Market, Realm, Product, founder, beneficiary, revisions, rent, work, and
  positive Ticket-owned Hoard principal. Its
  Close shape is disjoint and requires every occurrence-only field to be zero.
- `SeriesCoreAckV1` is a 264-byte Core-produced receipt for that direct boundary.
  It binds the Registry-selected Core program, exact request digest, release
  set, Template, Ticket, Market, occurrence-derived generation, caller replay
  revisions, and the digest of all Core-owned post-resources. Close has a
  disjoint zero-Ticket/zero-Market/zero-generation shape.

The full-effect digest is
`SHA256("dclutch/core-effect/v1" || u32_le(280) || envelope ||
u32_le(role_request_bytes) || role_request)`. The release-pinned caller PDA is
derived under the selected Core program from the sole release-set-owned
`CallerAuthoritySeedsV1` projection: `["dclutch:role-authority:v1",
release_set, market, Core, context, role_request_digest]`. This avoids both
caller-authored attestations and a digest fixed point: all PDA inputs exist
before the envelope is encoded, while the resulting authority is still exact
to one role request and replay context. The envelope's `target_role` is the
child role and never substitutes for caller role Core. Series is not an
execution-release role; its 336-byte request is a separate direct Series-to-Core
boundary authenticated from the exact Template and Ticket. For occurrence
actions, `market_generation = u64::from(occurrence) + 1`; Close has no Market
generation. The Series program signs with its PDA derived as
`["dclutch/series-core-caller/v1", template, SHA256(exact_request_bytes)]`.
Core must recompute that request digest, derive the PDA under the authenticated
owner of the exact Template/Ticket, and require the PDA signer. Series accepts
the 264-byte receipt only from the Registry-selected Core program as the
immediate return-data producer and rechecks the exact Core post-resource digest.

The interpreter validates all inputs before applying a transition. It separates
rent, unclassified donation, Source work funding, deferred custody rent, and
claimant Hoard principal. It consumes current Registry/Core execution-release
receipts but does not authenticate accounts, derive addresses, move tokens,
perform CPI, or supply transaction rollback. Those remain named adapter duties.

Run `./check.sh` to rebuild the Lean ABI, compare the checked-in Rust against
fresh generator output byte-for-byte, and run formatting, tests, and strict
Clippy.
