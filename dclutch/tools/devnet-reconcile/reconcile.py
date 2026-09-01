#!/usr/bin/env python3
"""Independent, bounded reconciliation of one complete Dragon's Clutch activity.

This program deliberately has no dependency on the TypeScript clients or an
indexer.  It checks a strict adapter manifest against captured or freshly read
finalized Solana JSON-RPC evidence and emits a deterministic unsigned dossier.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import pathlib
import secrets
import struct
import sys
import time
import urllib.error
import urllib.request
from typing import Any


DEVNET_GENESIS_HASH = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
MAINNET_GENESIS_HASH = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
LOADER_V3_PROGRAM_ID = "BPFLoaderUpgradeab1e11111111111111111111111"
MANIFEST_SCHEMA = "dclutch-activity-reconcile-manifest-v1"
CAPTURE_SCHEMA = "dclutch-captured-finalized-rpc-v1"
DOSSIER_SCHEMA = "dclutch-public-activity-dossier-v1"
OWNED_LOOPBACK_MANIFEST_SCHEMA = "dclutch-owned-loopback-activity-reconcile-manifest-v1"
OWNED_LOOPBACK_CAPTURE_SCHEMA = "dclutch-owned-loopback-captured-finalized-rpc-v1"
OWNED_LOOPBACK_RECEIPT_SCHEMA = "dclutch-owned-loopback-reconcile-session-receipt-v1"
OWNED_LOOPBACK_PROVIDER_CLOSURE_SCHEMA = (
    "dclutch-owned-loopback-pyth-provider-closure-v1"
)
OWNED_LOOPBACK_PROVIDER_PLAN_SCHEMA = "dclutch-local-successor-infrastructure-plan-v2"
OWNED_LOOPBACK_PROVIDER_PROFILE_SCHEMA = "dclutch-successor-local-validator-profile-v1"
OWNED_LOOPBACK_PRIVATE_SESSION_SCHEMA = (
    "dclutch-owned-loopback-private-lifecycle-session-v1"
)
OWNED_LOOPBACK_CHAOS_SESSION_SCHEMA = (
    "dclutch-owned-loopback-private-lifecycle-chaos-session-v1"
)
OWNED_LOOPBACK_DOSSIER_SCHEMA = "dclutch-owned-loopback-activity-dossier-v1"
OWNED_LOOPBACK_TERMINAL_COMPLETION_SCHEMA = (
    "dclutch-owned-loopback-terminal-sequence-completion-v1"
)
OWNED_LOOPBACK_TERMINAL_JOURNAL_SCHEMA = (
    "dclutch-owned-loopback-terminal-sequence-journal-v1"
)
OWNED_LOOPBACK_TERMINAL_SESSION_SCHEMA = (
    "dclutch-owned-loopback-terminal-sequence-session-v1"
)
EVENT_KINDS = ("founding", "participant", "direct", "resolution", "payout", "retirement")
RESOLUTION_OPERATIONS_V7 = (
    "resolution-submit",
    "resolution-provider-execute-v1",
    "core-terminal-accept-v1",
    "resolution-reclaim",
)
OWNED_LOOPBACK_COMPLETED_STAGES = (
    "founding", "participant", "alt", "seal", "direct", "resolution", "payout", "retirement",
)
OWNED_LOOPBACK_CHAOS_STAGES = (
    "founding", "participant", "alt", "seal", "hot", "resolution", "payout", "retire",
)
OWNED_LOOPBACK_PROGRAM_ROLES = (
    "registry", "rent", "custody", "resolution", "claims", "trading", "core",
    "pyth-receiver", "pyth-router",
)
MAX_JSON_BYTES = 32 * 1024 * 1024
MAX_EVENTS = 128
MAX_ACCOUNTS = 512
MAX_POLLS = 30
BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


class Refusal(Exception):
    """Evidence is absent, ambiguous, hostile, or contradictory."""


def refuse(message: str) -> None:
    raise Refusal(message)


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    out: dict[str, Any] = {}
    for key, value in pairs:
        if key in out:
            refuse(f"duplicate JSON key {key!r}")
        out[key] = value
    return out


def load_json(path: pathlib.Path) -> Any:
    try:
        size = path.stat().st_size
    except OSError as error:
        refuse(f"cannot stat {path}: {error}")
    if size > MAX_JSON_BYTES:
        refuse(f"{path} exceeds the {MAX_JSON_BYTES}-byte evidence bound")
    try:
        return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        refuse(f"cannot read strict JSON {path}: {error}")


def read_evidence_file(path: pathlib.Path) -> tuple[bytes, Any]:
    try:
        size = path.stat().st_size
        if size > MAX_JSON_BYTES:
            refuse(f"{path} exceeds the {MAX_JSON_BYTES}-byte evidence bound")
        raw = path.read_bytes()
        decoded = json.loads(raw, object_pairs_hook=unique_object)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        refuse(f"cannot read strict source journal {path}: {error}")
    if not isinstance(decoded, dict):
        refuse(f"source journal {path} must be a JSON object")
    return raw, decoded


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def exact_keys(value: Any, required: set[str], optional: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        refuse(f"{label} must be an object")
    keys = set(value)
    missing = required - keys
    unknown = keys - required - optional
    if missing:
        refuse(f"{label} omitted {sorted(missing)}")
    if unknown:
        refuse(f"{label} has unknown fields {sorted(unknown)}")
    return value


def text(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        refuse(f"{label} must be a nonempty string")
    return value


def decimal(value: Any, label: str, signed: bool = False) -> int:
    if not isinstance(value, str):
        refuse(f"{label} must be a decimal string")
    if signed and value.startswith("-"):
        body = value[1:]
        if not body or body == "0":
            refuse(f"{label} is not a canonical signed decimal string")
    else:
        body = value
    if not body.isdigit() or (len(body) > 1 and body.startswith("0")):
        refuse(f"{label} is not a canonical decimal string")
    result = int(value)
    if not signed and result < 0:
        refuse(f"{label} must be nonnegative")
    return result


def authenticated_transaction_fee(value: Any, label: str, cluster_kind: str) -> int:
    """Parse a present canonical fee under an already-authenticated cluster."""
    fee = decimal(value, label)
    if cluster_kind == "devnet" and fee == 0:
        refuse(
            f"{label} is exact zero on public devnet; zero is admitted only for an authenticated owned-loopback genesis"
        )
    if cluster_kind != "owned-loopback" and cluster_kind != "devnet":
        refuse(f"{label} has no authenticated cluster fee policy")
    return fee


def digest(value: Any, label: str) -> str:
    encoded = text(value, label)
    if len(encoded) != 64 or any(ch not in "0123456789abcdef" for ch in encoded):
        refuse(f"{label} must be lowercase sha256 hex")
    return encoded


def b64(value: Any, label: str) -> bytes:
    if not isinstance(value, str):
        refuse(f"{label} must be base64 text")
    encoded = value
    try:
        decoded = base64.b64decode(encoded, validate=True)
    except ValueError:
        refuse(f"{label} is not canonical base64")
    if base64.b64encode(decoded).decode() != encoded:
        refuse(f"{label} is not canonical base64")
    return decoded


def b58decode(value: str, label: str) -> bytes:
    number = 0
    for character in value:
        try:
            number = number * 58 + BASE58.index(character)
        except ValueError:
            refuse(f"{label} is not base58")
    raw = number.to_bytes((number.bit_length() + 7) // 8, "big") if number else b""
    return b"\0" * (len(value) - len(value.lstrip("1"))) + raw


def pubkey(value: Any, label: str) -> str:
    encoded = text(value, label)
    if len(b58decode(encoded, label)) != 32:
        refuse(f"{label} is not a 32-byte Solana address")
    return encoded


def account_data(value: Any, label: str) -> bytes:
    if not isinstance(value, list) or len(value) != 2 or value[1] != "base64":
        refuse(f"{label} must use exact [data, base64] RPC encoding")
    return b64(value[0], label)


def u64(data: bytes, offset: int) -> int:
    return struct.unpack_from("<Q", data, offset)[0]


def i128(data: bytes, offset: int) -> int:
    return int.from_bytes(data[offset : offset + 16], "little", signed=True)


def decode_token_account(data: bytes) -> dict[str, Any]:
    if len(data) < 165:
        refuse("Token-2022 account is shorter than the 165-byte base account")
    state = data[108]
    if state not in (1, 2):
        refuse("Token-2022 account has a non-live base state")
    return {
        "mintBytes": data[0:32],
        "authorityBytes": data[32:64],
        "amountAtoms": u64(data, 64),
    }


def decode_position(data: bytes) -> dict[str, Any]:
    if len(data) < 128 or data[0:8] != b"DCLLBP02" or struct.unpack_from("<H", data, 8)[0] != 2:
        refuse("Position is not exact LiabilityBasisPositionV2")
    if data[10:12] != b"\0\0" or data[120:128] != b"\0" * 8:
        refuse("Position has nonzero reserved bytes")
    count = struct.unpack_from("<I", data, 12)[0]
    if count < 2 or len(data) != 128 + 8 * count:
        refuse("Position has noncanonical claim geometry")
    balances = [u64(data, 128 + 8 * index) for index in range(count)]
    return {
        "claimCount": str(count),
        "revision": str(u64(data, 16)),
        "aggregateHex": data[24:56].hex(),
        "ownerHex": data[56:88].hex(),
        "basisHex": data[88:120].hex(),
        "balancesAtoms": [str(value) for value in balances],
    }


def nonzero(data: bytes, label: str) -> None:
    if data == b"\0" * len(data):
        refuse(f"ResolutionCertificateV2 {label} is zero")


def decode_certificate(data: bytes) -> dict[str, Any]:
    if len(data) != 312 or data[0:8] != b"DCSRCER2" or struct.unpack_from("<H", data, 8)[0] != 2:
        refuse("certificate is not exact ResolutionCertificateV2")
    if data[11:16] != b"\0" * 5 or data[260:264] != b"\0" * 4:
        refuse("ResolutionCertificateV2 has nonzero reserved bytes")
    kind_by_tag = {1: "resolution-success", 2: "recovery-advanced", 3: "exhausted", 4: "resolution-failure"}
    kind = kind_by_tag.get(data[10])
    if kind is None:
        refuse("ResolutionCertificateV2 has an unknown kind")
    fields = {
        "market": data[16:48], "route": data[48:80], "sourceMaterial": data[80:112],
        "productRecordDigest": data[112:144], "providerEvidence": data[144:176],
        "fundingAllocation": data[176:208], "receiptAccount": data[208:240],
    }
    for name in ("market", "sourceMaterial", "productRecordDigest", "receiptAccount"):
        nonzero(fields[name], name)
    generation = u64(data, 240)
    attempt = struct.unpack_from("<I", data, 248)[0]
    schedule = struct.unpack_from("<I", data, 252)[0]
    selector = struct.unpack_from("<I", data, 256)[0]
    work_paid = u64(data, 264)
    funding_remaining = u64(data, 272)
    numerator = i128(data, 280)
    denominator = u64(data, 296)
    observed_at = u64(data, 304)
    if generation == 0:
        refuse("ResolutionCertificateV2 generation is zero")
    zero = b"\0" * 32
    if kind == "resolution-success":
        nonzero(fields["route"], "route")
        nonzero(fields["providerEvidence"], "provider evidence")
        if denominator == 0 or observed_at == 0:
            refuse("ResolutionCertificateV2 success has an invalid result")
    elif kind == "resolution-failure":
        nonzero(fields["fundingAllocation"], "funding allocation")
        if fields["route"] != zero or fields["providerEvidence"] != zero or work_paid == 0 or schedule != 0 or numerator != 0 or denominator != 0 or observed_at != 0:
            refuse("ResolutionCertificateV2 failure has a noncanonical shape")
    else:
        nonzero(fields["route"], "route")
        nonzero(fields["fundingAllocation"], "funding allocation")
        if fields["providerEvidence"] != zero or selector != 0 or work_paid == 0 or numerator != 0 or denominator != 0 or observed_at == 0:
            refuse("ResolutionCertificateV2 liveness transition has a noncanonical shape")
    return {
        "kind": kind, "marketHex": fields["market"].hex(), "routeHex": fields["route"].hex(),
        "sourceMaterialHex": fields["sourceMaterial"].hex(),
        "productRecordDigestHex": fields["productRecordDigest"].hex(),
        "providerEvidenceHex": fields["providerEvidence"].hex(),
        "fundingAllocationHex": fields["fundingAllocation"].hex(),
        "receiptAccountHex": fields["receiptAccount"].hex(), "generation": str(generation),
        "attemptIndex": str(attempt), "scheduleIndex": str(schedule), "selector": str(selector),
        "workPaidAtoms": str(work_paid), "fundingRemainingAtoms": str(funding_remaining),
        "resultNumerator": str(numerator), "resultDenominator": str(denominator),
        "observedAt": str(observed_at),
    }


class CapturedRpc:
    def __init__(self, value: dict[str, Any], capture_sha256: str | None = None) -> None:
        self.value = value
        self.capture_sha256 = capture_sha256 or sha256_bytes(canonical_bytes(value))

    def provenance(self) -> dict[str, str]:
        return {"mode": "captured-finalized-rpc-replay", "captureSha256": self.capture_sha256}

    def genesis_hash(self) -> str:
        return text(self.value.get("genesisHash"), "capture genesisHash")

    def transaction(self, signature: str) -> Any:
        transactions = self.value.get("transactions")
        if not isinstance(transactions, dict) or signature not in transactions:
            refuse(f"finalized transaction {signature} is missing")
        return transactions[signature]

    def account(self, address: str) -> tuple[int, Any]:
        accounts = self.value.get("accounts")
        if not isinstance(accounts, dict) or address not in accounts:
            refuse(f"finalized account {address} is missing")
        observed = exact_keys(accounts[address], {"contextSlot", "value"}, set(), f"account {address}")
        return decimal(observed["contextSlot"], f"account {address} contextSlot"), observed["value"]


class OwnedLoopbackCapturedRpc(CapturedRpc):
    def __init__(self, value: dict[str, Any], capture_sha256: str | None = None) -> None:
        super().__init__(value, capture_sha256)
        self.finalized_slot = decimal(value.get("finalizedSlot"), "owned-loopback capture finalizedSlot")
        if self.finalized_slot == 0:
            refuse("owned-loopback capture finalizedSlot must be positive")
        accounts = value.get("accounts")
        if not isinstance(accounts, dict) or not accounts:
            refuse("owned-loopback capture account map must be nonempty")
        for address, raw_row in accounts.items():
            pubkey(address, "owned-loopback capture account address")
            row = exact_keys(
                raw_row,
                {"contextSlot", "value"},
                set(),
                f"owned-loopback capture account {address}",
            )
            if decimal(
                row["contextSlot"], f"owned-loopback capture account {address} contextSlot"
            ) != self.finalized_slot:
                refuse(
                    "owned-loopback capture account contextSlot differs from the singular finalizedSlot"
                )

    def provenance(self) -> dict[str, str]:
        return {
            "mode": "owned-loopback-captured-finalized-rpc-replay",
            "captureSha256": self.capture_sha256,
            "finalizedSlot": str(self.finalized_slot),
        }

    def transaction(self, signature: str) -> Any:
        result = super().transaction(signature)
        slot = result.get("slot") if isinstance(result, dict) else None
        if not isinstance(slot, int) or slot > self.finalized_slot:
            refuse(f"owned-loopback transaction {signature} is not covered by the finalized capture slot")
        return result

    def account(self, address: str) -> tuple[int, Any]:
        slot, value = super().account(address)
        if slot != self.finalized_slot:
            refuse(f"owned-loopback account {address} is not from the singular finalized capture slot")
        return slot, value


class LiveRpc:
    def __init__(self, url: str, timeout: float) -> None:
        if not url.startswith("https://"):
            refuse("live RPC URL must use https")
        self.url = url
        self.timeout = timeout
        self.request_id = 0

    def provenance(self) -> dict[str, str]:
        return {"mode": "live-finalized-rpc", "endpointSha256": sha256_bytes(self.url.encode())}

    def call(self, method: str, params: list[Any]) -> Any:
        self.request_id += 1
        body = json.dumps({"jsonrpc": "2.0", "id": self.request_id, "method": method, "params": params}).encode()
        request = urllib.request.Request(self.url, data=body, headers={"content-type": "application/json"}, method="POST")
        try:
            with urllib.request.urlopen(request, timeout=self.timeout) as response:
                raw = response.read(MAX_JSON_BYTES + 1)
        except (urllib.error.URLError, TimeoutError) as error:
            refuse(f"bounded finalized RPC {method} failed: {error}")
        if len(raw) > MAX_JSON_BYTES:
            refuse("RPC response exceeded the evidence bound")
        try:
            decoded = json.loads(raw, object_pairs_hook=unique_object)
        except (UnicodeError, json.JSONDecodeError) as error:
            refuse(f"RPC returned invalid strict JSON: {error}")
        if not isinstance(decoded, dict) or decoded.get("error") is not None or "result" not in decoded:
            refuse(f"RPC {method} refused or omitted its result")
        return decoded["result"]

    def genesis_hash(self) -> str:
        return text(self.call("getGenesisHash", []), "RPC genesis hash")

    def transaction(self, signature: str) -> Any:
        return self.call("getTransaction", [signature, {"commitment": "finalized", "encoding": "json", "maxSupportedTransactionVersion": 0}])

    def account(self, address: str) -> tuple[int, Any]:
        result = self.call("getAccountInfo", [address, {"commitment": "finalized", "encoding": "base64"}])
        result = exact_keys(result, {"context", "value"}, set(), f"getAccountInfo {address}")
        context = exact_keys(result["context"], {"slot"}, {"apiVersion"}, "account context")
        slot = context["slot"]
        if not isinstance(slot, int) or slot < 0:
            refuse("RPC account context slot is invalid")
        return slot, result["value"]


def account_keys(result: dict[str, Any]) -> list[str]:
    transaction = result.get("transaction")
    if not isinstance(transaction, dict):
        refuse("transaction result omitted transaction")
    message = transaction.get("message")
    if not isinstance(message, dict) or not isinstance(message.get("accountKeys"), list):
        refuse("transaction result omitted message account keys")
    keys: list[str] = []
    for index, item in enumerate(message["accountKeys"]):
        value = item.get("pubkey") if isinstance(item, dict) else item
        keys.append(pubkey(value, f"transaction account key {index}"))
    meta = result.get("meta")
    if not isinstance(meta, dict):
        refuse("transaction result omitted meta")
    loaded = meta.get("loadedAddresses") or {"writable": [], "readonly": []}
    if not isinstance(loaded, dict):
        refuse("transaction loadedAddresses is invalid")
    for label in ("writable", "readonly"):
        values = loaded.get(label, [])
        if not isinstance(values, list):
            refuse(f"transaction loaded {label} addresses are invalid")
        keys.extend(pubkey(value, f"loaded {label} address") for value in values)
    if len(keys) != len(set(keys)):
        refuse("transaction account vector aliases an address")
    return keys


def token_amounts(meta: dict[str, Any], key: str, keys: list[str]) -> dict[str, tuple[str, str, int]]:
    values = meta.get(key, [])
    if not isinstance(values, list):
        refuse(f"transaction {key} is invalid")
    out: dict[str, tuple[str, str, int]] = {}
    for entry in values:
        entry = exact_keys(entry, {"accountIndex", "mint", "uiTokenAmount"}, {"owner", "programId"}, key)
        index = entry["accountIndex"]
        if not isinstance(index, int) or index < 0 or index >= len(keys):
            refuse(f"transaction {key} has an invalid account index")
        address = keys[index]
        if address in out:
            refuse(f"transaction {key} duplicates token account {address}")
        ui = entry["uiTokenAmount"]
        if not isinstance(ui, dict) or "amount" not in ui:
            refuse(f"transaction {key} omitted raw token atoms")
        amount = decimal(ui["amount"], f"{key} amount")
        owner = pubkey(entry.get("owner"), f"{key} owner")
        out[address] = (pubkey(entry["mint"], f"{key} mint"), owner, amount)
    return out


def parse_delta_list(values: Any, field: str, accounts: dict[str, dict[str, Any]], signed: bool = True) -> dict[str, int]:
    if not isinstance(values, list):
        refuse(f"{field} must be an array")
    out: dict[str, int] = {}
    for index, entry in enumerate(values):
        entry = exact_keys(entry, {"account", "atoms" if field == "tokenDeltas" else "lamports"}, set(), f"{field}[{index}]")
        ref = text(entry["account"], f"{field} account")
        if ref not in accounts or ref in out:
            refuse(f"{field} has an unknown or duplicate account {ref}")
        amount_field = "atoms" if field == "tokenDeltas" else "lamports"
        out[ref] = decimal(entry[amount_field], f"{field} {ref}", signed=signed)
    return out


def validate_manifest_for(
    manifest: Any,
    *,
    manifest_schema: str,
    cluster_kind: str,
    genesis_hash: str,
) -> tuple[dict[str, dict[str, Any]], list[dict[str, Any]]]:
    manifest = exact_keys(manifest, {"schema", "activityId", "cluster", "accounts", "events", "finalAccounts", "sourceSetSha256"}, set(), "manifest")
    if manifest["schema"] != manifest_schema:
        refuse("activity manifest schema is not admitted")
    text(manifest["activityId"], "activityId")
    cluster = exact_keys(manifest["cluster"], {"kind", "genesisHash"}, set(), "cluster")
    if cluster["kind"] != cluster_kind or cluster["genesisHash"] != genesis_hash:
        if cluster_kind == "devnet":
            refuse("activity manifest is not pinned to exact Solana devnet")
        refuse(f"activity manifest is not pinned to exact {cluster_kind} genesis")
    digest(manifest["sourceSetSha256"], "sourceSetSha256")
    raw_accounts = manifest["accounts"]
    if not isinstance(raw_accounts, list) or not raw_accounts or len(raw_accounts) > MAX_ACCOUNTS:
        refuse("accounts must be a nonempty bounded array")
    accounts: dict[str, dict[str, Any]] = {}
    addresses: set[str] = set()
    for index, item in enumerate(raw_accounts):
        item = exact_keys(item, {"ref", "address", "kind", "role"}, {"mint", "assetClass", "authority", "programOwner"}, f"accounts[{index}]")
        ref = text(item["ref"], "account ref")
        address = pubkey(item["address"], f"account {ref} address")
        if ref in accounts or address in addresses:
            refuse("account identities contain a duplicate ref or aliased address")
        if item["kind"] not in ("wallet", "token", "position", "certificate", "protocol"):
            refuse(f"account {ref} has an unknown kind")
        text(item["role"], f"account {ref} role")
        if item["kind"] == "token":
            if set(item) != {"ref", "address", "kind", "role", "mint", "assetClass", "authority", "programOwner"}:
                refuse(f"token account {ref} must declare exact mint, class, authority, and program owner")
            pubkey(item["mint"], f"token account {ref} mint")
            pubkey(item["authority"], f"token account {ref} authority")
            pubkey(item["programOwner"], f"token account {ref} programOwner")
            if item["assetClass"] not in ("collateral", "claim"):
                refuse(f"token account {ref} has an unknown assetClass")
        elif any(field in item for field in ("mint", "assetClass", "authority", "programOwner")):
            refuse(f"non-token account {ref} declares token-only fields")
        accounts[ref] = item
        addresses.add(address)
    events = manifest["events"]
    if not isinstance(events, list) or len(events) < len(EVENT_KINDS) or len(events) > MAX_EVENTS:
        refuse("activity must contain a bounded event chain covering every required kind")
    ids: set[str] = set()
    signatures: set[str] = set()
    seen_kinds: set[str] = set()
    previous: str | None = None
    prior_slot = -1
    optional_event_fields = {"direct", "position", "certificate", "payout", "retirement"}
    if cluster_kind == "owned-loopback":
        optional_event_fields.update({"positions", "feeSettlement"})
    for index, event in enumerate(events):
        event = exact_keys(event, {"id", "kind", "operation", "predecessor", "signature", "slot", "feePayer", "feeLamports", "computeUnitsConsumed", "lamportDeltas", "tokenDeltas", "sourcePath", "sourceSha256"}, optional_event_fields, f"events[{index}]")
        event_id = text(event["id"], "event id")
        text(event["operation"], f"event {event_id} operation")
        signature = text(event["signature"], f"event {event_id} signature")
        slot = decimal(event["slot"], f"event {event_id} slot")
        if event["kind"] not in EVENT_KINDS:
            refuse(f"event {event_id} has an unknown kind")
        kind_index = EVENT_KINDS.index(event["kind"])
        if seen_kinds and kind_index < max(EVENT_KINDS.index(kind) for kind in seen_kinds):
            refuse("activity events move backward across lifecycle kinds")
        seen_kinds.add(event["kind"])
        if event_id in ids or signature in signatures:
            refuse("activity contains a duplicate event id or transaction signature")
        if event["predecessor"] != previous:
            refuse(f"event {event_id} forks or omits its exact predecessor")
        if slot < prior_slot:
            refuse("activity event slots regress")
        fee_ref = text(event["feePayer"], f"event {event_id} fee payer")
        if fee_ref not in accounts or accounts[fee_ref]["kind"] != "wallet":
            refuse(f"event {event_id} fee payer is not a declared wallet")
        authenticated_transaction_fee(
            event["feeLamports"], f"event {event_id} fee", cluster_kind
        )
        if decimal(event["computeUnitsConsumed"], f"event {event_id} compute units") == 0:
            refuse(f"event {event_id} compute units must be positive")
        source_path = text(event["sourcePath"], f"event {event_id} sourcePath")
        parts = pathlib.PurePosixPath(source_path)
        if parts.is_absolute() or source_path != parts.as_posix() or any(part in ("", ".", "..") for part in parts.parts):
            refuse(f"event {event_id} sourcePath is not a canonical relative path")
        digest(event["sourceSha256"], f"event {event_id} sourceSha256")
        parse_delta_list(event["lamportDeltas"], "lamportDeltas", accounts)
        parse_delta_list(event["tokenDeltas"], "tokenDeltas", accounts)
        ids.add(event_id); signatures.add(signature); previous = event_id; prior_slot = slot
    if seen_kinds != set(EVENT_KINDS):
        refuse("activity event chain does not cover every required lifecycle kind")
    resolution_events = [event for event in events if event["kind"] == "resolution"]
    if tuple(event["operation"] for event in resolution_events) != RESOLUTION_OPERATIONS_V7:
        refuse("Resolution activity is not exact submit, provider execute, Core accept, then reclaim")
    if any(
        decimal(left["slot"], "Resolution slot") >= decimal(right["slot"], "Resolution slot")
        for left, right in zip(resolution_events, resolution_events[1:])
    ):
        refuse("Resolution activity slots are not strictly ordered")
    source_set = [{"event": event["id"], "sha256": event["sourceSha256"]} for event in events]
    if sha256_bytes(canonical_bytes(source_set)) != manifest["sourceSetSha256"]:
        refuse("sourceSetSha256 does not bind the ordered operation evidence")
    semantic_fields = {
        "direct": ("direct", 1, None), "certificate": ("resolution", 1, 1),
        "payout": ("payout", 1, None),
    }
    if cluster_kind == "owned-loopback":
        semantic_fields["feeSettlement"] = ("direct", 1, 1)
    for field, (owner_kind, minimum, maximum) in semantic_fields.items():
        owners = [event for event in events if field in event]
        if len(owners) < minimum or (maximum is not None and len(owners) > maximum) or any(event["kind"] != owner_kind for event in owners):
            refuse(f"activity has an invalid number or owner of {field} facts")
    certificate_owners = [event for event in resolution_events if "certificate" in event]
    if certificate_owners != [resolution_events[1]]:
        refuse("ResolutionCertificateV2 belongs only to provider execute before Core accept")
    if cluster_kind == "owned-loopback":
        hot = [event for event in events if "direct" in event]
        position_sets = [event for event in events if "positions" in event]
        settlements = [event for event in events if "feeSettlement" in event]
        if len(hot) != 1 or position_sets != hot or "position" in hot[0]:
            refuse("owned-loopback Direct must have one Hot owner of its exact two-Position facts")
        if (
            len(settlements) != 1
            or settlements[0]["operation"] != "direct-fee-settlement"
            or events.index(settlements[0]) <= events.index(hot[0])
        ):
            refuse("owned-loopback Direct must have one post-Hot fee-settlement owner")
        if any("position" in event and event["kind"] != "payout" for event in events):
            refuse("owned-loopback singular Position facts belong only to payout")
    for event in events:
        has_economics = event["kind"] in event or (
            event["kind"] == "direct" and "feeSettlement" in event
        )
        if event["kind"] in ("direct", "payout") and event["tokenDeltas"] and not has_economics:
            refuse(f"token-moving {event['kind']} event {event['id']} omitted its exact economic facts")
    if any(("retirement" in event) != (event["kind"] == "retirement") for event in events):
        refuse("every retirement transaction, and no other kind, must own retirement facts")
    final_accounts = manifest["finalAccounts"]
    if not isinstance(final_accounts, list) or len(final_accounts) > MAX_ACCOUNTS:
        refuse("finalAccounts must be a bounded array")
    final_refs: set[str] = set()
    for index, expected in enumerate(final_accounts):
        expected = exact_keys(expected, {"account", "closed"}, {"owner", "lamports", "dataSha256", "mint", "authority", "amountAtoms"}, f"finalAccounts[{index}]")
        ref = text(expected["account"], "final account ref")
        if ref not in accounts or ref in final_refs or not isinstance(expected["closed"], bool):
            refuse("finalAccounts contains an unknown/duplicate ref or invalid closure flag")
        final_refs.add(ref)
        if not expected["closed"]:
            for field in ("owner", "lamports", "dataSha256"):
                if field not in expected:
                    refuse(f"live final account {ref} omitted {field}")
            pubkey(expected["owner"], f"final account {ref} owner")
            decimal(expected["lamports"], f"final account {ref} lamports")
            digest(expected["dataSha256"], f"final account {ref} dataSha256")
            if accounts[ref]["kind"] == "token":
                for field in ("mint", "authority", "amountAtoms"):
                    if field not in expected:
                        refuse(f"final token account {ref} omitted {field}")
                pubkey(expected["mint"], f"final token {ref} mint")
                pubkey(expected["authority"], f"final token {ref} authority")
                decimal(expected["amountAtoms"], f"final token {ref} amountAtoms")
                if expected["mint"] != accounts[ref]["mint"]:
                    refuse(f"final token account {ref} differs from its declared mint")
                if expected["authority"] != accounts[ref]["authority"] or expected["owner"] != accounts[ref]["programOwner"]:
                    refuse(f"final token account {ref} differs from its declared authority or program owner")
    critical_refs = {ref for ref, account in accounts.items() if account["kind"] in ("token", "position", "certificate")}
    if not critical_refs.issubset(final_refs):
        refuse("finalAccounts omits a token, Position, or certificate account")
    return accounts, events


def validate_manifest(manifest: Any) -> tuple[dict[str, dict[str, Any]], list[dict[str, Any]]]:
    return validate_manifest_for(
        manifest,
        manifest_schema=MANIFEST_SCHEMA,
        cluster_kind="devnet",
        genesis_hash=DEVNET_GENESIS_HASH,
    )


def validate_owned_loopback_manifest(
    manifest: Any,
) -> tuple[dict[str, dict[str, Any]], list[dict[str, Any]]]:
    cluster = manifest.get("cluster") if isinstance(manifest, dict) else None
    if not isinstance(cluster, dict):
        refuse("owned-loopback manifest omitted its cluster identity")
    genesis = pubkey(cluster.get("genesisHash"), "owned-loopback manifest genesisHash")
    if genesis in (DEVNET_GENESIS_HASH, MAINNET_GENESIS_HASH):
        refuse("owned-loopback manifest carries a public cluster genesis hash")
    return validate_manifest_for(
        manifest,
        manifest_schema=OWNED_LOOPBACK_MANIFEST_SCHEMA,
        cluster_kind="owned-loopback",
        genesis_hash=genesis,
    )


def authenticate_sources_for(
    manifest: dict[str, Any],
    journal_root: pathlib.Path,
    validator: Any,
) -> None:
    _, events = validator(manifest)
    try:
        root = journal_root.resolve(strict=True)
    except OSError as error:
        refuse(f"cannot resolve journal root {journal_root}: {error}")
    if not root.is_dir():
        refuse("journal root is not a directory")
    observed: dict[str, str] = {}
    for event in events:
        relative = event["sourcePath"]
        if relative not in observed:
            try:
                source = (root / relative).resolve(strict=True)
            except OSError as error:
                refuse(f"cannot resolve source journal {relative}: {error}")
            try:
                source.relative_to(root)
            except ValueError:
                refuse(f"source journal {relative} escapes its journal root")
            if not source.is_file():
                refuse(f"source journal {relative} is not a file")
            raw, _ = read_evidence_file(source)
            observed[relative] = sha256_bytes(raw)
        if observed[relative] != event["sourceSha256"]:
            refuse(f"event {event['id']} source journal digest differs from exact bytes")


def authenticate_sources(manifest: dict[str, Any], journal_root: pathlib.Path) -> None:
    authenticate_sources_for(manifest, journal_root, validate_manifest)


def authenticate_owned_loopback_sources(
    manifest: dict[str, Any], journal_root: pathlib.Path
) -> None:
    authenticate_sources_for(manifest, journal_root, validate_owned_loopback_manifest)


def reconcile_event(
    event: dict[str, Any],
    accounts: dict[str, dict[str, Any]],
    rpc: Any,
    collateral_mint: str,
    cluster_kind: str,
) -> dict[str, Any]:
    result = rpc.transaction(event["signature"])
    if result is None or not isinstance(result, dict):
        refuse(f"finalized transaction {event['signature']} is absent")
    slot = result.get("slot")
    if not isinstance(slot, int) or slot != decimal(event["slot"], "event slot"):
        refuse(f"transaction {event['signature']} landed in a substituted slot")
    meta = result.get("meta")
    if not isinstance(meta, dict) or meta.get("err") is not None:
        refuse(f"transaction {event['signature']} is absent, failed, or not finalized")
    fee = meta.get("fee")
    expected_fee = authenticated_transaction_fee(
        event["feeLamports"], "event fee", cluster_kind
    )
    if (
        not isinstance(fee, int)
        or isinstance(fee, bool)
        or fee < 0
        or fee != expected_fee
        or (cluster_kind == "devnet" and fee == 0)
    ):
        refuse(f"transaction {event['signature']} has a substituted fee")
    compute = meta.get("computeUnitsConsumed")
    if not isinstance(compute, int) or compute != decimal(
        event["computeUnitsConsumed"], "event compute units"
    ):
        refuse(f"transaction {event['signature']} has substituted compute units")
    transaction = result.get("transaction")
    signatures = transaction.get("signatures") if isinstance(transaction, dict) else None
    if not isinstance(signatures, list) or not signatures or signatures[0] != event["signature"]:
        refuse("RPC transaction does not bind its requested first signature")
    keys = account_keys(result)
    if keys[0] != accounts[event["feePayer"]]["address"]:
        refuse(f"transaction {event['signature']} substitutes its fee payer")
    pre = meta.get("preBalances"); post = meta.get("postBalances")
    if not isinstance(pre, list) or not isinstance(post, list) or len(pre) != len(keys) or len(post) != len(keys):
        refuse("transaction lamport balance vectors differ from account keys")
    observed_lamports: dict[str, int] = {}
    observed_lamport_states: dict[str, tuple[int, int]] = {}
    by_address = {account["address"]: ref for ref, account in accounts.items()}
    for index, address in enumerate(keys):
        if not isinstance(pre[index], int) or not isinstance(post[index], int):
            refuse("transaction lamport balance is not an integer")
        delta = post[index] - pre[index]
        if delta:
            ref = by_address.get(address)
            if ref is None:
                refuse(f"transaction changed undeclared lamport account {address}")
            observed_lamports[ref] = delta
            observed_lamport_states[ref] = (pre[index], post[index])
    expected_lamports = parse_delta_list(event["lamportDeltas"], "lamportDeltas", accounts)
    if observed_lamports != expected_lamports:
        refuse(f"event {event['id']} lamport deltas differ from finalized transaction")
    pre_tokens = token_amounts(meta, "preTokenBalances", keys)
    post_tokens = token_amounts(meta, "postTokenBalances", keys)
    observed_tokens: dict[str, int] = {}
    token_mints: dict[str, str] = {}
    observed_token_states: dict[str, tuple[int, int, str, str]] = {}
    for address in set(pre_tokens) | set(post_tokens):
        before = pre_tokens.get(address); after = post_tokens.get(address)
        mint = (before or after)[0]
        if before and after and (before[0] != after[0] or before[1] != after[1]):
            refuse(f"transaction substitutes the mint or owner of token account {address}")
        amount = (after[2] if after else 0) - (before[2] if before else 0)
        if amount:
            ref = by_address.get(address)
            if ref is None or accounts[ref]["kind"] != "token":
                refuse(f"transaction changed undeclared token account {address}")
            observed_tokens[ref] = amount
            token_mints[ref] = mint
            owner = (after or before)[1]
            observed_token_states[ref] = (before[2] if before else 0, after[2] if after else 0, mint, owner)
    expected_tokens = parse_delta_list(event["tokenDeltas"], "tokenDeltas", accounts)
    if observed_tokens != expected_tokens:
        refuse(f"event {event['id']} token deltas differ from finalized transaction")
    if any(
        mint != accounts[ref]["mint"] or observed_token_states[ref][3] != accounts[ref]["authority"]
        for ref, mint in token_mints.items()
    ):
        refuse(f"event {event['id']} substitutes a declared token-account mint or authority")
    projection: dict[str, Any] = {
        "id": event["id"], "kind": event["kind"], "operation": event["operation"], "predecessor": event["predecessor"],
        "signature": event["signature"], "slot": event["slot"], "feePayer": event["feePayer"],
        "transactionFeeLamports": event["feeLamports"], "computeUnitsConsumed": event["computeUnitsConsumed"],
        "lamportDeltas": event["lamportDeltas"],
        "tokenDeltas": event["tokenDeltas"], "sourceSha256": event["sourceSha256"],
        "lamportObservations": [
            {"account": ref, "beforeLamports": str(observed_lamport_states[ref][0]), "afterLamports": str(observed_lamport_states[ref][1]), "deltaLamports": str(observed_lamports[ref])}
            for ref in sorted(observed_lamport_states)
        ],
        "tokenObservations": [
            {"account": ref, "mint": observed_token_states[ref][2], "owner": observed_token_states[ref][3], "beforeAtoms": str(observed_token_states[ref][0]), "afterAtoms": str(observed_token_states[ref][1]), "deltaAtoms": str(observed_tokens[ref])}
            for ref in sorted(observed_token_states)
        ],
    }
    if "direct" in event:
        direct = exact_keys(event.get("direct"), {"fillAtoms", "executionPrice", "priceScale", "feeBasisPointsPerSide", "sellerToken", "buyerToken", "feeRecipientToken", "mint"}, set(), "direct facts")
        fill = decimal(direct["fillAtoms"], "Direct fillAtoms")
        price = decimal(direct["executionPrice"], "Direct executionPrice")
        scale = decimal(direct["priceScale"], "Direct priceScale")
        bps = decimal(direct["feeBasisPointsPerSide"], "Direct fee bps")
        if fill == 0 or scale == 0 or bps != 50:
            refuse("Direct exact fill/scale or 50-bps-per-side policy is invalid")
        refs = [direct["sellerToken"], direct["buyerToken"], direct["feeRecipientToken"]]
        if len(set(refs)) != 3 or any(ref not in accounts or accounts[ref]["kind"] != "token" or accounts[ref]["assetClass"] != "collateral" for ref in refs):
            refuse("Direct seller, buyer, and fee recipient token roles alias or are absent")
        mint = pubkey(direct["mint"], "Direct mint")
        if mint != collateral_mint:
            refuse("Direct mint differs from the lifecycle collateral mint")
        if any(accounts[ref]["mint"] != mint for ref in refs):
            refuse("Direct finalized transaction has mixed collateral mints")
        product = fill * price
        if product % scale:
            refuse("Direct gross quote crosses an unnamed rounding boundary")
        gross = product // scale
        seller_fee = gross * bps // 10_000
        buyer_fee = gross * bps // 10_000
        seller_net = gross - seller_fee
        required = (
            {refs[0]: seller_net, refs[1]: -seller_net}
            if cluster_kind == "owned-loopback"
            else {refs[0]: seller_net, refs[1]: -(gross + buyer_fee), refs[2]: seller_fee + buyer_fee}
        )
        if expected_tokens != required:
            refuse(
                "Direct Hot seller-net movement differs from its exact first-transaction arithmetic"
                if cluster_kind == "owned-loopback"
                else "Direct gross, independent side-floor fees, or token transfers disagree"
            )
        projection["direct"] = {
            **{key: direct[key] for key in ("fillAtoms", "executionPrice", "priceScale", "feeBasisPointsPerSide", "sellerToken", "buyerToken", "feeRecipientToken", "mint")},
            "grossAtoms": str(gross), "sellerFeeAtoms": str(seller_fee), "buyerFeeAtoms": str(buyer_fee),
            "feeRecipientAtoms": str(seller_fee + buyer_fee),
        }
    if "feeSettlement" in event:
        settlement = exact_keys(
            event["feeSettlement"],
            {
                "generation", "debtor", "makerReplay", "feeAtoms", "sourceToken",
                "destinationToken", "destinationOwner", "callerAuthority",
                "callerAuthorityBump", "standingAllowanceAtoms",
                "custodyExpectedRevision", "custodyResultingRevision", "submissionClass",
                "capitalizationClass",
            },
            set(),
            "Direct fee-settlement facts",
        )
        for field in (
            "generation", "feeAtoms", "callerAuthorityBump", "standingAllowanceAtoms",
            "custodyExpectedRevision", "custodyResultingRevision",
        ):
            decimal(settlement[field], f"Direct fee settlement {field}")
        for field in ("debtor", "makerReplay", "destinationOwner", "callerAuthority"):
            pubkey(settlement[field], f"Direct fee settlement {field}")
        fee_atoms = decimal(settlement["feeAtoms"], "Direct fee settlement feeAtoms")
        allowance = decimal(settlement["standingAllowanceAtoms"], "Direct fee settlement standingAllowanceAtoms")
        expected_revision = decimal(settlement["custodyExpectedRevision"], "Direct fee settlement custodyExpectedRevision")
        resulting_revision = decimal(settlement["custodyResultingRevision"], "Direct fee settlement custodyResultingRevision")
        source = settlement["sourceToken"]
        destination = settlement["destinationToken"]
        if (
            fee_atoms == 0
            or decimal(settlement["callerAuthorityBump"], "Direct fee settlement caller authority bump") > 255
            or allowance < fee_atoms
            or resulting_revision != expected_revision + 1
            or source == destination
            or any(ref not in accounts or accounts[ref]["kind"] != "token" for ref in (source, destination))
            or accounts[source]["mint"] != collateral_mint
            or accounts[destination]["mint"] != collateral_mint
            or expected_tokens != {source: -fee_atoms, destination: fee_atoms}
        ):
            refuse("Direct fee settlement changed its exact debt, revision, mint, or token movement")
        if settlement["submissionClass"] != "permissionless-state-derived-stranger":
            refuse("Direct fee settlement lost its permissionless stranger classification")
        if settlement["capitalizationClass"] != "debtor-collateral-obligation-not-future-revenue-or-hoard":
            refuse("Direct fee settlement claims future-revenue or Hoard capitalization")
        projection["feeSettlement"] = settlement
    if "position" in event:
        position = exact_keys(event["position"], {"account", "preDataBase64", "postDataBase64"}, set(), "position facts")
        ref = position["account"]
        if ref not in accounts or accounts[ref]["kind"] != "position":
            refuse("position facts reference a non-Position account")
        before = decode_position(b64(position["preDataBase64"], "position prestate"))
        after = decode_position(b64(position["postDataBase64"], "position poststate"))
        if before["claimCount"] != after["claimCount"] or before["aggregateHex"] != after["aggregateHex"] or before["ownerHex"] != after["ownerHex"] or before["basisHex"] != after["basisHex"] or int(after["revision"]) != int(before["revision"]) + 1:
            refuse("Position transition substitutes identity, geometry, or exact next revision")
        projection["position"] = {"account": ref, "pre": before, "post": after}
    if "positions" in event:
        rows = event["positions"]
        if not isinstance(rows, list) or len(rows) != 2:
            refuse("owned-loopback Direct positions must be exact ordered seller and buyer rows")
        projected_positions = []
        seen_position_refs: set[str] = set()
        for index, raw in enumerate(rows):
            role = ("seller", "buyer")[index]
            position = exact_keys(raw, {"account", "owner", "preDataBase64", "postDataBase64"}, set(), f"Direct {role} Position facts")
            ref = position["account"]
            owner = pubkey(position["owner"], f"Direct {role} Position owner")
            if ref in seen_position_refs or ref not in accounts or accounts[ref]["kind"] != "position":
                refuse("Direct seller and buyer Position facts alias or reference a non-Position account")
            seen_position_refs.add(ref)
            before = decode_position(b64(position["preDataBase64"], f"Direct {role} Position prestate"))
            after = decode_position(b64(position["postDataBase64"], f"Direct {role} Position poststate"))
            if (
                before["claimCount"] != after["claimCount"]
                or before["aggregateHex"] != after["aggregateHex"]
                or before["ownerHex"] != after["ownerHex"]
                or before["basisHex"] != after["basisHex"]
                or before["ownerHex"] != b58decode(owner, f"Direct {role} Position owner").hex()
                or int(after["revision"]) != int(before["revision"]) + 1
            ):
                refuse("Direct Position transition substitutes owner, identity, geometry, or exact next revision")
            projected_positions.append({"role": role, "account": ref, "owner": owner, "pre": before, "post": after})
        fill = decimal(event["direct"]["fillAtoms"], "Direct Position fillAtoms")
        seller = projected_positions[0]
        buyer = projected_positions[1]
        if any(
            seller["pre"][field] != buyer["pre"][field]
            for field in ("claimCount", "aggregateHex", "basisHex")
        ):
            refuse("Direct seller and buyer Position geometry differs")
        seller_deltas = [int(after) - int(before) for before, after in zip(seller["pre"]["balancesAtoms"], seller["post"]["balancesAtoms"], strict=True)]
        buyer_deltas = [int(after) - int(before) for before, after in zip(buyer["pre"]["balancesAtoms"], buyer["post"]["balancesAtoms"], strict=True)]
        changed = [index for index, (seller_delta, buyer_delta) in enumerate(zip(seller_deltas, buyer_deltas, strict=True)) if seller_delta or buyer_delta]
        if len(changed) != 1 or seller_deltas[changed[0]] != -fill or buyer_deltas[changed[0]] != fill:
            refuse("Direct Position transitions do not conserve the exact single-outcome fill")
        projection["positions"] = projected_positions
    if "certificate" in event:
        certificate = exact_keys(event.get("certificate"), {"account", "owner", "dataBase64", "market"}, set(), "certificate facts")
        ref = certificate["account"]
        if ref not in accounts or accounts[ref]["kind"] != "certificate":
            refuse("resolution references a non-certificate account")
        pubkey(certificate["owner"], "certificate owner")
        market = pubkey(certificate["market"], "certificate market")
        decoded = decode_certificate(b64(certificate["dataBase64"], "certificate data"))
        if decoded["marketHex"] != b58decode(market, "certificate market").hex():
            refuse("ResolutionCertificateV2 substitutes its market")
        projection["certificate"] = {"account": ref, "owner": certificate["owner"], "dataSha256": sha256_bytes(b64(certificate["dataBase64"], "certificate data")), **decoded}
    if "payout" in event:
        payout_fields = {
            "hoardToken", "recipientToken", "position", "principalAtoms",
            "claimsBurnedAtoms", "mint",
        }
        if cluster_kind == "owned-loopback":
            payout_fields |= {"holder", "holderChargeClass"}
        payout = exact_keys(event.get("payout"), payout_fields, set(), "payout facts")
        principal = decimal(payout["principalAtoms"], "payout principal")
        hoard = payout["hoardToken"]; recipient = payout["recipientToken"]
        if hoard == recipient or any(ref not in accounts or accounts[ref]["kind"] != "token" or accounts[ref]["assetClass"] != "collateral" for ref in (hoard, recipient)):
            refuse("payout Hoard and recipient token roles alias or are absent")
        mint = pubkey(payout["mint"], "payout mint")
        if token_mints.get(hoard) != mint or token_mints.get(recipient) != mint:
            refuse("payout crosses or omits its immutable collateral mint")
        if expected_tokens.get(hoard) != -principal or expected_tokens.get(recipient) != principal:
            refuse("payout does not conserve exact Hoard principal")
        if cluster_kind == "owned-loopback":
            holder = pubkey(payout["holder"], "payout holder")
            if (
                accounts[recipient]["authority"] != holder
                or accounts[event["feePayer"]]["address"] == holder
                or payout["holderChargeClass"]
                != "terminal-holder-is-not-transaction-fee-payer"
            ):
                refuse("terminal payout charged or substituted its holder")
        burns = payout["claimsBurnedAtoms"]
        if not isinstance(burns, list) or not burns:
            refuse("payout omitted the exact claim burn vector")
        burn_values = [decimal(value, "payout claim burn") for value in burns]
        projected_position = projection.get("position")
        if projected_position is None or payout["position"] != projected_position["account"]:
            refuse("payout omitted its exact Position transition")
        pre_bal = [int(value) for value in projected_position["pre"]["balancesAtoms"]]
        post_bal = [int(value) for value in projected_position["post"]["balancesAtoms"]]
        if len(burn_values) != len(pre_bal) or any(before - after != burn for before, after, burn in zip(pre_bal, post_bal, burn_values)):
            refuse("payout claim burn vector differs from its Position debit")
        projection["payout"] = {**payout, "principalClass": "hoard-principal-not-fee"}
    if event["kind"] == "retirement":
        retirement = exact_keys(
            event.get("retirement"),
            {"stage", "closedAccounts", "refundLamports"},
            {"conservation"},
            "retirement facts",
        )
        text(retirement["stage"], "retirement stage")
        closed = retirement["closedAccounts"]
        if not isinstance(closed, list) or len(closed) != len(set(closed)) or any(ref not in accounts for ref in closed):
            refuse("retirement closure set is duplicate or unknown")
        refunds = parse_delta_list(retirement["refundLamports"], "lamportDeltas", accounts)
        if any(amount <= 0 or expected_lamports.get(ref) != amount for ref, amount in refunds.items()):
            refuse("retirement refund facts differ from finalized lamport deltas")
        projected_retirement = {
            "stage": retirement["stage"],
            "closedAccounts": closed,
            "refundLamports": retirement["refundLamports"],
        }
        if "conservation" in retirement:
            conservation = exact_keys(
                retirement["conservation"],
                {
                    "refundBeneficiary", "payer", "classifiedLamports",
                    "totalTransactionFeesLamports", "terminalRefundWalletLamports",
                    "beneficiaryClass", "capitalizationClass",
                },
                set(),
                "retirement conservation",
            )
            beneficiary = conservation["refundBeneficiary"]
            payer = conservation["payer"]
            if (
                beneficiary not in accounts
                or accounts[beneficiary]["kind"] != "wallet"
                or payer not in accounts
                or accounts[payer]["kind"] != "wallet"
                or payer == beneficiary
            ):
                refuse("retirement conservation changed its distinct payer or fixed beneficiary")
            classified = exact_keys(
                conservation["classifiedLamports"],
                {
                    "market", "rentCredit", "claimsRefund", "custodyReplay",
                    "hoardVaultRent", "expectedRefundDelta", "refundWalletBefore",
                },
                set(),
                "retirement classified lamports",
            )
            values = {
                field: decimal(value, f"retirement classified {field}")
                for field, value in classified.items()
            }
            expected_refund = values["expectedRefundDelta"]
            classified_sum = sum(
                values[field]
                for field in ("market", "rentCredit", "claimsRefund", "custodyReplay", "hoardVaultRent")
            )
            terminal = decimal(
                conservation["terminalRefundWalletLamports"],
                "retirement terminal refund wallet",
            )
            decimal(
                conservation["totalTransactionFeesLamports"],
                "retirement total transaction fees",
            )
            if (
                classified_sum != expected_refund
                or refunds != {beneficiary: expected_refund}
                or terminal != values["refundWalletBefore"] + expected_refund
                or conservation["beneficiaryClass"] != "creation-fixed-refund-wallet"
            ):
                refuse("retirement classified lamports differ from the exact beneficiary refund")
            if conservation["capitalizationClass"] != "historical-account-lamports-not-future-revenue-or-hoard-principal":
                refuse("retirement conservation claims future-revenue or Hoard-principal capitalization")
            projected_retirement["conservation"] = conservation
        projection["retirement"] = projected_retirement
    return projection


def reconcile_final_accounts(manifest: dict[str, Any], accounts: dict[str, dict[str, Any]], rpc: Any, floor: int) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    by_ref = {entry["account"]: entry for entry in manifest["finalAccounts"]}
    retired = {
        ref
        for event in manifest["events"] if event["kind"] == "retirement"
        for ref in event["retirement"]["closedAccounts"]
    }
    if retired - {ref for ref, expected in by_ref.items() if expected["closed"]}:
        refuse("retirement closure set lacks a finalized vacant account observation")
    for ref in sorted(by_ref):
        expected = by_ref[ref]
        slot, value = rpc.account(accounts[ref]["address"])
        if slot < floor:
            refuse(f"final account {ref} observation predates the activity")
        if expected["closed"]:
            if value is not None:
                refuse(f"retired account {ref} is not vacant at finalized")
            out.append({"account": ref, "address": accounts[ref]["address"], "closed": True, "observedSlot": str(slot)})
            continue
        value = exact_keys(value, {"lamports", "owner", "data", "executable", "rentEpoch"}, {"space"}, f"final account {ref}")
        data = account_data(value["data"], f"final account {ref} data")
        if value["owner"] != expected["owner"] or value["lamports"] != decimal(expected["lamports"], "final lamports") or sha256_bytes(data) != expected["dataSha256"]:
            refuse(f"final account {ref} differs in owner, lamports, or exact data digest")
        item: dict[str, Any] = {"account": ref, "address": accounts[ref]["address"], "closed": False, "owner": value["owner"], "lamports": str(value["lamports"]), "dataSha256": expected["dataSha256"], "observedSlot": str(slot)}
        if accounts[ref]["kind"] == "token":
            decoded = decode_token_account(data)
            if decoded["mintBytes"] != b58decode(expected["mint"], "final token mint") or decoded["authorityBytes"] != b58decode(expected["authority"], "final token authority") or decoded["amountAtoms"] != decimal(expected["amountAtoms"], "final token amount"):
                refuse(f"final token account {ref} substitutes mint, authority, or atoms")
            item.update({"mint": expected["mint"], "authority": expected["authority"], "amountAtoms": expected["amountAtoms"]})
        elif accounts[ref]["kind"] == "position":
            item["position"] = decode_position(data)
        elif accounts[ref]["kind"] == "certificate":
            item["certificate"] = decode_certificate(data)
        out.append(item)
    return out


def reconcile_for(
    manifest: dict[str, Any],
    rpc: Any,
    *,
    validator: Any,
    cluster_kind: str,
    dossier_schema: str,
    expected_genesis: str,
    session_evidence: dict[str, Any] | None = None,
) -> dict[str, Any]:
    accounts, events = validator(manifest)
    direct_events = [event for event in events if "direct" in event]
    collateral_mint = pubkey(direct_events[0]["direct"]["mint"], "lifecycle collateral mint")
    for direct_event in direct_events:
        if direct_event["direct"]["mint"] != collateral_mint:
            refuse("Direct events disagree on the lifecycle collateral mint")
        for ref in (direct_event["direct"]["sellerToken"], direct_event["direct"]["buyerToken"], direct_event["direct"]["feeRecipientToken"]):
            if ref not in accounts or accounts[ref].get("mint") != collateral_mint:
                refuse("Direct account inventory differs from the lifecycle collateral mint")
    payout_events = [event for event in events if "payout" in event]
    for payout_event in payout_events:
        if payout_event["payout"]["mint"] != collateral_mint:
            refuse("payout events disagree on the lifecycle collateral mint")
        for ref in (payout_event["payout"]["hoardToken"], payout_event["payout"]["recipientToken"]):
            if ref not in accounts or accounts[ref].get("mint") != collateral_mint:
                refuse("payout account inventory differs from the lifecycle collateral mint")
    genesis = rpc.genesis_hash()
    if genesis != expected_genesis:
        if cluster_kind == "devnet":
            refuse(f"RPC genesis {genesis!r} is not exact Solana devnet")
        refuse(f"RPC genesis {genesis!r} is not exact owned-loopback genesis")
    projections = [
        reconcile_event(event, accounts, rpc, collateral_mint, cluster_kind)
        for event in events
    ]
    last_lamports: dict[str, int] = {}
    last_tokens: dict[str, int] = {}
    last_positions: dict[str, dict[str, Any]] = {}
    for event in projections:
        for observation in event["lamportObservations"]:
            ref = observation["account"]
            before = int(observation["beforeLamports"])
            if ref in last_lamports and last_lamports[ref] != before:
                refuse(f"activity lamport history for {ref} is discontinuous")
            last_lamports[ref] = int(observation["afterLamports"])
        for observation in event["tokenObservations"]:
            ref = observation["account"]
            before = int(observation["beforeAtoms"])
            if ref in last_tokens and last_tokens[ref] != before:
                refuse(f"activity token history for {ref} is discontinuous")
            last_tokens[ref] = int(observation["afterAtoms"])
        if "position" in event:
            ref = event["position"]["account"]
            if ref in last_positions and last_positions[ref] != event["position"]["pre"]:
                refuse(f"activity Position history for {ref} is discontinuous")
            last_positions[ref] = event["position"]["post"]
        for position in event.get("positions", []):
            ref = position["account"]
            if ref in last_positions and last_positions[ref] != position["pre"]:
                refuse(f"activity Position history for {ref} is discontinuous")
            last_positions[ref] = position["post"]
    final = reconcile_final_accounts(manifest, accounts, rpc, max(int(event["slot"]) for event in events))
    retirement_conservation = [
        event["retirement"]["conservation"]
        for event in projections
        if "conservation" in event.get("retirement", {})
    ]
    if cluster_kind == "owned-loopback":
        if len(retirement_conservation) != 1:
            refuse("owned-loopback retirement omitted one terminal conservation owner")
        conservation = retirement_conservation[0]
        payer = conservation["payer"]
        beneficiary = conservation["refundBeneficiary"]
        retirement_events = [event for event in projections if event["kind"] == "retirement"]
        if any(event["feePayer"] != payer for event in retirement_events):
            refuse("retirement transaction fee payer differs from its conservation receipt")
        final_beneficiary = next(
            (row for row in final if row["account"] == beneficiary), None
        )
        if (
            final_beneficiary is None
            or final_beneficiary["closed"]
            or final_beneficiary["lamports"] != conservation["terminalRefundWalletLamports"]
        ):
            refuse("creation-fixed retirement beneficiary differs from its terminal poststate")
    for observed in final:
        ref = observed["account"]
        if observed["closed"]:
            continue
        if ref in last_tokens and int(observed["amountAtoms"]) != last_tokens[ref]:
            refuse(f"final token account {ref} advanced outside the activity chain")
        if ref in last_positions and observed.get("position") != last_positions[ref]:
            refuse(f"final Position {ref} advanced outside the activity chain")
    transaction_fees = sum(int(event["feeLamports"]) for event in events)
    compute_units = sum(int(event["computeUnitsConsumed"]) for event in events)
    directs = [item["direct"] for item in projections if "direct" in item]
    settlements = [item["feeSettlement"] for item in projections if "feeSettlement" in item]
    if cluster_kind == "owned-loopback":
        if len(directs) != 1 or len(settlements) != 1:
            refuse("owned-loopback activity omitted the exact Direct Hot/fee-settlement pair")
        direct = directs[0]
        settlement = settlements[0]
        if (
            settlement["sourceToken"] != direct["buyerToken"]
            or settlement["destinationToken"] != direct["feeRecipientToken"]
            or int(settlement["feeAtoms"]) != int(direct["feeRecipientAtoms"])
        ):
            refuse("Direct fee settlement differs from the Hot transaction's exact obligation")
    payouts = [item["payout"] for item in projections if "payout" in item]
    source_digests = [{"event": event["id"], "sha256": event["sourceSha256"]} for event in events]
    evidence_core = {
        "manifestSha256": sha256_bytes(canonical_bytes(manifest)),
        "sourceDigests": source_digests,
        "rpc": rpc.provenance(),
    }
    if session_evidence is not None:
        evidence_core["ownedLoopbackSession"] = session_evidence
    dossier: dict[str, Any] = {
        "schema": dossier_schema, "signatureScheme": "none", "activityId": manifest["activityId"],
        "cluster": {"kind": cluster_kind, "genesisHash": genesis}, "evidence": evidence_core,
        "accounts": manifest["accounts"], "events": projections, "finalAccounts": final,
        "totals": {
            "transactionFeesLamports": str(transaction_fees),
            "computeUnitsConsumed": str(compute_units),
            "protocolFeesAtoms": str(
                sum(int(row["feeAtoms"]) for row in settlements)
                if cluster_kind == "owned-loopback"
                else sum(int(row["feeRecipientAtoms"]) for row in directs)
            ),
            "hoardPrincipalPaidAtoms": str(sum(int(payout["principalAtoms"]) for payout in payouts)),
            "hoardPrincipalClassification": "collateral-principal-not-fee-bounty-rent-reserve-or-treasury",
        },
    }
    dossier["dossierSha256"] = sha256_bytes(canonical_bytes(dossier))
    return dossier


def reconcile(manifest: dict[str, Any], rpc: Any) -> dict[str, Any]:
    return reconcile_for(
        manifest,
        rpc,
        validator=validate_manifest,
        cluster_kind="devnet",
        dossier_schema=DOSSIER_SCHEMA,
        expected_genesis=DEVNET_GENESIS_HASH,
    )


def reconcile_owned_loopback(
    manifest: dict[str, Any],
    rpc: OwnedLoopbackCapturedRpc,
    session_evidence: dict[str, Any],
) -> dict[str, Any]:
    genesis = manifest.get("cluster", {}).get("genesisHash")
    return reconcile_for(
        manifest,
        rpc,
        validator=validate_owned_loopback_manifest,
        cluster_kind="owned-loopback",
        dossier_schema=OWNED_LOOPBACK_DOSSIER_SCHEMA,
        expected_genesis=genesis,
        session_evidence=session_evidence,
    )


def canonical_relative_evidence(
    root: pathlib.Path, relative: Any, label: str
) -> tuple[pathlib.Path, str]:
    value = text(relative, f"{label} path")
    parts = pathlib.PurePosixPath(value)
    if parts.is_absolute() or value != parts.as_posix() or any(
        part in ("", ".", "..") for part in parts.parts
    ):
        refuse(f"{label} path is not canonical relative evidence")
    candidate = root / value
    try:
        candidate.lstat()
        if candidate.is_symlink() or not candidate.is_file():
            refuse(f"{label} is not one regular non-symlink evidence file")
        path = candidate.resolve(strict=True)
        path.relative_to(root)
    except (OSError, ValueError) as error:
        refuse(f"{label} path escapes or is absent: {error}")
    return path, value


def json_pointer(value: Any, pointer: Any, label: str) -> Any:
    path = text(pointer, f"{label} completionPointer")
    if not path.startswith("/"):
        refuse(f"{label} completionPointer is not canonical RFC6901")
    current = value
    for raw in path[1:].split("/"):
        if "~" in raw:
            index = 0
            while index < len(raw):
                if raw[index] == "~" and (index + 1 >= len(raw) or raw[index + 1] not in "01"):
                    refuse(f"{label} completionPointer has an invalid RFC6901 escape")
                index += 2 if raw[index] == "~" else 1
        part = raw.replace("~1", "/").replace("~0", "~")
        if isinstance(current, dict) and part in current:
            current = current[part]
        elif isinstance(current, list) and part.isdigit() and (part == "0" or not part.startswith("0")) and int(part) < len(current):
            current = current[int(part)]
        else:
            refuse(f"{label} completionPointer {path} is absent")
    return current


def canonical_directory(root: pathlib.Path, value: Any, label: str) -> pathlib.Path:
    path = pathlib.Path(text(value, label))
    try:
        resolved = path.resolve(strict=True)
        metadata = resolved.lstat()
    except OSError as error:
        refuse(f"{label} is absent: {error}")
    if path != resolved or not resolved.is_dir() or resolved.is_symlink():
        refuse(f"{label} is not one canonical ordinary directory")
    try:
        resolved.relative_to(root)
    except ValueError:
        refuse(f"{label} escapes the owned-loopback evidence root")
    return resolved


def canonical_file(root: pathlib.Path, value: Any, label: str) -> pathlib.Path:
    path = pathlib.Path(text(value, label))
    try:
        resolved = path.resolve(strict=True)
        metadata = path.lstat()
    except OSError as error:
        refuse(f"{label} is absent: {error}")
    if path != resolved or not path.is_file() or path.is_symlink():
        refuse(f"{label} is not one canonical ordinary file")
    try:
        resolved.relative_to(root)
    except ValueError:
        refuse(f"{label} escapes the owned-loopback evidence root")
    return resolved


def terminal_mutation(value: Any, label: str) -> tuple[str, int | None]:
    row = exact_keys(value, {"kind"}, {"prefixLen"}, label)
    kind = text(row["kind"], f"{label} kind")
    if kind == "lookup-extend":
        if set(row) != {"kind", "prefixLen"}:
            refuse(f"{label} lookup extension omitted its exact prefixLen")
        prefix = decimal(row["prefixLen"], f"{label} prefixLen")
        if prefix == 0:
            refuse(f"{label} prefixLen is zero")
        return kind, prefix
    if set(row) != {"kind"}:
        refuse(f"{label} non-extension mutation carries prefixLen")
    admitted = {
        "lookup-create",
        "lookup-freeze",
        "resolution-receipt-prepay",
        "core-begin-retiring",
        "direct-begin-retiring",
        "resolution-close-fund",
        "direct-close-capability",
        "retirement-replay-handoff",
        "aggregate-retirement",
    }
    if kind not in admitted:
        refuse(f"{label} has an unknown mutation kind")
    return kind, None


def persisted_terminal_mutation(value: Any, label: str) -> tuple[str, int | None]:
    row = exact_keys(value, {"kind"}, {"prefixLen", "stage"}, label)
    kind = text(row["kind"], f"{label} kind")
    if kind == "lookup-extend":
        if set(row) != {"kind", "prefixLen"} or not isinstance(row["prefixLen"], int):
            refuse(f"{label} lookup extension has another prefix")
        if isinstance(row["prefixLen"], bool) or row["prefixLen"] <= 0:
            refuse(f"{label} lookup extension prefix is not positive")
        return kind, row["prefixLen"]
    if kind == "protocol":
        if set(row) != {"kind", "stage"}:
            refuse(f"{label} protocol mutation omitted its stage")
        stage = text(row["stage"], f"{label} stage")
        admitted = {
            "core-begin-retiring",
            "direct-begin-retiring",
            "resolution-close-fund",
            "direct-close-capability",
            "retirement-replay-handoff",
            "aggregate-retirement",
        }
        if stage not in admitted:
            refuse(f"{label} has another protocol stage")
        return stage, None
    if set(row) != {"kind"} or kind not in {
        "lookup-create", "lookup-freeze", "resolution-receipt-prepay"
    }:
        refuse(f"{label} has another durable mutation")
    return kind, None


def authenticate_terminal_completion(
    source_path: pathlib.Path,
    source: dict[str, Any],
    evidence_root: pathlib.Path,
    genesis: str,
    capture_slot: int,
) -> dict[str, Any]:
    source = exact_keys(
        source,
        {
            "schema", "status", "cluster", "genesisHash", "invocation", "session",
            "journalDirectory", "market", "payer", "lookupTable", "journals",
            "finalizedSlot", "transactionFeesLamports", "computeUnitsConsumed",
        },
        set(),
        "owned-loopback terminal completion",
    )
    if (
        source["schema"] != OWNED_LOOPBACK_TERMINAL_COMPLETION_SCHEMA
        or source["status"] != "finalized"
        or source["cluster"] != "owned-loopback"
        or source["genesisHash"] != genesis
    ):
        refuse("terminal completion names another schema, phase, or cluster")
    market = pubkey(source["market"], "terminal completion market")
    payer = pubkey(source["payer"], "terminal completion payer")
    lookup_table = pubkey(source["lookupTable"], "terminal completion lookup table")
    invocation = exact_keys(
        source["invocation"],
        {
            "command", "rpcUrl", "planPath", "marketInputPath", "evidencePath",
            "market", "feePayer", "feePayerKeypairPath", "sessionPath",
            "journalDirectory", "completionPath", "suppliedLookupTable", "execute",
        },
        set(),
        "terminal completion invocation",
    )
    if (
        invocation["command"] != "local-private-validator-terminal-sequence-v1"
        or invocation["market"] != market
        or invocation["feePayer"] != payer
        or invocation["execute"] is not True
    ):
        refuse("terminal completion invocation is not the exact executed owned-loopback command")
    text(invocation["rpcUrl"], "terminal completion RPC URL")
    for field in ("planPath", "marketInputPath", "evidencePath", "feePayerKeypairPath"):
        text(invocation[field], f"terminal completion invocation {field}")
    completion_path = canonical_file(
        evidence_root, invocation["completionPath"], "terminal completion invocation output"
    )
    if completion_path != source_path.resolve(strict=True):
        refuse("terminal completion invocation names another completion output")
    journal_directory = canonical_directory(
        evidence_root,
        invocation["journalDirectory"],
        "terminal completion invocation journal directory",
    )
    journal_relative = text(source["journalDirectory"], "terminal completion journalDirectory")
    relative_parts = pathlib.PurePosixPath(journal_relative)
    if relative_parts.is_absolute() or journal_relative != relative_parts.as_posix():
        refuse("terminal completion journalDirectory is not canonical relative evidence")
    if (evidence_root / journal_relative).resolve(strict=True) != journal_directory:
        refuse("terminal completion journalDirectory projection changed")
    supplied = invocation["suppliedLookupTable"]
    if supplied is not None and pubkey(supplied, "supplied terminal lookup table") != lookup_table:
        refuse("terminal completion supplied another lookup table")

    session_ref = exact_keys(
        source["session"],
        {"path", "sha256", "schema", "sessionSha256"},
        set(),
        "terminal completion session",
    )
    session_path, session_relative = canonical_relative_evidence(
        evidence_root, session_ref["path"], "terminal completion session"
    )
    if canonical_file(evidence_root, invocation["sessionPath"], "terminal invocation session") != session_path:
        refuse("terminal completion invocation names another session")
    session_raw, session = read_evidence_file(session_path)
    if (
        digest(session_ref["sha256"], "terminal session sha256") != sha256_bytes(session_raw)
        or session_ref["schema"] != OWNED_LOOPBACK_TERMINAL_SESSION_SCHEMA
        or session.get("schema") != session_ref["schema"]
        or digest(session_ref["sessionSha256"], "terminal session internal digest")
        != session.get("sessionSha256")
    ):
        refuse("terminal completion session is missing or substituted")

    rows = source["journals"]
    if not isinstance(rows, list) or not rows or len(rows) > 64:
        refuse("terminal completion has no bounded journal sequence")
    seen_paths: set[str] = set()
    seen_signatures: set[str] = set()
    mutations: list[tuple[str, int | None]] = []
    projected: list[dict[str, Any]] = []
    total_fees = 0
    total_compute = 0
    prior_slot = 0
    max_slot = 0
    for index, raw_row in enumerate(rows):
        row = exact_keys(
            raw_row,
            {
                "path", "sha256", "schema", "mutation", "phase", "feePayer",
                "signature", "finalizedSlot", "computeUnitsConsumed",
                "transactionFeeLamports", "protocolLamportDeltas",
            },
            set(),
            f"terminal completion journal {index}",
        )
        path, relative = canonical_relative_evidence(
            evidence_root, row["path"], f"terminal completion journal {index}"
        )
        try:
            path.relative_to(journal_directory)
        except ValueError:
            refuse("terminal completion journal escaped its journalDirectory")
        raw, persisted = read_evidence_file(path)
        signature = text(row["signature"], f"terminal completion journal {index} signature")
        if len(b58decode(signature, f"terminal completion journal {index} signature")) != 64:
            refuse("terminal completion journal signature is not 64 bytes")
        if (
            relative in seen_paths
            or signature in seen_signatures
            or digest(row["sha256"], f"terminal completion journal {index} sha256") != sha256_bytes(raw)
            or row["schema"] != OWNED_LOOPBACK_TERMINAL_JOURNAL_SCHEMA
            or persisted.get("schema") != row["schema"]
            or row["phase"] != "finalized"
            or persisted.get("phase") != "finalized"
            or pubkey(row["feePayer"], f"terminal completion journal {index} fee payer") != payer
        ):
            refuse("terminal completion journal identity, bytes, phase, or payer changed")
        seen_paths.add(relative)
        seen_signatures.add(signature)
        slot = decimal(row["finalizedSlot"], f"terminal completion journal {index} slot")
        compute = decimal(row["computeUnitsConsumed"], f"terminal completion journal {index} CU")
        fee = decimal(row["transactionFeeLamports"], f"terminal completion journal {index} fee")
        if slot == 0 or compute == 0 or slot < prior_slot or slot > capture_slot:
            refuse("terminal completion journal has invalid/regressing/out-of-capture execution")
        prior_slot = slot
        max_slot = max(max_slot, slot)
        total_fees += fee
        total_compute += compute
        if total_fees > 2**64 - 1 or total_compute > 2**64 - 1:
            refuse("terminal completion aggregate arithmetic exceeds u64")
        deltas = row["protocolLamportDeltas"]
        if not isinstance(deltas, list):
            refuse("terminal completion protocolLamportDeltas is not a list")
        prior_address = ""
        delta_sum = 0
        projected_deltas: list[dict[str, str]] = []
        for delta_index, raw_delta in enumerate(deltas):
            delta = exact_keys(
                raw_delta,
                {"accountAddress", "deltaLamports"},
                set(),
                f"terminal completion journal {index} delta {delta_index}",
            )
            address = pubkey(delta["accountAddress"], "terminal delta account")
            amount = decimal(delta["deltaLamports"], "terminal delta lamports", signed=True)
            if prior_address >= address:
                refuse("terminal completion deltas are not unique canonical address order")
            prior_address = address
            delta_sum += amount
            projected_deltas.append({"accountAddress": address, "deltaLamports": str(amount)})
        if delta_sum != 0:
            refuse("terminal completion protocol deltas do not conserve before fee")
        mutation = terminal_mutation(row["mutation"], f"terminal completion journal {index} mutation")
        intent = exact_keys(
            persisted.get("intent"),
            {"mutation", "payer", "transactionFeeLamports", "protocolLamportDeltas"},
            {
                "observationSlot", "observationUnixTimestamp", "programId", "programClass",
                "accounts", "instructionDataBase64", "instructionDataSha256", "lookupTable",
                "lookupTableAddresses", "lookupTableAddressesSha256", "loadedWritable",
                "loadedReadonly", "resolvedAccountKeys", "preBalances", "postBalances",
                "recentBlockhash", "lastValidBlockHeight", "wireBytes", "messageBase64",
                "messageSha256", "prestate", "expectedAccounts", "expectedReturnData",
            },
            f"terminal persisted journal {index} intent",
        )
        finalized = exact_keys(
            persisted.get("finalized"),
            {"signature", "slot", "feeLamports", "computeUnitsConsumed", "packetSha256", "poststate"},
            set(),
            f"terminal persisted journal {index} finalization",
        )
        persisted_deltas = intent["protocolLamportDeltas"]
        if not isinstance(persisted_deltas, dict):
            refuse("terminal persisted protocolLamportDeltas is not an object")
        projected_persisted_deltas: list[dict[str, str]] = []
        for address, amount in persisted_deltas.items():
            pubkey(address, "terminal persisted delta account")
            if not isinstance(amount, int) or isinstance(amount, bool):
                refuse("terminal persisted delta is not an integer")
            projected_persisted_deltas.append(
                {"accountAddress": address, "deltaLamports": str(amount)}
            )
        if (
            persisted_terminal_mutation(
                intent["mutation"], f"terminal persisted journal {index} mutation"
            )
            != mutation
            or intent["payer"] != payer
            or intent["transactionFeeLamports"] != fee
            or projected_persisted_deltas != projected_deltas
            or finalized["signature"] != signature
            or finalized["slot"] != slot
            or finalized["feeLamports"] != fee
            or finalized["computeUnitsConsumed"] != compute
        ):
            refuse("terminal completion row differs from its persisted semantic-owner journal")
        mutations.append(mutation)
        projected.append(
            {
                "path": relative,
                "sha256": row["sha256"],
                "mutation": row["mutation"],
                "signature": signature,
                "finalizedSlot": str(slot),
                "computeUnitsConsumed": str(compute),
                "transactionFeeLamports": str(fee),
                "protocolLamportDeltas": projected_deltas,
            }
        )

    index = 0
    if supplied is None:
        if mutations[index] != ("lookup-create", None):
            refuse("terminal completion omitted lookup creation")
        index += 1
        prior_prefix = 0
        while index < len(mutations) and mutations[index][0] == "lookup-extend":
            prefix = mutations[index][1]
            assert prefix is not None
            if prefix <= prior_prefix:
                refuse("terminal completion lookup prefixes are not strictly increasing")
            prior_prefix = prefix
            index += 1
        if prior_prefix == 0 or index >= len(mutations) or mutations[index] != ("lookup-freeze", None):
            refuse("terminal completion omitted lookup extension/freeze")
        index += 1
    elif any(kind.startswith("lookup-") for kind, _ in mutations):
        refuse("terminal completion mutated a supplied lookup table")
    if index < len(mutations) and mutations[index] == ("resolution-receipt-prepay", None):
        index += 1
    required_tail = [
        ("core-begin-retiring", None),
        ("direct-begin-retiring", None),
        ("resolution-close-fund", None),
        ("direct-close-capability", None),
        ("retirement-replay-handoff", None),
        ("aggregate-retirement", None),
    ]
    if mutations[index:] != required_tail:
        refuse("terminal completion mutation sequence is partial or noncanonical")
    if (
        decimal(source["finalizedSlot"], "terminal completion finalizedSlot") != max_slot
        or decimal(source["transactionFeesLamports"], "terminal completion fee total") != total_fees
        or decimal(source["computeUnitsConsumed"], "terminal completion CU total") != total_compute
    ):
        refuse("terminal completion aggregate slot, fee, or CU arithmetic changed")
    return {
        "schema": OWNED_LOOPBACK_TERMINAL_COMPLETION_SCHEMA,
        "sha256": sha256_bytes(source_path.read_bytes()),
        "sessionPath": session_relative,
        "market": market,
        "payer": payer,
        "lookupTable": lookup_table,
        "finalizedSlot": str(max_slot),
        "transactionFeesLamports": str(total_fees),
        "computeUnitsConsumed": str(total_compute),
        "journals": projected,
    }


def authenticate_loader_v3_program(
    rpc_capture: OwnedLoopbackCapturedRpc,
    role: str,
    program_id: str,
    programdata_id: str,
    deployment_slot: int,
    elf_sha256: str,
    genesis_programdata_sha256: str,
    upgrade_authority: str | None,
) -> None:
    program_slot, program_value = rpc_capture.account(program_id)
    programdata_slot, programdata_value = rpc_capture.account(programdata_id)
    if program_slot < deployment_slot or programdata_slot < deployment_slot:
        refuse(f"{role} Loader identities were observed before their deployment slot")
    program = exact_keys(
        program_value,
        {"lamports", "owner", "data", "executable", "rentEpoch"},
        {"space"},
        f"{role} Program account",
    )
    programdata = exact_keys(
        programdata_value,
        {"lamports", "owner", "data", "executable", "rentEpoch"},
        {"space"},
        f"{role} ProgramData account",
    )
    if (
        program["owner"] != LOADER_V3_PROGRAM_ID
        or program.get("executable") is not True
        or not isinstance(program.get("lamports"), int)
        or program["lamports"] <= 0
        or programdata["owner"] != LOADER_V3_PROGRAM_ID
        or programdata.get("executable") is not False
        or not isinstance(programdata.get("lamports"), int)
        or programdata["lamports"] <= 0
    ):
        refuse(f"{role} Program/ProgramData Loader owner, privilege, or funding differs")
    program_bytes = account_data(program["data"], f"{role} Program data")
    programdata_bytes = account_data(programdata["data"], f"{role} ProgramData data")
    if (
        len(program_bytes) != 36
        or struct.unpack_from("<I", program_bytes, 0)[0] != 2
        or program_bytes[4:36] != b58decode(programdata_id, f"{role} ProgramData link")
    ):
        refuse(f"{role} Program does not carry its exact Loader-v3 ProgramData link")
    authority_tag = programdata_bytes[12] if len(programdata_bytes) > 12 else -1
    authority_matches = (
        authority_tag == 0
        if upgrade_authority is None
        else (
            authority_tag == 1
            and programdata_bytes[13:45]
            == b58decode(upgrade_authority, f"{role} upgrade authority")
        )
    )
    if (
        len(programdata_bytes) <= 45
        or struct.unpack_from("<I", programdata_bytes, 0)[0] != 3
        or not authority_matches
        or u64(programdata_bytes, 4) != deployment_slot
        or sha256_bytes(programdata_bytes) != genesis_programdata_sha256
        or sha256_bytes(programdata_bytes[45:]) != elf_sha256
    ):
        refuse(f"{role} ProgramData slot, authority, genesis bytes, or exact ELF tail differs")


def canonical_absolute_source(value: Any, label: str) -> pathlib.Path:
    path = pathlib.Path(text(value, f"{label} path"))
    try:
        path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        refuse(f"{label} is absent: {error}")
    if not path.is_absolute() or path != resolved or path.is_symlink() or not path.is_file():
        refuse(f"{label} is not one canonical absolute regular non-symlink file")
    return resolved


def authenticate_provider_source_ref(
    value: Any, expected_schema: str, label: str
) -> dict[str, str]:
    reference = exact_keys(value, {"path", "sha256", "schema"}, set(), label)
    path = canonical_absolute_source(reference["path"], label)
    raw, source = read_evidence_file(path)
    source_sha256 = digest(reference["sha256"], f"{label} sha256")
    if (
        reference["schema"] != expected_schema
        or source.get("schema") != expected_schema
        or source_sha256 != sha256_bytes(raw)
    ):
        refuse(f"{label} source bytes, digest, or schema differ")
    return {"path": str(path), "sha256": source_sha256, "schema": expected_schema}


def authenticate_owned_loopback_provider_closure(
    reference_value: Any,
    evidence_root: pathlib.Path,
    capture_path: pathlib.Path,
    rpc_capture: OwnedLoopbackCapturedRpc,
    genesis_hash: str,
    programs: list[dict[str, Any]],
) -> dict[str, Any]:
    reference = exact_keys(
        reference_value,
        {"path", "sha256", "schema"},
        set(),
        "owned-loopback provider closure reference",
    )
    closure_path, closure_relative = canonical_relative_evidence(
        evidence_root, reference["path"], "provider closure"
    )
    closure_raw, closure = read_evidence_file(closure_path)
    closure_sha256 = digest(reference["sha256"], "provider closure sha256")
    if (
        reference["schema"] != OWNED_LOOPBACK_PROVIDER_CLOSURE_SCHEMA
        or closure_sha256 != sha256_bytes(closure_raw)
    ):
        refuse("owned-loopback provider closure is missing, substituted, or another schema")
    closure = exact_keys(
        closure,
        {
            "schema", "cluster", "genesisHash", "status", "finalizedObservationSlot",
            "plan", "localValidatorProfile", "finalizedCapture", "providerPrograms",
        },
        set(),
        "owned-loopback provider closure",
    )
    if (
        closure["schema"] != OWNED_LOOPBACK_PROVIDER_CLOSURE_SCHEMA
        or closure["cluster"] != "owned-loopback"
        or closure["status"] != "finalized"
        or pubkey(closure["genesisHash"], "provider closure genesisHash") != genesis_hash
        or decimal(
            closure["finalizedObservationSlot"], "provider closure finalizedObservationSlot"
        ) != rpc_capture.finalized_slot
    ):
        refuse("owned-loopback provider closure is provisional or names another capture boundary")

    plan = authenticate_provider_source_ref(
        closure["plan"], OWNED_LOOPBACK_PROVIDER_PLAN_SCHEMA, "provider closure plan"
    )
    profile = authenticate_provider_source_ref(
        closure["localValidatorProfile"],
        OWNED_LOOPBACK_PROVIDER_PROFILE_SCHEMA,
        "provider closure local-validator profile",
    )
    if plan["path"] == profile["path"]:
        refuse("provider closure aliases its plan and local-validator profile")

    capture = exact_keys(
        closure["finalizedCapture"],
        {"path", "sha256", "schema", "finalizedSlot"},
        set(),
        "provider closure finalized capture",
    )
    closure_capture_path = canonical_absolute_source(
        capture["path"], "provider closure finalized capture"
    )
    try:
        supplied_capture_path = capture_path.resolve(strict=True)
    except OSError as error:
        refuse(f"cannot resolve supplied owned-loopback capture: {error}")
    if (
        closure_capture_path != supplied_capture_path
        or digest(capture["sha256"], "provider closure capture sha256")
        != rpc_capture.capture_sha256
        or capture["schema"] != OWNED_LOOPBACK_CAPTURE_SCHEMA
        or decimal(capture["finalizedSlot"], "provider closure capture finalizedSlot")
        != rpc_capture.finalized_slot
    ):
        refuse("provider closure substitutes the singular finalized capture")

    provider_programs = closure["providerPrograms"]
    if not isinstance(provider_programs, list) or len(provider_programs) != 2:
        refuse("provider closure omits the exact Receiver and Router rows")
    projected: list[dict[str, Any]] = []
    for index, (raw_program, expected) in enumerate(
        zip(provider_programs, programs[-2:], strict=True)
    ):
        row = exact_keys(
            raw_program,
            {
                "role", "programId", "programDataAddress", "deploymentSlot", "elfSha256",
                "genesisProgramDataSha256", "upgradeAuthority",
            },
            set(),
            f"provider closure program {index}",
        )
        normalized = {
            "role": text(row["role"], f"provider closure program {index} role"),
            "programId": pubkey(row["programId"], f"provider closure program {index} programId"),
            "programDataAddress": pubkey(
                row["programDataAddress"], f"provider closure program {index} programDataAddress"
            ),
            "deploymentSlot": str(
                decimal(row["deploymentSlot"], f"provider closure program {index} deploymentSlot")
            ),
            "elfSha256": digest(row["elfSha256"], f"provider closure program {index} elfSha256"),
            "genesisProgramDataSha256": digest(
                row["genesisProgramDataSha256"],
                f"provider closure program {index} genesisProgramDataSha256",
            ),
            "upgradeAuthority": row["upgradeAuthority"],
        }
        if normalized["upgradeAuthority"] is not None or normalized != expected:
            refuse("provider closure rows differ from the captured immutable provider programs")
        projected.append(normalized)

    return {
        "path": closure_relative,
        "sha256": closure_sha256,
        "schema": OWNED_LOOPBACK_PROVIDER_CLOSURE_SCHEMA,
        "finalizedObservationSlot": str(rpc_capture.finalized_slot),
        "plan": plan,
        "localValidatorProfile": profile,
        "providerPrograms": projected,
    }


def authenticate_owned_loopback_private_session(
    session_relative: str,
    session_raw: bytes,
    session_value: Any,
    receipt_value: Any,
    evidence_root: pathlib.Path,
    genesis_hash: str,
    journals: list[dict[str, str]],
) -> dict[str, Any]:
    receipt = exact_keys(
        receipt_value,
        {"path", "sha256", "schema", "status", "completedStages"},
        set(),
        "owned-loopback private session reference",
    )
    session = exact_keys(
        session_value,
        {
            "schema", "status", "cluster", "genesisHash", "stages", "completedStages",
            "stageSetSha256",
        },
        set(),
        "owned-loopback private session",
    )
    completed = session["completedStages"]
    if (
        receipt["path"] != session_relative
        or digest(receipt["sha256"], "private session sha256") != sha256_bytes(session_raw)
        or receipt["schema"] != OWNED_LOOPBACK_PRIVATE_SESSION_SCHEMA
        or receipt["status"] != "finalized"
        or receipt["completedStages"] != completed
        or session["schema"] != OWNED_LOOPBACK_PRIVATE_SESSION_SCHEMA
        or session["status"] != "finalized"
        or session["cluster"] != "owned-loopback"
        or pubkey(session["genesisHash"], "private session genesisHash") != genesis_hash
        or completed != list(OWNED_LOOPBACK_COMPLETED_STAGES)
    ):
        refuse("owned-loopback private session is missing, substituted, provisional, or partial")

    stages = session["stages"]
    if not isinstance(stages, list) or len(stages) != len(OWNED_LOOPBACK_COMPLETED_STAGES):
        refuse("owned-loopback private session omits the exact eight lifecycle stage owners")
    journal_by_path = {journal["path"]: journal for journal in journals}
    if journal_by_path.get(session_relative) != {
        "path": session_relative,
        "sha256": receipt["sha256"],
        "schema": OWNED_LOOPBACK_PRIVATE_SESSION_SCHEMA,
        "completionPointer": "/status",
        "completionValue": "finalized",
    }:
        refuse("private session differs from the authenticated top-level completion journal")
    projected_stages: list[dict[str, str]] = []
    stage_paths: set[str] = set()
    for index, (raw_stage, expected_stage) in enumerate(
        zip(stages, OWNED_LOOPBACK_COMPLETED_STAGES, strict=True)
    ):
        stage = exact_keys(
            raw_stage,
            {
                "stage", "path", "sha256", "schema", "completionPointer",
                "completionValue",
            },
            set(),
            f"private session stage {index}",
        )
        stage_path, relative = canonical_relative_evidence(
            evidence_root, stage["path"], f"private session stage {expected_stage}"
        )
        if (
            stage["stage"] != expected_stage
            or relative == session_relative
            or relative in stage_paths
            or stage["completionValue"] != "finalized"
        ):
            refuse("private session stage order, ownership, or completion is noncanonical")
        stage_paths.add(relative)
        stage_raw, stage_source = read_evidence_file(stage_path)
        source_schema = text(stage["schema"], f"private session stage {expected_stage} schema")
        completion_pointer = text(
            stage["completionPointer"],
            f"private session stage {expected_stage} completionPointer",
        )
        projected = {
            "stage": expected_stage,
            "path": relative,
            "sha256": digest(stage["sha256"], f"private session stage {expected_stage} sha256"),
            "schema": source_schema,
            "completionPointer": completion_pointer,
            "completionValue": "finalized",
        }
        journal = journal_by_path.get(relative)
        if (
            journal is None
            or projected["sha256"] != sha256_bytes(stage_raw)
            or stage_source.get("schema") != source_schema
            or json_pointer(
                stage_source, completion_pointer, f"private session stage {expected_stage}"
            ) != "finalized"
            or {key: projected[key] for key in (
                "path", "sha256", "schema", "completionPointer", "completionValue"
            )} != journal
        ):
            refuse("private session stage bytes differ from the authenticated journal closure")
        projected_stages.append(projected)
    stage_set_sha256 = digest(session["stageSetSha256"], "private session stageSetSha256")
    if stage_set_sha256 != sha256_bytes(canonical_bytes(projected_stages)):
        refuse("private session stageSetSha256 differs from its exact ordered rows")
    return {
        "path": session_relative,
        "sha256": receipt["sha256"],
        "schema": OWNED_LOOPBACK_PRIVATE_SESSION_SCHEMA,
        "status": "finalized",
        "completedStages": list(OWNED_LOOPBACK_COMPLETED_STAGES),
        "stageSetSha256": stage_set_sha256,
    }


def authenticate_owned_loopback_chaos_session(
    receipt_value: Any,
    evidence_root: pathlib.Path,
    journals: list[dict[str, str]],
) -> dict[str, str]:
    receipt = exact_keys(
        receipt_value,
        {"path", "sha256", "schema", "status"},
        set(),
        "owned-loopback chaos session reference",
    )
    path, relative = canonical_relative_evidence(
        evidence_root, receipt["path"], "chaos session"
    )
    raw, source = read_evidence_file(path)
    source = exact_keys(
        source,
        {"schema", "status", "stages"},
        set(),
        "owned-loopback chaos session",
    )
    chaos_sha256 = digest(receipt["sha256"], "chaos session sha256")
    if (
        receipt["schema"] != OWNED_LOOPBACK_CHAOS_SESSION_SCHEMA
        or receipt["status"] != "finalized"
        or source["schema"] != OWNED_LOOPBACK_CHAOS_SESSION_SCHEMA
        or source["status"] != "finalized"
        or chaos_sha256 != sha256_bytes(raw)
    ):
        refuse("owned-loopback chaos session is missing, substituted, or provisional")
    stages = source["stages"]
    if not isinstance(stages, list) or len(stages) != len(OWNED_LOOPBACK_CHAOS_STAGES):
        refuse("owned-loopback chaos session omits the exact eight-stage hostile run")
    for index, (raw_stage, expected_stage) in enumerate(
        zip(stages, OWNED_LOOPBACK_CHAOS_STAGES, strict=True)
    ):
        stage = exact_keys(
            raw_stage,
            {"stage", "status", "intentSha256"},
            set(),
            f"chaos session stage {index}",
        )
        if (
            stage["stage"] != expected_stage
            or stage["status"] != "finalized"
        ):
            refuse("chaos session stage order or completion is noncanonical")
        digest(stage["intentSha256"], f"chaos session {expected_stage} intentSha256")
    journal = next((row for row in journals if row["path"] == relative), None)
    if journal != {
        "path": relative,
        "sha256": chaos_sha256,
        "schema": OWNED_LOOPBACK_CHAOS_SESSION_SCHEMA,
        "completionPointer": "/status",
        "completionValue": "finalized",
    }:
        refuse("chaos session differs from the authenticated journal closure")
    return {
        "path": relative,
        "sha256": chaos_sha256,
        "schema": OWNED_LOOPBACK_CHAOS_SESSION_SCHEMA,
        "status": "finalized",
    }


def authenticate_owned_loopback_session(
    receipt_path: pathlib.Path,
    expected_receipt_sha256: str,
    evidence_root: pathlib.Path,
    manifest_path: pathlib.Path,
    capture_path: pathlib.Path,
    manifest: dict[str, Any],
    rpc_capture: OwnedLoopbackCapturedRpc,
) -> dict[str, Any]:
    try:
        root = evidence_root.resolve(strict=True)
    except OSError as error:
        refuse(f"cannot resolve owned-loopback evidence root: {error}")
    if not root.is_dir():
        refuse("owned-loopback evidence root is not a directory")
    raw, receipt = read_evidence_file(receipt_path)
    if digest(expected_receipt_sha256, "expected session receipt sha256") != sha256_bytes(raw):
        refuse("owned-loopback session receipt differs from its expected SHA-256")
    receipt = exact_keys(
        receipt,
        {
            "schema", "status", "cluster", "sourceCommit", "checkedReleaseGateSha256",
            "programs", "manifestSha256", "capture", "providerClosure", "journals",
            "journalSetSha256", "privateSession", "chaosSession",
        },
        set(),
        "owned-loopback session receipt",
    )
    if receipt["schema"] != OWNED_LOOPBACK_RECEIPT_SCHEMA or receipt["status"] != "finalized":
        refuse("owned-loopback session receipt is provisional or another schema")
    cluster = exact_keys(receipt["cluster"], {"kind", "genesisHash"}, set(), "receipt cluster")
    genesis = pubkey(cluster["genesisHash"], "receipt genesisHash")
    if cluster["kind"] != "owned-loopback" or genesis in (DEVNET_GENESIS_HASH, MAINNET_GENESIS_HASH):
        refuse("owned-loopback session receipt names a public cluster")
    if manifest.get("cluster") != cluster or rpc_capture.genesis_hash() != genesis:
        refuse("owned-loopback manifest, receipt, and capture genesis identities differ")
    source_commit = text(receipt["sourceCommit"], "receipt sourceCommit")
    if len(source_commit) != 40 or any(ch not in "0123456789abcdef" for ch in source_commit):
        refuse("receipt sourceCommit is not one full lowercase Git commit")
    gate_sha256 = digest(receipt["checkedReleaseGateSha256"], "checkedReleaseGateSha256")
    if digest(receipt["manifestSha256"], "manifestSha256") != sha256_bytes(manifest_path.read_bytes()):
        refuse("owned-loopback receipt manifest digest differs from exact bytes")

    programs = receipt["programs"]
    if not isinstance(programs, list) or len(programs) != len(OWNED_LOOPBACK_PROGRAM_ROLES):
        refuse("owned-loopback receipt omitted the exact seven-plus-provider program closure")
    program_ids: set[str] = set()
    programdata_ids: set[str] = set()
    retained_authorities: set[str] = set()
    projected_programs: list[dict[str, Any]] = []
    for index, (raw_program, expected_role) in enumerate(zip(programs, OWNED_LOOPBACK_PROGRAM_ROLES, strict=True)):
        program = exact_keys(
            raw_program,
            {
                "role", "programId", "programDataAddress", "deploymentSlot", "elfSha256",
                "genesisProgramDataSha256", "upgradeAuthority",
            },
            set(),
            f"owned-loopback program {index}",
        )
        if program["role"] != expected_role:
            refuse("owned-loopback receipt program roles are absent, substituted, or noncanonical")
        program_id = pubkey(program["programId"], f"{expected_role} programId")
        programdata_id = pubkey(program["programDataAddress"], f"{expected_role} programDataAddress")
        if program_id in program_ids or programdata_id in programdata_ids or program_id == programdata_id:
            refuse("owned-loopback receipt aliases a Program or ProgramData identity")
        program_ids.add(program_id)
        programdata_ids.add(programdata_id)
        slot = decimal(program["deploymentSlot"], f"{expected_role} deploymentSlot")
        if slot > rpc_capture.finalized_slot or (
            slot == 0 and not expected_role.startswith("pyth-")
        ):
            refuse(f"{expected_role} deployment is absent from the finalized capture boundary")
        elf_sha256 = digest(program["elfSha256"], f"{expected_role} elfSha256")
        genesis_programdata_sha256 = digest(
            program["genesisProgramDataSha256"], f"{expected_role} genesisProgramDataSha256"
        )
        raw_authority = program["upgradeAuthority"]
        upgrade_authority = (
            None
            if raw_authority is None
            else pubkey(raw_authority, f"{expected_role} upgradeAuthority")
        )
        if expected_role.startswith("pyth-"):
            if upgrade_authority is not None:
                refuse("owned-loopback Pyth provider programs must be immutable at genesis")
        elif upgrade_authority is None:
            refuse("owned-loopback dClutch programs omitted their retained disposable authority")
        else:
            retained_authorities.add(upgrade_authority)
        projected_programs.append(
            {
                "role": expected_role,
                "programId": program_id,
                "programDataAddress": programdata_id,
                "deploymentSlot": str(slot),
                "elfSha256": elf_sha256,
                "genesisProgramDataSha256": genesis_programdata_sha256,
                "upgradeAuthority": upgrade_authority,
            }
        )
        authenticate_loader_v3_program(
            rpc_capture,
            expected_role,
            program_id,
            programdata_id,
            slot,
            elf_sha256,
            genesis_programdata_sha256,
            upgrade_authority,
        )
    if len(retained_authorities) != 1:
        refuse("owned-loopback dClutch programs do not share one retained disposable authority")
    if program_ids & programdata_ids:
        refuse("owned-loopback Program and ProgramData closures overlap")

    capture = exact_keys(
        receipt["capture"],
        {"path", "sha256", "schema", "commitment", "finalizedSlot"},
        set(),
        "owned-loopback capture receipt",
    )
    receipt_capture_path, _ = canonical_relative_evidence(root, capture["path"], "capture")
    try:
        actual_capture = capture_path.resolve(strict=True)
    except OSError as error:
        refuse(f"cannot resolve supplied owned-loopback capture: {error}")
    if receipt_capture_path != actual_capture:
        refuse("owned-loopback receipt binds another capture path")
    if (
        digest(capture["sha256"], "capture sha256") != rpc_capture.capture_sha256
        or capture["schema"] != OWNED_LOOPBACK_CAPTURE_SCHEMA
        or capture["commitment"] != "finalized"
        or decimal(capture["finalizedSlot"], "capture finalizedSlot") != rpc_capture.finalized_slot
    ):
        refuse("owned-loopback receipt substitutes or does not finalize its RPC capture")

    provider_closure = authenticate_owned_loopback_provider_closure(
        receipt["providerClosure"],
        root,
        capture_path,
        rpc_capture,
        genesis,
        projected_programs,
    )

    journals = receipt["journals"]
    if not isinstance(journals, list) or not journals or len(journals) > MAX_EVENTS + 32:
        refuse("owned-loopback receipt has no bounded canonical journal set")
    projected_journals: list[dict[str, str]] = []
    journal_paths: set[str] = set()
    terminal_completion: dict[str, Any] | None = None
    for index, raw_journal in enumerate(journals):
        journal = exact_keys(
            raw_journal,
            {"path", "sha256", "schema", "completionPointer", "completionValue"},
            set(),
            f"owned-loopback journal {index}",
        )
        path, relative = canonical_relative_evidence(root, journal["path"], f"journal {index}")
        if relative in journal_paths:
            refuse("owned-loopback receipt repeats a journal path")
        journal_paths.add(relative)
        source_raw, source = read_evidence_file(path)
        source_schema = text(journal["schema"], f"journal {relative} schema")
        completion_pointer = text(
            journal["completionPointer"], f"journal {relative} completionPointer"
        )
        if (
            digest(journal["sha256"], f"journal {relative} sha256") != sha256_bytes(source_raw)
            or source.get("schema") != source_schema
            or journal["completionValue"] != "finalized"
            or json_pointer(source, completion_pointer, f"journal {relative}") != "finalized"
        ):
            refuse(f"journal {relative} is missing, substituted, provisional, or partial")
        if source_schema == OWNED_LOOPBACK_TERMINAL_COMPLETION_SCHEMA:
            if terminal_completion is not None:
                refuse("owned-loopback receipt repeats terminal completion")
            terminal_completion = authenticate_terminal_completion(
                path, source, root, genesis, rpc_capture.finalized_slot
            )
        projected_journals.append(
            {
                "path": relative,
                "sha256": journal["sha256"],
                "schema": source_schema,
                "completionPointer": completion_pointer,
                "completionValue": "finalized",
            }
        )
    if [row["path"] for row in projected_journals] != sorted(journal_paths):
        refuse("owned-loopback receipt journals are not in canonical path order")
    if digest(receipt["journalSetSha256"], "journalSetSha256") != sha256_bytes(
        canonical_bytes(projected_journals)
    ):
        refuse("owned-loopback journalSetSha256 differs from its exact ordered rows")
    event_paths = {event["sourcePath"] for event in manifest["events"]}
    if not event_paths.issubset(journal_paths):
        refuse("owned-loopback receipt omits a lifecycle event source journal")
    if terminal_completion is None:
        refuse("owned-loopback receipt omits typed terminal completion")

    private_session_ref = exact_keys(
        receipt["privateSession"],
        {"path", "sha256", "schema", "status", "completedStages"},
        set(),
        "owned-loopback private session reference",
    )
    session_path, session_relative = canonical_relative_evidence(
        root, private_session_ref["path"], "private session"
    )
    session_raw, session = read_evidence_file(session_path)
    if session_relative not in journal_paths:
        refuse("owned-loopback private session is absent from the authenticated journal closure")
    private_session = authenticate_owned_loopback_private_session(
        session_relative,
        session_raw,
        session,
        private_session_ref,
        root,
        genesis,
        projected_journals,
    )
    chaos_session = authenticate_owned_loopback_chaos_session(
        receipt["chaosSession"], root, projected_journals
    )

    return {
        "classification": "owned-loopback-local-evidence-not-public-devnet-or-live-observation",
        "sessionReceiptSha256": sha256_bytes(raw),
        "sourceCommit": source_commit,
        "checkedReleaseGateSha256": gate_sha256,
        "finalizedCaptureSlot": str(rpc_capture.finalized_slot),
        "programs": projected_programs,
        "providerClosure": provider_closure,
        "journalSetSha256": receipt["journalSetSha256"],
        "privateSession": private_session,
        "chaosSession": chaos_session,
        "terminalCompletion": terminal_completion,
    }


def captured(path: pathlib.Path) -> CapturedRpc:
    raw, value = read_evidence_file(path)
    value = exact_keys(value, {"schema", "genesisHash", "transactions", "accounts"}, set(), "captured RPC")
    if value["schema"] != CAPTURE_SCHEMA:
        refuse("captured RPC schema is not admitted")
    return CapturedRpc(value, sha256_bytes(raw))


def captured_owned_loopback(path: pathlib.Path) -> OwnedLoopbackCapturedRpc:
    raw, value = read_evidence_file(path)
    value = exact_keys(
        value,
        {"schema", "genesisHash", "commitment", "finalizedSlot", "transactions", "accounts"},
        set(),
        "owned-loopback captured RPC",
    )
    genesis = pubkey(value["genesisHash"], "owned-loopback capture genesisHash")
    if value["schema"] != OWNED_LOOPBACK_CAPTURE_SCHEMA or value["commitment"] != "finalized":
        refuse("owned-loopback captured RPC schema or commitment is not admitted")
    if genesis in (DEVNET_GENESIS_HASH, MAINNET_GENESIS_HASH):
        refuse("owned-loopback captured RPC carries a public cluster genesis hash")
    return OwnedLoopbackCapturedRpc(value, sha256_bytes(raw))


def write_dossier(
    path: pathlib.Path | None,
    dossier: dict[str, Any],
    *,
    owned_loopback_terminal: bool = False,
) -> None:
    encoded = canonical_bytes(dossier)
    if path is None:
        sys.stdout.buffer.write(encoded)
    elif not owned_loopback_terminal:
        path.write_bytes(encoded)
    else:
        if not path.is_absolute() or path.name in ("", ".", ".."):
            refuse("owned-loopback dossier output must be one absent absolute file")
        parent = path.parent
        try:
            parent_metadata = parent.lstat()
            canonical_parent = parent.resolve(strict=True)
        except OSError as error:
            refuse(f"owned-loopback dossier output parent is absent: {error}")
        if parent.is_symlink() or not parent.is_dir() or parent != canonical_parent:
            refuse("owned-loopback dossier output parent is not one canonical ordinary directory")
        try:
            path.lstat()
        except FileNotFoundError:
            pass
        except OSError as error:
            refuse(f"cannot inspect owned-loopback dossier output: {error}")
        else:
            refuse("owned-loopback dossier output already exists")

        temporary = parent / f".{path.name}.{os.getpid()}.{secrets.token_hex(8)}.tmp"
        file_descriptor: int | None = None
        try:
            file_descriptor = os.open(
                temporary,
                os.O_WRONLY | os.O_CREAT | os.O_EXCL,
                0o600,
            )
            with os.fdopen(file_descriptor, "wb") as output:
                file_descriptor = None
                output.write(encoded)
                output.flush()
                os.fsync(output.fileno())
            os.link(temporary, path, follow_symlinks=False)
            directory_descriptor = os.open(parent, os.O_RDONLY)
            try:
                os.fsync(directory_descriptor)
            finally:
                os.close(directory_descriptor)
        except FileExistsError:
            refuse("owned-loopback dossier output already exists")
        except OSError as error:
            refuse(f"cannot publish owned-loopback dossier output: {error}")
        finally:
            if file_descriptor is not None:
                os.close(file_descriptor)
            try:
                temporary.unlink()
            except FileNotFoundError:
                pass
            except OSError as error:
                refuse(f"cannot remove owned-loopback dossier temporary file: {error}")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    sub = root.add_subparsers(dest="command", required=True)
    offline = sub.add_parser("captured", help="reconcile a captured finalized RPC fixture")
    offline.add_argument("--manifest", required=True, type=pathlib.Path)
    offline.add_argument("--rpc-capture", required=True, type=pathlib.Path)
    offline.add_argument("--journal-root", required=True, type=pathlib.Path)
    offline.add_argument("--out", type=pathlib.Path)
    loopback = sub.add_parser(
        "owned-loopback-captured",
        help="reconcile an authenticated finalized owned-loopback lifecycle capture",
    )
    loopback.add_argument("--manifest", required=True, type=pathlib.Path)
    loopback.add_argument("--rpc-capture", required=True, type=pathlib.Path)
    loopback.add_argument("--session-receipt", required=True, type=pathlib.Path)
    loopback.add_argument("--expected-session-receipt-sha256", required=True)
    loopback.add_argument("--evidence-root", required=True, type=pathlib.Path)
    loopback.add_argument("--out", type=pathlib.Path)
    follow = sub.add_parser("follow", help="bounded finalized-only devnet polling")
    follow.add_argument("--manifest", required=True, type=pathlib.Path)
    follow.add_argument("--rpc-url", required=True)
    follow.add_argument("--journal-root", required=True, type=pathlib.Path)
    follow.add_argument("--out", type=pathlib.Path)
    follow.add_argument("--max-polls", type=int, default=5)
    follow.add_argument("--interval-seconds", type=float, default=2.0)
    follow.add_argument("--timeout-seconds", type=float, default=10.0)
    return root


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        manifest = load_json(args.manifest)
        if args.command == "captured":
            authenticate_sources(manifest, args.journal_root)
            dossier = reconcile(manifest, captured(args.rpc_capture))
        elif args.command == "owned-loopback-captured":
            authenticate_owned_loopback_sources(manifest, args.evidence_root)
            loopback_rpc = captured_owned_loopback(args.rpc_capture)
            session_evidence = authenticate_owned_loopback_session(
                args.session_receipt,
                args.expected_session_receipt_sha256,
                args.evidence_root,
                args.manifest,
                args.rpc_capture,
                manifest,
                loopback_rpc,
            )
            dossier = reconcile_owned_loopback(manifest, loopback_rpc, session_evidence)
        else:
            authenticate_sources(manifest, args.journal_root)
            if not 1 <= args.max_polls <= MAX_POLLS or not 0 <= args.interval_seconds <= 60 or not 1 <= args.timeout_seconds <= 30:
                refuse("poll count, interval, or timeout exceeds the bounded live policy")
            client = LiveRpc(args.rpc_url, args.timeout_seconds)
            last: Refusal | None = None
            dossier = None
            for attempt in range(args.max_polls):
                try:
                    dossier = reconcile(manifest, client)
                    break
                except Refusal as error:
                    last = error
                    if attempt + 1 < args.max_polls:
                        time.sleep(args.interval_seconds)
            if dossier is None:
                raise last or Refusal("bounded polling produced no complete dossier")
        write_dossier(
            args.out,
            dossier,
            owned_loopback_terminal=args.command == "owned-loopback-captured",
        )
        return 0
    except Refusal as error:
        print(f"REFUSED: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
