# SPDX-License-Identifier: AGPL-3.0-or-later
"""Adversarial checks for the sealed R1 liveness profile."""

from __future__ import annotations

import copy
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from admission_math import AdmissionError, QuotePolicy, quote_route
from terminal_profile import build_terminal
import policy


def _route_tables(node, path=()):
    """Yield every ``routes`` table in a projection, however deeply nested.

    The scans below assert that a route name appears in no subsystem it does
    not belong to.  Recursing is what gives them teeth: a route table nested
    one level down would otherwise slip past a top-level-only scan.
    """

    if not isinstance(node, dict):
        return
    for key, value in node.items():
        if key == "routes" and isinstance(value, dict):
            yield path, value
        elif isinstance(value, dict):
            yield from _route_tables(value, (*path, key))


class PolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.evidence = policy.load_evidence()

    def test_projection_is_exactly_rederived(self) -> None:
        self.assertEqual(policy.derive(self.evidence), self.evidence["projection"])
        tampered = copy.deepcopy(self.evidence)
        historical = next(iter(tampered["historical_artifacts"]))
        tampered["measurements"]["resolution_work"]["artifact_sha256"] = historical
        with self.assertRaises(policy.CheckError):
            policy.check_artifact_binding(tampered)

    def test_exact_headroom_boundary_fails_closed(self) -> None:
        inputs = policy.quote_policy(self.evidence)
        at_gate = quote_route(1_120_000, inputs)
        above_gate = quote_route(1_120_001, inputs)
        self.assertTrue(at_gate.admitted)
        self.assertEqual(at_gate.selected_limit_cu, 1_400_000)
        self.assertFalse(above_gate.admitted)
        self.assertIsNone(above_gate.selected_limit_cu)
        self.assertIsNone(above_gate.keeper_reward_lamports)

    def test_resolution_work_maximum_path_is_exact(self) -> None:
        row = policy.derive(self.evidence)["resolution_work"]
        self.assertEqual(row["status"], "PASS")
        # The Fold(1) route covers its widest observation, the 88,641-CU
        # singleton measured inside the two-fold batch scenario.
        self.assertEqual(row["routes"]["fold_1"]["measured_cu"], 88_641)
        prefund = row["protocol_minimum_prefund"]
        self.assertEqual(prefund["worst_case_fold_calls"], 32)
        self.assertEqual(prefund["worst_case_fold_outflow_lamports"], 37_120_000)
        self.assertEqual(prefund["spendable_reserve_lamports"], 38_630_000)
        self.assertEqual(prefund["rent_principal_lamports"], 10_801_920)
        self.assertEqual(prefund["minimum_prefund_lamports"], 49_431_920)
        self.assertFalse(prefund["external_transaction_budget_included"])
        self.assertFalse(prefund["hoard_principal_included"])
        self.assertFalse(prefund["future_fee_revenue_included"])

        plan = row["runtime_execution_plan"]
        self.assertEqual(plan["fold_call_widths"], [4] * 8)
        self.assertEqual(plan["transaction_fold_call_counts"], [6, 2])
        self.assertEqual(plan["success_payout_lamports"], 10_790_000)
        self.assertEqual(plan["success_payer_refund_lamports"], 38_641_920)
        self.assertEqual(plan["abort_payout_lamports"], 10_140_000)
        self.assertEqual(plan["abort_payer_refund_lamports"], 39_291_920)

        external = row["external_keeper_budget_singleton_transactions"]
        self.assertEqual(external["fold_transactions_budget_lamports"], 7_360_000)
        self.assertEqual(external["success_post_begin_budget_lamports"], 7_680_000)
        self.assertEqual(external["success_total_budget_lamports"], 7_910_000)
        coverage = row["runtime_schedule_policy_coverage"]
        self.assertTrue(coverage["matches_policy"])
        self.assertTrue(all(item["covered"] for item in coverage["rows"]))
        self.assertNotIn("runtime_schedule_matches_policy", row)

    def test_batched_folds_quote_external_keeper_budget_only(self) -> None:
        """The plan may only use widths that fit in a packet.

        The sealed ``[12, 12, 8]`` plan was chosen on compute alone and cannot
        be sent: the keeper's wire probe measured six Fold instructions at
        1,216 bytes and twelve at 2,002, against a 1,232-byte packet budget.
        Width 6 is measured rather than interpolated, widths 8 and 12 keep
        their rows because they are real bank measurements, and the plan is
        composed only of sendable widths.
        """

        derived = policy.derive(self.evidence)
        row = derived["resolution_work_batched"]
        self.assertEqual(row["status"], "PASS")
        # Compute still admits twelve; the wire does not.
        self.assertEqual(row["maximum_admitted_batch"], 12)
        self.assertEqual(row["maximum_sendable_batch"], 6)
        self.assertEqual(row["cluster_packet_budget_bytes"], 1232)
        self.assertEqual(row["measured_but_unsendable_batches"], [8, 12])
        self.assertEqual(row["superseded_plan"], [12, 12, 8])
        self.assertNotIn("cluster_packet_budget", row, "the caveat is discharged")

        # The unsendable rows survive, labelled, rather than being deleted.
        self.assertEqual(row["routes"]["fold_batch_12"]["measured_cu"], 932_057)
        self.assertEqual(row["routes"]["fold_batch_12"]["selected_limit_cu"], 1_170_000)
        self.assertEqual(
            row["routes"]["fold_batch_12"]["keeper_reward_lamports"], 1_280_000
        )
        # The widest sendable batch, measured at this seal and not interpolated
        # between the four and eight that bracket it.
        self.assertEqual(row["routes"]["fold_batch_6"]["measured_cu"], 486_413)
        self.assertEqual(row["routes"]["fold_batch_6"]["selected_limit_cu"], 610_000)

        self.assertEqual(row["fewest_transaction_plan"], [6, 6, 6, 6, 6, 2])
        self.assertEqual(row["fold_transactions"], 6)
        self.assertEqual(row["plan_batches"], [1, 2, 4, 6])
        self.assertNotIn(
            8, row["plan_batches"], "an unsendable width may not compose a plan"
        )
        self.assertNotIn(12, row["plan_batches"])
        external = row["external_keeper_budget"]
        self.assertEqual(external["fold_transactions_budget_lamports"], 3_940_000)
        self.assertEqual(external["success_post_begin_budget_lamports"], 4_260_000)
        self.assertEqual(external["success_total_budget_lamports"], 4_490_000)
        self.assertFalse(external["rent_principal_included"])
        invalid = row["invalid_non_runtime_amount"]
        self.assertEqual(invalid["lamports"], 15_291_920)
        self.assertEqual(
            invalid["label"],
            "INVALID_RENT_PLUS_EXTERNAL_KEEPER_BUDGET_NOT_RUNTIME_PREFUND",
        )
        self.assertNotEqual(
            invalid["lamports"],
            derived["resolution_work"]["protocol_minimum_prefund"][
                "minimum_prefund_lamports"
            ],
        )
        dense = row["record_dense_fold4_external_budget"]
        self.assertEqual(dense["transaction_fold_call_counts"], [6, 2])
        self.assertEqual(
            dense["status"], "STOP_UNMEASURED_COMPOSED_FOLD4_TRANSACTION_CU"
        )
        self.assertIsNone(dense["total_external_keeper_budget_lamports"])
        self.assertTrue(row["runtime_schedule_batch_coverage"]["matches_policy"])

    def test_explicit_current_tree_fold4_overlay_is_identity_bound_and_separate(
        self,
    ) -> None:
        path = (
            Path(__file__).resolve().parent
            / "inflight"
            / "record-dense-fold4-current-tree.json"
        )
        measurement = policy.load_current_tree_fold4(path)
        overlay = policy.derive_current_tree_fold4(
            self.evidence, measurement, verify_disk=False
        )
        self.assertEqual(overlay["admission"], "UNSEALED_CURRENT_TREE")
        self.assertEqual(
            overlay["promotion"], "STOP_CURRENT_TREE_MEASUREMENT_NOT_SEALED"
        )
        self.assertFalse(overlay["sealed_projection_mutated"])

        external = overlay["external_keeper_budget"]
        self.assertEqual(external["status"], "PASS")
        self.assertEqual(external["fold_transactions_budget_lamports"], 1_090_000)
        self.assertEqual(external["success_lifecycle_budget_lamports"], 1_610_000)
        self.assertFalse(external["runtime_payout_included"])
        self.assertFalse(external["protocol_prefund_included"])
        self.assertFalse(external["rent_principal_included"])
        routes = external["routes"]
        self.assertEqual(routes["fold4_six_calls"]["measured_cu"], 514_332)
        self.assertEqual(routes["fold4_six_calls"]["packet_bytes"], 1_228)
        self.assertEqual(routes["fold4_six_calls"]["selected_limit_cu"], 650_000)
        self.assertEqual(routes["fold4_six_calls"]["keeper_reward_lamports"], 760_000)
        self.assertEqual(routes["fold4_two_calls"]["measured_cu"], 171_765)
        self.assertEqual(routes["fold4_two_calls"]["packet_bytes"], 704)
        self.assertEqual(routes["fold4_two_calls"]["selected_limit_cu"], 220_000)
        self.assertEqual(routes["fold4_two_calls"]["keeper_reward_lamports"], 330_000)
        for route in routes.values():
            self.assertEqual(route["admission"], "UNSEALED_CURRENT_TREE")
            self.assertEqual(route["elf_sha256"], overlay["elf_sha256"])
            self.assertEqual(
                route["source_scope_sha256"], overlay["source_scope_sha256"]
            )

        runtime = overlay["runtime_economics_cross_check"]
        self.assertEqual(runtime["fold_rewards_lamports"], 9_280_000)
        self.assertEqual(runtime["finalize_reward_lamports"], 1_510_000)
        self.assertEqual(runtime["payer_refund_lamports"], 38_641_920)
        self.assertFalse(runtime["external_keeper_budget_included"])
        self.assertEqual(
            overlay["protocol_prefund_cross_check"]["minimum_prefund_lamports"],
            49_431_920,
        )
        self.assertTrue(overlay["atomicity"]["whole_transaction_reverted"])

        # The default sealed 0d52 projection remains the fail-closed STOP.  An
        # explicit current-tree overlay cannot silently promote or overwrite it.
        dense = policy.derive(self.evidence)["resolution_work_batched"]
        dense = dense["record_dense_fold4_external_budget"]
        self.assertEqual(
            dense["status"], "STOP_UNMEASURED_COMPOSED_FOLD4_TRANSACTION_CU"
        )
        self.assertIsNone(dense["composed_transaction_budget_lamports"])

        tampered = copy.deepcopy(measurement)
        tampered["measurements"]["fold_transactions"][0]["admission"] = "SEALED"
        with self.assertRaises(policy.CheckError):
            policy.derive_current_tree_fold4(
                self.evidence, tampered, verify_disk=False
            )

        tampered = copy.deepcopy(measurement)
        tampered["measurements"]["fold_transactions"][0]["packet_bytes"] = 1_233
        with self.assertRaises(policy.CheckError):
            policy.derive_current_tree_fold4(
                self.evidence, tampered, verify_disk=False
            )

    def test_runtime_schedule_underfunding_is_rejected(self) -> None:
        finalize = policy.derive(self.evidence)["resolution_work"]["routes"]["finalize"]
        tampered = copy.deepcopy(self.evidence)
        tampered["resolution_work"]["runtime_reward_schedule"]["finalize_lamports"] = (
            finalize["keeper_reward_lamports"] - 1
        )
        with self.assertRaises(AdmissionError):
            policy.derive(tampered)

    def test_runtime_charge_schedule_rederives_prefund_and_plan_refund(self) -> None:
        tampered = copy.deepcopy(self.evidence)
        tampered["resolution_work"]["runtime_charge_schedule"][
            "fold_base_lamports"
        ] = 1
        row = policy.derive(tampered)["resolution_work"]
        prefund = row["protocol_minimum_prefund"]
        plan = row["runtime_execution_plan"]
        # The protocol minimum covers 32 legal Fold(1) calls, while the named
        # Fold(4) plan actually executes and charges only eight calls.
        self.assertEqual(prefund["minimum_prefund_lamports"], 49_431_952)
        self.assertEqual(plan["success_charges_lamports"], 8)
        self.assertEqual(plan["success_payer_refund_lamports"], 38_641_944)

    def test_direct_select_completes_but_v2_is_not_promoted(self) -> None:
        """The select CU-exhaustion STOP dissolved with the syscall hasher.

        The measured route now quotes normally, and the occupation-v4
        monolithic profile clears its headroom gate.  Neither is a promotion:
        V2 still stops on its unimplemented empty-frozen lapse, and a stopped
        headroom is still refused a lamport quote rather than clamped.
        """

        derived = policy.derive(self.evidence)
        row = derived["direct_v2"]["select"]
        self.assertEqual(row["status"], "PASS")
        self.assertEqual(row["measured_cu"], 227_464)
        self.assertEqual(row["selected_limit_cu"], 290_000)
        self.assertEqual(row["keeper_reward_lamports"], 400_000)
        self.assertEqual(derived["direct_v2"]["status"], "STOP")
        self.assertEqual(
            derived["direct_v2"]["empty_frozen_lapse"], "UNIMPLEMENTED_STOP"
        )
        self.assertEqual(derived["occupation_v4_monolithic"]["status"], "PASS")
        tampered = copy.deepcopy(self.evidence)
        tampered["measurements"]["direct_v2"]["select_cu"] = [1_400_000]
        stopped = policy.derive(tampered)["direct_v2"]
        self.assertEqual(stopped["status"], "STOP")
        self.assertEqual(stopped["select"]["status"], "STOP_HEADROOM")
        self.assertIsNone(stopped["select"]["selected_limit_cu"])
        self.assertIsNone(stopped["select"]["keeper_reward_lamports"])

    def test_walk_families_are_sealed_evidence_and_quoted_without_promotion(
        self,
    ) -> None:
        """T2-6 walk CU evidence binds to the exact artifact; W1 quotes it only.

        The general-epoch and clear-walk families are SBF-executed bank
        evidence for tags 49-53, sealed with their three logs.  At rung W1 the
        projection derives CU/quote/reward rows for them under
        ``ADMISSION_ROWS_NO_LIVE_FLAGS`` and does nothing else: the family
        status is still a STOP, live flags are still untouched, and the
        admission decision still belongs to ember.  The family block itself
        carries no quote — every lamport lives in a named W1 route row.
        Dropping the unpromoted declaration must refuse, and binding a walk
        family to a historical artifact must refuse.
        """

        derived = policy.derive(self.evidence)
        walk = derived["general_clearing_walk"]
        self.assertEqual(walk["status"], "SBF_EXECUTED_EVIDENCE_UNPROMOTED_STOP")
        self.assertEqual(walk["promotion_rung"], "W1")
        self.assertEqual(walk["admission_declaration"], "ADMISSION_ROWS_NO_LIVE_FLAGS")
        self.assertTrue(walk["admission_rows_derived"])
        self.assertEqual(walk["live_flags"], "UNTOUCHED")
        self.assertEqual(walk["decision_owner"], "ember")
        self.assertNotIn("selected_limit_cu", walk)
        self.assertNotIn("keeper_reward_lamports", walk)
        self.assertEqual(
            walk["measured_families"],
            [
                "general_epoch",
                "clear_walk",
                "candidate_selection",
                "entitled_clearing",
                "disagreement_exhibit",
                "scale_clearing",
                "terminal_closure",
            ],
        )
        for family in walk["measured_families"]:
            row = self.evidence["measurements"][family]
            self.assertEqual(
                row["admission"], "UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY", family
            )
            self.assertIn(family, policy.SAME_ELF_MEASUREMENTS)
        # The six general-clearing logs are sealed with the current root.
        artifact_root = self.evidence["artifact"]["path"].rsplit("/", 1)[0]
        for log in (
            "general_epoch",
            "clear_walk",
            "clear_lifecycle",
            "candidate_selection",
            "entitled_clearing",
            "disagreement_exhibit",
        ):
            self.assertIn(
                f"{artifact_root}/logs/bank/{log}.log", self.evidence["evidence_files"]
            )
        # Exact measured pins from those logs.
        epoch = self.evidence["measurements"]["general_epoch"]
        self.assertEqual(epoch["init_epoch_cu"], [42_766])
        self.assertEqual(
            epoch["freeze_epoch_rows"][2],
            {"pages": 3, "orders": 40, "cu": [717_842]},
        )
        walk_rows = self.evidence["measurements"]["clear_walk"]
        self.assertEqual(max(walk_rows["forty_order_pass1_cu"]), 385_439)
        self.assertEqual(walk_rows["complete_cu"], [121_149, 125_397])
        # The exhibit's third book composition, sealed from its own log.
        exhibit = self.evidence["measurements"]["disagreement_exhibit"]
        self.assertEqual(exhibit["exhibit_pass1_cu"], [414_566])
        self.assertEqual(exhibit["exhibit_pass2_cu"], [301_278])
        self.assertEqual(exhibit["init_clear_work_cu"], [70_635])
        self.assertEqual(len(exhibit["entitle_slice_single_cu"]), 5)
        self.assertEqual(len(exhibit["settle_page_entitled_direct_slice_cu"]), 5)
        self.assertEqual(exhibit["test_result"], "PASS_2_OF_2")
        tampered = copy.deepcopy(self.evidence)
        tampered["measurements"]["clear_walk"]["admission"] = "PROMOTED"
        with self.assertRaises(policy.CheckError):
            policy.derive(tampered)
        tampered = copy.deepcopy(self.evidence)
        historical = next(iter(tampered["historical_artifacts"]))
        tampered["measurements"]["general_epoch"]["artifact_sha256"] = historical
        with self.assertRaises(policy.CheckError):
            policy.check_artifact_binding(tampered)

    def test_selection_and_entitlement_families_are_sealed_but_never_promoted(
        self,
    ) -> None:
        """T2-7/T2-8 CU evidence binds to the exact artifact and derives nothing.

        The candidate-selection and entitled-clearing families seal the
        selection (tags 54-57) and entitlement/settlement (tags 58-59 plus
        the widened SettlePage) bank evidence with their two logs; the
        projection stays UNPROMOTED and no admission, quote, or reward row
        exists for any of their routes.  Dropping either family's unpromoted
        declaration must refuse, exactly like the walk's.
        """

        selection = self.evidence["measurements"]["candidate_selection"]
        self.assertEqual(max(selection["seal_candidate_cu"]), 64_174)
        self.assertEqual(selection["seal_candidate_displacing_cu"], [64_174])
        self.assertEqual(
            {row["shape"]: row["cu"] for row in selection["finalize_selection_rows"]},
            {
                "3_retained_2_verified_selects_winner": [49_307],
                "2_verified_beyond_128_bit_digest_tie": [39_539],
                "0_verified_honest_lapse": [20_708],
            },
        )
        self.assertEqual(selection["test_result"], "PASS_5_OF_5")
        entitled = self.evidence["measurements"]["entitled_clearing"]
        # Two sends now: the partial-fill wave added an inexact book that
        # funds the rounding pot, and it freezes more cheaply.
        self.assertEqual(entitled["freeze_entitlement_cu"], [92_560, 98_579])
        # ONE PAGE.  The same instruction measures 416,385 CU at two pages
        # and 759,892 at four; those are scale_clearing rows with the page
        # count in the key, not a widening of this one.
        self.assertEqual(entitled["entitle_slice_single_cu"], [207_315])
        self.assertEqual(entitled["entitle_slice_portfolio_pair_cu"], [245_842])
        self.assertEqual(entitled["settle_page_entitled_direct_slice_cu"], [51_479])
        self.assertEqual(
            entitled["settle_page_entitled_portfolio_full_pair_cu"], [226_212]
        )
        self.assertEqual(
            entitled["bank_conservation"],
            "POSITIONS_BYTE_EQUAL_IMPLIED_ALLOCATION_TOTAL_CASH_AND_EGGS_EXACT",
        )
        self.assertEqual(entitled["test_result"], "PASS_8_OF_8")
        derived = policy.derive(self.evidence)
        # Selection and entitlement routes exist in exactly one place: the W1
        # block, each carrying W1_QUOTED_NO_LIVE_FLAG.  No promoted subsystem
        # quotes one.  The scan recurses, so nesting a route table cannot hide
        # it from this check.
        w1_routes = set(derived["general_clearing_walk"]["w1"]["routes"])
        elsewhere = {
            name
            for path, table in _route_tables(derived)
            if path != ("general_clearing_walk", "w1")
            for name in table
        }
        for name in w1_routes:
            self.assertEqual(
                derived["general_clearing_walk"]["w1"]["routes"][name]["admission"],
                policy.WALK_PLANE_W1_ADMISSION,
                name,
            )
        for name in elsewhere:
            self.assertNotIn("entitle", name)
            self.assertNotIn("candidate", name)
        self.assertTrue(any("entitle" in name for name in w1_routes))
        self.assertTrue(any("candidate" in name for name in w1_routes))
        for family in ("candidate_selection", "entitled_clearing"):
            tampered = copy.deepcopy(self.evidence)
            del tampered["measurements"][family]["admission"]
            with self.assertRaises(policy.CheckError):
                policy.derive(tampered)

    def test_walk_plane_w1_quote_table_is_exactly_rederived(self) -> None:
        """Rung W1's thirty-five rows are the sealed maxima, quoted exactly.

        Every row is ``ceil(measured * 5/4)`` rounded up to the 10,000-CU
        quantum, priced at 10,000 lamports base cap + 1 lamport/CU + the
        100,000-lamport keeper tip — the same arithmetic every promoted family
        uses, run against this seal's own tables rather than transcribed from
        the promotion report — which was compiled against the superseded
        superseded root of its own cycle.  Seven of the 25 routes carried over
        from the 4fded7a6… seal move a selected limit by one quantum here, and
        ten further routes join with the disagreement exhibit's third book
        composition.  The pins below are the route maxima, and a maximum that
        drifts re-derives its limit and its reward, which the projection
        equality then catches.
        """

        w1 = policy.derive(self.evidence)["general_clearing_walk"]["w1"]
        self.assertEqual(w1["rung"], "W1")
        self.assertEqual(w1["status"], "PASS")
        self.assertEqual(w1["stopped_routes"], [])
        # 35 at the df0aece1… seal; 107 here.  The six scale campaigns joined
        # as 64 (route, shape) groups and the partial-fill wave's eight new
        # measured fields in entitled_clearing had to be quoted before the
        # coverage rule would pass.
        self.assertEqual(w1["quoted_route_count"], 107)
        self.assertEqual(len(w1["routes"]), 107)
        self.assertEqual(
            w1["quoted_families"],
            [
                "general_epoch",
                "clear_walk",
                "candidate_selection",
                "entitled_clearing",
                "disagreement_exhibit",
                "scale_clearing",
            ],
        )
        # The worst route is no longer the three-page book.  The maximum
        # 64-order book across four dense pages is 988,469 CU, and it still
        # admits — 1,240,000 selected against the 1,400,000 ceiling.
        self.assertEqual(w1["worst_route"], "scale_freeze_epoch_4pages_64orders")
        self.assertEqual(w1["worst_measured_cu"], 988_469)
        # The exhibit's book measures three quoted routes HOTTER than the
        # two-suite books do; that is the whole reason it is quoted.
        self.assertGreater(
            w1["routes"]["advance_clear_work_pass1_exhibit_book"]["measured_cu"],
            w1["routes"]["advance_clear_work_pass1_forty_order"]["measured_cu"],
        )
        self.assertGreater(
            w1["routes"]["entitle_slice_single_exhibit_book"]["measured_cu"],
            w1["routes"]["entitle_slice_single"]["measured_cu"],
        )
        self.assertGreater(
            w1["routes"][
                "settle_page_entitled_portfolio_full_pair_exhibit_book"
            ]["measured_cu"],
            w1["routes"]["settle_page_entitled_portfolio_full_pair"]["measured_cu"],
        )
        expected = {
            "advance_clear_slices": (173_890, 220_000, 330_000),
            "advance_clear_slices_exhibit_book": (163_327, 210_000, 320_000),
            "advance_clear_work_pass1_exhibit_book": (414_566, 520_000, 630_000),
            "advance_clear_work_pass1_forty_order": (385_439, 490_000, 600_000),
            "advance_clear_work_pass1_small_book": (292_742, 370_000, 480_000),
            "advance_clear_work_pass2_exhibit_book": (301_278, 380_000, 490_000),
            "advance_clear_work_pass2_forty_order": (306_043, 390_000, 500_000),
            "advance_clear_work_pass2_small_book": (287_224, 360_000, 470_000),
            "complete_clear_work_exhibit_book": (118_734, 150_000, 260_000),
            "complete_clear_work_selection": (126_241, 160_000, 270_000),
            "complete_clear_work_walk": (125_397, 160_000, 270_000),
            "entitle_slice_fragmented_buy": (194_906, 250_000, 360_000),
            "entitle_slice_inexact_pot_funding": (192_833, 250_000, 360_000),
            "entitle_slice_mixed_leg": (191_238, 240_000, 350_000),
            "entitle_slice_portfolio_pair": (245_842, 310_000, 420_000),
            "entitle_slice_portfolio_pair_exhibit_book": (270_118, 340_000, 450_000),
            "entitle_slice_single": (207_315, 260_000, 370_000),
            "entitle_slice_single_exhibit_book": (227_343, 290_000, 400_000),
            "entitle_slice_strand": (201_659, 260_000, 370_000),
            "finalize_selection_3_retained_winner": (49_307, 70_000, 180_000),
            "finalize_selection_digest_tie": (39_539, 50_000, 160_000),
            "finalize_selection_honest_lapse": (20_708, 30_000, 140_000),
            "freeze_entitlement": (98_579, 130_000, 240_000),
            "freeze_entitlement_exhibit_book": (98_575, 130_000, 240_000),
            "freeze_epoch_1page_4orders": (233_581, 300_000, 410_000),
            "freeze_epoch_2pages_17orders": (478_022, 600_000, 710_000),
            "freeze_epoch_3pages_40orders": (717_842, 900_000, 1_010_000),
            "init_clear_work_exhibit_book": (70_635, 90_000, 200_000),
            "init_epoch": (42_766, 60_000, 170_000),
            "place_order_portfolio": (194_452, 250_000, 360_000),
            "place_order_single": (195_136, 250_000, 360_000),
            "scale_advance_pass1_14orders": (414_367, 520_000, 630_000),
            "scale_advance_pass1_16orders": (434_705, 550_000, 660_000),
            "scale_advance_pass1_4orders": (340_959, 430_000, 540_000),
            "scale_advance_pass1_8orders": (346_451, 440_000, 550_000),
            "scale_advance_pass2_1page": (298_741, 380_000, 490_000),
            "scale_advance_pass2_2pages": (355_518, 450_000, 560_000),
            "scale_advance_pass2_4pages": (309_977, 390_000, 500_000),
            "scale_advance_slices_batch1": (169_983, 220_000, 330_000),
            "scale_advance_slices_batch12": (168_703, 220_000, 330_000),
            "scale_advance_slices_batch2": (166_983, 210_000, 320_000),
            "scale_advance_slices_batch8": (167_062, 210_000, 320_000),
            "scale_complete_clear_work_1page": (127_743, 160_000, 270_000),
            "scale_complete_clear_work_2pages": (124_918, 160_000, 270_000),
            "scale_complete_clear_work_4pages": (117_758, 150_000, 260_000),
            "scale_entitle_slice_single_1page": (217_235, 280_000, 390_000),
            "scale_entitle_slice_single_2pages": (416_385, 530_000, 640_000),
            "scale_entitle_slice_single_4pages": (759_892, 950_000, 1_060_000),
            "scale_entitle_slice_single_inexact_2pages": (394_397, 500_000, 610_000),
            "scale_finalize_selection_1retained": (31_630, 40_000, 150_000),
            "scale_finalize_selection_digest_tie_3retained": (53_470, 70_000, 180_000),
            "scale_freeze_entitlement_1page": (112_898, 150_000, 260_000),
            "scale_freeze_entitlement_2pages": (108_397, 140_000, 250_000),
            "scale_freeze_entitlement_4pages": (105_394, 140_000, 250_000),
            "scale_freeze_entitlement_inexact_2pages": (114_371, 150_000, 260_000),
            "scale_freeze_epoch_1page_4orders": (250_407, 320_000, 430_000),
            "scale_freeze_epoch_2pages_24orders": (488_317, 620_000, 730_000),
            "scale_freeze_epoch_2pages_30orders": (506_611, 640_000, 750_000),
            "scale_freeze_epoch_4pages_64orders": (988_469, 1_240_000, 1_350_000),
            "scale_init_clear_work_plus_4_grows_1page": (123_619, 160_000, 270_000),
            "scale_init_clear_work_plus_4_grows_2pages": (108_619, 140_000, 250_000),
            "scale_init_clear_work_plus_4_grows_4pages": (86_119, 110_000, 220_000),
            "scale_init_epoch_11ticks": (52_878, 70_000, 180_000),
            "scale_init_epoch_64ticks": (66_484, 90_000, 200_000),
            "scale_init_order_page_1page": (236_760, 300_000, 410_000),
            "scale_init_order_page_2pages": (227_760, 290_000, 400_000),
            "scale_init_order_page_4pages": (221_760, 280_000, 390_000),
            "scale_place_order_single_64ticks": (209_302, 270_000, 380_000),
            "scale_place_order_tick_probe_11ticks": (196_666, 250_000, 360_000),
            "scale_place_order_tick_probe_64ticks": (209_302, 270_000, 380_000),
            "scale_place_order_worst_rank_rank1": (201_724, 260_000, 370_000),
            "scale_place_order_worst_rank_rank26": (203_893, 260_000, 370_000),
            "scale_place_order_worst_rank_rank3": (210_726, 270_000, 380_000),
            "scale_place_order_worst_rank_rank6": (201_786, 260_000, 370_000),
            "scale_seal_candidate_0retained": (44_930, 60_000, 170_000),
            "scale_seal_candidate_1retained": (49_860, 70_000, 180_000),
            "scale_seal_candidate_2retained": (51_798, 70_000, 180_000),
            "scale_seal_candidate_3retained": (68_738, 90_000, 200_000),
            "scale_seal_candidate_displacing_3retained": (68_738, 90_000, 200_000),
            "scale_seal_candidate_refused_tied_3retained": (32_988, 50_000, 160_000),
            "scale_settle_page_direct_1page": (57_479, 80_000, 190_000),
            "scale_settle_page_direct_2pages": (61_995, 80_000, 190_000),
            "scale_settle_page_direct_4pages": (55_991, 70_000, 180_000),
            "scale_settle_page_potted_2pages": (58_386, 80_000, 190_000),
            "scale_submit_candidate_1page": (48_883, 70_000, 180_000),
            "scale_submit_candidate_2pages": (41_380, 60_000, 170_000),
            "scale_submit_candidate_4pages": (36_868, 50_000, 160_000),
            "scale_write_feed_fills_x16": (6_941, 10_000, 120_000),
            "scale_write_feed_fills_x24": (7_141, 10_000, 120_000),
            "scale_write_feed_fills_x4": (12_665, 20_000, 130_000),
            "scale_write_feed_fills_x6": (6_713, 10_000, 120_000),
            "scale_write_feed_slices_x12": (7_816, 10_000, 120_000),
            "scale_write_feed_slices_x16": (8_180, 20_000, 130_000),
            "scale_write_feed_slices_x2": (12_906, 20_000, 130_000),
            "scale_write_feed_slices_x8": (7_452, 10_000, 120_000),
            "seal_candidate_including_displacing": (64_174, 90_000, 200_000),
            "settle_page_entitled_direct_slice": (51_479, 70_000, 180_000),
            "settle_page_entitled_direct_slice_exhibit_book": (48_056, 70_000, 180_000),
            "settle_page_entitled_portfolio_full_pair": (226_212, 290_000, 400_000),
            "settle_page_entitled_portfolio_full_pair_exhibit_book": (252_563, 320_000, 430_000),
            "settle_page_mixed_leg": (50_986, 70_000, 180_000),
            "settle_page_partial_slice": (46_662, 60_000, 170_000),
            "settle_page_potted": (46_386, 60_000, 170_000),
            "settle_page_strand": (52_988, 70_000, 180_000),
            "submit_candidate": (35_734, 50_000, 160_000),
            "write_candidate_feed_fills": (9_665, 20_000, 130_000),
            "write_candidate_feed_slices": (9_906, 20_000, 130_000),
        }
        self.assertEqual(set(w1["routes"]), set(expected))
        boundary = policy.derive(self.evidence)[
            "maximum_raw_cu_with_requested_headroom"
        ]
        for name, (measured, limit, reward) in expected.items():
            row = w1["routes"][name]
            self.assertEqual(row["measured_cu"], measured, name)
            self.assertEqual(row["selected_limit_cu"], limit, name)
            self.assertEqual(row["keeper_reward_lamports"], reward, name)
            self.assertEqual(row["status"], "PASS", name)
            self.assertEqual(row["admission"], policy.WALK_PLANE_W1_ADMISSION, name)
            self.assertIn(row["shape_variability"], policy.WALK_PLANE_W1_VARIABILITY)
            self.assertGreaterEqual(row["observations"], 1, name)
            # The rule itself, not a transcription of it.
            self.assertEqual(row["required_headroom_cu"], -(-measured * 5 // 4), name)
            self.assertEqual(limit % 10_000, 0, name)
            self.assertGreaterEqual(limit, row["required_headroom_cu"], name)
            self.assertLess(limit - row["required_headroom_cu"], 10_000, name)
            self.assertEqual(row["external_fee_cap_lamports"], 10_000 + limit, name)
            self.assertEqual(reward, row["external_fee_cap_lamports"] + 100_000, name)
            self.assertLessEqual(measured, boundary, name)
        # The ten genuinely variable routes say so: the driver picks the
        # batch composition, so the quote bounds the measured compositions only.
        self.assertEqual(
            sorted(
                name
                for name, row in w1["routes"].items()
                if row["shape_variability"] == policy.W1_BATCH_VARIABLE
            ),
            [
                "advance_clear_slices",
                "advance_clear_slices_exhibit_book",
                "advance_clear_work_pass1_exhibit_book",
                "advance_clear_work_pass1_forty_order",
                "advance_clear_work_pass1_small_book",
                "advance_clear_work_pass2_exhibit_book",
                "advance_clear_work_pass2_forty_order",
                "advance_clear_work_pass2_small_book",
                "entitle_slice_strand",
                "settle_page_strand",
            ],
        )
        self.assertEqual(w1["routes"]["advance_clear_work_pass1_forty_order"][
            "observations"
        ], 22)
        # A drifted maximum re-derives, and the sealed projection then refuses.
        drifted = copy.deepcopy(self.evidence)
        drifted["measurements"]["general_epoch"]["init_epoch_cu"].append(63_000)
        row = policy.derive(drifted)["general_clearing_walk"]["w1"]["routes"][
            "init_epoch"
        ]
        self.assertEqual(row["measured_cu"], 63_000)
        self.assertEqual(row["selected_limit_cu"], 80_000)
        self.assertEqual(row["keeper_reward_lamports"], 190_000)
        with self.assertRaises(policy.CheckError):
            policy.require_equal(
                policy.derive(drifted), drifted["projection"], "policy projection"
            )

    def test_w1_refuses_a_live_flag_on_any_walk_family(self) -> None:
        """W1 is the rung that quotes and moves nothing.

        A live flag on any of the five walk families must refuse while W2's
        evidence does not exist, and the refusal must name what is still
        outstanding.  Every other subsystem's live flag is untouched by the
        rung.
        """

        derived = policy.derive(self.evidence)
        w1 = derived["general_clearing_walk"]["w1"]
        self.assertEqual(w1["live_flags"], "UNTOUCHED")
        self.assertFalse(w1["keeper_program_consumes_quotes"])
        self.assertEqual(
            w1["runtime_reward_schedule"], "NONE_NO_KEEPER_PROGRAM_READS_THESE_QUOTES"
        )
        self.assertFalse(derived["direct_v2"]["live_v3"])
        self.assertEqual(derived["direct_selection_v3"]["live_flags"], "UNTOUCHED")
        self.assertEqual(derived["general_terminal_closure"]["live_flags"], "UNTOUCHED")
        self.assertEqual(
            derived["occupation_v4_monolithic"]["live_action"],
            "MONOLITHIC_INITIAL_AND_RETRY_ADMITTED",
        )
        for family in policy.WALK_PLANE_FAMILIES:
            for flag in ("live_action", "live_walk", "live_clearing"):
                self.assertNotIn(flag, self.evidence["measurements"][family], family)
                tampered = copy.deepcopy(self.evidence)
                tampered["measurements"][family][flag] = True
                with self.assertRaises(policy.CheckError) as caught:
                    policy.derive(tampered)
                self.assertIn(flag, str(caught.exception))
                self.assertIn("rung W2", str(caught.exception))

    def test_w1_quotes_no_rent_row_and_the_general_plane_still_stops(self) -> None:
        """The rent side is declared unquoted and the rows it names still STOP.

        TerminalClosure gave the plane real close routes; the cycle-E
        reclassification still leaves every general-plane row an honest STOP on
        the optional funding ledger and the owner-signed release edge.  W1
        publishes which rows those are and prices none of them, and it
        publishes no lifecycle or path total that could be read as one.
        """

        derived = policy.derive(self.evidence)
        w1 = derived["general_clearing_walk"]["w1"]
        self.assertEqual(w1["rent_side"], "NOT_QUOTED_GENERAL_PLANE_ROWS_KEEP_THEIR_STOPS")
        self.assertEqual(w1["unquoted_rent_rows"], sorted(policy.TERMINAL_CLOSURE_ROWS))
        self.assertEqual(w1["path_quote"], "NOT_DESIGNED_NO_BOUNDED_TRANSACTION_PLAN")
        accounts = build_terminal(self.evidence["runtime_ref"])["accounts"]
        for name in w1["unquoted_rent_rows"]:
            self.assertEqual(accounts[name]["lifecycle_class"], "UNCLASSIFIED_STOP", name)
        self.assertEqual(derived["general_terminal_closure"][
            "rows_reclassified_refundable"
        ], [])
        # No rent, principal, reserve, or cold-outlay lamport anywhere in the
        # block: the only lamport fields are the per-route fee cap and reward.
        lamport_fields = {
            key
            for key in w1
            if isinstance(key, str) and key.endswith("lamports")
        }
        self.assertEqual(lamport_fields, set())
        for name, row in w1["routes"].items():
            self.assertEqual(
                {key for key in row if key.endswith("lamports")},
                {"external_fee_cap_lamports", "keeper_reward_lamports"},
                name,
            )
        # A general-plane row that quietly became refundable refuses.
        promoted = build_terminal(self.evidence["runtime_ref"])
        promoted["accounts"]["legacy.clear_work"]["lifecycle_class"] = (
            "REFUNDABLE_TRANSIENT"
        )
        with self.assertRaises(policy.CheckError):
            policy.require_walk_plane_w1_quotes(
                self.evidence["measurements"], promoted, policy.quote_policy(self.evidence)
            )

    def test_w1_refuses_a_measured_route_it_does_not_quote(self) -> None:
        """Nothing the suite measures may go unpublished behind the block.

        A new CU field, a dropped CU field, a new freeze or finalize shape, and
        a duplicated shape label each refuse, so the block can never claim to
        price the plane while a measured route sits outside it.
        """

        measurements = self.evidence["measurements"]
        terminal = build_terminal(self.evidence["runtime_ref"])
        inputs = policy.quote_policy(self.evidence)
        policy.require_walk_plane_w1_quotes(measurements, terminal, inputs)

        def refuses(mutate) -> str:
            tampered = copy.deepcopy(measurements)
            mutate(tampered)
            with self.assertRaises(policy.CheckError) as caught:
                policy.require_walk_plane_w1_quotes(tampered, terminal, inputs)
            return str(caught.exception)

        self.assertIn(
            "coverage",
            refuses(
                lambda rows: rows["entitled_clearing"].update(
                    {"cancel_entitlement_cu": [123]}
                )
            ),
        )
        self.assertIn(
            "coverage", refuses(lambda rows: rows["general_epoch"].pop("init_epoch_cu"))
        )
        self.assertIn(
            "shape coverage",
            refuses(
                lambda rows: rows["general_epoch"]["freeze_epoch_rows"].append(
                    {"pages": 4, "orders": 80, "cu": [900_000]}
                )
            ),
        )
        self.assertIn(
            "shape coverage",
            refuses(
                lambda rows: rows["candidate_selection"][
                    "finalize_selection_rows"
                ].append({"shape": "4_verified_full_width_tie", "cu": [60_000]})
            ),
        )
        self.assertIn(
            "shape coverage",
            refuses(
                lambda rows: rows["candidate_selection"][
                    "finalize_selection_rows"
                ].pop()
            ),
        )
        self.assertIn(
            "twice",
            refuses(
                lambda rows: rows["general_epoch"]["freeze_epoch_rows"].append(
                    {"pages": 1, "orders": 4, "cu": [1]}
                )
            ),
        )
        # The declared surcharge is a non-route by name, not by omission.
        self.assertEqual(
            policy.WALK_PLANE_W1_SURCHARGE_FIELDS["clear_walk"],
            "request_heap_frame_262144_surcharge_cu",
        )
        self.assertIn(
            "coverage",
            refuses(
                lambda rows: rows["clear_walk"].pop(
                    "request_heap_frame_262144_surcharge_cu"
                )
            ),
        )

    def test_w1_stops_an_impossible_envelope_instead_of_pricing_it(self) -> None:
        """A route past the admission boundary is a STOP with no lamports.

        The whole plane sits at 64% of the 1,120,000 raw-CU boundary today, so
        this is the falsifier rather than an observed row: one CU past the
        boundary must publish no limit, no fee cap, and no keeper reward, and
        must drop the block itself to STOP_HEADROOM.
        """

        terminal = build_terminal(self.evidence["runtime_ref"])
        inputs = policy.quote_policy(self.evidence)
        boundary = 1_120_000
        at_gate = copy.deepcopy(self.evidence["measurements"])
        at_gate["entitled_clearing"]["entitle_slice_single_cu"] = [boundary]
        row = policy.require_walk_plane_w1_quotes(at_gate, terminal, inputs)["routes"][
            "entitle_slice_single"
        ]
        self.assertEqual(row["status"], "PASS")
        self.assertEqual(row["selected_limit_cu"], 1_400_000)
        self.assertEqual(row["admission"], policy.WALK_PLANE_W1_ADMISSION)

        over = copy.deepcopy(self.evidence["measurements"])
        over["entitled_clearing"]["entitle_slice_single_cu"] = [boundary + 1]
        block = policy.require_walk_plane_w1_quotes(over, terminal, inputs)
        stopped = block["routes"]["entitle_slice_single"]
        self.assertEqual(block["status"], "STOP_HEADROOM")
        self.assertEqual(block["stopped_routes"], ["entitle_slice_single"])
        self.assertEqual(stopped["status"], "STOP_HEADROOM")
        self.assertEqual(stopped["admission"], policy.WALK_PLANE_W1_STOPPED_ADMISSION)
        self.assertIsNone(stopped["selected_limit_cu"])
        self.assertIsNone(stopped["external_fee_cap_lamports"])
        self.assertIsNone(stopped["keeper_reward_lamports"])

    def test_w1_limits_cover_the_measured_heap_frame_request(self) -> None:
        """The 150-CU ComputeBudget rider is published, not dropped.

        The walk suite measures `request_heap_frame(262144)` at 150 CU, and it
        rides in the same transaction as an AdvanceClearWork route.  The
        quantum absorbs it at every current row, which is a fact to check
        rather than assume: a surcharge that stopped fitting must refuse.
        """

        derived = policy.derive(self.evidence)
        w1 = derived["general_clearing_walk"]["w1"]
        self.assertEqual(w1["heap_frame_request_surcharge_cu"], 150)
        self.assertTrue(w1["surcharge_absorbed_by_selected_limits"])
        self.assertEqual(
            self.evidence["measurements"]["clear_walk"][
                "request_heap_frame_262144_surcharge_cu"
            ],
            [150],
        )
        inputs = policy.quote_policy(self.evidence)
        for name, row in w1["routes"].items():
            if row["family"] != "clear_walk":
                continue
            with_rider = quote_route(row["measured_cu"] + 150, inputs)
            self.assertEqual(with_rider.selected_limit_cu, row["selected_limit_cu"], name)
        tampered = copy.deepcopy(self.evidence)
        tampered["measurements"]["clear_walk"][
            "request_heap_frame_262144_surcharge_cu"
        ] = [10_000]
        with self.assertRaises(policy.CheckError) as caught:
            policy.derive(tampered)
        self.assertIn("heap-frame request", str(caught.exception))

    def test_w2_stays_blocked_on_the_ids_and_gaps_the_report_named(self) -> None:
        """W1 publishes what full admission is still missing, and it has teeth.

        The three blocking ids are the ones the walk plane's own terminal rows
        carry; the five gaps are section 3 of the promotion report.  If every
        named id retired, the block's "W2 is blocked" declaration would stop
        being true, so the derivation refuses rather than silently upgrading.
        """

        derived = policy.derive(self.evidence)
        w1 = derived["general_clearing_walk"]["w1"]
        self.assertEqual(w1["w2_status"], "BLOCKED")
        self.assertEqual(
            w1["w2_blocking_ids"],
            [
                "GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT",
                "PROFILE.STORAGE_INVENTORY_INCOMPLETE",
                "RENT.ACCOUNT_REFUND_UNOWNED",
            ],
        )
        self.assertEqual(
            w1["w2_evidence_gaps"],
            [
                "WIDER_PAGE_ORDER_AND_CANDIDATE_GRIDS",
                "FULL_WIDTH_TIE_AND_DISPLACEMENT_CAMPAIGNS",
                "SECOND_INDEPENDENT_BANK_PROFILE",
                "RENT_AND_CLOSE_ROWS_UNDER_A_RATIFIED_R4_CARVE_OUT",
                "FREEZE_TO_SETTLE_PATH_QUOTE_MODEL",
            ],
        )
        for blocker in w1["w2_blocking_ids"]:
            self.assertIn(blocker, derived["terminal_blocking_ids"])
        retired = build_terminal(self.evidence["runtime_ref"])
        retired["blocking_ids"] = [
            b
            for b in retired["blocking_ids"]
            if b not in policy.WALK_PLANE_W2_BLOCKING_IDS
        ]
        with self.assertRaises(policy.CheckError) as caught:
            policy.require_walk_plane_w1_quotes(
                self.evidence["measurements"],
                retired,
                policy.quote_policy(self.evidence),
            )
        self.assertIn("re-decided", str(caught.exception))

    def test_direct_v3_families_are_sealed_evidence_but_never_promoted(self) -> None:
        """Rung V1: every V3 CU row is sealed and nothing is admitted.

        The Direct V3 two-order venue (tags 36-46) had two unsealed
        syscall-era figures and no measurement family at all.  It now has
        both families, bound to this exact artifact by three logs, and the
        projection derives no admission, quote, or reward row for any V3
        route; ``live_v3`` stays false.  Dropping either family's unpromoted
        declaration must refuse, and binding one to a historical artifact
        must refuse, exactly like the walk's.
        """

        derived = policy.derive(self.evidence)
        v3 = derived["direct_selection_v3"]
        self.assertEqual(v3["status"], "SBF_EXECUTED_EVIDENCE_UNPROMOTED_STOP")
        self.assertFalse(v3["admission_rows_derived"])
        self.assertEqual(v3["live_flags"], "UNTOUCHED")
        self.assertEqual(v3["decision_owner"], "ember")
        self.assertEqual(v3["measured_families"], ["direct_v3", "direct_v3_close"])
        self.assertNotIn("selected_limit_cu", v3)
        self.assertNotIn("keeper_reward_lamports", v3)
        # V2's own live flag is untouched by the V3 campaign.
        self.assertFalse(derived["direct_v2"]["live_v3"])
        artifact_root = self.evidence["artifact"]["path"].rsplit("/", 1)[0]
        for log in (
            "direct_selection_v3",
            "direct_selection_v3_run2",
            "direct_selection_v3_run3",
        ):
            self.assertIn(
                f"{artifact_root}/logs/bank/{log}.log", self.evidence["evidence_files"]
            )
        rows = self.evidence["measurements"]["direct_v3"]
        for family in v3["measured_families"]:
            self.assertEqual(
                self.evidence["measurements"][family]["admission"],
                "UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY",
                family,
            )
            self.assertIn(family, policy.SAME_ELF_MEASUREMENTS)
            self.assertEqual(
                self.evidence["measurements"][family]["bank_runs"],
                policy.MINIMUM_V3_BANK_RUNS,
                family,
            )
        # Rows the bump search cannot move are pinned exactly; rows it can
        # move are sealed as the three-run spread rather than one sample.
        self.assertEqual(rows["init_epoch_cu"], [41_238] * 3)
        self.assertEqual(rows["init_order_page_cu"], [221_040] * 3)
        self.assertEqual(rows["begin_verification_cu"], [23_598] * 3)
        self.assertEqual(rows["verify_candidate_rows"][0]["cu"], [151_409] * 3)
        self.assertEqual(rows["abort_unfrozen_rows"][0]["cu"], [10_395] * 3)
        self.assertEqual(max(rows["freeze_epoch_cu"]), 382_795)
        self.assertEqual(
            [row["disposition"] for row in rows["submit_candidate_rows"]],
            [
                "RETAINED",
                "RETAINED",
                "RETAINED",
                "REPLACEMENT_DISPLACING_THE_WORST",
                "NONCOMPETITIVE_NO_STATE",
            ],
        )
        self.assertEqual(
            max(rows["submit_candidate_max_cu"]),
            max(
                max(row["cu"]) for row in rows["submit_candidate_rows"]
            ),
        )
        # The whole venue sits under the profile's own raw-CU admission
        # boundary in every observation, which is a fact about the rows and
        # not an admission of them.
        boundary = derived["maximum_raw_cu_with_requested_headroom"]
        for key, value in rows.items():
            if key.endswith("_cu") and isinstance(value, list):
                self.assertLess(max(value), boundary, key)
        for family in ("direct_v3", "direct_v3_close"):
            tampered = copy.deepcopy(self.evidence)
            del tampered["measurements"][family]["admission"]
            with self.assertRaises(policy.CheckError):
                policy.derive(tampered)
            tampered = copy.deepcopy(self.evidence)
            historical = next(iter(tampered["historical_artifacts"]))
            tampered["measurements"][family]["artifact_sha256"] = historical
            with self.assertRaises(policy.CheckError):
                policy.check_artifact_binding(tampered)

    def test_v3_close_evidence_and_its_refundable_rows_cannot_drift_apart(self) -> None:
        """The retirement of DIRECT.V3_CLOSE_EVIDENCE_UNSEALED has teeth.

        The blocker's own text said what retires it: a sealed bank
        measurement of a V3 close and its rollback.  That measurement now
        exists for every close route the four blocked families have, so the
        rows are REFUNDABLE_TRANSIENT — and ``require_v3_close_evidence``
        refuses the classification without the evidence, the evidence
        without exact conservation, a missing route, a missing rollback
        observation, fewer than three agreeing bank runs, and any other row
        claiming refundable.
        """

        close = self.evidence["measurements"]["direct_v3_close"]
        self.assertTrue(close["runs_agree_exactly"])
        self.assertEqual(set(close["routes"]), policy.V3_CLOSE_ROUTES)
        for name, row in close["routes"].items():
            self.assertIn(row["conservation"], policy.EXACT_CONSERVATION, name)
        self.assertEqual(
            set(close["rollback_observations"]), policy.V3_ROLLBACK_OBSERVATIONS
        )
        # Settle's seven closes and their exact landing place.
        settle = close["routes"]["SettleDirectV3"]
        self.assertEqual(len(settle["closed_lamports"]), 7)
        self.assertEqual(sum(settle["closed_lamports"].values()), 27_706_854)
        self.assertEqual(sum(settle["recipient_deltas"].values()), 27_706_854)
        self.assertEqual(settle["recipient_deltas"]["buy_owner"], 5_192_160)
        self.assertEqual(settle["recipient_deltas"]["sell_owner"], 5_192_160)
        self.assertEqual(settle["recipient_deltas"]["submitter"], 9_576_960)
        # Exactly the rent-exempt minimum of each closed shape came back:
        # 618-byte reservation, 632-byte window plus 488-byte candidate.
        # (The V3 rows are pinned post-probe, so they live in the terminal
        # inventory rather than in the probed ``accounts`` block.)
        terminal = build_terminal(self.evidence["runtime_ref"])
        accounts = terminal["accounts"]
        self.assertEqual(
            settle["recipient_deltas"]["buy_owner"],
            accounts["direct.reservation.v2"]["rent_lamports"],
        )
        self.assertEqual(
            settle["recipient_deltas"]["submitter"],
            accounts["direct.window.v3"]["rent_lamports"]
            + accounts["direct.candidate.v3"]["rent_lamports"],
        )
        self.assertNotIn(
            "DIRECT.V3_CLOSE_EVIDENCE_UNSEALED", terminal["blocking_ids"]
        )
        for name in policy.V3_REFUNDABLE_ROWS:
            row = terminal["accounts"][name]
            self.assertEqual(row["lifecycle_class"], "REFUNDABLE_TRANSIENT", name)
            self.assertEqual(row["close_bank_evidence"], "PASS", name)
            self.assertEqual(row["rollback_bank_evidence"], "PASS", name)
        # The campaign also measured what stays stranded, including the V4
        # OrderPage the promotion report's rent story omits.
        derived = policy.derive(self.evidence)
        self.assertEqual(
            derived["direct_selection_v3"][
                "structural_strand_rent_lamports_per_epoch"
            ],
            35_941_440,
        )
        self.assertIn("order.page", close["structural_strand_lamports"])
        for name in policy.V3_STRUCTURAL_STRAND_ROWS:
            self.assertEqual(
                terminal["accounts"][name]["lifecycle_class"], "UNCLASSIFIED_STOP", name
            )
        # Retiring one blocker is not promoting a subsystem.
        self.assertEqual(derived["terminal_status"], "STOP")

        def refuse(mutate) -> None:
            tampered = copy.deepcopy(self.evidence)
            mutate(tampered["measurements"]["direct_v3_close"])
            with self.assertRaises(policy.CheckError):
                policy.derive(tampered)

        refuse(lambda row: row["routes"].pop("SettleDirectV3"))
        refuse(lambda row: row["routes"]["SettleDirectV3"].update(conservation="NEAR"))
        refuse(lambda row: row["rollback_observations"].popitem())
        refuse(lambda row: row.update(bank_runs=1))
        refuse(lambda row: row.update(runs_agree_exactly=False))
        refuse(lambda row: row["closed_rows"].pop("direct.window.v3"))
        refuse(lambda row: row["closed_rows"].update({"direct.window.v3": []}))
        refuse(lambda row: row["structural_strand_lamports"].pop("order.page"))
        refuse(
            lambda row: row["structural_strand_lamports"].update({"order.page": 1})
        )

    def test_reproducibility_probes_are_pinned_with_their_dispositions(self) -> None:
        """The build-path amendment's probe rows are sealed exactly.

        The canonical identity is the in-place double build.  The cross-path
        worktree build is recorded as an observed-digest LIST under the
        PATH_TIED_SYMBOL_ORDER disposition — never as an equality claim — and
        the relocated-Cargo-home probe diverged at this artifact, restoring the
        PATH_SENSITIVE finding the `e8ba31d5…` seal believed superseded.
        """

        row = self.evidence["artifact_reproducibility"]
        digest = self.evidence["artifact"]["sha256"]
        self.assertEqual(row["normal_build_1"], digest)
        self.assertEqual(row["normal_build_2"], digest)
        self.assertNotIn("cross_path_build", row, "scalar equality claim is retired")
        observed = row["cross_path_builds"]
        self.assertTrue(observed, "at least one cross-path observation is required")
        for entry in observed:
            self.assertEqual(set(entry), {"path", "sha256", "bytes"})
            self.assertNotEqual(entry["sha256"], digest)
        self.assertEqual(row["cross_path_disposition"], "PATH_TIED_SYMBOL_ORDER")
        # The amended probe (cycle F recorded the amendment as owed; it has
        # since landed) resolves its relocated CARGO_HOME symlink before using
        # it, and with that component gone the relocated build reproduces the
        # canonical bytes exactly.  Three seals reported PATH_SENSITIVE here on
        # the unamended probe; this one reports INDEPENDENT, which is the
        # reading cycle F's two hand-run controls predicted.
        self.assertEqual(row["relocated_cargo_home"], digest)
        self.assertTrue(row["relocated_disposition"].startswith("INDEPENDENT"))
        artifact_root = self.evidence["artifact"]["path"].rsplit("/", 1)[0]
        self.assertIn(
            f"{artifact_root}/logs/sbf-build-crosspath.log",
            self.evidence["evidence_files"],
        )

    def test_a_coincidental_cross_path_match_is_refused_as_evidence(self) -> None:
        """The exact misreading the `e8ba31d5…` seal made must now fail closed.

        That seal observed one cross-path build that happened to come back
        byte-identical and recorded it as a property.  The V3 campaign then
        found two other digests at two other paths, and this seal found a
        third.  A list entry equal to the canonical digest is a coincidence,
        not a reproducibility claim, and the checker refuses it.
        """

        digest = self.evidence["artifact"]["sha256"]
        coincidence = copy.deepcopy(self.evidence)
        coincidence["artifact_reproducibility"]["cross_path_builds"].append(
            {"path": "/somewhere/else", "sha256": digest, "bytes": 1}
        )
        with self.assertRaises(policy.CheckError) as caught:
            policy.check_artifact_binding(coincidence)
        self.assertIn("coincidence", str(caught.exception))

        scalar = copy.deepcopy(self.evidence)
        scalar["artifact_reproducibility"].pop("cross_path_builds")
        scalar["artifact_reproducibility"]["cross_path_build"] = digest
        with self.assertRaises(policy.CheckError):
            policy.check_artifact_binding(scalar)

    def test_relocated_disposition_cannot_disagree_with_its_own_digest(self) -> None:
        """The weld holds in BOTH directions, which is the point of it.

        Until this seal the probe diverged and the lie worth refusing was a
        claim of INDEPENDENT over diverged bytes.  The probe now reproduces the
        canonical bytes, so the lie available is the opposite one — claiming
        PATH_SENSITIVE, and with it the whole controls-and-attribution
        apparatus, over a digest that plainly matches.  Both are refused by the
        same equality, and both are exercised here so that neither direction
        can rot while the other is the live case.
        """

        digest = self.evidence["artifact"]["sha256"]
        self.assertEqual(
            self.evidence["artifact_reproducibility"]["relocated_cargo_home"], digest
        )

        lying = copy.deepcopy(self.evidence)
        lying["artifact_reproducibility"]["relocated_disposition"] = (
            "PATH_SENSITIVE_REGISTRY_PANIC_LOCATION_STRINGS"
        )
        with self.assertRaises(policy.CheckError) as caught:
            policy.check_artifact_binding(lying)
        self.assertIn("disagrees with its own digest", str(caught.exception))

        # And the direction the earlier seals exercised: diverged bytes may not
        # be published as INDEPENDENT.
        other = copy.deepcopy(self.evidence)
        other["artifact_reproducibility"]["relocated_cargo_home"] = "0" * 64
        with self.assertRaises(policy.CheckError) as caught:
            policy.check_artifact_binding(other)
        self.assertIn("disagrees with its own digest", str(caught.exception))

    def test_a_diverged_relocation_must_carry_the_controls_that_locate_it(
        self,
    ) -> None:
        """PATH_SENSITIVE is a measurement; its attribution is a claim.

        This seal's probe is INDEPENDENT, so the controls apparatus does not
        run on the live evidence at all.  That is exactly when a gate rots, so
        the teeth are exercised here against a SYNTHETIC diverged probe: a
        future artifact that diverges again must still carry the observations
        that locate the cause, and must not be able to publish a bare
        disposition, a mislabelled control, an all-diverged control set (which
        would support the WIDER claim, not a narrow one), or an attribution
        with nothing behind it.

        The narrow claim these teeth protect is cycle F's: the divergence
        tracked a ``CARGO_HOME`` whose path contained an unresolved symlink
        component, not relocation as such.  The amended probe resolving that
        component and reproducing the canonical bytes is what turned that
        attribution from a hypothesis into a measurement.
        """

        digest = self.evidence["artifact"]["sha256"]

        def diverged() -> dict:
            """The live evidence, rewound to a probe that diverged."""

            fixture = copy.deepcopy(self.evidence)
            row = fixture["artifact_reproducibility"]
            row["relocated_cargo_home"] = "d" * 64
            row["relocated_disposition"] = "PATH_SENSITIVE_REGISTRY_PANIC_LOCATION_STRINGS"
            row["relocated_divergence"] = "RODATA_GROWS_552_BYTES_THREE_ABSOLUTE_REGISTRY_PATHS"
            row["relocated_controls"] = [
                {
                    "path": "/Users/ember/jobs/reloc-probe-B",
                    "sha256": digest,
                    "bytes": self.evidence["artifact"]["bytes"],
                    "disposition": "REPRODUCED_CANONICAL_BYTES",
                },
                {
                    "path": "/private/var/folders/T/reloc-probe-C/cargo-home",
                    "sha256": digest,
                    "bytes": self.evidence["artifact"]["bytes"],
                    "disposition": "REPRODUCED_CANONICAL_BYTES",
                },
            ]
            row["relocated_attribution"] = (
                "SYMLINKED_CARGO_HOME_PATH_COMPONENT_NOT_RELOCATION_ITSELF"
            )
            return fixture

        # The well-formed diverged seal passes, so the refusals below are about
        # the missing evidence and not about the fixture being malformed.
        policy.check_artifact_binding(diverged())

        bare = diverged()
        bare["artifact_reproducibility"].pop("relocated_controls")
        with self.assertRaises(policy.CheckError) as caught:
            policy.check_artifact_binding(bare)
        self.assertIn("control builds", str(caught.exception))

        mislabelled = diverged()
        mislabelled["artifact_reproducibility"]["relocated_controls"][0][
            "disposition"
        ] = "DIVERGED_FROM_CANONICAL"
        with self.assertRaises(policy.CheckError):
            policy.check_artifact_binding(mislabelled)

        all_diverged = diverged()
        for control in all_diverged["artifact_reproducibility"]["relocated_controls"]:
            control["sha256"] = "0" * 64
            control["disposition"] = "DIVERGED_FROM_CANONICAL"
        with self.assertRaises(policy.CheckError) as caught:
            policy.check_artifact_binding(all_diverged)
        self.assertIn("widened", str(caught.exception))

        duplicated = diverged()
        controls = duplicated["artifact_reproducibility"]["relocated_controls"]
        controls[1]["path"] = controls[0]["path"]
        with self.assertRaises(policy.CheckError) as caught:
            policy.check_artifact_binding(duplicated)
        self.assertIn("twice", str(caught.exception))

        unattributed = diverged()
        unattributed["artifact_reproducibility"].pop("relocated_attribution")
        with self.assertRaises(policy.CheckError):
            policy.check_artifact_binding(unattributed)

    def test_entitle_slice_is_quoted_per_page_count_and_never_flat(self) -> None:
        """The correction the scale campaigns forced.

        ``EntitleSlice`` must be presented with the whole bound page set and
        re-derives the live orders by walking every page in it, so its cost is
        a function of the page count.  The sealed one-page row is 207,315 CU;
        the same instruction is 416,385 at two pages and 759,892 at four.  A
        flat row is not a stale number, it is a quote for a different
        transaction understating the real one by a factor.

        So the coordinate is in the route key, and this test refuses any
        arrangement that would let one page count vouch for another.
        """

        derived = policy.derive(self.evidence)
        routes = derived["general_clearing_walk"]["w1"]["routes"]
        measured = {
            1: routes["scale_entitle_slice_single_1page"]["measured_cu"],
            2: routes["scale_entitle_slice_single_2pages"]["measured_cu"],
            4: routes["scale_entitle_slice_single_4pages"]["measured_cu"],
        }
        self.assertEqual(measured, {1: 217_235, 2: 416_385, 4: 759_892})
        # Monotone in the page count, and steeply so: the four-page send is
        # more than 3.4x the one-page row this profile used to publish alone.
        self.assertLess(measured[1], measured[2])
        self.assertLess(measured[2], measured[4])
        self.assertGreater(
            measured[4], 3.4 * routes["entitle_slice_single"]["measured_cu"]
        )
        # Each page count carries its OWN limit; no shared row exists.
        limits = {
            page: routes[f"scale_entitle_slice_single_{page}page"
                         f"{'s' if page != 1 else ''}"]["selected_limit_cu"]
            for page in (1, 2, 4)
        }
        self.assertEqual(len(set(limits.values())), 3)
        self.assertNotIn("scale_entitle_slice_single", routes)
        # Every scale route declares its shape in the key, so none of them can
        # be read as a general statement about the instruction.
        for name, row in routes.items():
            if name.startswith("scale_"):
                self.assertEqual(row["shape_variability"], policy.W1_SHAPE_LABELLED)
                self.assertEqual(row["family"], "scale_clearing")
        # The coordinates the family publishes are the ones it is keyed on.
        coordinates = derived["general_clearing_walk"]["w1"]["scale_shape_coordinates"]
        self.assertEqual(coordinates["entitle_slice_single_rows"], ["pages"])
        self.assertEqual(coordinates["freeze_epoch_rows"], ["pages", "orders"])

    def test_a_scale_shape_may_not_go_unquoted_or_be_stated_twice(self) -> None:
        """The teeth on the generated routes.

        The scale routes are derived from the tables rather than hand-listed,
        so the failure mode is not a forgotten route — it is a table that stops
        declaring its shape, or declares one twice, or appears with no
        coordinate at all.  Each refuses.
        """

        measurements = copy.deepcopy(self.evidence["measurements"])
        # A row that drops its coordinate cannot be quoted shape by shape.
        stripped = copy.deepcopy(measurements)
        del stripped["scale_clearing"]["entitle_slice_single_rows"][0]["pages"]
        with self.assertRaises(policy.CheckError) as caught:
            policy.scale_clearing_routes(stripped)
        self.assertIn("shape coordinate", str(caught.exception))

        # Two rows at the same shape would make one of them invisible.
        duplicated = copy.deepcopy(measurements)
        table = duplicated["scale_clearing"]["entitle_slice_single_rows"]
        table.append(copy.deepcopy(table[0]))
        with self.assertRaises(policy.CheckError) as caught:
            policy.scale_clearing_routes(duplicated)
        self.assertIn("twice", str(caught.exception))

        # A table nobody declared may not reach the projection.
        undeclared = copy.deepcopy(measurements)
        undeclared["scale_clearing"]["some_new_route_rows"] = [
            {"pages": 1, "observations": 1, "cu": [1]}
        ]
        with self.assertRaises(policy.CheckError) as caught:
            policy.scale_clearing_routes(undeclared)
        self.assertIn("no declared row table covers", str(caught.exception))

    def test_single_observation_rows_carry_the_pda_attempt_quantum(self) -> None:
        """A one-send row says what its maximum is known to.

        ``find_program_address`` pays one ``create_program_address`` per failed
        attempt at 1,500 CU, and the fixture's genesis keys are freshly random
        per run, so a route sealed from a single send cannot separate its shape
        term from its bump term.  The row publishes that instead of presenting
        one observation as exact.  It does NOT widen the quote: the limit is
        still the ordinary 5/4 rule over the observed maximum.
        """

        w1 = policy.derive(self.evidence)["general_clearing_walk"]["w1"]
        self.assertEqual(w1["pda_attempt_quantum_cu"], 1_500)
        self.assertEqual(policy.PDA_ATTEMPT_QUANTUM_CU, 1_500)
        quantum_term = (
            "PLUS_OR_MINUS_K_TIMES_1500_CU_PDA_ATTEMPT_QUANTUM"
        )
        for name, row in w1["routes"].items():
            if row["observations"] == 1:
                self.assertTrue(row["single_observation"], name)
                self.assertEqual(row["measured_cu_known_to_within"], quantum_term, name)
            else:
                self.assertFalse(row["single_observation"], name)
                self.assertEqual(
                    row["measured_cu_known_to_within"],
                    "SPREAD_OVER_MULTIPLE_SENDS_SEALED",
                    name,
                )
            # The caveat is about the measurement, not the arithmetic.
            self.assertEqual(
                row["required_headroom_cu"], -(-row["measured_cu"] * 5 // 4), name
            )
        self.assertEqual(
            w1["single_observation_routes"],
            sorted(n for n, r in w1["routes"].items() if r["observations"] == 1),
        )
        # The exhibit's five sends are the evidence that the term is real, and
        # they show its exact shape: the gaps sit ON the 1,500-CU lattice, with
        # a small genuine residual on top.  At this seal three of the four
        # consecutive gaps are exactly one quantum or zero, and the fourth is
        # 16 CU — real per-slice work, not a bump.  That is why one observation
        # cannot separate the two terms, and why the row says so.
        sends = sorted(
            self.evidence["measurements"]["disagreement_exhibit"][
                "entitle_slice_single_cu"
            ]
        )
        residuals = [
            min((b - a) % 1_500, -(b - a) % 1_500) for a, b in zip(sends, sends[1:])
        ]
        self.assertTrue(all(r < 100 for r in residuals), (sends, residuals))
        self.assertIn(0, residuals, (sends, residuals))

    def test_the_quoted_scale_shapes_are_the_ledgered_ones(self) -> None:
        """An account created without its funding ledger can never be closed.

        Every W1 creation row in the four original families was measured on
        machinery created WITHOUT the optional ``GeneralFundingLedgerV1``
        sibling, which records no payer — so no close route will ever guess it.
        The campaigns pass a ledger everywhere, which is why their rows are the
        ones a keeper can actually drive to a close, and the family has to say
        so.  Dropping the declaration refuses.
        """

        family = self.evidence["measurements"]["scale_clearing"]
        self.assertEqual(
            family["funding_ledger"], policy.SCALE_CLEARING_LEDGER_DECLARATION
        )
        self.assertIn("LEDGER", family["funding_ledger"])
        self.assertEqual(family["admission"], "UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY")
        self.assertEqual(
            policy.derive(self.evidence)["general_clearing_walk"]["w1"][
                "scale_funding_ledger"
            ],
            policy.SCALE_CLEARING_LEDGER_DECLARATION,
        )

        silent = copy.deepcopy(self.evidence)
        silent["measurements"]["scale_clearing"].pop("funding_ledger")
        with self.assertRaises(policy.CheckError):
            policy.derive(silent)

        # The unledgered rows are KEPT rather than deleted — they are what the
        # older suites measured — and they are a different route from the
        # ledgered one at the same nominal shape.
        routes = policy.derive(self.evidence)["general_clearing_walk"]["w1"]["routes"]
        self.assertIn("init_epoch", routes)
        self.assertIn("scale_init_epoch_64ticks", routes)
        self.assertGreater(
            routes["scale_init_epoch_64ticks"]["measured_cu"],
            routes["init_epoch"]["measured_cu"],
        )

    def test_revenue_boundary_is_sealed_and_derives_no_compute_row(self) -> None:
        """The fee-bearing boundary is driven, refused, and never quoted.

        ``revenue_policy.rs`` prints no CU label and no headline row, so the
        seal derives no compute row, no quote, and no refusal code from it —
        the codes it asserts live in the suite source, and a number transcribed
        out of source is not evidence.  What it does weld is the funding story:
        both rates zero, the treasury a sentinel that refuses structurally, and
        the record row an honest STOP carrying its own residual id.
        """

        family = self.evidence["measurements"]["revenue_boundary"]
        self.assertEqual(family["admission"], "UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY")
        self.assertEqual(family["suites"], ["revenue_policy.rs"])
        self.assertEqual(family["per_route_cu"], "NOT_LABELLED_BY_SUITE_NO_ROW_DERIVED")
        self.assertEqual(
            family["refusal_codes"], "NOT_PRINTED_BY_SUITE_ASSERTED_IN_SOURCE_ONLY"
        )
        self.assertEqual(family["test_result"], "PASS_1_OF_1")
        self.assertIn("revenue_boundary", policy.SAME_ELF_MEASUREMENTS)

        derived = policy.derive(self.evidence)["revenue_boundary"]
        self.assertFalse(derived["admission_rows_derived"])
        self.assertFalse(derived["cu_rows_derived"])
        self.assertFalse(derived["quote_rows_derived"])
        self.assertFalse(derived["fee_bearing_epoch_admits"])
        self.assertFalse(derived["vault_built"])
        self.assertEqual(derived["live_flags"], "UNTOUCHED")
        self.assertEqual(derived["record_row"], "revenue.policy_record.v1")
        self.assertEqual(derived["record_bytes"], 156)
        self.assertEqual(derived["record_rent_lamports"], 1_976_640)
        self.assertEqual(
            derived["residual_blocking_id"], "REVENUE.REALM_PERMANENCE_HOLDS_RECORD"
        )
        self.assertEqual(derived["fees_as_liveness_funding"], "NEVER_NOT_AT_ANY_RATE")

        terminal = policy.build_terminal(self.evidence["runtime_ref"])

        invented = copy.deepcopy(family)
        invented["close_record_cu"] = [12_345]
        with self.assertRaises(policy.CheckError) as caught:
            policy.require_revenue_boundary_evidence(invented, terminal)
        self.assertIn("no CU label", str(caught.exception))

        for field in ("rates", "treasury", "per_route_cu", "refusal_codes"):
            drifted = copy.deepcopy(family)
            drifted[field] = "SOMETHING_ELSE"
            with self.assertRaises(policy.CheckError):
                policy.require_revenue_boundary_evidence(drifted, terminal)

        refundable = copy.deepcopy(terminal)
        refundable["accounts"]["revenue.policy_record.v1"]["lifecycle_class"] = (
            "REFUNDABLE_TRANSIENT"
        )
        with self.assertRaises(policy.CheckError):
            policy.require_revenue_boundary_evidence(family, refundable)

        unnamed = copy.deepcopy(terminal)
        unnamed["accounts"]["revenue.policy_record.v1"]["blocking_ids"] = []
        with self.assertRaises(policy.CheckError):
            policy.require_revenue_boundary_evidence(family, unnamed)

    def test_w1_charges_the_exhibit_its_borrowed_heap_frame_rider(self) -> None:
        """A borrowed measurement may not become a silent one.

        Every walk transaction the disagreement exhibit measures carries the
        same ``request_heap_frame(262144)`` instruction the ``clear_walk`` suite
        prices at 150 CU, and the exhibit never re-prices it.  Its routes are
        therefore charged the ``clear_walk`` figure — which is honest only while
        the family says out loud that it is borrowing one.
        """

        exhibit = self.evidence["measurements"]["disagreement_exhibit"]
        self.assertEqual(
            exhibit["heap_frame_rider"],
            policy.WALK_PLANE_W1_BORROWED_SURCHARGE_DECLARATION[
                "disagreement_exhibit"
            ],
        )
        self.assertIn("disagreement_exhibit", policy.WALK_PLANE_W1_SURCHARGE_BEARING_FAMILIES)
        w1 = policy.derive(self.evidence)["general_clearing_walk"]["w1"]
        surcharge = w1["heap_frame_request_surcharge_cu"]
        self.assertEqual(surcharge, 150)
        for name, row in w1["routes"].items():
            if row["family"] != "disagreement_exhibit":
                continue
            self.assertGreaterEqual(
                row["selected_limit_cu"],
                -(-(row["measured_cu"] + surcharge) * 5 // 4),
                name,
            )

        silent = copy.deepcopy(self.evidence)
        silent["measurements"]["disagreement_exhibit"].pop("heap_frame_rider")
        with self.assertRaises(policy.CheckError) as caught:
            policy.derive(silent)
        self.assertIn("heap-frame rider", str(caught.exception))

    def test_w1_refuses_an_unquoted_measured_field_in_the_exhibit_family(self) -> None:
        """Nothing measured goes unpublished — in the new family too.

        The exhibit joined rung W1 precisely because a family outside the
        quoted list escapes this rule; the rule has to bite on it now.
        """

        extra = copy.deepcopy(self.evidence)
        extra["measurements"]["disagreement_exhibit"]["some_new_route_cu"] = [1_000]
        with self.assertRaises(policy.CheckError) as caught:
            policy.derive(extra)
        self.assertIn("W1 route coverage of family disagreement_exhibit", str(caught.exception))

        dropped = copy.deepcopy(self.evidence)
        dropped["measurements"]["disagreement_exhibit"].pop("exhibit_pass1_cu")
        with self.assertRaises(policy.CheckError):
            policy.derive(dropped)

    def test_terminal_closure_family_is_sealed_and_promotes_nothing(self) -> None:
        """Tags 60-67 seal a close DAG, an exact conservation, and no promotion.

        The cleared walk reclaims 531,639,600 of the 531,652,377 lamports the
        machinery held, burns exactly the two injected donations, and leaves a
        residual that is exactly the declared-permanent batch-policy
        artifact's own rent row.  The lapsed walk reclaims everything it can
        and leaves the deliberately unledgered candidate pair standing.  No
        admission row, quote, reward, or CU row is derived for any close
        route, and the suite prints no per-route CU label, so none is invented.
        """

        closure = self.evidence["measurements"]["terminal_closure"]
        self.assertEqual(
            closure["admission"], "UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY"
        )
        self.assertEqual(closure["intents"], list(range(60, 68)))
        self.assertIn("terminal_closure", policy.SAME_ELF_MEASUREMENTS)
        self.assertEqual(
            closure["per_route_cu"], "NOT_LABELLED_BY_SUITE_NO_ROW_DERIVED"
        )
        cleared = closure["walks"]["cleared_epoch"]
        self.assertEqual(cleared["machinery_inventory_lamports"], 531_652_377)
        self.assertEqual(cleared["reclaimed_lamports"], 531_639_600)
        self.assertEqual(cleared["burned_at_frozen_sink_lamports"], 12_777)
        self.assertEqual(cleared["residual_lamports"], 1_336_320)
        self.assertEqual(
            cleared["machinery_inventory_lamports"],
            cleared["reclaimed_lamports"] + cleared["burned_at_frozen_sink_lamports"],
        )
        lapsed = closure["walks"]["lapsed_epoch"]
        self.assertEqual(lapsed["machinery_inventory_lamports"], 47_167_920)
        self.assertEqual(lapsed["reclaimed_lamports"], 47_167_920)
        self.assertEqual(lapsed["burned_at_frozen_sink_lamports"], 0)
        self.assertEqual(lapsed["unregistered_residual_lamports"], 47_738_640)
        # The residual is the permanent artifact's own rent, not a measured
        # balance a prefund could flatter.
        self.assertEqual(
            cleared["residual_lamports"],
            build_terminal()["accounts"]["artifact.batch_policy.final"][
                "rent_lamports"
            ],
        )
        derived = policy.derive(self.evidence)
        row = derived["general_terminal_closure"]
        self.assertEqual(row["status"], "SBF_EXECUTED_EVIDENCE_UNPROMOTED_STOP")
        self.assertFalse(row["admission_rows_derived"])
        self.assertFalse(row["per_route_cu_rows_derived"])
        self.assertEqual(row["live_flags"], "UNTOUCHED")
        self.assertEqual(row["decision_owner"], "ember")
        self.assertEqual(row["rows_reclassified_refundable"], [])
        self.assertNotIn("selected_limit_cu", row)
        self.assertNotIn("keeper_reward_lamports", row)
        artifact_root = self.evidence["artifact"]["path"].rsplit("/", 1)[0]
        self.assertIn(
            f"{artifact_root}/logs/bank/terminal_closure.log",
            self.evidence["evidence_files"],
        )
        # No route table anywhere in the projection — including the walk
        # plane's W1 rows — names a close or release route, and no W1 row
        # draws on the terminal_closure family.
        for _, table in _route_tables(derived):
            for name in table:
                self.assertNotIn("close", name)
                self.assertNotIn("release", name)
        w1 = derived["general_clearing_walk"]["w1"]
        self.assertEqual(w1["excluded_families"], ["terminal_closure"])
        self.assertEqual(w1["excluded_intents"], list(range(60, 68)))
        self.assertEqual(w1["exclusion_reason"], closure["per_route_cu"])
        self.assertNotIn("terminal_closure", w1["quoted_families"])
        for name, route in w1["routes"].items():
            self.assertNotEqual(route["family"], "terminal_closure", name)

    def test_terminal_closure_evidence_and_classification_cannot_drift(self) -> None:
        """The weld holds in both directions.

        A general-plane row cannot quietly become refundable while the ledger
        stays optional, the evidence cannot stop declaring the two residuals
        while the rows still STOP on them, a walk cannot stop conserving, and
        the residual cannot stop being the permanent artifact's own rent.
        """

        terminal = build_terminal(self.evidence["runtime_ref"])
        closure = self.evidence["measurements"]["terminal_closure"]

        # 1. The evidence stops declaring a residual the rows depend on.
        for field in (
            "funding_ledger_optional_at_creation",
            "release_is_owner_signed",
        ):
            tampered = copy.deepcopy(closure)
            tampered[field] = False
            with self.assertRaises(policy.CheckError) as caught:
                policy.require_terminal_closure_evidence(tampered, terminal)
            self.assertIn(field, str(caught.exception))

        # 2. A walk that does not conserve.
        tampered = copy.deepcopy(closure)
        tampered["walks"]["cleared_epoch"]["burned_at_frozen_sink_lamports"] += 1
        with self.assertRaises(policy.CheckError):
            policy.require_terminal_closure_evidence(tampered, terminal)

        # 3. A residual that is not the permanent artifact's own rent row.
        tampered = copy.deepcopy(closure)
        tampered["walks"]["cleared_epoch"]["residual_lamports"] = 1
        with self.assertRaises(policy.CheckError):
            policy.require_terminal_closure_evidence(tampered, terminal)

        # 4. A lapsed walk that hides the unledgered residual.
        tampered = copy.deepcopy(closure)
        tampered["walks"]["lapsed_epoch"]["unregistered_residual_lamports"] = 0
        with self.assertRaises(policy.CheckError):
            policy.require_terminal_closure_evidence(tampered, terminal)

        # 5. A general-plane row promoted to refundable behind the evidence.
        promoted = copy.deepcopy(terminal)
        promoted["accounts"]["epoch.receipt"]["lifecycle_class"] = (
            "REFUNDABLE_TRANSIENT"
        )
        with self.assertRaises(policy.CheckError) as caught:
            policy.require_terminal_closure_evidence(closure, promoted)
        self.assertIn("optional at creation", str(caught.exception))

        # 6. A residual blocking id deleted from the global set.
        stripped = copy.deepcopy(terminal)
        stripped["blocking_ids"] = [
            b
            for b in stripped["blocking_ids"]
            if b != "GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT"
        ]
        with self.assertRaises(policy.CheckError):
            policy.require_terminal_closure_evidence(closure, stripped)

        # 7. The family losing its unpromoted declaration refuses in derive.
        tampered_evidence = copy.deepcopy(self.evidence)
        tampered_evidence["measurements"]["terminal_closure"]["admission"] = "PROMOTED"
        with self.assertRaises(policy.CheckError):
            policy.derive(tampered_evidence)

    def test_v4_order_page_strand_must_carry_its_own_blocking_id(self) -> None:
        """The corrected 35,941,440-lamport strand is welded to its ids.

        The sealed campaign measures the V4 OrderPage still holding 28,814,401
        lamports after both settle and lapse.  A strand row that drops its own
        blocking id must refuse, so the number and the reason cannot part.
        """

        close = self.evidence["measurements"]["direct_v3_close"]
        self.assertEqual(
            close["structural_strand_lamports"]["order.page"], 28_814_401
        )
        derived = policy.derive(self.evidence)["direct_selection_v3"]
        self.assertEqual(
            derived["structural_strand_rent_lamports_per_epoch"], 35_941_440
        )
        self.assertIn("order.page", derived["structural_strand_rows"])
        stripped = build_terminal(self.evidence["runtime_ref"])
        stripped["accounts"]["order.page"]["blocking_ids"] = [
            b
            for b in stripped["accounts"]["order.page"]["blocking_ids"]
            if b != "DIRECT.ORDER_PAGE_RENT_PERSISTS"
        ]
        with self.assertRaises(policy.CheckError) as caught:
            policy.require_v3_close_evidence(close, stripped)
        self.assertIn("DIRECT.ORDER_PAGE_RENT_PERSISTS", str(caught.exception))

    def test_source_refusal_is_not_capitalized_as_success(self) -> None:
        row = policy.derive(self.evidence)["source_value_admission"]
        self.assertEqual(row["status"], "FAIL_CLOSED_STOP")
        self.assertTrue(row["refusal_cu_not_priced_as_success"])

    def test_profile_emits_no_complete_policy(self) -> None:
        projection = policy.derive(self.evidence)
        self.assertEqual(projection["complete_liveness_policy"], "NOT_EMITTED_STOP")
        self.assertEqual(projection["terminal_status"], "STOP")

    def test_no_price_volume_or_hoard_policy_inputs(self) -> None:
        forbidden = {
            "sol_price",
            "token_price",
            "future_volume",
            "future_fees",
            "future_subscribers",
            "hoard_lamports",
            "hoard_collateral",
        }
        self.assertTrue(set(self.evidence["policy_inputs"]).isdisjoint(forbidden))

    def test_historical_probe_manifest_lock_and_source_are_pinned(self) -> None:
        self.assertTrue(policy.SEALED_PROBE_PATHS <= set(self.evidence["source_blobs"]))
        tampered = copy.deepcopy(self.evidence)
        relative, row = tampered["evidence_files"].popitem()
        tampered["evidence_files"][
            "research/liveness-policy-profile/artifacts/bd20711b01828a74/" + relative
        ] = row
        with self.assertRaises(policy.CheckError):
            policy.check_artifact_binding(tampered)

    def test_superseded_seal_evidence_cannot_be_dropped(self) -> None:
        policy.check_artifact_binding(self.evidence)
        for missing in ("artifacts/never-sealed", "artifacts/af6bb79cc3766bd0"):
            tampered = copy.deepcopy(self.evidence)
            digest = next(iter(tampered["historical_artifacts"]))
            tampered["historical_artifacts"][digest]["path"] = (
                f"research/liveness-policy-profile/{missing}/clutch_sbf.so"
            )
            with self.assertRaises(policy.CheckError):
                policy.check_artifact_binding(tampered)

    def test_rent_drift_is_rejected(self) -> None:
        tampered = copy.deepcopy(self.evidence)
        tampered["accounts"]["source.archive"]["rent_lamports"] += 1
        with self.assertRaises(policy.CheckError):
            policy.check_rent_and_accounts(tampered)

    def test_post_probe_v3_rows_are_pinned_and_never_shadow_the_probe(self) -> None:
        """The sealed v2 probe enumerates no Direct V3 row; the pin has teeth.

        The pinned post-probe names must be disjoint from the probe rows, a
        byte-pin drift must refuse, a pinned name absent from the terminal
        inventory must refuse, and dropping a pin re-exposes its row to the
        probe equality, which then refuses the un-probed row.  Every refusal
        here fires before the historical probe executes.
        """

        self.assertTrue(
            set(policy.POST_PROBE_DIRECT_V3_ROWS).isdisjoint(self.evidence["accounts"])
        )
        pinned = dict(policy.POST_PROBE_DIRECT_V3_ROWS)
        try:
            policy.POST_PROBE_DIRECT_V3_ROWS["direct.epoch.v4"] = 671
            with self.assertRaises(policy.CheckError) as caught:
                policy.check_rent_and_accounts(self.evidence)
            self.assertIn("post-probe V3 bytes direct.epoch.v4", str(caught.exception))

            policy.POST_PROBE_DIRECT_V3_ROWS.clear()
            policy.POST_PROBE_DIRECT_V3_ROWS.update(
                pinned, **{"direct.never_built.v9": 100}
            )
            with self.assertRaises(policy.CheckError) as caught:
                policy.check_rent_and_accounts(self.evidence)
            self.assertIn("direct.never_built.v9", str(caught.exception))

            policy.POST_PROBE_DIRECT_V3_ROWS.clear()
            policy.POST_PROBE_DIRECT_V3_ROWS.update(pinned)
            del policy.POST_PROBE_DIRECT_V3_ROWS["direct.window.v3"]
            with self.assertRaises(policy.CheckError) as caught:
                policy.check_rent_and_accounts(self.evidence)
            self.assertIn("terminal/probe account inventory", str(caught.exception))
        finally:
            policy.POST_PROBE_DIRECT_V3_ROWS.clear()
            policy.POST_PROBE_DIRECT_V3_ROWS.update(pinned)

    def test_post_probe_t2_8_rows_are_pinned_and_never_shadow_the_probe(self) -> None:
        """The sealed probe enumerates no general-plane pot/receipt row.

        Same teeth as the V3 pins: the T2-8 names must be disjoint from the
        probe rows and from the V3 pins, a byte-pin drift must refuse, and
        dropping a pin re-exposes its row to the probe equality, which then
        refuses the un-probed row.
        """

        self.assertTrue(
            set(policy.POST_PROBE_T2_8_ROWS).isdisjoint(self.evidence["accounts"])
        )
        self.assertTrue(
            set(policy.POST_PROBE_T2_8_ROWS).isdisjoint(policy.POST_PROBE_DIRECT_V3_ROWS)
        )
        self.assertEqual(
            policy.POST_PROBE_T2_8_ROWS,
            {"epoch.final_pot": 262, "epoch.receipt": 217},
        )
        pinned = dict(policy.POST_PROBE_T2_8_ROWS)
        try:
            policy.POST_PROBE_T2_8_ROWS["epoch.receipt"] = 218
            with self.assertRaises(policy.CheckError) as caught:
                policy.check_rent_and_accounts(self.evidence)
            self.assertIn("post-probe T2-8 bytes epoch.receipt", str(caught.exception))

            policy.POST_PROBE_T2_8_ROWS.clear()
            policy.POST_PROBE_T2_8_ROWS.update(pinned)
            del policy.POST_PROBE_T2_8_ROWS["epoch.final_pot"]
            with self.assertRaises(policy.CheckError) as caught:
                policy.check_rent_and_accounts(self.evidence)
            self.assertIn("terminal/probe account inventory", str(caught.exception))
        finally:
            policy.POST_PROBE_T2_8_ROWS.clear()
            policy.POST_PROBE_T2_8_ROWS.update(pinned)

    def test_invalid_policy_cannot_sneak_through(self) -> None:
        invalid = QuotePolicy(5, 4, 0, 1_400_000, 10_000, 1_000_000, 100_000)
        with self.assertRaises(AdmissionError):
            quote_route(1, invalid)


class TrackedEvidenceTests(unittest.TestCase):
    """Construct the ignored-artifact hole in real repositories and refuse it.

    Nothing here is mocked: each case builds an actual git repository, reaches
    the failing condition through the same operations that produced the
    near-miss (a ``.gitignore`` with ``*.so``/``*.log`` plus a plain
    ``git add`` of an artifact root), and asks the checker about it.
    """

    IGNORED_ARTIFACT = "research/liveness-policy-profile/artifacts/deadbeefdeadbeef/clutch_sbf.so"
    IGNORED_LOG = "research/liveness-policy-profile/artifacts/deadbeefdeadbeef/logs/sbf-build-1.log"
    TRACKED_AUDIT = "research/liveness-policy-profile/artifacts/deadbeefdeadbeef/audit/metadata.json"
    CAPTURE = "research/liveness-policy-profile/captured-output.txt"

    @classmethod
    def setUpClass(cls) -> None:
        cls.evidence = policy.load_evidence()

    def git(self, repo: Path, *argv: str) -> None:
        environment = dict(
            os.environ,
            GIT_CONFIG_GLOBAL=os.devnull,
            GIT_CONFIG_SYSTEM=os.devnull,
            GIT_AUTHOR_NAME="seal",
            GIT_AUTHOR_EMAIL="seal@example.invalid",
            GIT_COMMITTER_NAME="seal",
            GIT_COMMITTER_EMAIL="seal@example.invalid",
        )
        subprocess.run(
            ["git", *argv],
            cwd=repo,
            env=environment,
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
        )

    def scratch_tree(self) -> tuple[Path, dict[str, object]]:
        """Write a seal-shaped tree whose artifact and log are gitignored."""

        root = Path(tempfile.mkdtemp(prefix="clutch-tracked-evidence-")).resolve()
        self.addCleanup(shutil.rmtree, root, ignore_errors=True)
        (root / ".gitignore").write_text("*.so\n*.log\n", encoding="utf-8")
        for relative, payload in (
            (self.IGNORED_ARTIFACT, b"\x7fELF sealed default artifact"),
            (self.IGNORED_LOG, b"cargo-build-sbf log\n"),
            (self.TRACKED_AUDIT, b'{"artifact":"deadbeefdeadbeef"}\n'),
            (self.CAPTURE, b'{"schema":"dragons-clutch/liveness-bank-capture/v2"}\n'),
        ):
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(payload)
        evidence = {
            "artifact": {"path": self.IGNORED_ARTIFACT},
            "capture": {"path": self.CAPTURE},
            "evidence_files": {self.IGNORED_LOG: {}, self.TRACKED_AUDIT: {}},
            "historical_artifacts": {},
        }
        return root, evidence

    def scratch_repository(self, *, force: bool) -> tuple[Path, dict[str, object]]:
        """Commit that tree the careless way (``force=False``) or completely."""

        root, evidence = self.scratch_tree()
        self.git(root, "init", "--quiet")
        self.git(root, "add", "--", ".gitignore", self.CAPTURE)
        artifact_root = str(Path(self.IGNORED_ARTIFACT).parent)
        if force:
            self.git(root, "add", "--force", "--", artifact_root)
        else:
            self.git(root, "add", "--", artifact_root)
        self.git(root, "commit", "--quiet", "--no-gpg-sign", "-m", "seal")
        return root, evidence

    def test_committed_seal_passes_tracking_check(self) -> None:
        policy.check_tracked_evidence(self.evidence)
        sealed = set(policy.sealed_disk_paths(self.evidence))
        self.assertIn(self.evidence["artifact"]["path"], sealed)
        self.assertIn(self.evidence["capture"]["path"], sealed)
        self.assertTrue(set(self.evidence["evidence_files"]) <= sealed)
        for row in self.evidence["historical_artifacts"].values():
            self.assertIn(row["path"], sealed)
        root, evidence = self.scratch_repository(force=True)
        policy.check_tracked_evidence(evidence, repo=root)

    def test_gitignored_artifact_file_is_refused_as_untracked(self) -> None:
        root, evidence = self.scratch_repository(force=False)
        self.assertTrue((root / self.IGNORED_ARTIFACT).is_file())
        self.assertTrue((root / self.IGNORED_LOG).is_file())
        with self.assertRaises(policy.CheckError) as caught:
            policy.check_tracked_evidence(evidence, repo=root)
        message = str(caught.exception)
        self.assertIn("untracked", message)
        self.assertIn(self.IGNORED_ARTIFACT, message)
        self.assertIn(self.IGNORED_LOG, message)
        self.assertNotIsInstance(caught.exception, policy.TrackingUnavailable)

    def test_committed_evidence_modified_on_disk_is_refused(self) -> None:
        root, evidence = self.scratch_repository(force=True)
        policy.check_tracked_evidence(evidence, repo=root)
        with (root / self.IGNORED_ARTIFACT).open("ab") as stream:
            stream.write(b"\x00")
        with self.assertRaises(policy.CheckError) as caught:
            policy.check_tracked_evidence(evidence, repo=root)
        message = str(caught.exception)
        self.assertIn("differs from its committed blob", message)
        self.assertIn(self.IGNORED_ARTIFACT, message)
        self.assertNotIsInstance(caught.exception, policy.TrackingUnavailable)

    def test_staged_but_never_committed_evidence_is_refused(self) -> None:
        root, evidence = self.scratch_repository(force=False)
        self.git(root, "add", "--force", "--", self.IGNORED_ARTIFACT, self.IGNORED_LOG)
        with self.assertRaises(policy.CheckError) as caught:
            policy.check_tracked_evidence(evidence, repo=root)
        message = str(caught.exception)
        self.assertIn("not committed at HEAD", message)
        self.assertIn(self.IGNORED_ARTIFACT, message)

    def test_unanswerable_git_reports_unavailable_and_never_passes(self) -> None:
        root, evidence = self.scratch_tree()
        with self.assertRaises(policy.TrackingUnavailable) as caught:
            policy.check_tracked_evidence(evidence, repo=root)
        message = str(caught.exception)
        self.assertIn("UNAVAILABLE", message)
        self.assertIn("not a git repository", message)
        self.assertIsInstance(caught.exception, policy.CheckError)

    def test_absent_git_binary_reports_unavailable_and_never_passes(self) -> None:
        empty = Path(tempfile.mkdtemp(prefix="clutch-no-git-")).resolve()
        self.addCleanup(shutil.rmtree, empty, ignore_errors=True)
        previous = os.environ.get("PATH")

        def restore_path() -> None:
            if previous is None:
                os.environ.pop("PATH", None)
            else:
                os.environ["PATH"] = previous

        self.addCleanup(restore_path)
        os.environ["PATH"] = str(empty)
        with self.assertRaises(policy.TrackingUnavailable) as caught:
            policy.check_tracked_evidence(self.evidence)
        message = str(caught.exception)
        self.assertIn("UNAVAILABLE", message)
        self.assertIn("cannot run 'git rev-parse'", message)
        self.assertIsInstance(caught.exception, policy.CheckError)

    def test_portable_attestation_clone_still_verifies(self) -> None:
        """The archive+bundle attestation context is a checkout: it still passes."""

        destination = Path(tempfile.mkdtemp(prefix="clutch-portable-seal-")).resolve()
        self.addCleanup(shutil.rmtree, destination, ignore_errors=True)
        bundle = destination / "repo.bundle"
        self.git(policy.REPO, "bundle", "create", str(bundle), "HEAD")
        clone = destination / "attestation"
        self.git(destination, "clone", "--quiet", str(bundle), str(clone))
        self.assertNotEqual(clone.resolve(), policy.REPO.resolve())
        policy.check_tracked_evidence(self.evidence, repo=clone)


if __name__ == "__main__":
    unittest.main()
