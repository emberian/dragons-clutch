"""The reference generator's gates: the dirty-tree refusal and the convergence loop, against stub node/cargo/census."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from gates import reference  # noqa: E402

# Only generate.mjs moves anything; it writes docs/reference/a.md on its first N invocations.
NODE_STUB = """#!/bin/sh
case "$1" in
*/tools/genref/generate.mjs)
  root=$(dirname "$(dirname "$(dirname "$1")")")
  n=$(cat "$root/COUNT"); n=$((n + 1)); echo "$n" > "$root/COUNT"
  [ "$n" -le "${REFERENCE_TEST_MOVES:-2}" ] && echo "$n" > "$root/docs/reference/a.md"
  ;;
esac
exit 0
"""
CENSUS_STUB = "#!/bin/sh\nwhile [ $# -gt 0 ]; do [ \"$1\" = --out ] && { shift; : > \"$1\"; }; shift; done\nexit 0\n"


def git(root: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(root), "-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false", *args],
                          check=True, text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL).stdout.strip()


class ConvergeTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="dclutch-reference-test."))
        self.root, self.bin = self.tmp / "repo", self.tmp / "bin"
        for directory in ("tools/genref", "docs/reference", "apps/dclutch-web/scripts", "apps/dclutch-web/lib/generated",
                          "packages/dclutch-sdk/scripts", "packages/dclutch-sdk/lib/generated"):
            (self.root / directory).mkdir(parents=True)
        self.bin.mkdir()
        (self.root / "tools/genref/generate.mjs").write_text("")
        for emitter in reference.REFERENCE_COUPLED:
            (self.root / emitter).write_text("// reads docs/reference\n")
        (self.root / "COUNT").write_text("0\n")
        (self.root / "docs/reference/a.md").write_text("0\n")
        (self.bin / "node").write_text(NODE_STUB)
        (self.bin / "census").write_text(CENSUS_STUB)
        for stub in ("node", "census"):
            (self.bin / stub).chmod(0o755)
        git(self.root, "init", "-q")
        git(self.root, "add", "-A")
        git(self.root, "commit", "-qm", "base")
        self.saved = dict(os.environ)
        os.environ["PATH"] = f"{self.bin}:{os.environ['PATH']}"
        os.environ["DCLUTCH_GATE_CENSUS_BIN"] = str(self.bin / "census")
        os.environ.pop("GENREF_ALLOW_DIRTY", None)

    def tearDown(self):
        os.environ.clear()
        os.environ.update(self.saved)
        shutil.rmtree(self.tmp, ignore_errors=True)

    def reset(self, value: str = "0"):
        (self.root / "COUNT").write_text(f"{value}\n")
        (self.root / "docs/reference/a.md").write_text(f"{value}\n")

    def run_reference(self, *args: str, moves: str = "2") -> int:
        os.environ["REFERENCE_TEST_MOVES"] = moves
        return reference.main(list(args), root=self.root)

    def test_a_dirty_tree_refuses_and_allow_dirty_admits(self):
        (self.root / "dirt").write_text("dirt\n")
        git(self.root, "add", "dirt")
        self.assertEqual(self.run_reference("--check"), 2)
        self.assertEqual(self.run_reference("--check", "--allow-dirty"), 0)
        os.environ["GENREF_ALLOW_DIRTY"] = "1"
        self.assertEqual(self.run_reference("--check"), 0)
        git(self.root, "rm", "-q", "--cached", "dirt")
        (self.root / "dirt").unlink()

    def test_converge_reaches_a_fixpoint_by_the_third_pass(self):
        self.reset()
        self.assertEqual(self.run_reference("--converge", "--allow-dirty", moves="2"), 0)
        self.assertEqual((self.root / "COUNT").read_text().strip(), "3")

    def test_no_fixpoint_in_three_passes_is_refused(self):
        self.reset()
        self.assertEqual(self.run_reference("--converge", "--allow-dirty", moves="99"), 1)

    def test_already_at_the_fixpoint_writes_nothing(self):
        self.reset("5")
        self.assertEqual(self.run_reference("--converge", "--allow-dirty", moves="2"), 0)
        self.assertEqual((self.root / "docs/reference/a.md").read_text().strip(), "5")

    def test_an_undeclared_reference_reading_emitter_is_refused(self):
        newcomer = self.root / "apps/dclutch-web/scripts/generate-newcomer.mjs"
        newcomer.write_text("// also reads docs/reference\n")
        self.reset()
        self.assertEqual(self.run_reference("--converge", "--allow-dirty"), 1)
        newcomer.unlink()

    def test_check_converge_measures_the_committed_revision_not_this_tree(self):
        self.reset("5")  # the working tree is converged; the commit is not
        self.assertEqual(self.run_reference("--check", "--converge"), 1)
        git(self.root, "add", "-A")
        git(self.root, "commit", "-qm", "converged")
        self.assertEqual(self.run_reference("--check", "--converge"), 0)


if __name__ == "__main__":
    unittest.main()
