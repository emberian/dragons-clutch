# `clutch-batch`

This is an executable, host-only prototype of a bounded fixed-grid frequent
batch relation. It has no Solana, Token-2022, CPI, keys, account layout, or
matching-service dependency.

Executable facts:

- the grid has at most 64 strictly increasing ticks and the book has at most 64
  orders;
- orders must be in strictly increasing canonical-order-ID order;
- the clearing tick maximizes matched quantity, then minimizes imbalance, then
  chooses the highest tick;
- each side is allocated by integer largest-remainder pro-rata using the frozen
  seed and canonical IDs;
- `DustPolicy::Reject` explicitly refuses unresolved dust, while
  `AssignCanonical` assigns every leftover atom deterministically;
- `verify` recomputes eligibility, fills, side totals, and conservation.

The implementation does not claim global optimality, fairness against order
fragmentation, privacy, settlement correctness, or formal verification. The
Verus theorem inventory and proof seam are under `verus/batch/`; those targets
remain open until a pinned Verus run checks this exact source.

## The coupled relation (`relation_v1`)

`src/relation_v1.rs` implements `BatchRelationV1` from
`docs/implementation/BATCH_RELATION_V1_DESIGN.md` beside the scalar lab, which is
retained unchanged. It is IMPLEMENTED host-model code: not verified, not the SVM
relation, and never an optimality claim. An accepted clearing is the best valid
submitted candidate of its proposal window, nothing more.

Executable facts:

- every fill is bound to `(owner, outcome, side)`; the fill vector, the price
  vector, the conversion pair, and the honored-minimum mask are the whole
  witness, and every claimed aggregate is recomputed from the frozen book;
- per-outcome conservation runs through one global virtual split/merge pair, so
  fills must carry the same net imbalance `c` on every active outcome and a
  cross-outcome "match" has no solution at all;
- V5 checks the exact integer pairing-feasibility inequality
  `part_i(O) <= F_i`, and the canonical constructor freezes the slice
  decomposition that settlement consumes;
- `FrozenPolicyV1` has no `Default`: every variant family named in the design
  (allocation A/B, self-cross N-a/b/c, all-or-none 2a/b/c, rounding R-a/b/c,
  residual settlement 1a/1b/1c, transfer phase T-a/b, portfolio lots P-a/b,
  pairing witness, dust, score, fee base) must be named at the construction site;
- the relation is `no_std`, allocation free, `forbid(unsafe_code)`, float free,
  and every accumulator is checked exact integer arithmetic.

Not implemented, and refused rather than guessed: portfolio marginal lot
rationing (`P-b` returns `PolicyVariantUnimplemented`), the `N-c` owner-aware
decreasing-fixed-point capping rule (infeasible candidates are refused, not
capped), and every fee base except the flat-notional and zero-fee controls.
Settlement, the kernel transfer, and the vertical-model joins live in their own
crates; this one only records their frozen selectors and freezes the slice
universe.
