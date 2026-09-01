#!/usr/bin/env python3
"""Offline tests for usage_parity: proven red before it is trusted green.

The instrument this file guards was WRONG on its first attempt -- scanned
per-file it reported 24 of 34 files disagreeing, and every one was a false
positive. These cases pin the three shapes that caused that, so a future
narrowing of the scan reintroduces the false reds here rather than in a report.
"""

from __future__ import annotations

import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import usage_parity  # noqa: E402


class UsageParityTests(unittest.TestCase):
    def crate(self, files: dict[str, str]) -> pathlib.Path:
        root = pathlib.Path(tempfile.mkdtemp(prefix="usage-parity-")) / "crate" / "src"
        root.mkdir(parents=True)
        for name, body in files.items():
            (root / name).write_text(body)
        return root

    def test_a_taught_flag_the_crate_parses_is_accepted(self) -> None:
        failures, _, count = usage_parity.audit(self.crate({
            "a.rs": 'fn usage() -> &str {\n  "tool --alpha VALUE"\n}\n'
                    'fn parse(f: &str) { match f { "--alpha" => (), _ => () } }\n',
        }))
        self.assertEqual(failures, [])
        self.assertEqual(count, 1)

    def test_a_taught_flag_nobody_parses_fails_by_name(self) -> None:
        failures, _, _ = usage_parity.audit(self.crate({
            "a.rs": 'fn usage() -> &str {\n  "tool --alpha V --ghost V"\n}\n'
                    'fn parse(f: &str) { match f { "--alpha" => (), _ => () } }\n',
        }))
        self.assertEqual(len(failures), 1)
        self.assertIn("--ghost", failures[0])

    def test_a_flag_parsed_in_a_SIBLING_file_is_not_a_failure(self) -> None:
        """The false positive that made the first cut report 24 of 34.

        `--i-mean-devnet` is taught in nine modules and parsed in the shared
        devnet arms of others. Scoped per file it looks absent nine times.
        """
        failures, _, _ = usage_parity.audit(self.crate({
            "teaches.rs": 'fn usage() -> &str {\n  "tool --i-mean-devnet HASH"\n}\n',
            "parses.rs": 'fn parse(f: &str) { match f { "--i-mean-devnet" => (), _ => () } }\n',
        }))
        self.assertEqual(failures, [])

    def test_a_strip_prefix_stem_covers_every_flag_under_it(self) -> None:
        """`--keypair-founding-founder` is real; no literal ever spells it."""
        failures, _, _ = usage_parity.audit(self.crate({
            "a.rs": 'fn usage() -> &str {\n  "tool --keypair-founding-founder P"\n}\n'
                    'fn parse(f: &str) { f.strip_prefix("--keypair-"); }\n',
        }))
        self.assertEqual(failures, [])

    def test_undocumented_flags_are_a_note_and_never_a_failure(self) -> None:
        """PARSED BUT NOT TAUGHT is usually deliberate. A gate that fires on it
        cries wolf, and a gate that cries wolf gets ignored."""
        failures, notes, _ = usage_parity.audit(self.crate({
            "a.rs": 'fn usage() -> &str {\n  "tool --alpha V"\n}\n'
                    'fn parse(f: &str) { match f { "--alpha" => (), "--quiet" => (), _ => () } }\n',
        }))
        self.assertEqual(failures, [])
        self.assertTrue(notes)

    def test_a_crate_with_no_usage_function_refuses_rather_than_passing(self) -> None:
        """An empty derivation is a broken checker, never a clean tree."""
        with self.assertRaises(ValueError):
            usage_parity.audit(self.crate({"a.rs": "fn main() {}\n"}))

    def test_an_empty_directory_refuses(self) -> None:
        with self.assertRaises(FileNotFoundError):
            usage_parity.audit(self.crate({}))

    def test_the_live_successor_crate_passes(self) -> None:
        """The control: the real tree, which measured zero crate-wide."""
        root = pathlib.Path(__file__).resolve().parents[2]
        src = root / "tools" / "local-validator" / "bootstrap" / "successor" / "src"
        if not src.is_dir():
            self.skipTest("successor crate absent from this tree")
        failures, _, count = usage_parity.audit(src)
        self.assertEqual(failures, [], f"live crate teaches unparsed flags: {failures}")
        self.assertGreater(count, 20)


if __name__ == "__main__":
    unittest.main(verbosity=1)
