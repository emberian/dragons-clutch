# Concept inventory — the Dragon's Clutch conceptual canon (.spw seed)

Status: **DISTILLATION SEED (2026-08-20).** Forty concepts, each defined in
the project's own vocabulary with its authoritative in-tree source, its
relations to the other concepts, and the microsite page that teaches it
(page ids per [`SITE_CONTENT_MAP_2026-08-20.md`](SITE_CONTENT_MAP_2026-08-20.md):
P1 The Clutch · P2 The Shape of a Claim · P3 The Clearing · P4 The Ledger ·
P5 The Price of Risk · P6 For the Machines · P7 The Evidence). Precision over
coverage: satellite terms ride as "adjacent" rather than earning entries.
Relation verbs: **uses** (builds on), **refines** (is a sharper form of),
**refuses** (is defined by what it rejects), **measures** (quantifies).

Definitions state the concept; they do not promote it. Evidence status for
any concept is whatever its source's claim plane says (entry 33).

---

## I. The claim algebra

### 1. Egg (claim)
One native basis claim — categorical at degree zero, an open-clamped
B-spline basis function at degrees one through three — that redeems
according to its exact integer weight at the resolved value.
**source** PROJECT.md §1–2 · **relations** uses partition-of-unity (4),
degree (3); composes into Clutch (2) and coefficient vectors (5); measured
by sup-norm collateral (12) · **page** P1, P2.

### 2. Clutch (complete set)
One unit of every Egg of a market — the constant payoff, worth exactly one
collateral unit at every admissible resolved value, mintable and meltable at
par. **source** PROJECT.md §2; RISK_SUMMED_POSITIONS.md §1.1 (unitality)
· **relations** uses Egg (1), partition of unity (4); is the kernel of
dispersion (30) and the free direction of diagonal motion (8); anchors
conservation (11) · **page** P1.

### 3. degree (the ladder)
The precision order of a market's basis: 0 is an exhaustive disjoint ordered
partition; 1–3 are native smooth B-spline semantics that must never be
silently lowered to one-hot bins. **source** PROJECT.md §1; CURRENT_TRUTH.md
§3 · **relations** refines binary markets; refuses compatibility lowering
(6) as a definition; sets where implied-measure-hood (26) holds (≤1) and
breaks (≥2) · **page** P2 · **adjacent** knot grid, open-clamped, span.

### 4. partition of unity
The frozen basis property H1/H2 — every weight nonnegative and bounded, all
weights summing exactly to the denominator at every admissible value — from
which solvency, complete-set par value, and price normalization all descend.
**source** RISK_SUMMED_POSITIONS.md §1.1; lean/DragonsClutch/BSpline.lean
· **relations** used by Egg (1), Clutch (2), simplex prices (25);
machine-checked in the Lean model (P7) · **page** P2.

### 5. coefficient vector (portfolio)
A payoff shape written exactly as integer coefficients over the native
basis — one atomic asset, one order, one fill; a vector is not
automatically an approximation. **source** PROJECT.md §1;
docs/design/TIER2_PORTFOLIO_CLEARING_PLAN_2026-08-20.md · **relations**
uses Egg (1); carried by the book (14) as `OrderSlot::Portfolio`; valued at
`dot(c, p)` against simplex prices (25); charged by dispersion (30)
· **page** P2, P3.

### 6. compatibility lowering
The explicitly-labeled adapter that samples a shaped payoff onto degree-zero
bins — always disclosed, always carrying an error statement when inexact,
never a redefinition of the smooth product. **source** CURRENT_TRUTH.md §3
item 3; PROJECT.md §1 · **relations** refuses silent substitution for
degree (3); guarded by refusal (32) · **page** P2.

### 7. largest remainder
The single deterministic rounding rule — canonical largest-remainder
quantization with lowest-index ties — applied once at each named boundary so
exactness is preserved or the discrepancy is named. **source** PROJECT.md
§6; CURRENT_TRUTH.md §3; lean/DragonsClutch/BSpline.lean (constructive
uniqueness) · **relations** used by Hatch (35) and fee close; proved
canonical in the Lean model · **page** P4.

## II. Custody and conservation

### 8. diagonal motion (split / merge)
The counterparty-free value-preserving moves: split debits cash and credits
every Egg equally, merge is its exact inverse — provably the *only* such
moves besides representation changes. **source** CURRENT_TRUTH.md §5 table;
RISK_SUMMED_POSITIONS.md Prop 4 · **relations** uses Clutch (2); free under
every admissible fee base; identity on the risk quotient (13) · **page**
P1, P4.

### 9. materialize / dematerialize
The hybrid-representation boundary: debit an internal Egg balance to mint
its canonical Token-2022 asset, or burn the asset to restore the balance —
paying the composability cost only when a user asks for it. **source**
PROJECT.md §5 · **relations** identity on positions and on conservation
(11); connects Eggs (1) to external venues · **page** P1 · **adjacent**
bearer, positionless redemption.

### 10. custody boundary (Endow / WithdrawCash)
The only two transitions where collateral tokens actually cross the Hoard's
edge: Endow is the sole inbound boundary, exact unreserved WithdrawCash the
outbound one; everything between is pooled-accounting reclassification.
**source** CURRENT_TRUTH.md §4 (pooled custody row), §5 · **relations**
uses Hoard (22); Endow currently refuses `0x79` on the default artifact
(see 37) · **page** P4.

### 11. conservation
The custody identity `H = L + P + S` — actual Hoard atoms equal retained
claim backing plus all Position cash plus unsolicited surplus — plus its
per-transition exact-delta table, asserted to the atom at settlement.
**source** CURRENT_TRUTH.md §5; GOAL.md done-log T2-8 (whole-plane
assertion) · **relations** measures every transition; anchored by Clutch
(2); the settled end-state of the walk-to-zero (P4) · **page** P4.

### 12. sup-norm collateral (solvency)
The required-collateral rule: reserve the maximum component of the position
vector — exactly the supremum of realizable payoff at degrees ≤ 1 — so
every claim is prepaid at worst case and "margin call" has no referent.
**source** RISK_SUMMED_POSITIONS.md Props 1–2, §2.4; lean P_SOLV_01 family
(lean/DragonsClutch/Solvency.lean, Transitions.lean) · **relations**
measures coefficient vectors (5); refuses leverage and liquidation;
monotone under partial resolution (Prop 8) · **page** P5.

### 13. risk quotient
Where risk lives: the position space modulo the diagonal of complete sets,
with quotient norm half the range `R(T)/2` — total risk across holders is
identically zero at every Active state, moved only pairwise at trades.
**source** RISK_SUMMED_POSITIONS.md §1.3–1.4 · **relations** refines
"position"; the domain any principled fee base must factor through (30,
31); collapsed to a point by Hatch (35) · **page** P5.

## III. The book and the clearing

### 14. book
The frozen order set of an epoch: multi-page order accounts holding single
and portfolio slots, sealed under a digest commitment at freeze so the
clearing's input is immutable and re-derivable. **source**
TIER2_PORTFOLIO_CLEARING_PLAN_2026-08-20.md T2-2/T2-6a;
programs/solana-layout (SOLANA_LAYOUT.md) · **relations** uses reservation
(18), tombstone (15); projected into the relation (20) by live rank (16)
· **page** P3.

### 15. tombstone
The retirement marker a cancelled order leaves in its page slot —
digest-covered, skipped by the projection, consuming no live rank — so
cancellation is a recorded fact, not an erasure. **source**
TIER2_PORTFOLIO_CLEARING_PLAN_2026-08-20.md T2-2/T2-4 · **relations** used
by book (14); a tombstone-bearing set must verdict-match its tombstone-free
equivalent (T2-2 gate) · **page** P3.

### 16. live rank
The canonical zero-based renumbering of surviving orders after tombstones
are skipped — the identity by which fills, slices, and candidates refer to
orders. **source** TIER2_PORTFOLIO_CLEARING_PLAN_2026-08-20.md T2-2 (index
vocabularies pinned) · **relations** uses tombstone (15); binds candidate
(19) fill indices to the book (14) · **page** P3.

### 17. owner tag
The first-appearance interned index (bounded at 64) standing for an owner
throughout one clearing, refused unless the interned count equals the
epoch's owner count at pass-1 end. **source**
TIER2_PORTFOLIO_CLEARING_PLAN_2026-08-20.md T2-6b (OwnerInterner)
· **relations** used by the walk (21); supports self-cross refusal and the
owner-congestion duals (DUAL_IS_THE_MEASURE §5.6) · **page** P3.

### 18. reservation
The canonical pre-fund-safe per-order encumbrance of exact cash or internal
Eggs, created at placement, re-verified order-by-order during pass 1, and
released only per the verified summary. **source** CURRENT_TRUTH.md §4
(funded order admission); TIER2 plan T2-6b join 1 · **relations** uses
custody (10); the walk (21) is its sweep; consumed by entitlement (23)
· **page** P3, P4.

### 19. candidate
A solver's complete proposed clearing — price vector, imbalance, fills,
slices — submitted permissionlessly and trusted for nothing: every derived
quantity is recomputed and checked for exact equality. **source**
DUAL_IS_THE_MEASURE.md §1; CURRENT_TRUTH.md §4 clearing row · **relations**
verified by the relation (20) via the walk (21); competes under selection
(24); its price vector is the dual (27) and the measure (26) · **page**
P3, P6.

### 20. relation
The frozen batch-clearing predicate (stages V0–V9): simplex gate,
eligibility trichotomy, fill box, per-outcome conservation, pairing,
surplus, cash closure, score — a disassembled optimality-certificate
checker derived as refusal discipline. **source**
crates/clutch-batch/src/relation_v1.rs; DUAL_IS_THE_MEASURE.md §4.4
· **relations** consumes book (14) and candidate (19); emits verdict (22);
streamed by the walk (21) · **page** P3.

### 21. the walk
Streaming verification: the relation executed across many transactions over
digest-verified pages — push order by order, slice by slice, checkpointed
between steps — so verification cost is bounded per transaction, not per
book. **source** crates/clutch-batch/src/relation_v1_stream.rs; TIER2 plan
T2-6; GOAL.md done-log T2-6 · **relations** uses checkpoint (28), live rank
(16), owner tag (17); its on-chain verdict is byte-equal to the host
relation's in bank evidence · **page** P3.

### 22. verdict
The relation's total answer for one candidate — VERIFIED with its summary
and recomputed score components, or a typed refusal — never a partial
opinion, never trusted from the claimant. **source** relation_v1_stream.rs
(verdict/`FullScoreV1`); CURRENT_TRUTH.md §4 · **relations** produced by
relation (20); gates selection (24); the summary's implied allocation is
what settlement must byte-match (11) · **page** P3.

### 23. entitlement
The immutable per-slice receipt frozen from the selected candidate before
any value moves — the consumption latch that makes settlement resumable and
exactly-once. **source** TIER2 plan T2-8; GOAL.md done-log T2-8
· **relations** uses selection (24); consumed by settlement under
conservation (11); each receipt consumed exactly once · **page** P4.

### 24. selection
Choosing the best valid *submitted* candidate: only VERIFIED candidates
compete, compared by a frozen total order over re-derived full-width score
components and tie digests — a deterministic policy choice on the optimal
face, and honestly described as such. **source** GOAL.md done-log T2-7;
DUAL_IS_THE_MEASURE.md §5.3, §9.3 · **relations** uses verdict (22);
freezes entitlement (23); an unverified claim can never displace a verified
candidate · **page** P3 · **adjacent** score, tie digest, candidate window.

### 25. simplex prices
The published price vector: nonnegative scaled integers summing exactly to
`PRICE_SCALE`, a normalization *forced* by the mere availability of
split/merge (dual feasibility of the all-ones columns), not asserted by
fiat. **source** DUAL_IS_THE_MEASURE.md Thm 3.1 · **relations** uses
partition of unity (4); is the dual (27) on the accept set; is the measure
(26) at degree ≤ 1 · **page** P3, P6.

### 26. implied measure
At degree ≤ 1, every publishable price vector is exactly a probability
measure's basis-moment vector — a positive normalized quadrature rule, with
Breeden–Litzenberger pre-inverted because hat claims *are* butterflies;
refuted in general at degree ≥ 2. **source** DUAL_IS_THE_MEASURE.md §7
(Thm 7.1, §7.3, §7.4) · **relations** refines simplex prices (25);
measures the market's belief at grid resolution; read natively by machines
(P6) · **page** P3, P6.

### 27. zero-gap certificate
Under the certificate-demanding policy tuple, an accepted candidate carries
inside its own witness a zero-duality-gap proof of surplus optimality for
the LP relaxation, with the price vector as optimal dual — the
`StrictUnderfill` refusal is a positive-gap detector. **source**
DUAL_IS_THE_MEASURE.md Thm 5.1, §5.4 · **relations** refines
verify-not-find (29) toward "optimal, certified"; gap itemized per order
when policy relaxes; promotion gated on named falsifiers and the Lean
instantiation · **page** P3, P7.

### 28. checkpoint
The persistent ClearWork account carrying the walk's exact resumable state —
staged into existence across grow instructions, encoded/decoded at every
boundary, and defended by a three-layer tamper stack (fold latch, header
validation, order-set binding). **source** TIER2 plan T2-1/T2-3;
relation_v1_stream.rs · **relations** used by the walk (21); refuses resumed
tampering (`ResumeFoldMismatch`, anchor comparison) · **page** P3.

### 29. verify-not-find
The venue's stance: the chain never searches for a clearing, it verifies
submitted ones exactly — "best valid submitted candidate," never "optimal
clearing," unless a checked certificate exists. **source** AGENTS.md
(correctness vocabulary); DUAL_IS_THE_MEASURE.md §1 · **relations** frames
candidate (19), relation (20), selection (24); upgraded — scoped and gated —
by the certificate (27) · **page** P3, P6.

### 30. dispersion
The candidate fee base `G(a,p) = Σ_{i<j} p_i p_j |a_i − a_j| / S²` — the
unique 1-homogeneous, layer-additive extension of the binary `q·p·(1−p)`;
complete-set invariant, relabeling symmetric, subadditive, ≤ 120 exact
integer pair terms — and provably *not* the model-free risk norm.
**source** docs/FEE_GEOMETRY.md §2–3; RISK_SUMMED_POSITIONS.md Props 9–12
· **relations** measures risk transfer under the market's own measure;
vanishes on Clutch (2); its zero-price kernel hole is a named falsifier;
forced to zero everywhere in the current tree · **page** P5 · **adjacent**
kappa, fee carry, terminal-ceil close.

### 31. quotient norm
The price-free control candidate `κ'·R(a)` — the range seminorm, the same
functional the solvency machinery locks — dispersion's exact envelope
(`G ≤ R/4`) and the other arm of the undecided fee fork. **source**
RISK_SUMMED_POSITIONS.md §1.3, §3.4 · **relations** refines risk quotient
(13) into a fee base; the fork against dispersion (30) is economics the
mathematics cannot close · **page** P5.

## IV. Resolution and refusal

### 32. refusal
The design default at every boundary: hostile bytes, ineligible fills,
fractional lots, ambiguous evidence, and unadmitted sources are refused
with typed errors before any mutation — a refusal is never weakened to make
an integration pass. **source** AGENTS.md (evidence and edits); PROJECT.md
§6 · **relations** frames the relation (20), Hatch (35), custody (10);
project-plane form is STOP (33); economic form is lapse (34) · **page**
P1–P7 (pervasive; taught at P1) · **adjacent** exact-or-refuse,
RemainderRequired, dormancy (`EvidenceOnlyRecoveryV1`, ECONOMICS.md §7).

### 33. claim plane (promotion)
The nontransitive evidence vocabulary — PROVED-MODEL, CHECKED-RUST-SUBSET,
CHECKED-FINITE, HOST-TESTED, SBF-EXECUTED, PROFILE-ADMITTED, MODEL-ONLY,
PROPOSED, IN-FLIGHT, STOP — where every claim names its plane and no plane
substitutes for another; promotion is crossing a named gate with artifacts
from the final joined bytes. **source** CURRENT_TRUTH.md §1, §7
· **relations** governs every other entry's status; STOP is its
blocking-surface form; displayed on-site as evidence badges (site map §4)
· **page** P7.

### 34. lapse
The clearing's honest failure mode: when no admissible certified candidate
exists (provably, exactly when the LP's optimal dual face misses the
admissible price box, in the analyzed cases), the epoch clears nothing
rather than publish a false number. **source** DUAL_IS_THE_MEASURE.md §6
· **relations** refines refusal (32) at the venue plane; dual phenomenon of
certificate (27) · **page** P3, P5.

### 35. Hatch (resolution)
The immutable evidence-derived resolution transition — the system's one
non-isometry, a measurement that collapses the risk quotient to a point —
evaluating the frozen basis at the uniquely admitted statistic and fixing
the payout vector by largest remainder. **source** README.md; PROJECT.md
§6; RISK_SUMMED_POSITIONS.md §1.5 · **relations** uses Feed/Window
evidence and largest remainder (7); requirement never rises through it
(Prop 8); enables redemption under conservation (11) · **page** P4
· **adjacent** Feed, Window, sealed SourceArchive, point Resolve.

## V. The evidence culture

### 36. seal
Binding a claim to exact bytes: a named commit fixes the ELF identity
(SHA-256, byte length), its audit, and its measured behavior, so later
statements are about *that artifact* and drift is machine-refused —
byte-identical rebuilds are part of the seal. **source** CURRENT_TRUTH.md
§2 · **relations** consumed by manifest and attestation (38); any
closure-byte change forks the identity and forces a reseal · **page** P7
· **adjacent** manifest (schema-v2, 100/100 executed gates), baseline,
runtime ancestry.

### 37. two-ELF discipline (the mock boundary)
Success against mock infrastructure requires a distinct, explicitly
non-production artifact; the default sealed program refuses value with
`SourceReleaseUnavailable` (`0x79`) because its source registry is empty —
incompleteness made machine-visible rather than papered over. **source**
CURRENT_TRUTH.md §4 (pooled custody row), §6.3;
docs/reviews/PLANNED_VS_BUILT_2026-08-19.md · **relations** refines refusal
(32) into release engineering; guards custody (10); the named injections of
every walk are inventoried in-tree · **page** P7.

### 38. attestation
Independent re-execution of the portable evidence on a second host from a
fresh archive — gates PASS/STOP counted, files compared twice, the sealed
ELF byte-verified in multiple contexts — recorded as a durable job, never
as a release claim. **source** CURRENT_TRUTH.md §2 (Persvati 41/41, 44/44
PASS 0 STOP); GOAL.md done-log Cycles B–D · **relations** uses seal (36)
and the manifest; distinct from rebuild (cross-OS divergence exhaustively
classified); feeds no promotion by itself (33) · **page** P7.

## VI. Frame concepts

### 39. Realm & Hoard
A Realm is the immutable collateral profile and version namespace a market
lives in; its Hoard is the market-local vault whose principal pays only
claimants — never keepers, rent, fees, or treasury — with liveness work
independently prepaid at admission. **source** PROJECT.md §2–3;
ECONOMICS.md §1–2; AGENTS.md · **relations** custody boundary (10) crosses
the Hoard's edge; conservation (11) is stated over it; prepaid liveness is
the separate second proposition (ECONOMICS.md §1) · **page** P1, P4
· **adjacent** prepaid liveness, protected pools, DREGG (one optional
dogfood profile, never a privileged branch).

### 40. Eggcrate (kernel)
The pure deterministic verification-target transition kernel: `no_std`,
no-alloc, safe, fixed-layout, total Rust with no Solana SDK, oracle SDK, or
CPI — the semantic owner every adapter merely transports for. **source**
PROJECT.md §2, §4; AGENTS.md (kernel policy) · **relations** owns the
transitions diagonal motion (8), materialize (9), Hatch (35); shadowed by
the Lean model and the Verus subset (33); everything runtime is adapter,
not kernel · **page** P1, P7.

---

## Coverage map (concept → page, at a glance)

| Page | Teaches |
| --- | --- |
| P1 The Clutch | 1, 2, 8, 9, 32, 39, 40 |
| P2 The Shape of a Claim | 3, 4, 5, 6 |
| P3 The Clearing | 14–22, 24, 25, 27, 28, 29, 34 |
| P4 The Ledger | 7, 10, 11, 18, 23, 35 |
| P5 The Price of Risk | 12, 13, 30, 31, 34 |
| P6 For the Machines | 19, 25, 26, 29 |
| P7 The Evidence | 27, 33, 36, 37, 38, 40 |

Every concept has exactly one *teaching* page (first listed above) and may
recur elsewhere by reference. The .spw canon should preserve the relation
verbs as typed edges; the densest hubs — Clutch (2), conservation (11),
refusal (32), claim plane (33) — are the canon's natural roots.
