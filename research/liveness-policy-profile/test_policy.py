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
        # The Fold(1) route covers its widest observation, the 88,433-CU
        # singleton measured inside the two-fold batch scenario.
        self.assertEqual(row["routes"]["fold_1"]["measured_cu"], 88_433)
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
        self.assertEqual(row["routes"]["fold_batch_12"]["measured_cu"], 929_561)
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
        self.assertEqual(row["measured_cu"], 226_445)
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
            ["general_epoch", "clear_walk", "candidate_selection", "entitled_clearing"],
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
        self.assertEqual(epoch["init_epoch_cu"], [42_557])
        self.assertEqual(
            epoch["freeze_epoch_rows"][2],
            {"pages": 3, "orders": 40, "cu": [717_829, 717_829]},
        )
        walk_rows = self.evidence["measurements"]["clear_walk"]
        self.assertEqual(max(walk_rows["forty_order_pass1_cu"]), 400_428)
        self.assertEqual(walk_rows["complete_cu"], [122_869, 127_085])
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
        self.assertEqual(max(selection["seal_candidate_cu"]), 64_170)
        self.assertEqual(selection["seal_candidate_displacing_cu"], [64_170])
        self.assertEqual(
            {row["shape"]: row["cu"] for row in selection["finalize_selection_rows"]},
            {
                "3_retained_2_verified_selects_winner": [49_230],
                "2_verified_beyond_128_bit_digest_tie": [39_462],
                "0_verified_honest_lapse": [20_695],
            },
        )
        self.assertEqual(selection["test_result"], "PASS_5_OF_5")
        entitled = self.evidence["measurements"]["entitled_clearing"]
        self.assertEqual(entitled["freeze_entitlement_cu"], [100_052])
        self.assertEqual(entitled["entitle_slice_single_cu"], [204_577])
        self.assertEqual(entitled["entitle_slice_portfolio_pair_cu"], [246_173])
        self.assertEqual(entitled["settle_page_entitled_direct_slice_cu"], [54_834])
        self.assertEqual(
            entitled["settle_page_entitled_portfolio_full_pair_cu"], [225_739]
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

    def test_reproducibility_probes_are_pinned_with_their_dispositions(self) -> None:
        """The build-path amendment's probe rows are sealed exactly.

        The canonical identity is the in-place double build; the cross-path
        worktree build is recorded under the PATH_TIED_SYMBOL_ORDER
        disposition (its observed digest listed, byte-identical at this
        seal), and the relocated-Cargo-home probe returned byte-identical.
        """

        row = self.evidence["artifact_reproducibility"]
        digest = self.evidence["artifact"]["sha256"]
        self.assertEqual(row["normal_build_1"], digest)
        self.assertEqual(row["normal_build_2"], digest)
        self.assertEqual(row["cross_path_build"], digest)
        self.assertEqual(row["cross_path_disposition"], "PATH_TIED_SYMBOL_ORDER")
        self.assertEqual(row["relocated_cargo_home"], digest)
        self.assertEqual(
            row["relocated_disposition"], "INDEPENDENT_BYTE_IDENTICAL_SINGLE_HOST"
        )
        artifact_root = self.evidence["artifact"]["path"].rsplit("/", 1)[0]
        self.assertIn(
            f"{artifact_root}/logs/sbf-build-crosspath.log",
            self.evidence["evidence_files"],
        )

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
