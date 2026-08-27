# Liability basis V2 kernel

Status: pure semantic and differential slice. It is not a capability release,
Market migration, SBF artifact, or deployment claim.

This crate is a handwritten safe `no_std`, `no_alloc`, runtime-width kernel for
the Lean-owned `LiabilityBasisV2` contract. For an integer payout vector `p`,
positive scale `Q`, supplies `T`, and Hoard collateral `H`:

```text
sum p_i = Q
L(T,p) = sum T_i * p_i
L(T + q*1, p) = L(T,p) + q*Q
H >= L(T,p)  =>  H + q*Q >= L(T + q*1,p)
```

The first concrete evaluator is a two-claim capped ramp. Its one named
apportionment boundary is `capped_ramp_complement_floor_boundary_v2`:

```text
primary    = floor(Q * elapsed / width)
complement = Q - primary
```

Tails clamp to `[0,Q]` and `[Q,0]`. No second rounding or remainder exists.
Categorical claims embed as the runtime-width `Q=1` one-hot profile.

The provisional physical request is exactly 64 bytes. It uses `u32` positive
scale and denominators plus `i64` signed numerators. Checked `i128` cross
products and `u128` interpolation products cover that complete profile. These
are physical representation bounds, not mathematical basis-width limits.

Complete-set merge, claim transfer, and single-claim terminal redemption are
executed by the same runtime-width planner as split. Liability and collateral
move by exactly the same amount in every admitted transition:

```text
split   L(T + q*1, p) = L(T,p) + q*Q      H' = H + q*Q
merge   L(T - q*1, p) + q*Q = L(T,p)      H' + q*Q = H
redeem  L(T - q*e_i, p) + q*p_i = L(T,p)  H' + q*p_i = H
trade   aggregate T unchanged, so L and H are both unchanged
```

`maximum_liability_v2` is the certified pre-resolution envelope `Q * peak(T)`.
Lean bounds exact liability by it for every basis and proves it attained for
both admitted evaluator families, so covering it is solvency at every admitted
terminal result without enumerating the result domain.

`src/generated.rs` contains only Lean-emitted ABI constants plus three corpora:
sixteen agreement cases, nineteen hostile refusal cases, and twenty-four
runtime-width transition cases covering split, merge, and terminal redemption
with every reachable refusal tag. The evaluator, hostile decoder, liability
arithmetic, and transition planners are handwritten Rust. Regeneration must go
to a temporary file and be compared before atomically replacing the accepted
generated file; `check-generated.sh` performs exactly that.

## The degree-1..3 B-spline profile

The second evaluator family is the B-spline basis of degree `1..3`, owned by
`DClutchSemantics.LiabilityBasisV2Spline` and its `PhysicalAbi`. Each
elementary claim is one basis function, so the outstanding supply vector is the
spline's **control polygon** and terminal liability is the spline curve at the
resolved coordinate. `SplineProfile.liability_is_the_control_polygon_curve`
bounds the integer liability against `Q` times that curve by one collateral
atom per outstanding claim.

The capped ramp's single apportionment boundary generalizes rather than being
replaced. `cumulativeFloorBoundaryV2` floors the *running* weight sum and each
claim receives the difference between consecutive floors:

```text
c_j = floor(Q * (w_0 + ... + w_j) / D)      c_{-1} = 0,  c_{K-1} = Q
p_j = c_j - c_{j-1}
```

The partition sum is exact by telescoping at every width, so there is still no
second rounding decision and no residue. `apportion_width_two` proves the
width-two instance is exactly one floor plus its exact complement, and
`cumulativeFloorBoundaryV2_eq_cappedRamp` proves the boundary itself is
`cappedRampComplementFloorBoundaryV2`.

Evaluation is Cox-de-Boor on integers. Level values are `Nat` numerators over
one accumulating denominator, so nonnegativity is structural; one
degree-raising step sends `(q-p)*v` left and `p*v` right and scales the level
by `q`. **No greatest common divisor is ever computed** — weights stay
unreduced and the only division in the whole evaluation is the one floor per
claim. Interior knot multiplicity is admitted: a repeated knot collapses a span,
the locator skips it, and that is also what forces every de Boor denominator
positive without a special case.

`src/generated_spline.rs` is Lean-emitted and holds the ABI constants plus 28
agreement cases and 32 refusal cases reaching all twelve guarded tags.
`check-generated-spline.sh` regenerates to a temporary file and byte-compares.
The evaluator, hostile decoder and apportionment in `src/spline.rs` are
handwritten Rust and agreed with the corpus on the first run.

The provisional physical request is exactly 144 bytes: profile `2` of schema
`2`, a `u32` scale and denominators, an `i64` coordinate numerator, a degree,
and up to twelve `i64` knots over one common denominator — so the widest basis
the record expresses is ten claims, at degree one. Those are physical
representation bounds, not mathematical width or degree limits. Refusal tags
`0`-`6` mean what they mean for the ramp; `15` unsupported degree, `16` knot
count out of range, `17` non-canonical knot padding, `18` knots not
nondecreasing and `19` a degenerate or out-of-domain located span are new. Tag
`11` is the one refusal with no Lean counterpart: Lean is unbounded, the kernel
evaluates in `u128`, and it fails closed.

## The degree-`>= 2` price gate

**A degree-`>= 2` basis must not reach a Market without a price-plane
admission gate.** At degree `>= 2` an interior claim can never pay a whole
complete set — the ceilings are `3/4` and `2/3` — so `p >= 0, sum p = Q` stops
being the no-arbitrage condition on a price vector: *three complete sets, short
four units of the interior claim* has a globally nonnegative payoff and, at the
simplex-admissible price `Q * e_j`, a strictly negative price.

`src/price_gate.rs` is that gate, owned by
`DClutchSemantics.LiabilityBasisV2PriceGate` and its `PhysicalAbi`. It is
**integer hull membership**: a price is admitted when a certificate exhibits it
as a nonnegative integer mixture of actually attainable payout vectors.

```text
0 < W,  every weight positive,  sum weights = W
W * p_i = sum over atoms of weight * evaluate(coordinate)_i   for every claim i
```

Every atom is **recomputed by `evaluate_spline_v2`** and never taken off the
wire. The certificate asks a basis for exactly one thing — a deterministic
integer evaluator whose payouts sum to a fixed scale — so it needs no
uniformity, no knot vector, no degree and no span decomposition, and is
indifferent to every axis on which the spline family is more general than its
predecessors. Lean's `Certificate.no_arbitrage` is the theorem; `price_sum`
proves the simplex condition is a *consequence* of hull membership rather than a
second premise, so the gate can only ever refuse more than `p >= 0, sum p = Q`.

`admit_and_evaluate_spline_v2` is the **admission conjunct**: a request of
degree above `PRICE_GATE_EXEMPT_DEGREE_V1` is evaluated for sale only alongside
a certificate this gate accepts against that same request, and without one it is
refused with tag `31` before evaluation. Degree `<= 1` needs none — every claim
attains a whole complete set at its own knot, so `no_cap_of_attained_scale`
leaves the capped-claim refusal with no instance — but a certificate that *is*
offered is verified regardless of degree, so a present input is never silently
ignored. The certificate is checked against an *already authenticated*
`SplineRequestV2`, never against a digest of one: there is no hash preimage
question and no second copy of the basis to disagree with.

The physical record is exactly 320 bytes: profile `1` of schema `1`, magic
`DCLTPGT1`, a `u32` scale, a `u64` mixture mass, the degree and width it claims
to be for, up to ten `u64` prices, and up to ten atoms as a `u64` weight, an
`i64` coordinate numerator and a `u32` denominator. **Ten atoms is not an
arbitrary capacity**: every payout vector lies in the affine hyperplane
`sum = Q`, of dimension at most `width - 1`, so affine Caratheodory bounds a
hull point's support by the width, and the width is at most ten. New refusal
tags are `20` zero mass, `21` width out of range, `22` atom count out of range,
`23` non-canonical gate padding, `24` zero atom weight, `25` non-canonical atom
order, `26` weight/mass mismatch, `27` non-primitive weight scale, `28` price
not a partition, `29` basis mismatch, `30` price reconstruction mismatch, and
`31` the admission conjunct — which no record can carry, because it fires when
there is no record at all.

`src/generated_price_gate.rs` is Lean-emitted and holds 22 agreement cases and
45 refusal cases reaching all twenty record-carried guarded tags;
`check-generated-price-gate.sh` regenerates to a temporary file and
byte-compares. The corpus carries **both directions of generation two's
adversarial pair**: the live quantized point generation one wrongly refused, and
generation one's own counterexample basis, where the price it wrongly *accepted*
dies at the hull equation while what is genuinely attainable on the same basis
is certified.

**The residual, named.** The mass is a `u64`. A price inside the hull whose
every representation needs a larger common denominator is refused. That is a
sufficient inner certificate and it fails closed. Generation two carried the same
residual and named it; nothing here closes it. See
`docs/research/BSPLINE_ECLIPSE_SCORECARD_2026_08_27.md`,
`docs/compost/PRICE_GATE_HULL_2026_08_27.md`, and `ASPIRATION_LEDGER.md` `G-1`.

The crate is a canonical workspace member. Capability admission and Market
migration remain separately gated work.
