# Decision register — 2026-08-20

Status: **CENSUS / ASSESSMENT.** Every open decision in this repository that a
human (ember) must make, in one register, swept 2026-08-20 from the whole
tree. This document decides nothing and promotes nothing; the claim vocabulary
of `CURRENT_TRUTH.md` §1 governs. It exists to drive a fan-out of per-decision
analysis reports (list at the end). Each entry cites where the decision
surfaces; cites were read, not recalled.

Sweep sources: GOAL.md (current queue lines 47–66 plus every done-log flag),
CURRENT_TRUTH.md §2/§4/§6, docs/OPEN_QUESTIONS.md, all of docs/design/ and
docs/reviews/, the drift reviews' consolidated morning-decision lists
(docs/implementation/DRIFT_REVIEW_2026-08-19.md:301–334,
DRIFT_REVIEW_2026-08-19B.md:387–444), the executable blocker ledgers
(orders_batch/settlement.rs, portfolio_settlement.rs), the liveness profile
(policy.py / terminal_profile.py / evidence.json), and tree-wide greps for
PROPOSED / decision_owner / pending ratification / sign-off / human-gated /
unfrozen / "V1 has no" / "deliberately waits".

Owner vocabulary:
- **ember** — a value/authority call; evidence is in-tree and sufficient.
- **ember+counsel** — requires qualified legal input (Gate-L0-adjacent).
- **ember-after-evidence** — the decision is real but its inputs are not yet
  assembled; deciding today would be a guess the project's own gates refuse.

Entry fields: statement / owner / surfaces / options / evidence / blocked-on-it
/ urgency / interactions.

**Status pass, 2026-08-20** (added after ember's adoption record,
[ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md)). Entries the adoption record
covers now carry a `**Status:** DECIDED` line naming the adoption item; the two
explicitly deferred decisions carry `**Status:** DEFERRED` with the reason as
the record states it. **No analysis below was rewritten** — the options,
evidence, and counterarguments stand as swept, and the reports the adoption
cites carry the counterarguments as part of the record. Entries reserved to
ember by that record (the filing submissions, E2, E3, the treasury pubkey,
counsel/security/license engagements, mainnet, L0) are deliberately left
**unchanged**, as are the entries the record does not reach — those remain open
and unmarked, which is the honest reading: an unmarked entry is not decided.

Known hard dates in this register: **Aug 24** (two CFTC filings + John-packet
prerequisites), **Aug 26** (perpetuals RFC due; Pyth receiver cutover
16:00 UTC), **Aug 27** (IAC written-statement "should submit by"), **Oct 5**
(conflicts NPRM), and the ~**Oct 19** close of the Compute Derivatives RFC
(60 days from 2026-08-19 FR issuance, exact FR date to confirm).

---

## Cluster A — policy freezes (clearing plane)

### A1. `general-clearing-policy-freeze`

- **Status: DECIDED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) item 1.**
  Frozen as pinned (option 1). The pins are the enabling choices; a future
  profile is a sibling const with a new digest, so the freeze forecloses no
  dynamics. The in-source doc-comment status updates ride the next
  reseal-bearing wave rather than opening a drift window for a comment.
- **Decision:** freeze `GENERAL_CLEARING_POLICY_V1` (the Tier-2 general
  portfolio-clearing `FrozenPolicyV1` profile) as pinned, or amend selectors
  before freezing. Sign-off is what turns PROPOSED into frozen.
- **Owner:** ember.
- **Surfaces:**
  `research/batch-policy-identity/src/general_clearing_v1.rs:19-82` (the
  const, "Ember's sign-off is what freezes it");
  `docs/design/TIER2_PORTFOLIO_CLEARING_PLAN_2026-08-20.md:236-240`;
  `docs/implementation/BATCH_POLICY_IDENTITY_V1.md:268`; `CURRENT_TRUTH.md:307`
  (matrix: "PROPOSED pending ember's freeze"); `GOAL.md:34-35,265-267`.
- **Options:** (1) freeze as pinned (fee None/0bps, dust AssignCanonical,
  RefuseOverlap, TerminalOwnerFloor, AON RefuseAdmission, ExplicitSlices,
  StrictWholeOrder, UniqueSliceReceipts, ActiveOrResolved,
  LexicographicDispersionV1); (2) amend the dust choice to `Reject`
  (documented liveness hazard: pro-rata leftover atom leaves NO valid
  candidate — `general_clearing_v1.rs:43-51` and its executable test);
  (3) amend rounding to `None` exact-or-refuse (refuses generic remainder
  books — same hazard family); (4) defer — the whole plane stays
  evidence-only.
- **Evidence:** the T2-5 cross-plane verdict-identity gate (streaming verdict
  == `verify_submitted_candidate` V0–V8); the dust-choice clearability test;
  `DIRECT_POLICY_V1` precedent with its two-order vacuity argued per selector
  in the doc comment. Open check: the 08-19 drift review demanded an
  exercising test for the R-b rounding boundary "before any R-b freeze"
  (`docs/implementation/DRIFT_REVIEW_2026-08-19B.md:437-438`) — verify the
  Tier-2 suites now exercise it, or that demand carries into this freeze.
- **Blocked on it:** any promotion of the general clearing plane (walk, T2-7
  selection, T2-8 entitlement) out of
  `UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY`; any liveness/admission row for
  tags 47–59; D1 in the fan-out sequencing (this is the top item in GOAL's
  "next 3 moves").
- **Urgency:** rank 1 (no calendar date; first item of the morning queue).
- **Interactions:** A2 (window pin rides the same wire), B1 (fee_base pinned
  None deliberately does NOT preempt the fee fork), D1/D2 (promotion),
  C5 (TerminalClosure rows for the plane's accounts). Subsumes most of A3.

### A2. `candidate-window-slots-pin`

- **Status: DECIDED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) item 1.**
  Frozen at 1,000 slots in the same act as A1 (option 1; no wire change).
- **Decision:** freeze `CANDIDATE_WINDOW_SLOTS = 1_000` as a fixed schedule
  pin, or move the window length onto a revised `InitEpoch` (tag-49) wire as
  an operator-chosen parameter.
- **Owner:** ember.
- **Surfaces:** `programs/solana-layout/src/clearing.rs:743-754` ("PROPOSED
  schedule pin … freezing the value (or moving it onto a revised InitEpoch
  wire) is ember's sign-off"); `GOAL.md:124-126`.
- **Options:** (1) freeze 1,000 slots (~400 s) — simplest, no wire change;
  (2) different fixed value; (3) tag-49 wire revision making it
  epoch-creation-chosen — flexibility at the cost of a wire change to a
  sealed-baseline intent and a new validation surface.
- **Evidence:** T2-7 merged and measured (selection CU ~49k worst,
  `GOAL.md:116-127`); the V3 window precedent (`MAX_RETAINED_CANDIDATES = 3`,
  clearing.rs:738-742).
- **Blocked on it:** same as A1 — the selection lifecycle cannot be promoted
  under a proposed schedule pin.
- **Urgency:** rank 1 (rides A1).
- **Interactions:** A1 (one freeze act can cover both).

### A3. `carried-policy-freeze-queue-retirement`

- **Decision:** formally retire (or re-open) the carried 08-18/08-19 policy
  freeze queue — residual-settlement variant 1a/1b-canonical/1c, transfer
  phase T-a vs T-b, AON, lots — as **subsumed** by the
  `GENERAL_CLEARING_POLICY_V1` selectors, so the old queue rows stop
  reappearing in reviews.
- **Owner:** ember (a bookkeeping ratification, but only ember can retire
  queue rows addressed to ember).
- **Surfaces:** `docs/implementation/DRIFT_REVIEW_2026-08-19B.md:378,396-401`
  (row 21 + G.2, "user decision, not code");
  `docs/implementation/DRIFT_REVIEW_2026-08-19.md:301-309`; `GOAL.md:1231`
  (historic queue item 5);
  `docs/implementation/POLICY_ANALYSIS_LOTS_FEES.md:270-297` (the analysis
  the old queue pointed at). The fractional-payout half is already decided
  (`docs/implementation/FAILURE_PAYOUT_DECISION_V1.md`; see C4).
- **Options:** (1) declare the queue subsumed by A1's freeze; (2) keep any
  row open that A1's selector does not actually cover (candidate: the VM-INT
  trace naming, DRIFT_REVIEW_2026-08-19B.md:437-439).
- **Blocked on it:** nothing runs on it; it blocks only ledger hygiene —
  every future review re-lists these rows until retired.
- **Urgency:** rank 4.
- **Interactions:** A1, C4.

### A4. `simplex-auction-p2-backlog`

- **Decision:** the OPEN_QUESTIONS P2 rows not embodied in A1's const:
  standing-maker definition, same-Epoch crossings/self-crosses beyond
  RefuseOverlap, `PRICE_SCALE`/normalization/tie-rule ratification, admitted
  portfolio-intent language (partial fills, coefficient bounds), candidate
  public score/optimality certificates, replacement window/proposer
  bond/withholding, complete-set virtual split/merge fee treatment, pro-rata
  remainder, page sizing, cancellation cutoff, commit/reveal.
- **Owner:** ember (mostly ember-after-evidence).
- **Surfaces:** `docs/OPEN_QUESTIONS.md:68-88` (P2, "before simplex-auction
  freeze").
- **Options:** per-row; many have de-facto V1 answers now living in code
  (PRICE_SCALE, tie digests, RefuseOverlap, StrictWholeOrder) — the real
  decision is which rows A1 retires versus which stay open for V2.
- **Blocked on it:** a *final* simplex-auction freeze claim; nothing in the
  current evidence-only plane.
- **Urgency:** rank 5.
- **Interactions:** A1, B4 (standing maker is a fee-split input), C5
  (PartialFillLedger is the partial-fill row's runtime half).

### A5. `partition-payoff-compiler-freeze`

- **Decision:** freeze the V1 Statistic/Partition closed enum, canonical
  boundary representation, unit algebra, Template equivalence rule; decide
  empty-cell rejection and whether portfolio coefficients are arbitrary
  bounded integers or an audited product subset.
- **Owner:** ember.
- **Surfaces:** `docs/OPEN_QUESTIONS.md:48-53` (P0);
  `docs/reviews/PLANNED_VS_BUILT_2026-08-19.md:79-83` (Template/Instance/
  Series: zero code, no retirement note).
- **Options:** (1) freeze a minimal enum matching what Terms v3 already
  expresses; (2) author the obligation-18 revision first (A6) and freeze
  both; (3) explicitly defer the compiler beyond V1 and retire the planning
  vocabulary.
- **Blocked on it:** any Template/Series (permissionless repeated creation)
  work; threshold/TWAP market families.
- **Urgency:** rank 5 (nothing active is waiting).
- **Interactions:** A6, C-cluster (payout compilers meet lots/credits).

### A6. `obligation-18-terms-revision`

- **Decision:** author the TermsAccount revision that carries statistic id,
  ambiguity policy id, coverage parameter, source/evaluator versions, and a
  boundary table + payout map inside the digest (making threshold/TWAP
  families expressible) — or leave the family closed for V1.
- **Owner:** ember.
- **Surfaces:** `docs/implementation/DRIFT_REVIEW_2026-08-19.md:310-317`
  (G.2); STAT-05's 256-bit comparison question rides along.
- **Options:** (1) author it (a schema rev + digest change — a reseal-cycle
  item); (2) closed for V1, revisit with A5.
- **Blocked on it:** threshold and TWAP-family markets (inexpressible today).
- **Urgency:** rank 5.
- **Interactions:** A5, E-cluster (source/evaluator versions bind R2 pins).

### A7. `internal-venue-ownership`

- **Decision:** close the P0 row: issuance and simplex venue in one immutable
  program (the de-facto built answer) versus a venue calling
  conservation-checking instructions on an Eggcrate-owned Position program.
- **Owner:** ember.
- **Surfaces:** `docs/OPEN_QUESTIONS.md:34-38`.
- **Options:** (1) ratify the built single-program answer and retire the row;
  (2) keep the split-program architecture as a V2 target.
- **Evidence:** the entire clutch-sbf program is the single-program answer;
  no separate venue writes Position bytes.
- **Blocked on it:** ledger hygiene only — but an unretired P0 row
  contradicts "intentionally unresolved" once code has silently selected an
  answer, which OPEN_QUESTIONS.md:3-4 forbids.
- **Urgency:** rank 4.
- **Interactions:** F2 (upgrade posture shares the "ratify what's built"
  shape).

### A8. `realm-admission-allowlist-freeze`

- **Status: DECIDED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) item 4.**
  FROZEN as built: Token-2022 base mints, extension ceiling zero,
  ImmutableOwner required on the Hoard, unknown discriminants fail closed. The
  deliberate strong choice — fail-closed admission preserves future options;
  admission-then-exploitation would foreclose them. The record states plainly
  that the DREGG dogfood mint has no executable V1 profile. (The F5 ELF pin is
  not covered by that item and remains open.)
- **Decision:** freeze the V1 collateral-profile allowlist — whether
  transfer-fee, transfer-hook, interest-bearing, confidential, rebase-like,
  or freezable Token-2022 collateral is rejected categorically; demonstrate
  generic semantics with two synthetic Realms.
- **Owner:** ember.
- **Surfaces:** `docs/OPEN_QUESTIONS.md:40-46` (P0);
  `docs/implementation/COLLATERAL_PROFILES.md` (the profile matrix; note
  `TOKEN2022_PLAN.md:733-741` — the adapter is already stricter than the
  matrix on ImmutableOwner, "named as a divergence").
- **Options:** (1) freeze the conservative matrix as implemented (categorical
  rejections); (2) widen to plain-SPL; (3) freeze after the F5 Token-2022
  ELF pin so the allowlist and the pin are one act.
- **Blocked on it:** any real Realm profile authentication/freeze/release
  (`CURRENT_TRUTH.md:292` boundary cell: "No real Realm profile is
  authenticated, frozen, or released").
- **Urgency:** rank 3 (a devnet Realm wants at least a provisional answer).
- **Interactions:** F5, E2 (a real market needs Realm + source together).

---

## Cluster B — fee / revenue

### B1. `fee-base-selection`

- **Status: DECIDED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) item 9.**
  The composite `kappa*G + kappa'*R` **SHAPE** is selected (the register's
  option 5, modeled by the report). **Both rates remain undecided**; every byte
  stays `FeeBaseV1::None` until the destination lands; reversible until a rate
  freezes. The FEE_GEOMETRY promotion criteria are rewritten per ADR-0005.
- **Decision:** select the fee base (the "fee-base fork"): flat-notional,
  per-Egg leg, atomic simplex-dispersion `G(a,p)`, or price-free
  quotient-norm `kappa'·R(a)` — or decide the comparison protocol that will
  select it. The rate (numerator) is a strictly-after decision; no numerator
  is proposed anywhere in-tree.
- **Owner:** ember-after-evidence (the comparison is startable but un-run).
- **Surfaces:** `docs/FEE_GEOMETRY.md:195-247` (§6 controls, §7 promotion
  criteria); `docs/reviews/FEE_ECONOMICS_FINDINGS_2026-08-19.md:49-67,131-162`
  (§3 gate unstartable-as-written, §6 the real shape);
  `research/economics-admission/model.py:529-533` (four arms),
  `run_lab.py:39-78` + `test_model.py:500-534` (zero-price laundering
  fixture); `GOAL.md:294-307` (the decision-relevant surprise);
  `docs/design/TIER2_PORTFOLIO_CLEARING_PLAN_2026-08-20.md:362-364` ("the
  fee-base fork remains ember's decision"); `docs/ECONOMICS.md:120-138`.
- **Options:** (1) dispersion `G` (characterized uniquely, Props 11-12; but
  shares the zero-price hole); (2) flat-notional (complete arm exists in
  `clutch-batch` relation, unreachable from the program; also shares the
  hole); (3) per-Egg leg (arm landed in the lab 2026-08-19; shares the
  hole); (4) quotient-norm `R(a)` (the ONLY arm that charges zero-priced
  risk transfer — exactly M/1000 at every tested scale to 10^30); (5) a
  hybrid floor (e.g. G plus a quotient-norm floor) — not modeled anywhere
  yet; (6) zero-fee-forever for V1 (the current byte truth).
- **Evidence:** the executable Prop-9 falsifier: **three of four arms charge
  exactly zero on risk transfer supported entirely on zero-priced outcomes —
  the evasion channel is a property of every consideration-proportional
  base, not of dispersion** (`GOAL.md:300-307`; run_lab.py:59-77). Prop 10
  (G is not the risk norm) and Props 11-12 (G is characterized) absorbed
  into FEE_GEOMETRY §3. Also decision-relevant: cancel now costs MORE CU
  than place (282,868 vs 185,807, `GOAL.md:327-329`) — no policy reads it
  yet; a fee design that prices cancellation should know it.
- **Blocked on it:** any nonzero fee anywhere (five `max_fee_atoms == 0`
  gates, REVENUE_POLICY_V1 §9); the fee-bearing `FrozenPolicyV1` sibling and
  candidate-ABI change (§8); B2's bounds only bind once a base exists;
  ECONOMICS.md §6 break-even stops returning `unbounded`.
- **Urgency:** rank 2 (behind the destination decisions B4, which
  deliberately precede it: findings §6 "decide the destination before the
  base").
- **Interactions:** B2, B3, B4 (all), A1 (fee_base None pinned), C5
  (FeeCarryAccount blocker), A4 (standing maker).

### B2. `fee-bounds-freeze`

- **Decision:** freeze the five bounds FEE_GEOMETRY demands *before*
  implementation — exact maximum coefficient, price scale, lot count, fee
  coefficient (kappa), intermediate width — noting the ordering is already
  violated: `dispersion_fee_step` is implemented in checked `u128` with none
  of the five frozen, so its domain is "whatever does not overflow", not an
  audited envelope.
- **Owner:** ember (after B1 picks the arm the bounds apply to).
- **Surfaces:** `docs/FEE_GEOMETRY.md:99-111`;
  `programs/solana-layout/src/portfolio_settlement.rs:388` (the
  implementation, orphaned inside its own module);
  `docs/reviews/FEE_ECONOMICS_FINDINGS_2026-08-19.md:88-91` (§4.6).
- **Options:** (1) freeze bounds for the selected base and re-verify the
  implementation against them; (2) declare checked-arithmetic-domain
  acceptable for V1 and record the claim change explicitly.
- **Blocked on it:** the audited-envelope claim; B1's promotion criteria.
- **Urgency:** rank 3 (strictly after B1).
- **Interactions:** B1.

### B3. `market-quality-axes-scope`

- **Status: DECIDED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) item 9.**
  The market-quality descope is RATIFIED (option 1): the four axes move from
  promotion-blocking to explicitly out of scope for V1, and FEE_GEOMETRY §6/§7
  now say so along with what the descope costs.
- **Decision:** declare the four unmeasurable market-quality axes (depth,
  participation, fill rate, route leakage) out of scope for V1 fee-base
  selection — so the base is chosen on arithmetic invariants and laundering
  resistance alone, with the document saying that is what happened — or fund
  a market-quality simulator that exists nowhere in the tree.
- **Owner:** ember.
- **Surfaces:** `docs/reviews/FEE_ECONOMICS_FINDINGS_2026-08-19.md:63-67,
  159-162`; `docs/FEE_GEOMETRY.md:208-231` (§6 measures);
  `docs/design/REVENUE_POLICY_V1.md:451-455` ("the market-quality axes stay
  declared out of V1 scope per findings §6" — the design already assumes the
  descope; ember has not ratified it).
- **Options:** (1) descope (recommended by findings; makes B1 startable);
  (2) build the simulator (order-flow generator, elasticity model,
  counterparty model — a research program, not a lane).
- **Blocked on it:** B1's gate closes as written only via (2); via (1) it
  closes redefined.
- **Urgency:** rank 2 (it is the gatekeeper on B1's startability).
- **Interactions:** B1.

### B4. `revenue-policy-v1-queue` (six sub-decisions)

**Status: DECIDED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) items 6 and
8.** All six sub-decisions returned, with two carve-outs inside them:

- **B4a — DECIDED (item 8):** custody requirements adopted; the **treasury
  pubkey is deferred to the first fee-bearing Realm** and stays reserved to
  ember.
- **B4b — DECIDED (item 8):** treasury Position (D6) adopted, with the
  mid-epoch-close grief rider joining the hostile walk.
- **B4c — DECIDED (item 6):** all five ResolutionWork charges are
  **permanently zero as frozen policy**. No vault is built. A V2 cost schedule
  may reintroduce charges for new Works — the weak form.
- **B4d — DECIDED (item 8):** sequencing per B4c.
- **B4e — DECIDED (item 8):** 60/0/40 + `AllRestingMakers` adopted as the V1
  vector; it constrains nothing until a fee-bearing Realm exists.
- **B4f — DECIDED (item 8):** both terminal rows accepted under item 7.

The RevenuePolicy V1 design (PROPOSED / DESIGN-ONLY,
`docs/design/REVENUE_POLICY_V1.md`, queue at `:447-485`) exists precisely to
be decided; its §11 names the smallest sufficient set. One entry each:

- **B4a. `revenue-treasury-key`** — choose the treasury key (custody), and
  accept that recipient rotation is representable only as a new Realm
  (D3/D4 immutability). Everything in §3-§5 hangs off this key.
  Surfaces: REVENUE_POLICY_V1.md:462-466. Owner: ember. Urgency: rank 2
  (first of the six; nothing else in the cluster can land without it).
- **B4b. `revenue-plane-c-shape`** — treasury Position (D6, recommended:
  zero new account families, conservation/terminal/withdrawal inherited)
  versus a standalone pot family; if D6, confirm the treasury authority
  will run the owner-signed Endow-path creation per Market.
  Surfaces: :193-241,467-469. Owner: ember.
- **B4c. `revenue-plane-l-disposition`** — L1 per-Realm RevenueVault
  (recommended) versus L0 burn, **and whether ResolutionWork charges should
  exist at all versus staying a permanent zero** (charging resolution may be
  anti-liveness — an economics call). The live seam is the five charge
  fields hardcoded zero: "Every protocol charge is zero because V1 has no
  authenticated fee sink"
  (`programs/clutch-sbf/program/src/instructions/resolution_work.rs:357`,
  fields :370-377, pins :796-812, refusals :997/:1282/:1489).
  Surfaces: REVENUE_POLICY_V1.md:140-192,470-473. Owner: ember.
- **B4d. `revenue-sequencing`** — Plane L (lamports) before Plane C
  (collateral atoms), D2 recommended: L needs no candidate ABI change and
  converts "no authenticated fee sink" from universal blocker into solved
  precedent. Surfaces: :66-72,474. Owner: ember.
- **B4e. `revenue-split-vector`** — 60/0/40 with executor deferred (D9) and
  the trivially-true `AllRestingMakers` standing-maker predicate, versus
  holding Plane C until the real standing-maker definition
  (OPEN_QUESTIONS P2) is decided. Envelope (≤15 executor / ≥25 treasury)
  becomes a structural `validate()` refusal.
  Surfaces: :294-321,475-478; `docs/ECONOMICS.md:141-168`. Owner: ember.
- **B4f. `revenue-terminal-rows`** — accept the two new Realm-lifetime
  terminal rows (policy record §3, vault §4) with TerminalIdentityV1
  headers, or demand a stricter bound before any implementation lane starts
  (`terminal_profile.py` gains both rows first).
  Surfaces: :111-139,154-178,479-481. Owner: ember. Interacts C1.

- **Blocked on the six together:** all of §9's five `max_fee_atoms` gates
  (`orders_batch.rs:880`, `orders_batch/settlement.rs:429`, `:596-600`,
  `direct_selection.rs:906-907`, `:1755`) stay closed; the §10 falsifier
  suite has nothing to bind to; every break-even and Sybil number in
  ECONOMICS.md stays prose.
- **Urgency:** rank 2 as a cluster; explicitly *before* B1 per the findings'
  dependency order ("decide the destination before the base").
- **Interactions:** B1 (base-agnostic within §8.3's requirements), C1
  (headers), A4/B4e (standing maker), F-cluster (real-money activation is a
  Track question this design does not touch).

---

## Cluster C — terminal / closure

### C1. `r4-terminal-ratification`

- **Status: DECIDED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) item 7.**
  RATIFIED: incinerator sink and the failure-payout decision stand as designed;
  **legacy-rows-permanent is ratified ONLY under the scope amendment** (legacy
  mints + prototype instances; the live general plane is explicitly NOT
  declared permanent). The permanent-rent rows stand PERMANENT_TOMBSTONE for V1
  with the shrink successor recorded. The §8 variant is deferred — see C3.
- **Decision:** ratify the R4 terminal-lifecycle runtime design
  (TerminalIdentityV1 header everywhere; **the frozen program-wide
  incinerator as the one neutral sink** with burn-only surplus disposal; the
  37-row classification dispositions incl. **the four legacy rows + legacy
  outcome mints declared PERMANENT_INFRA with no migration/reap ABI ever**;
  economic-close-strictly-before-rent-close; permissionless close order;
  MintCloseAuthority on new mints only) — or amend before any lane starts.
  The interim TerminalIdentityV1 research crate is itself "PROPOSED pending
  ratification".
- **Owner:** ember.
- **Surfaces:** `docs/design/TERMINAL_LIFECYCLE_RUNTIME_V1.md` (whole; sink
  :50-59, legacy-permanent :109-114, close order :145-168, mints :194-203);
  `GOAL.md:57-60` (queue item 3), `:731-737` (crate at eb1215a);
  `CURRENT_TRUTH.md:311-312` (matrix rows), `:423-437` (STOP 7);
  `research/terminal-identity-v1/` (the crate).
- **Options:** (1) ratify as proposed; (2) amend the sink choice (per-market
  `surplus_sink` was explicitly rejected — reversing it re-opens a sweep
  surface); (3) amend legacy-permanent (any reap authority invented now is
  a sweep right — the design's own argument); (4) defer, leaving all 37-row
  dispositions and the versioned-family lanes unstartable.
- **Evidence:** `research/terminal-lifecycle-v2` model, the 37-row machine
  inventory (`terminal_profile.py`/`terminal_admission.py`), ResolutionWork
  and Direct-V3 funding precedents, `research/fractional-redemption`'s
  impossibility result; falsifiers stated per choice in the design.
- **Blocked on it:** the five interim landable steps (§9: validator
  amendment, EXTERNAL_OWNER_STATE rows, header codec wiring, blocker
  conversion, per-family versioned layouts); retirement of the
  decision-owned terminal blocking ids (C6); B4f's rows adopt the header
  this design defines.
- **Urgency:** rank 2 (queue item 3; every terminal lane is parked on it).
- **Interactions:** C2, C3, C4, C6, B4f, E-cluster (§8 inputs).

### C2. `r4-fractional-arm-a`

- **Status: DECIDED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) item 7.**
  Fractional Arm A is RATIFIED (option 1).
- **Decision:** ratify Arm A ("live-until-aggregated"): one atom = one raw
  claim, redemption-boundary enforcement only, post-resolution exact lot
  `L(w_i) = D/gcd(D,w_i)` exposed, voluntary aggregation, **no credit
  account plane in V1**, accepting openly that an abandoned sub-lot fragment
  keeps its market non-retirable forever. Arm B (authenticated numerator
  credits with a separately capitalized remainder reserve) stays a versioned
  successor.
- **Owner:** ember.
- **Surfaces:** `docs/design/TERMINAL_LIFECYCLE_RUNTIME_V1.md:170-193` (§5);
  `CURRENT_TRUTH.md:385-387` (STOP 2: "Arm A live-until-aggregated is
  PROPOSED in the R4 design, pending ratification");
  `research/fractional-redemption/` (the impossibility model);
  blocking id `CLAIM.SUBLOT_FRAGMENT_NO_TOTAL_EXIT`
  (`terminal_profile.py:253-260` roster).
- **Options:** (1) Arm A as proposed — closes the *decision* while keeping
  the row honest ("policy selected: live-until-aggregated"); (2) Arm B now —
  requires the capitalized reserve and a new account plane, contradicting
  the R4 no-new-credit-plane frame; (3) V2-model resolution-time per-Position
  lot refusal — explicitly rejected in the design (transferred fragments
  make it unenforceable; refusing resolution punishes everyone).
- **Blocked on it:** STOP 2's "freeze the fragment/credit policy promised to
  bearers"; conversion of the sub-lot blocker; bearer-facing Terms language.
- **Urgency:** rank 2 (rides C1's ratification act).
- **Interactions:** C1, C6.

### C3. `r4-section8-reference-ownership`

- **Status: DEFERRED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) item 7.**
  The §8 reference-ownership variant is **EXPLICITLY DEFERRED** until the
  provider-horizon evidence exists — the weakest of its options. Neither A nor
  B is selected; the archive close routes and `SOURCE.NO_TERMINAL_RELEASE` stay
  blocked, by choice rather than by omission.
- **Decision:** pick the source/artifact reference-ownership variant the R4
  design deliberately leaves open: **A. maturity-horizon reap** (archive
  closes after a frozen horizon beyond its window's maturity bucket —
  requires R2 to freeze a maximum admitted market maturity) versus
  **B. per-archive reference counting** (exact, but a griefable shared
  counter and a new failure compartment).
- **Owner:** ember (with the R2 retention design owning half the inputs).
- **Surfaces:** `docs/design/TERMINAL_LIFECYCLE_RUNTIME_V1.md:213-229` (§8;
  "Variant A is recommended if R2 freezes a maximum market maturity");
  `docs/OPEN_QUESTIONS.md:64` (P1 archive retention row);
  `docs/design/SOURCE_PROVIDER_V1_SELECTION.md:185-187` (Hermes/Benchmarks
  retention horizon undocumented — must be measured and stated in Terms).
- **Options:** A (simple, couples archive lifetime to max maturity); B
  (exact, griefable); defer until R2's retention design exists.
- **Evidence:** falsifier for A stated in-design: one admitted market shape
  legitimately needing the archive after the horizon forces B.
- **Blocked on it:** archive close routes; retirement of
  `SOURCE.NO_TERMINAL_RELEASE`; R2 retention design completion.
- **Urgency:** rank 3.
- **Interactions:** C1, E1/E2, C6.

### C4. `failure-payout-ratification`

- **Status: DECIDED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) item 7.**
  The failure-payout decision is RATIFIED (option 1). Ratification closes the
  decision, not the runtime promotion falsifiers, which remain open in
  FAILURE_PAYOUT_DECISION_V1.
- **Decision:** ratify (or veto) the two already-recorded decisions taken by
  agent lanes on 2026-08-19: `EvidenceOnlyRecoveryV1` (no numeric
  data-failure payout; recoverable dormancy; residue to the incinerator) and
  lot-scaled bearer units (no persistent remainder credits; imported nonzero
  credit numerator is a terminal STOP). GOAL records these as
  "resolved-by-codex pending your ratification pass".
- **Owner:** ember.
- **Surfaces:** `docs/OPEN_QUESTIONS.md:8-26` ("Decided 2026-08-19" ×2);
  `docs/implementation/FAILURE_PAYOUT_DECISION_V1.md`;
  `research/failure-payout-v1/`; `GOAL.md:749-757`, `:858-866`;
  `docs/design/TERMINAL_LIFECYCLE_RUNTIME_V1.md:134-138` (§3(17) reserves
  the interface and leaves the rule to "the economics frontier lane with its
  own owner").
- **Options:** (1) ratify both; (2) re-open either (the equal-sum
  non-neutrality argument and the five-way impossibility argument are the
  things a veto must answer).
- **Blocked on it:** runtime promotion falsifiers in
  FAILURE_PAYOUT_DECISION_V1; Terms failure-consequence language (E-cluster
  "stall-then-lapse, never substitution").
- **Urgency:** rank 3.
- **Interactions:** C1, C2, E2.

### C5. `tier2-standing-blocker-order`

- **Decision:** authorize and order the next general-clearing engineering
  units against the standing executable blocker ledgers:
  **PartialFillLedger** (entitlement freeze refuses non-full fills),
  **VirtualPot** (refuses virtual split/merge and nonzero rounding pots),
  **TerminalClosure** (nothing reclaims ClearWork/feed/receipt/pot/window
  rent; consumed reservations persist as archive), **FeeCarryAccount**
  (fees forced zero — retires only via cluster B), and the recorded
  reservation-expiry/lapse-vs-frozen-epoch racing question.
- **Owner:** ember (prioritization + blessings; the units themselves are
  engineering).
- **Surfaces:**
  `programs/clutch-sbf/program/src/instructions/orders_batch/settlement.rs:703-772`
  (enum + retired-5/standing-3 ledgers);
  `programs/solana-layout/src/portfolio_settlement.rs:934-988`
  (discharged-6/standing-2 portfolio ledger);
  `docs/design/TIER2_PORTFOLIO_CLEARING_PLAN_2026-08-20.md:360-373` (§3
  ranked blockers deliberately standing, incl. the racing item and the V5
  K-pass alternative recorded-not-taken); `GOAL.md:100-105`.
- **Options:** ordering choices — (1) TerminalClosure first (it is also the
  walk-plane's rent story and C1's runtime half); (2) PartialFillLedger
  first (widest product effect: partial fills); (3) VirtualPot first
  (complete-set legs); (4) hold all until A1/D1 decide the plane's future.
- **Blocked on it:** the plane staying full-fill-only, virtual-leg-refusing,
  rent-stranding; TerminalClosure blocks 8 of the profile's STOP rows.
- **Urgency:** rank 3.
- **Interactions:** A1, C1, C6, B (FeeCarryAccount), D1.

### C6. `terminal-blocking-ids-retirement-map`

- **Decision:** for each of the **14 terminal blocking ids** (roster from
  `terminal_profile.py` via `build_terminal()`:
  `CLAIM.SUBLOT_FRAGMENT_NO_TOTAL_EXIT`, `DIRECT.ACCOUNT_REFUND_UNOWNED`,
  `DIRECT.CANDIDATE_RENT_PERSISTS`, `DIRECT.EMPTY_FROZEN_NO_LAPSE`,
  `DIRECT.EPOCH_RECEIPT_RENT_PERSISTS`,
  `DIRECT.POLICY_ARTIFACT_RENT_PERSISTS`,
  `DIRECT.V3_CLOSE_EVIDENCE_UNSEALED`,
  `HOARD.RESIDUAL_DISPOSITION_UNSELECTED`,
  `PROFILE.STORAGE_INVENTORY_INCOMPLETE`, `RENT.ACCOUNT_REFUND_UNOWNED`,
  `RENT.ARTIFACT_PREFUND_WINDFALL`, `SOURCE.DEFAULT_REGISTRY_EMPTY`,
  `SOURCE.NO_TERMINAL_RELEASE`, `TOKEN.OUTCOME_MINT_PERMANENT`), confirm the
  retirement route — each retires only by a named decision plus sealed
  evidence, never by prose. The decision-owned ones:
  `HOARD.RESIDUAL_DISPOSITION_UNSELECTED` (C1 §3), `CLAIM.SUBLOT…` (C2),
  `TOKEN.OUTCOME_MINT_PERMANENT` (C1 §6 — new mints only; legacy declared
  permanent), `RENT.ARTIFACT_PREFUND_WINDFALL` (C1 §1(11)),
  `SOURCE.*` (E2/C3). The evidence-owned ones
  (`DIRECT.V3_CLOSE_EVIDENCE_UNSEALED`, `DIRECT.EMPTY_FROZEN_NO_LAPSE`,
  rent-persist rows, `PROFILE.STORAGE_INVENTORY_INCOMPLETE`) need sealed
  bank measurements or close routes (C5, D2).
- **Owner:** ember for the decision-owned ids; ember-after-evidence for the
  rest.
- **Surfaces:** `research/liveness-policy-profile/terminal_profile.py`
  (ACCOUNT_ROWS + `blockers = sorted(...)` at :253-260);
  `terminal_admission.py` (the validator);
  `research/liveness-policy-profile/policy.py:487-488`;
  `docs/reviews/PLANNED_VS_BUILT_2026-08-19.md:92-98` (how the V3 families
  entered — since classified).
- **Blocked on it:** `terminal_status` leaving STOP;
  `claims_universal_no_stranded_value` stays hard-False permanently by
  design (`terminal_profile.py:302`) — that one is deliberately
  unretirable.
- **Urgency:** rank 3 (follows C1/C2/C5/E2 — this entry is the map, not a
  separate act).
- **Interactions:** C1, C2, C3, C5, E2, D2.

---

## Cluster D — promotion / admission

### D1. `walk-plane-admission-treatment`

- **Status: DECIDED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) item 10.**
  The walk plane **advances to rung W1** — CU/quote rows, **no live flags** —
  now that item 1 holds. This authorizes the rung; the rows are not yet
  derived, and W2 is not granted.
- **Decision:** decide the admission-policy treatment of the general
  clearing plane (tags 47–59): the entire Tier-2 evidence set is sealed as
  `UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY` with `decision_owner: ember`, and
  the derivation machine-refuses any walk family that loses that
  declaration. Promoting means deriving admission/quote/reward rows and
  flipping live flags; not promoting means the plane stays evidence.
- **Owner:** ember (the profile names ember as decision owner explicitly).
- **Surfaces:** `research/liveness-policy-profile/policy.py:366-385`
  (refusal), `:475-486` (`"decision_owner": "ember"`);
  `CURRENT_TRUTH.md:307` (matrix boundary cell); `GOAL.md:34-38` (queue),
  `:82-93` (cycle D seal: 15 bank suites / 77 tests sealed unpromoted).
- **Options:** (1) promote the measured families now (rows exist in sealed
  evidence; fees zero; A1/A2 must freeze first); (2) promote after a second
  bank profile / wider shape coverage (the V3 precedent: one bank profile
  was held as insufficient); (3) promote after devnet paces exercise the
  plane publicly; (4) keep evidence-only indefinitely (the plane remains a
  demonstration).
- **Evidence:** whole-plane conservation asserted on-bank (cash, per-outcome
  positions, release identity, byte-equal Positions); 57/57 profile tests;
  terminal rows standing (C5/C6).
- **Blocked on it:** any claim stronger than SBF-EXECUTED for portfolio
  clearing; liveness rows for tags 47–59; the "portfolio orders clear"
  headline leaving the evidence plane.
- **Urgency:** rank 2 (queue item; A1/A2 are its prerequisites).
- **Interactions:** A1, A2, C5, C6, D2, F1.

### D2. `v3-promotion`

- **Status: DECIDED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) item 10.**
  The **V3 syscall-era sealed measurement campaign is commissioned** (option 1's
  first half). V3 is **not** promoted on current evidence; `live_v3` stays
  false until the campaign's rows seal.
- **Decision:** promote Direct V3 in the liveness profile (`live_v3` is
  false: no measured CU, rent/refund/close, or terminal-admission rows; the
  Direct STOPs in the profile remain V2's), or leave it resident-unpromoted.
  Includes deciding what evidence suffices: the campaign covers one bank
  profile (five candidates, 11-tick grid); 64-tick, exact-tie, and
  reordered-retained-account behavior are model+host only; V3 close
  evidence exists in tests but is unsealed
  (`DIRECT.V3_CLOSE_EVIDENCE_UNSEALED`).
- **Owner:** ember-after-evidence (the missing rows are measurements; the
  standard for "enough profiles" is ember's).
- **Surfaces:** `research/liveness-policy-profile/policy.py:460` +
  `evidence.json:1056` (`live_v3: false`); `CURRENT_TRUTH.md:307`, `:404-417`
  (STOP 5); `GOAL.md:7-9` ("V3 atomic promotion" named in the goal),
  `:556-575` (merge record incl. the epoch-atomic no-per-order-cancellation
  design decision — already taken and recorded as a venue property).
- **Options:** (1) commission the measurement campaign (CU/rent/close rows,
  wider grids/ties) then promote; (2) promote on current evidence (against
  the profile's own precedent); (3) hold V3 unpromoted and let the general
  plane (D1) supersede it as the promotion target.
- **Blocked on it:** retiring V2's profile STOPs (`DIRECT.EMPTY_FROZEN_NO_LAPSE`
  stays V2's recorded blocker either way until a lapse lands); the "venue
  exists" claim at any plane above bank evidence.
- **Urgency:** rank 3.
- **Interactions:** D1 (competing promotion targets), C6, F1.

### D3. `v3-findings-bc-signoff`

- **Status: DECIDED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) item 10.**
  The B/C closures are **ratified as content**, with the process note recorded:
  an ember-pending finding was closed without ember, and the record says so.
- **Decision:** sign off (or refuse) the two V3 findings closed unilaterally
  by the codex lane while their status was ember-pending: finding B
  (verify_lease tautology) and finding C (FROZEN_EMPTY admission-field
  pinning), closed at 6267fde/081bd81 on `codex/r3-direct-v3`.
- **Owner:** ember.
- **Surfaces:** `GOAL.md:61-63` (queue item 4: "your sign-off on those two
  closures is still owed; review them on codex/r3-direct-v3"), `:793-796`
  (the lane-collision record).
- **Options:** (1) review the diffs and ratify; (2) refuse and re-open.
- **Blocked on it:** governance hygiene (an ember-pending finding closed
  without ember is a precedent worth ruling on either way).
- **Urgency:** rank 3.
- **Interactions:** D2.

### D4. `admission-shape-history` (context, decided)

- Not an open entry: the admission-shape quantum (10k CU rounding) and
  batched-fold routes were **blessed by ember 2026-08-19 ~23:00**
  (`GOAL.md:26-30`; `research/liveness-policy-profile/policy.py:223`,
  `admission_math.py:42-71`, `evidence.json:400-401`). Recorded so the
  fan-out does not re-litigate it.

---

## Cluster E — R2 / source provider

### E1. `r2-model-close-ratification`

- **Status: DECIDED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) item 3.**
  The R2 model close is ratified (option 1). Research-only; it authorizes
  nothing, and the standing double-witness falsifier survives ratification.
- **Decision:** ratify the R2 successor model close taken by the codex
  convergence wave — closing-boundary `CROSSING_V1` rule id 2 only,
  368-byte SourceSpecV2, exact ProgramData/config pins, zero grid origin,
  decoded-body duplicate collapse, start-aware contiguity, named overflow
  refusals. GOAL records it as "resolved-by-codex pending your ratification
  pass"; the selection carries a standing falsifier (one demonstrated
  double-witness boundary reopens the provider selection entirely).
- **Owner:** ember.
- **Surfaces:** `GOAL.md:49-53` (queue item 1), `:749-757`;
  `docs/design/SOURCE_PROVIDER_V1_SELECTION.md:96-157` (§4 semantics +
  resolved ambiguities), `:189-199` (falsifier summary);
  `docs/implementation/R2_PULL_PROMOTION_PLAN.md:14-40`.
- **Options:** (1) ratify the model close (research-only; authorizes
  nothing); (2) reopen any of the six frozen ambiguities (each has a stated
  falsifier).
- **Blocked on it:** cleanly, nothing runtime (it is research-only) — but E2
  and E3 build on it, and ratifying late means re-litigating under cutover
  pressure.
- **Urgency:** rank 2 (before Aug 26 makes it clean).
- **Interactions:** E2, E3, C3.

### E2. `r2-identity-freeze` — **dated: 2026-08-26**

- **Decision:** execute the deliberately-sequenced R2 production identity
  freeze *after* the Pyth DAO receiver cutover lands (2026-08-26 16:00 UTC,
  13-of-19 wormhole → 3-of-5 router quorum, in-place upgrade): freeze
  receiver program/ProgramData identity bytes, the config byte digest, the
  SDK release pin, and only then a registry entry. A pre-cutover identity
  freeze is forbidden in the design; the model "does not authorize an
  interim registry entry or value admission."
- **Owner:** ember (timing + the freeze act; the pins themselves are
  mechanical once cut).
- **Surfaces:** `docs/design/SOURCE_PROVIDER_V1_SELECTION.md:159-177` (§5);
  `GOAL.md:54-56` (queue item 2: "deliberately waits");
  `CURRENT_TRUTH.md:305` (matrix boundary: "post-cutover identities remain
  unfrozen"), `:391-397` (STOP 3);
  `docs/implementation/R2_PULL_PROMOTION_PLAN.md:110-133` (Phase 2).
- **Options:** (1) freeze promptly post-cutover (watch: SDK migration guide
  and manifest disagree on version 1.2.0 vs 2.0.0 — resolve which); (2)
  wait longer for post-cutover stability evidence; (3) reopen provider
  selection (only if the §4 falsifier fires).
- **Blocked on it:** the entire route to a registry entry; every Terms trust-
  floor statement ("3-of-5 router quorum plus pinned config generation");
  the R2 phase-1 checklist.
- **Urgency:** **date: Aug 26** (earliest possible moment; the decision is
  *when after*, not whether).
- **Interactions:** E1, E3, E5, C3.

### E3. `r2-registry-flip`

- **Decision:** ember's explicit go for compiling a production source
  release into the default ELF — "the protocol's first value-admission
  authority and … not covered by standing swarm authorization." This is the
  single decision that ends the empty-registry/`0x79` era; it forces a full
  reseal cycle (new ELF identity by construction) and narrows — never
  removes — the refusal boundary.
- **Owner:** ember (explicitly reserved to ember in every authorization
  statement: GOAL.md:11-13, CURRENT_TRUTH.md:143-145).
- **Surfaces:** `docs/implementation/R2_PULL_PROMOTION_PLAN.md:135-151`
  (§5 gates; ember's go at :149-151); `CURRENT_TRUTH.md:300` (Endow gate),
  `:391-397` (STOP 3); blocking id `SOURCE.DEFAULT_REGISTRY_EMPTY`.
- **Options:** (1) go, once every §5 gate is green post-E2; (2) hold — the
  default artifact remains structurally value-refusing (a legitimate
  long-term posture for Track A/B).
- **Blocked on it:** real value admission anywhere, ever; the blank-bank
  lifecycle without the mock-ELF split (injections drop from four toward
  one); the devnet story upgrading from refusal-boundary demonstration to
  funded lifecycle on a public cluster.
- **Urgency:** rank 2 (strictly after E2; the largest single go/no-go in the
  engineering plane).
- **Interactions:** E2, F1, F2, G-cluster (a value-admitting artifact
  changes the filings' factual posture), STOP 3/4.

### E4. `r2-runtime-capabilities-merge`

- **Decision:** merge the parked `fable/r2-runtime-capabilities` branch
  (f9045a0: Upgradeable-Loader ProgramData + Instructions-sysvar decoders,
  42 adversarial tests, wired into nothing) — needs a rebase and a reseal
  ride-along, since any closure-byte change forks the ELF identity.
- **Owner:** ember (merge+reseal scheduling authority; content is done).
- **Surfaces:** `GOAL.md:36-38` (queue), `:636-653` (the branch record incl.
  the revoked-authority stale-bytes finding); branch exists in-repo
  (`git branch`: `fable/r2-runtime-capabilities`).
- **Options:** (1) merge in the next reseal cycle (recommended shape: ride
  whichever cycle E2/E3 or the next engineering wave forces); (2) hold
  parked (costs nothing but bit-rot risk against `genesis.rs`/`seeds.rs`
  shared-edit churn).
- **Blocked on it:** R2 Phase-0.3's authenticator trait needs these
  decoders; every day parked adds rebase distance.
- **Urgency:** rank 3.
- **Interactions:** E2, E5, the seal protocol (F-cluster).

### E5. `r2-legal-tos-lane`

- **Decision:** commission the in-house legal-analysis lane the R2 design
  routes around engineering: Pyth ToS prohibits bulk automated extraction
  and is silent on on-chain protocol usage; post-cutover historical payloads
  need a billed API key whose secret cannot live in a static frontend.
- **Owner:** ember+counsel.
- **Surfaces:** `docs/design/SOURCE_PROVIDER_V1_SELECTION.md:179-187` (§6).
- **Options:** (1) run the analysis before E3 (conservative); (2) run it
  before any public frontend references Hermes/Benchmarks; (3) accept the
  risk for devnet-only usage and defer.
- **Blocked on it:** Terms language for late recovery; any client-side
  historical-recovery feature.
- **Urgency:** rank 3.
- **Interactions:** E2, E3, F6, G-cluster.

### E6. `p1-source-backlog`

- **Decision:** the OPEN_QUESTIONS P1 rows still open after E1/E2: exact
  initial source presets for the house DREGG Realm; whether DREGG/PumpSwap
  has any acceptable cumulative native history source; Raydium/Meteora
  adapter versions; security tiers and per-feed exposure limits;
  multi-source aggregation; the supported monoidal feature family; archive
  page size/retention (shared with C3); reverse-Dutch bounty step count;
  whether any historical provider dependency is acceptable for repair.
- **Owner:** ember-after-evidence (each row wants its own dossier like the
  Pyth one).
- **Surfaces:** `docs/OPEN_QUESTIONS.md:55-67` (P1).
- **Blocked on it:** second providers, DREGG-native market Templates,
  multi-source designs — all explicitly out of the R2 V1 scope already
  (`R2_PULL_PROMOTION_PLAN.md` §6).
- **Urgency:** rank 5.
- **Interactions:** E1, E2, C3.

---

## Cluster F — deployment / assurance

### F1. `opt-z-deploy-economics`

- **Status: DECIDED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) item 5.**
  Deployments use the **sealed opt-3 identity only**; opt-z is **refused**
  until re-greened and gate-campaigned at its own identity (option 1, with
  option 3 left available on that condition). The devnet beta authority is
  ratified as recorded in the deploy job.
- **Decision:** deploy the devnet ELF at the default `opt-level=3` identity
  (1,785,904 B at the walk-era seal, ~13.3 SOL deploy rent) or the opt-z
  option (~−23%, ~2.3 SOL saved; suite FULLY GREEN since the Tier-0 frame
  work) at the cost of +60–220% CU on some rows — "a per-deployment
  economics choice, not a default." Also: whether devnet should carry the
  sealed identity for evidence-continuity even at higher rent.
- **Owner:** ember.
- **Surfaces:** `docs/design/FRAME_BUDGET_PLAN_2026-08-19.md:51-72` (§3
  inversion + Tier-0 payoff sentence); `GOAL.md:43-45` (next-3 item 3),
  `:249-250` (opt-z suite green at 1,092,928 B), `:446-448` (the measured
  negative that predated Tier 0).
- **Options:** (1) opt-3 sealed identity (evidence continuity; costs SOL);
  (2) opt-z (cheaper; CU regression measured post-SHA as affordable; a
  *different* identity — devnet evidence stops being about the sealed
  bytes); (3) opt-3 now, opt-z as a later devnet-2 comparison deployment.
- **Blocked on it:** the devnet deployment fires "the moment the deployer is
  funded" — the collector is polling; this is the one open choice in that
  path (deployment itself is already authorized as Track C).
- **Urgency:** rank 2 (faucet-gated, could become same-day actionable).
- **Interactions:** D1/D2 (what devnet exercises), E3, F4.

### F2. `upgrade-posture`

- **Status: DEFERRED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md),
  "Deferred with the tension named".** The report recommended
  immutable-at-first-deployment; ember's weakest-choice principle favors
  upgradeable-then-burn (burn is always available; un-burn never is). Deferred
  on that tension: mainnet is gated regardless, and the devnet posture is
  settled by item 5.
- **Decision:** the P0 row no queue currently carries: does the reference
  deployment have a time-bounded audited beta upgrade authority followed by
  irrevocable removal, or is it immutable at first deployment? Source must
  support either without pretending one is the other.
- **Owner:** ember (ember+counsel once real money is in scope).
- **Surfaces:** `docs/OPEN_QUESTIONS.md:28-32`;
  `docs/DEPLOYMENT_REVENUE_BOUNDARY.md:99-110` (Tracks B/C assume exact
  build identity); REVENUE_POLICY_V1.md:174-175 (recipient rotation
  "representable only as a program upgrade, i.e. not representable in this
  immutable deployment" — the design already leans immutable).
- **Options:** (1) immutable-at-first-deployment (matches the honesty
  apparatus; makes every policy const truly frozen); (2) time-bounded beta
  authority with audited removal (eases early defect response; complicates
  every immutability claim in filings and Terms); (3) devnet-only decision
  now (devnet can be upgradeable while mainnet posture stays open).
- **Blocked on it:** release-manifest fields (deployer/upgrade authority are
  named manifest content, DEPLOYMENT_REVENUE_BOUNDARY.md:126-128); the
  verified-build UX row (OPEN_QUESTIONS P3); mainnet track design.
- **Urgency:** rank 3 (before any Track-B kit or Track-D motion).
- **Interactions:** F6, B4a (rotation story), G-cluster (filing language).

### F3. `release-assurance-human-items`

- **Decision:** the STOP-8 items that are human review, not engineering:
  (a) review the flagged dependency-license rows (MPL-2.0 family, CDLA
  roots, one license-file-only crate); (b) authorize/commission an external
  security review; (c) sign a release tag; (d) procure the second macOS
  host for byte-level seal reproduction (a purchase/ops decision).
- **Owner:** ember.
- **Surfaces:** `CURRENT_TRUTH.md:438-451` (STOP 8);
  `research/liveness-policy-profile/dependency_license_complete.tsv` +
  `scripts/dependency_license_check.py`; `GOAL.md:739-747` (SBOM closure
  landed, "notable-but-green license rows flagged for release eyes").
- **Options:** per-item accept/act; none has alternatives so much as a
  yes/when.
- **Blocked on it:** any release claim; the manifest's remaining gap list.
- **Urgency:** rank 4 (release-gated, not date-gated).
- **Interactions:** F2, F6.

### F4. `vendored-syscall-signoff`

- **Decision:** the carried vendored-crate sign-off: accept
  `solana-define-syscall 5.1.0` (verified verbatim, checksum-matched) or
  drop it once the registry archive is reachable; if kept, add the
  Apache-2.0 license text beside the verbatim tree.
- **Owner:** ember.
- **Surfaces:** `docs/implementation/DRIFT_REVIEW_2026-08-19B.md:423-425`
  (carried item 5), `DRIFT_REVIEW_2026-08-19.md:321-324`; `GOAL.md:1230`
  (historic queue item 4).
- **Blocked on it:** a fully clean license posture for F3(a).
- **Urgency:** rank 5.
- **Interactions:** F3.

### F5. `token2022-pinned-elf`

- **Decision:** TOKEN2022_PLAN open decision 7, explicitly unresolved:
  select and pin the exact Token-2022 program artifact (the probe drove
  10.0.0 via solana-program-binaries, litesvm ships 11.0.0, clusters run
  what they run — "a program id is not a pin"). Riding along: formally
  ratify the decisions already taken in-direction (checked-mirror
  `collateral_atoms` #3, ImmutableOwner-required #4 — stricter than the
  collateral matrix, named as a divergence — decimals-0/no-freeze #5,
  no-ATA #6).
- **Owner:** ember.
- **Surfaces:** `docs/implementation/TOKEN2022_PLAN.md:716-750` (§5);
  `docs/implementation/DRIFT_REVIEW_2026-08-19B.md:407-411` (G.3).
- **Options:** (1) pin the cluster-deployed artifact hash per target cluster
  and record per-cluster identity; (2) pin the probe's 10.0.0 and accept
  cluster drift as a recorded assumption; (3) leave unpinned for devnet,
  pin at mainnet gate.
- **Blocked on it:** obligation 5 of the token plan; A8's allowlist freeze
  wants the same act.
- **Urgency:** rank 4.
- **Interactions:** A8, F6.

### F6. `mainnet-and-l0`

- **Decision:** the standing human gates that no engineering closes:
  mainnet, real value, market creation for real users, official-claim
  language, and Gate L0 (exact legal/entity/control/deployment facts,
  qualified advice, any required relief, separate current user
  authorization). Track D (author-affiliated real-money) and Track E (JOSHI
  principal trading) each carry their own blocked-until lists.
- **Owner:** ember+counsel.
- **Surfaces:** `CURRENT_TRUTH.md:136-148` (authorization scope), `:452-454`
  (STOP 9); `GOAL.md:10-14`, `:26-30` (human gates restated in both
  mandates); `docs/DEPLOYMENT_REVENUE_BOUNDARY.md:46-50` (counsel
  prerequisite), `:91-121` (Tracks A–E);
  `docs/regulatory/AUTHORITY_MATRIX.md`.
- **Options:** not an options decision today — an ordered set of
  prerequisites (written counsel analysis, audits, incident/disclosure plan,
  conflict policy, surveillance design, capitalization, exact revenue
  policy, separate authorization) whose commissioning order is ember's.
- **Blocked on it:** everything Track D/E; the "official" instance question
  (OPEN_QUESTIONS P3 "publishes only code or also a reference devnet
  deployment").
- **Urgency:** rank 4 for commissioning; the gates themselves are indefinite.
- **Interactions:** E3, F2, F3, B4, G-cluster.

### F7. `p3-release-backlog`

- **Decision:** OPEN_QUESTIONS P3 rows not covered above: bare vs immutable
  in-mint metadata; static-client framework + wallet adapter; IPFS pinning
  diversity and canonical release-manifest location; program upgrade and
  verified-build UX; publish-code-only vs reference devnet deployment.
- **Owner:** ember.
- **Surfaces:** `docs/OPEN_QUESTIONS.md:89-99`.
- **Urgency:** rank 5.
- **Interactions:** F2, F3, F6.

### F8. `ci-adoption`

- **Decision:** whether to add CI at all — `.github/` does not exist; there
  is no automated regression defense; the manifest gate system is the
  de-facto CI and runs only when invoked. Named by the scorecard, queued
  nowhere.
- **Owner:** ember (repo-governance and infra-spend call; also interacts
  with the private-repo posture).
- **Surfaces:** `docs/reviews/PLANNED_VS_BUILT_2026-08-19.md:125-130`.
- **Options:** (1) GitHub Actions running the fast manifest check + host
  tests; (2) a self-hosted runner (hbox) for SBF gates; (3) explicit
  decision to stay CI-less with the manifest protocol as the recorded
  substitute.
- **Blocked on it:** nothing structurally; every merge relies on lane
  discipline instead.
- **Urgency:** rank 4.
- **Interactions:** F3 (release confidence), the seal protocol.

---

## Cluster G — filings-adjacent (cross-repo; artifacts live in degg-research, decisions are ember's)

Per scope rules these are noted, not fully analyzed here; GOAL.md is the
in-tree surface for each.

### G1. `filing-freeze-aug24` — **dated: Aug 24**

- Send John packet ROUND 1 (degg-research 55ce13a); signature block +
  dual-route answers needed **before Aug 24**; the two Aug-24 filings freeze
  and submit; filing-day gates (evidence-section re-pin at a frozen commit,
  ledger gate-4 PDF hash re-pin, docket re-checks). Surfaces:
  `GOAL.md:64-66`; `docs/implementation/DRIFT_REVIEW_2026-08-19B.md:396-405`.
  Owner: ember (human-only per GOAL). Interactions: G2, G3, D2 (the
  freeze-sensitive V3 sentences update at filing freeze, GOAL.md:667-673).

### G2. `perpetuals-rfc-aug26` — **dated: Aug 26**

- Go/no-go on the 24/7-perpetuals RFC filing (Draft 1 exists; corrected by
  its own manipulation-cost experiment). Surfaces: `GOAL.md:66`,
  `:1107-1110`, `:1216-1218`. Owner: ember.

### G3. `iac-statement-aug27` — **dated: Aug 27 ("should submit by")**

- File the IAC written statement (meeting was Aug 20, listen-only; the Aug 27
  date is a soft deadline, FR close date null; statements land unreviewed on
  the docket). Content calls riding along: the packet-length editorial
  decisions (Draft 12/13 both flag that further cuts delete claims — ember's
  editorial call). Surfaces: `GOAL.md:355-364`, `:378-383`, `:419-424`.
  Owner: ember.

### G4. `compute-derivatives-rfc` — **dated: ~Oct 19 (60 days from 2026-08-19 FR issuance; confirm exact FR date)**

- Go/no-go on responding to the Compute Derivatives RFC (RIN 3038-AF77) —
  "asking about unobservable/unsurveillable reference prices and perpetual
  compute futures — squarely our material." Surfaces: `GOAL.md:385-389`.
  Owner: ember.

### G5. `conflicts-nprm-oct5` — **dated: Oct 5**

- Go/no-go, deliberately unhurried ("decide in September"). Surfaces:
  `GOAL.md:1223`; `DRIFT_REVIEW_2026-08-19B.md:402-403`. Owner: ember.

### G6. `bedrock-mpc-tls-session` (carried, possibly stale)

- One paid Bedrock MPC-TLS session to produce the first provider-attested
  D-grade transcript (attestation-survey recommendation; carried through two
  drift reviews and the historic morning queue, never actioned or retired).
  Surfaces: `GOAL.md:1203-1204`, `:1229`;
  `DRIFT_REVIEW_2026-08-19B.md:421-422`. Owner: ember. Decide or retire.

---

## Cluster H — formal methods / governance records

### H1. `adr-0003-supersession`

- **Status: DECIDED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) item 2.**
  Option 1: **ADR-0005 — Lean is the proof substrate of record** (adopted text
  at `docs/adr/0005-lean-proof-substrate-of-record.md`). Verus retained for
  checked-executable-body results; the Rocq role retired with `rocq/` kept as a
  historical specification; the native_decide ban codified. FEE_GEOMETRY §7 is
  rewritten; the rest of the report's §3 cleanup inventory is still owed.
- **Decision:** the standing formal-methods substrate decision: ADR-0003
  designated Verus the executable-kernel gate and Rocq the independent
  shadow, and warned against Lean "becoming mandatory by inertia." Reality
  is inverted with no superseding record: Rocq has zero theorems (one
  machine-checked vacuous conjunct), Verus covers ~1.5 of 11 named
  properties, Lean carries 184+ theorems with zero sorry. Author the
  superseding ADR (ratify Lean-primary), or re-invest in the ADR's
  architecture, or a recorded hybrid.
- **Owner:** ember.
- **Surfaces:** `docs/adr/0003-verus-first-shadow-models.md` (the record);
  `docs/reviews/PLANNED_VS_BUILT_2026-08-19.md:69-78` (the inversion,
  ranked #1 among quietly-superseded); `CURRENT_TRUTH.md:293-295` (matrix
  Verus/Lean rows); `docs/FEE_GEOMETRY.md:229-230` ("Nothing is closed in
  Verus or Rocq; Rocq currently contains zero theorems" — a promotion
  criterion still written against the dead architecture).
- **Options:** (1) ADR-0005: Lean is the proof substrate of record; Verus
  stays for the narrow checked-Rust-subset wins; Rocq row retired — then
  FIX the downstream promotion criteria (FEE_GEOMETRY §7 currently demands
  Verus+Rocq closure that will never come); (2) re-staff Verus/Rocq to the
  ADR (no evidence anyone wants this); (3) hybrid with named per-layer
  substrates.
- **Blocked on it:** every promotion criterion written as "Verus and Rocq
  close X" is currently unsatisfiable-as-written (B1's §7 list); governance
  truthfulness of the ADR directory.
- **Urgency:** rank 3.
- **Interactions:** B1 (its promotion list), VERIFICATION.md's property
  table, the house AIR-in-Lean rule (consistent with option 1).

### H2. `verus-probe-and-vector-spine-carried-items` (carried, possibly stale)

- Small carried rulings never closed: (a) E0/Verus probe posture — re-author
  a reviewed probe (new digest) or keep the recorded failure (E1 NO-GO
  either way); (b) vector-spine G1/G2 re-scope to the twelve error enums —
  "G1–G7 are human decisions and none of them is made by this drop";
  (c) VM-INT trace naming (accept `golden/coupled.trace` or rename to
  `relation_v1.trace`). Surfaces:
  `docs/implementation/DRIFT_REVIEW_2026-08-19B.md:436-444`; `GOAL.md:1126-1128`.
  Owner: ember. Urgency: rank 5. Decide or retire each explicitly.

### H3. `native-decide-rule` (context, effectively closed)

- **Status: DECIDED — [ADOPTED_2026-08-20.md](ADOPTED_2026-08-20.md) item 2.**
  The native_decide ban is **codified** — written into ADR-0005 as a rule of
  the record rather than an audit convention.

- The historic "house rule to add: ban native_decide" (`GOAL.md:995-996`) is
  effectively enforced: the Lean audit greps for
  `sorry|axiom|native_decide|unsafe|implemented_by` (`lean/README.md:73`)
  and CURRENT_TRUTH.md:186-190 attests the clean result. If ember wants it
  as a written rule rather than an audit convention, that is a one-line
  AGENTS.md edit — noted, not queued.

### H4. `cross-market-netting` (context, decided-out)

- The historic queue's "PROJECT.md section 9 vs cross-market netting"
  (`GOAL.md:998`) is decided-out for V1: cross-market collateral netting is
  listed under "Explicit future research, not V1 dependencies"
  (`docs/OPEN_QUESTIONS.md:104`). Recorded so it stops resurfacing.

---

## Proposed ANALYSIS FAN-OUT

A standalone decision report is deserved where options genuinely compete and
evidence needs assembling; the rest are adequately served by their register
paragraph above.

**Standalone reports (10):**

1. **`fee-base-selection`** (B1, folding B2 bounds + B3 axes-descope + the
   deferred rate) — four-plus arms, a proved shared evasion channel, an
   unstartable-as-written gate to redefine, lab outputs to tabulate.
2. **`revenue-policy-v1`** (B4a–f as one report) — six coupled decisions
   with recommended arms, rejected alternatives already named, and a
   falsifier suite to sequence.
3. **`r4-terminal-ratification`** (C1 + C2 + C3, with C6's retirement map as
   an appendix) — sink/legacy/fractional/reference-ownership forks each
   carry live falsifiers and precedents to weigh.
4. **`general-clearing-policy-freeze`** (A1 + A2) — selector-by-selector
   defense incl. the dust liveness hazard, the R-b exercising-test check,
   and the window-pin wire question.
5. **`clearing-plane-promotion`** (D1 + D2 together) — what evidence
   standard promotes a plane; the V3-vs-general-plane competition for
   promotion target; what devnet adds.
6. **`r2-cutover-and-registry-flip`** (E2 + E3, with E1's ratification and
   E5's ToS question as inputs) — the calendar-pinned one; freeze-what-when
   table against the Aug-26 cutover, flip-gate checklist, SDK version
   discrepancy.
7. **`opt-z-deploy-economics`** (F1) — measured CU deltas vs rent savings vs
   evidence-continuity; concrete SOL numbers exist in-tree.
8. **`upgrade-posture`** (F2) — immutable vs bounded-beta, with consequences
   traced through revenue rotation, filings language, and the release
   manifest.
9. **`realm-admission-allowlist`** (A8 + F5) — the collateral matrix, the
   adapter's recorded divergence, and the Token-2022 pin belong in one
   report.
10. **`adr-0003-supersession`** (H1) — small report: the inventory of every
    downstream criterion written against the dead architecture, plus the
    draft superseding ADR.

**Paragraph-sufficient (remain as register entries):** A3, A4, A5, A6, A7,
C4 (ratify-or-veto of recorded decisions), C5 (ordering call), D3 (a diff
review), E4 (scheduling), E6, F3, F4, F7, F8, F6 (its analysis is counsel
work, not a report), all of G (calendar-driven ops; the analysis lives in
degg-research), H2, H3, H4.

---

*Register compiled 2026-08-20 in a read-only sweep; one file, no other tree
changes. Corrections to this register belong in a dated successor or an
edit that preserves entry ids.*
