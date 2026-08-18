# SPDX-License-Identifier: AGPL-3.0-or-later
"""Executed falsifiers for the POLICY_ANALYSIS_LOTS_FEES.md section 5 matrix.

Every function here is one row of that matrix.  Each returns an
:class:`ExperimentResult` that carries the row's falsification condition as a
string, the counts that prove the enumeration was not vacuous, and any
witnesses that fired.  Nothing in this module promotes a policy: the candidate
payout arms (a)/(b)/(c), the carry domains, the fee-side arms, ``kappa``, and
the 60/15/25 allocation are all unpromoted experimental parameters.

Bounds are stated per experiment and are deliberately small enough that the
whole suite stays a fast, deterministic, standard-library-only run.
"""

from __future__ import annotations

from dataclasses import dataclass, field, replace
from itertools import combinations, permutations, product
from math import lcm
from typing import Callable, Iterable, Optional, Sequence

from model import (
    CarryClose,
    CarryDomain,
    FeeSideArm,
    Fill,
    IntegerPayoutSet,
    KernelRefusal,
    ModelError,
    PayoutPolicy,
    WeightedBook,
    allocate_fee,
    dispersion_numerator,
    enumerate_weighted_traces,
    fee_fragmentation_result,
    escrow_reservation,
    exact_consideration,
    fee_denominator,
    fee_numerator,
    max_single_egg_fee_numerator,
    on_grid,
    payout_set,
    run_fee_schedule,
    single_egg_dispersion_numerator,
    sybil_wash_result,
)


@dataclass(frozen=True)
class ExperimentResult:
    """One executed matrix row."""

    experiment: str
    claim: str
    falsifier: str
    bounds: dict[str, object]
    counts: dict[str, int]
    data: dict[str, object] = field(default_factory=dict)
    witnesses: tuple[str, ...] = ()

    @property
    def falsified(self) -> bool:
        return bool(self.witnesses)

    def as_dict(self) -> dict[str, object]:
        return {
            "bounds": self.bounds,
            "claim": self.claim,
            "counts": self.counts,
            "data": self.data,
            "experiment": self.experiment,
            "falsified": self.falsified,
            "falsifier": self.falsifier,
            "witnesses": list(self.witnesses),
        }


# ---------------------------------------------------------------------------
# Bounded payout-set enumeration helpers
# ---------------------------------------------------------------------------


def weight_tuples(outcomes: int, denominator: int) -> Iterable[tuple[int, ...]]:
    """Every weight tuple of width ``outcomes`` summing to ``denominator``."""

    if outcomes == 1:
        yield (denominator,)
        return
    for head in range(denominator + 1):
        for rest in weight_tuples(outcomes - 1, denominator - head):
            yield (head, *rest)


def enumerate_payout_sets(
    outcomes: int, denominator: int, max_vectors: int = 2
) -> Iterable[IntegerPayoutSet]:
    """Every payout set of size ``1..max_vectors`` over one (outcomes, D) cell."""

    rows = tuple(weight_tuples(outcomes, denominator))
    for size in range(1, max_vectors + 1):
        for chosen in combinations(rows, size):
            yield payout_set(outcomes, chosen, denominator)


def equal_weight_family(
    outcomes: int, subset_sizes: Sequence[int]
) -> tuple[IntegerPayoutSet, int]:
    """Equal-weight compatible-subset fallbacks (ECONOMICS.md section 7 family).

    The first ``k`` outcomes carry weight ``D/k`` for each admitted subset size
    ``k``; the common denominator is ``lcm(subset_sizes)``.
    """

    denominator = 1
    for size in subset_sizes:
        denominator = lcm(denominator, size)
    rows = []
    for size in subset_sizes:
        share = denominator // size
        rows.append([share if index < size else 0 for index in range(outcomes)])
    return payout_set(outcomes, rows, denominator), denominator


# ---------------------------------------------------------------------------
# EXP-LOT rows (POLICY_ANALYSIS section 1)
# ---------------------------------------------------------------------------


def exp_lot_a1(
    max_outcomes: int = 4, max_denominator: int = 6, trace_depth: int = 5
) -> ExperimentResult:
    """EXP-LOT-A1: candidate (a) one-hot admission and remainder unreachability.

    Falsifier: any admitted set reaches a ``remainder_required`` refusal, or the
    gate refuses a one-hot set.
    """

    witnesses: list[str] = []
    admitted = refused = 0
    traced_sets = 0
    walked_states = 0
    for outcomes in range(2, max_outcomes + 1):
        for denominator in range(1, max_denominator + 1):
            for candidate in enumerate_payout_sets(outcomes, denominator):
                one_hot = all(
                    weight in (0, denominator)
                    for vector in candidate.active()
                    for weight in vector.weights[:outcomes]
                )
                try:
                    book = WeightedBook.open(candidate, PayoutPolicy.ONE_HOT)
                except KernelRefusal as refusal:
                    refused += 1
                    if one_hot:
                        witnesses.append(
                            f"one-hot set refused: {candidate} -> {refusal.error_class}"
                        )
                    continue
                admitted += 1
                if not one_hot:
                    witnesses.append(f"non one-hot set admitted: {candidate}")
                    continue
                if outcomes <= 3 and denominator <= 3 and candidate.count <= 2:
                    traced_sets += 1
                    walk = enumerate_weighted_traces(
                        candidate,
                        PayoutPolicy.ONE_HOT,
                        depth=trace_depth,
                        collateral_cap=2,
                    )
                    walked_states += int(walk["states"])
                    refusal_classes = walk["refusals"]
                    if refusal_classes.get("remainder_required"):
                        witnesses.append(
                            f"remainder reachable under one-hot set {candidate}"
                        )
                    if walk["exit_dead_states"]:
                        witnesses.append(f"exit-dead state under one-hot set {candidate}")
    return ExperimentResult(
        experiment="EXP-LOT-A1",
        claim=(
            "candidate (a) admits exactly the one-hot payout sets and makes "
            "remainder_required unreachable"
        ),
        falsifier=(
            "any admitted set reaches a refusal class remainder_required, any "
            "one-hot set is refused, or any admitted set holds an exit-dead state"
        ),
        bounds={
            "outcomes": f"2..{max_outcomes}",
            "denominator": f"1..{max_denominator}",
            "payout_set_size": "1..2",
            "trace_cell": "outcomes<=3, D<=3, count<=2",
            "trace_depth": trace_depth,
        },
        counts={
            "admitted": admitted,
            "refused": refused,
            "traced_sets": traced_sets,
            "walked_states": walked_states,
        },
        witnesses=tuple(witnesses[:8]),
    )


def exp_lot_b1(max_denominator: int = 12) -> ExperimentResult:
    """EXP-LOT-B1: ``L_i = D / gcd(D, {v_i != 0})`` is the minimal q-modulus.

    Falsifier: either direction fails -- a multiple of ``L_i`` that remainders
    under some admitted resolution, or a non-multiple that redeems exactly under
    every admitted resolution.
    """

    witnesses: list[str] = []
    checked_sets = 0
    checked_quantities = 0
    for outcomes, denominators in ((2, range(1, max_denominator + 1)), (3, range(1, 7))):
        for denominator in denominators:
            for candidate in enumerate_payout_sets(outcomes, denominator):
                checked_sets += 1
                for outcome in range(outcomes):
                    lot = candidate.redemption_lot(outcome)
                    for quantity in range(1, 4 * denominator + 1):
                        checked_quantities += 1
                        exact_everywhere = all(
                            (quantity * vector.weight(outcome)) % denominator == 0
                            for vector in candidate.active()
                        )
                        if exact_everywhere != (quantity % lot == 0):
                            witnesses.append(
                                f"lot {lot} wrong for outcome {outcome} q={quantity} "
                                f"set={candidate}"
                            )
    return ExperimentResult(
        experiment="EXP-LOT-B1",
        claim="the derived redemption lot is exactly the minimal exact-redemption modulus",
        falsifier=(
            "any (set, outcome, quantity) where lot divisibility and exact "
            "redemption under every admitted vector disagree"
        ),
        bounds={
            "outcomes": "2 (D<=12), 3 (D<=6)",
            "payout_set_size": "1..2",
            "quantity": "1..4D",
        },
        counts={"sets": checked_sets, "quantities": checked_quantities},
        witnesses=tuple(witnesses[:8]),
    )


def exp_lot_b2(max_denominator: int = 4, depth: int = 4) -> ExperimentResult:
    """EXP-LOT-B2: candidate (b) internal closure.

    Falsifier: any reachable exit-dead internal state under lot gating, any
    reachable sub-lot internal balance, or the P1-A fixture not refused at split.
    """

    witnesses: list[str] = []
    walked = 0
    states = 0
    resolved_sub_lot = 0
    for outcomes in (2, 3):
        for denominator in range(1, max_denominator + 1):
            if outcomes == 3 and denominator > 3:
                continue
            for candidate in enumerate_payout_sets(outcomes, denominator):
                walked += 1
                walk = enumerate_weighted_traces(
                    candidate,
                    PayoutPolicy.LOTS,
                    depth=depth,
                    quantities=(1, candidate.split_lot()),
                    collateral_cap=2 * candidate.split_lot(),
                )
                states += int(walk["states"])
                resolved_sub_lot += int(walk["resolved_sub_lot_internal_states"])
                if walk["active_sub_lot_internal_states"]:
                    witnesses.append(
                        f"Active-phase sub-lot internal balance reachable: {candidate}"
                    )
                if walk["exit_dead_states"]:
                    witnesses.append(f"exit-dead state reachable: {candidate}")
    p1a = payout_set(2, [[1, 1]], 2)
    book = WeightedBook.open(p1a, PayoutPolicy.LOTS)
    try:
        book.split(0, 1)
        witnesses.append("P1-A split of one atom admitted under lot gating")
        p1a_class = "admitted"
    except KernelRefusal as refusal:
        p1a_class = refusal.error_class
        if refusal.error_class != "lot_violation":
            witnesses.append(f"P1-A split refused with {refusal.error_class}")
    return ExperimentResult(
        experiment="EXP-LOT-B2",
        claim=(
            "lot-gated Active-phase transitions cannot reach a sub-lot internal "
            "balance, and no lot-gated state is exit-dead in either phase"
        ),
        falsifier=(
            "any reachable Active-phase sub-lot internal balance, any reachable "
            "exit-dead state, or a P1-A single-atom split that is not lot-refused"
        ),
        bounds={
            "outcomes": f"2 (D<={max_denominator}), 3 (D<=3)",
            "payout_set_size": "1..2",
            "depth": depth,
            "complete_set_gate": "L_split under candidate (b)",
        },
        counts={
            "sets": walked,
            "states": states,
            "resolved_sub_lot_internal_states": resolved_sub_lot,
        },
        data={
            "p1a_split_refusal": p1a_class,
            "p1a_split_lot": p1a.split_lot(),
            "note_resolved_sub_lot": (
                "after resolution the binding modulus is D/gcd(D, resolved w_i), "
                "which can be smaller than the set-wide L_i; those balances are "
                "sub-set-lot but redeem exactly, so they are counted, not refused"
            ),
        },
        witnesses=tuple(witnesses[:8]),
    )


def exp_lot_b3(max_total: int = 6) -> ExperimentResult:
    """EXP-LOT-B3: candidate (b1) external fragmentation adversary (3 wallets).

    Falsifier: trapped collateral exceeds the sub-lot dust bound, aggregate
    supply leaves lot alignment, or full-lot recombination fails to recover.
    """

    witnesses: list[str] = []
    schedules = 0
    max_trapped = 0
    candidate = payout_set(2, [[2, 2], [4, 0]], 4)
    lot0 = candidate.redemption_lot(0)
    split_lot = candidate.split_lot()
    for total in range(split_lot, max_total + 1, split_lot):
        base = WeightedBook.open(candidate, PayoutPolicy.LOTS, wallets=3)
        base = base.split(0, total)
        base = base.materialize(0, 0, total - (total % lot0))
        externals = base.positions[0].external[0]
        for first in range(externals + 1):
            for second in range(externals - first + 1):
                schedules += 1
                book = base
                if first:
                    book = book.transfer_external(0, 1, 0, first)
                if second:
                    book = book.transfer_external(0, 2, 0, second)
                for index in range(candidate.count):
                    resolved = book.resolve(index)
                    if sum(resolved.total_supply) and any(
                        supply % candidate.redemption_lot(outcome) != 0
                        for outcome, supply in enumerate(resolved.total_supply)
                    ):
                        witnesses.append("aggregate supply left lot alignment")
                    trapped = resolved.stranded_numerator()
                    max_trapped = max(max_trapped, trapped)
                    bound = 3 * candidate.outcomes * candidate.denominator
                    if trapped >= bound:
                        witnesses.append(
                            f"trapped {trapped} exceeds sub-lot dust bound {bound}"
                        )
                    # Recombination: fragments returned to one wallet must exit.
                    recombined = resolved
                    for wallet in (1, 2):
                        amount = recombined.positions[wallet].external[0]
                        if amount:
                            recombined = recombined.transfer_external(wallet, 0, 0, amount)
                    if recombined.stranded_numerator() != 0:
                        witnesses.append("full-lot recombination did not recover value")
    return ExperimentResult(
        experiment="EXP-LOT-B3",
        claim=(
            "under (b1) external fragmentation strands at most sub-lot dust per "
            "wallet and recombination always recovers it"
        ),
        falsifier=(
            "trapped value at or above one atom per wallet-outcome, aggregate "
            "supply off lot alignment, or recombination that cannot recover"
        ),
        bounds={
            "wallets": 3,
            "payout_set": "outcomes=2, D=4, vectors=[[2,2],[4,0]]",
            "external_total": f"multiples of {split_lot} up to {max_total}",
        },
        counts={"schedules": schedules, "max_trapped_numerator": max_trapped},
        data={
            "redemption_lots": [candidate.redemption_lot(i) for i in range(2)],
            "split_lot": split_lot,
        },
        witnesses=tuple(witnesses[:8]),
    )


def exp_lot_b4(max_outcomes: int = 16) -> ExperimentResult:
    """EXP-LOT-B4: lot magnitude for the candidate ambiguity families (data row).

    Falsifier (data, plus a stated-arithmetic check): the section 1.3 claim that
    equal-weight fallbacks over subset sizes 2..8 force ``D = 840`` and
    ``L_split = 840`` at six-decimal collateral.
    """

    witnesses: list[str] = []
    rows: list[dict[str, object]] = []
    families = (
        ("one_hot_only", 4, (1,)),
        ("halves", 4, (2,)),
        ("sizes_2_3", 4, (2, 3)),
        ("sizes_2_to_4", 8, (2, 3, 4)),
        ("sizes_2_to_8", max_outcomes, (2, 3, 4, 5, 6, 7, 8)),
        ("adversarial_5_7_8", max_outcomes, (5, 7, 8)),
    )
    for name, outcomes, sizes in families:
        candidate, denominator = equal_weight_family(outcomes, sizes)
        candidate.validate()
        lots = [candidate.redemption_lot(index) for index in range(outcomes)]
        rows.append(
            {
                "family": name,
                "outcomes": outcomes,
                "subset_sizes": list(sizes),
                "denominator": denominator,
                "redemption_lots": lots,
                "split_lot": candidate.split_lot(),
                "split_lot_in_six_decimal_tokens": f"{candidate.split_lot()}/1000000",
                "vectors": candidate.count,
            }
        )
    full = next(row for row in rows if row["family"] == "sizes_2_to_8")
    if full["denominator"] != 840 or full["split_lot"] != 840:
        witnesses.append(
            "section 1.3 arithmetic wrong: expected D=840 and L_split=840, got "
            f"{full['denominator']}/{full['split_lot']}"
        )
    one_hot_row = next(row for row in rows if row["family"] == "one_hot_only")
    if one_hot_row["split_lot"] != 1 or set(one_hot_row["redemption_lots"]) != {1}:
        witnesses.append("one-hot family did not degenerate to unit lots")
    return ExperimentResult(
        experiment="EXP-LOT-B4",
        claim="lot magnitude for the intended equal-weight compatibility families",
        falsifier=(
            "the document's stated D=840 / L_split=840 for subset sizes 2..8 is "
            "wrong, or one-hot families do not degenerate to unit lots"
        ),
        bounds={"outcomes": f"<= {max_outcomes}", "vectors": "<= 8"},
        counts={"families": len(rows)},
        data={"rows": rows},
        witnesses=tuple(witnesses[:8]),
    )


def exp_lot_b5(max_denominator: int = 6, supply_multiples: int = 2) -> ExperimentResult:
    """EXP-LOT-B5: lot-aligned states reserve exactly (no ceiling).

    Falsifier: any lot-aligned state whose ``required_collateral`` rounds up.
    """

    witnesses: list[str] = []
    checked = 0
    for outcomes in (2, 3):
        for denominator in range(1, max_denominator + 1):
            for candidate in enumerate_payout_sets(outcomes, denominator):
                lots = [candidate.redemption_lot(i) for i in range(outcomes)]
                grids = [
                    range(0, supply_multiples * lot + 1, lot) for lot in lots
                ]
                empty = WeightedBook.open(candidate, PayoutPolicy.LOTS)
                for supplies in product(*grids):
                    checked += 1
                    book = replace(
                        empty,
                        collateral=sum(supplies),
                        total_supply=tuple(supplies),
                    )
                    if not book.liability_is_integral():
                        witnesses.append(
                            f"lot-aligned supplies {supplies} ceiling under {candidate}"
                        )
    return ExperimentResult(
        experiment="EXP-LOT-B5",
        claim="every lot-aligned state has integral liability, so reservation is exact",
        falsifier="any lot-aligned state whose required collateral rounds up",
        bounds={
            "outcomes": "2..3",
            "denominator": f"1..{max_denominator}",
            "supply": f"multiples of L_i up to {supply_multiples}*L_i",
        },
        counts={"states": checked},
        witnesses=tuple(witnesses[:8]),
    )


def _compositions(total: int, parts: int) -> Iterable[tuple[int, ...]]:
    """Every ordered decomposition of ``total`` into ``parts`` nonnegative ints."""

    if parts == 1:
        yield (total,)
        return
    for head in range(total + 1):
        for rest in _compositions(total - head, parts - 1):
            yield (head, *rest)


def _positive_compositions(total: int, parts: Optional[int] = None) -> Iterable[tuple[int, ...]]:
    """Every ordered decomposition of ``total`` into strictly positive parts.

    With ``parts`` omitted this is all ``2**(total-1)`` compositions; with
    ``parts`` given it is the fixed-length family.
    """

    if parts is None:
        for size in range(1, total + 1):
            yield from _positive_compositions(total, size)
        return
    if parts <= 0 or total < parts:
        return
    if parts == 1:
        yield (total,)
        return
    for head in range(1, total - parts + 2):
        for rest in _positive_compositions(total - head, parts - 1):
            yield (head, *rest)


def exp_lot_c1(max_denominator: int = 6, max_quantity: int = 8) -> ExperimentResult:
    """EXP-LOT-C1: candidate (c) credit conservation.

    Falsifier: one atom created or destroyed by any redemption or fragmentation,
    a carry outside ``[0, D)``, or a solvency invariant violation with the
    ``credit_num_total`` term.
    """

    witnesses: list[str] = []
    redemptions = 0
    fragmentations = 0
    for outcomes in (2, 3):
        for denominator in range(1, max_denominator + 1):
            if outcomes == 3 and denominator > 3:
                continue
            for candidate in enumerate_payout_sets(outcomes, denominator):
                for index in range(candidate.count):
                    vector = candidate.vector(index)
                    for outcome in range(outcomes):
                        weight = vector.weight(outcome)
                        for quantity in range(1, max_quantity + 1):
                            base = WeightedBook.open(
                                candidate, PayoutPolicy.CREDIT, wallets=3
                            )
                            base = base.split(0, quantity).resolve(index)
                            book, paid = base.redeem_internal(0, outcome, quantity)
                            redemptions += 1
                            credit = book.positions[0].credit
                            if quantity * weight != denominator * paid + credit:
                                witnesses.append(
                                    "per-step conservation broken for "
                                    f"q={quantity} w={weight} D={denominator}"
                                )
                            if not 0 <= credit < denominator:
                                witnesses.append(f"noncanonical credit {credit}")
                            if book.credit_total != credit:
                                witnesses.append("market credit total disagrees")
                            if not book.is_solvent():
                                witnesses.append("credit arm broke solvency")
                            # Fragmenting the same redemption inside one position
                            # must pay identically.
                            for parts in (2, 3):
                                if quantity < parts or quantity > 6:
                                    continue
                                for schedule in _positive_compositions(quantity, parts):
                                    fragmentations += 1
                                    walk = base
                                    total_paid = 0
                                    for part in schedule:
                                        walk, step = walk.redeem_internal(
                                            0, outcome, part
                                        )
                                        total_paid += step
                                    try:
                                        walk, claimed = walk.claim_credit(0)
                                        total_paid += claimed
                                    except KernelRefusal:
                                        pass
                                    whole = paid
                                    if total_paid != whole:
                                        witnesses.append(
                                            "fragmented redemption paid "
                                            f"{total_paid} != {whole}"
                                        )
                                    if walk.positions[0].credit != credit:
                                        witnesses.append(
                                            "fragmented redemption left a different credit"
                                        )
    return ExperimentResult(
        experiment="EXP-LOT-C1",
        claim="candidate (c) conserves every atom exactly and keeps credit in [0, D)",
        falsifier=(
            "any step where q*v_i != D*paid + credit_delta, a credit outside "
            "[0, D), a fragmentation that pays differently, or an insolvent state"
        ),
        bounds={
            "outcomes": f"2 (D<={max_denominator}), 3 (D<=3)",
            "payout_set_size": "1..2",
            "positions": 3,
            "quantity": f"1..{max_quantity}",
            "fragments": "2..3 for q<=6",
        },
        counts={"redemptions": redemptions, "fragmentations": fragmentations},
        witnesses=tuple(witnesses[:8]),
    )


def exp_lot_c2(max_positions: int = 4, max_quantity: int = 6) -> ExperimentResult:
    """EXP-LOT-C2: candidate (c) fragmented-redemption arbitrage.

    Falsifier: any positive arbitrage -- a split across positions that pays more
    than the unsplit redemption, or stranded residue at or above ``k`` atoms.
    """

    witnesses: list[str] = []
    schedules = 0
    for denominator in range(2, 7):
        for weight in range(1, denominator):
            vectors = [[weight, denominator - weight], [denominator, 0]]
            candidate = payout_set(2, vectors, denominator)
            for quantity in range(1, max_quantity + 1):
                unsplit_book = WeightedBook.open(
                    candidate, PayoutPolicy.CREDIT, wallets=max_positions
                )
                unsplit_book = unsplit_book.split(0, quantity).resolve(0)
                unsplit_book, unsplit_paid = unsplit_book.redeem_internal(
                    0, 0, quantity
                )
                try:
                    unsplit_book, extra = unsplit_book.claim_credit(0)
                    unsplit_paid += extra
                except KernelRefusal:
                    pass
                for positions in range(2, max_positions + 1):
                    for schedule in _compositions(quantity, positions):
                        schedules += 1
                        book = WeightedBook.open(
                            candidate, PayoutPolicy.CREDIT, wallets=max_positions
                        )
                        book = book.split(0, quantity)
                        for wallet, part in enumerate(schedule):
                            if wallet == 0 or part == 0:
                                continue
                            # Move claims to the Sybil's other positions before
                            # resolution; only outcome 0 matters for the payout.
                            book = book.materialize(0, 0, part)
                            book = book.transfer_external(0, wallet, 0, part)
                        book = book.resolve(0)
                        paid = 0
                        residue = 0
                        for wallet, part in enumerate(schedule):
                            if part == 0:
                                continue
                            internal = wallet == 0
                            book, step = (
                                book.redeem_internal(wallet, 0, part)
                                if internal
                                else book.redeem_external(wallet, 0, part)
                            )
                            paid += step
                            try:
                                book, extra = book.claim_credit(wallet)
                                paid += extra
                            except KernelRefusal:
                                pass
                            residue += book.positions[wallet].credit
                        if paid > unsplit_paid:
                            witnesses.append(
                                f"fragmentation paid {paid} > unsplit {unsplit_paid}"
                            )
                        live = sum(1 for part in schedule if part)
                        if residue >= live * denominator:
                            witnesses.append(
                                f"stranded residue {residue} >= {live} atoms"
                            )
    return ExperimentResult(
        experiment="EXP-LOT-C2",
        claim="splitting a redemption across positions never pays more and strands < k atoms",
        falsifier="any positive arbitrage or residue at or above one atom per position",
        bounds={
            "denominator": "2..6",
            "positions": f"2..{max_positions}",
            "quantity": f"1..{max_quantity}",
        },
        counts={"schedules": schedules},
        witnesses=tuple(witnesses[:8]),
    )


def exp_lot_x1(max_denominator: int = 5, max_quantity: int = 4) -> ExperimentResult:
    """EXP-LOT-X1: terminal complete-set redemption (section 1.5) in every arm.

    Falsifier: any remainder, any payout other than the burned quantity, or any
    solvency drift; the P1-A trap failing to exit.
    """

    witnesses: list[str] = []
    checked = 0
    for outcomes in (2, 3):
        for denominator in range(1, max_denominator + 1):
            for candidate in enumerate_payout_sets(outcomes, denominator):
                for policy in (
                    PayoutPolicy.KERNEL_BASELINE,
                    PayoutPolicy.LOTS,
                    PayoutPolicy.CREDIT,
                ):
                    try:
                        book = WeightedBook.open(candidate, policy)
                    except KernelRefusal:
                        continue
                    lot = candidate.split_lot() if policy is PayoutPolicy.LOTS else 1
                    for quantity in range(lot, max_quantity + 1, lot):
                        for index in range(candidate.count):
                            checked += 1
                            state = book.split(0, quantity).resolve(index)
                            before = state.collateral
                            state, paid = state.redeem_complete_set(0, quantity)
                            if paid != quantity:
                                witnesses.append(
                                    f"complete-set redemption paid {paid} != {quantity}"
                                )
                            if state.collateral != before - quantity:
                                witnesses.append("collateral drift on complete-set exit")
                            if not state.is_solvent():
                                witnesses.append("complete-set exit broke solvency")
                            if state.required_collateral() != 0:
                                witnesses.append("liability survived a full exit")
    p1a = payout_set(2, [[1, 1]], 2)
    trap = WeightedBook.open(p1a, PayoutPolicy.KERNEL_BASELINE).split(0, 1).resolve(0)
    trap_before = trap.retirement_residue(terminal_complete_set=False)
    exited, paid = trap.redeem_complete_set(0, 1)
    if paid != 1 or exited.collateral != 0 or sum(exited.total_supply) != 0:
        witnesses.append("P1-A trap did not exit under complete-set redemption")
    return ExperimentResult(
        experiment="EXP-LOT-X1",
        claim="joint complete-set redemption is exact in every arm and exits the P1-A trap",
        falsifier="any remainder, wrong payout, solvency drift, or a surviving P1-A trap",
        bounds={
            "outcomes": "2..3",
            "denominator": f"1..{max_denominator}",
            "quantity": f"1..{max_quantity}",
        },
        counts={"redemptions": checked},
        data={
            "p1a_residue_without_primitive": trap_before,
            "p1a_payout_with_primitive": paid,
        },
        witnesses=tuple(witnesses[:8]),
    )


def exp_lot_x2(depth: int = 5) -> ExperimentResult:
    """EXP-LOT-X2: retirement liveness per candidate arm (data row).

    Falsifier (expectation check): the section 1.3 finding that (b1) admits
    irreparable zombie markets while one-hot does not, and the section 1.5 claim
    that the complete-set primitive removes the baseline P1-A zombie.
    """

    witnesses: list[str] = []
    rows: list[dict[str, object]] = []
    fractional = payout_set(2, [[1, 1]], 2)
    one_hot = payout_set(2, [[1, 0], [0, 1]], 1)
    scenarios = (
        ("kernel_baseline_fractional", fractional, PayoutPolicy.KERNEL_BASELINE, 1, False),
        ("one_hot", one_hot, PayoutPolicy.ONE_HOT, 1, False),
        ("credit_fractional", fractional, PayoutPolicy.CREDIT, 1, False),
        ("lots_b1_external", payout_set(2, [[2, 2], [4, 0]], 4), PayoutPolicy.LOTS, 3, True),
    )
    for name, candidate, policy, wallets, transfers in scenarios:
        quantities = (1, candidate.split_lot()) if policy is PayoutPolicy.LOTS else (1,)
        walk_states = _reachable_resolved_states(
            candidate,
            policy,
            depth=depth,
            wallets=wallets,
            quantities=quantities,
            allow_external_transfer=transfers,
            collateral_cap=2 * candidate.split_lot(),
        )
        with_primitive = sum(
            1 for state in walk_states if state.is_unretireable(True)
        )
        without_primitive = sum(
            1 for state in walk_states if state.is_unretireable(False)
        )
        rows.append(
            {
                "arm": name,
                "policy": policy.value,
                "resolved_states": len(walk_states),
                "unretireable_with_complete_set_primitive": with_primitive,
                "unretireable_without_primitive": without_primitive,
            }
        )
    by_arm = {row["arm"]: row for row in rows}
    if by_arm["one_hot"]["unretireable_with_complete_set_primitive"]:
        witnesses.append("one-hot arm produced an unretireable state")
    if by_arm["credit_fractional"]["unretireable_with_complete_set_primitive"]:
        witnesses.append("credit arm produced an unretireable state")
    if not by_arm["kernel_baseline_fractional"]["unretireable_without_primitive"]:
        witnesses.append("baseline fractional arm showed no zombie without the primitive")
    if not by_arm["lots_b1_external"]["unretireable_with_complete_set_primitive"]:
        witnesses.append("(b1) external fragmentation produced no zombie state")
    return ExperimentResult(
        experiment="EXP-LOT-X2",
        claim="retirement liveness differs per candidate; (b1) admits irreparable zombies",
        falsifier=(
            "one-hot or credit arms admit an unretireable state, or (b1) external "
            "fragmentation admits none, or the baseline shows no zombie without "
            "the complete-set primitive"
        ),
        bounds={"depth": depth, "wallets": "1..3"},
        counts={"scenarios": len(rows)},
        data={"rows": rows},
        witnesses=tuple(witnesses[:8]),
    )


def _reachable_resolved_states(
    payouts: IntegerPayoutSet,
    policy: PayoutPolicy,
    depth: int,
    wallets: int,
    quantities: Sequence[int],
    allow_external_transfer: bool,
    collateral_cap: int,
) -> tuple[WeightedBook, ...]:
    """Collect the resolved states of one bounded walk (helper for EXP-LOT-X2)."""

    initial = WeightedBook.open(payouts, policy, wallets=wallets)
    seen = {initial}
    frontier = [initial]
    for _ in range(depth):
        nxt: list[WeightedBook] = []
        for state in frontier:
            candidates: list[Callable[[], object]] = []
            for wallet in range(wallets):
                for quantity in quantities:
                    if state.resolved_payout is None:
                        if state.collateral + quantity <= collateral_cap:
                            candidates.append(
                                lambda s=state, w=wallet, q=quantity: s.split(w, q)
                            )
                        for outcome in range(payouts.outcomes):
                            candidates.append(
                                lambda s=state, w=wallet, o=outcome, q=quantity: (
                                    s.materialize(w, o, q)
                                )
                            )
                        if allow_external_transfer:
                            for other in range(wallets):
                                if other == wallet:
                                    continue
                                for outcome in range(payouts.outcomes):
                                    candidates.append(
                                        lambda s=state, a=wallet, b=other, o=outcome, q=quantity: (
                                            s.transfer_external(a, b, o, q)
                                        )
                                    )
                    else:
                        for outcome in range(payouts.outcomes):
                            candidates.append(
                                lambda s=state, w=wallet, o=outcome, q=quantity: (
                                    s.redeem_internal(w, o, q)
                                )
                            )
            if state.resolved_payout is None:
                for index in range(payouts.count):
                    candidates.append(lambda s=state, i=index: s.resolve(i))
            for transition in candidates:
                try:
                    outcome = transition()
                except ModelError:
                    continue
                book = outcome[0] if isinstance(outcome, tuple) else outcome
                if book not in seen:
                    seen.add(book)
                    nxt.append(book)
        frontier = nxt
        if not frontier:
            break
    return tuple(
        state
        for state in sorted(seen, key=lambda item: repr(item))
        if state.resolved_payout is not None and sum(state.total_supply) > 0
    )


# ---------------------------------------------------------------------------
# EXP-FEE rows (POLICY_ANALYSIS section 2)
# ---------------------------------------------------------------------------

#: Experimental fee parameters.  Neither the kappa nor the allocation split is a
#: promoted protocol constant; both are arms.
KAPPA_NUM = 4
KAPPA_DEN = 1_000


def _dust_fills(
    count: int,
    quantity: int,
    price: int,
    intents: int = 1,
    positions: int = 1,
    epochs: int = 1,
) -> tuple[Fill, ...]:
    fills = []
    for index in range(count):
        fills.append(
            Fill(
                quantity=quantity,
                price=price,
                buyer_intent=f"buy-{index % intents}",
                seller_intent=f"sell-{index % intents}",
                buyer_position=f"buyer-{index % positions}",
                seller_position=f"seller-{index % positions}",
                epoch=index % epochs,
            )
        )
    return tuple(fills)


def exp_fee_d1(
    price_scale: int = 100,
    price: int = 50,
    max_quantity: int = 13,
    wide_quantity: int = 24,
) -> ExperimentResult:
    """EXP-FEE-D1: terminal-ceil carry is fragmentation-exact and reset-proof.

    Falsifier: any fragmentation that pays less than ``ceil(exact)`` inside one
    domain instance, any cross-domain split that pays less than the unsplit
    schedule, or a positive carry-reset gain.
    """

    witnesses: list[str] = []
    compositions_checked = 0
    cross_domain_checked = 0
    max_reset_gain = 0
    for total in range(1, max_quantity + 1):
        for schedule in _positive_compositions(total):
            compositions_checked += 1
            result = fee_fragmentation_result(
                schedule, price, price_scale, KAPPA_NUM, KAPPA_DEN
            )
            if result["terminal_ceil_total"] != result["exact_ceil_total"]:
                witnesses.append(
                    f"terminal-ceil total {result['terminal_ceil_total']} != "
                    f"ceil {result['exact_ceil_total']} for {schedule}"
                )
            if result["dropped_carry_total"] > result["terminal_ceil_total"]:
                witnesses.append("dropped carry charged more than terminal ceil")
    for total in range(1, wide_quantity + 1):
        for parts in (2, 3):
            for schedule in _positive_compositions(total, parts):
                compositions_checked += 1
                result = fee_fragmentation_result(
                    schedule, price, price_scale, KAPPA_NUM, KAPPA_DEN
                )
                if result["terminal_ceil_total"] != result["exact_ceil_total"]:
                    witnesses.append(f"wide composition {schedule} broke the identity")
    # Quantities are doubled so that every consideration stays exactly on the
    # (S=100, p=50) cash grid; the fee numerator is linear in quantity, so the
    # fragmentation structure under test is unchanged.
    for total in range(1, 13):
        whole = run_fee_schedule(
            _dust_fills(1, 2 * total, price),
            price_scale,
            KAPPA_NUM,
            KAPPA_DEN,
            domain=CarryDomain.INTENT,
            close_policy=CarryClose.TERMINAL_CEIL,
            # PROPOSED variant, explicitly named (P0-5)
            side_arm=FeeSideArm.PER_INTENT_BOTH_SIDES,
        )
        for instances in range(1, 5):
            for schedule in _positive_compositions(total, instances):
                cross_domain_checked += 1
                fills = tuple(
                    Fill(
                        quantity=2 * part,
                        price=price,
                        buyer_intent=f"buy-{index}",
                        seller_intent=f"sell-{index}",
                    )
                    for index, part in enumerate(schedule)
                )
                split = run_fee_schedule(
                    fills,
                    price_scale,
                    KAPPA_NUM,
                    KAPPA_DEN,
                    domain=CarryDomain.INTENT,
                    close_policy=CarryClose.TERMINAL_CEIL,
                    # PROPOSED variant, explicitly named (P0-5)
                    side_arm=FeeSideArm.PER_INTENT_BOTH_SIDES,
                )
                if split.fee_pot < whole.fee_pot:
                    witnesses.append(
                        f"splitting into {instances} intents paid "
                        f"{split.fee_pot} < {whole.fee_pot}"
                    )
                max_reset_gain = max(max_reset_gain, whole.fee_pot - split.fee_pot)
    return ExperimentResult(
        experiment="EXP-FEE-D1",
        claim=(
            "terminal-ceil carry pays exactly ceil(exact) per domain instance and "
            "inverts the sign of the carry-reset attack"
        ),
        falsifier=(
            "any within-domain fragmentation paying less than ceil(exact), any "
            "cross-domain split paying less than the unsplit schedule, or a "
            "positive reset gain"
        ),
        bounds={
            "within_domain_quantity": f"all compositions of 1..{max_quantity}",
            "wide_quantity": f"2- and 3-part compositions of 1..{wide_quantity}",
            "domain_instances": "1..4",
            "price_scale": price_scale,
            "price": price,
        },
        counts={
            "compositions": compositions_checked,
            "cross_domain_schedules": cross_domain_checked,
            "max_reset_gain": max_reset_gain,
        },
        witnesses=tuple(witnesses[:8]),
    )


def exp_fee_d2(
    price_scale: int = 100, price: int = 50, epochs: int = 24
) -> ExperimentResult:
    """EXP-FEE-D2: the Epoch carry domain with dropped carry loses dust fees.

    Falsifier (regression for the refusal): if fees do NOT vanish under dropped
    epoch carry while volume is positive, the section 2.2 Epoch-domain criticism
    weakens.
    """

    witnesses: list[str] = []
    rows: list[dict[str, object]] = []
    for count in range(1, epochs + 1):
        fills = _dust_fills(count, 2, price, epochs=count)
        volume = sum(fill.quantity for fill in fills)
        # PROPOSED variant, explicitly named (P0-5): the side arm is held
        # fixed across the three close-policy arms compared here.
        dropped = run_fee_schedule(
            fills,
            price_scale,
            KAPPA_NUM,
            KAPPA_DEN,
            domain=CarryDomain.EPOCH,
            close_policy=CarryClose.DROPPED,
            side_arm=FeeSideArm.PER_INTENT_BOTH_SIDES,
        )
        terminal = run_fee_schedule(
            fills,
            price_scale,
            KAPPA_NUM,
            KAPPA_DEN,
            domain=CarryDomain.EPOCH,
            close_policy=CarryClose.TERMINAL_CEIL,
            side_arm=FeeSideArm.PER_INTENT_BOTH_SIDES,
        )
        intent_terminal = run_fee_schedule(
            fills,
            price_scale,
            KAPPA_NUM,
            KAPPA_DEN,
            domain=CarryDomain.INTENT,
            close_policy=CarryClose.TERMINAL_CEIL,
            side_arm=FeeSideArm.PER_INTENT_BOTH_SIDES,
        )
        rows.append(
            {
                "epochs": count,
                "volume": volume,
                "epoch_dropped_pot": dropped.fee_pot,
                "epoch_terminal_ceil_pot": terminal.fee_pot,
                "intent_terminal_ceil_pot": intent_terminal.fee_pot,
            }
        )
        if volume > 0 and dropped.fee_pot != 0:
            witnesses.append(
                f"dropped epoch carry still collected {dropped.fee_pot} at {count} epochs"
            )
        if terminal.fee_pot < count:
            witnesses.append("epoch terminal-ceil collected less than one atom per epoch")
    return ExperimentResult(
        experiment="EXP-FEE-D2",
        claim="dropped epoch carry drives dust fees to zero while volume is positive",
        falsifier=(
            "dust fees do not vanish under dropped epoch carry (which would weaken "
            "the Epoch-domain refusal) or terminal-ceil fails to floor at one atom "
            "per instance"
        ),
        bounds={"epochs": f"1..{epochs}", "price_scale": price_scale, "price": price},
        counts={"schedules": len(rows)},
        data={"rows": rows},
        witnesses=tuple(witnesses[:8]),
    )


def exp_fee_p1(
    price_scale: int = 20, max_quantity: int = 8
) -> ExperimentResult:
    """EXP-FEE-P1: payer conservation, escrow head-room, and an untouched Hoard.

    Falsifier: any schedule where ``sum(buyer debits) - sum(seller credits)``
    differs from the fee pot delta, any negative seller credit, any intent whose
    cash exceeds its escrow reservation, or any Hoard movement.
    """

    witnesses: list[str] = []
    schedules = 0
    denominator = fee_denominator(KAPPA_DEN, price_scale)
    grid = [
        (quantity, price)
        for quantity in range(1, max_quantity + 1)
        for price in range(0, price_scale + 1)
        if on_grid(quantity, price, price_scale)
    ]
    for domain in CarryDomain:
        for close_policy in CarryClose:
            for side_arm in FeeSideArm:
                for quantity, price in grid:
                    for parts in (1, 2, 3):
                        fills = tuple(
                            Fill(quantity=quantity, price=price, epoch=index)
                            for index in range(parts)
                        )
                        schedules += 1
                        result = run_fee_schedule(
                            fills,
                            price_scale,
                            KAPPA_NUM,
                            KAPPA_DEN,
                            domain=domain,
                            close_policy=close_policy,
                            side_arm=side_arm,
                        )
                        if not result.conserves:
                            witnesses.append(
                                "conservation miss: "
                                f"{result.buyer_debit_total} - "
                                f"{result.seller_credit_total} != {result.fee_pot}"
                            )
                        if result.hoard_delta != 0:
                            witnesses.append("a fee leg touched the Hoard")
                        cash = dict(result.intent_cash)
                        for intent, value in cash.items():
                            if intent.startswith("sell") and value < 0:
                                witnesses.append(
                                    f"seller credit went negative for {intent}"
                                )
                        limit_consideration = parts * exact_consideration(
                            quantity, price, price_scale
                        )
                        reservation = escrow_reservation(
                            limit_consideration,
                            max_single_egg_fee_numerator(
                                parts * quantity, price_scale, KAPPA_NUM
                            ),
                            denominator,
                        )
                        for intent, value in cash.items():
                            if intent.startswith("buy") and value > reservation:
                                witnesses.append(
                                    f"buyer cash {value} exceeded escrow {reservation}"
                                )
                        # Joint cash conservation of the whole vertical: the
                        # payers' cash deltas and the pot sum to zero.
                        net = (
                            -result.buyer_debit_total
                            + result.seller_credit_total
                            + result.fee_pot
                        )
                        if net != 0:
                            witnesses.append(f"vertical cash did not conserve: {net}")
    return ExperimentResult(
        experiment="EXP-FEE-P1",
        claim=(
            "every fee atom is debited from a named payer, the identity "
            "sum(buyer debits) - sum(seller credits) = fee pot holds, and the "
            "Hoard is never a fee source"
        ),
        falsifier=(
            "any conservation miss, negative seller credit, escrow overrun, or "
            "Hoard movement in any (domain x close x side arm) cell"
        ),
        bounds={
            "price_scale": price_scale,
            "quantity": f"1..{max_quantity} on-grid",
            "fills_per_schedule": "1..3",
            "cells": "3 domains x 2 close policies x 2 side arms",
        },
        counts={"schedules": schedules},
        witnesses=tuple(witnesses[:8]),
    )


def exp_fee_p2(price_scale: int = 20, max_quantity: int = 20) -> ExperimentResult:
    """EXP-FEE-P2: per-intent both-sides versus charge-once-split (data row).

    Two grids: the section 5 dust grid (q <= 20 on the price grid), where the
    terminal-ceil floor of one atom per intent makes the two arms indistinguishable,
    and a supra-atom grid where whole atoms clear and the arms separate by the
    documented factor of two.

    Falsifier: charge-once-split collecting more than per-intent-both-sides, the
    taker half rounding below the maker half, either arm breaking payer
    conservation, or the two arms never differing anywhere (which would mean the
    section 2.3 policy fork is not a fork at all).
    """

    witnesses: list[str] = []
    dust_rows: list[dict[str, object]] = []
    supra_rows: list[dict[str, object]] = []
    identical_cells = 0
    differing_cells = 0

    def measure(quantity: int, price: int) -> dict[str, object]:
        nonlocal identical_cells, differing_cells
        fills = (Fill(quantity=quantity, price=price),)
        # PROPOSED variant, explicitly named (P0-5): domain and close policy
        # are held fixed so the two arms differ only in the side arm.
        both = run_fee_schedule(
            fills,
            price_scale,
            KAPPA_NUM,
            KAPPA_DEN,
            domain=CarryDomain.INTENT,
            close_policy=CarryClose.TERMINAL_CEIL,
            side_arm=FeeSideArm.PER_INTENT_BOTH_SIDES,
        )
        once = run_fee_schedule(
            fills,
            price_scale,
            KAPPA_NUM,
            KAPPA_DEN,
            domain=CarryDomain.INTENT,
            close_policy=CarryClose.TERMINAL_CEIL,
            side_arm=FeeSideArm.CHARGE_ONCE_SPLIT,
        )
        consideration = exact_consideration(quantity, price, price_scale)
        both_fee = dict(both.intent_fee)
        once_fee = dict(once.intent_fee)
        if once.fee_pot > both.fee_pot:
            witnesses.append(
                f"charge-once collected {once.fee_pot} > both-sides {both.fee_pot}"
            )
        if once_fee.get("buy-1", 0) < once_fee.get("sell-1", 0):
            witnesses.append("taker-side rounding gave the buyer the smaller half")
        if not once.conserves or not both.conserves:
            witnesses.append("a side arm broke payer conservation")
        if once.fee_pot == both.fee_pot:
            identical_cells += 1
        else:
            differing_cells += 1
        return {
            "quantity": quantity,
            "price": price,
            "consideration": consideration,
            "both_sides_pot": both.fee_pot,
            "both_sides_buy_atoms": both_fee.get("buy-1", 0),
            "both_sides_sell_atoms": both_fee.get("sell-1", 0),
            "charge_once_pot": once.fee_pot,
            "charge_once_buy_atoms": once_fee.get("buy-1", 0),
            "charge_once_sell_atoms": once_fee.get("sell-1", 0),
            "both_sides_bps_of_consideration": (
                f"{both.fee_pot * 10_000}/{consideration}"
                if consideration
                else "undefined"
            ),
            "charge_once_bps_of_consideration": (
                f"{once.fee_pot * 10_000}/{consideration}"
                if consideration
                else "undefined"
            ),
            # PROPOSED variant, explicitly named (P0-5)
            "wash_loss_both_sides": sybil_wash_result(
                fills,
                price_scale,
                KAPPA_NUM,
                KAPPA_DEN,
                domain=CarryDomain.INTENT,
                close_policy=CarryClose.TERMINAL_CEIL,
                side_arm=FeeSideArm.PER_INTENT_BOTH_SIDES,
                maker_num=60,
                executor_num=15,
                denominator=100,
                executor_cap=None,
            )["net_wash"],
            # PROPOSED variant, explicitly named (P0-5)
            "wash_loss_charge_once": sybil_wash_result(
                fills,
                price_scale,
                KAPPA_NUM,
                KAPPA_DEN,
                domain=CarryDomain.INTENT,
                close_policy=CarryClose.TERMINAL_CEIL,
                side_arm=FeeSideArm.CHARGE_ONCE_SPLIT,
                maker_num=60,
                executor_num=15,
                denominator=100,
                executor_cap=None,
            )["net_wash"],
        }

    for quantity in range(1, max_quantity + 1):
        for price in range(0, price_scale + 1):
            if on_grid(quantity, price, price_scale):
                dust_rows.append(measure(quantity, price))
    for quantity in (1_000, 2_000, 4_000, 8_000):
        for price in range(0, price_scale + 1):
            supra_rows.append(measure(quantity, price))
    if not any(
        row["charge_once_pot"] != row["both_sides_pot"] for row in supra_rows
    ):
        witnesses.append("the two fee-side readings never differ at any tested size")
    return ExperimentResult(
        experiment="EXP-FEE-P2",
        claim=(
            "the two fee-side readings of FEE_GEOMETRY section 4 are different "
            "policies above the one-atom floor, and indistinguishable below it"
        ),
        falsifier=(
            "charge-once-split collecting more than per-intent-both-sides, the "
            "taker half rounding below the maker half, either arm breaking payer "
            "conservation, or the arms never differing at any size"
        ),
        bounds={
            "price_scale": price_scale,
            "dust_grid": f"q 1..{max_quantity} on-grid, all prices",
            "supra_atom_grid": "q in {1000, 2000, 4000, 8000}, all prices",
        },
        counts={
            "dust_points": len(dust_rows),
            "supra_atom_points": len(supra_rows),
            "identical_cells": identical_cells,
            "differing_cells": differing_cells,
        },
        data={"dust_rows": dust_rows, "supra_atom_rows": supra_rows},
        witnesses=tuple(witnesses[:8]),
    )


def exp_fee_g1(
    max_outcomes: int = 5, max_scale: int = 12, max_payoff: int = 4
) -> ExperimentResult:
    """EXP-FEE-G1: exact seminorm identities of the dispersion base.

    Falsifier: any identity failing at any enumerated point -- single-Egg
    reduction, complete-set invariance, relabeling symmetry, homogeneity,
    subadditivity, or partition-refinement invariance.
    """

    witnesses: list[str] = []
    counts = {
        "single_egg": 0,
        "translation": 0,
        "relabeling": 0,
        "homogeneity": 0,
        "subadditivity": 0,
        "partition_refinement": 0,
    }
    # (1) single-Egg reduction: exhaustive over n <= max_outcomes and every price
    # composition of S <= max_scale.
    for outcomes in range(2, max_outcomes + 1):
        for scale in range(1, max_scale + 1):
            for prices in _compositions(scale, outcomes):
                for index in range(outcomes):
                    for quantity in range(0, 4):
                        counts["single_egg"] += 1
                        payoffs = tuple(
                            quantity if position == index else 0
                            for position in range(outcomes)
                        )
                        expected = single_egg_dispersion_numerator(
                            quantity, prices[index], scale
                        )
                        if dispersion_numerator(payoffs, prices) != expected:
                            witnesses.append(
                                f"single-Egg reduction failed at {payoffs}/{prices}"
                            )
    # (2)-(4) translation, relabeling, homogeneity.
    for outcomes in range(2, 5):
        scale_cap = 6 if outcomes < 4 else 5
        for scale in range(1, scale_cap + 1):
            for prices in _compositions(scale, outcomes):
                for payoffs in product(range(max_payoff + 1), repeat=outcomes):
                    base = dispersion_numerator(payoffs, prices)
                    counts["translation"] += 1
                    shifted = tuple(value + 3 for value in payoffs)
                    if dispersion_numerator(shifted, prices) != base:
                        witnesses.append(f"translation failed at {payoffs}/{prices}")
                    counts["homogeneity"] += 1
                    scaled = tuple(value * 3 for value in payoffs)
                    if dispersion_numerator(scaled, prices) != 3 * base:
                        witnesses.append(f"homogeneity failed at {payoffs}/{prices}")
                    if outcomes <= 3:
                        for order in permutations(range(outcomes)):
                            counts["relabeling"] += 1
                            if (
                                dispersion_numerator(
                                    tuple(payoffs[index] for index in order),
                                    tuple(prices[index] for index in order),
                                )
                                != base
                            ):
                                witnesses.append(
                                    f"relabeling failed at {payoffs}/{prices}"
                                )
    # (5) subadditivity.
    for outcomes in (2, 3):
        for scale in range(1, 6):
            for prices in _compositions(scale, outcomes):
                for left in product(range(3), repeat=outcomes):
                    for right in product(range(3), repeat=outcomes):
                        counts["subadditivity"] += 1
                        combined = tuple(a + b for a, b in zip(left, right))
                        if dispersion_numerator(combined, prices) > (
                            dispersion_numerator(left, prices)
                            + dispersion_numerator(right, prices)
                        ):
                            witnesses.append(
                                f"subadditivity failed at {left}+{right}/{prices}"
                            )
    # (6) partition refinement: split one outcome into two equal-payoff subcells
    # whose prices sum to the original price.
    for outcomes in (2, 3):
        for scale in range(2, 8):
            for prices in _compositions(scale, outcomes):
                for payoffs in product(range(4), repeat=outcomes):
                    base = dispersion_numerator(payoffs, prices)
                    for index in range(outcomes):
                        for left_price in range(prices[index] + 1):
                            counts["partition_refinement"] += 1
                            refined_prices = (
                                *prices[:index],
                                left_price,
                                prices[index] - left_price,
                                *prices[index + 1 :],
                            )
                            refined_payoffs = (
                                *payoffs[:index],
                                payoffs[index],
                                payoffs[index],
                                *payoffs[index + 1 :],
                            )
                            if (
                                dispersion_numerator(refined_payoffs, refined_prices)
                                != base
                            ):
                                witnesses.append(
                                    f"partition refinement failed at {payoffs}/{prices}"
                                )
    return ExperimentResult(
        experiment="EXP-FEE-G1",
        claim="the dispersion base satisfies its six stated seminorm identities exactly",
        falsifier="any identity failing at any enumerated point",
        bounds={
            "single_egg": f"n<= {max_outcomes}, S<= {max_scale}, q<=3",
            "translation_homogeneity": "n<=4, S<=6 (n<4) / S<=5 (n=4), payoffs<=4",
            "relabeling": "n<=3, S<=6, payoffs<=4",
            "subadditivity": "n<=3, S<=5, payoffs<=2 on both operands",
            "partition_refinement": "n<=3, S<=7, payoffs<=3, every price split",
        },
        counts=counts,
        witnesses=tuple(witnesses[:8]),
    )


def exp_fee_g2() -> ExperimentResult:
    """EXP-FEE-G2: width proposal for the frozen fee constants.

    Falsifier: the closed-form maximum is not the true maximum on the verified
    small cells, or overflow is reachable inside the proposed bounds.
    """

    witnesses: list[str] = []
    # Verify the closed form max_a,p G_num = A_max * floor(S/2) * ceil(S/2)
    # exhaustively on small cells before using it at the proposed bounds.
    verified_cells = 0
    for outcomes in range(2, 5):
        for scale in range(1, 9):
            for amplitude in range(1, 4):
                best = 0
                for prices in _compositions(scale, outcomes):
                    for payoffs in product(range(amplitude + 1), repeat=outcomes):
                        best = max(best, dispersion_numerator(payoffs, prices))
                closed_form = amplitude * (scale // 2) * ((scale + 1) // 2)
                verified_cells += 1
                if best != closed_form:
                    witnesses.append(
                        f"closed form {closed_form} != exhaustive max {best} at "
                        f"n={outcomes} S={scale} A={amplitude}"
                    )
    rows: list[dict[str, object]] = []
    proposals = (
        {"price_scale": 10_000, "amplitude": 2**32 - 1, "max_lots": 2**40, "kappa_num": 4},
        {"price_scale": 10_000, "amplitude": 2**40, "max_lots": 2**32, "kappa_num": 4},
        {"price_scale": 65_536, "amplitude": 2**32, "max_lots": 2**32, "kappa_num": 4},
    )
    limit = 2**128
    for proposal in proposals:
        scale = proposal["price_scale"]
        max_dispersion = proposal["amplitude"] * (scale // 2) * ((scale + 1) // 2)
        max_fee_num = fee_numerator(
            proposal["max_lots"], max_dispersion, proposal["kappa_num"]
        )
        denominator = fee_denominator(KAPPA_DEN, scale)
        headroom_bits = 0
        value = max_fee_num
        while value < limit:
            value *= 2
            headroom_bits += 1
        rows.append(
            {
                **proposal,
                "kappa_den": KAPPA_DEN,
                "denominator": denominator,
                "max_fee_numerator": max_fee_num,
                "fits_u128": max_fee_num < limit,
                "u128_headroom_bits": headroom_bits,
            }
        )
        if max_fee_num >= limit:
            witnesses.append(f"proposal overflows u128: {proposal}")
    return ExperimentResult(
        experiment="EXP-FEE-G2",
        claim=(
            "the exact fee numerator maximum is kappa_num * q_max * A_max * "
            "floor(S/2) * ceil(S/2), and the proposed frozen bounds keep it inside u128"
        ),
        falsifier=(
            "the closed form is not the exhaustive maximum on the verified cells, "
            "or any proposed bound set reaches u128 overflow"
        ),
        bounds={
            "verified_cells": "n<=4, S<=8, A<=3 exhaustive",
            "proposals": "3 candidate frozen-bound sets",
        },
        counts={"verified_cells": verified_cells, "proposals": len(rows)},
        data={"rows": rows},
        witnesses=tuple(witnesses[:8]),
    )


def exp_fee_w1(price_scale: int = 100, price: int = 50, max_fills: int = 24) -> ExperimentResult:
    """EXP-FEE-W1: self-wash sign across the whole policy matrix.

    Falsifier: any cell with positive wash profit, any Sybil recovery above the
    maker plus executor allocation, or any terminal-ceil cell whose net wash is
    not strictly negative.
    """

    witnesses: list[str] = []
    rows: list[dict[str, object]] = []
    zero_fee_cells = 0
    for domain in CarryDomain:
        for close_policy in CarryClose:
            for side_arm in FeeSideArm:
                for executor_cap in (0, 1, None):
                    for count in range(1, max_fills + 1):
                        fills = _dust_fills(
                            count,
                            2,
                            price,
                            intents=count,
                            positions=count,
                            epochs=count,
                        )
                        result = sybil_wash_result(
                            fills,
                            price_scale,
                            KAPPA_NUM,
                            KAPPA_DEN,
                            domain=domain,
                            close_policy=close_policy,
                            side_arm=side_arm,
                            # PROPOSED variant, explicitly named (P0-5)
                            maker_num=60,
                            executor_num=15,
                            denominator=100,
                            executor_cap=executor_cap,
                        )
                        if result["net_wash"] > 0:
                            witnesses.append(
                                f"positive wash in {domain.value}/{close_policy.value}"
                            )
                        if result["recovered"] * 100 > result["fee_pot"] * 75:
                            witnesses.append("Sybil recovered more than 75% of its fee")
                        if close_policy is CarryClose.TERMINAL_CEIL:
                            if result["net_wash"] >= 0:
                                witnesses.append(
                                    "terminal-ceil cell did not lose money: "
                                    f"{domain.value}/{side_arm.value}"
                                )
                        if result["fee_pot"] == 0:
                            zero_fee_cells += 1
                        if count == max_fills:
                            rows.append(
                                {
                                    "carry_domain": domain.value,
                                    "close_policy": close_policy.value,
                                    "fee_side_arm": side_arm.value,
                                    "executor_cap": (
                                        "uncapped" if executor_cap is None else executor_cap
                                    ),
                                    **{
                                        key: value
                                        for key, value in sorted(result.items())
                                    },
                                }
                            )
    return ExperimentResult(
        experiment="EXP-FEE-W1",
        claim=(
            "no configuration of carry domain, close policy, fee side, and "
            "executor cap gives a self-washer a nonnegative return"
        ),
        falsifier=(
            "any cell with positive wash, recovery above maker+executor, or a "
            "terminal-ceil cell that is not strictly negative"
        ),
        bounds={
            "fills": f"1..{max_fills}",
            "cells": "3 domains x 2 close policies x 2 side arms x 3 executor caps",
            "price_scale": price_scale,
        },
        counts={"cells": len(rows), "zero_fee_cells": zero_fee_cells},
        data={"rows": rows},
        witnesses=tuple(witnesses[:8]),
    )


def exp_fee_a1(max_pot: int = 10_000) -> ExperimentResult:
    """EXP-FEE-A1: allocation exactness with an executor cap.

    Falsifier: any pot where maker + executor + treasury != pot, the executor
    exceeds its cap, or treasury falls below 25% of the pot.
    """

    witnesses: list[str] = []
    checked = 0
    for pot in range(0, max_pot + 1):
        for cap in (0, 1, pot // 10, None):
            checked += 1
            # PROPOSED variant, explicitly named (P0-5)
            allocation = allocate_fee(
                pot,
                maker_num=60,
                executor_num=15,
                denominator=100,
                executor_cap=cap,
            )
            if allocation.total != pot:
                witnesses.append(f"allocation lost an atom at pot={pot} cap={cap}")
            if cap is not None and allocation.executor > cap:
                witnesses.append(f"executor exceeded cap at pot={pot} cap={cap}")
            if allocation.executor > pot * 15 // 100:
                witnesses.append(f"executor exceeded its share at pot={pot}")
            if allocation.treasury * 100 < pot * 25:
                witnesses.append(f"treasury floor broken at pot={pot} cap={cap}")
    return ExperimentResult(
        experiment="EXP-FEE-A1",
        claim="allocation conserves every atom, respects the executor cap, and floors treasury at 25%",
        falsifier="any lost atom, cap violation, or treasury below the floor",
        bounds={"pot": f"0..{max_pot}", "executor_cap": "{0, 1, pot/10, uncapped}"},
        counts={"allocations": checked},
        witnesses=tuple(witnesses[:8]),
    )


LOT_EXPERIMENTS: tuple[Callable[[], ExperimentResult], ...] = (
    exp_lot_a1,
    exp_lot_b1,
    exp_lot_b2,
    exp_lot_b3,
    exp_lot_b4,
    exp_lot_b5,
    exp_lot_c1,
    exp_lot_c2,
    exp_lot_x1,
    exp_lot_x2,
)

FEE_EXPERIMENTS: tuple[Callable[[], ExperimentResult], ...] = (
    exp_fee_d1,
    exp_fee_d2,
    exp_fee_p1,
    exp_fee_p2,
    exp_fee_g1,
    exp_fee_g2,
    exp_fee_w1,
    exp_fee_a1,
)

ALL_EXPERIMENTS = LOT_EXPERIMENTS + FEE_EXPERIMENTS
