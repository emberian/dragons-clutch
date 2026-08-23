#!/usr/bin/env python3
"""Deterministic unit tests for the schema-V2 measurement producer."""

from __future__ import annotations

import contextlib
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))
import measure_capability_profiles as measure  # noqa: E402


class MeasurementProducerTests(unittest.TestCase):
    def test_absent_identity_manifest_emits_unavailable_and_refuses(self) -> None:
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            status = measure.main([])
        self.assertEqual(status, 2)
        self.assertIn('"availability": "unavailable"', output.getvalue())
        self.assertIn('"profiles": []', output.getvalue())
        self.assertIn("no concrete fully linked", output.getvalue())

    def test_loader_rent_names_data_overhead_persistent_and_transient_separately(
        self,
    ) -> None:
        value = measure.loader_measurement(1_000, 2_000)
        self.assertEqual(value["program"]["data_len_bytes"], 36)  # type: ignore[index]
        self.assertEqual(value["programdata"]["data_len_bytes"], 2_045)  # type: ignore[index]
        self.assertEqual(value["buffer"]["data_len_bytes"], 2_037)  # type: ignore[index]
        self.assertEqual(value["program"]["storage_overhead_bytes"], 128)  # type: ignore[index]
        self.assertEqual(
            value["persistent_program_plus_programdata_rent_lamports"],
            (36 + 128 + 45 + 2_000 + 128) * 6_960,
        )
        self.assertEqual(
            value["transient_buffer_rent_lamports"], (37 + 2_000 + 128) * 6_960
        )
        self.assertFalse(value["exact_size_allocation"])

    def test_elf_larger_than_chosen_programdata_max_len_refuses(self) -> None:
        with self.assertRaisesRegex(measure.MeasurementError, "exceeds chosen"):
            measure.loader_measurement(2_001, 2_000)

    def test_status_split_preserves_tracked_and_untracked_inputs(self) -> None:
        tracked, untracked = measure.split_status(
            [" M crates/a.rs", "A  programs/b.rs", "?? crates/new.rs"]
        )
        self.assertEqual(tracked, [" M crates/a.rs", "A  programs/b.rs"])
        self.assertEqual(untracked, ["crates/new.rs"])

    def test_source_state_detects_both_tracked_and_untracked_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
            (repo / "linked").mkdir()
            (repo / "linked" / "tracked.txt").write_text("one", encoding="utf-8")
            subprocess.run(["git", "add", "linked/tracked.txt"], cwd=repo, check=True)
            subprocess.run(
                [
                    "git",
                    "-c",
                    "user.name=Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "commit",
                    "-qm",
                    "fixture",
                ],
                cwd=repo,
                check=True,
            )
            clean = measure.source_state(repo, ["linked"])
            self.assertEqual(clean["tracked_dirty"], [])
            self.assertEqual(clean["untracked"], [])

            (repo / "linked" / "tracked.txt").write_text("two", encoding="utf-8")
            (repo / "linked" / "new.txt").write_text("new", encoding="utf-8")
            dirty = measure.source_state(repo, ["linked"])
            self.assertTrue(dirty["tracked_dirty"])
            self.assertEqual(dirty["untracked"], ["linked/new.txt"])
            with self.assertRaisesRegex(
                measure.MeasurementError, "tracked=.*untracked="
            ):
                measure.require_clean_state(dirty, "fixture")

    def test_section_and_syscall_parsers_are_exact(self) -> None:
        section_text = """
Section {
  Name: .text (1)
  Size: 123
}
Section {
  Name: .rodata (2)
  Size: 45
}
  Symbol {
    Name: sol_log_ (7)
    Section: Undefined
  }
  Symbol {
    Name: abort (8)
    Section: Undefined
  }
"""
        self.assertEqual(measure.section_size(section_text, ".text"), 123)
        self.assertEqual(measure.section_size(section_text, ".rodata"), 45)
        self.assertEqual(
            measure.undefined_dynamic_symbols(section_text), ["abort", "sol_log_"]
        )

    def test_final_frame_audit_refuses_out_of_frame_reference(self) -> None:
        symbols = "0000000000000010 g F .text 00000008 test"
        good = "0000000000000010 <test>:\n  ldxdw r1, [r10 - 0x100]"
        result = measure.final_frame_audit(symbols, good)
        self.assertEqual(result["direct_frame_bounds"], "PASS")
        self.assertEqual(result["deepest_direct_r10_offset"], 256)
        bad = "0000000000000010 <test>:\n  ldxdw r1, [r10 - 0x1001]"
        with self.assertRaisesRegex(measure.MeasurementError, "out-of-frame"):
            measure.final_frame_audit(symbols, bad)


if __name__ == "__main__":
    unittest.main()
