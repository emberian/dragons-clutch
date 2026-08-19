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

- `allocate_conserves`: a successful positive-total quotient/remainder
  allocation sums exactly to its target and every fill lies between zero and
  its order quantity;
- `choose_tick_deterministic`: every nonempty bounded score grid has one unique
  lexicographic winner under max-volume, min-imbalance, highest-tick ordering;
- `relation_conserves`: successful scalar verification makes the buy and sell
  fill folds both equal `matched`, while their partition equals twice matched;
- `canonical_padding_zero`: validated zero padding contributes nothing to a
  full fixed-array fold.

The proof unit has no project assumptions or axioms. Its premises describe
successful production seams and are visible in the theorem signatures. In
particular, the allocation theorem does not prove that the production dust loop
always finds the required distinct positive-remainder entries.

This is not an executable-body refinement result. The runner SHA-256-pins the
mathematical source, the scalar production source, and the precise production
implementations reviewed for correspondence. Pinned digests detect drift; they
do not make that human correspondence proof automatic. The coupled
`relation_v1`, its streaming verifier, pairing feasibility, portfolio legs,
Solana/SBF, accounts, serialization, and deployment are expressly excluded.

See `BATCH_ASSUMPTIONS.md` for the exact mapping, assumptions, and remaining
STOPs. Captured output is in `evidence/batch_proofs.txt`.
