# SPDX-License-Identifier: AGPL-3.0-or-later
"""Exact offline admission and fee-policy model for Dragon's Clutch.

This is an independent falsifier, not consensus code.  Every quantity is an
integer atom in one explicitly named asset.  In particular, there is no token
price, swap, future-volume, or treasury-conversion input to market admission.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
from enum import Enum
from typing import Iterable, Optional, Sequence


class ModelError(ValueError):
    """A fail-closed refusal in the offline model."""


def _nonnegative(value: int, name: str) -> None:
    if value < 0:
        raise ModelError(f"{name} cannot be negative")


def _positive(value: int, name: str) -> None:
    if value <= 0:
        raise ModelError(f"{name} must be positive")


def ceil_div(numerator: int, denominator: int) -> int:
    _nonnegative(numerator, "numerator")
    _positive(denominator, "denominator")
    return (numerator + denominator - 1) // denominator


def integer_shares(total: int, participants: int) -> tuple[int, ...]:
    """Canonical atom allocation; entries differ by at most one atom."""

    _nonnegative(total, "share total")
    _positive(participants, "participant count")
    quotient, remainder = divmod(total, participants)
    return tuple(
        quotient + (1 if index < remainder else 0)
        for index in range(participants)
    )


@dataclass(frozen=True)
class CollateralLedger:
    """Realm-collateral compartments with no implicit cross-pool authority.

    ``claim_collateral`` is claimant principal.  The other fields may use the
    same mint, but no method can debit claim collateral for an order or fee.
    ``reserved_fee`` is fee head-room owned by live signed intents, while
    ``fee_pot`` contains fees already debited from named payers.
    """

    claim_collateral: int = 0
    free_user_cash: int = 0
    reserved_consideration: int = 0
    reserved_fee: int = 0
    fee_pot: int = 0
    maker_claimable: int = 0
    executor_claimable: int = 0
    treasury_revenue: int = 0

    def __post_init__(self) -> None:
        for name, value in self.__dict__.items():
            _nonnegative(value, name)

    @property
    def total_atoms(self) -> int:
        return sum(self.__dict__.values())

    def lock_claim_collateral(self, amount: int) -> "CollateralLedger":
        """Move backed user cash into protected claimant principal."""

        _nonnegative(amount, "claim collateral")
        if amount > self.free_user_cash:
            raise ModelError("insufficient free cash for claim collateral")
        return replace(
            self,
            free_user_cash=self.free_user_cash - amount,
            claim_collateral=self.claim_collateral + amount,
        )

    def reserve_order(
        self, consideration: int, worst_case_fee: int
    ) -> "CollateralLedger":
        """Reserve exact consideration and worst-case fee head-room."""

        _nonnegative(consideration, "consideration")
        _nonnegative(worst_case_fee, "worst-case fee")
        total = consideration + worst_case_fee
        if total > self.free_user_cash:
            raise ModelError("insufficient free cash for order reservation")
        return replace(
            self,
            free_user_cash=self.free_user_cash - total,
            reserved_consideration=self.reserved_consideration + consideration,
            reserved_fee=self.reserved_fee + worst_case_fee,
        )

    def settle_reserved_buy(
        self, consideration: int, fee: int
    ) -> "CollateralLedger":
        """Settle a buyer leg and credit the seller aggregate plus fee pot.

        The seller's receipt is modeled as free user cash.  Unused reservation
        remains reserved until an explicit release, so settlement cannot spend
        a different intent's cash.
        """

        _nonnegative(consideration, "consideration")
        _nonnegative(fee, "fee")
        if consideration > self.reserved_consideration:
            raise ModelError("consideration exceeds reservation")
        if fee > self.reserved_fee:
            raise ModelError("fee exceeds reserved head-room")
        return replace(
            self,
            reserved_consideration=self.reserved_consideration - consideration,
            reserved_fee=self.reserved_fee - fee,
            free_user_cash=self.free_user_cash + consideration,
            fee_pot=self.fee_pot + fee,
        )

    def release_order(self, consideration: int, fee_headroom: int) -> "CollateralLedger":
        _nonnegative(consideration, "consideration release")
        _nonnegative(fee_headroom, "fee release")
        if consideration > self.reserved_consideration:
            raise ModelError("consideration release exceeds reservation")
        if fee_headroom > self.reserved_fee:
            raise ModelError("fee release exceeds reservation")
        return replace(
            self,
            reserved_consideration=self.reserved_consideration - consideration,
            reserved_fee=self.reserved_fee - fee_headroom,
            free_user_cash=self.free_user_cash + consideration + fee_headroom,
        )

    def allocate_fees(
        self,
        maker_numerator: int,
        executor_numerator: int,
        denominator: int,
        executor_cap: Optional[int],
    ) -> "CollateralLedger":
        allocation = allocate_fee(
            self.fee_pot,
            maker_numerator,
            executor_numerator,
            denominator,
            executor_cap,
        )
        return replace(
            self,
            fee_pot=0,
            maker_claimable=self.maker_claimable + allocation.maker,
            executor_claimable=self.executor_claimable + allocation.executor,
            treasury_revenue=self.treasury_revenue + allocation.treasury,
        )


@dataclass(frozen=True, order=True)
class MandatoryJob:
    """Worst-case budgets for one already-promised transition."""

    job_id: str
    max_work_lamports: int
    max_storage_lamports: int = 0
    max_service_atoms: int = 0

    def __post_init__(self) -> None:
        if not self.job_id:
            raise ModelError("job id cannot be empty")
        _nonnegative(self.max_work_lamports, "work maximum")
        _nonnegative(self.max_storage_lamports, "storage maximum")
        _nonnegative(self.max_service_atoms, "service maximum")


@dataclass(frozen=True)
class JobPayment:
    work_lamports: int
    storage_lamports: int
    service_atoms: int


@dataclass(frozen=True)
class EndowmentBook:
    """Prepaid SOL work/storage plus one optional generic service asset.

    SOL work is the only source for mandatory work payouts.  Storage principal
    is separately booked and remains refundable only through valid closure.
    The reward asset is a supplemental promise (DREGG or any other selected
    token); its atom count never substitutes for lamports.
    """

    work_lamports: int
    storage_lamports: int
    service_asset: Optional[str]
    service_atoms: int
    jobs: tuple[MandatoryJob, ...]
    storage_locked: int = 0
    completed: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        _nonnegative(self.work_lamports, "work endowment")
        _nonnegative(self.storage_lamports, "storage endowment")
        _nonnegative(self.service_atoms, "service endowment")
        _nonnegative(self.storage_locked, "locked storage")
        ids = tuple(job.job_id for job in self.jobs)
        if len(ids) != len(set(ids)) or set(ids).intersection(self.completed):
            raise ModelError("duplicate or completed mandatory job")
        if self.service_asset is None and self.service_atoms != 0:
            raise ModelError("service atoms require a named asset")
        if self.free_work_lamports < 0:
            raise ModelError("work endowment is underfunded")
        if self.free_storage_lamports < 0:
            raise ModelError("storage endowment is underfunded")
        if self.free_service_atoms < 0:
            raise ModelError("service endowment is underfunded")

    @property
    def booked_work_lamports(self) -> int:
        return sum(job.max_work_lamports for job in self.jobs)

    @property
    def booked_storage_lamports(self) -> int:
        return sum(job.max_storage_lamports for job in self.jobs)

    @property
    def booked_service_atoms(self) -> int:
        return sum(job.max_service_atoms for job in self.jobs)

    @property
    def free_work_lamports(self) -> int:
        return self.work_lamports - self.booked_work_lamports

    @property
    def free_storage_lamports(self) -> int:
        return self.storage_lamports - self.booked_storage_lamports

    @property
    def free_service_atoms(self) -> int:
        return self.service_atoms - self.booked_service_atoms

    @classmethod
    def admit(
        cls,
        jobs: Iterable[MandatoryJob],
        work_lamports: int,
        storage_lamports: int,
        service_asset: Optional[str],
        service_atoms: int,
    ) -> "EndowmentBook":
        """Admit only from atoms present now; there is no forecast argument."""

        canonical = tuple(sorted(jobs))
        return cls(
            work_lamports=work_lamports,
            storage_lamports=storage_lamports,
            service_asset=service_asset,
            service_atoms=service_atoms,
            jobs=canonical,
        )

    def add_job(
        self,
        job: MandatoryJob,
        work_deposit: int = 0,
        storage_deposit: int = 0,
        service_deposit: int = 0,
        service_asset: Optional[str] = None,
    ) -> "EndowmentBook":
        """Order/feed growth books its own worst-case remaining work."""

        if job.job_id in self.completed or any(j.job_id == job.job_id for j in self.jobs):
            raise ModelError("job is already known")
        _nonnegative(work_deposit, "work deposit")
        _nonnegative(storage_deposit, "storage deposit")
        _nonnegative(service_deposit, "service deposit")
        if service_deposit:
            if self.service_asset is None or service_asset != self.service_asset:
                raise ModelError("wrong service reward asset")
        return replace(
            self,
            work_lamports=self.work_lamports + work_deposit,
            storage_lamports=self.storage_lamports + storage_deposit,
            service_atoms=self.service_atoms + service_deposit,
            jobs=tuple(sorted((*self.jobs, job))),
        )

    def complete(self, job_id: str, payment: JobPayment) -> "EndowmentBook":
        _nonnegative(payment.work_lamports, "work payment")
        _nonnegative(payment.storage_lamports, "storage payment")
        _nonnegative(payment.service_atoms, "service payment")
        matches = tuple(job for job in self.jobs if job.job_id == job_id)
        if len(matches) != 1:
            raise ModelError("job is not unfinished")
        job = matches[0]
        if payment.work_lamports > job.max_work_lamports:
            raise ModelError("work payment exceeds booked maximum")
        if payment.storage_lamports > job.max_storage_lamports:
            raise ModelError("storage payment exceeds booked maximum")
        if payment.service_atoms > job.max_service_atoms:
            raise ModelError("service payment exceeds booked maximum")
        return replace(
            self,
            work_lamports=self.work_lamports - payment.work_lamports,
            storage_lamports=self.storage_lamports - payment.storage_lamports,
            storage_locked=self.storage_locked + payment.storage_lamports,
            service_atoms=self.service_atoms - payment.service_atoms,
            jobs=tuple(item for item in self.jobs if item.job_id != job_id),
            completed=tuple(sorted((*self.completed, job_id))),
        )

    def close_storage(self, returned_lamports: int) -> "EndowmentBook":
        """Return validly closed storage principal to the storage pool only."""

        _nonnegative(returned_lamports, "returned storage")
        if returned_lamports > self.storage_locked:
            raise ModelError("storage refund exceeds locked principal")
        return replace(
            self,
            storage_locked=self.storage_locked - returned_lamports,
            storage_lamports=self.storage_lamports + returned_lamports,
        )


@dataclass(frozen=True)
class FeedJoin:
    subscriber_id: str
    deposit: int
    reimbursements: tuple[tuple[str, int], ...]
    capital_shares: tuple[tuple[str, int], ...]


@dataclass(frozen=True)
class FeedSettlement:
    success: bool
    keeper_paid: int
    subscriber_costs: tuple[tuple[str, int], ...]
    subscriber_refunds: tuple[tuple[str, int], ...]
    neutral_roll: int


@dataclass(frozen=True)
class SharedFeedReserve:
    """Equal-net-capital reserve for one frozen source/grid epoch.

    The reserve is fully capitalized by the first subscriber.  Later joiners
    pay exactly the reduction in incumbents' capital shares; their deposit does
    not pretend to create additional reserve principal.
    """

    reserve_cap: int
    reserve_balance: int
    subscribers: tuple[str, ...]
    capital_shares: tuple[int, ...]

    def __post_init__(self) -> None:
        _nonnegative(self.reserve_cap, "feed reserve cap")
        _nonnegative(self.reserve_balance, "feed reserve balance")
        if len(self.subscribers) != len(set(self.subscribers)):
            raise ModelError("duplicate feed subscriber")
        if len(self.subscribers) != len(self.capital_shares):
            raise ModelError("subscriber/share width mismatch")
        if self.subscribers:
            if self.capital_shares != integer_shares(
                self.reserve_cap, len(self.subscribers)
            ):
                raise ModelError("noncanonical feed shares")
            if self.reserve_balance != self.reserve_cap:
                raise ModelError("active feed reserve must remain fully capitalized")
        elif self.capital_shares or self.reserve_balance:
            raise ModelError("empty feed reserve has no capital")

    @classmethod
    def empty(cls, reserve_cap: int) -> "SharedFeedReserve":
        return cls(reserve_cap, 0, (), ())

    def required_join_deposit(self) -> int:
        if not self.subscribers:
            return self.reserve_cap
        return integer_shares(self.reserve_cap, len(self.subscribers) + 1)[-1]

    def join(self, subscriber_id: str, deposit: int) -> tuple["SharedFeedReserve", FeedJoin]:
        if not subscriber_id or subscriber_id in self.subscribers:
            raise ModelError("invalid or duplicate feed subscriber")
        _nonnegative(deposit, "feed join deposit")
        required = self.required_join_deposit()
        if deposit != required:
            raise ModelError("feed join deposit must equal the canonical share")
        new_subscribers = (*self.subscribers, subscriber_id)
        new_shares = integer_shares(self.reserve_cap, len(new_subscribers))
        if not self.subscribers:
            reimbursements: tuple[int, ...] = ()
            new_balance = deposit
        else:
            reimbursements = tuple(
                old - new
                for old, new in zip(self.capital_shares, new_shares)
            )
            if any(value < 0 for value in reimbursements):
                raise AssertionError("join increased an incumbent share")
            if sum(reimbursements) != deposit:
                raise AssertionError("join deposit does not fund reimbursements")
            new_balance = self.reserve_balance
        reserve = SharedFeedReserve(
            self.reserve_cap, new_balance, new_subscribers, new_shares
        )
        return reserve, FeedJoin(
            subscriber_id=subscriber_id,
            deposit=deposit,
            reimbursements=tuple(zip(self.subscribers, reimbursements)),
            capital_shares=tuple(zip(new_subscribers, new_shares)),
        )

    def settle(self, keeper_paid: int, success: bool) -> FeedSettlement:
        if not self.subscribers:
            raise ModelError("cannot settle an unsubscribed feed")
        _nonnegative(keeper_paid, "feed keeper payment")
        if keeper_paid > self.reserve_balance:
            raise ModelError("keeper payment exceeds feed reserve")
        if success:
            costs = integer_shares(keeper_paid, len(self.subscribers))
            refunds = tuple(
                share - cost
                for share, cost in zip(self.capital_shares, costs)
            )
            if any(value < 0 for value in refunds):
                raise AssertionError("actual cost exceeds subscriber capital")
            return FeedSettlement(
                True,
                keeper_paid,
                tuple(zip(self.subscribers, costs)),
                tuple(zip(self.subscribers, refunds)),
                0,
            )
        return FeedSettlement(
            False,
            keeper_paid,
            tuple(zip(self.subscribers, self.capital_shares)),
            tuple((subscriber, 0) for subscriber in self.subscribers),
            self.reserve_balance - keeper_paid,
        )


@dataclass(frozen=True)
class AdmissionQuote:
    work_lamports: int
    storage_lamports: int
    service_atoms: int
    feed_join_lamports: int


def quote_admission(
    jobs: Sequence[MandatoryJob], feed: SharedFeedReserve
) -> AdmissionQuote:
    """Worst-case obligations present at admission; future volume counts zero."""

    return AdmissionQuote(
        work_lamports=sum(job.max_work_lamports for job in jobs),
        storage_lamports=sum(job.max_storage_lamports for job in jobs),
        service_atoms=sum(job.max_service_atoms for job in jobs),
        feed_join_lamports=feed.required_join_deposit(),
    )


@dataclass(frozen=True)
class AdmissionFunding:
    work_lamports: int
    storage_lamports: int
    service_asset: Optional[str]
    service_atoms: int
    feed_join_lamports: int

    def __post_init__(self) -> None:
        _nonnegative(self.work_lamports, "admission work funding")
        _nonnegative(self.storage_lamports, "admission storage funding")
        _nonnegative(self.service_atoms, "admission service funding")
        _nonnegative(self.feed_join_lamports, "admission feed funding")


@dataclass(frozen=True)
class AdmittedMarket:
    endowment: EndowmentBook
    feed: SharedFeedReserve
    feed_join: FeedJoin


def admit_market(
    market_id: str,
    jobs: Sequence[MandatoryJob],
    feed: SharedFeedReserve,
    funding: AdmissionFunding,
) -> AdmittedMarket:
    """Fail closed on every protected asset independently.

    Excess local funding remains explicit free endowment.  Feed deposits are
    exact because an excess has no canonical owner in the sharing relation.
    """

    if not market_id:
        raise ModelError("market id cannot be empty")
    quote = quote_admission(jobs, feed)
    if funding.work_lamports < quote.work_lamports:
        raise ModelError("underfunded mandatory SOL work")
    if funding.storage_lamports < quote.storage_lamports:
        raise ModelError("underfunded SOL storage")
    if funding.service_atoms < quote.service_atoms:
        raise ModelError("underfunded service reward")
    if quote.service_atoms and not funding.service_asset:
        raise ModelError("promised service reward needs a named asset")
    if funding.feed_join_lamports != quote.feed_join_lamports:
        raise ModelError("noncanonical shared-feed contribution")
    endowment = EndowmentBook.admit(
        jobs,
        funding.work_lamports,
        funding.storage_lamports,
        funding.service_asset,
        funding.service_atoms,
    )
    new_feed, join = feed.join(market_id, funding.feed_join_lamports)
    return AdmittedMarket(endowment, new_feed, join)


class FeeBasis(str, Enum):
    FLAT_CASH = "flat_cash_notional"
    SIMPLEX_DISPERSION = "simplex_dispersion"
    PER_EGG_LEG = "per_egg_leg"
    QUOTIENT_RANGE = "quotient_range_norm"


@dataclass(frozen=True)
class FeePolicy:
    basis: FeeBasis
    rate_numerator: int
    rate_denominator: int

    def __post_init__(self) -> None:
        _nonnegative(self.rate_numerator, "fee rate numerator")
        _positive(self.rate_denominator, "fee rate denominator")


@dataclass(frozen=True)
class FeeQuote:
    basis: FeeBasis
    base_numerator: int
    base_denominator: int
    exact_numerator: int
    exact_denominator: int
    floor_atoms: int
    terminal_ceil_atoms: int
    carry: int


def validate_payoff_price_vectors(
    payoffs: Sequence[int], prices: Sequence[int], price_scale: int
) -> None:
    if len(payoffs) != len(prices) or len(payoffs) < 2:
        raise ModelError("payoff and price vectors need equal width at least two")
    if any(value < 0 for value in payoffs):
        raise ModelError("negative payoff")
    if any(value < 0 for value in prices):
        raise ModelError("negative price")
    _positive(price_scale, "price scale")
    if sum(prices) != price_scale:
        raise ModelError("prices must lie on the exact simplex")


def flat_cash_base(
    payoffs: Sequence[int], prices: Sequence[int], price_scale: int
) -> tuple[int, int]:
    """Expected nonnegative payoff, the atomic flat cash-notional control."""

    validate_payoff_price_vectors(payoffs, prices, price_scale)
    return sum(payoff * price for payoff, price in zip(payoffs, prices)), price_scale


def dispersion_base(
    payoffs: Sequence[int], prices: Sequence[int], price_scale: int
) -> tuple[int, int]:
    """Pairwise simplex dispersion ``sum p_i p_j |a_i-a_j| / S^2``."""

    validate_payoff_price_vectors(payoffs, prices, price_scale)
    numerator = 0
    for left in range(len(payoffs)):
        for right in range(left + 1, len(payoffs)):
            numerator += (
                prices[left]
                * prices[right]
                * abs(payoffs[left] - payoffs[right])
            )
    return numerator, price_scale * price_scale


def per_egg_leg_base(
    payoffs: Sequence[int], prices: Sequence[int], price_scale: int
) -> tuple[int, int]:
    """Per-Egg control: every leg charged ``a_i * p_i * (S - p_i) / S^2``.

    This is the leg-by-leg benchmark the dispersion base was built to beat
    (FEE_GEOMETRY section 6 arm 3).  It ignores netting, so a risk-free
    complete set is charged as if each leg were a separate single-Egg trade.
    """

    validate_payoff_price_vectors(payoffs, prices, price_scale)
    numerator = sum(
        payoff * price * (price_scale - price)
        for payoff, price in zip(payoffs, prices)
    )
    return numerator, price_scale * price_scale


def quotient_range_base(
    payoffs: Sequence[int], prices: Sequence[int], price_scale: int
) -> tuple[int, int]:
    """Price-free quotient-norm control ``R(a) = max(a) - min(a)``.

    RISK_SUMMED_POSITIONS.md section 3.4 demands this arm: the model-free
    range is the unique price-free quotient-seminorm member and has no
    zero-price kernel hole.  Prices are validated but deliberately unused.
    """

    validate_payoff_price_vectors(payoffs, prices, price_scale)
    return max(payoffs) - min(payoffs), 1


def fee_quote(
    payoffs: Sequence[int],
    prices: Sequence[int],
    price_scale: int,
    policy: FeePolicy,
    prior_carry: int = 0,
) -> FeeQuote:
    if policy.basis is FeeBasis.FLAT_CASH:
        base_numerator, base_denominator = flat_cash_base(
            payoffs, prices, price_scale
        )
    elif policy.basis is FeeBasis.SIMPLEX_DISPERSION:
        base_numerator, base_denominator = dispersion_base(
            payoffs, prices, price_scale
        )
    elif policy.basis is FeeBasis.PER_EGG_LEG:
        base_numerator, base_denominator = per_egg_leg_base(
            payoffs, prices, price_scale
        )
    elif policy.basis is FeeBasis.QUOTIENT_RANGE:
        base_numerator, base_denominator = quotient_range_base(
            payoffs, prices, price_scale
        )
    else:  # pragma: no cover - enum makes this unreachable in ordinary Python
        raise ModelError("unknown fee basis")
    denominator = policy.rate_denominator * base_denominator
    _nonnegative(prior_carry, "fee carry")
    if prior_carry >= denominator:
        raise ModelError("noncanonical fee carry")
    numerator = policy.rate_numerator * base_numerator + prior_carry
    paid, carry = divmod(numerator, denominator)
    return FeeQuote(
        basis=policy.basis,
        base_numerator=base_numerator,
        base_denominator=base_denominator,
        exact_numerator=numerator,
        exact_denominator=denominator,
        floor_atoms=paid,
        terminal_ceil_atoms=ceil_div(numerator, denominator),
        carry=carry,
    )


@dataclass(frozen=True)
class FeeSequence:
    paid_before_close: int
    terminal_charge: int
    total_paid: int
    final_carry: int
    exact_numerator: int
    exact_denominator: int


def fee_sequence(
    payoff_fragments: Sequence[Sequence[int]],
    prices: Sequence[int],
    price_scale: int,
    policy: FeePolicy,
) -> FeeSequence:
    """One intent's persistent carry followed by one terminal ceiling."""

    if not payoff_fragments:
        raise ModelError("fee sequence needs a fragment")
    carry = 0
    paid = 0
    exact_numerator = 0
    exact_denominator: Optional[int] = None
    for fragment in payoff_fragments:
        quote = fee_quote(fragment, prices, price_scale, policy, carry)
        if exact_denominator is None:
            exact_denominator = quote.exact_denominator
        elif quote.exact_denominator != exact_denominator:
            raise AssertionError("one fee policy produced inconsistent denominators")
        # Remove prior carry before accumulating the new exact contribution.
        exact_numerator += quote.exact_numerator - carry
        paid += quote.floor_atoms
        carry = quote.carry
    assert exact_denominator is not None
    terminal = 1 if carry else 0
    return FeeSequence(
        paid_before_close=paid,
        terminal_charge=terminal,
        total_paid=paid + terminal,
        final_carry=carry,
        exact_numerator=exact_numerator,
        exact_denominator=exact_denominator,
    )


def maximum_binary_single_egg_fee(
    quantity: int, price_scale: int, policy: FeePolicy
) -> int:
    """Exact exhaustive price-grid head-room for one binary Egg intent."""

    _nonnegative(quantity, "quantity")
    _positive(price_scale, "price scale")
    return max(
        fee_quote(
            (quantity, 0),
            (price, price_scale - price),
            price_scale,
            policy,
        ).terminal_ceil_atoms
        for price in range(price_scale + 1)
    )


def split_identical_cell(
    payoffs: Sequence[int],
    prices: Sequence[int],
    index: int,
    left_price: int,
) -> tuple[tuple[int, ...], tuple[int, ...]]:
    """Refine one state into two identical-payoff states with exact price sum."""

    validate_payoff_price_vectors(payoffs, prices, sum(prices))
    if not 0 <= index < len(payoffs):
        raise ModelError("refinement index out of range")
    old_price = prices[index]
    if not 0 <= left_price <= old_price:
        raise ModelError("invalid refined price")
    new_payoffs = (
        tuple(payoffs[:index])
        + (payoffs[index], payoffs[index])
        + tuple(payoffs[index + 1 :])
    )
    new_prices = (
        tuple(prices[:index])
        + (left_price, old_price - left_price)
        + tuple(prices[index + 1 :])
    )
    return new_payoffs, new_prices


@dataclass(frozen=True)
class FeeAllocation:
    maker: int
    executor: int
    treasury: int

    @property
    def total(self) -> int:
        return self.maker + self.executor + self.treasury


@dataclass(frozen=True)
class MaintainerCashflow:
    """Business report without inventing a cross-asset conversion.

    Realm-collateral revenue is reported in its own atoms.  Only direct SOL
    service revenue is compared with measured SOL operating cost.
    """

    treasury_collateral_atoms: int
    direct_sol_revenue_lamports: int
    measured_sol_cost_lamports: int

    def __post_init__(self) -> None:
        _nonnegative(self.treasury_collateral_atoms, "treasury collateral")
        _nonnegative(self.direct_sol_revenue_lamports, "direct SOL revenue")
        _nonnegative(self.measured_sol_cost_lamports, "measured SOL cost")

    @property
    def direct_sol_surplus_lamports(self) -> int:
        return self.direct_sol_revenue_lamports - self.measured_sol_cost_lamports

    @property
    def direct_sol_break_even(self) -> bool:
        return self.direct_sol_surplus_lamports >= 0


def allocate_fee(
    fee_atoms: int,
    maker_numerator: int,
    executor_numerator: int,
    denominator: int,
    executor_cap: Optional[int],
) -> FeeAllocation:
    _nonnegative(fee_atoms, "fee pot")
    _nonnegative(maker_numerator, "maker share")
    _nonnegative(executor_numerator, "executor share")
    _positive(denominator, "allocation denominator")
    if maker_numerator + executor_numerator > denominator:
        raise ModelError("maker and executor shares exceed the pot")
    if executor_cap is not None:
        _nonnegative(executor_cap, "executor cap")
    maker = fee_atoms * maker_numerator // denominator
    executor = fee_atoms * executor_numerator // denominator
    if executor_cap is not None:
        executor = min(executor, executor_cap)
    return FeeAllocation(maker, executor, fee_atoms - maker - executor)


def wash_result(
    quote: FeeQuote,
    maker_numerator: int,
    executor_numerator: int,
    denominator: int,
    executor_cap: Optional[int],
    network_lamports: int,
) -> dict[str, int]:
    """Worst Sybil recovery when one actor controls taker, maker, executor."""

    _nonnegative(network_lamports, "network cost")
    fee = quote.terminal_ceil_atoms
    allocation = allocate_fee(
        fee,
        maker_numerator,
        executor_numerator,
        denominator,
        executor_cap,
    )
    recovered = allocation.maker + allocation.executor
    return {
        "fee": fee,
        "recovered": recovered,
        "treasury": allocation.treasury,
        "network_lamports": network_lamports,
        # Asset units differ: this field deliberately excludes network cost.
        # Lamports are reported separately rather than converted by an oracle.
        "collateral_net": recovered - fee,
    }
