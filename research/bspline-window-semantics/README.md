# B-spline window-semantics lab

An isolated, dependency-free exact model for comparing native degree-two/three
B-spline settlement over source points, confidence intervals, TWAP, and path
occupation. See `DESIGN.md` for the conclusions and proof-status boundary.

Run from the repository root:

```sh
python3 -m unittest discover \
  -s research/bspline-window-semantics -p 'test_*.py' -v
python3 research/bspline-window-semantics/compare.py
```

Expected comparison:

```text
basis: degree=2 knots=(0,16,32) D=64
path: 1*4, 1*28; exact TWAP=16
evaluate-at-TWAP:            (0, 32, 32, 0)
quantized-basis occupation:  (18, 14, 14, 18)
exact-basis occupation:      (18, 14, 14, 18)
```

The equality of the two occupation arms in that example is not universal;
tests contain coarse-denominator paths where local quantization changes the
final atom allocation.
