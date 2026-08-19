# SPDX-License-Identifier: AGPL-3.0-or-later
"""Joined checks for the complete current-runtime terminal profile."""

from __future__ import annotations

import unittest

from terminal_admission import validate_terminal_admission
from terminal_profile import ACCOUNT_ROWS, EXPECTED_ACCOUNTS, build_terminal


class TerminalProfileTests(unittest.TestCase):
    def test_inventory_names_are_unique_and_current_profile_stops(self) -> None:
        self.assertEqual(len(ACCOUNT_ROWS), len(EXPECTED_ACCOUNTS))
        terminal = build_terminal("f" * 40)
        self.assertEqual(
            validate_terminal_admission(terminal, expected_accounts=EXPECTED_ACCOUNTS),
            "STOP",
        )

    def test_pinned_default_rent_equation_covers_every_row(self) -> None:
        for name, bytes_, rent, *_ in ACCOUNT_ROWS:
            self.assertEqual(rent, (bytes_ + 128) * 6_960, name)

    def test_only_resolution_work_and_reserve_are_refundable(self) -> None:
        terminal = build_terminal()
        refundable = {
            name
            for name, row in terminal["accounts"].items()
            if row["lifecycle_class"] == "REFUNDABLE_TRANSIENT"
        }
        self.assertEqual(
            refundable,
            {"resolution.work.v1", "resolution.reserve.v1"},
        )

    def test_source_gate_blocks_value_but_does_not_claim_future_liveness(self) -> None:
        source = build_terminal()["source_release"]
        self.assertFalse(source["default_release_available"])
        self.assertFalse(source["value_admission_without_release"])
        self.assertEqual(source["endow_fail_closed_bank_evidence"], "PASS")


if __name__ == "__main__":
    unittest.main()
