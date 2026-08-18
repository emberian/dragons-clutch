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
