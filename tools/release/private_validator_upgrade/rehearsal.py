#!/usr/bin/env python3
"""Durable, key-free controller for private-validator Loader-v3 rehearsal.

The controller does not construct Solana instructions or read signer material.
An injected executable owns those operations.  This file owns the narrow
recovery contract: exact immutable identities, the five-Upgrade/two-carry
policy, frozen signed packets, poll-first recovery, and chain-derived progress.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
from typing import Any, Mapping, Protocol, Sequence
from urllib.parse import urlparse


SPEC_SCHEMA = "dclutch-private-loader-v3-rehearsal-spec-v1"
JOURNAL_SCHEMA = "dclutch-private-loader-v3-rehearsal-journal-v1"
DRIVER_REQUEST_SCHEMA = "dclutch-private-loader-v3-rehearsal-driver-request-v1"
DRIVER_RESPONSE_SCHEMA = "dclutch-private-loader-v3-rehearsal-driver-response-v1"
SUMMARY_SCHEMA = "dclutch-private-loader-v3-rehearsal-summary-v1"
CANONICAL_ROWS = (
    ("registry", "carry_forward"),
    ("rent-credit", "carry_forward"),
    ("custody", "upgrade"),
    ("resolution", "upgrade"),
    ("claims", "upgrade"),
    ("trading", "upgrade"),
    ("core", "upgrade"),
)
PHASES = {
    "signed_not_submitted",
    "dispatching",
    "submitted",
    "finalized",
}
MAX_JSON_BYTES = 16 * 1024 * 1024
MAX_PAYLOAD_BYTES = 16 * 1024 * 1024
MAX_PACKET_BYTES = 1232
BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


class Refusal(RuntimeError):
    """A fail-closed admission or recovery refusal."""


class Pending(RuntimeError):
    """The exact submitted transaction is not finalized yet."""


class AmbiguousTransport(RuntimeError):
    """The driver lost a response; the persisted signature must be polled."""


class InjectedCrash(RuntimeError):
    """A deterministic private-test interruption boundary."""


class Driver(Protocol):
    def call(self, operation: str, body: Mapping[str, Any]) -> Mapping[str, Any]: ...


def canonical_json(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise Refusal(f"JSON repeats key {key!r}")
        result[key] = value
    return result


def read_json(path: Path, label: str) -> tuple[dict[str, Any], bytes]:
    canonical = path.resolve(strict=True)
    if canonical != path.absolute() or path.is_symlink() or not path.is_file():
        raise Refusal(f"{label} is not an exact regular non-symlink file")
    raw = path.read_bytes()
    if not raw or len(raw) > MAX_JSON_BYTES:
        raise Refusal(f"{label} is empty or exceeds {MAX_JSON_BYTES} bytes")
    try:
        value = json.loads(raw, object_pairs_hook=_unique_object)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise Refusal(f"{label} is not unique-key JSON: {error}") from error
    if not isinstance(value, dict):
        raise Refusal(f"{label} is not a JSON object")
    return value, raw


def exact_keys(value: Mapping[str, Any], expected: set[str], label: str) -> None:
    actual = set(value)
    if actual != expected:
        raise Refusal(
            f"{label} keys differ: missing={sorted(expected - actual)} "
            f"unknown={sorted(actual - expected)}"
        )


def require_hex64(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or value.lower() != value
        or any(char not in "0123456789abcdef" for char in value)
    ):
        raise Refusal(f"{label} is not 64 lowercase hexadecimal digits")
    return value


def require_git_revision(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 40
        or value.lower() != value
        or any(char not in "0123456789abcdef" for char in value)
    ):
        raise Refusal(f"{label} is not a full 40-digit lowercase Git revision")
    return value


def base58_decode(value: Any, label: str, width: int = 32) -> bytes:
    if not isinstance(value, str) or not value:
        raise Refusal(f"{label} is empty or not text")
    number = 0
    for char in value:
        try:
            digit = BASE58_ALPHABET.index(char)
        except ValueError as error:
            raise Refusal(f"{label} is not base58") from error
        number = number * 58 + digit
    encoded_width = (number.bit_length() + 7) // 8
    body = number.to_bytes(encoded_width, "big") if encoded_width else b""
    decoded = b"\0" * (len(value) - len(value.lstrip("1"))) + body
    if len(decoded) != width:
        raise Refusal(f"{label} does not decode to {width} bytes")
    return decoded


def require_pubkey(value: Any, label: str) -> str:
    base58_decode(value, label)
    return str(value)


def require_u64(value: Any, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or not 0 <= value < 2**64:
        raise Refusal(f"{label} is not a u64")
    return value


def require_absolute_file(value: Any, label: str) -> Path:
    if not isinstance(value, str):
        raise Refusal(f"{label} is not text")
    path = Path(value)
    if not path.is_absolute():
        raise Refusal(f"{label} is not absolute")
    try:
        metadata = path.lstat()
    except OSError as error:
        raise Refusal(f"{label} cannot be inspected: {error}") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise Refusal(f"{label} is not a regular non-symlink file")
    if path.resolve(strict=True) != path:
        raise Refusal(f"{label} is not canonical")
    return path


def require_loopback_rpc(value: Any) -> str:
    if not isinstance(value, str):
        raise Refusal("rpcUrl is not text")
    parsed = urlparse(value)
    if (
        parsed.scheme != "http"
        or parsed.hostname not in {"127.0.0.1", "::1"}
        or parsed.port is None
        or parsed.username is not None
        or parsed.password is not None
        or parsed.path not in {"", "/"}
        or parsed.params
        or parsed.query
        or parsed.fragment
    ):
        raise Refusal("rehearsal RPC must be literal owned loopback http://127.0.0.1:PORT")
    return value


def atomic_write(path: Path, value: Mapping[str, Any]) -> None:
    parent = path.parent.resolve(strict=True)
    if parent != path.parent or path.is_symlink():
        raise Refusal("journal path or parent is not canonical/non-symlink")
    body = canonical_json(value)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=parent)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(body)
            output.flush()
            os.fsync(output.fileno())
        os.chmod(temporary, 0o600)
        os.replace(temporary, path)
        directory = os.open(parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def persisted_journal(path: Path, journal: dict[str, Any]) -> None:
    unsigned = dict(journal)
    unsigned.pop("journalSha256", None)
    journal["journalSha256"] = sha256_bytes(canonical_json(unsigned))
    atomic_write(path, journal)


def verify_journal_digest(journal: Mapping[str, Any]) -> None:
    expected = require_hex64(journal.get("journalSha256"), "journalSha256")
    unsigned = dict(journal)
    unsigned.pop("journalSha256", None)
    if sha256_bytes(canonical_json(unsigned)) != expected:
        raise Refusal("journal self-digest differs")


def parse_spec(path: Path) -> dict[str, Any]:
    value, raw = read_json(path, "rehearsal spec")
    exact_keys(
        value,
        {
            "schema",
            "cluster",
            "sourceRevision",
            "sourceTreeSha256",
            "checkedReleaseGateSha256",
            "rpcUrl",
            "retainedUpgradeAuthority",
            "chunkBytes",
            "roles",
        },
        "rehearsal spec",
    )
    if value["schema"] != SPEC_SCHEMA or value["cluster"] != "owned-loopback":
        raise Refusal("spec is not the owned-loopback rehearsal schema")
    require_git_revision(value["sourceRevision"], "sourceRevision")
    for field in ("sourceTreeSha256", "checkedReleaseGateSha256"):
        require_hex64(value[field], field)
    value["rpcUrl"] = require_loopback_rpc(value["rpcUrl"])
    retained = require_pubkey(value["retainedUpgradeAuthority"], "retainedUpgradeAuthority")
    chunk_bytes = require_u64(value["chunkBytes"], "chunkBytes")
    if not 1 <= chunk_bytes <= 900:
        raise Refusal("chunkBytes is outside the private rehearsal 1..=900 bound")
    rows = value["roles"]
    if not isinstance(rows, list) or len(rows) != len(CANONICAL_ROWS):
        raise Refusal("spec must contain exactly seven deployment rows")
    programs: set[str] = set()
    programdata: set[str] = set()
    buffers: set[str] = set()
    parsed_rows: list[dict[str, Any]] = []
    for ordinal, ((expected_role, expected_disposition), raw_row) in enumerate(
        zip(CANONICAL_ROWS, rows, strict=True)
    ):
        if not isinstance(raw_row, dict):
            raise Refusal(f"role row {ordinal} is not an object")
        common = {
            "role",
            "disposition",
            "program",
            "programData",
            "currentSlot",
            "currentPayloadBytes",
            "currentPayloadSha256",
        }
        upgrade = {
            "buffer",
            "bufferAuthority",
            "targetPayloadPath",
            "targetPayloadBytes",
            "targetPayloadSha256",
            "activationRecord",
        }
        exact_keys(raw_row, common | (upgrade if expected_disposition == "upgrade" else set()), f"{expected_role} row")
        if raw_row["role"] != expected_role or raw_row["disposition"] != expected_disposition:
            raise Refusal(f"row {ordinal} is not canonical {expected_role}:{expected_disposition}")
        row = dict(raw_row)
        row["program"] = require_pubkey(row["program"], f"{expected_role} program")
        row["programData"] = require_pubkey(row["programData"], f"{expected_role} ProgramData")
        row["currentSlot"] = require_u64(row["currentSlot"], f"{expected_role} currentSlot")
        row["currentPayloadBytes"] = require_u64(row["currentPayloadBytes"], f"{expected_role} currentPayloadBytes")
        require_hex64(row["currentPayloadSha256"], f"{expected_role} currentPayloadSha256")
        if row["program"] in programs or row["programData"] in programdata:
            raise Refusal("Program or ProgramData identity aliases another role")
        if row["program"] in programdata or row["programData"] in programs or row["program"] == row["programData"]:
            raise Refusal("Program and ProgramData identity sets overlap")
        programs.add(row["program"])
        programdata.add(row["programData"])
        if expected_disposition == "upgrade":
            row["activationRecord"] = require_pubkey(row["activationRecord"], f"{expected_role} activationRecord")
            row["buffer"] = require_pubkey(row["buffer"], f"{expected_role} buffer")
            row["bufferAuthority"] = require_pubkey(row["bufferAuthority"], f"{expected_role} bufferAuthority")
            if row["bufferAuthority"] != retained:
                raise Refusal(f"{expected_role} buffer authority is not retained upgrade authority")
            if row["buffer"] in buffers or row["buffer"] in programs or row["buffer"] in programdata:
                raise Refusal(f"{expected_role} buffer identity aliases another durable identity")
            buffers.add(row["buffer"])
            payload = require_absolute_file(row["targetPayloadPath"], f"{expected_role} target payload")
            payload_bytes = payload.stat().st_size
            if not 0 < payload_bytes <= MAX_PAYLOAD_BYTES:
                raise Refusal(f"{expected_role} target payload is empty or too large")
            if payload_bytes != require_u64(row["targetPayloadBytes"], f"{expected_role} targetPayloadBytes"):
                raise Refusal(f"{expected_role} target payload byte count differs")
            if sha256_file(payload) != require_hex64(row["targetPayloadSha256"], f"{expected_role} targetPayloadSha256"):
                raise Refusal(f"{expected_role} target payload SHA-256 differs")
        parsed_rows.append(row)
    value["roles"] = parsed_rows
    value["specRawSha256"] = sha256_bytes(raw)
    semantic = dict(value)
    semantic.pop("specRawSha256")
    value["specSemanticSha256"] = sha256_bytes(canonical_json(semantic))
    return value


def role_observation(driver: Driver, spec: Mapping[str, Any], row: Mapping[str, Any]) -> dict[str, Any]:
    response = dict(driver.call("observe_role", {"role": row["role"], "program": row["program"], "programData": row["programData"]}))
    exact_keys(
        response,
        {"program", "programData", "slot", "upgradeAuthority", "livePayloadBytes", "livePayloadSha256", "programAccountSha256", "programDataAccountSha256"},
        f"{row['role']} Loader observation",
    )
    if response["program"] != row["program"] or response["programData"] != row["programData"]:
        raise Refusal(f"{row['role']} observation changed immutable Program/ProgramData")
    require_u64(response["slot"], f"{row['role']} observed slot")
    require_pubkey(response["upgradeAuthority"], f"{row['role']} observed authority")
    require_u64(response["livePayloadBytes"], f"{row['role']} observed payload bytes")
    for field in ("livePayloadSha256", "programAccountSha256", "programDataAccountSha256"):
        require_hex64(response[field], f"{row['role']} {field}")
    if response["upgradeAuthority"] != spec["retainedUpgradeAuthority"]:
        raise Refusal(f"{row['role']} live authority differs from retained authority")
    return response


def require_initial(row: Mapping[str, Any], observed: Mapping[str, Any]) -> None:
    if (
        observed["slot"] != row["currentSlot"]
        or observed["livePayloadBytes"] != row["currentPayloadBytes"]
        or observed["livePayloadSha256"] != row["currentPayloadSha256"]
    ):
        raise Refusal(f"{row['role']} initial chain state differs from the bound capture")


def require_upgraded(row: Mapping[str, Any], observed: Mapping[str, Any]) -> None:
    if (
        observed["slot"] <= row["currentSlot"]
        or observed["livePayloadBytes"] != row["targetPayloadBytes"]
        or observed["livePayloadSha256"] != row["targetPayloadSha256"]
    ):
        raise Refusal(f"{row['role']} upgraded chain state is not the exact target at a later slot")


def observe_buffer(driver: Driver, row: Mapping[str, Any]) -> dict[str, Any]:
    response = dict(driver.call("observe_buffer", {"role": row["role"], "buffer": row["buffer"]}))
    if set(response) == {"exists"} and response["exists"] is False:
        return response
    exact_keys(response, {"exists", "buffer", "authority", "capacity", "uploadedBytes", "uploadedPrefixSha256", "owner"}, f"{row['role']} buffer observation")
    if response["exists"] is not True or response["buffer"] != row["buffer"]:
        raise Refusal(f"{row['role']} buffer observation is malformed")
    require_pubkey(response["authority"], f"{row['role']} buffer authority")
    require_pubkey(response["owner"], f"{row['role']} buffer owner")
    require_u64(response["capacity"], f"{row['role']} buffer capacity")
    require_u64(response["uploadedBytes"], f"{row['role']} uploaded bytes")
    require_hex64(response["uploadedPrefixSha256"], f"{row['role']} prefix SHA-256")
    if response["authority"] != row["bufferAuthority"] or response["capacity"] != row["targetPayloadBytes"]:
        raise Refusal(f"{row['role']} existing buffer authority/capacity differs")
    return response


def action_intent(action: Mapping[str, Any]) -> str:
    return sha256_bytes(canonical_json(action))


def crash(crash_at: str | None, boundary: str) -> None:
    if crash_at == boundary:
        raise InjectedCrash(boundary)


def _validate_prepared(response: Mapping[str, Any], intent: str) -> dict[str, Any]:
    exact_keys(response, {"intentSha256", "packetBase64", "packetSha256", "signature", "recentBlockhash", "lastValidBlockHeight"}, "prepared transaction")
    if require_hex64(response["intentSha256"], "prepared intent") != intent:
        raise Refusal("driver prepared a transaction for another intent")
    if not isinstance(response["packetBase64"], str):
        raise Refusal("prepared packet is not base64 text")
    try:
        packet = base64.b64decode(response["packetBase64"], validate=True)
    except (ValueError, binascii.Error) as error:
        raise Refusal("prepared packet is not canonical base64") from error
    if not 0 < len(packet) <= MAX_PACKET_BYTES:
        raise Refusal("prepared packet is outside the Solana packet bound")
    if sha256_bytes(packet) != require_hex64(response["packetSha256"], "prepared packet SHA-256"):
        raise Refusal("prepared packet digest differs")
    base58_decode(response["signature"], "prepared signature", 64)
    base58_decode(response["recentBlockhash"], "prepared blockhash", 32)
    require_u64(response["lastValidBlockHeight"], "lastValidBlockHeight")
    result = dict(response)
    result["phase"] = "signed_not_submitted"
    result["finalizedSlot"] = None
    return result


def _transaction(journal: dict[str, Any], action: Mapping[str, Any]) -> dict[str, Any] | None:
    intent = action_intent(action)
    matches = [row for row in journal["transactions"] if row["intentSha256"] == intent]
    if len(matches) > 1:
        raise Refusal("journal repeats a transaction intent")
    if matches and matches[0]["action"] != action:
        raise Refusal("journal intent digest aliases different action bytes")
    return matches[0] if matches else None


def finalize_transaction(
    driver: Driver,
    journal_path: Path,
    journal: dict[str, Any],
    action: dict[str, Any],
    crash_at: str | None,
    before_send_boundary: str | None = None,
    after_send_boundary: str | None = None,
) -> dict[str, Any]:
    intent = action_intent(action)
    transaction = _transaction(journal, action)
    if transaction is None:
        prepared = driver.call("prepare_transaction", {"intentSha256": intent, "action": action})
        transaction = _validate_prepared(prepared, intent)
        transaction["action"] = action
        journal["transactions"].append(transaction)
        persisted_journal(journal_path, journal)
    elif transaction["phase"] not in PHASES:
        raise Refusal("journal transaction phase is unknown")

    if transaction["phase"] == "finalized":
        return transaction
    if transaction["phase"] == "signed_not_submitted":
        transaction["phase"] = "dispatching"
        persisted_journal(journal_path, journal)
    if transaction["phase"] == "dispatching":
        status = driver.call("poll_transaction", {"signature": transaction["signature"]})
        exact_keys(status, {"state", "slot", "error"}, "transaction status")
        state = status["state"]
        if state == "failed":
            raise Refusal(f"frozen transaction failed: {status['error']!r}")
        if state == "finalized":
            transaction["phase"] = "finalized"
            transaction["finalizedSlot"] = require_u64(status["slot"], "finalized slot")
            persisted_journal(journal_path, journal)
            return transaction
        if state == "processed":
            raise Pending("frozen transaction is processed but not finalized")
        if state != "absent" or status["slot"] is not None or status["error"] is not None:
            raise Refusal("absent signature status is malformed")
        height = driver.call("get_block_height", {})
        exact_keys(height, {"blockHeight"}, "block height")
        if require_u64(height["blockHeight"], "block height") > transaction["lastValidBlockHeight"]:
            raise Refusal("frozen packet expired before an attributable finalized send")
        if before_send_boundary is not None:
            crash(crash_at, before_send_boundary)
        try:
            sent = driver.call(
                "send_transaction",
                {
                    "intentSha256": intent,
                    "packetBase64": transaction["packetBase64"],
                    "packetSha256": transaction["packetSha256"],
                    "signature": transaction["signature"],
                },
            )
        except AmbiguousTransport:
            raise
        exact_keys(sent, {"signature"}, "send response")
        if sent["signature"] != transaction["signature"]:
            raise Refusal("send response changed the frozen signature")
        if after_send_boundary is not None:
            crash(crash_at, after_send_boundary)
        transaction["phase"] = "submitted"
        persisted_journal(journal_path, journal)
    if transaction["phase"] == "submitted":
        status = driver.call("poll_transaction", {"signature": transaction["signature"]})
        exact_keys(status, {"state", "slot", "error"}, "submitted transaction status")
        if status["state"] == "failed":
            raise Refusal(f"submitted transaction failed: {status['error']!r}")
        if status["state"] != "finalized":
            raise Pending("submitted transaction is not finalized")
        transaction["phase"] = "finalized"
        transaction["finalizedSlot"] = require_u64(status["slot"], "finalized slot")
        persisted_journal(journal_path, journal)
    return transaction


def fresh_journal(spec: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "schema": JOURNAL_SCHEMA,
        "specRawSha256": spec["specRawSha256"],
        "specSemanticSha256": spec["specSemanticSha256"],
        "sourceRevision": spec["sourceRevision"],
        "sourceTreeSha256": spec["sourceTreeSha256"],
        "checkedReleaseGateSha256": spec["checkedReleaseGateSha256"],
        "rpcUrl": spec["rpcUrl"],
        "retainedUpgradeAuthority": spec["retainedUpgradeAuthority"],
        "roles": [
            {
                "role": row["role"],
                "disposition": row["disposition"],
                "phase": "planned",
                "before": None,
                "bufferUploadedBytes": 0,
                "postcapture": None,
                "activation": None,
            }
            for row in spec["roles"]
        ],
        "transactions": [],
        "complete": False,
        "journalSha256": "",
    }


def load_or_create_journal(path: Path, spec: Mapping[str, Any]) -> dict[str, Any]:
    if path.exists():
        value, _ = read_json(path, "rehearsal journal")
        verify_journal_digest(value)
        if (
            value.get("schema") != JOURNAL_SCHEMA
            or value.get("specRawSha256") != spec["specRawSha256"]
            or value.get("specSemanticSha256") != spec["specSemanticSha256"]
            or value.get("sourceRevision") != spec["sourceRevision"]
            or value.get("sourceTreeSha256") != spec["sourceTreeSha256"]
            or value.get("checkedReleaseGateSha256") != spec["checkedReleaseGateSha256"]
            or value.get("rpcUrl") != spec["rpcUrl"]
            or value.get("retainedUpgradeAuthority") != spec["retainedUpgradeAuthority"]
        ):
            raise Refusal("journal is not bound to the exact rehearsal spec/context")
        exact_keys(
            value,
            {
                "schema",
                "specRawSha256",
                "specSemanticSha256",
                "sourceRevision",
                "sourceTreeSha256",
                "checkedReleaseGateSha256",
                "rpcUrl",
                "retainedUpgradeAuthority",
                "roles",
                "transactions",
                "complete",
                "journalSha256",
            },
            "rehearsal journal",
        )
        roles = value.get("roles")
        if not isinstance(roles, list) or [(r.get("role"), r.get("disposition")) for r in roles if isinstance(r, dict)] != list(CANONICAL_ROWS):
            raise Refusal("journal deployment rows are not canonical")
        if not isinstance(value.get("transactions"), list) or not isinstance(value.get("complete"), bool):
            raise Refusal("journal transaction/complete shape is malformed")
        allowed_role_phases = {
            "planned",
            "buffering",
            "upgraded",
            "postcaptured",
            "activating",
            "complete",
        }
        for state in roles:
            exact_keys(
                state,
                {
                    "role",
                    "disposition",
                    "phase",
                    "before",
                    "bufferUploadedBytes",
                    "postcapture",
                    "activation",
                },
                f"{state.get('role')} journal role",
            )
            if state["phase"] not in allowed_role_phases:
                raise Refusal(f"{state['role']} journal phase is unknown")
            require_u64(state["bufferUploadedBytes"], f"{state['role']} journal buffer progress")
            for field in ("before", "postcapture", "activation"):
                if state[field] is not None and not isinstance(state[field], dict):
                    raise Refusal(f"{state['role']} journal {field} is malformed")
        seen_intents: set[str] = set()
        seen_signatures: set[str] = set()
        for transaction in value["transactions"]:
            if not isinstance(transaction, dict):
                raise Refusal("journal transaction is not an object")
            exact_keys(
                transaction,
                {
                    "intentSha256",
                    "packetBase64",
                    "packetSha256",
                    "signature",
                    "recentBlockhash",
                    "lastValidBlockHeight",
                    "phase",
                    "finalizedSlot",
                    "action",
                },
                "journal transaction",
            )
            if not isinstance(transaction["action"], dict):
                raise Refusal("journal transaction action is not an object")
            intent = require_hex64(transaction["intentSha256"], "journal intent")
            if intent != action_intent(transaction["action"]):
                raise Refusal("journal transaction intent does not bind its action")
            validated = _validate_prepared(
                {
                    key: transaction[key]
                    for key in (
                        "intentSha256",
                        "packetBase64",
                        "packetSha256",
                        "signature",
                        "recentBlockhash",
                        "lastValidBlockHeight",
                    )
                },
                intent,
            )
            if transaction["phase"] not in PHASES:
                raise Refusal("journal transaction phase is unknown")
            if transaction["phase"] == "finalized":
                require_u64(transaction["finalizedSlot"], "journal finalized slot")
            elif transaction["finalizedSlot"] is not None:
                raise Refusal("nonfinal transaction claims a finalized slot")
            if intent in seen_intents or validated["signature"] in seen_signatures:
                raise Refusal("journal repeats an intent or signature")
            seen_intents.add(intent)
            seen_signatures.add(validated["signature"])
        if value["complete"] != all(state["phase"] == "complete" for state in roles):
            raise Refusal("journal complete flag differs from its seven role phases")
        return value
    if path.is_symlink() or not path.is_absolute() or path.parent.resolve(strict=True) != path.parent:
        raise Refusal("new journal path must be absolute beneath a canonical directory")
    journal = fresh_journal(spec)
    persisted_journal(path, journal)
    return journal


def _prefix_sha(payload: Path, width: int) -> str:
    digest = hashlib.sha256()
    with payload.open("rb") as source:
        remaining = width
        while remaining:
            chunk = source.read(min(remaining, 1024 * 1024))
            if not chunk:
                raise Refusal("payload ended before the requested prefix")
            digest.update(chunk)
            remaining -= len(chunk)
    return digest.hexdigest()


def upgrade_action(spec: Mapping[str, Any], row: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "kind": "loader_v3_upgrade",
        "role": row["role"],
        "program": row["program"],
        "programData": row["programData"],
        "buffer": row["buffer"],
        "upgradeAuthority": spec["retainedUpgradeAuthority"],
        "expectedCurrentSlot": row["currentSlot"],
        "expectedCurrentPayloadSha256": row["currentPayloadSha256"],
        "targetPayloadBytes": row["targetPayloadBytes"],
        "targetPayloadSha256": row["targetPayloadSha256"],
    }


def activation_action(
    spec: Mapping[str, Any], row: Mapping[str, Any], observed: Mapping[str, Any]
) -> dict[str, Any]:
    return {
        "kind": "activate_checked_release",
        "role": row["role"],
        "program": row["program"],
        "programData": row["programData"],
        "deploymentSlot": observed["slot"],
        "livePayloadBytes": observed["livePayloadBytes"],
        "livePayloadSha256": observed["livePayloadSha256"],
        "activationRecord": row["activationRecord"],
        "checkedReleaseGateSha256": spec["checkedReleaseGateSha256"],
    }


def buffer_create_action(row: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "kind": "buffer_create",
        "role": row["role"],
        "buffer": row["buffer"],
        "bufferAuthority": row["bufferAuthority"],
        "capacity": row["targetPayloadBytes"],
        "targetPayloadSha256": row["targetPayloadSha256"],
    }


def buffer_write_action(
    row: Mapping[str, Any], payload: Path, offset: int, chunk_bytes: int
) -> dict[str, Any]:
    with payload.open("rb") as source:
        source.seek(offset)
        chunk = source.read(min(chunk_bytes, row["targetPayloadBytes"] - offset))
    if not chunk:
        raise Refusal(f"{row['role']} target payload ended at buffer offset {offset}")
    return {
        "kind": "buffer_write",
        "role": row["role"],
        "buffer": row["buffer"],
        "bufferAuthority": row["bufferAuthority"],
        "offset": offset,
        "bytes": len(chunk),
        "chunkSha256": sha256_bytes(chunk),
        "chunkBase64": base64.b64encode(chunk).decode(),
        "targetPayloadSha256": row["targetPayloadSha256"],
    }


def run_role(
    driver: Driver,
    spec: Mapping[str, Any],
    row: Mapping[str, Any],
    state: dict[str, Any],
    journal: dict[str, Any],
    journal_path: Path,
    crash_at: str | None,
) -> None:
    observed = role_observation(driver, spec, row)
    if state["before"] is None:
        require_initial(row, observed)
        state["before"] = observed
        persisted_journal(journal_path, journal)

    if row["disposition"] == "carry_forward":
        require_initial(row, observed)
        state["phase"] = "complete"
        state["postcapture"] = observed
        persisted_journal(journal_path, journal)
        return

    if state["before"] != observed and state["phase"] in {"planned", "buffering"}:
        # The process may have died after the Upgrade reached the validator but
        # before its response or the controller's phase write.  A target-looking
        # payload is not enough: recovery must also authenticate and finalize
        # the one already-fsynced transaction for this exact action.
        action = upgrade_action(spec, row)
        transaction = _transaction(journal, action)
        if transaction is None or transaction["phase"] not in {
            "dispatching",
            "submitted",
            "finalized",
        }:
            raise Refusal(f"{row['role']} Loader prestate changed without an attributable Upgrade")
        finalize_transaction(driver, journal_path, journal, action, crash_at)
        require_upgraded(row, observed)
        state["phase"] = "upgraded"
        persisted_journal(journal_path, journal)

    payload = Path(row["targetPayloadPath"])
    if state["phase"] in {"upgraded", "postcaptured", "activating", "complete"}:
        require_upgraded(row, observed)
    else:
        require_initial(row, observed)

    buffer = observe_buffer(driver, row)
    if state["phase"] in {"planned", "buffering"}:
        create = buffer_create_action(row)
        if buffer == {"exists": False}:
            finalize_transaction(driver, journal_path, journal, create, crash_at)
            crash(crash_at, f"after_buffer_create:{row['role']}")
            buffer = observe_buffer(driver, row)
        if buffer == {"exists": False}:
            raise Refusal(f"{row['role']} finalized buffer creation has no chain account")
        create_transaction = _transaction(journal, create)
        if create_transaction is None:
            raise Refusal(
                f"{row['role']} persistent buffer has no attributable create transaction"
            )
        finalize_transaction(driver, journal_path, journal, create, crash_at)
        uploaded = buffer["uploadedBytes"]
        if uploaded > row["targetPayloadBytes"] or uploaded % spec["chunkBytes"] != 0 and uploaded != row["targetPayloadBytes"]:
            raise Refusal(f"{row['role']} buffer uploaded width is not a canonical chunk boundary")
        if buffer["uploadedPrefixSha256"] != _prefix_sha(payload, uploaded):
            raise Refusal(f"{row['role']} existing buffer prefix differs from target bytes")
        state["phase"] = "buffering"
        recorded_uploaded = state["bufferUploadedBytes"]
        if uploaded < recorded_uploaded:
            raise Refusal(f"{row['role']} chain buffer regressed behind its journal")
        if uploaded > recorded_uploaded:
            recovery_action = buffer_write_action(
                row, payload, recorded_uploaded, spec["chunkBytes"]
            )
            expected_uploaded = recorded_uploaded + recovery_action["bytes"]
            if uploaded != expected_uploaded or _transaction(journal, recovery_action) is None:
                raise Refusal(
                    f"{row['role']} chain buffer advanced without one attributable write"
                )
            finalize_transaction(
                driver, journal_path, journal, recovery_action, crash_at
            )
            state["bufferUploadedBytes"] = uploaded
            persisted_journal(journal_path, journal)
        else:
            persisted_journal(journal_path, journal)
        index = uploaded // spec["chunkBytes"]
        while uploaded < row["targetPayloadBytes"]:
            action = buffer_write_action(row, payload, uploaded, spec["chunkBytes"])
            finalize_transaction(driver, journal_path, journal, action, crash_at)
            crash(crash_at, f"after_buffer_write:{row['role']}:{index}")
            uploaded += action["bytes"]
            state["bufferUploadedBytes"] = uploaded
            persisted_journal(journal_path, journal)
            observed_buffer = observe_buffer(driver, row)
            if observed_buffer == {"exists": False} or observed_buffer["uploadedBytes"] != uploaded or observed_buffer["uploadedPrefixSha256"] != _prefix_sha(payload, uploaded):
                raise Refusal(f"{row['role']} chain buffer did not authenticate finalized chunk {index}")
            index += 1

        upgrade = upgrade_action(spec, row)
        finalize_transaction(
            driver,
            journal_path,
            journal,
            upgrade,
            crash_at,
            f"before_upgrade_send:{row['role']}",
            f"after_upgrade_send:{row['role']}",
        )
        state["phase"] = "upgraded"
        persisted_journal(journal_path, journal)

    observed = role_observation(driver, spec, row)
    require_upgraded(row, observed)
    if state["postcapture"] is None:
        state["postcapture"] = observed
        state["phase"] = "postcaptured"
        persisted_journal(journal_path, journal)
        crash(crash_at, f"after_postcapture:{row['role']}")
    elif state["postcapture"] != observed:
        raise Refusal(f"{row['role']} upgraded postcapture drifted")

    activation_action_value = activation_action(spec, row, observed)
    state["phase"] = "activating"
    persisted_journal(journal_path, journal)
    finalized = finalize_transaction(
        driver, journal_path, journal, activation_action_value, crash_at
    )
    activation = dict(driver.call("observe_activation", activation_action_value))
    exact_keys(activation, {"role", "program", "programData", "deploymentSlot", "livePayloadSha256", "activationRecord", "activationRecordSha256", "finalizedSlot"}, f"{row['role']} activation observation")
    if (
        activation["role"] != row["role"]
        or activation["program"] != row["program"]
        or activation["programData"] != row["programData"]
        or activation["deploymentSlot"] != observed["slot"]
        or activation["livePayloadSha256"] != row["targetPayloadSha256"]
        or activation["activationRecord"] != row["activationRecord"]
    ):
        raise Refusal(f"{row['role']} activation does not bind exact upgraded release")
    require_hex64(activation["activationRecordSha256"], f"{row['role']} activation record SHA-256")
    require_u64(activation["finalizedSlot"], f"{row['role']} activation finalized slot")
    if activation["finalizedSlot"] < finalized["finalizedSlot"]:
        raise Refusal(f"{row['role']} activation observation predates its transaction")
    state["activation"] = activation
    if role_observation(driver, spec, row) != state["postcapture"]:
        raise Refusal(f"{row['role']} activation changed the upgraded Loader state")
    state["phase"] = "complete"
    persisted_journal(journal_path, journal)
    crash(crash_at, f"after_activation:{row['role']}")


def run(spec: Mapping[str, Any], journal_path: Path, driver: Driver, crash_at: str | None = None) -> dict[str, Any]:
    journal = load_or_create_journal(journal_path, spec)
    for row, state in zip(spec["roles"], journal["roles"], strict=True):
        if state["phase"] == "complete":
            observed = role_observation(driver, spec, row)
            if row["disposition"] == "carry_forward":
                require_initial(row, observed)
            else:
                require_upgraded(row, observed)
                if state["postcapture"] != observed:
                    raise Refusal(f"{row['role']} completed postcapture drifted")
                activation = driver.call(
                    "observe_activation", activation_action(spec, row, observed)
                )
                if activation != state["activation"]:
                    raise Refusal(f"{row['role']} completed activation record drifted")
            continue
        run_role(driver, spec, row, state, journal, journal_path, crash_at)

    # Reobserve the two infrastructure rows after every mutation.  Their
    # immutability is a final condition, not merely a preflight claim.
    for row, state in zip(spec["roles"], journal["roles"], strict=True):
        final_observation = role_observation(driver, spec, row)
        if row["disposition"] == "carry_forward":
            require_initial(row, final_observation)
        else:
            require_upgraded(row, final_observation)
            if final_observation != state["postcapture"]:
                raise Refusal(f"{row['role']} final Loader state differs from postcapture")
            if driver.call(
                "observe_activation",
                activation_action(spec, row, final_observation),
            ) != state["activation"]:
                raise Refusal(f"{row['role']} final activation record drifted")
    if [state["phase"] for state in journal["roles"]] != ["complete"] * 7:
        raise Refusal("rehearsal ended with an incomplete deployment row")
    journal["complete"] = True
    persisted_journal(journal_path, journal)
    return {
        "schema": SUMMARY_SCHEMA,
        "status": "passed",
        "journal": str(journal_path),
        "journalSha256": journal["journalSha256"],
        "sourceRevision": spec["sourceRevision"],
        "sourceTreeSha256": spec["sourceTreeSha256"],
        "checkedReleaseGateSha256": spec["checkedReleaseGateSha256"],
        "carryForwardCount": 2,
        "upgradeCount": 5,
        "transactionCount": len(journal["transactions"]),
        "programRecycleCount": 0,
    }


class SubprocessDriver:
    def __init__(self, executable: Path, rpc_url: str, timeout_seconds: int = 60):
        self.executable = require_absolute_file(str(executable), "driver executable")
        if not os.access(self.executable, os.X_OK):
            raise Refusal("driver is not executable")
        self.rpc_url = require_loopback_rpc(rpc_url)
        self.timeout_seconds = timeout_seconds

    def call(self, operation: str, body: Mapping[str, Any]) -> Mapping[str, Any]:
        request = {
            "schema": DRIVER_REQUEST_SCHEMA,
            "operation": operation,
            "rpcUrl": self.rpc_url,
            "body": body,
        }
        try:
            completed = subprocess.run(
                [str(self.executable)],
                input=canonical_json(request),
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=self.timeout_seconds,
                check=False,
                env={"PATH": "/usr/bin:/bin"},
            )
        except subprocess.TimeoutExpired as error:
            if operation == "send_transaction":
                raise AmbiguousTransport("driver timed out after an attributable send attempt") from error
            raise Refusal(f"driver timed out during {operation}") from error
        if completed.returncode != 0:
            if operation == "send_transaction":
                raise AmbiguousTransport("driver lost the send response; resume must poll the frozen signature")
            raise Refusal(f"driver refused {operation} with exit {completed.returncode}")
        try:
            response = json.loads(completed.stdout, object_pairs_hook=_unique_object)
        except (json.JSONDecodeError, UnicodeDecodeError) as error:
            if operation == "send_transaction":
                raise AmbiguousTransport("driver returned no attributable send response") from error
            raise Refusal(f"driver returned malformed JSON for {operation}") from error
        if not isinstance(response, dict):
            raise Refusal("driver response is not an object")
        exact_keys(response, {"schema", "operation", "body"}, "driver envelope")
        if response["schema"] != DRIVER_RESPONSE_SCHEMA or response["operation"] != operation or not isinstance(response["body"], dict):
            raise Refusal("driver response envelope differs from request")
        return response["body"]


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spec", type=Path, required=True)
    parser.add_argument("--journal", type=Path, required=True)
    parser.add_argument("--driver", type=Path, required=True)
    parser.add_argument("--crash-at")
    arguments = parser.parse_args(argv)
    try:
        spec = parse_spec(arguments.spec)
        driver = SubprocessDriver(arguments.driver, spec["rpcUrl"])
        summary = run(spec, arguments.journal, driver, arguments.crash_at)
    except InjectedCrash as error:
        print(f"private-validator-upgrade: INJECTED CRASH: {error}", file=sys.stderr)
        return 75
    except Pending as error:
        print(f"private-validator-upgrade: PENDING: {error}", file=sys.stderr)
        return 76
    except AmbiguousTransport as error:
        print(f"private-validator-upgrade: AMBIGUOUS SEND: {error}", file=sys.stderr)
        return 77
    except (OSError, Refusal) as error:
        print(f"private-validator-upgrade: REFUSED: {error}", file=sys.stderr)
        return 1
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
