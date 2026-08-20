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
`FixedBook` implementation, exact `allocate_side` body, and `Candidate`
definition. Those digests make the following review stable, but do not turn it
into a machine-checked refinement:

- `dust_loop_has_positive_choice` proves the arithmetic progress obligation
  for one eligible side. From `sum(quantity) = total`, quotient floors, and a
  one-shot `assigned` mask whose count is below `dust`, it derives an
  unassigned positive remainder. `dust_loop_maximal_choice_is_positive` then
  proves any maximal unassigned remainder is positive. The concrete
  `dust_loop_progress_nonvacuous_example` checks that these premises are
  satisfiable. The model sequence is the active eligible orders, in production
  order; ineligible active orders and inactive tail slots are omitted, and
  `assigned` is the production array projected onto those same eligible
  indices. Human correspondence maps its count to completed loop iterations
  and maximality to the nested scan over exactly that eligible projection;
  those filtering and executable loop invariants are not imported into Verus.

- `allocation_decomposes_and_bounds` models one eligible side after
  `side_total`: `quantity` is mathematical nonnegative order quantity,
  `floor_fill` is the production `u128` product/quotient, and `selected[i]` is
  a caller-supplied one-shot selection mask. The theorem proves floor-sum and
  per-fill bounds plus structural decomposition. It does not prove that mask
  is the production dust loop's completed output.
- `choose_tick_deterministic` models the successful `choose_tick` scan after
  every checked side fold has produced its `(volume, imbalance)` score. The
  mathematical index is the production grid index and the three score clauses
  match the frozen highest-tick tie rule.
- `accepted_sides_partition_whole_fill` assumes the successful
  `validate_fills` and `verify` equality checks: the candidate's `matched` field
  has matched the recomputed expectation, and both recomputed side folds have
  matched it. Only the partition of the whole fill fold is derived.
- `canonical_padding_fold_identity` assumes a zero suffix. Human correspondence
  maps that premise to numeric arrays for which production validation checks
  every inactive slot: `PriceGrid::ticks`, `Candidate::fills`, and the scalar
  book's inactive canonical order IDs. The theorem does not prove validation
  establishes this premise and does not claim every inactive scalar `Order`
  field is zero.

The reviewed series `c1a0a656 -> e6ce886 -> c9d1cd4` was independently rerun
before this increment and reproduced `20 verified, 0 errors` plus its four
required expected-red mutants. Against sealed main
`6743b9d5b4bee313987770cc048983e26d8c70f3`, all three reviewed production
files are byte-identical: scalar `lib.rs`
`f25ce5524a71f9e8ad5200992bb69290444865243f26040906d7aa6798013249`,
`relation_v1.rs`
`9d4e3cc0fdfc03a4cd2d08f0257224f79fe4a8f0d1f861a09b75e92755bd30da`,
and `relation_v1_stream.rs`
`53a37049c88a2a2abefec5d3f34f7042a6d546e7469ec543d37583cd49813bf3`.
This is source identity evidence, not a proof of correspondence. (Excluded-source
digests re-recorded 2026-08-20 after semantics-preserving stack-hygiene and
checkpoint-codec changes to the two excluded files — `relation_v1.rs` in-place
zeroing/out-param normalization, `relation_v1_stream.rs` encode/decode codec.
Both changes are gated by the relation's own 19,520-comparison equivalence
suite; the exclusion scope is unchanged and nothing new is claimed proven.)

The `relation_v1` and `relation_v1_stream` digests are recorded as excluded
sources. These theorems are not proofs of the coupled outcome-conservation,
owner-pairing, AON-mask, portfolio, or streaming relations bearing those names.

## Remaining STOPs

1. The production `allocate_side` loop itself is not verified. The new model
   theorem proves enough distinct positive remainders exist at every unfinished
   one-shot assignment and that a maximal choice is positive. Mapping
   `left = dust - count(assigned)`, preservation of one-shot assignment, and
   the nested scan's maximality to the executable Rust remains a digest-pinned
   source review. Thus `selected.ok_or(ArithmeticOverflow)` is mathematically
   unreachable under those mapped invariants, not verifier-checked Rust.
2. The quotient model uses unbounded mathematical integers. The review relies
   on the production checked `u64` side sum and widening `u64 * u64` to `u128`;
   compiler correspondence and the absence of production arithmetic errors are
   not proved by this shadow.
3. Score construction (`side_total`, `min`, and `abs_diff`) and the production
   scan loop are digest-pinned but not refined instruction-by-instruction.
4. The two side-equals-`matched` facts are premises extracted from production's
   acceptance checks, not theorem conclusions. The theorem derives the
   whole-fill partition only. It does not repair or verify the scalar relation's
   documented executable-pairing defect, and makes no global optimality claim.
5. Scalar `FixedBook::validate` checks only the canonical order ID in inactive
   order slots. Other inactive `Order` fields need not be zero, so the stronger
   whole-record canonical-padding statement is intentionally not claimed.
6. Cargo/host execution, serialization, accounts, Solana/SBF, CPI, deployment,
   and every coupled V1 property remain outside this result.
