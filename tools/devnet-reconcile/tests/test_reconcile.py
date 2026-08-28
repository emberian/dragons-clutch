import base64
import copy
import hashlib
import importlib.util
import json
import pathlib
import struct
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("dclutch_devnet_reconcile", ROOT / "reconcile.py")
reconcile = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(reconcile)


ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def b58(raw: bytes) -> str:
    number = int.from_bytes(raw, "big")
    encoded = ""
    while number:
        number, remainder = divmod(number, 58)
        encoded = ALPHABET[remainder] + encoded
    return "1" * (len(raw) - len(raw.lstrip(b"\0"))) + (encoded or "")


def key(index: int) -> tuple[str, bytes]:
    raw = bytes([index]) * 32
    return b58(raw), raw


def token_data(mint: bytes, authority: bytes, amount: int) -> bytes:
    data = bytearray(165)
    data[0:32] = mint
    data[32:64] = authority
    struct.pack_into("<Q", data, 64, amount)
    data[108] = 1
    return bytes(data)


def position_data(owner: bytes, revision: int, balances: list[int]) -> bytes:
    data = bytearray(128 + 8 * len(balances))
    data[0:8] = b"DCLLBP02"
    struct.pack_into("<H", data, 8, 2)
    struct.pack_into("<I", data, 12, len(balances))
    struct.pack_into("<Q", data, 16, revision)
    data[24:56] = bytes([40]) * 32
    data[56:88] = owner
    data[88:120] = bytes([41]) * 32
    for index, amount in enumerate(balances):
        struct.pack_into("<Q", data, 128 + 8 * index, amount)
    return bytes(data)


def certificate_data(market: bytes) -> bytes:
    data = bytearray(312)
    data[0:8] = b"DCSRCER2"
    struct.pack_into("<H", data, 8, 2)
    data[10] = 1
    data[16:48] = market
    data[48:80] = bytes([51]) * 32
    data[80:112] = bytes([52]) * 32
    data[112:144] = bytes([53]) * 32
    data[144:176] = bytes([54]) * 32
    data[208:240] = bytes([55]) * 32
    struct.pack_into("<Q", data, 240, 1)
    struct.pack_into("<I", data, 256, 0)
    struct.pack_into("<Q", data, 296, 1)
    struct.pack_into("<Q", data, 304, 100)
    return bytes(data)


def rpc_account(owner: str, lamports: int, data: bytes):
    return {
        "lamports": lamports,
        "owner": owner,
        "data": [base64.b64encode(data).decode(), "base64"],
        "executable": False,
        "rentEpoch": 0,
        "space": len(data),
    }


def token_balance(index: int, mint: str, authority: str, amount: int):
    return {
        "accountIndex": index,
        "mint": mint,
        "owner": authority,
        "programId": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
        "uiTokenAmount": {"amount": str(amount), "decimals": 0, "uiAmount": None, "uiAmountString": str(amount)},
    }


def transaction(signature: str, slot: int, addresses: list[str], lamport_deltas: dict[str, int], token_changes: dict[str, tuple[str, str, int, int]]):
    pre_balances = [100_000 for _ in addresses]
    post_balances = list(pre_balances)
    for address, delta in lamport_deltas.items():
        post_balances[addresses.index(address)] += delta
    pre_tokens = []
    post_tokens = []
    for address, (mint, authority, before, after) in token_changes.items():
        index = addresses.index(address)
        pre_tokens.append(token_balance(index, mint, authority, before))
        post_tokens.append(token_balance(index, mint, authority, after))
    return {
        "slot": slot,
        "blockTime": 1_800_000_000 + slot,
        "transaction": {"signatures": [signature], "message": {"accountKeys": addresses, "instructions": [], "recentBlockhash": "blockhash"}},
        "meta": {
            "err": None,
            "fee": 5_000,
            "preBalances": pre_balances,
            "postBalances": post_balances,
            "preTokenBalances": pre_tokens,
            "postTokenBalances": post_tokens,
            "loadedAddresses": {"writable": [], "readonly": []},
            "computeUnitsConsumed": 100_000,
        },
    }


def fixture():
    names = [
        "payer", "refund", "source_token", "participant_token", "seller_token", "buyer_token",
        "fee_token", "hoard_token", "recipient_token", "position", "certificate", "market",
        "closed_protocol", "token_authority", "token_program", "protocol_owner",
    ]
    identities = {name: key(index + 1) for index, name in enumerate(names)}
    mint_address, mint_raw = key(90)
    account_kinds = {
        "payer": "wallet", "refund": "wallet", "source_token": "token", "participant_token": "token",
        "seller_token": "token", "buyer_token": "token", "fee_token": "token", "hoard_token": "token",
        "recipient_token": "token", "position": "position", "certificate": "certificate",
        "market": "protocol", "closed_protocol": "protocol", "token_authority": "wallet",
        "token_program": "protocol", "protocol_owner": "protocol",
    }
    accounts = [
        {"ref": name, "address": identities[name][0], "kind": account_kinds[name], "role": name.replace("_", "-")}
        for name in names
    ]
    source_bytes = b'{"schema":"fixture-semantic-owner-journal-v1"}\n'
    zero_digest = hashlib.sha256(source_bytes).hexdigest()
    pos_pre = position_data(identities["token_authority"][1], 1, [100, 0])
    pos_post = position_data(identities["token_authority"][1], 2, [50, 0])
    cert = certificate_data(identities["market"][1])

    def event(index, kind, lamports, tokens, **extra):
        return {
            "id": f"event-{index}-{kind}",
            "kind": kind,
            "operation": f"fixture-{kind}",
            "predecessor": None if index == 1 else f"event-{index - 1}-{reconcile.EVENT_KINDS[index - 2]}",
            "signature": f"signature-{kind}",
            "slot": str(100 + index),
            "feePayer": "payer",
            "feeLamports": "5000",
            "lamportDeltas": [{"account": ref, "lamports": str(amount)} for ref, amount in lamports.items()],
            "tokenDeltas": [{"account": ref, "atoms": str(amount)} for ref, amount in tokens.items()],
            "sourcePath": "fixture-journal.json",
            "sourceSha256": zero_digest,
            **extra,
        }

    events = [
        event(1, "founding", {"payer": -5000}, {}),
        event(2, "participant", {"payer": -5000}, {"source_token": -100, "participant_token": 100}),
        event(3, "direct", {"payer": -5000}, {"seller_token": 1990, "buyer_token": -2010, "fee_token": 20}, direct={
            "fillAtoms": "2000", "executionPrice": "100", "priceScale": "100",
            "feeBasisPointsPerSide": "50", "sellerToken": "seller_token", "buyerToken": "buyer_token",
            "feeRecipientToken": "fee_token", "mint": mint_address,
        }),
        event(4, "resolution", {"payer": -5000}, {}, certificate={
            "account": "certificate", "owner": identities["protocol_owner"][0],
            "dataBase64": base64.b64encode(cert).decode(), "market": identities["market"][0],
        }),
        event(5, "payout", {"payer": -5000}, {"hoard_token": -50, "recipient_token": 50},
              position={"account": "position", "preDataBase64": base64.b64encode(pos_pre).decode(), "postDataBase64": base64.b64encode(pos_post).decode()},
              payout={"hoardToken": "hoard_token", "recipientToken": "recipient_token", "position": "position", "principalAtoms": "50", "claimsBurnedAtoms": ["50", "0"], "mint": mint_address}),
        event(6, "retirement", {"payer": -5000, "refund": 10000}, {}, retirement={
            "stage": "aggregate-retirement",
            "closedAccounts": ["closed_protocol"],
            "refundLamports": [{"account": "refund", "lamports": "10000"}],
        }),
    ]
    final_amounts = {
        "source_token": 900, "participant_token": 100, "seller_token": 1990,
        "buyer_token": 2990, "fee_token": 20, "hoard_token": 950, "recipient_token": 50,
    }
    final_accounts = []
    capture_accounts = {}
    token_program = identities["token_program"][0]
    authority = identities["token_authority"][0]
    for ref, amount in final_amounts.items():
        data = token_data(mint_raw, identities["token_authority"][1], amount)
        final_accounts.append({"account": ref, "closed": False, "owner": token_program, "lamports": "2039280", "dataSha256": hashlib.sha256(data).hexdigest(), "mint": mint_address, "authority": authority, "amountAtoms": str(amount)})
        capture_accounts[identities[ref][0]] = {"contextSlot": "200", "value": rpc_account(token_program, 2_039_280, data)}
    for ref, data in (("position", pos_post), ("certificate", cert)):
        final_accounts.append({"account": ref, "closed": False, "owner": identities["protocol_owner"][0], "lamports": "3000000", "dataSha256": hashlib.sha256(data).hexdigest()})
        capture_accounts[identities[ref][0]] = {"contextSlot": "200", "value": rpc_account(identities["protocol_owner"][0], 3_000_000, data)}
    final_accounts.append({"account": "closed_protocol", "closed": True})
    capture_accounts[identities["closed_protocol"][0]] = {"contextSlot": "200", "value": None}
    manifest = {
        "schema": reconcile.MANIFEST_SCHEMA,
        "activityId": "fixture-complete-lifecycle",
        "cluster": {"kind": "devnet", "genesisHash": reconcile.DEVNET_GENESIS_HASH},
        "accounts": accounts,
        "events": events,
        "finalAccounts": final_accounts,
    }
    source_set = [{"event": event["id"], "sha256": event["sourceSha256"]} for event in events]
    manifest["sourceSetSha256"] = hashlib.sha256(reconcile.canonical_bytes(source_set)).hexdigest()
    transactions = {}
    participant_changes = {
        identities["source_token"][0]: (mint_address, authority, 1000, 900),
        identities["participant_token"][0]: (mint_address, authority, 0, 100),
    }
    direct_changes = {
        identities["seller_token"][0]: (mint_address, authority, 0, 1990),
        identities["buyer_token"][0]: (mint_address, authority, 5000, 2990),
        identities["fee_token"][0]: (mint_address, authority, 0, 20),
    }
    payout_changes = {
        identities["hoard_token"][0]: (mint_address, authority, 1000, 950),
        identities["recipient_token"][0]: (mint_address, authority, 0, 50),
    }
    for ev in events:
        lamports = {identities[item["account"]][0]: int(item["lamports"]) for item in ev["lamportDeltas"]}
        changes = participant_changes if ev["kind"] == "participant" else direct_changes if ev["kind"] == "direct" else payout_changes if ev["kind"] == "payout" else {}
        addresses = list(lamports) + list(changes)
        transactions[ev["signature"]] = transaction(ev["signature"], int(ev["slot"]), addresses, lamports, changes)
    capture = {"schema": reconcile.CAPTURE_SCHEMA, "genesisHash": reconcile.DEVNET_GENESIS_HASH, "transactions": transactions, "accounts": capture_accounts}
    return manifest, capture


class ReconcileTest(unittest.TestCase):
    def test_complete_captured_activity_emits_deterministic_unsigned_dossier(self):
        manifest, capture = fixture()
        dossier = reconcile.reconcile(manifest, reconcile.CapturedRpc(capture))
        self.assertEqual(dossier["schema"], reconcile.DOSSIER_SCHEMA)
        self.assertEqual(dossier["signatureScheme"], "none")
        self.assertEqual(dossier["totals"]["transactionFeesLamports"], "30000")
        self.assertEqual(dossier["totals"]["protocolFeesAtoms"], "20")
        self.assertEqual(dossier["totals"]["hoardPrincipalPaidAtoms"], "50")
        self.assertEqual(dossier, reconcile.reconcile(manifest, reconcile.CapturedRpc(capture)))

    def assert_refuses(self, mutate, message):
        manifest, capture = fixture()
        mutate(manifest, capture)
        with self.assertRaisesRegex(reconcile.Refusal, message):
            reconcile.reconcile(manifest, reconcile.CapturedRpc(capture))

    def test_missing_transaction_refuses(self):
        self.assert_refuses(lambda manifest, capture: capture["transactions"].pop("signature-direct"), "missing")

    def test_duplicate_signature_refuses(self):
        self.assert_refuses(lambda manifest, capture: manifest["events"][2].update(signature="signature-participant"), "duplicate")

    def test_forked_predecessor_refuses(self):
        self.assert_refuses(lambda manifest, capture: manifest["events"][4].update(predecessor="event-2-participant"), "forks")

    def test_mixed_direct_mint_refuses(self):
        def mutate(manifest, capture):
            other, _ = key(91)
            tx = capture["transactions"]["signature-direct"]
            tx["meta"]["postTokenBalances"][1]["mint"] = other
        self.assert_refuses(mutate, "mint")

    def test_mixed_participant_mint_refuses(self):
        def mutate(manifest, capture):
            other, _ = key(93)
            tx = capture["transactions"]["signature-participant"]
            tx["meta"]["preTokenBalances"][0]["mint"] = other
            tx["meta"]["postTokenBalances"][0]["mint"] = other
        self.assert_refuses(mutate, "non-Realm collateral mint")

    def test_side_fee_substitution_refuses(self):
        def mutate(manifest, capture):
            manifest["events"][2]["tokenDeltas"][0]["atoms"] = "1989"
            tx = capture["transactions"]["signature-direct"]
            tx["meta"]["postTokenBalances"][0]["uiTokenAmount"]["amount"] = "1989"
        self.assert_refuses(mutate, "gross")

    def test_missing_changed_wallet_refuses(self):
        def mutate(manifest, capture):
            tx = capture["transactions"]["signature-founding"]
            extra = manifest["accounts"][1]["address"]
            tx["transaction"]["message"]["accountKeys"].append(extra)
            tx["meta"]["preBalances"].append(0)
            tx["meta"]["postBalances"].append(1)
        self.assert_refuses(mutate, "deltas differ")

    def test_mixed_payout_mint_refuses(self):
        def mutate(manifest, capture):
            other, _ = key(92)
            tx = capture["transactions"]["signature-payout"]
            tx["meta"]["preTokenBalances"][1]["mint"] = other
            tx["meta"]["postTokenBalances"][1]["mint"] = other
        self.assert_refuses(mutate, "non-Realm collateral mint")

    def test_missing_retired_vacancy_refuses(self):
        def mutate(manifest, capture):
            address = next(item["address"] for item in manifest["accounts"] if item["ref"] == "closed_protocol")
            owner = next(item["address"] for item in manifest["accounts"] if item["ref"] == "protocol_owner")
            capture["accounts"][address]["value"] = rpc_account(owner, 1, b"")
        self.assert_refuses(mutate, "not vacant")

    def test_duplicate_json_key_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "duplicate.json"
            path.write_text('{"schema":"a","schema":"b"}', encoding="utf-8")
            with self.assertRaisesRegex(reconcile.Refusal, "duplicate JSON key"):
                reconcile.load_json(path)

    def test_wrong_genesis_refuses_before_activity_claim(self):
        self.assert_refuses(lambda manifest, capture: capture.update(genesisHash="not-devnet"), "not exact Solana devnet")

    def test_cli_writes_same_dossier(self):
        manifest, capture = fixture()
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest_path = root / "manifest.json"
            capture_path = root / "capture.json"
            out_path = root / "dossier.json"
            journal_root = root / "evidence"
            journal_root.mkdir()
            (journal_root / "fixture-journal.json").write_bytes(b'{"schema":"fixture-semantic-owner-journal-v1"}\n')
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            capture_path.write_text(json.dumps(capture), encoding="utf-8")
            status = reconcile.main(["captured", "--manifest", str(manifest_path), "--journal-root", str(journal_root), "--rpc-capture", str(capture_path), "--out", str(out_path)])
            self.assertEqual(status, 0)
            self.assertEqual(json.loads(out_path.read_text()), reconcile.reconcile(manifest, reconcile.CapturedRpc(capture)))

    def test_source_journal_digest_substitution_refuses_before_rpc(self):
        manifest, _ = fixture()
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "fixture-journal.json").write_text('{"schema":"substituted"}\n', encoding="utf-8")
            with self.assertRaisesRegex(reconcile.Refusal, "digest differs"):
                reconcile.authenticate_sources(manifest, root)


if __name__ == "__main__":
    unittest.main()
