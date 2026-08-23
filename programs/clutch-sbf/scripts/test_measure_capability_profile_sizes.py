#!/usr/bin/env python3
"""Deterministic unit tests for the non-promotable size diagnostic."""

from __future__ import annotations

from pathlib import Path
import copy
import sys
import unittest


SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))
import measure_capability_profile_sizes as sizes  # noqa: E402


def measurement(elf: int, text: int, rodata: int, digest: str) -> dict:
    return {
        "elf_sha256": digest,
        "elf_bytes": elf,
        "text_bytes": text,
        "rodata_bytes": rodata,
        "undefined_dynamic_symbols": ["abort"],
        "backend_stack_diagnostic_lines": 2,
        "backend_stack_diagnostic_symbols": 2,
        "backend_stack_diagnostic_survivors": 0,
        "final_frame_audit": {"direct_frame_bounds": "PASS"},
        "loader": {
            "persistent_program_plus_programdata_rent_lamports": (elf + 337)
            * 6_960
        },
        "text_symbol_attribution": {
            "groups": [{"name": "fixture", "text_bytes": text}]
        },
    }


class SizeDiagnosticTests(unittest.TestCase):
    def test_explicit_full_retains_cargo_default_identity_marker(self) -> None:
        self.assertEqual(
            sizes.explicit_profile_features("profile-full"),
            ["custom-heap", "default", "profile-full"],
        )
        self.assertEqual(
            sizes.explicit_profile_features("profile-direct-v3-source-v2-point"),
            ["custom-heap", "profile-direct-v3-source-v2-point"],
        )

    def test_profile_selectors_are_explicit_unique_and_known(self) -> None:
        self.assertEqual(
            sizes.parse_profile_specs(
                [
                    "full=profile-full",
                    "direct=profile-direct-v3-source-v2-point",
                ]
            ),
            [
                ("full", "profile-full"),
                ("direct", "profile-direct-v3-source-v2-point"),
            ],
        )
        with self.assertRaisesRegex(sizes.DiagnosticError, "at least one"):
            sizes.parse_profile_specs(None)
        with self.assertRaisesRegex(sizes.DiagnosticError, "duplicate profile name"):
            sizes.parse_profile_specs(["full=profile-full", "full=profile-general-source-v2-point"])
        with self.assertRaisesRegex(sizes.DiagnosticError, "unknown"):
            sizes.parse_profile_specs(["full=profile-invented"])

    def test_pairwise_deltas_keep_direction_and_exact_rent_units(self) -> None:
        profiles = [
            {"name": "full", "measurements": [measurement(2_000, 1_800, 100, "a" * 64)]},
            {"name": "direct", "measurements": [measurement(1_000, 900, 50, "b" * 64)]},
        ]
        self.assertEqual(
            sizes.pairwise_deltas(profiles),
            [
                {
                    "from": "full",
                    "to": "direct",
                    "elf_bytes_delta": -1_000,
                    "text_bytes_delta": -900,
                    "rodata_bytes_delta": -50,
                    "persistent_loader_rent_lamports_delta": -6_960_000,
                    "text_symbol_group_deltas": [
                        {"name": "fixture", "text_bytes_delta": -900}
                    ],
                }
            ],
        )

    def test_text_attribution_deduplicates_aliases_and_names_conflicts(self) -> None:
        symbols = """
0000000000000010 l F .text 00000008 clutch_sbf::instructions::orders_batch::run::h1
0000000000000018 g F .text 00000004 .hidden clutch_batch::fold::h2
000000000000001c l F .text 00000004 clutch_sbf::source_archive::id::h3
000000000000001c l F .text 00000004 clutch_sbf::source_archive_v2::id::h4
"""
        value = sizes.text_symbol_attribution(symbols, 0x10, 16)
        self.assertTrue(value["matches_text_section"])
        self.assertEqual(value["text_section_start"], 0x10)
        self.assertEqual(value["text_section_end_exclusive"], 0x20)
        self.assertTrue(value["canonical_union_exact_coverage"])
        self.assertEqual(value["function_regions"], 3)
        self.assertEqual(value["merged_alias_regions"], 1)
        self.assertEqual(
            value["groups"],
            [
                {
                    "name": "clutch_sbf::instructions::orders_batch",
                    "text_bytes": 8,
                },
                {"name": "clutch_batch", "text_bytes": 4},
                {"name": "merged-aliases", "text_bytes": 4},
            ],
        )

    def test_text_attribution_rejects_equal_sum_overlap_gap_cancellation(self) -> None:
        symbols = """
0000000000000010 l F .text 0000000c first
0000000000000018 l F .text 00000004 second
"""
        with self.assertRaisesRegex(sizes.DiagnosticError, "overlapping"):
            sizes.text_symbol_attribution(symbols, 0x10, 16)

    def test_text_attribution_rejects_zero_out_of_range_gap_and_overlap(self) -> None:
        cases = (
            ("0000000000000010 l F .text 00000000 zero", "zero-sized"),
            ("000000000000000f l F .text 00000010 low", "outside"),
            ("0000000000000010 l F .text 00000011 high", "outside"),
            ("0000000000000010 l F .text 00000008 first\n0000000000000019 l F .text 00000007 second", "gap"),
            ("0000000000000010 l F .text 00000010 first\n0000000000000010 l F .text 00000008 second", "overlapping"),
        )
        for symbols, message in cases:
            with self.subTest(message=message), self.assertRaisesRegex(
                sizes.DiagnosticError, message
            ):
                sizes.text_symbol_attribution(symbols, 0x10, 16)

    def test_default_comparison_exposes_hash_fork_without_calling_it_equal(self) -> None:
        explicit = measurement(2_000, 1_800, 100, "a" * 64)
        default = measurement(2_000, 1_800, 100, "b" * 64)
        value = sizes.comparison(explicit, [default, dict(default)])
        self.assertTrue(value["cargo_default_reproducible"])
        self.assertFalse(value["byte_identical_to_explicit_profile"])
        self.assertEqual(value["strict_v2_default_equivalence_gate"], "REFUSE")
        self.assertEqual(
            value["mismatches"]["elf_sha256"],
            {"explicit": "a" * 64, "cargo_default": "b" * 64},
        )

    def test_same_elf_hash_backend_or_frame_mismatch_refuses_strict_gate(self) -> None:
        explicit = measurement(2_000, 1_800, 100, "a" * 64)
        mutations = (
            ("backend_stack_diagnostic_lines", 3),
            ("final_frame_audit", {"direct_frame_bounds": "PASS", "deepest": 512}),
        )
        for key, changed in mutations:
            with self.subTest(key=key):
                default = copy.deepcopy(explicit)
                default[key] = changed
                value = sizes.comparison(explicit, [default, copy.deepcopy(default)])
                self.assertTrue(value["byte_identical_to_explicit_profile"])
                self.assertEqual(value["strict_v2_default_equivalence_gate"], "REFUSE")
                self.assertIn(key, value["mismatches"])


if __name__ == "__main__":
    unittest.main()
