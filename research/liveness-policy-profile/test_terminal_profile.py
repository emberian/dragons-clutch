# SPDX-License-Identifier: AGPL-3.0-or-later
"""Joined checks for the complete current-runtime terminal profile."""

from __future__ import annotations

import unittest

from terminal_admission import validate_terminal_admission
from terminal_profile import (
    ACCOUNT_ROWS,
    EXPECTED_ACCOUNTS,
    REFUNDABLE_FACTS,
    _row,
    build_terminal,
)


class TerminalProfileTests(unittest.TestCase):
    def test_inventory_names_are_unique_and_current_profile_stops(self) -> None:
        self.assertEqual(len(ACCOUNT_ROWS), len(EXPECTED_ACCOUNTS))
        # 38 sealed-probe rows (the T2-6 probe adds epoch.window) plus the
        # seven Direct V3 rows, the two T2-8 general-plane pot/receipt rows,
        # and the TerminalClosure funding ledger, all classified after the
        # probe.  A row added or dropped without reclassifying here fails.
        self.assertEqual(len(ACCOUNT_ROWS), 48)
        terminal = build_terminal("f" * 40)
        self.assertEqual(
            validate_terminal_admission(terminal, expected_accounts=EXPECTED_ACCOUNTS),
            "STOP",
        )

    def test_direct_v3_families_are_classified_and_none_is_promoted(self) -> None:
        """The V3 merge's persistent families must stay in the inventory.

        The layout crate defines six persistent program-owned V3 families;
        the policy artifact contributes stage and final rows, giving seven
        inventory rows.  Three still STOP: the terminal Epoch V4 and the
        per-epoch policy final on structurally persisting rent, the stage
        row on the artifact-stage windfall.  The other four are
        REFUNDABLE_TRANSIENT since the e8ba31d5 close campaign sealed every
        close route they have — and that class is not an edit anyone can
        make here alone: `_row` refuses a refundable row that names no close
        mechanism, and policy.py refuses one whose sealed close evidence
        does not cover it.
        """

        terminal = build_terminal()
        stopped = {
            "direct.epoch.v4": ["DIRECT.EPOCH_RECEIPT_RENT_PERSISTS"],
            "artifact.direct_batch_policy_v3.final": [
                "DIRECT.POLICY_ARTIFACT_RENT_PERSISTS"
            ],
            "artifact.direct_batch_policy_v3.stage": [
                "RENT.ARTIFACT_PREFUND_WINDFALL"
            ],
        }
        for name, blockers in stopped.items():
            row = terminal["accounts"][name]
            self.assertEqual(row["lifecycle_class"], "UNCLASSIFIED_STOP", name)
            self.assertEqual(row["promotion"], "STOP", name)
            self.assertEqual(row["blocking_ids"], blockers, name)
        for name in (
            "direct.candidate.v3",
            "direct.window.v3",
            "direct.work_budget.v1",
            "direct.reservation.v2",
        ):
            row = terminal["accounts"][name]
            self.assertEqual(row["lifecycle_class"], "REFUNDABLE_TRANSIENT", name)
            self.assertNotIn("blocking_ids", row)
            self.assertTrue(row["rent_principal_recorded"], name)
            self.assertTrue(row["rent_principal_owner"].endswith(".funding.payer"), name)
            self.assertEqual(
                row["donation_disposition"], "FROZEN_NEUTRAL_SINK", name
            )
            self.assertEqual(row["expiry_or_reaper"], "PASS", name)
            self.assertIn("Lapse", row["expiry_route"] + row["physical_close_route"])
            self.assertEqual(row["close_bank_evidence"], "PASS", name)
            self.assertEqual(row["rollback_bank_evidence"], "PASS", name)
        # The retired blocker is gone from the whole inventory, not merely
        # from the rows that carried it.
        self.assertNotIn(
            "DIRECT.V3_CLOSE_EVIDENCE_UNSEALED", terminal["blocking_ids"]
        )
        for row in terminal["accounts"].values():
            self.assertNotIn(
                "DIRECT.V3_CLOSE_EVIDENCE_UNSEALED", row.get("blocking_ids", [])
            )
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
        ):
            self.assertIn(blocker, terminal["blocking_ids"])

    def test_general_epoch_window_and_clearing_rows_track_the_t2_6_probe(self) -> None:
        """The walk's account movements land in the inventory unpromoted.

        InitEpoch (tag 49) creates exactly one deadline-window companion per
        general epoch at seeds::epoch_window_pda and no handler closes it —
        T2-7's EpochWindow v2 moves the row 84 -> 231 bytes (the format
        revision appends the candidate-window deadline, live cardinality,
        3-slot retained registry, and selection result); the T2-6
        owner-interner region moves the checkpoint to 50,054 bytes and
        CandidateRecord v3's score digest moves the record to 337.  All
        three rows stay UNCLASSIFIED_STOP on the clearing plane's standing
        blocker — promoting any of them takes a close path (TerminalClosure,
        a recorded ranked Tier 2 blocker) and sealed bank evidence, not an
        edit here.
        """

        terminal = build_terminal()
        window = terminal["accounts"]["epoch.window"]
        self.assertEqual(window["bytes"], 231)
        self.assertEqual(window["rent_lamports"], 2_498_640)
        self.assertEqual(window["scope"], "PER_GENERAL_EPOCH")
        self.assertEqual(window["max_instances"], {"kind": "FIXED", "hard_max": 1})
        self.assertEqual(window["lifecycle_class"], "UNCLASSIFIED_STOP")
        self.assertEqual(window["physical_close_route"], "NONE")
        # TerminalClosure discharges the *reason* this row carried
        # PROFILE.STORAGE_INVENTORY_INCOMPLETE — no close path existed — and
        # replaces it with the two residuals that actually remain.
        self.assertEqual(
            window["blocking_ids"],
            [
                "RENT.ACCOUNT_REFUND_UNOWNED",
                "GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT",
            ],
        )
        for name, bytes_ in (
            ("legacy.clear_work", 50_054),
            ("legacy.candidate", 337),
        ):
            row = terminal["accounts"][name]
            self.assertEqual(row["bytes"], bytes_, name)
            self.assertEqual(row["lifecycle_class"], "UNCLASSIFIED_STOP", name)
            self.assertEqual(row["physical_close_route"], "NONE", name)

    def test_t2_8_pot_and_receipt_rows_are_classified_and_never_promoted(self) -> None:
        """The entitlement freeze's persistent families stay honest STOPs.

        FreezeEntitlement (58) creates one 262-byte FinalPotAccount per
        general epoch at seeds::pot_pda; EntitleSlice (59) creates one
        217-byte SettlementReceiptAccount per (candidate, slice) at
        seeds::receipt_pda, at most MAX_SLICES = 416 per selected candidate.
        No handler closes either family — consumption stamps the receipt
        exhausted in place (TerminalClosure stands in the settlement blocker
        ledger) — so both rows stay UNCLASSIFIED_STOP with no close route,
        and the CandidateFeedStage prefix deliberately has no row of its own
        (it is the feed account in its staging state, not a second family).
        Promoting either row takes a close path and sealed bank evidence,
        not an edit here.
        """

        terminal = build_terminal()
        pot = terminal["accounts"]["epoch.final_pot"]
        self.assertEqual(pot["bytes"], 262)
        self.assertEqual(pot["rent_lamports"], 2_714_400)
        self.assertEqual(pot["scope"], "PER_GENERAL_EPOCH")
        self.assertEqual(pot["max_instances"], {"kind": "FIXED", "hard_max": 1})
        receipt = terminal["accounts"]["epoch.receipt"]
        self.assertEqual(receipt["bytes"], 217)
        self.assertEqual(receipt["rent_lamports"], 2_401_200)
        self.assertEqual(receipt["scope"], "PER_SELECTED_CANDIDATE")
        self.assertEqual(receipt["max_instances"], {"kind": "FIXED", "hard_max": 416})
        for name, row in (("epoch.final_pot", pot), ("epoch.receipt", receipt)):
            self.assertEqual(row["lifecycle_class"], "UNCLASSIFIED_STOP", name)
            self.assertEqual(row["physical_close_route"], "NONE", name)
            self.assertEqual(row["promotion"], "STOP", name)
        self.assertEqual(
            pot["blocking_ids"],
            [
                "RENT.ACCOUNT_REFUND_UNOWNED",
                "GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT",
            ],
        )
        # The receipt close (tag 61) gates only on the receipt being
        # exhausted by permissionless settlement, so the abandoned-reservation
        # residual does not reach it; the optional funding ledger still does.
        self.assertEqual(
            receipt["blocking_ids"], ["RENT.ACCOUNT_REFUND_UNOWNED"]
        )
        self.assertNotIn("legacy.candidate_feed_stage", terminal["accounts"])
        self.assertNotIn("epoch.feed_stage", terminal["accounts"])

    def test_terminal_closure_names_its_residuals_and_promotes_nothing(self) -> None:
        """TerminalClosure gives the general plane a close DAG, not a promotion.

        Tags 60-67 close every general-plane family in dependency order, and
        the sealed walk drives both a cleared and a lapsed epoch to the root
        with exact conservation.  That retires the reason these rows carried
        PROFILE.STORAGE_INVENTORY_INCOMPLETE — *no close path existed* — but
        it cannot make any of them REFUNDABLE_TRANSIENT, because
        terminal_admission requires `rent_principal_recorded` and
        `expiry_or_reaper` unconditionally and the general plane holds neither
        unconditionally:

        * the GeneralFundingLedgerV1 sibling is OPTIONAL at every creating
          instruction (`accounts.len() == N || N + 1`), every close runs
          through `close_ledgered_group`, which requires it, and the sealed
          LAPSED walk proves the unledgered state is reachable —
          RENT.ACCOUNT_REFUND_UNOWNED;
        * `release_terminal_reservation` (60) is the only signer-gated edge in
          the DAG, so an abandoned zero-fill reservation holds its page, the
          pot, and the epoch root open —
          GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT.

        A future edit that flips one of these rows to refundable without
        closing both gaps fails here and again in
        policy.py::require_terminal_closure_evidence.
        """

        terminal = build_terminal()
        general = (
            "epoch.window",
            "epoch.final_pot",
            "epoch.receipt",
            "general.funding_ledger",
            "legacy.epoch.v2",
            "legacy.candidate",
            "legacy.candidate_feed",
            "legacy.clear_work",
        )
        residuals = {
            "RENT.ACCOUNT_REFUND_UNOWNED",
            "GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT",
        }
        for name in general:
            row = terminal["accounts"][name]
            self.assertEqual(row["lifecycle_class"], "UNCLASSIFIED_STOP", name)
            self.assertEqual(row["promotion"], "STOP", name)
            self.assertEqual(row["physical_close_route"], "NONE", name)
            self.assertNotIn(name, REFUNDABLE_FACTS, name)
            self.assertTrue(residuals & set(row["blocking_ids"]), name)
        for blocker in residuals:
            self.assertIn(blocker, terminal["blocking_ids"])
        # The four legacy rows keep the generic inventory blocker for a
        # reason the closure wave does not touch: their cardinality is
        # UNADMITTED, and terminal_admission refuses an unbounded refundable.
        for name in (
            "legacy.epoch.v2",
            "legacy.candidate",
            "legacy.candidate_feed",
            "legacy.clear_work",
        ):
            row = terminal["accounts"][name]
            self.assertIn("PROFILE.STORAGE_INVENTORY_INCOMPLETE", row["blocking_ids"])
            self.assertEqual(row["max_instances"]["hard_max"], None, name)
        # Both general-plane terminal shapes are REACHABLE for the first time
        # and both still STOP.
        shapes = terminal["reachable_terminal_shapes"]
        for name in ("general.cleared_epoch_closure", "general.lapsed_epoch_closure"):
            self.assertTrue(shapes[name]["reachable"], name)
            self.assertEqual(shapes[name]["status"], "STOP", name)

    def test_general_funding_ledger_row_is_pinned_to_the_layout_crate(self) -> None:
        """Account tag 26: 2 + 32 + 32 + 8 + 8 + 1 + 1 + 1 = 85 bytes."""

        row = build_terminal()["accounts"]["general.funding_ledger"]
        self.assertEqual(row["bytes"], 2 + 32 + 32 + 8 + 8 + 1 + 1 + 1)
        self.assertEqual(row["bytes"], 85)
        self.assertEqual(row["rent_lamports"], (85 + 128) * 6_960)
        self.assertEqual(row["rent_lamports"], 1_482_480)
        self.assertEqual(row["scope"], "PER_LEDGERED_GROUP")

    def test_v4_order_page_strand_carries_its_own_blocking_id(self) -> None:
        """The V3 campaign's third stranded family stops hiding in a generic id.

        `init_direct_v4_order_page` creates the 4,012-byte page with
        `create_pda_account_full_principal` — no ledger, no recorded payer —
        and no V3 route closes it; all three sealed bank runs re-measure
        28,814,401 lamports still held after settle and after lapse.  The
        general plane's instance of the same row *does* close (tag 63), which
        is exactly why the row stays an honest STOP rather than moving either
        way.
        """

        row = build_terminal()["accounts"]["order.page"]
        self.assertEqual(row["lifecycle_class"], "UNCLASSIFIED_STOP")
        self.assertEqual(row["bytes"], 4_012)
        self.assertEqual(row["rent_lamports"], 28_814_400)
        self.assertEqual(
            row["blocking_ids"],
            [
                "DIRECT.ACCOUNT_REFUND_UNOWNED",
                "DIRECT.EMPTY_FROZEN_NO_LAPSE",
                "DIRECT.ORDER_PAGE_RENT_PERSISTS",
            ],
        )

    def test_pinned_default_rent_equation_covers_every_row(self) -> None:
        for name, bytes_, rent, *_ in ACCOUNT_ROWS:
            self.assertEqual(rent, (bytes_ + 128) * 6_960, name)

    def test_refundable_rows_are_exactly_the_ones_with_sealed_close_evidence(
        self,
    ) -> None:
        """Six refundable rows, each naming its own close mechanism.

        The two ResolutionWork rows and the four Direct V3 families the
        e8ba31d5 close campaign measured.  A seventh cannot appear by
        editing the table: `_row` refuses a REFUNDABLE_TRANSIENT row that is
        absent from REFUNDABLE_FACTS, and policy.py refuses one that the
        sealed close family does not cover.
        """

        terminal = build_terminal()
        refundable = {
            name
            for name, row in terminal["accounts"].items()
            if row["lifecycle_class"] == "REFUNDABLE_TRANSIENT"
        }
        self.assertEqual(
            refundable,
            {
                "resolution.work.v1",
                "resolution.reserve.v1",
                "direct.candidate.v3",
                "direct.window.v3",
                "direct.work_budget.v1",
                "direct.reservation.v2",
            },
        )
        self.assertEqual(refundable, set(REFUNDABLE_FACTS))
        for name in refundable:
            row = terminal["accounts"][name]
            self.assertNotEqual(row["physical_close_route"], "NONE", name)
            self.assertNotIn(
                row["rent_principal_owner"], {"NONE", "UNRECORDED"}, name
            )
            self.assertEqual(row["max_instances"]["kind"], "FIXED", name)

    def test_a_refundable_row_without_a_close_mechanism_refuses(self) -> None:
        with self.assertRaises(ValueError):
            _row(
                "invented.transient",
                8,
                946_560,
                "PER_NOTHING",
                1,
                "REFUNDABLE_TRANSIENT",
                (),
            )

    def test_source_gate_blocks_value_but_does_not_claim_future_liveness(self) -> None:
        source = build_terminal()["source_release"]
        self.assertFalse(source["default_release_available"])
        self.assertFalse(source["value_admission_without_release"])
        self.assertEqual(source["endow_fail_closed_bank_evidence"], "PASS")


if __name__ == "__main__":
    unittest.main()
