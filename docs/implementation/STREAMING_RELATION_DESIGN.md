# Streaming relation verification design (`relation_v1_stream`)

Status vocabulary: **IMPLEMENTED** marks code that exists in
`crates/clutch-batch/src/relation_v1_stream.rs` and is exercised by the
equivalence gate in `relation_v1_stream_tests.rs`.  **PROPOSED** marks a design
that no code enforces yet — in particular the whole of §10, the page→order
projection, which is a specification for the layout lane and not an edit to any
program.  Nothing in this document is verified in a proof-assistant sense, and
an accepted candidate remains only the **best valid submitted candidate** of
its proposal window; no wording here upgrades that claim.

## 1. The measured problem

Commit 5cb4ad1 measured why `BatchRelationV1` cannot run on-chain.  On the
pinned `cargo-build-sbf` (platform-tools v1.53), against SBF's 4,096-byte
per-frame maximum:

| function | estimated frame |
| --- | --- |
| `relation_v1::canonical_candidate` | 45,824 |
| `relation_v1::verify_inner` | 39,104 |
| `relation_v1::canonical_pairing` | 38,016 |

The cause is type width, not code shape: `verify_inner` holds a `BookV1`
(11,272 B), the `NormalizedBookV1` it derives (11,912 B), and a
`ParticipationV1` (16,384 B) as **single locals**.  No `#[inline(never)]`
arrangement helps, because the large values are single locals rather than a
composition of small ones.  The repair must change the interface: the working
set of every function must be **one order (176 B), not one book (11 KB)**.

## 2. Shape of the answer

The streaming verifier is a **caller-driven, multi-pass fold** with an explicit
resumable state object:

* every large table lives in one caller-owned state struct, `ClearWorkV1`,
  which is only ever touched **by reference** — on-chain it is account data,
  never a stack frame;
* orders are **pushed one at a time**, in canonical (strictly increasing id)
  sequence, each with its candidate fill; pairing slices, when the frozen
  policy carries an explicit witness, are pushed one at a time as their own
  pass;
* the verdict — `Ok(SummaryV1)` or `Err(ErrorV1)` — is **the same verdict**
  `relation_v1::verify` produces on the same domain, book, candidate, and
  pairing witness, including which refusal class is reported.  Equivalence is
  the gate (§8), and a divergence is a finding, never a tune.

The precedent is `programs/solana-layout/src/stream.rs`: the buffered decoder
stays the golden reference, the streaming path is a new arrangement of the same
refusals with a per-call working set of one slot, and the equivalence contract
is stated over whole verdicts, not per-helper.

### API (IMPLEMENTED)

```text
ClearWorkV1::new()                                  the zeroed checkpoint object
begin(&mut self, domain, &StreamCandidateV1,
      strict_claims)                                freeze coordinates, start pass 1;
                                                    strict_claims=false mirrors
                                                    verify_ignoring_claimed_aggregates
push_order(&mut self, &OrderV1, fill: u64)          one order of the current pass
push_slice(&mut self, &PairingSliceV1)              one slice of the slice pass
end_pass(&mut self)                                 close the pass, learn the next step
status(&self) -> FeedStatusV1                       NeedOrders{pass} | NeedSlices | Complete
verdict(&self) -> Option<Result<&SummaryV1, ErrorV1>>
consumed_fold(&self) -> u128                        the continuation digest (§6)
```

`StreamCandidateV1` is the candidate header: everything in `CandidateV1`
except the fill array (fills travel with their orders) and the padding (the
streamed candidate has no representation for non-canonical padding; §7).
Feed-protocol faults (`FeedErrorV1`: wrong phase, over-long feed, a resumed
pass whose orders are not the pass-1 orders) are deliberately a different type
from relation refusals: a protocol fault means the *feed* is broken and the
verification must be restarted; a relation refusal is the verdict.

## 3. Stage decomposition

`V0`–`V9` decompose into per-order folds, bounded accumulators, and
finalize-time scans over the checkpoint state.  "Pool" below means one
pro-rata allocation pool, keyed `(outcome, side)`; under allocation **A** its
members are the marginal orders of that outcome and side, under **B** every
non-forced active order.  Portfolio orders are always forced when active
(policy P-a), so **every pool member is a single-Egg order and each order
belongs to at most one pool** — that fact bounds the dust machinery in §5.

| stage | streamed per order | cross-order state | bound | decided |
| --- | --- | --- | --- | --- |
| V0 admission | every `BookV1::validate` check | monotone id, portfolio count | O(1) | at each push (terminal) |
| V0 owner interning | slot lookup/insert | owner tag table | 64 × u16 | per push |
| V0 self-cross N-a | presence bits | per-(owner,outcome) buy/sell bits | 2 × 128 B | pass-1 finalize |
| V0 self-cross N-b | gross side totals, then per-order cancel take | per-(owner,outcome) side totals → remaining-cancel counters; per-order `cancelled` | 2 × 8 KiB (reused scratch) + 512 B | totals pass 1, assignment pass 2 |
| V1 simplex | — | none (prices are candidate header) | O(1) | at `begin` (latched) |
| V2 eligibility | one classification from prices | class byte per order | 64 B | accumulate pass |
| witness-fill checks | the whole `validate_witness_fills` body for one order | none | O(1) | accumulate pass (latched) |
| V4 churn | — | none | O(1) | at `begin` (latched) |
| V4 flows | per-leg add | per-outcome buy/sell flow | 2 × 16 × u128 | accumulate; identity at finalize |
| V3 aggregates | demand/supply/forced/strict/pool sums | 8 × u128 per outcome + pool totals | ≈ 2.5 KiB | accumulate finalize |
| V3 canonical fills | floor + remainder key per pool member | pool key table (one row per order) | 64 × 64 B | floor pass finalize (§5) |
| V5 participation | per-leg add into owner row | per-(owner,outcome) buy/sell | 16 KiB | accumulate; H-i-O scan at finalize (§4) |
| V5 explicit slices | per-slice executability | covered per-(order,outcome), split/merge per outcome | 8 KiB (reused scratch) + 256 B | slice pass + floor pass |
| V6–V8 ledger | one order's cash/Egg terms | per-owner reserved/debit/credit/fee, per-outcome Egg arrays, scalars | ≈ 4.6 KiB | accumulate; closures at finalize |
| V9 score | — | reads participation + flows + ledger | — | final finalize |
| V9 digest | fold `(fills[j])` | two 16-byte mix states | O(1) | final finalize |

The one stage that is *not* a bounded-accumulator fold is the canonical
pro-rata dust selection, analyzed in §5: its state is bounded by **order
count** (one 64-byte row per order), not by a constant.

### Pass schedule

Each pass is one walk of the order sequence (on-chain: one walk of the frozen
page set, resumable mid-pass across transactions).

| frozen self-cross | passes over orders | slice pass |
| --- | --- | --- |
| N-a / N-c | 2 — `Admit+Accumulate`, then `Floor` | between them, iff `ExplicitSlices` and a witness is declared |
| N-b | 3 — `Admit+NetTotals`, `NetAssign+Accumulate`, `Floor` | same position |

N-b needs the extra pass because an order's cancelled quantity depends on
whole-book per-(owner, outcome) totals, and everything downstream (V2 classes,
effective quantities, every aggregate) depends on cancellation.  The slice pass
sits before `Floor` so the covered-versus-legs comparison can ride the `Floor`
walk instead of costing a fourth one.

## 4. The H-i-O bound

The pairing-feasibility gate checks `part_i(O) <= F_i` for every active
outcome `i` and owner `O`.  `F_i` is known only when the stream ends, and two
owners' partial sums cannot be merged — the check is per-owner, not an
aggregate — so **no o(owners) sketch can decide H-i-O**: any state that forgets
one owner's running participation can be defeated by an adversarial
interleaving that hides that owner's excess.  The participation table is
therefore irreducible cross-order state, and its bound is structural:

* one distinct owner slot can exist per admitted order, so
  `owner_slots <= min(order_count, MAX_ORDERS) = 64`;
* the score's self-overlap term needs buy and sell separately (H-i-O alone
  would need only their sum), so the table is
  `2 × 64 × 16 × 8 B = 16,384 B` — exactly the `ParticipationV1` that
  overflowed the *frame* in the batch verifier, now living in the *account*,
  touched one row per push.

For a domain that admits fewer owners the on-chain account can be sized by
`domain.owner_count`; the host-model struct keeps the fixed worst case.

## 5. Canonical pro-rata: the key table

Pro-rata verification needs, per pool: the target, the member total, each
member's floor `⌊q_j·target/total⌋`, the dust `D = target − Σfloor`, and the
fact that the candidate's `+1` set is exactly the top-`D` members under the
frozen key order `(remainder desc, seeded rank asc, id asc)`.  Selecting the
`D`-th largest key from a stream in O(1) state is impossible in general, so
this stage keeps **one 64-byte row per pool member** (key, floor, effective
quantity and minimum for the obligation walk, fill parity, pool tag).  Because every pool member is a single-Egg order in
exactly one pool (§3), the table is bounded by the order count: 64 rows,
≈ 3 KiB.  At `Floor`-pass finalize an O(n²) scan (≤ 4,096 comparisons)
resolves top-`D` membership exactly — keys are totally ordered because the id
component is unique.

The same table discharges the one genuinely subtle refusal-identity case: the
batch verifier checks minimum-fill and all-or-none obligations **on the derived
vector** after derivation, before the equality comparison.  When the candidate
equals the canonical fills that walk can never fire (the same predicates were
already applied to the candidate's fills at the witness-fill stage), but when
the candidate *differs*, the batch verdict can be `AllOrNoneViolation` or
`MinimumFillViolation` — a fact about the canonical fills, not the submitted
ones — instead of `CandidateMismatch`.  The key table makes the derived value
of every pool member exactly computable at finalize (`floor + [member of
top-D]`), so the streaming verifier replays that walk bit-for-bit.  This case
is reachable only under AON policy 2c with a marginal AON or minimum-fill
order, and the equivalence battery covers it (§8).

## 6. Refusal identity and the position ladder

`verify_inner` reports the refusal of the **first stage that fails, at the
first program point inside that stage**.  A streaming verifier discovers the
same facts at different wall-clock moments (a later-stage fact can surface in
an earlier pass), so verdict identity is engineered, not accidental:

* every refusal source is assigned a **position** on a single total ladder
  that mirrors `verify_inner`'s program order — stage major, then outcome /
  order-index / slot minors in exactly the order the batch code visits them;
* the state carries **one latch**: the least-position `(position, ErrorV1)`
  pair seen so far.  Nothing is reported when latched; resolution happens once,
  at the final finalize, where the latch (if any) is the verdict;
* the only immediate refusals are V0 admission faults, which the batch
  verifier also reports before reading anything else, in the same per-order
  sequence; and a pass finalize may end the feed early when every stage at or
  before the latched major is fully decided (V0/length/V1 faults terminate
  after pass 1 under N-a/N-c, after pass 2 under N-b);
* single-class stages need no minor at all (every arithmetic-overflow source
  inside the V4 flow fold latches the same `ArithmeticOverflow`; every V0
  self-cross source latches the same `SelfCrossRefused`), which keeps the
  ladder small.  Where classes differ inside one stage — the V3 derivation
  ladder, the V5 feasibility scan with its `PairingInfeasible { outcome,
  owner }` payload, the V6–V8 walk — the scan at finalize runs in the batch
  code's exact visit order, so the first hit is the batch's first hit.

**P-BATCH-03 (the design's central obligation).**  *Resumed verification
equals one ordered fold*: for every split of the feed into chunks, with the
checkpoint object saved and restored between chunks, the verdict — and the
whole `SummaryV1` on acceptance — is identical to the uninterrupted fold, and
identical to `relation_v1::verify` on the assembled book.  This is stated as
the theorem to test, not a theorem proved: the equivalence gate exercises it
at every split boundary on the oracle domains (§8), and the checkpoint's
integrity rule makes the "same feed" premise checkable rather than assumed:

* pass 1 folds every `(order, fill)` pair into a running digest (the
  `mix`-based candidate-digest permutation, reused as a stream identity — a
  deterministic consistency device, **not** a cryptographic commitment);
* every later pass folds the same way and `end_pass` refuses with a
  feed-protocol error when the fold or the count differs: a resumed
  verification that is not provably the continuation of the same order
  sequence never yields a verdict at all — refusal-on-tamper;
* the cryptographic anchor is the layout's, not this crate's: on-chain, every
  pass must be fed from the same frozen, SHA-256-digest-verified page set, and
  the projection (§10) binds `consumed_fold` to that page-set identity.  The
  in-crate fold then guards against feeder bugs, and the page digests guard
  against hostile bytes.

### Representational non-cases

Three batch refusals are facts about the fixed-array *representation* and have
no streamed counterpart, because the feed has no padding: `NonCanonicalPadding`
for book slots at or beyond `len`, for candidate fills at or beyond `len`, and
for pairing-witness slices at or beyond `witness.len`.  The streamed candidate
also cannot express `fills[j] != 0` for `j >= len`.  Equivalence is therefore
stated over **canonically padded** batch inputs — exactly the inputs the batch
verifier accepts as representable — and the batch tests that pin those
padding refusals stay the owners of that surface.  A feed longer than the
frozen bounds refuses (`TooManyOrders` at push 65; `SliceSumMismatch` past
`MAX_SLICES`), matching the batch classes for the same excess.

## 7. The checkpoint object

`ClearWorkV1` is one flat struct — measured `size_of` = **48,592 bytes**,
pinned by `clear_work_size_is_pinned` — every field fixed-size, `no_std`, no
allocation, `Clone + PartialEq` (which is what makes P-BATCH-03 *testable*:
save = copy, resume = keep using the copy).  Inventory:

| region | contents | bytes (≈) |
| --- | --- | --- |
| control | phase, pass index, cursors, latch, folds, digest states | 250 |
| frozen coordinates | `RelationDomainV1`, candidate header | 350 |
| interning + descriptors | owner tag table, per-order slot/side/touch bytes | 600 |
| per-order bytes | class, flags, cancelled, key-table rows (§5) | 4.2 KiB |
| participation | buy/sell per (owner slot, outcome) | 16 KiB |
| scratch A/B | N-b side totals → remaining-cancel counters; reused as the slice `covered` table after the accumulate pass | 16 KiB |
| V3 aggregates + pools | 8 per-outcome u128 sums; per-pool totals/targets | 3.4 KiB |
| V6–V8 ledger | per-owner u128 arrays, per-outcome Egg arrays, scalars | 4.9 KiB |
| flows + slice sums | per-outcome u128 flows, split/merge used | 768 |
| summary | the recomputed `SummaryV1` | 1.2 KiB |

The struct is the resumable `ClearWork` account body the P1-C list wanted.
Its serialization (zero-copy layout, versioning, rent) is the layout lane's;
this crate's contract is only that every entry point takes `&mut ClearWorkV1`
and never holds more than one order, one slice, or one scalar row on the
frame.

## 8. Equivalence gate (IMPLEMENTED — results in §9)

The gate reuses the bounded enumerations the batch oracle already trusts, and
asserts *verdict identity* — same acceptance, same `SummaryV1`, same refusal
class, same `PairingInfeasible` payload — between `relation_v1::verify` and
the streaming feed:

1. **The 2,592-book domain** (1,296 shapes × 2 owner layouts, 3 price ticks ×
   3 imbalances each): every canonical candidate the constructor produces is
   verified through both paths; every accepted candidate is then mutated
   (fill bump, fill zero, sigma/mu twiddle, mask bit, stale score, stale
   digest, price swap) and each mutation is verified through both paths.
2. **The pairing-feasibility tables**: the 4,096-code owner/side flow-table
   enumeration (both the one-outcome and the coupled-outcome variant),
   driven through both paths, asserting identical classes — on infeasible
   tables that includes the `PairingInfeasible { outcome, owner }` payload.
3. **Policy-variant fixtures**: the N-a/N-b/N-c self-cross books, the AON
   two-cycle mask domain (all 16 masks × both books), the 2c marginal
   min-fill corner of §5, portfolio books, fee and rounding variants,
   explicit-slice witnesses (verbatim and forged), and the epoch-lapse book.
4. **P-BATCH-03 splits**: representative coordinates re-verified with the
   checkpoint cloned and resumed at *every* push boundary, asserting
   bit-identical state and verdict against the uninterrupted feed; plus the
   tamper case (a pass-2 feed that differs in one order) refusing with the
   feed-protocol error and yielding no verdict.

## 9. Results (measured)

Numbers from the gate run and the SBF probe on the pinned toolchain
(`cargo-build-sbf 4.0.0`, platform-tools v1.53, rustc 1.89.0):

* clutch-batch suite: **61 tests green** (44 pre-existing + 17 streaming);
  equivalence comparisons executed by the new tests:
  * 2,592-book domain, all 23,328 `(book, p, c)` coordinates enumerated:
    2,440 canonical candidates verified verdict-identically through both
    paths, then 17,080 mutated candidates (7 mutation families over every
    accepted candidate) — 19,520 verdict comparisons, 14,840 of the mutations
    refused, identically, on both paths;
  * feasibility tables: 3,255 conserving one-outcome flow tables plus 848
    coupled-outcome tables, class- and payload-identical (the 300
    `PairingInfeasible { outcome, owner }` refusals compared exactly);
  * policy fixtures: self-cross N-a/N-b/N-c, the 16-mask AON two-cycle
    domain, the §5 derived-vector obligation corner (both the AON and the
    minimum-fill shape, five imbalances each), allocation A × B under both
    dust policies with the dust-atom transfer forgery (a fill moved between
    two pool members, conserving every flow and violating only
    largest-remainder canonicality — aimed straight at the §5 key table),
    portfolio books, fee × rounding variants (3 × 2), explicit-slice
    witnesses verbatim and forged, admission/domain refusal fixtures — all
    verdict-identical;
  * P-BATCH-03: every-split resumption over 18 cases (accept and refuse
    verdicts, N-b netting, explicit slices) — 210 checkpoint copies, final
    state and verdict identical to the uninterrupted fold; tampered
    resumptions (changed fill, changed order, short pass, long pass) refuse
    with `FeedErrorV1::ResumeFoldMismatch`/`TooManyPushes` and yield no
    verdict.
* `core::mem::size_of::<ClearWorkV1>()` = **48,592 bytes** (host, pinned
  exactly by `clear_work_size_is_pinned`).
* SBF frames, measured from the `.stack_sizes` section emitted by the pinned
  platform-tools rustc (`RUSTFLAGS=-Zemit-stack-sizes cargo-build-sbf` on a
  scratchpad probe: a `cdylib` holding the checkpoint in a `static`, driving
  `begin`/3 order passes/slice pass/`verdict` through `black_box`, plus a
  control function calling `relation_v1::verify`):

  | function | frame bytes |
  | --- | --- |
  | `ClearWorkV1::begin` | 0 |
  | `ClearWorkV1::push_order` (admission, netting, accumulate, floor all inlined) | 1,280 |
  | `ClearWorkV1::push_slice` | 64 |
  | `ClearWorkV1::end_pass` (every finalize inlined) | 832 |
  | probe entrypoint (drives a whole verification) | 768 |
  | — batch controls in the same build — | |
  | `relation_v1::verify_inner` | **39,104** — diagnostic fires |
  | `relation_v1::participation_from_fills` | 24,704 — diagnostic fires |
  | probe control (`relation_v1::verify` caller) | 16,512 — diagnostic fires |
  | `relation_v1::normalize` | 12,160 — diagnostic fires |

  Every streaming entry point is under a third of the 4,096-byte maximum;
  `cargo-build-sbf` emits **zero** frame diagnostics for any
  `relation_v1_stream` function while the batch controls in the very same
  build reproduce the orders lane's 39,104-byte measurement exactly — the
  positive control that proves the zero is meaningful.

## 10. Page→order projection (PROPOSED — a spec for the layout lane)

The streaming verifier consumes `(OrderV1, fill)` pairs plus a candidate
header and, under `ExplicitSlices`, a slice stream.  The frozen page format
does not yet supply three of the coordinates (5cb4ad1's second blocker).  This
section is the requirement list; every item is PROPOSED and owned by the
layout lane — nothing here edits `programs/**`.

### Per-order feed requirements

| `OrderV1` coordinate | on-chain source | refusal rule |
| --- | --- | --- |
| all record fields (side, quantity, limits, coefficients, …) | the verified `OrderSlot` record, via the existing streaming page cursor | the page's own digest/decode refusals, already landed |
| `canonical_order_id` | **derived, not stored**: the record's rank in the frozen page-set fold, plus one — the cross-page order-id chain `verify_page_set` already establishes fixes one strictly increasing visit order, so rank is deterministic and needs no storage; the feed passes rank+1 and the relation's own monotonicity check (`NonCanonicalOrderOrder`) refuses any walk that visits pages out of set order | refuse the feed (feed-protocol, not relation) if a page is visited out of `page_index` order or a record out of slot order |
| `owner: u16` | **first-appearance interning during the same walk**: the feed keeps a 64-entry table of 32-byte owners in `ClearWork` (2 KiB) and tags each order with the index of its owner's first appearance.  This makes the tag bijective into `0..owner_count` *by construction* — the property SOLANA_LAYOUT.md notes nothing currently proves.  `EpochAccount` freeze records `owner_count` = the distinct-owner count of the same walk | refuse if a 65th distinct owner appears (`TooManyOrders` mirrors the relation bound); refuse the epoch if the frozen `owner_count` differs from the interning count (tamper) |
| `expiry_epoch` | **epoch-level single expiry**: no record persists an expiry, so the epoch account carries one `expiry_epoch` for the whole frozen set and the feed supplies it uniformly.  Trade-off stated plainly: this makes every order live exactly its epoch (no GTC granularity); the alternative — a per-order u64 — requires a page-format revision, which the cancellation tombstone finding already puts on the table.  Recommend epoch-level now, per-order field folded into the same format revision as the tombstone later | refuse the feed if `epoch.expiry_epoch < domain.epoch` (the whole set is stale); per-order admission then never fires, by construction |

### Candidate and ClearWork accounts

* **Candidate account** (solver-written, frozen at proposal): the
  `StreamCandidateV1` header (order_len, 16 prices, sigma, mu, mask, claimed
  score, claimed digest — ≈ 250 B) plus the fill array as `order_len` u64s
  (≤ 512 B) read in step with the page walk, plus, under `ExplicitSlices`,
  up to 416 slices (≈ 7.5 KiB).  The relation refuses fills the pages cannot
  justify; the account only has to deliver them in walk order.
* **ClearWork account**: the §7 object (48,592-byte body; rent-funded by the
  clearing crank; one per (market, epoch, candidate) — reusable across
  candidates only via `begin`, which resets it).  `consumed_fold` must be
  bound to the layout's `order_set` digest at pass-1 finalize: record
  `(order_set, consumed_fold)` in the ClearWork account and refuse any later
  pass whose epoch shows a different `order_set`.  That is the cryptographic
  anchoring of §6: SHA-256 page digests authenticate the bytes; the in-crate
  fold authenticates the walk.
* **Compute envelope**: each pass is one page-set walk; PlaceOrder's measured
  177-block fold already brushes the default budget, so a pass will need
  `SetComputeUnitLimit` and, plausibly, mid-pass checkpointing across
  transactions — which is exactly what `ClearWorkV1`'s cursor supports; the
  design requires no per-pass atomicity beyond the account itself.

### What stays refused

> **Current disposition (2026-08-19).** The general streaming relation remains
> blocked as described below. A separate, deliberately narrow same-page,
> full-fill, direct single-Egg settlement slice is now executable and is
> specified in [COUPLED_SETTLEMENT_V1.md](COUPLED_SETTLEMENT_V1.md). It does not
> imply that this general projection or verifier has landed.

Until the general projection lands, relation-backed `SettlePage` keeps refusing. The streaming
verifier removes the *frame* blocker only; the projection items above are the
remaining blockers, and each has a named owner (layout lane) and a named
refusal rule.  No claim is made that the projection is implemented, and
nothing in this crate consumes it.

## 11. Non-claims

* Not verified: no machine-checked theorem covers the streaming/batch
  equivalence or P-BATCH-03; both are design obligations backed by the §8
  bounded oracles.  The formal-shadow home for P-BATCH-03 is the same Rocq
  target the batch relation's feasibility theorem names (BATCH_RELATION_V1
  §8.4); nothing is discharged there yet.
* Not an SVM relation: `clutch-batch` stays Solana-independent; the SBF probe
  measures frames, it does not make the crate a program.
* Not an optimality claim: the streaming verifier accepts exactly the batch
  relation's **best valid submitted candidate** semantics, nothing stronger.
* The `mix` fold is a deterministic identity, not a cryptographic commitment;
  §10 assigns the cryptographic anchoring to the layout's SHA-256 digests.
