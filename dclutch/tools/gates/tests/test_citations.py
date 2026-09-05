"""The citation tripwire, both directions: a real commit is clean; each defect class fires.

The gate is only worth its runtime if it can go red, so every refusal it claims
is proved here against a scratch repository with real git objects -- an
unadjudicated dangling citation, a register row that started resolving, a row
nothing cites, a row whose documents drifted, and a class no `classes` note
declares.
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from gates import citations  # noqa: E402
from gates.common import EXIT_FAIL, EXIT_PASS, Prereq  # noqa: E402


def git(root: Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-C", str(root), "-c", "user.email=c@x", "-c", "user.name=c",
         "-c", "commit.gpgsign=false", *args],
        check=True, capture_output=True, text=True,
    ).stdout.strip()


class CitationTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix="dclutch-citations-test."))
        (self.root / "docs/decisions").mkdir(parents=True)
        (self.root / "docs/evidence").mkdir(parents=True)
        git(self.root, "init", "-q")
        (self.root / "seed.txt").write_text("seed\n")
        git(self.root, "add", "seed.txt")
        git(self.root, "commit", "-qm", "the commit a record may honestly cite")
        self.real = git(self.root, "rev-parse", "--short=9", "HEAD")
        self.register = self.root / "register.json"
        self.write_register({})

    def tearDown(self):
        shutil.rmtree(self.root, ignore_errors=True)

    def write_register(self, adjudicated: dict, classes: dict | None = None):
        self.register.write_text(json.dumps({
            "classes": classes if classes is not None
            else {"artifact-digest": "a digest, not a commit"},
            "adjudicated": adjudicated,
        }))

    def doc(self, relative: str, text: str):
        (self.root / relative).write_text(text)

    def run_gate(self) -> tuple[int, str]:
        return citations.check(self.root, register_path=self.register)

    # ---------------------------------------------------------------- green

    def test_a_citation_that_resolves_is_clean(self):
        self.doc("docs/decisions/0001-a.md", f"Landed at `{self.real}`.\n")
        self.assertEqual(self.run_gate(), (EXIT_PASS, ""))

    def test_an_adjudicated_non_commit_is_clean(self):
        self.doc("docs/evidence/E.md", "The ELF digest is `deadbeef12`.\n")
        self.write_register({"deadbeef12": {
            "class": "artifact-digest", "cited_by": ["docs/evidence/E.md"]}})
        self.assertEqual(self.run_gate(), (EXIT_PASS, ""))

    def test_a_token_too_short_to_be_a_citation_is_not_a_candidate(self):
        self.doc("docs/decisions/0001-a.md",
                 f"Landed at `{self.real}`; the byte is `0xab`, the field `beef`.\n")
        self.assertEqual(self.run_gate()[0], EXIT_PASS)

    # ------------------------------------------------------------- refusals

    def test_a_dangling_citation_with_no_register_row_refuses(self):
        """The synthetic bad citation: a well-formed sha this repository never held."""
        self.doc("docs/decisions/0001-a.md",
                 f"Landed at `{self.real}` and at `abcdef0123456789`.\n")
        code, detail = self.run_gate()
        self.assertEqual(code, EXIT_FAIL)
        self.assertIn("1 cited commit(s) do not exist", detail)

    def test_a_dangling_citation_inside_evidence_refuses_too(self):
        self.doc("docs/evidence/E.md", "Measured at `0123456789abcdef`.\n")
        self.assertEqual(self.run_gate()[0], EXIT_FAIL)

    def test_a_register_row_that_started_resolving_refuses(self):
        self.doc("docs/decisions/0001-a.md", f"Landed at `{self.real}`.\n")
        self.write_register({self.real: {
            "class": "artifact-digest", "cited_by": ["docs/decisions/0001-a.md"]}})
        code, detail = self.run_gate()
        self.assertEqual(code, EXIT_FAIL)
        self.assertIn("1 register row(s) went false", detail)

    def test_a_register_row_nothing_cites_refuses(self):
        self.doc("docs/decisions/0001-a.md", f"Landed at `{self.real}`.\n")
        self.write_register({"deadbeef12": {
            "class": "artifact-digest", "cited_by": ["docs/evidence/gone.md"]}})
        code, detail = self.run_gate()
        self.assertEqual(code, EXIT_FAIL)
        self.assertIn("1 row(s) cite nothing", detail)

    def test_a_register_row_naming_the_wrong_document_refuses(self):
        self.doc("docs/evidence/E.md", "The ELF digest is `deadbeef12`.\n")
        self.write_register({"deadbeef12": {
            "class": "artifact-digest", "cited_by": ["docs/evidence/somewhere-else.md"]}})
        code, detail = self.run_gate()
        self.assertEqual(code, EXIT_FAIL)
        self.assertIn("1 row(s) name the wrong documents", detail)

    def test_a_row_whose_class_is_undeclared_refuses(self):
        self.doc("docs/evidence/E.md", "The ELF digest is `deadbeef12`.\n")
        self.write_register(
            {"deadbeef12": {"class": "invented", "cited_by": ["docs/evidence/E.md"]}},
            classes={"artifact-digest": "a digest, not a commit"},
        )
        code, detail = self.run_gate()
        self.assertEqual(code, EXIT_FAIL)
        self.assertIn("1 undeclared class(es)", detail)

    # --------------------------------------------------- never a silent pass

    def test_no_candidate_at_all_is_a_prerequisite_failure_not_a_pass(self):
        """An empty survey is a disconnected instrument, and must not read as green."""
        with self.assertRaises(Prereq):
            self.run_gate()

    def test_a_missing_register_is_a_prerequisite_failure_not_a_pass(self):
        self.doc("docs/decisions/0001-a.md", f"Landed at `{self.real}`.\n")
        self.register.unlink()
        with self.assertRaises(Prereq):
            self.run_gate()


if __name__ == "__main__":
    unittest.main()
