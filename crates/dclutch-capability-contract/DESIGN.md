# Capability manifest contract

The Market's immutable `capability_manifest_id` is the only capability
authority. It must be the composing hash policy's content identity for the
exact canonical manifest bytes. Indexes, bit sets, UI labels, and cached
feature summaries are untrusted projections and cannot grant a capability.

Each entry uses content identities for its kind, implementation release,
configuration, capacity profile, child schema, and child derivation policy.
There is deliberately no global capability enum or permanent bit position.
Entries are strictly ordered by kind identity, so one manifest cannot select
two competing releases for the same kind.

## Provisional artifact profile

Artifact profile 1 admits at most 16 entries and at most 16 dependencies per
entry. These are provisional encoding and measured SVM-frame bounds, not
mathematical or product limits. Dependency references are canonical entry
indices, not a global bit mask. A future profile lifts either bound by defining
a new profile/schema decoder and wider entry layout. Existing profile-1 bytes
and content identities remain valid; a Market that needs the wider profile is
founded with that profile's manifest identity rather than mutating an existing
Market.

## Funding boundary

`FundingQuoteV1` is immutable manifest content. `FundingStateV1` is a separate
mutable ledger bound to the manifest content identity and entry index. Every
principal atom is segregated as rent, creation, work, provider, bounty,
liquidity, or service principal. Initial state requires all quoted principal
to be presently held. Remaining plus released principal must equal the quote
in every compartment, and the observed holding account must equal remaining
principal exactly.

Hoard collateral and expected future fees are outside this model and cannot be
named as capability funding. An adapter must atomically pair a successful
state transition with the corresponding lamport or token movement; this pure
contract does not claim to observe account balances itself.

`RequiredAtFounding` entries must activate before Market opening.
`PrepaidLazy` entries may remain pending through opening, but their exact
creation and rent principal is present from founding and activation must occur
no later than the committed slot deadline.

Market founding selects resolution funding by immutable meaning, not by a
caller-supplied amount or a conventional manifest position. The authenticated
manifest must contain exactly one `RequiredAtFounding` entry whose `config_id`
equals the Market identity's `resolution_policy_id`. The total no-allocation
selector returns that entry together with its canonical index and refuses both
missing and ambiguous matches; manifest order is never a tie breaker.

The current one-shot Pyth resolution Fund is a specialized adapter profile. At
its adapter boundary, the selected entry's quote must contain exactly:

- Fund-account rent equal to the authenticated Rent calculation;
- provider reimbursement committed by the manifest; and
- a positive resolution-success bounty.

Creation, work, liquidity, and service principal must all be zero because the
specialized Fund does not physically hold those compartments. The provider and
bounty values are derived from this immutable quote. They do not appear in the
founding instruction, and neither collateral nor future fees may replace them.

## Market-opening readiness

`MarketOpeningReadinessV1` is a transient direct Market child, not an opaque
caller attestation and not another economic ledger. Its 128-byte exact record
binds Market key, generation, manifest content identity, exact manifest entry
count, canonical next entry index, and the sponsor rent-refund identity.
There is no stored Ready status: it is derived only when `next_entry_index ==
entry_count`.

The adapter authenticates the manifest content hash, derives the child using
`MARKET_OPENING_READINESS_PDA_DOMAIN`, and starts it with the Market child
count. That canonical domain is `dclutch/open-readiness/v1` (25 bytes), below
the chain-derived 32-byte maximum for one PDA seed component; adapters must not
hash or rewrite it. Each advance must name exactly the next manifest index and
supplies the
actual canonical `FundingStateV1`, observed present principal, and current
slot. The kernel calls `validate_market_open`; required pending entries, lazy
deadline expiry, wrong binding, underfunding, replay, skips, and reordering
refuse before readiness changes. Funding state remains the sole owner of every
amount, quote, released compartment, and activation fact.

After an advance and before Open, the SBF adapter must seal capability
operations: while the Market is Founding, no capability operation may release
principal. At Open, SBF must include the immutable `CapabilityManifest` account,
authenticate its content identity from the Market root, decode its exact
canonical bytes, and pass that actual manifest to `require_ready_for_open` for
the exact Market/generation/manifest. It then atomically consumes and
rent-refunds readiness while creating custody, keeping the direct-child count
coherent. Malformed, noncanonical, wrong-identity, or wrong-count manifest
bytes must refuse before custody creation. This is an adapter transition
contract only; this crate does not implement SBF or duplicate custody facts.
