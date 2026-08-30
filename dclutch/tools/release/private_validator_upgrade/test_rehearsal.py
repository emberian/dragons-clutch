#!/usr/bin/env python3

from __future__ import annotations

import base64
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from tools.release.private_validator_upgrade import rehearsal as MODULE


def base58_encode(value: bytes) -> str:
    zeroes = len(value) - len(value.lstrip(b"\0"))
    number = int.from_bytes(value, "big")
    encoded = ""
    while number:
        number, remainder = divmod(number, 58)
        encoded = MODULE.BASE58_ALPHABET[remainder] + encoded
    return "1" * zeroes + encoded


def key(seed: int) -> str:
    return base58_encode(bytes([seed]) * 32)


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def write_spec(root: Path, *, mutate: object | None = None) -> Path:
    roles: list[dict[str, object]] = []
    for ordinal, (role, disposition) in enumerate(MODULE.CANONICAL_ROWS):
        old = f"old-{role}".encode()
        row: dict[str, object] = {
            "role": role,
            "disposition": disposition,
            "program": key(ordinal + 1),
            "programData": key(ordinal + 21),
            "currentSlot": 100 + ordinal,
            "currentPayloadBytes": len(old),
            "currentPayloadSha256": digest(old),
        }
        if disposition == "upgrade":
            payload = (f"new-{role}-" * 3).encode()
            path = (root / f"{role}.so").resolve()
            path.write_bytes(payload)
            row.update(
                {
                    "buffer": key(ordinal + 61),
                    "bufferAuthority": key(99),
                    "activationRecord": key(ordinal + 41),
                    "targetPayloadPath": str(path),
                    "targetPayloadBytes": len(payload),
                    "targetPayloadSha256": digest(payload),
                }
            )
        roles.append(row)
    value: dict[str, object] = {
        "schema": MODULE.SPEC_SCHEMA,
        "cluster": "owned-loopback",
        "sourceRevision": "a" * 40,
        "sourceTreeSha256": "b" * 64,
        "checkedReleaseGateSha256": "c" * 64,
        "rpcUrl": "http://127.0.0.1:18899",
        "retainedUpgradeAuthority": key(99),
        "chunkBytes": 9,
        "roles": roles,
    }
    if mutate is not None:
        mutate(value)
    path = (root / "spec.json").resolve()
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    return path


class FakeDriver:
    def __init__(self, spec: dict[str, object]):
        self.spec = spec
        self.roles: dict[str, dict[str, object]] = {}
        for row in spec["roles"]:
            self.roles[row["role"]] = {
                "program": row["program"],
                "programData": row["programData"],
                "slot": row["currentSlot"],
                "upgradeAuthority": spec["retainedUpgradeAuthority"],
                "livePayloadBytes": row["currentPayloadBytes"],
                "livePayloadSha256": row["currentPayloadSha256"],
                "programAccountSha256": digest(f"program:{row['role']}".encode()),
                "programDataAccountSha256": digest(f"programdata:{row['role']}:old".encode()),
            }
        self.buffers: dict[str, dict[str, object]] = {}
        self.prepared: dict[str, dict[str, object]] = {}
        self.finalized: dict[str, int] = {}
        self.activations: dict[str, dict[str, object]] = {}
        self.sends: dict[str, int] = {}
        self.block_height = 1
        self.slot = 1000
        self.response_loss_kind: str | None = None
        self.response_loss_used = False
        self.mutate_registry_on_core_activation = False

    def _prepare(self, intent: str) -> dict[str, object]:
        packet = bytes.fromhex(intent) + b"frozen-packet"
        signature = base58_encode(hashlib.sha512(packet).digest())
        result = {
            "intentSha256": intent,
            "packetBase64": base64.b64encode(packet).decode(),
            "packetSha256": digest(packet),
            "signature": signature,
            "recentBlockhash": key(111),
            "lastValidBlockHeight": 500,
        }
        self.prepared[signature] = result
        return result

    def _apply(self, action: dict[str, object], signature: str) -> None:
        kind = action["kind"]
        if kind == "buffer_create":
            self.buffers[action["buffer"]] = {
                "exists": True,
                "buffer": action["buffer"],
                "authority": action["bufferAuthority"],
                "capacity": action["capacity"],
                "uploadedBytes": 0,
                "uploadedPrefixSha256": digest(b""),
                "owner": key(120),
                "body": b"",
            }
        elif kind == "buffer_write":
            buffer = self.buffers[action["buffer"]]
            chunk = base64.b64decode(action["chunkBase64"], validate=True)
            if buffer["uploadedBytes"] != action["offset"] or digest(chunk) != action["chunkSha256"]:
                raise AssertionError("fake received noncanonical buffer write")
            buffer["body"] += chunk
            buffer["uploadedBytes"] = len(buffer["body"])
            buffer["uploadedPrefixSha256"] = digest(buffer["body"])
        elif kind == "loader_v3_upgrade":
            role = self.roles[action["role"]]
            body = self.buffers[action["buffer"]]["body"]
            if digest(body) != action["targetPayloadSha256"]:
                raise AssertionError("fake received Upgrade before complete buffer")
            self.slot += 1
            role["slot"] = self.slot
            role["livePayloadBytes"] = action["targetPayloadBytes"]
            role["livePayloadSha256"] = action["targetPayloadSha256"]
            role["programDataAccountSha256"] = digest(
                f"programdata:{action['role']}:{self.slot}:{action['targetPayloadSha256']}".encode()
            )
            del self.buffers[action["buffer"]]
        elif kind == "activate_checked_release":
            self.slot += 1
            self.activations[action["role"]] = {
                "role": action["role"],
                "program": action["program"],
                "programData": action["programData"],
                "deploymentSlot": action["deploymentSlot"],
                "livePayloadSha256": action["livePayloadSha256"],
                "activationRecord": action["activationRecord"],
                "activationRecordSha256": digest(MODULE.canonical_json(action)),
                "finalizedSlot": self.slot,
            }
            if self.mutate_registry_on_core_activation and action["role"] == "core":
                self.roles["registry"]["livePayloadSha256"] = "d" * 64
        else:
            raise AssertionError(f"unknown fake action {kind}")
        self.slot += 1
        if kind == "activate_checked_release":
            self.activations[action["role"]]["finalizedSlot"] = self.slot
        self.finalized[signature] = self.slot

    def call(self, operation: str, body: dict[str, object]) -> dict[str, object]:
        if operation == "observe_role":
            return copy.deepcopy(self.roles[body["role"]])
        if operation == "observe_buffer":
            value = self.buffers.get(body["buffer"])
            if value is None:
                return {"exists": False}
            result = copy.deepcopy(value)
            del result["body"]
            return result
        if operation == "prepare_transaction":
            return self._prepare(body["intentSha256"])
        if operation == "poll_transaction":
            slot = self.finalized.get(body["signature"])
            return (
                {"state": "absent", "slot": None, "error": None}
                if slot is None
                else {"state": "finalized", "slot": slot, "error": None}
            )
        if operation == "get_block_height":
            return {"blockHeight": self.block_height}
        if operation == "send_transaction":
            prepared = self.prepared[body["signature"]]
            if body["packetSha256"] != prepared["packetSha256"] or body["packetBase64"] != prepared["packetBase64"]:
                raise AssertionError("controller changed frozen packet")
            intent = body["intentSha256"]
            self.sends[intent] = self.sends.get(intent, 0) + 1
            action = next(
                action
                for action in self.current_journal["transactions"]
                if action["intentSha256"] == intent
            )["action"]
            if self.sends[intent] != 1:
                raise AssertionError("controller sent one intent twice")
            self._apply(action, body["signature"])
            if self.response_loss_kind == action["kind"] and not self.response_loss_used:
                self.response_loss_used = True
                raise MODULE.AmbiguousTransport("injected fake response loss")
            return {"signature": body["signature"]}
        if operation == "observe_activation":
            return copy.deepcopy(self.activations[body["role"]])
        raise AssertionError(f"unknown fake operation {operation}")

    def bind_journal(self, path: Path) -> None:
        # The fake uses the persisted action only to model a real driver that
        # decodes the signed transaction it constructed. Tests refresh this
        # projection before each driver call through the wrapper below.
        self.journal_path = path

    @property
    def current_journal(self) -> dict[str, object]:
        return json.loads(self.journal_path.read_text())


class RehearsalTests(unittest.TestCase):
    def _setup(self, root: Path) -> tuple[dict[str, object], Path, FakeDriver]:
        spec = MODULE.parse_spec(write_spec(root))
        journal = (root / "journal.json").resolve()
        driver = FakeDriver(spec)
        driver.bind_journal(journal)
        return spec, journal, driver

    def test_complete_five_upgrade_two_carry_forward_without_recycle(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            spec, journal, driver = self._setup(Path(root_text))
            summary = MODULE.run(spec, journal, driver)
            self.assertEqual(summary["status"], "passed")
            self.assertEqual(summary["upgradeCount"], 5)
            self.assertEqual(summary["carryForwardCount"], 2)
            self.assertEqual(summary["programRecycleCount"], 0)
            self.assertEqual(len(driver.sends), summary["transactionCount"])
            self.assertTrue(all(count == 1 for count in driver.sends.values()))
            durable = json.loads(journal.read_text())
            self.assertTrue(durable["complete"])
            self.assertEqual([row["phase"] for row in durable["roles"]], ["complete"] * 7)
            self.assertTrue(all(row["phase"] == "finalized" for row in durable["transactions"]))

    def test_every_required_interruption_resumes_from_chain_without_duplicate_send(self) -> None:
        boundaries = (
            "after_buffer_create:custody",
            *(f"after_buffer_write:custody:{index}" for index in range(4)),
            "before_upgrade_send:custody",
            "after_upgrade_send:custody",
            "after_postcapture:custody",
            "after_activation:custody",
        )
        for boundary in boundaries:
            with self.subTest(boundary=boundary), tempfile.TemporaryDirectory() as root_text:
                spec, journal, driver = self._setup(Path(root_text))
                with self.assertRaisesRegex(MODULE.InjectedCrash, boundary):
                    MODULE.run(spec, journal, driver, boundary)
                summary = MODULE.run(spec, journal, driver)
                self.assertEqual(summary["status"], "passed")
                self.assertTrue(all(count == 1 for count in driver.sends.values()))

    def test_rpc_response_loss_polls_frozen_signature_and_never_resends(self) -> None:
        for kind in (
            "buffer_create",
            "buffer_write",
            "loader_v3_upgrade",
            "activate_checked_release",
        ):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as root_text:
                spec, journal, driver = self._setup(Path(root_text))
                driver.response_loss_kind = kind
                with self.assertRaises(MODULE.AmbiguousTransport):
                    MODULE.run(spec, journal, driver)
                durable = json.loads(journal.read_text())
                active = [row for row in durable["transactions"] if row["phase"] == "dispatching"]
                self.assertEqual(len(active), 1)
                frozen_signature = active[0]["signature"]
                summary = MODULE.run(spec, journal, driver)
                self.assertEqual(summary["status"], "passed")
                intent = active[0]["intentSha256"]
                self.assertEqual(driver.sends[intent], 1)
                self.assertIn(frozen_signature, driver.finalized)

    def test_expired_pre_send_packet_refuses_without_resigning(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            spec, journal, driver = self._setup(Path(root_text))
            boundary = "before_upgrade_send:custody"
            with self.assertRaises(MODULE.InjectedCrash):
                MODULE.run(spec, journal, driver, boundary)
            before = json.loads(journal.read_text())
            transaction_count = len(before["transactions"])
            driver.block_height = 501
            with self.assertRaisesRegex(MODULE.Refusal, "expired"):
                MODULE.run(spec, journal, driver)
            after = json.loads(journal.read_text())
            self.assertEqual(len(after["transactions"]), transaction_count)

    def test_programdata_substitution_and_buffer_prefix_drift_refuse(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            spec, journal, driver = self._setup(Path(root_text))
            driver.roles["custody"]["programData"] = key(200)
            with self.assertRaisesRegex(MODULE.Refusal, "immutable"):
                MODULE.run(spec, journal, driver)
        with tempfile.TemporaryDirectory() as root_text:
            spec, journal, driver = self._setup(Path(root_text))
            row = spec["roles"][2]
            with self.assertRaises(MODULE.InjectedCrash):
                MODULE.run(spec, journal, driver, "after_buffer_create:custody")
            driver.buffers[row["buffer"]]["uploadedBytes"] = 9
            driver.buffers[row["buffer"]]["uploadedPrefixSha256"] = digest(b"hostile!!")
            driver.buffers[row["buffer"]]["body"] = b"hostile!!"
            with self.assertRaisesRegex(MODULE.Refusal, "prefix"):
                MODULE.run(spec, journal, driver)

    def test_carry_forward_is_reobserved_after_all_mutations(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            spec, journal, driver = self._setup(Path(root_text))
            driver.mutate_registry_on_core_activation = True
            with self.assertRaisesRegex(MODULE.Refusal, "initial chain state"):
                MODULE.run(spec, journal, driver)

    def test_policy_order_payload_authority_and_rpc_are_fail_closed(self) -> None:
        cases = (
            lambda value: value["roles"].reverse(),
            lambda value: value.update(rpcUrl="https://api.devnet.solana.com"),
            lambda value: value["roles"][2].update(bufferAuthority=key(98)),
            lambda value: value["roles"][2].update(targetPayloadSha256="0" * 64),
        )
        for mutate in cases:
            with self.subTest(mutate=mutate), tempfile.TemporaryDirectory() as root_text:
                path = write_spec(Path(root_text), mutate=mutate)
                with self.assertRaises(MODULE.Refusal):
                    MODULE.parse_spec(path)

    def test_corrupt_journal_self_digest_refuses(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            spec, journal, driver = self._setup(Path(root_text))
            with self.assertRaises(MODULE.InjectedCrash):
                MODULE.run(spec, journal, driver, "after_buffer_create:custody")
            value = json.loads(journal.read_text())
            value["sourceRevision"] = "e" * 40
            journal.write_text(json.dumps(value, sort_keys=True) + "\n")
            with self.assertRaisesRegex(MODULE.Refusal, "self-digest"):
                MODULE.run(spec, journal, driver)


if __name__ == "__main__":
    unittest.main()
