#!/usr/bin/env python3
"""Hostiles for the bounded mixed-gate-to-Hot-CU projection bridge."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


HERE = Path(__file__).resolve().parent
ADAPTER = HERE / "mixed-gate-selection.py"
ROLES = ("registry", "trading", "core", "claims", "custody")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class MixedGateSelectionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="dclutch-hot-cu-mixed-test-")
        self.root = Path(self.temporary.name).resolve()
        self.repo = self.root / "repo"
        verifier = self.repo / "tools" / "release" / "compose-mixed-gate.py"
        verifier.parent.mkdir(parents=True)
        verifier.write_text(
            """#!/usr/bin/env python3
import argparse
from pathlib import Path
import shutil

parser = argparse.ArgumentParser()
parser.add_argument("command")
parser.add_argument("--gate")
parser.add_argument("--expected-gate-sha256")
parser.add_argument("--expected-source-revision")
parser.add_argument("--expected-source-tree-sha256")
parser.add_argument("--selected-link")
parser.add_argument("--output")
args = parser.parse_args()
if args.command != "verify" or args.selected_link != "trading":
    raise SystemExit(1)
shutil.copyfile(Path(__file__).with_name("projection.json"), args.output)
"""
        )

        self.gate_root = self.root / "gate"
        (self.gate_root / "elf").mkdir(parents=True)
        self.gate = self.gate_root / "CHECKED_UPGRADE_GATE.json"
        self.gate.write_text('{"fixture":"gate is opaque to the adapter"}\n')
        for role in ROLES:
            (self.gate_root / "elf" / f"{role}.so").write_bytes(
                b"\x7fELF" + role.encode("ascii")
            )
        trading = self.gate_root / "elf" / "trading.so"
        self.selection = {
            "schema": "dclutch-checked-mixed-gate-link-selection-v1",
            "gate_path": str(self.gate),
            "gate_sha256": digest(self.gate),
            "source_revision": "a" * 40,
            "source_tree_sha256": "b" * 64,
            "solana_cli_version": "solana-cli fixture",
            "label": "trading",
            "package": "dclutch-trading-sbf",
            "disposition": "carry-forward",
            "artifact_source_revision": "c" * 40,
            "artifact_source_tree_sha256": "d" * 64,
            "artifact_build_run_id": "e" * 64,
            "artifact_provenance": {
                "bytes": 1,
                "canonical_path": "provenance/trading.json",
                "sha256": "f" * 64,
            },
            "elf": {
                "bytes": trading.stat().st_size,
                "canonical_path": "elf/trading.so",
                "sha256": digest(trading),
            },
            "checked_manifest": {
                "bytes": 1,
                "canonical_path": "evidence/trading/checked.bin",
                "sha256": "1" * 64,
            },
            "carry_forward_plan": {
                "bytes": 1,
                "canonical_path": "carry-forward-plan.json",
                "sha256": "2" * 64,
            },
        }
        self.selection_path = self.root / "selection.json"
        self._write_selection(self.selection)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _write_selection(self, value: dict) -> None:
        source = json.dumps(value, indent=2, sort_keys=True) + "\n"
        self.selection_path.write_text(source)
        (self.repo / "tools" / "release" / "projection.json").write_text(source)

    def _run(self, *, selection_sha: str | None = None, gate_sha: str | None = None):
        output = self.root / "normalized.json"
        output.unlink(missing_ok=True)
        return subprocess.run(
            [
                sys.executable,
                str(ADAPTER),
                "--repo",
                str(self.repo),
                "--gate",
                str(self.gate),
                "--gate-sha256",
                gate_sha or digest(self.gate),
                "--selection",
                str(self.selection_path),
                "--selection-sha256",
                selection_sha or digest(self.selection_path),
                "--output",
                str(output),
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def test_exact_trading_projection_normalizes_five_role_paths(self) -> None:
        completed = self._run()
        self.assertEqual(completed.returncode, 0, completed.stderr)
        value = json.loads((self.root / "normalized.json").read_text())
        self.assertEqual(value["trading_elf_sha256"], self.selection["elf"]["sha256"])
        self.assertEqual(list(value["role_elf_paths"]), sorted(ROLES))

    def test_wrong_role_refuses_even_with_recomputed_projection_digest(self) -> None:
        hostile = dict(self.selection)
        hostile["label"] = "core"
        self._write_selection(hostile)
        completed = self._run()
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("role, package, gate path, or gate SHA-256 differs", completed.stderr)

    def test_changed_trading_elf_refuses(self) -> None:
        (self.gate_root / "elf" / "trading.so").write_bytes(b"\x7fELFchanged")
        completed = self._run()
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("evidence bytes or SHA-256 differ", completed.stderr)

    def test_wrong_out_of_band_gate_digest_refuses(self) -> None:
        completed = self._run(gate_sha="0" * 64)
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("mixed checked gate SHA-256 differs", completed.stderr)


if __name__ == "__main__":
    unittest.main()
