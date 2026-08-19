# SPDX-License-Identifier: AGPL-3.0-or-later
"""Tests for terminal storage/value inventory admission."""

from __future__ import annotations

import unittest

from terminal_admission import TerminalAdmissionError, validate_terminal_admission


def permanent() -> dict[str, object]:
    return {
        "bytes": 64,
        "rent_lamports": 1_336_320,
        "scope": "PER_REALM",
        "max_instances": {"kind": "FIXED", "hard_max": 1},
        "lifecycle_class": "PERMANENT_INFRA",
        "physical_close_route": "NONE",
        "rent_principal_owner": "NONE",
        "full_rent_capitalized": True,
        "promotion": "PASS",
    }


def refundable() -> dict[str, object]:
    return {
        "bytes": 1_296,
        "rent_lamports": 9_911_040,
        "scope": "PER_ACTIVE_MARKET_WORK",
        "max_instances": {"kind": "FIXED", "hard_max": 1},
        "lifecycle_class": "REFUNDABLE_TRANSIENT",
        "physical_close_route": "FinalizeResolutionWorkV1_or_AbortResolutionWorkV1",
        "rent_principal_recorded": True,
        "rent_principal_owner": "ResolutionWork.payer",
        "donation_disposition": "FROZEN_NEUTRAL_SINK",
        "expiry_or_reaper": "PASS",
        "close_bank_evidence": "PASS",
        "rollback_bank_evidence": "PASS",
        "promotion": "PASS",
    }


def base(accounts: dict[str, dict[str, object]]) -> dict[str, object]:
    return {
        "schema": "dragons-clutch/terminal-admission/v1",
        "runtime_ref": "0" * 40,
        "status": "PASS",
        "source_release": {"default_release_available": True},
        "reachable_terminal_shapes": {
            "resolution.success": {"reachable": True, "required": True, "status": "PASS"}
        },
        "accounts": accounts,
        "token_residues": {},
        "hoard_counts_as_liveness_funding": False,
        "blocking_ids": [],
    }


class TerminalAdmissionTests(unittest.TestCase):
    def test_refundable_and_permanent_rows_pass_exact_rules(self) -> None:
        terminal = base({"batch.policy": permanent(), "resolution.work": refundable()})
        self.assertEqual(
            validate_terminal_admission(
                terminal, expected_accounts={"batch.policy", "resolution.work"}
            ),
            "PASS",
        )

    def test_refundable_requires_owner_close_donation_and_bank_evidence(self) -> None:
        for key, bad in (
            ("rent_principal_owner", "UNRECORDED"),
            ("physical_close_route", "NONE"),
            ("donation_disposition", "UNTRACKED"),
            ("expiry_or_reaper", "STOP"),
            ("close_bank_evidence", "STOP"),
            ("rollback_bank_evidence", "STOP"),
        ):
            row = refundable()
            row[key] = bad
            terminal = base({"resolution.work": row})
            with self.assertRaises(TerminalAdmissionError, msg=key):
                validate_terminal_admission(terminal, expected_accounts={"resolution.work"})

    def test_tombstone_must_be_asset_empty(self) -> None:
        row = permanent()
        row["lifecycle_class"] = "PERMANENT_TOMBSTONE"
        row["economic_assets_empty"] = False
        with self.assertRaises(TerminalAdmissionError):
            validate_terminal_admission(
                base({"epoch.tombstone": row}), expected_accounts={"epoch.tombstone"}
            )

    def test_missing_account_is_not_hidden_by_aggregate_rent(self) -> None:
        terminal = base({"batch.policy": permanent()})
        with self.assertRaises(TerminalAdmissionError):
            validate_terminal_admission(
                terminal, expected_accounts={"batch.policy", "direct.candidate"}
            )

    def test_candidate_bound_is_lifetime_grid_not_retained_top3(self) -> None:
        row = permanent()
        row["scope"] = "PER_EPOCH_PER_CANDIDATE"
        row["max_instances"] = {
            "kind": "BOUNDED_BY_ACCOUNT_FIELD",
            "field": "DirectWindow.retained_count",
            "hard_max": 3,
        }
        with self.assertRaises(TerminalAdmissionError):
            validate_terminal_admission(
                base({"direct.candidate": row}), expected_accounts={"direct.candidate"}
            )

    def test_any_reachable_terminal_stop_propagates(self) -> None:
        terminal = base({"batch.policy": permanent()})
        terminal["reachable_terminal_shapes"]["direct.empty"] = {
            "reachable": True,
            "required": True,
            "status": "STOP",
        }
        terminal["status"] = "STOP"
        terminal["blocking_ids"] = ["DIRECT.EMPTY_FROZEN_NO_LAPSE"]
        self.assertEqual(
            validate_terminal_admission(terminal, expected_accounts={"batch.policy"}),
            "STOP",
        )

    def test_default_empty_source_forces_stop(self) -> None:
        terminal = base({"batch.policy": permanent()})
        terminal["source_release"]["default_release_available"] = False
        terminal["status"] = "STOP"
        terminal["blocking_ids"] = ["SOURCE.DEFAULT_REGISTRY_EMPTY"]
        self.assertEqual(
            validate_terminal_admission(terminal, expected_accounts={"batch.policy"}),
            "STOP",
        )

    def test_hoard_cannot_be_turned_into_liveness_funding(self) -> None:
        terminal = base({"batch.policy": permanent()})
        terminal["hoard_counts_as_liveness_funding"] = True
        with self.assertRaises(TerminalAdmissionError):
            validate_terminal_admission(terminal, expected_accounts={"batch.policy"})

    def test_declared_pass_cannot_hide_stopped_account(self) -> None:
        row = permanent()
        row["lifecycle_class"] = "UNCLASSIFIED_STOP"
        row["promotion"] = "STOP"
        terminal = base({"unknown": row})
        with self.assertRaises(TerminalAdmissionError):
            validate_terminal_admission(terminal, expected_accounts={"unknown"})


if __name__ == "__main__":
    unittest.main()
