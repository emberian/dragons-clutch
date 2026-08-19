# Verified scalar batch shadow

`batch.rs` is a checked mathematical shadow of the scalar `FixedBook` relation
in `crates/clutch-batch/src/lib.rs`. With the repository's pinned local Verus
release it reports:

```text
verification results:: 28 verified, 0 errors
```

Reproduce the pinned proof and its five required red mutants offline:

```sh
sh verus/batch/run_batch_proofs.sh
```

The exported theorem inventory is:

- `dust_loop_has_positive_choice`: derives from quotient/remainder arithmetic
  that every unfinished one-shot dust assignment has an unassigned
  positive-remainder entry;
- `dust_loop_maximal_choice_is_positive`: proves that a maximal unassigned
  entry returned by the production-shaped scan has positive remainder;
- `dust_loop_progress_nonvacuous_example`: checks a concrete two-unit-order,
  one-atom target satisfying the progress theorem premises;
- `allocation_decomposes_and_bounds`: quotient floors sum at most to the target;
  for a caller-supplied one-shot selection mask, modeled fills decompose into
  floor sum plus selection count and are quantity-bounded under an explicit
  positive-remainder premise;
- `choose_tick_deterministic`: every nonempty bounded score grid has one unique
  lexicographic winner under max-volume, min-imbalance, highest-tick ordering;
- `accepted_sides_partition_whole_fill`: takes the accepted buy-equals-matched
  and sell-equals-matched checks as premises, and derives only the whole-fill
  partition identity and its twice-matched consequence;
- `canonical_padding_fold_identity`: takes a canonical zero suffix as a premise
  and proves that it contributes nothing to a full fixed-array fold.

The proof unit has no project assumptions or axioms. Its premises describe
model facts and are visible in the theorem signatures. In particular, the
two dust lemmas establish mathematical progress and positivity for a
production-shaped one-shot assignment state. They do not verify the executable
loop, its `left`/`assigned` invariant, or the scan implementation. The relation
theorem does not derive the two accepted side equalities, and the padding
theorem does not prove production validation establishes its zero-suffix
premise.

This is not an executable-body refinement result. The runner SHA-256-pins the
mathematical source, the scalar production source, the exact `allocate_side`
body, and the precise production implementations reviewed for correspondence.
Pinned digests detect drift; they do not make that human correspondence proof
automatic. The coupled
`relation_v1`, its streaming verifier, pairing feasibility, portfolio legs,
Solana/SBF, accounts, serialization, and deployment are expressly excluded.

See `BATCH_ASSUMPTIONS.md` for the exact mapping, assumptions, and remaining
STOPs. Captured output is in `evidence/batch_proofs.txt`.
