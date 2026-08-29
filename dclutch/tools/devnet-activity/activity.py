#!/usr/bin/env python3
"""Crash-safe, secret-safe dClutch activity orchestration.

This module is deliberately outside every protocol authority.  It consumes a
separately owned economic scenario, creates disposable keypairs through
``solana-keygen`` without reading their bytes, and invokes accepted public
callers without a shell.  Every mutation is fenced by a write-ahead journal.

Public devnet mutation additionally requires a short-lived authorization file
bound to the exact scenario and activity-manifest bytes.  Owned loopback never
accepts that authorization and public devnet never accepts loopback affordances.
"""

from __future__ import annotations

import argparse
from concurrent.futures import Future, ThreadPoolExecutor, wait, FIRST_COMPLETED
import dataclasses
import datetime as dt
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tempfile
import threading
import time
from typing import Any, Iterable, Mapping, Sequence
from urllib import parse as urlparse
from urllib import request as urlrequest


MANIFEST_SCHEMA = "dclutch-devnet-activity-manifest-v2"
WALLET_LEDGER_SCHEMA = "dclutch-devnet-activity-wallet-ledger-v1"
PRIVATE_INDEX_SCHEMA = "dclutch-devnet-activity-private-wallet-index-v1"
FUNDING_JOURNAL_SCHEMA = "dclutch-devnet-activity-funding-journal-v1"
FUNDING_CLOSURE_SCHEMA = "dclutch-devnet-activity-funding-closure-v1"
AUTHORIZATION_SCHEMA = "dclutch-devnet-activity-live-authorization-v1"
BOUNDED_AUTHORIZATION_SCHEMA = "dclutch-devnet-activity-live-authorization-v2"
STOP_SCHEMA = "dclutch-devnet-activity-stop-v1"
ADAPTER_JOURNAL_SCHEMA = "dclutch-devnet-activity-adapter-journal-v1"
RECONCILIATION_SCHEMA = "dclutch-devnet-activity-reconciliation-v1"
SUPERVISOR_REQUEST_SCHEMA = "dclutch-devnet-activity-supervisor-request-v2"
SUPERVISOR_STATUS_SCHEMA = "dclutch-devnet-activity-supervisor-status-v2"
CAMPAIGN_REPORT_SCHEMA = "dclutch-successor-campaign-report-v1"
DEVNET_GENESIS_HASH = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
DEVNET_MANIFEST_RPC_URL = "https://api.devnet.solana.com:443/"
DEVNET_SUPERVISOR_RPC_URL = "https://api.devnet.solana.com/"
MEMO_PREFIX = "dclutch-activity-fund-v1:"
PUBKEY_RE = re.compile(r"^[1-9A-HJ-NP-Za-km-z]{32,44}$")
HEX_RE = re.compile(r"^[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
ID_RE = re.compile(r"^[a-z][a-z0-9-]{0,63}$")
REF_RE = re.compile(r"^[a-z][a-z0-9.-]{0,191}$")
DECIMAL_RE = re.compile(r"^(?:0|[1-9][0-9]*)$")
SIGNED_DECIMAL_RE = re.compile(r"^-?(?:0|[1-9][0-9]*)$")
SIGNATURE_RE = re.compile(r"^[1-9A-HJ-NP-Za-km-z]{64,96}$")
TEMPLATE_RE = re.compile(r"\{\{([^{}]+)\}\}")
SECRET_FLAGS = frozenset(
    {
        "--keypair",
        "--fee-payer",
        "--from",
        "--position-owner-keypair",
        "--payer-keypair",
        "--funder-keypair",
    }
)


class Refusal(RuntimeError):
    """A fail-closed activity-harness refusal."""


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()


def compact_json(value: Any) -> bytes:
    """Match serde_json compact struct serialization for scenario body binding."""
    return json.dumps(value, sort_keys=False, separators=(",", ":"), ensure_ascii=False).encode()


def state_digest(value: Mapping[str, Any]) -> str:
    return sha256_bytes(canonical_json({key: item for key, item in value.items() if key != "stateSha256"}))


def read_exact_json(path: Path, label: str) -> Any:
    def pairs(rows: list[tuple[str, Any]]) -> dict[str, Any]:
        output: dict[str, Any] = {}
        for key, value in rows:
            if key in output:
                raise Refusal(f"{label} duplicated JSON key {key!r}")
            output[key] = value
        return output

    try:
        return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=pairs)
    except Refusal:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise Refusal(f"{label} is not exact JSON: {error}") from error


def exact_keys(value: Mapping[str, Any], expected: Iterable[str], label: str) -> None:
    observed = set(value)
    wanted = set(expected)
    if observed != wanted:
        missing = sorted(wanted - observed)
        unknown = sorted(observed - wanted)
        raise Refusal(f"{label} has missing fields {missing} or unknown fields {unknown}")


def exact_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise Refusal(f"{label} must be one JSON object")
    return value


def exact_list(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        raise Refusal(f"{label} must be one JSON array")
    return value


def text(value: Any, label: str, maximum: int = 4096) -> str:
    if not isinstance(value, str) or not value or len(value) > maximum or value.strip() != value:
        raise Refusal(f"{label} must be bounded canonical text")
    return value


def stable_id(value: Any, label: str) -> str:
    candidate = text(value, label, 64)
    if ID_RE.fullmatch(candidate) is None:
        raise Refusal(f"{label} must match {ID_RE.pattern}")
    return candidate


def logical_ref(value: Any, label: str, *, allow_empty: bool = False) -> str:
    if allow_empty and value == "":
        return ""
    candidate = text(value, label, 192)
    if REF_RE.fullmatch(candidate) is None or ".." in candidate or candidate.endswith("."):
        raise Refusal(f"{label} must be one canonical logical reference")
    return candidate


def decimal(value: Any, label: str, *, positive: bool = False) -> int:
    candidate = text(value, label, 32)
    if DECIMAL_RE.fullmatch(candidate) is None:
        raise Refusal(f"{label} must be one canonical unsigned decimal string")
    number = int(candidate)
    if positive and number == 0:
        raise Refusal(f"{label} must be positive")
    if number > 2**64 - 1:
        raise Refusal(f"{label} exceeds u64")
    return number


def signed_decimal(value: Any, label: str) -> int:
    candidate = text(value, label, 33)
    if SIGNED_DECIMAL_RE.fullmatch(candidate) is None or candidate == "-0":
        raise Refusal(f"{label} must be one canonical signed decimal string")
    number = int(candidate)
    if number < -(2**63) or number > 2**63 - 1:
        raise Refusal(f"{label} exceeds i64")
    return number


def digest_text(value: Any, label: str) -> str:
    candidate = text(value, label, 64)
    if HEX_RE.fullmatch(candidate) is None:
        raise Refusal(f"{label} must be exactly 32 lowercase hexadecimal bytes")
    return candidate


def pubkey_text(value: Any, label: str) -> str:
    candidate = text(value, label, 44)
    if PUBKEY_RE.fullmatch(candidate) is None:
        raise Refusal(f"{label} is not a canonical base58 public-key shape")
    return candidate


def signature_text(value: Any, label: str) -> str:
    candidate = text(value, label, 96)
    if SIGNATURE_RE.fullmatch(candidate) is None:
        raise Refusal(f"{label} is not a canonical Solana signature shape")
    return candidate


def canonical_existing_file(value: str | Path, label: str, *, executable: bool = False) -> Path:
    path = Path(value)
    if not path.is_absolute() or path.is_symlink() or not path.is_file():
        raise Refusal(f"{label} must be one existing absolute non-symlink file: {path}")
    result = path.resolve(strict=True)
    if executable and not os.access(result, os.X_OK):
        raise Refusal(f"{label} is not executable: {result}")
    return result


def canonical_directory(value: str | Path, label: str) -> Path:
    path = Path(value)
    if not path.is_absolute() or path.is_symlink() or not path.is_dir():
        raise Refusal(f"{label} must be one existing absolute non-symlink directory: {path}")
    return path.resolve(strict=True)


def new_work_directory(value: str | Path) -> Path:
    path = Path(value)
    if not path.is_absolute() or path.is_symlink():
        raise Refusal("--work must be one absolute non-symlink path")
    if path.exists() and not path.is_dir():
        raise Refusal("--work exists but is not a directory")
    path.mkdir(mode=0o700, parents=False, exist_ok=True)
    path.chmod(0o700)
    return path.resolve(strict=True)


def fsync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def atomic_write_json(path: Path, value: Mapping[str, Any], *, mode: int = 0o600) -> None:
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    payload = dict(value)
    payload["stateSha256"] = state_digest(payload)
    encoded = json.dumps(payload, sort_keys=True, indent=2, ensure_ascii=True).encode() + b"\n"
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    temporary_path = Path(temporary)
    try:
        os.fchmod(descriptor, mode)
        with os.fdopen(descriptor, "wb", closefd=True) as output:
            output.write(encoded)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary_path, path)
        fsync_directory(path.parent)
    finally:
        if temporary_path.exists():
            temporary_path.unlink()


def authenticated_state(path: Path, label: str) -> dict[str, Any]:
    value = exact_object(read_exact_json(path, label), label)
    observed = digest_text(value.get("stateSha256"), f"{label} state digest")
    if observed != state_digest(value):
        raise Refusal(f"{label} state digest changed")
    return value


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


def redact_argv(argv: Sequence[str]) -> list[str]:
    output: list[str] = []
    redact_next = False
    for argument in argv:
        if redact_next:
            output.append("<secret-path>")
            redact_next = False
        else:
            output.append(argument)
            redact_next = argument in SECRET_FLAGS
    return output


@dataclasses.dataclass(frozen=True)
class Limits:
    max_concurrency: int
    min_dispatch_interval_ms: int
    max_transactions: int
    poll_interval_ms: int
    max_polls: int


@dataclasses.dataclass(frozen=True)
class WalletSpec:
    wallet_id: str
    roles: tuple[str, ...]
    funding_lamports: int
    collateral_account_ref: str
    claim_account_refs: tuple[str, ...]
    position_account_ref: str


@dataclasses.dataclass(frozen=True)
class TokenDeltaSpec:
    wallet_ref: str | None
    account_ref: str
    mint_ref: str
    before_state: str
    after_state: str
    delta_atoms: int


@dataclasses.dataclass(frozen=True)
class OperationSpec:
    operation_id: str
    kind: str
    wallet_ids: tuple[str, ...]
    depends_on: tuple[str, ...]
    mutation_expected: bool
    expected_lamport_deltas: Mapping[str, int]
    expected_token_deltas: tuple[TokenDeltaSpec, ...]
    caller_target: str
    caller_schema: str | None
    caller_availability: str
    evidence_output_ref: str


@dataclasses.dataclass(frozen=True)
class AccountSpec:
    account_ref: str
    kind: str
    mint_ref: str | None
    token_authority_wallet_ref: str | None


@dataclasses.dataclass(frozen=True)
class Scenario:
    path: Path
    sha256: str
    schema: str
    scenario_id: str
    cluster_target: str
    genesis_hash: str
    market_ref: str
    wallets: tuple[WalletSpec, ...]
    accounts: tuple[AccountSpec, ...]
    operations: tuple[OperationSpec, ...]
    limits: Limits


@dataclasses.dataclass(frozen=True)
class AddressBinding:
    reference: str
    kind: str
    wallet_ref: str | None
    address: str | None
    input_id: str | None
    pointer: str | None


@dataclasses.dataclass(frozen=True)
class CompletionSpec:
    path: str
    schema: str | None
    signature_pointers: tuple[str, ...]
    transaction_list_pointer: str | None
    required_transaction_labels: tuple[str, ...]
    required_values: Mapping[str, Any]


@dataclasses.dataclass(frozen=True)
class AdapterSpec:
    adapter_id: str
    covers: tuple[str, ...]
    caller: str
    argv: tuple[str, ...]
    depends_on: tuple[str, ...]
    wallet_ids: tuple[str, ...]
    mutation: bool
    completion: CompletionSpec


@dataclasses.dataclass(frozen=True)
class Manifest:
    path: Path
    sha256: str
    scenario: Scenario
    rpc_url: str
    devnet_genesis_hash: str | None
    inputs: Mapping[str, Path]
    address_bindings: tuple[AddressBinding, ...]
    adapters: tuple[AdapterSpec, ...]


def parse_limits(value: Any, label: str, cluster_target: str) -> Limits:
    source = exact_object(value, label)
    exact_keys(
        source,
        {"maxConcurrency", "minDispatchIntervalMs", "maxTransactions", "pollIntervalMs", "maxPolls"},
        label,
    )
    fields: dict[str, int] = {}
    for name in source:
        item = source[name]
        if not isinstance(item, int) or isinstance(item, bool):
            raise Refusal(f"{label} {name} must be one JSON integer")
        fields[name] = item
    if fields["maxConcurrency"] < 1 or fields["maxConcurrency"] > 8:
        raise Refusal("maxConcurrency must be in 1..8")
    if cluster_target == "devnet" and fields["maxConcurrency"] > 2:
        raise Refusal("public devnet concurrency is capped at two")
    minimum_interval = 1_000 if cluster_target == "devnet" else 0
    if fields["minDispatchIntervalMs"] < minimum_interval or fields["minDispatchIntervalMs"] > 60_000:
        raise Refusal(f"minDispatchIntervalMs must be in {minimum_interval}..60000")
    if fields["maxTransactions"] < 1 or fields["maxTransactions"] > 10_000:
        raise Refusal("maxTransactions must be in 1..10000")
    if fields["pollIntervalMs"] < 250 or fields["pollIntervalMs"] > 60_000:
        raise Refusal("pollIntervalMs must be in 250..60000")
    if fields["maxPolls"] < 1 or fields["maxPolls"] > 3_600:
        raise Refusal("maxPolls must be in 1..3600")
    return Limits(
        fields["maxConcurrency"],
        fields["minDispatchIntervalMs"],
        fields["maxTransactions"],
        fields["pollIntervalMs"],
        fields["maxPolls"],
    )


def parse_ledger_delta(value: Any, label: str, wallet_ids: set[str]) -> tuple[dict[str, int], tuple[TokenDeltaSpec, ...]]:
    source = exact_object(value, label)
    exact_keys(source, {"lamportDeltas", "tokenDeltas", "accountStateChanges", "positionChanges"}, label)
    lamports: dict[str, int] = {}
    for index, raw in enumerate(exact_list(source["lamportDeltas"], f"{label} lamport deltas")):
        row = exact_object(raw, f"{label} lamport delta {index}")
        exact_keys(row, {"accountRef", "deltaLamports"}, f"{label} lamport delta {index}")
        account_ref = logical_ref(row["accountRef"], f"{label} lamport account")
        if account_ref in lamports:
            raise Refusal(f"{label} repeats lamport account {account_ref}")
        lamports[account_ref] = signed_decimal(row["deltaLamports"], f"{label} {account_ref} lamports")
    token_rows: list[TokenDeltaSpec] = []
    token_keys: set[tuple[str, str]] = set()
    for index, raw in enumerate(exact_list(source["tokenDeltas"], f"{label} token deltas")):
        row = exact_object(raw, f"{label} token delta {index}")
        exact_keys(row, {"walletRef", "accountRef", "mintRef", "beforeState", "afterState", "deltaAtoms"}, f"{label} token delta {index}")
        wallet_value = row["walletRef"]
        wallet_ref = None if wallet_value is None else stable_id(wallet_value, f"{label} token wallet")
        if wallet_ref is not None and wallet_ref not in wallet_ids:
            raise Refusal(f"{label} token delta names absent wallet {wallet_ref}")
        account_ref = logical_ref(row["accountRef"], f"{label} token account")
        mint_ref = logical_ref(row["mintRef"], f"{label} token Mint")
        row_key = (account_ref, mint_ref)
        if row_key in token_keys:
            raise Refusal(f"{label} repeats token account/Mint {row_key}")
        token_keys.add(row_key)
        before = text(row["beforeState"], f"{label} token before state", 16)
        after = text(row["afterState"], f"{label} token after state", 16)
        if before not in {"absent", "present", "closed"} or after not in {"absent", "present", "closed"}:
            raise Refusal(f"{label} token state is not canonical")
        token_rows.append(TokenDeltaSpec(wallet_ref, account_ref, mint_ref, before, after, signed_decimal(row["deltaAtoms"], f"{label} token atoms")))
    exact_list(source["accountStateChanges"], f"{label} account-state changes")
    exact_list(source["positionChanges"], f"{label} position changes")
    return lamports, tuple(token_rows)


def parse_scenario(path: Path, expected_sha256: str) -> Scenario:
    path = canonical_existing_file(path, "economic scenario")
    observed_sha256 = sha256_file(path)
    if observed_sha256 != expected_sha256:
        raise Refusal("economic scenario bytes differ from the activity manifest")
    envelope = exact_object(read_exact_json(path, "economic scenario"), "economic scenario")
    exact_keys(envelope, {"schema", "version", "scenarioId", "bodyDigestScope", "bodySha256", "body"}, "economic scenario")
    schema = text(envelope["schema"], "economic scenario schema", 128)
    if schema != "dclutch-devnet-economic-scenario-v1" or envelope["version"] != 1 or envelope["bodyDigestScope"] != "canonical-compact-scenario-body-json-v1":
        raise Refusal("economic scenario is not the canonical v1 envelope")
    body = exact_object(envelope["body"], "economic scenario body")
    if digest_text(envelope["bodySha256"], "economic scenario body digest") != sha256_bytes(compact_json(body)):
        raise Refusal("economic scenario body digest changed")
    exact_keys(
        body,
        {"scenarioId", "title", "description", "clusterTarget", "genesisHash", "evidenceLevel", "market", "limits", "wallets", "accounts", "initialSnapshot", "operations", "finalSnapshot", "retireEligible"},
        "economic scenario body",
    )
    scenario_id = stable_id(body["scenarioId"], "scenario id")
    if envelope["scenarioId"] != scenario_id:
        raise Refusal("economic scenario envelope substitutes scenarioId")
    text(body["title"], "scenario title", 256)
    text(body["description"], "scenario description", 4096)
    cluster_target = text(body["clusterTarget"], "cluster target", 32)
    if cluster_target not in {"owned-loopback", "devnet"} or body["evidenceLevel"] != "scenario-only":
        raise Refusal("scenario must be an owned-loopback/devnet scenario-only fixture")
    genesis_hash = text(body["genesisHash"], "scenario genesis hash", 64)
    if cluster_target == "devnet" and genesis_hash != DEVNET_GENESIS_HASH:
        raise Refusal("devnet scenario carries another genesis hash")
    if cluster_target == "owned-loopback" and genesis_hash == DEVNET_GENESIS_HASH:
        raise Refusal("owned-loopback scenario carries the public devnet genesis hash")
    limits = parse_limits(body["limits"], "scenario limits", cluster_target)

    market = exact_object(body["market"], "scenario market")
    exact_keys(
        market,
        {"profile", "marketRef", "inputArtifact", "outcomeCount", "collateralMintRef", "claimMintRefs", "resolution", "priceScaleAtoms", "feeDenominator", "feeBasisPointsPerSide", "feeRecipientAccountRef", "hoardPrincipalAccountRef"},
        "scenario market",
    )
    market_ref = logical_ref(market["marketRef"], "market ref")
    logical_ref(market["collateralMintRef"], "collateral Mint ref")
    for raw in exact_list(market["claimMintRefs"], "claim Mint refs"):
        logical_ref(raw, "claim Mint ref")

    wallets: list[WalletSpec] = []
    wallet_ids: set[str] = set()
    for index, raw in enumerate(exact_list(body["wallets"], "scenario wallets")):
        source = exact_object(raw, f"wallet {index}")
        exact_keys(source, {"id", "roles", "fundingLamports", "collateralAccountRef", "claimAccountRefs", "positionAccountRef"}, f"wallet {index}")
        wallet_id = stable_id(source["id"], f"wallet {index} id")
        if wallet_id in wallet_ids:
            raise Refusal(f"scenario repeats wallet {wallet_id}")
        wallet_ids.add(wallet_id)
        roles = tuple(stable_id(role, f"wallet {wallet_id} role") for role in exact_list(source["roles"], f"wallet {wallet_id} roles"))
        if not roles or len(set(roles)) != len(roles):
            raise Refusal(f"wallet {wallet_id} roles must be nonempty and unique")
        wallets.append(
            WalletSpec(
                wallet_id,
                roles,
                decimal(source["fundingLamports"], f"wallet {wallet_id} funding", positive=True),
                logical_ref(source["collateralAccountRef"], f"wallet {wallet_id} collateral account"),
                tuple(logical_ref(item, f"wallet {wallet_id} claim account") for item in exact_list(source["claimAccountRefs"], f"wallet {wallet_id} claim accounts")),
                "" if source["positionAccountRef"] is None else logical_ref(source["positionAccountRef"], f"wallet {wallet_id} Position", allow_empty=True),
            )
        )
    if not wallets:
        raise Refusal("scenario must name at least one disposable wallet")

    accounts: list[AccountSpec] = []
    account_ids: set[str] = set()
    for index, raw in enumerate(exact_list(body["accounts"], "scenario accounts")):
        source = exact_object(raw, f"scenario account {index}")
        exact_keys(source, {"id", "kind", "address", "expectedOwnerRef", "mintRef", "tokenAuthorityWalletRef"}, f"scenario account {index}")
        account_ref = logical_ref(source["id"], f"scenario account {index} ref")
        if account_ref in account_ids or source["address"] is not None:
            raise Refusal("scenario-only fixture repeats an account or carries a live address")
        account_ids.add(account_ref)
        kind = text(source["kind"], f"scenario account {account_ref} kind", 32)
        if kind not in {"wallet", "token", "hoard-principal", "position", "certificate", "market"}:
            raise Refusal(f"scenario account {account_ref} has another kind")
        logical_ref(source["expectedOwnerRef"], f"scenario account {account_ref} owner ref")
        mint_ref = None if source["mintRef"] is None else logical_ref(source["mintRef"], f"scenario account {account_ref} Mint ref")
        authority = None if source["tokenAuthorityWalletRef"] is None else stable_id(source["tokenAuthorityWalletRef"], f"scenario account {account_ref} token authority")
        if authority is not None and authority not in wallet_ids:
            raise Refusal(f"scenario account {account_ref} names an absent token authority")
        accounts.append(AccountSpec(account_ref, kind, mint_ref, authority))

    kinds = {"found", "participant", "direct", "resolve", "redeem", "retire"}
    operations: list[OperationSpec] = []
    operation_ids: set[str] = set()
    for index, raw in enumerate(exact_list(body["operations"], "scenario operations")):
        source = exact_object(raw, f"operation {index}")
        exact_keys(
            source,
            {"id", "order", "kind", "predecessorId", "dependencyIds", "feePayerWalletRef", "callerTarget", "callerSchema", "callerAvailability", "mutationExpected", "evidenceOutputRef", "capture", "input", "expectedObservedDelta", "projectedAcceptedDelta"},
            f"operation {index}",
        )
        operation_id = stable_id(source["id"], f"operation {index} id")
        if operation_id in operation_ids or source["order"] != index:
            raise Refusal(f"scenario repeats or misorders operation {operation_id}")
        operation_ids.add(operation_id)
        kind = text(source["kind"], f"operation {operation_id} kind", 32)
        if kind not in kinds:
            raise Refusal(f"operation {operation_id} has unknown kind {kind}")
        dependencies = tuple(stable_id(item, f"operation {operation_id} dependency") for item in exact_list(source["dependencyIds"], f"operation {operation_id} dependencies"))
        predecessor = source["predecessorId"]
        if (index == 0 and (predecessor is not None or dependencies)) or (index > 0 and (predecessor != body["operations"][index - 1]["id"] or dependencies != (predecessor,))):
            raise Refusal(f"operation {operation_id} predecessor/dependency chain is not canonical")
        fee_payer = stable_id(source["feePayerWalletRef"], f"operation {operation_id} fee payer")
        if fee_payer not in wallet_ids:
            raise Refusal(f"operation {operation_id} names absent fee payer {fee_payer}")
        mutation = source["mutationExpected"]
        if not isinstance(mutation, bool):
            raise Refusal(f"operation {operation_id} mutationExpected must be boolean")
        availability = text(source["callerAvailability"], f"operation {operation_id} caller availability", 32)
        if availability not in {"public-executable", "preflight-only", "adapter-required"}:
            raise Refusal(f"operation {operation_id} has another caller availability")
        caller_schema = None if source["callerSchema"] is None else text(source["callerSchema"], f"operation {operation_id} caller schema", 128)
        if (availability == "public-executable") != mutation:
            raise Refusal(f"operation {operation_id} caller availability disagrees with mutationExpected")
        if availability == "public-executable" and caller_schema is None:
            raise Refusal(f"operation {operation_id} public caller omitted its schema")
        if availability == "adapter-required" and caller_schema is not None:
            raise Refusal(f"operation {operation_id} adapter-required gap invented a schema")
        capture = exact_object(source["capture"], f"operation {operation_id} capture")
        exact_keys(capture, {"signature", "finalizedSlot", "transactionFeeLamports"}, f"operation {operation_id} capture")
        if any(value is not None for value in capture.values()):
            raise Refusal("scenario-only operation carried captured execution evidence")
        operation_input = exact_object(source["input"], f"operation {operation_id} input")
        if operation_input.get("kind") != kind:
            raise Refusal(f"operation {operation_id} input kind differs from the operation")
        operation_wallets = {fee_payer}
        for field in ("walletRef", "sellerWalletRef", "buyerWalletRef", "rentRefundWalletRef"):
            if field in operation_input:
                referenced_wallet = stable_id(operation_input[field], f"operation {operation_id} {field}")
                if referenced_wallet not in wallet_ids:
                    raise Refusal(f"operation {operation_id} input names absent wallet {referenced_wallet}")
                operation_wallets.add(referenced_wallet)
        expected_lamports, expected_tokens = parse_ledger_delta(source["expectedObservedDelta"], f"operation {operation_id} expected observed delta", wallet_ids)
        parse_ledger_delta(source["projectedAcceptedDelta"], f"operation {operation_id} projected accepted delta", wallet_ids)
        operations.append(
            OperationSpec(
                operation_id,
                kind,
                tuple(sorted(operation_wallets)),
                dependencies,
                mutation,
                expected_lamports,
                expected_tokens,
                text(source["callerTarget"], f"operation {operation_id} caller target", 128),
                caller_schema,
                availability,
                logical_ref(source["evidenceOutputRef"], f"operation {operation_id} evidence output ref"),
            )
        )
    if not operations:
        raise Refusal("scenario must name at least one activity operation")
    require_acyclic({item.operation_id: item.depends_on for item in operations}, "scenario operation graph")
    for snapshot_name in ("initialSnapshot", "finalSnapshot"):
        snapshot = exact_object(body[snapshot_name], f"scenario {snapshot_name}")
        exact_keys(snapshot, {"accountStates", "tokenBalances", "positionRevisions"}, f"scenario {snapshot_name}")
        for field in snapshot:
            exact_list(snapshot[field], f"scenario {snapshot_name} {field}")
    if not isinstance(body["retireEligible"], bool):
        raise Refusal("scenario retireEligible must be boolean")
    return Scenario(
        path,
        observed_sha256,
        schema,
        scenario_id,
        cluster_target,
        genesis_hash,
        market_ref,
        tuple(wallets),
        tuple(accounts),
        tuple(operations),
        limits,
    )


def require_acyclic(graph: Mapping[str, Sequence[str]], label: str) -> None:
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(node: str) -> None:
        if node in visiting:
            raise Refusal(f"{label} contains a cycle at {node}")
        if node in visited:
            return
        visiting.add(node)
        for parent in graph[node]:
            visit(parent)
        visiting.remove(node)
        visited.add(node)

    for node in graph:
        visit(node)


def validate_rpc_url(value: Any, target: str) -> str:
    candidate = text(value, "RPC URL", 512)
    parsed = urlparse.urlsplit(candidate)
    if parsed.scheme not in {"http", "https"} or parsed.username is not None or parsed.password is not None or parsed.query or parsed.fragment:
        raise Refusal("RPC URL must be one credential-free HTTP(S) origin")
    if parsed.path not in {"", "/"} or parsed.port is None:
        raise Refusal("RPC URL must have an explicit port and no path")
    host = parsed.hostname
    if host is None:
        raise Refusal("RPC URL has no host")
    loopback = host in {"127.0.0.1", "localhost", "::1"}
    if target == "owned-loopback" and (parsed.scheme != "http" or not loopback):
        raise Refusal("owned-loopback requires a literal loopback HTTP origin")
    if target == "devnet" and loopback:
        raise Refusal("devnet activity refuses every loopback RPC origin")
    return candidate


def parse_completion(value: Any, adapter_id: str) -> CompletionSpec:
    source = exact_object(value, f"adapter {adapter_id} completion")
    exact_keys(
        source,
        {
            "path",
            "schema",
            "signaturePointers",
            "transactionListPointer",
            "requiredTransactionLabels",
            "requiredValues",
        },
        f"adapter {adapter_id} completion",
    )
    path = text(source["path"], f"adapter {adapter_id} completion path")
    schema_value = source["schema"]
    schema = None if schema_value is None else text(schema_value, f"adapter {adapter_id} completion schema", 128)
    pointers = tuple(text(item, f"adapter {adapter_id} signature pointer", 256) for item in exact_list(source["signaturePointers"], f"adapter {adapter_id} signature pointers"))
    if len(set(pointers)) != len(pointers):
        raise Refusal(f"adapter {adapter_id} repeats a signature pointer")
    transaction_list_value = source["transactionListPointer"]
    transaction_list_pointer = (
        None
        if transaction_list_value is None
        else text(
            transaction_list_value,
            f"adapter {adapter_id} transaction-list pointer",
            256,
        )
    )
    required_transaction_labels = tuple(
        text(item, f"adapter {adapter_id} required transaction label", 512)
        for item in exact_list(
            source["requiredTransactionLabels"],
            f"adapter {adapter_id} required transaction labels",
        )
    )
    if len(set(required_transaction_labels)) != len(required_transaction_labels):
        raise Refusal(f"adapter {adapter_id} repeats a required transaction label")
    if transaction_list_pointer is None:
        if required_transaction_labels:
            raise Refusal(
                f"adapter {adapter_id} has required transaction labels without a transaction list"
            )
    elif (
        schema != CAMPAIGN_REPORT_SCHEMA
        or transaction_list_pointer != "/execution/transactions"
        or pointers
        or not required_transaction_labels
    ):
        raise Refusal(
            f"adapter {adapter_id} campaign transaction-list completion has another exact shape"
        )
    required_values = exact_object(source["requiredValues"], f"adapter {adapter_id} required values")
    for pointer in required_values:
        if not pointer.startswith("/"):
            raise Refusal(f"adapter {adapter_id} required-value key is not a JSON pointer")
    return CompletionSpec(
        path,
        schema,
        pointers,
        transaction_list_pointer,
        required_transaction_labels,
        required_values,
    )


def parse_address_binding(value: Any, index: int, inputs: Mapping[str, Path], wallet_ids: set[str]) -> AddressBinding:
    source = exact_object(value, f"address binding {index}")
    exact_keys(source, {"ref", "source"}, f"address binding {index}")
    reference = logical_ref(source["ref"], f"address binding {index} ref")
    binding = exact_object(source["source"], f"address binding {reference} source")
    kind = text(binding.get("kind"), f"address binding {reference} kind", 32)
    if kind == "wallet":
        exact_keys(binding, {"kind", "walletRef"}, f"address binding {reference} source")
        wallet_ref = stable_id(binding["walletRef"], f"address binding {reference} wallet")
        if wallet_ref not in wallet_ids:
            raise Refusal(f"address binding {reference} names absent wallet {wallet_ref}")
        return AddressBinding(reference, kind, wallet_ref, None, None, None)
    if kind == "literal":
        exact_keys(binding, {"kind", "address"}, f"address binding {reference} source")
        return AddressBinding(reference, kind, None, pubkey_text(binding["address"], f"address binding {reference}"), None, None)
    if kind == "input-json":
        exact_keys(binding, {"kind", "inputId", "pointer"}, f"address binding {reference} source")
        input_id = stable_id(binding["inputId"], f"address binding {reference} input")
        if input_id not in inputs:
            raise Refusal(f"address binding {reference} names absent input {input_id}")
        binding_pointer = text(binding["pointer"], f"address binding {reference} pointer", 256)
        if not binding_pointer.startswith("/"):
            raise Refusal(f"address binding {reference} pointer is not canonical")
        return AddressBinding(reference, kind, None, None, input_id, binding_pointer)
    raise Refusal(f"address binding {reference} has unknown source kind {kind}")


def parse_manifest(path: Path) -> Manifest:
    path = canonical_existing_file(path, "activity manifest")
    manifest_sha256 = sha256_file(path)
    value = exact_object(read_exact_json(path, "activity manifest"), "activity manifest")
    exact_keys(value, {"schema", "scenario", "target", "inputs", "addressBindings", "adapters"}, "activity manifest")
    if value["schema"] != MANIFEST_SCHEMA:
        raise Refusal(f"activity manifest schema is not {MANIFEST_SCHEMA}")
    scenario_source = exact_object(value["scenario"], "activity scenario binding")
    exact_keys(scenario_source, {"path", "sha256"}, "activity scenario binding")
    scenario_path = canonical_existing_file(text(scenario_source["path"], "scenario path"), "economic scenario")
    scenario = parse_scenario(scenario_path, digest_text(scenario_source["sha256"], "scenario digest"))

    target = exact_object(value["target"], "activity target")
    exact_keys(target, {"kind", "rpcUrl", "devnetGenesisHash"}, "activity target")
    kind = text(target["kind"], "activity target kind", 32)
    if kind != scenario.cluster_target:
        raise Refusal("activity target kind differs from the economic scenario")
    rpc_url = validate_rpc_url(target["rpcUrl"], kind)
    genesis_value = target["devnetGenesisHash"]
    if kind == "devnet":
        genesis = text(genesis_value, "devnet genesis hash", 64)
        if genesis != DEVNET_GENESIS_HASH:
            raise Refusal("activity target does not name Solana devnet's exact genesis hash")
    elif genesis_value is not None:
        raise Refusal("owned-loopback must not carry a devnet genesis acknowledgment")
    else:
        genesis = None

    inputs: dict[str, Path] = {}
    for index, raw in enumerate(exact_list(value["inputs"], "activity inputs")):
        source = exact_object(raw, f"activity input {index}")
        exact_keys(source, {"id", "path", "sha256"}, f"activity input {index}")
        input_id = stable_id(source["id"], f"activity input {index} id")
        if input_id in inputs:
            raise Refusal(f"activity manifest repeats input {input_id}")
        input_path = canonical_existing_file(text(source["path"], f"activity input {input_id} path"), f"activity input {input_id}")
        if sha256_file(input_path) != digest_text(source["sha256"], f"activity input {input_id} digest"):
            raise Refusal(f"activity input {input_id} changed")
        inputs[input_id] = input_path

    wallet_ids = {item.wallet_id for item in scenario.wallets}
    address_bindings: list[AddressBinding] = []
    bound_refs: set[str] = set()
    bound_literals: set[str] = set()
    for index, raw in enumerate(exact_list(value["addressBindings"], "activity address bindings")):
        binding = parse_address_binding(raw, index, inputs, wallet_ids)
        if binding.reference in bound_refs:
            raise Refusal(f"activity manifest repeats address binding {binding.reference}")
        if binding.address is not None and binding.address in bound_literals:
            raise Refusal(f"activity manifest aliases literal address {binding.address}")
        bound_refs.add(binding.reference)
        if binding.address is not None:
            bound_literals.add(binding.address)
        address_bindings.append(binding)

    operation_by_id = {item.operation_id: item for item in scenario.operations}
    covered: set[str] = set()
    adapters: list[AdapterSpec] = []
    adapter_ids: set[str] = set()
    for index, raw in enumerate(exact_list(value["adapters"], "activity adapters")):
        source = exact_object(raw, f"activity adapter {index}")
        exact_keys(source, {"id", "covers", "caller", "argv", "dependsOn", "wallets", "mutation", "completion"}, f"activity adapter {index}")
        adapter_id = stable_id(source["id"], f"activity adapter {index} id")
        if adapter_id in adapter_ids:
            raise Refusal(f"activity manifest repeats adapter {adapter_id}")
        adapter_ids.add(adapter_id)
        covers = tuple(stable_id(item, f"adapter {adapter_id} coverage") for item in exact_list(source["covers"], f"adapter {adapter_id} coverage"))
        if not covers or any(item not in operation_by_id or item in covered for item in covers):
            raise Refusal(f"adapter {adapter_id} has absent, empty, or repeated operation coverage")
        covered.update(covers)
        caller = text(source["caller"], f"adapter {adapter_id} caller", 32)
        if caller not in {"dclutch-cli", "successor"}:
            raise Refusal(f"adapter {adapter_id} caller is not a public dClutch caller")
        argv = tuple(text(item, f"adapter {adapter_id} argv", 1024) for item in exact_list(source["argv"], f"adapter {adapter_id} argv"))
        if not argv:
            raise Refusal(f"adapter {adapter_id} has no command")
        validate_caller_command(adapter_id, caller, argv, tuple(operation_by_id[item].kind for item in covers), kind)
        dependencies = tuple(stable_id(item, f"adapter {adapter_id} dependency") for item in exact_list(source["dependsOn"], f"adapter {adapter_id} dependencies"))
        adapter_wallets = tuple(stable_id(item, f"adapter {adapter_id} wallet") for item in exact_list(source["wallets"], f"adapter {adapter_id} wallets"))
        required_wallets = set().union(*(set(operation_by_id[item].wallet_ids) for item in covers))
        if len(set(adapter_wallets)) != len(adapter_wallets) or set(adapter_wallets) != required_wallets:
            raise Refusal(f"adapter {adapter_id} does not name the exact involved wallet set")
        mutation = source["mutation"]
        if not isinstance(mutation, bool):
            raise Refusal(f"adapter {adapter_id} mutation must be boolean")
        if any(operation_by_id[item].mutation_expected for item in covers) and not mutation:
            raise Refusal(f"adapter {adapter_id} disables a scenario operation with a committed mutating caller")
        completion = parse_completion(source["completion"], adapter_id)
        if mutation and not (
            completion.signature_pointers or completion.required_transaction_labels
        ):
            raise Refusal(f"mutating adapter {adapter_id} has no finalized-signature evidence pointer")
        if completion.schema == CAMPAIGN_REPORT_SCHEMA and (
            "checked-release" not in inputs or "market" not in inputs
        ):
            raise Refusal(
                f"campaign adapter {adapter_id} requires checked-release and market inputs"
            )
        adapters.append(AdapterSpec(adapter_id, covers, caller, argv, dependencies, tuple(sorted(adapter_wallets)), mutation, completion))
    if covered != set(operation_by_id):
        raise Refusal(f"activity adapters do not cover exactly the scenario operations: missing {sorted(set(operation_by_id) - covered)}")
    for adapter in adapters:
        if any(item not in adapter_ids or item == adapter.adapter_id for item in adapter.depends_on):
            raise Refusal(f"adapter {adapter.adapter_id} has an absent or self dependency")
    adapter_for_operation = {operation_id: adapter.adapter_id for adapter in adapters for operation_id in adapter.covers}
    adapter_by_id = {adapter.adapter_id: adapter for adapter in adapters}
    for adapter in adapters:
        required_dependencies = {
            adapter_for_operation[dependency]
            for operation_id in adapter.covers
            for dependency in operation_by_id[operation_id].depends_on
            if adapter_for_operation[dependency] != adapter.adapter_id
        }
        if set(adapter.depends_on) != required_dependencies:
            raise Refusal(f"adapter {adapter.adapter_id} dependencies do not preserve the exact scenario graph")
    require_acyclic({item.adapter_id: item.depends_on for item in adapters}, "activity adapter graph")
    if sum(len(item.completion.signature_pointers) for item in adapters) > scenario.limits.max_transactions:
        raise Refusal("activity adapter finalized-signature count exceeds maxTransactions")
    return Manifest(path, manifest_sha256, scenario, rpc_url, genesis, inputs, tuple(address_bindings), tuple(adapters))


def validate_caller_command(adapter_id: str, caller: str, argv: tuple[str, ...], kinds: tuple[str, ...], target: str) -> None:
    command = argv[0]
    dclutch = {
        "found": {"found"},
        "redeem": {"redeem"},
        "direct": {"intent"},
    }
    successor = {
        "found": {"campaign", "local-private-validator-lifecycle-v1"},
        "participant": {"devnet-user-position-admission-v1", "local-private-validator-user-position-admission-v1", "local-private-validator-lifecycle-v1"},
        "direct": {"devnet-direct-trade-v1", "local-private-validator-lifecycle-v1"},
        "resolve": {"devnet-terminal-sequence-v1", "local-private-validator-lifecycle-v1"},
        "redeem": {"devnet-terminal-sequence-v1", "local-private-validator-lifecycle-v1"},
        "retire": {"devnet-terminal-sequence-v1", "local-private-validator-lifecycle-v1"},
    }
    table = dclutch if caller == "dclutch-cli" else successor
    if any(command not in table.get(kind, set()) for kind in kinds):
        raise Refusal(f"adapter {adapter_id} command {command} is not accepted for {kinds}")
    if len(kinds) > 1 and command != "local-private-validator-lifecycle-v1":
        raise Refusal(f"adapter {adapter_id} covers multiple operations without the one full-lifecycle caller")
    if command == "local-private-validator-lifecycle-v1" and target != "owned-loopback":
        raise Refusal("the private-validator lifecycle caller is loopback-only")
    if command.startswith("devnet-") and target != "devnet":
        raise Refusal(f"adapter {adapter_id} offers a devnet caller to owned loopback")
    if target == "devnet" and command == "intent":
        raise Refusal("an off-chain Direct intent cannot count as devnet Direct mutation")


def pointer(value: Any, path: str, label: str) -> Any:
    if path == "":
        return value
    if not path.startswith("/"):
        raise Refusal(f"{label} is not a JSON pointer")
    current = value
    for raw in path[1:].split("/"):
        part = raw.replace("~1", "/").replace("~0", "~")
        if isinstance(current, dict) and part in current:
            current = current[part]
        elif isinstance(current, list) and part.isdigit() and int(part) < len(current):
            current = current[int(part)]
        else:
            raise Refusal(f"{label} pointer {path} is absent")
    return current


class Rpc:
    def __init__(self, url: str, *, minimum_interval_ms: int = 0, timeout: float = 20.0):
        self.url = url
        self.minimum_interval = minimum_interval_ms / 1000
        self.timeout = timeout
        self.sequence = 0
        self.last_call = 0.0

    def call(self, method: str, params: list[Any]) -> Any:
        elapsed = time.monotonic() - self.last_call
        if elapsed < self.minimum_interval:
            time.sleep(self.minimum_interval - elapsed)
        self.sequence += 1
        payload = json.dumps({"jsonrpc": "2.0", "id": self.sequence, "method": method, "params": params}).encode()
        req = urlrequest.Request(self.url, data=payload, method="POST", headers={"content-type": "application/json"})
        try:
            with urlrequest.urlopen(req, timeout=self.timeout) as response:
                body = response.read(4 * 1024 * 1024 + 1)
        except Exception as error:
            raise Refusal(f"RPC {method} failed: {error}") from error
        self.last_call = time.monotonic()
        if len(body) > 4 * 1024 * 1024:
            raise Refusal(f"RPC {method} response exceeds 4 MiB")
        try:
            parsed = json.loads(body)
        except json.JSONDecodeError as error:
            raise Refusal(f"RPC {method} returned non-JSON") from error
        if not isinstance(parsed, dict) or parsed.get("id") != self.sequence or parsed.get("jsonrpc") != "2.0" or "error" in parsed or "result" not in parsed:
            raise Refusal(f"RPC {method} returned another envelope: {parsed.get('error') if isinstance(parsed, dict) else 'not object'}")
        return parsed["result"]

    def genesis_hash(self) -> str:
        return text(self.call("getGenesisHash", []), "RPC genesis hash", 64)

    def balance(self, address: str) -> tuple[int, int]:
        result = exact_object(self.call("getBalance", [address, {"commitment": "finalized"}]), "getBalance result")
        context = exact_object(result.get("context"), "getBalance context")
        slot = context.get("slot")
        value = result.get("value")
        if not isinstance(slot, int) or not isinstance(value, int) or slot < 0 or value < 0 or value > 2**64 - 1:
            raise Refusal("getBalance returned another slot or lamport shape")
        return slot, value

    def transaction(self, signature: str) -> dict[str, Any] | None:
        result = self.call(
            "getTransaction",
            [signature, {"encoding": "jsonParsed", "commitment": "finalized", "maxSupportedTransactionVersion": 0}],
        )
        if result is None:
            return None
        return exact_object(result, "getTransaction result")

    def signatures_for_address(self, address: str, limit: int = 100, before: str | None = None) -> list[dict[str, Any]]:
        options: dict[str, Any] = {"commitment": "finalized", "limit": limit}
        if before is not None:
            options["before"] = before
        result = exact_list(self.call("getSignaturesForAddress", [address, options]), "getSignaturesForAddress result")
        return [exact_object(item, "signature row") for item in result]

    def all_signatures_for_address(self, address: str, ceiling: int) -> list[dict[str, Any]]:
        output: list[dict[str, Any]] = []
        before: str | None = None
        while len(output) <= ceiling:
            page_size = min(1_000, ceiling + 1 - len(output))
            rows = self.signatures_for_address(address, page_size, before)
            output.extend(rows)
            if not rows or len(rows) < page_size:
                break
            before = signature_text(rows[-1].get("signature"), "signature history cursor")
        if len(output) > ceiling:
            raise Refusal(f"wallet {address} history exceeds the bounded scenario ceiling")
        return output


def authenticate_cluster(manifest: Manifest, rpc: Rpc) -> str:
    genesis = rpc.genesis_hash()
    if manifest.scenario.cluster_target == "devnet":
        if genesis != DEVNET_GENESIS_HASH or genesis != manifest.devnet_genesis_hash:
            raise Refusal("RPC does not prove the exact acknowledged Solana devnet")
    elif genesis == DEVNET_GENESIS_HASH:
        raise Refusal("owned-loopback RPC unexpectedly answers with Solana devnet")
    return genesis


def authorization(path: Path, manifest: Manifest, *, allow_expired: bool = False) -> dict[str, Any]:
    value = exact_object(read_exact_json(canonical_existing_file(path, "live authorization"), "live authorization"), "live authorization")
    schema = value.get("schema")
    common = {
        "schema", "manifestSha256", "scenarioSha256", "devnetGenesisHash",
        "marketRef", "notBefore", "expiresAt", "authorization",
    }
    if schema == AUTHORIZATION_SCHEMA:
        exact_keys(value, common, "live authorization")
        phrase = "authorize-one-devnet-activity-run"
    elif schema == BOUNDED_AUTHORIZATION_SCHEMA:
        exact_keys(
            value,
            common
            | {
                "maxCycles",
                "maxSpendLamports",
                "maxFeeLamports",
                "prefundedWalletClosureSha256",
                "checkedReleaseSha256",
                "marketSha256",
                "acceptedHarnessSha256",
                "acceptedHarnessSourceCommit",
                "dclutchSha256",
                "successorSha256",
                "solanaKeygenSha256",
            },
            "live authorization",
        )
        phrase = "authorize-bounded-devnet-activity-live-send"
        max_cycles = value["maxCycles"]
        if not isinstance(max_cycles, int) or isinstance(max_cycles, bool) or not 1 <= max_cycles <= 72:
            raise Refusal("bounded live authorization maxCycles must be in 1..72")
        decimal(value["maxSpendLamports"], "bounded live authorization maxSpendLamports", positive=True)
        decimal(value["maxFeeLamports"], "bounded live authorization maxFeeLamports", positive=True)
        digest_text(
            value["prefundedWalletClosureSha256"],
            "bounded live authorization prefunded wallet closure digest",
        )
        for key in (
            "checkedReleaseSha256",
            "marketSha256",
            "acceptedHarnessSha256",
            "dclutchSha256",
            "successorSha256",
            "solanaKeygenSha256",
        ):
            digest_text(value[key], f"bounded live authorization {key}")
        source_commit = text(
            value["acceptedHarnessSourceCommit"],
            "bounded live authorization accepted harness source commit",
            40,
        )
        if COMMIT_RE.fullmatch(source_commit) is None:
            raise Refusal("bounded live authorization source commit is not canonical")
    else:
        raise Refusal("live authorization schema is not admitted")
    if value["manifestSha256"] != manifest.sha256 or value["scenarioSha256"] != manifest.scenario.sha256:
        raise Refusal("live authorization is not bound to this exact manifest and scenario")
    if value["devnetGenesisHash"] != DEVNET_GENESIS_HASH or value["marketRef"] != manifest.scenario.market_ref:
        raise Refusal("live authorization names another cluster or Market")
    if value["authorization"] != phrase:
        raise Refusal("live authorization lacks its exact authorization phrase")
    try:
        now = dt.datetime.now(dt.timezone.utc)
        not_before = dt.datetime.fromisoformat(text(value["notBefore"], "authorization notBefore").replace("Z", "+00:00"))
        expires = dt.datetime.fromisoformat(text(value["expiresAt"], "authorization expiresAt").replace("Z", "+00:00"))
    except ValueError as error:
        raise Refusal("live authorization timestamps are not RFC3339 timestamps") from error
    if not_before.tzinfo is None or expires.tzinfo is None or not_before >= expires or expires - not_before > dt.timedelta(hours=6):
        raise Refusal("live authorization is not one ordered at-most-six-hour window")
    if not allow_expired and not (not_before <= now < expires):
        raise Refusal("live authorization is outside its current window")
    return value


def bounded_live_authorization(
    path: Path, manifest: Manifest, *, allow_expired: bool = False
) -> tuple[str, int, int, int, str]:
    value = authorization(path, manifest, allow_expired=allow_expired)
    if value["schema"] != BOUNDED_AUTHORIZATION_SCHEMA:
        raise Refusal("live-send requires the bounded v2 authorization schema")
    max_cycles = value["maxCycles"]
    max_spend = decimal(
        value["maxSpendLamports"],
        "bounded live authorization maxSpendLamports",
        positive=True,
    )
    max_fee = decimal(
        value["maxFeeLamports"],
        "bounded live authorization maxFeeLamports",
        positive=True,
    )
    prefunded_closure_sha256 = digest_text(
        value["prefundedWalletClosureSha256"],
        "bounded live authorization prefunded wallet closure digest",
    )
    bankroll = sum(wallet.funding_lamports for wallet in manifest.scenario.wallets)
    if max_spend > bankroll:
        raise Refusal("live authorization maxSpendLamports exceeds scenario wallet bankroll")
    return (
        sha256_file(canonical_existing_file(path, "live authorization")),
        max_cycles,
        max_spend,
        max_fee,
        prefunded_closure_sha256,
    )


def require_live_authorization(manifest: Manifest, path: Path | None, *, allow_expired: bool = False) -> str | None:
    if manifest.scenario.cluster_target == "owned-loopback":
        if path is not None:
            raise Refusal("owned-loopback refuses a devnet live-authorization file")
        return None
    if path is None:
        raise Refusal("public devnet mutation is held until --live-authorization names this exact run")
    return sha256_file(canonical_existing_file(path, "live authorization")) if authorization(path, manifest, allow_expired=allow_expired) else None


def wallet_paths(work: Path) -> tuple[Path, Path, Path]:
    return work / "private" / "wallets", work / "private" / "wallet-index.json", work / "public" / "wallet-ledger.json"


def run_checked(argv: Sequence[str], *, stdout: int | Any = subprocess.PIPE, stderr: int | Any = subprocess.PIPE) -> subprocess.CompletedProcess[bytes]:
    try:
        return subprocess.run(list(argv), stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr, check=False)
    except OSError as error:
        raise Refusal(f"execute {redact_argv(argv)}: {error}") from error


def prepare_wallets(manifest: Manifest, work: Path, keygen: Path) -> dict[str, Any]:
    wallet_dir, private_index_path, public_ledger_path = wallet_paths(work)
    wallet_dir.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    wallet_dir.parent.chmod(0o700)
    wallet_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
    wallet_dir.chmod(0o700)
    if private_index_path.exists() != public_ledger_path.exists():
        raise Refusal("wallet preparation has only one of its private/public ledgers")
    if private_index_path.exists():
        private = authenticated_state(private_index_path, "private wallet index")
        public = authenticated_state(public_ledger_path, "public wallet ledger")
        if private.get("schema") != PRIVATE_INDEX_SCHEMA or public.get("schema") != WALLET_LEDGER_SCHEMA or private.get("scenarioSha256") != manifest.scenario.sha256 or public.get("scenarioSha256") != manifest.scenario.sha256:
            raise Refusal("existing wallet ledgers belong to another scenario")
        verify_wallet_files(private, public, manifest, keygen, work)
        return public

    private_rows: list[dict[str, Any]] = []
    public_rows: list[dict[str, Any]] = []
    created: list[Path] = []
    try:
        for spec in manifest.scenario.wallets:
            keypair_path = wallet_dir / f"{spec.wallet_id}.json"
            if keypair_path.exists() or keypair_path.is_symlink():
                raise Refusal(f"wallet path already exists: {keypair_path}")
            result = run_checked([str(keygen), "new", "--no-bip39-passphrase", "--silent", "--outfile", str(keypair_path)])
            if result.returncode != 0:
                raise Refusal(f"solana-keygen new refused for wallet {spec.wallet_id}: {result.stderr.decode(errors='replace')[-1000:]}")
            created.append(keypair_path)
            keypair_path.chmod(0o600)
            public_result = run_checked([str(keygen), "pubkey", str(keypair_path)])
            if public_result.returncode != 0:
                raise Refusal(f"solana-keygen pubkey refused for wallet {spec.wallet_id}")
            address = pubkey_text(public_result.stdout.decode().strip(), f"wallet {spec.wallet_id} address")
            private_rows.append({"id": spec.wallet_id, "address": address, "keypair": str(keypair_path)})
            public_rows.append({"id": spec.wallet_id, "address": address, "roles": list(spec.roles), "fundingLamports": str(spec.funding_lamports)})
        if len({row["address"] for row in public_rows}) != len(public_rows):
            raise Refusal("solana-keygen repeated a disposable wallet address")
        private_value = {
            "schema": PRIVATE_INDEX_SCHEMA,
            "manifestSha256": manifest.sha256,
            "scenarioSha256": manifest.scenario.sha256,
            "createdAt": utc_now(),
            "keygenSha256": sha256_file(keygen),
            "wallets": private_rows,
        }
        public_value = {
            "schema": WALLET_LEDGER_SCHEMA,
            "manifestSha256": manifest.sha256,
            "scenarioSha256": manifest.scenario.sha256,
            "scenarioId": manifest.scenario.scenario_id,
            "clusterTarget": manifest.scenario.cluster_target,
            "wallets": public_rows,
        }
        atomic_write_json(private_index_path, private_value)
        atomic_write_json(public_ledger_path, public_value, mode=0o644)
        return authenticated_state(public_ledger_path, "public wallet ledger")
    except BaseException:
        for path in created:
            try:
                path.unlink()
            except FileNotFoundError:
                pass
        raise


def verify_wallet_files(
    private: Mapping[str, Any],
    public: Mapping[str, Any],
    manifest: Manifest,
    keygen: Path,
    work: Path,
) -> None:
    if private.get("keygenSha256") != sha256_file(keygen):
        raise Refusal("wallet keygen binary changed since preparation")
    private_rows = exact_list(private.get("wallets"), "private wallet rows")
    public_rows = exact_list(public.get("wallets"), "public wallet rows")
    if len(private_rows) != len(manifest.scenario.wallets) or len(public_rows) != len(private_rows):
        raise Refusal("wallet ledger width differs from the scenario")
    for spec, private_raw, public_raw in zip(manifest.scenario.wallets, private_rows, public_rows, strict=True):
        private_row = exact_object(private_raw, f"private wallet {spec.wallet_id}")
        public_row = exact_object(public_raw, f"public wallet {spec.wallet_id}")
        if private_row.get("id") != spec.wallet_id or public_row.get("id") != spec.wallet_id or private_row.get("address") != public_row.get("address"):
            raise Refusal(f"wallet ledgers substitute {spec.wallet_id}")
        keypair = canonical_existing_file(text(private_row.get("keypair"), f"wallet {spec.wallet_id} keypair"), f"wallet {spec.wallet_id} keypair")
        expected_keypair = wallet_paths(work)[0] / f"{spec.wallet_id}.json"
        try:
            expected_keypair = expected_keypair.resolve(strict=True)
        except OSError as error:
            raise Refusal(f"wallet {spec.wallet_id} exact disposable key path is absent") from error
        if keypair != expected_keypair:
            raise Refusal(
                f"wallet {spec.wallet_id} keypair is not its exact disposable scenario path"
            )
        mode = stat.S_IMODE(keypair.stat().st_mode)
        if mode & 0o077:
            raise Refusal(f"wallet {spec.wallet_id} keypair is not private (mode {mode:o})")
        result = run_checked([str(keygen), "pubkey", str(keypair)])
        if result.returncode != 0 or result.stdout.decode().strip() != private_row.get("address"):
            raise Refusal(f"wallet {spec.wallet_id} keypair no longer derives its recorded address")


def sol_text(lamports: int) -> str:
    whole, fraction = divmod(lamports, 1_000_000_000)
    return f"{whole}.{fraction:09d}"


def funding_journal_path(work: Path, wallet_id: str) -> Path:
    return work / "journals" / "funding" / f"{wallet_id}.json"


def funding_closure_path(work: Path) -> Path:
    return work / "public" / "funding-closure.json"


def funding_journal_phases(manifest: Manifest, work: Path) -> dict[str, str]:
    journal_dir = work / "journals" / "funding"
    if not journal_dir.exists():
        return {}
    if journal_dir.is_symlink() or not journal_dir.is_dir():
        raise Refusal("funding journal directory changed kind")
    wallet_ids = {wallet.wallet_id for wallet in manifest.scenario.wallets}
    observed_paths = {path.stem: path for path in journal_dir.glob("*.json")}
    unknown = set(observed_paths) - wallet_ids
    if unknown:
        raise Refusal(f"funding journal directory carries unknown wallets {sorted(unknown)}")
    phases: dict[str, str] = {}
    for wallet_id, path in observed_paths.items():
        journal = authenticated_state(path, f"funding journal {wallet_id}")
        if journal.get("schema") != FUNDING_JOURNAL_SCHEMA or journal.get("manifestSha256") != manifest.sha256 or journal.get("scenarioSha256") != manifest.scenario.sha256 or journal.get("walletId") != wallet_id:
            raise Refusal(f"funding journal {wallet_id} belongs to another run")
        phase = text(journal.get("phase"), f"funding journal {wallet_id} phase", 16)
        if phase not in {"planned", "dispatching", "finalized"}:
            raise Refusal(f"funding journal {wallet_id} has unknown phase")
        phases[wallet_id] = phase
    return phases


def new_funding_journal(manifest: Manifest, wallet_id: str, address: str, funder: str, amount: int, authorization_sha256: str | None) -> dict[str, Any]:
    operation_nonce = os.urandom(16).hex()
    return {
        "schema": FUNDING_JOURNAL_SCHEMA,
        "manifestSha256": manifest.sha256,
        "scenarioSha256": manifest.scenario.sha256,
        "clusterTarget": manifest.scenario.cluster_target,
        "walletId": wallet_id,
        "walletAddress": address,
        "funderAddress": funder,
        "transferLamports": str(amount),
        "authorizationSha256": authorization_sha256,
        "operationNonce": operation_nonce,
        "memo": MEMO_PREFIX + operation_nonce,
        "phase": "planned",
        "plannedAt": utc_now(),
        "dispatchStartedAt": None,
        "signature": None,
        "finalizedAt": None,
        "slot": None,
        "feeLamports": None,
        "funderPreLamports": None,
        "funderPostLamports": None,
        "walletPreLamports": None,
        "walletPostLamports": None,
        "transactionSha256": None,
    }


def parse_signature_output(body: bytes) -> str | None:
    try:
        value = json.loads(body)
    except json.JSONDecodeError:
        return None
    candidates: list[str] = []

    def walk(item: Any, key: str | None = None) -> None:
        if isinstance(item, dict):
            for child_key, child in item.items():
                walk(child, child_key)
        elif isinstance(item, list):
            for child in item:
                walk(child, key)
        elif key in {"signature", "transactionSignature"} and isinstance(item, str):
            candidates.append(item)

    walk(value)
    return candidates[0] if len(candidates) == 1 else None


def account_keys(transaction: Mapping[str, Any]) -> list[str]:
    tx = exact_object(transaction.get("transaction"), "transaction body")
    message = exact_object(tx.get("message"), "transaction message")
    output: list[str] = []
    for raw in exact_list(message.get("accountKeys"), "transaction account keys"):
        if isinstance(raw, str):
            output.append(pubkey_text(raw, "transaction account key"))
        else:
            output.append(pubkey_text(exact_object(raw, "parsed account key").get("pubkey"), "transaction account key"))
    return output


def parsed_instructions(transaction: Mapping[str, Any]) -> list[dict[str, Any]]:
    tx = exact_object(transaction.get("transaction"), "transaction body")
    message = exact_object(tx.get("message"), "transaction message")
    return [exact_object(item, "parsed instruction") for item in exact_list(message.get("instructions"), "transaction instructions")]


def verify_funding_transaction(transaction: Mapping[str, Any], journal: Mapping[str, Any], signature: str) -> dict[str, Any]:
    slot = transaction.get("slot")
    meta = exact_object(transaction.get("meta"), "funding transaction meta")
    if not isinstance(slot, int) or slot < 0 or meta.get("err") is not None:
        raise Refusal("funding transaction is absent, failed, or has another slot")
    keys = account_keys(transaction)
    pre = exact_list(meta.get("preBalances"), "funding preBalances")
    post = exact_list(meta.get("postBalances"), "funding postBalances")
    if len(keys) != len(pre) or len(pre) != len(post) or any(not isinstance(item, int) or item < 0 for item in pre + post):
        raise Refusal("funding balance vectors do not cover the exact account list")
    funder = pubkey_text(journal.get("funderAddress"), "funding journal funder")
    wallet = pubkey_text(journal.get("walletAddress"), "funding journal wallet")
    if keys.count(funder) != 1 or keys.count(wallet) != 1:
        raise Refusal("funding transaction does not name one exact funder and wallet")
    funder_index = keys.index(funder)
    wallet_index = keys.index(wallet)
    amount = decimal(journal.get("transferLamports"), "funding journal amount", positive=True)
    fee = meta.get("fee")
    if not isinstance(fee, int) or fee < 0:
        raise Refusal("funding transaction has another fee shape")
    if post[wallet_index] - pre[wallet_index] != amount:
        raise Refusal("funding transaction did not credit the exact wallet amount")
    if pre[funder_index] - post[funder_index] != amount + fee:
        raise Refusal("funding transaction funder arithmetic is not transfer plus exact fee")
    memo = text(journal.get("memo"), "funding journal memo", 128)
    saw_transfer = False
    saw_memo = False
    for instruction in parsed_instructions(transaction):
        program = instruction.get("program")
        parsed = instruction.get("parsed")
        if program == "system" and isinstance(parsed, dict) and parsed.get("type") == "transfer":
            info = parsed.get("info")
            if isinstance(info, dict) and info.get("source") == funder and info.get("destination") == wallet and info.get("lamports") == amount:
                saw_transfer = True
        if program in {"spl-memo", "memo"} and parsed == memo:
            saw_memo = True
    # RPC versions represent Memo as an unparsed instruction.  The exact memo
    # must still occur in the transaction JSON, never only in logs.
    if memo in json.dumps(exact_object(transaction.get("transaction"), "transaction body"), separators=(",", ":")):
        saw_memo = True
    if not saw_transfer or not saw_memo:
        raise Refusal("funding transaction omits its exact System transfer or recovery memo")
    result = dict(journal)
    result.update(
        {
            "phase": "finalized",
            "signature": signature,
            "finalizedAt": utc_now(),
            "slot": str(slot),
            "feeLamports": str(fee),
            "funderPreLamports": str(pre[funder_index]),
            "funderPostLamports": str(post[funder_index]),
            "walletPreLamports": str(pre[wallet_index]),
            "walletPostLamports": str(post[wallet_index]),
            "transactionSha256": sha256_bytes(canonical_json(transaction)),
        }
    )
    return result


def recover_funding_signature(rpc: Rpc, journal: Mapping[str, Any]) -> str | None:
    recorded = journal.get("signature")
    if isinstance(recorded, str) and recorded:
        return recorded
    funder = pubkey_text(journal.get("funderAddress"), "funding journal funder")
    memo = text(journal.get("memo"), "funding journal memo", 128)
    matches: list[str] = []
    for row in rpc.signatures_for_address(funder, 100):
        signature = row.get("signature")
        if not isinstance(signature, str) or row.get("err") is not None:
            continue
        transaction = rpc.transaction(signature)
        if transaction is None:
            continue
        if memo not in json.dumps(transaction, separators=(",", ":")):
            continue
        try:
            verify_funding_transaction(transaction, journal, signature)
        except Refusal:
            continue
        matches.append(signature)
    if len(matches) > 1:
        raise Refusal("funding recovery memo appears in more than one exact transfer")
    return matches[0] if matches else None


def recover_funding_journals(
    manifest: Manifest,
    work: Path,
    rpc: Rpc,
    authorization_sha256: str | None,
) -> str:
    """Observe existing funding journals without opening a funder or wallet key."""
    phases = funding_journal_phases(manifest, work)
    submitted = {wallet_id for wallet_id, phase in phases.items() if phase in {"dispatching", "finalized"}}
    if not submitted:
        return "no-pending-funding"
    pending = False
    for wallet_id in sorted(submitted):
        path = funding_journal_path(work, wallet_id)
        journal = authenticated_state(path, f"funding journal {wallet_id}")
        if journal.get("authorizationSha256") != authorization_sha256:
            raise Refusal(f"funding journal {wallet_id} belongs to another live authorization")
        signature = recover_funding_signature(rpc, journal)
        if signature is None:
            if journal["phase"] == "finalized":
                raise Refusal(f"finalized funding {wallet_id} lost its exact transaction")
            pending = True
            continue
        transaction = rpc.transaction(signature)
        if transaction is None:
            pending = True
            continue
        final = verify_funding_transaction(transaction, journal, signature)
        if journal["phase"] != "finalized":
            atomic_write_json(path, final, mode=0o644)
    return "pending-funding" if pending else "funding-finalized"


def load_wallet_indexes(manifest: Manifest, work: Path, keygen: Path) -> tuple[dict[str, Any], dict[str, Any]]:
    _, private_path, public_path = wallet_paths(work)
    if not private_path.exists() or not public_path.exists():
        raise Refusal("prepare-wallets must complete before funding or activity")
    private = authenticated_state(private_path, "private wallet index")
    public = authenticated_state(public_path, "public wallet ledger")
    verify_wallet_files(private, public, manifest, keygen, work)
    return private, public


def fund_wallets(manifest: Manifest, work: Path, solana: Path, keygen: Path, funder_keypair: Path, live_authorization: Path | None, *, poll_only: bool) -> None:
    authorization_sha256 = require_live_authorization(manifest, live_authorization, allow_expired=poll_only)
    private, public = load_wallet_indexes(manifest, work, keygen)
    rpc = Rpc(manifest.rpc_url, minimum_interval_ms=manifest.scenario.limits.min_dispatch_interval_ms)
    genesis = authenticate_cluster(manifest, rpc)
    funder_result = run_checked([str(keygen), "pubkey", str(funder_keypair)])
    if funder_result.returncode != 0:
        raise Refusal("solana-keygen could not derive the funder address")
    funder = pubkey_text(funder_result.stdout.decode().strip(), "funder address")
    private_by_id = {row["id"]: row for row in exact_list(private["wallets"], "private wallets")}
    public_by_id = {row["id"]: row for row in exact_list(public["wallets"], "public wallets")}

    for wallet in manifest.scenario.wallets:
        journal_path = funding_journal_path(work, wallet.wallet_id)
        if journal_path.exists():
            journal = authenticated_state(journal_path, f"funding journal {wallet.wallet_id}")
        else:
            if poll_only:
                raise Refusal(f"poll-only funding has no journal for {wallet.wallet_id}")
            journal = new_funding_journal(manifest, wallet.wallet_id, public_by_id[wallet.wallet_id]["address"], funder, wallet.funding_lamports, authorization_sha256)
            atomic_write_json(journal_path, journal)
            journal = authenticated_state(journal_path, f"funding journal {wallet.wallet_id}")
        if journal.get("schema") != FUNDING_JOURNAL_SCHEMA or journal.get("manifestSha256") != manifest.sha256 or journal.get("scenarioSha256") != manifest.scenario.sha256:
            raise Refusal(f"funding journal {wallet.wallet_id} belongs to another run")
        if journal.get("authorizationSha256") != authorization_sha256:
            raise Refusal(f"funding journal {wallet.wallet_id} belongs to another live authorization")
        if journal.get("phase") == "finalized":
            signature = text(journal.get("signature"), f"funding journal {wallet.wallet_id} signature", 128)
            transaction = rpc.transaction(signature)
            if transaction is None:
                raise Refusal(f"finalized funding transaction {signature} disappeared")
            verify_funding_transaction(transaction, journal, signature)
            continue

        signature = recover_funding_signature(rpc, journal)
        if signature is not None:
            transaction = rpc.transaction(signature)
            if transaction is None:
                raise Refusal("recovered funding signature is not finalized; journal remains poll-only")
            atomic_write_json(journal_path, verify_funding_transaction(transaction, journal, signature))
            continue
        if poll_only or journal.get("phase") == "dispatching":
            raise Refusal(
                f"funding {wallet.wallet_id} is ambiguous and remains poll-only; no matching finalized memo was found"
            )
        if journal.get("phase") != "planned":
            raise Refusal(f"funding journal {wallet.wallet_id} has unknown phase {journal.get('phase')}")

        dispatching = dict(journal)
        dispatching["phase"] = "dispatching"
        dispatching["dispatchStartedAt"] = utc_now()
        atomic_write_json(journal_path, dispatching)
        journal = authenticated_state(journal_path, f"funding journal {wallet.wallet_id}")
        recipient = pubkey_text(public_by_id[wallet.wallet_id]["address"], f"wallet {wallet.wallet_id} address")
        result = run_checked(
            [
                str(solana), "transfer", "--url", manifest.rpc_url, "--keypair", str(funder_keypair),
                "--fee-payer", str(funder_keypair), "--commitment", "finalized", "--output", "json-compact",
                "--allow-unfunded-recipient", "--with-memo", text(journal["memo"], "funding memo", 128),
                recipient, sol_text(wallet.funding_lamports),
            ]
        )
        signature = parse_signature_output(result.stdout)
        if signature is not None:
            journal = dict(journal)
            journal["signature"] = signature
            atomic_write_json(journal_path, journal)
        if result.returncode != 0:
            raise Refusal(
                f"funding dispatch for {wallet.wallet_id} exited {result.returncode}; journal is ambiguous and will only poll"
            )
        if signature is None:
            signature = recover_funding_signature(rpc, journal)
        if signature is None:
            raise Refusal(f"funding dispatch for {wallet.wallet_id} returned no recoverable finalized signature")
        transaction = rpc.transaction(signature)
        if transaction is None:
            raise Refusal(f"funding signature {signature} is not finalized; rerun with --poll-only")
        atomic_write_json(journal_path, verify_funding_transaction(transaction, journal, signature))

    write_funding_closure(
        manifest,
        work,
        genesis,
        authorization_sha256,
        funder,
    )


def funding_closure_rows(
    manifest: Manifest,
    work: Path,
    funding_authorization_sha256: str | None,
    funder: str,
) -> tuple[list[dict[str, str]], int, int]:
    rows: list[dict[str, str]] = []
    total_transfer = 0
    total_fee = 0
    for wallet in manifest.scenario.wallets:
        path = funding_journal_path(work, wallet.wallet_id)
        journal = authenticated_state(path, f"funding journal {wallet.wallet_id}")
        if (
            journal.get("schema") != FUNDING_JOURNAL_SCHEMA
            or journal.get("manifestSha256") != manifest.sha256
            or journal.get("scenarioSha256") != manifest.scenario.sha256
            or journal.get("clusterTarget") != manifest.scenario.cluster_target
            or journal.get("walletId") != wallet.wallet_id
            or journal.get("phase") != "finalized"
            or journal.get("authorizationSha256") != funding_authorization_sha256
            or journal.get("funderAddress") != funder
        ):
            raise Refusal(f"funding closure refuses journal {wallet.wallet_id}")
        transfer = decimal(
            journal.get("transferLamports"),
            f"funding journal {wallet.wallet_id} transfer",
            positive=True,
        )
        fee = decimal(
            journal.get("feeLamports"),
            f"funding journal {wallet.wallet_id} fee",
        )
        if transfer != wallet.funding_lamports:
            raise Refusal(f"funding closure changed wallet {wallet.wallet_id} bankroll")
        row = {
            "walletId": wallet.wallet_id,
            "address": pubkey_text(
                journal.get("walletAddress"),
                f"funding journal {wallet.wallet_id} address",
            ),
            "journalPath": f"journals/funding/{wallet.wallet_id}.json",
            "journalSha256": sha256_file(path),
            "signature": signature_text(
                journal.get("signature"),
                f"funding journal {wallet.wallet_id} signature",
            ),
            "slot": str(
                decimal(
                    journal.get("slot"),
                    f"funding journal {wallet.wallet_id} slot",
                )
            ),
            "transferLamports": str(transfer),
            "feeLamports": str(fee),
        }
        rows.append(row)
        total_transfer += transfer
        total_fee += fee
        if total_transfer > 2**64 - 1 or total_fee > 2**64 - 1:
            raise Refusal("funding closure totals exceed u64")
    return rows, total_transfer, total_fee


def write_funding_closure(
    manifest: Manifest,
    work: Path,
    genesis: str,
    funding_authorization_sha256: str | None,
    funder: str,
) -> dict[str, Any]:
    rows, total_transfer, total_fee = funding_closure_rows(
        manifest, work, funding_authorization_sha256, funder
    )
    public_ledger = wallet_paths(work)[2]
    expected = {
        "schema": FUNDING_CLOSURE_SCHEMA,
        "manifestSha256": manifest.sha256,
        "scenarioSha256": manifest.scenario.sha256,
        "clusterTarget": manifest.scenario.cluster_target,
        "devnetGenesisHash": genesis,
        "walletLedgerSha256": sha256_file(public_ledger),
        "fundingAuthorizationSha256": funding_authorization_sha256,
        "funderAddress": funder,
        "wallets": rows,
        "totalTransferLamports": str(total_transfer),
        "totalFundingFeeLamports": str(total_fee),
    }
    path = funding_closure_path(work)
    if path.exists():
        prior = authenticated_state(path, "funding closure")
        exact_keys(prior, set(expected) | {"closedAt", "stateSha256"}, "funding closure")
        for key, value in expected.items():
            if prior.get(key) != value:
                raise Refusal(f"funding closure changed {key}")
        text(prior.get("closedAt"), "funding closure timestamp", 64)
        return prior
    atomic_write_json(path, {**expected, "closedAt": utc_now()}, mode=0o644)
    return authenticated_state(path, "funding closure")


def authenticate_funding_closure(
    manifest: Manifest, work: Path, expected_sha256: str
) -> dict[str, Any]:
    path = canonical_existing_file(funding_closure_path(work), "funding closure")
    if sha256_file(path) != expected_sha256:
        raise Refusal("funding closure differs from live authorization")
    value = authenticated_state(path, "funding closure")
    exact_keys(
        value,
        {
            "schema",
            "manifestSha256",
            "scenarioSha256",
            "clusterTarget",
            "devnetGenesisHash",
            "walletLedgerSha256",
            "fundingAuthorizationSha256",
            "funderAddress",
            "wallets",
            "totalTransferLamports",
            "totalFundingFeeLamports",
            "closedAt",
            "stateSha256",
        },
        "funding closure",
    )
    genesis = (
        DEVNET_GENESIS_HASH
        if manifest.scenario.cluster_target == "devnet"
        else manifest.scenario.genesis_hash
    )
    if (
        value.get("schema") != FUNDING_CLOSURE_SCHEMA
        or value.get("manifestSha256") != manifest.sha256
        or value.get("scenarioSha256") != manifest.scenario.sha256
        or value.get("clusterTarget") != manifest.scenario.cluster_target
        or value.get("devnetGenesisHash") != genesis
        or value.get("walletLedgerSha256") != sha256_file(wallet_paths(work)[2])
    ):
        raise Refusal("funding closure belongs to another run")
    funding_authorization = value.get("fundingAuthorizationSha256")
    if funding_authorization is not None:
        digest_text(funding_authorization, "funding closure authorization digest")
    funder = pubkey_text(value.get("funderAddress"), "funding closure funder")
    rows, total_transfer, total_fee = funding_closure_rows(
        manifest, work, funding_authorization, funder
    )
    if value.get("wallets") != rows:
        raise Refusal("funding closure wallet rows changed")
    if value.get("totalTransferLamports") != str(total_transfer):
        raise Refusal("funding closure transfer total changed")
    if value.get("totalFundingFeeLamports") != str(total_fee):
        raise Refusal("funding closure fee total changed")
    text(value.get("closedAt"), "funding closure timestamp", 64)
    return value


def adapter_journal_path(work: Path, adapter_id: str) -> Path:
    return work / "journals" / "activity" / f"{adapter_id}.json"


def stop_requested(work: Path) -> bool:
    path = work / "control" / "STOP.json"
    if not path.exists():
        return False
    value = authenticated_state(path, "activity stop control")
    exact_keys(value, {"schema", "requestedAt", "reason", "stateSha256"}, "activity stop control")
    if value["schema"] != STOP_SCHEMA:
        raise Refusal("activity stop control has another schema")
    text(value["requestedAt"], "activity stop timestamp", 64)
    text(value["reason"], "activity stop reason", 256)
    return True


def caller_binaries(dclutch_bin: Path, successor_bin: Path) -> dict[str, Path]:
    return {"dclutch-cli": dclutch_bin, "successor": successor_bin}


def probe_callers(manifest: Manifest, binaries: Mapping[str, Path]) -> dict[str, str]:
    """Prove each exact public command is dispatched before any wallet access."""
    digests = {name: sha256_file(path) for name, path in binaries.items()}
    observed: set[tuple[str, str]] = set()
    for adapter in manifest.adapters:
        key = (adapter.caller, adapter.argv[0])
        if key in observed:
            continue
        observed.add(key)
        binary = binaries[adapter.caller]
        try:
            result = subprocess.run(
                [str(binary), adapter.argv[0], "--help"],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
                timeout=20,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise Refusal(f"caller capability probe failed for {adapter.caller}/{adapter.argv[0]}: {error}") from error
        if len(result.stdout) + len(result.stderr) > 2 * 1024 * 1024:
            raise Refusal(f"caller capability probe for {adapter.argv[0]} exceeded 2 MiB")
        if result.returncode != 0:
            raise Refusal(f"accepted public caller does not dispatch {adapter.argv[0]}")
    return digests


def unverified_wallet_indexes(manifest: Manifest, work: Path) -> tuple[dict[str, Any], dict[str, Any]]:
    _, private_path, public_path = wallet_paths(work)
    if not private_path.exists() or not public_path.exists():
        raise Refusal("prepare-wallets must complete before activity")
    private = authenticated_state(private_path, "private wallet index")
    public = authenticated_state(public_path, "public wallet ledger")
    if private.get("schema") != PRIVATE_INDEX_SCHEMA or public.get("schema") != WALLET_LEDGER_SCHEMA:
        raise Refusal("wallet indexes have another schema")
    if private.get("manifestSha256") != manifest.sha256 or public.get("manifestSha256") != manifest.sha256:
        raise Refusal("wallet indexes belong to another activity manifest")
    private_rows = exact_list(private.get("wallets"), "private wallets")
    public_rows = exact_list(public.get("wallets"), "public wallets")
    if len(private_rows) != len(manifest.scenario.wallets) or len(public_rows) != len(private_rows):
        raise Refusal("wallet indexes have another width")
    return private, public


def expand_template(
    value: str,
    manifest: Manifest,
    work: Path,
    private_wallets: Mapping[str, Mapping[str, Any]],
    public_wallets: Mapping[str, Mapping[str, Any]],
) -> str:
    def replace(match: re.Match[str]) -> str:
        token = match.group(1)
        if token == "rpc":
            return manifest.rpc_url
        if token == "work":
            return str(work)
        if token == "devnetGenesis":
            if manifest.devnet_genesis_hash is None:
                raise Refusal("owned-loopback template requested a devnet genesis hash")
            return manifest.devnet_genesis_hash
        if token.startswith("input."):
            input_id = stable_id(token[6:], "template input id")
            if input_id not in manifest.inputs:
                raise Refusal(f"template names absent input {input_id}")
            return str(manifest.inputs[input_id])
        if token.startswith("wallet."):
            parts = token.split(".")
            if len(parts) != 3:
                raise Refusal(f"template has malformed wallet token {token}")
            wallet_id = stable_id(parts[1], "template wallet id")
            if wallet_id not in private_wallets or wallet_id not in public_wallets:
                raise Refusal(f"template names absent wallet {wallet_id}")
            if parts[2] == "keypair":
                return text(private_wallets[wallet_id].get("keypair"), f"wallet {wallet_id} keypair path")
            if parts[2] == "address":
                return pubkey_text(public_wallets[wallet_id].get("address"), f"wallet {wallet_id} address")
        raise Refusal(f"template contains unknown token {token}")

    expanded = TEMPLATE_RE.sub(replace, value)
    if "{{" in expanded or "}}" in expanded or len(expanded) > 8192:
        raise Refusal("template expansion left malformed or oversized content")
    return expanded


def expanded_adapter(
    adapter: AdapterSpec,
    manifest: Manifest,
    work: Path,
    private_wallets: Mapping[str, Mapping[str, Any]],
    public_wallets: Mapping[str, Mapping[str, Any]],
) -> tuple[tuple[str, ...], Path]:
    argv = tuple(expand_template(item, manifest, work, private_wallets, public_wallets) for item in adapter.argv)
    completion_text = expand_template(adapter.completion.path, manifest, work, private_wallets, public_wallets)
    completion_path = Path(completion_text)
    if not completion_path.is_absolute() or completion_path.is_symlink():
        raise Refusal(f"adapter {adapter.adapter_id} completion must be an absolute non-symlink path")
    if adapter.completion.schema == CAMPAIGN_REPORT_SCHEMA:
        expected_pairs = {
            "--plan": str(manifest.inputs["checked-release"]),
            "--market": str(manifest.inputs["market"]),
            "--evidence": str(completion_path),
        }
        for flag, expected in expected_pairs.items():
            if argv.count(flag) != 1:
                raise Refusal(
                    f"campaign adapter {adapter.adapter_id} must name {flag} exactly once"
                )
            index = argv.index(flag)
            if index + 1 >= len(argv) or argv[index + 1] != expected:
                raise Refusal(
                    f"campaign adapter {adapter.adapter_id} substitutes {flag}"
                )
        if argv.count("--execute") != 1:
            raise Refusal(
                f"campaign adapter {adapter.adapter_id} is not one exact executed campaign"
            )
    return argv, completion_path


def new_adapter_journal(
    manifest: Manifest,
    adapter: AdapterSpec,
    binary_sha256: str,
    argv: Sequence[str],
    completion_path: Path,
    authorization_sha256: str | None,
) -> dict[str, Any]:
    return {
        "schema": ADAPTER_JOURNAL_SCHEMA,
        "manifestSha256": manifest.sha256,
        "scenarioSha256": manifest.scenario.sha256,
        "clusterTarget": manifest.scenario.cluster_target,
        "adapterId": adapter.adapter_id,
        "covers": list(adapter.covers),
        "caller": adapter.caller,
        "command": adapter.argv[0],
        "binarySha256": binary_sha256,
        "argvSha256": sha256_bytes(canonical_json(list(argv))),
        "completionPathSha256": sha256_bytes(str(completion_path).encode()),
        "completionSpecSha256": sha256_bytes(canonical_json(dataclasses.asdict(adapter.completion))),
        "authorizationSha256": authorization_sha256,
        "phase": "planned",
        "plannedAt": utc_now(),
        "dispatchStartedAt": None,
        "processExitCode": None,
        "completionSha256": None,
        "signatures": [],
        "transactions": [],
        "finalizedAt": None,
    }


ADAPTER_JOURNAL_KEYS = {
    "schema", "manifestSha256", "scenarioSha256", "clusterTarget", "adapterId", "covers", "caller", "command",
    "binarySha256", "argvSha256", "completionPathSha256", "completionSpecSha256", "authorizationSha256", "phase",
    "plannedAt", "dispatchStartedAt", "processExitCode", "completionSha256", "signatures", "transactions", "finalizedAt",
    "stateSha256",
}


def validate_adapter_journal(
    journal: Mapping[str, Any], manifest: Manifest, adapter: AdapterSpec, binary_sha256: str, argv: Sequence[str], completion_path: Path,
    authorization_sha256: str | None = None,
) -> None:
    exact_keys(journal, ADAPTER_JOURNAL_KEYS, f"adapter journal {adapter.adapter_id}")
    expected = {
        "schema": ADAPTER_JOURNAL_SCHEMA,
        "manifestSha256": manifest.sha256,
        "scenarioSha256": manifest.scenario.sha256,
        "clusterTarget": manifest.scenario.cluster_target,
        "adapterId": adapter.adapter_id,
        "covers": list(adapter.covers),
        "caller": adapter.caller,
        "command": adapter.argv[0],
        "binarySha256": binary_sha256,
        "argvSha256": sha256_bytes(canonical_json(list(argv))),
        "completionPathSha256": sha256_bytes(str(completion_path).encode()),
        "completionSpecSha256": sha256_bytes(canonical_json(dataclasses.asdict(adapter.completion))),
    }
    for key, value in expected.items():
        if journal.get(key) != value:
            raise Refusal(f"adapter journal {adapter.adapter_id} changed {key}")
    if authorization_sha256 is not None and journal.get("authorizationSha256") != authorization_sha256:
        raise Refusal(f"adapter journal {adapter.adapter_id} belongs to another live authorization")
    if journal.get("phase") not in {"planned", "dispatching", "finalized"}:
        raise Refusal(f"adapter journal {adapter.adapter_id} has unknown phase")


def transaction_signatures(transaction: Mapping[str, Any]) -> list[str]:
    body = exact_object(transaction.get("transaction"), "transaction body")
    return [signature_text(item, "transaction signature") for item in exact_list(body.get("signatures"), "transaction signatures")]


def token_amount_rows(transaction: Mapping[str, Any], field: str, keys: Sequence[str]) -> dict[tuple[str, str], tuple[int, str | None]]:
    meta = exact_object(transaction.get("meta"), "transaction meta")
    output: dict[tuple[str, str], tuple[int, str | None]] = {}
    for index, raw in enumerate(exact_list(meta.get(field, []), field)):
        row = exact_object(raw, f"{field} row {index}")
        account_index = row.get("accountIndex")
        if not isinstance(account_index, int) or isinstance(account_index, bool) or account_index < 0 or account_index >= len(keys):
            raise Refusal(f"{field} row has an absent account index")
        mint = pubkey_text(row.get("mint"), f"{field} Mint")
        owner_value = row.get("owner")
        owner = None if owner_value is None else pubkey_text(owner_value, f"{field} owner")
        ui = exact_object(row.get("uiTokenAmount"), f"{field} token amount")
        amount = decimal(ui.get("amount"), f"{field} atom amount")
        key = (keys[account_index], mint)
        if key in output:
            raise Refusal(f"{field} repeats token account/Mint {key}")
        output[key] = (amount, owner)
    return output


def transaction_evidence(
    transaction: Mapping[str, Any],
    signature: str,
    wallet_addresses: Mapping[str, str],
    *,
    require_success: bool = True,
) -> dict[str, Any]:
    slot = transaction.get("slot")
    meta = exact_object(transaction.get("meta"), "transaction meta")
    error = meta.get("err")
    if not isinstance(slot, int) or isinstance(slot, bool) or slot < 0:
        raise Refusal(f"activity signature {signature} has another finalized slot")
    if require_success and error is not None:
        raise Refusal(f"required activity signature {signature} failed")
    if signature not in transaction_signatures(transaction):
        raise Refusal(f"RPC transaction substitutes activity signature {signature}")
    fee = meta.get("fee")
    if not isinstance(fee, int) or isinstance(fee, bool) or fee < 0:
        raise Refusal("activity transaction fee has another shape")
    keys = account_keys(transaction)
    pre = exact_list(meta.get("preBalances"), "activity preBalances")
    post = exact_list(meta.get("postBalances"), "activity postBalances")
    if len(keys) != len(pre) or len(pre) != len(post) or any(not isinstance(item, int) or isinstance(item, bool) or item < 0 for item in pre + post):
        raise Refusal("activity transaction balance vectors differ from exact account keys")
    wallet_deltas: dict[str, str] = {}
    for wallet_id, address in wallet_addresses.items():
        if keys.count(address) > 1:
            raise Refusal(f"activity transaction repeats wallet {wallet_id}")
        if address in keys:
            account_index = keys.index(address)
            wallet_deltas[wallet_id] = str(post[account_index] - pre[account_index])
    before_tokens = token_amount_rows(transaction, "preTokenBalances", keys)
    after_tokens = token_amount_rows(transaction, "postTokenBalances", keys)
    token_deltas: list[dict[str, Any]] = []
    for account_address, mint_address in sorted(set(before_tokens) | set(after_tokens)):
        before_amount, before_owner = before_tokens.get((account_address, mint_address), (0, None))
        after_amount, after_owner = after_tokens.get((account_address, mint_address), (0, None))
        if before_owner is not None and after_owner is not None and before_owner != after_owner:
            raise Refusal("activity token account owner changes across one transaction")
        token_deltas.append(
            {
                "accountAddress": account_address,
                "mintAddress": mint_address,
                "ownerAddress": after_owner if after_owner is not None else before_owner,
                "deltaAtoms": str(after_amount - before_amount),
            }
        )
    return {
        "signature": signature,
        "slot": str(slot),
        "succeeded": error is None,
        "errorSha256": None if error is None else sha256_bytes(canonical_json(error)),
        "feeLamports": str(fee),
        "transactionSha256": sha256_bytes(canonical_json(transaction)),
        "walletLamportDeltas": wallet_deltas,
        "tokenDeltas": token_deltas,
    }


def inspect_completion(
    manifest: Manifest,
    adapter: AdapterSpec,
    completion_path: Path,
    rpc: Rpc,
    wallet_addresses: Mapping[str, str],
) -> tuple[str, list[str], list[dict[str, Any]]] | None:
    if not completion_path.exists():
        return None
    completion_path = canonical_existing_file(completion_path, f"adapter {adapter.adapter_id} completion")
    value = read_exact_json(completion_path, f"adapter {adapter.adapter_id} completion")
    if adapter.completion.schema is not None:
        source = exact_object(value, f"adapter {adapter.adapter_id} completion")
        if source.get("schema") != adapter.completion.schema:
            raise Refusal(f"adapter {adapter.adapter_id} completion schema changed")
    for required_pointer, expected in adapter.completion.required_values.items():
        if pointer(value, required_pointer, f"adapter {adapter.adapter_id} required value") != expected:
            raise Refusal(f"adapter {adapter.adapter_id} completion changed {required_pointer}")
    required_success_signatures = [
        signature_text(
            pointer(value, item, f"adapter {adapter.adapter_id} signature"),
            f"adapter {adapter.adapter_id} signature",
        )
        for item in adapter.completion.signature_pointers
    ]
    signatures = list(required_success_signatures)
    if adapter.completion.transaction_list_pointer is not None:
        source = exact_object(value, f"adapter {adapter.adapter_id} campaign completion")
        expected_cluster = (
            "devnet"
            if manifest.scenario.cluster_target == "devnet"
            else "loopback"
        )
        if (
            source.get("cluster") != expected_cluster
            or source.get("mode") != "execute"
            or pointer(
                source,
                "/execution/completed",
                f"adapter {adapter.adapter_id} campaign completion",
            )
            is not True
            or source.get("plan_sha256")
            != sha256_file(manifest.inputs["checked-release"])
            or source.get("market_sha256") != sha256_file(manifest.inputs["market"])
        ):
            raise Refusal(
                f"adapter {adapter.adapter_id} campaign report changed cluster/mode/release/Market completion"
            )
        transaction_rows = exact_list(
            pointer(
                source,
                adapter.completion.transaction_list_pointer,
                f"adapter {adapter.adapter_id} transaction list",
            ),
            f"adapter {adapter.adapter_id} transaction list",
        )
        if not transaction_rows or len(transaction_rows) > manifest.scenario.limits.max_transactions:
            raise Refusal(
                f"adapter {adapter.adapter_id} transaction list is outside its scenario bound"
            )
        by_label: dict[str, str] = {}
        signatures = []
        for index, raw_row in enumerate(transaction_rows):
            row = exact_object(raw_row, f"adapter {adapter.adapter_id} transaction row {index}")
            label = text(
                row.get("label"),
                f"adapter {adapter.adapter_id} transaction label {index}",
                512,
            )
            if label in by_label:
                raise Refusal(f"adapter {adapter.adapter_id} repeats transaction label {label}")
            signature = signature_text(
                row.get("signature"),
                f"adapter {adapter.adapter_id} transaction signature {index}",
            )
            by_label[label] = signature
            signatures.append(signature)
        for label in adapter.completion.required_transaction_labels:
            if label not in by_label:
                raise Refusal(
                    f"adapter {adapter.adapter_id} omitted required transaction {label}"
                )
            required_success_signatures.append(by_label[label])
    if len(set(signatures)) != len(signatures):
        raise Refusal(f"adapter {adapter.adapter_id} completion repeats a signature")
    required_success_set = set(required_success_signatures)
    transactions: list[dict[str, Any]] = []
    for signature in signatures:
        transaction = rpc.transaction(signature)
        if transaction is None:
            return None
        transactions.append(
            transaction_evidence(
                transaction,
                signature,
                wallet_addresses,
                require_success=signature in required_success_set,
            )
        )
    return sha256_file(completion_path), signatures, transactions


class DispatchLimiter:
    def __init__(self, minimum_interval_ms: int):
        self.minimum_interval = minimum_interval_ms / 1000
        self.last_dispatch = 0.0
        self.lock = threading.Lock()

    def enter(self, work: Path) -> None:
        with self.lock:
            if stop_requested(work):
                raise Refusal("activity STOP prevents another dispatch")
            remaining = self.minimum_interval - (time.monotonic() - self.last_dispatch)
            if remaining > 0:
                time.sleep(remaining)
            if stop_requested(work):
                raise Refusal("activity STOP prevents another dispatch")
            self.last_dispatch = time.monotonic()


def await_completion(
    manifest: Manifest,
    adapter: AdapterSpec,
    journal_path: Path,
    completion_path: Path,
    rpc: Rpc,
    wallet_addresses: Mapping[str, str],
) -> dict[str, Any]:
    for poll_index in range(manifest.scenario.limits.max_polls):
        observed = inspect_completion(manifest, adapter, completion_path, rpc, wallet_addresses)
        if observed is not None:
            completion_sha256, signatures, transactions = observed
            journal = authenticated_state(journal_path, f"adapter journal {adapter.adapter_id}")
            final = dict(journal)
            final.update(
                {
                    "phase": "finalized",
                    "completionSha256": completion_sha256,
                    "signatures": signatures,
                    "transactions": transactions,
                    "finalizedAt": utc_now(),
                }
            )
            atomic_write_json(journal_path, final, mode=0o644)
            return final
        if poll_index + 1 < manifest.scenario.limits.max_polls:
            time.sleep(manifest.scenario.limits.poll_interval_ms / 1000)
    raise Refusal(f"adapter {adapter.adapter_id} remains ambiguous; only poll-only resume is allowed")


def dispatch_adapter(
    manifest: Manifest,
    adapter: AdapterSpec,
    binary: Path,
    binary_sha256: str,
    argv: tuple[str, ...],
    completion_path: Path,
    work: Path,
    rpc: Rpc,
    wallet_addresses: Mapping[str, str],
    authorization_sha256: str | None,
    limiter: DispatchLimiter,
) -> dict[str, Any]:
    journal_path = adapter_journal_path(work, adapter.adapter_id)
    if journal_path.exists():
        journal = authenticated_state(journal_path, f"adapter journal {adapter.adapter_id}")
        validate_adapter_journal(journal, manifest, adapter, binary_sha256, argv, completion_path, authorization_sha256)
        if journal["phase"] == "finalized":
            return await_completion(manifest, adapter, journal_path, completion_path, rpc, wallet_addresses)
        if journal["phase"] == "dispatching":
            return await_completion(manifest, adapter, journal_path, completion_path, rpc, wallet_addresses)
    else:
        journal = new_adapter_journal(manifest, adapter, binary_sha256, argv, completion_path, authorization_sha256)
        atomic_write_json(journal_path, journal, mode=0o644)
    if stop_requested(work):
        raise Refusal("activity STOP prevents another dispatch")
    limiter.enter(work)
    journal = authenticated_state(journal_path, f"adapter journal {adapter.adapter_id}")
    dispatching = dict(journal)
    dispatching["phase"] = "dispatching"
    dispatching["dispatchStartedAt"] = utc_now()
    atomic_write_json(journal_path, dispatching, mode=0o644)

    log_path = work / "private" / "logs" / f"{adapter.adapter_id}.log"
    log_path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    descriptor = os.open(log_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as log:
        result = run_checked([str(binary), *argv], stdout=log, stderr=subprocess.STDOUT)
        log.flush()
        os.fsync(log.fileno())
    journal = authenticated_state(journal_path, f"adapter journal {adapter.adapter_id}")
    exited = dict(journal)
    exited["processExitCode"] = result.returncode
    atomic_write_json(journal_path, exited, mode=0o644)
    try:
        return await_completion(manifest, adapter, journal_path, completion_path, rpc, wallet_addresses)
    except Refusal as error:
        if result.returncode != 0:
            raise Refusal(f"adapter {adapter.adapter_id} exited {result.returncode}; dispatch is ambiguous and poll-only") from error
        raise


def prefunded_closure_digest(
    manifest: Manifest,
    live_authorization: Path | None,
    *,
    allow_expired: bool,
) -> str | None:
    if live_authorization is None or manifest.scenario.cluster_target != "devnet":
        return None
    value = authorization(live_authorization, manifest, allow_expired=allow_expired)
    if value["schema"] != BOUNDED_AUTHORIZATION_SCHEMA:
        return None
    return digest_text(
        value["prefundedWalletClosureSha256"],
        "live authorization prefunded wallet closure digest",
    )


def require_finalized_funding(
    manifest: Manifest,
    work: Path,
    authorization_sha256: str | None = None,
    prefunded_closure_sha256: str | None = None,
) -> dict[str, Any] | None:
    if prefunded_closure_sha256 is not None:
        return authenticate_funding_closure(
            manifest, work, prefunded_closure_sha256
        )
    for wallet in manifest.scenario.wallets:
        path = funding_journal_path(work, wallet.wallet_id)
        if not path.exists():
            raise Refusal(f"activity requires exact finalized funding for wallet {wallet.wallet_id}")
        journal = authenticated_state(path, f"funding journal {wallet.wallet_id}")
        if journal.get("phase") != "finalized" or journal.get("authorizationSha256") != authorization_sha256:
            raise Refusal(f"activity requires exact finalized funding for wallet {wallet.wallet_id}")
    return None


def activity_journal_phases(manifest: Manifest, work: Path) -> dict[str, str]:
    journal_dir = work / "journals" / "activity"
    if not journal_dir.exists():
        return {}
    if journal_dir.is_symlink() or not journal_dir.is_dir():
        raise Refusal("activity journal directory changed kind")
    accepted_ids = {adapter.adapter_id for adapter in manifest.adapters}
    observed_paths = {path.stem: path for path in journal_dir.glob("*.json")}
    unknown = set(observed_paths) - accepted_ids
    if unknown:
        raise Refusal(f"activity journal directory carries unknown adapters {sorted(unknown)}")
    phases: dict[str, str] = {}
    for adapter_id, path in observed_paths.items():
        journal = authenticated_state(path, f"adapter journal {adapter_id}")
        if journal.get("schema") != ADAPTER_JOURNAL_SCHEMA or journal.get("manifestSha256") != manifest.sha256 or journal.get("scenarioSha256") != manifest.scenario.sha256 or journal.get("adapterId") != adapter_id:
            raise Refusal(f"adapter journal {adapter_id} belongs to another run")
        phase = text(journal.get("phase"), f"adapter journal {adapter_id} phase", 16)
        if phase not in {"planned", "dispatching", "finalized"}:
            raise Refusal(f"adapter journal {adapter_id} has unknown phase")
        phases[adapter_id] = phase
    return phases


def run_activity(
    manifest: Manifest,
    work: Path,
    dclutch_bin: Path,
    successor_bin: Path,
    keygen: Path | None,
    live_authorization: Path | None,
    *,
    poll_only: bool,
) -> str:
    binaries = caller_binaries(dclutch_bin, successor_bin)
    initial_phases = activity_journal_phases(manifest, work) if poll_only else {}
    submitted_ids = {adapter_id for adapter_id, phase in initial_phases.items() if phase in {"dispatching", "finalized"}}
    initial_funding_phases = funding_journal_phases(manifest, work) if poll_only else {}
    pending_funding_ids = {wallet_id for wallet_id, phase in initial_funding_phases.items() if phase == "dispatching"}
    if poll_only and not submitted_ids and not pending_funding_ids:
        return "no-pending-submissions"
    binary_digests = {name: sha256_file(path) for name, path in binaries.items()} if poll_only else probe_callers(manifest, binaries)
    authorization_sha256 = require_live_authorization(
        manifest, live_authorization, allow_expired=poll_only
    ) if any(item.mutation for item in manifest.adapters) else None
    closure_sha256 = prefunded_closure_digest(
        manifest, live_authorization, allow_expired=poll_only
    )
    rpc = Rpc(manifest.rpc_url, minimum_interval_ms=manifest.scenario.limits.min_dispatch_interval_ms)
    authenticate_cluster(manifest, rpc)
    if poll_only and pending_funding_ids:
        funding_recovery = recover_funding_journals(manifest, work, rpc, authorization_sha256)
        if funding_recovery == "pending-funding":
            return funding_recovery
        if not submitted_ids:
            return "funding-finalized"
    if poll_only:
        private, public = unverified_wallet_indexes(manifest, work)
    else:
        if keygen is None:
            raise Refusal("new activity dispatch requires solana-keygen wallet verification")
        private, public = load_wallet_indexes(manifest, work, keygen)
    require_finalized_funding(
        manifest, work, authorization_sha256, closure_sha256
    )
    private_wallets = {row["id"]: exact_object(row, "private wallet") for row in exact_list(private["wallets"], "private wallets")}
    public_wallets = {row["id"]: exact_object(row, "public wallet") for row in exact_list(public["wallets"], "public wallets")}
    wallet_addresses = {wallet_id: pubkey_text(row.get("address"), f"wallet {wallet_id} address") for wallet_id, row in public_wallets.items()}
    expanded = {
        adapter.adapter_id: expanded_adapter(adapter, manifest, work, private_wallets, public_wallets)
        for adapter in manifest.adapters
    }
    by_id = {item.adapter_id: item for item in manifest.adapters}
    completed: set[str] = set()
    for adapter in manifest.adapters:
        path = adapter_journal_path(work, adapter.adapter_id)
        argv, completion_path = expanded[adapter.adapter_id]
        if path.exists():
            journal = authenticated_state(path, f"adapter journal {adapter.adapter_id}")
            validate_adapter_journal(journal, manifest, adapter, binary_digests[adapter.caller], argv, completion_path, authorization_sha256)
            if journal["phase"] == "finalized":
                await_completion(manifest, adapter, path, completion_path, rpc, wallet_addresses)
                completed.add(adapter.adapter_id)
    if poll_only:
        for adapter in manifest.adapters:
            if adapter.adapter_id in completed:
                continue
            path = adapter_journal_path(work, adapter.adapter_id)
            if not path.exists():
                continue
            journal = authenticated_state(path, f"adapter journal {adapter.adapter_id}")
            argv, completion_path = expanded[adapter.adapter_id]
            validate_adapter_journal(journal, manifest, adapter, binary_digests[adapter.caller], argv, completion_path, authorization_sha256)
            if journal["phase"] == "dispatching":
                await_completion(manifest, adapter, path, completion_path, rpc, wallet_addresses)
            elif journal["phase"] != "planned":
                raise Refusal(f"poll-only activity found another adapter phase {adapter.adapter_id}")
        final_phases = activity_journal_phases(manifest, work)
        return "complete" if len(final_phases) == len(manifest.adapters) and set(final_phases.values()) == {"finalized"} else "partial-recovery"

    limiter = DispatchLimiter(manifest.scenario.limits.min_dispatch_interval_ms)
    running: dict[Future[dict[str, Any]], AdapterSpec] = {}
    used_wallets: set[str] = set()
    first_error: BaseException | None = None
    with ThreadPoolExecutor(max_workers=manifest.scenario.limits.max_concurrency) as executor:
        while len(completed) < len(manifest.adapters):
            if first_error is None and not stop_requested(work):
                for adapter in sorted(manifest.adapters, key=lambda item: item.adapter_id):
                    if adapter.adapter_id in completed or adapter in running.values():
                        continue
                    if not set(adapter.depends_on).issubset(completed) or set(adapter.wallet_ids) & used_wallets:
                        continue
                    argv, completion_path = expanded[adapter.adapter_id]
                    future = executor.submit(
                        dispatch_adapter,
                        manifest,
                        adapter,
                        binaries[adapter.caller],
                        binary_digests[adapter.caller],
                        argv,
                        completion_path,
                        work,
                        Rpc(manifest.rpc_url),
                        wallet_addresses,
                        authorization_sha256,
                        limiter,
                    )
                    running[future] = adapter
                    used_wallets.update(adapter.wallet_ids)
                    if len(running) >= manifest.scenario.limits.max_concurrency:
                        break
            if not running:
                if stop_requested(work):
                    raise Refusal("activity STOP left undispatched operations")
                if first_error is not None:
                    raise first_error
                raise Refusal("activity graph made no progress")
            done, _ = wait(tuple(running), return_when=FIRST_COMPLETED)
            for future in done:
                adapter = running.pop(future)
                used_wallets.difference_update(adapter.wallet_ids)
                try:
                    future.result()
                    completed.add(adapter.adapter_id)
                except BaseException as error:
                    if first_error is None:
                        first_error = error
        if first_error is not None:
            raise first_error
    return "complete"


def resolve_address_bindings(manifest: Manifest, public_wallets: Mapping[str, Mapping[str, Any]]) -> dict[str, str]:
    resolved: dict[str, str] = {}
    addresses: set[str] = set()
    for binding in manifest.address_bindings:
        if binding.kind == "wallet":
            assert binding.wallet_ref is not None
            address = pubkey_text(public_wallets[binding.wallet_ref].get("address"), f"binding {binding.reference}")
        elif binding.kind == "literal":
            assert binding.address is not None
            address = binding.address
        elif binding.kind == "input-json":
            assert binding.input_id is not None and binding.pointer is not None
            value = read_exact_json(manifest.inputs[binding.input_id], f"binding input {binding.input_id}")
            address = pubkey_text(pointer(value, binding.pointer, f"binding {binding.reference}"), f"binding {binding.reference}")
        else:  # Parser made this total; retain a local refusal at the trust boundary.
            raise Refusal(f"binding {binding.reference} has another source kind")
        if address in addresses:
            raise Refusal(f"runtime address binding aliases {address}")
        addresses.add(address)
        resolved[binding.reference] = address
    return resolved


def load_finalized_activity_journals(
    manifest: Manifest,
    work: Path,
    binaries: Mapping[str, Path],
    private_wallets: Mapping[str, Mapping[str, Any]],
    public_wallets: Mapping[str, Mapping[str, Any]],
    authorization_sha256: str | None,
) -> list[dict[str, Any]]:
    output: list[dict[str, Any]] = []
    binary_digests = {name: sha256_file(path) for name, path in binaries.items()}
    for adapter in manifest.adapters:
        path = adapter_journal_path(work, adapter.adapter_id)
        if not path.exists():
            raise Refusal(f"reconciliation has no activity journal for {adapter.adapter_id}")
        journal = authenticated_state(path, f"adapter journal {adapter.adapter_id}")
        argv, completion_path = expanded_adapter(adapter, manifest, work, private_wallets, public_wallets)
        validate_adapter_journal(journal, manifest, adapter, binary_digests[adapter.caller], argv, completion_path, authorization_sha256)
        if journal["phase"] != "finalized":
            raise Refusal(f"reconciliation refuses non-finalized adapter {adapter.adapter_id}")
        output.append(journal)
    return output


def reconcile_activity(
    manifest: Manifest,
    work: Path,
    dclutch_bin: Path,
    successor_bin: Path,
    keygen: Path | None,
    live_authorization: Path | None = None,
) -> dict[str, Any]:
    authorization_sha256 = require_live_authorization(manifest, live_authorization, allow_expired=True)
    private, public = unverified_wallet_indexes(manifest, work)
    closure_sha256 = prefunded_closure_digest(
        manifest, live_authorization, allow_expired=True
    )
    funding_closure = require_finalized_funding(
        manifest, work, authorization_sha256, closure_sha256
    )
    funding_authorization_sha256 = (
        authorization_sha256
        if funding_closure is None
        else funding_closure["fundingAuthorizationSha256"]
    )
    private_wallets = {row["id"]: exact_object(row, "private wallet") for row in exact_list(private["wallets"], "private wallets")}
    public_wallets = {row["id"]: exact_object(row, "public wallet") for row in exact_list(public["wallets"], "public wallets")}
    wallet_addresses = {wallet_id: pubkey_text(row.get("address"), f"wallet {wallet_id} address") for wallet_id, row in public_wallets.items()}
    bindings = resolve_address_bindings(manifest, public_wallets)
    journals = load_finalized_activity_journals(
        manifest, work, caller_binaries(dclutch_bin, successor_bin), private_wallets, public_wallets, authorization_sha256
    )
    rpc = Rpc(manifest.rpc_url, minimum_interval_ms=manifest.scenario.limits.min_dispatch_interval_ms)
    genesis = authenticate_cluster(manifest, rpc)

    seen_signatures: set[str] = set()
    funding_by_wallet: dict[str, dict[str, Any]] = {}
    wallet_signature_sets: dict[str, set[str]] = {wallet_id: set() for wallet_id in wallet_addresses}
    wallet_activity_deltas: dict[str, int] = {wallet_id: 0 for wallet_id in wallet_addresses}
    observed_bound_lamports: dict[str, int] = {}
    observed_tokens: dict[tuple[str, str], int] = {}
    observed_token_owners: dict[tuple[str, str], str | None] = {}
    activity_rows: list[dict[str, Any]] = []

    for wallet in manifest.scenario.wallets:
        journal = authenticated_state(funding_journal_path(work, wallet.wallet_id), f"funding journal {wallet.wallet_id}")
        if journal.get("authorizationSha256") != funding_authorization_sha256:
            raise Refusal(f"funding journal {wallet.wallet_id} belongs to another live authorization")
        signature = signature_text(journal.get("signature"), f"funding {wallet.wallet_id} signature")
        if signature in seen_signatures:
            raise Refusal(f"signature {signature} is reused across funding/activity evidence")
        transaction = rpc.transaction(signature)
        if transaction is None:
            raise Refusal(f"funding signature {signature} disappeared during reconciliation")
        final = verify_funding_transaction(transaction, journal, signature)
        seen_signatures.add(signature)
        wallet_signature_sets[wallet.wallet_id].add(signature)
        funding_by_wallet[wallet.wallet_id] = {
            "signature": signature,
            "transferLamports": final["transferLamports"],
            "feeLamports": final["feeLamports"],
            "walletPreLamports": final["walletPreLamports"],
            "walletPostLamports": final["walletPostLamports"],
            "funderPreLamports": final["funderPreLamports"],
            "funderPostLamports": final["funderPostLamports"],
            "transactionSha256": final["transactionSha256"],
        }

    for journal in journals:
        adapter_id = stable_id(journal["adapterId"], "activity journal adapter id")
        journal_signatures = [signature_text(item, f"adapter {adapter_id} signature") for item in exact_list(journal["signatures"], f"adapter {adapter_id} signatures")]
        captured_rows = exact_list(journal["transactions"], f"adapter {adapter_id} transactions")
        if len(journal_signatures) != len(captured_rows):
            raise Refusal(f"adapter {adapter_id} signature/capture width changed")
        refreshed: list[dict[str, Any]] = []
        for signature, captured_raw in zip(journal_signatures, captured_rows, strict=True):
            if signature in seen_signatures:
                raise Refusal(f"signature {signature} is reused across funding/activity evidence")
            transaction = rpc.transaction(signature)
            if transaction is None:
                raise Refusal(f"activity signature {signature} disappeared during reconciliation")
            evidence = transaction_evidence(
                transaction,
                signature,
                wallet_addresses,
                require_success=False,
            )
            if exact_object(captured_raw, f"adapter {adapter_id} captured transaction") != evidence:
                raise Refusal(f"adapter {adapter_id} captured transaction changed on finalized RPC")
            seen_signatures.add(signature)
            for wallet_id, delta in evidence["walletLamportDeltas"].items():
                wallet_signature_sets[wallet_id].add(signature)
                wallet_activity_deltas[wallet_id] += signed_decimal(delta, f"adapter {adapter_id} wallet delta")
            transaction_keys = account_keys(transaction)
            transaction_meta = exact_object(transaction.get("meta"), "reconciled transaction meta")
            transaction_pre = exact_list(transaction_meta.get("preBalances"), "reconciled preBalances")
            transaction_post = exact_list(transaction_meta.get("postBalances"), "reconciled postBalances")
            for reference, address in bindings.items():
                if address in transaction_keys:
                    account_index = transaction_keys.index(address)
                    observed_bound_lamports[reference] = observed_bound_lamports.get(reference, 0) + transaction_post[account_index] - transaction_pre[account_index]
            for token_raw in evidence["tokenDeltas"]:
                token = exact_object(token_raw, f"adapter {adapter_id} token delta")
                key = (pubkey_text(token["accountAddress"], "observed token account"), pubkey_text(token["mintAddress"], "observed token Mint"))
                observed_tokens[key] = observed_tokens.get(key, 0) + signed_decimal(token["deltaAtoms"], "observed token delta")
                owner = token["ownerAddress"]
                if key in observed_token_owners and owner is not None and observed_token_owners[key] not in {None, owner}:
                    raise Refusal("activity token evidence changes owner across transactions")
                if key not in observed_token_owners or owner is not None:
                    observed_token_owners[key] = owner
                if owner in wallet_addresses.values() and key[0] not in set(bindings.values()):
                    raise Refusal("activity changed an ephemeral-wallet token account absent from exact bindings")
            refreshed.append(evidence)
        activity_rows.append({"adapterId": adapter_id, "signatures": journal_signatures, "transactions": refreshed})

    expected_tokens: dict[tuple[str, str], int] = {}
    expected_lamports: dict[str, int] = {}
    for operation in manifest.scenario.operations:
        for account_ref, delta in operation.expected_lamport_deltas.items():
            if account_ref not in bindings:
                raise Refusal(f"expected lamport account {account_ref} has no exact address binding")
            expected_lamports[account_ref] = expected_lamports.get(account_ref, 0) + delta
        for token in operation.expected_token_deltas:
            if token.account_ref not in bindings or token.mint_ref not in bindings:
                raise Refusal(f"expected token {token.account_ref}/{token.mint_ref} has no exact address binding")
            key = (bindings[token.account_ref], bindings[token.mint_ref])
            expected_tokens[key] = expected_tokens.get(key, 0) + token.delta_atoms
            if token.wallet_ref is not None and observed_token_owners.get(key) != wallet_addresses[token.wallet_ref]:
                raise Refusal(f"observed token {token.account_ref} has another wallet authority")
    for account_ref, expected_delta in expected_lamports.items():
        if observed_bound_lamports.get(account_ref, 0) != expected_delta:
            raise Refusal(f"observed lamport delta for {account_ref} differs from expectedObservedDelta")
    observed_relevant = {
        key: delta
        for key, delta in observed_tokens.items()
        if key[0] in set(bindings.values()) or key[1] in set(bindings.values())
    }
    if observed_relevant != expected_tokens:
        raise Refusal(f"observed token deltas differ from scenario expectedObservedDelta: expected {expected_tokens}, observed {observed_relevant}")

    wallet_rows: list[dict[str, Any]] = []
    history_ceiling = manifest.scenario.limits.max_transactions + len(manifest.scenario.wallets)
    for wallet in manifest.scenario.wallets:
        address = wallet_addresses[wallet.wallet_id]
        history_rows = rpc.all_signatures_for_address(address, history_ceiling)
        history = {
            signature_text(row.get("signature"), f"wallet {wallet.wallet_id} history signature")
            for row in history_rows
        }
        if history != wallet_signature_sets[wallet.wallet_id]:
            raise Refusal(f"wallet {wallet.wallet_id} finalized history has missing or foreign signatures")
        _, final_lamports = rpc.balance(address)
        funding = funding_by_wallet[wallet.wallet_id]
        expected_final = decimal(funding["walletPostLamports"], f"wallet {wallet.wallet_id} funded balance") + wallet_activity_deltas[wallet.wallet_id]
        if final_lamports != expected_final:
            raise Refusal(f"wallet {wallet.wallet_id} final lamports do not reconcile exactly")
        wallet_rows.append(
            {
                "walletId": wallet.wallet_id,
                "address": address,
                "funding": funding,
                "activityLamportDelta": str(wallet_activity_deltas[wallet.wallet_id]),
                "finalLamports": str(final_lamports),
                "finalizedSignatures": sorted(history),
            }
        )
    result = {
        "schema": RECONCILIATION_SCHEMA,
        "manifestSha256": manifest.sha256,
        "scenarioSha256": manifest.scenario.sha256,
        "scenarioId": manifest.scenario.scenario_id,
        "clusterTarget": manifest.scenario.cluster_target,
        "genesisHash": genesis,
        "reconciledAt": utc_now(),
        "wallets": wallet_rows,
        "activity": activity_rows,
        "expectedObservedLamportDeltas": {key: str(value) for key, value in sorted(expected_lamports.items())},
        "expectedObservedTokenDeltas": [
            {"accountAddress": key[0], "mintAddress": key[1], "deltaAtoms": str(value)}
            for key, value in sorted(expected_tokens.items())
        ],
        "untrustedProjectionUsed": False,
    }
    atomic_write_json(work / "public" / "reconciliation.json", result, mode=0o644)
    return authenticated_state(work / "public" / "reconciliation.json", "activity reconciliation")


def supervisor_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="dclutch-wallet-harness supervisor-cycle-v1")
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--manifest-sha256", required=True)
    parser.add_argument("--scenario-id", required=True)
    parser.add_argument("--work", required=True)
    parser.add_argument("--rpc-url", required=True)
    parser.add_argument("--i-mean-devnet", required=True)
    parser.add_argument("--journal", required=True)
    parser.add_argument("--evidence-dir", required=True)
    parser.add_argument("--accepted-harness-sha256", required=True)
    parser.add_argument("--accepted-harness-source-commit", required=True)
    parser.add_argument("--scenario-sha256", required=True)
    parser.add_argument("--checked-release", required=True)
    parser.add_argument("--checked-release-sha256", required=True)
    parser.add_argument("--market", required=True)
    parser.add_argument("--market-sha256", required=True)
    parser.add_argument("--cycle-id", required=True)
    parser.add_argument("--dclutch-bin", required=True)
    parser.add_argument("--accepted-dclutch-sha256", required=True)
    parser.add_argument("--successor-bin", required=True)
    parser.add_argument("--accepted-successor-sha256", required=True)
    parser.add_argument("--solana-keygen-bin", required=True)
    parser.add_argument("--accepted-solana-keygen-sha256", required=True)
    parser.add_argument("--live-authorization")
    parser.add_argument("--live-authorization-sha256")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--no-send", action="store_true")
    mode.add_argument("--live-send", action="store_true")
    parser.add_argument("--poll-only", action="store_true")
    return parser


def require_tank_path(path: Path, label: str, *, directory: bool) -> Path:
    resolved = canonical_directory(path, label) if directory else canonical_existing_file(path, label)
    root = Path("/tank/dclutch-activity").resolve(strict=True)
    if not resolved.is_relative_to(root):
        raise Refusal(f"{label} must remain under /tank/dclutch-activity")
    return resolved


def validate_supervisor_rpc_join(supervisor_rpc_url: str, manifest_rpc_url: str) -> None:
    # Infra's fixed, reader-facing spelling omits the default HTTPS port. The
    # activity manifest retains its stronger explicit-port invariant. This one
    # exact join admits no other normalization or provider alias.
    if supervisor_rpc_url != DEVNET_SUPERVISOR_RPC_URL or manifest_rpc_url != DEVNET_MANIFEST_RPC_URL:
        raise Refusal("supervisor/manifest RPC spellings are not the one frozen devnet join")


def reconciled_wallet_debit_lamports(value: Mapping[str, Any]) -> int:
    if value.get("schema") != RECONCILIATION_SCHEMA:
        raise Refusal("supervisor reconciliation has another schema")
    debit = 0
    for activity_index, raw_activity in enumerate(
        exact_list(value.get("activity"), "reconciliation activity")
    ):
        activity_row = exact_object(
            raw_activity, f"reconciliation activity {activity_index}"
        )
        for transaction_index, raw_transaction in enumerate(
            exact_list(
                activity_row.get("transactions"),
                f"reconciliation activity {activity_index} transactions",
            )
        ):
            transaction = exact_object(
                raw_transaction,
                f"reconciliation activity {activity_index} transaction {transaction_index}",
            )
            deltas = exact_object(
                transaction.get("walletLamportDeltas"),
                f"reconciliation activity {activity_index} transaction {transaction_index} wallet deltas",
            )
            for wallet_id, raw_delta in deltas.items():
                stable_id(wallet_id, "reconciled wallet id")
                delta = signed_decimal(
                    raw_delta,
                    f"reconciliation activity {activity_index} transaction {transaction_index} wallet {wallet_id} delta",
                )
                if delta < 0:
                    debit += -delta
                if debit > 2**64 - 1:
                    raise Refusal("reconciled wallet debit exceeds u64")
    return debit


def reconciled_activity_fee_lamports(value: Mapping[str, Any]) -> int:
    if value.get("schema") != RECONCILIATION_SCHEMA:
        raise Refusal("supervisor reconciliation has another schema")
    total = 0
    for activity_index, raw_activity in enumerate(
        exact_list(value.get("activity"), "reconciliation activity")
    ):
        activity_row = exact_object(
            raw_activity, f"reconciliation activity {activity_index}"
        )
        for transaction_index, raw_transaction in enumerate(
            exact_list(
                activity_row.get("transactions"),
                f"reconciliation activity {activity_index} transactions",
            )
        ):
            transaction = exact_object(
                raw_transaction,
                f"reconciliation activity {activity_index} transaction {transaction_index}",
            )
            total += decimal(
                transaction.get("feeLamports"),
                f"reconciliation activity {activity_index} transaction {transaction_index} fee",
            )
            if total > 2**64 - 1:
                raise Refusal("reconciled activity fees exceed u64")
    return total


def supervisor_cycle(arguments: argparse.Namespace) -> None:
    manifest_path = canonical_existing_file(arguments.manifest, "supervisor activity manifest")
    manifest_sha256 = digest_text(arguments.manifest_sha256, "supervisor manifest digest")
    if sha256_file(manifest_path) != manifest_sha256:
        raise Refusal("supervisor manifest digest changed")
    manifest = parse_manifest(manifest_path)
    if manifest.scenario.cluster_target != "devnet" or arguments.i_mean_devnet != DEVNET_GENESIS_HASH:
        raise Refusal("supervisor-cycle-v1 is exact-devnet-only")
    if arguments.scenario_id != manifest.scenario.scenario_id:
        raise Refusal("supervisor scenario or RPC differs from the manifest")
    scenario_sha256 = digest_text(arguments.scenario_sha256, "supervisor scenario digest")
    if scenario_sha256 != manifest.scenario.sha256:
        raise Refusal("supervisor scenario digest differs from the manifest")
    validate_supervisor_rpc_join(arguments.rpc_url, manifest.rpc_url)
    work = require_tank_path(Path(arguments.work), "supervisor work", directory=True)
    expected_work = Path("/tank/dclutch-activity/runs") / manifest.sha256
    if work != expected_work.resolve(strict=True):
        raise Refusal("supervisor work is not /tank/dclutch-activity/runs/<manifest-sha256>")
    journal_path = require_tank_path(Path(arguments.journal), "supervisor request journal", directory=False)
    evidence_dir = require_tank_path(Path(arguments.evidence_dir), "supervisor evidence directory", directory=True)
    harness_path = canonical_existing_file(Path(__file__).resolve(), "accepted activity harness")
    harness_sha256 = digest_text(arguments.accepted_harness_sha256, "accepted harness digest")
    if sha256_file(harness_path) != harness_sha256:
        raise Refusal("installed activity harness differs from its accepted SHA-256")
    source_commit = text(arguments.accepted_harness_source_commit, "accepted harness source commit", 40)
    if COMMIT_RE.fullmatch(source_commit) is None:
        raise Refusal("accepted harness source commit must be one full lowercase Git commit")
    cycle_id = stable_id(arguments.cycle_id, "supervisor cycle id")
    checked_release = canonical_existing_file(arguments.checked_release, "supervisor checked release")
    checked_release_sha256 = digest_text(
        arguments.checked_release_sha256, "supervisor checked release digest"
    )
    if sha256_file(checked_release) != checked_release_sha256:
        raise Refusal("supervisor checked release digest changed")
    market = canonical_existing_file(arguments.market, "supervisor Market artifact")
    market_sha256 = digest_text(arguments.market_sha256, "supervisor Market digest")
    if sha256_file(market) != market_sha256:
        raise Refusal("supervisor Market artifact digest changed")
    if (
        manifest.inputs.get("checked-release") != checked_release
        or manifest.inputs.get("market") != market
    ):
        raise Refusal(
            "supervisor checked release/Market are not the exact manifest campaign inputs"
        )
    dclutch_bin = canonical_existing_file(arguments.dclutch_bin, "supervisor dclutch CLI", executable=True)
    successor_bin = canonical_existing_file(arguments.successor_bin, "supervisor successor CLI", executable=True)
    keygen_bin = canonical_existing_file(
        arguments.solana_keygen_bin, "supervisor solana-keygen", executable=True
    )
    dclutch_sha256 = digest_text(arguments.accepted_dclutch_sha256, "accepted dclutch digest")
    successor_sha256 = digest_text(
        arguments.accepted_successor_sha256, "accepted successor digest"
    )
    keygen_sha256 = digest_text(
        arguments.accepted_solana_keygen_sha256, "accepted solana-keygen digest"
    )
    for path, expected, label in (
        (dclutch_bin, dclutch_sha256, "dclutch CLI"),
        (successor_bin, successor_sha256, "successor CLI"),
        (keygen_bin, keygen_sha256, "solana-keygen"),
    ):
        if sha256_file(path) != expected:
            raise Refusal(f"supervisor {label} differs from its accepted digest")
    dispatch_mode = "live-send" if arguments.live_send else "no-send"
    send_allowed = os.environ.get("DCLUTCH_ACTIVITY_SEND_ALLOWED")
    if send_allowed != ("1" if arguments.live_send else "0"):
        raise Refusal("supervisor explicit mode differs from DCLUTCH_ACTIVITY_SEND_ALLOWED")
    mode = "poll-only" if arguments.poll_only else dispatch_mode
    request_state = authenticated_state(journal_path, "supervisor request journal")
    exact_keys(
        request_state,
        {
            "schema", "manifestSha256", "scenarioId", "workPath", "supervisorRpcUrl", "manifestRpcUrl", "devnetGenesisHash",
            "acceptedHarnessSha256", "acceptedHarnessSourceCommit", "scenarioSha256",
            "checkedReleaseSha256", "marketSha256", "dclutchSha256", "successorSha256",
            "solanaKeygenSha256", "cycleId", "requestedAt", "mode", "dispatchMode",
            "evidenceDirectory", "liveAuthorizationSha256", "authorizationMaxCycles",
            "authorizationMaxSpendLamports", "authorizationMaxFeeLamports",
            "prefundedWalletClosureSha256", "stateSha256",
        },
        "supervisor request journal",
    )
    live_path = None if arguments.live_authorization is None else canonical_existing_file(arguments.live_authorization, "supervisor live authorization")
    live_sha = None if live_path is None else sha256_file(live_path)
    if (arguments.live_authorization_sha256 is None) != (live_path is None):
        raise Refusal("supervisor live authorization path/digest must be both present or absent")
    if live_sha is not None and digest_text(arguments.live_authorization_sha256, "supervisor live authorization digest") != live_sha:
        raise Refusal("supervisor live authorization digest changed")
    authorization_max_cycles = None
    authorization_max_spend = None
    authorization_max_fee = None
    prefunded_wallet_closure_sha256 = None
    if live_path is not None and arguments.live_send:
        (
            _,
            authorization_max_cycles,
            authorization_max_spend,
            authorization_max_fee,
            prefunded_wallet_closure_sha256,
        ) = bounded_live_authorization(
            live_path, manifest, allow_expired=arguments.poll_only
        )
        if authorization_max_cycles != 1:
            raise Refusal("this lifecycle instance requires bounded authorization maxCycles 1")
        live_value = authorization(
            live_path, manifest, allow_expired=arguments.poll_only
        )
        authorization_pins = {
            "checkedReleaseSha256": checked_release_sha256,
            "marketSha256": market_sha256,
            "acceptedHarnessSha256": harness_sha256,
            "acceptedHarnessSourceCommit": source_commit,
            "dclutchSha256": dclutch_sha256,
            "successorSha256": successor_sha256,
            "solanaKeygenSha256": keygen_sha256,
        }
        for key, expected in authorization_pins.items():
            if live_value.get(key) != expected:
                raise Refusal(f"bounded live authorization changed {key}")
        assert prefunded_wallet_closure_sha256 is not None
        authenticate_funding_closure(
            manifest, work, prefunded_wallet_closure_sha256
        )
    expected_request = {
        "schema": SUPERVISOR_REQUEST_SCHEMA,
        "manifestSha256": manifest.sha256,
        "scenarioId": manifest.scenario.scenario_id,
        "workPath": str(work),
        "supervisorRpcUrl": arguments.rpc_url,
        "manifestRpcUrl": manifest.rpc_url,
        "devnetGenesisHash": DEVNET_GENESIS_HASH,
        "acceptedHarnessSha256": harness_sha256,
        "acceptedHarnessSourceCommit": source_commit,
        "scenarioSha256": scenario_sha256,
        "checkedReleaseSha256": checked_release_sha256,
        "marketSha256": market_sha256,
        "dclutchSha256": dclutch_sha256,
        "successorSha256": successor_sha256,
        "solanaKeygenSha256": keygen_sha256,
        "cycleId": cycle_id,
        "mode": mode,
        "dispatchMode": dispatch_mode,
        "evidenceDirectory": str(evidence_dir),
        "liveAuthorizationSha256": live_sha,
        "authorizationMaxCycles": authorization_max_cycles,
        "authorizationMaxSpendLamports": (
            None if authorization_max_spend is None else str(authorization_max_spend)
        ),
        "authorizationMaxFeeLamports": (
            None if authorization_max_fee is None else str(authorization_max_fee)
        ),
        "prefundedWalletClosureSha256": prefunded_wallet_closure_sha256,
    }
    for key, value in expected_request.items():
        if request_state.get(key) != value:
            raise Refusal(f"supervisor request journal changed {key}")
    text(request_state.get("requestedAt"), "supervisor request timestamp", 64)

    new_dispatches = 0
    reconciled_debit = None
    reconciled_activity_fee = None
    if mode == "no-send":
        if live_path is None:
            raise Refusal("no-send readiness requires one current exact live authorization")
        require_live_authorization(manifest, live_path)
        probe_callers(manifest, caller_binaries(dclutch_bin, successor_bin))
        rpc = Rpc(manifest.rpc_url, minimum_interval_ms=manifest.scenario.limits.min_dispatch_interval_ms)
        authenticate_cluster(manifest, rpc)
        reconciliation_sha256 = None
        status = "ready-no-send"
    elif mode == "live-send":
        if live_path is None:
            raise Refusal("live-send requires one current exact bounded authorization")
        bounded_live_authorization(live_path, manifest)
        if stop_requested(work):
            raise Refusal("activity STOP prevents live-send")
        before = activity_journal_phases(manifest, work)
        run_activity(
            manifest,
            work,
            dclutch_bin,
            successor_bin,
            keygen_bin,
            live_path,
            poll_only=False,
        )
        reconciliation = reconcile_activity(
            manifest, work, dclutch_bin, successor_bin, keygen_bin, live_path
        )
        reconciliation_sha256 = sha256_file(work / "public" / "reconciliation.json")
        reconciled_debit = reconciled_wallet_debit_lamports(reconciliation)
        reconciled_activity_fee = reconciled_activity_fee_lamports(reconciliation)
        assert authorization_max_spend is not None
        assert authorization_max_fee is not None
        if reconciled_debit > authorization_max_spend:
            raise Refusal("finalized reconciliation exceeds authorization maxSpendLamports")
        if reconciled_activity_fee > authorization_max_fee:
            raise Refusal("finalized reconciliation exceeds authorization maxFeeLamports")
        after = activity_journal_phases(manifest, work)
        new_dispatches = sum(
            1
            for adapter_id, phase in after.items()
            if phase in {"dispatching", "finalized"}
            and before.get(adapter_id) not in {"dispatching", "finalized"}
        )
        status = "complete-reconciled-live-send"
    else:
        phases = activity_journal_phases(manifest, work)
        submitted = {adapter_id for adapter_id, phase in phases.items() if phase in {"dispatching", "finalized"}}
        funding_phases = funding_journal_phases(manifest, work)
        pending_funding = {wallet_id for wallet_id, phase in funding_phases.items() if phase == "dispatching"}
        if not submitted and not pending_funding:
            if live_path is not None:
                raise Refusal("fresh poll-only recovery must not carry an authorization affordance")
            rpc = Rpc(manifest.rpc_url, minimum_interval_ms=manifest.scenario.limits.min_dispatch_interval_ms)
            authenticate_cluster(manifest, rpc)
            reconciliation_sha256 = None
            status = "no-pending-submissions"
        else:
            if live_path is None:
                raise Refusal("poll-only supervisor recovery requires the original authorization")
            recovery = run_activity(manifest, work, dclutch_bin, successor_bin, None, live_path, poll_only=True)
            if recovery == "complete":
                reconcile_activity(manifest, work, dclutch_bin, successor_bin, None, live_path)
                reconciliation_sha256 = sha256_file(work / "public" / "reconciliation.json")
                reconciled_debit = reconciled_wallet_debit_lamports(
                    authenticated_state(
                        work / "public" / "reconciliation.json",
                        "activity reconciliation",
                    )
                )
                reconciled_activity_fee = reconciled_activity_fee_lamports(
                    authenticated_state(
                        work / "public" / "reconciliation.json",
                        "activity reconciliation",
                    )
                )
                if authorization_max_spend is not None and reconciled_debit > authorization_max_spend:
                    raise Refusal("recovered reconciliation exceeds authorization maxSpendLamports")
                if authorization_max_fee is not None and reconciled_activity_fee > authorization_max_fee:
                    raise Refusal("recovered reconciliation exceeds authorization maxFeeLamports")
                status = "complete-reconciled-poll-only"
            elif recovery == "pending-funding":
                reconciliation_sha256 = None
                status = "pending-funding-poll-only"
            elif recovery == "funding-finalized":
                reconciliation_sha256 = None
                status = "funding-recovered-no-pending-activity"
            else:
                reconciliation_sha256 = None
                status = "partial-reconciled-poll-only"
    request_sha256 = sha256_file(journal_path)
    status_path = evidence_dir / f"{manifest.sha256}.{cycle_id}.{request_sha256}.supervisor-status.json"
    result = {
        "schema": SUPERVISOR_STATUS_SCHEMA,
        "manifestSha256": manifest.sha256,
        "scenarioSha256": manifest.scenario.sha256,
        "scenarioId": manifest.scenario.scenario_id,
        "supervisorRequestSha256": sha256_file(journal_path),
        "acceptedHarnessSha256": harness_sha256,
        "acceptedHarnessSourceCommit": source_commit,
        "cycleId": cycle_id,
        "mode": mode,
        "status": status,
        "completedAt": utc_now(),
        "reconciliationSha256": reconciliation_sha256,
        "newDispatches": str(new_dispatches),
        "authorizationMaxCycles": authorization_max_cycles,
        "authorizationMaxSpendLamports": (
            None if authorization_max_spend is None else str(authorization_max_spend)
        ),
        "authorizationMaxFeeLamports": (
            None if authorization_max_fee is None else str(authorization_max_fee)
        ),
        "prefundedWalletClosureSha256": prefunded_wallet_closure_sha256,
        "reconciledWalletDebitLamports": (
            None if reconciled_debit is None else str(reconciled_debit)
        ),
        "reconciledActivityFeeLamports": (
            None if reconciled_activity_fee is None else str(reconciled_activity_fee)
        ),
    }
    if status_path.exists():
        prior = authenticated_state(status_path, "supervisor status")
        for key in (
            "schema", "manifestSha256", "scenarioSha256", "scenarioId",
            "supervisorRequestSha256", "acceptedHarnessSha256",
            "acceptedHarnessSourceCommit", "cycleId", "mode", "status",
            "reconciliationSha256", "newDispatches", "authorizationMaxCycles",
            "authorizationMaxSpendLamports", "authorizationMaxFeeLamports",
            "prefundedWalletClosureSha256", "reconciledWalletDebitLamports",
            "reconciledActivityFeeLamports",
        ):
            if prior.get(key) != result[key]:
                raise Refusal("existing supervisor status belongs to another request or result")
        return
    atomic_write_json(status_path, result, mode=0o644)


def validate_only(manifest: Manifest) -> None:
    # Parsing is the validation. Keep an explicit function so the command has
    # no reason to construct an RPC or inspect a wallet.
    if not manifest.adapters:
        raise Refusal("activity manifest has no caller-backed adapters")


def stop(work: Path, reason: str) -> None:
    path = work / "control" / "STOP.json"
    if path.exists():
        authenticated_state(path, "activity stop control")
        return
    atomic_write_json(
        path,
        {"schema": STOP_SCHEMA, "requestedAt": utc_now(), "reason": text(reason, "stop reason", 256)},
        mode=0o644,
    )


def cleanup_keys(manifest: Manifest, work: Path, keygen: Path, confirm_scenario: str) -> None:
    if confirm_scenario != manifest.scenario.scenario_id:
        raise Refusal("cleanup requires --confirm-scenario with the exact scenario id")
    private, public = load_wallet_indexes(manifest, work, keygen)
    for wallet in manifest.scenario.wallets:
        journal_path = funding_journal_path(work, wallet.wallet_id)
        if journal_path.exists() and authenticated_state(journal_path, f"funding journal {wallet.wallet_id}").get("phase") != "finalized":
            raise Refusal(f"cleanup refuses while funding {wallet.wallet_id} is not finalized")
    for adapter in manifest.adapters:
        journal_path = adapter_journal_path(work, adapter.adapter_id)
        if not journal_path.exists():
            raise Refusal(f"cleanup refuses before activity {adapter.adapter_id} is finalized")
        journal = authenticated_state(journal_path, f"adapter journal {adapter.adapter_id}")
        if journal.get("schema") != ADAPTER_JOURNAL_SCHEMA or journal.get("manifestSha256") != manifest.sha256 or journal.get("scenarioSha256") != manifest.scenario.sha256 or journal.get("adapterId") != adapter.adapter_id or journal.get("phase") != "finalized":
            raise Refusal(f"cleanup refuses before activity {adapter.adapter_id} is finalized")
    removed: list[dict[str, str]] = []
    for row in exact_list(private["wallets"], "private wallets"):
        source = exact_object(row, "private wallet")
        keypair = canonical_existing_file(text(source.get("keypair"), "cleanup keypair"), "cleanup keypair")
        keypair.unlink()
        removed.append({"id": source["id"], "address": source["address"]})
    private_path = wallet_paths(work)[1]
    private_path.unlink()
    fsync_directory(private_path.parent)
    atomic_write_json(
        work / "public" / "wallet-cleanup.json",
        {
            "schema": "dclutch-devnet-activity-wallet-cleanup-v1",
            "manifestSha256": manifest.sha256,
            "scenarioSha256": manifest.scenario.sha256,
            "cleanedAt": utc_now(),
            "removedEphemeralKeypairs": removed,
            "publicWalletLedgerSha256": sha256_file(wallet_paths(work)[2]),
            "keyRecoveryPossible": False,
        },
        mode=0o644,
    )


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    root.add_argument("--manifest", required=True, help="absolute activity manifest JSON")
    root.add_argument("--work", required=True, help="absolute private run directory")
    sub = root.add_subparsers(dest="command", required=True)
    sub.add_parser("validate")
    wallets = sub.add_parser("prepare-wallets")
    wallets.add_argument("--solana-keygen", required=True)
    funding = sub.add_parser("fund")
    funding.add_argument("--solana", required=True)
    funding.add_argument("--solana-keygen", required=True)
    funding.add_argument("--funder-keypair", required=True)
    funding.add_argument("--live-authorization")
    funding.add_argument("--poll-only", action="store_true")
    activity_run = sub.add_parser("run")
    activity_run.add_argument("--dclutch-bin", required=True)
    activity_run.add_argument("--successor-bin", required=True)
    activity_run.add_argument("--solana-keygen", required=True)
    activity_run.add_argument("--live-authorization")
    resume = sub.add_parser("resume")
    resume.add_argument("--dclutch-bin", required=True)
    resume.add_argument("--successor-bin", required=True)
    resume.add_argument("--solana-keygen", required=True)
    resume.add_argument("--live-authorization")
    reconcile = sub.add_parser("reconcile")
    reconcile.add_argument("--dclutch-bin", required=True)
    reconcile.add_argument("--successor-bin", required=True)
    reconcile.add_argument("--solana-keygen", required=True)
    reconcile.add_argument("--live-authorization")
    stop_parser = sub.add_parser("stop")
    stop_parser.add_argument("--reason", required=True)
    cleanup = sub.add_parser("cleanup-keys")
    cleanup.add_argument("--solana-keygen", required=True)
    cleanup.add_argument("--confirm-scenario", required=True)
    return root


def main(argv: Sequence[str] | None = None) -> int:
    raw_arguments = list(sys.argv[1:] if argv is None else argv)
    try:
        if raw_arguments and raw_arguments[0] == "supervisor-cycle-v1":
            supervisor_cycle(supervisor_parser().parse_args(raw_arguments[1:]))
            return 0
        arguments = parser().parse_args(raw_arguments)
        manifest = parse_manifest(Path(arguments.manifest))
        work = new_work_directory(arguments.work)
        lock_path = work / ".activity.lock"
        lock = lock_path.open("a+")
        try:
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise Refusal("another activity process owns this exact work directory") from error
        if arguments.command == "validate":
            validate_only(manifest)
        elif arguments.command == "prepare-wallets":
            keygen = canonical_existing_file(arguments.solana_keygen, "solana-keygen", executable=True)
            prepare_wallets(manifest, work, keygen)
        elif arguments.command == "fund":
            solana = canonical_existing_file(arguments.solana, "solana CLI", executable=True)
            keygen = canonical_existing_file(arguments.solana_keygen, "solana-keygen", executable=True)
            funder = canonical_existing_file(arguments.funder_keypair, "funder keypair")
            live = None if arguments.live_authorization is None else Path(arguments.live_authorization)
            fund_wallets(manifest, work, solana, keygen, funder, live, poll_only=arguments.poll_only)
        elif arguments.command in {"run", "resume"}:
            dclutch_bin = canonical_existing_file(arguments.dclutch_bin, "dclutch CLI", executable=True)
            successor_bin = canonical_existing_file(arguments.successor_bin, "successor CLI", executable=True)
            keygen = canonical_existing_file(arguments.solana_keygen, "solana-keygen", executable=True)
            live = None if arguments.live_authorization is None else Path(arguments.live_authorization)
            run_activity(
                manifest,
                work,
                dclutch_bin,
                successor_bin,
                keygen,
                live,
                poll_only=arguments.command == "resume",
            )
        elif arguments.command == "reconcile":
            dclutch_bin = canonical_existing_file(arguments.dclutch_bin, "dclutch CLI", executable=True)
            successor_bin = canonical_existing_file(arguments.successor_bin, "successor CLI", executable=True)
            keygen = canonical_existing_file(arguments.solana_keygen, "solana-keygen", executable=True)
            live = None if arguments.live_authorization is None else Path(arguments.live_authorization)
            reconcile_activity(manifest, work, dclutch_bin, successor_bin, keygen, live)
        elif arguments.command == "stop":
            stop(work, arguments.reason)
        elif arguments.command == "cleanup-keys":
            keygen = canonical_existing_file(arguments.solana_keygen, "solana-keygen", executable=True)
            cleanup_keys(manifest, work, keygen, arguments.confirm_scenario)
        else:
            raise Refusal(f"unknown command {arguments.command}")
        return 0
    except Refusal as error:
        print(f"activity refused: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
