# SPDX-License-Identifier: AGPL-3.0-or-later
"""Exact, host-only models for the Dragon's Clutch economics laboratory.

This module is deliberately independent of the protocol implementation.  It uses
integers and ``fractions.Fraction`` only; none of its types are consensus types.

Two families live here:

* the original rational scenario models (pools, liveness, shared feed, fees);
* the kernel-mirror integer models added for
  ``docs/implementation/POLICY_ANALYSIS_LOTS_FEES.md`` sections 3.1-3.3.  Those
  mirror ``crates/clutch-kernel`` semantics with plain integers (never
  ``Fraction``) so that the same language-neutral fixtures can be replayed on
  both sides.  Every payout and fee policy arm in this file is MODEL or
  PROPOSED; none of them is a promoted protocol constant.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
from enum import Enum
from fractions import Fraction
from itertools import product
from math import gcd
from typing import Iterable, Mapping, Optional, Sequence


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
        if sum(vector, Fraction(0)) != 1:
            raise ModelError("payout weights must sum to exactly one collateral unit")
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
    executor_cap: Optional[int] = None,
) -> FeeAllocation:
    """Allocate a collected pot; rebates floor and treasury takes the remainder.

    ``executor_cap`` is the per-batch executor cap of POLICY_ANALYSIS section
    2.3; ``None`` means uncapped, which is the pre-existing behaviour.
    """

    _require_nonnegative(fee, "fee")
    if denominator <= 0 or maker_num < 0 or executor_num < 0:
        raise ModelError("invalid allocation shares")
    if maker_num + executor_num > denominator:
        raise ModelError("allocation exceeds collected fee")
    if executor_cap is not None:
        _require_nonnegative(executor_cap, "executor cap")
    maker = fee * maker_num // denominator
    executor = fee * executor_num // denominator
    if executor_cap is not None:
        executor = min(executor, executor_cap)
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
    denominator = fee_denominator(kappa_den, price_scale)
    accrued = kappa_num * whole_base
    return {
        "persistent_total": persistent_total,
        "persistent_carry": carry,
        "reset_total": reset_total,
        "ceil_total": ceil_total,
        "whole_floor": whole_floor,
        "whole_carry": whole_carry,
        # Third arm (POLICY_ANALYSIS section 2.2): persistent carry with one
        # terminal ceiling charge when the domain instance closes.
        "terminal_ceil_total": persistent_total + (1 if carry > 0 else 0),
        # Naive floor with the carry dropped at domain close: the arm the
        # terminal-ceil arm is measured against.
        "dropped_carry_total": persistent_total,
        "exact_ceil_total": -(-accrued // denominator),
        "denominator": denominator,
        "accrued_numerator": accrued,
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



# ---------------------------------------------------------------------------
# Kernel-mirror integer payout semantics
# (POLICY_ANALYSIS_LOTS_FEES.md sections 1, 3.1, 3.2 -- all MODEL/PROPOSED)
# ---------------------------------------------------------------------------

LAB_MAX_OUTCOMES = 16
LAB_MAX_PAYOUTS = 8
LAB_MIN_OUTCOMES = 2
LAB_AMOUNT_MAX = 2**64 - 1

#: Shared, language-neutral refusal vocabulary (POLICY_ANALYSIS section 3.4).
#: The first block mirrors ``clutch_kernel::Error`` one-for-one.  ``lot_violation``
#: and ``no_credit`` belong to the PROPOSED candidate (b) and (c) arms and have no
#: kernel counterpart today.
ERROR_CLASSES = (
    "invalid_outcome_count",
    "invalid_payout_count",
    "invalid_payout_index",
    "invalid_denominator",
    "invalid_payout_weights",
    "zero_quantity",
    "arithmetic_overflow",
    "arithmetic_underflow",
    "insufficient_balance",
    "insufficient_collateral",
    "not_active",
    "already_resolved",
    "not_resolved",
    "invariant_violation",
    "remainder_required",
    "lot_violation",
    "no_credit",
)


class KernelRefusal(ModelError):
    """A refusal carrying one member of the shared ``ERROR_CLASSES`` vocabulary."""

    def __init__(self, error_class: str, detail: str = "") -> None:
        if error_class not in ERROR_CLASSES:
            raise ModelError(f"unknown error class {error_class!r}")
        super().__init__(error_class if not detail else f"{error_class}: {detail}")
        self.error_class = error_class


class PayoutPolicy(str, Enum):
    """Admission/exit policy arms from POLICY_ANALYSIS section 1.

    ``KERNEL_BASELINE`` is the landed kernel behaviour and is retained only as a
    contrast arm: it is the arm under which the P1-A exit-dead trap is reachable.
    """

    KERNEL_BASELINE = "kernel_baseline"
    ONE_HOT = "one_hot"
    LOTS = "lots"
    CREDIT = "credit"


PAYOUT_POLICIES = tuple(policy.value for policy in PayoutPolicy)


def _gcd_all(values: Iterable[int], fallback: int) -> int:
    result = 0
    for value in values:
        result = gcd(result, value)
    return fallback if result == 0 else result


@dataclass(frozen=True)
class IntegerPayoutVector:
    """Integer mirror of ``clutch_kernel::PayoutVector`` (never ``Fraction``)."""

    denominator: int
    weights: tuple[int, ...]

    def __post_init__(self) -> None:
        if len(self.weights) > LAB_MAX_OUTCOMES:
            raise KernelRefusal("invalid_payout_weights", "too many weights")

    @classmethod
    def zero(cls) -> "IntegerPayoutVector":
        return cls(0, ())

    def weight(self, index: int) -> int:
        return self.weights[index] if index < len(self.weights) else 0

    def is_zero(self) -> bool:
        return self.denominator == 0 and all(weight == 0 for weight in self.weights)

    def validate(self, outcome_count: int) -> None:
        """Mirror ``PayoutVector::validate`` including its refusal ordering."""

        if self.denominator == 0:
            raise KernelRefusal("invalid_denominator")
        total = 0
        for index in range(LAB_MAX_OUTCOMES):
            weight = self.weight(index)
            if index >= outcome_count:
                if weight != 0:
                    raise KernelRefusal("invalid_payout_weights", "padding is nonzero")
            else:
                # The kernel weight type is u64; a negative weight is
                # unrepresentable there and is classified here as a weight defect.
                if weight < 0 or weight > self.denominator:
                    raise KernelRefusal("invalid_payout_weights", "weight out of range")
                total += weight
        if total != self.denominator:
            raise KernelRefusal("invalid_payout_weights", "weights do not sum to D")

    def is_one_hot(self, outcome_count: int) -> bool:
        return all(
            self.weight(index) in (0, self.denominator)
            for index in range(outcome_count)
        )


@dataclass(frozen=True)
class IntegerPayoutSet:
    """Integer mirror of ``clutch_kernel::PayoutSet``."""

    count: int
    outcomes: int
    vectors: tuple[IntegerPayoutVector, ...]

    def validate(self) -> None:
        """Mirror ``PayoutSet::validate`` including its refusal ordering."""

        if self.outcomes < LAB_MIN_OUTCOMES or self.outcomes > LAB_MAX_OUTCOMES:
            raise KernelRefusal("invalid_outcome_count")
        if self.count <= 0 or self.count > LAB_MAX_PAYOUTS:
            raise KernelRefusal("invalid_payout_count")
        if len(self.vectors) > LAB_MAX_PAYOUTS:
            raise KernelRefusal("invalid_payout_count", "too many vectors")
        if len(self.vectors) < self.count:
            raise KernelRefusal("invalid_payout_count", "fewer vectors than count")
        common = self.vectors[0].denominator
        for index in range(LAB_MAX_PAYOUTS):
            vector = (
                self.vectors[index]
                if index < len(self.vectors)
                else IntegerPayoutVector.zero()
            )
            if index < self.count:
                vector.validate(self.outcomes)
                if vector.denominator != common:
                    raise KernelRefusal("invalid_denominator", "mixed denominators")
            elif not vector.is_zero():
                raise KernelRefusal("invalid_payout_weights", "nonzero padding vector")

    @property
    def denominator(self) -> int:
        return self.vectors[0].denominator

    def active(self) -> tuple[IntegerPayoutVector, ...]:
        return tuple(self.vectors[: self.count])

    def vector(self, index: int) -> IntegerPayoutVector:
        if not 0 <= index < self.count:
            raise KernelRefusal("invalid_payout_index")
        return self.vectors[index]

    def is_one_hot(self) -> bool:
        return all(vector.is_one_hot(self.outcomes) for vector in self.active())

    def outcome_gcd(self, outcome: int) -> int:
        """``g_i = gcd(D, {v_i : v in P, v_i != 0})`` (``D`` when all are zero)."""

        denominator = self.denominator
        nonzero = [
            vector.weight(outcome)
            for vector in self.active()
            if vector.weight(outcome) != 0
        ]
        return _gcd_all([denominator, *nonzero], denominator)

    def redemption_lot(self, outcome: int) -> int:
        """``L_i = D / g_i`` -- POLICY_ANALYSIS section 1.1 fact 3."""

        return self.denominator // self.outcome_gcd(outcome)

    def split_lot(self) -> int:
        """``L_split = lcm_i L_i = D / gcd_i g_i``."""

        denominator = self.denominator
        combined = _gcd_all(
            (self.outcome_gcd(outcome) for outcome in range(self.outcomes)),
            denominator,
        )
        return denominator // combined


def payout_set(
    outcomes: int, rows: Sequence[Sequence[int]], denominator: int
) -> IntegerPayoutSet:
    """Build a payout set from dense weight rows sharing one denominator."""

    vectors = tuple(
        IntegerPayoutVector(denominator, tuple(int(weight) for weight in row))
        for row in rows
    )
    return IntegerPayoutSet(len(vectors), outcomes, vectors)


def one_hot_payout_set(outcomes: int, denominator: int = 1) -> IntegerPayoutSet:
    rows = [
        [denominator if index == winner else 0 for index in range(outcomes)]
        for winner in range(outcomes)
    ]
    return payout_set(outcomes, rows, denominator)


@dataclass(frozen=True)
class WeightedPosition:
    """One owner's internal/external claims plus its candidate (c) credit."""

    internal: tuple[int, ...]
    external: tuple[int, ...]
    credit: int = 0

    @classmethod
    def empty(cls, outcomes: int) -> "WeightedPosition":
        return cls((0,) * outcomes, (0,) * outcomes, 0)

    def balance(self, outcome: int, internal: bool) -> int:
        return self.internal[outcome] if internal else self.external[outcome]

    def with_balance(
        self, outcome: int, internal: bool, value: int
    ) -> "WeightedPosition":
        target = list(self.internal if internal else self.external)
        target[outcome] = value
        if internal:
            return replace(self, internal=tuple(target))
        return replace(self, external=tuple(target))


@dataclass(frozen=True)
class WeightedBook:
    """Kernel-mirror market plus a fixed roster of positions.

    Every transition is total and returns a new book; refusals raise
    :class:`KernelRefusal` carrying a shared error class.
    """

    payouts: IntegerPayoutSet
    policy: PayoutPolicy
    collateral: int
    total_supply: tuple[int, ...]
    positions: tuple[WeightedPosition, ...]
    resolved_payout: Optional[int] = None
    credit_total: int = 0

    # -- construction -------------------------------------------------------

    @classmethod
    def open(
        cls,
        payouts: IntegerPayoutSet,
        policy: PayoutPolicy = PayoutPolicy.KERNEL_BASELINE,
        collateral: int = 0,
        wallets: int = 1,
    ) -> "WeightedBook":
        """Admission: kernel shape rules first, then the policy-arm gate."""

        payouts.validate()
        if policy is PayoutPolicy.ONE_HOT and not payouts.is_one_hot():
            # Candidate (a1): tighten PayoutVector::validate, reusing
            # InvalidPayoutWeights (POLICY_ANALYSIS section 1.2).
            raise KernelRefusal("invalid_payout_weights", "non one-hot vector")
        if wallets < 1:
            raise ModelError("a book needs at least one position")
        book = cls(
            payouts=payouts,
            policy=policy,
            collateral=collateral,
            total_supply=(0,) * payouts.outcomes,
            positions=tuple(
                WeightedPosition.empty(payouts.outcomes) for _ in range(wallets)
            ),
        )
        book.check_invariants()
        return book

    @property
    def outcomes(self) -> int:
        return self.payouts.outcomes

    @property
    def denominator(self) -> int:
        return self.payouts.denominator

    @property
    def resolved(self) -> bool:
        return self.resolved_payout is not None

    def resolved_vector(self) -> IntegerPayoutVector:
        if self.resolved_payout is None:
            raise KernelRefusal("not_resolved")
        return self.payouts.vector(self.resolved_payout)

    # -- invariants ---------------------------------------------------------

    def liability_numerator(self, vector: IntegerPayoutVector) -> int:
        return sum(
            supply * vector.weight(index)
            for index, supply in enumerate(self.total_supply)
        )

    def required_collateral(self) -> int:
        """Ceiling-rounded maximum liability, mirroring ``required_collateral``.

        Under the candidate (c) arm the owed credit numerator joins the
        numerator, which is the POLICY_ANALYSIS section 1.4 invariant.
        """

        denominator = self.denominator
        extra = self.credit_total if self.policy is PayoutPolicy.CREDIT else 0
        if self.resolved_payout is None:
            vectors = self.payouts.active()
        else:
            vectors = (self.resolved_vector(),)
        return max(
            -(-(self.liability_numerator(vector) + extra) // denominator)
            for vector in vectors
        )

    def liability_is_integral(self) -> bool:
        """True when no ceiling is applied to any admissible liability."""

        denominator = self.denominator
        extra = self.credit_total if self.policy is PayoutPolicy.CREDIT else 0
        vectors = (
            self.payouts.active()
            if self.resolved_payout is None
            else (self.resolved_vector(),)
        )
        return all(
            (self.liability_numerator(vector) + extra) % denominator == 0
            for vector in vectors
        )

    def is_solvent(self) -> bool:
        return self.collateral >= self.required_collateral()

    def check_invariants(self) -> None:
        if not self.is_solvent():
            raise KernelRefusal("invariant_violation", "collateral below liability")

    # -- guards -------------------------------------------------------------

    def _require_active(self) -> None:
        if self.resolved_payout is not None:
            raise KernelRefusal("already_resolved")

    def _require_resolved(self) -> None:
        if self.resolved_payout is None:
            raise KernelRefusal("not_resolved")

    def _require_outcome(self, outcome: int) -> int:
        if not 0 <= outcome < self.outcomes:
            raise KernelRefusal("invalid_payout_index")
        return outcome

    def _require_wallet(self, wallet: int) -> int:
        if not 0 <= wallet < len(self.positions):
            raise ModelError("wallet out of range")
        return wallet

    @staticmethod
    def _require_quantity(quantity: int) -> int:
        if quantity < 0:
            raise KernelRefusal("arithmetic_underflow", "negative quantity")
        if quantity == 0:
            raise KernelRefusal("zero_quantity")
        if quantity > LAB_AMOUNT_MAX:
            raise KernelRefusal("arithmetic_overflow", "quantity exceeds u64")
        return quantity

    @staticmethod
    def _checked(value: int) -> int:
        if value > LAB_AMOUNT_MAX:
            raise KernelRefusal("arithmetic_overflow")
        return value

    def _require_lot(self, quantity: int, lot: int) -> None:
        if self.policy is PayoutPolicy.LOTS and quantity % lot != 0:
            raise KernelRefusal("lot_violation", f"quantity not a multiple of {lot}")

    # -- transitions --------------------------------------------------------

    def split(self, wallet: int, quantity: int) -> "WeightedBook":
        self._require_active()
        self._require_quantity(quantity)
        wallet = self._require_wallet(wallet)
        self._require_lot(quantity, self.payouts.split_lot())
        position = self.positions[wallet]
        supply = tuple(
            self._checked(value + quantity) for value in self.total_supply
        )
        internal = tuple(
            self._checked(value + quantity) for value in position.internal
        )
        book = replace(
            self,
            collateral=self._checked(self.collateral + quantity),
            total_supply=supply,
            positions=_replace_at(
                self.positions, wallet, replace(position, internal=internal)
            ),
        )
        book.check_invariants()
        return book

    def merge(self, wallet: int, quantity: int) -> "WeightedBook":
        self._require_active()
        self._require_quantity(quantity)
        wallet = self._require_wallet(wallet)
        self._require_lot(quantity, self.payouts.split_lot())
        if self.collateral < quantity:
            raise KernelRefusal("insufficient_collateral")
        position = self.positions[wallet]
        for index in range(self.outcomes):
            if (
                position.internal[index] < quantity
                or self.total_supply[index] < quantity
            ):
                raise KernelRefusal("insufficient_balance")
        book = replace(
            self,
            collateral=self.collateral - quantity,
            total_supply=tuple(value - quantity for value in self.total_supply),
            positions=_replace_at(
                self.positions,
                wallet,
                replace(
                    position,
                    internal=tuple(value - quantity for value in position.internal),
                ),
            ),
        )
        book.check_invariants()
        return book

    def materialize(self, wallet: int, outcome: int, quantity: int) -> "WeightedBook":
        return self._move_boundary(wallet, outcome, quantity, to_external=True)

    def dematerialize(self, wallet: int, outcome: int, quantity: int) -> "WeightedBook":
        return self._move_boundary(wallet, outcome, quantity, to_external=False)

    def _move_boundary(
        self, wallet: int, outcome: int, quantity: int, to_external: bool
    ) -> "WeightedBook":
        self._require_active()
        self._require_quantity(quantity)
        wallet = self._require_wallet(wallet)
        outcome = self._require_outcome(outcome)
        self._require_lot(quantity, self.payouts.redemption_lot(outcome))
        position = self.positions[wallet]
        source = position.internal if to_external else position.external
        if source[outcome] < quantity:
            raise KernelRefusal("insufficient_balance")
        internal = list(position.internal)
        external = list(position.external)
        if to_external:
            internal[outcome] -= quantity
            external[outcome] += quantity
        else:
            external[outcome] -= quantity
            internal[outcome] += quantity
        updated = replace(
            position, internal=tuple(internal), external=tuple(external)
        )
        book = replace(self, positions=_replace_at(self.positions, wallet, updated))
        book.check_invariants()
        return book

    def transfer_external(
        self, source: int, destination: int, outcome: int, quantity: int
    ) -> "WeightedBook":
        """Bearer Token-2022 transfer: never lot-gated (no transfer hook exists).

        This op has no kernel counterpart; it is the lab's model of the
        external-transfer hole named in POLICY_ANALYSIS section 1.3 (b1).
        """

        self._require_quantity(quantity)
        source = self._require_wallet(source)
        destination = self._require_wallet(destination)
        outcome = self._require_outcome(outcome)
        if source == destination:
            raise ModelError("transfer needs two distinct wallets")
        sender = self.positions[source]
        if sender.external[outcome] < quantity:
            raise KernelRefusal("insufficient_balance")
        receiver = self.positions[destination]
        positions = _replace_at(
            self.positions,
            source,
            sender.with_balance(outcome, False, sender.external[outcome] - quantity),
        )
        positions = _replace_at(
            positions,
            destination,
            receiver.with_balance(
                outcome, False, receiver.external[outcome] + quantity
            ),
        )
        return replace(self, positions=positions)

    def resolve(self, payout_index: int) -> "WeightedBook":
        self._require_active()
        self.payouts.vector(payout_index)
        book = replace(self, resolved_payout=payout_index)
        book.check_invariants()
        return book

    def redeem_internal(
        self, wallet: int, outcome: int, quantity: int
    ) -> tuple["WeightedBook", int]:
        return self._redeem(wallet, outcome, quantity, internal=True)

    def redeem_external(
        self, wallet: int, outcome: int, quantity: int
    ) -> tuple["WeightedBook", int]:
        return self._redeem(wallet, outcome, quantity, internal=False)

    def _redeem(
        self, wallet: int, outcome: int, quantity: int, internal: bool
    ) -> tuple["WeightedBook", int]:
        self._require_resolved()
        self._require_quantity(quantity)
        wallet = self._require_wallet(wallet)
        outcome = self._require_outcome(outcome)
        position = self.positions[wallet]
        available = position.balance(outcome, internal)
        if available < quantity or self.total_supply[outcome] < quantity:
            raise KernelRefusal("insufficient_balance")
        vector = self.resolved_vector()
        numerator = quantity * vector.weight(outcome)
        denominator = self.denominator
        credit_delta = numerator % denominator
        if self.policy is PayoutPolicy.CREDIT:
            payout = numerator // denominator
        else:
            if credit_delta != 0:
                raise KernelRefusal("remainder_required")
            payout = numerator // denominator
            credit_delta = 0
        if self.collateral < payout:
            raise KernelRefusal("insufficient_collateral")
        updated = position.with_balance(outcome, internal, available - quantity)
        if credit_delta:
            updated = replace(updated, credit=updated.credit + credit_delta)
        supply = list(self.total_supply)
        supply[outcome] -= quantity
        book = replace(
            self,
            collateral=self.collateral - payout,
            total_supply=tuple(supply),
            positions=_replace_at(self.positions, wallet, updated),
            credit_total=self.credit_total + credit_delta,
        )
        book.check_invariants()
        return book, payout

    def claim_credit(self, wallet: int) -> tuple["WeightedBook", int]:
        """Candidate (c): pay ``floor(credit_num / D)``, keep the sub-atom residue."""

        wallet = self._require_wallet(wallet)
        if self.policy is not PayoutPolicy.CREDIT:
            raise KernelRefusal("no_credit", "policy arm has no credit balance")
        position = self.positions[wallet]
        denominator = self.denominator
        payout = position.credit // denominator
        if payout == 0:
            raise KernelRefusal("no_credit", "less than one whole atom accrued")
        if self.collateral < payout:
            raise KernelRefusal("insufficient_collateral")
        updated = replace(position, credit=position.credit - payout * denominator)
        book = replace(
            self,
            collateral=self.collateral - payout,
            positions=_replace_at(self.positions, wallet, updated),
            credit_total=self.credit_total - payout * denominator,
        )
        book.check_invariants()
        return book, payout

    def redeem_complete_set(
        self, wallet: int, quantity: int, internal: bool = True
    ) -> tuple["WeightedBook", int]:
        """POLICY_ANALYSIS section 1.5: Resolved-phase joint complete-set exit.

        Pays exactly ``quantity`` because ``sum_i q * w_i = q * D``.  Available in
        every policy arm and never remainders.

        Under candidate (b) it is gated at ``L_split``: section 1.5 calls this
        transition the Resolved-phase twin of ``merge``, and section 1.3 gates
        ``merge`` at ``L_split`` because it burns every outcome symmetrically.
        Ungated, it re-creates sub-lot internal balances and breaks (b)'s
        internal-closure claim (EXP-LOT-B2 finding).
        """

        self._require_resolved()
        self._require_quantity(quantity)
        wallet = self._require_wallet(wallet)
        self._require_lot(quantity, self.payouts.split_lot())
        position = self.positions[wallet]
        for outcome in range(self.outcomes):
            if (
                position.balance(outcome, internal) < quantity
                or self.total_supply[outcome] < quantity
            ):
                raise KernelRefusal("insufficient_balance")
        if self.collateral < quantity:
            raise KernelRefusal("insufficient_collateral")
        updated = position
        for outcome in range(self.outcomes):
            updated = updated.with_balance(
                outcome, internal, updated.balance(outcome, internal) - quantity
            )
        book = replace(
            self,
            collateral=self.collateral - quantity,
            total_supply=tuple(value - quantity for value in self.total_supply),
            positions=_replace_at(self.positions, wallet, updated),
        )
        book.check_invariants()
        return book, quantity

    # -- fixture replay -----------------------------------------------------

    def apply(self, step: Mapping[str, object]) -> tuple["WeightedBook", Optional[int]]:
        """Replay one fixture step; returns ``(book, payout_or_None)``."""

        operation = str(step["op"])
        wallet = int(step.get("wallet", 0))
        if operation == "split":
            return self.split(wallet, int(step["quantity"])), None
        if operation == "merge":
            return self.merge(wallet, int(step["quantity"])), None
        if operation == "materialize":
            return (
                self.materialize(wallet, int(step["outcome"]), int(step["quantity"])),
                None,
            )
        if operation == "dematerialize":
            return (
                self.dematerialize(wallet, int(step["outcome"]), int(step["quantity"])),
                None,
            )
        if operation == "transfer_external":
            return (
                self.transfer_external(
                    wallet,
                    int(step["destination"]),
                    int(step["outcome"]),
                    int(step["quantity"]),
                ),
                None,
            )
        if operation == "resolve":
            return self.resolve(int(step["payout_index"])), None
        if operation == "redeem_internal":
            return self.redeem_internal(
                wallet, int(step["outcome"]), int(step["quantity"])
            )
        if operation == "redeem_external":
            return self.redeem_external(
                wallet, int(step["outcome"]), int(step["quantity"])
            )
        if operation == "claim_credit":
            return self.claim_credit(wallet)
        if operation == "redeem_complete_set":
            return self.redeem_complete_set(
                wallet, int(step["quantity"]), bool(step.get("internal", True))
            )
        raise ModelError(f"unknown operation {operation!r}")

    def state_summary(self) -> dict[str, object]:
        return {
            "collateral": self.collateral,
            "credit_total": self.credit_total,
            "positions": [
                {
                    "credit": position.credit,
                    "external": list(position.external),
                    "internal": list(position.internal),
                }
                for position in self.positions
            ],
            "required_collateral": self.required_collateral(),
            "resolved_payout": self.resolved_payout,
            "total_supply": list(self.total_supply),
        }

    # -- exit-liveness analysis --------------------------------------------

    def wallet_exit_value(self, wallet: int) -> tuple[int, int]:
        """Return ``(extractable_atoms, nominal_numerator)`` for one position.

        ``extractable`` maximizes over every exit schedule the arm admits:
        complete-set redemption (section 1.5) for any joint amount, per-outcome
        redemption, and -- under candidate (c) -- credit accrual plus claims.
        Internal and external balances redeem independently, so the joint amount
        is maximized on each side separately.
        """

        self._require_resolved()
        vector = self.resolved_vector()
        denominator = self.denominator
        position = self.positions[wallet]
        nominal = position.credit
        for internal in (True, False):
            nominal += sum(
                position.balance(outcome, internal) * vector.weight(outcome)
                for outcome in range(self.outcomes)
            )
        if self.policy is PayoutPolicy.CREDIT:
            # Every numerator reaches the credit counter exactly, so any schedule
            # pays the same floor and strands strictly less than one atom.
            return nominal // denominator, nominal
        extractable = 0
        joint_step = self.payouts.split_lot() if self.policy is PayoutPolicy.LOTS else 1
        for internal in (True, False):
            balances = tuple(
                position.balance(outcome, internal) for outcome in range(self.outcomes)
            )
            best = 0
            for joint in range(0, min(balances) + 1, joint_step):
                paid = joint
                for outcome, balance in enumerate(balances):
                    numerator = (balance - joint) * vector.weight(outcome)
                    if numerator % denominator == 0:
                        paid += numerator // denominator
                best = max(best, paid)
            extractable += best
        return extractable, nominal

    def stranded_numerator(self) -> int:
        """Claim value (in ``1/D`` units) that no exit schedule can release."""

        total = 0
        for wallet in range(len(self.positions)):
            extractable, nominal = self.wallet_exit_value(wallet)
            total += nominal - extractable * self.denominator
        return total

    def is_exit_dead(self) -> bool:
        """A resolved state holding value that cannot be redeemed at all."""

        return self.stranded_numerator() >= self.denominator

    def has_sub_atom_residue(self) -> bool:
        return 0 < self.stranded_numerator() < self.denominator

    def retirement_residue(self, terminal_complete_set: bool = True) -> int:
        """Claim atoms that no admissible transition can remove (section 1.3/1.5).

        Retirement needs every liability to reach zero.  A claim atom is
        removable when some redemption burns it: losing claims always redeem,
        exactly-divisible parcels redeem, a complete set redeems jointly when the
        section 1.5 primitive exists, and candidate (c) floors-with-credit always
        redeems.  Wallets are assumed not to cooperate, so each position is
        analyzed on its own.
        """

        self._require_resolved()
        vector = self.resolved_vector()
        denominator = self.denominator
        residue = 0
        for position in self.positions:
            for internal in (True, False):
                balances = tuple(
                    position.balance(outcome, internal)
                    for outcome in range(self.outcomes)
                )
                step = (
                    self.payouts.split_lot()
                    if self.policy is PayoutPolicy.LOTS
                    else 1
                )
                joints = (
                    range(0, min(balances) + 1, step)
                    if terminal_complete_set
                    else (0,)
                )
                best = 0
                for joint in joints:
                    removed = joint * self.outcomes
                    for outcome, balance in enumerate(balances):
                        rest = balance - joint
                        numerator = rest * vector.weight(outcome)
                        if (
                            self.policy is PayoutPolicy.CREDIT
                            or numerator % denominator == 0
                        ):
                            removed += rest
                    best = max(best, removed)
                residue += sum(balances) - best
        return residue

    def is_unretireable(self, terminal_complete_set: bool = True) -> bool:
        return self.retirement_residue(terminal_complete_set) > 0


def _replace_at(
    values: tuple[WeightedPosition, ...], index: int, value: WeightedPosition
) -> tuple[WeightedPosition, ...]:
    items = list(values)
    items[index] = value
    return tuple(items)


# ---------------------------------------------------------------------------
# Fee policy: payer debit, carry domains, terminal-ceil close
# (POLICY_ANALYSIS_LOTS_FEES.md sections 2.2-2.5, 3.3 -- all MODEL/PROPOSED)
# ---------------------------------------------------------------------------


class CarryDomain(str, Enum):
    """Object whose lifetime owns a fractional fee carry (section 2.2)."""

    POSITION = "position"
    INTENT = "intent"
    EPOCH = "epoch"


class CarryClose(str, Enum):
    """What happens to a nonzero carry when its domain instance ends."""

    TERMINAL_CEIL = "terminal_ceil"
    DROPPED = "dropped_carry"


class FeeSideArm(str, Enum):
    """Section 2.3 policy fork: who pays on a matched pair."""

    PER_INTENT_BOTH_SIDES = "per_intent_both_sides"
    CHARGE_ONCE_SPLIT = "charge_once_split"


CARRY_DOMAINS = tuple(domain.value for domain in CarryDomain)
CARRY_CLOSES = tuple(close.value for close in CarryClose)
FEE_SIDE_ARMS = tuple(arm.value for arm in FeeSideArm)


def fee_denominator(kappa_den: int, price_scale: int) -> int:
    """``den = kappa_den * S^2`` (section 2.4)."""

    if kappa_den <= 0 or price_scale <= 0:
        raise ModelError("invalid fee scale")
    return kappa_den * price_scale * price_scale


def fee_numerator(quantity: int, dispersion_num: int, kappa_num: int) -> int:
    """``fee_num = kappa_num * q * G_num(a, p)`` (section 2.4)."""

    _require_nonnegative(quantity, "quantity")
    _require_nonnegative(dispersion_num, "dispersion numerator")
    if kappa_num < 0:
        raise ModelError("negative kappa numerator")
    return kappa_num * quantity * dispersion_num


def max_single_egg_fee_numerator(
    quantity: int, price_scale: int, kappa_num: int
) -> int:
    """Largest ``fee_num`` over every admissible clearing price on the grid."""

    _require_nonnegative(quantity, "quantity")
    if price_scale <= 0 or kappa_num < 0:
        raise ModelError("invalid fee scale")
    return kappa_num * quantity * (price_scale // 2) * ((price_scale + 1) // 2)


def exact_consideration(quantity: int, price: int, price_scale: int) -> int:
    """Cash consideration in collateral atoms; refuses off-grid (q, p) pairs.

    Section 2.3 assumes ``C = quantity * p_c`` is an exact integer "by
    construction of the grid".  The lab makes that construction explicit instead
    of silently flooring.
    """

    _require_nonnegative(quantity, "quantity")
    if price_scale <= 0 or not 0 <= price <= price_scale:
        raise ModelError("invalid simplex price")
    numerator = quantity * price
    if numerator % price_scale != 0:
        raise ModelError("off-grid consideration would require rounding")
    return numerator // price_scale


def on_grid(quantity: int, price: int, price_scale: int) -> bool:
    return (quantity * price) % price_scale == 0


def escrow_reservation(
    limit_consideration: int, max_fee_numerator: int, denominator: int
) -> int:
    """Section 2.3: reserve limit consideration plus worst-case fee head-room."""

    _require_nonnegative(limit_consideration, "limit consideration")
    _require_nonnegative(max_fee_numerator, "maximum fee numerator")
    if denominator <= 0:
        raise ModelError("invalid fee denominator")
    return limit_consideration + -(-max_fee_numerator // denominator)


@dataclass(frozen=True)
class CarryAccount:
    """Floor-with-carry accumulator for one carry-domain instance."""

    denominator: int
    carry: int = 0
    paid: int = 0
    accrued: int = 0
    closed: bool = False

    def charge(self, numerator: int) -> tuple["CarryAccount", int]:
        if self.closed:
            raise ModelError("carry domain instance is closed")
        _require_nonnegative(numerator, "fee numerator")
        paid, carry = divmod(self.carry + numerator, self.denominator)
        return (
            replace(
                self,
                carry=carry,
                paid=self.paid + paid,
                accrued=self.accrued + numerator,
            ),
            paid,
        )

    def close(self, policy: CarryClose) -> tuple["CarryAccount", int]:
        """Terminal-ceil charges one more atom for a nonzero carry; dropped does not."""

        if self.closed:
            raise ModelError("carry domain instance is already closed")
        extra = 1 if (policy is CarryClose.TERMINAL_CEIL and self.carry > 0) else 0
        return (
            replace(self, carry=0, paid=self.paid + extra, closed=True),
            extra,
        )

    @property
    def exact_ceiling(self) -> int:
        return -(-self.accrued // self.denominator)


@dataclass(frozen=True)
class Fill:
    """One settled fill of a matched pair at a frozen clearing price."""

    quantity: int
    price: int
    buyer_intent: str = "buy-1"
    seller_intent: str = "sell-1"
    buyer_position: str = "buyer-1"
    seller_position: str = "seller-1"
    epoch: int = 0

    def __post_init__(self) -> None:
        _require_nonnegative(self.quantity, "fill quantity")
        _require_nonnegative(self.price, "fill price")
        _require_nonnegative(self.epoch, "epoch")


@dataclass(frozen=True)
class FeeRunResult:
    """Payer-debit accounting for one schedule of fills (section 2.3)."""

    fee_pot: int
    buyer_debit_total: int
    seller_credit_total: int
    consideration_total: int
    fee_numerator_total: int
    terminal_charges: int
    hoard_delta: int
    domain_paid: tuple[tuple[str, int], ...]
    intent_fee: tuple[tuple[str, int], ...]
    intent_cash: tuple[tuple[str, int], ...]
    #: One entry per settled leg: (intent, side, atoms paid, carry after the leg).
    fill_legs: tuple[tuple[str, str, int, int], ...] = ()

    @property
    def conserves(self) -> bool:
        """``sum(buyer debits) - sum(seller credits) = fee pot delta``."""

        return self.buyer_debit_total - self.seller_credit_total == self.fee_pot

    def as_dict(self) -> dict[str, object]:
        return {
            "buyer_debit_total": self.buyer_debit_total,
            "conserves": self.conserves,
            "consideration_total": self.consideration_total,
            "domain_paid": [list(item) for item in self.domain_paid],
            "fee_numerator_total": self.fee_numerator_total,
            "fill_legs": [list(item) for item in self.fill_legs],
            "fee_pot": self.fee_pot,
            "hoard_delta": self.hoard_delta,
            "intent_cash": [list(item) for item in self.intent_cash],
            "intent_fee": [list(item) for item in self.intent_fee],
            "seller_credit_total": self.seller_credit_total,
            "terminal_charges": self.terminal_charges,
        }


def _domain_key(domain: CarryDomain, fill: Fill, side: str) -> str:
    if domain is CarryDomain.INTENT:
        return fill.buyer_intent if side == "buy" else fill.seller_intent
    position = fill.buyer_position if side == "buy" else fill.seller_position
    if domain is CarryDomain.POSITION:
        return position
    return f"{position}@{fill.epoch}"


def run_fee_schedule(
    fills: Sequence[Fill],
    price_scale: int,
    kappa_num: int,
    kappa_den: int,
    domain: CarryDomain = CarryDomain.INTENT,
    close_policy: CarryClose = CarryClose.TERMINAL_CEIL,
    side_arm: FeeSideArm = FeeSideArm.PER_INTENT_BOTH_SIDES,
) -> FeeRunResult:
    """Settle fills with explicit payer debits; every atom has a named payer.

    Buyer cash debit is ``C + f_b`` and seller cash credit is ``C - f_s``
    (section 2.3).  The fee pot is only ever credited by a leg that debited a
    payer, so ``fee_pot`` is never incremented from thin air and the Hoard is
    never touched.  Every open domain instance is closed at the end of the
    schedule under ``close_policy``.
    """

    denominator = fee_denominator(kappa_den, price_scale)
    accounts: dict[str, CarryAccount] = {}
    last_side: dict[str, str] = {}
    intent_fee: dict[str, int] = {}
    intent_cash: dict[str, int] = {}
    legs_trace: list[tuple[str, str, int, int]] = []
    pot = 0
    buyer_debits = 0
    seller_credits = 0
    consideration_total = 0
    numerator_total = 0
    terminal_charges = 0

    for fill in fills:
        consideration = exact_consideration(fill.quantity, fill.price, price_scale)
        dispersion = single_egg_dispersion_numerator(
            fill.quantity, fill.price, price_scale
        )
        gross = fee_numerator(1, dispersion, kappa_num)
        if side_arm is FeeSideArm.PER_INTENT_BOTH_SIDES:
            legs = (("buy", gross), ("sell", gross))
        else:
            # Charge once on the transferred vector and split it; the taker-side
            # (buyer) half rounds up so the split never loses a 1/den unit.
            legs = (("buy", (gross + 1) // 2), ("sell", gross // 2))
        for side, leg_numerator in legs:
            intent = fill.buyer_intent if side == "buy" else fill.seller_intent
            key = _domain_key(domain, fill, side)
            account = accounts.get(key, CarryAccount(denominator))
            account, paid = account.charge(leg_numerator)
            accounts[key] = account
            last_side[key] = side
            legs_trace.append((intent, side, paid, account.carry))
            numerator_total += leg_numerator
            pot += paid
            intent_fee[intent] = intent_fee.get(intent, 0) + paid
            if side == "buy":
                buyer_debits += consideration + paid
                intent_cash[intent] = intent_cash.get(intent, 0) + consideration + paid
            else:
                seller_credits += consideration - paid
                intent_cash[intent] = intent_cash.get(intent, 0) + consideration - paid
        consideration_total += consideration

    for key in sorted(accounts):
        account, extra = accounts[key].close(close_policy)
        accounts[key] = account
        if extra:
            terminal_charges += extra
            pot += extra
            side = last_side[key]
            # The terminal charge is attributed to the last leg that touched the
            # instance; for the intent domain that is by construction the
            # intent's own side.
            if side == "buy":
                buyer_debits += extra
            else:
                seller_credits -= extra
            if domain is CarryDomain.INTENT:
                intent_fee[key] = intent_fee.get(key, 0) + extra
                intent_cash[key] = intent_cash.get(key, 0) + (
                    extra if side == "buy" else -extra
                )

    return FeeRunResult(
        fee_pot=pot,
        buyer_debit_total=buyer_debits,
        seller_credit_total=seller_credits,
        consideration_total=consideration_total,
        fee_numerator_total=numerator_total,
        terminal_charges=terminal_charges,
        hoard_delta=0,
        domain_paid=tuple(sorted((key, accounts[key].paid) for key in accounts)),
        intent_fee=tuple(sorted(intent_fee.items())),
        intent_cash=tuple(sorted(intent_cash.items())),
        fill_legs=tuple(legs_trace),
    )


def carry_domain_totals(
    fills: Sequence[Fill],
    price_scale: int,
    kappa_num: int,
    kappa_den: int,
    side_arm: FeeSideArm = FeeSideArm.PER_INTENT_BOTH_SIDES,
) -> dict[str, dict[str, int]]:
    """Fee pot for every (carry domain x close policy) cell of section 2.2."""

    table: dict[str, dict[str, int]] = {}
    for domain in CarryDomain:
        row: dict[str, int] = {}
        for close_policy in CarryClose:
            result = run_fee_schedule(
                fills,
                price_scale,
                kappa_num,
                kappa_den,
                domain=domain,
                close_policy=close_policy,
                side_arm=side_arm,
            )
            row[close_policy.value] = result.fee_pot
        table[domain.value] = row
    return table


def sybil_wash_result(
    fills: Sequence[Fill],
    price_scale: int,
    kappa_num: int,
    kappa_den: int,
    domain: CarryDomain = CarryDomain.INTENT,
    close_policy: CarryClose = CarryClose.TERMINAL_CEIL,
    side_arm: FeeSideArm = FeeSideArm.PER_INTENT_BOTH_SIDES,
    maker_num: int = 60,
    executor_num: int = 15,
    executor_cap: Optional[int] = None,
    network_cost: int = 0,
) -> dict[str, int]:
    """Self-wash accounting for one cell of the section 2.5 matrix.

    A Sybil that controls taker, maker, and executor pays the whole pot and
    recovers at most the maker plus executor allocations.
    """

    result = run_fee_schedule(
        fills,
        price_scale,
        kappa_num,
        kappa_den,
        domain=domain,
        close_policy=close_policy,
        side_arm=side_arm,
    )
    allocation = allocate_fee(
        result.fee_pot,
        maker_num=maker_num,
        executor_num=executor_num,
        executor_cap=executor_cap,
    )
    recovered = allocation.maker + allocation.executor
    return {
        "fee_pot": result.fee_pot,
        "recovered": recovered,
        "treasury": allocation.treasury,
        "net_wash": recovered - result.fee_pot - network_cost,
        "terminal_charges": result.terminal_charges,
    }


def enumerate_weighted_traces(
    payouts: IntegerPayoutSet,
    policy: PayoutPolicy = PayoutPolicy.KERNEL_BASELINE,
    depth: int = 5,
    quantities: Sequence[int] = (1,),
    collateral_cap: int = 3,
    wallets: int = 1,
    allow_external_transfer: bool = False,
    witness_limit: int = 4,
) -> dict[str, object]:
    """Bounded exhaustive walk of the kernel-mirror transition graph.

    This is the ``enumerate_solvency_traces`` companion asked for by
    POLICY_ANALYSIS section 3.2: it walks one policy arm and reports solvency,
    refusal classes, sub-lot residency, and exit-liveness -- not solvency alone.
    Every reachable state is checked for solvency by the transitions themselves;
    a violation raises rather than being counted.
    """

    initial = WeightedBook.open(payouts, policy, collateral=0, wallets=wallets)
    seen = {initial}
    frontier = [initial]
    transitions = 0
    refusals: dict[str, int] = {}
    exit_dead: list[dict[str, object]] = []
    exit_dead_states = 0
    sub_lot_states = 0
    resolved_states = 0
    max_stranded = 0
    outcomes = payouts.outcomes

    def attempt(call) -> None:
        nonlocal transitions
        try:
            outcome = call()
        except KernelRefusal as refusal:
            refusals[refusal.error_class] = refusals.get(refusal.error_class, 0) + 1
            return
        except ModelError:
            return
        book = outcome[0] if isinstance(outcome, tuple) else outcome
        transitions += 1
        if book not in seen:
            seen.add(book)
            next_frontier.append(book)

    for _ in range(depth):
        next_frontier: list[WeightedBook] = []
        for state in frontier:
            for wallet in range(wallets):
                for quantity in quantities:
                    if state.resolved_payout is None:
                        if state.collateral + quantity <= collateral_cap:
                            attempt(lambda s=state, w=wallet, q=quantity: s.split(w, q))
                        attempt(lambda s=state, w=wallet, q=quantity: s.merge(w, q))
                        for outcome in range(outcomes):
                            attempt(
                                lambda s=state, w=wallet, o=outcome, q=quantity: (
                                    s.materialize(w, o, q)
                                )
                            )
                            attempt(
                                lambda s=state, w=wallet, o=outcome, q=quantity: (
                                    s.dematerialize(w, o, q)
                                )
                            )
                    else:
                        for outcome in range(outcomes):
                            attempt(
                                lambda s=state, w=wallet, o=outcome, q=quantity: (
                                    s.redeem_internal(w, o, q)
                                )
                            )
                            attempt(
                                lambda s=state, w=wallet, o=outcome, q=quantity: (
                                    s.redeem_external(w, o, q)
                                )
                            )
                        attempt(
                            lambda s=state, w=wallet, q=quantity: (
                                s.redeem_complete_set(w, q, True)
                            )
                        )
                        attempt(
                            lambda s=state, w=wallet, q=quantity: (
                                s.redeem_complete_set(w, q, False)
                            )
                        )
                        attempt(lambda s=state, w=wallet: s.claim_credit(w))
                    if allow_external_transfer:
                        for other in range(wallets):
                            if other == wallet:
                                continue
                            for outcome in range(outcomes):
                                attempt(
                                    lambda s=state, a=wallet, b=other, o=outcome, q=quantity: (
                                        s.transfer_external(a, b, o, q)
                                    )
                                )
            if state.resolved_payout is None:
                for index in range(payouts.count):
                    attempt(lambda s=state, i=index: s.resolve(i))
        frontier = next_frontier
        if not frontier:
            break

    active_sub_lot_states = 0
    for state in seen:
        off_lot = any(
            balance % payouts.redemption_lot(outcome) != 0
            for position in state.positions
            for outcome, balance in enumerate(position.internal)
        )
        if state.resolved_payout is None:
            if off_lot:
                active_sub_lot_states += 1
                sub_lot_states += 1
            continue
        resolved_states += 1
        stranded = state.stranded_numerator()
        max_stranded = max(max_stranded, stranded)
        if state.is_exit_dead():
            exit_dead_states += 1
            if len(exit_dead) < witness_limit:
                exit_dead.append(state.state_summary())
        if off_lot:
            sub_lot_states += 1

    return {
        "states": len(seen),
        "transitions": transitions,
        "refusals": dict(sorted(refusals.items())),
        "resolved_states": resolved_states,
        "exit_dead_states": exit_dead_states,
        "exit_dead_witnesses": exit_dead,
        "sub_lot_internal_states": sub_lot_states,
        "active_sub_lot_internal_states": active_sub_lot_states,
        "resolved_sub_lot_internal_states": sub_lot_states - active_sub_lot_states,
        "max_stranded_numerator": max_stranded,
        "denominator": payouts.denominator,
    }
