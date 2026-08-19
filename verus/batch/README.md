# Verified scalar batch shadow

`batch.rs` is a checked mathematical shadow of the scalar `FixedBook` relation
in `crates/clutch-batch/src/lib.rs`. With the repository's pinned local Verus
release it reports:

```text
verification results:: 20 verified, 0 errors
```

Reproduce the pinned proof and its four required red mutants offline:

```sh
sh verus/batch/run_batch_proofs.sh
```

The exported theorem inventory is:

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
allocation theorem does not prove production dust-loop progress, its completed
selection count, or its choice of positive-remainder entries. The relation
theorem does not derive the two accepted side equalities, and the padding
theorem does not prove production validation establishes its zero-suffix
premise.

This is not an executable-body refinement result. The runner SHA-256-pins the
mathematical source, the scalar production source, and the precise production
implementations reviewed for correspondence. Pinned digests detect drift; they
do not make that human correspondence proof automatic. The coupled
`relation_v1`, its streaming verifier, pairing feasibility, portfolio legs,
Solana/SBF, accounts, serialization, and deployment are expressly excluded.

See `BATCH_ASSUMPTIONS.md` for the exact mapping, assumptions, and remaining
STOPs. Captured output is in `evidence/batch_proofs.txt`.
