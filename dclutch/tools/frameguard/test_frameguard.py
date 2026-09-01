#!/usr/bin/env python3

from __future__ import annotations

import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


HERE = Path(__file__).resolve().parent
TOOL = HERE / "frameguard.py"
SPEC = importlib.util.spec_from_file_location("frameguard", TOOL)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def manifest() -> dict:
    links = []
    for index in range(MODULE.EXPECTED_LINK_COUNT):
        links.append(
            {
                "package": f"program-{index:02d}",
                "frame_count": 2,
                "functions": [
                    {"symbol": "alpha", "frames_bytes": [3072]},
                    {"symbol": "beta", "frames_bytes": [128]},
                ],
            }
        )
    return {
        "schema": MODULE.MANIFEST_SCHEMA,
        "bound_bytes": MODULE.SBPF_V0_FRAME_BYTES,
        "link_count": MODULE.EXPECTED_LINK_COUNT,
        "links": links,
    }


class FrameGuardTests(unittest.TestCase):
    def run_tool(self, *arguments: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(TOOL), *arguments],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def write(self, path: Path, value: dict) -> None:
        path.write_text(json.dumps(value))

    def test_hashes_are_nonsemantic_but_colliding_instances_survive(self) -> None:
        report = {
            "schema": MODULE.REPORT_SCHEMA,
            "bound_bytes": 4096,
            "frame_count": 2,
            "frames": [
                {"bytes": 64, "symbol": "_ZN4demo3run17h0123456789abcdefE"},
                {"bytes": 128, "symbol": "_ZN4demo3run17hfedcba9876543210E"},
            ],
        }
        canonical = MODULE.canonicalize_report(report, "fixture")
        self.assertEqual(canonical["frame_count"], 2)
        self.assertEqual(len(canonical["functions"]), 1)
        self.assertEqual(canonical["functions"][0]["frames_bytes"], [128, 64])

    def test_adversarial_640_byte_silent_growth_is_red(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            admitted = manifest()
            admitted["schema"] = MODULE.BASELINE_SCHEMA
            candidate = manifest()
            candidate["links"][0]["functions"][0]["frames_bytes"] = [3712]
            before = root / "baseline.json"
            after = root / "candidate.json"
            self.write(before, admitted)
            self.write(after, candidate)
            result = self.run_tool(
                "check", "--baseline", str(before), "--candidate", str(after)
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("GREW alpha: [3072] -> [3712]", result.stderr)

    def test_shrink_is_red_until_the_ratchet_is_lowered(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            admitted = manifest()
            admitted["schema"] = MODULE.BASELINE_SCHEMA
            candidate = manifest()
            candidate["links"][0]["functions"][0]["frames_bytes"] = [2048]
            before = root / "baseline.json"
            after = root / "candidate.json"
            self.write(before, admitted)
            self.write(after, candidate)
            result = self.run_tool(
                "check", "--baseline", str(before), "--candidate", str(after)
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("changed/ratcheted alpha", result.stderr)

    def test_growth_in_a_collided_monomorph_is_named_as_growth(self) -> None:
        before = manifest()
        after = copy.deepcopy(before)
        before["links"][0]["functions"][0]["frames_bytes"] = [3072, 128]
        before["links"][0]["frame_count"] = 3
        after["links"][0]["functions"][0]["frames_bytes"] = [3072, 768]
        after["links"][0]["frame_count"] = 3
        delta = MODULE.differences(before, after)
        self.assertIn("GREW alpha: [3072, 128] -> [3072, 768]", delta[0])

    def test_missing_or_malformed_inputs_are_exit_two(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            missing = Path(temporary) / "missing.json"
            result = self.run_tool(
                "check", "--baseline", str(missing), "--candidate", str(missing)
            )
            self.assertEqual(result.returncode, 2)
            self.assertIn("COULD NOT RUN", result.stderr)

    def test_baseline_requires_two_identical_independent_captures(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            first = root / "first.json"
            second = root / "second.json"
            output = root / "baseline.json"
            self.write(first, manifest())
            changed = copy.deepcopy(manifest())
            changed["links"][0]["functions"][0]["frames_bytes"] = [3712]
            self.write(second, changed)
            result = self.run_tool(
                "accept",
                "--first",
                str(first),
                "--second",
                str(second),
                "--output",
                str(output),
            )
            self.assertEqual(result.returncode, 1)
            self.assertFalse(output.exists())

            self.write(second, manifest())
            result = self.run_tool(
                "accept",
                "--first",
                str(first),
                "--second",
                str(second),
                "--output",
                str(output),
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            accepted = json.loads(output.read_text())
            self.assertEqual(accepted["schema"], MODULE.BASELINE_SCHEMA)


if __name__ == "__main__":
    unittest.main()
