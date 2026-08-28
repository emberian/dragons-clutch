#!/usr/bin/env python3
"""Focused hostile tests for the activity harness.

Every filesystem and RPC fixture lives in one temporary directory.  The fake
keygen deliberately writes recognizable secret bytes; tests assert they never
appear in the public ledger or captured output.
"""

from __future__ import annotations

from contextlib import contextmanager, redirect_stderr, redirect_stdout
import dataclasses
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
WALLET = "2" * 32
SIGNATURE = "3" * 64
SECRET_MARKER = "THIS_IS_TEST_SECRET_KEY_MATERIAL"


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n", encoding="utf-8")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def limits(target: str = "owned-loopback") -> dict[str, int]:
    return {
        "maxConcurrency": 1,
        "minDispatchIntervalMs": 0 if target == "owned-loopback" else 1000,
        "maxTransactions": 20,
        "pollIntervalMs": 250,
        "maxPolls": 10,
    }


def operation(identifier: str, kind: str, depends: list[str]) -> dict[str, object]:
    return {
        "id": identifier,
        "kind": kind,
        "wallets": ["alice"],
        "dependsOn": depends,
        "mutationExpected": True,
        "inputs": {},
        "expectedLamportDeltas": {"alice": "0"},
        "expectedTokenDeltas": [],
        "receiptRef": f"{identifier}-receipt",
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
        rows.append(operation(identifier, kind, parent))
        parent = [identifier]
    return {
        "schema": "dclutch-economic-activity-scenario-v1",
        "scenarioId": "test-lifecycle",
        "clusterTarget": target,
        "marketRef": "flagship",
        "wallets": [{"id": "alice", "roles": ["participant", "trader"], "fundingLamports": "10000"}],
        "operations": rows,
        "limits": limits(target),
    }


def manifest_value(scenario: Path, target: str = "owned-loopback", rpc_url: str = "http://127.0.0.1:18899/") -> dict[str, object]:
    return {
        "schema": activity.MANIFEST_SCHEMA,
        "scenario": {"path": str(scenario), "sha256": digest(scenario)},
        "target": {
            "kind": target,
            "rpcUrl": rpc_url,
            "devnetGenesisHash": activity.DEVNET_GENESIS_HASH if target == "devnet" else None,
        },
        "inputs": [],
        "adapters": [
            {
                "id": "private-lifecycle",
                "covers": ["found", "participant", "direct", "resolve", "redeem", "retire"],
                "caller": "successor",
                "argv": ["local-private-validator-lifecycle-v1", "--execute"],
                "dependsOn": [],
                "wallets": ["alice"],
                "mutation": True,
                "completion": {
                    "path": "{{work}}/receipts/private-lifecycle.json",
                    "schema": "dclutch-local-private-validator-lifecycle-v1",
                    "signaturePointers": [],
                    "requiredValues": {"/completed": True},
                },
            }
        ],
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


def funding_transaction(amount: int = 10_000, fee: int = 5_000, memo: str = "") -> dict[str, object]:
    return {
        "slot": 99,
        "transaction": {
            "message": {
                "accountKeys": [
                    {"pubkey": FUNDER, "signer": True, "writable": True},
                    {"pubkey": WALLET, "signer": False, "writable": True},
                    {"pubkey": "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr", "signer": False, "writable": False},
                ],
                "instructions": [
                    {"program": "system", "parsed": {"type": "transfer", "info": {"source": FUNDER, "destination": WALLET, "lamports": amount}}},
                    {"program": "spl-memo", "parsed": memo},
                ],
            }
        },
        "meta": {
            "err": None,
            "fee": fee,
            "preBalances": [100_000, 0, 0],
            "postBalances": [100_000 - amount - fee, amount, 0],
            "preTokenBalances": [],
            "postTokenBalances": [],
        },
    }


class RpcState:
    def __init__(self) -> None:
        self.genesis = "loopback-test-genesis"
        self.transactions: dict[str, dict[str, object]] = {}
        self.signatures: list[str] = []


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
                result = {"context": {"slot": 98}, "value": 0}
            elif method == "getTransaction":
                result = state.transactions.get(body["params"][0])
            elif method == "getSignaturesForAddress":
                result = [{"signature": signature, "err": None, "slot": 99} for signature in state.signatures]
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

    def test_cycle_and_preflight_only_direct_refuse(self) -> None:
        changed = scenario_value()
        changed["operations"][0]["dependsOn"] = ["retire"]  # type: ignore[index]
        write_json(self.scenario, changed)
        write_json(self.manifest, manifest_value(self.scenario))
        with self.assertRaisesRegex(activity.Refusal, "cycle"):
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

    def test_cleanup_requires_final_journal_and_removes_only_secret_keys(self) -> None:
        manifest = self.parsed()
        activity.prepare_wallets(manifest, self.work, self.keygen)
        journal = activity.new_funding_journal(manifest, "alice", WALLET, FUNDER, 10_000, None)
        activity.atomic_write_json(activity.funding_journal_path(self.work, "alice"), journal)
        with self.assertRaisesRegex(activity.Refusal, "not finalized"):
            activity.cleanup_keys(manifest, self.work, self.keygen, manifest.scenario.scenario_id)
        final = activity.verify_funding_transaction(funding_transaction(memo=journal["memo"]), journal, SIGNATURE)
        activity.atomic_write_json(activity.funding_journal_path(self.work, "alice"), final)
        activity.cleanup_keys(manifest, self.work, self.keygen, manifest.scenario.scenario_id)
        self.assertFalse((self.work / "private" / "wallets" / "alice.json").exists())
        self.assertTrue((self.work / "public" / "wallet-ledger.json").exists())
        self.assertTrue((self.work / "public" / "wallet-cleanup.json").exists())


if __name__ == "__main__":
    unittest.main()
