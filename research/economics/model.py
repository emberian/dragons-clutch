# SPDX-License-Identifier: AGPL-3.0-or-later
"""Exact, host-only models for the Dragon's Clutch economics laboratory.

This module is deliberately independent of the protocol implementation.  It uses
integers and ``fractions.Fraction`` only; none of its types are consensus types.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
from enum import Enum
from fractions import Fraction
from itertools import product
from typing import Iterable, Optional, Sequence


class ModelError(ValueError):
    """A refused transition in the host-only reference model."""


class Pool(str, Enum):
    HOARD = "hoard"
    ORDER = "order"
    FEE = "fee"
    LIVENESS_SOL = "liveness_sol"
    KEEPER_REWARD = "keeper_reward"
    RENT_SOL = "rent_sol"
    TREASURY = "treasury"


ALLOWED_POOL_PURPOSES = {
    Pool.HOARD: frozenset({"merge", "redemption"}),
    Pool.ORDER: frozenset({"match", "cancel"}),
    Pool.FEE: frozenset({"maker_rebate", "batch_executor", "protocol"}),
    Pool.LIVENESS_SOL: frozenset(
        {"observation", "repair", "resolution", "finalization", "cleanup"}
    ),
    Pool.KEEPER_REWARD: frozenset(
        {"observation", "repair", "resolution", "finalization", "cleanup"}
    ),
    Pool.RENT_SOL: frozenset({"account_create", "valid_close_refund"}),
    Pool.TREASURY: frozenset({"operations", "liveness_top_up"}),
}


@dataclass(frozen=True)
class ProtectedPools:
    """Separate accounting identities, modeled without token conversion."""

    hoard: int = 0
    order: int = 0
    fee: int = 0
    liveness_sol: int = 0
    keeper_reward: int = 0
    rent_sol: int = 0
    treasury: int = 0

    def __post_init__(self) -> None:
        if any(value < 0 for value in self.as_tuple()):
            raise ModelError("pool balance cannot be negative")

    def as_tuple(self) -> tuple[int, ...]:
        return (
            self.hoard,
            self.order,
            self.fee,
            self.liveness_sol,
            self.keeper_reward,
            self.rent_sol,
            self.treasury,
        )

    def balance(self, pool: Pool) -> int:
        return int(getattr(self, pool.value))

    def credit(self, pool: Pool, amount: int) -> "ProtectedPools":
        _require_nonnegative(amount, "credit")
        return replace(self, **{pool.value: self.balance(pool) + amount})

    def debit(self, pool: Pool, purpose: str, amount: int) -> "ProtectedPools":
        _require_nonnegative(amount, "debit")
        if purpose not in ALLOWED_POOL_PURPOSES[pool]:
            raise ModelError(f"{pool.value} cannot pay {purpose}")
        if amount > self.balance(pool):
            raise ModelError("insufficient protected-pool balance")
        return replace(self, **{pool.value: self.balance(pool) - amount})


def _require_nonnegative(value: int, name: str) -> None:
    if value < 0:
        raise ModelError(f"{name} cannot be negative")


def maximum_liability(
    supplies: Sequence[int], payout_vectors: Iterable[Sequence[Fraction]]
) -> Fraction:
    """Return the largest liability over a finite frozen payout set."""

    vectors = tuple(tuple(vector) for vector in payout_vectors)
    if not vectors:
        raise ModelError("payout-vector set cannot be empty")
    if any(supply < 0 for supply in supplies):
        raise ModelError("supply cannot be negative")
    liabilities: list[Fraction] = []
    for vector in vectors:
        if len(vector) != len(supplies):
            raise ModelError("payout-vector width mismatch")
        if any(weight < 0 for weight in vector):
            raise ModelError("negative payout weight")
        if sum(vector, Fraction(0)) > 1:
            raise ModelError("payout vector exceeds one collateral unit")
        liabilities.append(
            sum(
                (Fraction(supply) * weight for supply, weight in zip(supplies, vector)),
                Fraction(0),
            )
        )
    return max(liabilities)


def one_hot_vectors(outcomes: int) -> tuple[tuple[Fraction, ...], ...]:
    if outcomes < 2:
        raise ModelError("categorical market needs at least two outcomes")
    return tuple(
        tuple(Fraction(1 if i == winner else 0) for i in range(outcomes))
        for winner in range(outcomes)
    )


@dataclass(frozen=True)
class CategoricalMarket:
    """Small reachable-state model for split/burn/merge/resolve/redeem."""

    hoard: int
    supplies: tuple[int, ...]
    winner: Optional[int] = None

    def __post_init__(self) -> None:
        if len(self.supplies) < 2:
            raise ModelError("categorical market needs at least two outcomes")
        if self.hoard < 0 or any(supply < 0 for supply in self.supplies):
            raise ModelError("negative market amount")
        if self.winner is not None and not 0 <= self.winner < len(self.supplies):
            raise ModelError("winner out of range")

    @classmethod
    def empty(cls, outcomes: int) -> "CategoricalMarket":
        return cls(0, (0,) * outcomes)

    def required_collateral(self) -> int:
        if self.winner is None:
            return max(self.supplies)
        return self.supplies[self.winner]

    def is_solvent(self) -> bool:
        return self.hoard >= self.required_collateral()

    def split(self, quantity: int) -> "CategoricalMarket":
        self._require_open()
        _require_positive(quantity, "split quantity")
        return replace(
            self,
            hoard=self.hoard + quantity,
            supplies=tuple(supply + quantity for supply in self.supplies),
        )

    def merge(self, quantity: int) -> "CategoricalMarket":
        self._require_open()
        _require_positive(quantity, "merge quantity")
        if quantity > min(self.supplies) or quantity > self.hoard:
            raise ModelError("incomplete clutch")
        return replace(
            self,
            hoard=self.hoard - quantity,
            supplies=tuple(supply - quantity for supply in self.supplies),
        )

    def burn(self, outcome: int, quantity: int) -> "CategoricalMarket":
        self._require_outcome(outcome)
        _require_positive(quantity, "burn quantity")
        if quantity > self.supplies[outcome]:
            raise ModelError("burn exceeds supply")
        supplies = list(self.supplies)
        supplies[outcome] -= quantity
        return replace(self, supplies=tuple(supplies))

    def resolve(self, winner: int) -> "CategoricalMarket":
        self._require_open()
        self._require_outcome(winner)
        return replace(self, winner=winner)

    def redeem(self, outcome: int, quantity: int) -> "CategoricalMarket":
        if self.winner is None:
            raise ModelError("market is unresolved")
        self._require_outcome(outcome)
        _require_positive(quantity, "redeem quantity")
        if quantity > self.supplies[outcome]:
            raise ModelError("redemption exceeds supply")
        payout = quantity if outcome == self.winner else 0
        if payout > self.hoard:
            raise ModelError("redemption exceeds Hoard")
        supplies = list(self.supplies)
        supplies[outcome] -= quantity
        return replace(self, hoard=self.hoard - payout, supplies=tuple(supplies))

    def _require_open(self) -> None:
        if self.winner is not None:
            raise ModelError("market is already resolved")

    def _require_outcome(self, outcome: int) -> None:
        if not 0 <= outcome < len(self.supplies):
            raise ModelError("outcome out of range")


def _require_positive(value: int, name: str) -> None:
    if value <= 0:
        raise ModelError(f"{name} must be positive")


def enumerate_solvency_traces(
    outcomes: int = 3, depth: int = 7, hoard_cap: int = 5
) -> dict[str, int]:
    """Exhaustively enumerate a bounded transition graph and check solvency."""

    initial = CategoricalMarket.empty(outcomes)
    seen = {initial}
    frontier = {initial}
    checked_transitions = 0
    for _ in range(depth):
        next_frontier: set[CategoricalMarket] = set()
        for state in sorted(frontier, key=repr):
            candidates = []
            if state.winner is None:
                if state.hoard < hoard_cap:
                    candidates.append(lambda s=state: s.split(1))
                candidates.append(lambda s=state: s.merge(1))
                for winner in range(outcomes):
                    candidates.append(lambda winner=winner, s=state: s.resolve(winner))
            for outcome in range(outcomes):
                candidates.append(lambda outcome=outcome, s=state: s.burn(outcome, 1))
                if state.winner is not None:
                    candidates.append(
                        lambda outcome=outcome, s=state: s.redeem(outcome, 1)
                    )
            for transition in candidates:
                try:
                    new_state = transition()
                except ModelError:
                    continue
                checked_transitions += 1
                if not new_state.is_solvent():
                    raise AssertionError(f"insolvent reachable state: {new_state!r}")
                if new_state not in seen:
                    seen.add(new_state)
                    next_frontier.add(new_state)
        frontier = next_frontier
        if not frontier:
            break
    return {"states": len(seen), "transitions": checked_transitions}


@dataclass(frozen=True, order=True)
class LivenessJob:
    job_id: str
    max_sol: int
    max_reward: int = 0

    def __post_init__(self) -> None:
        _require_nonnegative(self.max_sol, "maximum SOL bounty")
        _require_nonnegative(self.max_reward, "maximum reward bounty")


@dataclass(frozen=True)
class LivenessBook:
    sol_balance: int
    reward_balance: int
    jobs: tuple[LivenessJob, ...] = ()
    completed: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        _require_nonnegative(self.sol_balance, "SOL balance")
        _require_nonnegative(self.reward_balance, "reward balance")
        ids = [job.job_id for job in self.jobs]
        if len(ids) != len(set(ids)) or set(ids).intersection(self.completed):
            raise ModelError("duplicate liveness job")
        if self.free_sol() < 0 or self.free_reward() < 0:
            raise ModelError("liveness book is undercapitalized")

    def booked_sol(self) -> int:
        return sum(job.max_sol for job in self.jobs)

    def booked_reward(self) -> int:
        return sum(job.max_reward for job in self.jobs)

    def free_sol(self) -> int:
        return self.sol_balance - self.booked_sol()

    def free_reward(self) -> int:
        return self.reward_balance - self.booked_reward()

    def book(self, job: LivenessJob) -> "LivenessBook":
        if job.job_id in self.completed or any(j.job_id == job.job_id for j in self.jobs):
            raise ModelError("job already known")
        if job.max_sol > self.free_sol() or job.max_reward > self.free_reward():
            raise ModelError("future job is not fully prepaid")
        return replace(self, jobs=tuple(sorted((*self.jobs, job))))

    def complete(
        self, job_id: str, paid_sol: int, paid_reward: int = 0
    ) -> "LivenessBook":
        _require_nonnegative(paid_sol, "paid SOL")
        _require_nonnegative(paid_reward, "paid reward")
        matching = [job for job in self.jobs if job.job_id == job_id]
        if len(matching) != 1:
            raise ModelError("job is not unfinished")
        job = matching[0]
        if paid_sol > job.max_sol or paid_reward > job.max_reward:
            raise ModelError("payment exceeds frozen job maximum")
        remaining = tuple(item for item in self.jobs if item.job_id != job_id)
        return replace(
            self,
            sol_balance=self.sol_balance - paid_sol,
            reward_balance=self.reward_balance - paid_reward,
            jobs=remaining,
            completed=tuple(sorted((*self.completed, job_id))),
        )


@dataclass(frozen=True)
class ReverseDutchSchedule:
    offers: tuple[int, ...]

    def __post_init__(self) -> None:
        if not self.offers:
            raise ModelError("bounty schedule cannot be empty")
        if any(offer < 0 for offer in self.offers):
            raise ModelError("negative bounty")
        if any(left > right for left, right in zip(self.offers, self.offers[1:])):
            raise ModelError("bounty schedule must be monotone")

    @property
    def booked_maximum(self) -> int:
        return self.offers[-1]

    def offer(self, step: int) -> int:
        if not 0 <= step < len(self.offers):
            raise ModelError("bounty step out of range")
        return self.offers[step]


def integer_shares(total: int, participants: int) -> tuple[int, ...]:
    """Deterministically allocate atoms with at most one atom of dispersion."""

    _require_nonnegative(total, "share total")
    if participants <= 0:
        raise ModelError("participants must be positive")
    quotient, remainder = divmod(total, participants)
    return tuple(
        quotient + (1 if index < remainder else 0)
        for index in range(participants)
    )


@dataclass(frozen=True)
class JoinResult:
    subscriber_count: int
    deposit: int
    reimbursements: tuple[int, ...]
    capital_shares: tuple[int, ...]


@dataclass(frozen=True)
class EpochSettlement:
    success: bool
    keeper_paid: int
    subscriber_costs: tuple[int, ...]
    subscriber_refunds: tuple[int, ...]
    neutral_reserve_roll: int


@dataclass(frozen=True)
class SharedFeedEpoch:
    """Integer-atom shadow of equal shared-feed reserve capitalization."""

    reserve_cap: int
    capital_shares: tuple[int, ...]

    def __post_init__(self) -> None:
        _require_nonnegative(self.reserve_cap, "reserve cap")
        if not self.capital_shares:
            raise ModelError("feed epoch needs a subscriber")
        expected = integer_shares(self.reserve_cap, len(self.capital_shares))
        if self.capital_shares != expected:
            raise ModelError("capital shares are not canonical")

    @classmethod
    def first(cls, reserve_cap: int) -> "SharedFeedEpoch":
        return cls(reserve_cap, integer_shares(reserve_cap, 1))

    def join(self) -> tuple["SharedFeedEpoch", JoinResult]:
        old = self.capital_shares
        new = integer_shares(self.reserve_cap, len(old) + 1)
        reimbursements = tuple(old_value - new_value for old_value, new_value in zip(old, new))
        if any(value < 0 for value in reimbursements):
            raise AssertionError("existing subscriber share increased")
        deposit = new[-1]
        if sum(reimbursements) != deposit:
            raise AssertionError("join deposit does not fund reimbursements")
        epoch = SharedFeedEpoch(self.reserve_cap, new)
        return epoch, JoinResult(len(new), deposit, reimbursements, new)

    def settle(self, keeper_paid: int, success: bool) -> EpochSettlement:
        _require_nonnegative(keeper_paid, "keeper payment")
        if keeper_paid > self.reserve_cap:
            raise ModelError("keeper payment exceeds reserve cap")
        if success:
            costs = integer_shares(keeper_paid, len(self.capital_shares))
            refunds = tuple(
                capital - cost for capital, cost in zip(self.capital_shares, costs)
            )
            return EpochSettlement(True, keeper_paid, costs, refunds, 0)
        return EpochSettlement(
            False,
            keeper_paid,
            self.capital_shares,
            (0,) * len(self.capital_shares),
            self.reserve_cap - keeper_paid,
        )


def compatible_payout(outcomes: int, compatible: Sequence[int]) -> tuple[Fraction, ...]:
    if outcomes < 2:
        raise ModelError("categorical market needs at least two outcomes")
    unique = tuple(sorted(set(compatible)))
    if not unique or unique != tuple(compatible):
        raise ModelError("compatible outcomes must be nonempty, unique, and ordered")
    if unique[0] < 0 or unique[-1] >= outcomes:
        raise ModelError("compatible outcome out of range")
    weight = Fraction(1, len(unique))
    return tuple(weight if index in unique else Fraction(0) for index in range(outcomes))


def portfolio_payout(holdings: Sequence[int], payout: Sequence[Fraction]) -> Fraction:
    if len(holdings) != len(payout):
        raise ModelError("portfolio width mismatch")
    if any(holding < 0 for holding in holdings):
        raise ModelError("negative holding")
    return sum(
        (Fraction(holding) * weight for holding, weight in zip(holdings, payout)),
        Fraction(0),
    )


def dominant_tail_attack(
    outcomes: int, cap: int, tail_price_mass: Fraction
) -> dict[str, Fraction]:
    """Cost and payoff for buying every nonwinner Egg before equal failure."""

    if outcomes < 2 or cap < 0:
        raise ModelError("invalid attack dimensions")
    if not 0 <= tail_price_mass <= 1:
        raise ModelError("tail price mass must be in [0,1]")
    fallback_payoff = Fraction(cap * (outcomes - 1), outcomes)
    acquisition_cost = Fraction(cap) * tail_price_mass
    return {
        "fallback_payoff": fallback_payoff,
        "acquisition_cost": acquisition_cost,
        "net_gain": fallback_payoff - acquisition_cost,
        "single_tail_gross_gain": Fraction(cap, outcomes),
    }


def common_mode_exposure(
    market_caps: Sequence[int], maximum_payout_changes: Sequence[Fraction]
) -> Fraction:
    if len(market_caps) != len(maximum_payout_changes):
        raise ModelError("common-mode input width mismatch")
    if any(cap < 0 for cap in market_caps):
        raise ModelError("negative market cap")
    if any(not 0 <= change <= 1 for change in maximum_payout_changes):
        raise ModelError("payout change must be in [0,1]")
    return sum(
        (
            Fraction(cap) * change
            for cap, change in zip(market_caps, maximum_payout_changes)
        ),
        Fraction(0),
    )


def exposure_admissible(
    exposure: Fraction, manipulation_cost_lower_bound: Fraction, numerator: int = 1, denominator: int = 10
) -> bool:
    if exposure < 0 or manipulation_cost_lower_bound < 0:
        raise ModelError("negative exposure or manipulation cost")
    if numerator < 0 or denominator <= 0:
        raise ModelError("invalid exposure fraction")
    return exposure * denominator <= manipulation_cost_lower_bound * numerator


def dispersion_numerator(payoffs: Sequence[int], prices: Sequence[int]) -> int:
    if len(payoffs) != len(prices) or len(payoffs) < 2:
        raise ModelError("dispersion vectors must have equal width >= 2")
    if any(payoff < 0 for payoff in payoffs) or any(price < 0 for price in prices):
        raise ModelError("negative payoff or price")
    total = 0
    for left in range(len(payoffs)):
        for right in range(left + 1, len(payoffs)):
            total += prices[left] * prices[right] * abs(payoffs[left] - payoffs[right])
    return total


def single_egg_dispersion_numerator(quantity: int, price: int, price_scale: int) -> int:
    _require_nonnegative(quantity, "quantity")
    if price_scale <= 0 or not 0 <= price <= price_scale:
        raise ModelError("invalid simplex price")
    return quantity * price * (price_scale - price)


def fee_with_carry(
    dispersion_num: int,
    price_scale: int,
    kappa_num: int,
    kappa_den: int,
    carry: int = 0,
) -> tuple[int, int]:
    _require_nonnegative(dispersion_num, "dispersion numerator")
    _require_nonnegative(carry, "carry")
    if price_scale <= 0 or kappa_num < 0 or kappa_den <= 0:
        raise ModelError("invalid fee scale")
    denominator = kappa_den * price_scale * price_scale
    if carry >= denominator:
        raise ModelError("noncanonical fee carry")
    return divmod(kappa_num * dispersion_num + carry, denominator)


def stateless_ceil_fee(
    dispersion_num: int, price_scale: int, kappa_num: int, kappa_den: int
) -> int:
    _require_nonnegative(dispersion_num, "dispersion numerator")
    if price_scale <= 0 or kappa_num < 0 or kappa_den <= 0:
        raise ModelError("invalid fee scale")
    numerator = kappa_num * dispersion_num
    denominator = kappa_den * price_scale * price_scale
    return (numerator + denominator - 1) // denominator


@dataclass(frozen=True)
class FeeAllocation:
    maker: int
    executor: int
    treasury: int

    @property
    def total(self) -> int:
        return self.maker + self.executor + self.treasury


def allocate_fee(
    fee: int,
    maker_num: int = 60,
    executor_num: int = 15,
    denominator: int = 100,
) -> FeeAllocation:
    _require_nonnegative(fee, "fee")
    if denominator <= 0 or maker_num < 0 or executor_num < 0:
        raise ModelError("invalid allocation shares")
    if maker_num + executor_num > denominator:
        raise ModelError("allocation exceeds collected fee")
    maker = fee * maker_num // denominator
    executor = fee * executor_num // denominator
    treasury = fee - maker - executor
    return FeeAllocation(maker, executor, treasury)


def wash_cycle_loss(fee: int, network_cost: int = 0) -> int:
    allocation = allocate_fee(fee)
    _require_nonnegative(network_cost, "network cost")
    return fee - allocation.maker - allocation.executor + network_cost


def midpoint_effective_bps(kappa: Fraction) -> Fraction:
    """Gross fee divided by cash consideration at p=1/2, in basis points."""

    if kappa < 0:
        raise ModelError("negative kappa")
    return kappa * Fraction(1, 2) * 10_000


def maintenance_revenue_sol(
    weighted_volume: Fraction,
    kappa: Fraction,
    treasury_share: Fraction,
    sol_per_collateral: Fraction,
    service_premium_sol: Fraction = Fraction(0),
) -> Fraction:
    values = (weighted_volume, kappa, treasury_share, sol_per_collateral, service_premium_sol)
    if any(value < 0 for value in values):
        raise ModelError("negative break-even input")
    return treasury_share * kappa * weighted_volume * sol_per_collateral + service_premium_sol


def required_weighted_volume(
    operating_cost_sol: Fraction,
    service_premium_sol: Fraction,
    kappa: Fraction,
    treasury_share: Fraction,
    sol_per_collateral: Fraction,
) -> Optional[Fraction]:
    values = (
        operating_cost_sol,
        service_premium_sol,
        kappa,
        treasury_share,
        sol_per_collateral,
    )
    if any(value < 0 for value in values):
        raise ModelError("negative break-even input")
    uncovered = max(Fraction(0), operating_cost_sol - service_premium_sol)
    if uncovered == 0:
        return Fraction(0)
    denominator = kappa * treasury_share * sol_per_collateral
    if denominator == 0:
        return None
    return uncovered / denominator


def fee_fragmentation_result(
    quantities: Sequence[int],
    price: int,
    price_scale: int,
    kappa_num: int,
    kappa_den: int,
) -> dict[str, int]:
    if not quantities or any(quantity < 0 for quantity in quantities):
        raise ModelError("invalid quantities")
    carry = 0
    persistent_total = 0
    reset_total = 0
    ceil_total = 0
    for quantity in quantities:
        base = single_egg_dispersion_numerator(quantity, price, price_scale)
        paid, carry = fee_with_carry(base, price_scale, kappa_num, kappa_den, carry)
        persistent_total += paid
        reset_total += fee_with_carry(base, price_scale, kappa_num, kappa_den, 0)[0]
        ceil_total += stateless_ceil_fee(base, price_scale, kappa_num, kappa_den)
    whole_base = single_egg_dispersion_numerator(sum(quantities), price, price_scale)
    whole_floor, whole_carry = fee_with_carry(
        whole_base, price_scale, kappa_num, kappa_den, 0
    )
    return {
        "persistent_total": persistent_total,
        "persistent_carry": carry,
        "reset_total": reset_total,
        "ceil_total": ceil_total,
        "whole_floor": whole_floor,
        "whole_carry": whole_carry,
    }


def exhaustive_liveness_orders() -> dict[str, int]:
    """Check booking-order independence for a small fixed job family."""

    jobs = (
        LivenessJob("observe", 7, 3),
        LivenessJob("repair", 11, 5),
        LivenessJob("finalize", 13, 2),
    )
    admitted = 0
    refused = 0
    for mask in product((False, True), repeat=len(jobs)):
        wanted = tuple(job for selected, job in zip(mask, jobs) if selected)
        for sol_balance in range(0, 35):
            for reward_balance in range(0, 12):
                book = LivenessBook(sol_balance, reward_balance)
                accepted = True
                try:
                    for job in reversed(wanted):
                        book = book.book(job)
                except ModelError:
                    accepted = False
                should_accept = (
                    sum(job.max_sol for job in wanted) <= sol_balance
                    and sum(job.max_reward for job in wanted) <= reward_balance
                )
                if accepted != should_accept:
                    raise AssertionError("liveness admission depends on booking order")
                if accepted:
                    admitted += 1
                else:
                    refused += 1
    return {"admitted": admitted, "refused": refused}

