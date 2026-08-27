"""Derive the V1b moment-cone tables for the open-clamped uniform B-spline basis.

This is the generator behind `relation_v1.rs::claim_ceiling` and
`butterfly_weight`, and behind the tables printed in
`docs/research/DUAL_IS_THE_MEASURE.md` section 7.6.5.  It computes, for every
admitted `(degree, outcome_count)` with degree in {2, 3}:

* `max_x N_j(x)`, exactly (sympy rationals and radicals), for every claim `j`;
* `sup_x N_j / (N_{j-1} + N_{j+1})`, the smallest butterfly weight that keeps
  `k*(N_{j-1} + N_{j+1}) - N_j` nonnegative everywhere, certified exactly.

The relation's tables are these values rounded *up* to small rationals where the
exact value is irrational, which only weakens a certificate and is therefore
sound.  Nothing in the crate imports this file; it is the derivation record.

    python3 crates/clutch-batch/fixtures/generate_moment_cone_tables.py peaks
    python3 crates/clutch-batch/fixtures/generate_moment_cone_tables.py butterflies

Requires sympy.  Runtime is a few minutes for the butterfly pass.
"""

import sys

import sympy as sp

u = sp.symbols("u")

MAX_OUTCOMES = 16
MAX_DEGREE = 3


def expanded_knots(count, degree):
    """The open-clamped expansion of `count` unit-spaced stored breakpoints."""
    return (
        [sp.Integer(0)] * (degree + 1)
        + [sp.Integer(i) for i in range(1, count - 1)]
        + [sp.Integer(count - 1)] * (degree + 1)
    )


def pane_polys(count, degree, pane):
    """Global index -> polynomial in `u` on the span `[pane, pane+1]`."""
    knots = expanded_knots(count, degree)
    span = None
    for i in range(len(knots) - 1):
        if knots[i] <= pane < knots[i + 1]:
            span = i
    assert span is not None, (count, degree, pane)
    x = sp.Integer(pane) + u
    current = {span: sp.Integer(1)}
    for order in range(1, degree + 1):
        nxt = {}
        for i in range(span - order, span + 1):
            acc = sp.Integer(0)
            left = knots[i + order] - knots[i]
            if left != 0 and i in current:
                acc += (x - knots[i]) / left * current[i]
            right = knots[i + order + 1] - knots[i + 1]
            if right != 0 and (i + 1) in current:
                acc += (knots[i + order + 1] - x) / right * current[i + 1]
            nxt[i] = sp.expand(acc)
        current = nxt
    return current


def basis_table(count, degree):
    return [pane_polys(count, degree, pane) for pane in range(count - 1)]


def check_partition_of_unity(count, degree):
    for pane, block in enumerate(basis_table(count, degree)):
        assert sp.simplify(sum(block.values()) - 1) == 0, (count, degree, pane)


def poly_min_on_unit(expr):
    expr = sp.expand(expr)
    candidates = [sp.Integer(0), sp.Integer(1)]
    derivative = sp.diff(expr, u)
    if derivative != 0:
        for root in sp.solve(sp.Eq(derivative, 0), u):
            if root.is_real and sp.simplify(root >= 0) and sp.simplify(root <= 1):
                candidates.append(root)
    return sp.Min(*[sp.simplify(expr.subs(u, c)) for c in candidates])


def poly_max_on_unit(expr):
    return -poly_min_on_unit(-expr)


def peak(count, degree, claim):
    """Exact `max_x N_claim(x)`."""
    blocks = basis_table(count, degree)
    return sp.nsimplify(sp.Max(*[poly_max_on_unit(b[claim]) for b in blocks if claim in b]))


def butterfly_supremum(count, degree, claim, samples=4001):
    """Numeric `sup_x N_claim/(N_{claim-1} + N_{claim+1})`, or None if unbounded."""
    best = 0.0
    for block in basis_table(count, degree):
        if claim not in block:
            continue
        middle = sp.lambdify(u, block[claim], "math")
        wings = sp.lambdify(
            u, block.get(claim - 1, sp.Integer(0)) + block.get(claim + 1, sp.Integer(0)), "math"
        )
        for step in range(samples + 1):
            point = step / samples
            top = middle(point)
            bottom = wings(point)
            if top <= 1e-15:
                continue
            if bottom <= 1e-15:
                return None
            best = max(best, top / bottom)
    return best


def certified_butterfly(count, degree, claim, max_denominator=64):
    """Smallest `a/b` with `b <= max_denominator` that is exactly a valid weight."""
    approximate = butterfly_supremum(count, degree, claim)
    if approximate is None:
        return None
    blocks = basis_table(count, degree)

    def valid(weight):
        for block in blocks:
            wings = block.get(claim - 1, sp.Integer(0)) + block.get(claim + 1, sp.Integer(0))
            payoff = weight * wings - block.get(claim, sp.Integer(0))
            if poly_min_on_unit(sp.expand(payoff)) < 0:
                return False
        return True

    best = None
    for denominator in range(1, max_denominator + 1):
        base = int(approximate * denominator)
        for numerator in (base, base + 1, base + 2):
            candidate = sp.Rational(numerator, denominator)
            if best is not None and candidate >= best:
                continue
            if candidate >= approximate - 1e-12 and valid(candidate):
                best = candidate
                break
    return best


def main():
    what = sys.argv[1] if len(sys.argv) > 1 else "peaks"
    for degree in (2, 3):
        for count in range(2, MAX_OUTCOMES):
            outcomes = count - 1 + degree
            if outcomes > MAX_OUTCOMES:
                continue
            check_partition_of_unity(count, degree)
            if what == "peaks":
                row = [str(peak(count, degree, j)) for j in range(outcomes)]
            else:
                row = []
                for j in range(outcomes):
                    if j == 0 or j == outcomes - 1:
                        row.append("-")
                    else:
                        weight = certified_butterfly(count, degree, j)
                        row.append("unbounded" if weight is None else str(weight))
            print(f"degree={degree} knots={count} outcomes={outcomes} {what}={row}", flush=True)


if __name__ == "__main__":
    main()
