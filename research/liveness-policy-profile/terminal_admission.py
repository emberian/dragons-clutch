#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Machine checks for account/value terminal admission.

The checker is intentionally stricter than a prose inventory.  An account is
either explicitly permanent, an asset-empty replay tombstone, a fully owned and
tested refundable transient, external owner state, or a STOP.  Unknown refund
ownership, unknown cardinality, and missing close routes cannot disappear into
one aggregate storage number.
"""

from __future__ import annotations

from typing import Any, Mapping


LIFECYCLE_CLASSES = {
    "PERMANENT_INFRA",
    "PERMANENT_TOMBSTONE",
    "REFUNDABLE_TRANSIENT",
    "EXTERNAL_OWNER_STATE",
    "UNCLASSIFIED_STOP",
}
PASS = "PASS"
STOP = "STOP"


class TerminalAdmissionError(ValueError):
    """A terminal inventory is incomplete or internally contradictory."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise TerminalAdmissionError(message)


def _bounded(row: Mapping[str, Any]) -> bool:
    maximum = row.get("max_instances", {})
    return (
        maximum.get("kind") in {"FIXED", "BOUNDED_BY_ACCOUNT_FIELD"}
        and isinstance(maximum.get("hard_max"), int)
        and maximum["hard_max"] >= 0
    )


def validate_account(name: str, row: Mapping[str, Any]) -> None:
    """Validate one exactly classified account row."""

    lifecycle = row.get("lifecycle_class")
    _require(lifecycle in LIFECYCLE_CLASSES, f"{name}: invalid lifecycle class")
    _require(isinstance(row.get("bytes"), int) and row["bytes"] >= 0, f"{name}: bytes")
    _require(
        isinstance(row.get("rent_lamports"), int) and row["rent_lamports"] >= 0,
        f"{name}: rent",
    )
    _require(row.get("scope"), f"{name}: missing scope")
    _require(row.get("promotion") in {PASS, STOP}, f"{name}: invalid promotion")

    if lifecycle == "REFUNDABLE_TRANSIENT":
        _require(_bounded(row), f"{name}: refundable cardinality is not bounded")
        _require(row.get("rent_principal_recorded") is True, f"{name}: principal unrecorded")
        _require(
            row.get("rent_principal_owner") not in {None, "NONE", "UNRECORDED"},
            f"{name}: refund owner is not immutable",
        )
        _require(
            row.get("physical_close_route") not in {None, "NONE"},
            f"{name}: no physical close route",
        )
        _require(
            row.get("donation_disposition") == "FROZEN_NEUTRAL_SINK",
            f"{name}: donation is not segregated to a neutral sink",
        )
        _require(row.get("expiry_or_reaper") == PASS, f"{name}: no expiry/reaper")
        _require(row.get("close_bank_evidence") == PASS, f"{name}: no close bank evidence")
        _require(
            row.get("rollback_bank_evidence") == PASS,
            f"{name}: no close rollback evidence",
        )
    elif lifecycle in {"PERMANENT_INFRA", "PERMANENT_TOMBSTONE"}:
        _require(_bounded(row), f"{name}: permanent cardinality is not bounded")
        _require(
            row.get("rent_principal_owner") in {None, "NONE"},
            f"{name}: permanent account claims a refund owner",
        )
        _require(
            row.get("physical_close_route") in {None, "NONE"},
            f"{name}: permanent account claims a close route",
        )
        _require(
            row.get("full_rent_capitalized") is True,
            f"{name}: permanent rent is not fully capitalized",
        )
        if lifecycle == "PERMANENT_TOMBSTONE":
            _require(
                row.get("economic_assets_empty") is True,
                f"{name}: tombstone retains an economic asset",
            )
    elif lifecycle == "UNCLASSIFIED_STOP":
        _require(row.get("promotion") == STOP, f"{name}: unclassified account must STOP")
        blockers = row.get("blocking_ids")
        _require(
            isinstance(blockers, list)
            and bool(blockers)
            and all(isinstance(value, str) and value for value in blockers),
            f"{name}: unclassified STOP requires explicit blocker ids",
        )

    if name == "direct.candidate":
        maximum = row.get("max_instances", {})
        _require(
            maximum.get("kind") == "BOUNDED_BY_ACCOUNT_FIELD"
            and maximum.get("field") == "PriceGrid.tick_count"
            and maximum.get("hard_max") == 64,
            "direct.candidate: lifetime bound must be PriceGrid.tick_count <= 64",
        )


def validate_terminal_admission(
    terminal: Mapping[str, Any],
    *,
    expected_accounts: set[str],
) -> str:
    """Validate a complete terminal inventory and derive its global status."""

    _require(
        terminal.get("schema") == "dragons-clutch/terminal-admission/v1",
        "unexpected terminal-admission schema",
    )
    accounts = terminal.get("accounts")
    _require(isinstance(accounts, dict), "terminal accounts must be an object")
    _require(set(accounts) == expected_accounts, "terminal account inventory is incomplete")
    for name, row in accounts.items():
        _require(isinstance(row, dict), f"{name}: account row must be an object")
        validate_account(name, row)

    computed = PASS
    if terminal.get("source_release", {}).get("default_release_available") is not True:
        computed = STOP
    for row in terminal.get("reachable_terminal_shapes", {}).values():
        if row.get("reachable") is True and row.get("required") is True:
            if row.get("status") != PASS:
                computed = STOP
    for row in accounts.values():
        if row.get("promotion") != PASS:
            computed = STOP
    for row in terminal.get("token_residues", {}).values():
        if row.get("required") is True and row.get("status") != PASS:
            computed = STOP

    blockers = terminal.get("blocking_ids")
    _require(isinstance(blockers, list), "blocking_ids must be a list")
    _require(len(blockers) == len(set(blockers)), "blocking_ids contain duplicates")
    blocker_set = set(blockers)
    for name, row in accounts.items():
        for blocker in row.get("blocking_ids", []):
            _require(blocker in blocker_set, f"{name}: blocker is absent from global set")

    if terminal.get("source_release", {}).get("default_release_available") is not True:
        _require(
            "SOURCE.DEFAULT_REGISTRY_EMPTY" in blocker_set,
            "empty default source registry requires its canonical blocker",
        )

    _require(
        terminal.get("hoard_counts_as_liveness_funding") is False,
        "Hoard value must never fund liveness",
    )
    _require(
        terminal.get("claims_universal_no_stranded_value") is False,
        "the current profile must not claim universal no-stranded-value",
    )
    _require(terminal.get("status") == computed, "declared terminal status disagrees")
    if computed == STOP:
        _require(bool(blockers), "STOP requires blocker ids")
    return computed
