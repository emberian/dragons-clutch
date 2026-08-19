# Scalar batch-shadow assumptions and STOPs

The checked result depends on the soundness of pinned Verus, its vstd
specifications, its Rust 1.97.1 frontend, and bundled Z3 4.16.0. It also
depends on ordinary SHA-256 collision resistance and on human review of the
correspondence below. There are no project `assume`, `admit`, axiom,
`external_body`, `assume_specification`, unsafe block, or proof-only executable
branch in the checked proof unit.

## Refinement boundary

Verus checks the mathematical model in `batch.rs`; it does not import or
compile `crates/clutch-batch/src/lib.rs`. The runner refuses drift in the whole
scalar production file and separately in the `PriceGrid` implementation,
`FixedBook` implementation, and `Candidate` definition. Those digests make the
following review stable, but do not turn it into a machine-checked refinement:

- `allocate_conserves` models one eligible side after `side_total`: `quantity`
  is mathematical nonnegative order quantity, `floor_fill` is the production
  `u128` product/quotient, and `selected[i]` is one successful dust-loop
  selection. The theorem covers the successful positive-total branch.
- `choose_tick_deterministic` models the successful `choose_tick` scan after
  every checked side fold has produced its `(volume, imbalance)` score. The
  mathematical index is the production grid index and the three score clauses
  match the frozen highest-tick tie rule.
- `relation_conserves` models the successful `validate_fills` and `verify`
  seam: the candidate's `matched` field has matched the recomputed expectation,
  and the recomputed buy and sell folds have each matched that expectation.
- `canonical_padding_zero` applies to numeric arrays for which production
  validation checks every inactive slot: `PriceGrid::ticks`,
  `Candidate::fills`, and the scalar book's inactive canonical order IDs. It
  does not claim that every field of an inactive scalar `Order` is zero.

The `relation_v1` and `relation_v1_stream` digests are recorded as excluded
sources. The four theorems are not proofs of the coupled outcome-conservation,
owner-pairing, AON-mask, portfolio, or streaming relations bearing those names.

## Remaining STOPs

1. The production `allocate_side` loop itself is not verified. In particular,
   a proof is still needed that the dust count equals the number of one-shot
   selections it completes, that enough distinct positive remainders exist,
   and therefore that `selected.ok_or(ArithmeticOverflow)` is unreachable for
   `AssignCanonical`. These appear as explicit premises of
   `allocate_conserves`, not axioms.
2. The quotient model uses unbounded mathematical integers. The review relies
   on the production checked `u64` side sum and widening `u64 * u64` to `u128`;
   compiler correspondence and the absence of production arithmetic errors are
   not proved by this shadow.
3. Score construction (`side_total`, `min`, and `abs_diff`) and the production
   scan loop are digest-pinned but not refined instruction-by-instruction.
4. `relation_conserves` proves the scalar relation's two-side equality. It does
   not repair or verify the scalar relation's documented executable-pairing
   defect, and makes no global optimality claim.
5. Scalar `FixedBook::validate` checks only the canonical order ID in inactive
   order slots. Other inactive `Order` fields need not be zero, so the stronger
   whole-record canonical-padding statement is intentionally not claimed.
6. Cargo/host execution, serialization, accounts, Solana/SBF, CPI, deployment,
   and every coupled V1 property remain outside this result.
