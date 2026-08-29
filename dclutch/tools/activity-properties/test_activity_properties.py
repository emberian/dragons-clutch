import copy
import hashlib
import importlib.util
import pathlib
import tempfile
import unittest


HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def load(name: str, path: pathlib.Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


properties = load("dclutch_activity_properties", HERE / "activity_properties.py")
fixtures = load(
    "dclutch_reconcile_fixtures",
    ROOT / "tools" / "devnet-reconcile" / "tests" / "test_reconcile.py",
)
reconcile = properties.reconcile
BASE_DOSSIER = None


def refresh(dossier):
    dossier.pop("dossierSha256", None)
    dossier["dossierSha256"] = reconcile.sha256_bytes(reconcile.canonical_bytes(dossier))
    return dossier


def exact_retirement(dossier):
    event = next(row for row in dossier["events"] if row["kind"] == "retirement")
    event["lamportDeltas"].append({"account": "closed_protocol", "lamports": "-10000"})
    event["lamportObservations"].append(
        {
            "account": "closed_protocol",
            "beforeLamports": "10000",
            "afterLamports": "0",
            "deltaLamports": "-10000",
        }
    )
    return refresh(dossier)


def fixture_dossier():
    global BASE_DOSSIER
    if BASE_DOSSIER is None:
        with tempfile.TemporaryDirectory() as directory:
            manifest, capture, _, _, _, _, _ = fixtures.owned_loopback_fixture(pathlib.Path(directory))
            BASE_DOSSIER = exact_retirement(
                reconcile.reconcile_owned_loopback(
                    manifest,
                    reconcile.OwnedLoopbackCapturedRpc(capture),
                    {},
                )
            )
    return copy.deepcopy(BASE_DOSSIER)


def set_source(dossier, event, digest):
    event["sourceSha256"] = digest
    for row in dossier["evidence"]["sourceDigests"]:
        if row["event"] == event["id"]:
            row["sha256"] = digest


def independent_clone(dossier, activity_index=2):
    clone = copy.deepcopy(dossier)
    clone["activityId"] = f"fixture-complete-lifecycle-{activity_index}"
    address_map = {}
    for index, account in enumerate(clone["accounts"]):
        address = fixtures.key(110 + index)[0]
        address_map[account["ref"]] = address
        account["address"] = address
    for final in clone["finalAccounts"]:
        final["address"] = address_map[final["account"]]
    for index, event in enumerate(clone["events"]):
        event["signature"] = f"clone-{activity_index}-{index}-{event['signature']}"
        event["slot"] = str(int(event["slot"]) + activity_index * 1000)
    direct = next(event for event in clone["events"] if event["kind"] == "direct")
    set_source(clone, direct, hashlib.sha256(f"direct-{activity_index}".encode()).hexdigest())
    return refresh(clone)


def shift_position_revisions(dossier, ref, offset):
    for event in dossier["events"]:
        for position in event.get("positions", []):
            if position["account"] == ref:
                position["pre"]["revision"] = str(int(position["pre"]["revision"]) + offset)
                position["post"]["revision"] = str(int(position["post"]["revision"]) + offset)
        position = event.get("position")
        if position is not None and position["account"] == ref:
            position["pre"]["revision"] = str(int(position["pre"]["revision"]) + offset)
            position["post"]["revision"] = str(int(position["post"]["revision"]) + offset)
    final = next(row for row in dossier["finalAccounts"] if row["account"] == ref)
    final["position"]["revision"] = str(int(final["position"]["revision"]) + offset)


class ActivityPropertiesTest(unittest.TestCase):
    def test_exact_whole_lifecycle_conservation_holds(self):
        report = properties.validate_many([fixture_dossier()])
        self.assertEqual(report["status"], "holds")
        self.assertEqual(report["totals"]["transactionFeesLamports"], "30000")
        self.assertEqual(report["totals"]["protocolFeesAtoms"], "20")
        self.assertEqual(report["totals"]["hoardPrincipalPaidAtoms"], "50")
        self.assertEqual(report["totals"]["closedRentLamports"], "10000")
        self.assertEqual(report["totals"]["refundLamports"], "10000")

    def test_lamport_creation_or_leak_refuses(self):
        dossier = fixture_dossier()
        event = next(row for row in dossier["events"] if row["kind"] == "participant")
        event["lamportDeltas"][0]["lamports"] = "-4999"
        event["lamportObservations"][0]["afterLamports"] = str(
            int(event["lamportObservations"][0]["beforeLamports"]) - 4999
        )
        event["lamportObservations"][0]["deltaLamports"] = "-4999"
        refresh(dossier)
        with self.assertRaisesRegex(properties.Refusal, "leaks or creates lamports"):
            properties.validate_many([dossier])

    def test_retirement_refund_substitution_refuses_even_when_fee_balances(self):
        dossier = fixture_dossier()
        event = next(row for row in dossier["events"] if row["kind"] == "retirement")
        refund_delta = next(row for row in event["lamportDeltas"] if row["account"] == "refund")
        refund_observation = next(row for row in event["lamportObservations"] if row["account"] == "refund")
        payer_delta = next(row for row in event["lamportDeltas"] if row["account"] == "payer")
        payer_observation = next(row for row in event["lamportObservations"] if row["account"] == "payer")
        event["retirement"]["refundLamports"][0]["lamports"] = "9999"
        refund_delta["lamports"] = "9999"
        refund_observation["afterLamports"] = str(int(refund_observation["beforeLamports"]) + 9999)
        refund_observation["deltaLamports"] = "9999"
        payer_delta["lamports"] = "-4999"
        payer_observation["afterLamports"] = str(int(payer_observation["beforeLamports"]) - 4999)
        payer_observation["deltaLamports"] = "-4999"
        refresh(dossier)
        with self.assertRaisesRegex(properties.Refusal, "rent removed.*differs"):
            properties.validate_many([dossier])

    def test_scaled_integer_direct_substitution_refuses(self):
        dossier = fixture_dossier()
        direct = next(event["direct"] for event in dossier["events"] if "direct" in event)
        direct["grossAtoms"] = "1999"
        refresh(dossier)
        with self.assertRaisesRegex(properties.Refusal, "grossAtoms differs"):
            properties.validate_many([dossier])

    def test_payout_principal_substitution_refuses(self):
        dossier = fixture_dossier()
        payout = next(event["payout"] for event in dossier["events"] if "payout" in event)
        payout["principalAtoms"] = "49"
        refresh(dossier)
        with self.assertRaisesRegex(properties.Refusal, "Hoard principal"):
            properties.validate_many([dossier])

    def test_missing_phase_refuses(self):
        dossier = fixture_dossier()
        events = dossier["events"]
        removed = next(index for index, event in enumerate(events) if event["kind"] == "resolution")
        del events[removed]
        events[removed]["predecessor"] = events[removed - 1]["id"]
        refresh(dossier)
        with self.assertRaisesRegex(properties.Refusal, "whole lifecycle|discontinuous"):
            properties.validate_many([dossier])

    def test_independent_multiwallet_lifecycles_hold(self):
        first = fixture_dossier()
        second = independent_clone(first)
        report = properties.validate_many([first, second])
        self.assertEqual(report["multiwallet"]["status"], "holds")
        self.assertEqual(len(report["multiwallet"]["payerAddresses"]), 2)
        self.assertEqual(report["totals"]["transactionFeesLamports"], "60000")

    def test_duplicate_direct_semantic_owner_refuses(self):
        first = fixture_dossier()
        second = independent_clone(first)
        first_direct = next(event for event in first["events"] if event["kind"] == "direct")
        second_direct = next(event for event in second["events"] if event["kind"] == "direct")
        set_source(second, second_direct, first_direct["sourceSha256"])
        refresh(second)
        with self.assertRaisesRegex(properties.Refusal, "replays a Direct"):
            properties.validate_many([first, second])

    def test_duplicate_transaction_signature_refuses(self):
        first = fixture_dossier()
        second = independent_clone(first)
        second["events"][0]["signature"] = first["events"][0]["signature"]
        refresh(second)
        with self.assertRaisesRegex(properties.Refusal, "replays a transaction signature"):
            properties.validate_many([first, second])

    def test_multiwallet_fee_payer_alias_refuses(self):
        first = fixture_dossier()
        second = independent_clone(first)
        first_payer = next(row for row in first["accounts"] if row["ref"] == "payer")
        second_payer = next(row for row in second["accounts"] if row["ref"] == "payer")
        second_payer["address"] = first_payer["address"]
        refresh(second)
        with self.assertRaisesRegex(properties.Refusal, "aliases a disposable fee-payer"):
            properties.validate_many([first, second])

    def test_event_source_must_match_dossier_evidence(self):
        dossier = fixture_dossier()
        direct = next(event for event in dossier["events"] if event["kind"] == "direct")
        direct["sourceSha256"] = hashlib.sha256(b"substitution").hexdigest()
        refresh(dossier)
        with self.assertRaisesRegex(properties.Refusal, "source evidence differs"):
            properties.validate_many([dossier])

    def test_public_dossier_classification_refuses_without_both_positions(self):
        dossier = fixture_dossier()
        dossier["schema"] = reconcile.DOSSIER_SCHEMA
        dossier["cluster"] = {"kind": "devnet", "genesisHash": reconcile.DEVNET_GENESIS_HASH}
        refresh(dossier)
        with self.assertRaisesRegex(properties.Refusal, "schema or signature"):
            properties.validate_many([dossier])

    def test_final_position_advance_refuses(self):
        dossier = fixture_dossier()
        final = next(row for row in dossier["finalAccounts"] if row["account"] == "position")
        final["position"]["revision"] = "4"
        refresh(dossier)
        with self.assertRaisesRegex(properties.Refusal, "advanced outside"):
            properties.validate_many([dossier])

    def test_shared_position_duplicate_nonce_refuses(self):
        first = fixture_dossier()
        second = independent_clone(first)
        first_position = next(row for row in first["accounts"] if row["ref"] == "position")
        second_position = next(row for row in second["accounts"] if row["ref"] == "position")
        second_position["address"] = first_position["address"]
        next(row for row in second["finalAccounts"] if row["account"] == "position")["address"] = first_position["address"]
        refresh(second)
        with self.assertRaisesRegex(properties.Refusal, "replays a revision nonce"):
            properties.validate_many([first, second])

    def test_shared_position_crossed_nonce_refuses(self):
        first = fixture_dossier()
        second = independent_clone(first)
        first_position = next(row for row in first["accounts"] if row["ref"] == "position")
        second_position = next(row for row in second["accounts"] if row["ref"] == "position")
        second_position["address"] = first_position["address"]
        next(row for row in second["finalAccounts"] if row["account"] == "position")["address"] = first_position["address"]
        shift_position_revisions(second, "position", 4)
        refresh(second)
        with self.assertRaisesRegex(properties.Refusal, "crossed or missing revision"):
            properties.validate_many([first, second])

    def test_shared_token_crossed_transition_refuses(self):
        first = fixture_dossier()
        second = independent_clone(first)
        first_token = next(row for row in first["accounts"] if row["ref"] == "seller_token")
        second_token = next(row for row in second["accounts"] if row["ref"] == "seller_token")
        second_token["address"] = first_token["address"]
        next(row for row in second["finalAccounts"] if row["account"] == "seller_token")["address"] = first_token["address"]
        refresh(second)
        with self.assertRaisesRegex(properties.Refusal, "crossed or missing concurrent transition"):
            properties.validate_many([first, second])


if __name__ == "__main__":
    unittest.main()
