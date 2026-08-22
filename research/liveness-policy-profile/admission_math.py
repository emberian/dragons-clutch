#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Exact, fail-closed liveness quote and staged-path arithmetic.

This module deliberately knows nothing about an ELF, a SOL price, future block
inclusion, or future venue volume.  A caller supplies a measured transaction CU
observation and explicit policy caps.  If the requested headroom does not fit
under the transaction ceiling, no lamport quote exists.

The ResolutionWork helpers keep three arithmetic planes disjoint:

* the protocol minimum prefund, derived from the onchain charge/reward schedule
  and the worst legal successful-Fold-call count;
* actual runtime payout and payer refund for a named Fold-call plan; and
* external transaction/CU keeper budgets derived only from measured routes.

They therefore cannot turn a failed transaction-headroom gate into a finite
keeper promise, add rent principal to an external quote, or let a preferred
batch plan reduce the invariant protocol minimum.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Mapping, Sequence


MICRO_LAMPORTS_PER_LAMPORT = 1_000_000


class AdmissionError(ValueError):
    """An input or reachable staged path is not admissible."""


def ceil_div(value: int, denominator: int) -> int:
    """Return exact nonnegative ceiling division."""

    if value < 0 or denominator <= 0:
        raise AdmissionError("ceil_div requires value >= 0 and denominator > 0")
    return (value + denominator - 1) // denominator


def round_up(value: int, quantum: int) -> int:
    """Round a nonnegative integer upward to a positive quantum."""

    if quantum <= 0:
        raise AdmissionError("rounding quantum must be positive")
    return ceil_div(value, quantum) * quantum


@dataclass(frozen=True)
class QuotePolicy:
    """Explicit policy inputs for one transaction class.

    ``base_fee_cap_lamports`` is already the route-specific signature/base-fee
    cap.  It must not be inferred from a universal signature count.
    """

    headroom_numerator: int
    headroom_denominator: int
    rounding_quantum_cu: int
    transaction_ceiling_cu: int
    base_fee_cap_lamports: int
    micro_lamports_per_cu_cap: int
    keeper_tip_lamports: int

    def validate(self) -> None:
        if (
            self.headroom_numerator <= 0
            or self.headroom_denominator <= 0
            or self.headroom_numerator < self.headroom_denominator
            or self.rounding_quantum_cu <= 0
            or self.transaction_ceiling_cu <= 0
            or self.base_fee_cap_lamports < 0
            or self.micro_lamports_per_cu_cap < 0
            or self.keeper_tip_lamports < 0
        ):
            raise AdmissionError("invalid quote policy")


@dataclass(frozen=True)
class RouteQuote:
    """A measured route quote, or a fail-closed STOP with no lamport amount."""

    measured_cu: int
    required_headroom_cu: int
    selected_limit_cu: int | None
    external_fee_cap_lamports: int | None
    keeper_reward_lamports: int | None
    status: str

    @property
    def admitted(self) -> bool:
        return self.status == "PASS"

    def require_reward(self) -> int:
        if not self.admitted or self.keeper_reward_lamports is None:
            raise AdmissionError("stopped route has no keeper reward quote")
        return self.keeper_reward_lamports


def quote_route(measured_cu: int, policy: QuotePolicy) -> RouteQuote:
    """Quote a measured route without clamping an impossible envelope.

    The returned reward is the selected external fee cap plus the explicit
    keeper tip.  A finite cap does not guarantee future inclusion.
    """

    policy.validate()
    if measured_cu < 0:
        raise AdmissionError("measured CU must be nonnegative")
    required = ceil_div(
        measured_cu * policy.headroom_numerator,
        policy.headroom_denominator,
    )
    selected = round_up(required, policy.rounding_quantum_cu)
    if selected > policy.transaction_ceiling_cu:
        return RouteQuote(
            measured_cu=measured_cu,
            required_headroom_cu=required,
            selected_limit_cu=None,
            external_fee_cap_lamports=None,
            keeper_reward_lamports=None,
            status="STOP_HEADROOM",
        )
    priority = ceil_div(
        selected * policy.micro_lamports_per_cu_cap,
        MICRO_LAMPORTS_PER_LAMPORT,
    )
    external = policy.base_fee_cap_lamports + priority
    return RouteQuote(
        measured_cu=measured_cu,
        required_headroom_cu=required,
        selected_limit_cu=selected,
        external_fee_cap_lamports=external,
        keeper_reward_lamports=external + policy.keeper_tip_lamports,
        status="PASS",
    )


@dataclass(frozen=True)
class ExternalResolutionBudgetQuote:
    """Externally funded transaction/CU budget for singleton Fold sends.

    These are policy quotes for transaction fees plus an explicit keeper tip.
    They are neither an onchain reward schedule nor protocol-owned reserve.
    Rent principal is intentionally absent, so it cannot be added to a quote
    and mislabeled as a runtime minimum deposit.
    """

    record_count: int
    fold_transactions_budget_lamports: int | None
    success_post_begin_budget_lamports: int | None
    worst_abort_post_begin_budget_lamports: int | None
    begin_transaction_budget_lamports: int | None
    success_total_budget_lamports: int | None
    worst_abort_total_budget_lamports: int | None
    status: str


@dataclass(frozen=True)
class BatchedExternalResolutionBudgetQuote:
    """Externally funded budget for a measured Fold(1)-batch plan.

    ``transaction_plan`` lists the number of singleton Fold instructions in
    each measured transaction, largest first.  It does not describe Fold call
    widths and cannot alter the protocol's per-successful-call reserve rule.
    """

    record_count: int
    transaction_plan: tuple[int, ...] | None
    fold_transactions: int | None
    fold_transactions_budget_lamports: int | None
    success_post_begin_budget_lamports: int | None
    worst_abort_post_begin_budget_lamports: int | None
    begin_transaction_budget_lamports: int | None
    success_total_budget_lamports: int | None
    worst_abort_total_budget_lamports: int | None
    status: str


@dataclass(frozen=True)
class RuntimeCostSchedule:
    """Exact onchain ResolutionWork charge and reward schedule."""

    maximum_records: int
    maximum_fold_width: int
    begin_charge_lamports: int
    fold_base_charge_lamports: int
    fold_per_record_charge_lamports: int
    fold_base_reward_lamports: int
    fold_per_record_reward_lamports: int
    finalize_charge_lamports: int
    finalize_reward_lamports: int
    abort_charge_lamports: int
    abort_reward_lamports: int

    def validate(self) -> None:
        if self.maximum_records <= 0 or self.maximum_fold_width <= 0:
            raise AdmissionError("runtime schedule bounds must be positive")
        if self.maximum_fold_width > self.maximum_records:
            raise AdmissionError("runtime Fold width exceeds maximum records")
        if any(
            value < 0 or value > (1 << 64) - 1
            for value in (
                self.begin_charge_lamports,
                self.fold_base_charge_lamports,
                self.fold_per_record_charge_lamports,
                self.fold_base_reward_lamports,
                self.fold_per_record_reward_lamports,
                self.finalize_charge_lamports,
                self.finalize_reward_lamports,
                self.abort_charge_lamports,
                self.abort_reward_lamports,
            )
        ):
            raise AdmissionError("runtime schedule values must be nonnegative")

    def fold_charge(self, width: int) -> int:
        self.validate()
        if not 1 <= width <= self.maximum_fold_width:
            raise AdmissionError("runtime Fold width exceeds the schedule bound")
        return add_many(
            self.fold_base_charge_lamports,
            checked_mul(self.fold_per_record_charge_lamports, width),
        )

    def fold_reward(self, width: int) -> int:
        self.validate()
        if not 1 <= width <= self.maximum_fold_width:
            raise AdmissionError("runtime Fold width exceeds the schedule bound")
        return add_many(
            self.fold_base_reward_lamports,
            checked_mul(self.fold_per_record_reward_lamports, width),
        )


@dataclass(frozen=True)
class ProtocolResolutionPrefund:
    """Protocol minimum under the onchain worst-case successful-call path."""

    record_count: int
    worst_case_fold_calls: int
    singleton_fold_charge_lamports: int
    singleton_fold_reward_lamports: int
    worst_case_fold_outflow_lamports: int
    finalize_outflow_lamports: int
    abort_outflow_lamports: int
    terminal_outflow_lamports: int
    spendable_reserve_lamports: int
    rent_principal_lamports: int
    minimum_prefund_lamports: int


@dataclass(frozen=True)
class RuntimeExecutionPlan:
    """Exact runtime payout/refund for one named Fold-call plan."""

    name: str
    record_count: int
    fold_call_widths: tuple[int, ...]
    transaction_fold_call_counts: tuple[int, ...]
    fold_calls: int
    fold_transactions: int
    fold_charges_lamports: int
    fold_rewards_lamports: int
    success_charges_lamports: int
    success_payout_lamports: int
    success_unused_prepaid_lamports: int
    success_rent_principal_refund_lamports: int
    success_payer_refund_lamports: int
    abort_charges_lamports: int
    abort_payout_lamports: int
    abort_unused_prepaid_lamports: int
    abort_rent_principal_refund_lamports: int
    abort_payer_refund_lamports: int


@dataclass(frozen=True)
class RuntimeCoverageRow:
    """One mechanically compared runtime reward and external policy quote."""

    route: str
    external_keeper_budget_lamports: int
    runtime_reward_lamports: int
    margin_lamports: int
    covered: bool


@dataclass(frozen=True)
class RuntimeScheduleCoverage:
    """Derived coverage report; no caller-provided truth flag exists."""

    rows: tuple[RuntimeCoverageRow, ...]

    @property
    def matches_policy(self) -> bool:
        return bool(self.rows) and all(row.covered for row in self.rows)

    def require(self) -> None:
        for row in self.rows:
            if not row.covered:
                raise AdmissionError(f"runtime {row.route} reward is under policy")


@dataclass(frozen=True)
class DirectWorkBudgetQuote:
    """Worst accepted staged direct-selection work budget.

    Candidate submission is intentionally absent: the submitter pays that
    optional transaction directly.  This quote covers only the finite work
    promised after a two-order Epoch freezes successfully.  Rent principal is
    reported independently and cannot become a keeper reward.
    """

    max_candidates: int
    selected_success_rewards_lamports: int | None
    unselected_lapse_rewards_lamports: int | None
    selected_lapse_rewards_lamports: int | None
    empty_lapse_rewards_lamports: int | None
    spendable_reserve_lamports: int | None
    rent_principal_lamports: int
    persistent_budget_lamports: int | None
    status: str


def worst_fold_partition(
    record_count: int,
    fold_quotes: Mapping[int, RouteQuote],
) -> tuple[int, tuple[int, ...]]:
    """Return the most expensive accepted partition using widths 1..4.

    Every declared width is an accepted runtime choice, so every width must
    itself have a passing quote.  Dynamic programming then prices the worst
    reachable partition instead of assuming either singleton or maximum-width
    folding without evidence.
    """

    if record_count < 0:
        raise AdmissionError("record count must be nonnegative")
    if set(fold_quotes) != {1, 2, 3, 4}:
        raise AdmissionError("Fold evidence must contain exact widths 1 through 4")
    for width in range(1, 5):
        if not fold_quotes[width].admitted:
            raise AdmissionError(f"Fold({width}) is stopped")

    best: list[tuple[int, tuple[int, ...]] | None] = [None] * (record_count + 1)
    best[0] = (0, ())
    for end in range(1, record_count + 1):
        candidates: list[tuple[int, tuple[int, ...]]] = []
        for width in range(1, 5):
            start = end - width
            if start < 0 or best[start] is None:
                continue
            prior_cost, prior_partition = best[start]
            candidates.append(
                (
                    prior_cost + fold_quotes[width].require_reward(),
                    prior_partition + (width,),
                )
            )
        if candidates:
            # Lexicographic partition tie-break makes the result reproducible.
            best[end] = max(candidates, key=lambda item: (item[0], item[1]))
    if best[record_count] is None:
        raise AdmissionError("no Fold partition reaches the record count")
    return best[record_count]


def external_resolution_budget_quote(
    *,
    record_count: int,
    begin: RouteQuote,
    fold_quotes: Mapping[int, RouteQuote],
    finalize: RouteQuote,
    abort: RouteQuote,
) -> ExternalResolutionBudgetQuote:
    """Price external singleton-transaction work, propagating every STOP."""

    if not 1 <= record_count <= 32:
        raise AdmissionError("record count must be in 1..=32")
    if not begin.admitted:
        return ExternalResolutionBudgetQuote(
            record_count,
            None,
            None,
            None,
            None,
            None,
            None,
            "STOP_BEGIN",
        )
    try:
        fold_path, _ = worst_fold_partition(record_count, fold_quotes)
    except AdmissionError:
        return ExternalResolutionBudgetQuote(
            record_count,
            None,
            None,
            None,
            begin.require_reward(),
            None,
            None,
            "STOP_FOLD",
        )
    if not finalize.admitted:
        return ExternalResolutionBudgetQuote(
            record_count,
            fold_path,
            None,
            None,
            begin.require_reward(),
            None,
            None,
            "STOP_FINALIZE",
        )
    if not abort.admitted:
        return ExternalResolutionBudgetQuote(
            record_count,
            fold_path,
            None,
            None,
            begin.require_reward(),
            None,
            None,
            "STOP_ABORT",
        )

    finalize_reward = finalize.require_reward()
    abort_reward = abort.require_reward()
    success = add_many(fold_path, finalize_reward)

    # Any prefix may expire.  Because every Fold quote is nonnegative, the
    # most expensive abort prefix is the full worst-case Fold path.  Keeping it
    # explicit documents that Abort is a path alternative, not an added job.
    worst_abort = add_many(fold_path, abort_reward)
    begin_external = begin.require_reward()
    return ExternalResolutionBudgetQuote(
        record_count=record_count,
        fold_transactions_budget_lamports=fold_path,
        success_post_begin_budget_lamports=success,
        worst_abort_post_begin_budget_lamports=worst_abort,
        begin_transaction_budget_lamports=begin_external,
        success_total_budget_lamports=add_many(begin_external, success),
        worst_abort_total_budget_lamports=add_many(begin_external, worst_abort),
        status="PASS",
    )


def fewest_transaction_batch_plan(
    record_count: int,
    batch_quotes: Mapping[int, RouteQuote],
) -> tuple[int, ...]:
    """Return the fewest-transaction batch cover of singleton folds.

    Every key is a measured single-transaction batch of that many singleton
    Fold(1) instructions (size 1 is the ordinary one-fold transaction).  Only
    an admitted batch route may appear in the plan; a stopped size simply
    drops out of the search instead of being clamped into a price.  Equal
    transaction counts break toward the lexicographically greatest descending
    plan so the result is reproducible.
    """

    if record_count < 1:
        raise AdmissionError("record count must be positive")
    if any(size < 1 for size in batch_quotes):
        raise AdmissionError("batch sizes must be positive")
    sizes = sorted(size for size, quote in batch_quotes.items() if quote.admitted)
    if not sizes:
        raise AdmissionError("no admitted fold-batch route")
    best: list[tuple[int, tuple[int, ...]] | None] = [None] * (record_count + 1)
    best[0] = (0, ())
    for end in range(1, record_count + 1):
        candidates: list[tuple[int, tuple[int, ...]]] = []
        for size in sizes:
            start = end - size
            if start < 0 or best[start] is None:
                continue
            count, plan = best[start]
            candidates.append((count + 1, tuple(sorted((*plan, size), reverse=True))))
        if candidates:
            best[end] = min(
                candidates,
                key=lambda item: (item[0], tuple(-size for size in item[1])),
            )
    if best[record_count] is None:
        raise AdmissionError("no fold-batch plan reaches the record count")
    return best[record_count][1]


def batched_external_resolution_budget_quote(
    *,
    record_count: int,
    begin: RouteQuote,
    batch_quotes: Mapping[int, RouteQuote],
    finalize: RouteQuote,
    abort: RouteQuote,
) -> BatchedExternalResolutionBudgetQuote:
    """Price a measured Fold(1)-batch transaction plan, propagating STOPs."""

    if not 1 <= record_count <= 32:
        raise AdmissionError("record count must be in 1..=32")

    def stopped(
        status: str, begin_external: int | None
    ) -> BatchedExternalResolutionBudgetQuote:
        return BatchedExternalResolutionBudgetQuote(
            record_count=record_count,
            transaction_plan=None,
            fold_transactions=None,
            fold_transactions_budget_lamports=None,
            success_post_begin_budget_lamports=None,
            worst_abort_post_begin_budget_lamports=None,
            begin_transaction_budget_lamports=begin_external,
            success_total_budget_lamports=None,
            worst_abort_total_budget_lamports=None,
            status=status,
        )

    if not begin.admitted:
        return stopped("STOP_BEGIN", None)
    try:
        plan = fewest_transaction_batch_plan(record_count, batch_quotes)
    except AdmissionError:
        return stopped("STOP_FOLD_BATCH", begin.require_reward())
    if not finalize.admitted:
        return stopped("STOP_FINALIZE", begin.require_reward())
    if not abort.admitted:
        return stopped("STOP_ABORT", begin.require_reward())

    fold_path = add_many(*(batch_quotes[size].require_reward() for size in plan))
    success = add_many(fold_path, finalize.require_reward())
    worst_abort = add_many(fold_path, abort.require_reward())
    begin_external = begin.require_reward()
    return BatchedExternalResolutionBudgetQuote(
        record_count=record_count,
        transaction_plan=plan,
        fold_transactions=len(plan),
        fold_transactions_budget_lamports=fold_path,
        success_post_begin_budget_lamports=success,
        worst_abort_post_begin_budget_lamports=worst_abort,
        begin_transaction_budget_lamports=begin_external,
        success_total_budget_lamports=add_many(begin_external, success),
        worst_abort_total_budget_lamports=add_many(begin_external, worst_abort),
        status="PASS",
    )


def direct_work_budget_quote(
    *,
    max_candidates: int,
    begin: RouteQuote,
    verify: RouteQuote,
    finalize: RouteQuote,
    settle: RouteQuote,
    lapse: RouteQuote,
    rent_principal_lamports: int,
) -> DirectWorkBudgetQuote:
    """Price every reachable post-Freeze Direct V3 terminal alternative.

    A non-empty successful path is ``Begin + k*Verify + Finalize + Settle``.
    A pre-selection timeout may follow ``Begin + j*Verify`` without Finalize.
    A selected timeout substitutes ``Lapse`` for ``Settle`` after Finalize.  An
    empty frozen Epoch needs only ``Lapse``.  The account must reserve the
    greatest path for the bounded retained-candidate maximum; it must not add
    mutually exclusive terminal actions or divide the budget between orders.
    """

    if not 1 <= max_candidates <= 3:
        raise AdmissionError("direct candidate maximum must be in 1..=3")
    if rent_principal_lamports <= 0:
        raise AdmissionError("direct work-budget rent principal must be positive")

    routes = (
        ("BEGIN", begin),
        ("VERIFY", verify),
        ("FINALIZE", finalize),
        ("SETTLE", settle),
        ("LAPSE", lapse),
    )
    for label, route in routes:
        if not route.admitted:
            return DirectWorkBudgetQuote(
                max_candidates=max_candidates,
                selected_success_rewards_lamports=None,
                unselected_lapse_rewards_lamports=None,
                selected_lapse_rewards_lamports=None,
                empty_lapse_rewards_lamports=None,
                spendable_reserve_lamports=None,
                rent_principal_lamports=rent_principal_lamports,
                persistent_budget_lamports=None,
                status=f"STOP_{label}",
            )

    verifying_prefix = add_many(
        begin.require_reward(),
        checked_mul(verify.require_reward(), max_candidates),
    )
    selected_prefix = add_many(
        verifying_prefix,
        finalize.require_reward(),
    )
    success = add_many(selected_prefix, settle.require_reward())
    unselected_lapse = add_many(verifying_prefix, lapse.require_reward())
    selected_lapse = add_many(selected_prefix, lapse.require_reward())
    empty_lapse = lapse.require_reward()
    spendable = max(success, unselected_lapse, selected_lapse, empty_lapse)
    return DirectWorkBudgetQuote(
        max_candidates=max_candidates,
        selected_success_rewards_lamports=success,
        unselected_lapse_rewards_lamports=unselected_lapse,
        selected_lapse_rewards_lamports=selected_lapse,
        empty_lapse_rewards_lamports=empty_lapse,
        spendable_reserve_lamports=spendable,
        rent_principal_lamports=rent_principal_lamports,
        persistent_budget_lamports=add_many(rent_principal_lamports, spendable),
        status="PASS",
    )


def checked_mul(value: int, multiplier: int) -> int:
    """Model the nonnegative checked-u64 multiplication used onchain."""

    if value < 0 or multiplier < 0:
        raise AdmissionError("checked multiplication requires nonnegative inputs")
    product = value * multiplier
    if product > (1 << 64) - 1:
        raise AdmissionError("u64 multiplication overflow")
    return product


def add_many(*values: int) -> int:
    """Model checked-u64 addition for policy totals."""

    total = 0
    for value in values:
        if value < 0:
            raise AdmissionError("checked addition requires nonnegative inputs")
        total += value
        if total > (1 << 64) - 1:
            raise AdmissionError("u64 addition overflow")
    return total


def runtime_fold_reward(base: int, per_record: int, width: int) -> int:
    """Evaluate the exact linear runtime Fold schedule."""

    if base < 0 or per_record < 0 or not 1 <= width <= 4:
        raise AdmissionError("invalid runtime Fold schedule")
    return base + per_record * width


def protocol_resolution_prefund(
    *,
    record_count: int,
    rent_principal_lamports: int,
    schedule: RuntimeCostSchedule,
) -> ProtocolResolutionPrefund:
    """Mirror ``ResolutionWorkCostScheduleV1::minimum_deposit`` exactly.

    The protocol must admit every legal partition.  Its worst case is therefore
    ``record_count`` successful Fold(1) calls, regardless of how a preferred
    keeper plan batches instructions into transactions or folds more records in
    each call.
    """

    schedule.validate()
    if not 1 <= record_count <= schedule.maximum_records:
        raise AdmissionError("record count exceeds the runtime schedule bound")
    if rent_principal_lamports <= 0 or rent_principal_lamports > (1 << 64) - 1:
        raise AdmissionError("rent principal must be positive")
    singleton_charge = schedule.fold_charge(1)
    singleton_reward = schedule.fold_reward(1)
    singleton_outflow = add_many(singleton_charge, singleton_reward)
    fold_outflow = checked_mul(singleton_outflow, record_count)
    finalize_outflow = add_many(
        schedule.finalize_charge_lamports,
        schedule.finalize_reward_lamports,
    )
    abort_outflow = add_many(
        schedule.abort_charge_lamports,
        schedule.abort_reward_lamports,
    )
    terminal_outflow = max(finalize_outflow, abort_outflow)
    spendable = add_many(
        schedule.begin_charge_lamports,
        fold_outflow,
        terminal_outflow,
    )
    return ProtocolResolutionPrefund(
        record_count=record_count,
        worst_case_fold_calls=record_count,
        singleton_fold_charge_lamports=singleton_charge,
        singleton_fold_reward_lamports=singleton_reward,
        worst_case_fold_outflow_lamports=fold_outflow,
        finalize_outflow_lamports=finalize_outflow,
        abort_outflow_lamports=abort_outflow,
        terminal_outflow_lamports=terminal_outflow,
        spendable_reserve_lamports=spendable,
        rent_principal_lamports=rent_principal_lamports,
        minimum_prefund_lamports=add_many(rent_principal_lamports, spendable),
    )


def runtime_execution_plan(
    *,
    name: str,
    fold_call_widths: Sequence[int],
    transaction_fold_call_counts: Sequence[int],
    prefund: ProtocolResolutionPrefund,
    schedule: RuntimeCostSchedule,
) -> RuntimeExecutionPlan:
    """Derive actual payouts and terminal payer refunds for one plan.

    Transaction grouping affects the external budget only.  Runtime rewards
    are paid once per successful Fold call, so this function takes call widths
    and transaction call counts as two independent, cross-checked partitions.
    """

    if not name:
        raise AdmissionError("runtime execution plan must be named")
    widths = tuple(fold_call_widths)
    transactions = tuple(transaction_fold_call_counts)
    if not widths or any(
        not 1 <= width <= schedule.maximum_fold_width for width in widths
    ):
        raise AdmissionError("plan Fold widths must fit the runtime schedule")
    if sum(widths) != prefund.record_count:
        raise AdmissionError("plan Fold widths must cover the record count exactly")
    if not transactions or any(count <= 0 for count in transactions):
        raise AdmissionError("transaction Fold-call counts must be positive")
    if sum(transactions) != len(widths):
        raise AdmissionError("transaction plan must contain every Fold call exactly once")
    schedule.validate()

    fold_charges = add_many(*(schedule.fold_charge(width) for width in widths))
    fold_rewards = add_many(*(schedule.fold_reward(width) for width in widths))
    prepaid = prefund.spendable_reserve_lamports

    success_charges = add_many(
        schedule.begin_charge_lamports,
        fold_charges,
        schedule.finalize_charge_lamports,
    )
    success_payout = add_many(fold_rewards, schedule.finalize_reward_lamports)
    success_unused = prepaid - add_many(success_charges, success_payout)
    if success_unused < 0:
        raise AdmissionError("runtime success plan exceeds protocol prefund")

    abort_charges = add_many(
        schedule.begin_charge_lamports,
        fold_charges,
        schedule.abort_charge_lamports,
    )
    abort_payout = add_many(fold_rewards, schedule.abort_reward_lamports)
    abort_unused = prepaid - add_many(abort_charges, abort_payout)
    if abort_unused < 0:
        raise AdmissionError("runtime abort plan exceeds protocol prefund")

    return RuntimeExecutionPlan(
        name=name,
        record_count=prefund.record_count,
        fold_call_widths=widths,
        transaction_fold_call_counts=transactions,
        fold_calls=len(widths),
        fold_transactions=len(transactions),
        fold_charges_lamports=fold_charges,
        fold_rewards_lamports=fold_rewards,
        success_charges_lamports=success_charges,
        success_payout_lamports=success_payout,
        success_unused_prepaid_lamports=success_unused,
        success_rent_principal_refund_lamports=prefund.rent_principal_lamports,
        success_payer_refund_lamports=add_many(
            success_unused, prefund.rent_principal_lamports
        ),
        abort_charges_lamports=abort_charges,
        abort_payout_lamports=abort_payout,
        abort_unused_prepaid_lamports=abort_unused,
        abort_rent_principal_refund_lamports=prefund.rent_principal_lamports,
        abort_payer_refund_lamports=add_many(
            abort_unused, prefund.rent_principal_lamports
        ),
    )


def runtime_schedule_policy_coverage(
    *,
    fold_quotes: Mapping[int, RouteQuote],
    schedule: RuntimeCostSchedule,
    finalize_quote: RouteQuote,
    abort_quote: RouteQuote,
) -> RuntimeScheduleCoverage:
    """Compare every runtime reward to its external keeper budget."""

    expected_widths = set(range(1, schedule.maximum_fold_width + 1))
    if set(fold_quotes) != expected_widths:
        raise AdmissionError(
            "Fold evidence must contain every runtime width exactly once"
        )
    schedule.validate()
    comparisons = [
        (
            f"Fold({width})",
            fold_quotes[width].require_reward(),
            schedule.fold_reward(width),
        )
        for width in range(1, schedule.maximum_fold_width + 1)
    ]
    comparisons.extend(
        (
            (
                "Finalize",
                finalize_quote.require_reward(),
                schedule.finalize_reward_lamports,
            ),
            (
                "Abort",
                abort_quote.require_reward(),
                schedule.abort_reward_lamports,
            ),
        )
    )
    return RuntimeScheduleCoverage(
        rows=tuple(
            RuntimeCoverageRow(
                route=route,
                external_keeper_budget_lamports=required,
                runtime_reward_lamports=actual,
                margin_lamports=actual - required,
                covered=actual >= required,
            )
            for route, required, actual in comparisons
        )
    )


def runtime_schedule_batch_coverage(
    *,
    batch_quotes: Mapping[int, RouteQuote],
    schedule: RuntimeCostSchedule,
) -> RuntimeScheduleCoverage:
    """Compare measured Fold(1)-batch budgets to per-call runtime rewards."""

    schedule.validate()
    rows = []
    for size, quote in sorted(batch_quotes.items()):
        if not quote.admitted:
            continue
        required = quote.require_reward()
        actual = checked_mul(schedule.fold_reward(1), size)
        rows.append(
            RuntimeCoverageRow(
                route=f"FoldBatch({size})",
                external_keeper_budget_lamports=required,
                runtime_reward_lamports=actual,
                margin_lamports=actual - required,
                covered=actual >= required,
            )
        )
    return RuntimeScheduleCoverage(rows=tuple(rows))


def require_runtime_schedule_covers_policy(
    *,
    fold_quotes: Mapping[int, RouteQuote],
    fold_base_reward: int,
    fold_per_record_reward: int,
    finalize_quote: RouteQuote,
    finalize_reward: int,
    abort_quote: RouteQuote,
    abort_reward: int,
) -> None:
    """Refuse a runtime schedule below any selected policy quote."""

    runtime_schedule_policy_coverage(
        fold_quotes=fold_quotes,
        schedule=RuntimeCostSchedule(
            maximum_records=32,
            maximum_fold_width=4,
            begin_charge_lamports=0,
            fold_base_charge_lamports=0,
            fold_per_record_charge_lamports=0,
            fold_base_reward_lamports=fold_base_reward,
            fold_per_record_reward_lamports=fold_per_record_reward,
            finalize_charge_lamports=0,
            finalize_reward_lamports=finalize_reward,
            abort_charge_lamports=0,
            abort_reward_lamports=abort_reward,
        ),
        finalize_quote=finalize_quote,
        abort_quote=abort_quote,
    ).require()


def require_runtime_schedule_covers_batches(
    *,
    batch_quotes: Mapping[int, RouteQuote],
    fold_base_reward: int,
    fold_per_record_reward: int,
) -> None:
    """Refuse a runtime schedule below any admitted batch-transaction quote.

    A batch of ``n`` singleton folds pays the runtime Fold schedule ``n``
    times inside one transaction, so that runtime total must cover the single
    batched transaction's policy quote.  Stopped batch sizes carry no quote to
    cover and are skipped rather than clamped.
    """

    runtime_schedule_batch_coverage(
        batch_quotes=batch_quotes,
        schedule=RuntimeCostSchedule(
            maximum_records=32,
            maximum_fold_width=4,
            begin_charge_lamports=0,
            fold_base_charge_lamports=0,
            fold_per_record_charge_lamports=0,
            fold_base_reward_lamports=fold_base_reward,
            fold_per_record_reward_lamports=fold_per_record_reward,
            finalize_charge_lamports=0,
            finalize_reward_lamports=0,
            abort_charge_lamports=0,
            abort_reward_lamports=0,
        ),
    ).require()


def exact_unique_labels(rows: Sequence[int], expected: Sequence[int], label: str) -> None:
    """Reject duplicate or incomplete shape labels before flattening evidence."""

    if list(rows) != list(expected) or len(set(rows)) != len(rows):
        raise AdmissionError(f"{label} labels must be exactly {list(expected)!r}")
