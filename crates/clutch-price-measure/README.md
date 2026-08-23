# `clutch-price-measure`

Safe, `no_std`, allocation-free exact price-coherence certificates for
continuous smooth payouts and production-quantized payout bases.

## Exact positive atom-mixture profile

`verify_quantized_atom_mixture_v1` is the admission-oriented positive
certificate for live degree-two and degree-three quantized splines. It accepts
only prices on the Basis payout-denominator scale and checks the literal
component equations

```text
sum_k weight_k = W
sum_i price_i = D
atom_k = production_largest_remainder(Basis, Terms, coordinate_k)
price_i * W = sum_k weight_k * atom_k[i]
sum_i sum_k weight_k * atom_k[i] = D * W
```

All operations are checked integers. The verifier receives an ephemeral
`BoundQuantizedSplineV1` projected from owner-checked Market, complete Terms,
Basis, edge-registry, and price bodies. The certificate repeats their exact
identities and cannot select a different evaluator, rounding rule, domain,
knot vector, or payout denominator. A pure `BoundQuantizedSplineV1` does not
authenticate those bodies by itself; the account-owning adapter must derive it.

`QuantizedAtomMixtureCertificateV1` is a canonical 544-byte fixed-capacity
body. Active coordinates are strictly increasing and carry positive primitive
weights; inactive slots are zero. Profile V1 admits at most `outcome_count`
coordinates: all payout atoms lie in the affine hyperplane `sum(atom_i) = D`,
of dimension at most `outcome_count - 1`, so affine Caratheodory gives support
at most `outcome_count`. This proves membership in the convex hull of the
actual finite quantized atom set. It neither uses nor accepts the continuous
moment-cone witness and makes no uniqueness or optimality claim.

### Exact bounded atom construction

`solve_quantized_atom_pair_hull_v1` constructs a canonical V1 certificate when
the target is exactly one production atom or an exact rational interpolation
of two atoms in a caller-declared coordinate set. It evaluates the immutable
Basis at each integer coordinate, searches pairs in lexicographic order,
derives a primitive weight from the first differing payout component, checks
every component equation with checked integers, and then calls
`verify_quantized_atom_mixture_v1` on its own result.

There is no new rounding boundary: atom evaluation uses the frozen production
largest-remainder/lowest-index-tie rule selected by V1, and the inverse solve
itself never rounds or approximates. The caller supplies a positive pair-work
limit, and the result distinguishes a found certificate, a complete negative
result for all singleton/pair mixtures in the declared coordinate set, and an
explicitly truncated search. A report separately states whether that set was
every integer in the complete Terms domain.

This constructor does not authenticate the repeated Market, Terms, Basis, or
price identities. An owning adapter must authenticate those bodies before
using its certificate. It also makes no claim about representations requiring
three or more atoms, and its deterministic first solution is not an economic
optimum or a unique representation.

`solve_quantized_atom_support3_hull_v1` extends that search through every
lexicographic coordinate triple under separate pair and triple work limits.
For an affine-independent triple, it derives exact barycentric numerators with
checked signed 2-by-2 determinants, checks the reconstruction in every active
outcome, reduces all masses and their denominator by their exact common gcd,
and independently invokes the same production verifier.

The determinant substrate uses a fixed 2048-bit magnitude so the difference of
two full `u64` products, determinants through the maximum 15-by-15 affine
system, Bareiss intermediates, and subsequent exact reconstruction remain
representable without signed overflow. It uses exact division and binary gcd;
there are no unchecked casts, allocations, floats, or rounding. Exhaustive
outcomes separate no rational singleton/pair/triple, exact triples whose
primitive masses exceed V1's `u64` encoding, and work truncation. The
support-three API makes no statement about representations requiring four
through `outcome_count` atoms.

`solve_quantized_atom_support4_hull_v1` adds a separately bounded
lexicographic quartet search. For each affine-independent quartet it selects
three independent payout equations, derives the four exact barycentric masses,
checks every active payout equation, reduces the complete mass vector, and
passes the constructed certificate through `verify_quantized_atom_mixture_v1`.
The search and verifier continue to share the one named production
largest-remainder/lowest-index-tie payout boundary; the inverse solve adds no
rounding or approximation.

The quartet path uses a crate-internal fixed 3-by-3 matrix and row-pivoted
Bareiss fraction-free elimination over signed 2048-bit magnitudes. The fixed
width covers full-`u64` 3-by-3 determinants and determinant-times-payout
reconstruction; overflow and non-exact divisions refuse explicitly. This is
safe, `no_std`, allocation-free arithmetic rather than a general dynamic
linear-algebra surface.

The support-four outcome keeps four facts distinct: `Solved` means the emitted
certificate passed the independent production verifier; `WorkTruncated` means
one declared family budget ended; `OutOfProfile` means an exact positive
solution needed primitive integers outside the certificate's `u64` encoding;
and `Unsupported` means the declared coordinates were exhausted through
support four without a representable certificate. In particular,
`Unsupported` is not a price-incoherence claim: support five through
`outcome_count` remains unimplemented, and the coordinate report separately
records whether the declared set covered the complete integer Terms domain.
The deterministic first certificate is neither a fair-value nor an optimality
claim.

`solve_quantized_atom_hull_v1` removes the support-four construction ceiling.
It searches support sizes in increasing order through `outcome_count`, with a
positive caller-declared subset budget applied separately to each support
family. A fixed lexicographic combination cursor uses no recursion or
allocation. Each support first passes an exact rectangular fraction-free rank
selection. Affinely dependent supports are skipped soundly: a positive convex
representation on a dependent set can move to a proper face, hence to a
smaller support the increasing search already exhausted. An independent
support uses the selected square subsystem, exact Cramer determinants, a check
of every original payout equation, primitive reduction, and the independent
production verifier.

The largest native system has side `outcome_count - 1 = 15`. Hadamard bounds
its full-`u64` determinants below 1000 bits and the largest pre-division
Bareiss product below 2048 bits; determinant-times-payout reconstruction also
stays below that fixed width. Thus the remaining capability limits are not an
artificial support cap: they are the caller's explicit per-support work bound,
the named primitive-`u64` certificate-mass profile, and any coordinates omitted
from a partial declared set. If every family through `outcome_count` and every
integer Terms coordinate are exhausted, `Unsupported` is a complete finite
quantized-hull negative by affine Caratheodory. Otherwise `WorkTruncated`,
`OutOfProfile`, or a non-full-domain report states the narrower fact.
The large fixed working matrices are constructor machinery; their SBF stack
and compute-unit suitability has not been measured and is not an adapter
admission claim.

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
