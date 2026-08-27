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
        self.assertEqual(len(self.rows), 263)
        self.assertEqual(len({row["scenario_id"] for row in self.rows}), 263)
        arms = {arm: 0 for arm in cost_lab.ARMS}
        for row in self.rows:
            arms[row["arm"]] += 1
        # The hypothesis arm is retained whole beside the landed arm, never replaced by it.
        self.assertEqual(arms[cost_lab.ARM_HYPOTHESIS], 193)
        self.assertEqual(arms[cost_lab.ARM_LANDED], 58)
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
            "order_page": 4012,
            "supply_ledger": 333,
            "terms": 1656,
            "price_grid": 589,
            "epoch": 329,
            "candidate_record": 337,
            "final_pot": 262,
            "settlement_receipt": 217,
            "resolution": 165,
            "clear_work": 50_054,
            "candidate_feed": 6_266,
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

    def test_pinned_identifiers_are_rederived_from_the_codec_on_disk(self) -> None:
        source = self.landed_source()
        if source is None:
            self.skipTest("landed codec source is not present in this checkout")
        derived = cost_lab.derive_pinned_identifiers_from_source(source)
        self.assertEqual(set(derived), set(cost_lab.RUST_IDENTIFIER_VALUES))
        for name, pinned in cost_lab.RUST_IDENTIFIER_VALUES.items():
            self.assertEqual(
                derived[name],
                pinned,
                f"{name}: the codec moved and the cost lab substitution table is stale",
            )

    def test_abi_audit_reports_no_drift(self) -> None:
        if self.landed_source() is None:
            self.skipTest("landed codec source is not present in this checkout")
        _notes, drift = cost_lab.abi_audit(self.constants)
        self.assertEqual(drift, [])

    def test_abi_expression_evaluator_refuses_anything_but_sizes(self) -> None:
        self.assertEqual(cost_lab.evaluate_rust_arithmetic("2 + (7 * 32) + 1"), 227)
        self.assertEqual(
            cost_lab.evaluate_rust_arithmetic("MAX_ORDERS_PER_PAGE * ORDER_RECORD_BYTES"), 1712
        )
        self.assertEqual(
            cost_lab.evaluate_rust_arithmetic("MAX_ORDERS_PER_PAGE * ORDER_SLOT_BYTES"), 3776
        )
        self.assertEqual(cost_lab.evaluate_rust_arithmetic("MAX_KNOTS * 16"), 256)
        self.assertEqual(
            cost_lab.evaluate_rust_arithmetic("2 + (2 * HASH) + 1", {"HASH": 32}), 67
        )
        for hostile in ("__import__('os')", "1 - 1", "open(1)"):
            with self.assertRaises(cost_lab.ModelError):
                cost_lab.evaluate_rust_arithmetic(hostile)
        # An unpinned identifier is a value the audit can name, not a bare refusal: this is the
        # exception that once aborted the whole gate on MAX_KNOTS.
        with self.assertRaises(cost_lab.UnknownRustToken) as caught:
            cost_lab.evaluate_rust_arithmetic("MAX_UNKNOWN * 8")
        self.assertEqual(caught.exception.token, "MAX_UNKNOWN")

    def test_order_page_geometry_is_forced(self) -> None:
        bounds = self.landed["bounds"]
        self.assertEqual(bounds["order_record_bytes"], 107)
        self.assertEqual(bounds["portfolio_record_bytes"], 235)
        self.assertEqual(bounds["tombstone_record_bytes"], 80)
        self.assertEqual(bounds["order_slot_bytes"], 236)
        self.assertEqual(bounds["max_orders_per_page"], 16)
        self.assertEqual(bounds["max_order_pages"], 4)
        self.assertEqual(bounds["max_epoch_orders"], 64)
        self.assertEqual(bounds["relation_max_orders"], 64)
        self.assertEqual(bounds["max_portfolio_orders"], 8)
        # One slot is a kind byte plus the widest admitted body, and the page is a dense
        # array of those slots: both families share one page, one chain, one fold.
        self.assertEqual(bounds["order_slot_bytes"], 1 + bounds["portfolio_record_bytes"])
        self.assertGreater(bounds["portfolio_record_bytes"], bounds["order_record_bytes"])
        self.assertEqual(
            bounds["order_page_header_bytes"] + 16 * bounds["order_slot_bytes"],
            self.landed["accounts"]["order_page"]["bytes"],
        )
        for order_count, pages in ((1, 1), (16, 1), (17, 2), (32, 2), (48, 3), (64, 4)):
            self.assertEqual(cost_lab.landed_page_count(self.constants, order_count), pages)
        self.assertIsNone(cost_lab.landed_page_count(self.constants, 65))

    def test_one_instance_inventory_totals(self) -> None:
        row = cost_lab.find_row(self.rows, "landed-account-inventory-one-instance")
        self.assertEqual(row["outputs"]["data_bytes"], 65_568)
        self.assertEqual(row["outputs"]["rent_principal_lamports"], 471_498_240)
        self.assertEqual(row["outputs"]["rent_overhead_component_lamports"], 15_144_960)
        self.assertEqual(row["outputs"]["largest_account"], "clear_work")
        self.assertEqual(row["outputs"]["smallest_account"], "realm")

    def test_landed_rent_examples(self) -> None:
        self.assertEqual(cost_lab.rent_minimum(220, self.constants), 2_422_080)
        self.assertEqual(cost_lab.rent_minimum(4_012, self.constants), 28_814_400)
        self.assertEqual(cost_lab.rent_minimum(333, self.constants), 3_208_560)

    def test_epoch_book_refuses_more_than_max_epoch_orders(self) -> None:
        row = cost_lab.find_row(self.rows, "landed-epoch-book-m65")
        self.assertFalse(row["outputs"]["landed_codec_representable"])
        self.assertFalse(row["admission"]["v1_admitted"])
        self.assertIsNone(row["outputs"]["page_count"])
        full = cost_lab.find_row(self.rows, "landed-epoch-book-m64")
        self.assertTrue(full["admission"]["v1_admitted"])
        self.assertEqual(full["outputs"]["page_count"], 4)
        self.assertEqual(full["outputs"]["padding_slot_bytes"], 0)
        self.assertEqual(full["outputs"]["rent_principal_lamports"], 4 * 28_814_400)
        partial = cost_lab.find_row(self.rows, "landed-epoch-book-m17")
        self.assertEqual(partial["outputs"]["page_count"], 2)
        self.assertEqual(partial["outputs"]["final_page_order_count"], 1)
        self.assertEqual(partial["outputs"]["padding_slot_bytes"], 15 * 236)

    def test_landed_intent_payload_widths(self) -> None:
        expected = {
            "create_market": 139,
            "split": 74,
            "merge": 74,
            "materialize": 107,
            "dematerialize": 107,
            "feed_advance": 74,
            "place_order": 310,
            "cancel_order": 138,
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
            self.assertEqual(row["outputs"]["portfolio_orders_persistable_in_landed_pages"], 8)
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
            "diff-single-egg-order-record": (80, 107, 27),
            "diff-order-page-account": (8192, 4012, -4180),
            "diff-order-page-header": (128, 236, 108),
            "diff-order-page-record-capacity": (100, 16, -84),
            "diff-epoch-book-order-capacity": (512, 64, -448),
            "diff-claim-instruction-internal-split": (11, 74, 63),
            "diff-claim-instruction-materialize-one": (11, 107, 96),
            "diff-portfolio-order-record": (208, 235, 27),
        }
        for scenario_id, (hypothesis, landed, delta) in expected.items():
            output = cost_lab.find_row(self.rows, scenario_id)["outputs"]
            self.assertEqual(output["hypothesis_value"], hypothesis, scenario_id)
            self.assertEqual(output["landed_value"], landed, scenario_id)
            self.assertEqual(output["delta"], delta, scenario_id)
            self.assertTrue(output["change"].strip(), scenario_id)
        for scenario_id in ("diff-accumulator-full-summary",):
            output = cost_lab.find_row(self.rows, scenario_id)["outputs"]
            self.assertIsNone(output["landed_value"], scenario_id)
            self.assertIsNone(output["delta"], scenario_id)
        landed_only = cost_lab.find_row(self.rows, "diff-landed-only-account-family")["outputs"]
        self.assertIsNone(landed_only["hypothesis_value"])
        self.assertEqual(landed_only["landed_value"], 61_003)

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

    def test_hypothesis_arm_rows_are_byte_identical_to_their_pinned_digest(self) -> None:
        """The design arm is falsification history: a landed re-pin must not move one byte of it."""

        rows = [row for row in self.rows if row["arm"] == cost_lab.ARM_HYPOTHESIS]
        self.assertEqual(len(rows), cost_lab.RETAINED_HYPOTHESIS_ROW_COUNT)
        self.assertEqual(
            cost_lab.sha256_bytes(cost_lab.canonical_json_bytes(rows)),
            "f2ed6d5345517b65dd3c87410bb0cd5c40baebe709ba5621ca0389cf16104131",
            "the retained layout_hypothesis rows moved; they are kept unchanged so their "
            "falsifications stay readable, and a landed re-pin must not touch them",
        )

    def test_constants_refuse_a_landed_width_that_is_not_its_field_terms(self) -> None:
        import copy

        broken = copy.deepcopy(self.constants)
        broken[cost_lab.ARM_LANDED]["accounts"]["position"]["bytes"] = 192
        with self.assertRaises(cost_lab.ModelError):
            cost_lab.verify_landed_arm(broken)


class AbiAuditHardeningTests(unittest.TestCase):
    """The gate must be loud.

    Every one of these mutates a copy of the real codec source and asserts the audit *reports*
    rather than aborts. The audit spent several commits dead — `refusing to evaluate unknown
    token in ABI expression: MAX_KNOTS`, exit 2, no drift list — because an unpinned identifier
    raised instead of being named.
    """

    @classmethod
    def setUpClass(cls) -> None:
        cls.constants = cost_lab.load_constants()
        path = cost_lab.REPO_ROOT / cls.constants[cost_lab.ARM_LANDED]["source"]["codec_path"]
        cls.source = path.read_text() if path.is_file() else None

    def codec_source(self) -> str:
        if self.source is None:
            self.skipTest("landed codec source is not present in this checkout")
        return self.source

    def test_the_audit_is_clean_on_the_unmutated_source(self) -> None:
        self.assertEqual(cost_lab.abi_drift(self.constants, self.codec_source()), [])

    def test_an_unpinned_identifier_is_a_named_drift_line_not_a_dead_gate(self) -> None:
        source = self.codec_source()
        mutated = source.replace(
            "pub const MAX_KNOTS: usize = 16;",
            "pub const MAX_KNOTS: usize = 16;\npub const MAX_SPLINE_KNOTS: usize = 16;",
            1,
        ).replace("+ (MAX_KNOTS * 16)", "+ (MAX_SPLINE_KNOTS * 16)", 1)
        self.assertNotEqual(mutated, source)
        drift = cost_lab.abi_drift(self.constants, mutated)
        named = [line for line in drift if "MAX_SPLINE_KNOTS" in line]
        self.assertTrue(named, drift)
        self.assertTrue(
            any("account_len::TERMS" in line for line in named),
            "the drift line must name the constant that referenced the unpinned token",
        )
        self.assertTrue(
            any(cost_lab.PIN_TABLE in line and '"MAX_SPLINE_KNOTS": 16' in line for line in named),
            "the drift line must name the pin-table fix and the codec's own value",
        )

    def test_an_unresolvable_token_does_not_stop_the_rest_of_the_audit(self) -> None:
        source = self.codec_source()
        mutated = source.replace("+ (MAX_KNOTS * 16)", "+ (MAX_MYSTERY * 16)", 1).replace(
            "pub const MAX_GRID_TICKS: usize = 64;", "pub const MAX_GRID_TICKS: usize = 65;", 1
        )
        self.assertNotEqual(mutated, source)
        drift = cost_lab.abi_drift(self.constants, mutated)
        self.assertTrue(
            any("MAX_MYSTERY" in line and "account_len::TERMS" in line for line in drift), drift
        )
        self.assertTrue(
            any("MAX_GRID_TICKS: codec says 65" in line for line in drift),
            "an unreadable expression must not abort the audit before the next check",
        )

    def test_a_moved_identifier_reports_both_the_identifier_and_the_width(self) -> None:
        """A referenced identifier is compared to its pin, never substituted for it.

        Substituting the pin is what hid the v4 page: `ORDER_SLOT_BYTES` 228 -> 236 read as no
        identifier drift at all and `ORDER_PAGE` moved by the one header byte instead of 129.
        """

        source = self.codec_source()
        mutated = source.replace(
            "pub const ORDER_SLOT_BYTES: usize = 1 + PORTFOLIO_RECORD_BYTES;",
            "pub const ORDER_SLOT_BYTES: usize = 2 + PORTFOLIO_RECORD_BYTES;",
            1,
        )
        self.assertNotEqual(mutated, source)
        drift = cost_lab.abi_drift(self.constants, mutated)
        self.assertIn("ORDER_SLOT_BYTES: codec says 237, cost lab pins 236", drift)
        self.assertTrue(
            any(
                "account_len::ORDER_PAGE references ORDER_SLOT_BYTES" in line
                and "codec says 237" in line
                for line in drift
            ),
            drift,
        )
        # 236 header + 16 * 237: the whole move, not the masked one-byte remainder.
        self.assertIn("order_page: codec says 4028 bytes, cost lab pins 4012", drift)

    def test_a_lockstep_move_through_a_derived_identifier_is_still_reported(self) -> None:
        source = self.codec_source()
        mutated = source.replace(
            "pub const PORTFOLIO_RECORD_BYTES: usize = 32 + 32 + 1 + 1 + 1 + (MAX_OUTCOMES * 8) + (5 * 8);",
            "pub const PORTFOLIO_RECORD_BYTES: usize = 32 + 32 + 1 + 1 + 1 + (MAX_OUTCOMES * 8) + (6 * 8);",
            1,
        )
        self.assertNotEqual(mutated, source)
        drift = cost_lab.abi_drift(self.constants, mutated)
        self.assertIn("PORTFOLIO_RECORD_BYTES: codec says 243, cost lab pins 235", drift)
        # ORDER_SLOT_BYTES is `1 + PORTFOLIO_RECORD_BYTES`, so it moves without its own edit.
        self.assertIn("ORDER_SLOT_BYTES: codec says 244, cost lab pins 236", drift)
        self.assertIn("order_page: codec says 4140 bytes, cost lab pins 4012", drift)

    def test_a_rustfmt_wrapped_declaration_is_not_drift(self) -> None:
        """Formatting is not an ABI change; the parser used to die on it with a SyntaxError."""

        source = self.codec_source()
        wrapped = source.replace(
            "pub const ORDER_SLOT_BYTES: usize = 1 + PORTFOLIO_RECORD_BYTES;",
            "pub const ORDER_SLOT_BYTES: usize =\n    1 + PORTFOLIO_RECORD_BYTES;",
            1,
        )
        self.assertNotEqual(wrapped, source)
        self.assertEqual(
            cost_lab.derive_pinned_identifiers_from_source(wrapped)["ORDER_SLOT_BYTES"], 236
        )
        self.assertEqual(cost_lab.abi_drift(self.constants, wrapped), [])

    def test_a_declaration_inside_a_comment_is_not_a_declaration(self) -> None:
        source = self.codec_source()
        commented = source.replace(
            "pub const ORDER_SLOT_BYTES: usize = 1 + PORTFOLIO_RECORD_BYTES;",
            "/* pub const ORDER_SLOT_BYTES: usize = 999; */\n"
            "pub const ORDER_SLOT_BYTES: usize = 1 + PORTFOLIO_RECORD_BYTES;",
            1,
        )
        self.assertNotEqual(commented, source)
        self.assertEqual(cost_lab.abi_drift(self.constants, commented), [])

    def test_the_multi_line_account_length_is_read_whole(self) -> None:
        source = self.codec_source()
        self.assertIn("pub const TERMS: usize = 2\n", source, "TERMS is the wrapped-const case")
        self.assertEqual(cost_lab.derive_account_lengths_from_source(source)["TERMS"], 1_656)

    def test_intent_widths_are_read_from_the_codec_match_arms(self) -> None:
        arms = cost_lab.derive_intent_lengths_from_source(self.codec_source())
        self.assertIn("PlaceOrder.Portfolio", arms)
        self.assertIn("PlaceOrder.Single", arms)
        self.assertEqual(
            cost_lab.evaluate_rust_arithmetic(arms["PlaceOrder.Single"]), 182
        )
        self.assertEqual(
            cost_lab.evaluate_rust_arithmetic(arms["PlaceOrder.Portfolio"]), 310
        )
        mutated = self.codec_source().replace(
            "Self::CancelOrder { .. } => 2 + 32 + 32 + 32 + 32 + 8,",
            "Self::CancelOrder { .. } => 2 + 32 + 32 + 32 + 32 + 8 + 8,",
            1,
        )
        drift = cost_lab.abi_drift(self.constants, mutated)
        self.assertIn("intent cancel_order: codec says 146 bytes, cost lab pins 138", drift)

    def test_cross_module_widths_use_only_explicitly_pinned_paths(self) -> None:
        source = self.codec_source()
        lengths = cost_lab.derive_account_lengths_from_source(source)
        self.assertEqual(lengths["CLEAR_WORK"], 50_054)
        self.assertEqual(lengths["CANDIDATE_FEED"], 6_266)
        arms = cost_lab.derive_intent_lengths_from_source(source)
        self.assertEqual(
            cost_lab.evaluate_rust_arithmetic(arms["BeginResolutionWork"]), 83
        )
        self.assertEqual(cost_lab.evaluate_rust_arithmetic(arms["FoldResolutionWork"]), 107)
        self.assertEqual(
            cost_lab.evaluate_rust_arithmetic(arms["WriteArtifact"]), 263
        )
        with self.assertRaises(cost_lab.UnknownRustToken):
            cost_lab.evaluate_rust_arithmetic("artifact::UNREVIEWED_CHUNK_BYTES")
        mutated = source.replace(
            "Self::WriteArtifact { .. } => 2 + 1 + 32 + 32 + 2 + 2 + artifact::ARTIFACT_CHUNK_BYTES,",
            "Self::WriteArtifact { .. } => 2 + 1 + 32 + 32 + 2 + 2 + artifact::UNREVIEWED_CHUNK_BYTES,",
            1,
        )
        drift = cost_lab.abi_drift(self.constants, mutated)
        self.assertTrue(
            any(
                "Intent::WriteArtifact" in line and "artifact::UNREVIEWED_CHUNK_BYTES" in line
                for line in drift
            ),
            drift,
        )

    def test_stale_v2_placement_widths_are_named_drift(self) -> None:
        import copy

        stale = copy.deepcopy(self.constants)
        bounds = stale[cost_lab.ARM_LANDED]["bounds"]
        bounds["max_intent_bytes"] = 302
        bounds["max_intent_field_terms"] = [2, 64, 1, 235]
        intent = stale[cost_lab.ARM_LANDED]["intents"]["place_order"]
        intent["bytes"] = 302
        intent["field_terms"] = [2, 32, 32, 1, 235]
        intent["formula"] = "2 + 32 + 32 + 1 + PORTFOLIO_RECORD_BYTES"
        drift = cost_lab.abi_drift(stale, self.codec_source())
        self.assertIn(
            "bounds.max_intent_bytes: codec says 402 for MAX_INTENT_BYTES, constants.json pins 302",
            drift,
        )
        self.assertIn("intent place_order: codec says 310 bytes, cost lab pins 302", drift)

    def test_a_lumped_field_term_list_is_drift_even_when_it_sums(self) -> None:
        import copy

        broken = copy.deepcopy(self.constants)
        account = broken[cost_lab.ARM_LANDED]["accounts"]["order_page"]
        account["field_terms"] = [sum(account["field_terms"])]
        cost_lab.verify_landed_arm(broken)  # the sum still closes, so this alone is not enough
        drift = cost_lab.abi_drift(broken, self.codec_source())
        self.assertTrue(
            any("account_len::ORDER_PAGE" in line and "field terms" in line for line in drift),
            drift,
        )

    def test_a_moved_account_version_or_discriminator_is_drift(self) -> None:
        import copy

        broken = copy.deepcopy(self.constants)
        broken[cost_lab.ARM_LANDED]["accounts"]["order_page"]["schema_version"] = 3
        broken[cost_lab.ARM_LANDED]["accounts"]["terms"]["discriminator_tag"] = 11
        drift = cost_lab.abi_drift(broken, self.codec_source())
        self.assertIn(
            "order_page: codec writes schema version 4, cost lab pins 3", drift
        )
        self.assertIn("terms: codec discriminator TERMS_TAG is 10, cost lab pins 11", drift)


class GoldenTests(unittest.TestCase):
    def test_checked_in_artifacts_match_model(self) -> None:
        constants = cost_lab.load_constants()
        rows = cost_lab.generate_rows(constants)
        artifacts = cost_lab.rendered_artifacts(constants, rows)
        cost_lab.check_artifacts(cost_lab.DEFAULT_GOLDEN, artifacts)


if __name__ == "__main__":
    unittest.main()
