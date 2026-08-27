# Certificate derivation and trust boundary

This note states the mathematical claims implemented by the research compiler.
It is explanatory evidence, not a machine-checked proof.

## 1. Convex-hull payout bound

For admitted basis functions `B_i(x)` and coefficients `0 <= a_i <= H`, the
native payout is

```text
S(x) = sum_i a_i B_i(x).
```

The basis evaluator's intended invariant is `B_i(x) >= 0` and
`sum_i B_i(x) = 1`. Therefore `0 <= S(x) <= H`. With integer weights `w_i`
whose exact sum is `D`, consensus computes `sum_i a_i w_i / D`, which has the
same convex-hull bound even though it differs from the unquantized spline.

This compiler validates the coefficient interval. It relies on the selected
`clutch-bspline` semantics for the basis and integer-weight invariants.

## 2. Exact span classifications

- Degree-zero step functions are exact when every effective interior jump is a
  cell boundary and values at every boundary agree with the categorical
  right-hand/closed-top convention.
- A continuous piecewise-linear function is exact in a degree-one open-clamped
  basis when every effective interior kink is a knot. Degree-one control values
  are the target's knot values.
- Every B-spline degree reproduces affine functions when control coefficients
  are target values at the Greville abscissae. Constants are included.

No other target in the current public `Shape` enum is promoted to exact status.
In particular, simple-knot quadratic and cubic spaces do not contain a hard
jump or a non-differentiable triangle/call-spread corner.

## 3. Piecewise-rational enclosure

On a leaf interval containing no target or basis discontinuity, let
`e(x) = |f(x) - S(x)|`. If `L` bounds `|f'| + |S'|`, then `e` is `L`-Lipschitz.
At leaf midpoint `m` and width `w`:

```text
max(0, e(m) - L w/2) <= e(x) <= e(m) + L w/2.
```

Multiplying these bounds by `w` bounds the leaf's L1 contribution. Taking the
maximum of the midpoint errors supplies a sup lower bound; taking the maximum
of leaf upper bounds supplies a sup upper bound. All arithmetic is rational.

The derivative control points of a degree-`p` B-spline are

```text
p (a_(i+1) - a_i) / (t_(i+p+1) - t_(i+1)).
```

The derivative curve is a convex combination of these values, so the maximum
absolute derivative control point bounds `|S'|`. Intervals are split at all
knots before using this bound. Discontinuity points are evaluated separately
for the sup norm; isolated points have zero L1 measure.

## 4. Gaussian coefficient enclosure

For `z >= 0`, the alternating Taylor series for `exp(-z)` encloses the result
between an odd and the following even truncation when range reduction has made
`z <= 1/2`. Positive squaring preserves interval order. To bound exact-rational
growth, after a frozen number of reductions the compiler instead uses

```text
e^(z/m) >= 1 + z/m
=> e^z >= (1 + z/m)^m
=> e^-z <= (1 + z/m)^-m.
```

The lower bound zero and this rational upper bound are sufficient to choose a
rational midpoint coefficient with a known error radius.

For the Gaussian target, `|f'| <= H/sigma`. If `g_i` is Greville site `i`,
`B_i(x)` is active only on its support, and every coefficient sample error is
at most `epsilon`, then

```text
|f(x) - sum_i a_i B_i(x)|
 <= sum_i B_i(x) (|f(x)-f(g_i)| + |f(g_i)-a_i|)
 <= (H/sigma) rho + epsilon,
```

where `rho` is the maximum site-to-support-endpoint distance.

## 5. Consensus weight quantization

`WEIGHT-ROUND-01` changes at most `degree + 1` local weights and preserves their
sum. The compiler currently reports the deliberately conservative bound

```text
H * degree / D
```

for the induced payout error. It reports this independently from analytic
approximation error so a consumer cannot confuse a better shape compiler with
a larger consensus denominator.

## 6. Unverified boundary

The claims above assume the basis description admitted by `clutch-bspline`, the
meaning of its open-clamped knots, its exact partition and rounding invariants,
and ordinary correctness of `num-bigint`/`num-rational`. No theorem currently
connects this host compiler, a serialized artifact, an SBF parser, authenticated
resolution evidence, token supply, and payout transfer in one verified chain.

The V1 serialization in `src/artifact.rs` closes byte malleability at the host
compiler/client seam and requires exact recompilation on Rust decode. It does
not close this end-to-end boundary: current Terms binds the native basis but no
shape-certificate digest, and SBF neither parses nor persists the certificate.
