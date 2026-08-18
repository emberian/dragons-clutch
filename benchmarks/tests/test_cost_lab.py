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
        self.assertEqual(len(self.rows), 261)
        self.assertEqual(len({row["scenario_id"] for row in self.rows}), 261)
        arms = {arm: 0 for arm in cost_lab.ARMS}
        for row in self.rows:
            arms[row["arm"]] += 1
        # The hypothesis arm is retained whole beside the landed arm, never replaced by it.
        self.assertEqual(arms[cost_lab.ARM_HYPOTHESIS], 193)
        self.assertEqual(arms[cost_lab.ARM_LANDED], 56)
        self.assertEqual(arms[cost_lab.ARM_DIFFERENTIAL], 12)

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


class LandedAbiTests(unittest.TestCase):
    """The landed arm must equal the codec, not a restatement of it."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.constants = cost_lab.load_constants()
        cls.rows = cost_lab.generate_rows(cls.constants)
        cls.landed = cls.constants[cost_lab.ARM_LANDED]

    def landed_source(self) -> str | None:
        path = cost_lab.REPO_ROOT / self.landed["source"]["codec_path"]
        if not path.is_file():
            return None
        return path.read_text()

    def test_pinned_account_widths(self) -> None:
        expected = {
            "realm": 70,
            "profile": 100,
            "market": 726,
            "hoard": 108,
            "position": 220,
            "feed_head": 124,
            "order_page": 1819,
            "supply_ledger": 333,
            "terms": 1304,
            "price_grid": 589,
            "epoch": 328,
            "candidate_record": 305,
            "final_pot": 262,
            "settlement_receipt": 217,
            "resolution": 165,
        }
        self.assertEqual(set(expected), set(cost_lab.LANDED_ACCOUNT_ORDER))
        for name, width in expected.items():
            account = self.landed["accounts"][name]
            self.assertEqual(account["bytes"], width, name)
            self.assertEqual(sum(account["field_terms"]), width, name)

    def test_widths_are_rederived_from_the_codec_on_disk(self) -> None:
        source = self.landed_source()
        if source is None:
            self.skipTest("landed codec source is not present in this checkout")
        derived = cost_lab.derive_account_lengths_from_source(source)
        for name in cost_lab.LANDED_ACCOUNT_ORDER:
            account = self.landed["accounts"][name]
            rust_name = account["rust_const"].split("::", 1)[1]
            self.assertIn(rust_name, derived)
            self.assertEqual(
                derived[rust_name],
                account["bytes"],
                f"{name}: the codec moved and the cost lab arm is stale; "
                "re-pin constants.json and regenerate the goldens",
            )

    def test_abi_audit_reports_no_drift(self) -> None:
        if self.landed_source() is None:
            self.skipTest("landed codec source is not present in this checkout")
        _notes, drift = cost_lab.abi_audit(self.constants)
        self.assertEqual(drift, [])

    def test_abi_expression_evaluator_refuses_anything_but_sizes(self) -> None:
        self.assertEqual(cost_lab.evaluate_rust_arithmetic("2 + (7 * 32) + 1"), 227)
        self.assertEqual(
            cost_lab.evaluate_rust_arithmetic("MAX_ORDERS_PER_PAGE * ORDER_RECORD_BYTES"), 1584
        )
        for hostile in ("__import__('os')", "1 - 1", "open(1)", "MAX_UNKNOWN"):
            with self.assertRaises(cost_lab.ModelError):
                cost_lab.evaluate_rust_arithmetic(hostile)

    def test_order_page_geometry_is_forced(self) -> None:
        bounds = self.landed["bounds"]
        self.assertEqual(bounds["order_record_bytes"], 99)
        self.assertEqual(bounds["max_orders_per_page"], 16)
        self.assertEqual(bounds["max_order_pages"], 4)
        self.assertEqual(bounds["max_epoch_orders"], 64)
        self.assertEqual(bounds["relation_max_orders"], 64)
        self.assertEqual(
            bounds["order_page_header_bytes"] + 16 * bounds["order_record_bytes"],
            self.landed["accounts"]["order_page"]["bytes"],
        )
        for order_count, pages in ((1, 1), (16, 1), (17, 2), (32, 2), (48, 3), (64, 4)):
            self.assertEqual(cost_lab.landed_page_count(self.constants, order_count), pages)
        self.assertIsNone(cost_lab.landed_page_count(self.constants, 65))

    def test_one_instance_inventory_totals(self) -> None:
        row = cost_lab.find_row(self.rows, "landed-account-inventory-one-instance")
        self.assertEqual(row["outputs"]["data_bytes"], 6_670)
        self.assertEqual(row["outputs"]["rent_principal_lamports"], 59_786_400)
        self.assertEqual(row["outputs"]["rent_overhead_component_lamports"], 13_363_200)
        self.assertEqual(row["outputs"]["largest_account"], "order_page")
        self.assertEqual(row["outputs"]["smallest_account"], "realm")

    def test_landed_rent_examples(self) -> None:
        self.assertEqual(cost_lab.rent_minimum(220, self.constants), 2_422_080)
        self.assertEqual(cost_lab.rent_minimum(1_819, self.constants), 13_551_120)
        self.assertEqual(cost_lab.rent_minimum(333, self.constants), 3_208_560)

    def test_epoch_book_refuses_more_than_max_epoch_orders(self) -> None:
        row = cost_lab.find_row(self.rows, "landed-epoch-book-m65")
        self.assertFalse(row["outputs"]["landed_codec_representable"])
        self.assertFalse(row["admission"]["v1_admitted"])
        self.assertIsNone(row["outputs"]["page_count"])
        full = cost_lab.find_row(self.rows, "landed-epoch-book-m64")
        self.assertTrue(full["admission"]["v1_admitted"])
        self.assertEqual(full["outputs"]["page_count"], 4)
        self.assertEqual(full["outputs"]["padding_record_bytes"], 0)
        self.assertEqual(full["outputs"]["rent_principal_lamports"], 4 * 13_551_120)
        partial = cost_lab.find_row(self.rows, "landed-epoch-book-m17")
        self.assertEqual(partial["outputs"]["page_count"], 2)
        self.assertEqual(partial["outputs"]["final_page_order_count"], 1)
        self.assertEqual(partial["outputs"]["padding_record_bytes"], 15 * 99)

    def test_landed_intent_payload_widths(self) -> None:
        expected = {
            "create_market": 139,
            "split": 74,
            "merge": 74,
            "materialize": 107,
            "dematerialize": 107,
            "feed_advance": 74,
            "place_order": 165,
            "cancel_order": 130,
            "settle_page": 68,
        }
        self.assertEqual(set(expected), set(cost_lab.LANDED_INTENT_ORDER))
        for name, width in expected.items():
            intent = self.landed["intents"][name]
            self.assertEqual(intent["bytes"], width, name)
            payload = cost_lab.landed_intent_data(self.constants, name)
            self.assertEqual(len(payload), width, name)
            self.assertEqual(payload[0], intent["intent_tag"], name)
            self.assertEqual(payload[1], intent["intent_version"], name)
            self.assertLessEqual(width, self.landed["bounds"]["max_intent_bytes"], name)

    def test_landed_relation_rows_respect_max_orders(self) -> None:
        seen = set()
        for row in self.rows:
            if row["family"] != "landed_batch_relation":
                continue
            order_count = row["inputs"]["order_count"]
            seen.add(order_count)
            self.assertLessEqual(order_count, 64)
            self.assertEqual(
                row["outputs"]["order_authentications_lower_bound"], order_count
            )
            self.assertEqual(row["outputs"]["portfolio_orders_persistable_in_landed_pages"], 0)
            self.assertEqual(
                row["outputs"]["portfolio_orders_admitted_by_relation_upper_bound"], 8
            )
        self.assertEqual(seen, {16, 32, 64})

    def test_n24_is_a_codec_refusal_in_the_landed_arm(self) -> None:
        landed_n24 = [
            row
            for row in self.rows
            if row["arm"] == cost_lab.ARM_LANDED and row["inputs"].get("outcomes") == 24
        ]
        self.assertEqual(len(landed_n24), 3)
        for row in landed_n24:
            self.assertFalse(row["admission"]["v1_admitted"])
            self.assertIn("landed_codec", row["admission"]["reason"])
            self.assertFalse(row["outputs"]["landed_codec_representable"])

    def test_differential_deltas_are_exact(self) -> None:
        expected = {
            "diff-position-account": (192, 220, 28),
            "diff-supply-ledger-account": (320, 333, 13),
            "diff-single-egg-order-record": (80, 99, 19),
            "diff-order-page-account": (8192, 1819, -6373),
            "diff-order-page-header": (128, 235, 107),
            "diff-order-page-record-capacity": (100, 16, -84),
            "diff-epoch-book-order-capacity": (512, 64, -448),
            "diff-claim-instruction-internal-split": (11, 74, 63),
            "diff-claim-instruction-materialize-one": (11, 107, 96),
        }
        for scenario_id, (hypothesis, landed, delta) in expected.items():
            output = cost_lab.find_row(self.rows, scenario_id)["outputs"]
            self.assertEqual(output["hypothesis_value"], hypothesis, scenario_id)
            self.assertEqual(output["landed_value"], landed, scenario_id)
            self.assertEqual(output["delta"], delta, scenario_id)
            self.assertTrue(output["change"].strip(), scenario_id)
        for scenario_id in ("diff-portfolio-order-record", "diff-accumulator-full-summary"):
            output = cost_lab.find_row(self.rows, scenario_id)["outputs"]
            self.assertIsNone(output["landed_value"], scenario_id)
            self.assertIsNone(output["delta"], scenario_id)
        landed_only = cost_lab.find_row(self.rows, "diff-landed-only-account-family")["outputs"]
        self.assertIsNone(landed_only["hypothesis_value"])
        self.assertEqual(landed_only["landed_value"], 4_298)

    def test_position_rent_delta_is_reported(self) -> None:
        output = cost_lab.find_row(self.rows, "diff-position-account")["outputs"]
        self.assertEqual(output["hypothesis_rent_principal_lamports"], 2_227_200)
        self.assertEqual(output["landed_rent_principal_lamports"], 2_422_080)
        self.assertEqual(output["delta_rent_principal_lamports"], 194_880)

    def test_landed_arm_claims_no_compute_units(self) -> None:
        for row in self.rows:
            if row["arm"] == cost_lab.ARM_HYPOTHESIS:
                continue
            for key in row["outputs"]:
                self.assertFalse(key.endswith("_cu"), f"{row['scenario_id']}:{key}")
            self.assertEqual(
                row["evidence"].get("compute"), "not_measured_no_sbf_program", row["scenario_id"]
            )

    def test_hypothesis_arm_values_are_retained_unchanged(self) -> None:
        bounds = self.constants["dragon_design_bounds"]
        self.assertEqual(bounds["arm"], cost_lab.ARM_HYPOTHESIS)
        self.assertEqual(bounds["single_egg_order_bytes"], 80)
        self.assertEqual(bounds["page_header_bytes"], 128)
        self.assertEqual(bounds["position_header_bytes"], 64)
        self.assertEqual(bounds["page_sizes_bytes"], [4096, 8192, 10240])
        self.assertEqual(bounds["order_counts"], [32, 128, 512])
        page = cost_lab.find_row(self.rows, "page-n16-b8192-m512")
        self.assertEqual(page["arm"], cost_lab.ARM_HYPOTHESIS)
        self.assertEqual(page["outputs"]["single_order_bytes"], 80)
        self.assertEqual(page["outputs"]["half_mix_pages"], 10)

    def test_constants_refuse_a_landed_width_that_is_not_its_field_terms(self) -> None:
        import copy

        broken = copy.deepcopy(self.constants)
        broken[cost_lab.ARM_LANDED]["accounts"]["position"]["bytes"] = 192
        with self.assertRaises(cost_lab.ModelError):
            cost_lab.verify_landed_arm(broken)


class GoldenTests(unittest.TestCase):
    def test_checked_in_artifacts_match_model(self) -> None:
        constants = cost_lab.load_constants()
        rows = cost_lab.generate_rows(constants)
        artifacts = cost_lab.rendered_artifacts(constants, rows)
        cost_lab.check_artifacts(cost_lab.DEFAULT_GOLDEN, artifacts)


if __name__ == "__main__":
    unittest.main()
