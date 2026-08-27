# Exact price-measure witness laboratory

Status: **CONTINUOUS-EXACT MODEL / NOT CURRENT RUNTIME SEMANTICS**.

This dependency-free Python laboratory executes the candidate witness designed
in [`PRICE_MEASURE_WITNESS_V2.md`](../../docs/design/PRICE_MEASURE_WITNESS_V2.md).
It generates the canonical per-span B-spline-to-Bernstein transfer matrices
with exact `Fraction` arithmetic and checks the linear reconstruction and
truncated Hausdorff quadrics without floating point.

It exists to close the real multi-span degree-two/three price-coherence gap,
not to weaken the current refusal gate. The model proves only its exact finite
arithmetic transitions. It is not an SBF implementation, a completeness proof
for a bounded witness denominator, or a formal-verification claim.

The safe Rust reproduction is now in `crates/clutch-price-measure`, but the
continuous witness is deliberately refused for today's settlement profile.
Current `clutch-bspline` evaluates integer coordinates and then applies
largest-remainder payout quantization. Its true price body is a finite polytope
depending on knots, spacing, payout denominator, domain, and rounding version.
The Rust crate separately implements a support-bounded atomic certificate that
recomputes those exact runtime payout vectors.

Run:

```sh
python3 -m unittest discover -s research/price-measure-witness -p 'test_*.py'
```

This corpus includes exact continuous point measures, mixtures crossing span
boundaries, quadratic/cubic Hausdorff-boundary mutations,
canonical-denominator refusal, price mutation, and the named five-claim vector
that V1b accepts even though the wide portfolio `(1,-2,10,40,64)` has
nonnegative payoff `(3x-1)^2` and strictly negative continuous price. That
separating portfolio is not an unconditional runtime claim after payout
quantization.
