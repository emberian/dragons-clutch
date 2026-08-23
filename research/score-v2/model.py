"""Exact host-only model for the proposed quotient-risk ScoreV2.

This module does not mirror consensus bytes.  It isolates the economic score
from price quality, fees, solver compensation, and candidate validity so each
claim can be falsified independently.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Iterable, Sequence


MIN_OUTCOMES = 2
MAX_OUTCOMES = 16
U64_MAX = (1 << 64) - 1
U128_MAX = (1 << 128) - 1


class ModelError(ValueError):
    """The modeled input is outside the frozen V1 integer envelope."""


class Side(Enum):
    BUY = "buy"
    SELL = "sell"


@dataclass(frozen=True)
class ExecutedLeg:
    """One same-outcome fill used only by the owner-normalization falsifier."""

    owner: str
    outcome: int
    side: Side
    quantity: int


@dataclass(frozen=True)
class RiskObjectiveV2:
    """The complete-set-quotiented economic objective.

    ``certified_risk_flow_atoms`` is the range of aggregate direct outcome
    flow.  It is the least total range-risk compatible with an owner-blind
    decomposition of that flow.
    """

    certified_risk_flow_atoms: int

    @classmethod
    def from_direct_flow(cls, direct_flow: Sequence[int]) -> "RiskObjectiveV2":
        flow = validate_direct_flow(direct_flow)
        return cls(max(flow) - min(flow))

    def is_better_than(self, other: "RiskObjectiveV2") -> bool:
        return self.certified_risk_flow_atoms > other.certified_risk_flow_atoms


@dataclass(frozen=True)
class SelectionKeyV2:
    """A total order around, but distinct from, the economic objective.

    Directions are:

    1. maximize quotient risk;
    2. minimize the directly crossed complete-set layer;
    3. minimize virtual split/merge churn; and
    4. prefer the lexicographically smaller full candidate digest.

    Fields two through four are representation/cost canonicalizers.  They are
    not additional claims about useful risk, price quality, or personhood.
    """

    objective: RiskObjectiveV2
    cash_equivalent_direct_flow_atoms: int
    virtual_churn_atoms: int
    digest: bytes

    @classmethod
    def from_candidate(
        cls,
        direct_flow: Sequence[int],
        virtual_split: int,
        virtual_merge: int,
        digest: bytes,
    ) -> "SelectionKeyV2":
        flow = validate_direct_flow(direct_flow)
        split = validate_u64(virtual_split, "virtual split")
        merge = validate_u64(virtual_merge, "virtual merge")
        churn = split + merge
        if churn > U64_MAX:
            raise ModelError("virtual churn exceeds u64")
        if not isinstance(digest, bytes) or len(digest) != 32:
            raise ModelError("candidate digest must be exactly 32 bytes")
        return cls(
            objective=RiskObjectiveV2.from_direct_flow(flow),
            cash_equivalent_direct_flow_atoms=min(flow),
            virtual_churn_atoms=churn,
            digest=digest,
        )

    def compare(self, other: "SelectionKeyV2") -> int:
        """Return positive when ``self`` is preferred, negative when worse."""

        left = self.objective.certified_risk_flow_atoms
        right = other.objective.certified_risk_flow_atoms
        if left != right:
            return 1 if left > right else -1
        if (
            self.cash_equivalent_direct_flow_atoms
            != other.cash_equivalent_direct_flow_atoms
        ):
            return (
                1
                if self.cash_equivalent_direct_flow_atoms
                < other.cash_equivalent_direct_flow_atoms
                else -1
            )
        if self.virtual_churn_atoms != other.virtual_churn_atoms:
            return 1 if self.virtual_churn_atoms < other.virtual_churn_atoms else -1
        if self.digest != other.digest:
            return 1 if self.digest < other.digest else -1
        return 0

    def is_better_than(self, other: "SelectionKeyV2") -> bool:
        return self.compare(other) > 0


def validate_u64(value: int, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise ModelError(f"{label} must be an integer")
    if value < 0 or value > U64_MAX:
        raise ModelError(f"{label} is outside u64")
    return value


def validate_direct_flow(direct_flow: Sequence[int]) -> tuple[int, ...]:
    flow = tuple(direct_flow)
    if not MIN_OUTCOMES <= len(flow) <= MAX_OUTCOMES:
        raise ModelError("outcome count is outside 2..=16")
    return tuple(validate_u64(value, "direct flow") for value in flow)


def quotient_representative(direct_flow: Sequence[int]) -> tuple[int, ...]:
    """Return the unique min-zero nonnegative representative of the flow."""

    flow = validate_direct_flow(direct_flow)
    cash_layer = min(flow)
    return tuple(value - cash_layer for value in flow)


def objective_from_padded_flow(
    padded_direct_flow: Sequence[int], outcome_count: int
) -> RiskObjectiveV2:
    """Score an active prefix and require the fixed-array padding to be zero."""

    padded = tuple(padded_direct_flow)
    if len(padded) != MAX_OUTCOMES:
        raise ModelError("padded direct flow must have exactly 16 cells")
    if not MIN_OUTCOMES <= outcome_count <= MAX_OUTCOMES:
        raise ModelError("outcome count is outside 2..=16")
    checked = tuple(validate_u64(value, "padded direct flow") for value in padded)
    if any(checked[outcome_count:]):
        raise ModelError("inactive direct-flow padding must be zero")
    return RiskObjectiveV2.from_direct_flow(checked[:outcome_count])


def direct_flow_from_buy_side(
    aggregate_buy_flow: Sequence[int], virtual_split: int
) -> tuple[int, ...]:
    """Derive the relation identity ``d_i = B_i - sigma`` exactly."""

    buys = validate_direct_flow(aggregate_buy_flow)
    split = validate_u64(virtual_split, "virtual split")
    if any(value < split for value in buys):
        raise ModelError("virtual split exceeds an aggregate buy coordinate")
    return tuple(value - split for value in buys)


def direct_flow_from_sell_side(
    aggregate_sell_flow: Sequence[int], virtual_merge: int
) -> tuple[int, ...]:
    """Derive the equivalent relation identity ``d_i = E_i - mu``."""

    sells = validate_direct_flow(aggregate_sell_flow)
    merge = validate_u64(virtual_merge, "virtual merge")
    if any(value < merge for value in sells):
        raise ModelError("virtual merge exceeds an aggregate sell coordinate")
    return tuple(value - merge for value in sells)


def price_weighted_gini_numerator(
    direct_flow: Sequence[int], prices: Sequence[int], price_scale: int
) -> int:
    """Report ``sum p_i p_j |d_i-d_j|`` without using it for selection.

    This is the existing state-contingent Gini geometry.  It is quotient
    invariant, but it directly depends on a candidate-controlled price and has
    a zero-price kernel, so ScoreV2-Q deliberately keeps it observational.
    """

    flow = validate_direct_flow(direct_flow)
    if len(prices) != len(flow):
        raise ModelError("price and direct-flow widths differ")
    scale = validate_u64(price_scale, "price scale")
    if scale == 0:
        raise ModelError("price scale must be positive")
    checked_prices = tuple(validate_u64(price, "price") for price in prices)
    if any(price > scale for price in checked_prices):
        raise ModelError("price exceeds price scale")
    if sum(checked_prices) != scale:
        raise ModelError("prices do not lie on the exact simplex")
    numerator = 0
    for left in range(len(flow)):
        for right in range(left + 1, len(flow)):
            numerator += (
                checked_prices[left]
                * checked_prices[right]
                * abs(flow[left] - flow[right])
            )
    if numerator > U128_MAX:
        raise ModelError("Gini numerator exceeds u128")
    return numerator


def score_v1_primary(
    direct_flow: Sequence[int], prices: Sequence[int], price_scale: int
) -> int:
    """The owner-overlap-free ScoreV1 primary used by its known wash fixture."""

    flow = validate_direct_flow(direct_flow)
    if len(prices) != len(flow):
        raise ModelError("price and direct-flow widths differ")
    scale = validate_u64(price_scale, "price scale")
    if scale == 0:
        raise ModelError("price scale must be positive")
    checked_prices = tuple(validate_u64(price, "price") for price in prices)
    if any(price > scale for price in checked_prices):
        raise ModelError("price exceeds price scale")
    if sum(checked_prices) != scale:
        raise ModelError("prices do not lie on the exact simplex")
    return sum(
        price * (scale - price) * quantity
        for quantity, price in zip(flow, checked_prices)
    )


def aggregate_vectors(vectors: Iterable[Sequence[int]]) -> tuple[int, ...]:
    """Aggregate a decomposition, refusing width drift and u64 overflow."""

    iterator = iter(vectors)
    try:
        aggregate = list(validate_direct_flow(next(iterator)))
    except StopIteration as error:
        raise ModelError("at least one vector is required") from error
    for vector in iterator:
        checked = validate_direct_flow(vector)
        if len(checked) != len(aggregate):
            raise ModelError("decomposition widths differ")
        for index, value in enumerate(checked):
            total = aggregate[index] + value
            if total > U64_MAX:
                raise ModelError("aggregate direct flow exceeds u64")
            aggregate[index] = total
    return tuple(aggregate)


def owner_normalized_direct_flow(
    outcome_count: int, legs: Iterable[ExecutedLeg]
) -> tuple[int, ...]:
    """Model the V1 same-owner/same-outcome cancellation boundary.

    This function is a counterexample generator, not a ScoreV2 dependency.  It
    shows why removing ``distinct_owners`` from the score cannot by itself make
    a relation Sybil-neutral while owner-aware normalization changes which
    fills exist.
    """

    if not MIN_OUTCOMES <= outcome_count <= MAX_OUTCOMES:
        raise ModelError("outcome count is outside 2..=16")
    table: dict[tuple[str, int], list[int]] = {}
    for leg in legs:
        if not isinstance(leg.owner, str) or not leg.owner:
            raise ModelError("owner must be a nonempty label")
        if not 0 <= leg.outcome < outcome_count:
            raise ModelError("leg outcome is outside the market")
        quantity = validate_u64(leg.quantity, "leg quantity")
        cell = table.setdefault((leg.owner, leg.outcome), [0, 0])
        side = 0 if leg.side is Side.BUY else 1
        cell[side] += quantity
        if cell[side] > U64_MAX:
            raise ModelError("owner participation exceeds u64")

    buys = [0] * outcome_count
    sells = [0] * outcome_count
    for (_, outcome), (buy, sell) in table.items():
        overlap = min(buy, sell)
        buys[outcome] += buy - overlap
        sells[outcome] += sell - overlap
        if buys[outcome] > U64_MAX or sells[outcome] > U64_MAX:
            raise ModelError("normalized participation exceeds u64")
    if buys != sells:
        raise ModelError("modeled legs do not conserve by outcome")
    return tuple(buys)


def indistinguishable_owner_worlds(
    honest_keys: Sequence[str], sybil_keys: Sequence[str]
) -> bool:
    """State the identity impossibility for identical public key transcripts."""

    return tuple(honest_keys) == tuple(sybil_keys)
