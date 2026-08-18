# SPDX-License-Identifier: AGPL-3.0-or-later
"""Language-neutral differential fixtures (POLICY_ANALYSIS_LOTS_FEES.md 3.4).

The three fixture families below are authored by hand from the policy analysis,
not generated from the model: the expectations are the contract, and the lab is
one of the two sides that must satisfy them.  A future Rust consumer in
``clutch-kernel`` / ``vertical-model`` replays the same files.

Rules carried from section 3.4: exact integers only, every vector names its
policy arm, a fixture that fails on either side is a finding rather than a
fixture to edit, and minimized failures become permanent named vectors.

Serialization is deterministic: UTF-8, two-space indent, sorted keys, trailing
newline, no timestamps, no host paths, no randomness.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Mapping, Optional, Sequence

from model import (
    ERROR_CLASSES,
    LAB_MAX_OUTCOMES,
    LAB_MAX_PAYOUTS,
    LAB_MIN_OUTCOMES,
    CarryClose,
    CarryDomain,
    FeeSideArm,
    Fill,
    IntegerPayoutSet,
    IntegerPayoutVector,
    KernelRefusal,
    PayoutPolicy,
    WeightedBook,
    allocate_fee,
    dispersion_numerator,
    fee_denominator,
    fee_numerator,
    run_fee_schedule,
)

FIXTURE_DIRNAME = "economics"

ADMISSION_SCHEMA = "dragons-clutch/economics/admission-vectors/v1"
TRACE_SCHEMA = "dragons-clutch/economics/trace-vectors/v1"
FEE_SCHEMA = "dragons-clutch/economics/fee-vectors/v1"

STATUS = (
    "PROPOSED model vectors from docs/implementation/POLICY_ANALYSIS_LOTS_FEES.md; "
    "no protocol constant, fee parameter, or payout candidate is promoted here"
)

POLICY_ARMS = tuple(policy.value for policy in PayoutPolicy)


def _bounds() -> dict[str, object]:
    return {
        "amount_max": "2**64 - 1",
        "max_outcomes": LAB_MAX_OUTCOMES,
        "max_payouts": LAB_MAX_PAYOUTS,
        "min_outcomes": LAB_MIN_OUTCOMES,
    }


# ---------------------------------------------------------------------------
# 1. Admission vectors (P-SOLV-01 support)
# ---------------------------------------------------------------------------


def _admit() -> dict[str, str]:
    return {"result": "admit"}


def _refuse(error_class: str) -> dict[str, str]:
    return {"error_class": error_class, "result": "refuse"}


ADMISSION_VECTORS: tuple[dict[str, object], ...] = (
    {
        "id": "ADM-001",
        "name": "binary-one-hot",
        "note": "the degenerate case: one-hot vectors, unit lots, admitted by every arm",
        "outcomes": 2,
        "count": 2,
        "payout_vectors": [
            {"denominator": 1, "weights": [1, 0]},
            {"denominator": 1, "weights": [0, 1]},
        ],
        "derived_lots": {"redemption_lots": [1, 1], "split_lot": 1},
        "expected": {
            "kernel_baseline": _admit(),
            "one_hot": _admit(),
            "lots": _admit(),
            "credit": _admit(),
        },
    },
    {
        "id": "ADM-002",
        "name": "p1a-half-half",
        "note": "the section 1.1 trap set; candidate (a) refuses it at admission",
        "outcomes": 2,
        "count": 1,
        "payout_vectors": [{"denominator": 2, "weights": [1, 1]}],
        "derived_lots": {"redemption_lots": [2, 2], "split_lot": 2},
        "expected": {
            "kernel_baseline": _admit(),
            "one_hot": _refuse("invalid_payout_weights"),
            "lots": _admit(),
            "credit": _admit(),
        },
    },
    {
        "id": "ADM-003",
        "name": "weights-sum-below-denominator",
        "note": "sum < D: the lab used to admit this, the kernel never did (section 3.1)",
        "outcomes": 2,
        "count": 1,
        "payout_vectors": [{"denominator": 2, "weights": [1, 0]}],
        "expected": {
            "kernel_baseline": _refuse("invalid_payout_weights"),
            "one_hot": _refuse("invalid_payout_weights"),
            "lots": _refuse("invalid_payout_weights"),
            "credit": _refuse("invalid_payout_weights"),
        },
    },
    {
        "id": "ADM-004",
        "name": "weights-sum-above-denominator",
        "outcomes": 2,
        "count": 1,
        "payout_vectors": [{"denominator": 2, "weights": [2, 1]}],
        "expected": {
            "kernel_baseline": _refuse("invalid_payout_weights"),
            "one_hot": _refuse("invalid_payout_weights"),
            "lots": _refuse("invalid_payout_weights"),
            "credit": _refuse("invalid_payout_weights"),
        },
    },
    {
        "id": "ADM-005",
        "name": "single-weight-above-denominator",
        "outcomes": 2,
        "count": 1,
        "payout_vectors": [{"denominator": 2, "weights": [3, 0]}],
        "expected": {
            "kernel_baseline": _refuse("invalid_payout_weights"),
            "one_hot": _refuse("invalid_payout_weights"),
            "lots": _refuse("invalid_payout_weights"),
            "credit": _refuse("invalid_payout_weights"),
        },
    },
    {
        "id": "ADM-006",
        "name": "mixed-denominators",
        "note": "the kernel requires one common D across the whole set",
        "outcomes": 2,
        "count": 2,
        "payout_vectors": [
            {"denominator": 1, "weights": [1, 0]},
            {"denominator": 2, "weights": [1, 1]},
        ],
        "expected": {
            "kernel_baseline": _refuse("invalid_denominator"),
            "one_hot": _refuse("invalid_denominator"),
            "lots": _refuse("invalid_denominator"),
            "credit": _refuse("invalid_denominator"),
        },
    },
    {
        "id": "ADM-007",
        "name": "zero-denominator",
        "outcomes": 2,
        "count": 1,
        "payout_vectors": [{"denominator": 0, "weights": [0, 0]}],
        "expected": {
            "kernel_baseline": _refuse("invalid_denominator"),
            "one_hot": _refuse("invalid_denominator"),
            "lots": _refuse("invalid_denominator"),
            "credit": _refuse("invalid_denominator"),
        },
    },
    {
        "id": "ADM-008",
        "name": "nonzero-weight-padding",
        "note": "weights beyond outcome_count must be zero",
        "outcomes": 2,
        "count": 1,
        "payout_vectors": [{"denominator": 1, "weights": [1, 0, 1]}],
        "expected": {
            "kernel_baseline": _refuse("invalid_payout_weights"),
            "one_hot": _refuse("invalid_payout_weights"),
            "lots": _refuse("invalid_payout_weights"),
            "credit": _refuse("invalid_payout_weights"),
        },
    },
    {
        "id": "ADM-009",
        "name": "nonzero-vector-padding",
        "note": "vectors beyond count must be the zero vector",
        "outcomes": 2,
        "count": 1,
        "payout_vectors": [
            {"denominator": 1, "weights": [1, 0]},
            {"denominator": 1, "weights": [0, 1]},
        ],
        "expected": {
            "kernel_baseline": _refuse("invalid_payout_weights"),
            "one_hot": _refuse("invalid_payout_weights"),
            "lots": _refuse("invalid_payout_weights"),
            "credit": _refuse("invalid_payout_weights"),
        },
    },
    {
        "id": "ADM-010",
        "name": "too-few-outcomes",
        "outcomes": 1,
        "count": 1,
        "payout_vectors": [{"denominator": 1, "weights": [1]}],
        "expected": {
            "kernel_baseline": _refuse("invalid_outcome_count"),
            "one_hot": _refuse("invalid_outcome_count"),
            "lots": _refuse("invalid_outcome_count"),
            "credit": _refuse("invalid_outcome_count"),
        },
    },
    {
        "id": "ADM-011",
        "name": "zero-payout-count",
        "outcomes": 2,
        "count": 0,
        "payout_vectors": [{"denominator": 1, "weights": [1, 0]}],
        "expected": {
            "kernel_baseline": _refuse("invalid_payout_count"),
            "one_hot": _refuse("invalid_payout_count"),
            "lots": _refuse("invalid_payout_count"),
            "credit": _refuse("invalid_payout_count"),
        },
    },
    {
        "id": "ADM-012",
        "name": "payout-count-above-maximum",
        "outcomes": 2,
        "count": 9,
        "payout_vectors": [{"denominator": 1, "weights": [1, 0]}] * 9,
        "expected": {
            "kernel_baseline": _refuse("invalid_payout_count"),
            "one_hot": _refuse("invalid_payout_count"),
            "lots": _refuse("invalid_payout_count"),
            "credit": _refuse("invalid_payout_count"),
        },
    },
    {
        "id": "ADM-013",
        "name": "equal-weight-compatible-fallback",
        "note": "the ECONOMICS.md section 7 fallback family; refused only by candidate (a)",
        "outcomes": 4,
        "count": 2,
        "payout_vectors": [
            {"denominator": 4, "weights": [2, 2, 0, 0]},
            {"denominator": 4, "weights": [1, 1, 1, 1]},
        ],
        "derived_lots": {"redemption_lots": [4, 4, 4, 4], "split_lot": 4},
        "expected": {
            "kernel_baseline": _admit(),
            "one_hot": _refuse("invalid_payout_weights"),
            "lots": _admit(),
            "credit": _admit(),
        },
    },
    {
        "id": "ADM-014",
        "name": "ternary-one-hot-scaled-denominator",
        "note": "one-hot is a property of the weights relative to D, not of D itself",
        "outcomes": 3,
        "count": 3,
        "payout_vectors": [
            {"denominator": 3, "weights": [3, 0, 0]},
            {"denominator": 3, "weights": [0, 3, 0]},
            {"denominator": 3, "weights": [0, 0, 3]},
        ],
        "derived_lots": {"redemption_lots": [1, 1, 1], "split_lot": 1},
        "expected": {
            "kernel_baseline": _admit(),
            "one_hot": _admit(),
            "lots": _admit(),
            "credit": _admit(),
        },
    },
    {
        "id": "ADM-015",
        "name": "mixed-one-hot-and-fractional",
        "note": "a set is one-hot only if every vector is",
        "outcomes": 2,
        "count": 2,
        "payout_vectors": [
            {"denominator": 2, "weights": [2, 0]},
            {"denominator": 2, "weights": [1, 1]},
        ],
        "derived_lots": {"redemption_lots": [2, 2], "split_lot": 2},
        "expected": {
            "kernel_baseline": _admit(),
            "one_hot": _refuse("invalid_payout_weights"),
            "lots": _admit(),
            "credit": _admit(),
        },
    },
)


def build_admission_fixture() -> dict[str, object]:
    return {
        "bounds": _bounds(),
        "error_classes": list(ERROR_CLASSES),
        "experiment": "EXP-ALIGN-01",
        "policy_arms": list(POLICY_ARMS),
        "schema": ADMISSION_SCHEMA,
        "source": "docs/implementation/POLICY_ANALYSIS_LOTS_FEES.md sections 3.1, 3.4",
        "status": STATUS,
        "vectors": list(ADMISSION_VECTORS),
    }


# ---------------------------------------------------------------------------
# 2. Trace vectors (P-SOLV-01)
# ---------------------------------------------------------------------------


def _ok(payout: Optional[int] = None) -> dict[str, object]:
    step: dict[str, object] = {"result": "ok"}
    if payout is not None:
        step["payout"] = payout
    return step


def _step_refuse(error_class: str) -> dict[str, object]:
    return {"error_class": error_class, "result": "refuse"}


def _final(
    collateral: int,
    total_supply: Sequence[int],
    positions: Sequence[Mapping[str, object]],
    credit_total: int = 0,
) -> dict[str, object]:
    return {
        "collateral": collateral,
        "credit_total": credit_total,
        "positions": [dict(position) for position in positions],
        "total_supply": list(total_supply),
    }


def _position(
    internal: Sequence[int], external: Sequence[int], credit: int = 0
) -> dict[str, object]:
    return {"credit": credit, "external": list(external), "internal": list(internal)}


TRACE_VECTORS: tuple[dict[str, object], ...] = (
    {
        "id": "TRC-001",
        "name": "p1a-exit-dead-complete-set",
        "note": (
            "POLICY_ANALYSIS section 1.1: weights [1,1] over D=2, split one atom, "
            "resolve. Both per-outcome redemptions refuse forever under the landed "
            "kernel; the section 1.5 complete-set exit clears it, candidate (a) "
            "refuses the market, candidate (b) refuses the split, candidate (c) "
            "pays a floor and a credit"
        ),
        "market": {
            "outcomes": 2,
            "count": 1,
            "payout_vectors": [{"denominator": 2, "weights": [1, 1]}],
            "initial_collateral": 0,
            "wallets": 1,
        },
        "steps": [
            {"op": "split", "wallet": 0, "quantity": 1},
            {"op": "resolve", "payout_index": 0},
            {"op": "redeem_internal", "wallet": 0, "outcome": 0, "quantity": 1},
            {"op": "redeem_internal", "wallet": 0, "outcome": 1, "quantity": 1},
            {"op": "claim_credit", "wallet": 0},
            {"op": "redeem_complete_set", "wallet": 0, "quantity": 1, "internal": True},
        ],
        "arms": {
            "kernel_baseline": {
                "admitted": True,
                "results": [
                    _ok(),
                    _ok(),
                    _step_refuse("remainder_required"),
                    _step_refuse("remainder_required"),
                    _step_refuse("no_credit"),
                    _ok(1),
                ],
                "final": _final(0, [0, 0], [_position([0, 0], [0, 0])]),
            },
            "one_hot": {
                "admitted": False,
                "error_class": "invalid_payout_weights",
            },
            "lots": {
                "admitted": True,
                "results": [
                    _step_refuse("lot_violation"),
                    _ok(),
                    _step_refuse("insufficient_balance"),
                    _step_refuse("insufficient_balance"),
                    _step_refuse("no_credit"),
                    _step_refuse("lot_violation"),
                ],
                "final": _final(0, [0, 0], [_position([0, 0], [0, 0])]),
            },
            "credit": {
                "admitted": True,
                "results": [
                    _ok(),
                    _ok(),
                    _ok(0),
                    _ok(0),
                    _ok(1),
                    _step_refuse("insufficient_balance"),
                ],
                "final": _final(0, [0, 0], [_position([0, 0], [0, 0], 0)]),
            },
        },
    },
    {
        "id": "TRC-002",
        "name": "one-hot-full-lifecycle",
        "note": "every arm must agree exactly on the degenerate one-hot lifecycle",
        "market": {
            "outcomes": 2,
            "count": 2,
            "payout_vectors": [
                {"denominator": 1, "weights": [1, 0]},
                {"denominator": 1, "weights": [0, 1]},
            ],
            "initial_collateral": 0,
            "wallets": 1,
        },
        "steps": [
            {"op": "split", "wallet": 0, "quantity": 5},
            {"op": "materialize", "wallet": 0, "outcome": 1, "quantity": 2},
            {"op": "dematerialize", "wallet": 0, "outcome": 1, "quantity": 1},
            {"op": "resolve", "payout_index": 1},
            {"op": "redeem_internal", "wallet": 0, "outcome": 0, "quantity": 5},
            {"op": "redeem_internal", "wallet": 0, "outcome": 1, "quantity": 4},
            {"op": "redeem_external", "wallet": 0, "outcome": 1, "quantity": 1},
            {"op": "claim_credit", "wallet": 0},
            {"op": "redeem_complete_set", "wallet": 0, "quantity": 1, "internal": True},
        ],
        "arms": {
            arm: {
                "admitted": True,
                "results": [
                    _ok(),
                    _ok(),
                    _ok(),
                    _ok(),
                    _ok(0),
                    _ok(4),
                    _ok(1),
                    _step_refuse("no_credit"),
                    _step_refuse("insufficient_balance"),
                ],
                "final": _final(0, [0, 0], [_position([0, 0], [0, 0])]),
            }
            for arm in POLICY_ARMS
        },
    },
    {
        "id": "TRC-003",
        "name": "p1a-materialize-strands-the-set",
        "note": (
            "moving one leg of the complete set across the Token-2022 boundary "
            "removes the section 1.5 exit: internal and external sides can no "
            "longer form a set, so the baseline arm is exit-dead with liability "
            "outstanding"
        ),
        "market": {
            "outcomes": 2,
            "count": 1,
            "payout_vectors": [{"denominator": 2, "weights": [1, 1]}],
            "initial_collateral": 0,
            "wallets": 1,
        },
        "steps": [
            {"op": "split", "wallet": 0, "quantity": 1},
            {"op": "materialize", "wallet": 0, "outcome": 0, "quantity": 1},
            {"op": "resolve", "payout_index": 0},
            {"op": "redeem_external", "wallet": 0, "outcome": 0, "quantity": 1},
            {"op": "redeem_internal", "wallet": 0, "outcome": 1, "quantity": 1},
            {"op": "redeem_complete_set", "wallet": 0, "quantity": 1, "internal": True},
            {"op": "redeem_complete_set", "wallet": 0, "quantity": 1, "internal": False},
            {"op": "claim_credit", "wallet": 0},
        ],
        "arms": {
            "kernel_baseline": {
                "admitted": True,
                "results": [
                    _ok(),
                    _ok(),
                    _ok(),
                    _step_refuse("remainder_required"),
                    _step_refuse("remainder_required"),
                    _step_refuse("insufficient_balance"),
                    _step_refuse("insufficient_balance"),
                    _step_refuse("no_credit"),
                ],
                "final": _final(1, [1, 1], [_position([0, 1], [1, 0])]),
                "exit_dead": True,
            },
            "one_hot": {
                "admitted": False,
                "error_class": "invalid_payout_weights",
            },
            "lots": {
                "admitted": True,
                "results": [
                    _step_refuse("lot_violation"),
                    _step_refuse("lot_violation"),
                    _ok(),
                    _step_refuse("insufficient_balance"),
                    _step_refuse("insufficient_balance"),
                    _step_refuse("lot_violation"),
                    _step_refuse("lot_violation"),
                    _step_refuse("no_credit"),
                ],
                "final": _final(0, [0, 0], [_position([0, 0], [0, 0])]),
                "exit_dead": False,
            },
            "credit": {
                "admitted": True,
                "results": [
                    _ok(),
                    _ok(),
                    _ok(),
                    _ok(0),
                    _ok(0),
                    _step_refuse("insufficient_balance"),
                    _step_refuse("insufficient_balance"),
                    _ok(1),
                ],
                "final": _final(0, [0, 0], [_position([0, 0], [0, 0], 0)]),
                "exit_dead": False,
            },
        },
    },
    {
        "id": "TRC-004",
        "name": "lot-gated-two-wallet-fragmentation",
        "note": (
            "candidate (b1): internal transitions are lot-gated, the bearer "
            "transfer is not, so wallet 1 ends holding a sub-lot fragment that "
            "refuses redemption while the aggregate stays lot-aligned"
        ),
        "market": {
            "outcomes": 2,
            "count": 2,
            "payout_vectors": [
                {"denominator": 4, "weights": [2, 2]},
                {"denominator": 4, "weights": [4, 0]},
            ],
            "initial_collateral": 0,
            "wallets": 2,
        },
        "steps": [
            {"op": "split", "wallet": 0, "quantity": 2},
            {"op": "materialize", "wallet": 0, "outcome": 0, "quantity": 1},
            {"op": "materialize", "wallet": 0, "outcome": 0, "quantity": 2},
            {
                "op": "transfer_external",
                "wallet": 0,
                "destination": 1,
                "outcome": 0,
                "quantity": 1,
            },
            {"op": "resolve", "payout_index": 0},
            {"op": "redeem_external", "wallet": 1, "outcome": 0, "quantity": 1},
            {"op": "redeem_external", "wallet": 0, "outcome": 0, "quantity": 1},
            {"op": "redeem_internal", "wallet": 0, "outcome": 1, "quantity": 2},
            {"op": "redeem_complete_set", "wallet": 0, "quantity": 1, "internal": True},
        ],
        "arms": {
            "kernel_baseline": {
                "admitted": True,
                "results": [
                    _ok(),
                    _ok(),
                    _step_refuse("insufficient_balance"),
                    _ok(),
                    _ok(),
                    _step_refuse("remainder_required"),
                    _step_refuse("insufficient_balance"),
                    _ok(1),
                    _step_refuse("insufficient_balance"),
                ],
                "final": _final(
                    1,
                    [2, 0],
                    [_position([1, 0], [0, 0]), _position([0, 0], [1, 0])],
                ),
            },
            "one_hot": {
                "admitted": False,
                "error_class": "invalid_payout_weights",
            },
            "lots": {
                "admitted": True,
                "results": [
                    _ok(),
                    _step_refuse("lot_violation"),
                    _ok(),
                    _ok(),
                    _ok(),
                    _step_refuse("remainder_required"),
                    _step_refuse("remainder_required"),
                    _ok(1),
                    _step_refuse("lot_violation"),
                ],
                "final": _final(
                    1,
                    [2, 0],
                    [_position([0, 0], [1, 0]), _position([0, 0], [1, 0])],
                ),
            },
            "credit": {
                "admitted": True,
                "results": [
                    _ok(),
                    _ok(),
                    _step_refuse("insufficient_balance"),
                    _ok(),
                    _ok(),
                    _ok(0),
                    _step_refuse("insufficient_balance"),
                    _ok(1),
                    _step_refuse("insufficient_balance"),
                ],
                "final": _final(
                    1,
                    [1, 0],
                    [_position([1, 0], [0, 0]), _position([0, 0], [0, 0], 2)],
                    credit_total=2,
                ),
            },
        },
    },
    {
        "id": "TRC-005",
        "name": "refusal-vocabulary",
        "note": "one vector per shared error class that a one-hot market can reach",
        "market": {
            "outcomes": 2,
            "count": 2,
            "payout_vectors": [
                {"denominator": 1, "weights": [1, 0]},
                {"denominator": 1, "weights": [0, 1]},
            ],
            "initial_collateral": 0,
            "wallets": 1,
        },
        "steps": [
            {"op": "redeem_internal", "wallet": 0, "outcome": 0, "quantity": 1},
            {"op": "split", "wallet": 0, "quantity": 0},
            {"op": "split", "wallet": 0, "quantity": 3},
            {"op": "merge", "wallet": 0, "quantity": 4},
            {"op": "materialize", "wallet": 0, "outcome": 5, "quantity": 1},
            {"op": "resolve", "payout_index": 5},
            {"op": "resolve", "payout_index": 0},
            {"op": "resolve", "payout_index": 1},
            {"op": "merge", "wallet": 0, "quantity": 1},
            {"op": "redeem_internal", "wallet": 0, "outcome": 0, "quantity": 4},
            {"op": "redeem_internal", "wallet": 0, "outcome": 0, "quantity": 3},
        ],
        "arms": {
            arm: {
                "admitted": True,
                "results": [
                    _step_refuse("not_resolved"),
                    _step_refuse("zero_quantity"),
                    _ok(),
                    _step_refuse("insufficient_collateral"),
                    _step_refuse("invalid_payout_index"),
                    _step_refuse("invalid_payout_index"),
                    _ok(),
                    _step_refuse("already_resolved"),
                    _step_refuse("already_resolved"),
                    _step_refuse("insufficient_balance"),
                    _ok(3),
                ],
                "final": _final(0, [0, 3], [_position([0, 3], [0, 0])]),
            }
            for arm in POLICY_ARMS
        },
    },
    {
        "id": "TRC-006",
        "name": "ternary-equal-weight-residue",
        "note": (
            "equal-weight fallback over three outcomes with D=3: two atoms "
            "remainder under the baseline, the complete-set exit clears them, and "
            "candidate (c) leaves a sub-atom residue that cannot yet be claimed"
        ),
        "market": {
            "outcomes": 3,
            "count": 1,
            "payout_vectors": [{"denominator": 3, "weights": [1, 1, 1]}],
            "initial_collateral": 0,
            "wallets": 1,
        },
        "steps": [
            {"op": "split", "wallet": 0, "quantity": 2},
            {"op": "resolve", "payout_index": 0},
            {"op": "redeem_internal", "wallet": 0, "outcome": 0, "quantity": 2},
            {"op": "redeem_complete_set", "wallet": 0, "quantity": 2, "internal": True},
            {"op": "claim_credit", "wallet": 0},
        ],
        "arms": {
            "kernel_baseline": {
                "admitted": True,
                "results": [
                    _ok(),
                    _ok(),
                    _step_refuse("remainder_required"),
                    _ok(2),
                    _step_refuse("no_credit"),
                ],
                "final": _final(0, [0, 0, 0], [_position([0, 0, 0], [0, 0, 0])]),
            },
            "one_hot": {
                "admitted": False,
                "error_class": "invalid_payout_weights",
            },
            "lots": {
                "admitted": True,
                "results": [
                    _step_refuse("lot_violation"),
                    _ok(),
                    _step_refuse("insufficient_balance"),
                    _step_refuse("lot_violation"),
                    _step_refuse("no_credit"),
                ],
                "final": _final(0, [0, 0, 0], [_position([0, 0, 0], [0, 0, 0])]),
            },
            "credit": {
                "admitted": True,
                "results": [
                    _ok(),
                    _ok(),
                    _ok(0),
                    _step_refuse("insufficient_balance"),
                    _step_refuse("no_credit"),
                ],
                "final": _final(
                    2,
                    [0, 2, 2],
                    [_position([0, 2, 2], [0, 0, 0], 2)],
                    credit_total=2,
                ),
            },
        },
    },
)


#: Which side of the design each replayable operation sits on, so a consumer
#: that implements only the landed kernel knows exactly which steps it can run.
#: ``redeem_complete_set`` is split by side: the kernel landed the internal
#: (section 1.5) transition in commit d60ccf3, while the external variant is a
#: lab extension used to show that materialization can strand a complete set.
TRANSITION_CLASS = {
    "split": "landed_kernel",
    "merge": "landed_kernel",
    "materialize": "landed_kernel",
    "dematerialize": "landed_kernel",
    "resolve": "landed_kernel",
    "redeem_internal": "landed_kernel",
    "redeem_external": "landed_kernel",
    "transfer_external": "external_adapter",
    "claim_credit": "proposed_candidate_c",
    "redeem_complete_set": "landed_kernel",
}

TRANSITION_CLASSES = (
    "landed_kernel",
    "external_adapter",
    "lab_extension",
    "proposed_candidate_c",
)


def _transition_class(step: Mapping[str, object]) -> str:
    operation = str(step["op"])
    if operation == "redeem_complete_set" and not step.get("internal", True):
        return "lab_extension"
    return TRANSITION_CLASS[operation]

#: Policy arms and whether they exist today.
POLICY_ARM_STATUS = {
    "kernel_baseline": "landed: crates/clutch-kernel as committed",
    "one_hot": "PROPOSED candidate (a1)",
    "lots": "PROPOSED candidate (b1)",
    "credit": "PROPOSED candidate (c)",
}


def _annotated_vectors() -> list[dict[str, object]]:
    annotated = []
    for vector in TRACE_VECTORS:
        entry = dict(vector)
        entry["steps"] = [
            {**step, "transition_class": _transition_class(step)}
            for step in vector["steps"]
        ]
        annotated.append(entry)
    return annotated


def build_trace_fixture() -> dict[str, object]:
    return {
        "bounds": _bounds(),
        "error_classes": list(ERROR_CLASSES),
        "experiment": "EXP-ALIGN-02",
        "policy_arm_status": POLICY_ARM_STATUS,
        "replay_contract": (
            "A consumer that implements only landed_kernel transitions replays the "
            "step prefix up to the first proposed step expected to succeed; a "
            "refused step never changes state, so refusal expectations before that "
            "point are binding for every consumer"
        ),
        "operations": [
            "split",
            "merge",
            "materialize",
            "dematerialize",
            "transfer_external",
            "resolve",
            "redeem_internal",
            "redeem_external",
            "claim_credit",
            "redeem_complete_set",
        ],
        "policy_arms": list(POLICY_ARMS),
        "schema": TRACE_SCHEMA,
        "source": "docs/implementation/POLICY_ANALYSIS_LOTS_FEES.md sections 1, 3.2, 3.4",
        "status": STATUS,
        "vectors": _annotated_vectors(),
    }


# ---------------------------------------------------------------------------
# 3. Fee vectors (P-FEE-01)
# ---------------------------------------------------------------------------

#: Experimental fee parameters carried by the vectors below.  `kappa = 4/1000`
#: and the 60/15/25 allocation are arms under test, never promoted constants.
FEE_KAPPA_NUM = 4
FEE_KAPPA_DEN = 1_000


def _fill(
    quantity: int,
    price: int,
    buyer_intent: str = "buy-1",
    seller_intent: str = "sell-1",
    buyer_position: str = "buyer-1",
    seller_position: str = "seller-1",
    epoch: int = 0,
) -> dict[str, object]:
    return {
        "buyer_intent": buyer_intent,
        "buyer_position": buyer_position,
        "epoch": epoch,
        "price": price,
        "quantity": quantity,
        "seller_intent": seller_intent,
        "seller_position": seller_position,
    }


FEE_VECTORS: tuple[dict[str, object], ...] = (
    {
        "id": "FEE-001",
        "name": "dust-fill-terminal-ceil-both-sides",
        "kind": "single_egg_schedule",
        "note": (
            "one dust fill: the exact fee is 40000/10000000 of an atom on each "
            "side, so both sides pay zero on the fill and one atom at intent "
            "close -- the structural anti-dust floor of section 2.2"
        ),
        "derivation": (
            "G=q*p*(S-p)=2*50*50=5000; fee_num=kappa_num*G=20000 per side; "
            "den=kappa_den*S^2=10^7; floor=0, carry=20000; terminal ceil=1 per "
            "intent; C=q*p/S=1"
        ),
        "price_scale": 100,
        "kappa_num": FEE_KAPPA_NUM,
        "kappa_den": FEE_KAPPA_DEN,
        "carry_domain": "intent",
        "carry_close": "terminal_ceil",
        "fee_side_arm": "per_intent_both_sides",
        "close_events": "every open domain instance closes at the end of the schedule",
        "fills": [_fill(2, 50)],
        "expected": {
            "denominator": 10_000_000,
            "fee_numerator_total": 40_000,
            "fee_pot": 2,
            "terminal_charges": 2,
            "consideration_total": 1,
            "buyer_debit_total": 2,
            "seller_credit_total": 0,
            "hoard_delta": 0,
            "fill_legs": [["buy-1", "buy", 0, 20_000], ["sell-1", "sell", 0, 20_000]],
            "domain_paid": [["buy-1", 1], ["sell-1", 1]],
            "allocation": {"executor": 0, "executor_cap": None, "maker": 1, "treasury": 1},
            "conservation": {"hoard_untouched": True, "payer_identity": True},
        },
    },
    {
        "id": "FEE-002",
        "name": "four-dust-fills-one-intent",
        "kind": "single_egg_schedule",
        "note": "fragmentation inside one domain instance still pays exactly ceil(exact)",
        "derivation": "4 x 20000 = 80000 numerator per side; floor 0; one terminal atom per side",
        "price_scale": 100,
        "kappa_num": FEE_KAPPA_NUM,
        "kappa_den": FEE_KAPPA_DEN,
        "carry_domain": "intent",
        "carry_close": "terminal_ceil",
        "fee_side_arm": "per_intent_both_sides",
        "close_events": "every open domain instance closes at the end of the schedule",
        "fills": [_fill(2, 50) for _ in range(4)],
        "expected": {
            "denominator": 10_000_000,
            "fee_numerator_total": 160_000,
            "fee_pot": 2,
            "terminal_charges": 2,
            "consideration_total": 4,
            "buyer_debit_total": 5,
            "seller_credit_total": 3,
            "hoard_delta": 0,
            "fill_legs": [
                ["buy-1", "buy", 0, 20_000],
                ["sell-1", "sell", 0, 20_000],
                ["buy-1", "buy", 0, 40_000],
                ["sell-1", "sell", 0, 40_000],
                ["buy-1", "buy", 0, 60_000],
                ["sell-1", "sell", 0, 60_000],
                ["buy-1", "buy", 0, 80_000],
                ["sell-1", "sell", 0, 80_000],
            ],
            "domain_paid": [["buy-1", 1], ["sell-1", 1]],
            "allocation": {"executor": 0, "executor_cap": None, "maker": 1, "treasury": 1},
            "conservation": {"hoard_untouched": True, "payer_identity": True},
        },
    },
    {
        "id": "FEE-003",
        "name": "four-dust-fills-four-intents",
        "kind": "single_egg_schedule",
        "note": (
            "the same economic flow split across four signed intents pays four "
            "times as much: the carry-reset attack inverts sign under terminal ceil"
        ),
        "derivation": "8 domain instances, each carrying 20000 < den, each charged one terminal atom",
        "price_scale": 100,
        "kappa_num": FEE_KAPPA_NUM,
        "kappa_den": FEE_KAPPA_DEN,
        "carry_domain": "intent",
        "carry_close": "terminal_ceil",
        "fee_side_arm": "per_intent_both_sides",
        "close_events": "every open domain instance closes at the end of the schedule",
        "fills": [
            _fill(2, 50, buyer_intent=f"buy-{index}", seller_intent=f"sell-{index}")
            for index in range(4)
        ],
        "expected": {
            "denominator": 10_000_000,
            "fee_numerator_total": 160_000,
            "fee_pot": 8,
            "terminal_charges": 8,
            "consideration_total": 4,
            "buyer_debit_total": 8,
            "seller_credit_total": 0,
            "hoard_delta": 0,
            "fill_legs": [
                ["buy-0", "buy", 0, 20_000],
                ["sell-0", "sell", 0, 20_000],
                ["buy-1", "buy", 0, 20_000],
                ["sell-1", "sell", 0, 20_000],
                ["buy-2", "buy", 0, 20_000],
                ["sell-2", "sell", 0, 20_000],
                ["buy-3", "buy", 0, 20_000],
                ["sell-3", "sell", 0, 20_000],
            ],
            "domain_paid": [
                ["buy-0", 1],
                ["buy-1", 1],
                ["buy-2", 1],
                ["buy-3", 1],
                ["sell-0", 1],
                ["sell-1", 1],
                ["sell-2", 1],
                ["sell-3", 1],
            ],
            "allocation": {"executor": 1, "executor_cap": None, "maker": 4, "treasury": 3},
            "conservation": {"hoard_untouched": True, "payer_identity": True},
        },
    },
    {
        "id": "FEE-004",
        "name": "epoch-domain-dropped-carry-evasion",
        "kind": "single_egg_schedule",
        "note": (
            "the Epoch carry domain with a dropped carry collects nothing at all "
            "while volume is positive: the section 2.2 refusal evidence"
        ),
        "derivation": "each (payer, epoch) instance carries 20000 < den and drops it at close",
        "price_scale": 100,
        "kappa_num": FEE_KAPPA_NUM,
        "kappa_den": FEE_KAPPA_DEN,
        "carry_domain": "epoch",
        "carry_close": "dropped_carry",
        "fee_side_arm": "per_intent_both_sides",
        "close_events": "every open domain instance closes at the end of the schedule",
        "fills": [_fill(2, 50, epoch=index) for index in range(4)],
        "expected": {
            "denominator": 10_000_000,
            "fee_numerator_total": 160_000,
            "fee_pot": 0,
            "terminal_charges": 0,
            "consideration_total": 4,
            "buyer_debit_total": 4,
            "seller_credit_total": 4,
            "hoard_delta": 0,
            "fill_legs": [
                ["buy-1", "buy", 0, 20_000],
                ["sell-1", "sell", 0, 20_000],
                ["buy-1", "buy", 0, 20_000],
                ["sell-1", "sell", 0, 20_000],
                ["buy-1", "buy", 0, 20_000],
                ["sell-1", "sell", 0, 20_000],
                ["buy-1", "buy", 0, 20_000],
                ["sell-1", "sell", 0, 20_000],
            ],
            "domain_paid": [
                ["buyer-1@0", 0],
                ["buyer-1@1", 0],
                ["buyer-1@2", 0],
                ["buyer-1@3", 0],
                ["seller-1@0", 0],
                ["seller-1@1", 0],
                ["seller-1@2", 0],
                ["seller-1@3", 0],
            ],
            "allocation": {"executor": 0, "executor_cap": None, "maker": 0, "treasury": 0},
            "conservation": {"hoard_untouched": True, "payer_identity": True},
        },
    },
    {
        "id": "FEE-005",
        "name": "supra-atom-fill-both-sides",
        "kind": "single_egg_schedule",
        "note": "a fill large enough that the fee clears whole atoms without any terminal charge",
        "derivation": (
            "G=2000*50*50=5*10^6; fee_num=2*10^7 per side; den=10^7; floor=2, "
            "carry=0; C=2000*50/100=1000"
        ),
        "price_scale": 100,
        "kappa_num": FEE_KAPPA_NUM,
        "kappa_den": FEE_KAPPA_DEN,
        "carry_domain": "intent",
        "carry_close": "terminal_ceil",
        "fee_side_arm": "per_intent_both_sides",
        "close_events": "every open domain instance closes at the end of the schedule",
        "fills": [_fill(2000, 50)],
        "expected": {
            "denominator": 10_000_000,
            "fee_numerator_total": 40_000_000,
            "fee_pot": 4,
            "terminal_charges": 0,
            "consideration_total": 1000,
            "buyer_debit_total": 1002,
            "seller_credit_total": 998,
            "hoard_delta": 0,
            "fill_legs": [["buy-1", "buy", 2, 0], ["sell-1", "sell", 2, 0]],
            "domain_paid": [["buy-1", 2], ["sell-1", 2]],
            "allocation": {"executor": 0, "executor_cap": None, "maker": 2, "treasury": 2},
            "conservation": {"hoard_untouched": True, "payer_identity": True},
        },
    },
    {
        "id": "FEE-006",
        "name": "supra-atom-fill-charge-once-split",
        "kind": "single_egg_schedule",
        "note": (
            "the same fill under the charge-once-split reading of FEE_GEOMETRY "
            "section 4: half the venue take, half the incidence per side"
        ),
        "derivation": "gross 2*10^7 split ceil/floor into 10^7 + 10^7; each floors to one atom",
        "price_scale": 100,
        "kappa_num": FEE_KAPPA_NUM,
        "kappa_den": FEE_KAPPA_DEN,
        "carry_domain": "intent",
        "carry_close": "terminal_ceil",
        "fee_side_arm": "charge_once_split",
        "close_events": "every open domain instance closes at the end of the schedule",
        "fills": [_fill(2000, 50)],
        "expected": {
            "denominator": 10_000_000,
            "fee_numerator_total": 20_000_000,
            "fee_pot": 2,
            "terminal_charges": 0,
            "consideration_total": 1000,
            "buyer_debit_total": 1001,
            "seller_credit_total": 999,
            "hoard_delta": 0,
            "fill_legs": [["buy-1", "buy", 1, 0], ["sell-1", "sell", 1, 0]],
            "domain_paid": [["buy-1", 1], ["sell-1", 1]],
            "allocation": {"executor": 0, "executor_cap": None, "maker": 1, "treasury": 1},
            "conservation": {"hoard_untouched": True, "payer_identity": True},
        },
    },
    {
        "id": "FEE-007",
        "name": "multi-outcome-dispersion-point",
        "kind": "dispersion_point",
        "note": "the general G_num base of section 2.4 on a three-outcome vector",
        "derivation": (
            "G=2*3*|3-0| + 2*5*|3-1| + 3*5*|0-1| = 18+20+15 = 53; "
            "fee_num=kappa_num*q*G=4*1*53=212; den=kappa_den*S^2=10^5; "
            "floor=0, carry=212, terminal ceil=1"
        ),
        "price_scale": 10,
        "kappa_num": FEE_KAPPA_NUM,
        "kappa_den": FEE_KAPPA_DEN,
        "payoffs": [3, 0, 1],
        "prices": [2, 3, 5],
        "quantity": 1,
        "expected": {
            "dispersion_numerator": 53,
            "fee_numerator": 212,
            "denominator": 100_000,
            "paid": 0,
            "carry": 212,
            "terminal_ceil_charge": 1,
            "allocation": {"executor": 0, "executor_cap": None, "maker": 0, "treasury": 1},
        },
    },
    {
        "id": "FEE-008",
        "name": "allocation-executor-cap",
        "kind": "allocation_point",
        "note": "section 2.3 allocation with and without the batch executor cap",
        "derivation": (
            "maker=floor(1000*60/100)=600; executor=min(floor(1000*15/100), cap); "
            "treasury=pot-maker-executor"
        ),
        "pot": 1000,
        "expected": {
            "allocations": [
                {"cap": None, "executor": 150, "maker": 600, "treasury": 250},
                {"cap": 100, "executor": 100, "maker": 600, "treasury": 300},
                {"cap": 0, "executor": 0, "maker": 600, "treasury": 400},
            ]
        },
    },
)


def build_fee_fixture() -> dict[str, object]:
    return {
        "allocation_arm": {
            "denominator": 100,
            "executor_numerator": 15,
            "maker_numerator": 60,
            "status": "experimental arm; 60/15/25 is not promoted",
        },
        "carry_closes": [item.value for item in CarryClose],
        "carry_domains": [item.value for item in CarryDomain],
        "experiment": "EXP-ALIGN-03",
        "fee_side_arms": [item.value for item in FeeSideArm],
        "schema": FEE_SCHEMA,
        "source": "docs/implementation/POLICY_ANALYSIS_LOTS_FEES.md sections 2.2-2.5, 3.4",
        "status": STATUS,
        "vectors": list(FEE_VECTORS),
    }


# ---------------------------------------------------------------------------
# Replay helpers (used by the Python side of EXP-ALIGN-01/02/03)
# ---------------------------------------------------------------------------


def payout_set_from_fixture(market: Mapping[str, object]) -> IntegerPayoutSet:
    vectors = tuple(
        IntegerPayoutVector(
            int(entry["denominator"]), tuple(int(value) for value in entry["weights"])
        )
        for entry in market["payout_vectors"]
    )
    return IntegerPayoutSet(int(market["count"]), int(market["outcomes"]), vectors)


def classify_admission(
    market: Mapping[str, object], arm: str
) -> dict[str, str]:
    """Admit or refuse one admission vector under one policy arm."""

    try:
        WeightedBook.open(payout_set_from_fixture(market), PayoutPolicy(arm))
    except KernelRefusal as refusal:
        return {"error_class": refusal.error_class, "result": "refuse"}
    return {"result": "admit"}


def replay_trace(vector: Mapping[str, object], arm: str) -> dict[str, object]:
    """Replay one trace vector under one policy arm and report what happened."""

    market = vector["market"]
    payouts = payout_set_from_fixture(market)
    try:
        book = WeightedBook.open(
            payouts,
            PayoutPolicy(arm),
            collateral=int(market.get("initial_collateral", 0)),
            wallets=int(market.get("wallets", 1)),
        )
    except KernelRefusal as refusal:
        return {"admitted": False, "error_class": refusal.error_class}
    results: list[dict[str, object]] = []
    for step in vector["steps"]:
        try:
            book, payout = book.apply(step)
        except KernelRefusal as refusal:
            results.append({"error_class": refusal.error_class, "result": "refuse"})
            continue
        entry: dict[str, object] = {"result": "ok"}
        if payout is not None:
            entry["payout"] = payout
        results.append(entry)
    summary = book.state_summary()
    return {
        "admitted": True,
        "results": results,
        "final": {
            "collateral": summary["collateral"],
            "credit_total": summary["credit_total"],
            "positions": summary["positions"],
            "total_supply": summary["total_supply"],
        },
        "exit_dead": book.is_exit_dead() if book.resolved else False,
    }


def replay_fee_vector(vector: Mapping[str, object]) -> dict[str, object]:
    """Replay one fee vector and report the observable accounting."""

    kind = vector["kind"]
    if kind == "single_egg_schedule":
        result = run_fee_schedule(
            [
                Fill(
                    quantity=int(entry["quantity"]),
                    price=int(entry["price"]),
                    buyer_intent=str(entry["buyer_intent"]),
                    seller_intent=str(entry["seller_intent"]),
                    buyer_position=str(entry["buyer_position"]),
                    seller_position=str(entry["seller_position"]),
                    epoch=int(entry["epoch"]),
                )
                for entry in vector["fills"]
            ],
            int(vector["price_scale"]),
            int(vector["kappa_num"]),
            int(vector["kappa_den"]),
            domain=CarryDomain(vector["carry_domain"]),
            close_policy=CarryClose(vector["carry_close"]),
            side_arm=FeeSideArm(vector["fee_side_arm"]),
        )
        # PROPOSED variant, explicitly named (P0-5)
        allocation = allocate_fee(
            result.fee_pot,
            maker_num=60,
            executor_num=15,
            denominator=100,
            executor_cap=None,
        )
        return {
            "allocation": {
                "executor": allocation.executor,
                "executor_cap": None,
                "maker": allocation.maker,
                "treasury": allocation.treasury,
            },
            "buyer_debit_total": result.buyer_debit_total,
            "conservation": {
                "hoard_untouched": result.hoard_delta == 0,
                "payer_identity": result.conserves,
            },
            "consideration_total": result.consideration_total,
            "denominator": fee_denominator(
                int(vector["kappa_den"]), int(vector["price_scale"])
            ),
            "domain_paid": [list(item) for item in result.domain_paid],
            "fee_numerator_total": result.fee_numerator_total,
            "fee_pot": result.fee_pot,
            "fill_legs": [list(item) for item in result.fill_legs],
            "hoard_delta": result.hoard_delta,
            "seller_credit_total": result.seller_credit_total,
            "terminal_charges": result.terminal_charges,
        }
    if kind == "dispersion_point":
        dispersion = dispersion_numerator(vector["payoffs"], vector["prices"])
        numerator = fee_numerator(
            int(vector["quantity"]), dispersion, int(vector["kappa_num"])
        )
        denominator = fee_denominator(
            int(vector["kappa_den"]), int(vector["price_scale"])
        )
        paid, carry = divmod(numerator, denominator)
        # PROPOSED variant, explicitly named (P0-5)
        allocation = allocate_fee(
            paid + (1 if carry else 0),
            maker_num=60,
            executor_num=15,
            denominator=100,
            executor_cap=None,
        )
        return {
            "allocation": {
                "executor": allocation.executor,
                "executor_cap": None,
                "maker": allocation.maker,
                "treasury": allocation.treasury,
            },
            "carry": carry,
            "denominator": denominator,
            "dispersion_numerator": dispersion,
            "fee_numerator": numerator,
            "paid": paid,
            "terminal_ceil_charge": 1 if carry else 0,
        }
    if kind == "allocation_point":
        rows = []
        for cap in (None, 100, 0):
            # PROPOSED variant, explicitly named (P0-5)
            allocation = allocate_fee(
                int(vector["pot"]),
                maker_num=60,
                executor_num=15,
                denominator=100,
                executor_cap=cap,
            )
            rows.append(
                {
                    "cap": cap,
                    "executor": allocation.executor,
                    "maker": allocation.maker,
                    "treasury": allocation.treasury,
                }
            )
        return {"allocations": rows}
    raise ValueError(f"unknown fee vector kind {kind!r}")


# ---------------------------------------------------------------------------
# Deterministic serialization
# ---------------------------------------------------------------------------

FIXTURE_FILES = {
    "admission_vectors.json": build_admission_fixture,
    "fee_vectors.json": build_fee_fixture,
    "trace_vectors.json": build_trace_fixture,
}


def canonical_bytes(payload: object) -> bytes:
    """Sorted-key, two-space-indent JSON with a trailing newline."""

    return (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode("utf-8")


def fixture_directory() -> Path:
    return Path(__file__).resolve().parents[2] / "fixtures" / FIXTURE_DIRNAME


def write_fixtures(directory: Optional[Path] = None) -> list[Path]:
    """Write every fixture file deterministically; returns the paths written."""

    target = Path(directory) if directory is not None else fixture_directory()
    target.mkdir(parents=True, exist_ok=True)
    written = []
    for name, builder in sorted(FIXTURE_FILES.items()):
        path = target / name
        path.write_bytes(canonical_bytes(builder()))
        written.append(path)
    return written


if __name__ == "__main__":
    for path in write_fixtures():
        print(path)
