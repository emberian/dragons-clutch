# Native and lowered claim semantics

Status: **DESIGN CONTROL / IN-FLIGHT** (2026-08-19). This document prevents an
implementation lowering from silently redefining the product. It changes no
consensus bytes, market terms, proof claim, or release status.

## The distinction that must remain explicit

There are three different constructions which earlier planning sometimes
called a "portfolio":

1. **Native basis semantics.** A market freezes basis functions `B_i(x)` and
   consensus derives their exact settlement weights from authenticated
   evidence. For Dragon's Clutch, the intended smooth family is the bounded
   open-clamped B-spline basis of degree zero through three. Its partition of
   unity is load-bearing: `B_i(x) >= 0` and `sum_i B_i(x) = 1`.
2. **Exact coefficient algebra.** A bounded payoff in the native span is
   `f(x) = sum_i a_i B_i(x)`. The vector `a` is not an approximation merely
   because it is a vector. It is the canonical finite representation of the
   payoff in that basis. Consensus may validate coefficients and evaluate the
   dot product without implementing a floating-point expression language.
3. **Categorical compatibility lowering.** Sample or integrate a desired curve
   on a degree-zero one-hot partition, then hold a basket of categorical Eggs.
   This is useful for external composability and conservative fallback, but it
   is generally an approximation to smooth settlement. It must not be called
   the definition of native shaped claims.

A fourth issue is orthogonal: a coefficient vector can be an atomic signed
intent while still settling into separable bearer components. If the product
promises a transferable named shaped claim, its identity and atomic lifecycle
need an explicit wrapper or native position representation; a UI label on a
basket is not enough.

## Inventory of the earlier narrowing

| Product surface | Earlier narrowed representation | Required primary representation | Compatibility path |
| --- | --- | --- | --- |
| categorical claim | one-hot Egg | native degree-zero basis Egg | identical |
| smooth distributional settlement | sampled categorical histogram | native degree-one through degree-three B-spline weights derived from evidence | degree-zero approximation with a disclosed error bound |
| hard range and tail | sum of categorical Eggs | exact coefficients over the selected native basis; state whether the basis represents an indicator exactly or an admitted approximation | categorical basket |
| triangle / tent | sampled coefficient vector over bins | exact degree-one spline member where the frozen knots admit it | sampled basket with error certificate |
| capped-linear, call, put, spread | sampled or tabulated categorical payoff | exact piecewise-linear spline member when knots contain every breakpoint; otherwise a certified basis projection | categorical basket |
| Gaussian / proximity kernel | finite sampled table | certified coefficient construction in the native spline space with an explicit norm/error bound | sampled categorical table |
| arbitrary bounded tabulated payoff | `ExactSamples` escape hatch | exact coefficient artifact, with no analytic claim beyond those bytes | unchanged when degree zero is intentionally selected |
| market-implied distribution | prices of one-hot cells | prices of native basis Eggs plus an explicitly defined reconstruction functional | categorical price histogram |
| named curve position | freely separable basket | canonical descriptor and atomic position or wrapper semantics where promised | component materialization and external basket trading |
| shaped early exit | opposite basket order | atomic coefficient-vector order in the coupled relation | component sales on external venues |
| passive range liquidity | finite schedule of basket quotes | a frozen, fully capitalized liquidity policy over native coefficient intents; a cost potential only if its loss theorem and update transition are proved | schedule compiler |

## What was not converted into portfolio sugar

The source/archive accumulator, window statistics, resolution provenance,
pooled custody equation, collateral Realms, Token-2022 supply truth, categorical
bearer exit, coupled batch conservation, prepaid liveness accounts, and fee
admission are separate protocol semantics. They may feed or trade a native
shaped market, but they are not payoff-compiler artifacts.

Margin, leverage, liquidation, privacy, generic cost-function AMMs, and a full
passive-LP lifecycle were deferred or remain research surfaces. They were not
implemented by the categorical portfolio compiler and must not be described as
such.

## Promotion rules

Native degree `d` may be enabled only when one joined evidence chain binds:

1. immutable knot vector, degree, edge policy, denominator, evaluator version,
   source/window identity, and rounding rule;
2. exact safe-Rust evaluator with hostile-input and rational-oracle tests;
3. nonnegative local support and exact partition-of-unity results for the same
   construction, including clamped endpoints;
4. an interval-evidence rule that never substitutes an arbitrary midpoint;
5. exact fractional redemption through a frozen lot or remainder-credit rule;
6. account/layout/SBF integration, rollback, and runtime evidence for that
   degree; and
7. an approximation certificate whenever an analytic label denotes something
   outside the finite spline space.

Until a degree passes those gates, it refuses. Refusal does not authorize the
client to relabel a categorical approximation as native smooth settlement.

## Architectural direction

```text
analytic or tabulated payoff request
                  |
       certified coefficient compiler
                  |
       exact native coefficient artifact
                  |
authenticated evidence -> native B-spline evaluator -> simplex weights
                  |                                  |
                  +---------- exact dot product -----+
                                      |
                              terminal payout

optional adapter: native claim -> disclosed categorical approximation
```

The categorical compiler therefore remains valuable, but in the right place:
as an adapter, comparison oracle, external-liquidity bridge, and emergency
fallback. It is not the semantic ceiling of Dragon's Clutch.
