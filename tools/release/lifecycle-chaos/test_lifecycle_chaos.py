#!/usr/bin/env python3

from __future__ import annotations

import base64
import contextlib
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
                response = {
                    "jsonrpc": "2.0",
                    "id": call["id"],
                    "result": MODULE.base58_encode(b"\0" * 64),
                }
        elif method == "getSignatureStatuses":
            response = {
                "jsonrpc": "2.0",
                "id": call["id"],
                "result": {"context": {"slot": 9}, "value": [{"confirmationStatus": "finalized"}]},
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
        with tempfile.TemporaryDirectory() as root_text, upstream() as (server, rpc_url):
            root = Path(root_text)
            spec_path = write_spec(root, rpc_url)
            spec = MODULE.parse_spec(spec_path)
            work = root / "work"
            summary = MODULE.run(spec, work, MODULE.ALL_CASES)
            self.assertEqual(summary["status"], "passed")
            self.assertEqual(summary["passCount"], len(MODULE.ALL_CASES))
            self.assertEqual(summary["caseCount"], len(MODULE.ALL_CASES))
            self.assertEqual([row["case"] for row in summary["cases"]], list(MODULE.ALL_CASES))
            for boundary in MODULE.BOUNDARIES:
                row = next(row for row in summary["cases"] if row["case"] == f"kill-{boundary}")
                self.assertEqual(row["killedIntentSha256"], MODULE.sha256_bytes(f"fake-intent:{boundary}".encode()))
                self.assertTrue((work / "cases" / f"kill-{boundary}" / "attempt-1-killed").is_dir())
                self.assertTrue((work / "cases" / f"kill-{boundary}" / "attempt-2-resumed").is_dir())
            for case in MODULE.ALL_CASES:
                self.assertTrue((work / "cases" / case / "teardown" / "receipt.json").is_file())
            refusal_rows = [row for row in summary["cases"] if row["expectedRefusal"]]
            self.assertEqual(
                {row["case"] for row in refusal_rows},
                {*MODULE.REFUSAL_CASES, "blockhash-expiry"},
            )
            baseline = next(row for row in summary["cases"] if row["case"] == "baseline")
            for row in summary["cases"]:
                if not row["expectedRefusal"]:
                    self.assertEqual(row["snapshotSha256"], baseline["snapshotSha256"])
            timeout_trace = MODULE.read_unique_json(
                work / "cases" / "rpc-timeout" / "RPC_TRACE.json", "timeout trace"
            )
            self.assertEqual(
                [row["method"] for row in timeout_trace["rows"] if row["source"] == "client"],
                ["getLatestBlockhash", "sendTransaction", "getSignatureStatuses"],
            )
            duplicate_trace = MODULE.read_unique_json(
                work / "cases" / "duplicate-send" / "RPC_TRACE.json", "duplicate trace"
            )
            self.assertEqual(
                len([row for row in duplicate_trace["rows"] if row["source"] == "client"]),
                2,
            )
            self.assertEqual(
                len([row for row in duplicate_trace["rows"] if row["source"] == "injected-forward"]),
                2,
            )
            self.assertGreaterEqual(server.methods.count("sendTransaction"), 3)
            expiry_trace = MODULE.read_unique_json(
                work / "cases" / "blockhash-expiry" / "RPC_TRACE.json", "expiry trace"
            )
            expired = next(
                row for row in expiry_trace["rows"] if row["source"] == "injected-expired-forward"
            )
            self.assertGreater(expired["observedBlockHeight"], expired["lastValidBlockHeight"])
            self.assertEqual(expired["transactionSignature"], MODULE.base58_encode(b"\0" * 64))

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

    def test_spec_refuses_external_rpc_reordered_boundaries_and_unknown_fields(self) -> None:
        with tempfile.TemporaryDirectory() as root_text, upstream() as (_server, rpc_url):
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
        with self.assertRaisesRegex(MODULE.Refusal, "instead of polling"):
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

    def test_proxy_refuses_send_before_durable_submitted(self) -> None:
        with tempfile.TemporaryDirectory() as root_text, upstream() as (_server, rpc_url):
            root = Path(root_text)
            journal = root / "hot.json"
            journal.write_text(
                json.dumps(
                    {
                        "schema": MODULE.STAGE_JOURNAL_SCHEMA,
                        "stage": "hot",
                        "phase": "signed-not-submitted",
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
                self.assertIn("before durable Submitted", proxy.state.failure)


if __name__ == "__main__":
    unittest.main()
