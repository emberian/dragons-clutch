#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("plan-sbf-release-batch.py")
SPEC = importlib.util.spec_from_file_location("plan_sbf_release_batch", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class BatchPlanTests(unittest.TestCase):
    def test_dependency_closure_changes_only_rebuild_its_consumers(self) -> None:
        with tempfile.TemporaryDirectory() as before_text, tempfile.TemporaryDirectory() as after_text:
            before = Path(before_text)
            after = Path(after_text)
            paths = [
                "Cargo.toml",
                "Cargo.lock",
                "programs/alpha/Cargo.toml",
                "programs/alpha/src/lib.rs",
                "programs/beta/Cargo.toml",
                "programs/beta/src/lib.rs",
                "crates/shared/Cargo.toml",
                "crates/shared/src/lib.rs",
                "docs/unrelated.md",
            ]
            for root in (before, after):
                for path in paths:
                    target = root / path
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_text(path + "\n")
            (after / "crates/shared/src/lib.rs").write_text("changed shared source\n")
            (after / "docs/unrelated.md").write_text("changed unrelated docs\n")
            packages = {
                "alpha": {
                    "manifest_path": str(before / "programs/alpha/Cargo.toml"),
                    "dependencies": [{"name": "shared", "kind": None}],
                },
                "beta": {
                    "manifest_path": str(before / "programs/beta/Cargo.toml"),
                    "dependencies": [],
                },
                "shared": {
                    "manifest_path": str(before / "crates/shared/Cargo.toml"),
                    "dependencies": [],
                },
            }
            after_packages = {
                name: {
                    **value,
                    "manifest_path": value["manifest_path"].replace(
                        str(before), str(after)
                    ),
                }
                for name, value in packages.items()
            }
            alpha_before = set(MODULE.closure_paths(before, paths, packages, "alpha"))
            alpha_after = set(
                MODULE.closure_paths(after, paths, after_packages, "alpha")
            )
            beta_before = set(MODULE.closure_paths(before, paths, packages, "beta"))
            beta_after = set(MODULE.closure_paths(after, paths, after_packages, "beta"))
            self.assertEqual(
                MODULE.changed_paths(before, after, alpha_before, alpha_after),
                ["crates/shared/src/lib.rs"],
            )
            self.assertEqual(
                MODULE.changed_paths(before, after, beta_before, beta_after), []
            )
            self.assertNotIn("docs/unrelated.md", alpha_after)

    def test_dev_and_external_dependencies_do_not_pollute_sbf_closure(self) -> None:
        packages = {
            "program": {
                "dependencies": [
                    {"name": "runtime", "kind": None},
                    {"name": "test-only", "kind": "dev"},
                    {"name": "external", "kind": None},
                ]
            },
            "runtime": {"dependencies": []},
            "test-only": {"dependencies": []},
        }
        self.assertEqual(
            MODULE.dependency_closure(packages, "program"), {"program", "runtime"}
        )

    def test_inventory_refuses_anything_other_than_exact_thirteen(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            for index in range(12):
                manifest = root / f"programs/program-{index}/Cargo.toml"
                manifest.parent.mkdir(parents=True)
                manifest.write_text("[package]\n")
            with self.assertRaisesRegex(MODULE.Refusal, "exact 13-link"):
                MODULE.link_inventory(root)


if __name__ == "__main__":
    unittest.main()
