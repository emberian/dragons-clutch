"""The frame ratchet, shown capable of refusing: growth, shrinkage, unnamed captures, dirty trees, and who owes rows."""

from __future__ import annotations

import copy
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from gates import frames  # noqa: E402
from gates.common import Failed, Prereq  # noqa: E402

FIRST, SECOND = "1" * 40, "2" * 40
HAVE_CARGO = shutil.which("cargo") is not None


def manifest(commit: str | None = FIRST) -> dict:
    links = [{"package": f"program-{i:02d}", "frame_count": 2,
              "functions": [{"symbol": "alpha", "frames_bytes": [3072]}, {"symbol": "beta", "frames_bytes": [128]}]}
             for i in range(frames.EXPECTED_LINK_COUNT)]
    value = {"schema": frames.MANIFEST_SCHEMA, "bound_bytes": frames.SBPF_V0_FRAME_BYTES,
             "link_count": frames.EXPECTED_LINK_COUNT, "links": links}
    if commit is not None:
        value[frames.COMMIT_FIELD] = commit
    return value


def git(root: Path, *args: str) -> str:
    env = {**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@invalid",
           "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@invalid"}
    return subprocess.run(["git", "-C", str(root), "-c", "commit.gpgsign=false", *args], check=True, env=env,
                          text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL).stdout.strip()


class ManifestTests(unittest.TestCase):
    def test_hashes_are_nonsemantic_but_colliding_instances_survive(self):
        report = {"schema": frames.REPORT_SCHEMA, "bound_bytes": 4096, "frame_count": 2, "frames": [
            {"bytes": 64, "symbol": "_ZN4demo3run17h0123456789abcdefE"},
            {"bytes": 128, "symbol": "_ZN4demo3run17hfedcba9876543210E"}]}
        canonical = frames.canonicalize_report(report, "fixture")
        self.assertEqual(len(canonical["functions"]), 1)
        self.assertEqual(canonical["functions"][0]["frames_bytes"], [128, 64])

    def test_growth_and_shrinkage_are_both_red(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "b.json").write_text(json.dumps({**manifest(), "schema": frames.BASELINE_SCHEMA}))
            for size, word in ((3712, "GREW"), (2048, "changed/ratcheted")):
                candidate = manifest()
                candidate["links"][0]["functions"][0]["frames_bytes"] = [size]
                (root / "c.json").write_text(json.dumps(candidate))
                with self.assertRaises(Failed) as refusal:
                    frames.check(root / "b.json", root / "c.json")
                self.assertIn(f"{word} alpha", str(refusal.exception))

    def test_missing_inputs_are_exit_two_not_a_verdict(self):
        with self.assertRaises(Prereq):
            frames.check(Path("/nonexistent/b.json"), Path("/nonexistent/c.json"))

    def test_the_base_survives_acceptance_and_is_not_frame_content(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "1.json").write_text(json.dumps(manifest()))
            (root / "2.json").write_text(json.dumps(manifest()))
            self.assertEqual(frames.accept(root / "1.json", root / "2.json", root / "b.json"), FIRST)
            (root / "c.json").write_text(json.dumps(manifest(SECOND)))
            self.assertEqual(frames.check(root / "b.json", root / "c.json"), FIRST)

    def test_captures_of_two_commits_or_of_none_are_not_a_baseline(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "1.json").write_text(json.dumps(manifest(FIRST)))
            (root / "2.json").write_text(json.dumps(manifest(SECOND)))
            with self.assertRaises(Failed):
                frames.accept(root / "1.json", root / "2.json", root / "b.json")
            (root / "1.json").write_text(json.dumps(manifest(None)))
            (root / "2.json").write_text(json.dumps(manifest(None)))
            with self.assertRaises(Prereq):
                frames.accept(root / "1.json", root / "2.json", root / "b.json")
            self.assertFalse((root / "b.json").exists())
            changed = copy.deepcopy(manifest())
            changed["links"][0]["functions"][0]["frames_bytes"] = [3712]
            (root / "1.json").write_text(json.dumps(manifest()))
            (root / "2.json").write_text(json.dumps(changed))
            with self.assertRaises(Failed):
                frames.accept(root / "1.json", root / "2.json", root / "b.json")


def repository(root: Path) -> None:
    """alpha-sbf -> crates/mid -> crates/leaf: leaf is TWO edges from the link, invisible to a programs/*/src predicate."""

    def crate(directory: str, name: str, dependency: str | None = None):
        (root / directory / "src").mkdir(parents=True)
        (root / directory / "Cargo.toml").write_text(
            f'[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2021"\n\n[workspace]\n\n[dependencies]\n' + (dependency or "") + "\n")
        (root / directory / "src" / "lib.rs").write_text("// one\n")

    def commit(message: str):
        git(root, "add", "-A")
        git(root, "commit", "-q", "-m", message)

    git(root, "init", "-q")
    (root / "tools/gates").mkdir(parents=True)
    (root / "tools/gates/frames-baseline.json").write_text("{}\n")
    crate("crates/leaf", "leaf")
    crate("crates/mid", "mid", 'leaf = { path = "../leaf" }')
    crate("crates/unrelated", "unrelated")
    crate("programs/alpha-sbf", "alpha-sbf", 'mid = { path = "../../crates/mid" }')
    crate("programs/beta-sbf", "beta-sbf")
    commit("base")
    (root / "programs/alpha-sbf/src/lib.rs").write_text("// two\n")
    commit("alpha moves a frame")
    (root / "crates/unrelated/src/lib.rs").write_text("// two\n")
    commit("not in any closure")
    (root / "programs/beta-sbf/src/lib.rs").write_text("// two\n")
    (root / "tools/gates/frames-baseline.json").write_text('{"n": 1}\n')
    commit("beta carries its rows")
    (root / "crates/leaf/src/lib.rs").write_text("// two\n")
    commit("leaf is two edges from the link")
    (root / "crates/leaf/README.md").write_text("not a compiler input\n")
    commit("documentation beside a compiled crate")


@unittest.skipUnless(HAVE_CARGO, "the closure is read from cargo metadata")
class OwedTests(unittest.TestCase):
    def test_owed_follows_the_closure_and_names_only_reached_links(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            repository(root)
            base = git(root, "rev-list", "--max-parents=0", "HEAD")
            with self.assertRaises(Failed) as ledger:
                frames.owed(root, base, None, "HEAD")
            text = str(ledger.exception)
            self.assertIn("leaf is two edges from the link", text)
            self.assertIn("alpha moves a frame", text)
            self.assertNotIn("not in any closure", text)
            self.assertNotIn("beta carries its rows", text)
            self.assertNotIn("documentation beside a compiled crate", text)
            self.assertIn("2 links over 4 first-party crates", text)
            for line in text.splitlines():
                if "reaches" in line:
                    self.assertNotIn("beta-sbf", line)

    def test_owed_reads_its_range_from_the_baselines_own_commit(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            repository(root)
            base = git(root, "rev-list", "--max-parents=0", "HEAD")
            admitted = root / "admitted.json"
            admitted.write_text(json.dumps({**manifest(base), "schema": frames.BASELINE_SCHEMA}))
            with self.assertRaises(Failed):
                frames.owed(root, None, admitted, "HEAD")
            frames.owed(root, git(root, "rev-parse", "HEAD"), None, "HEAD")  # empty range: nobody owes
            unnamed = root / "unnamed.json"
            unnamed.write_text(json.dumps({**manifest(None), "schema": frames.BASELINE_SCHEMA}))
            with self.assertRaises(Prereq):
                frames.owed(root, None, unnamed, "HEAD")


PARSER_STUB = """#!/usr/bin/env python3
import json, sys
print(json.dumps({"schema": "dclutch-sbf-frame-sizes-v1", "bound_bytes": 4096, "frame_count": 1,
                  "frames": [{"bytes": int(open(sys.argv[-1]).read().strip()), "symbol": "fixture"}]}))
"""
CARGO_STUB = """#!/usr/bin/env bash
set -eu
manifest=""
while [ "$#" -gt 0 ]; do [ "$1" = "--manifest-path" ] && { shift; manifest="$1"; }; shift; done
package="$(basename "$(dirname "$manifest")")"
stem="$(printf '%s' "$package" | tr '-' '_')"
mkdir -p "$CARGO_TARGET_DIR/sbpf-solana-solana/release/deps"
cp "$(dirname "$manifest")/src/frame.txt" "$CARGO_TARGET_DIR/sbpf-solana-solana/release/deps/$stem.o"
printf '   Compiling %s v0.1.0 (%s)\\n' "$package" "$(dirname "$manifest")"
if [ "${FRAMES_INJECT_DIAGNOSTIC:-}" = "$package" ]; then
    printf 'warning: A function call in method fixture overwrites values in the frame\\n'
fi
"""


class MeasureTests(unittest.TestCase):
    """The runner with a fake cargo: a capture names its commit, --at measures that commit, a dirty tree is refused."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="dclutch-frames-test."))
        self.fixture, self.bin = self.tmp / "source", self.tmp / "bin"
        (self.fixture / "tools").mkdir(parents=True)
        self.bin.mkdir()
        for index in range(1, frames.EXPECTED_LINK_COUNT + 1):
            package = self.fixture / "programs" / f"program-{index:02d}"
            (package / "src").mkdir(parents=True)
            (package / "Cargo.toml").write_text(f'[package]\nname = "program-{index:02d}"\nversion = "0.1.0"\n')
            (package / "src" / "frame.txt").write_text("128\n")
        (self.fixture / "tools/sbf-frame-sizes.py").write_text(PARSER_STUB)
        (self.bin / "cargo-build-sbf").write_text("#!/usr/bin/env bash\nexit 0\n")
        (self.bin / "cargo").write_text(CARGO_STUB)
        for stub in ("cargo-build-sbf", "cargo"):
            (self.bin / stub).chmod(0o755)
        git(self.fixture, "init", "-q")
        git(self.fixture, "add", "-A")
        git(self.fixture, "commit", "-q", "-m", "fixture")
        self.head = git(self.fixture, "rev-parse", "HEAD")
        self.saved_path = os.environ["PATH"]
        os.environ["PATH"] = f"{self.bin}:{self.saved_path}"

    def tearDown(self):
        os.environ["PATH"] = self.saved_path
        os.environ.pop("FRAMES_INJECT_DIAGNOSTIC", None)
        shutil.rmtree(self.tmp, ignore_errors=True)

    def run_frames(self, *args: str) -> int:
        return frames.main(["--source", str(self.fixture), "--tools", str(self.fixture), *args])

    def recorded(self, path: Path):
        return json.loads(path.read_text()).get("commit")

    def measured(self, path: Path) -> int:
        return json.loads(path.read_text())["links"][0]["functions"][0]["frames_bytes"][0]

    def test_a_clean_tree_captures_the_full_link_set_and_records_its_own_head(self):
        capture = self.tmp / "capture.json"
        self.assertEqual(self.run_frames("--capture", str(capture)), 0)
        self.assertEqual(self.recorded(capture), self.head)

    def test_a_zero_exit_build_with_a_frame_diagnostic_is_still_red(self):
        os.environ["FRAMES_INJECT_DIAGNOSTIC"] = "program-06"
        rejected = self.tmp / "rejected.json"
        self.assertEqual(self.run_frames("--capture", str(rejected)), 1)
        self.assertFalse(rejected.exists())

    def test_dirty_tree_capture_is_refused_and_at_measures_the_named_commit(self):
        (self.fixture / "programs/program-01/src/frame.txt").write_text("3712\n")
        unnamed = self.tmp / "unnamed.json"
        self.assertEqual(self.run_frames("--capture", str(unnamed)), 2)
        self.assertFalse(unnamed.exists())
        named = self.tmp / "named.json"
        self.assertEqual(self.run_frames("--at", self.head, "--capture", str(named)), 0)
        self.assertEqual(self.recorded(named), self.head)
        self.assertEqual(self.measured(named), 128)
        worktrees = git(self.fixture, "worktree", "list", "--porcelain").count("worktree ")
        self.assertEqual(worktrees, 1, "the detached worktree is removed")

    def test_a_revision_that_does_not_exist_is_exit_two(self):
        self.assertEqual(self.run_frames("--at", "nonesuch", "--capture", str(self.tmp / "x.json")), 2)
        self.assertFalse((self.tmp / "x.json").exists())

    def test_missing_sbf_toolchain_is_exit_two(self):
        os.environ["PATH"] = self.saved_path.replace(str(Path.home() / ".local/share/solana/install/active_release/bin"), "/nonexistent")
        if shutil.which("cargo-build-sbf"):
            self.skipTest("cargo-build-sbf is on PATH somewhere this test cannot hide")
        self.assertEqual(self.run_frames("--at", self.head, "--capture", str(self.tmp / "y.json")), 2)


if __name__ == "__main__":
    unittest.main()
