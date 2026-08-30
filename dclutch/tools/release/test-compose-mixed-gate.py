#!/usr/bin/env python3
"""Hostiles for bounded multi-link mixed-gate selection."""

import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest


HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
SPEC = importlib.util.spec_from_file_location("compose_mixed_gate", HERE / "compose-mixed-gate.py")
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def plan(changed):
    rows = []
    for label, package, artifact_stem in MODULE.SHIPPED_LINKS:
        changed_row = label in changed
        rows.append({
            "label": label,
            "package": package,
            "artifact_stem": artifact_stem,
            "base_input_digest": "1" * 64,
            "candidate_input_digest": "2" * 64 if changed_row else "1" * 64,
            "requires_new_artifact": changed_row,
            "changed_inputs": [f"programs/{label}/changed.rs"] if changed_row else [],
            "consumers": ["fixture"],
        })
    return {
        "schema": MODULE.PLAN_SCHEMA,
        "base_revision": "a" * 40,
        "base_source_tree_sha256": "b" * 64,
        "candidate_revision": "c" * 40,
        "candidate_source_tree_sha256": "d" * 64,
        "link_count": len(MODULE.SHIPPED_LINKS),
        "changed_link_count": len(changed),
        "links": rows,
        "qualification": "fixture",
    }


class MixedGateSelectionTests(unittest.TestCase):
    def load(self, value, labels):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "plan.json"
            path.write_text(json.dumps(value))
            return MODULE.load_plan(path, labels)

    def test_three_rebuilt_labels_are_canonical(self):
        labels = MODULE.rebuilt_labels(["core", "resolution", "trading"])
        self.assertEqual(labels, ("core", "resolution", "trading"))
        self.load(plan(set(labels)), labels)

    def test_duplicate_rebuilt_label_refuses(self):
        with self.assertRaisesRegex(MODULE.Refusal, "must not repeat"):
            MODULE.rebuilt_labels(["core", "core"])

    def test_missing_rebuilt_label_refuses(self):
        labels = MODULE.rebuilt_labels(["core", "resolution", "trading"])
        with self.assertRaisesRegex(MODULE.Refusal, "requested rebuilt count"):
            self.load(plan({"core", "resolution"}), labels)

    def test_extra_rebuilt_label_refuses(self):
        labels = MODULE.rebuilt_labels(["core", "resolution", "trading"])
        with self.assertRaisesRegex(MODULE.Refusal, "requested rebuilt count"):
            self.load(plan({"core", "resolution", "trading", "claims"}), labels)


if __name__ == "__main__":
    unittest.main()
