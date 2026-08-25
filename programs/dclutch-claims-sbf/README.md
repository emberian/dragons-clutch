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

Generic Core and Trading callers atomically compose Claims with the canonical
Custody child. Claims returns the exact 256-byte `ClaimsReceiptV1`; the outer
caller must authenticate its producer, request digest, payout, revisions, and
post-resource digest before committing its own state.

The program is intentionally outside the root Cargo graph until its shared
Core and Custody composition is released. Token-2022 representation actions
share this same economic owner; Token-2022 is an adapter boundary, never a
second claims ledger. Direct wrapper terminal redemption deliberately returns
`CustodyRequired` until its Lean-owned wire carries the same canonical Core,
Realm, Custody-request, replay, and receipt join. It never performs an unpaired
Hoard debit.

Host tests, strict all-target Clippy, and the optimized SBF verifier pass. The
current standalone ELF is 141 KiB; this is build evidence, not deployment or
mainnet evidence.
