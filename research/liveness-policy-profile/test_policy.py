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
        # The Fold(1) route covers its widest observation, the 88,434-CU
        # singleton measured inside the two-fold batch scenario.
        self.assertEqual(row["routes"]["fold_1"]["measured_cu"], 88_434)
        self.assertEqual(row["fold_path_lamports"], 7_360_000)
        self.assertEqual(row["success_rewards_lamports"], 7_680_000)
        self.assertEqual(row["worst_abort_rewards_lamports"], 7_530_000)
        self.assertEqual(row["rent_principal_lamports"], 10_801_920)
        self.assertEqual(row["persistent_reserve_lamports"], 18_481_920)
        self.assertEqual(row["payer_cold_outlay_lamports"], 18_711_920)

    def test_batched_folds_are_admitted_and_collapse_the_cold_outlay(self) -> None:
        derived = policy.derive(self.evidence)
        row = derived["resolution_work_batched"]
        self.assertEqual(row["status"], "PASS")
        self.assertEqual(row["maximum_admitted_batch"], 12)
        self.assertEqual(row["routes"]["fold_batch_12"]["measured_cu"], 929_573)
        self.assertEqual(row["routes"]["fold_batch_12"]["selected_limit_cu"], 1_170_000)
        self.assertEqual(
            row["routes"]["fold_batch_12"]["keeper_reward_lamports"], 1_280_000
        )
        self.assertEqual(row["fewest_transaction_plan"], [12, 12, 8])
        self.assertEqual(row["fold_transactions"], 3)
        self.assertEqual(row["fold_path_lamports"], 3_510_000)
        self.assertEqual(row["payer_cold_outlay_lamports"], 14_861_920)
        # Batching collapses the 32-transaction fixed overhead, never the
        # rent or the terminal quotes: both paths share one Begin and rent.
        per_transaction = derived["resolution_work"]
        self.assertLess(
            row["payer_cold_outlay_lamports"],
            per_transaction["payer_cold_outlay_lamports"],
        )
        self.assertEqual(
            row["rent_principal_lamports"],
            per_transaction["rent_principal_lamports"],
        )
        # A batch measured over the raw admission bound loses its quote and
        # drops out of the plan instead of being clamped.
        tampered = copy.deepcopy(self.evidence)
        tampered["measurements"]["resolution_work_batch"]["fold_batch_12_cu"] = [
            1_120_001
        ]
        stopped = policy.derive(tampered)["resolution_work_batched"]
        self.assertEqual(stopped["routes"]["fold_batch_12"]["status"], "STOP_HEADROOM")
        self.assertIsNone(stopped["routes"]["fold_batch_12"]["keeper_reward_lamports"])
        self.assertEqual(stopped["maximum_admitted_batch"], 8)
        self.assertEqual(stopped["fewest_transaction_plan"], [8, 8, 8, 8])

    def test_runtime_schedule_underfunding_is_rejected(self) -> None:
        finalize = policy.derive(self.evidence)["resolution_work"]["routes"]["finalize"]
        tampered = copy.deepcopy(self.evidence)
        tampered["resolution_work"]["runtime_reward_schedule"]["finalize_lamports"] = (
            finalize["keeper_reward_lamports"] - 1
        )
        with self.assertRaises(AdmissionError):
            policy.derive(tampered)

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
        self.assertEqual(row["measured_cu"], 226_444)
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

    def test_walk_families_are_sealed_evidence_but_never_promoted(self) -> None:
        """T2-6 walk CU evidence binds to the exact artifact and derives nothing.

        The general-epoch and clear-walk families are SBF-executed bank
        evidence for tags 49-53, sealed with their three logs; the projection
        records them as UNPROMOTED and derives no admission, quote, or reward
        row for any walk route — live flags untouched, the admission decision
        stays with ember.  Dropping the unpromoted declaration must refuse,
        and binding a walk family to a historical artifact must refuse.
        """

        derived = policy.derive(self.evidence)
        walk = derived["general_clearing_walk"]
        self.assertEqual(walk["status"], "SBF_EXECUTED_EVIDENCE_UNPROMOTED_STOP")
        self.assertFalse(walk["admission_rows_derived"])
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
                "terminal_closure",
            ],
        )
        for family in walk["measured_families"]:
            row = self.evidence["measurements"][family]
            self.assertEqual(
                row["admission"], "UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY", family
            )
            self.assertIn(family, policy.SAME_ELF_MEASUREMENTS)
        # The five general-clearing logs are sealed with the current root.
        artifact_root = self.evidence["artifact"]["path"].rsplit("/", 1)[0]
        for log in (
            "general_epoch",
            "clear_walk",
            "clear_lifecycle",
            "candidate_selection",
            "entitled_clearing",
        ):
            self.assertIn(
                f"{artifact_root}/logs/bank/{log}.log", self.evidence["evidence_files"]
            )
        # Exact measured pins from those logs.
        epoch = self.evidence["measurements"]["general_epoch"]
        self.assertEqual(epoch["init_epoch_cu"], [42_699])
        self.assertEqual(
            epoch["freeze_epoch_rows"][2],
            {"pages": 3, "orders": 40, "cu": [717_825, 717_825]},
        )
        walk_rows = self.evidence["measurements"]["clear_walk"]
        self.assertEqual(max(walk_rows["forty_order_pass1_cu"]), 391_428)
        self.assertEqual(walk_rows["complete_cu"], [122_865, 127_081])
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
        self.assertEqual(max(selection["seal_candidate_cu"]), 64_168)
        self.assertEqual(selection["seal_candidate_displacing_cu"], [64_168])
        self.assertEqual(
            {row["shape"]: row["cu"] for row in selection["finalize_selection_rows"]},
            {
                "3_retained_2_verified_selects_winner": [49_228],
                "2_verified_beyond_128_bit_digest_tie": [39_465],
                "0_verified_honest_lapse": [20_693],
            },
        )
        self.assertEqual(selection["test_result"], "PASS_5_OF_5")
        entitled = self.evidence["measurements"]["entitled_clearing"]
        self.assertEqual(entitled["freeze_entitlement_cu"], [100_158])
        self.assertEqual(entitled["entitle_slice_single_cu"], [210_607])
        self.assertEqual(entitled["entitle_slice_portfolio_pair_cu"], [243_518])
        self.assertEqual(entitled["settle_page_entitled_direct_slice_cu"], [53_330])
        self.assertEqual(
            entitled["settle_page_entitled_portfolio_full_pair_cu"], [234_735]
        )
        self.assertEqual(
            entitled["bank_conservation"],
            "POSITIONS_BYTE_EQUAL_IMPLIED_ALLOCATION_TOTAL_CASH_AND_EGGS_EXACT",
        )
        self.assertEqual(entitled["test_result"], "PASS_4_OF_4")
        derived = policy.derive(self.evidence)
        for subsystem in derived.values():
            if isinstance(subsystem, dict) and "routes" in subsystem:
                for name in subsystem["routes"]:
                    self.assertNotIn("entitle", name)
                    self.assertNotIn("candidate", name)
        for family in ("candidate_selection", "entitled_clearing"):
            tampered = copy.deepcopy(self.evidence)
            del tampered["measurements"][family]["admission"]
            with self.assertRaises(policy.CheckError):
                policy.derive(tampered)

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
        self.assertEqual(rows["init_epoch_cu"], [41_226] * 3)
        self.assertEqual(rows["init_order_page_cu"], [221_020] * 3)
        self.assertEqual(rows["begin_verification_cu"], [23_596] * 3)
        self.assertEqual(rows["verify_candidate_rows"][0]["cu"], [151_358] * 3)
        self.assertEqual(rows["abort_unfrozen_rows"][0]["cu"], [10_393] * 3)
        self.assertEqual(max(rows["freeze_epoch_cu"]), 382_784)
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
        self.assertNotEqual(row["relocated_cargo_home"], digest)
        self.assertTrue(row["relocated_disposition"].startswith("PATH_SENSITIVE"))
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
        lying = copy.deepcopy(self.evidence)
        lying["artifact_reproducibility"]["relocated_disposition"] = (
            "INDEPENDENT_BYTE_IDENTICAL_SINGLE_HOST"
        )
        with self.assertRaises(policy.CheckError) as caught:
            policy.check_artifact_binding(lying)
        self.assertIn("disagrees with its own digest", str(caught.exception))

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
        for subsystem in derived.values():
            if isinstance(subsystem, dict) and "routes" in subsystem:
                for name in subsystem["routes"]:
                    self.assertNotIn("close", name)
                    self.assertNotIn("release", name)

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
