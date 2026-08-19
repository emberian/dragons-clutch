# Native B-spline shape compiler

Status: **offline research tool; not consensus code**.

This crate compiles bounded payout descriptions into coefficient vectors over
the exact open-clamped degree-0 through degree-3 basis owned by
`clutch-bspline`. It exists to keep three statements distinct:

1. a target payoff is exactly in the selected finite spline span;
2. a bounded coefficient vector defines a native spline payout but only
   approximates the named analytic target; and
3. a degree-zero categorical basket is a compatibility lowering, not the
   definition of native smooth settlement.

All construction and certification arithmetic is `BigRational`. Floating point
is available only through an explicitly display-only helper.

`src/artifact.rs` gives this host compiler a canonical, domain-separated
BasisSpec and shape-certificate byte boundary, plus the exact live typed Terms
upload and CreateMarket intents. Rust decoding recompiles the certificate.
Current Terms does not commit the certificate digest and the on-chain program
does not parse it, so the certificate remains offline evidence.

## Supported families

| Family | Exact cases | Other cases |
| --- | --- | --- |
| hard range and upper/lower tail | aligned degree-zero cells, including the frozen closed-top convention | certified native quasi-interpolant |
| triangle/tent | degree-one basis with every interior kink frozen as a knot; globally affine restrictions in every smooth degree | certified native quasi-interpolant |
| capped call/call spread | degree-one with aligned kinks; globally affine restrictions in every smooth degree | certified native quasi-interpolant |
| capped put/put spread | degree-one with aligned kinks; globally affine restrictions in every smooth degree | certified native quasi-interpolant |
| Gaussian proximity kernel | never labeled exact in the finite polynomial spline span | rationally enclosed Greville samples plus a global error certificate |

`CappedCall { low, high, height }` is the bounded call-spread ramp: zero through
`low`, affine on `(low, high)`, and `height` from `high` onward. `CappedPut` is
its reflected put-spread form. Unbounded calls and puts are intentionally not a
valid claim family because they do not have a frozen maximum liability.

## Liability rule

Every emitted coefficient is checked in `[0, H]`. Since the native basis is
nonnegative and sums exactly to one, its unquantized payout is a convex
combination of those coefficients and therefore also lies in `[0, H]`.
Consensus basis weights independently sum to denominator `D`, so the same bound
holds after `WEIGHT-ROUND-01`; the certificate separately reports the maximum
target error introduced by weight quantization.

This is a bounded-liability argument, not an end-to-end solvency proof. Custody,
mint supply, coefficient artifact admission, settlement authorization, and
redemption remain outside this crate.

## Certificates

`Compilation` reports:

- `SpanStatus::ExactInSpan` only for a construction with an algebraic identity;
- otherwise `CertifiedApproximation`;
- rational lower and upper enclosures for the spline error's sup and L1 norms;
- a conservative consensus-weight quantization term;
- the combined target-versus-consensus upper bounds; and
- the rational enclosure error used to sample a Gaussian coefficient.

Piecewise-rational targets are split at both basis breaks and target breaks.
Each resulting interval is subdivided to a frozen depth. Exact midpoint error
and an exact rational Lipschitz radius enclose every point in that leaf. Values
at discontinuities are checked separately, which is essential because the
native domain has a closed top.

For a Gaussian `H exp(-(x-c)^2/(2 sigma^2))`, each coefficient sample is enclosed
without floating point. Near the center, an alternating Taylor enclosure is
range-reduced and squared. Far in the tail, the compiler avoids exponentially
large rationals using

```text
exp(-z) <= (1 + z/m)^(-m),  m = 32.
```

The global approximation bound uses `|f'| <= H/sigma`, the maximum distance
between an active basis function's support and its Greville site, and the
coefficient enclosure error.

`compare_categorical_lowering` compiles the same shape against a native basis
and a degree-zero basis with the identical closed domain. It then produces
direct rational sup/L1 bounds between the two payout functions, plus a
consensus-quantized upper bound. This makes compatibility loss measurable rather
than rhetorical.

## Why quadratic and cubic control values are not interpolation

For degree one, coefficients at knot/Greville sites interpolate a continuous
piecewise-linear target when all kinks align. For degree two and three,
arbitrary values sampled at Greville sites are **not** interpolation
coefficients. The compiler calls that construction a Schoenberg-Greville
quasi-interpolant and certifies its error. It promotes only constants and affine
restrictions to exact status, using the B-spline affine-reproduction identity.

Simple interior knots also mean quadratic/cubic splines have continuity that a
hard jump, triangle corner, or call-spread corner does not. Those targets cannot
be exact merely because their breakpoints happen to be knots.

## Checks

```sh
cargo test --manifest-path research/bspline-shape-compiler/Cargo.toml
cargo clippy --manifest-path research/bspline-shape-compiler/Cargo.toml \
  --all-targets -- -D warnings
```

The tests cover exact categorical ranges, the closed-top counterexample,
degree-one exact tents and spreads, degree-2/3 affine reproduction, refusal to
call degree-2/3 samples interpolation, coefficient convex-hull bounds, narrow
and edge-centered Gaussians, extreme distance/sigma ratios, categorical
comparison, low-denominator consensus quantization, and dense exact-rational
adversarial points across all piecewise families and degrees.

`tests/native_artifact.rs` additionally covers every degree and shape tag,
canonical rationals/digests, Terms projection, the nine-write artifact upload,
exact intent decoding, and the Rust-generated fixture consumed by the static
client.

## Deliberate limits

- The canonical certificate is a host artifact, not an on-chain coefficient
  parser, account type, or Terms commitment.
- It does not prove that a named transferable shaped position remains atomic.
- It does not turn a coefficient vector into externally materialized Eggs.
- It does not certify an arbitrary user-provided coefficient vector's claimed
  analytic meaning.
- The fixed subdivision certificate is conservative, not a best approximation
  solver and not an optimality claim.
- Tests support these claims but are not a formal proof.
