# ADR-0001: specialized transparent batch relation

Status: proposed

## Context

The native venue must couple exhaustive outcomes, exact payoff portfolios, and
complete-set conversion. A generic exchange VM would enlarge the trusted and
verified surface while concealing useful structure. V1 also makes no confidential
order claim.

## Decision

Specify one versioned `BatchRelationV1` with closed order variants, exact simplex
prices, bounded portfolio valuation, virtual split/merge, typed conservation
folds, deterministic allocation, and a public score. Candidate search is
offchain and permissionless; the onchain verifier alone authorizes finalization.

## Consequences

- Relation kernels can be optimized and proved directly.
- Solvers are replaceable and may be incomplete.
- The accepted result is the best valid submitted candidate unless an optimality
  certificate is later checked.
- Orders and witnesses remain public in V1.
- Arbitrary order programs and unrestricted combinatorial baskets are refused.

## Evidence required

Tiny exhaustive oracle agreement, Verus arithmetic/conservation proofs,
Rocq/Lean shadow relation, cross-runtime vectors, adversarial score/allocation
falsifiers, and SBF resource measurements.

## Authority impact

None. The relation design neither authorizes deployment nor changes Gate L0.
