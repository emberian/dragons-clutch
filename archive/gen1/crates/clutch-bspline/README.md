# `clutch-bspline`

This crate is the pure semantic owner of Dragon's Clutch's frozen payout-basis
evaluation. It is safe Rust, `no_std`, allocation-free, float-free, dependency-
free, and total over hostile `BasisSpec` values.

It implements:

- degree 0 categorical cells with a closed top;
- degree 1 hat functions on uniform or non-uniform anchors; the earlier
  host-only directional rounding vectors are intentionally superseded by the
  same lower-error largest-remainder rule used for every smooth degree;
- degree 2 and 3 open-clamped uniform B-splines, including every end pane;
- canonical `WEIGHT-ROUND-01`: floor every exact scaled coefficient, then
  distribute the at-most-`degree` remaining atoms to the largest fractional
  remainders, with lowest outcome index breaking exact ties;
- checked `u128` arithmetic, canonical padding and count refusals, exact
  partition of unity, and support of at most `degree + 1` claims.

`oracle/check.py` is an independent Python `Fraction`/Cox-de-Boor differential
oracle. It checks exhaustive bounded domains, deterministic random cases, and
mutants for independent rounding, non-clamped edge formulas, open-top handling,
and the incompatible degree-1 rounding direction. The Rust implementation does
not use the oracle algorithm: it uses the bounded basis-functions recurrence
with a reduced exact-rational type.

The crate knows no Solana accounts, evidence source, statistic, Token-2022
state, clock, signer, or CPI. A caller must bind a validated spec and resolved
value to immutable authenticated protocol state. Tests and an oracle are not a
formal proof.

## Native basis, coefficient algebra, and compatibility lowering

The B-spline basis is native settlement semantics. The next two layers must not
be conflated with it or with each other:

- **Native basis:** degree-zero categorical Eggs, degree-one hats, and
  quadratic/cubic smooth Eggs. Consensus derives these settlement weights.
- **Exact coefficient algebra:** range/tail exposure, triangles, capped-linear
  calls/puts/spreads, and any other curve genuinely in the selected finite
  spline span are canonical coefficient vectors over native Eggs. Gaussian or
  proximity curves, arbitrary sampled payoffs, market-implied continuous
  histograms, curve-builder identities, and LP range policies also live here
  when accompanied by the required exact identity or approximation certificate.
- **Categorical compatibility lowering:** sampling or integrating a shaped
  curve over degree-zero Eggs is an explicitly approximate interoperability
  path unless the target is already a categorical step function.

Exact coefficient algebra inherits the native basis's interpolation and
settlement rule; compatibility lowering does not replace that rule. Compiler
identity, atomic position semantics, and approximation certificates remain
separate from an exact coefficient vector.
