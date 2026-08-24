# General V2 SBF vertical slice

Status: source implementation in the isolated non-production profile. Actions
2, 6, 7, 8, 9, 10, 14, 15, 16, 20, 21, and 32 have strict payload decoders, pure
poststate owners, SBF dispatch/account handlers, and fresh PDA helpers. Current
production profiles still disable every General V2 action. A real SBF build
and committing local-bank campaign have not yet been run for this source
checkpoint, so this status is not execution evidence.

This document defines the smallest honest current-head path from the pure
General V2 contracts to a committing local-validator execution. The first
executable target is deliberately an **identity-only lab**: a signed,
committing SBF candidate lifecycle over a genesis-assisted Market and an empty
RelationV2 book, ending at a materialized `SelectedCandidateV1` authority whose
authenticated settlement-candidate identity matches an independent
recomputation.

That target is not market creation, order placement, trading, clearing,
liquidity, entitlement, token settlement, source/provider integration,
deployability evidence, or mainnet evidence. Those capabilities require later
vertical slices and must remain disabled until their own evidence gates pass.

The current source successor is narrower and stronger than the original
degree-zero-through-three price declaration below. It admits only degree-two
and degree-three bases to candidate ranking because those are the degrees
covered by the exact finite production atom-mixture verifier. The successor
Relation policy identity commits that restriction and the certificate profile.
`InitClearWork` verifies the sealed feed against that certificate before
creating resumable work. Every streamed-order and settlement-slice resume
remints the full Product/Grid capability before consuming Work, and terminal
completion repeats the same admission before projecting ScoreV2-Q. ClearWork V3's
retained identities are not treated as proof of a verifier version it predates.
The witness body stays outside candidate identity; the exact semantic price and
successor policy stay inside it. This checkpoint has not been built or executed
and does not activate a production profile.

## 1. Why this is a separate profile

The General V2 extension family is centrally reserved at outer intent family
tag 74 decimal (`0x4a`), family version 1. Its family-local actions are
allocated at `1..=38`. Allocation does not authorize execution.

Every production SBF profile has an empty General V2 extension capability table.
The existing `profile-general-source-v2-point` is the legacy General V1
program, not a General V2 successor. Adding V2 success paths under that label
or its existing profile identity would change release semantics without
changing release identity.

The first success-capable source build uses the mutually exclusive feature and
fresh profile identity:

```text
profile-non-production-general-v2-empty-book-identity-lab
```

All production profiles continue to reject every General V2 local action as
`UnsupportedInstruction` before inspecting accounts. The lab profile enables
only the exact actions listed in section 7.

## 2. Current pure-contract allocations

The current pure contract is `no_std`, allocator-free, safe Rust, and contains
no Solana SDK, account memory, CPI, clock, rent, signature, or PDA code. Its
account bytes are contracts for a future adapter, not live routes.

`programs/solana-layout` remains the global wire-allocation owner. The General
contract's tag/version constants mirror its reserved
coordinates; `solana-layout` must not gain a dependency on the General crate
merely to compare those constants. Actual activation instead requires explicit
adapter parity assertions and frozen cross-crate vectors without perturbing
the pinned layout dependency graph.

| Semantic owner | Tag/version | Exact active length |
|---|---:|---:|
| `MarketRuntimeV3AccountV1` | `3/3` | 148 |
| `GeneralEpochV6AccountV1` | `11/6` | 321 |
| `ClearWorkV2` | `17/2` | `672 + 16*O + 8*N*O`, maximum 9,120 |
| sealed `CandidateFeedV2` | `18/2` | `538 + 8*O + 8*N + 24*A + 13*S`, maximum 6,970 |
| `CandidateWindowV4` | `24/4` | 565 |
| `CandidateFeedStageV2` | `25/2` | same active length as the sealed feed |
| `AdmissionNodeV3AccountV1` | `0x77/1` | 743 |
| `EpochBudgetV2AccountV1` | `0x78/1` | 272 |
| immutable `MarketBindingV1` | `0x79/1` | 540 |
| counted-retirement Replay successor | `0x7a/1` | 132 |
| immutable `EconomicDomainV2AccountV1` | `0x7b/1` | 297 |
| `SelectedCandidateV1AccountV1` settlement authority | `0x7c/1` | 789 |
| disabled `OwnerSettlementV1AccountV1` envelope | `0x81/1` | 292 |
| disabled selected fee-record envelope | `0x82/1` | 340 |
| disabled owner fee-carry envelope | `0x83/1` | 132 |
| disabled temporary payer-allocation envelope | `0x84/1` | 2,684 |
| historical recipient-allocation envelope | `0x85/1` | 2,644 |
| historical complete-fee-book recipient certificate | `0x85/2` | 2,764 |
| current V2-stream/Hamilton recipient allocation | `0x85/3` | 2,796 |
| disabled treasury-ledger envelope | `0x86/1` | 148 |
| disabled buyer-first settlement cash-pot envelope | `0x87/1` | 260 |
| disabled combined FinalPot/virtual-budget envelope | `0x89/1` | 332 |

Here `O` is the active outcome count, `N` the active order count, `A` the
active quantized-atom count, and `S` the active settlement-slice count. Feeds
and work accounts contain no maximum-width inactive tail. The maximum
ClearWork allocation is below 10 KiB, so the old `GrowClearWork` shape is not
needed.

The selected authority's exact width is
`2 + 19*32 + 96 + 3*8 + 2*2 + 48 + 7 = 789`: header, nineteen identities,
rank, generation/ordinal/slot, slice count and cursor, rent owner, and seven
one-byte semantic/status fields. The slice fields are required so its retained
Feed cannot retire before every slice has materialized into counted settlement
state.

The central collision ledger reserves these coordinates as
`ReservedDisabled`, records retirement's provisional tombstones at `0x75/1`
and `0x76/1` plus the permanent Position tombstone at `0x75/2`, and proves its
recorded rows internally disjoint. Dealer owns `0x7d/1` and `0x7e/1`, while
Source/Series owns the `0x7f` and `0x80` coordinates (with `0x7f/1` withdrawn
and `0x7f/2` current); General does not reinterpret those coordinates. A complete
legacy-account inventory cross-check remains an activation gate. The numeric
constants in the standalone General crate describe matching codec bytes, not a
second allocation authority; the eventual adapter must add an explicit parity
gate against the central registry.

The same central block reserves the StructuredClaim descriptor at `0x88/1`
and a fresh General FinalPot at `0x89/1`. The FinalPot now has a strict
332-byte outer codec around the canonical 328-byte combined FinalPot and
selected virtual-budget body, but neither reservation supplies a live route or
FinalPot retirement authority in this lab. SourcePlane V3 owns
the immediately following `0x8a/1` through `0x92/1` block; General does not
reinterpret those release, head, lineage, raw-page, work, seal, statistic, or
liveness-receipt coordinates. Dealer allocations begin only after that
coordinated block is complete.

The FinalPot body is exactly five 32-byte slots (Market, Epoch, final
candidate, owner/order-set digest, and the optional virtual relation witness), cash principal,
`internal[16]`, authorized/processed/sequence `u64`s, four one-byte
outcome/phase/kind/state fields, and four zero bytes. Its outer decoder also
requires typed adapter facts for the derived FinalPot PDA and bump, both
program owners, the writable bit, and the exact decoded SelectedCandidate PDA.
The stored Epoch, Market, final candidate, and non-`None` relation witness must
equal that selected authority before create, mutation, or close.

The first implementation checkpoint additionally froze and centrally reserved:

- Market semantic tag `3`, fresh version `3`: a RelationV2-native mutable
  Market runtime/cursor that points to the immutable `MarketBindingV1` and
  preserves the full `MarketInstanceV2Id` join.
- Epoch semantic tag `11`, fresh version `6`: a RelationV2-native counted
  General Epoch with full MarketInstanceV2-derived semantics, phase,
  generation, frozen order-set identity, and exhaustive child counts.

The retirement Market V2 and Epoch V5 are not substitutes: Market V2 inherits the legacy
lowered Market identity, while Epoch V5 composes a legacy General Epoch whose
relation version is one. No adapter may reinterpret either as RelationV2.

## 3. Semantic owners that must exist before SBF mutation

### 3.1 General-specific candidate transitions

Current CandidateLifecycle V4 is incompatible with General V2. It validates a
rank whose suffix is the candidate/node identity. General V2's canonical rank
has 88 active bytes inside a 96-byte container:

1. risk flow, `u64` big-endian, maximized;
2. complemented cash flow, `u64` big-endian;
3. complemented virtual churn, `u64` big-endian;
4. complemented final `SettlementCandidateId`, 32 bytes;
5. complemented first-admitted coordinate, 32 bytes, encoded as 24 zero bytes
   followed by the Window-owned one-based ordinal in big-endian form;
6. eight canonical zero padding bytes.

The General node also owns generation, commitment-opened bundle identity,
final/base/witness identities, funding compartments, and the admission
ordinal. The SBF program must call a fresh General-specific pure transition
owner. It must not project into CandidateLifecycle V4 or reproduce a second
transition implementation inside instruction handlers.

This rank selects the **best valid submitted candidate**. It is not an
optimality certificate and must not be called optimal clearing.

### 3.2 Economic and candidate digest ownership

`EconomicDomainV2AccountV1` is the single persisted owner of its exact
per-Epoch transcript, including the inclusive integer-coordinate minimum and
maximum projected from Genesis V2. The adapter constructs that typed transcript
and hashes:

```text
"dragons-clutch/economic-domain/v2\0" || encoded_transcript
```

It never hashes a caller-supplied opaque preimage. The portable and native SHA
backends need frozen differential vectors before SBF activation.

RelationV2 is the sole semantic owner of the exact price-semantics digest. The
dependency-free General contract mirrors its fixed transcript byte for byte:
the projected RelationV2 domain followed by all sixteen prices, including
validated zero padding outside the active outcome prefix. Differential tests
cover widths two and sixteen plus field, padding, and simplex mutations.
Additional single-owner transcripts are still required:

- an Epoch/order-set transcript over the economic-domain identity and exact
  canonical active order bytes/count; the empty book must have a nonzero,
  domain-separated digest;
- an exact certificate-body projection into PriceMeasure V3 and its body
  digest;
- a base RelationV2 candidate transcript independent of settlement slices;
- a settlement-witness transcript over the base RelationV2 candidate identity
  and exact active slices; the empty slice list must still have a nonzero
  digest;
- a candidate-bundle transcript over the economic domain, route, exact price
  and relation claims, and every active feed-tail element.

There must be no digest cycle. For a Direct candidate, the final settlement
candidate identity equals the recomputed base RelationV2 candidate identity.
A CoveredDealer identity is a distinct checked dealer-economic digest and
stays disabled until the dealer adapter is atomic end to end.

### 3.3 Product authentication

Every Epoch joins the full typed identities selected by
`MarketBindingV1`: `MarketGenesisProfileV2Id`, `MarketInstanceV2Id`,
`SeriesPlanV5Id`, `SeriesFundingTermsV2Id`, RelationV2 policy,
`PriceMeasurePolicyV1Id`, `NativeClaimBasisV1Id`, admission policy, score
policy, settlement policy, and neutral sink.

The identity-only lab may genesis-load exact Product bodies and a derivation
manifest. The SBF verifier must still decode the exact `NativeClaimBasisV1`
body, recompute its 2,352-byte content identity, and match the binding. This is
a named genesis-assistance trust boundary, not blank-bank market creation.

The current artifact transport caps bodies at 1,656 bytes, so it cannot upload
a NativeClaimBasisV1. Before real market creation, the artifact plane needs
typed successor support for at least NativeClaimBasisV1 and
PriceMeasurePolicyV1, and eventually MarketGenesisProfileV2 and SeriesPlanV5.
It must authenticate exact codecs and content-derived PDAs rather than adding a
generic blob kind.

The legacy selected Product policy admits QuantizedIntegerGrid V3 degrees zero
through three. The exact finite Relation successor deliberately narrows live
candidate admission to degree two and three, witness schema V3, and quantized
semantics V1; it has no continuous or floating-point fallback. Degree zero and
one require their own equally exact admitted-certificate profile before they
can re-enter successor ranking.

### 3.4 RelationV2 verification

The existing RelationV2 verifier is a bounded one-shot reference. The General
ClearWork SHA checkpoint is a resumable digest checkpoint, not a complete
streaming RelationV2 aggregate. It lacks the persisted relation aggregates
needed to verify a generic nonempty book across transactions.

The identity-only lab may use a measured one-shot RelationV2 check for exactly
zero orders and zero settlement slices. Production activation requires either
a byte-native streaming RelationV2 successor with differential equivalence to
the reference or measured proof that one-shot verification covers the entire
advertised bound. A lab-specific one-shot success is not evidence of real
trading or clearing.

## 4. PDA and authority rules

The following frozen first-spine seed tuples are canonical. Every account is
derived with the checked program ID; the actual account key and stored bump
must match the canonical derivation. A caller-provided expected address is
never authority.

| Account | Canonical seeds |
|---|---|
| MarketBinding | `["general-market-binding:v1", MarketInstanceV2Id]` |
| MarketRuntime | `["general-market-runtime:v1", MarketBinding_PDA]` |
| Epoch | `["general-epoch:v2", MarketBinding_PDA, epoch_index_le]` |
| EconomicDomain | `["economic-domain:v2", Epoch_PDA]` |
| Window | `["general-window:v4", Epoch_PDA]` |
| Budget | `["candidate-budget:v2", Epoch_PDA]` |
| AdmissionNode | `["general-candidate-admission:v1", Epoch_PDA, ordinal_le]` |
| Feed or FeedStage | `["candidate-feed:v2", AdmissionNode_PDA]` |
| ClearWork | `["clear-work:v2", AdmissionNode_PDA]` |
| SelectedCandidate | `["selected-candidate:v1", Epoch_PDA, settlement_candidate_id]` |
| OwnerSettlement | `["owner-settlement:v1", Epoch_PDA, settlement_candidate_id, semantic_owner]` |
| selected fee record | `["selected-fee-record:v1", SelectedCandidate_PDA]` |
| owner fee carry | `["owner-fee-carry:v1", selected_fee_record_PDA, semantic_owner]` |
| temporary payer allocation | `["owner-payer-allocation:v1", selected_fee_record_PDA, semantic_owner]` |
| current recipient allocation | `["candidate-recipient-allocation:v1", selected_fee_record_PDA]`; `0x85/3` is the sole current creation target and its same-rollback SBF writer must consume the authenticated traversal-backed weight stream |
| treasury ledger | `["fee-treasury-ledger:v1", selected_fee_record_PDA]` |
| settlement cash pot | `["settlement-cash-pot:v1", Epoch_PDA, settlement_candidate_id]` |
| FinalPot | `["general-final-pot:v2", Epoch_PDA, settlement_candidate_id]` |

The exported order-page, reservation, and receipt prefixes do not freeze their
suffix tuples. Those remain unallocated until their complete
successor handler contracts exist. The OwnerSettlement tuple and account bytes
are centrally reserved but runtime-disabled pending the complete authenticated
order/fee projection described below.

Window assigns the next one-based ordinal atomically before deriving the node.
Feed and Work then inherit the node identity. This permits economically funded
duplicates while ensuring neither submitter, commitment, reveal secret, nor
candidate digest can grind the node address or the rank tie. The fresh tuple
deliberately supersedes ADR-0008's submitter/commitment-derived
`candidate-admission-v3`; no adapter may mix the two derivations.

The commitment preimage remains exactly:

```text
"dragons-clutch/candidate-commitment/v1"
|| epoch
|| market
|| relation_policy
|| admission_policy
|| score_policy
|| frozen_slot_le
|| submitter
|| solver_destination
|| candidate_bundle_digest
|| secret32
```

This transcript authenticates the later reveal opening only. It is not a PDA
seed, does not choose the Window ordinal, and does not contribute an
address-controlled rank suffix. Rank uses the checked final settlement
candidate identity followed by the Window-owned ordinal.

The adapter must enforce only semantically forbidden aliases. It must not use a
blanket all-accounts-distinct rule because payer, submitter, solver, keeper, and
refund destinations may intentionally coincide. All lamport credits to one
destination are coalesced before mutation.

## 5. Exact extension payloads

The frozen request wrapper remains unchanged. The General extension inner wire
is:

```text
family_tag=74 || family_version=1 || local_action || action_payload
```

The maximum action payload is 399 bytes. Every decoder rejects truncated
input, trailing bytes, unknown variants, noncanonical padding, cursor mismatch,
count/record-width mismatch, and arithmetic overflow.

These are frozen codec contracts. The live source handlers are exactly the
capability set in section 7; payload facts for disabled actions 24 through 26
and 36 through 38 do not create a success route.

| Local action | Exact payload |
|---:|---|
| 2 `InitEpoch` | `market_instance_v2_id[32] || epoch_index u64_le || freeze_deadline_slot u64_le` (48 bytes) |
| 6 `FreezeEpoch` | `epoch_semantics_id[32]` |
| 7 `BeginCandidate` | `epoch[32] || commitment[32]` |
| 9 `SealCandidate` | `epoch[32] || node[32]` |
| 10 `InitClearWork` | `epoch[32] || node[32]` |
| 14 `CompleteCandidateVerification` | `epoch[32] || node[32]` |
| 15 `FinalizeSelection` | `epoch[32]` |
| 16 `ExpireCommittedCandidate` | `epoch[32] || node[32]` |
| 20 `CleanupCandidate` | `epoch[32] || node[32] || selected_candidate[32]` |
| 21 `ClaimSolver` | `epoch[32]` |
| 24 `FreezeEntitlement` (disabled selector) | `epoch[32] || settlement_root[32]` (64 bytes) |
| 25 `AccountReceiptEnd` (disabled selector) | `epoch[32] || settlement_root[32] || owner_settlement[32] || receipt[32]` (128 bytes) |
| 26 `ConsumeDirectReceiptEggs` (disabled selector) | `epoch[32] || receipt[32]` (64 bytes) |
| 32 `CloseClearWork` | `epoch[32] || node[32]` |
| 36 `ConsumeVirtualSplitReceiptEggs` (disabled selector) | `epoch[32] || receipt[32]` (64 bytes) |
| 37 `ConsumeVirtualMergeReceiptEggs` (disabled selector) | `epoch[32] || receipt[32]` (64 bytes) |
| 38 `FinalizeOwnerSettlement` (disabled selector) | `epoch[32] || settlement_root[32] || owner_settlement[32] || position[32] || settlement_cash_pot[32]` (160 bytes) |

Local action 8, `WriteCandidateFeed`, is a strict tagged union.

Variant zero opens the commitment and the stage. Its 336-byte payload is:

```text
variant=0 u8
|| epoch[32]
|| node[32]
|| secret[32]
|| candidate_bundle_digest[32]
|| settlement_candidate_id[32]
|| base_relation_candidate_id[32]
|| settlement_witness_digest[32]
|| candidate_price_digest[32]
|| price_body_digest[32]
|| virtual_split u64_le
|| virtual_merge u64_le
|| honored_aon_mask u64_le
|| price_scale u64_le
|| common_denominator u64_le
|| basis_degree u8
|| outcome_count u8
|| order_count u8
|| atom_count u8
|| slice_count u16_le
|| candidate_kind u8
```

Variants one through four write prices, fills, quantized atoms, and settlement
slices respectively:

```text
variant u8
|| epoch[32]
|| node[32]
|| cursor u16_le
|| count u16_le
|| exact_records
```

The fixed portion of a later write is 69 bytes, leaving at most 330 record
bytes. Prices and fills are 8 bytes each, atoms are 24 bytes each (`u128`
coordinate then `u64` mass, little-endian), and settlement slices are 13 bytes
each. A write must start at the persisted sequential cursor and end at the
cursor implied by its exact record count.

## 6. Ordered account metas and transitions

The order below is part of each action contract. Every handler first checks the
exact meta count, signer/writable/executable flags, owners, codec bytes,
identities, PDAs, bumps, clocks, aliases, rent, and aggregate balances.
These ordered lists describe the current source handler contract. They do not
claim that a built ELF or local-bank transaction has exercised it yet.

### 6.1 `InitEpoch`

0. payer, signer and writable
1. immutable MarketBinding, read-only and program-owned
2. MarketRuntime, writable and program-owned
3. vacant Epoch PDA, writable
4. vacant EconomicDomain PDA, writable
5. vacant Window PDA, writable
6. vacant Budget PDA, writable
7. authenticated NativeClaimBasis artifact, read-only and program-owned
8. authenticated MarketGenesisProfileV2 artifact, read-only and program-owned
9. authenticated PriceMeasurePolicyV1 artifact, read-only and program-owned
10. System program, read-only and executable
11. Rent sysvar, read-only
12. Clock sysvar, read-only

Authenticate the full MarketInstanceV2 join through the MarketBinding PDA and
require the canonical RelationV2 and ScoreV2-Q policy identities, a smooth
degree-two or degree-three basis, and the exact next Epoch index. Derive the
four frozen child PDAs,
calculate full rent principals without
discounting hostile prefunds, route prefunds into donation compartments,
prepay the fixed 789-byte SelectedCandidate rent principal into the Budget's
distinct selected-rent compartment, create and encode every account atomically,
and advance the Market runtime cursor exactly once. Budget records the exact
funding payer; selected-artifact rent is neither a future fee nor a keeper
top-up.

### 6.2 `FreezeEpoch`

0. Epoch, writable
1. EconomicDomain, read-only
2. Window, writable
3. Budget, writable
4. MarketBinding, read-only
5. Clock sysvar, read-only
6. keeper reward destination, writable

At or after the Epoch freeze deadline, authenticate the canonical empty-book
order-set identity, stamp the actual freeze slot and the commit/reveal/
submission/verification boundaries `F/R/S/V`, transition the Epoch to Frozen,
and pay only the present-funded freeze reward.

### 6.3 `BeginCandidate`

0. payer, signer and writable
1. submitter, signer and read-only
2. refund destination, read-only
3. solver destination, read-only
4. Epoch, writable
5. Window, writable
6. MarketBinding, read-only
7. vacant AdmissionNode PDA, writable
8. System program, read-only and executable
9. Rent sysvar, read-only
10. Clock sysvar, read-only

Only `[F,R)` admits a commitment. Compute the next one-based Window ordinal and
prior reverse-head link, derive the node from the fresh
`[general-candidate-admission:v1, Epoch PDA, ordinal_le]` tuple, fund exactly
Node refundable rent principal, bond, and node-close reward, create the node,
and update the Epoch/Window counts and head in the same transaction. The
commitment is stored in the node but is not part of its address.

Candidate dimensions are hidden at this point. A commit cannot truthfully
preprice feed rent or verification work.

### 6.4 `WriteCandidateFeed`, open variant

0. recorded payer, signer and writable
1. recorded submitter, signer and read-only
2. Epoch, read-only
3. Window, writable
4. MarketBinding, read-only
5. EconomicDomain, read-only
6. AdmissionNode, writable
7. vacant FeedStage PDA, writable
8. System program, read-only and executable
9. Rent sysvar, read-only
10. Clock sysvar, read-only

Only `[R,S)` admits reveal. Verify the exact commitment opening before using
any revealed dimensions. Call the pure funding decomposition with authenticated
dimensions, MarketBinding policy, and three independent rent compartments. The
reveal debit is exactly Feed refundable rent plus feed-close reward plus
ClearWork refundable rent plus the price/order/slice/completion/work-close
reward reserve. It excludes the Node rent, bond, and node-close reward already
paid at commitment. Aggregate-check that reveal debit, fully fund the FeedStage
and node work-escrow compartments, create the stage, and transition the node
and Window to revealed before writing the opening fields.

The adapter records and tests `commit_payer_funding`,
`reveal_payer_funding`, and the audit-only lifetime sum separately. The
lifetime total is never charged as a second transition debit, and hostile
prefund floors never discount refundable payer principal.

Later action-8 writes require the submitter signer, Epoch/Window/Node read-only,
FeedStage writable, and Clock read-only. They update only the exact sequential
tail and its cursor.

### 6.5 `SealCandidate`

0. submitter, signer and read-only
1. Epoch, read-only
2. Window, read-only
3. AdmissionNode, read-only
4. FeedStage, writable
5. MarketBinding, read-only
6. EconomicDomain, read-only
7. Clock sysvar, read-only

Require every active cursor complete. Validate the price simplex, strictly
ordered positive atoms, primitive mass scale, slice shape, Product and economic
joins, and exact candidate-bundle/body/witness digests. On success change only
the stage tag/version into the sealed-feed coordinates. Sealing makes no
economic-validity verdict.

### 6.6 `InitClearWork`

0. keeper reward destination, writable
1. Epoch, writable
2. Window, read-only
3. MarketBinding, read-only
4. EconomicDomain, read-only
5. AdmissionNode, writable
6. sealed CandidateFeed, read-only
7. authenticated NativeClaimBasis artifact, read-only
8. vacant ClearWork PDA, writable
9. System program, read-only and executable
10. Rent sysvar, read-only
11. Clock sysvar, read-only
12. canonical PriceGrid PDA, read-only and program-owned
13. authenticated ProductTemplateV4 artifact, read-only and program-owned
14. authenticated MarketGenesisProfileV2 artifact, read-only and program-owned
15. authenticated PriceMeasurePolicyV1 artifact, read-only and program-owned
16. content-addressed MarketInstancePreimageV2 artifact, read-only

This action is permissionless. Move the exact pre-funded work rent and reward
reserve from the program-owned node compartment, allocate and assign the exact
active-width Work PDA, decrement node escrow, and increment the Epoch's work
count atomically. Before any allocation, the successor handler authenticates
the complete MarketInstance/Template/Basis/Policy/Genesis/coordinate-domain
tuple, exact PriceGrid PDA and active tick membership, successor Relation and
Score policies, and the finite production atom-mixture certificate. The
isolated handler owns this 17-account expectation; shared account-meta and
capability registration must adopt the tuple atomically before enabling the
route. Do not reinstate the legacy roughly 50 KiB staged-grow path.

#### 6.6a `AdvanceClearOrders` / `AdvanceClearSlices` successor seam

0. keeper reward destination, writable
1. Epoch, read-only
2. Window, read-only
3. MarketBinding, read-only
4. EconomicDomain, read-only
5. AdmissionNode, read-only
6. sealed CandidateFeed, read-only
7. ClearWork V3, writable
8 through `8 + page_count - 1`. complete canonical OrderPage V5 set, read-only
`8 + page_count`. Clock sysvar, read-only
`9 + page_count`. canonical PriceGrid PDA, read-only and program-owned
`10 + page_count`. authenticated ProductTemplateV4 artifact, read-only
`11 + page_count`. authenticated NativeClaimBasis artifact, read-only
`12 + page_count`. authenticated MarketGenesisProfileV2 artifact, read-only
`13 + page_count`. authenticated PriceMeasurePolicyV1 artifact, read-only
`14 + page_count`. authenticated MarketInstancePreimageV2 artifact, read-only

Both actions use the same exact account list and count: `15 + page_count`, for
`1 <= page_count <= 4`.
Before borrowing any OrderPage body, a separate bounded frame authenticates
content-derived Product artifact PDAs, the canonical Grid PDA and every active
price tick, recreates the finite atom-mixture capability, and joins its full
MarketBinding/Genesis/MarketInstance/feed/domain/body/price/policy/basis facts
to ClearWork. ClearWork V3 retaining the same IDs, or merely naming the
successor Relation policy, is not a substitute for that call-local capability.
The raw V1 order and slice transitions remain available to legacy callers only;
the successor SBF source calls the capability-requiring V2 wrappers.

### 6.7 `CompleteCandidateVerification`

0. keeper reward destination, writable
1. Epoch, read-only
2. Window, writable
3. MarketBinding, read-only
4. EconomicDomain, read-only
5. AdmissionNode, writable
6. sealed CandidateFeed, read-only
7. canonical PriceGrid PDA, read-only and program-owned
8. authenticated ProductTemplateV4 artifact, read-only and program-owned
9. authenticated NativeClaimBasis artifact, read-only and program-owned
10. authenticated MarketGenesisProfileV2 artifact, read-only and program-owned
11. authenticated PriceMeasurePolicyV1 artifact, read-only and program-owned
12. authenticated MarketInstancePreimageV2 artifact, read-only and program-owned
13. ClearWork, writable
14. Clock sysvar, read-only

Only `[S,V)` admits ordinary completion. The active-width lab consumes
`clutch-general-v2-runtime::verify_quantized_relation_product_price_admission_v2`
and `verify_quantized_relation_candidate_v2` over the exact empty or projected
active-width RelationV2 book. It also exact-joins the call-local Product/Grid
capability to terminal ClearWork before ranking. That private-construction result joins the full
Product/Genesis/MarketInstance/PriceGrid bodies, canonical policy IDs,
the V3 quantized witness, owner-blind RelationV2, and ScoreV2-Q. A well-formed,
authenticated but economically invalid candidate returns success after
terminalizing the node as `VerifiedRefused`. Malformed bytes, bad authority,
broken binding, or corrupt persisted state return an error and roll back.

A valid completion recomputes the base and final identities and obtains the
88-byte rank from the private checked wrapper. The mutation owner independently
re-encodes the score fields with the authenticated Node ordinal, and the
adapter requires byte equality before writing. It then terminalizes the node,
updates Window counts and best node, and pays only present-funded checked-work
rewards. The adapter never trusts a candidate-supplied score, rank, ordinal, or
final identity.

### 6.8 `ExpireCommittedCandidate`

0. Epoch, read-only
1. Window, writable
2. unrevealed committed AdmissionNode, writable
3. Clock sysvar, read-only

Expiry is permissionless and moves no lamports. At or after submission close,
the adapter authenticates the exact Epoch, Window, and ordinal-derived Node
PDAs, codecs, owners, generations, policy IDs, frozen and committed slots,
count parity, and each account's complete present-funded balance. The pure
owner accepts only a `Committed` node admitted before reveal-open, records the
current slot as its terminal slot, changes its state to `ExpiredCommitment`,
and increments only Window's checked expired-commitment count. Epoch candidate
count and Window live-node count remain unchanged until action 20 unlinks the
terminal reverse-list head.

### 6.9 `FinalizeSelection`

0. Epoch, writable
1. Window, writable
2. Budget, writable
3. best AdmissionNode, read-only
4. its sealed CandidateFeed, read-only
5. MarketBinding, read-only
6. EconomicDomain, read-only
7. vacant SelectedCandidate PDA, writable
8. finalizer reward destination, writable
9. System program, read-only and executable
10. Rent sysvar, read-only
11. Clock sysvar, read-only

Finalization is permissionless; no payer or finalizer signature is required
because root rewards and SelectedCandidate rent are already capitalized.

At or after `S`, finalization is allowed only if every admission is already
terminal; otherwise it waits until at or after `V`. Authenticate the Window's
best node, its retained sealed Feed, every copied binding, final identity, and
rank. Move the exact prepaid SelectedCandidate rent from Budget, record
Budget's funding payer and the Selected PDA's hostile-prefund donation floor,
create the SelectedCandidate atomically, increment the Epoch selected-artifact
count, zero every Window working-best field, and store only the
SelectedCandidate pointer in the Window. Then pay the finalization reward and
transition the Epoch to `CandidateSelected`.

Finalization copies the authenticated solver reward destination from the node
but does not pay it. The unique solver prize remains present-funded in Budget
until local action 21 consumes it.

### 6.10 `ClaimSolver`

0. Epoch, read-only
1. Window, read-only
2. Budget, writable
3. SelectedCandidate, read-only
4. the SelectedCandidate's solver reward destination, writable

The claim is permissionless. It authenticates the finalized counted graph,
every PDA and generation join, the Window's selected pointer, and the exact
immutable destination copied into SelectedCandidate. It consumes only the
Budget's present-funded solver compartment and marks its one-way state paid.
No caller signature can redirect the prize.

### 6.11 `CloseClearWork`

0. Epoch, writable
1. MarketBinding, read-only
2. terminal AdmissionNode, read-only
3. terminal ClearWork, writable
4. keeper reward destination, writable
5. recorded Work-rent payer, writable
6. immutable neutral sink, writable

Only the bounded zero-order/zero-slice terminal Work owned by a verified-valid
or checked-refused node can close. The pure owner authenticates every graph,
generation, policy, reward, and active-width join, decrements Epoch's counted
Work, and returns destination-coalesced rent-principal, donation, and close-
reward credits before the adapter releases the account.

### 6.12 `CleanupCandidate`

0. Epoch, writable
1. Window, writable
2. MarketBinding, read-only
3. reverse-head AdmissionNode, writable
4. derived sealed Feed, writable, or canonical absent account
5. derived ClearWork, canonical absent account
6. SelectedCandidate, read-only, or System-program sentinel
7. keeper reward destination, writable
8. recorded Node-rent payer, writable
9. recorded candidate refund destination, writable
10. immutable neutral sink, writable
11. recorded Feed-rent payer, writable; aliases node payer when Feed is absent
12. Clock sysvar, read-only

Cleanup refuses the pre-finalization working best and any node with live Work.
For the selected source it closes only Node and retains Feed under the counted
SelectedCandidate. For a non-selected terminal head it closes Node plus Feed,
or Node after canonical Feed absence. It atomically decrements the Epoch and
Window live counts, advances the reverse head, increments the closed count, and
applies only the pure owner's destination-coalesced principal, donation,
keeper, bond-refund, invalidity, and abandonment credits. Expired-unverified
cleanup remains disabled because its residual Work economics are not frozen.

Before finalization, Window exclusively owns the working best node, final ID,
rank, and ordinal. Afterwards, SelectedCandidate is the single downstream
settlement authority. It retains the sealed Feed and owns the immutable
candidate identity plus mutable, monotone `slice_count`, `next_slice_index`,
and entitlement-state progress. This is an atomic semantic-ownership transfer,
not a parallel DTO truth. No reservation, receipt, final pot, token account, or
token transfer is created by the identity-only lab.

## 7. Capability boundary

The lab enables exactly these local actions:

```text
2  InitEpoch
6  FreezeEpoch
7  BeginCandidate
8  WriteCandidateFeed
9  SealCandidate
10 InitClearWork
14 CompleteCandidateVerification
15 FinalizeSelection
16 ExpireCommittedCandidate
20 CleanupCandidate
21 ClaimSolver
32 CloseClearWork
```

All other General V2 actions remain disabled. In particular:

- action 1 `CreateMarket` is disabled because Product artifacts and blank-bank
  creation are not live;
- actions 3-5 are disabled, so there are no order pages, placements, cancels,
  or trades;
- action 11 `GrowClearWork` remains permanently disabled because active-width
  work fits under the one-creation ceiling;
- actions 12-13 now have capability-requiring exact-admission source seams.
  The existing non-production profile identity predates their hardened
  `15 + page_count` ABI and must rotate atomically before an artifact can claim
  the new semantics;
- actions 17-19, 22-31, and 33-38 remain disabled, including the remaining
  candidate terminal paths, entitlements, settlement, selected-artifact
  retirement, root retirement, and the reserved Position asset-transfer
  primitive.

Actions 24 and 25 have strict capability-disabled payload and account-envelope
codecs, including the sole future `0x81/2` OwnerSettlement envelope, but
no successful SBF transition. They cannot activate until an authenticated,
complete filled-order projection, exactly one selected fee row per canonical
owner, checked candidate totals, a canonically derived owner-order-set digest,
and exact receipt, reservation, and SelectedCandidate joins exist. Action 24
is strictly next-slice: it derives one receipt plus its one or two real ends,
creates a pristine row or entitlement stamp only on that owner's/order's first
canonical selected slice, requires the exact existing state later, and advances
the Selected slice cursor only after the receipt and all needed rows/stamps are
materialized. Thus the terminal slice cursor proves every participating owner
and selected order has materialized without a second manifest or cursor.
Action 25 is the accounting-only `AccountReceiptEnd`, not Egg delivery. Its
receipt-derived accounting identity is distinct from every later delivery
transition identity. It adds the authenticated receipt end's price units to
exactly one owner row; the receipt and selected-order projection, rather than
caller bytes, own slice, order, side, price, and completion. Accounting and
delivery therefore have independent, once-only latches.

Action 38 `FinalizeOwnerSettlement` is separately reserved because the last
receipt fragment cannot always realize an owner. It atomically joins an
accounting-complete state-zero owner row, the same owner/Market/generation
Position, the selected owner fee, and the directional candidate cash pot.
Owner net debits are source-before-sink progress and always enter the pot;
owner credits refuse without mutation when buyer or completed-merge liquidity
is not yet present, and may be retried later without consuming replay or
liveness funding. The owner row reaches state one only after the exact
Position-to-pot or pot-to-Position transfer succeeds. The request-scoped
finalized owner-row data ID is adapter-derived from that canonical 288-byte
successor; it is not accepted from the caller or copied into every row. The in-place
fee finalization receipt and row state own persistent replay safety. No
Reservation DTO is copied into this transition: its
terminal accounting is joined through the canonical row and Position facts.
The 292-byte row outer stores no duplicate rent DTO. Its pre-fund-safe creation
plan must atomically update the separate authenticated rent ledger that owns
the payer principal, refund recipient, and donation sink.

The centrally reserved `0x82/1` through `0x86/1` fee envelopes are likewise
capability-disabled. Their inner codecs re-enter the typed fee constructors;
the outer bytes add only tag/version, PDA bump, and zero flags. A separately
authenticated runtime/rent ledger must own funding, refundable principal, and
hostile-prefund disposition. Carry and payer allocation are keyed by
`(selected fee record, owner)`, never by an order, reservation, or intent. No
local action or ordered SBF meta contract has been allocated for these
accounts, so they cannot relax the zero-fee boundary or move Position value.

The `0x87/1` cash-pot envelope is also capability-disabled. Its exact
256-byte semantic body enforces buyer-first candidate-wide allocation and
segregates consideration, fees, rounding price units, and exactly one typed
virtual-cash direction. `Split` names terminal cash that funds complete-set
creation; `Merge` names opening proceeds contributed before seller
realization; `None` requires zero virtual cash. The terminal conservation
equation is `buyer debit + opening merge - seller credit = rounding + terminal
split`.
Receipt-end accounting is not value movement. No settlement action may consume
its plan until an authenticated matching complete Egg/reservation transition
is in the same atomic write set, and allocation completion does not authorize
cash-pot, owner-row, or FinalPot retirement.

Action 26 `ConsumeDirectReceiptEggs` is capability-disabled even though its
96-byte selector and complete pure direct planner are frozen. The planner
requires both real receipt ends already accounting-latched by action 25, then
atomically stages the distinct delivery latch, two Position poststates, and
two Reservation poststates while treating state-one owner rows as authenticated
read-only finalization evidence. It moves only internal native Eggs and does not
convert cash. An SBF meta contract cannot freeze until the
receipt also authenticates the exact Settlement-compartment liveness receipt,
call ordinal, quote ceiling, keeper payment, and payer refund; virtual
split/merge receipts require distinct actions and contracts.

Actions 36 and 37 reserve those distinct virtual contracts. Both use a strict
96-byte selector, but they decode into different types and cannot be routed
through action 26 or through one another. Action 36 must atomically join the
selected virtual-split authority, its complete-set split inventory mutation,
and the associated real buy receipt/Position/Reservation delivery while
authenticating the finalized buyer row without rewriting it.
Action 37 must atomically join the selected virtual-merge authority, its real
sell receipt/Position/Reservation delivery, and the complete-set merge while
authenticating the AccountingComplete state-zero seller row without rewriting
it, because the resulting merge proceeds precede seller finalization. Every
layer must name the same Epoch, selected candidate, checked
relation witness, receipt, and delivery transition ID. The FinalPot, Hoard,
aggregate claim ledger, embedded FinalPot inventory-budget cursors, receipt, Position, Reservation,
delivery latch, and Settlement liveness compartment form one rollback
boundary; the owner row/accounting latch is an immutable authorization join.
The split route additionally requires every owner row finalized and the
all-owner cash pot terminal with its exact split principal present. The merge
route contributes its opening proceeds before credit-bearing owner
finalizations may consume them.
No separately addressed budget account or callable inventory action is
allocated; the selected budget and its cursors share the canonical 328-byte
FinalPot body, lifetime, rent owner, and close authority. Exact ordered SBF metas
remain an activation blocker until the receipt codec projects the complete
selected witness and the liveness owner freezes the call ordinal, quote
ceiling, keeper payment, payer refund, and unique receipt join.

This lab can close completed Work and unlink terminal source nodes while
retaining the selected Feed. It still leaves the retained Feed,
SelectedCandidate, Window, Budget, EconomicDomain, and Epoch live. That is
acceptable only under the explicit non-production label. The retained Feed and SelectedCandidate may close only
after every selected slice has materialized into counted settlement state and
settlement retirement authenticates the terminal. Those cleanup and settlement
actions remain disabled in this lab.

Action 20 cleanup calls the pure cleanup classifier
over an exhaustive authenticated partition. Cleanup is admitted only at or
after submission close and at or after the node's terminal slot; Work must be
canonically absent. Epoch generation/count must equal Window generation/live
count, the node must be the reverse-list head, its one-based ordinal must equal
the live count, and its predecessor must equal the PDA derived from ordinal
minus one. The decision returns the exact Epoch candidate count, Window live
count, Window closed count, and Window head poststate together.

Before finalization, the working-best node returns the explicit protected
refusal without changing counts or head. After finalization, the selected
source transition closes only its Node while the counted SelectedCandidate
retains the authenticated Feed. A non-selected terminal transition either
closes Node and authenticated Feed atomically or closes Node after proving
canonical Feed absence. Generic cleanup must never destroy the working winner
or the retained settlement-data authority.

SelectedCandidate retirement is a separate atomic Feed/artifact transition.
It requires all slices materialized, entitlement state terminal, the exact
artifact/Window/Feed/Budget PDAs, matching Epoch identities and generations,
matching Feed and artifact slice counts, Budget selected-rent state already
materialized with zero remaining principal, Budget's original payer equal to
the artifact rent payer, and Budget's solver state paid or explicitly
neutralized. Only then may it close the retained Feed and artifact and
decrement the Epoch selected-artifact count. Counted receipts, reservations,
and pots—not a static client—must already own every downstream liability.

## 8. Mutation and rollback discipline

Every handler follows this order:

1. decode the exact request envelope and payload;
2. require the exact ordered account count and meta flags;
3. authenticate program/sysvar IDs and executable bits;
4. authenticate account owners, exact lengths, tags, versions, and canonical
   padding;
5. authenticate all typed IDs, generations, PDAs, actual keys, and stored
   bumps;
6. authenticate slot interval, phase, policy, and only the forbidden aliases;
7. calculate exact rent, prefund, reward, bond, and aggregate debit/credit
   arithmetic;
8. compute the complete pure poststate and funding plan without mutation;
9. perform CPI allocation/assignment, program-owned lamport movement, and
   exact encoding;
10. return success only after all planned facts and balances match.

Any instruction error rolls back every CPI, write, and lamport movement in its
transaction. Successful separate transactions are durable boundaries: a
partially written FeedStage or initialized Work account remains live. Exact
monotone cursors make such state inert and replay-safe, but they do not replace
expiry and cleanup in a production liveness argument.

Invalidity has a separate boundary. A checked economic refusal is a successful
state transition to `VerifiedRefused`; it is not represented by an instruction
error that erases the checked verdict.

## 9. Required tests before enabling the lab profile

### 9.1 Pure and codec tests

- Freeze exact coordinates, fixed lengths, active-length formulas, transcript
  domains, and golden bytes.
- Differentially compare native and portable SHA for every semantic transcript
  and resumable checkpoint boundary. At outcome widths two and sixteen, prove
  the General price mirror is byte-for-byte identical to RelationV2 over all
  sixteen price slots, including canonical inactive zero padding.
- Prove the General-specific transition owner preserves exhaustive counts,
  reverse-head deletion invariants, generation, rank padding, and first-
  admitted duplicate ordering.
- Freeze fresh AdmissionNode vectors for Epoch and ordinal mutation. Prove that
  submitter, commitment, secret, and candidate changes do not alter the node
  address for one fixed Epoch/ordinal, and refuse every attempt to substitute
  the superseded `candidate-admission-v3` tuple.
- Prove candidate funding returns disjoint commitment and reveal debits, exact
  Node/Feed/Work allocations, prefund-inclusive post-balances, and one
  audit-only lifetime total without double charging.
- Prove cleanup protects the pre-finalization working best; authenticates
  terminal time, reverse-head ordinal/predecessor, count/generation parity, and
  Work absence; and returns all four poststate facts atomically.
- Prove selected retirement refuses until slice materialization, Feed/artifact/
  Window/Budget Epoch-generation parity, selected-rent ownership, solver
  paid-or-neutralized state, and exact counted decrement all agree.
- Refuse every truncated, trailing, wrong-version, wrong-tag, noncanonical-
  padding, invalid-count, cursor, arithmetic, and digest input.

### 9.2 SVM refusal and rollback tests

For every action, snapshot all participating account bytes and lamports, invoke
one hostile mutation, and assert exact before/after equality for:

- missing or unexpected signer/writable/executable flags;
- wrong account count/order, owner, program ID, sysvar, PDA, bump, tag, version,
  length, generation, and parent identity;
- forbidden aliases and permitted actor coalescing;
- every interval boundary immediately before, at, and after `F/R/S/V`;
- underfunding, hostile prefunding, reward exhaustion, and arithmetic overflow;
- repeated, skipped, or out-of-order feed cursors;
- old node derivation, wrong Window ordinal, wrong predecessor PDA, or any
  commitment-controlled node-address substitution;
- double-charged commitment capital, reveal omission of feed/work close reward,
  or hostile prefund used to discount refundable principal;
- commitment, economic-domain, order-set, price-body, bundle, witness, base-ID,
  final-ID, candidate-price, and rank mismatch;
- wrong SelectedCandidate PDA, retained Feed, source node, prepaid rent owner,
  selected-rent state, slice count/cursor, or entitlement state;
- any finalization that fails to zero all Window working-best fields or fails
  to increment the Epoch selected-artifact count atomically;
- any selected retirement with an unmaterialized slice, mismatched retained
  Feed, open solver prize, wrong Budget/Window derivation, mismatched rent
  payer/principal, stale generation, or surviving downstream liability.

Include one transaction containing two program instructions where the first
would succeed and the second refuses; assert transaction-wide rollback. Also
pin the two intentional non-rollback cases: a failed seal leaves the stage
unchanged and live, while a well-formed economically invalid candidate commits
`VerifiedRefused`.

### 9.3 Capability tests

- Every current production profile keeps an empty General V2 extension table.
- Every allocated General V2 action under those profiles refuses before reading
  accounts.
- The lab build enables only the eight actions in section 7.
- Changing the lab action set, verifier release, Product policy, payload bytes,
  or account schema changes the lab profile identity.

## 10. Reproducible local-validator launcher

Add a new committed launcher; do not reuse or relabel the legacy V1
`run_general_committed.sh`.

1. Build the SBF program twice with no default profile and the exact lab
   feature set; compare ELF SHA-256 values.
2. Verify the pinned patched loopback validator runtime and probe its exact
   listener before continuing.
3. Create a fresh `mktemp` work directory, ledger, and test-only keypairs. Do
   not read Solana CLI wallet files or browser state.
4. Genesis-load the exact ELF plus program-owned MarketBinding, MarketRuntime,
   NativeClaimBasis, and PriceMeasurePolicy fixtures. Emit a source/derivation
   manifest containing every canonical body hash, PDA input, profile identity,
   program hash, and validator release hash. Mark the Market and Product state
   as genesis-assisted.
5. Use one frozen, independently derived, degree-two Quantized V3 price fixture
   with zero orders and zero settlement slices.
6. Submit signed `InitEpoch` and reload/decode all created accounts.
7. Advance using the validator's real local Clock until the freeze deadline;
   submit `FreezeEpoch`; independently recompute the EconomicDomain and
   canonical nonzero empty order-set identity.
8. During `[F,R)`, submit `BeginCandidate` with the exact hidden bundle
   commitment. Independently derive ordinal one from the Window prestate and
   assert the created node equals
   `[general-candidate-admission:v1, Epoch PDA, 1_u64_le]`; also assert the old
   ADR-0008 tuple is not the created address.
9. During `[R,S)`, open the commitment, write every active feed segment, and
   seal the feed. Assert Feed/Stage and Work derive only from the fresh node PDA
   (`candidate-feed:v2` and `clear-work:v2`, respectively), then reload and
   independently validate the Feed's exact active frame.
10. At or after `S`, initialize ClearWork and complete candidate verification.
    Independently recompute the price certificate, empty-book RelationV2
    candidate identity, score, and rank.
11. Because the sole admission is terminal, submit `FinalizeSelection` without
    waiting for `V`.
12. Reload exact bytes and assert: Window's working best node, ID, rank, and
    ordinal are zero; `window.selected_candidate_artifact` equals the derived
    SelectedCandidate PDA; the artifact points to the source node and retained
    sealed Feed; and its `settlement_candidate_id` equals the independently
    recomputed direct RelationV2 candidate identity. Assert the solver prize
    remains present in Budget because action 21 was not exercised.
13. Run a fresh-ledger negative scenario with one mutated atom, semantic
    digest, or expected terminal identity and require the launcher to fail red.
14. Probe listeners after the scenario, stop the exact validator process, and
    remove only the explicit temporary key and ledger paths.

The launcher may print only this success claim:

> Signed, committing local-SBF candidate-identity lifecycle on a
> genesis-assisted empty RelationV2 book.

It must print adjacent explicit negatives: no market creation, order placement,
trading, clearing, liquidity, entitlement, token settlement, live source,
deployment, or mainnet evidence was exercised.

## 11. What may land before the active lab

The following are independently useful and may land while every runtime
capability remains disabled. The frozen pure-contract/registry wave is the
first item; the remaining semantic and adapter owners are still future work:

1. land the frozen disabled General account, funding, rank, cleanup,
   selected-retirement, seed-domain, and price-mirror contracts plus their
   central `ReservedDisabled` registry rows;
2. add the missing General lifecycle transitions and Epoch/order-set semantic
   owner without projecting through CandidateLifecycle V4;
3. add exact General V2 payload codecs and hostile-wire tests;
4. add SBF metadata/PDA/authentication helpers that cannot return success;
5. add exhaustive default-disabled capability tests for every current profile;
6. add the launcher and derivation-manifest scaffold with an initial expected
   disabled-refusal scenario.

The lab success route may activate only after the new Market/Epoch codecs,
empty-book semantic owner, bounded RelationV2 verifier, Product-body
authentication, pure lifecycle transitions, selected-artifact ownership
transfer, funding arithmetic, rollback tests, unique profile identity, and
positive-plus-negative local launcher are green together.

## 12. Later real-protocol slices

The identity-only lab is a scaffolding milestone, not a substitute for the
protocol. Later slices must separately deliver and evidence:

1. authenticated Product artifact upload and blank-bank Market creation;
2. RelationV2-native counted order pages, reservation ownership, placement,
   cancellation, canonical freeze, and real funded orders;
3. generic streaming RelationV2 candidate admission and verification over
   nonempty books;
4. permissionless expiry, dependent cleanup, selected/unselected terminal
   handling, and complete rent/bond/reward retirement;
5. entitlement creation from the selected candidate identity;
6. exact integer settlement allocation, receipts, reservations, final pot,
   Token-2022 CPI, and independently checked conservation;
7. local-validator scenarios with multiple submitters, competing valid and
   refused candidates, partial fills, virtual complete-set legs, AON behavior,
   settlement slices, token balances, and full account retirement.

Only those later slices can substantiate claims about real trading, clearing,
liquidity, or settlement. Devnet execution would remain deployment evidence,
not mainnet evidence, and neither local nor devnet execution is formal
verification.
