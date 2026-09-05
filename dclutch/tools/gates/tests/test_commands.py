"""The runbook replayer, both directions: a working runbook is clean; each defect class fires; unprobed is 2, never 0 or 1."""

from __future__ import annotations

import contextlib
import io
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from gates import commands  # noqa: E402

THING = "#!/bin/sh\ncase \"${1:-}\" in\n    --help) printf 'usage: thing.sh [--loud]\\n\\n  --loud   say it twice\\n' ; exit 0 ;;\nesac\n"
NEEDY = "import argparse\nparser = argparse.ArgumentParser()\nparser.add_argument('--must', required=True)\nparser.add_argument('--maybe')\nparser.parse_args()\n"
NESTED = ("#!/bin/sh\ncase \"${1:-}\" in\n    inner)\n        case \"${2:-}\" in\n"
          "            --help) printf 'usage: nested.sh inner [--deep VALUE]\\n\\n  --deep   only ever named here\\n' ; exit 0 ;;\n        esac ;;\n"
          "    --help) printf 'usage: nested.sh <command>\\n\\ncommands:\\n  inner   do the inner thing\\n' ; exit 0 ;;\nesac\n")


def git(root: Path, *args: str) -> None:
    subprocess.run(["git", "-C", str(root), "-c", "user.email=c@x", "-c", "user.name=c", "-c", "commit.gpgsign=false", *args],
                   check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


class CommandsTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix="dclutch-commands-test."))
        (self.root / "docs/guides").mkdir(parents=True)
        (self.root / "tools").mkdir()
        git(self.root, "init", "-q")
        self.write("tools/thing.sh", THING, executable=True)
        self.write("tools/needy.py", NEEDY)

    def tearDown(self):
        shutil.rmtree(self.root, ignore_errors=True)

    def write(self, relative: str, text: str, *, executable: bool = False):
        path = self.root / relative
        path.write_text(text)
        if executable:
            path.chmod(0o755)

    def survey(self, *args: str) -> tuple[int, str]:
        git(self.root, "add", "-A")
        git(self.root, "commit", "-qm", "step", "--allow-empty")
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
            code = commands.main(["--root", str(self.root), *args])
        return code, out.getvalue()

    def test_a_runbook_whose_commands_work_is_clean(self):
        self.write("docs/guides/good.md", "# ok\n\n```sh\ntools/thing.sh --loud\npython3 tools/needy.py --must yes --maybe no\n```\n")
        code, report = self.survey("--check")
        self.assertEqual(code, 0, report)
        self.assertIn("against their own --help", report)

    def test_each_of_the_three_defects_is_named_and_check_fires(self):
        self.write("docs/guides/bad.md", "# rot\n\n```sh\ntools/gone.sh --loud\ntools/thing.sh --quiet\npython3 tools/needy.py --maybe no\n```\n")
        code, report = self.survey("--check")
        self.assertEqual(code, 1)
        for expected in ("unresolved program", "rejected by its own program", "incomplete as published", "--quiet"):
            self.assertIn(expected, report)

    def test_an_unprobed_command_is_exit_two(self):
        self.write("tools/opaque.sh", "#!/bin/sh\necho 'this program handles no help flag'\n", executable=True)
        self.write("docs/guides/unprobed.md", "# opaque\n\n```sh\ntools/opaque.sh --whatever\n```\n")
        code, _ = self.survey("--check")
        self.assertEqual(code, 2)

    def test_a_command_that_only_asks_for_usage_is_not_incomplete(self):
        """`--help` is answerable before a program's own requirements apply.

        `needy.py` requires `--must`. A runbook publishing `needy.py --help` is
        publishing the one command a reader types precisely because they do not
        yet know what to pass, and holding it to `--must` reports a defect in
        the escape hatch. The second half is the control: the same program
        without `--help` is still incomplete, so this exemption cannot be the
        check quietly going away.
        """
        self.write("docs/guides/asking.md", "# asking\n\n```sh\npython3 tools/needy.py --help\n```\n")
        code, report = self.survey("--check")
        self.assertEqual(code, 0, report)
        self.assertNotIn("incomplete as published", report)
        self.write("docs/guides/asking.md", "# asking\n\n```sh\npython3 tools/needy.py --maybe no\n```\n")
        code, report = self.survey("--check")
        self.assertEqual(code, 1)
        self.assertIn("incomplete as published", report)

    def test_descends_into_a_subcommand_page_without_swallowing_an_unknown_flag(self):
        self.write("tools/nested.sh", NESTED, executable=True)
        self.write("docs/guides/nested.md", "# nested\n\n```sh\ntools/nested.sh inner --deep 3\n```\n")
        code, report = self.survey("--check")
        self.assertEqual(code, 0, report)
        self.write("docs/guides/nested.md", "# nested\n\n```sh\ntools/nested.sh inner --shallow 3\n```\n")
        code, report = self.survey("--check")
        self.assertEqual(code, 1)
        self.assertIn("--shallow", report)


if __name__ == "__main__":
    unittest.main()
