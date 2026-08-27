"""Exact research model for native B-spline window settlement semantics.

This directory is deliberately isolated from protocol code.  The model uses
integer coordinates and :class:`fractions.Fraction`; no float participates in
an admitted result.  It compares four *different* semantics:

* one authenticated source point, with confidence used either as an admission
  bound or as an uncertainty interval;
* evaluation of the native basis at an exact rational TWAP;
* time occupation of the canonical quantized native basis; and
* an exact-basis occupation oracle which quantizes only once, at finalization.

The production B-spline evaluator is the semantic reference for point
quantization: floor exact scaled weights, then award residual atoms to the
largest fractional remainders, with the lowest outcome index winning ties.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from fractions import Fraction
from math import lcm
from typing import Iterable, Optional, Sequence


class Refusal(ValueError):
    """A deterministic research-model refusal."""


class EdgePolicy(Enum):
    """Frozen handling of values outside the native knot span."""

    CLAMP = "clamp"
    REFUSE = "refuse"


class ConfidencePolicy(Enum):
    """Frozen interpretation of a source point and confidence interval."""

    # The provider's canonical point is the fact.  Confidence is an admission
    # quality bound, not a set of possible settlement values.
    POINT_AUTHORITATIVE = "point_authoritative"
    # Every integer source atom in the confidence interval is compatible.  A
    # payout is admitted only when the canonical quantized vector is invariant.
    PAYOUT_INVARIANT = "payout_invariant"


@dataclass(frozen=True)
class BasisSpec:
    """Open-clamped uniform degree-two/three B-spline basis."""

    degree: int
    breakpoints: tuple[int, ...]
    denominator: int
    edge_policy: EdgePolicy = EdgePolicy.CLAMP

    def validate(self) -> None:
        if self.degree not in (2, 3):
            raise Refusal("degree")
        if self.denominator <= 0:
            raise Refusal("denominator")
        if len(self.breakpoints) < 2:
            raise Refusal("breakpoints")
        if any(not isinstance(point, int) or point < 0 for point in self.breakpoints):
            raise Refusal("breakpoint-domain")
        gaps = tuple(b - a for a, b in zip(self.breakpoints, self.breakpoints[1:]))
        if any(gap <= 0 for gap in gaps) or len(set(gaps)) != 1:
            raise Refusal("uniform-positive-spacing")
        if self.outcome_count > 16:
            raise Refusal("outcome-count")

    @property
    def outcome_count(self) -> int:
        return len(self.breakpoints) - 1 + self.degree

    @property
    def gap(self) -> int:
        self.validate()
        return self.breakpoints[1] - self.breakpoints[0]

    @property
    def expanded_knots(self) -> tuple[int, ...]:
        self.validate()
        degree = self.degree
        return (
            (self.breakpoints[0],) * (degree + 1)
            + self.breakpoints[1:-1]
            + (self.breakpoints[-1],) * (degree + 1)
        )

    @property
    def identity(self) -> tuple[object, ...]:
        """Everything this model needs to prevent cross-basis composition."""

        return (self.degree, self.breakpoints, self.denominator, self.edge_policy)


def _handle_edge(spec: BasisSpec, value: Fraction | int) -> Fraction:
    spec.validate()
    x = Fraction(value)
    low = Fraction(spec.breakpoints[0])
    high = Fraction(spec.breakpoints[-1])
    if low <= x <= high:
        return x
    if spec.edge_policy is EdgePolicy.REFUSE:
        raise Refusal("value-out-of-range")
    return max(low, min(high, x))


def exact_basis(spec: BasisSpec, value: Fraction | int) -> tuple[Fraction, ...]:
    """Evaluate every open-clamped basis function exactly at one value."""

    x = _handle_edge(spec, value)
    high = Fraction(spec.breakpoints[-1])
    outcomes = spec.outcome_count
    if x == high:
        return (Fraction(0),) * (outcomes - 1) + (Fraction(1),)

    knots = tuple(Fraction(knot) for knot in spec.expanded_knots)
    row = [
        Fraction(int(knots[index] <= x < knots[index + 1]))
        for index in range(len(knots) - 1)
    ]
    for degree in range(1, spec.degree + 1):
        next_row: list[Fraction] = []
        for index in range(len(row) - 1):
            left_denominator = knots[index + degree] - knots[index]
            right_denominator = knots[index + degree + 1] - knots[index + 1]
            left = (
                Fraction(0)
                if left_denominator == 0
                else (x - knots[index]) * row[index] / left_denominator
            )
            right = (
                Fraction(0)
                if right_denominator == 0
                else (knots[index + degree + 1] - x)
                * row[index + 1]
                / right_denominator
            )
            next_row.append(left + right)
        row = next_row
    result = tuple(row[:outcomes])
    if any(weight < 0 for weight in result) or sum(result) != 1:
        raise AssertionError("exact basis violated partition of unity")
    if sum(weight > 0 for weight in result) > spec.degree + 1:
        raise AssertionError("exact basis violated local support")
    return result


def quantize_simplex(
    rational: Sequence[Fraction | int], denominator: int
) -> tuple[int, ...]:
    """Canonical largest-remainder quantization of an exact simplex vector."""

    values = tuple(Fraction(value) for value in rational)
    if denominator <= 0 or not values:
        raise Refusal("quantization-shape")
    if any(value < 0 for value in values) or sum(values) != 1:
        raise Refusal("not-a-simplex")
    floors = [
        (denominator * value.numerator) // value.denominator for value in values
    ]
    remainders = [
        denominator * value - floor for value, floor in zip(values, floors)
    ]
    residual = denominator - sum(floors)
    order = sorted(
        range(len(values)), key=lambda index: (remainders[index], -index), reverse=True
    )
    for index in order[:residual]:
        if remainders[index] <= 0:
            raise AssertionError("largest-remainder residual had no recipient")
        floors[index] += 1
    result = tuple(floors)
    if min(result) < 0 or sum(result) != denominator:
        raise AssertionError("quantization violated partition of unity")
    return result


def quantize_weights(spec: BasisSpec, value: Fraction | int) -> tuple[int, ...]:
    """Evaluate and canonically quantize one native B-spline vector."""

    return quantize_simplex(exact_basis(spec, value), spec.denominator)


@dataclass(frozen=True)
class ConstantCertificate:
    """Bounded exhaustive certificate over a named discrete evidence lattice."""

    vector: tuple[int, ...]
    low: Fraction
    high: Fraction
    evaluated_points: int
    lattice: str


def certify_constant_integer_interval(
    spec: BasisSpec,
    low: int,
    high: int,
    *,
    max_points: int = 4096,
) -> ConstantCertificate:
    """Prove ``W_D(x)`` constant for every integer ``x`` in ``[low, high]``.

    This is an exact V1 candidate, not an endpoint heuristic.  The compatible
    evidence domain is explicitly the integer source-atom lattice.  Runtime is
    bounded by ``max_points`` and a wider interval refuses rather than sampling.
    """

    if low < 0 or low > high:
        raise Refusal("reversed-interval")
    points = high - low + 1
    if points > max_points:
        raise Refusal("interval-certificate-budget")
    target = quantize_weights(spec, low)
    for value in range(low + 1, high + 1):
        if quantize_weights(spec, value) != target:
            raise Refusal("ambiguous-interval")
    return ConstantCertificate(target, Fraction(low), Fraction(high), points, "integer")


def certify_constant_ratio_interval(
    spec: BasisSpec,
    numerator_low: int,
    numerator_high: int,
    positive_denominator: int,
    *,
    max_points: int = 4096,
) -> ConstantCertificate:
    """Prove constancy over an exact rational lattice ``n / denominator``."""

    if numerator_low < 0 or numerator_low > numerator_high:
        raise Refusal("ratio-interval")
    if positive_denominator <= 0:
        raise Refusal("ratio-denominator")
    points = numerator_high - numerator_low + 1
    if points > max_points:
        raise Refusal("interval-certificate-budget")
    low = Fraction(numerator_low, positive_denominator)
    high = Fraction(numerator_high, positive_denominator)
    target = quantize_weights(spec, low)
    for numerator in range(numerator_low + 1, numerator_high + 1):
        if quantize_weights(spec, Fraction(numerator, positive_denominator)) != target:
            raise Refusal("ambiguous-interval")
    return ConstantCertificate(target, low, high, points, "rational-numerator")


def resolve_source_point(
    spec: BasisSpec,
    point: int,
    confidence_low: int,
    confidence_high: int,
    policy: ConfidencePolicy,
    *,
    max_points: int = 4096,
) -> tuple[int, ...]:
    """Resolve a canonical source point under one immutable confidence policy."""

    if point < 0 or confidence_low < 0 or not confidence_low <= point <= confidence_high:
        raise Refusal("point-outside-confidence")
    if policy is ConfidencePolicy.POINT_AUTHORITATIVE:
        return quantize_weights(spec, point)
    return certify_constant_integer_interval(
        spec, confidence_low, confidence_high, max_points=max_points
    ).vector


def resolve_exact_twap(
    spec: BasisSpec, price_time_integral: int, covered_duration: int
) -> tuple[int, ...]:
    """Evaluate at the exact rational TWAP; never floor or midpoint it."""

    if price_time_integral < 0 or covered_duration <= 0:
        raise Refusal("twap")
    return quantize_weights(spec, Fraction(price_time_integral, covered_duration))


def resolve_conservative_integer_interval(
    spec: BasisSpec, low: int, high: int, *, max_points: int = 4096
) -> tuple[int, ...]:
    """Settle iff all compatible integer source atoms pay identically."""

    return certify_constant_integer_interval(
        spec, low, high, max_points=max_points
    ).vector


@dataclass(frozen=True)
class TwapSummary:
    """Associative exact point-TWAP summary over adjacent bucket spans."""

    spec_identity: tuple[object, ...]
    start_bucket: Optional[int]
    end_bucket_exclusive: Optional[int]
    duration: int
    price_time_integral: int

    @classmethod
    def empty(cls, spec: BasisSpec) -> "TwapSummary":
        spec.validate()
        return cls(spec.identity, None, None, 0, 0)

    @classmethod
    def from_point(
        cls, spec: BasisSpec, bucket: int, duration: int, point: int
    ) -> "TwapSummary":
        spec.validate()
        if bucket < 0 or duration <= 0 or point < 0:
            raise Refusal("twap-observation")
        return cls(spec.identity, bucket, bucket + 1, duration, duration * point)

    def combine(self, other: "TwapSummary") -> "TwapSummary":
        if self.spec_identity != other.spec_identity:
            raise Refusal("summary-domain")
        if self.start_bucket is None:
            return other
        if other.start_bucket is None:
            return self
        if self.end_bucket_exclusive != other.start_bucket:
            raise Refusal("nonadjacent")
        return TwapSummary(
            self.spec_identity,
            self.start_bucket,
            other.end_bucket_exclusive,
            self.duration + other.duration,
            self.price_time_integral + other.price_time_integral,
        )

    def finalize(self, spec: BasisSpec) -> tuple[int, ...]:
        if spec.identity != self.spec_identity or self.duration <= 0:
            raise Refusal("summary-domain")
        return resolve_exact_twap(spec, self.price_time_integral, self.duration)


@dataclass(frozen=True)
class QuantizedOccupationSummary:
    """Associative duration-weighted occupation of canonical ``W_D(x)``."""

    spec_identity: tuple[object, ...]
    start_bucket: Optional[int]
    end_bucket_exclusive: Optional[int]
    duration: int
    masses: tuple[int, ...]
    denominator: int

    @classmethod
    def empty(cls, spec: BasisSpec) -> "QuantizedOccupationSummary":
        spec.validate()
        return cls(
            spec.identity,
            None,
            None,
            0,
            (0,) * spec.outcome_count,
            spec.denominator,
        )

    @classmethod
    def from_vector(
        cls,
        spec: BasisSpec,
        bucket: int,
        duration: int,
        vector: Sequence[int],
    ) -> "QuantizedOccupationSummary":
        spec.validate()
        vector = tuple(vector)
        if bucket < 0 or duration <= 0:
            raise Refusal("bucket")
        if len(vector) != spec.outcome_count:
            raise Refusal("vector-shape")
        if any(weight < 0 for weight in vector) or sum(vector) != spec.denominator:
            raise Refusal("vector")
        return cls(
            spec.identity,
            bucket,
            bucket + 1,
            duration,
            tuple(duration * weight for weight in vector),
            spec.denominator,
        )

    @classmethod
    def from_point(
        cls, spec: BasisSpec, bucket: int, duration: int, point: int
    ) -> "QuantizedOccupationSummary":
        return cls.from_vector(spec, bucket, duration, quantize_weights(spec, point))

    @classmethod
    def from_interval(
        cls,
        spec: BasisSpec,
        bucket: int,
        duration: int,
        low: int,
        high: int,
        *,
        max_points: int = 4096,
    ) -> "QuantizedOccupationSummary":
        vector = resolve_conservative_integer_interval(
            spec, low, high, max_points=max_points
        )
        return cls.from_vector(spec, bucket, duration, vector)

    def combine(self, other: "QuantizedOccupationSummary") -> "QuantizedOccupationSummary":
        if (
            self.spec_identity != other.spec_identity
            or len(self.masses) != len(other.masses)
            or self.denominator != other.denominator
        ):
            raise Refusal("summary-domain")
        if self.start_bucket is None:
            return other
        if other.start_bucket is None:
            return self
        if self.end_bucket_exclusive != other.start_bucket:
            raise Refusal("nonadjacent")
        masses = tuple(left + right for left, right in zip(self.masses, other.masses))
        duration = self.duration + other.duration
        if sum(masses) != self.denominator * duration:
            raise AssertionError("occupation mass lost partition of unity")
        return QuantizedOccupationSummary(
            self.spec_identity,
            self.start_bucket,
            other.end_bucket_exclusive,
            duration,
            masses,
            self.denominator,
        )

    def finalize(self) -> tuple[int, ...]:
        """Average exactly, then apply largest remainder once more."""

        if self.duration <= 0:
            raise Refusal("empty-occupation")
        if sum(self.masses) != self.denominator * self.duration:
            raise Refusal("summary-invariant")
        average = tuple(
            Fraction(mass, self.denominator * self.duration) for mass in self.masses
        )
        return quantize_simplex(average, self.denominator)


def _lcm_one_through(value: int) -> int:
    result = 1
    for factor in range(1, value + 1):
        result = lcm(result, factor)
    return result


def exact_integer_basis_scale(spec: BasisSpec) -> int:
    """A conservative common denominator for all integer-point basis values.

    In a uniform degree-``d`` recurrence every nonzero division is by
    ``m*h`` for ``m in 1..d``.  ``(lcm(1..d)*h)^d`` therefore clears every
    product of at most ``d`` such divisors.  It is intentionally conservative;
    its headroom cost is one reason this exact ideal is not the V1 favorite.
    """

    spec.validate()
    return (_lcm_one_through(spec.degree) * spec.gap) ** spec.degree


@dataclass(frozen=True)
class ExactBasisOccupationSummary:
    """Exact-basis occupation oracle, with only one final quantization."""

    spec_identity: tuple[object, ...]
    start_bucket: Optional[int]
    end_bucket_exclusive: Optional[int]
    duration: int
    masses: tuple[int, ...]
    basis_scale: int
    payout_denominator: int

    @classmethod
    def empty(cls, spec: BasisSpec) -> "ExactBasisOccupationSummary":
        scale = exact_integer_basis_scale(spec)
        return cls(
            spec.identity,
            None,
            None,
            0,
            (0,) * spec.outcome_count,
            scale,
            spec.denominator,
        )

    @classmethod
    def from_point(
        cls, spec: BasisSpec, bucket: int, duration: int, point: int
    ) -> "ExactBasisOccupationSummary":
        if bucket < 0 or duration <= 0 or point < 0:
            raise Refusal("occupation-observation")
        scale = exact_integer_basis_scale(spec)
        scaled: list[int] = []
        for weight in exact_basis(spec, point):
            atom = weight * scale
            if atom.denominator != 1:
                raise AssertionError("declared exact basis scale did not clear denominator")
            scaled.append(duration * atom.numerator)
        if sum(scaled) != duration * scale:
            raise AssertionError("exact occupation lost partition of unity")
        return cls(
            spec.identity,
            bucket,
            bucket + 1,
            duration,
            tuple(scaled),
            scale,
            spec.denominator,
        )

    def combine(self, other: "ExactBasisOccupationSummary") -> "ExactBasisOccupationSummary":
        if (
            self.spec_identity != other.spec_identity
            or self.basis_scale != other.basis_scale
            or self.payout_denominator != other.payout_denominator
            or len(self.masses) != len(other.masses)
        ):
            raise Refusal("summary-domain")
        if self.start_bucket is None:
            return other
        if other.start_bucket is None:
            return self
        if self.end_bucket_exclusive != other.start_bucket:
            raise Refusal("nonadjacent")
        masses = tuple(left + right for left, right in zip(self.masses, other.masses))
        duration = self.duration + other.duration
        if sum(masses) != self.basis_scale * duration:
            raise AssertionError("exact occupation lost partition of unity")
        return ExactBasisOccupationSummary(
            self.spec_identity,
            self.start_bucket,
            other.end_bucket_exclusive,
            duration,
            masses,
            self.basis_scale,
            self.payout_denominator,
        )

    def finalize(self) -> tuple[int, ...]:
        if self.duration <= 0:
            raise Refusal("empty-occupation")
        average = tuple(
            Fraction(mass, self.basis_scale * self.duration) for mass in self.masses
        )
        return quantize_simplex(average, self.payout_denominator)


def fold_summaries(items: Iterable[object], empty: object) -> object:
    """Left fold for any summary exposing the same partial ``combine`` law."""

    result = empty
    for item in items:
        result = result.combine(item)  # type: ignore[attr-defined]
    return result


@dataclass(frozen=True)
class ModeComparison:
    """Exact outputs for one point path under the three path interpretations."""

    evaluate_at_twap: tuple[int, ...]
    quantized_basis_occupation: tuple[int, ...]
    exact_basis_occupation: tuple[int, ...]
    price_time_integral: int
    duration: int


def compare_path_modes(
    spec: BasisSpec, observations: Sequence[tuple[int, int, int]]
) -> ModeComparison:
    """Compare ``(bucket, duration, point)`` observations without floats."""

    twap_parts = [
        TwapSummary.from_point(spec, bucket, duration, point)
        for bucket, duration, point in observations
    ]
    quantized_parts = [
        QuantizedOccupationSummary.from_point(spec, bucket, duration, point)
        for bucket, duration, point in observations
    ]
    exact_parts = [
        ExactBasisOccupationSummary.from_point(spec, bucket, duration, point)
        for bucket, duration, point in observations
    ]
    twap = fold_summaries(twap_parts, TwapSummary.empty(spec))
    quantized = fold_summaries(
        quantized_parts, QuantizedOccupationSummary.empty(spec)
    )
    exact = fold_summaries(exact_parts, ExactBasisOccupationSummary.empty(spec))
    assert isinstance(twap, TwapSummary)
    assert isinstance(quantized, QuantizedOccupationSummary)
    assert isinstance(exact, ExactBasisOccupationSummary)
    return ModeComparison(
        twap.finalize(spec),
        quantized.finalize(),
        exact.finalize(),
        twap.price_time_integral,
        twap.duration,
    )


def liability_numerator(weights: Sequence[int], supplies: Sequence[int]) -> int:
    """Exact numerator ``sum_i w_i*T_i`` used by the solvency checks."""

    if len(weights) != len(supplies) or any(value < 0 for value in supplies):
        raise Refusal("liability-shape")
    return sum(weight * supply for weight, supply in zip(weights, supplies))


def assert_simplex_solvency(
    weights: Sequence[int], denominator: int, supplies: Sequence[int]
) -> None:
    """Executable form of ``sum w_i*T_i / D <= max_i T_i``."""

    if denominator <= 0 or any(weight < 0 for weight in weights):
        raise Refusal("weight-shape")
    if sum(weights) != denominator or not supplies:
        raise Refusal("weight-simplex")
    numerator = liability_numerator(weights, supplies)
    if numerator > denominator * max(supplies):
        raise AssertionError("simplex liability exceeded max supply")
