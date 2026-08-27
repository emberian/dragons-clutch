#!/usr/bin/env python3
"""Independent Fraction/Cox-de-Boor differential and mutation oracle."""

from __future__ import annotations

import argparse
from fractions import Fraction
import itertools
import pathlib
import random
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
SENTINEL = "n"


def expanded_knots(knots: tuple[int, ...], degree: int) -> tuple[int, ...]:
    return ((knots[0],) * (degree + 1) + knots[1:-1] +
            (knots[-1],) * (degree + 1))


def cox(index: int, degree: int, value: int, knots: tuple[int, ...], top: int) -> Fraction:
    if degree == 0:
        if knots[index] <= value < knots[index + 1]:
            return Fraction(1)
        if value == top and knots[index + 1] == top and knots[index] < top:
            return Fraction(1)
        return Fraction(0)
    left = Fraction(0)
    left_den = knots[index + degree] - knots[index]
    if left_den:
        left = Fraction(value - knots[index], left_den) * cox(
            index, degree - 1, value, knots, top
        )
    right = Fraction(0)
    right_den = knots[index + degree + 1] - knots[index + 1]
    if right_den:
        right = Fraction(knots[index + degree + 1] - value, right_den) * cox(
            index + 1, degree - 1, value, knots, top
        )
    return left + right


def quantize(rational: tuple[Fraction, ...], denominator: int, policy: str) -> tuple[int, ...]:
    weights = [int(denominator * item) for item in rational]
    residual = denominator - sum(weights)
    remainders = [denominator * item - weight
                  for item, weight in zip(rational, weights)]
    if policy == "largest":
        order = sorted(range(len(rational)), key=lambda i: (remainders[i], -i),
                       reverse=True)
        for index in order[:residual]:
            assert remainders[index] > 0
            weights[index] += 1
    elif residual:
        support = [index for index, item in enumerate(rational) if item]
        weights[support[0 if policy == "low" else -1]] += residual
    assert sum(weights) == denominator
    return tuple(weights)


def oracle(case: tuple[int, int, int, str, str, tuple[int, ...]]) -> tuple[str, ...]:
    degree, denominator, value, edge, _, knots = case
    outcomes = len(knots) + 1 if degree == 0 else len(knots) - 1 + degree
    if degree == 0:
        cell = sum(value >= boundary for boundary in knots)
        weights = [0] * outcomes
        weights[cell] = denominator
        return tuple(map(str, weights))
    if value < knots[0] or value > knots[-1]:
        if edge == "r":
            return ("ERR",)
        value = min(max(value, knots[0]), knots[-1])
    if value == knots[-1]:
        weights = [0] * outcomes
        weights[-1] = denominator
        return tuple(map(str, weights))
    full = expanded_knots(knots, degree)
    rational = tuple(cox(i, degree, value, full, knots[-1]) for i in range(outcomes))
    assert sum(rational, Fraction(0)) == 1
    support = [index for index, item in enumerate(rational) if item]
    assert len(support) <= degree + 1
    weights = quantize(rational, denominator, "largest")
    return tuple(map(str, weights))


def line(case: tuple[int, int, int, str, str, tuple[int, ...]]) -> str:
    degree, denominator, value, edge, spacing, knots = case
    return ",".join(map(str, (degree, denominator, value, edge, spacing,
                              len(knots), *knots)))


def exhaustive_cases() -> list[tuple[int, int, int, str, str, tuple[int, ...]]]:
    cases: list[tuple[int, int, int, str, str, tuple[int, ...]]] = []
    for count in range(1, 5):
        for knots in itertools.combinations(range(1, 8), count):
            for denominator in (1, 3, 8):
                for value in range(0, 9):
                    cases.append((0, denominator, value, "c", SENTINEL, knots))
    for count in range(2, 6):
        for knots in itertools.combinations(range(0, 8), count):
            for denominator in range(1, 9):
                for value in range(0, 8):
                    cases.append((1, denominator, value, "c", SENTINEL, knots))
    for degree in (2, 3):
        for count in range(2, 7):
            for shift in range(0, 4):
                gap = 1 << shift
                knots = tuple(3 + index * gap for index in range(count))
                for denominator in range(1, 13):
                    for value in range(max(0, knots[0] - 1), knots[-1] + 2):
                        for edge in ("c", "r"):
                            cases.append((degree, denominator, value, edge, str(shift), knots))
    return cases


def random_cases(count: int, seed: int) -> list[tuple[int, int, int, str, str, tuple[int, ...]]]:
    rng = random.Random(seed)
    cases = []
    for _ in range(count):
        degree = rng.randrange(4)
        if degree == 0:
            knot_count = rng.randrange(1, 16)
            knots = tuple(sorted(rng.sample(range(1, 10_000), knot_count)))
            spacing = SENTINEL
        elif degree == 1 and rng.randrange(2) == 0:
            knot_count = rng.randrange(2, 17)
            knots = tuple(sorted(rng.sample(range(0, 10_000), knot_count)))
            spacing = SENTINEL
        else:
            max_knots = 17 - degree
            knot_count = rng.randrange(2, max_knots + 1)
            shift = rng.randrange(0, 13)
            origin = rng.randrange(0, 10_000)
            knots = tuple(origin + index * (1 << shift) for index in range(knot_count))
            spacing = str(shift)
        denominator = rng.randrange(1, 1 << 24)
        value = rng.randrange(max(0, knots[0] - 100), knots[-1] + 101)
        edge = rng.choice(("c", "r")) if degree else "c"
        cases.append((degree, denominator, value, edge, spacing, knots))
    return cases


def mutation_checks() -> None:
    knots = expanded_knots((0, 4), 2)
    rational = tuple(cox(i, 2, 2, knots, 4) for i in range(3))
    assert sum(7 * item.numerator // item.denominator for item in rational) == 5

    edge = tuple(cox(i, 2, 2, expanded_knots((0, 4, 8, 12), 2), 12)
                 for i in range(5))
    assert edge[:3] == (Fraction(1, 4), Fraction(5, 8), Fraction(1, 8))
    assert edge[:3] != (Fraction(1, 8), Fraction(3, 4), Fraction(1, 8))

    top = tuple(cox(i, 3, 8, expanded_knots((0, 4, 8), 3), 8)
                for i in range(5))
    assert top[-1] == 1 and sum(top, Fraction(0)) == 1

    first = tuple(cox(i, 1, 1, expanded_knots((0, 8), 1), 8) for i in range(2))
    assert quantize(first, 7, "largest") == (6, 1)
    assert quantize(first, 7, "low") == (7, 0)
    last = tuple(cox(i, 1, 7, expanded_knots((0, 8), 1), 8) for i in range(2))
    assert quantize(last, 7, "largest") == (1, 6)
    assert quantize(last, 7, "high") == (0, 7)


def rounding_falsifier() -> tuple[tuple[int, Fraction, Fraction, Fraction], ...]:
    report = []
    for degree in (1, 2, 3):
        errors = {policy: Fraction(0) for policy in ("low", "high", "largest")}
        cases = 0
        for knot_count in range(2, 8):
            knots = tuple(range(knot_count))
            full = expanded_knots(knots, degree)
            outcomes = knot_count - 1 + degree
            for denominator in range(1, 17):
                for pane in range(knot_count - 1):
                    for numerator in range(16):
                        value = Fraction(pane * 16 + numerator, 16)
                        rational = tuple(cox(i, degree, value, full, knots[-1])
                                         for i in range(outcomes))
                        for policy in errors:
                            weights = quantize(rational, denominator, policy)
                            errors[policy] += sum(
                                abs(Fraction(weight, denominator) - exact)
                                for weight, exact in zip(weights, rational)
                            )
                        cases += 1
        means = {policy: total / cases for policy, total in errors.items()}
        assert means["largest"] < means["low"]
        assert means["largest"] < means["high"]
        report.append((degree, means["low"], means["high"], means["largest"]))
    return tuple(report)


def run_driver(cases: list[tuple[int, int, int, str, str, tuple[int, ...]]]) -> list[str]:
    command = ["cargo", "run", "--quiet", "--example", "oracle_driver"]
    payload = "\n".join(line(case) for case in cases) + "\n"
    completed = subprocess.run(command, cwd=ROOT, input=payload, text=True,
                               capture_output=True, check=True)
    return completed.stdout.splitlines()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--random", type=int, default=2_048)
    parser.add_argument("--seed", type=int, default=0xD6E66)
    args = parser.parse_args()
    cases = exhaustive_cases() + random_cases(args.random, args.seed)
    actual = run_driver(cases)
    assert len(actual) == len(cases)
    for index, (case, output) in enumerate(zip(cases, actual)):
        expected = oracle(case)
        fields = tuple(output.split(","))
        if expected == ("ERR",):
            if fields != ("err", "ValueOutOfRange"):
                raise AssertionError((index, case, expected, fields))
        elif fields != ("ok", *expected):
            raise AssertionError((index, case, expected, fields))
    mutation_checks()
    rounding = rounding_falsifier()
    print(f"PASS: {len(cases)} exact differential cases; seed={args.seed}; mutants=6")
    for degree, low, high, largest in rounding:
        print(f"rounding d={degree}: mean-L1 low={float(low):.8f} "
              f"high={float(high):.8f} largest={float(largest):.8f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
