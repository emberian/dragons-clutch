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
EVENT_KINDS = ("founding", "participant", "direct", "resolution", "payout", "retirement")
RESOLUTION_OPERATIONS_V7 = (
    "resolution-submit",
    "resolution-provider-execute-v1",
    "core-terminal-accept-v1",
    "resolution-reclaim",
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


def captured(path: pathlib.Path) -> CapturedRpc:
    raw, value = read_evidence_file(path)
    value = exact_keys(value, {"schema", "genesisHash", "transactions", "accounts"}, set(), "captured RPC")
    if value["schema"] != CAPTURE_SCHEMA:
        refuse("captured RPC schema is not admitted")
    return CapturedRpc(value, sha256_bytes(raw))


def write_dossier(path: pathlib.Path | None, dossier: dict[str, Any]) -> None:
    encoded = canonical_bytes(dossier)
    if path is None:
        sys.stdout.buffer.write(encoded)
    else:
        path.write_bytes(encoded)


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    sub = root.add_subparsers(dest="command", required=True)
    offline = sub.add_parser("captured", help="reconcile a captured finalized RPC fixture")
    offline.add_argument("--manifest", required=True, type=pathlib.Path)
    offline.add_argument("--rpc-capture", required=True, type=pathlib.Path)
    offline.add_argument("--journal-root", required=True, type=pathlib.Path)
    offline.add_argument("--out", type=pathlib.Path)
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
        write_dossier(args.out, dossier)
        return 0
    except Refusal as error:
        print(f"REFUSED: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
