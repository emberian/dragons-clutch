# SPDX-License-Identifier: AGPL-3.0-or-later
"""Joined checks for the complete current-runtime terminal profile."""

from __future__ import annotations

import unittest

from terminal_admission import validate_terminal_admission
from terminal_profile import ACCOUNT_ROWS, EXPECTED_ACCOUNTS, build_terminal


class TerminalProfileTests(unittest.TestCase):
    def test_inventory_names_are_unique_and_current_profile_stops(self) -> None:
        self.assertEqual(len(ACCOUNT_ROWS), len(EXPECTED_ACCOUNTS))
        # 37 sealed-probe rows plus the seven Direct V3 rows classified after
        # the probe.  A row added or dropped without reclassifying here fails.
        self.assertEqual(len(ACCOUNT_ROWS), 44)
        terminal = build_terminal("f" * 40)
        self.assertEqual(
            validate_terminal_admission(terminal, expected_accounts=EXPECTED_ACCOUNTS),
            "STOP",
        )

    def test_direct_v3_families_are_classified_and_none_is_promoted(self) -> None:
        """The V3 merge's persistent families must stay in the inventory.

        The layout crate defines six persistent program-owned V3 families;
        the policy artifact contributes stage and final rows, giving seven
        inventory rows.  Every one is UNCLASSIFIED_STOP: the closeable rows
        stop on the unsealed close evidence, the terminal Epoch V4 and the
        per-epoch policy final stop on structurally persisting rent, and the
        stage row keeps the artifact-stage windfall blocker.  Promoting any
        of them takes sealed bank evidence, not an edit here.
        """

        terminal = build_terminal()
        expected = {
            "direct.epoch.v4": ["DIRECT.EPOCH_RECEIPT_RENT_PERSISTS"],
            "direct.candidate.v3": ["DIRECT.V3_CLOSE_EVIDENCE_UNSEALED"],
            "direct.window.v3": ["DIRECT.V3_CLOSE_EVIDENCE_UNSEALED"],
            "direct.work_budget.v1": ["DIRECT.V3_CLOSE_EVIDENCE_UNSEALED"],
            "direct.reservation.v2": ["DIRECT.V3_CLOSE_EVIDENCE_UNSEALED"],
            "artifact.direct_batch_policy_v3.final": [
                "DIRECT.POLICY_ARTIFACT_RENT_PERSISTS"
            ],
            "artifact.direct_batch_policy_v3.stage": [
                "RENT.ARTIFACT_PREFUND_WINDFALL"
            ],
        }
        for name, blockers in expected.items():
            row = terminal["accounts"][name]
            self.assertEqual(row["lifecycle_class"], "UNCLASSIFIED_STOP", name)
            self.assertEqual(row["promotion"], "STOP", name)
            self.assertEqual(row["blocking_ids"], blockers, name)
        # Exact layout-crate byte pins; rent follows from the equation test.
        for name, bytes_ in (
            ("direct.epoch.v4", 672),
            ("direct.candidate.v3", 488),
            ("direct.window.v3", 632),
            ("direct.work_budget.v1", 248),
            ("direct.reservation.v2", 618),
            ("artifact.direct_batch_policy_v3.final", 96),
            ("artifact.direct_batch_policy_v3.stage", 232),
        ):
            self.assertEqual(terminal["accounts"][name]["bytes"], bytes_, name)
        # Live-instance bounds: top-3 candidate retention, one window and one
        # WorkBudget per epoch, exactly the frozen two-order reservation pair.
        for name, hard_max in (
            ("direct.candidate.v3", 3),
            ("direct.window.v3", 1),
            ("direct.work_budget.v1", 1),
            ("direct.reservation.v2", 2),
        ):
            self.assertEqual(
                terminal["accounts"][name]["max_instances"],
                {"kind": "FIXED", "hard_max": hard_max},
                name,
            )
        self.assertEqual(
            terminal["accounts"]["direct.epoch.v4"]["max_instances"],
            {"kind": "UNADMITTED", "hard_max": None},
        )
        for blocker in (
            "DIRECT.EPOCH_RECEIPT_RENT_PERSISTS",
            "DIRECT.POLICY_ARTIFACT_RENT_PERSISTS",
            "DIRECT.V3_CLOSE_EVIDENCE_UNSEALED",
        ):
            self.assertIn(blocker, terminal["blocking_ids"])

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
