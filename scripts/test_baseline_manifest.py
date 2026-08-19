"""Focused declaration tests for the schema-v2 baseline manifest generator."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path


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
            "python.liveness_policy_profile_current_seal": (
                "python3 research/liveness-policy-profile/policy.py --check-current"
            ),
            "static_client.npm": "(cd apps/static-client && npm test && npm run check)",
        }
        for gate_id, command in expected_commands.items():
            self.assertEqual(gates[gate_id]["command"], command)
            self.assertEqual(gates[gate_id]["section"], "post-5-research" if gate_id != "static_client.npm" else "5")

        record = baseline_manifest.gate_manifest_record(gates["cargo_test.batch_policy_identity"])
        self.assertEqual(
            set(record),
            {"id", "section", "command", "cwd", "shell", "expected", "key_patterns", "note"},
        )

    def test_non_attestations_keep_current_boundaries_explicit(self) -> None:
        joined = "\n".join(baseline_manifest.NOT_ATTESTED)
        self.assertIn("isolated Verus batch shadow is in flight", joined)
        self.assertIn("sealed local R1 artifact admits measured ResolutionWork routes", joined)
        self.assertIn("registered source release", joined)
        self.assertIn("1,400,000-CU transaction limit", joined)
        self.assertIn("no terminal closure", joined)


if __name__ == "__main__":
    unittest.main()
