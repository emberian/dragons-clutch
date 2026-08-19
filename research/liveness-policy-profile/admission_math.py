#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Exact, fail-closed liveness quote and staged-path arithmetic.

This module deliberately knows nothing about an ELF, a SOL price, future block
inclusion, or future venue volume.  A caller supplies a measured transaction CU
observation and explicit policy caps.  If the requested headroom does not fit
under the transaction ceiling, no lamport quote exists.

The ResolutionWork helpers distinguish:

* the Begin transaction, paid directly by the payer creating the work item;
* rent principal locked in Work and Reserve;
* rewards prepaid for every accepted Fold partition and either terminal path.

They therefore cannot accidentally turn a failed transaction-headroom gate into
a finite keeper promise, or count already-paid Begin work inside the persistent
Reserve.
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
class ResolutionPathQuote:
    """Worst accepted ResolutionWork lifecycle quote.

    ``persistent_reserve_lamports`` excludes Begin's caller-paid transaction but
    includes Work/Reserve rent principal.  ``payer_cold_outlay_lamports`` adds
    the Begin transaction quote.
    """

    record_count: int
    fold_path_lamports: int | None
    success_rewards_lamports: int | None
    worst_abort_rewards_lamports: int | None
    spendable_reserve_lamports: int | None
    rent_principal_lamports: int
    persistent_reserve_lamports: int | None
    begin_external_lamports: int | None
    payer_cold_outlay_lamports: int | None
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


def resolution_path_quote(
    *,
    record_count: int,
    begin: RouteQuote,
    fold_quotes: Mapping[int, RouteQuote],
    finalize: RouteQuote,
    abort: RouteQuote,
    rent_principal_lamports: int,
) -> ResolutionPathQuote:
    """Price complete and abort alternatives, propagating every mandatory STOP."""

    if not 1 <= record_count <= 32:
        raise AdmissionError("record count must be in 1..=32")
    if rent_principal_lamports <= 0:
        raise AdmissionError("rent principal must be positive")
    if not begin.admitted:
        return ResolutionPathQuote(
            record_count,
            None,
            None,
            None,
            None,
            rent_principal_lamports,
            None,
            None,
            None,
            "STOP_BEGIN",
        )
    try:
        fold_path, _ = worst_fold_partition(record_count, fold_quotes)
    except AdmissionError:
        return ResolutionPathQuote(
            record_count,
            None,
            None,
            None,
            None,
            rent_principal_lamports,
            None,
            begin.require_reward(),
            None,
            "STOP_FOLD",
        )
    if not finalize.admitted:
        return ResolutionPathQuote(
            record_count,
            fold_path,
            None,
            None,
            None,
            rent_principal_lamports,
            None,
            begin.require_reward(),
            None,
            "STOP_FINALIZE",
        )
    if not abort.admitted:
        return ResolutionPathQuote(
            record_count,
            fold_path,
            None,
            None,
            None,
            rent_principal_lamports,
            None,
            begin.require_reward(),
            None,
            "STOP_ABORT",
        )

    finalize_reward = finalize.require_reward()
    abort_reward = abort.require_reward()
    success = fold_path + finalize_reward

    # Any prefix may expire.  Because every Fold quote is nonnegative, the
    # most expensive abort prefix is the full worst-case Fold path.  Keeping it
    # explicit documents that Abort is a path alternative, not an added job.
    worst_abort = fold_path + abort_reward
    spendable = max(success, worst_abort)
    persistent = rent_principal_lamports + spendable
    begin_external = begin.require_reward()
    return ResolutionPathQuote(
        record_count=record_count,
        fold_path_lamports=fold_path,
        success_rewards_lamports=success,
        worst_abort_rewards_lamports=worst_abort,
        spendable_reserve_lamports=spendable,
        rent_principal_lamports=rent_principal_lamports,
        persistent_reserve_lamports=persistent,
        begin_external_lamports=begin_external,
        payer_cold_outlay_lamports=persistent + begin_external,
        status="PASS",
    )


def runtime_fold_reward(base: int, per_record: int, width: int) -> int:
    """Evaluate the exact linear runtime Fold schedule."""

    if base < 0 or per_record < 0 or not 1 <= width <= 4:
        raise AdmissionError("invalid runtime Fold schedule")
    return base + per_record * width


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

    for width in range(1, 5):
        required = fold_quotes[width].require_reward()
        actual = runtime_fold_reward(fold_base_reward, fold_per_record_reward, width)
        if actual < required:
            raise AdmissionError(f"runtime Fold({width}) reward is under policy")
    if finalize_reward < finalize_quote.require_reward():
        raise AdmissionError("runtime Finalize reward is under policy")
    if abort_reward < abort_quote.require_reward():
        raise AdmissionError("runtime Abort reward is under policy")


def exact_unique_labels(rows: Sequence[int], expected: Sequence[int], label: str) -> None:
    """Reject duplicate or incomplete shape labels before flattening evidence."""

    if list(rows) != list(expected) or len(set(rows)) != len(rows):
        raise AdmissionError(f"{label} labels must be exactly {list(expected)!r}")
