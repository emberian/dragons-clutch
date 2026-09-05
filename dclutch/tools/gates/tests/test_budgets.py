"""The budget register's shape rules, each shown capable of refusing."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from gates import budgets  # noqa: E402


def document(**entry) -> dict:
    row = {"id": "row", "campaign": "tier1", "scope": "transaction", "transaction": "t", "measured": 100,
           "tolerance": 20, "budget": 120, "enforced": True, "provenance": "p"}
    row.update(entry)
    return {"schema": "dclutch-cu-budgets-v1", "ceiling": {"compute_units": budgets.CEILING}, "budgets": [row]}


class BudgetTests(unittest.TestCase):
    def test_a_well_formed_register_has_no_problems(self):
        self.assertEqual(budgets.problems(document(), {"tier1"}), [])

    def test_budget_must_be_measured_plus_tolerance(self):
        self.assertTrue(any("measured+tolerance" in p for p in budgets.problems(document(budget=121), {"tier1"})))

    def test_a_budget_above_the_ceiling_is_refused(self):
        found = budgets.problems(document(measured=budgets.CEILING, tolerance=1, budget=budgets.CEILING + 1), {"tier1"})
        self.assertTrue(any("ABOVE" in p for p in found))

    def test_scope_and_stage_shape(self):
        self.assertTrue(any("scope" in p for p in budgets.problems(document(scope="phase"), {"tier1"})))
        self.assertTrue(any("stage.index" in p for p in budgets.problems(document(scope="stage", stage={}), {"tier1"})))
        self.assertEqual(budgets.problems(document(scope="stage", stage={"index": 1, "name": "s"}), {"tier1"}), [])

    def test_unknown_campaign_only_matters_when_enforced(self):
        self.assertTrue(any("no bindings file" in p for p in budgets.problems(document(campaign="ghost"), set())))
        self.assertEqual(budgets.problems(document(campaign="ghost", enforced=False, unenforced_reason="recorded only"), set()), [])
        self.assertTrue(any("unenforced_reason" in p for p in budgets.problems(document(enforced=False), {"tier1"})))

    def test_duplicate_ids_are_refused(self):
        doc = document()
        doc["budgets"].append(dict(doc["budgets"][0]))
        self.assertTrue(any("duplicated" in p for p in budgets.problems(doc, {"tier1"})))


if __name__ == "__main__":
    unittest.main()
