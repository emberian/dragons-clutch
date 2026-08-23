# `clutch-price-measure`

Safe, `no_std`, allocation-free exact price-coherence certificates for the two
different smooth-payout semantics that Dragon's Clutch must not conflate.

## Continuous exact profile

`verify_continuous_price_measure_v2` implements the per-span Bernstein/Hausdorff witness
from `research/price-measure-witness`. It is sound only for a profile whose
payouts are the exact rational open-clamped uniform B-spline values before any
payout quantization. Static transfer tables use the explicitly versioned
universal scales 2 for degree two and 12 for degree three. Tests reproduce
those tables against `clutch-bspline` at exact polynomial nodes for every
supported degree/outcome width.

The continuous verifier must not be wired to today's settlement semantics.
Current `clutch-bspline` evaluates integer coordinates and quantizes weights by
largest remainder. Its attainable price body depends on the exact knots,
spacing, payout denominator, integer domain, and rounding version.

## Current quantized runtime profile

`verify_quantized_price_measure_v2` checks a finite atomic mixture directly
against `BasisSpec::evaluate`. The adapter supplies the owner-checked immutable
`BasisSpec`; the witness binds its full canonical digest and the frozen
largest-remainder/lowest-index-tie semantics. For atoms `(x_a,m_a)`:

```text
sum_a m_a = W
R_i = sum_a m_a * evaluate(x_a)[i]
p_i / S = R_i / (D * W)
```

The last equality compares independently reduced rational pairs, so all `u64`
price, payout, and witness denominators remain inside `u128` without a triple
cross-product. At most `outcome_count` atoms are required by Caratheodory's
theorem because all payout vectors lie in the affine `sum(weights)=D`
hyperplane.

Active atoms must be in the closed knot interval, strictly coordinate-sorted,
positive-mass, primitive as one integer vector, and followed by zero padding.
This canonicalizes one submitted quadrature, not the price: several primitive
quadratures can represent the same price. Witness-body bytes therefore must
not affect candidate identity, scoring, or digest tie-breaking.

## Adapter boundary

The crate compares, but does not compute, digests. A runtime adapter must:

1. owner-check and fully decode the immutable basis and candidate;
2. hash the canonical basis, price vector, and witness body excluding its
   digest field;
3. supply those observed digests through `AdapterBindingsV2`;
4. select the matching continuous or quantized payout semantics; and
5. persist success only in a versioned candidate checkpoint.

No SBF dispatcher, account layout, or profile currently selects this crate.

Reproduce:

```sh
cargo +1.93.1 test --manifest-path crates/clutch-price-measure/Cargo.toml --locked
cargo +1.93.1 clippy --manifest-path crates/clutch-price-measure/Cargo.toml \
  --locked --all-targets -- -D warnings
```
