# One-hot primitive Eggs versus native fractional basis Eggs

Status: **MODEL decision note**. This compares two architectures already
expressible in Dragon's Clutch. It does not select live terms, modify the
kernel, or make a deployment claim.

## Result

Keeping the externally composable primitive Eggs **one-hot** and compiling
graded products into integer portfolios is the lowest-risk V1. It subsumes the
desired payoff language at the admitted finite partition, gives a full market-
implied histogram, makes every terminal redemption atom-exact, and uses the
already-exercised finite-preset settlement path. Native fractional degree-1
Eggs remain a valuable later mode when smoothness per primitive is worth a
layout revision and an explicit fractional-redemption policy.

This is a product ordering, not a theorem that one-hot is universally better.

## Exact inclusion

Let one-hot Egg `E_i` pay one collateral atom exactly when cell `i` resolves.
An integer portfolio `a = (a_0,...,a_(n-1))` then pays exactly `a_j` when cell
`j` resolves. Consequently:

- binary and categorical claims are unit vectors;
- hard ranges are indicator vectors;
- graded proximity, triangular, capped-linear, spline-sampled, and tabulated
  kernel claims are their exact integer sample vectors;
- a market price vector over the primitive Eggs is a complete finite
  distribution, and every compiled product price is a linear functional of
  that distribution; and
- every bounded nonnegative payoff on the finite partition is represented by
  `ExactSamples`. This inclusion is exact at the partition, not merely a list
  of blessed curve names.

For a genuinely continuous target `f`, the statement is finite approximation,
not exact continuity. On cells of maximum radius `r`, a target with Lipschitz
constant `L` has step error at most `L*r`, plus coefficient quantization. A
Gaussian with peak `H` and scale `sigma` satisfies the conservative bound
`|f'| <= H/sigma`, so nearest-anchor cells have error at most

```text
H * ceil(max_gap / 2) / sigma + knot_quantization_error.
```

The executable model reports this bound. At a collateral with six decimal
atoms, choosing `H = 1_000_000` represents a maximum payout of exactly one
collateral unit and coefficient quantization below one atom (`10^-6` units).
It does **not** require one million collateral units: `H` is the ordinary atom
scale. Worst-case collateral for one basket is `max_i a_i <= H` atoms, exactly
one unit in this example.

## Why redemption becomes simpler

With one-hot primitives, a holder burns `q` Egg atoms and receives either `q`
or zero collateral atoms. An integer basket pays `q*a_j`. There is no division,
remainder account, minimum lot, dust beneficiary, or dependence on which
fractional weight happened to resolve.

With native fractional basis weights `w_i/D`, a basket pays

```text
q * sum_i a_i*w_i / D.
```

The least lot that is integral for every vector in the full integer simplex is

```text
lcm_i D / gcd(D, |a_i-a_0|),
```

which can be `D`. A complete set remains exact, but an individual externally
held Egg or shaped portfolio may need a large lot, persistent remainder credit,
or an exact-or-refuse UX. The model checks this formula exhaustively over small
coefficient cubes and simplex lattices.

## Honest tradeoff

| Property | One-hot primitive + integer portfolio | Native fractional degree-1 primitive |
|---|---|---|
| Terminal primitive redemption | exact `q` or `0` | may require lot/remainder state |
| Account-plane maturity | existing finite-preset path | derived-vector authority layout still open |
| Curve at fixed `n` | piecewise constant | piecewise linear |
| Gaussian interpolation bound | `O(H*h/sigma)` | `O(H*h^2/sigma^2)` |
| Runtime payout selection | one index, no division | at most two nonzero weights plus division |
| External token mental model | categorical cell claims | fractional basis-function claims |
| Exact full distribution | histogram over cells | expectations of overlapping hats |
| New primitive mints | `n` | `n` |
| Worst-state basket liability | `max_i a_i` | `max_i a_i` by partition unity |

The approximation gap is real. With uniform gap `h = sigma/2`, the simple
conservative Gaussian bounds are `H/4` for nearest-cell one-hot settlement and
`H/32` for degree-1 linear interpolation, before knot quantization. Sixteen
one-hot cells may therefore be too coarse for a wide-domain precision product.
That is the strongest reason to retain native fractional mode as a planned
extension rather than delete it.

## Recommended integration path

1. Keep market primitives and externally materialized Eggs one-hot for the
   first public implementation.
2. Freeze an exact coefficient-vector artifact and compiler version in terms
   metadata or a content-addressed client artifact. Do not evaluate a general
   payoff program onchain.
3. Submit ordinary bounded portfolio intents to the existing batch relation.
   A separate wrapper is unnecessary for V1; if added, it must escrow the exact
   integer basket and its supply must have one semantic owner.
4. Display `max(coefficients)`, coefficient atom scale, cell approximation
   bound, and realized-cell payout before signature.
5. Treat a higher outcome cap as an account/transaction/proof benchmark, not a
   documentation edit.
6. Promote native fractional degree-1 markets only after the resolution record
   and kernel account bind the resolved vector and one of these policies is
   frozen: exact lots, persistent remainder credits, or portfolio-atomic
   aggregate redemption. Silent floor rounding remains forbidden.

This path preserves the more general derived-basis research while making the
first composable product depend on fewer new consensus facts.
