"""Boundary tests for the offline rent and capital-time calculator."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path


SCRIPTS = Path(__file__).resolve().parent
ROOT = SCRIPTS.parent
sys.path.insert(0, str(SCRIPTS))
import rent_capital_time_audit as audit  # noqa: E402


class ExactIntegerTests(unittest.TestCase):
    def test_rent_formula_and_historical_loader_identity_are_exact(self) -> None:
        self.assertEqual(audit.rent_exempt_lamports(0), 890_880)
        self.assertEqual(audit.rent_exempt_lamports(1), 897_840)
        self.assertEqual(
            audit.loader_v3_persistent_rent_lamports(
                audit.CURRENT_UNSEALED_HISTORICAL_ELF_BYTES
            ),
            14_495_292_720,
        )

    def test_integer_domain_refuses_negative_boolean_float_and_zero_elf(self) -> None:
        for hostile in (-1, True, 1.0):
            with self.subTest(hostile=hostile):
                with self.assertRaises(audit.AuditInputError):
                    audit.rent_exempt_lamports(hostile)  # type: ignore[arg-type]
        with self.assertRaises(audit.AuditInputError):
            audit.loader_v3_persistent_rent_lamports(0)

    def test_ceil_div_boundaries_and_invalid_denominator(self) -> None:
        self.assertEqual(audit.ceil_div(0, 16), 0)
        self.assertEqual(audit.ceil_div(16, 16), 1)
        self.assertEqual(audit.ceil_div(17, 16), 2)
        with self.assertRaises(audit.AuditInputError):
            audit.ceil_div(1, 0)


class ActiveWidthTests(unittest.TestCase):
    def test_clear_work_small_and_max_widths_are_exact(self) -> None:
        self.assertEqual(audit.clear_work_v2_account_bytes(2, 4, 3), 2_326)
        self.assertEqual(
            audit.clear_work_v2_account_bytes(16, 64, 64),
            audit.CLEAR_WORK_V1_ACCOUNT_BYTES,
        )
        self.assertEqual(
            audit.rent_exempt_lamports(audit.CLEAR_WORK_V1_ACCOUNT_BYTES)
            - audit.rent_exempt_lamports(audit.clear_work_v2_account_bytes(2, 4, 3)),
            332_186_880,
        )

    def test_clear_work_refuses_invalid_geometry(self) -> None:
        for geometry in ((1, 4, 3), (17, 4, 3), (2, 65, 3), (2, 3, 4)):
            with self.subTest(geometry=geometry):
                with self.assertRaises(audit.AuditInputError):
                    audit.clear_work_v2_account_bytes(*geometry)

    def test_candidate_feed_small_and_max_widths_are_exact(self) -> None:
        self.assertEqual(audit.candidate_feed_v2_account_bytes(2, 2, 1), 263)
        self.assertEqual(
            audit.candidate_feed_v2_account_bytes(16, 64, 416),
            audit.CANDIDATE_FEED_V1_ACCOUNT_BYTES,
        )
        self.assertEqual(
            audit.rent_exempt_lamports(audit.CANDIDATE_FEED_V1_ACCOUNT_BYTES)
            - audit.rent_exempt_lamports(audit.candidate_feed_v2_account_bytes(2, 2, 1)),
            41_780_880,
        )

    def test_candidate_feed_refuses_empty_orders_and_excess_slices(self) -> None:
        for geometry in ((2, 0, 1), (2, 2, 417)):
            with self.subTest(geometry=geometry):
                with self.assertRaises(audit.AuditInputError):
                    audit.candidate_feed_v2_account_bytes(*geometry)

    def test_frozen_comparison_keeps_distinct_recorded_geometries_visible(self) -> None:
        self.assertEqual(
            audit.active_width_candidate_savings_lamports(
                2, 4, 3, 1, feed_orders=2, candidates=3
            ),
            1_121_903_280,
        )
        coherent = audit.active_width_candidate_savings_lamports(
            2, 4, 3, 1, candidates=3
        )
        self.assertEqual(coherent, 1_121_569_200)
        self.assertNotEqual(coherent, 1_121_903_280)


class PageAndCapitalTimeTests(unittest.TestCase):
    def test_receipt_page_crossover_is_strict_at_seven(self) -> None:
        pair = audit.receipt_and_ledger_rent_lamports()
        page = audit.rent_exempt_lamports(audit.RECEIPT_PAGE_V1_ACCOUNT_BYTES)
        self.assertEqual(pair, 3_883_680)
        self.assertEqual(page, 26_169_600)
        self.assertGreater(page, 6 * pair)
        self.assertLess(page, 7 * pair)
        self.assertEqual(audit.receipt_page_minimum_live_entries(), 7)
        self.assertLess(audit.receipt_page_savings_lamports(6), 0)
        self.assertGreater(audit.receipt_page_savings_lamports(7), 0)

    def test_receipt_page_boundaries_and_full_book_projection(self) -> None:
        self.assertEqual(audit.receipt_page_savings_lamports(416), 935_201_280)
        self.assertEqual(audit.ceil_div(416, audit.RECEIPTS_PER_PAGE), 26)
        with self.assertRaises(audit.AuditInputError):
            audit.receipt_page_savings_lamports(0)

    def test_series_closed_form_matches_explicit_schedule(self) -> None:
        expected = 7 * sum(slot - 10 for slot in (20, 23, 26, 29))
        self.assertEqual(expected, 406)
        self.assertEqual(audit.series_capital_time(7, 4, 10, 20, 3), expected)
        self.assertEqual(audit.series_capital_time(9, 1, 100, 111, 0), 99)

    def test_series_refuses_noncanonical_or_overflowing_schedule(self) -> None:
        invalid = (
            (1, 1, 10, 9, 0),
            (1, 1, 0, 0, 1),
            (1, 2, 0, 0, 0),
            (1, 2, 0, audit.U64_MAX, 1),
        )
        for arguments in invalid:
            with self.subTest(arguments=arguments):
                with self.assertRaises(audit.AuditInputError):
                    audit.series_capital_time(*arguments)

    def test_series_exact_sum_may_safely_exceed_u64(self) -> None:
        result = audit.series_capital_time(audit.U64_MAX, 2, 0, 1, 1)
        self.assertEqual(result, 3 * audit.U64_MAX)
        self.assertGreater(result, audit.U64_MAX)


class CompressionAndEvidenceTests(unittest.TestCase):
    def test_claim_basis_model_reconstructs_active_binary_and_cubic_widths(self) -> None:
        self.assertEqual(audit.claim_basis_v2_body_bytes(0, 2, 2, 1), 82)
        self.assertEqual(audit.claim_basis_v2_body_bytes(3, 16, 0, 14), 256)
        self.assertEqual(
            audit.rent_exempt_lamports(2_352)
            - audit.rent_exempt_lamports(audit.claim_basis_v2_body_bytes(0, 2, 2, 1)),
            15_799_200,
        )

    def test_claim_basis_model_refuses_inconsistent_geometry(self) -> None:
        invalid = ((0, 2, 0, 1), (0, 2, 2, 2), (1, 2, 1, 2), (3, 16, 0, 13))
        for arguments in invalid:
            with self.subTest(arguments=arguments):
                with self.assertRaises(audit.AuditInputError):
                    audit.claim_basis_v2_body_bytes(*arguments)

    def test_snapshot_labels_each_quantitative_section_and_disclaims_live_evidence(self) -> None:
        snapshot = audit.snapshot(ROOT)
        self.assertIn("No current-working-tree linked ELF", snapshot["claim_boundary"])
        self.assertEqual(
            snapshot["historical_artifacts"]["evidence_class"],
            audit.HISTORICAL_ARTIFACT,
        )
        self.assertEqual(snapshot["receipt_page"]["evidence_class"], audit.MODEL_ONLY)
        self.assertEqual(
            snapshot["series_capital_time"]["evidence_class"], audit.MODEL_ONLY
        )
        self.assertEqual(
            snapshot["compressed_claim_basis"]["evidence_class"], audit.MODEL_ONLY
        )
        self.assertEqual(
            snapshot["active_width"]["clear_work_evidence_class"],
            audit.SOURCE_DERIVED,
        )
        self.assertEqual(
            snapshot["active_width"]["candidate_feed_evidence_class"],
            audit.MODEL_ONLY,
        )

    def test_frozen_arithmetic_and_provenance_gate(self) -> None:
        audit.check_frozen_examples(ROOT)


if __name__ == "__main__":
    unittest.main()
