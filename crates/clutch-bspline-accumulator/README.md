# `clutch-bspline-accumulator`

Pure `no_std`, allocation-free accumulation of canonical native B-spline
payout vectors over equal-duration, adjacent buckets.

This crate owns no source authentication, account parsing, clock, coverage
policy, or Solana adapter. An authenticated caller supplies canonical integer
points or explicit gaps. The domain binds an opaque canonical-grid identity
and one nonzero equal bucket duration. Finalization refuses gaps and either
requires an exactly representable average or applies the separately named
`LargestRemainderV1` rule.

See `docs/implementation/BSPLINE_OCCUPATION_ACCUMULATOR.md` for the semantic
boundary, algebra, measurements, and promotion gates.
