#!/usr/bin/env python3
"""Focused hostile tests for the activity harness.

Every filesystem and RPC fixture lives in one temporary directory.  The fake
keygen deliberately writes recognizable secret bytes; tests assert they never
appear in the public ledger or captured output.
"""

from __future__ import annotations

from contextlib import contextmanager, redirect_stderr, redirect_stdout
import base64
import dataclasses
import datetime as dt
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import threading
import unittest
from unittest import mock


MODULE_PATH = Path(__file__).with_name("activity.py")
SPEC = importlib.util.spec_from_file_location("dclutch_devnet_activity", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
activity = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = activity
SPEC.loader.exec_module(activity)


FUNDER = "F" * 32
EXTERNAL_FUNDER = "E" * 32
WALLET = "2" * 32
SIGNATURE = "3" * 64
ACTIVITY_SIGNATURE = "4" * 64
SUBSTITUTED_SIGNATURE = "5" * 64
FAILED_ACTIVITY_SIGNATURE = "6" * 64
SECRET_MARKER = "THIS_IS_TEST_SECRET_KEY_MATERIAL"


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def base58_encode(value: bytes) -> str:
    alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    number = int.from_bytes(value, "big")
    output = ""
    while number:
        number, remainder = divmod(number, 58)
        output = alphabet[remainder] + output
    return "1" * (len(value) - len(value.lstrip(b"\0"))) + (output or "1")


def limits(target: str = "owned-loopback") -> dict[str, int]:
    return {
        "maxConcurrency": 1,
        "minDispatchIntervalMs": 0 if target == "owned-loopback" else 1000,
        "maxTransactions": 20,
        "pollIntervalMs": 250,
        "maxPolls": 10,
    }


def operation(identifier: str, kind: str, depends: list[str]) -> dict[str, object]:
    operation_input: dict[str, object] = {"kind": kind}
    if kind in {"participant", "redeem"}:
        operation_input["walletRef"] = "alice"
    elif kind == "direct":
        operation_input.update({"sellerWalletRef": "alice", "buyerWalletRef": "alice"})
    elif kind == "retire":
        operation_input["rentRefundWalletRef"] = "alice"
    empty_delta = {"lamportDeltas": [], "tokenDeltas": [], "accountStateChanges": [], "positionChanges": []}
    return {
        "id": identifier,
        "order": 0,
        "kind": kind,
        "predecessorId": depends[0] if depends else None,
        "dependencyIds": depends,
        "feePayerWalletRef": "alice",
        "callerTarget": f"test/{identifier}",
        "callerSchema": None if kind == "direct" else f"dclutch-test-{identifier}-v1",
        "callerAvailability": "adapter-required" if kind == "direct" else "public-executable",
        "mutationExpected": kind != "direct",
        "evidenceOutputRef": f"receipt.{identifier}",
        "capture": {"signature": None, "finalizedSlot": None, "transactionFeeLamports": None},
        "input": operation_input,
        "expectedObservedDelta": empty_delta,
        "projectedAcceptedDelta": empty_delta,
    }


def scenario_value(target: str = "owned-loopback") -> dict[str, object]:
    rows: list[dict[str, object]] = []
    parent: list[str] = []
    for identifier, kind in [
        ("found", "found"),
        ("participant", "participant"),
        ("direct", "direct"),
        ("resolve", "resolve"),
        ("redeem", "redeem"),
        ("retire", "retire"),
    ]:
        row = operation(identifier, kind, parent)
        row["order"] = len(rows)
        rows.append(row)
        parent = [identifier]
    body: dict[str, object] = {
        "scenarioId": "test-lifecycle",
        "title": "Test lifecycle",
        "description": "A deterministic test-only lifecycle.",
        "clusterTarget": target,
        "genesisHash": activity.DEVNET_GENESIS_HASH if target == "devnet" else "owned-loopback-test-genesis",
        "evidenceLevel": "scenario-only",
        "market": {
            "profile": "flagship",
            "marketRef": "market.test-lifecycle",
            "inputArtifact": None,
            "outcomeCount": 4,
            "collateralMintRef": "mint.collateral",
            "claimMintRefs": ["mint.claim.0", "mint.claim.1", "mint.claim.2", "mint.claim.3"],
            "resolution": {"kind": "categorical", "selector": 0, "payoutAtomsPerClaim": ["1", "0", "0", "0"]},
            "priceScaleAtoms": "1000",
            "feeDenominator": "10000",
            "feeBasisPointsPerSide": 50,
            "feeRecipientAccountRef": "token.test-lifecycle.alice.collateral",
            "hoardPrincipalAccountRef": "token.test-lifecycle.hoard",
        },
        "limits": limits(target),
        "wallets": [{
            "id": "alice",
            "roles": ["participant", "trader"],
            "fundingLamports": "10000",
            "collateralAccountRef": "token.test-lifecycle.alice.collateral",
            "claimAccountRefs": [f"token.test-lifecycle.alice.claim.{index}" for index in range(4)],
            "positionAccountRef": "position.test-lifecycle.alice",
        }],
        "accounts": [
            {"id": "wallet.alice", "kind": "wallet", "address": None, "expectedOwnerRef": "solana-system-program", "mintRef": None, "tokenAuthorityWalletRef": None},
        ],
        "initialSnapshot": {"accountStates": [], "tokenBalances": [], "positionRevisions": []},
        "operations": rows,
        "finalSnapshot": {"accountStates": [], "tokenBalances": [], "positionRevisions": []},
        "retireEligible": True,
    }
    body_sha = hashlib.sha256(json.dumps(body, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
    return {
        "schema": "dclutch-devnet-economic-scenario-v1",
        "version": 1,
        "scenarioId": "test-lifecycle",
        "bodyDigestScope": "canonical-compact-scenario-body-json-v1",
        "bodySha256": body_sha,
        "body": body,
    }


def manifest_value(scenario: Path, target: str = "owned-loopback", rpc_url: str = "http://127.0.0.1:18899/") -> dict[str, object]:
    return {
        "schema": activity.LEGACY_MANIFEST_SCHEMA,
        "scenario": {"path": str(scenario), "sha256": digest(scenario)},
        "target": {
            "kind": target,
            "rpcUrl": rpc_url,
            "devnetGenesisHash": activity.DEVNET_GENESIS_HASH if target == "devnet" else None,
        },
        "inputs": [],
        "addressBindings": [
            {"ref": "wallet.alice", "source": {"kind": "wallet", "walletRef": "alice"}},
        ],
        "adapters": [
            {
                "id": "private-lifecycle",
                "covers": ["found", "participant", "direct", "resolve", "redeem", "retire"],
                "caller": "successor",
                "argv": ["local-private-validator-lifecycle-v1", "--work", "{{work}}", "--execute"],
                "dependsOn": [],
                "wallets": ["alice"],
                "mutation": True,
                "completion": {
                    "path": "{{work}}/receipts/private-lifecycle.json",
                    "schema": "dclutch-local-private-validator-lifecycle-v1",
                    "signaturePointers": ["/signature"],
                    "transactionListPointer": None,
                    "requiredTransactionLabels": [],
                    "requiredValues": {"/completed": True},
                },
            }
        ],
    }


def v3_scenario_value() -> dict[str, object]:
    value = scenario_value()
    body = value["body"]
    assert isinstance(body, dict)

    def wallet(wallet_id: str, role: str, amount: str) -> dict[str, object]:
        return {
            "id": wallet_id,
            "roles": [role],
            "fundingLamports": amount,
            "collateralAccountRef": f"token.test-lifecycle.{wallet_id}.collateral",
            "claimAccountRefs": [
                f"token.test-lifecycle.{wallet_id}.claim.{index}" for index in range(4)
            ],
            "positionAccountRef": f"position.test-lifecycle.{wallet_id}",
        }

    body["wallets"] = [
        wallet("deployer", "campaign-payer", "100000"),
        wallet("collateral-mint", "collateral-mint", "0"),
        wallet("collateral-wallet", "collateral-wallet", "0"),
        wallet("founding-beneficiary", "founding-beneficiary", "0"),
        wallet(
            "founding-projection-witness", "founding-projection-witness", "0"
        ),
        wallet("founding-source-funder", "founding-source-funder", "0"),
        wallet("alice", "participant", "10000"),
    ]
    operations = body["operations"]
    assert isinstance(operations, list) and isinstance(operations[0], dict)
    operations[0]["feePayerWalletRef"] = "deployer"
    value["bodySha256"] = hashlib.sha256(
        json.dumps(body, separators=(",", ":"), ensure_ascii=False).encode()
    ).hexdigest()
    return value


def v3_manifest_value(
    scenario: Path, checked_release: Path, market: Path
) -> dict[str, object]:
    founder = "7" * 32
    substituted = "8" * 32
    return {
        "schema": activity.MANIFEST_SCHEMA,
        "scenario": {"path": str(scenario), "sha256": digest(scenario)},
        "target": {
            "kind": "owned-loopback",
            "rpcUrl": "http://127.0.0.1:18899/",
            "devnetGenesisHash": None,
        },
        "inputs": [
            {
                "id": "checked-release",
                "path": str(checked_release),
                "sha256": digest(checked_release),
            },
            {"id": "market", "path": str(market), "sha256": digest(market)},
        ],
        "addressBindings": [
            {
                "ref": "wallet.alice",
                "source": {"kind": "wallet", "walletRef": "alice"},
            },
            {
                "ref": "core-upgrade-authority",
                "source": {"kind": "literal", "address": "9" * 32},
            },
        ],
        "adapters": [
            {
                "id": "founding",
                "covers": ["found"],
                "caller": "successor",
                "argv": [
                    "campaign",
                    "--founding-only",
                    "--plan",
                    "{{input.checked-release}}",
                    "--market",
                    "{{input.market}}",
                    "--keypair-campaign-payer",
                    "{{wallet.deployer.keypair}}",
                    "--keypair-collateral-mint",
                    "{{wallet.collateral-mint.keypair}}",
                    "--keypair-collateral-wallet",
                    "{{wallet.collateral-wallet.keypair}}",
                    "--keypair-founding-beneficiary",
                    "{{wallet.founding-beneficiary.keypair}}",
                    "--founding-founder",
                    founder,
                    "--keypair-founding-projection-witness",
                    "{{wallet.founding-projection-witness.keypair}}",
                    "--keypair-founding-source-funder",
                    "{{wallet.founding-source-funder.keypair}}",
                    "--substituted-founder",
                    substituted,
                    "--evidence",
                    "{{work}}/receipts/founding.json",
                    "--execute",
                ],
                "dependsOn": [],
                "wallets": ["deployer"],
                "mutation": True,
                "completion": {
                    "path": "{{work}}/receipts/founding.json",
                    "schema": activity.CAMPAIGN_REPORT_SCHEMA,
                    "signaturePointers": [],
                    "transactionListPointer": "/execution/transactions",
                    "requiredTransactionLabels": ["DCLTCFQ1", "DCLTPCB2", "DCLTGMF2"],
                    "requiredValues": {},
                },
            },
            {
                "id": "remaining-lifecycle",
                "covers": ["participant", "direct", "resolve", "redeem", "retire"],
                "caller": "successor",
                "argv": [
                    "local-private-validator-lifecycle-v1",
                    "--work",
                    "{{work}}",
                    "--execute",
                ],
                "dependsOn": ["founding"],
                "wallets": ["alice"],
                "mutation": True,
                "completion": {
                    "path": "{{work}}/receipts/private-lifecycle.json",
                    "schema": "dclutch-local-private-validator-lifecycle-v1",
                    "signaturePointers": ["/signature"],
                    "transactionListPointer": None,
                    "requiredTransactionLabels": [],
                    "requiredValues": {"/completed": True},
                },
            },
        ],
        "campaign": {
            "identities": [
                {
                    "role": role,
                    "source": (
                        {"kind": "literal", "address": founder}
                        if role == "founding-founder"
                        else {"kind": "literal", "address": substituted}
                        if role == "substituted-founder"
                        else {
                            "kind": "wallet",
                            "walletRef": "deployer"
                            if role == "campaign-payer"
                            else role,
                        }
                    ),
                }
                for role in activity.CAMPAIGN_IDENTITY_ROLES
            ],
            "permanentAuthorityRef": "core-upgrade-authority",
            "foundingAdapter": "founding",
            "initialFunding": {
                "walletRef": "deployer",
                "transferLamports": "100000",
            },
            "postInitFunding": [
                {
                    "id": "fund-alice",
                    "walletRef": "alice",
                    "transferLamports": "10000",
                    "afterAdapter": "founding",
                }
            ],
        },
    }


def executable(path: Path, source: str) -> Path:
    path.write_text(source, encoding="utf-8")
    path.chmod(0o755)
    return path


def fake_keygen(path: Path) -> Path:
    return executable(
        path,
        f"""#!/usr/bin/env python3
import os, pathlib, sys
if sys.argv[1] == 'new':
    target = pathlib.Path(sys.argv[sys.argv.index('--outfile') + 1])
    target.write_text('{SECRET_MARKER}')
    os.chmod(target, 0o600)
    raise SystemExit(0)
if sys.argv[1] == 'pubkey':
    name = pathlib.Path(sys.argv[2]).name
    print('{FUNDER}' if name == 'funder.json' else '{WALLET}')
    raise SystemExit(0)
raise SystemExit(2)
""",
    )


def fake_solana(path: Path, *, exit_code: int = 0) -> Path:
    return executable(
        path,
        f"""#!/usr/bin/env python3
import json
print(json.dumps({{'signature': '{SIGNATURE}'}}))
raise SystemExit({exit_code})
""",
    )


def fake_sign_only_solana(path: Path, signature: str) -> Path:
    message = base64.b64encode(b"\x02" * 100).decode()
    return executable(
        path,
        f"""#!/usr/bin/env python3
import json
print(json.dumps({{
  'blockhash': '{'B' * 32}',
  'signers': ['{FUNDER}={signature}'],
  'absent': [],
  'badSig': [],
  'message': '{message}'
}}))
""",
    )


def fake_caller(path: Path, *, exit_code: int = 0, schema: str = "dclutch-local-private-validator-lifecycle-v1") -> Path:
    return executable(
        path,
        f"""#!/usr/bin/env python3
import json, pathlib, sys
if '--help' in sys.argv:
    print('local-private-validator-lifecycle-v1')
    raise SystemExit(0)
work = pathlib.Path(sys.argv[sys.argv.index('--work') + 1])
count = work / 'private' / 'caller-count'
count.parent.mkdir(parents=True, exist_ok=True)
count.write_text(str(int(count.read_text()) + 1) if count.exists() else '1')
print('{SECRET_MARKER}')
if {exit_code} == 0:
    receipt = work / 'receipts' / 'private-lifecycle.json'
    receipt.parent.mkdir(parents=True, exist_ok=True)
    receipt.write_text(json.dumps({{'schema': '{schema}', 'completed': True, 'signature': '{ACTIVITY_SIGNATURE}'}}))
raise SystemExit({exit_code})
""",
    )


def funding_transaction(
    amount: int = 10_000,
    fee: int = 5_000,
    memo: str = "",
    *,
    source: str = FUNDER,
    destination: str = WALLET,
    source_balance: int = 100_000,
) -> dict[str, object]:
    return {
        "slot": 99,
        "transaction": {
            "message": {
                "accountKeys": [
                    {"pubkey": source, "signer": True, "writable": True},
                    {"pubkey": destination, "signer": False, "writable": True},
                    {"pubkey": "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr", "signer": False, "writable": False},
                ],
                "instructions": [
                    {"program": "system", "parsed": {"type": "transfer", "info": {"source": source, "destination": destination, "lamports": amount}}},
                    {"program": "spl-memo", "parsed": memo},
                ],
            }
        },
        "meta": {
            "err": None,
            "fee": fee,
            "preBalances": [source_balance, 0, 0],
            "postBalances": [source_balance - amount - fee, amount, 0],
            "preTokenBalances": [],
            "postTokenBalances": [],
        },
    }


def activity_transaction(signature: str = ACTIVITY_SIGNATURE, *, embedded_signature: str | None = None) -> dict[str, object]:
    return {
        "slot": 100,
        "transaction": {
            "signatures": [signature if embedded_signature is None else embedded_signature],
            "message": {
                "accountKeys": [{"pubkey": WALLET, "signer": True, "writable": True}],
                "instructions": [],
            },
        },
        "meta": {
            "err": None,
            "fee": 1_000,
            "preBalances": [10_000],
            "postBalances": [9_000],
            "preTokenBalances": [],
            "postTokenBalances": [],
        },
    }


class RpcState:
    def __init__(self) -> None:
        self.genesis = "loopback-test-genesis"
        self.transactions: dict[str, dict[str, object]] = {}
        self.signatures: list[str] = []
        self.signature_errors: dict[str, object] = {}
        self.balances: dict[str, int] = {}
        self.multiple_accounts: list[object | None] = []
        self.sent_signature: str | None = None


@contextmanager
def rpc_server(state: RpcState):
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_: object) -> None:
            pass

        def do_POST(self) -> None:  # noqa: N802 - stdlib callback name
            length = int(self.headers["content-length"])
            body = json.loads(self.rfile.read(length))
            method = body["method"]
            if method == "getGenesisHash":
                result: object = state.genesis
            elif method == "getBalance":
                result = {"context": {"slot": 101}, "value": state.balances.get(body["params"][0], 0)}
            elif method == "getMultipleAccounts":
                result = {
                    "context": {"slot": 102},
                    "value": state.multiple_accounts,
                }
            elif method == "getLatestBlockhash":
                result = {
                    "context": {"slot": 103},
                    "value": {
                        "blockhash": "B" * 32,
                        "lastValidBlockHeight": 1234,
                    },
                }
            elif method == "getFeeForMessage":
                result = {"context": {"slot": 104}, "value": 5_000}
            elif method == "sendTransaction":
                result = state.sent_signature
            elif method == "getTransaction":
                result = state.transactions.get(body["params"][0])
            elif method == "getSignaturesForAddress":
                result = [
                    {
                        "signature": signature,
                        "err": state.signature_errors.get(signature),
                        "slot": 99,
                    }
                    for signature in state.signatures
                ]
            else:
                result = None
            encoded = json.dumps({"jsonrpc": "2.0", "id": body["id"], "result": result}).encode()
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(encoded)))
            self.end_headers()
            self.wfile.write(encoded)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}/"
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


class ActivityTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory(prefix="dclutch-activity-test-")
        self.root = Path(self.temp.name)
        self.scenario = self.root / "scenario.json"
        write_json(self.scenario, scenario_value())
        self.manifest = self.root / "manifest.json"
        write_json(self.manifest, manifest_value(self.scenario))
        self.work = self.root / "work"
        self.work.mkdir(mode=0o700)
        self.keygen = fake_keygen(self.root / "solana-keygen")
        self.funder = self.root / "funder.json"
        self.funder.write_text(SECRET_MARKER, encoding="utf-8")
        self.funder.chmod(0o600)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def parsed(self):
        return activity.parse_manifest(self.manifest)

    def write_v3(self) -> dict[str, object]:
        write_json(self.scenario, v3_scenario_value())
        checked_release = self.root / "checked-release.json"
        market = self.root / "market.json"
        write_json(checked_release, {"fixture": "checked-release"})
        write_json(market, {"fixture": "market"})
        value = v3_manifest_value(self.scenario, checked_release, market)
        write_json(self.manifest, value)
        return value

    def prepare_finalized_funding(self, manifest, state: RpcState) -> None:
        activity.prepare_wallets(manifest, self.work, self.keygen)
        journal = activity.new_funding_journal(manifest, "alice", WALLET, FUNDER, 10_000, None)
        transaction = funding_transaction(memo=journal["memo"])
        state.transactions[SIGNATURE] = transaction
        state.signatures = [SIGNATURE]
        state.balances[WALLET] = 10_000
        final = activity.verify_funding_transaction(transaction, journal, SIGNATURE)
        activity.atomic_write_json(activity.funding_journal_path(self.work, "alice"), final)

    def test_strict_manifest_covers_the_six_stage_private_lifecycle(self) -> None:
        manifest = self.parsed()
        self.assertEqual(manifest.scenario.cluster_target, "owned-loopback")
        self.assertEqual([item.kind for item in manifest.scenario.operations], ["found", "participant", "direct", "resolve", "redeem", "retire"])
        self.assertEqual(manifest.adapters[0].argv[0], "local-private-validator-lifecycle-v1")

        changed = json.loads(self.scenario.read_text())
        changed["ignored"] = True
        write_json(self.scenario, changed)
        rewritten = manifest_value(self.scenario)
        write_json(self.manifest, rewritten)
        with self.assertRaisesRegex(activity.Refusal, "unknown fields"):
            self.parsed()

    def test_v3_owns_exact_campaign_identities_and_funding_partition(self) -> None:
        self.write_v3()
        manifest = self.parsed()
        assert manifest.campaign is not None
        self.assertEqual(manifest.schema, activity.MANIFEST_SCHEMA)
        self.assertEqual(
            [identity.role for identity in manifest.campaign.identities],
            list(activity.CAMPAIGN_IDENTITY_ROLES),
        )
        self.assertEqual(manifest.campaign.payer_wallet_ref, "deployer")
        self.assertEqual(
            [wallet.wallet_id for wallet in activity.initial_funding_wallets(manifest)],
            ["deployer"],
        )
        self.assertEqual(
            [row.journal_id for row in manifest.campaign.post_init_funding],
            ["fund-alice"],
        )
        self.assertEqual(
            [row.wallet_ref for row in manifest.campaign.post_init_funding],
            ["alice"],
        )

    def test_v3_refuses_prefunded_fresh_roles_aliases_and_missing_post_init(self) -> None:
        value = self.write_v3()
        changed = json.loads(json.dumps(value))
        changed["campaign"]["identities"][7]["source"]["address"] = "7" * 32
        write_json(self.manifest, changed)
        with self.assertRaisesRegex(activity.Refusal, "alias"):
            self.parsed()

        changed = json.loads(json.dumps(value))
        changed["campaign"]["postInitFunding"] = []
        write_json(self.manifest, changed)
        with self.assertRaisesRegex(activity.Refusal, "transaction wallets"):
            self.parsed()

        scenario = v3_scenario_value()
        scenario["body"]["wallets"][1]["fundingLamports"] = "1"
        scenario["bodySha256"] = hashlib.sha256(
            json.dumps(scenario["body"], separators=(",", ":"), ensure_ascii=False).encode()
        ).hexdigest()
        write_json(self.scenario, scenario)
        checked_release = self.root / "checked-release.json"
        market = self.root / "market.json"
        changed = v3_manifest_value(self.scenario, checked_release, market)
        write_json(self.manifest, changed)
        with self.assertRaisesRegex(activity.Refusal, "zero prefunding"):
            self.parsed()

    def test_campaign_freshness_reads_five_absences_at_one_finalized_boundary(self) -> None:
        state = RpcState()
        state.multiple_accounts = [None] * 5
        with rpc_server(state) as rpc_url:
            rpc = activity.Rpc(rpc_url)
            self.assertEqual(rpc.absent_accounts([str(index + 2) * 32 for index in range(5)]), 102)
            state.multiple_accounts[3] = {"lamports": 1}
            with self.assertRaisesRegex(activity.Refusal, "not all absent"):
                rpc.absent_accounts([str(index + 2) * 32 for index in range(5)])

    def test_post_init_funding_is_sign_once_and_dispatch_marker_gated(self) -> None:
        self.write_v3()
        manifest = self.parsed()
        assert manifest.campaign is not None
        spec = manifest.campaign.post_init_funding[0]
        path = activity.post_init_funding_journal_path(self.work, spec.journal_id)
        planned = activity.new_post_init_funding_journal(
            manifest,
            spec,
            FUNDER,
            WALLET,
            "a" * 64,
            "b" * 64,
        )
        activity.atomic_write_json(path, planned)
        planned = activity.authenticated_state(path, "planned post-init")
        signature = base58_encode(b"\x01" * 64)
        message = base64.b64encode(b"\x02" * 100).decode()
        prepared = activity.prepare_post_init_funding_journal(
            planned,
            {
                "blockhash": "B" * 32,
                "signers": [f"{FUNDER}={signature}"],
                "absent": [],
                "badSig": [],
                "message": message,
            },
            "B" * 32,
            1234,
        )
        activity.atomic_write_json(path, prepared)
        prepared = activity.authenticated_state(path, "prepared post-init")
        self.assertEqual(prepared["phase"], "Prepared")
        self.assertEqual(
            prepared["packetSha256"],
            hashlib.sha256(base64.b64decode(prepared["packetBase64"])).hexdigest(),
        )
        with self.assertRaisesRegex(activity.Refusal, "Dispatching"):
            activity.submitted_post_init_funding_journal(prepared, signature)

        quoted = activity.quote_post_init_funding_fee(prepared, 104, 5_000)
        activity.atomic_write_json(path, quoted)
        quoted = activity.authenticated_state(path, "quoted post-init")
        dispatching = activity.dispatching_post_init_funding_journal(quoted)
        activity.atomic_write_json(path, dispatching)
        dispatching = activity.authenticated_state(path, "dispatching post-init")
        with self.assertRaisesRegex(activity.Refusal, "another packet"):
            activity.submitted_post_init_funding_journal(
                dispatching, base58_encode(b"\x03" * 64)
            )
        submitted = activity.submitted_post_init_funding_journal(
            dispatching, signature
        )
        activity.atomic_write_json(path, submitted)
        submitted = activity.authenticated_state(path, "submitted post-init")
        finalized = activity.finalize_post_init_funding_journal(
            submitted,
            funding_transaction(
                amount=spec.transfer_lamports,
                memo=submitted["memo"],
            ),
        )
        self.assertEqual(finalized["phase"], "Finalized")
        self.assertEqual(finalized["signature"], signature)
        self.assertEqual(finalized["feeLamports"], "5000")
        substituted_fee = funding_transaction(
            fee=5_001,
            amount=spec.transfer_lamports,
            memo=submitted["memo"],
        )
        with self.assertRaisesRegex(activity.Refusal, "pre-dispatch quote"):
            activity.finalize_post_init_funding_journal(submitted, substituted_fee)
        activity.atomic_write_json(path, finalized)
        post_closure = activity.write_post_init_funding_closure(
            manifest,
            self.work,
            FUNDER,
            "a" * 64,
            "b" * 64,
        )
        self.assertEqual(post_closure["totalTransferLamports"], "10000")
        self.assertEqual(post_closure["totalFeeLamports"], "5000")
        self.assertNotIn("packetBase64", json.dumps(post_closure))
        initial_path = activity.funding_closure_path(self.work, manifest)
        initial = {
            "schema": activity.INITIAL_FUNDING_CLOSURE_SCHEMA,
            "totalTransferLamports": "100000",
            "totalFundingFeeLamports": "5000",
        }
        activity.atomic_write_json(initial_path, initial)
        lifecycle = activity.write_funding_lifecycle(
            manifest,
            self.work,
            activity.authenticated_state(initial_path, "initial closure"),
            post_closure,
        )
        self.assertEqual(lifecycle["externalTransferLamports"], "100000")
        self.assertEqual(lifecycle["postInitTransferLamports"], "10000")
        self.assertEqual(lifecycle["grossFundingFeeLamports"], "10000")

    def test_post_init_controller_signs_sends_and_closes_exact_plan(self) -> None:
        self.write_v3()
        manifest = self.parsed()
        assert manifest.campaign is not None
        public_ledger_path = activity.wallet_paths(self.work)[2]
        activity.atomic_write_json(
            public_ledger_path,
            {
                "schema": activity.WALLET_LEDGER_SCHEMA,
                "manifestSha256": manifest.sha256,
                "scenarioSha256": manifest.scenario.sha256,
                "wallets": [],
            },
            mode=0o644,
        )
        initial = activity.new_funding_journal(
            manifest,
            "deployer",
            FUNDER,
            EXTERNAL_FUNDER,
            100_000,
            None,
        )
        initial_final = activity.verify_funding_transaction(
            funding_transaction(
                amount=100_000,
                memo=initial["memo"],
                source=EXTERNAL_FUNDER,
                destination=FUNDER,
                source_balance=200_000,
            ),
            initial,
            SIGNATURE,
        )
        activity.atomic_write_json(
            activity.funding_journal_path(self.work, "deployer"), initial_final
        )
        initial_closure = activity.write_funding_closure(
            manifest,
            self.work,
            manifest.scenario.genesis_hash,
            None,
            EXTERNAL_FUNDER,
        )
        initial_closure_sha256 = digest(
            activity.funding_closure_path(self.work, manifest)
        )
        activity.atomic_write_json(
            activity.adapter_journal_path(self.work, "founding"),
            {"phase": "finalized"},
        )
        payer_keypair = self.root / "campaign-payer.json"
        payer_keypair.write_text(SECRET_MARKER, encoding="utf-8")
        payer_keypair.chmod(0o600)
        private_wallets = {
            "deployer": {"keypair": str(payer_keypair), "address": FUNDER},
        }
        public_wallets = {
            "deployer": {"address": FUNDER},
            "alice": {"address": WALLET},
        }
        expected_signature = base58_encode(b"\x01" * 64)
        solana = fake_sign_only_solana(self.root / "solana-sign-only", expected_signature)
        state = RpcState()
        state.sent_signature = expected_signature
        with rpc_server(state) as rpc_url:
            manifest = dataclasses.replace(manifest, rpc_url=rpc_url)

            def send_and_install(packet: str) -> str:
                journal = activity.authenticated_state(
                    activity.post_init_funding_journal_path(self.work, "fund-alice"),
                    "dispatching post-init",
                )
                state.transactions[expected_signature] = funding_transaction(
                    memo=journal["memo"]
                )
                return expected_signature

            rpc = activity.Rpc(rpc_url)
            with (
                mock.patch.object(rpc, "fee_for_message", return_value=(104, 5_001)),
                mock.patch.object(
                    rpc, "send_transaction", side_effect=send_and_install
                ) as refused_send,
            ):
                with self.assertRaisesRegex(activity.Refusal, "fee quote exceeds"):
                    activity.run_post_init_funding(
                        manifest,
                        self.work,
                        solana,
                        private_wallets,
                        public_wallets,
                        rpc,
                        None,
                        initial_closure_sha256,
                        poll_only=False,
                        max_transfer_lamports=10_000,
                        max_fee_lamports=5_000,
                    )
                refused_send.assert_not_called()
            refused = activity.authenticated_state(
                activity.post_init_funding_journal_path(self.work, "fund-alice"),
                "over-cap post-init",
            )
            self.assertEqual(refused["phase"], "Prepared")
            self.assertEqual(refused["quotedFeeLamports"], "5001")
            activity.post_init_funding_journal_path(
                self.work, "fund-alice"
            ).unlink()

            with mock.patch.object(rpc, "send_transaction", side_effect=send_and_install):
                status = activity.run_post_init_funding(
                    manifest,
                    self.work,
                    solana,
                    private_wallets,
                    public_wallets,
                    rpc,
                    None,
                    initial_closure_sha256,
                    poll_only=False,
                    max_transfer_lamports=10_000,
                    max_fee_lamports=5_000,
                )
        self.assertEqual(status, "post-init-complete")
        self.assertEqual(initial_closure["totalTransferLamports"], "100000")
        self.assertTrue(activity.post_init_funding_closure_path(self.work).exists())
        self.assertTrue(activity.funding_lifecycle_path(self.work).exists())

    def test_cycle_and_preflight_only_direct_refuse(self) -> None:
        changed = scenario_value()
        changed["body"]["operations"][0]["predecessorId"] = "retire"  # type: ignore[index]
        changed["body"]["operations"][0]["dependencyIds"] = ["retire"]  # type: ignore[index]
        changed["bodySha256"] = hashlib.sha256(json.dumps(changed["body"], separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
        write_json(self.scenario, changed)
        write_json(self.manifest, manifest_value(self.scenario))
        with self.assertRaisesRegex(activity.Refusal, "canonical|cycle"):
            self.parsed()

        devnet_scenario = scenario_value("devnet")
        write_json(self.scenario, devnet_scenario)
        value = manifest_value(self.scenario, "devnet", "https://api.devnet.solana.com:443/")
        value["adapters"][0]["caller"] = "dclutch-cli"  # type: ignore[index]
        value["adapters"][0]["argv"] = ["intent", "sell"]  # type: ignore[index]
        write_json(self.manifest, value)
        with self.assertRaises(activity.Refusal):
            self.parsed()

    def test_wallet_preparation_never_exposes_secret_bytes_and_is_resumable(self) -> None:
        manifest = self.parsed()
        capture = io.StringIO()
        with redirect_stdout(capture), redirect_stderr(capture):
            public = activity.prepare_wallets(manifest, self.work, self.keygen)
            resumed = activity.prepare_wallets(manifest, self.work, self.keygen)
        self.assertEqual(public, resumed)
        self.assertNotIn(SECRET_MARKER, capture.getvalue())
        ledger_path = self.work / "public" / "wallet-ledger.json"
        self.assertNotIn(SECRET_MARKER, ledger_path.read_text())
        key_path = self.work / "private" / "wallets" / "alice.json"
        self.assertEqual(key_path.read_text(), SECRET_MARKER)
        self.assertEqual(key_path.stat().st_mode & 0o777, 0o600)
        self.assertEqual((self.work / "private").stat().st_mode & 0o777, 0o700)

        substituted = self.work / "private" / "not-a-scenario-wallet.json"
        substituted.write_text(SECRET_MARKER, encoding="utf-8")
        substituted.chmod(0o600)
        private_path = self.work / "private" / "wallet-index.json"
        private = activity.authenticated_state(private_path, "private wallet index")
        private["wallets"][0]["keypair"] = str(substituted)
        activity.atomic_write_json(private_path, private)
        with self.assertRaisesRegex(activity.Refusal, "exact disposable scenario path"):
            activity.prepare_wallets(manifest, self.work, self.keygen)

    def test_funding_finalizes_exact_transaction_arithmetic(self) -> None:
        state = RpcState()
        with rpc_server(state) as rpc_url:
            write_json(self.manifest, manifest_value(self.scenario, rpc_url=rpc_url))
            manifest = self.parsed()
            activity.prepare_wallets(manifest, self.work, self.keygen)
            journal = activity.new_funding_journal(manifest, "alice", WALLET, FUNDER, 10_000, None)
            state.transactions[SIGNATURE] = funding_transaction(memo=journal["memo"])
            state.signatures = [SIGNATURE]
            solana = fake_solana(self.root / "solana")
            with mock.patch.object(activity, "new_funding_journal", return_value=journal):
                activity.fund_wallets(manifest, self.work, solana, self.keygen, self.funder, None, poll_only=False)
            saved = activity.authenticated_state(activity.funding_journal_path(self.work, "alice"), "funding")
            self.assertEqual(saved["phase"], "finalized")
            self.assertEqual(saved["feeLamports"], "5000")
            self.assertEqual(saved["funderPreLamports"], "100000")
            self.assertEqual(saved["funderPostLamports"], "85000")
            self.assertEqual(saved["walletPreLamports"], "0")
            self.assertEqual(saved["walletPostLamports"], "10000")

    def test_dispatch_failure_becomes_poll_only_and_recovers_by_memo(self) -> None:
        state = RpcState()
        with rpc_server(state) as rpc_url:
            write_json(self.manifest, manifest_value(self.scenario, rpc_url=rpc_url))
            manifest = self.parsed()
            activity.prepare_wallets(manifest, self.work, self.keygen)
            journal = activity.new_funding_journal(manifest, "alice", WALLET, FUNDER, 10_000, None)
            failing = fake_solana(self.root / "solana", exit_code=1)
            with mock.patch.object(activity, "new_funding_journal", return_value=journal):
                with self.assertRaisesRegex(activity.Refusal, "ambiguous"):
                    activity.fund_wallets(manifest, self.work, failing, self.keygen, self.funder, None, poll_only=False)
            saved = activity.authenticated_state(activity.funding_journal_path(self.work, "alice"), "funding")
            self.assertEqual(saved["phase"], "dispatching")
            with self.assertRaisesRegex(activity.Refusal, "poll-only"):
                activity.fund_wallets(manifest, self.work, failing, self.keygen, self.funder, None, poll_only=True)
            state.transactions[SIGNATURE] = funding_transaction(memo=journal["memo"])
            state.signatures = [SIGNATURE]
            activity.fund_wallets(manifest, self.work, failing, self.keygen, self.funder, None, poll_only=True)
            final = activity.authenticated_state(activity.funding_journal_path(self.work, "alice"), "funding")
            self.assertEqual(final["phase"], "finalized")

    def test_hostile_funding_arithmetic_and_unacknowledged_devnet_refuse(self) -> None:
        manifest = self.parsed()
        journal = activity.new_funding_journal(manifest, "alice", WALLET, FUNDER, 10_000, None)
        hostile = funding_transaction(memo=journal["memo"])
        hostile["meta"]["postBalances"][1] = 9_999  # type: ignore[index]
        with self.assertRaisesRegex(activity.Refusal, "exact wallet amount"):
            activity.verify_funding_transaction(hostile, journal, SIGNATURE)

        devnet_scenario = dataclasses.replace(manifest.scenario, cluster_target="devnet")
        devnet = dataclasses.replace(
            manifest,
            scenario=devnet_scenario,
            rpc_url="https://api.devnet.solana.com:443/",
            devnet_genesis_hash=activity.DEVNET_GENESIS_HASH,
        )
        with self.assertRaisesRegex(activity.Refusal, "held until --live-authorization"):
            activity.require_live_authorization(devnet, None)

        activity.validate_supervisor_rpc_join(
            "https://api.devnet.solana.com/",
            "https://api.devnet.solana.com:443/",
        )
        with self.assertRaisesRegex(activity.Refusal, "frozen devnet join"):
            activity.validate_supervisor_rpc_join(
                "https://api.devnet.solana.com:443/",
                "https://api.devnet.solana.com:443/",
            )

    def test_bounded_live_authorization_caps_the_exact_wallet_bankroll(self) -> None:
        manifest = self.parsed()
        devnet = dataclasses.replace(
            manifest,
            scenario=dataclasses.replace(manifest.scenario, cluster_target="devnet"),
            rpc_url=activity.DEVNET_MANIFEST_RPC_URL,
            devnet_genesis_hash=activity.DEVNET_GENESIS_HASH,
        )
        now = dt.datetime.now(dt.timezone.utc)
        path = self.root / "bounded-live-authorization.json"
        value = {
            "schema": activity.BOUNDED_AUTHORIZATION_SCHEMA,
            "manifestSha256": devnet.sha256,
            "scenarioSha256": devnet.scenario.sha256,
            "devnetGenesisHash": activity.DEVNET_GENESIS_HASH,
            "marketRef": devnet.scenario.market_ref,
            "notBefore": (now - dt.timedelta(minutes=1)).isoformat().replace("+00:00", "Z"),
            "expiresAt": (now + dt.timedelta(hours=1)).isoformat().replace("+00:00", "Z"),
            "maxCycles": 1,
            "maxSpendLamports": "10000",
            "maxFeeLamports": "5000",
            "prefundedWalletClosureSha256": "2" * 64,
            "checkedReleaseSha256": "3" * 64,
            "marketSha256": "4" * 64,
            "acceptedHarnessSha256": "5" * 64,
            "acceptedHarnessSourceCommit": "6" * 40,
            "dclutchSha256": "7" * 64,
            "successorSha256": "8" * 64,
            "solanaKeygenSha256": "9" * 64,
            "authorization": "authorize-bounded-devnet-activity-live-send",
        }
        write_json(path, value)
        self.assertEqual(
            activity.bounded_live_authorization(path, devnet),
            (digest(path), 1, 10_000, 5_000, "2" * 64),
        )
        value["maxSpendLamports"] = "10001"
        write_json(path, value)
        with self.assertRaisesRegex(activity.Refusal, "exceeds scenario wallet bankroll"):
            activity.bounded_live_authorization(path, devnet)
        value["maxSpendLamports"] = "10000"
        value["maxCycles"] = 0
        write_json(path, value)
        with self.assertRaisesRegex(activity.Refusal, "maxCycles"):
            activity.bounded_live_authorization(path, devnet)

    def test_supervisor_mode_is_explicit_and_mutually_exclusive(self) -> None:
        parser = activity.supervisor_parser()
        required = [
            "--manifest", "/tmp/manifest",
            "--manifest-sha256", "1" * 64,
            "--scenario-id", "scenario",
            "--work", "/tank/dclutch-activity/runs/" + "1" * 64,
            "--rpc-url", activity.DEVNET_SUPERVISOR_RPC_URL,
            "--i-mean-devnet", activity.DEVNET_GENESIS_HASH,
            "--journal", "/tank/dclutch-activity/request.json",
            "--evidence-dir", "/tank/dclutch-activity/evidence",
            "--accepted-harness-sha256", "2" * 64,
            "--accepted-harness-source-commit", "3" * 40,
            "--scenario-sha256", "4" * 64,
            "--checked-release", "/tmp/release",
            "--checked-release-sha256", "5" * 64,
            "--market", "/tmp/market",
            "--market-sha256", "6" * 64,
            "--cycle-id", "cycle",
            "--dclutch-bin", "/tmp/dclutch",
            "--accepted-dclutch-sha256", "7" * 64,
            "--successor-bin", "/tmp/successor",
            "--accepted-successor-sha256", "8" * 64,
            "--solana-keygen-bin", "/tmp/keygen",
            "--accepted-solana-keygen-sha256", "9" * 64,
        ]
        with self.assertRaises(SystemExit):
            parser.parse_args(required)
        with self.assertRaises(SystemExit):
            parser.parse_args([*required, "--no-send", "--live-send"])
        parsed = parser.parse_args(
            [
                *required,
                "--solana-bin",
                "/tmp/solana",
                "--accepted-solana-sha256",
                "a" * 64,
                "--live-send",
            ]
        )
        self.assertTrue(parsed.live_send)
        self.assertEqual(parsed.solana_bin, "/tmp/solana")

    def test_v3_bounded_authorization_binds_plan_and_separate_funding_caps(self) -> None:
        self.write_v3()
        manifest = self.parsed()
        devnet = dataclasses.replace(
            manifest,
            scenario=dataclasses.replace(
                manifest.scenario,
                cluster_target="devnet",
                genesis_hash=activity.DEVNET_GENESIS_HASH,
            ),
            rpc_url=activity.DEVNET_MANIFEST_RPC_URL,
            devnet_genesis_hash=activity.DEVNET_GENESIS_HASH,
        )
        path = self.root / "v3-live-authorization.json"
        now = dt.datetime.now(dt.timezone.utc)
        value = {
            "schema": activity.V3_BOUNDED_AUTHORIZATION_SCHEMA,
            "manifestSha256": devnet.sha256,
            "scenarioSha256": devnet.scenario.sha256,
            "devnetGenesisHash": activity.DEVNET_GENESIS_HASH,
            "marketRef": devnet.scenario.market_ref,
            "notBefore": (now - dt.timedelta(minutes=1)).isoformat(),
            "expiresAt": (now + dt.timedelta(hours=1)).isoformat(),
            "authorization": "authorize-bounded-devnet-activity-v3-live-send",
            "maxCycles": 1,
            "maxSpendLamports": "100000",
            "maxFeeLamports": "20000",
            "maxPostInitTransferLamports": "10000",
            "maxPostInitFeeLamports": "5000",
            "initialFundingClosureSha256": "1" * 64,
            "postInitFundingPlanSha256": activity.post_init_funding_plan_sha256(
                devnet
            ),
            "checkedReleaseSha256": "2" * 64,
            "marketSha256": "3" * 64,
            "acceptedHarnessSha256": "4" * 64,
            "acceptedHarnessSourceCommit": "5" * 40,
            "dclutchSha256": "6" * 64,
            "successorSha256": "7" * 64,
            "solanaKeygenSha256": "8" * 64,
            "solanaSha256": "9" * 64,
        }
        write_json(path, value)
        self.assertEqual(
            activity.v3_bounded_live_authorization(path, devnet),
            (
                digest(path),
                1,
                100_000,
                20_000,
                10_000,
                5_000,
                "1" * 64,
                activity.post_init_funding_plan_sha256(devnet),
            ),
        )

        value["maxPostInitTransferLamports"] = "9999"
        write_json(path, value)
        with self.assertRaisesRegex(activity.Refusal, "plan exceeds"):
            activity.v3_bounded_live_authorization(path, devnet)

        value["maxPostInitTransferLamports"] = "10000"
        value["postInitFundingPlanSha256"] = "a" * 64
        write_json(path, value)
        with self.assertRaisesRegex(activity.Refusal, "changed the post-init funding plan"):
            activity.v3_bounded_live_authorization(path, devnet)

        value["postInitFundingPlanSha256"] = activity.post_init_funding_plan_sha256(
            devnet
        )
        value["maxSpendLamports"] = "14999"
        write_json(path, value)
        with self.assertRaisesRegex(activity.Refusal, "fee cap exceed"):
            activity.v3_bounded_live_authorization(path, devnet)
        with self.assertRaisesRegex(activity.Refusal, "fee cap exceed"):
            activity.run_activity(
                devnet,
                self.work,
                self.root / "absent-dclutch",
                self.root / "absent-successor",
                self.keygen,
                path,
                self.root / "absent-solana",
                poll_only=False,
            )

    def test_reconciled_wallet_debit_is_exact_and_bounded(self) -> None:
        value = {
            "schema": activity.RECONCILIATION_SCHEMA,
            "postInitFunding": [
                {
                    "transferLamports": "10000",
                    "feeLamports": "5000",
                    "payerLamportDelta": "-15000",
                    "walletLamportDelta": "10000",
                }
            ],
            "activity": [
                {
                    "transactions": [
                        {"walletLamportDeltas": {"alice": "-42", "bob": "7"}},
                        {"walletLamportDeltas": {"alice": "20", "bob": "-8"}},
                    ]
                },
            ],
        }
        self.assertEqual(activity.reconciled_wallet_debit_lamports(value), 15_050)
        self.assertEqual(
            activity.reconciled_post_init_funding_totals(value), (10_000, 5_000)
        )
        value["postInitFunding"][0]["payerLamportDelta"] = "-14999"
        with self.assertRaisesRegex(activity.Refusal, "do not close"):
            activity.reconciled_post_init_funding_totals(value)
        value["postInitFunding"][0]["payerLamportDelta"] = "-15000"
        value["activity"][0]["transactions"][0]["walletLamportDeltas"]["alice"] = "-0"
        with self.assertRaisesRegex(activity.Refusal, "canonical signed"):
            activity.reconciled_wallet_debit_lamports(value)

        fee_value = {
            "schema": activity.RECONCILIATION_SCHEMA,
            "activity": [
                {"transactions": [{"feeLamports": "5000"}, {"feeLamports": "7000"}]},
                {"transactions": [{"feeLamports": "3"}]},
            ],
        }
        self.assertEqual(activity.reconciled_activity_fee_lamports(fee_value), 12_003)
        fee_value["activity"][0]["transactions"][0]["feeLamports"] = "05000"
        with self.assertRaisesRegex(activity.Refusal, "canonical unsigned decimal"):
            activity.reconciled_activity_fee_lamports(fee_value)

    def test_finalized_funding_closure_is_distinct_and_substitution_hostile(self) -> None:
        manifest = self.parsed()
        activity.prepare_wallets(manifest, self.work, self.keygen)
        funding_authorization = "a" * 64
        journal = activity.new_funding_journal(
            manifest, "alice", WALLET, FUNDER, 10_000, funding_authorization
        )
        final = activity.verify_funding_transaction(
            funding_transaction(memo=journal["memo"]), journal, SIGNATURE
        )
        journal_path = activity.funding_journal_path(self.work, "alice")
        activity.atomic_write_json(journal_path, final)
        closure = activity.write_funding_closure(
            manifest,
            self.work,
            manifest.scenario.genesis_hash,
            funding_authorization,
            FUNDER,
        )
        closure_path = activity.funding_closure_path(self.work)
        self.assertEqual(closure["schema"], activity.FUNDING_CLOSURE_SCHEMA)
        self.assertEqual(closure["totalTransferLamports"], "10000")
        self.assertEqual(closure["wallets"][0]["journalSha256"], digest(journal_path))
        activity.authenticate_funding_closure(
            manifest, self.work, digest(closure_path)
        )
        final["feeLamports"] = "1"
        activity.atomic_write_json(journal_path, final)
        with self.assertRaisesRegex(activity.Refusal, "wallet rows changed"):
            activity.authenticate_funding_closure(
                manifest, self.work, digest(closure_path)
            )

    def test_cleanup_requires_final_journal_and_removes_only_secret_keys(self) -> None:
        manifest = self.parsed()
        activity.prepare_wallets(manifest, self.work, self.keygen)
        journal = activity.new_funding_journal(manifest, "alice", WALLET, FUNDER, 10_000, None)
        activity.atomic_write_json(activity.funding_journal_path(self.work, "alice"), journal)
        with self.assertRaisesRegex(activity.Refusal, "not finalized"):
            activity.cleanup_keys(manifest, self.work, self.keygen, manifest.scenario.scenario_id)
        final = activity.verify_funding_transaction(funding_transaction(memo=journal["memo"]), journal, SIGNATURE)
        activity.atomic_write_json(activity.funding_journal_path(self.work, "alice"), final)
        activity.atomic_write_json(
            activity.adapter_journal_path(self.work, "private-lifecycle"),
            {
                "schema": activity.ADAPTER_JOURNAL_SCHEMA,
                "manifestSha256": manifest.sha256,
                "scenarioSha256": manifest.scenario.sha256,
                "adapterId": "private-lifecycle",
                "phase": "finalized",
            },
        )
        activity.cleanup_keys(manifest, self.work, self.keygen, manifest.scenario.scenario_id)
        self.assertFalse((self.work / "private" / "wallets" / "alice.json").exists())
        self.assertTrue((self.work / "public" / "wallet-ledger.json").exists())
        self.assertTrue((self.work / "public" / "wallet-cleanup.json").exists())

    def test_scheduler_capability_journal_and_exact_reconciliation(self) -> None:
        state = RpcState()
        with rpc_server(state) as rpc_url:
            write_json(self.manifest, manifest_value(self.scenario, rpc_url=rpc_url))
            manifest = self.parsed()
            self.prepare_finalized_funding(manifest, state)
            caller = fake_caller(self.root / "successor")
            state.transactions[ACTIVITY_SIGNATURE] = activity_transaction()
            state.signatures = [ACTIVITY_SIGNATURE, SIGNATURE]
            state.balances[WALLET] = 9_000

            activity.run_activity(manifest, self.work, caller, caller, self.keygen, None, poll_only=False)
            journal = activity.authenticated_state(activity.adapter_journal_path(self.work, "private-lifecycle"), "adapter")
            self.assertEqual(journal["phase"], "finalized")
            self.assertEqual(journal["signatures"], [ACTIVITY_SIGNATURE])
            self.assertNotIn(SECRET_MARKER, json.dumps(journal))
            self.assertIn(SECRET_MARKER, (self.work / "private" / "logs" / "private-lifecycle.log").read_text())

            result = activity.reconcile_activity(manifest, self.work, caller, caller, self.keygen)
            self.assertEqual(result["schema"], activity.RECONCILIATION_SCHEMA)
            self.assertEqual(result["wallets"][0]["activityLamportDelta"], "-1000")
            self.assertEqual(result["wallets"][0]["finalLamports"], "9000")
            self.assertFalse(result["untrustedProjectionUsed"])

    def test_reconciliation_refuses_foreign_history_and_final_balance_mismatch(self) -> None:
        state = RpcState()
        with rpc_server(state) as rpc_url:
            write_json(self.manifest, manifest_value(self.scenario, rpc_url=rpc_url))
            manifest = self.parsed()
            self.prepare_finalized_funding(manifest, state)
            caller = fake_caller(self.root / "successor")
            state.transactions[ACTIVITY_SIGNATURE] = activity_transaction()
            state.signatures = [ACTIVITY_SIGNATURE, SIGNATURE]
            state.balances[WALLET] = 8_999
            activity.run_activity(manifest, self.work, caller, caller, self.keygen, None, poll_only=False)
            with self.assertRaisesRegex(activity.Refusal, "final lamports"):
                activity.reconcile_activity(manifest, self.work, caller, caller, self.keygen)
            state.balances[WALLET] = 9_000
            state.signatures = [SUBSTITUTED_SIGNATURE, ACTIVITY_SIGNATURE, SIGNATURE]
            with self.assertRaisesRegex(activity.Refusal, "foreign signatures"):
                activity.reconcile_activity(manifest, self.work, caller, caller, self.keygen)

    def test_dispatch_crash_can_only_resume_by_polling_completion(self) -> None:
        state = RpcState()
        with rpc_server(state) as rpc_url:
            write_json(self.manifest, manifest_value(self.scenario, rpc_url=rpc_url))
            manifest = self.parsed()
            self.prepare_finalized_funding(manifest, state)
            caller = fake_caller(self.root / "successor", exit_code=1)
            with self.assertRaisesRegex(activity.Refusal, "ambiguous|poll-only"):
                activity.run_activity(manifest, self.work, caller, caller, self.keygen, None, poll_only=False)
            self.assertEqual((self.work / "private" / "caller-count").read_text(), "1")
            receipt = self.work / "receipts" / "private-lifecycle.json"
            receipt.parent.mkdir(parents=True, exist_ok=True)
            write_json(receipt, {"schema": "dclutch-local-private-validator-lifecycle-v1", "completed": True, "signature": ACTIVITY_SIGNATURE})
            state.transactions[ACTIVITY_SIGNATURE] = activity_transaction()
            state.signatures = [ACTIVITY_SIGNATURE, SIGNATURE]
            activity.run_activity(manifest, self.work, caller, caller, self.keygen, None, poll_only=True)
            self.assertEqual((self.work / "private" / "caller-count").read_text(), "1")
            final = activity.authenticated_state(activity.adapter_journal_path(self.work, "private-lifecycle"), "adapter")
            self.assertEqual(final["phase"], "finalized")

    def test_caller_probe_and_stop_refuse_before_dispatch(self) -> None:
        manifest = self.parsed()
        absent = executable(self.root / "absent-caller", "#!/bin/sh\nexit 2\n")
        with self.assertRaisesRegex(activity.Refusal, "does not dispatch"):
            activity.run_activity(manifest, self.work, absent, absent, self.keygen, None, poll_only=False)
        self.assertFalse((self.work / "private").exists())

        state = RpcState()
        with rpc_server(state) as rpc_url:
            write_json(self.manifest, manifest_value(self.scenario, rpc_url=rpc_url))
            manifest = self.parsed()
            self.prepare_finalized_funding(manifest, state)
            caller = fake_caller(self.root / "stopped-successor")
            activity.stop(self.work, "test stop")
            with self.assertRaisesRegex(activity.Refusal, "STOP"):
                activity.run_activity(manifest, self.work, caller, caller, self.keygen, None, poll_only=False)
            self.assertFalse((self.work / "private" / "caller-count").exists())

    def test_poll_only_fresh_and_partial_states_never_dispatch(self) -> None:
        manifest = self.parsed()
        caller = fake_caller(self.root / "recovery-caller")
        self.assertEqual(
            activity.run_activity(manifest, self.work, caller, caller, self.keygen, None, poll_only=True),
            "no-pending-submissions",
        )
        self.assertFalse((self.work / "private").exists())

        state = RpcState()
        with rpc_server(state) as rpc_url:
            write_json(self.manifest, manifest_value(self.scenario, rpc_url=rpc_url))
            manifest = self.parsed()
            second = dataclasses.replace(
                manifest.adapters[0],
                adapter_id="later-undispatched",
                completion=dataclasses.replace(
                    manifest.adapters[0].completion,
                    path="{{work}}/receipts/later-undispatched.json",
                ),
            )
            partial_manifest = dataclasses.replace(manifest, adapters=(manifest.adapters[0], second))
            self.prepare_finalized_funding(partial_manifest, state)
            private, public = activity.load_wallet_indexes(partial_manifest, self.work, self.keygen)
            private_rows = {row["id"]: row for row in private["wallets"]}
            public_rows = {row["id"]: row for row in public["wallets"]}
            argv, completion = activity.expanded_adapter(
                partial_manifest.adapters[0], partial_manifest, self.work, private_rows, public_rows
            )
            journal = activity.new_adapter_journal(
                partial_manifest,
                partial_manifest.adapters[0],
                digest(caller),
                argv,
                completion,
                None,
            )
            journal["phase"] = "dispatching"
            activity.atomic_write_json(activity.adapter_journal_path(self.work, "private-lifecycle"), journal)
            completion.parent.mkdir(parents=True, exist_ok=True)
            write_json(completion, {"schema": "dclutch-local-private-validator-lifecycle-v1", "completed": True, "signature": ACTIVITY_SIGNATURE})
            state.transactions[ACTIVITY_SIGNATURE] = activity_transaction()
            state.signatures = [ACTIVITY_SIGNATURE, SIGNATURE]
            recovery = activity.run_activity(
                partial_manifest, self.work, caller, caller, self.keygen, None, poll_only=True
            )
            self.assertEqual(recovery, "partial-recovery")
            self.assertFalse(activity.adapter_journal_path(self.work, "later-undispatched").exists())
            self.assertFalse((self.work / "private" / "caller-count").exists())

    def test_poll_only_recovers_ambiguous_funding_without_any_key_path(self) -> None:
        state = RpcState()
        with rpc_server(state) as rpc_url:
            write_json(self.manifest, manifest_value(self.scenario, rpc_url=rpc_url))
            manifest = self.parsed()
            caller = fake_caller(self.root / "funding-recovery-caller")
            journal = activity.new_funding_journal(manifest, "alice", WALLET, FUNDER, 10_000, None)
            journal["phase"] = "dispatching"
            activity.atomic_write_json(activity.funding_journal_path(self.work, "alice"), journal)
            self.assertEqual(
                activity.run_activity(manifest, self.work, caller, caller, None, None, poll_only=True),
                "pending-funding",
            )
            self.assertFalse((self.work / "private").exists())
            state.transactions[SIGNATURE] = funding_transaction(memo=journal["memo"])
            state.signatures = [SIGNATURE]
            self.assertEqual(
                activity.run_activity(manifest, self.work, caller, caller, None, None, poll_only=True),
                "funding-finalized",
            )
            final = activity.authenticated_state(activity.funding_journal_path(self.work, "alice"), "funding")
            self.assertEqual(final["phase"], "finalized")
            self.assertFalse((self.work / "private").exists())

    def test_substituted_completion_schema_and_transaction_signature_refuse(self) -> None:
        state = RpcState()
        with rpc_server(state) as rpc_url:
            write_json(self.manifest, manifest_value(self.scenario, rpc_url=rpc_url))
            manifest = self.parsed()
            self.prepare_finalized_funding(manifest, state)
            caller = fake_caller(self.root / "successor", schema="legacy-local-v2")
            with self.assertRaisesRegex(activity.Refusal, "completion schema"):
                activity.run_activity(manifest, self.work, caller, caller, self.keygen, None, poll_only=False)

        # Use a fresh run because the hostile dispatch is permanently poll-only.
        self.tearDown()
        self.setUp()
        state = RpcState()
        with rpc_server(state) as rpc_url:
            write_json(self.manifest, manifest_value(self.scenario, rpc_url=rpc_url))
            manifest = self.parsed()
            self.prepare_finalized_funding(manifest, state)
            caller = fake_caller(self.root / "successor")
            state.transactions[ACTIVITY_SIGNATURE] = activity_transaction(embedded_signature=SUBSTITUTED_SIGNATURE)
            with self.assertRaisesRegex(activity.Refusal, "substitutes activity signature"):
                activity.run_activity(manifest, self.work, caller, caller, self.keygen, None, poll_only=False)

    def test_campaign_completion_binds_full_transaction_list_and_failed_fees(self) -> None:
        checked_release = self.root / "checked-release.json"
        market = self.root / "market.json"
        write_json(checked_release, {"checked": True})
        write_json(market, {"market": "bound"})
        manifest = self.parsed()
        completion = activity.CompletionSpec(
            path="{{work}}/receipts/campaign.json",
            schema=activity.CAMPAIGN_REPORT_SCHEMA,
            signature_pointers=(),
            transaction_list_pointer="/execution/transactions",
            required_transaction_labels=("found the market",),
            required_values={},
        )
        adapter = dataclasses.replace(
            manifest.adapters[0],
            argv=(
                "campaign",
                "--rpc-url",
                "{{rpc}}",
                "--plan",
                "{{input.checked-release}}",
                "--market",
                "{{input.market}}",
                "--evidence",
                "{{work}}/receipts/campaign.json",
                "--execute",
            ),
            completion=completion,
        )
        manifest = dataclasses.replace(
            manifest,
            inputs={"checked-release": checked_release, "market": market},
            adapters=(adapter,),
        )
        argv, completion_path = activity.expanded_adapter(
            adapter,
            manifest,
            self.work,
            {"alice": {"keypair": str(self.work / "private/wallets/alice.json")}},
            {"alice": {"address": WALLET}},
        )
        self.assertEqual(argv[argv.index("--market") + 1], str(market))
        completion_path.parent.mkdir(parents=True)
        write_json(
            completion_path,
            {
                "schema": activity.CAMPAIGN_REPORT_SCHEMA,
                "cluster": "loopback",
                "mode": "execute",
                "plan_sha256": digest(checked_release),
                "market_sha256": digest(market),
                "execution": {
                    "completed": True,
                    "transactions": [
                        {"label": "hostile refusal", "signature": FAILED_ACTIVITY_SIGNATURE},
                        {"label": "found the market", "signature": ACTIVITY_SIGNATURE},
                    ],
                },
            },
        )
        failed = activity_transaction(FAILED_ACTIVITY_SIGNATURE)
        failed["meta"]["err"] = {"InstructionError": [0, "Custom"]}  # type: ignore[index]
        state = RpcState()
        state.transactions[FAILED_ACTIVITY_SIGNATURE] = failed
        state.transactions[ACTIVITY_SIGNATURE] = activity_transaction()
        with rpc_server(state) as rpc_url:
            observed = activity.inspect_completion(
                dataclasses.replace(manifest, rpc_url=rpc_url),
                adapter,
                completion_path,
                activity.Rpc(rpc_url),
                {"alice": WALLET},
            )
        assert observed is not None
        _, signatures, transactions = observed
        self.assertEqual(signatures, [FAILED_ACTIVITY_SIGNATURE, ACTIVITY_SIGNATURE])
        self.assertEqual([row["succeeded"] for row in transactions], [False, True])
        self.assertEqual(sum(int(row["feeLamports"]) for row in transactions), 2_000)

        substituted = json.loads(completion_path.read_text())
        substituted["market_sha256"] = "7" * 64
        write_json(completion_path, substituted)
        with rpc_server(state) as rpc_url:
            with self.assertRaisesRegex(activity.Refusal, "release/Market"):
                activity.inspect_completion(
                    dataclasses.replace(manifest, rpc_url=rpc_url),
                    adapter,
                    completion_path,
                    activity.Rpc(rpc_url),
                    {"alice": WALLET},
                )

    def test_campaign_completion_refuses_omitted_required_stage_and_substituted_input(self) -> None:
        parsed = self.parsed()
        with self.assertRaisesRegex(activity.Refusal, "exact shape"):
            activity.parse_completion(
                {
                    "path": "/tmp/report.json",
                    "schema": activity.CAMPAIGN_REPORT_SCHEMA,
                    "signaturePointers": [],
                    "transactionListPointer": "/transactions",
                    "requiredTransactionLabels": ["DCLTCFQ1"],
                    "requiredValues": {},
                },
                "founding",
            )
        checked_release = self.root / "checked-release.json"
        market = self.root / "market.json"
        write_json(checked_release, {})
        write_json(market, {})
        adapter = dataclasses.replace(
            parsed.adapters[0],
            argv=(
                "campaign",
                "--plan",
                "{{input.checked-release}}",
                "--market",
                "{{input.checked-release}}",
                "--evidence",
                "{{work}}/campaign.json",
                "--execute",
            ),
            completion=activity.CompletionSpec(
                path="{{work}}/campaign.json",
                schema=activity.CAMPAIGN_REPORT_SCHEMA,
                signature_pointers=(),
                transaction_list_pointer="/execution/transactions",
                required_transaction_labels=("DCLTCFQ1", "DCLTPCB2", "DCLTGMF2"),
                required_values={},
            ),
        )
        manifest = dataclasses.replace(
            parsed,
            inputs={"checked-release": checked_release, "market": market},
            adapters=(adapter,),
        )
        with self.assertRaisesRegex(activity.Refusal, "substitutes --market"):
            activity.expanded_adapter(
                adapter,
                manifest,
                self.work,
                {"alice": {"keypair": str(self.work / "private/wallets/alice.json")}},
                {"alice": {"address": WALLET}},
            )

    def test_expired_capability_allows_only_original_journal_recovery(self) -> None:
        manifest = self.parsed()
        state = RpcState()
        state.genesis = activity.DEVNET_GENESIS_HASH
        with rpc_server(state) as rpc_url:
            devnet = dataclasses.replace(
                manifest,
                scenario=dataclasses.replace(manifest.scenario, cluster_target="devnet"),
                rpc_url=rpc_url,
                devnet_genesis_hash=activity.DEVNET_GENESIS_HASH,
            )
            expired = self.root / "expired-authorization.json"
            not_before = dt.datetime.now(dt.timezone.utc) - dt.timedelta(hours=3)
            expires = not_before + dt.timedelta(hours=1)
            auth_value = {
                "schema": activity.AUTHORIZATION_SCHEMA,
                "manifestSha256": devnet.sha256,
                "scenarioSha256": devnet.scenario.sha256,
                "devnetGenesisHash": activity.DEVNET_GENESIS_HASH,
                "marketRef": devnet.scenario.market_ref,
                "notBefore": not_before.isoformat().replace("+00:00", "Z"),
                "expiresAt": expires.isoformat().replace("+00:00", "Z"),
                "authorization": "authorize-one-devnet-activity-run",
            }
            write_json(expired, auth_value)
            with self.assertRaisesRegex(activity.Refusal, "outside its current window"):
                activity.require_live_authorization(devnet, expired)
            authorization_sha = activity.require_live_authorization(devnet, expired, allow_expired=True)
            self.assertEqual(authorization_sha, digest(expired))

            activity.prepare_wallets(devnet, self.work, self.keygen)
            funding = activity.new_funding_journal(devnet, "alice", WALLET, FUNDER, 10_000, authorization_sha)
            funding_tx = funding_transaction(memo=funding["memo"])
            state.transactions[SIGNATURE] = funding_tx
            activity.atomic_write_json(
                activity.funding_journal_path(self.work, "alice"),
                activity.verify_funding_transaction(funding_tx, funding, SIGNATURE),
            )
            caller = fake_caller(self.root / "capability-caller")
            private, public = activity.load_wallet_indexes(devnet, self.work, self.keygen)
            private_rows = {row["id"]: row for row in private["wallets"]}
            public_rows = {row["id"]: row for row in public["wallets"]}
            adapter = devnet.adapters[0]
            argv, completion = activity.expanded_adapter(adapter, devnet, self.work, private_rows, public_rows)
            journal = activity.new_adapter_journal(devnet, adapter, digest(caller), argv, completion, authorization_sha)
            journal["phase"] = "dispatching"
            activity.atomic_write_json(activity.adapter_journal_path(self.work, adapter.adapter_id), journal)
            completion.parent.mkdir(parents=True, exist_ok=True)
            write_json(completion, {"schema": "dclutch-local-private-validator-lifecycle-v1", "completed": True, "signature": ACTIVITY_SIGNATURE})
            state.transactions[ACTIVITY_SIGNATURE] = activity_transaction()
            state.signatures = [ACTIVITY_SIGNATURE, SIGNATURE]

            activity.run_activity(devnet, self.work, caller, caller, self.keygen, expired, poll_only=True)
            saved = activity.authenticated_state(activity.adapter_journal_path(self.work, adapter.adapter_id), "adapter")
            self.assertEqual(saved["phase"], "finalized")
            self.assertFalse((self.work / "private" / "caller-count").exists())
            with self.assertRaisesRegex(activity.Refusal, "outside its current window"):
                activity.run_activity(devnet, self.work, caller, caller, self.keygen, expired, poll_only=False)
            self.assertFalse((self.work / "private" / "caller-count").exists())

            substituted = self.root / "substituted-authorization.json"
            substituted.write_text(json.dumps(auth_value, indent=4) + "\n", encoding="utf-8")
            with self.assertRaisesRegex(activity.Refusal, "exact finalized funding|another live authorization"):
                activity.run_activity(devnet, self.work, caller, caller, self.keygen, substituted, poll_only=True)
            self.assertFalse((self.work / "private" / "caller-count").exists())


if __name__ == "__main__":
    unittest.main()
