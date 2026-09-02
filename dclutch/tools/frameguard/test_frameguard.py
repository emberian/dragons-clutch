#!/usr/bin/env python3

from __future__ import annotations

import copy
import importlib.util
import json
from pathlib import Path
import os
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


FIRST_COMMIT = "1" * 40
SECOND_COMMIT = "2" * 40


def manifest(commit: str | None = FIRST_COMMIT) -> dict:
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
    value = {
        "schema": MODULE.MANIFEST_SCHEMA,
        "bound_bytes": MODULE.SBPF_V0_FRAME_BYTES,
        "link_count": MODULE.EXPECTED_LINK_COUNT,
        "links": links,
    }
    if commit is not None:
        value[MODULE.COMMIT_FIELD] = commit
    return value


def repository(root: Path) -> None:
    """A tiny history whose only interesting property is which paths moved."""

    environment = {
        **os.environ,
        "GIT_AUTHOR_NAME": "frameguard",
        "GIT_AUTHOR_EMAIL": "frameguard@invalid",
        "GIT_COMMITTER_NAME": "frameguard",
        "GIT_COMMITTER_EMAIL": "frameguard@invalid",
    }

    def run(*arguments: str) -> None:
        subprocess.run(
            ["git", "-C", str(root), *arguments],
            check=True,
            env=environment,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

    def write(relative: str, text: str) -> None:
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    run("init", "-q")
    write("tools/frameguard/baseline.json", "{}\n")
    write("programs/alpha-sbf/src/lib.rs", "// one\n")
    run("add", "-A")
    run("-c", "commit.gpgsign=false", "commit", "-q", "-m", "base")
    write("programs/alpha-sbf/src/lib.rs", "// two\n")
    run("add", "-A")
    run("-c", "commit.gpgsign=false", "commit", "-q", "-m", "alpha moves a frame")
    write("crates/somewhere/src/lib.rs", "// three\n")
    run("add", "-A")
    run("-c", "commit.gpgsign=false", "commit", "-q", "-m", "not a program source")
    write("programs/beta-sbf/src/lib.rs", "// four\n")
    write("tools/frameguard/baseline.json", '{"n": 1}\n')
    run("add", "-A")
    run("-c", "commit.gpgsign=false", "commit", "-q", "-m", "beta carries its rows")


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

    def test_the_named_base_survives_acceptance_and_is_not_frame_content(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            first, second = root / "first.json", root / "second.json"
            output = root / "baseline.json"
            self.write(first, manifest())
            self.write(second, manifest())
            result = self.run_tool(
                "accept", "--first", str(first), "--second", str(second),
                "--output", str(output),
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            accepted = json.loads(output.read_text())
            self.assertEqual(accepted[MODULE.COMMIT_FIELD], FIRST_COMMIT)

            # Re-measuring the SAME frames at a later commit is agreement: the
            # ratchet is over frames, and the base is provenance, not content.
            candidate = root / "candidate.json"
            self.write(candidate, manifest(SECOND_COMMIT))
            result = self.run_tool(
                "check", "--baseline", str(output), "--candidate", str(candidate)
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn(FIRST_COMMIT, result.stdout)

    def test_captures_of_two_different_commits_are_not_a_baseline(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            first, second = root / "first.json", root / "second.json"
            output = root / "baseline.json"
            self.write(first, manifest(FIRST_COMMIT))
            self.write(second, manifest(SECOND_COMMIT))
            result = self.run_tool(
                "accept", "--first", str(first), "--second", str(second),
                "--output", str(output),
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("captured at different commits", result.stderr)
            self.assertFalse(output.exists())

    def test_a_capture_that_names_no_commit_is_never_admitted(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            first, second = root / "first.json", root / "second.json"
            output = root / "baseline.json"
            self.write(first, manifest(None))
            self.write(second, manifest(None))
            result = self.run_tool(
                "accept", "--first", str(first), "--second", str(second),
                "--output", str(output),
            )
            self.assertEqual(result.returncode, 2)
            self.assertIn("names the commit it measured", result.stderr)
            self.assertFalse(output.exists())

    def test_owed_names_the_commit_that_left_its_rows_to_someone_else(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repository(root)
            base = subprocess.run(
                ["git", "-C", str(root), "rev-list", "--max-parents=0", "HEAD"],
                check=True, text=True, stdout=subprocess.PIPE,
            ).stdout.strip()
            result = self.run_tool("owed", "--repo", str(root), "--since", base)
            self.assertEqual(result.returncode, 1)
            self.assertIn("alpha moves a frame", result.stderr)
            self.assertIn("alpha-sbf", result.stderr)
            # A commit that carried its rows, and one that touched no program
            # source at all, are both settled and must not be accused.
            self.assertNotIn("beta carries its rows", result.stderr)
            self.assertNotIn("not a program source", result.stderr)

            head = subprocess.run(
                ["git", "-C", str(root), "rev-parse", "HEAD"],
                check=True, text=True, stdout=subprocess.PIPE,
            ).stdout.strip()
            result = self.run_tool("owed", "--repo", str(root), "--since", head)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("no commit moved program sources", result.stdout)

    def test_owed_reads_its_range_from_the_baselines_own_commit(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repository(root)
            base = subprocess.run(
                ["git", "-C", str(root), "rev-list", "--max-parents=0", "HEAD"],
                check=True, text=True, stdout=subprocess.PIPE,
            ).stdout.strip()
            admitted = root / "admitted.json"
            self.write(admitted, {**manifest(base), "schema": MODULE.BASELINE_SCHEMA})
            result = self.run_tool(
                "owed", "--repo", str(root), "--baseline", str(admitted)
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("alpha moves a frame", result.stderr)

            unnamed = root / "unnamed.json"
            self.write(unnamed, {**manifest(None), "schema": MODULE.BASELINE_SCHEMA})
            result = self.run_tool(
                "owed", "--repo", str(root), "--baseline", str(unnamed)
            )
            self.assertEqual(result.returncode, 2)
            self.assertIn("names no captured commit", result.stderr)

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
