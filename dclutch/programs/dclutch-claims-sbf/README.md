# dclutch-claims-sbf

Standalone SBF trust boundary for the canonical runtime-width Claims owner.
It authenticates the sparse logical Core Market, the canonical Claims
aggregate and Position PDAs derived from that logical Market, a Market-selected
Registry, current Core/caller/Claims Loader deployments, the shared release-set
caller PDA authority, and exact optimistic revisions before executing one
`ClaimsPlanV1` basket. Immutable Realm/Product/release content remains owned by
Core and the finalized records; Claims consumes only Core-owned identity,
lifecycle, and manifest references.

The generic frame preserves its original first ten accounts and appends the
cross-owner Core join:

| Index | Account |
| ---: | --- |
| 0 | release-pinned caller authority signer |
| 1 | writable canonical Claims aggregate PDA |
| 2 | writable source Position, or current Claims executable sentinel |
| 3 | writable destination Position, or current Claims executable sentinel |
| 4 | Registry activation cache |
| 5–6 | current caller program and ProgramData |
| 7–8 | current Claims program and ProgramData |
| 9 | immutable Market-selected Registry program |
| 10 | canonical logical Core Market state (`ClaimsPlanV1.market`) |
| 11–12 | current Core program and ProgramData |

The one foundational exception is a named, non-aliasing
`InitializeClaims`/`InitializeCompleteSet` route. It uses the same first 13
accounts plus Rent at index 13 and the System program at index 14. The logical
Core Market must still be `Founding`; the canonical Claims aggregate and
founder Position must be System-owned, zero-data PDAs that already hold at
least their exact runtime-width rent floors. Claims signs only for those two
PDAs, allocates and assigns them, initializes the aggregate directly into
`Open`, mints the equal positive founding complete set, and returns the one
Core effect acknowledgement. Ordinary `SplitClaims` remains exact13 and
`Open`-only. Existing accounts, underfunding, caller signer authority, a
substituted PDA, or an aliased discriminator refuse; excess lamports remain a
donation in the created account rather than becoming economic principal.
The Claims child route and generated Core effect tag are implemented, but the
first isolated Core SBF slice at `c4b8baab` does not yet dispatch
`InitializeClaims`; founding is therefore not claimed as end-to-end physical
evidence from that Core ELF.

Generic Core and Trading callers atomically compose Claims with the canonical
Custody child. Claims returns the exact 256-byte `ClaimsReceiptV1`; the outer
caller must authenticate its producer, request digest, payout, revisions, and
post-resource digest before committing its own state.

`ClaimsPlanV1` and the EconomicSlice state it addresses are migration-only.
The exact remaining producers are
`crates/dclutch-general-adapter-contract/src/child_packets.rs`,
`programs/dclutch-trading-sbf/src/dealer/physical.rs`, and
`programs/dclutch-dealer-sbf/src/lib.rs`. No new controller may use that route;
removing those three producers permits deletion of the generic branch and its
EconomicSlice dependency.

Rational Representation V2 instead consumes the canonical runtime-width
LiabilityBasisV2 aggregate and ProtocolPositionV2 accounts. Its operator and
onchain adapter share the SDK-free state layout from
`dclutch-claims-svm::liability_basis_state_v2`; Core alone owns lifecycle and
winner. The obsolete `ActionV1` representation wire, its parallel state
adapter, and its terminal caller harness are not dispatched or built.
