# Central intent registry

Status: General V2 remains profile-gated. Dealer immutable-catalog actions and
the exact Initialize/BindEpoch slice are executable only in one explicitly
non-production laboratory profile.

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
| General V2 | 74 | `0x4a` | 1 | profile-gated non-production slice |
| Structured claim | 75 | `0x4b` | 1 | disabled |
| Covered dealer | 76 | `0x4c` | 1 | policy catalog plus Initialize/BindEpoch in the named non-production lab |
| Source plane / Series | 77 | `0x4d` | 2 | actions allocated, runtime disabled |
| Evidence-only recovery | 78 | `0x4e` | 1 | disabled |
| Exact fractional redemption | 79 | `0x4f` | 1 | disabled |

Source/Series starts at family version 2 deliberately. Numeric-fallback V3
Template/Payout proposals are not promoted into this registry.

General V2 reserves local actions 1 through 41, in order:

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
25. `AccountReceiptEnd`
26. `ConsumeDirectReceiptEggs`
27. `CloseReceipt`
28. `CloseReservation`
29. `ClosePage`
30. `ClosePot`
31. `CloseCandidate`
32. `CloseClearWork`
33. `CloseEpoch`
34. `ClosePosition`
35. `TransferPositionAssets`
36. `ConsumeVirtualSplitReceiptEggs`
37. `ConsumeVirtualMergeReceiptEggs`
38. `FinalizeOwnerSettlement`
39. `InitializeSettlementRoot`
40. `FinalizeMergeReceiptPayment`
41. `ReleaseUnfilledReservation`

These names allocate local tags only. They do not freeze payload bytes, account
lists, account codecs, or transition semantics. Dealer now allocates the
following bounded immutable-catalog transport. The same strict transport
admits exactly one typed body kind per stage: Dealer policy, Dealer action
liveness schedule, or generic seven-compartment runtime-liveness policy.

The collision ledger also reserves three same-tag General successors for the
still-disabled cost-aware action 14/15 path: Window `24/5` (565 bytes),
AdmissionNode `0x77/2` (775 bytes, adding only the checked cost-certificate
content ID), and MarketBinding `0x79/2` (572 bytes, adding only immutable
`batch_policy_id`). They do not enable those actions. No minimal
SelectedCandidate successor is allocated: action 15 remains unable to mint
legacy `0x7c/1` and must eventually hand the winning certificate into the
separately reviewed counted settlement root.

1. `BeginPolicy`
2. `WritePolicy`
3. `SealPolicy`
4. `AbortPolicy`

The exact payload widths are 80, 236, 40, and 40 bytes. Every payload begins
with a one-byte artifact kind and seven zero padding bytes, followed by the
full artifact identity. `WritePolicy` carries a 192-byte padded chunk and a
strict cursor. The `0x7d/1` stage remains 1,288 bytes and persists the selected
kind and its exact active body length. Policy seals to `0x7e/1`; schedule seals
to `0x93/1`; runtime policy seals as its exact raw 1,132-byte canonical codec
under a disjoint content-addressed Dealer PDA because the generic liveness
adapter consumes that exact codec. The local action values do not reuse the
pure Dealer runtime enum's zero-based representation. Catalog publication
does not by itself initialize liquidity. Dealer Initialize separately consumes
the published policy and schedule and atomically admits its seven segregated
runtime-liveness compartment accounts from present native lamports.

StructuredClaim `75/1` reserves actions 1 through 8:

1. `CreateDescriptor`
2. `WrapCanonical`
3. `WrapFull`
4. `UnwrapCanonical`
5. `UnwrapFull`
6. `CompactDonation`
7. `RedeemTerminal`
8. `RetireDescriptor`

SourceSeries `77/2` reserves disjoint owner ranges. SourcePlane V3 owns actions
1 through 12:

1. `RegisterRelease`
2. `InitializeHead`
3. `OpenRawPage`
4. `IngestBoundaryBatch`
5. `SealRawPage`
6. `InitializeWindowWork`
7. `FoldWindowPages`
8. `SealWindow`
9. `EvaluateStatistic`
10. `EmitFailureHandoff`
11. `ReopenGeneration`
12. `CloseGeneration`

Recurring Series owns actions 13 through 18:

13. `RegisterSeries`
14. `ActivateFunding`
15. `AdvanceOccurrence`
16. `LapseOccurrence`
17. `ObserveDonation`
18. `CloseFunding`

These registry names allocate local tags only; this document does not freeze
payload bytes, account lists, account codecs, transition semantics, or runtime
capabilities. Action-specific contracts may do so separately. In particular,
the non-production identity slice named below freezes a strict subset, and
actions 35 through 41 have canonical payload contracts while remaining
disabled. Actions 36 and 37 deliberately do not allocate separately callable
virtual-inventory actions: each future route must join its inventory mutation
and one real receipt end under one authenticated transition identity.
The exact recurring-Series laboratory payload codecs live in
`clutch_solana_layout::product_series`. Allocation still grants no execution
capability. The program's executable
Source/Series set remains empty. In particular, a decoded registry release ID
or capability-profile ID is not authority: registration stays disabled until
the adapter authenticates the authoritative central release, and every
value-bearing action stays disabled until its exact source, collateral,
liveness, and failure receipts are authenticated.

Dealer owns `0x7d/1` for its staged policy and `0x7e/1` for its immutable
policy. The Source/Series account namespace reserves the disjoint `0x7f/1` for
the persistent Series registration/replay anchor and `0x80/1` for the mutable
Series-funding wrapper. Their exact 168-byte and 376-byte codecs are fixed but
reserved-disabled. The funding wrapper adds tag/version/bump/flags, exact
refundable account-rent principal, and five release-selected collateral-vault
rent principals around the pure 324-byte `SeriesFundingStateV1`; it does not
copy its cursor or component-balance facts.

Recovery 78/v1 reserves these local actions, all disabled:

1. `InitializeFailureRoot`
2. `TriggerSourceFailure`
3. `TriggerRelationRefusal`
4. `AdvanceRecoverySchedule`
5. `AcceptRecoveryWork`
6. `ResolveCallerFunded`
7. `ResolvePaidRecovery`
8. `CloseRecoveryFunding`
9. `CloseFailureRoot`

FractionalRedemption 79/v1 reserves these local actions, all disabled:

1. `Initialize`
2. `RedeemInternalExact`
3. `RedeemBearerExact`
4. `RedeemInternalCredit`
5. `RedeemBearerCredit`
6. `TransferCredit`
7. `MergeCredit`
8. `CloseZeroCredit`
9. `SealClaimsExhausted`
10. `CloseEmptyLedger`

The current account coordinates are `0xa4/2` for the immutable
Market/Resolution/Realm/claim policy, `0xa5/1` for the sole aggregate numerator
credit and live-credit count, `0xa6/2` for one owner-scoped canonical numerator,
and `0xa7/2` for the permanent zero-credit replay tombstone. Their exact body
widths are 296, 224, 296, and 232 bytes. Resolution owns the vector,
ClaimLedger V3 owns native claim supply, Hoard V2 owns locked claim principal
and cash classification, Position V3 and Replay V3 own internal
custody/replay, and the Realm collateral adapter owns transfers. The fractional
accounts copy none of those mutable facts; ClaimLedger and `0xa5` advance one
sequence and exact cross-account semantic-ID receipt atomically.

The earlier `0xa4/1`, `0xa6/1`, and `0xa7/1` allocations are withdrawn, not
aliases. Their identity slots meant payout-vector digests; the current V2
schemas instead commit the PDA-bound Resolution V5 data identity. No current
decoder accepts V1, no migration is defined, and the policy/credit PDA and
policy-state identity domains advance to V2. `0xa5/1` is unchanged because it
never owned either identity and remains the sole aggregate-credit owner.

The only admitted terminal policy in the runtime contract is
`RetainUntilExactAggregation`: a sub-atom remainder keeps its credits and claim
backing live. `CloseEmptyLedger` requires claims, aggregate credit, live credit
accounts, and claim backing all to be zero. It then closes both `0xa4` and
`0xa5` under one private ProductOccurrenceRoot terminal authorization, refunds
each account's stored rent payer independently, and routes only hostile or
unsolicited lamports to the neutral sink. It therefore cannot sweep Hoard
principal, reinterpret donation surplus as revenue, strand policy rent, permit
reinitialization, or silently forfeit claimant value.

Dealer facility actions `5..=25` are allocated in runtime order
`Initialize..=Retire`, while only policy transport `1..=4` is executable in the
existing non-production catalog profile. Every facility action remains
capability-disabled. StructuredClaim payload codecs are owned by its separately
integrated runtime and adapter, while Recovery payload/account contracts are
owned by its dedicated modules; this central allocation duplicates neither
contract and activates none of those actions.

## General SettlementReceipt successor allocation

The central collision ledger preserves main-account coordinate `0x0f/3` as the
withdrawn 217-byte General SettlementReceipt history. It was a fresh version
of the existing receipt tag, not a reinterpretation of `0x0f/2`; the hostile
decoders still refuse each other, but no executable route may create V3.

V3 uses the fresh PDA seed tuple
`["general-receipt:v3", Epoch_PDA, final SettlementCandidateId,
slice_index_le]`. Its former reserved-zero final byte is the independent
buy/sell accounting mask. The V2 `consumed_flags` byte keeps its meaning as
delivered-buy, delivered-sell, and exhausted. Stable accounting and delivery
transition IDs are derived from the authenticated receipt PDA under distinct
contract domains; neither ID is accepted from a caller or persisted. The
receipt data ID instead hashes the authenticated PDA plus the exact current
217-byte prestate, so both mutable latch families are committed.

V3 and same-width V4 are now historical, withdrawn schemas. The sole future
route is `0x0f/5`, exactly 298 bytes under the fresh `general-receipt:v5` PDA
domain: the complete 217-byte V4 state machine, a one-byte typed transition
kind, a 32-byte transition commitment, and the exact 48-byte deletable-rent
owner. Kind zero requires a permanently zero commitment. Kind one is the
exclusive portfolio-pair V2 route: its commitment is zero only before
delivery and becomes one nonzero immutable hash atomically with the terminal
delivery latch. The exact portfolio preimage includes the V5 pre-data ID and
all transition/post semantic fields but excludes the circular V5 post-data ID;
the post-data ID then commits the stored hash. Unknown kinds and mismatched
kind/commitment states are refused.

## General OwnerSettlement successor allocation

The central ledger preserves withdrawn `0x81/3` history for the former General
owner row. Its exact 292 bytes are tag `0x81`, version `3`, the authoritative
288-byte `clutch-owner-settlement` V3 semantic body, stored bump, and one
reserved-zero flags byte. The strict outer decoder remains available only for
historical decoding; no executable route may create or mutate it.

Coordinates `0x81/1` and `0x81/2` remain separately reserved but withdrawn.
Future routes recognize neither as V3 and perform no migration or
reinterpretation. The canonical PDA tuple is `owner-settlement:v3`, counted
Epoch PDA, final `SettlementCandidateId`, and semantic owner.

V4 added exact merge-delivery accounting but remained unable to own its close
refund because its creation plan named no persisted rent-ledger schema. It is
therefore historical and withdrawn. The sole future row is `0x81/5`, exactly
340 bytes: tag/version, the unchanged authoritative 288-byte V4 semantic body,
one exact 48-byte deletable-rent owner, stored V5 bump, and zero flags. Its PDA
and account-data identity use fresh `/v5` domains; V1 through V4 cannot alias
or enter future executable routes.

## General OrderPage V5 allocation

The central collision ledger reserves main-account coordinate `8/5` as
`ReservedDisabled`. It is exactly 4,140 bytes: the complete historical V4
4,012-byte semantic prefix and slot array followed by sixteen little-endian
Position generations. Live single and portfolio slots require a nonzero
same-index generation; empty and tombstone slots require zero. V5 and V4
hostile decoders refuse each other.

The page commitment uses `dragons-clutch/order-page/v5` and commits the exact
slot sequence plus the complete generation tail. The ordered page-set fold uses
`dragons-clutch/order-set/v5`, so neither a V4 leaf nor a V4 set can be silently
reinterpreted. Position and Reservation identities are authenticated adapter
joins and are not persisted in the page.

## General ClearWork V3 allocation

The central ledger reserves `17/3` as `ReservedDisabled` for the resumable
RelationV2 Work successor. It is not a reinterpretation of withdrawn `17/2`:
the fresh PDA domain is `clear-work:v3`, and the hostile decoders refuse the
other version.

The exact account length is `710 + 16*O + 8*N*O` bytes, at most 9,158 bytes for
16 outcomes and 64 dense live orders. The 710-byte header owns the immutable
candidate bindings, frozen-page and dense-order cursors, the previous live
order ID, a canonical SHA-256 continuation, and a checked
Pending/Valid/Refused disposition. The active-width tail owns the aggregate
buy/sell flow vectors and exactly one filled-leg row per dense live order.
This reservation does not enable actions 10 through 14 or claim settlement;
their adapter must authenticate V5 pages, the retained feed, exact Product and
price artifacts, and present-funded liveness before capability admission.

## Coordinated successor account block

The central collision ledger is the sole allocation owner for the following
coordinated successor block. Dealer policy transport rows are
`NonProductionLab`; current unactivated rows are `ReservedDisabled`; and
historical General Reservation/receipt/owner-row coordinates and withdrawn
Fractional V1 coordinates remain occupied as `Withdrawn`. An account codec or
pure runtime elsewhere does not make a route executable.

| tag/version | owner | account |
|---:|---|---|
| `0x0f/3` | settlement history | historical General receipt V3 (217 bytes); withdrawn |
| `0x0f/4` | settlement history | historical merge-payment receipt V4 (217 bytes); withdrawn |
| `0x0f/5` | General V2 | sole future rent-owned typed receipt V5 (298 bytes) |
| `0x13/5` | retirement history | withdrawn counted General Reservation V5 (627 bytes); never reinterpreted |
| `0x13/7` | retirement history | withdrawn provisional deletable General Reservation V7 (675 bytes); no live route and never reinterpreted |
| `0x13/9` | General V2 | sole future rent-owned Reservation V9 (666 bytes); V4 live creation withdrawn |
| `0x7d/1` | Dealer | staged policy |
| `0x7e/1` | Dealer | immutable policy |
| `0x7f/1` | Recurring Series | registry |
| `0x80/1` | Recurring Series | present-funding compartments |
| `0x81/1` | General V2 | withdrawn owner settlement V1; never a live alias |
| `0x81/2` | General V2 | withdrawn presence-explicit owner settlement V2; never a live alias |
| `0x81/3` | General V2 | historical Reservation-handoff owner settlement V3; withdrawn |
| `0x81/4` | General V2 | historical merge-delivery owner settlement V4 (292 bytes); withdrawn |
| `0x81/5` | General V2 | sole future rent-owned owner settlement V5 (340 bytes) |
| `0x82/1` | General V2 | selected fee record |
| `0x83/1` | General V2 | owner fee carry |
| `0x84/1` | General V2 | payer allocation |
| `0x85/1` | General V2 | recipient allocation |
| `0x86/1` | General V2 | treasury ledger |
| `0x87/1` | General V2 | settlement cash pot |
| `0x88/1` | StructuredClaim | descriptor |
| `0x89/1` | General V2 | FinalPot |
| `0x8a/1` | SourcePlane V3 | historical release without receiver deployment authentication; never executable |
| `0x8a/2` | SourcePlane V3 | receiver-release-authenticated release |
| `0x8b/1` | SourcePlane V3 | head |
| `0x8c/2` | SourcePlane V3 | release/route-bound reopen lineage |
| `0x8d/1` | SourcePlane V3 | open raw page |
| `0x8e/1` | SourcePlane V3 | immutable raw page |
| `0x8f/1` | SourcePlane V3 | window work |
| `0x90/1` | SourcePlane V3 | window seal |
| `0x91/1` | SourcePlane V3 | statistic result |
| `0x92/1` | SourcePlane V3 | liveness work receipt |
| `0x93/1` | Dealer | immutable liveness schedule (380 bytes) |
| `0x94/1` | Dealer | authoritative State V2 with persisted terminal evidence (980 bytes) |
| `0x95/1` | Dealer | counted funded dependencies V2 (480 bytes) |
| `0x98/1` | Dealer | immutable-after-activation LP page V2 (980 bytes) |
| `0x99/1` | Dealer | selected-artifact-bound one-generation Lease V2 (1,076 bytes) |
| `0x9a/1` | Dealer | SettlementPot V2 (1,236 bytes) |
| `0x9b/1` | Dealer | counted General-generation-bound Epoch V2 (780 bytes) |
| `0x9c/1` | Dealer | page terminal allocation (756 bytes) |
| `0x9d/1` | Dealer | streamed terminal ClaimWork (1,148 bytes) |
| `0x9e/1` | Dealer | permanent root tombstone V2 (476 bytes) |
| `0x9f/1` | Dealer | owner-scoped exit ticket V1 (364 bytes) |
| `0xa0/1` | Failure | external semantic root; root rent only |
| `0xa1/1` | Liveness | immutable runtime policy |
| `0xa2/1` | Liveness | Recovery compartment; sole work/rent custody |
| `0xa3/1` | Terminal/replay | failure-generation tombstone |
| `0xa4/1` | FractionalRedemption | withdrawn payout-vector-bound policy; never a V2 alias |
| `0xa4/2` | FractionalRedemption | immutable Resolution-V5-data-bound policy (296 bytes) |
| `0xa5/1` | FractionalRedemption | sole aggregate numerator-credit ledger (224 bytes) |
| `0xa6/1` | FractionalRedemption | withdrawn payout-vector-bound credit; never a V2 alias |
| `0xa6/2` | FractionalRedemption | Resolution-V5-data-bound exact numerator credit (296 bytes) |
| `0xa7/1` | FractionalRedemption/replay | withdrawn payout-vector-bound tombstone; never a V2 alias |
| `0xa7/2` | FractionalRedemption/replay | Resolution-V5-data-bound zero-credit tombstone (232 bytes) |
| `0xa8/1` | Dealer | immutable deletable action-work receipt (540 bytes) |
| `0xa9/1` | General V2 | counted candidate-scoped SettlementRoot V1 (980 bytes) |
| `0xaa/1` | Product | reserved occurrence-scoped terminal root |

`0x96/1` and `0x97/1` remain unallocated. Dealer uses the canonical global
Position V3 and purpose-owned Replay V3 families rather than minting local
account-body duplicates at those coordinates.

The failure root never aliases `0xa2`, holds recovery work principal, or emits a
keeper transfer. Accepted work rewrites the failure root and Recovery
compartment atomically; only the latter is debited for the keeper payment and
payer headroom refund.

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
version, local action)`. Production profiles retain empty successor executable
sets. The distinct
`profile-non-production-dealer-policy-catalog-lab` identity enables only
`(76,1,1..=5)` and `(76,1,12)`: the four policy transport actions plus exact
facility Initialize and BindEpoch. The separate
`profile-non-production-general-v2-empty-book-identity-lab` enables only the
actions listed in `GENERAL_V2_SBF_VERTICAL_SLICE.md`; all other allocated
General actions return `UnsupportedInstruction` before their handlers read
accounts. Every production profile returns `UnsupportedInstruction` before
reading accounts for disabled successor actions. Every allocated Source/Series action also returns
`UnsupportedInstruction` before account reads. Unknown family versions and
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

General V2 local actions 1 through 41 are allocated numeric coordinates, not a
blanket activation. Actions 2, 6, 7, 8, 9, 10, 14, 15, 20, 21, and 32 are
confined to the named non-production profile. Actions 35 through 41 have
frozen canonical payload contracts but remain `ReservedDisabled`. Every other
General V2 action remains allocation-only. Unlisted future local-action proposals, and
every proposed account shape, stay outside the central ledger until their
atomic review is complete. Source/Series V2 local actions 1 through 18 are
likewise reserved-disabled allocations; a frozen laboratory payload codec does
not grant execution capability.
