"""CU witness verdicts keep regression thresholds distinct from chain allowance."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO = Path(__file__).resolve().parents[3]
CHECK = REPO / "tools" / "gauntlet" / "tier1" / "check-witnesses.sh"
CHAIN_ALLOWANCE = 1_400_000


@unittest.skipUnless(shutil.which("jq"), "jq is required by check-witnesses.sh")
class ComputeWitnessTests(unittest.TestCase):
    def run_case(
        self,
        *,
        measured: int,
        tolerance: int,
        observed: int,
        error: object | None = None,
        scope: str = "transaction",
        logs: list[str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            budget = root / "budgets.json"
            witnesses = root / "witnesses.json"
            evidence = root / "evidence.json"
            plan = root / "plan.json"
            budget.write_text(
                json.dumps(
                    {
                        "schema": "dclutch-cu-budgets-v1",
                        "ceiling": {"compute_units": CHAIN_ALLOWANCE},
                        "budgets": [
                            {
                                "id": "case",
                                "campaign": "case",
                                "scope": scope,
                                "transaction": "measured transaction",
                                "measured": measured,
                                "tolerance": tolerance,
                                "budget": measured + tolerance,
                                "enforced": True,
                                "provenance": "focused evaluator test",
                                **(
                                    {"stage": {"index": 1, "name": "inner stage"}}
                                    if scope == "stage"
                                    else {}
                                ),
                            }
                        ],
                    }
                )
            )
            witnesses.write_text(
                json.dumps(
                    {
                        "witnesses": [
                            {
                                "id": "cu",
                                "kind": "cu-budget",
                                "campaign": "case",
                                "provenance": "focused evaluator test",
                            }
                        ]
                    }
                )
            )
            evidence.write_text(
                json.dumps(
                    {
                        "transactions": [
                            {
                                "label": "measured transaction",
                                "compute_units_consumed": observed,
                                "error": error,
                                "logs": logs or [],
                            }
                        ]
                    }
                )
            )
            plan.write_text("{}")
            environment = os.environ.copy()
            environment["DCLUTCH_CU_BUDGETS_OVERRIDE"] = str(budget)
            return subprocess.run(
                [str(CHECK), str(witnesses), str(evidence), str(plan)],
                cwd=REPO,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )

    def test_fitting_draw_is_green_when_tolerance_crosses_chain_allowance(self):
        result = self.run_case(
            measured=1_395_000,
            tolerance=10_000,
            observed=1_395_000,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("OK", result.stdout)
        self.assertIn("1405000", result.stdout)
        self.assertIn("1400000", result.stdout)

    def test_regression_overage_is_red_while_chain_headroom_remains(self):
        result = self.run_case(measured=100, tolerance=20, observed=130)
        output = result.stdout + result.stderr
        self.assertNotEqual(result.returncode, 0, output)
        self.assertIn("OVER", output)
        self.assertIn("regression threshold exceeded by 10 CU", output)
        self.assertIn("1399870 CU remained under the chain allowance", output)

    def test_finalized_compute_exhaustion_has_its_own_verdict(self):
        result = self.run_case(
            measured=1_395_000,
            tolerance=10_000,
            observed=CHAIN_ALLOWANCE,
            error={"InstructionError": [1, "ComputationalBudgetExceeded"]},
        )
        output = result.stdout + result.stderr
        self.assertNotEqual(result.returncode, 0, output)
        self.assertIn("EXHAUSTED", output)
        self.assertIn("records compute exhaustion", output)

    def test_stage_regression_still_uses_depth_two_consumption(self):
        result = self.run_case(
            measured=100,
            tolerance=20,
            observed=500,
            scope="stage",
            logs=[
                "Program 11111111111111111111111111111111 invoke [1]",
                "Program 22222222222222222222222222222222 invoke [2]",
                "Program 22222222222222222222222222222222 consumed 130 of 1400000 compute units",
                "Program 22222222222222222222222222222222 success",
                "Program 11111111111111111111111111111111 consumed 500 of 1400000 compute units",
                "Program 11111111111111111111111111111111 success",
            ],
        )
        output = result.stdout + result.stderr
        self.assertNotEqual(result.returncode, 0, output)
        self.assertIn("OVER", output)
        self.assertIn("regression threshold exceeded by 10 CU", output)


if __name__ == "__main__":
    unittest.main()
