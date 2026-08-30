#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("final-generated-convergence.py")
SPEC = importlib.util.spec_from_file_location("final_generated_convergence", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class FinalGeneratedConvergenceTests(unittest.TestCase):
    def test_output_owner_allowlist_is_narrow(self) -> None:
        workspace_locks = {"Cargo.lock", "tools/example/Cargo.lock"}
        for path in (
            "Cargo.lock",
            "tools/example/Cargo.lock",
            "apps/dclutch-web/lib/generated/example.ts",
            "packages/dclutch-sdk/lib/generated/example.ts",
            "docs/reference/routes.md",
            "tools/sbom/SBOM.md",
            "tools/sbom/NOTICES.md",
        ):
            self.assertTrue(MODULE.allowed_output(path, workspace_locks), path)
        for path in (
            "Cargo.toml",
            "tools/stray/Cargo.lock",
            "apps/dclutch-web/lib/directHotChain.ts",
            "docs/VALIDATION_BACKLOG.md",
            "tools/sbom/README.md",
            "programs/dclutch-trading-sbf/src/lib.rs",
        ):
            self.assertFalse(MODULE.allowed_output(path, workspace_locks), path)

    def test_workspace_discovery_requires_adjacent_tracked_lock(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            (root / "Cargo.toml").write_text("[workspace]\nmembers = []\n")
            (root / "Cargo.lock").write_text("version = 4\n")
            self.assertEqual(
                MODULE.discover_workspaces(root, ["Cargo.toml"], {"Cargo.lock"}),
                [("Cargo.toml", "Cargo.lock")],
            )
            with self.assertRaisesRegex(MODULE.Refusal, "lacks its tracked adjacent"):
                MODULE.discover_workspaces(root, ["Cargo.toml"], set())

    def test_abi_inventory_refuses_writer_without_verifier(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            for relative in MODULE.PACKAGE_ROOTS:
                package = root / relative / "package.json"
                package.parent.mkdir(parents=True)
                package.write_text(
                    json.dumps(
                        {
                            "scripts": {
                                "abi:example": "node generate.mjs",
                                "abi:example:verify": "node generate.mjs --check",
                                "abi:coverage": "node coverage.mjs",
                            }
                        }
                    )
                )
            writers, verifiers = MODULE.abi_tasks(root)
            self.assertEqual(len(writers), 2)
            self.assertEqual(len(verifiers), 2)
            first = root / MODULE.PACKAGE_ROOTS[0] / "package.json"
            first.write_text(
                json.dumps({"scripts": {"abi:example": "node generate.mjs"}})
            )
            with self.assertRaisesRegex(MODULE.Refusal, "writer/verifier mismatch"):
                MODULE.abi_tasks(root)


if __name__ == "__main__":
    unittest.main()
