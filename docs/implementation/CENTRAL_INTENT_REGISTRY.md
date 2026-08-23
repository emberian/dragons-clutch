# Central intent registry

Status: General V2 remains registry-only. Dealer policy-catalog actions are
executable only in one explicitly non-production laboratory profile.

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
| Covered dealer | 76 | `0x4c` | 1 | policy catalog only in the named non-production lab |
| Source plane / Series | 77 | `0x4d` | 2 | disabled |
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

These names allocate local tags only. They do not freeze payload bytes, account
lists, account codecs, or transition semantics. Dealer now allocates the
following bounded policy transport without enabling any facility/economic
action:

1. `BeginPolicy`
2. `WritePolicy`
3. `SealPolicy`
4. `AbortPolicy`

The exact payload widths are 72, 228, 32, and 32 bytes. `WritePolicy` carries a
192-byte padded chunk and a strict cursor. The account coordinates `0x7d/1`
(1,288-byte stage) and `0x7e/1` (1,204-byte immutable catalog) are part of the
same atomic allocation. The local action values do not reuse the pure Dealer
runtime enum's zero-based representation. `SealPolicy` persists an unadmitted
catalog artifact; it does not initialize liquidity.

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
version, local action)`. All production executable sets remain empty for every
successor. The distinct
`profile-non-production-dealer-policy-catalog-lab` identity enables only
`(76,1,1..=4)`. Every production profile returns `UnsupportedInstruction`
before reading accounts. Unknown family versions and unknown local actions
fail strict decoding and cannot fall into a legacy handler.

A later activation must change the following atomically:

1. fix the action payload codec and hostile-byte tests;
2. fix every required account `(program namespace, tag, version)` and its
   lifecycle;
3. add the exact capability triple;
4. add one dedicated runtime route and pre-account refusal tests;
5. update this registry and its collision tests without changing legacy golden
   bytes or packet limits.

General V2 local actions 1 through 34 listed above are already
**reserved-disabled allocations**: their numeric coordinates are in the
registry, but they have no payload codec or executable capability. Unlisted
future local-action proposals, and every proposed account shape, stay outside
the central ledger until their atomic review is complete.
