# Price-measure witness V2

Status: **EXACT FINITE CERTIFICATE IN GENERAL V2 SOURCE ADMISSION /
NON-PRODUCTION PROFILE ONLY**.

`crates/clutch-price-measure` implements two deliberately separate exact
certificate interfaces. The continuous per-span Bernstein witness reproduces
`research/price-measure-witness`. The finite atom witness targets the actual
integer-coordinate, largest-remainder-quantized `clutch-bspline` payout map.
Both are safe, `no_std`, allocation-free, fixed-capacity Rust. The isolated
General SBF source now selects the finite production atom-mixture checker before
creating resumable work and on its empty-book completion path. It persists no
parallel verified-price truth; the immutable feed/body/price/policy identities
are the resumed-work binding. Production capability profiles remain disabled.

`crates/clutch-general-v2-runtime` now composes the V3 quantized checker with
the sealed General V2 feed codec, exact Product V2 bodies, canonical
PriceGrid membership, owner-blind RelationV2, and ScoreV2-Q for smooth degrees
two and three. That composition freezes the canonical fixed-width V3
witness-body digest and keeps it outside the economic candidate identity and
rank. A successor RelationV2 policy digest commits the exact finite-certificate
profile, and a private authority minted from the checked certificate is now
required before the builder or public sealed-feed API can invoke successor
ranking.

## 1. Critical semantic split

The continuous exact B-spline curve and today's settlement payout map do not
have the same moment body.

The research theorem uses exact rational basis values `N(x)` over a continuous
coordinate. Production settlement instead:

1. admits integer `u128` resolved coordinates;
2. evaluates an exact `BasisSpec`;
3. scales to its configured payout denominator `D`; and
4. applies largest remainder, with exact ties to the lowest outcome index.

The production price body is therefore the finite polytope

```text
conv { evaluate_and_quantize(x) : x is an integer in the stored knot interval }.
```

It depends on the exact knots, spacing, integer domain, payout denominator,
edge policy, evaluator version, and rounding version—not only on
`(degree,outcome_count)`. The continuous theorem that no finite linear family
decides membership does not describe this finite runtime polytope.

Two executable regressions pin both directions.

First, this is an exact coherent production point price:

```text
degree = 2
knots = [0,128,256,384]
D = S = 10,000
x = 85
evaluate(x) = (1128,6667,2205,0,0)
```

V1b refuses it because `3*6667 = 20001 > 2*10000`. Thus a V1b refusal does
not universally exhibit a runtime arbitrage under quantized settlement.

Second, single-span degree two on integer knots `[0,1]` has only its two
endpoint runtime vertices. The continuous Hankel vector `(1,2,1)/4` is in the
continuous moment body, but it is outside the runtime hull, whose middle
coordinate is always zero. Thus continuous exact admission is not a sufficient
runtime certificate either.

The continuous checker consequently accepts only
`ExactUnquantizedV1`. The runtime checker accepts only
`LargestRemainderLowestIndexV1` and recomputes every payout atom through the
production evaluator.

## 2. Continuous exact Bernstein witness

For the market's immutable open-clamped uniform basis, let:

- `d` be degree two or three;
- `n <= 16` be the outcome count;
- `K = n + 1 - d` be the distinct-breakpoint count;
- `S` be the exact integer candidate-price scale;
- `p_i` be exact integer simplex prices; and
- `T_k` express the B-splines active on span `k` in that span's Bernstein basis.

The candidate supplies nonnegative moment rows over a common denominator `W`:

```text
w[k][r] / W = integral_span_k BernsteinBasis[r] dQ.
```

The verifier requires:

```text
sum(k,r) w[k][r] = W
p_i / S = sum(k,r) T_k[i,r] * w[k][r] / W

degree 2: w[k][1]^2 <= 4*w[k][0]*w[k][2]

degree 3: w[k][1]^2 <= 3*w[k][0]*w[k][2]
          w[k][2]^2 <= 3*w[k][1]*w[k][3]
```

These are the exact truncated Hausdorff constraints in Bernstein coordinates.
For exact continuous payout semantics, every accepted witness constructs a
representing nonnegative measure. This is a price-coherence certificate, never
an optimality, candidate-ranking, identity, or solver-compensation certificate.

### Generated transfer tables

The program supplies every transfer coefficient. Callers never do. The frozen
universal denominator is 2 for degree two and 12 for degree three. These scales
are exact and all numerators are nonnegative; they are intentionally not
lowest-term on every short grid:

```text
degree 2 minimal scale: n=3 -> 1; n>=4 -> 2
degree 3 minimal scale: n=4 -> 1; n=5 -> 4; n>=6 -> 12
```

The versioned universal scale avoids shape-dependent arithmetic. Executable
tests compare every table against `clutch-bspline` at five exact polynomial
nodes on every span for every admitted degree/outcome width. Column sums equal
the table denominator, adjacent endpoints agree, and inactive rows/columns are
zero.

### Exact integer boundary

For transfer numerators `A` and denominator `tau`, the checker compares:

```text
p_i / S == sum(k,r) A[k][i,r]*w[k][r] / (tau*W).
```

It independently reduces both rational pairs by gcd rather than forming the
three-factor cross-product. With `sum w=W`, every accumulator is bounded by
`12*W`; the full `u64` range of `W` remains inside `u128`. A rational witness
could still require a denominator larger than `u64`, so the implementation is
a sufficient inner certificate until a constructive lattice bound is proved.

The encoding requires zero inactive padding and
`gcd(W, all active moments)=1`. This canonicalizes the integer scale of one
moment witness. It does not select a unique witness for one price: boundary
mass and other decompositions can remain nonunique. Witness-body bytes must not
affect candidate identity, scoring, or digest tie-breaking.

### Named continuous V1b false acceptance

On the degree-two, five-claim continuous grid with breakpoints `[0,1,2,3]`:

```text
p/S = (1/3,2/3,0,0,0)
c   = (1,-2,10,40,64)
```

V1b accepts at its claim-one ceiling and butterfly boundaries, while

```text
sum_i c_i N_i(x) = (3x-1)^2 >= 0
dot(c,p) = -S.
```

The Rust continuous checker refuses the forced moment row `(1,2,0)/3` at its
quadratic constraint. This is a continuous-exact separating portfolio, not an
unconditional production-settlement claim after payout quantization.

## 3. Current-runtime quantized atom witness

Let `v(x)` be the exact integer vector returned by the owner-checked immutable
`BasisSpec` at integer coordinate `x`, with `sum_i v_i(x)=D`. The certificate
supplies sorted atoms `(x_a,m_a)` and positive denominator `W`:

```text
sum_a m_a = W
R_i = sum_a m_a * v_i(x_a)
p_i / S = R_i / (D*W).
```

All runtime vertices lie in the affine hyperplane `sum v=D`, whose dimension
is at most `n-1`. Caratheodory therefore bounds support by
`n=outcome_count`. Because the vertex set is finite and integral, a rational
price in its hull has a rational basic feasible representation with at most
`n` atoms. The fixed `u64` mass denominator remains an inner-certificate bound;
support-boundedness alone does not prove that every lattice price has such a
small denominator.

### Runtime canonicality and arithmetic

The checker requires:

- `1 <= atom_count <= outcome_count`;
- coordinates inside the closed stored-knot interval, even for `Clamp`;
- strictly increasing coordinates and positive masses;
- `sum masses=W` and `gcd(W,masses)=1`;
- zero inactive coordinate/mass slots;
- an exact integer simplex price vector with zero padding; and
- adapter-authenticated basis, price, relation-domain, candidate, and body
  digests.

Every payout vector is recomputed through `BasisSpec::evaluate`; it is never
caller supplied. After validating total mass, `R_i <= D*W` and `D*W` fits
`u128` for any two `u64` operands. Reduced rational-pair comparison avoids any
triple product with `S`.

These rules canonicalize one submitted quadrature, not one price. For degree
two, knots `[0,4,8,12]`, and `D=8`:

```text
v(5)=(0,2,6,0,0)
v(6)=(0,1,6,1,0)
v(7)=(0,0,6,2,0)
```

Both the one-atom witness at 6 and the equal two-atom mixture at 5 and 7 are
primitive certificates for the same price. The body digest authenticates the
chosen sidecar but cannot enter the candidate's economic or tie-breaking key.

## 4. Adapter certificate interface

`AdapterBindingsV2` is a typed trust-boundary input, not an account layout. The
General successor adapter derives from owner-checked immutable state:

- the candidate-feed identity;
- the relation-domain digest;
- the exact basis digest, covering knots, spacing, domain, edge policy, payout
  denominator, evaluator version, and rounding version;
- the exact candidate-price digest; and
- the observed digest of the canonical witness body excluding its digest field.

The price-measure crate compares those bytes and validates the supplied
`BasisSpec`, but does not implement a hash, parser, PDA rule, lifecycle, or
account mutation. General's runtime seam now owns that adapter work: it decodes
the exact sealed feed, rederives the domain/body/price identities, joins the
MarketBinding and NativeClaimBasis, and admits only the current authenticated
Clamp registry selector. A witness may authenticate a candidate but does not
redefine it.

At maximum width the continuous body has 14 stride-four rows, or 56 `u64`
cells. The quantized body has 16 `(u128 coordinate,u64 mass)` slots. Exact
account size, rent, CU, streaming layout, and close obligations remain
unmeasured.

## 5. Profiles and compatibility

- Degree zero/one requires a separately analyzed simplex or finite-atom rule;
  this crate's V2 interfaces deliberately accept only degrees two and three.
- `continuous-exact-v2` may use the Bernstein witness only if settlement also
  uses the same exact unquantized payout semantics.
- `quantized-integer-grid-v2` uses the finite atom witness bound to the exact
  production `BasisSpec` and largest-remainder version.
- Existing `certified-refusal-v1b` is neither exact nor one-sided for general
  quantized runtime profiles. Its old “every refusal is an arbitrage” and
  “single-span exact” descriptions apply only to the continuous model.

Capability profiles must name which of these payout bodies they trade. A
continuous witness cannot be silently interpreted as evidence about quantized
settlement, or vice versa.

## 6. Remaining promotion gates

Before a production SBF profile selects the finite checker:

1. atomically adopt the staged 17-account nonempty Work tuple in shared
   account-meta/capability ownership and retain its checked identities through
   completion;
2. independently generate the transfer templates and derivation manifest;
3. add a solver that emits exact continuous moments or quantized atoms without
   treating floating residuals as consensus evidence;
4. decide whether the `u64` denominator lattice is a deliberate sufficient
   inner profile or prove a constructive completeness bound;
5. measure host, SBF, streamed-resume, stack, account-rent, and close costs;
6. carry the successor policy through selected-candidate and settlement
   lifecycle joins without letting an incomplete sidecar evict a verified
   candidate;
7. keep witness bytes outside score and digest tie-breaking despite
   representation nonuniqueness;
8. update or retire V1b claims and profiles against the actual payout map; and
9. extend Lean only for the exact checker correspondence proved—never for an
   unverified adapter/runtime boundary.

Reproduce the isolated implementation:

```sh
cargo +1.93.1 test --manifest-path crates/clutch-price-measure/Cargo.toml --locked
cargo +1.93.1 clippy --manifest-path crates/clutch-price-measure/Cargo.toml \
  --locked --all-targets -- -D warnings
```
