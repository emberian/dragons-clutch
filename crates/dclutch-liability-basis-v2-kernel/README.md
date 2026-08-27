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

**A degree-`>= 2` basis must not reach a Market without a price-plane
admission gate.** At degree `>= 2` an interior claim can never pay a whole
complete set — the ceilings are `3/4` and `2/3` — so `p >= 0, sum p = Q` stops
being the no-arbitrage condition on a price vector. Nothing in this crate gates
prices. See `docs/research/BSPLINE_ECLIPSE_SCORECARD_2026_08_27.md` and
`ASPIRATION_LEDGER.md` `G-1`.

The crate is a canonical workspace member. Capability admission and Market
migration remain separately gated work.
