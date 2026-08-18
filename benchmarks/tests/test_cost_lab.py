from __future__ import annotations

import sys
import unittest
from pathlib import Path


BENCHMARKS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(BENCHMARKS))

import cost_lab  # noqa: E402


class ShortvecTests(unittest.TestCase):
    def test_boundaries(self) -> None:
        self.assertEqual(cost_lab.shortvec(0), b"\x00")
        self.assertEqual(cost_lab.shortvec(127), b"\x7f")
        self.assertEqual(cost_lab.shortvec(128), b"\x80\x01")
        self.assertEqual(cost_lab.shortvec(16_383), b"\xff\x7f")
        self.assertEqual(cost_lab.shortvec(16_384), b"\x80\x80\x01")
        self.assertEqual(cost_lab.shortvec(65_535), b"\xff\xff\x03")

    def test_rejects_unbounded_values(self) -> None:
        with self.assertRaises(cost_lab.ModelError):
            cost_lab.shortvec(-1)
        with self.assertRaises(cost_lab.ModelError):
            cost_lab.shortvec(65_536)


class WireTests(unittest.TestCase):
    def test_measured_matches_independent_sum(self) -> None:
        for tx_format in ("legacy_inline", "v0_alt"):
            for accounts in (7, 16, 39, 64):
                spec = cost_lab.WireSpec(
                    tx_format=tx_format,
                    total_accounts=accounts,
                    writable_accounts=2,
                    static_accounts_v0=2,
                    instruction_data=bytes(range(17)),
                )
                self.assertEqual(
                    len(cost_lab.serialize_synthetic_transaction(spec)),
                    cost_lab.analytical_wire_size(spec),
                )

    def test_v0_alt_compresses_but_does_not_change_account_count(self) -> None:
        constants = cost_lab.load_constants()
        rows = cost_lab.generate_rows(constants)
        legacy = cost_lab.find_row(rows, "claim-external_split-n16-legacy_inline")
        v0 = cost_lab.find_row(rows, "claim-external_split-n16-v0_alt")
        self.assertFalse(legacy["outputs"]["fits_packet_snapshot"])
        self.assertTrue(v0["outputs"]["fits_packet_snapshot"])
        self.assertEqual(
            legacy["outputs"]["account_count"], v0["outputs"]["account_count"]
        )


class CostModelTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.constants = cost_lab.load_constants()
        cls.rows = cost_lab.generate_rows(cls.constants)

    def test_package_default_rent_examples(self) -> None:
        self.assertEqual(cost_lab.rent_minimum(82, self.constants), 1_461_600)
        self.assertEqual(cost_lab.rent_minimum(165, self.constants), 2_039_280)

    def test_matrix_is_complete(self) -> None:
        self.assertEqual(len(self.rows), 193)
        self.assertEqual(len({row["scenario_id"] for row in self.rows}), 193)

    def test_n24_is_always_refused(self) -> None:
        n24 = [row for row in self.rows if row["inputs"].get("outcomes") == 24]
        self.assertTrue(n24)
        self.assertTrue(all(not row["admission"]["v1_admitted"] for row in n24))

    def test_external_split_cpi_and_trace_lower_bounds(self) -> None:
        for outcomes in self.constants["dragon_design_bounds"]["outcome_axis"]:
            row = cost_lab.find_row(
                self.rows, f"claim-external_split-n{outcomes}-legacy_inline"
            )
            self.assertEqual(
                row["outputs"]["token_cpi_count_lower_bound"], outcomes + 1
            )
            self.assertEqual(
                row["outputs"]["instruction_trace_entries_lower_bound"], outcomes + 2
            )

    def test_batch_verification_authenticates_every_order(self) -> None:
        for row in self.rows:
            if row["family"] == "batch_verification":
                self.assertEqual(
                    row["outputs"]["order_authentications_lower_bound"],
                    row["inputs"]["order_count"],
                )

    def test_page_packer_does_not_split_records(self) -> None:
        self.assertEqual(cost_lab.pack_record_pages(256, 128, [80]), 1)
        self.assertEqual(cost_lab.pack_record_pages(256, 128, [80, 80]), 2)
        with self.assertRaises(cost_lab.ModelError):
            cost_lab.pack_record_pages(256, 128, [129])

    def test_no_validator_measurement_claim(self) -> None:
        for row in self.rows:
            self.assertNotIn("measured_validator_execution", row["evidence"].values())


class GoldenTests(unittest.TestCase):
    def test_checked_in_artifacts_match_model(self) -> None:
        constants = cost_lab.load_constants()
        rows = cost_lab.generate_rows(constants)
        artifacts = cost_lab.rendered_artifacts(constants, rows)
        cost_lab.check_artifacts(cost_lab.DEFAULT_GOLDEN, artifacts)


if __name__ == "__main__":
    unittest.main()
