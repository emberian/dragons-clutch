# Exact price-measure witness laboratory

Status: **MODEL / OFFLINE / NOT RUNTIME**.

This dependency-free Python laboratory executes the candidate witness designed
in [`PRICE_MEASURE_WITNESS_V2.md`](../../docs/design/PRICE_MEASURE_WITNESS_V2.md).
It generates the canonical per-span B-spline-to-Bernstein transfer matrices
with exact `Fraction` arithmetic and checks the linear reconstruction and
truncated Hausdorff quadrics without floating point.

It exists to close the real multi-span degree-two/three price-coherence gap,
not to weaken the current refusal gate. The model proves only its exact finite
arithmetic transitions. It is not an SBF implementation, a completeness proof
for a bounded witness denominator, or a formal-verification claim.

Run:

```sh
python3 -m unittest discover -s research/price-measure-witness -p 'test_*.py'
```

The corpus includes exact point measures, mixtures crossing span boundaries,
quadratic/cubic Hausdorff-boundary mutations, canonical-denominator refusal,
price mutation, and the named five-claim vector that V1b accepts even though
the wide portfolio `(1,-2,10,40,64)` has nonnegative payoff `(3x-1)^2` and
strictly negative price.
