# Specialized transparent batch relation

Status: E0 research specification. This document defines a target relation, not
an implemented auction or a claim of optimality.

## 1. Purpose

Dragon's Clutch should optimize the meaning of one bounded market transition, not
replay a generic exchange or arbitrary virtual machine instruction by instruction.
The V1 native venue therefore has one versioned public relation:

```text
BatchRelationV1(epoch_commitment, candidate_witness) = Valid(summary) | Error
```

The relation is transparent. Order pages, the accepted witness, the verified
price vector, and final aggregates are public on Solana. Nothing in this document
provides order confidentiality, front-running resistance, FHE, MPC, a TEE, or a
zero-knowledge proof.

The value of writing the venue as a relation is architectural:

- candidate construction can change without changing consensus semantics;
- the same relation can have a slow exact oracle, a fast host solver, a Verus
  verifier, an SBF adapter, and independent Rocq/Lean models;
- market structure becomes visible enough to optimize folds, monotone searches,
  conservation checks, and remainder allocation directly;
- no generic instruction dispatcher enters the trusted economic core.

## 2. Frozen public input

An Epoch becomes eligible for candidate verification only after these values are
immutable:

```text
RelationDomain {
    relation_version
    realm_id
    market_id
    epoch_id
    outcome_count
    price_scale
    fee_policy
    score_policy
    allocation_policy
    page_count
    order_count
    ordered_page_closure
    opening_asset_commitment
    proposal_deadline
}
```

`ordered_page_closure` commits to canonical page identity, generation, used
length, and content digest. The relation rejects missing, duplicated, reordered,
post-freeze, or foreign pages. Human names and client-generated summaries do not
enter the domain.

Every admitted order is already fully reserved in one ownership domain. Freezing
the Epoch prevents cancellation or mutation from racing verification.

## 3. Admitted order language

V1 admits only closed variants.

### Single Egg

```text
SingleEgg {
    owner_position
    outcome_index
    side
    quantity
    limit_tick
    minimum_fill
    partial_policy
    expiry_epoch
    canonical_order_id
}
```

### Proportional payoff intent

```text
Portfolio {
    owner_position
    side
    coefficients[MAX_OUTCOMES]
    active_len
    lots
    limit_collateral_per_lot
    minimum_fill
    partial_policy
    expiry_epoch
    canonical_order_id
}
```

The transferred portfolio for `x` filled lots is `x * coefficients`. Valuation is
one checked dot product followed by one named division. There is no arbitrary
basket bytecode, nested condition, callback, user-supplied predicate, or
all-or-none integer winner-determination language hidden behind the word solver.

Complete-set split and merge are relation-level conversion quantities, not
orders. They are risk-free at the raw simplex vector; all explicit fees remain
outside the price normalization.

## 4. Candidate witness

The candidate contains enough information to check the transition without
trusting the search procedure:

```text
CandidateWitness {
    domain_digest
    prices[MAX_OUTCOMES]
    fill_commitment
    page_witnesses[page_count]
    virtual_split
    virtual_merge
    remainder_seed
    claimed_score
    claimed_closure
    canonical_candidate_digest
}
```

Each page witness contains fixed-order fill quantities and page-local deltas for
collateral, Eggs, fees, carry, and settlement pots. The verifier recomputes those
values. A claimed aggregate is never accepted merely because its hash matches
another claimed aggregate.

Large books use resumable `ClearWork`. Intermediate work is bound to the Epoch,
candidate digest, page cursor, and prior accumulator. Switching candidates cannot
reuse incompatible partial verification.

## 5. Relation stages

The relation is decomposed into small exact stages with canonical intermediate
records:

```text
frozen pages
  -> canonical order fold
  -> simplex validation
  -> eligibility and fill validation
  -> exact portfolio valuation
  -> pro-rata/remainder allocation
  -> virtual complete-set accounting
  -> page-local settlement pots
  -> global conservation closure
  -> public score recomputation
  -> final candidate digest
```

### R0: domain validation

- reject unknown versions, dimensions, scales, or policies;
- require `2 <= outcome_count <= MAX_OUTCOMES`;
- bind every witness field to the exact frozen Epoch;
- reject noncanonical padding and semantically duplicate encodings.

### R1: simplex validation

For every active outcome:

```text
0 <= prices[i] <= PRICE_SCALE
sum(prices[0..n]) == PRICE_SCALE
```

Inactive entries are canonical zero. The sum uses a width with an explicit proof
bound. A price vector is not final until the entire candidate closes.

### R2: fill eligibility

For each order, the verifier recomputes:

- exact order identity and reservation domain;
- expiry and partial-fill policy;
- `0 <= fill <= reserved quantity/lots`;
- minimum fill or all-or-none condition;
- the single-Egg limit at `prices[outcome]`; or
- the portfolio limit from one checked dot product and final rounding.

An ineligible order must have zero fill. The relation never asks an offchain
solver to attest that an order was ineligible.

### R3: allocation

If a marginal set requires proportional allocation, the policy computes a total
quotient and remainder once, then assigns remainder atoms by a frozen permutation
derived from canonical order identity and `remainder_seed`. Required properties:

- exact total conservation;
- fill bounds and minimum rules remain satisfied;
- deterministic result independent of page processing order;
- retry and shard count do not change allocations;
- splitting an economically identical order cannot obtain an unbounded advantage.

The last property is a research gate, not assumed true for an arbitrary rule.

### R4: virtual complete-set conversion

For each outcome `i`, `virtual_split` creates one Egg unit per collateral unit and
`virtual_merge` consumes one Egg unit per collateral unit. The witness must make
all outcome and collateral deltas close simultaneously. Hoard principal appears
only in these exact split/merge equations; it is never a source of trade subsidy,
fees, or candidate bonds.

### R5: fee relation

The candidate fee is recomputed from the canonical filled intent, exact simplex
vector, frozen fee policy, and prior carry. One intent cannot declare a cheaper
payoff vector than it settles. Fee allocation must conserve collected fee, and no
maker/executor/treasury destination may draw from Hoard or prepaid liveness.

### R6: global closure

For each outcome:

```text
opening_reserved[i] + virtual_split
  == unfilled_refund[i]
   + buyer_credit[i]
   + seller_change[i]
   + final_pot[i]
   + virtual_merge
```

The exact decomposition will be frozen with the account layout; every term has
one owner and one sign convention. Collateral closure separately accounts for
reserved cash, consideration, virtual split/merge, fees, refunds, and final pots.

The final relation must prove conservation by construction, not merely compare a
single net-zero scalar that could hide offsetting errors.

### R7: public score

The relation recomputes a lexicographic score from accepted fills and exact
economic deltas. Candidate comparison is deterministic. Until a checked
optimality certificate exists, the result is only the **best valid submitted
candidate** during the bounded proposal window.

The score must not reward self-crossing, duplicated economic orders, artificial
fragmentation, or complete-set churn merely because these inflate a volume-like
counter. Every proposed component gets a small-book adversarial oracle and a
plain-language explanation before promotion.

## 6. Relation primitive set

The executable IR is not user programmable. It is a design vocabulary for
auditing and optimizing the one relation:

| Primitive | Meaning | Bound |
|---|---|---|
| `CheckedFold` | ordered fixed-width accumulation | page/order maximum |
| `BoundedDot` | payoff value with one final division | outcomes <= 16 |
| `SimplexCheck` | exact nonnegative normalized vector | outcomes <= 16 |
| `EligibilityStep` | limit and reservation predicate | one order |
| `AssetDelta` | typed debit/credit in one ownership domain | fixed assets |
| `CompleteSetDelta` | equal Egg creation/destruction | fixed outcomes |
| `DivRemAllocate` | exact quotient/remainder distribution | marginal set |
| `FeeCarryStep` | filled-intent fee and persistent carry | one order |
| `ScoreFold` | exact lexicographic score accumulator | one Epoch |
| `CommitmentFold` | canonical ordered domain separation | fixed pages |

No primitive dispatches an opcode selected by an order author. Each appears at a
fixed point in `BatchRelationV1` and has one semantic owner.

## 7. Search strategies are non-authoritative

Candidate constructors may include:

1. exhaustive enumeration for tiny books;
2. a single-Egg demand/supply curve solver;
3. a separable or dual-price method with complete-set conversion;
4. an exact-rational LP oracle for a restricted divisible portfolio fragment;
5. heuristics that produce valid but potentially inferior witnesses.

All constructors emit the same canonical witness. A faster solver earns no wider
authority. If two constructors disagree, the exact oracle and verifier diagnose
the relation; arrival order does not turn an invalid witness into a transition.

## 8. Verification and refinement targets

### Verus-first executable claims

- every successful stage is total and memory safe;
- active indices, widths, products, sums, and divisions are bounded;
- valuation rounds once at the frozen boundary;
- page folding is canonical and cannot omit or duplicate an order;
- asset deltas and fee allocations conserve exact atoms;
- `ClearWork` resumption is equivalent to one uninterrupted fold;
- final settlement pots cannot also be interpreted as reservations;
- repeated settlement is idempotent.

### Abstract shadow claims

Rocq is the leading abstract model. Lean may independently encode the finite
relation or check generated theorem/vector artifacts, but is not a second
production implementation. Shadow targets are:

- relation acceptance implies feasibility and conservation;
- simplex normalization and complete-set equivalence;
- page/shard decomposition invariance;
- order-permutation invariance where the policy promises it;
- exact remainder conservation;
- candidate comparison is a total deterministic order;
- finalization followed by settlement preserves global solvency.

The manual correspondence among model, Eggcrate, and SBF remains disclosed until
a checked refinement closes it.

## 9. Falsifiers

The initial laboratory must generate counterexamples for:

- one omitted, duplicated, reordered, or mutated order;
- forged page closure or wrong Epoch generation;
- simplex sum off by one atom;
- dot-product per-leg rounding instead of final rounding;
- fill exceeding reservation or violating minimum fill;
- split/merge imbalance in exactly one outcome;
- fee carry reset through order or Position fragmentation;
- self-cross/rebate loops and score inflation;
- candidate change after partial verification;
- page/shard order changing a remainder winner;
- settlement replay or opposite settlement order;
- maximum-width overflow and noncanonical inactive array entries.

Every discovered counterexample becomes a minimized fixture with a provenance and
derivation manifest.

## 10. Promotion boundaries

The relation does not advance beyond E1/E5 gates until:

- its binary and semantic domains are frozen;
- a tiny exhaustive oracle agrees with the verifier;
- Verus proves the bounded arithmetic and conservation core without prohibited
  assumptions;
- host and SBF vectors agree;
- adversarial mutation tests fail for the intended reason;
- resource use is measured at 2, 4, 8, and 16 outcomes and multiple page sizes;
- documentation uses no global-optimality or privacy claim; and
- Gate L0 remains visibly open for every public-network or real-fund action.
