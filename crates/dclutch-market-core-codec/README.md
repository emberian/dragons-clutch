# dClutch Market Core codec

This standalone crate is the generated, fixed-memory interpreter for
`DClutchSemantics.MarketCore`. It is safe Rust, `no_std`, `no_alloc`, and is not
yet an account-owning Solana adapter.

`MarketCoreAbi.lean` owns the semantic wire layout. The V2 Market header is
exactly 320 bytes and the request is 72 bytes. The header persists only:

- immutable Market identity references and generation;
- lifecycle phase, readiness, and terminal winner/receipt;
- the immutable RentCredit beneficiary; and
- a checked count of outstanding manifest-selected optional capabilities.

Realm, Product, release-set records, Claims aggregate/Hoard principal, Custody,
Source funding, capability FundingState, child balances, rent, and closure facts
remain owned by their canonical records and programs. Core reauthenticates those
facts at transition boundaries and consumes exact child effects; it does not
cache parallel mutable truths or store a width-dependent economic tail. Product
width is therefore runtime data with no const-N or N=16 semantic ceiling.

Found produces one dust-tolerant creation plan for the Core Market account.
Readiness, custody creation, Claims initialization, and optional capability
activation are separately authenticated child effects. `ActivateCapability`
and `CloseCapability` share one generic manifest/Funding-backed boundary and
maintain `outstanding_capabilities`; retirement refuses while it is nonzero.
Claims, Source, and Custody terminal closures are fixed lifecycle effects and
are not counted as optional capabilities. Retirement returns only the observed
Core account lamports to the persisted RentCredit after all exact closure
receipts authenticate.

## Physical boundaries

`MarketCorePhysicalAbi.lean` and `EmitMarketCorePhysicalRust.lean` own the
cross-program wires:

- `CoreEffectEnvelopeV1` is a 280-byte routing/replay prefix binding one exact
  role request by byte length and SHA-256 digest.
- `CoreEffectAckV1` is a 240-byte acknowledgement binding the full effect,
  selected role program, release set, Market, context, revisions, and child
  post-resource digest.
- `SeriesCoreRequestV1` is a separate 336-byte Series-to-Core request; Series is
  not an execution-release role. `SeriesCoreAckV1` is 264 bytes.

Fixed actions select Claims, Custody, or Resolution. The generic capability
actions accept exactly one non-Core target role. The release-pinned Core caller
PDA uses the sole release-set-owned `CallerAuthoritySeedsV1`; the target role is
never substituted for caller role Core.

The Market PDA under the Registry-selected Core program uses
`["dclutch/market-core/state/v1", realm, product, result_domain,
resolution_policy, capability_manifest, release_set, generation_le]`.
`MarketCoreStateSeedsV1` is the canonical projection.

`CoreMarketViewV1::authenticate` is a pure semantic join over a decoded sparse
Core header plus `CoreReferenceObservationV1`. A production adapter must derive
the observation booleans from exact finalized Realm, Product/Terms/result-domain,
and Registry release-set accounts and from the Claims-owned aggregate PDA; they
are not caller-authored instruction fields. The concrete account order, owner,
PDA, loader, and return-data checks remain the explicitly unverified isolated
Core SBF boundary.

The full-effect digest is
`SHA256("dclutch/core-effect/v1" || u32_le(280) || envelope ||
u32_le(role_request_bytes) || role_request)`.

Run `./check.sh` to rebuild the Lean ABI and examples, compare both generated
Rust files byte-for-byte, then run formatting, all crate tests, and strict
all-target Clippy.
