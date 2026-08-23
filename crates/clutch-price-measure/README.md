# `clutch-price-measure`

Safe, `no_std`, allocation-free exact price-coherence certificates for
continuous smooth payouts and production-quantized payout bases.

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

## Frozen V2 quantized profile

`verify_quantized_price_measure_v2` checks a finite atomic mixture directly
against `BasisSpec::evaluate`. The adapter supplies the owner-checked immutable
`BasisSpec`; the witness binds its full canonical digest and the frozen
largest-remainder/lowest-index-tie semantics. V2 remains restricted to degrees
two and three. Its public structures, refusal order, and valid/invalid behavior
are unchanged by the additive V3 interface. For atoms `(x_a,m_a)`:

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

## Additive V3 quantized profile

V3 has two deliberately separate entry points with no caller-selected
evaluator enum:

- `verify_quantized_price_measure_v3_degree_zero` consumes a
  `DegreeZeroPayoutTableV3` finite native-claim geometry;
- `verify_quantized_price_measure_v3_smooth` consumes a `BasisSpec` of degree
  one through three.

`QuantizedPriceMeasureAccumulatorV3::begin_degree_zero` and `begin_smooth`
provide the matching staged interfaces. The witness repeats its degree and
native outcome width and carries two exact semantic markers:

```text
PRICE_MEASURE_WITNESS_VERSION_V3 = 3
QUANTIZED_PRICE_MEASURE_SEMANTICS_VERSION_V1 = 1
```

The second marker freezes exact mapped payout rows at degree zero;
`clutch-bspline` integer-grid evaluation with largest-remainder/lowest-index
ties at degrees one through three; exact upstream simplex prices; strict
coordinate order; positive primitive mass; zero padding; and reduced-rational
reconstruction. There are no caller-selectable V3 evaluator or rounding
variants.

A degree-zero table has `2..=16` ordered coordinate cells and native claims,
`outcome_count - 1` strictly increasing interior knots, and
`1..=outcome_count` distinct exact simplex payout rows. Its canonical first-use
map selects a row for each cell; several cells may select the same row, payout
rows need not be one-hot, and `payout_count` need not equal `outcome_count`.
Coordinates are restricted to the adapter-supplied closed
`domain_min..=domain_max`, with knot equality selecting the cell on the right.
Rows use one exact positive payout denominator; there is no degree-zero payout
rounding boundary.

Degree-one bases use `outcome_count` stored knots and may use either a valid
nonuniform declaration or an exact declared power-of-two spacing. V3 restricts
certified atoms to the closed stored-knot interval even if the `BasisSpec` edge
policy would clamp. Degrees two and three retain the existing uniform-basis
requirement and the same closed-knot atom interval. Every smooth degree
requires `2..=16` native outcomes and at least `degree + 1` outcomes.

The support cap remains `MAX_QUANTIZED_ATOMS = MAX_OUTCOMES = 16`; the primitive
mass denominator admits the full positive `u64` range. Tests exhaust every
single atom and every primitive two-atom mixture over small finite degree-zero
and uniform/nonuniform degree-one domains. Degree-zero fixtures include
arbitrary non-one-hot rows, repeated map targets, and
`payout_count != native_outcome_count`. The complete V2 adversarial suite and a
frozen valid/low-degree regression remain compatibility gates.

The staged V3 accumulator borrows the authenticated finite table, price vector,
and witness instead of copying them. It retains only the validated smooth basis
when that path is selected. A compile-time assertion and a runtime regression
cap the accumulator type at 1,536 bytes, comfortably below the SBF 4,096-byte
frame ceiling. Each append deliberately stages one 128-byte payout vector and
one 256-byte accumulator vector so refusal remains transactional. These source
layout bounds do not replace an SBF artifact stack-frame inspection and runtime
campaign when an adapter begins calling this interface.

## Adapter boundary

The crate compares, but does not compute, digests. A runtime adapter must:

1. owner-check and fully decode the immutable basis, relation/domain, and
   candidate;
2. hash the adapter-owned canonical basis, price vector, and witness-body bytes
   (excluding the witness digest field);
3. supply those observed digests through the matching `AdapterBindingsV2` or
   `AdapterBindingsV3`;
4. select the matching continuous or quantized payout semantics; and
5. persist success only in a versioned candidate checkpoint.

For V3, `basis_digest` is the canonical `NativeClaimBasisV1Id`. It owns rows,
map, knots, payout denominator, and the immutable edge/ambiguity registry
selectors. `relation_domain_digest` owns coordinate bounds and binds that exact
basis identity. The ephemeral `DegreeZeroPayoutTableV3` combines the two
validated projections for arithmetic; it is not a new persisted truth and has
no combined table digest.

The same V3 rule applies to smooth evaluation: the adapter authenticates the
Product basis artifact and relation/domain separately, then constructs an
ephemeral `BasisSpec`. `basis_digest` is not a digest of that combined
`BasisSpec`. Before calling the checker, it must prove that every Product-owned
field, including the registry selectors, matches the basis artifact; that the
coordinate bounds match the relation/domain; and that the relation/domain
binds the exact basis ID. Frozen V2 retains its historical full-`BasisSpec`
digest contract. This crate neither defines canonical witness bytes nor
computes their digest; both are adapter-owned. Cloning or forking an in-memory
accumulator carries no persisted authority.

No SBF dispatcher, account layout, or profile currently selects this crate.

Reproduce:

```sh
cargo +1.93.1 test --manifest-path crates/clutch-price-measure/Cargo.toml --locked
cargo +1.93.1 clippy --manifest-path crates/clutch-price-measure/Cargo.toml \
  --locked --all-targets -- -D warnings
```
