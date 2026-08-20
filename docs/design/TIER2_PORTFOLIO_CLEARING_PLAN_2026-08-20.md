# Tier 2 — consuming the streaming batch relation on-chain (implementation plan)

Status: **PLAN / PROPOSED.** Produced 2026-08-20 by the Tier 2 planning lane
from a full read of the named ground truth; every file:line below was read,
not recalled. Executes the Tier 2 paragraph of
[FRAME_BUDGET_PLAN_2026-08-19.md](FRAME_BUDGET_PLAN_2026-08-19.md). Nothing
here is a promotion claim; every increment carries its claim plane.

**Objective in the repo's own vocabulary:**
`crates/clutch-batch/src/relation_v1_stream.rs` (HOST-TESTED, frame-measured:
`push_order` 1,280 B / `push_slice` 64 / `end_pass` 832, 19,520 verdict
comparisons at zero divergence) and `programs/solana-layout/src/clearing.rs`
(`ClearWorkAccount` 48,750 B, `CandidateFeedAccount` 6,266 B, both
consumerless) must be joined through the eight lifecycle joins named at
`programs/clutch-sbf/program/src/instructions/orders_batch.rs:269-284`, so
that `Intent::SettlePage`'s general refusal (SBF_BRINGUP item 14 / row
`0x0017`) retires and a placed `OrderSlot::Portfolio` (placeable since intent
v2 via the general `validate_place_order` arm at orders_batch.rs:796-806;
refused only by the Direct V4 branch at orders_batch.rs:888) becomes
clearable. Portfolio clearing is join-blocked, not frame-blocked; none of
this plan is stack work.

## 0. Route-family decision: Tier 2 is a NEW route family, not a V3 extension

**Decision: parallel V3's staged *shape* on the general `EpochAccount`
plane; do not extend `DirectEpochV4Account`.** The two-order constraint is
load-bearing in V3's account formats and wire, not just its checks. Every
place the V3/V2 engines constrain or conflict with general portfolio
clearing:

| Site | Constraint | Why it can't be widened in place |
| --- | --- | --- |
| `instructions/direct_selection_v3/common.rs:510-527` (`frozen_pair`) | `header.order_count == 2`, `page_index == 0`, `page_count == 1`, `tombstone_count == 0`; both slots must be `OrderSlot::Single` (Portfolio refused at 524-527) | This is V3's entire "verifier": it re-derives the unique crossing from exactly two single-Egg orders. There is no candidate/fill vector to generalize. |
| `instructions/direct_selection_v3/freeze_abort.rs:379-383` | Freeze requires `header.order_count <= 2`; `reservation_count = order_count` (max 2 reservation accounts wired) | The freeze transaction shape itself is two-order. |
| `instructions/direct_selection_v3.rs:224,228` (`init_epoch`) | `market.outcome_count == 2 && terms.outcome_count == 2` | V4 epochs are structurally binary. |
| `DirectV3Intent::SubmitCandidate { outcome_price }` (direct_selection_v3.rs:142-153) | The candidate wire is **one u64 price** | Cannot represent a 16-price simplex, sigma/mu, an AON mask, or a fill vector. A portfolio-clearing candidate is unrepresentable on V3's wire. |
| `orders_batch.rs:864-888` (Direct V4 `PlaceOrder` branch) | `outcome_count == 2`, `page_count == 1`, `order_count < 2`, `max_fee_atoms == 0`, `OrderSlot::Portfolio` refused (line 888) | Correct for its profile; the general branch (orders_batch.rs:734-824) already accepts Portfolio and is the placement path Tier 2 clears. |
| `orders_batch/settlement.rs:256-279` (narrow V2 seam) | `epoch.order_count == 2`, `outcome_count == 2`, `tombstone_count == 0` | The deliberate narrow consumption slice (COUPLED_SETTLEMENT_V1); Tier 2 supersedes it rather than widening it silently. |

**Reused from V3 rather than reinvented:** the staged intent pattern
(Submit → Begin → Verify-per-step → Finalize → Settle → Lapse, tags 36-46
precedent), permissionless deadline-slot transitions
(`DirectEpochV4Account.selection_deadline_slot` shape), prefund-safe PDA
creation (`create_pda_account_full_principal`, `direct_creation_funding` —
already imported by orders_batch.rs:459-462), the funding-ledger idiom
(`DirectFundingLedgerV3`), and the policy-artifact precedent:
`DirectBatchPolicyV3` (96 B, layout tag 23) already wraps the exact 64-byte
`BatchPolicyV1` artifact from `research/batch-policy-identity`, and the
program already depends on that crate. New intent tags start at 47
(`LAST_DIRECT_V3_INTENT_TAG = 46`); the reference adapter refuses them with
`UnsupportedIntent` exactly as it refuses `PlaceOrder`, so the SVM oracle is
the layout codec byte-for-byte, per the genesis.rs:107-114 precedent.

## 1. The dependency-ordered increments

### Wave 1 — five independent lanes (parallelizable)

**T2-1. ClearWork codec (join 5) — owner: `crates/clutch-batch`**

- File: `crates/clutch-batch/src/relation_v1_stream.rs` (+ new codec test
  file). Add to `impl ClearWorkV1`:
  - `pub const ENCODED_BYTES: usize` — canonical serialized length (explicit
    field order, LE, no `repr(Rust)` offset dependence).
  - `pub fn encode_into(&self, out: &mut [u8]) -> Result<(), CodecFaultV1>`
    and `pub fn decode_into(&mut self, input: &[u8]) -> Result<(), CodecFaultV1>`
    — both by reference (no 48 KB value crosses a frame; same discipline as
    the existing entry points).
  - Encoding rules for the non-trivially-valid fields: `phase`/class/flag
    bytes validated against their `PHASE_*`/`CLASS_*` ranges; the five
    `bool`s as `0/1` exactly; `StreamCandidateV1.declared_slices:
    Option<u16>` as flag byte + u16 (mirroring
    `CANDIDATE_FEED_FLAG_SLICES_DECLARED`); `latch_error: ErrorV1` as a
    registered code byte plus the one payload variant's `(outcome: u8,
    owner: u16)` (`PairingInfeasible`, relation_v1.rs:1256-1261);
    `DigestFoldV1` as two u64 (relation_v1.rs:2157-2160);
    `RelationDomainV1.policy: FrozenPolicyV1` via the canonical selector
    bytes (reuse `research/batch-policy-identity`'s
    `encode_batch_policy`/`decode_batch_policy` semantics — depend on it or
    restate the 13 selector bytes with a cross-crate equality test). Bulk
    arrays (~44 KB of u64/u128) as plain LE element runs.
  - Why serializer, not `repr(C)+Pod` (both sanctioned by SOLANA_LAYOUT.md
    and clearing.rs:36-39): `ClearWorkV1` transitively contains payload
    enums, `bool`s, `Option`, and shared `FrozenPolicyV1` enums; Pod-ifying
    forces a public-type rewrite plus re-verification of the equivalence
    gate for a representation change. If measured decode+encode CU (T2-6
    gate) exceeds budget, a Pod-shaped `ClearWorkV1` is the recorded
    fallback, re-gated by the same 19,520-comparison suite.
- Re-pin: `programs/solana-layout/src/lib.rs:136` `CLEAR_WORK_BODY_BYTES =
  48_592` becomes the codec's `ENCODED_BYTES` value (hand-pinned +
  cross-crate equality test, matching `clear_work_size_is_pinned` at
  relation_v1_stream_tests.rs:1308). `account_len::CLEAR_WORK`
  (lib.rs:904-905) and the rent figure move with it; safe because nothing
  consumes these accounts yet (clearing.rs:5).
- **Soundness obligation, precisely:**
  1. *Round-trip identity:* for every checkpoint state reachable by the feed
     protocol, `decode_into(encode_into(w)) == w` — asserted at **every push
     boundary** of the P-BATCH-03 corpus (18 cases × every split = 210
     resume points): the resumption test upgrades from `save = Clone` to
     `save = encode / resume = decode`, with final verdict + `consumed_fold`
     bit-identical to the uninterrupted feed.
  2. *Hostile-byte totality:* `decode_into` on arbitrary bytes never panics,
     never overflows, and either refuses or yields a struct whose every
     field re-validates (every byte of a valid encoding flipped; every
     control-field value swept out of range refuses with a typed fault).
  3. *Checkpoint tamper refusal — layered:* the codec claims *validity*, not
     tamper-proofing. Tamper refusal is the existing three-layer stack the
     codec must not weaken: (a) body edits changing consumed-fold state →
     next pass refuses `FeedErrorV1::ResumeFoldMismatch` and poisons
     (relation_v1_stream.rs:109-112); (b) header edits → refused by
     `ClearWorkHeader::validate` (clearing.rs:134-175) and
     `require_continuation` against `epoch.order_set` (clearing.rs:344-350);
     (c) wholesale body substitution with another internally-consistent
     checkpoint → refused because `bind_order_set` stamps
     `(order_set, consumed_fold)` into the layout-owned header
     (clearing.rs:321-336) and the program must compare
     `body.consumed_fold() == header.consumed_fold` at every resume. New
     tests: mutate each body region between two on-chain-shaped resume steps
     and assert (a)/(c); mutate the header and assert (b).
  4. *No-frame gate:* extend the SBF `.stack_sizes` probe
     (STREAMING_RELATION_DESIGN.md §9 method) with `encode_into`/`decode_into`;
     zero diagnostics, under ~1,500 B.
- Test gate: the clutch-batch suite (61 tests growing; equivalence corpus
  re-run unchanged). Claim plane: **HOST-TESTED** (+ measured SBF frames).

**T2-2. Page→order projection, host half — owner: `programs/solana-layout`**

- The projection spec is in-tree: STREAMING_RELATION_DESIGN.md §10 (per-order
  feed requirements) as amended by SOLANA_LAYOUT.md — the `PortfolioOrderV1`
  mapping table (lines 362-374), the single-Egg mapping, the v4 addendum's
  retirement rules (lines 608-637: tombstones digest-covered, skipped by the
  projection, no live rank), the doc-tested `PortfolioRecord` example
  (lib.rs:1170-1242). Two §10 items are superseded by page v4 — implement
  per the addendum: order ids are *stored ranks*
  (`canonical_order_id`/`order_id_rank`, lib.rs:562-604; "the only work left
  to the projection is the live-rank renumbering that skips retirements",
  SOLANA_LAYOUT.md:381) and `expiry_epoch` is per-order persisted, checked
  against `epoch.epoch_index` at bind time.
- New module `programs/solana-layout/src/projection.rs`:
  - `project_single(record, live_rank, owner_tag)`,
    `project_portfolio(record, live_rank, owner_tag)` — pure, exactly the two
    mapping tables (`canonical_order_id = live_rank` 1-based,
    `partial_policy` from flags bit 0, `side` 0/1 → `Side`); plus
    `project_slot(slot, ...) -> Option<OrderV1>` returning `None` for
    `Tombstone` (rank not consumed) and refusing `Empty` inside the
    populated prefix. Requires `clutch-solana-layout` to depend on
    `clutch-batch` (both inside the no_alloc boundary).
  - `OwnerInterner { owners: [Hash32; 64], count: u16 }` with
    first-appearance interning, refusing a 65th owner; bijection obligation
    discharged by construction per §10 — tags are `0..count` and the program
    later refuses if `count != epoch.owner_count` at pass-1 end.
  - Pin the index vocabularies the narrow seam left ambiguous:
    `CandidateFeedAccount` fill indices and `PairingSlice::LegRef::Order`
    indices are **live ranks (0-based)**, not global slot indices (readings
    coincide at tombstone_count == 0 — settlement.rs:769, common.rs:511);
    document + test the general reading.
- Host differential gate (backlog 6.2 line 259): build real v4 pages via
  `stream::{init_page, append_slot, write_tombstone, frozen_set_commitment,
  seal_page}` — portfolio records (≤ MAX_PORTFOLIO_ORDERS = 8/set),
  tombstones, multi-page up to 4×16 = 64 — walk through the projection into
  `ClearWorkV1::begin/push_order/end_pass`, assert verdict identity with
  `relation_v1::verify` on the equivalent hand-assembled `BookV1`. A
  tombstone-bearing set whose live orders equal a tombstone-free set must
  produce the identical verdict. Claim plane: **HOST-TESTED**.

**T2-3. Staged creation of the 48,750-byte account — owner: program**

Per the in-tree analysis (SOLANA_LAYOUT.md:1159-1204, genesis.rs:99-106):
**realloc path, checkpoint stays a PDA** (a keypair-addressed checkpoint is
substitutable — authentication over ergonomics).

- `seeds.rs`: `SEED_CLEAR_WORK = b"dragons-clutch:clear-work:v1"`,
  `clear_work_pda(program_id, epoch, candidate)` (one checkpoint per
  `(epoch, candidate)`; market bound via the epoch).
- New `orders_batch/clear_work.rs`, intents tags 47, 48 (layout `Intent` +
  reference-adapter refusal + dispatcher arm — watch the "one more
  dispatcher arm" 4,096-frame item):
  - `InitClearWork { market, epoch, candidate }` — ResolutionWork shape
    verbatim (resolution_work.rs:679-735): `require_creatable` →
    full-principal transfer for the FINAL rent → `allocate_data(10_240)` →
    `assign_data` under PDA seeds, post-checked. Write a grow-stage prefix:
    `CLEAR_WORK_TAG (17)`, version, `GROWING` marker, `target_len: u32`,
    identity triple, bump. A growing account is economically inert by
    construction: `check_header` refuses non-exact length
    (clearing.rs:206-218), so every consumer refuses it; the prefix makes it
    un-repurposable and resumable.
  - `GrowClearWork` ×4 — `AccountInfo::resize(min(len + 10_240, target))`
    (precedent direct_selection_v3/common.rs:340). Final grow finishes
    creation atomically in the same instruction: `clearing::init_clear_work`
    writes the real header (clearing.rs:284-311), then the idle body via a
    new `ClearWorkV1::encode_idle_into(out)` driven from `const NEW` (no
    48 KB frame value; relation_v1_stream.rs:440-441).
    MAX_PERMITTED_DATA_INCREASE is per-instruction so all five may share one
    transaction (SOLANA_LAYOUT.md:1184-1190); the design must not require
    that.
  - Mirror `InitCandidateFeed` in the same file — 6,266 B fits one CPI;
    `clearing::init_candidate_feed` exists; `SubmitDirectPage`'s creation
    code is the in-family precedent.
- Test gate: new svm-tests `clear_work_creation.rs`: five-in-one-transaction
  and five-across-transactions; duplicate/wrong-bump/pre-funded/
  partial-then-resume/late-failure-rollback; half-grown refuses every
  consumer; rent exactness per stage. Claim: **SBF-EXECUTED** (bank), no
  promotion.
- Dependency: needs T2-1's final ENCODED_BYTES only for the last-grow body
  write; machinery lands against the current 48,750 and re-pins at the join.

**T2-4. Tombstone cardinality (join 2) — owner: `programs/solana-layout`**

- Live count is derivable and digest-authenticated
  (`OrderPageHeader.live_count()`; tombstone_count in the page-digest
  preimage, SOLANA_LAYOUT.md:629-637; `verify_preflight` recomputes it,
  settlement.rs:103-111) — then `CandidateRecord::binds_epoch`
  (lib.rs:3793-3805) refuses because it demands `order_len as u16 ==
  epoch.order_count` (slots incl. tombstones).
- Change: exact live-cardinality binding — `binds_epoch(..., live_order_count:
  u16)` (or sibling `binds_epoch_live`) requiring `order_len as u16 ==
  live_order_count` and `live_order_count <= epoch.order_count`; the caller
  must have recomputed the live count from digest-verified page headers
  (`stream::epoch_binds_page_set` first). Update `verify_preflight`; delete
  its workaround comment (settlement.rs:117-124). No account-format bump.
- Gate: layout host tests — a cancelled book (1 tombstone) binds; a
  candidate claiming slot-count on a cancelled book refuses; a mutated
  tombstone_count is caught by the page digest, not this binding. Claim:
  **HOST-TESTED**.

**T2-5. Policy preimage + full-width id domain (joins 3, 4) — consumption of `research/batch-policy-identity`**

Both joins are solved offline (BATCH_POLICY_IDENTITY_V1.md: 64-byte
`BatchPolicyV1` artifact, `FullRelationDomainV1` 284-byte preimage, u64 tags
proven read only by the obsolete V9 digest → zero sentinels; 9 adversarial
tests incl. all 10,368 selector/fee products). Tier 2's work is consumption:

- General policy account: reuse `SEED_BATCH_POLICY`/`batch_policy_pda`
  (seeds.rs:249-250) with a `DirectBatchPolicyV3`-shaped 96-byte account,
  created at general InitEpoch with `epoch.policy ==
  batch_policy_digest(artifact)` enforced. Tier 2 frozen policy profile v1,
  pinned as a const (like `DIRECT_POLICY_V1`): `fee_base: None`, 0 bps; ONE
  pinned dust choice; `pairing_witness: ExplicitSlices`; `portfolio_lots:
  StrictWholeOrder`; `self_cross: RefuseOverlap` (2 passes, not 3).
  **PROPOSED → frozen is ember's sign-off; pins zero-fee and does not
  preempt the fee-base fork.**
- Domain construction (program side, in T2-6):
  `RelationDomainV1 { relation_version, market_id: 0, book_id: 0, epoch:
  epoch_index, policy_id: 0, order_set_id: 0, outcome_count, owner_count,
  price_scale, remainder_seed, policy }` — zero sentinels sound because
  `begin` runs `strict_claims = false` (the u64 tags feed only the legacy
  digest, never compared); authoritative identity binding is full-width:
  `ClearWorkHeader{market,epoch,candidate,order_set}` +
  `FullRelationDomainV1::digest()` recomputed where selection needs a total
  order. Score comparison uses `FullScoreV1::total_order` over components
  recomputed from the streamed `SummaryV1` — never the claimed u128 digest.
- Gate: cross-crate test — streaming verdict under zero-sentinel domain ==
  `verify_submitted_candidate`'s V0-V8 verdict on the same coordinates.
  Claim: **HOST-TESTED**.

### Wave 2 — the join (strictly after Wave 1)

**T2-6. General epoch lifecycle + the on-chain streaming walk (joins 1, 4-owner-half, 6-init-half)** — one lane, three increments, all in `orders_batch/`:

- **T2-6a. General `InitEpoch` (tag 49) + `FreezeEpoch` (tag 50)** — the gap
  named at genesis.rs:93-98 and backlog 6.1. InitEpoch creates the general
  `EpochAccount` (4-page, outcome_count ≤ 16) at `seeds::epoch_pda`, phase
  OPEN, binding market/terms/grid/policy PDAs (template
  direct_selection_v3.rs:199-249 minus the `== 2` gates); deadline slots ride
  a small companion window account (V3 window precedent), not an EPOCH format
  bump. FreezeEpoch at/after deadline: all pages in one instruction (≤ 4 ×
  4,012 B), `stream::frozen_set_commitment` → `seal_page` each → epoch:
  `order_set`, first/last order id, page_count, order_count, phase FROZEN
  (template direct_selection.rs:430-455; `epoch_binds_page_set` is the
  post-state check). Gate: SVM init + place (single + portfolio) + cancel +
  freeze; refusal matrix. Claim: **SBF-EXECUTED**.
- **T2-6b. `AdvanceClearWork` (tag 51)** — the walk instruction consuming
  everything from Wave 1. Per invocation: decode header
  (`verify_clear_work`), `require_continuation` on resumes,
  `request_heap_frame` + `Box<ClearWorkV1>` (heap per FRAME_BUDGET_PLAN §5;
  the program crate is outside no_alloc and boxes deliberately) →
  `decode_into` from the body → for up to `max_orders` slots from
  `(page_cursor, slot_cursor)`: `stream::OrderSlotCursor` on the
  digest-verified named page, skip tombstones, `OwnerInterner::intern`,
  `projection::project_slot`, `fill_at(feed, live_rank)`, verify the order's
  canonical `ReservationAccount`, `push_order` → `advance_walk` (monotone
  cursor, clearing.rs:360-380) → `encode_into` back → on pass-1 completion:
  `end_pass` + `bind_order_set(order_set, consumed_fold)` + refuse unless
  `interner.count == epoch.owner_count`; later passes: compare
  `body.consumed_fold() == header.consumed_fold`. First advance on an OPEN
  checkpoint performs `begin` with the T2-5 domain and `StreamCandidateV1`
  from the bound `CandidateFeedHeader` (binding template
  settlement.rs:146-169). The owner-interning table (64×32 B + count ≈
  2,052 B) lives in a new layout-owned region between header and body
  (`account_len::CLEAR_WORK` grows; coordinate with T2-1's re-pin).
  - **Join 1 (reservation-set commitment):** the pass-1 walk IS the sweep.
    Each pushed order requires its reservation account (PDA re-derived via
    `canonical_reservation_id`; `RESERVATION_STATE_ACTIVE`; plan re-derived
    by `ReservationPlan::for_order` — portfolio-aware since reservation.rs:98
    — matching the stored envelope; `max_fee_atoms == 0`). Page + ≤16
    reservations + fixed accounts ≈ 23 accounts per transaction. Pass-1
    completion proves "every live order has exactly one ACTIVE funded
    reservation" at bind time; nothing releases an ACTIVE reservation of a
    FROZEN epoch (cancellation requires OPEN) — state as an invariant test;
    permissionless-lapse racing is out of scope (backlog 6.1's open item).
- **T2-6c. `AdvanceClearSlices` (52) + `CompleteClearWork` (53)** — the slice
  pass (`slice_at` → `PairingSliceV1` → `push_slice`; 64 B frame, ≤ 416
  slices in the feed) and the close: `end_pass` → `verdict()` → Ok(summary):
  persist VERIFIED status + recomputed `FullScoreV1` components on the
  `CandidateRecord` (format rev), `complete_clear_work`; Err: mark REFUSED,
  complete. Claimed u128 digest never trusted.
- **Gates (the Tier 2 headline):** (i) SVM: 3-page, 40-order book with 2
  portfolio orders and 3 tombstones, cross-transaction resumption at
  arbitrary boundaries, verdict == host relation's verdict on the projected
  book (discharges backlog 6.2 line 259 on-chain); (ii) tamper battery:
  substituted page/feed/checkpoint refuses; reservation
  missing/RELEASED/wrong-plan refuses; 65th owner refuses; (iii) measure CU
  per AdvanceClearWork at various batch sizes (codec decode+encode is the
  unknown — measure before optimizing; Pod fallback recorded) plus
  heap-frame cost. Claim: **SBF-EXECUTED**, explicitly PROFILE-ADMITTED: no.

### Wave 3 — selection and entitlement (strictly after Wave 2)

**T2-7. Candidate window closure + selection (join 7)** — general
`SubmitCandidate` (solver-written `CandidateRecord` + `CandidateFeedAccount`
fills/slices at canonical PDAs, SUBMITTED; `SubmitDirectPage`'s plumbing
generalizes), bounded retained-candidate registry + deadline closure +
`FinalizeSelection` comparing only VERIFIED candidates by
`FullScoreV1::total_order`, stamping SELECTED and `EPOCH_PHASE_CLEARED` —
models: V3 `staged.rs`/`terminal.rs` and
`research/batch-policy-identity/src/direct_lifecycle_v3.rs`. "Best valid
**submitted** candidate" only; `propose_best_valid` stays host. Gate: SVM
multi-candidate selection incl. beyond-128-bit score ties (fixture exists)
and unverified-candidate exclusion. Claim: **SBF-EXECUTED**.

**T2-8. Entitlement freeze + generalized consumption (join 8)** — resumable
per-slice `SettlementReceiptAccount` creation from the SELECTED candidate's
feed (receipt PDA per `(candidate, slice_index)`; leg kinds
DIRECT/SPLIT/MERGE exist, lib.rs:2560-2564), `FinalPotAccount` (lib.rs:3891)
funded from the verified summary's rounding/refund scalars, then widening
`SettlePage`: portfolio full-pair legs consume
`portfolio_settlement::{prepare,apply}_full_pair` (**Tier 1's out-param
rework is the prerequisite here**), reservation release per the verified
summary. First cut refuses candidates with `virtual_split/merge != 0` at
entitlement freeze (VirtualPot stays a ranked blocker) and full-fill-only
receipts (PartialFillLedger likewise) — refusals stated in code, keeping the
`SETTLEMENT_BLOCKERS` ledger (settlement.rs:850-859) truthful as items
retire in order. Gate: SVM end-to-end — place portfolio + single → freeze →
submit → walk → select → entitle → consume → conservation assertions
(joined_lifecycle.rs pattern), including the "portfolio order actually
clears" transaction that is the whole point. Claim: **SBF-EXECUTED**.

## 2. Parallel vs ordered (orders_batch.rs:269-284 numbering)

- **Parallel now:** join 5 (T2-1), join 2 (T2-4), joins 3+4 preimage/domain
  (T2-5), the projection (T2-2), join 6's creation half (T2-3). Five
  independent worktrees.
- **Strictly after all five:** join 1 and join 4's owner-interning half —
  properties of the pass-1 walk (T2-6b), which needs codec + projection +
  domain + creation + general freeze (T2-6a; itself independent of Wave 1
  except the policy account from T2-5).
- **Strictly after the walk:** join 7, then join 8. Their codec/account
  design may be drafted in parallel with Wave 2; nothing SBF-lands out of
  order.

## 3. Out of scope (so nobody re-litigates mid-lane)

- **Fees stay forced zero** at all five gates (orders_batch.rs:878,
  settlement.rs:431/:592, direct_selection.rs:892-893/:1707); Tier 2 policy
  pins `FeeBaseV1::None`/0 bps. The fee-base fork remains ember's decision.
- **`propose_best_valid` stays host-side** — "Never" tier in the frame plan.
- **No promotion claims:** no liveness rows, no `live_*` flags, no
  deployment; every increment labeled HOST-TESTED or SBF-EXECUTED exactly.
- **Ranked blockers deliberately standing:** PartialFillLedger, VirtualPot,
  TerminalClosure (rent-reclaim close of ClearWork/feed/receipts),
  permissionless reservation expiry/lapse vs frozen-epoch racing, the V5
  K-pass memory alternative (recorded, not taken).
- **No V3/V2 behavior changes**; the narrow coupled seam keeps working
  unmodified until T2-8 supersedes it.

## 4. Envelope pins

Frame 4,096 / heap to 262,144 at 8 CU per extra 32 KiB / per-CPI allocation
10,240 / rent `(128+bytes)*6960` → checkpoint ≈ 0.3402 SOL (re-quote after
T2-1 re-pin); passes: 2 order passes (RefuseOverlap) + 1 slice pass; page
fold 61 SHA blocks; post-SHA route baseline ~180k CU. The single
measure-first risk is T2-1's decode/encode CU per transaction — measured in
T2-6b's gate before any optimization (Pod fallback recorded).
