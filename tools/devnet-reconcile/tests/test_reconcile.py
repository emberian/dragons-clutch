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


def owned_loopback_fixture(root: pathlib.Path):
    manifest, capture = fixture()
    genesis, _ = key(80)
    manifest["schema"] = reconcile.OWNED_LOOPBACK_MANIFEST_SCHEMA
    manifest["cluster"] = {"kind": "owned-loopback", "genesisHash": genesis}
    source = {"schema": "fixture-semantic-owner-journal-v1", "phase": "finalized"}
    source_raw = reconcile.canonical_bytes(source)
    source_sha = hashlib.sha256(source_raw).hexdigest()
    for event in manifest["events"]:
        event["sourceSha256"] = source_sha
    source_set = [{"event": event["id"], "sha256": source_sha} for event in manifest["events"]]
    manifest["sourceSetSha256"] = hashlib.sha256(reconcile.canonical_bytes(source_set)).hexdigest()
    capture.update(
        schema=reconcile.OWNED_LOOPBACK_CAPTURE_SCHEMA,
        genesisHash=genesis,
        commitment="finalized",
        finalizedSlot="200",
    )
    evidence = root / "evidence"
    evidence.mkdir()
    (evidence / "fixture-journal.json").write_bytes(source_raw)
    stage_directory = evidence / "stages"
    stage_directory.mkdir()
    session_stages = []
    for stage in reconcile.OWNED_LOOPBACK_COMPLETED_STAGES:
        relative = f"stages/{stage}.json"
        stage_source = {
            "schema": f"dclutch-fixture-{stage}-completion-v1",
            "status": "finalized",
        }
        stage_path = evidence / relative
        stage_path.write_bytes(reconcile.canonical_bytes(stage_source))
        session_stages.append(
            {
                "stage": stage,
                "path": relative,
                "sha256": hashlib.sha256(stage_path.read_bytes()).hexdigest(),
                "schema": stage_source["schema"],
                "completionPointer": "/status",
                "completionValue": "finalized",
            }
        )
    session = {
        "schema": reconcile.OWNED_LOOPBACK_PRIVATE_SESSION_SCHEMA,
        "status": "finalized",
        "cluster": "owned-loopback",
        "genesisHash": genesis,
        "stages": session_stages,
        "completedStages": list(reconcile.OWNED_LOOPBACK_COMPLETED_STAGES),
        "stageSetSha256": hashlib.sha256(
            reconcile.canonical_bytes(session_stages)
        ).hexdigest(),
    }
    (evidence / "session.json").write_bytes(reconcile.canonical_bytes(session))
    chaos = {
        "schema": reconcile.OWNED_LOOPBACK_CHAOS_SESSION_SCHEMA,
        "status": "finalized",
        "stages": [
            {
                "stage": stage,
                "status": "finalized",
                "intentSha256": hashlib.sha256(stage.encode()).hexdigest(),
            }
            for stage in reconcile.OWNED_LOOPBACK_CHAOS_STAGES
        ],
    }
    (evidence / "chaos-session.json").write_bytes(reconcile.canonical_bytes(chaos))
    terminal_session = {
        "schema": reconcile.OWNED_LOOPBACK_TERMINAL_SESSION_SCHEMA,
        "phase": "finalized",
        "sessionSha256": "aa" * 32,
    }
    (evidence / "terminal-session.json").write_bytes(
        reconcile.canonical_bytes(terminal_session)
    )
    manifest_path = root / "manifest.json"
    capture_path = evidence / "capture.json"
    receipt_path = root / "receipt.json"
    manifest_path.write_bytes(reconcile.canonical_bytes(manifest))

    def journal(path: str, schema: str, completion_pointer: str) -> dict[str, str]:
        raw = (evidence / path).read_bytes()
        return {
            "path": path,
            "sha256": hashlib.sha256(raw).hexdigest(),
            "schema": schema,
            "completionPointer": completion_pointer,
            "completionValue": "finalized",
        }

    terminal_directory = evidence / "terminal"
    terminal_directory.mkdir()
    terminal_rows = []
    terminal_kinds = [
        "core-begin-retiring",
        "direct-begin-retiring",
        "resolution-close-fund",
        "direct-close-capability",
        "retirement-replay-handoff",
        "aggregate-retirement",
    ]
    payer, _ = key(82)
    market, _ = key(83)
    lookup_table, _ = key(84)
    for index, kind in enumerate(terminal_kinds):
        relative = f"terminal/{index:02d}-{kind}.json"
        signature = b58(bytes([150 + index]) * 64)
        terminal_source = {
            "schema": reconcile.OWNED_LOOPBACK_TERMINAL_JOURNAL_SCHEMA,
            "phase": "finalized",
            "intent": {
                "mutation": {"kind": "protocol", "stage": kind},
                "payer": payer,
                "transactionFeeLamports": 5000,
                "protocolLamportDeltas": {},
            },
            "finalized": {
                "signature": signature,
                "slot": 150 + index,
                "feeLamports": 5000,
                "computeUnitsConsumed": 100,
                "packetSha256": "bb" * 32,
                "poststate": {},
            },
        }
        (evidence / relative).write_bytes(reconcile.canonical_bytes(terminal_source))
        terminal_rows.append(
            {
                "path": relative,
                "sha256": hashlib.sha256((evidence / relative).read_bytes()).hexdigest(),
                "schema": reconcile.OWNED_LOOPBACK_TERMINAL_JOURNAL_SCHEMA,
                "mutation": {"kind": kind},
                "phase": "finalized",
                "feePayer": payer,
                "signature": signature,
                "finalizedSlot": str(150 + index),
                "computeUnitsConsumed": "100",
                "transactionFeeLamports": "5000",
                "protocolLamportDeltas": [],
            }
        )
    terminal_completion_path = evidence / "terminal-completion.json"
    terminal_completion = {
        "schema": reconcile.OWNED_LOOPBACK_TERMINAL_COMPLETION_SCHEMA,
        "status": "finalized",
        "cluster": "owned-loopback",
        "genesisHash": genesis,
        "invocation": {
            "command": "local-private-validator-terminal-sequence-v1",
            "rpcUrl": "http://127.0.0.1:18899/",
            "planPath": str(evidence / "plan.json"),
            "marketInputPath": str(evidence / "market-input.json"),
            "evidencePath": str(evidence / "terminal-evidence.json"),
            "market": market,
            "feePayer": payer,
            "feePayerKeypairPath": str(evidence / "payer.json"),
            "sessionPath": str((evidence / "terminal-session.json").resolve()),
            "journalDirectory": str(terminal_directory.resolve()),
            "completionPath": str(terminal_completion_path.resolve()),
            "suppliedLookupTable": lookup_table,
            "execute": True,
        },
        "session": {
            "path": "terminal-session.json",
            "sha256": hashlib.sha256((evidence / "terminal-session.json").read_bytes()).hexdigest(),
            "schema": terminal_session["schema"],
            "sessionSha256": terminal_session["sessionSha256"],
        },
        "journalDirectory": "terminal",
        "market": market,
        "payer": payer,
        "lookupTable": lookup_table,
        "journals": terminal_rows,
        "finalizedSlot": "155",
        "transactionFeesLamports": "30000",
        "computeUnitsConsumed": "600",
    }
    terminal_completion_path.write_bytes(reconcile.canonical_bytes(terminal_completion))
    journals = [
        journal("chaos-session.json", chaos["schema"], "/status"),
        journal("fixture-journal.json", source["schema"], "/phase"),
        journal("session.json", session["schema"], "/status"),
        journal(
            "terminal-completion.json",
            reconcile.OWNED_LOOPBACK_TERMINAL_COMPLETION_SCHEMA,
            "/status",
        ),
    ]
    journals.extend(
        journal(
            f"stages/{stage}.json",
            f"dclutch-fixture-{stage}-completion-v1",
            "/status",
        )
        for stage in reconcile.OWNED_LOOPBACK_COMPLETED_STAGES
    )
    journals.sort(key=lambda row: row["path"])
    programs = []
    loader = reconcile.LOADER_V3_PROGRAM_ID
    authority, authority_raw = key(81)
    for index, role in enumerate(reconcile.OWNED_LOOPBACK_PROGRAM_ROLES):
        program_id, _ = key(100 + index)
        programdata, programdata_raw = key(120 + index)
        slot = 0 if role.startswith("pyth-") else index + 1
        elf = b"\x7fELF" + bytes([index + 1]) * 60
        program_bytes = struct.pack("<I", 2) + programdata_raw
        programdata_bytes = bytearray(45)
        struct.pack_into("<I", programdata_bytes, 0, 3)
        struct.pack_into("<Q", programdata_bytes, 4, slot)
        retained = not role.startswith("pyth-")
        programdata_bytes[12] = 1 if retained else 0
        if retained:
            programdata_bytes[13:45] = authority_raw
        programdata_bytes.extend(elf)
        program_account = rpc_account(loader, 1_140_000, program_bytes)
        program_account["executable"] = True
        capture["accounts"][program_id] = {"contextSlot": "200", "value": program_account}
        capture["accounts"][programdata] = {
            "contextSlot": "200",
            "value": rpc_account(loader, 2_000_000, bytes(programdata_bytes)),
        }
        programs.append(
            {
                "role": role,
                "programId": program_id,
                "programDataAddress": programdata,
                "deploymentSlot": str(slot),
                "elfSha256": hashlib.sha256(elf).hexdigest(),
                "genesisProgramDataSha256": hashlib.sha256(programdata_bytes).hexdigest(),
                "upgradeAuthority": authority if retained else None,
            }
        )
    capture_path.write_bytes(reconcile.canonical_bytes(capture))
    plan_path = evidence / "provider-plan.json"
    profile_path = evidence / "local-validator-profile.json"
    plan = {"schema": reconcile.OWNED_LOOPBACK_PROVIDER_PLAN_SCHEMA}
    profile = {"schema": reconcile.OWNED_LOOPBACK_PROVIDER_PROFILE_SCHEMA}
    plan_path.write_bytes(reconcile.canonical_bytes(plan))
    profile_path.write_bytes(reconcile.canonical_bytes(profile))
    provider_closure_path = evidence / "provider-closure.json"
    provider_closure = {
        "schema": reconcile.OWNED_LOOPBACK_PROVIDER_CLOSURE_SCHEMA,
        "cluster": "owned-loopback",
        "genesisHash": genesis,
        "status": "finalized",
        "finalizedObservationSlot": "200",
        "plan": {
            "path": str(plan_path.resolve()),
            "sha256": hashlib.sha256(plan_path.read_bytes()).hexdigest(),
            "schema": reconcile.OWNED_LOOPBACK_PROVIDER_PLAN_SCHEMA,
        },
        "localValidatorProfile": {
            "path": str(profile_path.resolve()),
            "sha256": hashlib.sha256(profile_path.read_bytes()).hexdigest(),
            "schema": reconcile.OWNED_LOOPBACK_PROVIDER_PROFILE_SCHEMA,
        },
        "finalizedCapture": {
            "path": str(capture_path.resolve()),
            "sha256": hashlib.sha256(capture_path.read_bytes()).hexdigest(),
            "schema": reconcile.OWNED_LOOPBACK_CAPTURE_SCHEMA,
            "finalizedSlot": "200",
        },
        "providerPrograms": programs[-2:],
    }
    provider_closure_path.write_bytes(reconcile.canonical_bytes(provider_closure))
    receipt = {
        "schema": reconcile.OWNED_LOOPBACK_RECEIPT_SCHEMA,
        "status": "finalized",
        "cluster": manifest["cluster"],
        "sourceCommit": "ab" * 20,
        "checkedReleaseGateSha256": "cd" * 32,
        "programs": programs,
        "manifestSha256": hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
        "capture": {
            "path": "capture.json",
            "sha256": hashlib.sha256(capture_path.read_bytes()).hexdigest(),
            "schema": reconcile.OWNED_LOOPBACK_CAPTURE_SCHEMA,
            "commitment": "finalized",
            "finalizedSlot": "200",
        },
        "providerClosure": {
            "path": "provider-closure.json",
            "sha256": hashlib.sha256(provider_closure_path.read_bytes()).hexdigest(),
            "schema": reconcile.OWNED_LOOPBACK_PROVIDER_CLOSURE_SCHEMA,
        },
        "journals": journals,
        "journalSetSha256": hashlib.sha256(reconcile.canonical_bytes(journals)).hexdigest(),
        "privateSession": {
            "path": "session.json",
            "sha256": hashlib.sha256((evidence / "session.json").read_bytes()).hexdigest(),
            "schema": session["schema"],
            "status": "finalized",
            "completedStages": list(reconcile.OWNED_LOOPBACK_COMPLETED_STAGES),
        },
        "chaosSession": {
            "path": "chaos-session.json",
            "sha256": hashlib.sha256(
                (evidence / "chaos-session.json").read_bytes()
            ).hexdigest(),
            "schema": reconcile.OWNED_LOOPBACK_CHAOS_SESSION_SCHEMA,
            "status": "finalized",
        },
    }
    receipt_path.write_bytes(reconcile.canonical_bytes(receipt))
    return manifest, capture, receipt, manifest_path, capture_path, receipt_path, evidence


def rewrite_owned_loopback_receipt(receipt: dict, receipt_path: pathlib.Path) -> None:
    receipt_path.write_bytes(reconcile.canonical_bytes(receipt))


class ReconcileTest(unittest.TestCase):
    def test_complete_captured_activity_emits_deterministic_unsigned_dossier(self):
        manifest, capture = fixture()
        dossier = reconcile.reconcile(manifest, reconcile.CapturedRpc(capture))
        self.assertEqual(dossier["schema"], reconcile.DOSSIER_SCHEMA)
        self.assertEqual(dossier["signatureScheme"], "none")
        self.assertEqual(dossier["evidence"]["rpc"]["mode"], "captured-finalized-rpc-replay")
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
        first_payout = manifest["events"][4]
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
        self.assertEqual(dossier["totals"]["transactionFeesLamports"], "40000")

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

    def test_owned_loopback_cli_emits_separately_typed_local_dossier(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, _, _, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            out_path = root / "dossier.json"
            status = reconcile.main(
                [
                    "owned-loopback-captured",
                    "--manifest", str(manifest_path),
                    "--rpc-capture", str(capture_path),
                    "--session-receipt", str(receipt_path),
                    "--expected-session-receipt-sha256", hashlib.sha256(receipt_path.read_bytes()).hexdigest(),
                    "--evidence-root", str(evidence),
                    "--out", str(out_path),
                ]
            )
            self.assertEqual(status, 0)
            dossier = json.loads(out_path.read_text())
            self.assertEqual(dossier["schema"], reconcile.OWNED_LOOPBACK_DOSSIER_SCHEMA)
            self.assertEqual(dossier["cluster"], manifest["cluster"])
            self.assertEqual(
                dossier["evidence"]["rpc"]["mode"],
                "owned-loopback-captured-finalized-rpc-replay",
            )
            self.assertEqual(
                dossier["evidence"]["ownedLoopbackSession"]["classification"],
                "owned-loopback-local-evidence-not-public-devnet-or-live-observation",
            )

    def test_owned_loopback_manifest_can_never_enter_public_devnet_reconcile(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest, capture, _, _, _, _, _ = owned_loopback_fixture(pathlib.Path(directory))
            with self.assertRaisesRegex(reconcile.Refusal, "not admitted"):
                reconcile.reconcile(manifest, reconcile.CapturedRpc(capture))

    def test_public_devnet_manifest_can_never_enter_owned_loopback_reconcile(self):
        manifest, capture = fixture()
        capture.update(commitment="finalized", finalizedSlot="200")
        rpc = reconcile.OwnedLoopbackCapturedRpc(capture)
        with self.assertRaisesRegex(reconcile.Refusal, "public cluster genesis|not admitted"):
            reconcile.reconcile_owned_loopback(manifest, rpc, {})

    def test_owned_loopback_receipt_program_substitution_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, _, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            expected = hashlib.sha256(receipt_path.read_bytes()).hexdigest()
            receipt["programs"][0]["elfSha256"] = "ef" * 32
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            rpc = reconcile.captured_owned_loopback(capture_path)
            with self.assertRaisesRegex(reconcile.Refusal, "expected SHA-256"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path, expected, evidence, manifest_path, capture_path, manifest, rpc
                )

    def test_owned_loopback_receipt_missing_program_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, _, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            receipt["programs"].pop()
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            with self.assertRaisesRegex(reconcile.Refusal, "seven-plus-provider"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path, hashlib.sha256(receipt_path.read_bytes()).hexdigest(), evidence, manifest_path, capture_path, manifest,
                    reconcile.captured_owned_loopback(capture_path),
                )

    def test_owned_loopback_receipt_missing_provider_closure_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, _, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            del receipt["providerClosure"]
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            with self.assertRaisesRegex(reconcile.Refusal, "providerClosure"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path,
                    hashlib.sha256(receipt_path.read_bytes()).hexdigest(),
                    evidence,
                    manifest_path,
                    capture_path,
                    manifest,
                    reconcile.captured_owned_loopback(capture_path),
                )

    def test_owned_loopback_provider_closure_program_substitution_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, _, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            closure_path = evidence / "provider-closure.json"
            closure = json.loads(closure_path.read_text())
            closure["providerPrograms"][0]["elfSha256"] = "ef" * 32
            closure_path.write_bytes(reconcile.canonical_bytes(closure))
            receipt["providerClosure"]["sha256"] = hashlib.sha256(
                closure_path.read_bytes()
            ).hexdigest()
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            with self.assertRaisesRegex(reconcile.Refusal, "captured immutable provider"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path,
                    hashlib.sha256(receipt_path.read_bytes()).hexdigest(),
                    evidence,
                    manifest_path,
                    capture_path,
                    manifest,
                    reconcile.captured_owned_loopback(capture_path),
                )

    def test_owned_loopback_provider_closure_provisional_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, _, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            closure_path = evidence / "provider-closure.json"
            closure = json.loads(closure_path.read_text())
            closure["status"] = "provisional"
            closure_path.write_bytes(reconcile.canonical_bytes(closure))
            receipt["providerClosure"]["sha256"] = hashlib.sha256(
                closure_path.read_bytes()
            ).hexdigest()
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            with self.assertRaisesRegex(reconcile.Refusal, "provisional"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path,
                    hashlib.sha256(receipt_path.read_bytes()).hexdigest(),
                    evidence,
                    manifest_path,
                    capture_path,
                    manifest,
                    reconcile.captured_owned_loopback(capture_path),
                )

    def test_owned_loopback_provider_closure_capture_substitution_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, capture, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            substitute_path = evidence / "substituted-capture.json"
            substitute_path.write_bytes(reconcile.canonical_bytes(capture))
            closure_path = evidence / "provider-closure.json"
            closure = json.loads(closure_path.read_text())
            closure["finalizedCapture"]["path"] = str(substitute_path.resolve())
            closure["finalizedCapture"]["sha256"] = hashlib.sha256(
                substitute_path.read_bytes()
            ).hexdigest()
            closure_path.write_bytes(reconcile.canonical_bytes(closure))
            receipt["providerClosure"]["sha256"] = hashlib.sha256(
                closure_path.read_bytes()
            ).hexdigest()
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            with self.assertRaisesRegex(reconcile.Refusal, "substitutes the singular finalized capture"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path,
                    hashlib.sha256(receipt_path.read_bytes()).hexdigest(),
                    evidence,
                    manifest_path,
                    capture_path,
                    manifest,
                    reconcile.captured_owned_loopback(capture_path),
                )

    def test_owned_loopback_captured_elf_substitution_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, capture, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            programdata = receipt["programs"][0]["programDataAddress"]
            encoded = capture["accounts"][programdata]["value"]["data"][0]
            body = bytearray(base64.b64decode(encoded))
            body[-1] ^= 1
            capture["accounts"][programdata]["value"]["data"][0] = base64.b64encode(body).decode()
            capture_path.write_bytes(reconcile.canonical_bytes(capture))
            receipt["capture"]["sha256"] = hashlib.sha256(capture_path.read_bytes()).hexdigest()
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            with self.assertRaisesRegex(reconcile.Refusal, "genesis bytes, or exact ELF"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path, hashlib.sha256(receipt_path.read_bytes()).hexdigest(), evidence,
                    manifest_path, capture_path, manifest,
                    reconcile.captured_owned_loopback(capture_path),
                )

    def test_owned_loopback_programdata_link_substitution_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, capture, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            program = receipt["programs"][0]["programId"]
            encoded = capture["accounts"][program]["value"]["data"][0]
            body = bytearray(base64.b64decode(encoded))
            body[-1] ^= 1
            capture["accounts"][program]["value"]["data"][0] = base64.b64encode(body).decode()
            capture_path.write_bytes(reconcile.canonical_bytes(capture))
            receipt["capture"]["sha256"] = hashlib.sha256(capture_path.read_bytes()).hexdigest()
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            with self.assertRaisesRegex(reconcile.Refusal, "exact Loader-v3 ProgramData link"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path, hashlib.sha256(receipt_path.read_bytes()).hexdigest(), evidence,
                    manifest_path, capture_path, manifest,
                    reconcile.captured_owned_loopback(capture_path),
                )

    def test_owned_loopback_self_consistent_authority_divergence_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, capture, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            program = receipt["programs"][1]
            programdata = program["programDataAddress"]
            body = bytearray(base64.b64decode(capture["accounts"][programdata]["value"]["data"][0]))
            substituted_authority, substituted_raw = key(82)
            body[13:45] = substituted_raw
            capture["accounts"][programdata]["value"]["data"][0] = base64.b64encode(body).decode()
            program["upgradeAuthority"] = substituted_authority
            program["genesisProgramDataSha256"] = hashlib.sha256(body).hexdigest()
            capture_path.write_bytes(reconcile.canonical_bytes(capture))
            receipt["capture"]["sha256"] = hashlib.sha256(capture_path.read_bytes()).hexdigest()
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            with self.assertRaisesRegex(reconcile.Refusal, "share one retained disposable authority"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path, hashlib.sha256(receipt_path.read_bytes()).hexdigest(), evidence,
                    manifest_path, capture_path, manifest,
                    reconcile.captured_owned_loopback(capture_path),
                )

    def test_owned_loopback_self_consistent_provider_authority_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, capture, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            program = receipt["programs"][-2]
            programdata = program["programDataAddress"]
            body = bytearray(base64.b64decode(capture["accounts"][programdata]["value"]["data"][0]))
            substituted_authority, substituted_raw = key(83)
            body[12] = 1
            body[13:45] = substituted_raw
            capture["accounts"][programdata]["value"]["data"][0] = base64.b64encode(body).decode()
            program["upgradeAuthority"] = substituted_authority
            program["genesisProgramDataSha256"] = hashlib.sha256(body).hexdigest()
            capture_path.write_bytes(reconcile.canonical_bytes(capture))
            receipt["capture"]["sha256"] = hashlib.sha256(capture_path.read_bytes()).hexdigest()
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            with self.assertRaisesRegex(reconcile.Refusal, "Pyth provider programs must be immutable"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path, hashlib.sha256(receipt_path.read_bytes()).hexdigest(), evidence,
                    manifest_path, capture_path, manifest,
                    reconcile.captured_owned_loopback(capture_path),
                )

    def test_owned_loopback_provisional_receipt_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, _, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            receipt["status"] = "provisional"
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            with self.assertRaisesRegex(reconcile.Refusal, "provisional"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path, hashlib.sha256(receipt_path.read_bytes()).hexdigest(), evidence, manifest_path, capture_path, manifest,
                    reconcile.captured_owned_loopback(capture_path),
                )

    def test_owned_loopback_partial_lifecycle_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, _, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            receipt["privateSession"]["completedStages"].pop()
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            with self.assertRaisesRegex(reconcile.Refusal, "partial"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path, hashlib.sha256(receipt_path.read_bytes()).hexdigest(), evidence, manifest_path, capture_path, manifest,
                    reconcile.captured_owned_loopback(capture_path),
                )

    def test_owned_loopback_self_consistent_partial_session_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, _, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            session_path = evidence / "session.json"
            session = json.loads(session_path.read_text())
            session["stages"].pop()
            session["completedStages"].pop()
            session["stageSetSha256"] = hashlib.sha256(
                reconcile.canonical_bytes(session["stages"])
            ).hexdigest()
            session_path.write_bytes(reconcile.canonical_bytes(session))
            session_sha256 = hashlib.sha256(session_path.read_bytes()).hexdigest()
            receipt["privateSession"]["sha256"] = session_sha256
            receipt["privateSession"]["completedStages"] = session["completedStages"]
            session_journal = next(
                row for row in receipt["journals"] if row["path"] == "session.json"
            )
            session_journal["sha256"] = session_sha256
            receipt["journalSetSha256"] = hashlib.sha256(
                reconcile.canonical_bytes(receipt["journals"])
            ).hexdigest()
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            with self.assertRaisesRegex(reconcile.Refusal, "partial"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path,
                    hashlib.sha256(receipt_path.read_bytes()).hexdigest(),
                    evidence,
                    manifest_path,
                    capture_path,
                    manifest,
                    reconcile.captured_owned_loopback(capture_path),
                )

    def test_owned_loopback_session_nested_completion_pointer_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, _, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            session_journal = next(
                row for row in receipt["journals"] if row["path"] == "session.json"
            )
            session_journal["completionPointer"] = "/stages/0/completionValue"
            receipt["journalSetSha256"] = hashlib.sha256(
                reconcile.canonical_bytes(receipt["journals"])
            ).hexdigest()
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            with self.assertRaisesRegex(reconcile.Refusal, "top-level completion journal"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path,
                    hashlib.sha256(receipt_path.read_bytes()).hexdigest(),
                    evidence,
                    manifest_path,
                    capture_path,
                    manifest,
                    reconcile.captured_owned_loopback(capture_path),
                )

    def test_owned_loopback_self_consistent_partial_chaos_session_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, _, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            chaos_path = evidence / "chaos-session.json"
            chaos = json.loads(chaos_path.read_text())
            chaos["stages"].pop()
            chaos_path.write_bytes(reconcile.canonical_bytes(chaos))
            chaos_sha256 = hashlib.sha256(chaos_path.read_bytes()).hexdigest()
            receipt["chaosSession"]["sha256"] = chaos_sha256
            chaos_journal = next(
                row for row in receipt["journals"] if row["path"] == "chaos-session.json"
            )
            chaos_journal["sha256"] = chaos_sha256
            receipt["journalSetSha256"] = hashlib.sha256(
                reconcile.canonical_bytes(receipt["journals"])
            ).hexdigest()
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            with self.assertRaisesRegex(reconcile.Refusal, "eight-stage hostile run"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path,
                    hashlib.sha256(receipt_path.read_bytes()).hexdigest(),
                    evidence,
                    manifest_path,
                    capture_path,
                    manifest,
                    reconcile.captured_owned_loopback(capture_path),
                )

    def test_owned_loopback_missing_typed_terminal_completion_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, _, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            receipt["journals"] = [
                row for row in receipt["journals"]
                if row["schema"] != reconcile.OWNED_LOOPBACK_TERMINAL_COMPLETION_SCHEMA
            ]
            receipt["journalSetSha256"] = hashlib.sha256(
                reconcile.canonical_bytes(receipt["journals"])
            ).hexdigest()
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            with self.assertRaisesRegex(reconcile.Refusal, "omits typed terminal completion"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path,
                    hashlib.sha256(receipt_path.read_bytes()).hexdigest(),
                    evidence,
                    manifest_path,
                    capture_path,
                    manifest,
                    reconcile.captured_owned_loopback(capture_path),
                )

    def test_owned_loopback_terminal_completion_cannot_lie_about_persisted_fee(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, _, receipt, manifest_path, capture_path, receipt_path, evidence = owned_loopback_fixture(root)
            completion_path = evidence / "terminal-completion.json"
            completion = json.loads(completion_path.read_text())
            completion["journals"][0]["transactionFeeLamports"] = "5001"
            completion["transactionFeesLamports"] = "30001"
            completion_path.write_bytes(reconcile.canonical_bytes(completion))
            descriptor = next(
                row for row in receipt["journals"]
                if row["schema"] == reconcile.OWNED_LOOPBACK_TERMINAL_COMPLETION_SCHEMA
            )
            descriptor["sha256"] = hashlib.sha256(completion_path.read_bytes()).hexdigest()
            receipt["journalSetSha256"] = hashlib.sha256(
                reconcile.canonical_bytes(receipt["journals"])
            ).hexdigest()
            rewrite_owned_loopback_receipt(receipt, receipt_path)
            with self.assertRaisesRegex(reconcile.Refusal, "persisted semantic-owner journal"):
                reconcile.authenticate_owned_loopback_session(
                    receipt_path,
                    hashlib.sha256(receipt_path.read_bytes()).hexdigest(),
                    evidence,
                    manifest_path,
                    capture_path,
                    manifest,
                    reconcile.captured_owned_loopback(capture_path),
                )

    def test_owned_loopback_missing_source_journal_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest, _, _, _, _, _, evidence = owned_loopback_fixture(root)
            (evidence / "fixture-journal.json").unlink()
            with self.assertRaisesRegex(reconcile.Refusal, "cannot resolve source journal"):
                reconcile.authenticate_owned_loopback_sources(manifest, evidence)

    def test_owned_loopback_completion_pointer_supports_nested_hot_and_refuses_bad_escape(self):
        value = {"terminal": {"hot": {"phase": "finalized"}}}
        self.assertEqual(
            reconcile.json_pointer(value, "/terminal/hot/phase", "nested Hot"),
            "finalized",
        )
        with self.assertRaisesRegex(reconcile.Refusal, "invalid RFC6901 escape"):
            reconcile.json_pointer(value, "/terminal/~2hot/phase", "nested Hot")

    def test_owned_loopback_capture_finality_boundary_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            _, capture, _, _, capture_path, _, _ = owned_loopback_fixture(root)
            capture["accounts"][next(iter(capture["accounts"]))]["contextSlot"] = "199"
            capture_path.write_bytes(reconcile.canonical_bytes(capture))
            with self.assertRaisesRegex(reconcile.Refusal, "singular finalizedSlot"):
                reconcile.captured_owned_loopback(capture_path)


if __name__ == "__main__":
    unittest.main()
