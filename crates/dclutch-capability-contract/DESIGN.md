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
