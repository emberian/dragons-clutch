#!/usr/bin/env python3
"""Key-free tests for the canonical Activity-v3 producer."""

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest


MODULE_PATH = Path(__file__).with_name("produce_v3.py")
SPEC = importlib.util.spec_from_file_location("dclutch_activity_v3_producer", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
producer = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = producer
SPEC.loader.exec_module(producer)
ledger = producer.load_module("dclutch_activity_v3_test_ledger", producer.LEDGER_PATH)


def load(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


class ActivityV3ProducerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.fixture_path = (
            producer.ROOT
            / "tools/economic-lifecycle-ledger/fixtures/activity-v3-canonical.json"
        )
        self.base_path = (
            producer.ROOT / "tools/devnet-scenarios/fixtures/flagship.json"
        )
        self.fixture = load(self.fixture_path)
        self.base = load(self.base_path)

    def test_scenario_is_the_exact_authority_projection(self) -> None:
        scenario = producer.canonical_scenario(self.base, self.fixture, ledger)
        body = scenario["body"]
        self.assertEqual(body["evidenceLevel"], "authenticated-activity-v3")
        self.assertEqual(len(body["operations"]), 25)
        self.assertTrue(
            all(
                row["mutationExpected"] is True
                and row["callerAvailability"] == "public-executable"
                and row["expectedObservedDelta"] == row["projectedAcceptedDelta"]
                for row in body["operations"]
            )
        )
        self.assertEqual(
            [(row["id"], row["fundingLamports"]) for row in body["wallets"]],
            [
                ("deployer", "360000000"),
                ("collateral-mint", "0"),
                ("collateral-wallet", "0"),
                ("founding-beneficiary", "0"),
                ("founding-projection-witness", "0"),
                ("founding-source-funder", "0"),
                ("ash", "50000000"),
                ("birch", "50000000"),
                ("cobalt", "50000000"),
                ("dahlia", "50000000"),
            ],
        )
        self.assertEqual(
            scenario["bodySha256"], producer.canonical_body_sha256(body)
        )
        self.assertEqual(
            ledger.authenticate_activity_v3_scenario(
                scenario, self.fixture["activityV3Authority"]
            )["status"],
            "accepted",
        )

    def test_manifest_funding_is_derived_not_supplied(self) -> None:
        scenario = producer.canonical_scenario(self.base, self.fixture, ledger)
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            scenario_path = root / "scenario.json"
            scenario_path.write_text(json.dumps(scenario) + "\n", encoding="utf-8")
            bindings = {
                "schema": producer.BINDINGS_SCHEMA,
                "target": {
                    "kind": "devnet",
                    "rpcUrl": "https://api.devnet.solana.com:443/",
                    "devnetGenesisHash": "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG",
                },
                "inputs": [],
                "addressBindings": [],
                "adapters": [],
                "campaignIdentities": [],
                "permanentAuthorityRef": "core-upgrade-authority",
                "foundingAdapter": "founding",
            }
            manifest = producer.canonical_manifest(
                scenario_path, scenario, self.fixture, bindings, ledger
            )
        campaign = manifest["campaign"]
        self.assertEqual(
            campaign["initialFunding"],
            {"walletRef": "deployer", "transferLamports": "360000000"},
        )
        self.assertEqual(
            [(row["walletRef"], row["transferLamports"]) for row in campaign["postInitFunding"]],
            [
                ("ash", "50000000"),
                ("birch", "50000000"),
                ("cobalt", "50000000"),
                ("dahlia", "50000000"),
            ],
        )
        self.assertEqual(
            sum(int(row["transferLamports"]) for row in campaign["postInitFunding"]),
            200_000_000,
        )

    def test_accepted_source_digests_refuse_substitution(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "fixture.json"
            path.write_text("{}\n", encoding="utf-8")
            accepted = hashlib.sha256(path.read_bytes()).hexdigest()
            self.assertEqual(
                producer.accepted_file(str(path), accepted, "fixture"), path.resolve()
            )
            path.write_text('{"changed":true}\n', encoding="utf-8")
            with self.assertRaisesRegex(producer.Refusal, "accepted SHA-256"):
                producer.accepted_file(str(path), accepted, "fixture")

    def test_canonical_manifest_shape_requires_one_terminal_aggregate(self) -> None:
        operation_kinds = (
            [("found", "found")]
            + [(f"participant-{item}", "participant") for item in range(4)]
            + [(f"direct-{item}", "direct") for item in range(4)]
            + [("resolve", "resolve")]
            + [(f"redeem-{item}", "redeem") for item in range(14)]
            + [("retire", "retire")]
        )
        operations = [
            SimpleNamespace(operation_id=operation_id, kind=kind)
            for operation_id, kind in operation_kinds
        ]

        def completion(schema: str, rows: str, label: str) -> SimpleNamespace:
            return SimpleNamespace(
                schema=schema,
                transaction_list_pointer=rows,
                transaction_label_pointer=label,
                transaction_signature_pointer="/signature",
                require_all_transactions_successful=True,
            )

        adapters = [
            SimpleNamespace(
                argv=("campaign",), covers=("found",), progressive=None,
                mutation=True, completion=SimpleNamespace(),
            )
        ]
        adapters.extend(
            SimpleNamespace(
                argv=("devnet-user-position-admission-v1",),
                covers=(f"participant-{item}",), progressive=None,
                mutation=True, completion=SimpleNamespace(),
            )
            for item in range(4)
        )
        adapters.extend(
            SimpleNamespace(
                argv=("devnet-direct-trade-v1",), covers=(f"direct-{item}",),
                progressive=SimpleNamespace(), mutation=True,
                completion=completion(
                    "dclutch-devnet-direct-trade-finalized-v1", "/mutations", "/kind"
                ),
            )
            for item in range(4)
        )
        terminal = SimpleNamespace(
            argv=("devnet-terminal-sequence-v1",),
            covers=tuple(
                operation_id
                for operation_id, kind in operation_kinds
                if kind in {"resolve", "redeem", "retire"}
            ),
            progressive=SimpleNamespace(), mutation=True,
            completion=completion(
                "dclutch-devnet-terminal-sequence-completion-v1",
                "/journals", "/mutation/kind",
            ),
        )
        adapters.append(terminal)
        manifest = SimpleNamespace(
            scenario=SimpleNamespace(operations=tuple(operations)),
            adapters=tuple(adapters),
        )
        producer.authenticate_canonical_manifest_shape(manifest)
        terminal.covers = ("resolve",)
        with self.assertRaisesRegex(producer.Refusal, "terminal coverage"):
            producer.authenticate_canonical_manifest_shape(manifest)


if __name__ == "__main__":
    unittest.main()
