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

The program is intentionally outside the root Cargo graph until its shared
Core and Custody composition is released. Token-2022 representation actions
share this same economic owner; Token-2022 is an adapter boundary, never a
second claims ledger. A positive-payout direct wrapper terminal redemption uses
the exact `ActionV1 || CustodyRequestV1` wire and appends accounts 16–25: Claims
caller authority, Custody program/ProgramData, finalized Realm, per-descriptor
Custody replay, collateral Mint, Market Hoard vault, claimant collateral
recipient, Custody transfer authority, and Realm token program. Claims binds
the Core winner/generation/Realm, burns the Token-2022 receipt, executes the
canonical Hoard-to-external Custody transfer, authenticates its immediate
producer/replay receipt, checks every postcondition, and only then commits its
wrapper state. A zero-payout terminal burn has no collateral effect and retains
the exact 16-account frame.

The real-ELF terminal campaign uses the isolated Core at `c4b8baab`, the
Registry/Core decoupling at `88dba0e`, and the real Token-2022 v11 ELF with
SHA-256
`495e9d7680dd555cb126a6a8e5464af5be9b01f02f2cd70634352722d22e3cad`.
The exact 776-byte instruction has 26 metas and 25 unique accounts: legacy is
1,804 bytes and v0 without an ALT is 1,806 bytes, while a live two-extension
ALT yields a 1,127-byte positive packet and a 1,130-byte late-refusal packet.
The positive terminal burn and Custody payout consume 698,392 CU. A test-only
real-SBF caller then refuses after Claims, Token-2022, and Custody all return;
726,309 CU later, all nine mutable account snapshots are byte-for-byte equal
to their pre-transaction state.

That campaign also caught a target-specific interpreter defect: host traversal
of the generated aggregate `ActionRule` table selected tag 3 correctly, while
real SBF effectively projected the Open-only rule. The Lean emitter now owns a
total literal tag-to-rule decision function instead. An independent real-SBF
corpus exercises all four tags across all four economic phases and refuses
unknown tags 0, 5, and 255 before the terminal CPI campaign. The current
verifier-clean Claims ELF is 245,472 bytes with SHA-256
`0572885ec6c72ac6554ff52bc3902036974dec6e390898e6f72642e96d3b2e3c`.
These are build and local ProgramTest results, not deployment or mainnet
evidence.
