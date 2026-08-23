# Central intent registry

Status: registry allocation only. No successor action in this document is an
executable runtime route.

## Frozen legacy space

The main program's existing intent encodings remain exactly tags `1..=73`, all
at intent version `3`. Their payloads, golden encodings, and 402-byte maximum
inner length do not change. The outer reference request therefore remains at
most 415 bytes:

```text
request tag/version (2) + sequence (8) + action (1) + inner length (2)
+ inner intent (at most 402) = 415
```

No successor is added to `Intent`. Legacy decoding therefore continues to
refuse every successor family.

## Reserved successor families

A successor inner envelope is:

```text
family tag (u8) | family version (u8) | local action (u8) | action payload
```

The exact family/version pair owns the local-action namespace. Changing a
family version creates a new namespace; it does not inherit capability.

| family | decimal tag | hexadecimal tag | family version | runtime |
| --- | ---: | ---: | ---: | --- |
| General V2 | 74 | `0x4a` | 1 | disabled |
| Structured claim | 75 | `0x4b` | 1 | disabled |
| Covered dealer | 76 | `0x4c` | 1 | disabled |
| Source plane / Series | 77 | `0x4d` | 2 | actions allocated, runtime disabled |
| Evidence-only recovery | 78 | `0x4e` | 1 | disabled |

Source/Series starts at family version 2 deliberately. Numeric-fallback V3
Template/Payout proposals are not promoted into this registry.

General V2 reserves local actions 1 through 34, in order:

1. `CreateMarket`
2. `InitEpoch`
3. `InitOrderPage`
4. `PlaceOrder`
5. `CancelOrder`
6. `FreezeEpoch`
7. `BeginCandidate`
8. `WriteCandidateFeed`
9. `SealCandidate`
10. `InitClearWork`
11. `GrowClearWork`
12. `AdvanceClearOrders`
13. `AdvanceClearSlices`
14. `CompleteCandidateVerification`
15. `FinalizeSelection`
16. `ExpireCandidate`
17. `MarkWorkClosed`
18. `ClaimCandidateBond`
19. `ClaimCandidateWork`
20. `CleanupCandidate`
21. `ClaimSolver`
22. `CloseCandidateIndexPage`
23. `ClaimEpochUnused`
24. `FreezeEntitlement`
25. `EntitleSlice`
26. `ReleaseTerminalReservation`
27. `CloseReceipt`
28. `CloseReservation`
29. `ClosePage`
30. `ClosePot`
31. `CloseCandidate`
32. `CloseClearWork`
33. `CloseEpoch`
34. `ClosePosition`

These General V2 names allocate local tags only. They do not freeze payload
bytes, account lists, account codecs, or transition semantics.

Source/Series V2 now allocates six action tags with exact laboratory payload
codecs in `clutch_solana_layout::product_series`:

1. `RegisterSeries`
2. `ActivateFunding`
3. `AdvanceOccurrence`
4. `LapseOccurrence`
5. `ObserveDonation`
6. `CloseFunding`

Allocation still grants no execution capability. The program's executable
extension set remains empty. In particular, a decoded registry release ID or
capability-profile ID is not authority: registration stays disabled until the
adapter can reconstruct `RegistryCapabilityProjectionV2` from an authenticated
central release, and each value-bearing action stays disabled until its exact
source, collateral, liveness, and failure receipts are authenticated.

Covered Dealer owns `0x7d/1` for its funded facility stage and `0x7e/1` for its
immutable policy. The Source/Series account namespace therefore reserves the
disjoint `0x7f/1` for the immutable Series registration and `0x80/1` for the
mutable Series-funding wrapper. Their exact 168-byte and 376-byte codecs are
fixed, but their allocation status is reserved-disabled. The funding wrapper
adds tag/version/bump/flags, exact refundable account-rent principal, and five
release-selected collateral-vault rent principals around the pure 324-byte
`SeriesFundingStateV1`; it does not copy its cursor or component-balance facts.

## Decimal 74 is not hexadecimal `0x74`

General V2's intent family is decimal 74, which is `0x4a`. The already-frozen
Source Archive V2 **account** discriminator is hexadecimal `0x74`, which is
decimal 116. They are not the same number and they also live in different wire
namespaces:

| allocation | namespace | decimal | hexadecimal | version |
| --- | --- | ---: | ---: | ---: |
| General V2 | main intent | 74 | `0x4a` | 1 |
| Source Archive V2 | main account | 116 | `0x74` | 1 |

`clutch_solana_layout::registry` pins both spellings with compile-time
assertions. Its collision ledger scopes uniqueness by namespace, tag, and
version. It is the semantic owner of the Source Archive V2 tag/version pair;
the SBF codec imports those constants directly while remaining the sole owner
of that account's body and exact length. The ledger is not a competing
account-layout inventory.

## Capability and activation rule

Capability membership is keyed by the exact triple `(family tag, family
version, local action)`. The executable set is currently empty. The SBF
dispatcher recognizes an allocated General V2 or Source/Series V2 triple only to return
`UnsupportedInstruction` before reading accounts. Unknown family versions and
unknown local actions fail strict decoding and cannot fall into a legacy
handler.

A later activation must change the following atomically:

1. fix the action payload codec and hostile-byte tests;
2. fix every required account `(program namespace, tag, version)` and its
   lifecycle;
3. add the exact capability triple;
4. add one dedicated runtime route and pre-account refusal tests;
5. update this registry and its collision tests without changing legacy golden
   bytes or packet limits.

General V2 local actions 1 through 34 and Source/Series V2 local actions 1
through 6 listed above are already
**reserved-disabled allocations**: their numeric coordinates are in the
registry, but they have no payload codec or executable capability. Unlisted
future local-action proposals, and every proposed account shape, stay outside
the central ledger until their atomic review is complete.
