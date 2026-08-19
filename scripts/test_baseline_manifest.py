"""Focused declaration tests for the schema-v2 baseline manifest generator."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest import mock


SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))
import baseline_manifest  # noqa: E402


class BaselineManifestDeclarationTests(unittest.TestCase):
    def test_gate_inventory_is_deterministic_and_has_unique_ids(self) -> None:
        first = baseline_manifest.build_gates()
        self.assertEqual(first, baseline_manifest.build_gates())
        ids = [gate["id"] for gate in first]
        self.assertEqual(len(ids), len(set(ids)))

    def test_current_research_and_frontend_gates_are_exactly_declared(self) -> None:
        gates = {gate["id"]: gate for gate in baseline_manifest.build_gates()}
        expected_commands = {
            "cargo_test.batch_policy_identity": (
                "cargo test --manifest-path research/batch-policy-identity/Cargo.toml "
                "--locked --offline --all-targets"
            ),
            "cargo_test.bspline_shape_compiler": (
                "cargo test --manifest-path research/bspline-shape-compiler/Cargo.toml "
                "--offline --locked"
            ),
            "cargo_test.resolution_work_v1": (
                "cargo test --manifest-path research/resolution-work-v1/Cargo.toml "
                "--offline --locked"
            ),
            "cargo_test.source_profile_v1": (
                "cargo test --manifest-path research/source-profile-v1/Cargo.toml "
                "--offline --locked"
            ),
            "cargo_test.failure_payout_v1": (
                "cargo test --manifest-path research/failure-payout-v1/Cargo.toml "
                "--offline --locked"
            ),
            "cargo_clippy.failure_payout_v1": (
                "cargo clippy --manifest-path research/failure-payout-v1/Cargo.toml "
                "--offline --locked --all-targets --all-features -- -D warnings"
            ),
            "cargo_test.terminal_economics_r4": (
                "cargo test --manifest-path research/terminal-economics-r4/Cargo.toml "
                "--offline --locked"
            ),
            "cargo_clippy.terminal_economics_r4": (
                "cargo clippy --manifest-path research/terminal-economics-r4/Cargo.toml "
                "--offline --locked --all-targets --all-features -- -D warnings"
            ),
            "python.liveness_policy_profile_current_seal": (
                "python3 research/liveness-policy-profile/policy.py --check-current"
            ),
            "proof.batch_scalar_shadow": "sh verus/batch/run_batch_proofs.sh",
            "proof.transfer_arithmetic_refinement": (
                "sh verus/kernel/run_transfer_refinement.sh"
            ),
            "proof.bspline_finite_refinement": "sh verus/bspline/run_bspline_refinement.sh",
            "lean.model_build": "(cd lean && lake build)",
            "sbf.token2022_program_test_non_production_mock": (
                "programs/clutch-sbf/svm-tests/run_svm_tests.sh "
                "--non-production-mock-source"
            ),
            "sbf.committed_signed_walk": "programs/clutch-sbf/scripts/run_committed.sh",
            "static_client.npm": "(cd apps/static-client && npm test && npm run check)",
        }
        for gate_id, command in expected_commands.items():
            self.assertEqual(gates[gate_id]["command"], command)
            self.assertEqual(
                gates[gate_id]["section"],
                "current-baseline" if gate_id == "static_client.npm" else (
                    "current-runtime"
                    if gate_id
                    in {
                        "sbf.token2022_program_test_non_production_mock",
                        "sbf.committed_signed_walk",
                    }
                    else "current-proof-boundary"
                    if gate_id.startswith("proof.") or gate_id == "lean.model_build"
                    else "current-research"
                ),
            )

        record = baseline_manifest.gate_manifest_record(gates["cargo_test.batch_policy_identity"])
        self.assertEqual(
            set(record),
            {"id", "section", "command", "cwd", "shell", "expected", "key_patterns", "note"},
        )

        for gate_id, count in (
            ("cargo_test.source_profile_v1", 32),
            ("cargo_test.failure_payout_v1", 18),
            ("cargo_test.terminal_economics_r4", 16),
        ):
            expected_patterns = baseline_manifest.counted_cargo_test_patterns(count)
            self.assertEqual(gates[gate_id]["key_patterns"], expected_patterns)
            self.assertEqual(
                gates[gate_id]["expected"]["required_output_patterns"],
                expected_patterns,
            )

        batch = gates["proof.batch_scalar_shadow"]
        self.assertIn(
            r"^verification results:: 28 verified, 0 errors$",
            batch["expected"]["required_output_patterns"],
        )
        self.assertEqual(
            sum(
                "status=EXPECTED_RED" in pattern
                for pattern in batch["expected"]["required_output_patterns"]
            ),
            5,
        )

    def test_gate_records_are_cache_and_path_stable(self) -> None:
        cold_doc = " Documenting clutch-kernel v0.1.0 (/private/tmp/cold/target/doc)\n"
        warm_doc = ""
        self.assertEqual(
            baseline_manifest.extract_key_lines(cold_doc, baseline_manifest.DOC_PATTERNS),
            baseline_manifest.extract_key_lines(warm_doc, baseline_manifest.DOC_PATTERNS),
        )

        cold = (
            "Running unittests src/lib.rs (/private/tmp/cold/target/debug/deps/x)\n"
            "test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; "
            "finished in 0.12s\n"
        )
        warm = (
            "Running unittests src/lib.rs (/different/machine/warm/target/debug/deps/x)\n"
            "test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; "
            "finished in 0.00s\n"
        )
        self.assertEqual(
            baseline_manifest.extract_key_lines(cold, baseline_manifest.TEST_RESULT_PATTERNS),
            baseline_manifest.extract_key_lines(warm, baseline_manifest.TEST_RESULT_PATTERNS),
        )

        record = baseline_manifest.run_gates(
            SCRIPTS.parent,
            [
                {
                    "id": "fixture.stable_record",
                    "command": (
                        "printf 'Documenting temporary target\\n'; "
                        "printf 'volatile=/private/tmp/host-artifact\\n'; "
                        "printf 'test result: ok; finished in 0.01s\\n'"
                    ),
                    "expected": {"mode": "zero", "exit": 0},
                    "key_patterns": baseline_manifest.TEST_RESULT_PATTERNS,
                    "volatile_patterns": [r"^volatile="],
                }
            ],
            timeout=2,
        )["fixture.stable_record"]
        self.assertNotIn("output_bytes", record)
        self.assertNotIn("tail", record)
        self.assertNotIn("volatile_lines", record)
        self.assertEqual(record["key_lines"], ["test result: ok"])

    def test_exact_verus_disposition_rejects_setup_refusals(self) -> None:
        expected = {"mode": "exact", "exit": 1}
        self.assertTrue(baseline_manifest.gate_outcome_ok(expected, 1))
        for setup_or_drift_exit in (0, 2, 3, 4):
            self.assertFalse(
                baseline_manifest.gate_outcome_ok(expected, setup_or_drift_exit)
            )

        probe = {
            gate["id"]: gate for gate in baseline_manifest.build_gates()
        }["proof.verus_probe"]
        self.assertEqual(probe["expected"]["mode"], "exact")
        self.assertEqual(probe["expected"]["exit"], 1)
        self.assertIn("Exit 2", probe["expected"]["reason"])
        self.assertFalse(
            baseline_manifest.gate_outcome_ok(probe["expected"], 1, "error: unrelated")
        )
        self.assertTrue(
            baseline_manifest.gate_outcome_ok(
                probe["expected"],
                1,
                "error: Error: The verus_builtin crate was not imported but it is necessary",
            )
        )

        counted = {
            "mode": "zero",
            "exit": 0,
            "required_output_patterns": baseline_manifest.counted_cargo_test_patterns(2),
        }
        self.assertTrue(
            baseline_manifest.gate_outcome_ok(
                counted,
                0,
                "running 2 tests\n"
                "test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; "
                "0 filtered out; finished in 0.01s\n",
            )
        )
        self.assertFalse(baseline_manifest.gate_outcome_ok(counted, 0, "running 0 tests\n"))

    def test_timeout_kills_the_complete_gate_process_group(self) -> None:
        class TimedOutProcess:
            pid = 4242
            returncode = -9

            def communicate(self, timeout: int | None = None) -> tuple[str, None]:
                if timeout is not None:
                    raise baseline_manifest.subprocess.TimeoutExpired("fixture", timeout)
                return "timed out\n", None

        process = TimedOutProcess()
        with mock.patch.object(baseline_manifest.subprocess, "Popen", return_value=process) as popen:
            with mock.patch.object(baseline_manifest.os, "killpg") as killpg:
                result = baseline_manifest.run_gates(
                    SCRIPTS.parent,
                    [
                        {
                            "id": "fixture.timeout",
                            "command": "ignored",
                            "expected": {"mode": "zero", "exit": 0},
                            "key_patterns": [],
                        }
                    ],
                    timeout=1,
                )["fixture.timeout"]
        self.assertTrue(result["timed_out"])
        self.assertFalse(result["matches_expectation"])
        killpg.assert_called_once_with(4242, baseline_manifest.signal.SIGKILL)
        self.assertTrue(popen.call_args.kwargs["start_new_session"])

    def test_successful_exact_gate_is_not_described_as_a_contradiction(self) -> None:
        gate = {
            "id": "fixture.exact",
            "command": "fixture",
            "expected": {"mode": "exact", "exit": 0},
        }
        rows = baseline_manifest.summarize_unavailable(
            [gate],
            {
                "fixture.exact": {
                    "exit_code": 0,
                    "matches_expectation": True,
                }
            },
        )
        self.assertEqual(len(rows), 1)
        self.assertTrue(rows[0]["matches_expectation"])
        self.assertNotIn("contradicted", rows[0]["reason"])
        self.assertIn("matches its declaration", rows[0]["reason"])

    def test_current_runtime_and_terms_authorities_are_declared(self) -> None:
        gates = {gate["id"]: gate for gate in baseline_manifest.build_gates()}
        self.assertEqual(len(gates), 100)
        bringup_patterns = gates["sbf.runtime_bringup"]["key_patterns"]
        for pattern in (
            r"^default pass [12]  sha256=[0-9a-f]{64}  bytes=[0-9]+$",
            r"^NON-PRODUCTION mock pass [12]  sha256=[0-9a-f]{64}  bytes=[0-9]+$",
            r"^default_sbf_elf_sha256=[0-9a-f]{64}$",
            r"^non_production_mock_sbf_elf_sha256=[0-9a-f]{64}$",
            r"^default_reproducibility=PASS$",
            r"^mock_reproducibility=PASS$",
            r"^source_campaign default-endow=REFUSE Custom\(0x0079\); lifecycle=NOT_DECLARED$",
            r"^source_campaign NON-PRODUCTION endow=EXPECTED_SUCCESS; lifecycle=EXPECTED_SUCCESS$",
        ):
            self.assertIn(pattern, bringup_patterns)
        self.assertEqual(gates["sbf.runtime_bringup"]["expected"]["mode"], "exact")
        self.assertIn(
            r"^test default_elf_refuses_endow_without_a_registered_source_release \.\.\. ok$",
            gates["sbf.token2022_program_test"]["expected"]["required_output_patterns"],
        )
        for gate_id in (
            "sbf.token2022_program_test",
            "sbf.token2022_program_test_non_production_mock",
        ):
            patterns = gates[gate_id]["key_patterns"]
            self.assertNotIn(r"^test [a-zA-Z0-9_]+ .*", patterns)
            self.assertNotIn(r"^SVM ", patterns)
            self.assertIn(r"^running [0-9]+ tests?$", patterns)
            self.assertIn(r"^test result: ", patterns)

        outputs = {item["id"]: item for item in baseline_manifest.DECLARED_BUILD_OUTPUTS}
        self.assertEqual(outputs["clutch_sbf.default_program_elf"]["handoff"], None)
        self.assertEqual(outputs["clutch_sbf.non_production_mock_program_elf"]["handoff"], None)
        self.assertEqual(
            baseline_manifest.DERIVED_DIGESTS[0]["handoff"],
            "62b06b2107636686648507e4f9ecd8a4d90733dcebf81177d4a63b25bc698d02",
        )

        signed_walk = gates["sbf.committed_signed_walk"]
        self.assertIn(
            r"^source_profile=NON-PRODUCTION-non-production-mock-source$",
            signed_walk["key_patterns"],
        )
        falsifier_pattern = (
            r"^  red: committed-.*committed bytes differ(?: \(observed .*, expected .*\))?$"
        )
        self.assertIn(falsifier_pattern, signed_walk["key_patterns"])
        self.assertIsNotNone(
            baseline_manifest.re.search(
                falsifier_pattern,
                "  red: committed-22-withdraw-second-owner-cash.committed-market."
                "hoard-token: committed bytes differ (observed 01, expected 00)",
            )
        )
        for pattern in (
            r"^committed_signed_transactions=22$",
            r"^committed_expected_refusals=1$",
            r"^committed_compute_exhaustions=1$",
            r"^committed_watched_accounts=18$",
            r"^genesis_assisted_program_accounts=12$",
            r"^falsifiability=PASS$",
        ):
            self.assertIn(pattern, signed_walk["key_patterns"])

        file_digests = {entry[0] for entry in baseline_manifest.FILE_DIGESTS}
        self.assertTrue(
            {
                "locks.clutch_sbf_committed_harness",
                "clutch_sbf.committed_harness_source",
                "clutch_sbf.committed_walk_runner",
                "locks.source_profile_v1",
                "locks.failure_payout_v1",
                "locks.terminal_economics_r4",
                "proof_shadow.verus_batch_runner",
            }.issubset(file_digests)
        )

    def test_non_attestations_keep_current_boundaries_explicit(self) -> None:
        joined = "\n".join(baseline_manifest.NOT_ATTESTED)
        self.assertNotIn("in flight", joined)
        self.assertIn("scalar mathematical shadow", joined)
        self.assertIn("sealed local R1 artifact admits measured ResolutionWork routes", joined)
        self.assertIn("registered source release", joined)
        self.assertIn("1,400,000-CU transaction limit", joined)
        self.assertIn("no terminal closure", joined)


if __name__ == "__main__":
    unittest.main()
