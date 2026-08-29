#!/usr/bin/env python3

from __future__ import annotations

import base64
import contextlib
import hashlib
import http.server
import importlib.util
import json
from pathlib import Path
import shutil
import sys
import tempfile
import threading
import time
import unittest
from urllib import error as urlerror
from urllib import request as urlrequest


ROOT = Path(__file__).resolve().parent
MODULE_PATH = ROOT / "lifecycle_chaos.py"
SPEC = importlib.util.spec_from_file_location("dclutch_lifecycle_chaos", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class _RpcHandler(http.server.BaseHTTPRequestHandler):
    server: "_RpcUpstream"

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def do_POST(self) -> None:  # noqa: N802 - stdlib hook
        width = int(self.headers["content-length"])
        call = json.loads(self.rfile.read(width))
        method = call["method"]
        with self.server.lock:
            self.server.methods.append(method)
        if method == "sendTransaction":
            _wire, signature, _blockhash = MODULE.frozen_transaction(call)
            if self.server.block_height > self.server.last_valid_block_height:
                response: dict[str, object] = {
                    "jsonrpc": "2.0",
                    "id": call["id"],
                    "error": {
                        "code": -32002,
                        "message": "Transaction simulation failed: Blockhash not found",
                        "data": {"err": "BlockhashNotFound"},
                    },
                }
            else:
                with self.server.lock:
                    self.server.accepted_signatures.add(signature)
                response = {
                    "jsonrpc": "2.0",
                    "id": call["id"],
                    "result": signature,
                }
        elif method == "getSignatureStatuses":
            values = call.get("params", [[]])[0]
            with self.server.lock:
                accepted = set(self.server.accepted_signatures)
            response = {
                "jsonrpc": "2.0",
                "id": call["id"],
                "result": {
                    "context": {"slot": 9},
                    "value": [
                        (
                            {"confirmationStatus": "finalized"}
                            if signature in accepted
                            else None
                        )
                        for signature in values
                    ],
                },
            }
        elif method == "getLatestBlockhash":
            response = {
                "jsonrpc": "2.0",
                "id": call["id"],
                "result": {
                    "context": {"slot": 1},
                    "value": {
                        "blockhash": MODULE.base58_encode(b"\x07" * 32),
                        "lastValidBlockHeight": self.server.last_valid_block_height,
                    },
                },
            }
        elif method == "getBlockHeight":
            with self.server.lock:
                self.server.block_height += 1
                height = self.server.block_height
            response = {"jsonrpc": "2.0", "id": call["id"], "result": height}
        else:
            response = {"jsonrpc": "2.0", "id": call["id"], "result": None}
        body = json.dumps(response).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


class _RpcUpstream(http.server.ThreadingHTTPServer):
    allow_reuse_address = False
    daemon_threads = True

    def __init__(self) -> None:
        super().__init__(("127.0.0.1", 0), _RpcHandler)
        self.methods: list[str] = []
        self.lock = threading.Lock()
        self.block_height = 0
        self.last_valid_block_height = 2
        self.accepted_signatures: set[str] = set()


@contextlib.contextmanager
def upstream() -> object:
    server = _RpcUpstream()
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        host, port = server.server_address
        yield server, f"http://{host}:{port}"
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


def write_spec(root: Path, rpc_url: str) -> Path:
    fixture = root / "fixture"
    shutil.copytree(ROOT / "fixture", fixture)
    fake = ROOT / "fake_session.py"
    python = str(Path(sys.executable).resolve(strict=True))
    spec = {
        "schema": MODULE.SPEC_SCHEMA,
        "cluster": "owned-loopback",
        "sourceRevision": "a" * 40,
        "command": {
            "argv": [python, str(fake), "session", "{caseWork}", "{rpcUrl}"],
            "cwd": str(ROOT),
            "environment": {},
        },
        "observer": {
            "argv": [python, str(fake), "observe", "{caseWork}", "{rpcUrl}"],
            "cwd": str(ROOT),
            "environment": {},
        },
        "teardown": {
            "argv": [python, str(fake), "teardown", "{caseWork}", "{rpcUrl}"],
            "cwd": str(ROOT),
            "environment": {},
        },
        "session": "session.json",
        "journalDir": "journals",
        "sessionSchema": "dclutch-fake-owned-loopback-lifecycle-v1",
        "rpcUpstream": rpc_url,
        "evidence": "evidence.json",
        "replacementEvidence": "replacement-evidence.json",
        "caseTimeoutSeconds": 12,
        "journalTimeoutSeconds": 4,
        "boundaries": list(MODULE.BOUNDARIES),
    }
    path = root / "spec.json"
    path.write_text(json.dumps(spec, indent=2, sort_keys=True) + "\n")
    return path


class LifecycleChaosTests(unittest.TestCase):
    def test_full_campaign_kills_every_boundary_and_runs_all_hostiles(self) -> None:
        with tempfile.TemporaryDirectory() as root_text, upstream() as (
            server,
            rpc_url,
        ):
            root = Path(root_text)
            spec_path = write_spec(root, rpc_url)
            spec = MODULE.parse_spec(spec_path)
            work = root / "work"
            summary = MODULE.run(spec, work, MODULE.ALL_CASES)
            self.assertEqual(summary["status"], "passed")
            self.assertEqual(summary["passCount"], len(MODULE.ALL_CASES))
            self.assertEqual(summary["caseCount"], len(MODULE.ALL_CASES))
            self.assertEqual(
                [row["case"] for row in summary["cases"]], list(MODULE.ALL_CASES)
            )
            for boundary in MODULE.BOUNDARIES:
                row = next(
                    row for row in summary["cases"] if row["case"] == f"kill-{boundary}"
                )
                self.assertEqual(
                    row["killedIntentSha256"],
                    MODULE.sha256_bytes(f"fake-intent:{boundary}".encode()),
                )
                self.assertEqual(
                    row["recoveryPolicy"],
                    "dispatching-poll-then-identical-send",
                )
                self.assertEqual(len(row["killedSignedPacketSha256"]), 64)
                self.assertEqual(len(row["killedDispatchingStateSha256"]), 64)
                self.assertTrue(row["killedSignature"])
                self.assertTrue(
                    (work / "cases" / f"kill-{boundary}" / "attempt-1-killed").is_dir()
                )
                self.assertTrue(
                    (work / "cases" / f"kill-{boundary}" / "attempt-2-resumed").is_dir()
                )
                trace = MODULE.read_unique_json(
                    work / "cases" / f"kill-{boundary}" / "RPC_TRACE.json",
                    "kill trace",
                )["rows"]
                client = [row for row in trace if row["source"] == "client"]
                send_index = next(
                    index
                    for index, item in enumerate(client)
                    if item["method"] == "sendTransaction" and item["stage"] == boundary
                )
                self.assertTrue(
                    any(
                        item["method"] == "getSignatureStatuses"
                        for item in client[:send_index]
                    )
                )
            for case in MODULE.ALL_CASES:
                self.assertTrue(
                    (work / "cases" / case / "teardown" / "receipt.json").is_file()
                )
            refusal_rows = [row for row in summary["cases"] if row["expectedRefusal"]]
            self.assertEqual(
                {row["case"] for row in refusal_rows},
                {*MODULE.REFUSAL_CASES, "blockhash-expiry"},
            )
            baseline = next(
                row for row in summary["cases"] if row["case"] == "baseline"
            )
            for row in summary["cases"]:
                if not row["expectedRefusal"]:
                    self.assertEqual(row["snapshotSha256"], baseline["snapshotSha256"])
            timeout_trace = MODULE.read_unique_json(
                work / "cases" / "rpc-timeout" / "RPC_TRACE.json", "timeout trace"
            )
            self.assertEqual(
                [
                    row["method"]
                    for row in timeout_trace["rows"]
                    if row["source"] == "client"
                ],
                ["getLatestBlockhash", "sendTransaction", "getSignatureStatuses"],
            )
            duplicate_trace = MODULE.read_unique_json(
                work / "cases" / "duplicate-send" / "RPC_TRACE.json", "duplicate trace"
            )
            self.assertEqual(
                len(
                    [
                        row
                        for row in duplicate_trace["rows"]
                        if row["source"] == "client"
                    ]
                ),
                2,
            )
            self.assertEqual(
                len(
                    [
                        row
                        for row in duplicate_trace["rows"]
                        if row["source"] == "injected-forward"
                    ]
                ),
                2,
            )
            self.assertGreaterEqual(server.methods.count("sendTransaction"), 3)
            expiry_trace = MODULE.read_unique_json(
                work / "cases" / "blockhash-expiry" / "RPC_TRACE.json", "expiry trace"
            )
            expired = next(
                row
                for row in expiry_trace["rows"]
                if row["source"] == "injected-expired-forward"
            )
            self.assertGreater(
                expired["observedBlockHeight"], expired["lastValidBlockHeight"]
            )
            signature_digest = hashlib.sha256(
                b"fake-signature:blockhash-expiry:hot"
            ).digest()
            self.assertEqual(
                expired["transactionSignature"],
                MODULE.base58_encode(signature_digest + signature_digest),
            )
            late = work / "cases" / "late-child-refusal"
            MODULE.write_json_new(
                late / "journals" / "retire.json",
                {
                    "schema": MODULE.STAGE_JOURNAL_SCHEMA,
                    "stage": "retire",
                    "phase": "planned",
                    "intentSha256": MODULE.sha256_bytes(b"hostile-retire"),
                },
            )
            with self.assertRaisesRegex(MODULE.Refusal, "prefix disagreed"):
                MODULE.validate_refusal_journals(spec, "late-child-refusal", late)

    def test_snapshot_refuses_digest_total_and_order_substitutions(self) -> None:
        canonical = json.loads((ROOT / "fixture" / "state.json").read_text())
        MODULE.validate_snapshot(json.dumps(canonical).encode(), "canonical")
        hostile = json.loads(json.dumps(canonical))
        hostile["accounts"][0]["dataSha256"] = "0" * 64
        with self.assertRaisesRegex(MODULE.Refusal, "digest"):
            MODULE.validate_snapshot(json.dumps(hostile).encode(), "hostile")
        hostile = json.loads(json.dumps(canonical))
        hostile["totals"]["lamports"] += 1
        with self.assertRaisesRegex(MODULE.Refusal, "totals"):
            MODULE.validate_snapshot(json.dumps(hostile).encode(), "hostile")
        second = json.loads(json.dumps(canonical["accounts"][0]))
        second["address"] = "0-first"
        hostile = json.loads(json.dumps(canonical))
        hostile["accounts"].append(second)
        hostile["totals"] = {"accountCount": 2, "lamports": 200}
        with self.assertRaisesRegex(MODULE.Refusal, "order"):
            MODULE.validate_snapshot(json.dumps(hostile).encode(), "hostile")

    def test_spec_refuses_external_rpc_reordered_boundaries_and_unknown_fields(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as root_text, upstream() as (
            _server,
            rpc_url,
        ):
            root = Path(root_text)
            path = write_spec(root, rpc_url)
            value = json.loads(path.read_text())
            value["rpcUpstream"] = "https://api.mainnet-beta.solana.com"
            path.write_text(json.dumps(value))
            with self.assertRaisesRegex(MODULE.Refusal, "loopback"):
                MODULE.parse_spec(path)
            value["rpcUpstream"] = rpc_url
            value["boundaries"][0], value["boundaries"][1] = (
                value["boundaries"][1],
                value["boundaries"][0],
            )
            path.write_text(json.dumps(value))
            with self.assertRaisesRegex(MODULE.Refusal, "ordered lifecycle"):
                MODULE.parse_spec(path)
            value["boundaries"] = list(MODULE.BOUNDARIES)
            value["surprise"] = True
            path.write_text(json.dumps(value))
            with self.assertRaisesRegex(MODULE.Refusal, "unknown"):
                MODULE.parse_spec(path)

    def test_poll_only_guard_refuses_same_intent_second_client_send(self) -> None:
        trace = [
            {
                "source": "client",
                "method": "sendTransaction",
                "stage": "hot",
                "intentSha256": "a" * 64,
            },
            {
                "source": "client",
                "method": "sendTransaction",
                "stage": "hot",
                "intentSha256": "a" * 64,
            },
        ]
        with self.assertRaisesRegex(MODULE.Refusal, "more than once"):
            MODULE.validate_poll_only(trace, "hostile")

        wrong_signature = [
            {
                "source": "client",
                "method": "sendTransaction",
                "stage": "hot",
                "intentSha256": "a" * 64,
            },
            {
                "source": "injected-forward",
                "method": "sendTransaction",
                "stage": "hot",
                "intentSha256": "a" * 64,
                "transactionSignature": "expected-signature",
            },
            {
                "source": "client",
                "method": "getSignatureStatuses",
                "signatureStatusValues": ["substituted-signature"],
            },
        ]
        with self.assertRaisesRegex(MODULE.Refusal, "another signature"):
            MODULE.validate_poll_only(wrong_signature, "rpc-timeout")

        selected_after_earlier_stage = [
            {
                "source": "client",
                "method": "sendTransaction",
                "stage": "founding",
                "intentSha256": "1" * 64,
            },
            {
                "source": "client",
                "method": "getSignatureStatuses",
                "stage": "participant",
                "intentSha256": "2" * 64,
                "signatureStatusValues": ["participant-signature"],
            },
            {
                "source": "client",
                "method": "sendTransaction",
                "stage": "participant",
                "intentSha256": "2" * 64,
            },
            {
                "source": "injected-forward",
                "method": "sendTransaction",
                "stage": "participant",
                "intentSha256": "2" * 64,
                "transactionSignature": "participant-signature",
            },
        ]
        MODULE.validate_poll_only(selected_after_earlier_stage, "kill-participant")

    def test_journal_projection_refuses_non_lowercase_digest(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            path = Path(root_text) / "hot.json"
            path.write_text(
                json.dumps(
                    {
                        "schema": MODULE.STAGE_JOURNAL_SCHEMA,
                        "stage": "hot",
                        "phase": "dispatching",
                        "intentSha256": "A" * 64,
                    }
                )
            )
            with self.assertRaisesRegex(MODULE.Refusal, "malformed"):
                MODULE.stable_journal(path, "hot")

    def test_kill_boundary_binds_packet_signature_state_and_owner(self) -> None:
        intent = "1" * 64
        canonical = {
            "schema": MODULE.CONTROL_SCHEMA,
            "state": "fault-armed",
            "fault": "kill-hot",
            "stage": "hot",
            "phase": "dispatching",
            "intentSha256": intent,
            "signedPacketSha256": "2" * 64,
            "signature": MODULE.base58_encode(b"\x03" * 64),
            "dispatchingStateSha256": "4" * 64,
        }
        facts = MODULE.authenticate_kill_boundary(canonical, "kill-hot", "hot", intent)
        self.assertEqual(facts.signed_packet_sha256, "2" * 64)
        self.assertEqual(facts.dispatching_state_sha256, "4" * 64)
        for field, replacement, refusal in (
            ("fault", "kill-seal", "owner identity"),
            ("signedPacketSha256", "A" * 64, "SHA-256"),
            ("signature", "not-base58-0", "signature"),
            ("dispatchingStateSha256", "5" * 63, "SHA-256"),
        ):
            hostile = dict(canonical)
            hostile[field] = replacement
            with self.assertRaisesRegex(MODULE.Refusal, refusal):
                MODULE.authenticate_kill_boundary(hostile, "kill-hot", "hot", intent)

    def test_wire_parser_binds_legacy_and_v0_signature_and_blockhash(self) -> None:
        signature = bytes(range(64))
        blockhash = b"\x08" * 32
        for prefix in (b"", b"\x80"):
            message = prefix + b"\x01\x00\x00\x01" + b"\0" * 32 + blockhash + b"\x00"
            wire = b"\x01" + signature + message
            call = {
                "params": [
                    base64.b64encode(wire).decode(),
                    {"encoding": "base64", "maxRetries": 0},
                ]
            }
            digest, actual_signature, actual_blockhash = MODULE.frozen_transaction(call)
            self.assertEqual(digest, MODULE.sha256_bytes(wire))
            self.assertEqual(actual_signature, MODULE.base58_encode(signature))
            self.assertEqual(actual_blockhash, MODULE.base58_encode(blockhash))

    def test_proxy_refuses_send_outside_durable_dispatching(self) -> None:
        with tempfile.TemporaryDirectory() as root_text, upstream() as (
            _server,
            rpc_url,
        ):
            root = Path(root_text)
            journal = root / "hot.json"
            journal.write_text(
                json.dumps(
                    {
                        "schema": MODULE.STAGE_JOURNAL_SCHEMA,
                        "stage": "hot",
                        "phase": "prepared",
                        "intentSha256": "a" * 64,
                    }
                )
            )
            proxy = MODULE.RpcFaultProxy(
                rpc_url,
                MODULE.RpcRule("duplicate-send", "sendTransaction", "hot"),
                root,
                2,
            )
            wire = b"\x01" + b"\0" * 64 + b"\x01\x00\x00\x01" + b"\0" * 65
            call = json.dumps(
                {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "sendTransaction",
                    "params": [base64.b64encode(wire).decode(), {"encoding": "base64"}],
                }
            ).encode()
            with proxy:
                request = urlrequest.Request(
                    proxy.url,
                    data=call,
                    headers={"content-type": "application/json"},
                )
                with self.assertRaises(urlerror.HTTPError) as raised:
                    urlrequest.urlopen(request, timeout=1)  # noqa: S310 test loopback
                raised.exception.close()
                deadline = time.monotonic() + 1
                while proxy.state.failure is None and time.monotonic() < deadline:
                    time.sleep(0.01)
                self.assertIn("outside durable Dispatching", proxy.state.failure)


if __name__ == "__main__":
    unittest.main()
