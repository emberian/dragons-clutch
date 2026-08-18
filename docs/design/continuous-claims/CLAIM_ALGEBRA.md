# Continuous-claim algebra

Status: **PROPOSED integration specification**. The underlying finite payout
vector and derived-basis kernel paths are IMPLEMENTED; the analytic compilers
and certificates below are not yet a release claim. The detailed B-spline
design remains [`../../implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md`](../../implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md).

## Consensus object

Let `Omega` be the frozen finite evidence-result domain and let the `n` canonical
Eggs have payout weights

```text
w(omega) = (w_0,...,w_{n-1}),
0 <= w_i <= D,
sum_i w_i = D.
```

One atom of Egg `i` pays `w_i/D` collateral atoms subject to the kernel's exact-
division/refusal rule. The consensus object is the exact integer vector and its
terms digest—not the words “continuous,” “Gaussian,” or “range.”

A derived claim is a nonnegative coefficient vector `a in [0,S]^n`. Holding
`a_i` atoms of Egg `i` produces terminal payout

```text
P_a(omega) = sum_i a_i w_i(omega)/D.
```

No new liability is created when the claim is represented as a portfolio. A
separately transferable wrapper must escrow the exact basket or enter the
authoritative supply ledger.

## Solvency theorem

Let `T_i` be the aggregate issued supply of Egg `i`. Because `w/D` lies in the
simplex,

```text
sum_i T_i w_i/D <= max_i T_i.
```

Therefore a Hoard containing at least `max_i T_i` collateral atoms covers every
admitted payout vector. This theorem is independent of price formation and of a
probability interpretation. Fees, keeper budgets, LP reserves, and insurance do
not count as Hoard collateral.

The stronger exact complete-set identity is

```text
P_(q,...,q)(omega) = q
```

for every `omega`. It licenses split/merge and complete-set redemption without
an oracle probability model.

## Range and graded compilation

For an outcome interval partition, a hard range is an indicator coefficient
vector. For a degree-1 hat/B-spline basis, sampled coefficients of any bounded
continuous target `f` produce its piecewise-linear interpolant. Range, tail,
call, put, triangular, capped-linear, and Gaussian-like curves are all instances.

The compiler must emit:

```text
ClaimArtifactV1 {
  market_terms_digest,
  basis_kind,
  knots_or_cells_digest,
  coefficient_denominator,
  coefficients[MAX_OUTCOMES],
  analytic_source_digest,
  rounding_rule,
  approximation_norm,
  approximation_bound,
  compiler_version,
}
```

All fields are canonical bytes. A claim is admitted only if coefficients are
bounded, padding is zero, arithmetic fits the frozen types, and the artifact
recomputes under the named compiler.

## Approximation contract

“Subsumes continuous claims” means parameterized finite approximation:

```text
sup_x |f(x) - f_hat(x)| <= epsilon_basis + epsilon_coeff.
```

The certificate must cover the full admitted interval, including between-grid
maxima and edge behavior. Sampling only knots or bin centers is insufficient.
Allowed proof methods include an exact piecewise analysis, interval enclosure,
or a conservative derivative/curvature bound. Refining the grid is a new market
terms version; it may not change a live claim's payout.

## Named rounding boundary

Independent rounding can destroy `sum w_i = D`. The canonical basis evaluator
floors all but the highest-index nonzero weight and derives the last as

```text
w_last = D - sum(previous weights).
```

This preserves nonnegativity and exact partition of unity. Portfolio coefficient
quantization has its own named rounding rule and beneficiary; dust never silently
flows to treasury.

## Refusals

Refuse unknown basis/compiler versions, noncanonical padding, negative or
unbounded coefficients, zero denominators, arithmetic overflow, unproved edge
policies, non-total evidence mappings, excess approximation error, and a wrapper
whose escrow basket cannot be matched exactly.

## Verification targets

1. exhaustive partition-of-unity checks at bounded domains;
2. theorem proof for nonnegativity, exact sum, and maximum-liability coverage;
3. independent compiler golden vectors for hard ranges and smooth kernels;
4. exact portfolio/wrapper conservation under transfer, merge, and redemption;
5. refinement and coefficient-splitting metamorphic tests; and
6. SBF account/compute measurements before increasing `MAX_OUTCOMES = 16`.
