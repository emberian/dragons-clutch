# BatchRelationV1 coupled-relation design

Status: **PROPOSED design document.** Nothing in this file is a frozen policy, a
landed implementation, or an evidence claim. Every policy decision below is
presented as explicitly named PROPOSED variants; per the handoff blocker "policy
freeze" ([`CODEX_HANDOFF.md`](../../CODEX_HANDOFF.md) §7 P0-5), convenience code
may not canonize a policy, and an implementation lane following this document
must build the variant scaffolding without selecting a silent default. Where
this document says "recommended," that is a design recommendation awaiting the
mechanism-owner falsifier gate, not a selection.

Claim labels follow the handoff vocabulary: IMPLEMENTED / MODEL / PROPOSED /
BLOCKER. The selected candidate of any epoch is only ever the **best valid
submitted candidate**; no statement here is an optimality claim.

Scope: this designs the host-model coupled relation that replaces the scalar
call-auction falsifier as the semantic target of `crates/clutch-batch`, plus the
kernel and vertical-model changes it needs. It does not design account layouts,
SBF adapters, authentication, privacy, or fees beyond naming the joins.

Ground truth this design refines:

- [`docs/SPECIALIZED_BATCH_RELATION.md`](../SPECIALIZED_BATCH_RELATION.md) — the
  documented target relation (stages R0–R7, witness shape, falsifier list).
- [`docs/SIMPLEX_AUCTION.md`](../SIMPLEX_AUCTION.md) — venue rationale, order
  families, tractability boundary, score proposal.
- `crates/clutch-batch/src/lib.rs` — IMPLEMENTED scalar falsifier
  (`MAX_ORDERS = 64`, canonical pro-rata/largest-remainder allocation,
  exact-equality verification). Retained unmodified as a regression lab.
- `research/vertical-model/src/lib.rs` — IMPLEMENTED host composition
  (`BoundOrder`, `BatchDomain`, `CandidateIdentity`, paired settlement receipts,
  cumulative per-order ledgers, clone/apply/check/commit staging).
- `crates/clutch-kernel/src/lib.rs` — IMPLEMENTED claim kernel
  (`MAX_OUTCOMES = 16`, `MAX_PAYOUTS = 8`; split/merge/materialize/
  dematerialize/resolve/redeem; **no transfer between positions exists today**).
- [`ADVERSARIAL_REVIEW_V0.md`](ADVERSARIAL_REVIEW_V0.md) — especially §P1-B and
  the §6 required counterexample suite.

## 1. The defect this relation repairs

§P1-B of the adversarial review, restated as the design obligation:

The landed scalar relation clears one grid tick over generic buy/sell
quantities. The vertical model's `BoundOrder` attaches owner and outcome, but
`clear_batch_with_bindings` strips those bindings before clearing: only the
scalar `Order` reaches `FixedBook`. Consequently a buy bound to outcome 0 and a
sell bound to outcome 1 produce `matched = 1`, a fee, and a liveness charge —
and then every paired settlement refuses the outcome mismatch. Self-cross-only
books and invalid owners are likewise not rejected at clear time. Candidate
volume is therefore not economically meaningful: the executed fixture charged
for volume that no executable transfer could ever realize.

**The core obligation of BatchRelationV1: verification must prove, from the
candidate witness and the frozen book alone, that the accepted fills admit a
complete executable pairing under the bound owner/outcome/side policy.**
Checking bindings only at lazy settlement is too late. A candidate whose fills
cannot all settle is invalid, not merely inconvenient.

Two structural mechanisms below discharge this obligation:

1. **Per-outcome conservation with a single global virtual split/merge pair**
   (§7) makes cross-outcome "matches" arithmetically impossible: the P1-B
   fixture has no solution to its conservation system and is refused at
   verification, never charged.
2. **The pairing-feasibility gate** (§8) is an exact integer inequality,
   necessary and sufficient for the existence of a complete distinct-owner
   pairing, checked at verification time in one bounded pass.

## 2. Conceptual frame — sealed candidate domain (attribution)

The following ideas are drawn, as ideas only, from the leanuweave candidate-
world results (`/Users/ember/dev/leanuweave/Uwueave/Holes.lean`,
`/Users/ember/dev/leanuweave/Uwueave/Gluing.lean`). No code moves between
repositories; [`docs/PROVENANCE.md`](../PROVENANCE.md) governs any future code
movement, and none is proposed here.

- **The frozen order set is the sealed candidate domain.** In `Holes.lean`,
  `stable_inputs_seal_the_result` shows that finalizing the inputs of a
  computation transports stability to every derived answer, whatever the
  computation was. The epoch freeze (immutable `ordered_page_closure`, no
  post-freeze placement or cancellation) is exactly that input-side licence:
  once the book is sealed, every candidate any solver can ever construct is an
  image of the same frozen domain.
- **Batch close is the stability cut that licenses the collapse.** Selecting
  one enforceable clearing from the candidate set is `SealsTo` in the
  `Holes.lean` sense — the claim that every surviving candidate is *the*
  answer — and `seal_survives_stable` says such a claim is safe only under a
  stability licence (`Stable`): nothing that can still arrive may move the
  result. The bounded proposal window plus complete page verification is that
  licence. "Best valid submitted candidate" is precisely a seal justified by
  window closure, **not** by optimality — the collapse is priced by what the
  licence actually bought, which is why the repository's vocabulary rule
  exists. A price vector is not final until the entire candidate closes
  (`SIMPLEX_AUCTION.md` §2); collapsing earlier is `unstable_seal_clash`: a
  wrong answer asserted with unearned confidence.
- **Track candidate state per book, never per order.** The phantom-composition
  theorem (`monadic_has_phantoms`) proves that tracking candidates per
  component and recombining them manufactures answers no whole-world candidate
  justifies — sound as an over-approximation, strictly lossy as a semantics.
  P1-B is this theorem wearing market clothes: scalar clearing composed
  per-projection summaries (side totals with outcome erased) and produced
  matched volume that no submitted book justifies. BatchRelationV1 therefore
  evaluates candidates only against the whole frozen book; no stage may accept
  an aggregate that was not recomputed from the full order set (this is also
  the existing witness rule: "a claimed aggregate is never accepted merely
  because its hash matches another claimed aggregate").
- **Exactly-once receipts need one sequential owner.** `Gluing.lean`'s
  `oneShotHole_never_glues` proves that an at-most-once discipline is a
  uniqueness invariant and can never be delivered by merging independently
  advanced states; it is exactly the coordination a mergeable state avoids.
  Settlement receipt state (which pair/slice has settled, and how much) must
  therefore live in one authoritative sequential ledger with a single semantic
  owner — never reconstructed by combining per-party or per-page views. The
  vertical model's per-candidate ledger already has this shape; the design
  keeps it and the eventual adapter must preserve it as a single persisted
  authority.

## 3. Relation overview

```text
BatchRelationV1(frozen domain, candidate witness) = Valid(summary) | Refusal
```

Pipeline (each stage exact, bounded, and refusing; stage names are used by the
error taxonomy in §12):

```text
V0 domain + admission + normalization      (orders -> bound legs)
V1 simplex validation                      (prices exact on the scaled simplex)
V2 eligibility classification              (strict / marginal / ineligible)
V3 canonical fill derivation + exact equality
V4 virtual complete-set conservation       (per-outcome closure, sigma/mu)
V5 pairing feasibility gate                (complete executable pairing)
V6 portfolio valuation + consideration     (one named rounding boundary)
V7 fee relation                            (payer debited; carry)
V8 per-asset conservation closure          (collateral + every Egg)
V9 score recomputation + total tie order + candidate digest
```

The witness carries the price vector, virtual split/merge quantities, the fill
vector, the policy-dependent auxiliary witnesses (§10, §11), and claimed
aggregates. Every claimed aggregate is recomputed. Verification given the
witness is a bounded number of passes over fixed arrays — no search. All search
lives in untrusted candidate constructors, exactly as in
`SPECIALIZED_BATCH_RELATION.md` §7.

## 4. Frozen domain, admitted orders, and normalization (V0)

### 4.1 Domain

```text
RelationDomainV1 {
    relation_version         // frozen constant for this design
    market_id, book_id, epoch, policy_id, order_set_id   // extends BatchDomain
    outcome_count            // 2 ..= MAX_OUTCOMES (16)
    price_scale              // PRICE_SCALE, exact integer
    policy: FrozenPolicyV1   // every variant selection in this document,
                             // explicit, no Default impl
    remainder_seed
}
```

`FrozenPolicyV1` names one member of every variant family in this document
(allocation A/B, normalization N-a/b/c, AON 2a/b/c, rounding R-a/b/c, residual
settlement 1a/b/c, transfer-phase T-a/b, plus the score component vector). The
host model constructs it explicitly per test; there is deliberately no default,
so no code path canonizes a policy by omission. In the eventual persisted
relation the policy identity is part of the frozen epoch domain.

### 4.2 Admitted order language

Two closed families, extending the vertical model's `BoundOrder`:

```text
SingleEggOrderV1 {
    canonical_order_id      // nonzero, strictly increasing in the book
    owner                   // bounded owner tag (host: index < OWNERS;
                            // relation: reserved owner_position identity)
    outcome                 // < outcome_count
    side                    // Buy | Sell
    quantity                // > 0 Egg atoms
    limit_price             // 0 ..= PRICE_SCALE, scaled integer
    minimum_fill, partial_policy, expiry_epoch
}

PortfolioOrderV1 {
    canonical_order_id
    owner
    side
    coefficients[MAX_OUTCOMES]   // exact nonnegative integers; canonical zero
                                 // beyond active_len; not all zero
    active_len
    lots                         // > 0
    limit_collateral_per_lot     // scaled integer, see V6
    minimum_fill_lots, partial_policy, expiry_epoch
}
```

Admission refusals (all at V0, before any fee or liveness charge):

- owner outside the admitted owner set / not the reservation owner
  (`InvalidOwner`);
- outcome or `active_len` out of range; noncanonical coefficient padding;
- zero quantity/lots; `minimum_fill > quantity`; AON with
  `minimum_fill != quantity`; expired orders;
- non-strictly-increasing `canonical_order_id`; noncanonical array padding
  (these preserve the scalar crate's canonical-sequence and padding gates).

### 4.3 Normalization to bound legs

Every admitted order is lowered to per-outcome **legs**:

- a single-Egg order is one leg `(owner, outcome, side, quantity, order ref)`;
- a portfolio order filled for `x` lots contributes, for each active outcome
  `i`, a leg of quantity `x * coefficients[i]`, same owner and side. Leg
  quantities are derived from the *filled lot count*, so portfolio legs enter
  V3–V8 only after lot fills are fixed.

Bounds: `MAX_ORDERS = 64` total orders retained; PROPOSED
`MAX_PORTFOLIO_ORDERS = 8` so the leg array is bounded by
`MAX_LEGS = MAX_ORDERS + MAX_PORTFOLIO_ORDERS * MAX_OUTCOMES = 192` fixed
entries. All arrays are fixed-size `no_std`/no-alloc; the two constants are
PROPOSED capacity parameters, not economics.

### 4.4 Self-cross normalization — PROPOSED variants N-a / N-b / N-c

The same owner holding buy and sell exposure on the same outcome in one epoch
is the root of wash volume, score inflation, and pairing infeasibility. Three
variants; the pairing mathematics of §8 is stated for the general case (N-c)
and degenerates to a construction theorem under N-a/N-b.

- **N-a (refuse overlap).** V0 refuses any book in which one owner has both a
  buy leg and a sell leg on the same outcome (portfolio legs included).
  State: none. Transition: admission-time refusal `SelfCrossRefused` naming
  the owner and outcome. Falsifier: a self-cross-only book refuses at V0 with
  no fee or liveness charge; a compliant book never triggers §8's owner
  inequality (asserted unreachable).
- **N-b (net at admission).** V0 cancels `min(buy, sell)` quantity of each
  same-owner/same-outcome opposite-side overlap *before* clearing, refunding
  the cancelled reservations; remaining legs are single-sided per owner per
  outcome. Netting is price-independent (limits are ignored), which is
  economically conservative: it may cancel volume that would not both have
  been eligible at the final price vector. That trade-off is the variant's
  documented cost, in exchange for unconditional pairing completeness and
  wash-volume elimination. A book that nets to zero volume clears as the
  canonical empty candidate (valid, zero score), not an error.
- **N-c (allow, gate at V5).** Owners may stand on both sides; §8's
  feasibility inequality becomes a live refusal (`PairingInfeasible`), and the
  canonical fill derivation must additionally cap offending owners' fills by
  the deterministic decreasing-fixed-point rule of §8.4. Full order autonomy,
  strictly more verification and proof burden, and the score must separately
  neutralize self-overlap volume (§11).

Under every variant, direct settlement pairs must have distinct owners (the
landed vertical-model rule is preserved).

## 5. Simplex price vector (V1)

Unchanged from the documented target, restated for closure:

```text
0 <= prices[i] <= PRICE_SCALE          for i < outcome_count
prices[i] == 0                         for i >= outcome_count (canonical)
sum(prices[0..outcome_count]) == PRICE_SCALE
```

The sum is accumulated in a width with an explicit bound
(`16 * PRICE_SCALE <= u64::MAX` for any sane scale; the implementation carries
the bound as a compile-time assertion). One-atom sum errors and noncanonical
inactive entries are distinct refusals (`SimplexSumMismatch`,
`NonCanonicalPadding`) so the mutation suite can kill each comparison
independently. A complete set values at exactly `PRICE_SCALE` price units
before explicit fees; fees never enter the normalization
(`SIMPLEX_AUCTION.md` §9).

Units: all consideration arithmetic inside the relation is in **price units**
(collateral atoms scaled by `PRICE_SCALE`), held in `u128` accumulators with
explicit width bounds. The single conversion back to collateral atoms is the
named rounding boundary of §9.

## 6. Candidate witness

```text
CandidateV1 {
    domain_digest            // host model: RelationDomainV1 by value
    prices[MAX_OUTCOMES]
    virtual_split            // sigma, in complete-set units
    virtual_merge            // mu
    fills[MAX_ORDERS]        // single-Egg: Egg atoms; portfolio: lots
    honored_aon_mask         // present only under AON variant 2b (§10)
    pairing_slices           // present only under witness-pairing fallback (§8.5)
    claimed_score            // recomputed at V9
    canonical_candidate_digest
}
```

The host model keeps `CandidateIdentity { domain, candidate }` as the ledger
key exactly as landed; the future persisted relation replaces by-value identity
with the committed digests of `SPECIALIZED_BATCH_RELATION.md` §4. Candidate
state — current best, proposal-window bookkeeping, settlement ledgers — is
tracked **per book**, one ledger per `CandidateIdentity`, never per order
(§2, phantom composition).

## 7. Eligibility, canonical fills, and virtual conversion (V2–V4)

### 7.1 Eligibility classes (V2)

At candidate prices `p`, each order is classified exactly:

- single-Egg buy: strict if `limit_price > p[outcome]`, marginal if equal,
  ineligible if less; sells mirrored;
- portfolio: with per-lot value `v = dot(coefficients, p)` (a `u128`
  `BoundedDot`), a buy is strict/marginal/ineligible by comparing `v` against
  `limit_collateral_per_lot * PRICE_SCALE` — a cross-multiplied exact integer
  comparison, so **eligibility involves no division and no rounding at all**;
  the only division in the relation is the named boundary of §9;
- expiry and reservation checks repeat here against the frozen domain.

An ineligible order must have fill 0 (`IneligibleFill`). This extends — never
weakens — the repaired scalar eligibility-recomputation gate.

### 7.2 The coupling identity

Let, per outcome `i` at prices `p`: `B_i` = executed buy quantity (all buy
legs, portfolio legs expanded), `E_i` = executed sell quantity. Virtual
conversion is a single global pair `(sigma, mu)`: `virtual_split = sigma`
creates `sigma` units of *every* Egg for `sigma * PRICE_SCALE` price units of
pot collateral; `virtual_merge = mu` is the inverse. Per-outcome conservation:

```text
B_i + mu == E_i + sigma        for every active outcome i        (C-i)
```

Subtracting (C-i) for two outcomes shows `B_i - E_i` is the **same constant
`c = sigma - mu` for every outcome**. This is the complete-set coupling made
arithmetic: fills must carry identical net imbalance on every outcome, or no
`(sigma, mu)` exists. Canonical form additionally requires
`min(sigma, mu) == 0` (`ChurnNotCanonical` otherwise), so `(sigma, mu)` is
exactly `(max(c,0), max(-c,0))` and is verified, not trusted.

**This closes P1-B by construction.** The executed counterexample — buy bound
to outcome 0, sell bound to outcome 1 — demands `B_0 - E_0 = 1` and
`B_1 - E_1 = -1`, which no constant `c` satisfies: the system (C-i) is
infeasible, verification refuses (`OutcomeConservationMismatch`), and no fee or
liveness charge occurs. (Economically: those two orders are the same
directional exposure; there is nothing to cross, and the arithmetic now says
so.) The named falsifier is
`batch_bound_outcomes_and_owners_admit_complete_executable_pairing`.

### 7.3 Canonical fill derivation (V3)

The candidate's free economic coordinates are `(p, c)` (plus the AON mask
under 2b). Everything else is derived and checked for exact equality:

1. `sigma = max(c, 0)`, `mu = max(-c, 0)`; must equal the witness fields.
2. Per outcome, with eligible demand `D_i(p)` and supply `S_i(p)`:
   `B_i = min(D_i, S_i + c)` and `E_i = B_i - c`. Refuse if any `B_i < c` or
   `E_i < 0` (`InfeasibleVirtualLeg`) — the candidate proposed more conversion
   than the book supports.
3. Distribute `B_i` among buy legs and `E_i` among sell legs by the frozen
   allocation policy (below); portfolio orders are rationed in whole lots.
4. Recompute the full fill vector and require **byte-for-byte equality** with
   `candidate.fills` — the repaired canonical-allocation exact-equality gate,
   extended to the coupled relation. Aggregate side totals are necessary but
   never sufficient (`ADVERSARIAL_REVIEW_V0.md` §2).

Allocation policy — PROPOSED variants (the marginal machinery — largest
remainder, then the frozen seeded permutation over canonical order identity —
is retained verbatim in structure from the scalar crate under both):

- **A (price priority, marginal pro-rata).** Strict orders fill fully;
  refusal `StrictUnderfill` if `B_i` cannot cover strict demand (the candidate
  is invalid at that `(p, c)`; solvers move `p` so the rationed order becomes
  marginal — standard uniform-price behavior). The marginal set absorbs the
  residual pro-rata with largest-remainder + seeded rank. Matches
  `SPECIALIZED_BATCH_RELATION.md` R3's "marginal set" language.
- **B (full pro-rata over all eligible).** All eligible orders pro-rated to
  the target, as the landed scalar crate does. No strict-underfill refusal;
  weaker price-priority economics. Kept because it is the behavior the scalar
  falsifier already froze and differentially tests.

Portfolio lot rationing — PROPOSED sub-variants:

- **P-a (V1-simple).** Portfolio orders fill only when strict, and in whole
  lots, all-or-none per order under variant A; marginal portfolios fill zero.
  Eliminates portfolio marginal rationing and its cross-outcome coupling from
  V1 entirely.
- **P-b (general).** Marginal portfolios receive a per-order lot quotient by
  pro-rata over lots, with per-outcome residuals reconciled by the single-Egg
  marginal set. Research-gated; not recommended for the first lane.

Existence note (open question, §15): under variant A a book can lack any valid
candidate at some price vectors; the relation does not promise existence, and
an epoch whose proposal window closes with no valid candidate lapses under the
epoch-lapse rule (full refund of reservations; liveness cost borne by the
prepaid liveness budget, never by Hoard). Lapse policy is itself PROPOSED.

## 8. The pairing witness (V5) — the core of this design

### 8.1 What must be proved

A complete executable pairing is an assignment of every filled unit to an
executable transfer:

- a **direct pair**: one buy-leg unit and one sell-leg unit, same outcome,
  **distinct bound owners**;
- a **split pair**: one buy-leg unit served from the virtual split's output on
  that outcome (capacity `sigma` per outcome);
- a **merge pair**: one sell-leg unit absorbed by the virtual merge's demand
  on that outcome (capacity `mu` per outcome);

such that every leg's fill is fully covered and the virtual capacities are
exactly consumed. Split–merge self-pairs are excluded by canonical
`min(sigma, mu) = 0`.

### 8.2 The feasibility theorem

Define per outcome `i` the total flow `F_i := B_i + mu = E_i + sigma`, and per
owner `O` the participation
`part_i(O) := buyfill_i(O) + sellfill_i(O)` (filled quantities only, portfolio
legs expanded).

**Theorem (pairing feasibility).** Given per-outcome conservation (C-i), a
complete executable pairing exists **iff** for every active outcome `i` and
every owner `O`:

```text
part_i(O) <= F_i                                            (H-i-O)
```

*Necessity.* Owner `O`'s buy units can pair only with sell units not owned by
`O` plus the split capacity: `buyfill_i(O) <= (E_i - sellfill_i(O)) + sigma`.
Substituting `E_i + sigma = F_i` gives (H-i-O); the sell-side inequality
`sellfill_i(O) <= (B_i - buyfill_i(O)) + mu` rearranges to the *same*
inequality — under conservation the two Hall conditions coincide, which is why
one scan suffices.

*Sufficiency.* Model outcome `i` as a transportation network: sources = buy
legs (capacity = fill) plus the merge node (capacity `mu`); sinks = sell legs
(capacity = fill) plus the split node (capacity `sigma`); an edge joins a buy
leg and a sell leg iff owners differ; merge connects to every sell leg; split
to every buy leg. For any set `S` of sources, if `S` contains legs of two
distinct owners its neighborhood is every sink; so the only binding Hall
constraints come from single-owner source sets, and those are exactly
(H-i-O). Max-flow min-cut then yields a feasible fractional flow, and the
integrality theorem for integer-capacity networks yields an integral one. This
is the König/flow-style argument; it is exact-integer and bounded, and the
formal shadow target is stated in §14.

**Verification therefore checks (H-i-O) directly**: one pass accumulating
`part_i(O)` into a fixed `[MAX_ORDERS][MAX_OUTCOMES]` table (owners are
deduplicated into at most `MAX_ORDERS` tags), then one comparison pass.
`O(legs + owners * outcomes)` exact integer work; no search, no pairing
construction at verify time. Refusal: `PairingInfeasible { outcome, owner }`.

Under normalization N-a/N-b every owner is single-sided per outcome, so
`part_i(O) <= max(B_i, E_i) <= F_i` holds automatically and (H-i-O) becomes a
construction theorem; the check is retained as defense-in-depth with a
falsifier asserting it is unreachable.

### 8.3 Decision: recomputed canonical pairing, gated by the inequality

**Decided shape (PROPOSED, recommended):** the candidate does **not** carry an
explicit pairing list. Verification accepts on (H-i-O) — which by the theorem
is exactly "a complete executable pairing exists" — and the **canonical
pairing constructor** (§8.4) is run once, deterministically, at candidate
finalization (batch close) to freeze the slice decomposition that settlement
consumes. Rationale:

- (H-i-O) is the *whole* proof obligation, checked in linear passes; an
  explicit per-pair witness adds `O(pairs)` bytes and per-slice checks without
  adding any acceptance power;
- the witness stays small and page-foldable (`ClearWork` resumability of the
  accumulation table is one fixed-size accumulator, matching the resumable
  fold requirements);
- the constructor runs once per epoch at the seal, not once per submitted
  candidate.

**Fallback variant (PROPOSED, kept fully specified):** if the constructor
completeness theorem (§8.4) resists mechanization, the candidate instead
carries `pairing_slices` — an explicit list of
`(buy_ref | MERGE, sell_ref | SPLIT, outcome, quantity)` — and verification
checks each slice's executability (opposite sides, same outcome, distinct
owners, nonzero quantity), that per-leg slice sums equal fills, and that
virtual slice sums equal `sigma`/`mu` per outcome. Bounded by
`MAX_SLICES = 2 * MAX_LEGS + 2 * MAX_OUTCOMES` fixed entries. This variant
trades witness size for zero algorithmic proof burden: the witness *is* the
pairing. Both variants refuse the same books; they differ only in who carries
the proof.

### 8.4 Canonical pairing constructor

Frozen deterministic algorithm, per outcome, over filled legs:

```text
while flow remains:
  choose the side (buy/sell) whose maximum remaining owner participation
    is largest; tie: buy side
  choose on that side the owner with largest remaining participation;
    tie: lowest seeded_rank(canonical_order_id, remainder_seed), then lowest id
  choose the counterparty: the opposite-side owner with largest remaining
    participation among owners != chosen owner, with the virtual node
    (split for a buy, merge for a sell) treated as an ownerless counterparty
    of participation = its remaining capacity; same tie rule
  pair quantity q = min(chosen leg remainder, counterparty leg remainder,
    slack), where slack = remaining_F - max over other owners of remaining
    participation, floored at 1
  emit slice (buy leg, sell leg | SPLIT/MERGE, outcome, q); decrement
```

The largest-remaining-counterparty rule is the standard argument shape for
matching with a single forbidden partner (naive first-fit strands: buys
`{A:1, C:1}` against sells `{B:1, C:1}` fails if `A` pairs `B` first, though
`A–C, C–B` completes). Invariant to be proved: after every emitted slice,
`max_O part_i(O) <= remaining F_i` is preserved, hence the loop terminates
with zero residue exactly when (H-i-O) held. Slice count is bounded because
each step exhausts a leg or a capacity.

Proof obligations and their homes:

- **MODEL now:** an exhaustive tiny-book oracle
  (`pairing_constructor_completes_iff_feasibility_inequality_holds`) checking,
  over all books within small bounds (≤ 6 legs, ≤ 3 owners, ≤ 3 outcomes,
  quantities ≤ 4, all `(p, c)`), that the constructor completes iff (H-i-O)
  holds, and that its output is invariant under page/shard order and order-id
  relabeling that preserves canonical rank inputs.
- **PROPOSED formal model:** Lean statement of the feasibility theorem and the
  constructor invariant (**Lean is the abstract model of record** per
  `SPECIALIZED_BATCH_RELATION.md` §8 and ADR-0005, adopted 2026-08-20; the Rocq
  shadow role is retired). BLOCKER: the theorems do not exist — the proof
  toolchain is pinned (`toolchain/PINNED_PROOF_TOOLS.md`), so pinning is no
  longer the obstacle. No proof claim may be made meanwhile.

### 8.5 Refusals from this stage

`PairingInfeasible` (H-i-O violated — includes every self-cross-only book
under N-c), `OutcomeConservationMismatch` (C-i unsatisfiable),
`ChurnNotCanonical`, `InfeasibleVirtualLeg`, and — under the fallback variant —
`SliceNotExecutable`, `SliceSumMismatch`.

## 9. Consideration, rounding, and per-asset closure (V6, V8)

### 9.1 Exact ledger

All batch cash is tracked in price units (`u128`): a buy leg of quantity `q`
on outcome `i` owes exactly `q * p_i` price units; a sell leg is owed the
same; the split consumes `sigma * PRICE_SCALE`; the merge produces
`mu * PRICE_SCALE`. With (C-i) these balance identically — the relation proves
it by summing both sides per outcome, not by one global net-zero scalar
(R6's "conservation by construction" requirement: every term has one owner and
one sign convention).

Per-Egg closure per outcome `i` (host-model form of the R6 equation), as two
equations whose every term has one owner and one sign convention:

```text
seller_debits[i] + sigma == buyer_credits[i] + mu          (Egg flow)
opening_reserved[i]      == seller_debits[i] + unfilled_refund[i]
                                                           (reservation split)
```

Substituting the second into the first recovers R6's single-line form
`opening_reserved[i] + sigma == unfilled_refund[i] + buyer_credits[i] + mu`
(with `final_pot[i]` appearing only transiently between clear and settlement
under §13's frozen variant); the two-equation form is the one verified,
because it separates reservation ownership from flow ownership.

Collateral closure separately accounts reserved cash, consideration in price
units, `sigma`/`mu` conversions, fees, refunds, and final pots; every atom has
exactly one owner at every stage, and no state may interpret the same asset as
both reservation and settlement pot (the R8 phase-ownership rule).

### 9.2 The one named rounding boundary — PROPOSED variants R-a / R-b / R-c

Price units convert to collateral atoms exactly once. Three variants, each
with one named boundary and an owner for every remainder atom:

- **R-a (lot admission; exact by construction).** Order quantities must be
  multiples of a frozen lot such that every consideration is divisible by
  `PRICE_SCALE` (sufficient: quantity lots of `PRICE_SCALE` atoms; refinements
  per-market). Boundary name: `RoundingBoundary::None` — the division is
  always exact, mirroring the kernel's exact-or-refuse redemption. Cost:
  coarse order sizes.
- **R-b (terminal floor per owner).** The exact price-unit ledger persists
  through settlement; conversion floors **once per owner per epoch** at
  settlement-pot payout; the summed remainders (bounded by
  `owners * (PRICE_SCALE - 1)` price units) are credited to a named rounding
  pot with a frozen terminal sweep rule (PROPOSED: swept to fee revenue at
  epoch close; never to Hoard). Boundary name:
  `RoundingBoundary::TerminalOwnerFloor`.
- **R-c (floor per receipt).** Each settlement receipt floors its own
  consideration; per-receipt remainders credit the rounding pot. Simplest to
  persist, most rounding events (still exactly one per receipt, and the pot
  conserves every atom). Boundary name: `RoundingBoundary::ReceiptFloor`.

Portfolio valuation uses the same single boundary: the dot product is exact in
price units; per-leg rounding is structurally impossible because no per-leg
division exists (§7.1). The falsifier
`portfolio_dot_product_rounds_once_at_named_boundary` mutates a per-leg
truncation into the pipeline and must observe refusal/divergence.

### 9.3 Fees (V7) — joins only

The fee relation recomputes the fee from canonical filled intent, the exact
simplex vector, the frozen fee policy, and prior carry. This design fixes the
P1-E structural defects at the join: the **payer is debited** (no fee is
created ex nihilo), allocation conserves the collected fee, and no destination
draws from Hoard or prepaid liveness. The fee *base* remains PROPOSED with the
state-contingent dispersion `G_num` of [`FEE_GEOMETRY.md`](../FEE_GEOMETRY.md)
as the leading candidate and flat-notional as the control; carry is keyed to
canonical owner identity so order fragmentation cannot reset it. Fee-policy
selection is outside this document's scope and remains gated by the economics
lab.

## 10. AON / minimum-fill — PROPOSED variants 2a / 2b / 2c

The review left open whether an unfillable AON order may poison an otherwise
feasible batch. Fully specified variants:

### 2a — refuse AON in V1

- **State:** none. **Transition:** V0 admission refuses
  `partial_policy = AllOrNone` and `minimum_fill > 1`
  (`AonNotAdmitted`, `MinimumFillNotAdmitted`).
- **Refusals:** at admission only; no clear-time interaction exists.
- **Falsifiers:** books containing AON refuse deterministically before any
  charge; the scalar crate's AON fixtures remain green in the scalar lab.
- Properties: zero poisoning surface, zero fixed-point analysis, strictly
  smaller relation. Cost: no minimum-fill expressiveness in V1.

### 2b — witnessed honored-AON subset (two-pass reframed)

The naive two-pass rule ("count AON at zero, then include honorable ones")
hides a fixed-point problem. Let `T(A)` = the set of AON orders honorable when
the canonical fills are computed with subset `A` included at full size. `T` is
**not monotone**: including an AON on one side shrinks same-side marginal
shares. Concrete 2-cycle: AON buys X and Y (each quantity 10) against marginal
sell supply 15 — `T({}) = {X, Y}` (each alone is honorable), but
`T({X, Y}) = {}` (pro-rata 7.5 < 10 each), and honorable subsets are exactly
`{}, {X}, {Y}` with no unique maximum. Knaster–Tarski does not apply, and
verifier-side iteration can oscillate. Therefore the verifier must not compute
the subset at all:

- **State:** `honored_aon_mask` in the candidate witness (fixed bitmask over
  the order array), chosen by the untrusted solver.
- **Transitions:** V3 computes canonical fills treating honored AON orders as
  firm full-size orders and unhonored ones as absent (fill 0). Verification
  checks: every honored AON is eligible and filled exactly fully; every
  unhonored AON has fill 0; fills are otherwise canonical given the mask.
- **Refusals:** `AonMaskDishonored` (an honored order not fully filled or not
  eligible), `AonMaskLeak` (an unhonored order with nonzero fill).
- **Score:** more honored volume scores higher, so candidate competition
  searches the subset lattice; the accepted result is, as everywhere, the best
  valid *submitted* candidate — maximality of the mask is explicitly **not**
  verified, and the documentation must say so.
- **Falsifiers:** the 2-cycle fixture above with masks `{}, {X}, {Y}, {X,Y}`
  (last refuses); `aon_witness_mask_cannot_claim_unhonorable_order`;
  determinism of fills given `(p, c, mask)`; poisoning is structurally
  impossible — an unfillable AON simply stays unhonored at zero, and
  `batch_aon_and_minimum_fill_poisoning_matches_frozen_policy` pins this.
- `minimum_fill` generalizes identically: the mask marks orders whose minimum
  is being honored; unhonored minimum-fill orders fill zero (never a positive
  amount below minimum — preserving the scalar crate's
  `MinimumFillViolation` gate).

### 2c — full-size counting with refusal (landed behavior, bounded)

- **State/transitions:** as the scalar crate today: AON counted at full size
  in targets; construction and verification both refuse when the canonical
  allocator cannot make the AON whole. Consistent, already regression-tested.
- **Poisoning bound (documented, not fixed):** one adversarial AON order that
  is eligible-but-unhonorable at a price vector invalidates every candidate at
  that vector; to deny the whole epoch it must do so at *every* otherwise
  chosen `(p, c)`, and the adversary must fully reserve the order's quantity
  and pay placement/liveness costs, so the attack is capital-priced but
  fee-cheap. The bound is quantified by the mechanism-owner lab on exhaustive
  tiny books before this variant could ever be frozen.
- **Falsifiers:** the scalar AON-bypass fixture (retained); epoch-denial
  fixture measuring the minimum reserved capital to poison given book shapes.

## 11. Score and total tie-ordering (V9)

Deterministic lexicographic score; every component an exact integer recomputed
onchain; comparison direction fixed per component; final strict tie-break the
canonical candidate digest (lowest wins), making the order total. Components
(each individually PROPOSED, per `SIMPLEX_AUCTION.md` §7, with the anti-gaming
requirements of R7 made concrete):

1. maximize executed risk mass — PROPOSED base: dispersion-weighted volume
   `sum_i F_i_direct * p_i * (PRICE_SCALE - p_i)` in exact scaled integers,
   where `F_i_direct := F_i - sigma - mu` counts only direct-pair flow, so
   complete-set churn earns nothing;
2. under N-c only: subtract self-overlap volume
   `sum_O sum_i min(buyfill_i(O), sellfill_i(O))` so wash participation cannot
   raise component 1 through mixed books (under N-a/N-b this term is
   identically zero and omitted);
3. maximize limit-price surplus (exact price-unit surplus over limits);
4. maximize distinct participating owners (not orders — order fragmentation
   must not move this component; the falsifier splits one order into `k` and
   requires an identical score vector);
5. minimize `sigma + mu` (churn and compute burden);
6. canonical digest, ascending.

Every component gets a small-book adversarial oracle
(`batch_fragmentation_and_seed_permutation_oracle` covers 1, 2, 4 under
fragmentation, seed permutation, and shard order) and a plain-language
explanation before promotion. The score never turns an invalid candidate valid;
it only orders valid ones within the proposal window.

## 12. Refusal taxonomy

Extending, never renaming, the scalar crate's `Error`:

```text
V0: InvalidOwner, InvalidOutcome, InvalidQuantity, InvalidMinimumFill,
    NonCanonicalOrderOrder, NonCanonicalPadding, AonNotAdmitted (2a),
    SelfCrossRefused (N-a), ExpiredOrder, TooManyOrders, TooManyPortfolios
V1: SimplexSumMismatch, PriceOutOfRange, NonCanonicalPadding
V2: IneligibleFill
V3: CandidateMismatch (fills != canonical), StrictUnderfill (alloc A),
    FillExceedsQuantity, MinimumFillViolation, AllOrNoneViolation,
    AonMaskDishonored / AonMaskLeak (2b), DustRejected
V4: OutcomeConservationMismatch, ChurnNotCanonical, InfeasibleVirtualLeg
V5: PairingInfeasible, SliceNotExecutable / SliceSumMismatch (fallback)
V6: ConsiderationMismatch, RemainderRequired (R-a)
V7: FeeMismatch, FeePayerUnfunded
V8: ConservationFailure (per named asset and term)
V9: ScoreMismatch, DigestMismatch
any: ArithmeticOverflow
```

Every refusal preserves exact prestate (the vertical model's staging wrapper is
retained around the new entry points) and charges no fee or liveness for
refused candidates.

## 13. Residual-pair settlement — PROPOSED variants 1a / 1b / 1c

Context: the landed model's `(buy_index, sell_index)` pair is one-shot even
when the receipt quantity is below both fills, which can strand settleable
residue (review §2, "Remaining P1 liveness/design gates"). Per §2's one-shot
lesson, whichever variant is frozen, the receipt ledger has exactly one
sequential owner. All variants retain the per-order cumulative ceilings
(`settled_by_order`) — extending, never weakening, the repaired cumulative
ledger — and all receipts bind full `CandidateIdentity`, both order ids,
outcome, quantity, and exact consideration as landed.

### 1a — full-pair-only receipts

- **State:** the canonical slice list from §8.4 frozen at batch close; per
  slice, one `settled: bool`.
- **Transitions:** `settle(slice_ref, receipt)` requires
  `receipt.quantity == slice.quantity` and `settled == false`; applies the
  claim leg via `transfer_internal` (§14.2) and the exact consideration; sets
  `settled`.
- **Refusals:** `PartialPairRefused` (quantity below slice),
  `PairAlreadySettled` (replay), plus every landed wrong-domain/order/price/
  side/owner refusal.
- **Falsifiers:** `q-1` then `q` (first refuses, second succeeds); replay;
  all-permutation idempotence; terminal state has every slice settled and pots
  empty.
- Properties: smallest state, no residue by construction, but one blocked
  slice (e.g. an unfunded buyer) permanently strands exactly that slice's
  volume; needs an epoch-terminal default rule for never-settled slices
  (PROPOSED: after a frozen deadline, unsettled slices cancel with full refund
  of both legs' reservations).

### 1b — cumulative per-pair remaining quantity

- **State:** `remaining[(buy, sell)]` ledger. **Two sub-variants differing in
  the pair universe:**
  - **1b-canonical:** pairs are exactly the frozen slices;
    `remaining` initializes to slice quantity. Behaviorally 1c with divisible
    receipts.
  - **1b-free:** any executable `(buy, sell)` pair (same outcome, opposite
    side, distinct owners) may settle any quantity while both orders' per-order
    cumulative ceilings permit. **Documented hazard:** permissionless
    settlement order can strand residue even when clear-time feasibility held —
    buys `{A:1, C:1}` against sells `{B:1, C:1}`: settling `A–B` first leaves
    only the forbidden `C–C`. 1b-free therefore requires either a terminal
    sweep authority that completes a canonical residual pairing, or explicit
    acceptance of strandable residue; both must be frozen with it.
- **Transitions:** `settle` with any `0 < q <= remaining` (and within both
  order ceilings); decrement.
- **Refusals:** `ExceedsPairRemaining`, `ZeroQuantity`, ceilings, domain, and
  the strand fixture's `PairingInfeasible`-at-settlement refusal under 1b-free.
- **Falsifiers:** the review's `3 + 2 + 1` against fills of 5 now settles
  `3`, `2` and refuses the sixth unit (`ExceedsPairRemaining`), replacing the
  strand-prone `PairAlreadySettled` at unit 4; the `A/C–B/C` strand fixture
  pins whichever terminal rule is frozen
  (`settlement_partial_pair_behavior_matches_frozen_terminal_policy`).

### 1c — unique match-slice receipts frozen at clear time

- **State:** the canonical slice list (id, buy ref or MERGE, sell ref or
  SPLIT, outcome, quantity) committed into the candidate digest at batch
  close; per-slice cumulative `settled_quantity`.
- **Transitions:** receipts name a slice id; any `q` up to the slice residue
  settles; virtual slices settle against the pot position (§14.3).
- **Refusals:** `UnknownSlice`, `SliceExceeded`, replay-by-exhaustion, plus
  the landed set.
- **Falsifiers:** slice-sum equals fills; permutation idempotence across
  slices; a receipt naming a valid pair that is not a frozen slice refuses
  (`UnknownSlice`) — freezing the decomposition is the point.
- Properties: deterministic residue accounting, natural fit with the §8.4
  constructor and with future persisted settlement pots (each slice is a pot
  row); largest state of the three.

## 14. Implementation plan

### 14.1 `crates/clutch-batch`

- **Unchanged:** the existing `FixedBook` scalar relation and its nine tests
  remain as the scalar regression lab; its evidence ledger stays separate from
  the coupled relation's (per §P1-B's instruction).
- **New module** `src/relation_v1.rs` (exported beside the scalar API), still
  `#![no_std]`, `#![forbid(unsafe_code)]`, no alloc, fixed arrays:
  - types: `RelationDomainV1`, `FrozenPolicyV1` (explicit variant selectors,
    no `Default`), `SingleEggOrderV1`, `PortfolioOrderV1`, `NormalizedBookV1`,
    `Leg`, `SimplexPrices`, `CandidateV1`, `PairingSlice`, `ErrorV1` (§12);
  - constants: `PRICE_SCALE` (PROPOSED value `10_000`; a domain parameter,
    not a canonized constant), `MAX_ORDERS = 64` (reused),
    `MAX_PORTFOLIO_ORDERS = 8`, `MAX_LEGS = 192`,
    `MAX_SLICES = 2 * MAX_LEGS + 2 * MAX_OUTCOMES`, with compile-time width
    bound assertions for every accumulator;
  - `verify(domain, book, candidate) -> Result<SummaryV1, ErrorV1>`
    implementing V0–V9 exactly as staged above; `propose(...)` as a
    non-authoritative constructor that searches `(p, c, mask)` on tiny books
    (exhaustive for the falsifier lab) and always round-trips through
    `verify`;
  - `canonical_pairing(book, candidate) -> [PairingSlice; MAX_SLICES]`
    implementing §8.4, deterministic, reusing `seeded_rank` semantics
    (structure retained; no code is shared with any external repository);
  - the per-`(owner, outcome)` participation table as a fixed
    `[[u64; MAX_OUTCOMES]; MAX_ORDERS]` with an owner-tag interning pass.
- The kernel is not a dependency of `clutch-batch` (ownership map preserved:
  batch owns admission/verification/allocation/score; the kernel owns claim
  transitions; composition stays in the vertical model).

### 14.2 `crates/clutch-kernel` — add `transfer_internal`

Today `research/vertical-model/src/lib.rs:774-777` mutates
`positions[seller].internal[...]` and `positions[buyer].internal[...]` inline;
claim movement between positions has no kernel owner. Add exactly one narrow
transition:

```rust
impl MarketState {
    /// Move internal claim balance between two positions of this market.
    /// Total supply and collateral are unchanged; `&self` (not `&mut`)
    /// makes that structural.
    pub fn transfer_internal(
        &self,
        from: &mut Position,
        to: &mut Position,
        outcome: u8,
        quantity: Amount,
    ) -> Result<()>
}
```

Semantics: `validate_shape`, `check_invariants`, phase gate (below),
`quantity > 0`, outcome bound, `from.internal[i] >= quantity`, checked
add/sub, and a post-condition that the two deltas are equal and opposite.
Rust's borrow rules already forbid aliasing the same `Position` through both
`&mut` parameters; distinct *semantic* owners remain the caller's obligation
and are enforced by the settlement path's distinct-owner refusal. No
`transfer_external` is added in V1 (external movement is the token adapter's
seam, not the kernel's).

Phase policy — PROPOSED variants:

- **T-a:** Active phase only. Settlement racing resolution strands unsettled
  receipts; requires an epoch/resolution ordering rule.
- **T-b (recommended for liveness):** Active or Resolved. The transfer moves a
  claim whose price was frozen at clear time; supply and collateral are
  untouched, so every kernel invariant is phase-independent here. Lazy
  settlement then cannot be bricked by resolution.

New kernel tests: `transfer_internal_conserves_supply_and_refuses_insufficient`
(round-trip, zero-quantity, bad outcome, insufficient balance, both phase
variants, invariants before/after).

### 14.3 `research/vertical-model`

- New entry point `clear_batch_relation_v1(domain, book, liveness_cost)`
  replacing the binding-stripping path for the coupled relation; the scalar
  `clear_batch*` entry points remain only as scalar-lab plumbing and are
  marked as such. The staging wrapper (`transact`) wraps every new mutation.
- The candidate ledger extends to store the frozen `CandidateV1`, the slice
  list (variants 1a/1c/1b-canonical), `settled_by_order` (retained), and the
  per-variant residual state of §13.
- Settlement (`settle_relation_receipt`) uses
  `market.transfer_internal(...)` via `split_at_mut` over the positions array
  instead of the inline field mutations at lines 774-777; the landed
  refusals (wrong domain/book/epoch/order/price/side/owner, replay,
  consideration exactness, conservation) are all retained verbatim.
- Virtual legs: add a **pot position** (`pot: Position`) and `pot_cash`
  bucket. At candidate finalization, `sigma` executes `market.split(&mut pot,
  sigma)` funded from collected buyer consideration; split-pair settlement
  transfers Egg legs out of the pot; merge-pair settlement transfers seller
  Eggs into the pot and `market.merge(&mut pot, mu)` returns collateral to
  the cash pot at closure. `check_conservation` extends: pot balances are
  owned terms in every per-outcome and cash equation, and the epoch-terminal
  condition is an empty pot. Hoard principal never appears in any of these
  terms.
- New golden trace `golden/relation_v1.trace` beside `golden/basic.trace`
  (the existing trace is never rewritten — extending, not weakening).

### 14.4 Capacity and gate preservation summary

- All bounds fixed: `MAX_ORDERS = 64`, `MAX_OUTCOMES = 16` (kernel-aligned),
  `MAX_PORTFOLIO_ORDERS = 8`, `MAX_LEGS = 192`, `MAX_GRID_TICKS` untouched in
  the scalar lab. `no_std`, no alloc, `forbid(unsafe_code)` everywhere. The
  participation table (8 KiB) and slice array are host-model sized; SBF
  sizing is an explicitly later gate, not silently assumed.
- Preserved repaired gates, extended: eligibility recomputation (V2 over
  every leg), canonical-allocation exact byte equality (V3 over the coupled
  fills, `(sigma, mu)`, and mask), cumulative settlement ledgers (per-order
  ceilings retained under all §13 variants), error-atomicity staging, and the
  candidate-replay idempotence of the ledger.

## 15. Named test list

Every §6 counterexample name from the adversarial review that touches batch or
settlement, preserved verbatim, plus the new relation falsifiers:

```text
# retained scalar-lab regressions (names from ADVERSARIAL_REVIEW_V0 §6)
batch_rejects_buy_below_clearing_tick
batch_rejects_sell_above_clearing_tick
batch_rejects_noncanonical_fill_reallocation
batch_aon_and_minimum_fill_poisoning_matches_frozen_policy
batch_fragmentation_and_seed_permutation_oracle
batch_bound_outcomes_and_owners_admit_complete_executable_pairing
settlement_cumulative_consumption_cannot_exceed_receipt
settlement_consumes_paired_buy_and_sell_fill_exactly_once
settlement_partial_pair_behavior_matches_frozen_terminal_policy
settlement_rejects_wrong_book_epoch_candidate_owner_side_asset_pair_and_generation
settlement_retry_and_all_permutations_are_idempotent
vertical_every_error_preserves_exact_prestate

# new coupled-relation falsifiers
relation_v1_simplex_sum_off_by_one_atom_refused
relation_v1_noncanonical_inactive_price_refused
relation_v1_cross_outcome_pair_cannot_produce_matched_volume   # the P1-B fixture
relation_v1_self_cross_only_book_refuses_per_frozen_variant    # N-a/N-b/N-c
relation_v1_invalid_owner_refused_before_any_charge
relation_v1_virtual_split_merge_imbalance_in_one_outcome_refused
relation_v1_churn_candidate_not_canonical_and_scores_below_churnless
relation_v1_strict_underfill_refused_under_price_priority      # alloc A
relation_v1_canonical_fills_are_exact_equality_not_aggregates
relation_v1_pairing_feasibility_inequality_is_necessary        # forged Hall break
pairing_constructor_completes_iff_feasibility_inequality_holds # exhaustive tiny books
pairing_constructor_invariant_under_shard_and_seed_permutation
aon_witness_mask_cannot_claim_unhonorable_order                # 2b
aon_two_cycle_book_has_no_unique_honorable_subset              # 2b analysis fixture
portfolio_dot_product_rounds_once_at_named_boundary
portfolio_lot_coupling_conserves_every_outcome_simultaneously
consideration_remainder_has_exactly_one_owner_per_frozen_variant  # R-a/R-b/R-c
fee_payer_is_debited_and_fee_allocation_conserves
fee_carry_survives_order_fragmentation
score_components_are_exact_and_ordering_is_total
transfer_internal_conserves_supply_and_refuses_insufficient
settlement_slice_universe_matches_frozen_variant               # 1a/1b/1c
settlement_strand_fixture_matches_frozen_terminal_policy       # A/C-B/C book
relation_v1_epoch_lapse_refunds_all_reservations
relation_v1_golden_trace_is_stable
```

Then bounded exhaustive/property generation and mutations that each named test
kills, with language-neutral fixtures shared toward the eventual Rocq/Verus
shadows and codec mapping — the same promotion shape §6 of the review already
requires.

## 16. Open questions

1. **Candidate existence under allocation A.** Strict-full-fill plus the
   constant-imbalance identity can make some `(p, c)` empty of valid
   candidates; whether every frozen book admits *some* valid candidate on the
   integer simplex is unproved. The epoch-lapse rule (§7.3) bounds the harm;
   the mechanism lab should measure lapse frequency on random tiny books.
2. **Owner-aware capping under N-c** (§8.4's decreasing fixed point) is
   specified only in shape; its determinism proof and interaction with
   minimum-fill need the exhaustive oracle before N-c is eligible for freeze.
3. **Portfolio marginal rationing (P-b)** couples outcomes through lot
   quotients; whether it preserves the constant-imbalance identity without
   per-outcome residual patches is unresolved — hence the P-a recommendation.
4. **Score component 1's dispersion weighting** shares its base with the
   unfrozen fee geometry; if the fee lab rejects `G_num`, the score component
   needs an independent justification or replacement.
5. **Rounding-pot sweep destination** (R-b/R-c): fee revenue is proposed;
   any alternative must keep Hoard and liveness untouchable.
6. **Slice-count tightness:** `MAX_SLICES` is a safe bound, not a tight one;
   the constructor's real worst case on 64-order books should be measured
   before persisted layouts size pot rows.
7. **`order_set_id` is still caller-supplied** in the host model; deriving it
   from canonical order bytes is an adapter-boundary gate this design inherits
   but does not close (review §2, paired-settlement gates).

## 17. Non-claims and promotion boundary

- This document is PROPOSED; implementing it produces MODEL/IMPLEMENTED
  offline falsifiers only. Nothing here authorizes SVM work, deployment,
  tokens, RPC, or any public-network action; Gate L0 remains open.
- No optimality: every accepted clearing is the best valid submitted
  candidate within its window, and the UI/docs must say so plainly.
- No privacy: the relation is transparent; all order fields are public
  pre-clear.
- No formal-verification claim: the feasibility theorem and constructor
  invariant are design arguments with an exhaustive-oracle falsifier plan;
  they become theorems only when **Lean — the substrate of record** (ADR-0005)
  closes them over the actual definitions (BLOCKER: the theorems do not exist;
  the toolchain is pinned).
- Promotion follows `SPECIALIZED_BATCH_RELATION.md` §10 unchanged: frozen
  domains, tiny exhaustive oracle agreement, proof-tool closure without
  prohibited assumptions, host/SBF vector agreement, mutation tests failing
  for the intended reason, resource measurement at 2/4/8/16 outcomes, and no
  global-optimality or privacy language anywhere.

## 18. Implementation record and corrections (2026-08-18)

IMPLEMENTED: the coupled relation landed in
`crates/clutch-batch/src/relation_v1.rs` (commit f7caf04) with 33 falsifier
tests beside the retained scalar lab. The module documentation is the
authoritative record of six documented deviations from this text; the two
that correct this document rather than refine it are:

1. **§8.4 slack-floor rule refuted.** As literally written (floor slack at
   1), the constructor emits `A-B` on the `{A:1,C:1}` / `{B:1,C:1}` book and
   strands a forbidden `C-C` residue. The implemented greedy keys the
   counterparty choice on **total participation** (not side participation),
   under which positive slack is forced whenever (H-i-O) holds. Oracle
   evidence: 3,255 + 1,072 exhaustive flow tables and 2,592 bounded books;
   the constructor's accept set coincides exactly with (H-i-O) and
   `ConstructorStalled` is unreachable on feasible tables. This section's
   original text is retained above for history; the implemented rule governs.
2. **§9.2 rounding direction.** Flooring both legs cannot conserve; the
   implemented boundary rounds debits up and credits down, with both
   remainders owned by one named non-negative pot.

These are design-document corrections discovered by exhaustive falsifiers,
not code weakenings: the accept set only shrank or stayed equal at every
deviation. The feasibility argument and constructor invariant remain design
arguments with oracle evidence, not machine-checked theorems.

Vertical-model integration record (commit f671156): the landed coupled golden
trace is `research/vertical-model/golden/coupled.trace`, not the
`golden/relation_v1.trace` named in §14.3's checklist above; and the R-b
price-unit/atom rounding pot is carried per ledger by the model but never
drawn on (the model settles in exact price units), so R-b's conversion
boundary is recorded, not exercised, on that path.
