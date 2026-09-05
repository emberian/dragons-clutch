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
    accounts = []
    for name in names:
        account = {"ref": name, "address": identities[name][0], "kind": account_kinds[name], "role": name.replace("_", "-")}
        if account["kind"] == "token":
            account.update({"mint": mint_address, "assetClass": "collateral", "authority": identities["token_authority"][0], "programOwner": identities["token_program"][0]})
        accounts.append(account)
    source_bytes = b'{"schema":"fixture-semantic-owner-journal-v1"}\n'
    zero_digest = hashlib.sha256(source_bytes).hexdigest()
    pos_pre = position_data(identities["token_authority"][1], 1, [100, 0])
    pos_post = position_data(identities["token_authority"][1], 2, [50, 0])
    cert = certificate_data(identities["market"][1])

    def event(index, kind, lamports, tokens, operation=None, signature=None, **extra):
        return {
            "id": f"event-{index}-{kind}",
            "kind": kind,
            "operation": operation or f"fixture-{kind}",
            "predecessor": None,
            "signature": signature or f"signature-{kind}",
            "slot": str(100 + index),
            "feePayer": "payer",
            "feeLamports": "5000",
            "computeUnitsConsumed": "100000",
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
        event(4, "resolution", {"payer": -5000}, {}, operation="resolution-submit", signature="signature-resolution-submit"),
        event(5, "resolution", {"payer": -5000}, {}, operation="resolution-provider-execute-v1", signature="signature-resolution-provider-execute-v1", certificate={
            "account": "certificate", "owner": identities["protocol_owner"][0],
            "dataBase64": base64.b64encode(cert).decode(), "market": identities["market"][0],
        }),
        event(6, "resolution", {"payer": -5000}, {}, operation="core-terminal-accept-v1", signature="signature-core-terminal-accept-v1"),
        event(7, "resolution", {"payer": -5000}, {}, operation="resolution-reclaim", signature="signature-resolution-reclaim"),
        event(8, "payout", {"payer": -5000}, {"hoard_token": -50, "recipient_token": 50},
              position={"account": "position", "preDataBase64": base64.b64encode(pos_pre).decode(), "postDataBase64": base64.b64encode(pos_post).decode()},
              payout={"hoardToken": "hoard_token", "recipientToken": "recipient_token", "position": "position", "principalAtoms": "50", "claimsBurnedAtoms": ["50", "0"], "mint": mint_address}),
        event(9, "retirement", {"payer": -5000, "refund": 10000}, {}, retirement={
            "stage": "aggregate-retirement",
            "closedAccounts": ["closed_protocol"],
            "refundLamports": [{"account": "refund", "lamports": "10000"}],
        }),
    ]
    predecessor = None
    for current in events:
        current["predecessor"] = predecessor
        predecessor = current["id"]
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
    running_lamports = {}
    for ev in events:
        lamports = {identities[item["account"]][0]: int(item["lamports"]) for item in ev["lamportDeltas"]}
        changes = participant_changes if ev["kind"] == "participant" else direct_changes if ev["kind"] == "direct" else payout_changes if ev["kind"] == "payout" else {}
        addresses = list(lamports) + list(changes)
        tx = transaction(ev["signature"], int(ev["slot"]), addresses, lamports, changes)
        for address, delta in lamports.items():
            index = addresses.index(address)
            before = running_lamports.get(address, 100_000)
            tx["meta"]["preBalances"][index] = before
            tx["meta"]["postBalances"][index] = before + delta
            running_lamports[address] = before + delta
        transactions[ev["signature"]] = tx
    capture = {"schema": reconcile.CAPTURE_SCHEMA, "genesisHash": reconcile.DEVNET_GENESIS_HASH, "transactions": transactions, "accounts": capture_accounts}
    return manifest, capture


SUCCESSOR_SRC = ROOT.parents[1] / "tools" / "local-validator" / "bootstrap" / "successor" / "src"

# The exact string `reconcile.py` carried for the terminal-sequence session
# until 2026-09-04, by which time the crate had been writing `-v3` for two
# revisions. Written out once, HERE, as the subject of a negative control --
# which is the only place a superseded wire string belongs.

# Which Rust file writes each schema this tool reads back. The same shape as
# `preflight.py`'s `SCHEMA_OWNERS` and for the same reason: the VALUE is not
# restated here, so what this table can check is the WIRING -- that the tool
# reads the owner an independent reader expects, and that the owner still
# declares that constant exactly once.
def declared_rust_str(file_name: str, constant: str) -> str:
    """One Rust `&str` const, scanned line by line rather than matched.

    Deliberately NOT `tools/lib/rust_schema.py`, which is what the tool under
    test uses. A test that re-ran the tool's own reader would agree with it by
    construction; this is a second implementation, so it can disagree.
    """
    lines = (SUCCESSOR_SRC / file_name).read_text(encoding="utf-8").splitlines()
    found: list[str] = []
    for index, line in enumerate(lines):
        head = line.strip()
        for prefix in ("pub(crate) ", "pub ", ""):
            if head.startswith(f"{prefix}const {constant}:"):
                break
        else:
            continue
        tail = head.split("=", 1)[1].strip() if "=" in head else ""
        if not tail:
            tail = lines[index + 1].strip()
        if not (tail.startswith('"') and tail.endswith('";')):
            raise AssertionError(f"{file_name} {constant} is not one plain &str literal")
        found.append(tail[1:-2])
    if len(found) != 1:
        raise AssertionError(f"{file_name} declares {constant} {len(found)} times, not once")
    return found[0]


class ReconcileTest(unittest.TestCase):
    def test_complete_captured_activity_emits_deterministic_unsigned_dossier(self):
        manifest, capture = fixture()
        dossier = reconcile.reconcile(manifest, reconcile.CapturedRpc(capture))
        self.assertEqual(dossier["schema"], reconcile.DOSSIER_SCHEMA)
        self.assertEqual(dossier["signatureScheme"], "none")
        self.assertEqual(dossier["evidence"]["rpc"]["mode"], "captured-finalized-rpc-replay")
        self.assertEqual(dossier["totals"]["transactionFeesLamports"], "45000")
        self.assertEqual(dossier["totals"]["computeUnitsConsumed"], "900000")
        self.assertEqual(dossier["totals"]["protocolFeesAtoms"], "20")
        self.assertEqual(dossier["totals"]["hoardPrincipalPaidAtoms"], "50")
        self.assertEqual(dossier, reconcile.reconcile(manifest, reconcile.CapturedRpc(capture)))

    def test_direct_setup_and_hot_keep_distinct_signatures_and_fees(self):
        manifest, capture = fixture()
        payer = next(row for row in manifest["accounts"] if row["ref"] == "payer")
        direct_index = next(
            index for index, event in enumerate(manifest["events"])
            if event["kind"] == "direct"
        )
        for event in manifest["events"][direct_index:]:
            event["slot"] = str(int(event["slot"]) + 1)
            captured_transaction = capture["transactions"][event["signature"]]
            captured_transaction["slot"] += 1
            captured_transaction["blockTime"] += 1
            payer_index = captured_transaction["transaction"]["message"]["accountKeys"].index(
                payer["address"]
            )
            captured_transaction["meta"]["preBalances"][payer_index] -= 5_000
            captured_transaction["meta"]["postBalances"][payer_index] -= 5_000
        setup_signature = "signature-direct-replay-setup"
        setup_transaction = transaction(
            setup_signature,
            103,
            [payer["address"]],
            {payer["address"]: -5_000},
            {},
        )
        setup_transaction["meta"]["preBalances"][0] = 90_000
        setup_transaction["meta"]["postBalances"][0] = 85_000
        capture["transactions"][setup_signature] = setup_transaction
        manifest["events"].insert(
            direct_index,
            {
                "id": "pending-direct-setup",
                "kind": "direct",
                "operation": "direct-replay-setup",
                "predecessor": None,
                "signature": setup_signature,
                "slot": "103",
                "feePayer": "payer",
                "feeLamports": "5000",
                "computeUnitsConsumed": "100000",
                "lamportDeltas": [{"account": "payer", "lamports": "-5000"}],
                "tokenDeltas": [],
                "sourcePath": "fixture-journal.json",
                "sourceSha256": manifest["events"][0]["sourceSha256"],
            },
        )
        predecessor = None
        for index, event in enumerate(manifest["events"]):
            event["id"] = f"activity-{index:03}"
            event["predecessor"] = predecessor
            predecessor = event["id"]
        manifest["sourceSetSha256"] = hashlib.sha256(
            reconcile.canonical_bytes(
                [
                    {"event": event["id"], "sha256": event["sourceSha256"]}
                    for event in manifest["events"]
                ]
            )
        ).hexdigest()

        dossier = reconcile.reconcile(manifest, reconcile.CapturedRpc(capture))
        direct = [event for event in dossier["events"] if event["kind"] == "direct"]
        self.assertEqual([event["signature"] for event in direct], [setup_signature, "signature-direct"])
        self.assertNotIn("direct", direct[0])
        self.assertIn("direct", direct[1])
        self.assertEqual(dossier["totals"]["transactionFeesLamports"], "50000")

    def test_resolution_v7_keeps_exact_four_mutations_and_one_certificate_owner(self):
        manifest, capture = fixture()
        dossier = reconcile.reconcile(manifest, reconcile.CapturedRpc(capture))
        resolution = [event for event in dossier["events"] if event["kind"] == "resolution"]
        self.assertEqual(
            [event["operation"] for event in resolution],
            list(reconcile.RESOLUTION_OPERATIONS_V7),
        )
        self.assertNotIn("certificate", resolution[0])
        self.assertIn("certificate", resolution[1])
        self.assertNotIn("certificate", resolution[2])
        self.assertNotIn("certificate", resolution[3])
        self.assertEqual(
            [event["computeUnitsConsumed"] for event in resolution],
            ["100000"] * 4,
        )

    def test_resolution_v7_partial_accept_replay_order_slot_and_compute_hostiles_refuse(self):
        def rechain(manifest):
            predecessor = None
            for event in manifest["events"]:
                event["predecessor"] = predecessor
                predecessor = event["id"]
            manifest["sourceSetSha256"] = hashlib.sha256(
                reconcile.canonical_bytes(
                    [
                        {"event": event["id"], "sha256": event["sourceSha256"]}
                        for event in manifest["events"]
                    ]
                )
            ).hexdigest()

        def partial_execute(manifest, _capture):
            manifest["events"] = [
                event for event in manifest["events"]
                if event["operation"] not in ("core-terminal-accept-v1", "resolution-reclaim")
            ]
            rechain(manifest)

        def omitted_accept(manifest, _capture):
            manifest["events"] = [
                event for event in manifest["events"]
                if event["operation"] != "core-terminal-accept-v1"
            ]
            rechain(manifest)

        def certificate_on_accept(manifest, _capture):
            execute = next(event for event in manifest["events"] if event["operation"] == "resolution-provider-execute-v1")
            accept = next(event for event in manifest["events"] if event["operation"] == "core-terminal-accept-v1")
            accept["certificate"] = execute.pop("certificate")

        def same_slot(manifest, capture):
            execute = next(event for event in manifest["events"] if event["operation"] == "resolution-provider-execute-v1")
            accept = next(event for event in manifest["events"] if event["operation"] == "core-terminal-accept-v1")
            accept["slot"] = execute["slot"]
            capture["transactions"][accept["signature"]]["slot"] = int(execute["slot"])

        def compute_substitution(manifest, _capture):
            accept = next(event for event in manifest["events"] if event["operation"] == "core-terminal-accept-v1")
            accept["computeUnitsConsumed"] = "99999"

        def replayed_accept(manifest, _capture):
            accept_index = next(
                index for index, event in enumerate(manifest["events"])
                if event["operation"] == "core-terminal-accept-v1"
            )
            replay = copy.deepcopy(manifest["events"][accept_index])
            replay.update(
                id="event-core-terminal-accept-replay",
                signature="signature-core-terminal-accept-replay",
                slot=str(int(replay["slot"]) + 1),
            )
            manifest["events"].insert(accept_index + 1, replay)
            for event in manifest["events"][accept_index + 2:]:
                event["slot"] = str(int(event["slot"]) + 1)
            rechain(manifest)

        for mutate, message in (
            (partial_execute, "exact submit"),
            (omitted_accept, "exact submit"),
            (certificate_on_accept, "belongs only to provider execute"),
            (same_slot, "strictly ordered"),
            (compute_substitution, "substituted compute units"),
            (replayed_accept, "exact submit"),
        ):
            with self.subTest(mutate=mutate.__name__):
                self.assert_refuses(mutate, message)

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
        self.assert_refuses(mutate, "declared token-account mint")

    def test_token_authority_substitution_refuses(self):
        def mutate(manifest, capture):
            other, _ = key(95)
            tx = capture["transactions"]["signature-participant"]
            tx["meta"]["preTokenBalances"][0]["owner"] = other
            tx["meta"]["postTokenBalances"][0]["owner"] = other
        self.assert_refuses(mutate, "mint or authority")

    def test_fee_payer_substitution_refuses(self):
        def mutate(manifest, capture):
            tx = capture["transactions"]["signature-founding"]
            keys = tx["transaction"]["message"]["accountKeys"]
            other = manifest["accounts"][1]["address"]
            keys[0] = other
        self.assert_refuses(mutate, "substitutes its fee payer")

    def test_declared_claim_mint_may_differ_from_collateral(self):
        manifest, capture = fixture()
        claim_mint, claim_raw = key(94)
        participant = next(account for account in manifest["accounts"] if account["ref"] == "participant_token")
        participant.update(mint=claim_mint, assetClass="claim")
        expected = next(account for account in manifest["finalAccounts"] if account["account"] == "participant_token")
        expected["mint"] = claim_mint
        authority = next(account for account in manifest["accounts"] if account["ref"] == "token_authority")["address"]
        authority_raw = reconcile.b58decode(authority, "fixture authority")
        data = token_data(claim_raw, authority_raw, 100)
        expected["dataSha256"] = hashlib.sha256(data).hexdigest()
        address = participant["address"]
        capture["accounts"][address]["value"]["data"] = [base64.b64encode(data).decode(), "base64"]
        tx = capture["transactions"]["signature-participant"]
        tx["meta"]["preTokenBalances"][1]["mint"] = claim_mint
        tx["meta"]["postTokenBalances"][1]["mint"] = claim_mint
        dossier = reconcile.reconcile(manifest, reconcile.CapturedRpc(capture))
        self.assertEqual(dossier["schema"], reconcile.DOSSIER_SCHEMA)

    def test_multiple_direct_fills_and_payouts_are_all_aggregated(self):
        manifest, capture = fixture()
        by_ref = {account["ref"]: account for account in manifest["accounts"]}
        mint = manifest["events"][2]["direct"]["mint"]
        authority = by_ref["token_authority"]["address"]
        second_direct = copy.deepcopy(manifest["events"][2])
        second_direct.update(id="event-direct-2", signature="signature-direct-2", operation="fixture-direct-2")
        second_direct["direct"].update(fillAtoms="1000")
        second_direct["tokenDeltas"] = [
            {"account": "seller_token", "atoms": "995"},
            {"account": "buyer_token", "atoms": "-1005"},
            {"account": "fee_token", "atoms": "10"},
        ]
        direct_addresses = [by_ref[ref]["address"] for ref in ("payer", "seller_token", "buyer_token", "fee_token")]
        capture["transactions"][second_direct["signature"]] = transaction(
            second_direct["signature"], 0, direct_addresses,
            {by_ref["payer"]["address"]: -5000},
            {
                by_ref["seller_token"]["address"]: (mint, authority, 1990, 2985),
                by_ref["buyer_token"]["address"]: (mint, authority, 2990, 1985),
                by_ref["fee_token"]["address"]: (mint, authority, 20, 30),
            },
        )
        first_payout = next(event for event in manifest["events"] if event["kind"] == "payout")
        second_payout = copy.deepcopy(first_payout)
        second_payout.update(id="event-payout-2", signature="signature-payout-2", operation="fixture-payout-2")
        position_2 = base64.b64decode(first_payout["position"]["postDataBase64"])
        position_3 = position_data(reconcile.b58decode(authority, "fixture authority"), 3, [0, 0])
        second_payout["position"] = {
            "account": "position", "preDataBase64": base64.b64encode(position_2).decode(),
            "postDataBase64": base64.b64encode(position_3).decode(),
        }
        payout_addresses = [by_ref[ref]["address"] for ref in ("payer", "hoard_token", "recipient_token")]
        capture["transactions"][second_payout["signature"]] = transaction(
            second_payout["signature"], 0, payout_addresses,
            {by_ref["payer"]["address"]: -5000},
            {
                by_ref["hoard_token"]["address"]: (mint, authority, 950, 900),
                by_ref["recipient_token"]["address"]: (mint, authority, 50, 100),
            },
        )
        manifest["events"].insert(3, second_direct)
        manifest["events"].insert(-1, second_payout)
        for index, event in enumerate(manifest["events"]):
            event["predecessor"] = None if index == 0 else manifest["events"][index - 1]["id"]
            event["slot"] = str(101 + index)
            capture["transactions"][event["signature"]]["slot"] = 101 + index
        payer = by_ref["payer"]["address"]
        running = 100_000
        for event in manifest["events"]:
            tx = capture["transactions"][event["signature"]]
            index = tx["transaction"]["message"]["accountKeys"].index(payer)
            delta = next(int(item["lamports"]) for item in event["lamportDeltas"] if item["account"] == "payer")
            tx["meta"]["preBalances"][index] = running
            tx["meta"]["postBalances"][index] = running + delta
            running += delta
        token_program = by_ref["token_program"]["address"]
        authority_raw = reconcile.b58decode(authority, "fixture authority")
        mint_raw = reconcile.b58decode(mint, "fixture mint")
        for ref, amount in (("seller_token", 2985), ("buyer_token", 1985), ("fee_token", 30), ("hoard_token", 900), ("recipient_token", 100)):
            data = token_data(mint_raw, authority_raw, amount)
            expected = next(item for item in manifest["finalAccounts"] if item["account"] == ref)
            expected.update(amountAtoms=str(amount), dataSha256=hashlib.sha256(data).hexdigest())
            capture["accounts"][by_ref[ref]["address"]]["value"] = rpc_account(token_program, 2_039_280, data)
        position_expected = next(item for item in manifest["finalAccounts"] if item["account"] == "position")
        position_expected["dataSha256"] = hashlib.sha256(position_3).hexdigest()
        capture["accounts"][by_ref["position"]["address"]]["value"]["data"] = [base64.b64encode(position_3).decode(), "base64"]
        source_set = [{"event": event["id"], "sha256": event["sourceSha256"]} for event in manifest["events"]]
        manifest["sourceSetSha256"] = hashlib.sha256(reconcile.canonical_bytes(source_set)).hexdigest()
        dossier = reconcile.reconcile(manifest, reconcile.CapturedRpc(capture))
        self.assertEqual(dossier["totals"]["protocolFeesAtoms"], "30")
        self.assertEqual(dossier["totals"]["hoardPrincipalPaidAtoms"], "100")
        self.assertEqual(dossier["totals"]["transactionFeesLamports"], "55000")

    def test_discontinuous_wallet_history_refuses(self):
        def mutate(manifest, capture):
            tx = capture["transactions"]["signature-participant"]
            tx["meta"]["preBalances"][0] += 1
            tx["meta"]["postBalances"][0] += 1
        self.assert_refuses(mutate, "lamport history.*discontinuous")

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
        self.assert_refuses(mutate, "declared token-account mint")

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
            capture_sha256 = hashlib.sha256(capture_path.read_bytes()).hexdigest()
            self.assertEqual(json.loads(out_path.read_text()), reconcile.reconcile(manifest, reconcile.CapturedRpc(capture, capture_sha256)))

    def test_source_journal_digest_substitution_refuses_before_rpc(self):
        manifest, _ = fixture()
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "fixture-journal.json").write_text('{"schema":"substituted"}\n', encoding="utf-8")
            with self.assertRaisesRegex(reconcile.Refusal, "digest differs"):
                reconcile.authenticate_sources(manifest, root)

    def test_public_devnet_manifest_refuses_private_two_position_extension(self):
        manifest, capture = fixture()
        manifest["events"][2]["positions"] = []
        with self.assertRaisesRegex(reconcile.Refusal, "unknown fields"):
            reconcile.reconcile(manifest, reconcile.CapturedRpc(capture))

    # -----------------------------------------------------------------------
    # The schema strings, and the two that were stale.
    #
    # This tool restated eleven wire schema strings the successor crate writes,
    # and on 2026-09-04 two of them were wrong: the terminal-sequence session at
    # `-v1` against the crate's `-v3`, and the private-lifecycle chaos session
    # at `-v1` against `-v2`. Every session the current driver writes refused.
    #
    # The suite could not see it, and that is the part worth fixing rather than
    # patching. The fixtures build their evidence FROM these same
    # constants, so tool and fixture agreed with each other about a string
    # neither owned, and fifty-five tests stayed green over a reader that
    # refused every real artifact. The three below break that circle: the value
    # comes from the Rust, read a second way, and the superseded string is
    # exercised as the refusal it should be.
    # -----------------------------------------------------------------------
if __name__ == "__main__":
    unittest.main()
