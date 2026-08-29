#!/usr/bin/env python3

from __future__ import annotations

import copy
import importlib.util
import json
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = Path(__file__).with_name("ledger.py")
SPEC = importlib.util.spec_from_file_location("economic_lifecycle_ledger", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
LEDGER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = LEDGER
SPEC.loader.exec_module(LEDGER)
PRIVATE_PATH = Path(__file__).with_name("fixtures") / "private-canonical.json"
ACTIVITY_PATH = Path(__file__).with_name("fixtures") / "activity-v3-canonical.json"
OLD_FLAGSHIP_PATH = ROOT / "tools" / "devnet-scenarios" / "fixtures" / "flagship.json"


def loaded(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text())
    assert isinstance(value, dict)
    return value


class EconomicLifecycleLedgerTests(unittest.TestCase):
    def test_private_fixture_closes_exact_supply_claim_fee_and_payout_ledger(self) -> None:
        derived = LEDGER.derive_fixture(loaded(PRIVATE_PATH))
        self.assertEqual(derived["fixtureId"], "private-canonical-four-outcome")
        by_stage = {row["stage"]: row["snapshot"] for row in derived["stageSnapshots"]}
        self.assertEqual(
            by_stage["founding"]["claimAggregateSupplyAtoms"],
            ["500000000"] * 4,
        )
        self.assertEqual(by_stage["founding"]["hoardPrincipalAtoms"], "500000000")
        self.assertEqual(by_stage["direct-hot"]["protocolFeeRevenueAtoms"], "500000")
        self.assertEqual(
            by_stage["direct-hot"]["collateralAccounts"],
            {
                "direct-fee-recipient": "500000",
                "founder-direct-recipient": "49750000",
                "founding-collateral-wallet": "500000000",
                "hoard-principal": "500000000",
                "participant-collateral": "49750000",
                "participant-fixture-source": "0",
            },
        )
        schedule = by_stage["resolution"]["frozenPayoutSchedule"]
        self.assertEqual(len(schedule), 5)
        self.assertEqual(
            [row["quantityAtoms"] for row in schedule],
            ["400000000", "500000000", "500000000", "500000000", "100000000"],
        )
        terminal = by_stage["aggregate-retirement"]
        self.assertEqual(terminal["claimAggregateSupplyAtoms"], ["0"] * 4)
        self.assertEqual(terminal["hoardPrincipalAtoms"], "0")
        self.assertTrue(terminal["retired"])
        self.assertEqual(
            terminal["collateralAccounts"],
            {
                "direct-fee-recipient": "500000",
                "founder-direct-recipient": "449750000",
                "founding-collateral-wallet": "500000000",
                "hoard-principal": "0",
                "participant-collateral": "149750000",
                "participant-fixture-source": "0",
            },
        )

    def test_activity_v3_fixture_is_exact_ten_wallet_authority_and_four_wallet_model(self) -> None:
        derived = LEDGER.derive_fixture(loaded(ACTIVITY_PATH))
        authority = derived["activityV3Authority"]
        self.assertEqual(len(authority["wallets"]), 10)
        self.assertEqual(
            authority["authorization"],
            {
                "initialFundingLamports": "360000000",
                "maxPostInitTransferLamports": "200000000",
                "maxPostInitFeeLamports": "10000000",
                "maxFeeLamports": "10000000",
                "maxSpendLamports": "210000000",
                "guaranteedPreLifecycleResidualLamports": "150000000",
            },
        )
        by_stage = {row["stage"]: row["snapshot"] for row in derived["stageSnapshots"]}
        resolved = by_stage["resolution"]
        self.assertEqual(resolved["claimAggregateSupplyAtoms"], ["2350"] * 4)
        self.assertEqual(resolved["hoardPrincipalAtoms"], "2350")
        self.assertEqual(len(resolved["frozenPayoutSchedule"]), 14)
        terminal = by_stage["aggregate-retirement"]
        self.assertEqual(terminal["protocolFeeRevenueAtoms"], "4")
        self.assertEqual(
            terminal["collateralAccounts"],
            {
                "ash-collateral": "50010",
                "birch-collateral": "50249",
                "cobalt-collateral": "49548",
                "dahlia-collateral": "50189",
                "deployer-fee-recipient": "4",
                "hoard-principal": "0",
            },
        )

    def test_fee_rounding_is_floor_per_side_after_exact_gross_boundary(self) -> None:
        self.assertEqual(
            LEDGER.exact_direct_quote(700, 500, 1000, 50, 10_000),
            {
                "grossCollateralAtoms": 350,
                "grossRemainderAtoms": 0,
                "sellerFeeAtoms": 1,
                "buyerFeeAtoms": 1,
                "sellerNetAtoms": 349,
                "buyerDebitAtoms": 351,
                "feeRecipientCreditAtoms": 2,
            },
        )
        with self.assertRaisesRegex(LEDGER.Refusal, "gross quote"):
            LEDGER.exact_direct_quote(1, 1, 3, 50, 10_000)
        with self.assertRaisesRegex(LEDGER.Refusal, "u128"):
            LEDGER.exact_direct_quote(1 << 127, 3, 1, 50, 10_000)

    def test_retirement_refuses_one_omitted_zero_payout_loser(self) -> None:
        fixture = loaded(PRIVATE_PATH)
        fixture["stages"][-2]["events"].pop(3)
        with self.assertRaisesRegex(LEDGER.Refusal, "exhaustive winning and losing"):
            LEDGER.derive_fixture(fixture)

    def test_redemption_refuses_changed_frozen_quantity_and_winner(self) -> None:
        changed = loaded(PRIVATE_PATH)
        changed["stages"][-2]["events"][0]["quantityAtoms"] = "399999999"
        with self.assertRaisesRegex(LEDGER.Refusal, "frozen live claim"):
            LEDGER.derive_fixture(changed)
        changed = loaded(PRIVATE_PATH)
        changed["stages"][3]["events"][0]["payoutAtomsPerClaim"] = ["0", "1", "0", "0"]
        # A different winner is arithmetically valid, but it changes the exact
        # final collateral snapshot and must be caught by a stage comparison.
        alternative = LEDGER.derive_fixture(changed)
        expected = LEDGER.derive_fixture(loaded(PRIVATE_PATH))
        with self.assertRaisesRegex(LEDGER.Refusal, "differs"):
            LEDGER.check_observed(
                expected,
                {
                    "schema": LEDGER.OBSERVED_SCHEMA,
                    "fixtureSha256": expected["fixtureSha256"],
                    "stage": "payouts",
                    "snapshot": LEDGER.stage_snapshot(alternative, "payouts"),
                },
            )

    def test_supply_fee_destination_and_activity_caps_fail_closed(self) -> None:
        changed = loaded(PRIVATE_PATH)
        changed["collateralMintSupplyAtoms"] = "1099999999"
        with self.assertRaisesRegex(LEDGER.Refusal, "exhaust Mint supply"):
            LEDGER.derive_fixture(changed)
        changed = loaded(PRIVATE_PATH)
        changed["stages"][2]["events"][0]["feeCollateral"] = "hoard-principal"
        with self.assertRaisesRegex(LEDGER.Refusal, "fee destination"):
            LEDGER.derive_fixture(changed)
        changed = loaded(ACTIVITY_PATH)
        changed["activityV3Authority"]["authorization"]["maxFeeLamports"] = "9999999"
        with self.assertRaisesRegex(LEDGER.Refusal, "spend cap"):
            LEDGER.derive_fixture(changed)

    def test_lamport_trace_closes_transfer_rent_refund_and_fee_conservation(self) -> None:
        fixture = loaded(PRIVATE_PATH)
        events = [
            {"kind": "transfer", "stage": "fund", "source": "genesis-mint", "destination": "lifecycle-payer", "lamports": "100000000000"},
            {"kind": "network-fee", "stage": "fund", "payer": "genesis-mint", "lamports": "5000"},
        ]
        for order, (classification, amount) in enumerate(
            (("market", 10), ("rent-credit", 20), ("claims-refund", 30), ("custody-replay", 40), ("hoard-vault", 50))
        ):
            events.append(
                {"kind": "rent-lock", "stage": f"open-{order}", "payer": "lifecycle-payer", "account": f"rent-{order}", "class": classification, "lamports": str(amount)}
            )
        events.append(
            {"kind": "network-fee", "stage": "founding", "payer": "lifecycle-payer", "lamports": "7"}
        )
        for order, (classification, amount) in enumerate(
            (("market", 10), ("rent-credit", 20), ("claims-refund", 30), ("custody-replay", 40), ("hoard-vault", 50))
        ):
            events.append(
                {"kind": "rent-refund", "stage": "aggregate-retirement", "recipient": "lifecycle-payer", "account": f"rent-{order}", "class": classification, "lamports": str(amount)}
            )
        trace = {"schema": LEDGER.LAMPORT_TRACE_SCHEMA, "events": events}
        result = LEDGER.derive_lamport_trace(fixture["lamportContract"], trace)
        self.assertTrue(result["conservation"]["holds"])
        self.assertEqual(result["liveRefundableRentLamports"], "0")
        self.assertEqual(result["totalNetworkFeesLamports"], "5007")
        self.assertEqual(
            result["walletEnvelopes"]["genesis-mint"]["grossDebitLamports"],
            "100000005000",
        )
        hostile = copy.deepcopy(trace)
        hostile["events"][-1]["lamports"] = "51"
        with self.assertRaisesRegex(LEDGER.Refusal, "exact locked"):
            LEDGER.derive_lamport_trace(fixture["lamportContract"], hostile)
        hostile = copy.deepcopy(trace)
        hostile["events"].pop()
        with self.assertRaisesRegex(LEDGER.Refusal, "classification"):
            LEDGER.derive_lamport_trace(fixture["lamportContract"], hostile)

    def test_stage_snapshot_is_fixture_digest_bound(self) -> None:
        derived = LEDGER.derive_fixture(loaded(PRIVATE_PATH))
        observed = {
            "schema": LEDGER.OBSERVED_SCHEMA,
            "fixtureSha256": derived["fixtureSha256"],
            "stage": "direct-hot",
            "snapshot": LEDGER.stage_snapshot(derived, "direct-hot"),
        }
        LEDGER.check_observed(derived, observed)
        observed["fixtureSha256"] = "0" * 64
        with self.assertRaisesRegex(LEDGER.Refusal, "another fixture"):
            LEDGER.check_observed(derived, observed)

    def test_old_flagship_is_explicitly_refused_as_scenario_only(self) -> None:
        authority = loaded(ACTIVITY_PATH)["activityV3Authority"]
        with self.assertRaisesRegex(LEDGER.Refusal, "scenario-only"):
            LEDGER.authenticate_activity_v3_scenario(
                loaded(OLD_FLAGSHIP_PATH), authority
            )

    def test_corrected_scenario_shape_joins_authority_without_weakening_mutations(self) -> None:
        scenario = loaded(OLD_FLAGSHIP_PATH)
        body = scenario["body"]
        body["evidenceLevel"] = "activity-v3-authority"
        authority = loaded(ACTIVITY_PATH)["activityV3Authority"]
        old_by_id = {row["id"]: row for row in body["wallets"]}
        wallets = []
        for expected in authority["wallets"]:
            wallet_id = expected["id"]
            if wallet_id in old_by_id:
                row = old_by_id[wallet_id]
            else:
                row = {
                    "id": wallet_id,
                    "roles": [expected["role"]],
                    "fundingLamports": "0",
                }
            if wallet_id == "deployer":
                row["roles"] = ["campaign-payer", "fee-payer", "fee-recipient", "retirement-beneficiary"]
                row["fundingLamports"] = "360000000"
            elif wallet_id in {"ash", "birch", "cobalt", "dahlia"}:
                row["fundingLamports"] = "50000000"
            wallets.append(row)
        body["wallets"] = wallets
        for operation in body["operations"]:
            operation["mutationExpected"] = True
        result = LEDGER.authenticate_activity_v3_scenario(scenario, authority)
        self.assertEqual(result["walletCount"], 10)
        scenario["body"]["operations"][-1]["mutationExpected"] = False
        with self.assertRaisesRegex(LEDGER.Refusal, "nonmutating"):
            LEDGER.authenticate_activity_v3_scenario(scenario, authority)


if __name__ == "__main__":
    unittest.main()
