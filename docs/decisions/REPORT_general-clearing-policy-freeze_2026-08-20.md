# Decision report — general-clearing-policy-freeze (A1 + A2)

Register entries: `docs/decisions/DECISION_REGISTER_2026-08-20.md` A1
(`general-clearing-policy-freeze`), A2 (`candidate-window-slots-pin`), with A3
(`carried-policy-freeze-queue-retirement`) riding as bookkeeping. Every cite
below was read in-tree at the head of this branch; the executable
justifications were re-run, not recalled
(`cargo test --manifest-path research/batch-policy-identity/Cargo.toml
--offline --all-targets`: **57 passed, 0 failed**, including the dust-choice
liveness test re-run by name).

**Recommendation up front: freeze both as pinned.** No selector deserves a
different pin before freeze (§2); the window pin should stay a fixed 1,000
slots (§3); the R-b exercising-test demand is satisfied with cites (§4); the
carried 1a/1b/1c-era freeze queue retires against this act except the two
halves that were never policy-const material (§5).

---

## 1. What freezing does

`GENERAL_CLEARING_POLICY_V1`
(`research/batch-policy-identity/src/general_clearing_v1.rs:70-82`) is a
`FrozenPolicyV1` whose canonical 64-byte artifact and domain-separated SHA-256
identity

```text
7a9ea80b819f853d9523a5e0ed0bb8e5ab4e167ab0c2245316775955c7a2065b
```

are already pinned against an independent third SHA-256 implementation
(`general_clearing_policy_identity_value_is_pinned`, general_clearing_v1.rs:299-338)
and already **compiled into the sealed program**: general `InitEpoch` (tag 49)
refuses any policy artifact that is not byte-for-byte this const
(`programs/clutch-sbf/program/src/instructions/orders_batch/general_epoch.rs:469-486`,
the `policy == GENERAL_CLEARING_POLICY_V1` equality gate at `:480`). The
status marker is the only thing that moves: "Ember's sign-off is what freezes
it" (general_clearing_v1.rs:22-24; TIER2 plan :239-240;
BATCH_POLICY_IDENTITY_V1.md:268; CURRENT_TRUTH.md matrix row "PROPOSED
pending ember's freeze").

After sign-off, the digest becomes load-bearing across epochs:

- **Every general epoch, forever, binds it.** `epoch.policy ==
  batch_policy_digest(artifact)` is enforced at init; the policy account PDA
  is seeded by the digest (`seeds::batch_policy_pda(epoch, digest)`,
  general_epoch.rs:32-41, seeds.rs:254); the walk re-derives it
  (settlement.rs:734-736, retired `FrozenPolicyPreimage` row).
- **Selectors cannot be substituted beneath the identity.** The
  `FullRelationDomainV1` 284-byte preimage embeds both the policy digest and
  the complete 64 policy bytes (BATCH_POLICY_IDENTITY_V1.md:112-118), and
  every selection tie digest derives from that domain. Every selector
  mutation, every registered alternative, and both fee boundaries provably
  move the identity (`every_selector_mutation_moves_the_general_clearing_identity`,
  general_clearing_v1.rs:341-421; plus the crate-root one-byte mutation sweep
  over all 64 bytes and the 10,368-product round trip,
  BATCH_POLICY_IDENTITY_V1.md:186-199).
- **What can never change after:** the ten selector values and the zero-fee
  parameter under this name and digest, and — via `RELATION_VERSION_V1`
  folded into the domain — the V0–V8 semantics those selector bytes denote. A
  wanted change is never an edit; it is a **sibling const with a new digest**
  plus a program-side admission change (the `:480` equality gate must learn
  the sibling), i.e. a new ELF identity and a reseal. This is the same
  sibling-const pattern the revenue design already standardizes
  (REVENUE_POLICY_V1.md:81-83, :305-307, :326-335).
- **What freezing does *not* do:** it does not promote anything. The plane
  stays `UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY` with `decision_owner: ember`
  (policy.py:366-385, :475-486; CURRENT_TRUTH matrix). Freezing is the
  *prerequisite* D1 promotion analysis needs, and it is what lets liveness
  rows, Terms language, and receipts cite a policy identity that is immutable
  rather than proposed.

---

## 2. Selector-by-selector analysis

Registry: `research/batch-policy-identity/src/lib.rs:286-451`
(`encode_batch_policy`/`decode_batch_policy`; wire bytes 12–21 plus the fee
field at 22–27, BATCH_POLICY_IDENTITY_V1.md §2). Decode accepts every
*registered* variant so registered semantics have stable identities;
`FrozenPolicyV1::validate` (relation_v1.rs:260-270) separately refuses the
unimplemented ones at execution. Pinned artifact bytes 12–21:
`00 00 00 01 03 01 00 01 00 00`.

| byte | selector | pinned (wire value) | registered alternatives | evidence class |
|---|---|---|---|---|
| 12 | allocation | PricePriorityMarginalProRata (0) | FullProRata (1) — implemented | canonical precedent — **convention flag** |
| 13 | self_cross | RefuseOverlap / N-a (0) | NetAtAdmission (1), AllowGateAtPairing (2) — both implemented; direct pins N-c | plan-pinned pass economy — **consequence flagged** |
| 14 | aon | RefuseAdmission / 2a (0) | WitnessedHonoredMask (1), FullSizeCounting (2) — both implemented | precedent + fail-closed — **convention flag** |
| 15 | rounding | TerminalOwnerFloor / R-b (1) | None / R-a (0), ReceiptFloor / R-c (2) | **executable** (liveness + dominance) |
| 16 | residual_settlement | UniqueSliceReceipts / 1c (3) | FullPairOnly (0), CumulativePairCanonical (1), CumulativePairFree (2, documented strand hazard) | runtime-built (T2-8) + precedent |
| 17 | transfer_phase | ActiveOrResolved / T-b (1) | ActiveOnly / T-a (0) | design liveness argument + precedent |
| 18 | portfolio_lots | StrictWholeOrder / P-a (0) | MarginalProRataLots (1) — registered, `validate()` **refuses** | forced (only implemented variant) |
| 19 | pairing_witness | ExplicitSlices (1) | RecomputedConstructor (0) — direct's pin | architecture-forced (streaming) |
| 20 | dust | AssignCanonical (0) | Reject (1) — direct's pin | **executable** (liveness) |
| 21 | score | LexicographicDispersionV1 (0) | none registered | forced (registry cardinality 1) |
| 22–27 | fee_base | None, 0 bps (tag 0) | FlatNotional { bps ≤ 10,000 } (tag 1) | deliberate deferral of B1 |

Note against the register's shorthand: the general profile matches
`DIRECT_POLICY_V1` (direct_window_v1.rs:63-75) on only seven of eleven
members. It deviates on **self_cross** (N-a vs direct's N-c), **rounding**
(R-b vs R-a), **pairing_witness** (ExplicitSlices vs RecomputedConstructor),
and **dust** (AssignCanonical vs Reject) — each deviation argued below, none
accidental.

### 2.1 The two executable pins

**dust = AssignCanonical.** The justification is a runnable test, not prose:
`general_clearing_dust_choice_keeps_remainder_books_clearable`
(general_clearing_v1.rs:429-456; re-run green for this report). A three-order
book (buys of 4 and 3 against a sell of 5 at price SCALE/2) whose
largest-remainder floors leave one atom clears under the pinned profile with
the streaming and full-width verdicts identical — and under `DustPolicy::Reject`
the canonical constructor returns `Err(ErrorV1::DustRejected)`: **no valid
candidate exists at all**. The hazard is generic on many-order marginal
pro-rata pools. Supporting arguments (general_clearing_v1.rs:37-51,
BATCH_POLICY_IDENTITY_V1.md:276-283): both relation test suites freeze
AssignCanonical in their base policies; the domain's `remainder_seed` exists
*solely* as the largest-remainder tie-break seed and would be a dead field
under Reject; and `DIRECT_POLICY_V1`'s `Reject` precedent carries no force —
at two orders every pool has one member whose floor equals the target, so
direct dust is structurally zero.

**rounding = TerminalOwnerFloor (R-b).** Same hazard family, executable at
the relation level: under `RoundingBoundaryV1::None` (R-a, exact-or-refuse)
any candidate whose per-owner cash conversion leaves a remainder refuses with
`ErrorV1::RemainderRequired` (relation_v1.rs:2131-2133), which
`consideration_remainder_has_exactly_one_owner_per_frozen_variant`
(relation_v1_tests.rs:1423-1434) demonstrates on a live book — generic on
general books for the same reason dust rejection is. Against R-c
(ReceiptFloor), the same test states a dominance fact executable-style:
per-receipt flooring produced a 20,000-price-unit pot where per-owner
flooring produced 10,000, with the invariant "more rounding events can never
mean fewer remainder atoms" asserted (relation_v1_tests.rs:1415-1421). R-b
minimizes remainder magnitude while staying total. Two boundaries to record,
not hazards: (a) the artifact freezes only the *boundary selector*; the
rounding **pot's sweep destination** (BATCH_RELATION_V1_DESIGN.md §9.2
proposes fee-revenue-at-close, never Hoard) is NOT in the 64 bytes and stays
an open revenue-cluster decision; (b) on-chain today the entitlement freeze
**refuses** any summary with a nonzero pot (entitlement.rs:18-19, :324-331 —
the standing `VirtualPot` blocker, settlement.rs:758-761), so the funded-pot
consumption path is deliberately unreachable until C5 lands. Freezing R-b
freezes the computation semantics; the consumption seam stays fail-closed.

### 2.2 The forced pins

**portfolio_lots = StrictWholeOrder** — the only variant the relation
implements; selecting `MarginalProRataLots` refuses with
`PolicyVariantUnimplemented` (relation_v1.rs:261-263). The alternative is
registered so its future identity is stable, exactly as designed
(BATCH_POLICY_IDENTITY_V1.md:78-83). **score =
LexicographicDispersionV1** — the registry has exactly one member
(lib.rs:431-442). Neither is a choice today; freezing them is honest
bookkeeping of the implemented universe. A future P-b or second score family
arrives as a sibling profile.

### 2.3 The architecture-forced pin

**pairing_witness = ExplicitSlices.** The on-chain streaming walk verifies a
candidate feed that *persists* the pairing slices and streams them through
`push_slice` (T2-5/T2-6c; general_clearing_v1.rs:31-32, the plan's staged
four-tag feed wire — a 6,266-byte feed cannot ride one transaction,
GOAL.md:119-124), and T2-8's entitlements create one receipt PDA per
`(candidate, slice_index)` — receipts need persisted slices to exist.
Direct's `RecomputedConstructor` works there because the direct profile
carries no witness at all and the constructor runs once at finalization
(lib.rs:258-265). For the general plane the explicit witness is what the
runtime is built on; the full candidate digest commits to every slice so an
explicit witness cannot be swapped under the tie identity
(BATCH_POLICY_IDENTITY_V1.md:155-162). Both variants refuse the same books
(relation_v1.rs:196-203), so this is representation, not economics.

### 2.4 The argued deviations and conventions

**self_cross = RefuseOverlap (N-a).** Plan-pinned for pass economy: two order
passes plus one slice pass instead of three order passes
(TIER2 plan §4 envelope, :379; general_clearing_v1.rs:36). This is the one
selector where the pin has a real recorded cost: `refuse_self_cross` refuses
the **whole book** (`ErrorV1::SelfCrossRefused`, relation_v1.rs:758-788), so
one owner standing on both sides of one outcome makes every candidate refuse
and the epoch lapse ("zero-verified lapses honestly", GOAL.md:126) — a
grief-to-lapse vector priced at one overlapping order plus its reservations
and rent. The alternatives do not remove it cheaply: N-b (`NetAtAdmission`)
still refuses any overlap involving a portfolio order because lot-coupled
netting is an unresolved design question (relation_v1.rs:821-834), and costs
the third pass; N-c (direct's pin) moves the gate to V5 pairing feasibility,
which is trivially checkable at two orders and would need the extra pass
here too. Two bounds on the hazard: the griefer's lapse also releases nothing
early for the griefer, and the current lapse-reservation-release gap it would
exploit is the already-standing `TerminalClosure` row (settlement.rs:762-766)
regardless of the self-cross policy. The P2 backlog row "same-Epoch
crossings/self-crosses beyond RefuseOverlap" (OPEN_QUESTIONS.md:68-88, A4)
stays open for a V2 sibling. **Verdict: keep N-a; record the grief-to-lapse
consequence as a known property, not a surprise.**

**aon = RefuseAdmission (2a).** Matches direct and both relation test bases;
keeps `honored_aon_mask == 0` for the first general profile
(general_clearing_v1.rs:58-59). Both alternatives are implemented
(relation_v1.rs:1521, :2394, :3146 for the witnessed mask; 2c per the scalar
lab), so this is **pinned by precedent and fail-closed conservatism, not by
an executable comparison** — flagged as such. It is the correct freeze
posture anyway: refusing AON at admission is a pure narrowing (no admitted
order's semantics depend on it), and widening to 2b/2c later is a sibling
profile, not a broken promise to anyone whose order was admitted under 2a.

**allocation = PricePriorityMarginalProRata.** `FullProRata` is a real
implemented alternative (relation_v1.rs:1646-1650, :1743-1754; exercised in
relation tests :198, :531). The pin follows the relation's canonical
allocation — every base policy in tree pins it, the canonical constructor's
strict-first pass and the dispersion-weighted score were designed around it —
but no executable comparison for the general book exists. **Pinned by
canonical precedent — convention flag.** Both variants are total (no
liveness hazard either way); the difference is distributional (strict orders
fill whole before marginal pro-rata vs everything pro-rata), i.e. a fairness
choice a V2 could revisit with actual market evidence. Not worth blocking a
V1 freeze that the entire sealed evidence corpus already executed under.

**residual_settlement = UniqueSliceReceipts (1c).** The strongest kind of
evidence short of a falsifier: **the runtime is built on it** — T2-8 creates
one receipt PDA per `(candidate, slice_index)` (general_clearing_v1.rs:66-67,
settlement.rs:740-742), matching direct. The 1b-free alternative carries a
documented strand hazard the crate does not discharge (relation_v1.rs:168-170).
This is the selector that retires the old 1a/1b/1c queue row (§5).

**transfer_phase = ActiveOrResolved (T-b).** Recorded for the kernel layer
rather than enforced by the relation (relation_v1.rs:175-182). The design's
own analysis recommends T-b "for liveness": T-a strands unsettled legs when
settlement races resolution (BATCH_RELATION_V1_DESIGN.md:871-873). Matches
direct. The 08-19 review's note that the T-a/T-b choice "needs the §14.2
epoch/resolution ordering rule" is satisfied in the direction that matters
for freezing: T-b is the variant that does *not* depend on winning that race.

**fee_base = None (0 bps).** Deliberate non-preemption of B1
(general_clearing_v1.rs:24-26, TIER2 plan :362-364), independently enforced
by the five `max_fee_atoms == 0` program gates (orders_batch.rs:910,
direct_selection.rs:908-909, :1759, plus the settlement pair; register B4).
See §6.

### 2.5 Does any selector deserve a different pin before freeze?

**No.** The two selectors with genuine competing options (dust, rounding)
have executable liveness evidence *for* the pinned values and against the
alternatives. The forced pins have no alternative to take. The
architecture-forced pin is what the shipped runtime is. The three
convention-flagged pins (allocation, aon, self_cross) are each fail-closed or
distribution-neutral choices whose plausible revisions are widening moves
that arrive naturally as sibling profiles — and every one of them is the
value the entire sealed Tier-2 evidence corpus (bank walks, 57/57 profile
tests, verdict-identity gates) actually executed. Amending any selector today
discards that binding for no demonstrated gain (§8 prices this).

---

## 3. The window-slots pin (A2)

`CANDIDATE_WINDOW_SLOTS = 1_000`
(`programs/solana-layout/src/clearing.rs:743-754`): `FreezeEpoch` stamps
`selection_deadline_slot = freeze_slot + 1_000` into the epoch's window
account (general_epoch.rs:27, :453; EpochWindowAccount v2,
clearing.rs:850-879). The freeze that seals the book opens candidate
submission; nothing else consumes the const.

**Wall clock.** At the 400 ms nominal slot, 1,000 slots ≈ **400 s ≈ 6.7
minutes** (the register's ~400 s); at observed cluster slot times (~400–500
ms) call it **7–8 minutes**. Adequacy check against measured facts: candidate
submission is the staged four-tag wire (a feed lands over roughly a dozen
transactions, i.e. seconds), selection worst case is ~49k CU (GOAL.md:116-127),
and the registry retains 3 candidates with displacement by verified components
only. Several independent submitters fit in the window with two orders of
magnitude to spare; the countervailing cost of a long window is only capital
lockup (frozen books hold reservations until selection or lapse). 1,000 is a
sane, honest first pin — neither a race nor a parking lot.

**The operator-chosen alternative and why it was out of bounds.** Making the
window epoch-creation-chosen requires revising the `Intent::InitEpoch`
(tag 49) wire, which "is sealed at the attested baseline"
(clearing.rs:749-751). That is: a program change → a new ELF identity (every
closure-byte change forks it) → a full reseal cycle (seal → gates → manifest
100/100 → post-commit check → independent portable attestation) and
re-recording of the just-sealed T2-7/T2-8 evidence families — squarely
outside a lane whose job was the selection lifecycle, and wrong to smuggle
into it. It also buys less than it appears to: general InitEpoch is
permissionless, so "operator-chosen" means **creator-chosen**, and a hostile
creator choosing a 1-slot window (lapse everyone) or a u64::MAX window (lock
capital forever) forces you to freeze validated min/max bounds anyway. You
end up pinning *two* schedule consts plus a new validation surface instead of
one const.

**Intermediate options.** A per-Realm (or per-market) const table is the
worst of both: still an in-ELF change today, still frozen (an immutable
program cannot grow the table for new Realms), N numbers to defend instead of
one, and no wire flexibility gained. If differentiated windows are ever
actually wanted, the honest shape is the tag-49 wire revision *with frozen
bounds*, taken as its own decided reseal-cycle act — at which point the
then-frozen 1,000 becomes the documented default. Nothing about freezing
1,000 now forecloses that; it is the cheapest reversible-by-supersession pin
in this whole report.

**Verdict: freeze `CANDIDATE_WINDOW_SLOTS = 1_000` in the same sign-off act
as A1.**

---

## 4. The R-b rounding-boundary test question — resolved: SATISFIED

**The demand.** Carried in three consecutive drift reviews: "R-b's rounding
boundary still needs an exercising test before any R-b freeze"
(DRIFT_REVIEW_2026-08-19B.md:437-438; DRIFT_REVIEW_2026-08-19.md:317-320;
DRIFT_REVIEW_2026-08-18.md:266-268). Its origin is precise and narrower than
the carried sentence: the **vertical-model integration record** — the coupled
golden trace carries the R-b pot per ledger but settles in exact price units
and never draws on it, "so R-b's conversion boundary is recorded, not
exercised, **on that path**" (BATCH_RELATION_V1_DESIGN.md:1045-1050;
VERTICAL_MODEL.md:171-176). Since `GENERAL_CLEARING_POLICY_V1` pins
`rounding: TerminalOwnerFloor`, this freeze **is** an R-b freeze and the
demand binds it.

**What exists now.**

1. **A direct exercising falsifier at the relation level** — the level whose
   semantics the frozen selector byte denotes:
   `consideration_remainder_has_exactly_one_owner_per_frozen_variant`
   (`crates/clutch-batch/src/relation_v1_tests.rs:1376-1445`, the falsifier
   named in BATCH_RELATION_V1_DESIGN.md:958). It is nonvacuous: under R-b it
   produces a **nonzero pot of 10,000 price units**, asserts debit rounds up
   (2 atoms) and credit floors (1 atom), and asserts the conservation
   identity `debit_remainder + credit_remainder == rounding_pot`. It then
   contrasts all three variants on the same book: R-c pots 20,000 with "more
   rounding events can never mean fewer remainder atoms" asserted, and R-a
   refuses with `RemainderRequired`.
2. **Streaming parity across all three variants:**
   `stream_matches_batch_on_fee_and_rounding_variants`
   (relation_v1_stream_tests.rs:873+) drives a crossing book at price SCALE/3
   (guaranteed remainders) through {R-a, R-b, R-c} × {no-fee, 30 bps} and
   asserts the streamed verdict equals the batch relation's, mutations
   included.
3. **The pinned profile itself exercises a nonzero pot in its own T2-5
   gate:** the dust test's book (odd 3/2 fill split at price SCALE/2,
   general_clearing_v1.rs:429-442) makes one buyer's conversion round up by
   half a scale and the seller's floor by half a scale — pot = one full
   SCALE of price units — and `assert_verdict_identity` compares the complete
   streamed economics (pot field included) against
   `verify_submitted_candidate`'s. Re-run green for this report.

**Chronology of the carry.** The relation falsifier landed in `f7caf04`
(2026-08-18 02:35), hours *before* the 19B review was committed (`25fb3c2`);
the demand sentence lived inside the carried VM-INT pair item and was carried
forward verbatim without being re-checked against the relation suite. The
carried sentence outlived the gap it named.

**What remains true, honestly bounded.** (a) The **vertical-model path**
still never draws on the pot — the original VM-INT flag stands, but it is a
statement about that model's fixture fidelity, belongs to the H2 carried
items, and does not gate the selector freeze: the frozen byte's semantics
live in `clutch-batch`, where the boundary is exercised. (b) **On-chain**, no
SBF walk floors a nonzero pot into a funded account, because the entitlement
freeze deliberately refuses nonzero pots until `VirtualPot` lands
(entitlement.rs:324-331; the SVM walk asserts pot == 0,
entitled_clearing.rs:1457). That is the recorded C5 blocker doing its job —
fail-closed, not unexercised-by-accident.

**Verdict: the demand is satisfied for this freeze; no test is owed as a
freeze-blocker.** Record it discharged (against the three review cites) in
the sign-off note. One cheap non-blocking follow-up is worth taking: add an
explicit `assert_ne!(rounding_pot, 0)` (plus the exact expected value) to the
T2-5 profile suite so the pinned profile's nonzero-pot exercise is pinned on
purpose rather than implied by the dust book's shape.

---

## 5. Freeze-queue retirement (A3)

The carried 08-18/08-19 policy freeze queue — DRIFT_REVIEW_2026-08-19B.md
row 21 (:378) and G.2 item 2 (:407-413), DRIFT_REVIEW_2026-08-19.md G.1
(:301-310), GOAL.md:1231 (historic queue item 5: "residual 1a/1b/1c, lots,
AON, fee carry"), pointing at POLICY_ANALYSIS_LOTS_FEES.md — retires against
this freeze as follows. The reviews themselves are immutable records; the
retirement is recorded here, in the sign-off note, and by striking the
GOAL.md:1231 row (GOAL is living).

| carried row | disposition |
|---|---|
| residual settlement 1a / 1b-canonical / 1c (1b-free refused at clear time) | **Subsumed** by `residual_settlement: UniqueSliceReceipts` (1c), wire byte 16 = 3; the runtime consumes it (T2-8 per-slice receipts). |
| transfer phase T-a vs T-b | **Subsumed** by `transfer_phase: ActiveOrResolved` (T-b), byte 17 = 1; the design's own liveness recommendation. |
| AON | **Subsumed** by `aon: RefuseAdmission` (2a), byte 14 = 0; 2b/2c remain registered for a sibling. |
| lots — the batch half (portfolio lot rationing P-a/P-b) | **Subsumed** by `portfolio_lots: StrictWholeOrder`; P-b stays registered-and-refused. |
| lots — the kernel fractional-payout half (a1/b1/c, complete-set primitive) | **Not this act.** Already decided as lot-scaled bearer units in FAILURE_PAYOUT_DECISION_V1; its *ratification* is C4, a separate sign-off. |
| fee arms (terminal-ceil vs dropped-carry, kappa, 60/15/25, executor cap) | **Not subsumed and deliberately so** — `fee_base: None` does not preempt the fork; stays B1/B4. |
| VM-INT trace naming (`golden/coupled.trace` vs `relation_v1.trace`) | **Not covered** by any selector; stays open under H2, exactly as register A3 option 2 anticipated. |
| R-b exercising test (rode the same VM-INT item) | **Discharged** per §4, with cites. |

Net effect: after this sign-off, no future review should re-list residual /
transfer-phase / AON / batch-lots as open ember decisions; the only survivors
of the old queue are C4's ratification, B1's fork, and H2's trace-naming
ruling.

---

## 6. Interactions

- **Fees (B1/B4) — this freeze does not block them, structurally.**
  `fee_base` is a digest-folded policy member and a later fee profile is a
  **sibling const with a new digest**, never a mutation
  (REVENUE_POLICY_V1.md:326-335; the same pattern at :305-307). The fee path
  already requires a program change regardless of this freeze — the
  general_epoch.rs:480 equality gate must learn the sibling, and the
  candidate ABI must grow fee columns (§8.2 of that design) — so freezing the
  zero-fee profile now adds *zero* marginal cost to the future fee fork.
  Meanwhile the five `max_fee_atoms == 0` gates enforce zero independently of
  the policy bytes. The one fee-adjacent residue this freeze creates: the
  frozen R-b selector produces a rounding pot whose **sweep destination**
  (proposed: fee revenue, never Hoard — BATCH_RELATION_V1_DESIGN.md §9.2)
  is not in the artifact; that decision joins the revenue cluster and C5's
  VirtualPot work.
- **Promotion (D1, and D2's competition).** A1+A2 are D1's named
  prerequisites; freezing converts "PROPOSED pin" from a machine-refused
  promotion input into a frozen one. It does not itself derive admission
  rows, flip `live_*` flags, or pick between the general plane and V3 as
  promotion target.
- **R4 / terminal (C1, C5, C6).** Unaffected in both directions: the
  standing PartialFillLedger / VirtualPot / TerminalClosure blockers
  (settlement.rs:768-772) neither gate the policy freeze nor are eased by
  it. The lapse-reservation-release gap noted in §2.4 is TerminalClosure's
  row, already recorded. The plane's terminal rows (C5/C6) remain the
  promotion blockers they were.
- **A4 (P2 backlog).** This freeze pins V1 answers for self-cross scope, AON,
  and lots-rationing; the P2 rows stay open as V2/sibling questions, and A4's
  real content becomes "which rows did A1 just retire" — several
  (PRICE_SCALE, tie digests, RefuseOverlap, StrictWholeOrder) now have frozen
  in-code answers.
- **The window pin (A2)** rides the same wire and the same sign-off; there is
  no ordering between them.

---

## 7. Recommendation and counterargument

**Recommendation: freeze both as pinned, in one sign-off act** — the
`GENERAL_CLEARING_POLICY_V1` const at digest `7a9e…065b` and
`CANDIDATE_WINDOW_SLOTS = 1_000` — recording alongside it: the §4 discharge
of the R-b demand, the §5 queue retirements, the self-cross grief-to-lapse
consequence as a known recorded property, and the rounding-pot sweep
destination as explicitly *not* decided by this act.

**The strongest counterargument, stated fairly.** Four selectors deviate
from the only shipped precedent, three are pinned by convention rather than
executable comparison, and the runtime the profile governs is still
full-fill-only, virtual-leg-refusing, and pot-refusing — so several frozen
selectors' general-book behavior is exercised only at host level plus bank
evidence that never entitles a nonzero pot or a partial fill. Freezing before
PartialFillLedger and VirtualPot land means the dust/rounding/AON selectors'
interaction with partial fills has never run anywhere. Why not wait until C5
closes the seams and freeze once, with total coverage?

**Why the counterargument loses.** (1) The waiting has no object: dust and
rounding are *relation-level* semantics, complete and exercised now; the
standing blockers are consumption seams that refuse, not semantics that might
change — and if landing PartialFillLedger ever did argue for different
selector values, that argument would produce a **V2 sibling profile**, which
is exactly as available after freezing as before. (2) The asymmetry of
evidence: every sealed Tier-2 artifact — the bank walks, the verdict-identity
gates, the 57/57 profile tests, the T2-7/T2-8 evidence families sealed at
cycle D — executed under *these exact 64 bytes*, and the sealed ELF
equality-gates them; amending any selector today discards that binding and
forces a reseal for a speculative benefit no falsifier supports. (3) The
convention-flagged pins are all narrowing/fail-closed choices, the cheapest
kind to widen later and the safest kind to freeze. Deferral, meanwhile, has a
real recurring cost: every review re-lists the queue, and D1 stays
machine-blocked behind a decision whose evidence is complete.

---

## 8. Execution cost

**Freeze as pinned (recommended): one documentation commit, zero program
bytes.** The ELF already enforces the const and the window value
(general_epoch.rs:480, :453); the digest test already pins the artifact
against a third SHA-256 implementation. The sign-off edit set is
status-language only:

- `research/batch-policy-identity/src/general_clearing_v1.rs:19-26` — doc
  header PROPOSED → FROZEN (sign-off date), plus the §4/§5 discharge notes;
- `programs/solana-layout/src/clearing.rs:751-753` — "**PROPOSED schedule
  pin**" → frozen;
- `docs/implementation/BATCH_POLICY_IDENTITY_V1.md:265-268` (§8 status) and
  `docs/design/TIER2_PORTFOLIO_CLEARING_PLAN_2026-08-20.md:239-240`;
- `CURRENT_TRUTH.md` matrix cell (":307" row — "PROPOSED pending ember's
  freeze" → frozen-unpromoted) and the GOAL.md queue/done-log, striking
  GOAL.md:1231's subsumed halves;
- the register's dated successor note for A1/A2/A3.

Doc-comment and prose edits do not move emitted program bytes, so no reseal
is *forced*; if the seal protocol's conservatism prefers any source-touching
commit to ride a cycle, ride the one E4's parked merge is already queued for.
The optional §4 follow-up (explicit pot assertion in the T2-5 suite) is a
research-crate test-only change with the same property.

**Amend any selector instead: a full reseal cycle.** New 64 bytes → new
digest → the program's equality gate and the pinned-digest tests change → new
ELF identity → seal → gates → manifest 100/100 → post-commit check →
portable attestation, plus re-recording the sealed T2 evidence families that
bound the old digest through `epoch.policy`. The same bill applies to the
window value (compiled into FreezeEpoch) and, with a wire-revision surcharge
(tag-49 codec + validation bounds), to the operator-chosen window variant.

---

*Report compiled 2026-08-20 for the decision-register fan-out (register item
4 of the standalone-reports list). Evidence commands run:
`cargo test --manifest-path research/batch-policy-identity/Cargo.toml
--offline --all-targets` (57/57) and the named dust test re-run singly
(1/1). No code, no program bytes, and no status lines were changed by this
report; sign-off remains ember's.*
