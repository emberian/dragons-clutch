#!/usr/bin/env python3

from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import unittest


DIRECTORY = Path(__file__).resolve().parent
sys.path.insert(0, str(DIRECTORY))
SPEC = importlib.util.spec_from_file_location("economic_multiwallet", DIRECTORY / "multiwallet.py")
assert SPEC is not None and SPEC.loader is not None
MODEL = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODEL
SPEC.loader.exec_module(MODEL)
CONTRACT_PATH = DIRECTORY / "fixtures" / "multiwallet-20-seeds.json"
EXPECTED_PATH = DIRECTORY / "fixtures" / "multiwallet-20-seeds.expected.json"


def contract() -> dict[str, object]:
    value = json.loads(CONTRACT_PATH.read_text())
    assert isinstance(value, dict)
    return value


def scenario(derived: dict[str, object], seed_name: str) -> dict[str, object]:
    rows = [row for row in derived["scenarios"] if row["seedName"] == seed_name]
    assert len(rows) == 1
    return rows[0]


class MultiwalletModelTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.derived = MODEL.derive_contract(contract())

    def test_exact_twenty_named_seed_domain_and_persisted_oracle(self) -> None:
        self.assertEqual(self.derived["scenarioCount"], 20)
        self.assertEqual(
            [row["seedName"] for row in self.derived["scenarios"]],
            [f"seed-{index:02d}" for index in range(1, 21)],
        )
        for row in self.derived["scenarios"]:
            expected = hashlib.sha256(
                b"dclutch/private-validator-lifecycle/named-seed/v1\0"
                + row["seedName"].encode()
            ).hexdigest()
            self.assertEqual(row["seedSha256"], expected)
        persisted = MODEL.load_json(EXPECTED_PATH)
        self.assertEqual(persisted, self.derived)
        self.assertEqual(
            MODEL.economic.canonical_bytes(MODEL.derive_contract(contract())),
            MODEL.economic.canonical_bytes(self.derived),
        )

    def test_every_transition_is_digest_bound_and_conserves_exact_integers(self) -> None:
        for case in self.derived["scenarios"]:
            previous = None
            for ordinal, transition in enumerate(case["transitions"]):
                self.assertEqual(transition["ordinal"], ordinal)
                self.assertEqual(
                    transition["postSnapshotSha256"], MODEL.digest(transition["snapshot"])
                )
                if previous is not None:
                    self.assertEqual(transition["preSnapshotSha256"], MODEL.digest(previous))
                snapshot = transition["snapshot"]
                conserved = transition["conservation"]
                self.assertEqual(
                    conserved["collateralAccountSumAtoms"],
                    conserved["collateralMintSupplyAtoms"],
                )
                self.assertEqual(
                    conserved["claimPositionSumsAtoms"],
                    conserved["claimAggregateSupplyAtoms"],
                )
                for liability in conserved["backedLiabilityAtoms"]:
                    self.assertEqual(liability, conserved["hoardPrincipalAtoms"])
                self.assertTrue(conserved["holds"])
                if transition["expectedStatus"] in {"refused", "checkpoint"}:
                    self.assertEqual(
                        transition["preSnapshotSha256"], transition["postSnapshotSha256"]
                    )
                previous = snapshot

    def test_crossed_owner_nonces_are_gap_free_and_replays_do_not_mutate(self) -> None:
        baseline = scenario(self.derived, "seed-01")
        accepted = baseline["summary"]["acceptedDirectActions"]
        self.assertEqual(
            [
                (
                    row["action"]["sellerOwner"], row["action"]["sellerNonce"],
                    row["action"]["buyerOwner"], row["action"]["buyerNonce"],
                )
                for row in accepted
            ],
            [
                ("ash", 0, "birch", 0),
                ("birch", 1, "cobalt", 0),
                ("cobalt", 1, "dahlia", 0),
                ("dahlia", 1, "ash", 1),
            ],
        )
        for seed_name, code in (
            ("seed-02", "seller-nonce-mismatch"),
            ("seed-03", "buyer-nonce-mismatch"),
            ("seed-04", "duplicate-paired-intent"),
            ("seed-18", "seller-nonce-mismatch"),
        ):
            case = scenario(self.derived, seed_name)
            refused = [row for row in case["transitions"] if row["expectedStatus"] == "refused"]
            self.assertEqual([row["refusalCode"] for row in refused], [code])
            self.assertEqual(refused[0]["preSnapshotSha256"], refused[0]["postSnapshotSha256"])

    def test_simultaneous_actions_have_one_canonical_winner_and_one_stale_refusal(self) -> None:
        for seed_name, group, refusal in (
            ("seed-05", "simultaneous-seller-canonical-order", "seller-nonce-mismatch"),
            ("seed-06", "simultaneous-buyer-canonical-order", "buyer-nonce-mismatch"),
        ):
            case = scenario(self.derived, seed_name)
            rows = [row for row in case["transitions"] if row["dispatchGroup"] == group]
            self.assertEqual(len(rows), 2)
            self.assertEqual([row["expectedStatus"] for row in rows], ["accepted", "refused"])
            self.assertEqual(rows[1]["refusalCode"], refusal)
            intent_ids = [row["details"]["action"]["pairedIntentId"] for row in rows]
            self.assertEqual(intent_ids, sorted(intent_ids))
            self.assertEqual(rows[1]["preSnapshotSha256"], rows[1]["postSnapshotSha256"])

    def test_signed_collateral_accounts_can_switch_but_foreign_account_refuses(self) -> None:
        seller_switch = scenario(self.derived, "seed-07")["summary"]["acceptedDirectActions"]
        birch_routes = [
            (row["action"]["buyerCollateral"], row["action"]["sellerCollateral"])
            for row in seller_switch
            if row["action"]["buyerOwner"] == "birch" or row["action"]["sellerOwner"] == "birch"
        ]
        self.assertIn(("birch-primary", "ash-primary"), birch_routes)
        self.assertIn(("cobalt-primary", "birch-alternate"), birch_routes)
        buyer_switch = scenario(self.derived, "seed-08")["summary"]["acceptedDirectActions"]
        ash_accounts = {
            row["action"]["sellerCollateral"]
            if row["action"]["sellerOwner"] == "ash"
            else row["action"]["buyerCollateral"]
            for row in buyer_switch
            if "ash" in (row["action"]["sellerOwner"], row["action"]["buyerOwner"])
        }
        self.assertEqual(ash_accounts, {"ash-primary", "ash-alternate"})
        foreign = scenario(self.derived, "seed-09")
        self.assertEqual(
            foreign["summary"]["refusalCodes"], ["seller-collateral-owner-mismatch"]
        )

    def test_exact_winner_boundaries_and_provider_failure(self) -> None:
        expected = {
            "seed-10": ("11999", 0),
            "seed-11": ("12000", 1),
            "seed-12": ("17999", 1),
            "seed-13": ("18000", 2),
            "seed-14": ("18001", 2),
        }
        for seed_name, (numerator, winner) in expected.items():
            facts = scenario(self.derived, seed_name)["winner"]
            self.assertEqual(facts["priceNumerator"], numerator)
            self.assertEqual(facts["selectedOutcome"], winner)
            self.assertEqual(facts["providerStatus"], "success")
        failure = scenario(self.derived, "seed-15")["winner"]
        self.assertIsNone(failure["priceNumerator"])
        self.assertEqual(failure["selectedOutcome"], 3)
        self.assertEqual(failure["providerStatus"], "failure")

    def test_fee_rounding_boundaries_are_floor_per_side(self) -> None:
        below = scenario(self.derived, "seed-16")["summary"]["acceptedDirectActions"][0]
        at = scenario(self.derived, "seed-17")["summary"]["acceptedDirectActions"][0]
        self.assertEqual(
            (below["quote"]["grossCollateralAtoms"], below["quote"]["sellerFeeAtoms"]),
            ("199", "0"),
        )
        self.assertEqual(
            (at["quote"]["grossCollateralAtoms"], at["quote"]["sellerFeeAtoms"]),
            ("200", "1"),
        )
        self.assertEqual(below["quote"]["feeRecipientCreditAtoms"], "0")
        self.assertEqual(at["quote"]["feeRecipientCreditAtoms"], "2")

    def test_every_frozen_row_burns_and_zero_payout_losers_are_not_omitted(self) -> None:
        for case in self.derived["scenarios"]:
            summary = case["summary"]
            payouts = [
                row for row in case["transitions"]
                if row["kind"] == "full-frozen-row-redemption"
            ]
            self.assertEqual(len(payouts), summary["frozenScheduleRows"])
            self.assertEqual(
                sum(row["details"]["payoutCollateralAtoms"] == "0" for row in payouts),
                summary["zeroPayoutBurnRows"],
            )
            self.assertGreater(summary["zeroPayoutBurnRows"], 0)
            terminal = case["transitions"][-1]["snapshot"]["economic"]
            self.assertEqual(terminal["claimAggregateSupplyAtoms"], ["0"] * 4)
            self.assertEqual(terminal["hoardPrincipalAtoms"], "0")
            self.assertTrue(terminal["retired"])

    def test_partial_payout_resume_and_premature_retirement_are_exact_frontiers(self) -> None:
        resumed = scenario(self.derived, "seed-19")
        checkpoints = [
            row for row in resumed["transitions"] if row["actionId"] == "payout-resume-frontier"
        ]
        self.assertEqual(len(checkpoints), 1)
        checkpoint = checkpoints[0]
        self.assertEqual(checkpoint["details"]["completedRows"], 7)
        self.assertEqual(checkpoint["details"]["remainingRows"], 7)
        self.assertEqual(checkpoint["preSnapshotSha256"], checkpoint["postSnapshotSha256"])
        premature = scenario(self.derived, "seed-20")
        retirement = [
            row for row in premature["transitions"]
            if row["actionId"] == "retirement-before-zero"
        ]
        self.assertEqual(len(retirement), 1)
        self.assertEqual(retirement[0]["refusalCode"], "retirement-before-zero")
        self.assertEqual(retirement[0]["preSnapshotSha256"], retirement[0]["postSnapshotSha256"])

    def test_observed_check_is_contract_seed_ordinal_and_snapshot_bound(self) -> None:
        expected = scenario(self.derived, "seed-05")["transitions"][3]
        observed = {
            "schema": MODEL.OBSERVED_SCHEMA,
            "contractSha256": self.derived["contractSha256"],
            "seedName": "seed-05",
            "ordinal": 3,
            "snapshot": copy.deepcopy(expected["snapshot"]),
        }
        result = MODEL.check_observed(self.derived, observed)
        self.assertEqual(result["snapshotSha256"], expected["postSnapshotSha256"])
        hostile = copy.deepcopy(observed)
        hostile["snapshot"]["control"]["makerNextNonces"]["ash"] = "999"
        with self.assertRaisesRegex(MODEL.economic.Refusal, "differs"):
            MODEL.check_observed(self.derived, hostile)
        hostile = copy.deepcopy(observed)
        hostile["contractSha256"] = "0" * 64
        with self.assertRaisesRegex(MODEL.economic.Refusal, "another contract"):
            MODEL.check_observed(self.derived, hostile)

    def test_contract_refuses_seed_and_source_fixture_substitution(self) -> None:
        for mutation in ("missing", "duplicate", "reordered"):
            hostile = contract()
            if mutation == "missing":
                hostile["seeds"].pop()
            elif mutation == "duplicate":
                hostile["seeds"][1] = copy.deepcopy(hostile["seeds"][0])
            else:
                hostile["seeds"][0], hostile["seeds"][1] = hostile["seeds"][1], hostile["seeds"][0]
            with self.assertRaisesRegex(MODEL.economic.Refusal, "twenty|missing, duplicated, or reordered"):
                MODEL.derive_contract(hostile)
        hostile = contract()
        hostile["sourceFixtures"][1]["sha256"] = "0" * 64
        with self.assertRaisesRegex(MODEL.economic.Refusal, "digest changed"):
            MODEL.derive_contract(hostile)


if __name__ == "__main__":
    unittest.main()
