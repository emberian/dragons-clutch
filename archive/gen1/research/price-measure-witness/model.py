"""Exact research model for a per-span B-spline price-measure witness."""

from __future__ import annotations

from fractions import Fraction
from math import comb, gcd, lcm
from typing import Iterable, Sequence


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def open_clamped_knots(degree: int, breakpoint_count: int) -> tuple[Fraction, ...]:
    """Return the canonical unit-spaced open-clamped knot vector."""

    _require(degree in (0, 1, 2, 3), "degree")
    _require(breakpoint_count >= 2, "breakpoint_count")
    high = breakpoint_count - 1
    values = [Fraction(0)] * (degree + 1)
    values.extend(Fraction(index) for index in range(1, high))
    values.extend([Fraction(high)] * (degree + 1))
    return tuple(values)


def basis_values(degree: int, breakpoint_count: int, x: Fraction) -> tuple[Fraction, ...]:
    """Evaluate every canonical basis function with exact Cox-de Boor recurrence."""

    knots = open_clamped_knots(degree, breakpoint_count)
    outcome_count = breakpoint_count - 1 + degree
    _require(Fraction(0) <= x <= Fraction(breakpoint_count - 1), "x")
    if x == breakpoint_count - 1:
        return (Fraction(0),) * (outcome_count - 1) + (Fraction(1),)

    def evaluate(index: int, order: int) -> Fraction:
        if order == 0:
            return Fraction(int(knots[index] <= x < knots[index + 1]))
        value = Fraction(0)
        left_denominator = knots[index + order] - knots[index]
        if left_denominator:
            value += (x - knots[index]) * evaluate(index, order - 1) / left_denominator
        right_denominator = knots[index + order + 1] - knots[index + 1]
        if right_denominator:
            value += (
                (knots[index + order + 1] - x)
                * evaluate(index + 1, order - 1)
                / right_denominator
            )
        return value

    return tuple(evaluate(index, degree) for index in range(outcome_count))


def bernstein_values(degree: int, u: Fraction) -> tuple[Fraction, ...]:
    """Evaluate the degree-d Bernstein basis on the unit span."""

    _require(Fraction(0) <= u <= Fraction(1), "u")
    return tuple(
        Fraction(comb(degree, index)) * u**index * (1 - u) ** (degree - index)
        for index in range(degree + 1)
    )


def _solve(matrix: Sequence[Sequence[Fraction]], values: Sequence[Fraction]) -> tuple[Fraction, ...]:
    """Solve one small exact square linear system."""

    size = len(values)
    rows = [list(matrix[index]) + [values[index]] for index in range(size)]
    for column in range(size):
        pivot = next((row for row in range(column, size) if rows[row][column]), None)
        _require(pivot is not None, "singular interpolation matrix")
        rows[column], rows[pivot] = rows[pivot], rows[column]
        divisor = rows[column][column]
        rows[column] = [entry / divisor for entry in rows[column]]
        for row in range(size):
            if row == column:
                continue
            factor = rows[row][column]
            if factor:
                rows[row] = [
                    rows[row][entry] - factor * rows[column][entry]
                    for entry in range(size + 1)
                ]
    return tuple(rows[index][-1] for index in range(size))


def transfer_table(
    degree: int, breakpoint_count: int
) -> tuple[tuple[tuple[Fraction, ...], ...], ...]:
    """Return T[span][outcome][bernstein_index] exactly."""

    _require(degree in (1, 2, 3), "degree")
    outcome_count = breakpoint_count - 1 + degree
    nodes = tuple(Fraction(index, degree) for index in range(degree + 1))
    interpolation = tuple(bernstein_values(degree, node) for node in nodes)
    spans = []
    for span in range(breakpoint_count - 1):
        samples = tuple(basis_values(degree, breakpoint_count, span + node) for node in nodes)
        outcomes = []
        for outcome in range(outcome_count):
            outcomes.append(_solve(interpolation, tuple(row[outcome] for row in samples)))
        spans.append(tuple(outcomes))
    return tuple(spans)


def _canonical(values: Iterable[int]) -> bool:
    divisor = 0
    for value in values:
        divisor = gcd(divisor, value)
    return divisor == 1


def validate_witness(
    *,
    degree: int,
    breakpoint_count: int,
    price_scale: int,
    prices: Sequence[int],
    common_denominator: int,
    moments: Sequence[Sequence[int]],
) -> None:
    """Validate a complete exact witness or raise ``ValueError``."""

    _require(degree in (2, 3), "degree")
    outcome_count = breakpoint_count - 1 + degree
    span_count = breakpoint_count - 1
    _require(price_scale > 0, "price_scale")
    _require(len(prices) == outcome_count, "price length")
    _require(all(price >= 0 for price in prices), "negative price")
    _require(sum(prices) == price_scale, "simplex")
    _require(common_denominator > 0, "common denominator")
    _require(len(moments) == span_count, "span count")
    _require(all(len(row) == degree + 1 for row in moments), "row width")
    flattened = tuple(value for row in moments for value in row)
    _require(all(value >= 0 for value in flattened), "negative moment")
    _require(sum(flattened) == common_denominator, "total mass")
    _require(_canonical((common_denominator, *flattened)), "noncanonical denominator")

    for row in moments:
        if degree == 2:
            _require(row[1] * row[1] <= 4 * row[0] * row[2], "quadratic Hausdorff")
        else:
            _require(row[1] * row[1] <= 3 * row[0] * row[2], "cubic Hausdorff left")
            _require(row[2] * row[2] <= 3 * row[1] * row[3], "cubic Hausdorff right")

    table = transfer_table(degree, breakpoint_count)
    for outcome in range(outcome_count):
        reconstructed = Fraction(0)
        for span in range(span_count):
            for index in range(degree + 1):
                reconstructed += table[span][outcome][index] * Fraction(
                    moments[span][index], common_denominator
                )
        _require(reconstructed == Fraction(prices[outcome], price_scale), "price reconstruction")


def _span_coordinate(breakpoint_count: int, x: Fraction) -> tuple[int, Fraction]:
    high = breakpoint_count - 1
    _require(Fraction(0) <= x <= high, "atom x")
    if x == high:
        return high - 1, Fraction(1)
    floor = x.numerator // x.denominator
    if x.denominator == 1 and floor > 0:
        return floor - 1, Fraction(1)
    return floor, x - floor


def atomic_witness(
    degree: int,
    breakpoint_count: int,
    atoms: Sequence[tuple[Fraction, Fraction]],
) -> tuple[int, tuple[int, ...], int, tuple[tuple[int, ...], ...]]:
    """Construct canonical integer prices and moments for an atomic measure."""

    _require(degree in (2, 3), "degree")
    _require(atoms and sum((mass for _, mass in atoms), Fraction(0)) == 1, "atom mass")
    _require(all(mass >= 0 for _, mass in atoms), "negative atom mass")
    outcome_count = breakpoint_count - 1 + degree
    moment_fractions = [
        [Fraction(0) for _ in range(degree + 1)] for _ in range(breakpoint_count - 1)
    ]
    price_fractions = [Fraction(0) for _ in range(outcome_count)]
    for x, mass in atoms:
        span, u = _span_coordinate(breakpoint_count, x)
        for index, value in enumerate(bernstein_values(degree, u)):
            moment_fractions[span][index] += mass * value
        for outcome, value in enumerate(basis_values(degree, breakpoint_count, x)):
            price_fractions[outcome] += mass * value

    common_denominator = 1
    for row in moment_fractions:
        for value in row:
            common_denominator = lcm(common_denominator, value.denominator)
    moments = tuple(
        tuple(value.numerator * (common_denominator // value.denominator) for value in row)
        for row in moment_fractions
    )
    divisor = common_denominator
    for row in moments:
        for value in row:
            divisor = gcd(divisor, value)
    common_denominator //= divisor
    moments = tuple(tuple(value // divisor for value in row) for row in moments)

    price_scale = 1
    for value in price_fractions:
        price_scale = lcm(price_scale, value.denominator)
    prices = tuple(value.numerator * (price_scale // value.denominator) for value in price_fractions)
    divisor = price_scale
    for value in prices:
        divisor = gcd(divisor, value)
    price_scale //= divisor
    prices = tuple(value // divisor for value in prices)
    validate_witness(
        degree=degree,
        breakpoint_count=breakpoint_count,
        price_scale=price_scale,
        prices=prices,
        common_denominator=common_denominator,
        moments=moments,
    )
    return price_scale, prices, common_denominator, moments


def spline_local_power_coefficients(
    degree: int, breakpoint_count: int, coefficients: Sequence[int], span: int
) -> tuple[Fraction, ...]:
    """Return local-u power coefficients for a portfolio spline on one span."""

    _require(len(coefficients) == breakpoint_count - 1 + degree, "coefficient length")
    nodes = tuple(Fraction(index, degree) for index in range(degree + 1))
    matrix = tuple(tuple(node**power for power in range(degree + 1)) for node in nodes)
    values = []
    for node in nodes:
        basis = basis_values(degree, breakpoint_count, span + node)
        values.append(sum(Fraction(coefficient) * value for coefficient, value in zip(coefficients, basis)))
    return _solve(matrix, values)


def v1b_degree_two_accepts(prices: Sequence[int], scale: int) -> bool:
    """Mirror the current degree-two ceiling/butterfly family for one vector."""

    outcomes = len(prices)
    if any(price < 0 for price in prices) or sum(prices) != scale or outcomes < 3:
        return False
    for claim in range(1, outcomes - 1):
        ceiling_num, ceiling_den = (
            (1, 2)
            if outcomes == 3
            else ((2, 3) if claim == 1 or claim + 2 == outcomes else (3, 4))
        )
        if prices[claim] * ceiling_den > scale * ceiling_num:
            return False
        weight = 1 if outcomes == 3 else (2 if claim == 1 or claim + 2 == outcomes else 3)
        if prices[claim] > weight * (prices[claim - 1] + prices[claim + 1]):
            return False
    if outcomes == 3 and prices[1] ** 2 > 4 * prices[0] * prices[2]:
        return False
    return True
