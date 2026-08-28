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
import dataclasses
import datetime as dt
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import time
from typing import Any, Iterable, Mapping, Sequence
from urllib import parse as urlparse
from urllib import request as urlrequest


MANIFEST_SCHEMA = "dclutch-devnet-activity-manifest-v1"
WALLET_LEDGER_SCHEMA = "dclutch-devnet-activity-wallet-ledger-v1"
PRIVATE_INDEX_SCHEMA = "dclutch-devnet-activity-private-wallet-index-v1"
FUNDING_JOURNAL_SCHEMA = "dclutch-devnet-activity-funding-journal-v1"
AUTHORIZATION_SCHEMA = "dclutch-devnet-activity-live-authorization-v1"
STOP_SCHEMA = "dclutch-devnet-activity-stop-v1"
DEVNET_GENESIS_HASH = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
MEMO_PREFIX = "dclutch-activity-fund-v1:"
PUBKEY_RE = re.compile(r"^[1-9A-HJ-NP-Za-km-z]{32,44}$")
HEX_RE = re.compile(r"^[0-9a-f]{64}$")
ID_RE = re.compile(r"^[a-z][a-z0-9-]{0,63}$")
DECIMAL_RE = re.compile(r"^(?:0|[1-9][0-9]*)$")
SIGNED_DECIMAL_RE = re.compile(r"^-?(?:0|[1-9][0-9]*)$")
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


@dataclasses.dataclass(frozen=True)
class OperationSpec:
    operation_id: str
    kind: str
    wallet_ids: tuple[str, ...]
    depends_on: tuple[str, ...]
    mutation_expected: bool
    expected_lamport_deltas: Mapping[str, int]


@dataclasses.dataclass(frozen=True)
class Scenario:
    path: Path
    sha256: str
    schema: str
    scenario_id: str
    cluster_target: str
    market_ref: str
    wallets: tuple[WalletSpec, ...]
    operations: tuple[OperationSpec, ...]
    limits: Limits


@dataclasses.dataclass(frozen=True)
class CompletionSpec:
    path: str
    schema: str | None
    signature_pointers: tuple[str, ...]
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


def parse_scenario(path: Path, expected_sha256: str) -> Scenario:
    path = canonical_existing_file(path, "economic scenario")
    observed_sha256 = sha256_file(path)
    if observed_sha256 != expected_sha256:
        raise Refusal("economic scenario bytes differ from the activity manifest")
    value = exact_object(read_exact_json(path, "economic scenario"), "economic scenario")
    exact_keys(
        value,
        {"schema", "scenarioId", "clusterTarget", "marketRef", "wallets", "operations", "limits"},
        "economic scenario",
    )
    schema = text(value["schema"], "economic scenario schema", 128)
    scenario_id = stable_id(value["scenarioId"], "scenario id")
    cluster_target = text(value["clusterTarget"], "cluster target", 32)
    if cluster_target not in {"owned-loopback", "devnet"}:
        raise Refusal("clusterTarget must be owned-loopback or devnet")
    market_ref = stable_id(value["marketRef"], "market ref")
    limits = parse_limits(value["limits"], "scenario limits", cluster_target)

    wallets: list[WalletSpec] = []
    wallet_ids: set[str] = set()
    for index, raw in enumerate(exact_list(value["wallets"], "scenario wallets")):
        source = exact_object(raw, f"wallet {index}")
        exact_keys(source, {"id", "roles", "fundingLamports"}, f"wallet {index}")
        wallet_id = stable_id(source["id"], f"wallet {index} id")
        if wallet_id in wallet_ids:
            raise Refusal(f"scenario repeats wallet {wallet_id}")
        wallet_ids.add(wallet_id)
        roles = tuple(stable_id(role, f"wallet {wallet_id} role") for role in exact_list(source["roles"], f"wallet {wallet_id} roles"))
        if not roles or len(set(roles)) != len(roles):
            raise Refusal(f"wallet {wallet_id} roles must be nonempty and unique")
        wallets.append(WalletSpec(wallet_id, roles, decimal(source["fundingLamports"], f"wallet {wallet_id} funding", positive=True)))
    if not wallets:
        raise Refusal("scenario must name at least one disposable wallet")

    kinds = {"found", "participant", "direct", "resolve", "redeem", "retire"}
    operations: list[OperationSpec] = []
    operation_ids: set[str] = set()
    for index, raw in enumerate(exact_list(value["operations"], "scenario operations")):
        source = exact_object(raw, f"operation {index}")
        exact_keys(
            source,
            {"id", "kind", "wallets", "dependsOn", "mutationExpected", "inputs", "expectedLamportDeltas", "expectedTokenDeltas", "receiptRef"},
            f"operation {index}",
        )
        operation_id = stable_id(source["id"], f"operation {index} id")
        if operation_id in operation_ids:
            raise Refusal(f"scenario repeats operation {operation_id}")
        operation_ids.add(operation_id)
        kind = text(source["kind"], f"operation {operation_id} kind", 32)
        if kind not in kinds:
            raise Refusal(f"operation {operation_id} has unknown kind {kind}")
        operation_wallets = tuple(stable_id(item, f"operation {operation_id} wallet") for item in exact_list(source["wallets"], f"operation {operation_id} wallets"))
        if not operation_wallets or len(set(operation_wallets)) != len(operation_wallets) or any(item not in wallet_ids for item in operation_wallets):
            raise Refusal(f"operation {operation_id} names absent or repeated wallets")
        dependencies = tuple(stable_id(item, f"operation {operation_id} dependency") for item in exact_list(source["dependsOn"], f"operation {operation_id} dependencies"))
        mutation = source["mutationExpected"]
        if not isinstance(mutation, bool):
            raise Refusal(f"operation {operation_id} mutationExpected must be boolean")
        exact_object(source["inputs"], f"operation {operation_id} inputs")
        exact_list(source["expectedTokenDeltas"], f"operation {operation_id} token deltas")
        stable_id(source["receiptRef"], f"operation {operation_id} receipt ref")
        delta_source = exact_object(source["expectedLamportDeltas"], f"operation {operation_id} lamport deltas")
        if any(item not in wallet_ids for item in delta_source):
            raise Refusal(f"operation {operation_id} carries a lamport delta for an absent wallet")
        deltas = {item: signed_decimal(delta, f"operation {operation_id} {item} lamport delta") for item, delta in delta_source.items()}
        operations.append(OperationSpec(operation_id, kind, operation_wallets, dependencies, mutation, deltas))
    if not operations:
        raise Refusal("scenario must name at least one activity operation")
    for operation in operations:
        if any(dependency not in operation_ids or dependency == operation.operation_id for dependency in operation.depends_on):
            raise Refusal(f"operation {operation.operation_id} has an absent or self dependency")
    require_acyclic({item.operation_id: item.depends_on for item in operations}, "scenario operation graph")
    return Scenario(path, observed_sha256, schema, scenario_id, cluster_target, market_ref, tuple(wallets), tuple(operations), limits)


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
    exact_keys(source, {"path", "schema", "signaturePointers", "requiredValues"}, f"adapter {adapter_id} completion")
    path = text(source["path"], f"adapter {adapter_id} completion path")
    schema_value = source["schema"]
    schema = None if schema_value is None else text(schema_value, f"adapter {adapter_id} completion schema", 128)
    pointers = tuple(text(item, f"adapter {adapter_id} signature pointer", 256) for item in exact_list(source["signaturePointers"], f"adapter {adapter_id} signature pointers"))
    if len(set(pointers)) != len(pointers):
        raise Refusal(f"adapter {adapter_id} repeats a signature pointer")
    required_values = exact_object(source["requiredValues"], f"adapter {adapter_id} required values")
    for pointer in required_values:
        if not pointer.startswith("/"):
            raise Refusal(f"adapter {adapter_id} required-value key is not a JSON pointer")
    return CompletionSpec(path, schema, pointers, required_values)


def parse_manifest(path: Path) -> Manifest:
    path = canonical_existing_file(path, "activity manifest")
    manifest_sha256 = sha256_file(path)
    value = exact_object(read_exact_json(path, "activity manifest"), "activity manifest")
    exact_keys(value, {"schema", "scenario", "target", "inputs", "adapters"}, "activity manifest")
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

    operation_by_id = {item.operation_id: item for item in scenario.operations}
    covered: set[str] = set()
    adapters: list[AdapterSpec] = []
    adapter_ids: set[str] = set()
    wallet_ids = {item.wallet_id for item in scenario.wallets}
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
        if len(set(adapter_wallets)) != len(adapter_wallets) or any(item not in wallet_ids for item in adapter_wallets):
            raise Refusal(f"adapter {adapter_id} names absent or repeated wallets")
        mutation = source["mutation"]
        if not isinstance(mutation, bool) or any(operation_by_id[item].mutation_expected != mutation for item in covers):
            raise Refusal(f"adapter {adapter_id} mutation disagrees with its scenario operations")
        adapters.append(AdapterSpec(adapter_id, covers, caller, argv, dependencies, adapter_wallets, mutation, parse_completion(source["completion"], adapter_id)))
    if covered != set(operation_by_id):
        raise Refusal(f"activity adapters do not cover exactly the scenario operations: missing {sorted(set(operation_by_id) - covered)}")
    for adapter in adapters:
        if any(item not in adapter_ids or item == adapter.adapter_id for item in adapter.depends_on):
            raise Refusal(f"adapter {adapter.adapter_id} has an absent or self dependency")
    require_acyclic({item.adapter_id: item.depends_on for item in adapters}, "activity adapter graph")
    if sum(1 for item in adapters if item.mutation) > scenario.limits.max_transactions:
        raise Refusal("activity adapter mutation count exceeds maxTransactions")
    return Manifest(path, manifest_sha256, scenario, rpc_url, genesis, inputs, tuple(adapters))


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

    def signatures_for_address(self, address: str, limit: int = 100) -> list[dict[str, Any]]:
        result = exact_list(self.call("getSignaturesForAddress", [address, {"commitment": "finalized", "limit": limit}]), "getSignaturesForAddress result")
        return [exact_object(item, "signature row") for item in result]


def authenticate_cluster(manifest: Manifest, rpc: Rpc) -> str:
    genesis = rpc.genesis_hash()
    if manifest.scenario.cluster_target == "devnet":
        if genesis != DEVNET_GENESIS_HASH or genesis != manifest.devnet_genesis_hash:
            raise Refusal("RPC does not prove the exact acknowledged Solana devnet")
    elif genesis == DEVNET_GENESIS_HASH:
        raise Refusal("owned-loopback RPC unexpectedly answers with Solana devnet")
    return genesis


def authorization(path: Path, manifest: Manifest) -> dict[str, Any]:
    value = exact_object(read_exact_json(canonical_existing_file(path, "live authorization"), "live authorization"), "live authorization")
    exact_keys(value, {"schema", "manifestSha256", "scenarioSha256", "devnetGenesisHash", "marketRef", "notBefore", "expiresAt", "authorization"}, "live authorization")
    if value["schema"] != AUTHORIZATION_SCHEMA or value["manifestSha256"] != manifest.sha256 or value["scenarioSha256"] != manifest.scenario.sha256:
        raise Refusal("live authorization is not bound to this exact manifest and scenario")
    if value["devnetGenesisHash"] != DEVNET_GENESIS_HASH or value["marketRef"] != manifest.scenario.market_ref:
        raise Refusal("live authorization names another cluster or Market")
    if value["authorization"] != "authorize-one-devnet-activity-run":
        raise Refusal("live authorization lacks the exact one-run authorization phrase")
    try:
        now = dt.datetime.now(dt.timezone.utc)
        not_before = dt.datetime.fromisoformat(text(value["notBefore"], "authorization notBefore").replace("Z", "+00:00"))
        expires = dt.datetime.fromisoformat(text(value["expiresAt"], "authorization expiresAt").replace("Z", "+00:00"))
    except ValueError as error:
        raise Refusal("live authorization timestamps are not RFC3339 timestamps") from error
    if not_before.tzinfo is None or expires.tzinfo is None or not (not_before <= now < expires) or expires - not_before > dt.timedelta(hours=6):
        raise Refusal("live authorization is outside its at-most-six-hour window")
    return value


def require_live_authorization(manifest: Manifest, path: Path | None) -> str | None:
    if manifest.scenario.cluster_target == "owned-loopback":
        if path is not None:
            raise Refusal("owned-loopback refuses a devnet live-authorization file")
        return None
    if path is None:
        raise Refusal("public devnet mutation is held until --live-authorization names this exact run")
    return sha256_file(canonical_existing_file(path, "live authorization")) if authorization(path, manifest) else None


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
        verify_wallet_files(private, public, manifest, keygen)
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


def verify_wallet_files(private: Mapping[str, Any], public: Mapping[str, Any], manifest: Manifest, keygen: Path) -> None:
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


def load_wallet_indexes(manifest: Manifest, work: Path, keygen: Path) -> tuple[dict[str, Any], dict[str, Any]]:
    _, private_path, public_path = wallet_paths(work)
    if not private_path.exists() or not public_path.exists():
        raise Refusal("prepare-wallets must complete before funding or activity")
    private = authenticated_state(private_path, "private wallet index")
    public = authenticated_state(public_path, "public wallet ledger")
    verify_wallet_files(private, public, manifest, keygen)
    return private, public


def fund_wallets(manifest: Manifest, work: Path, solana: Path, keygen: Path, funder_keypair: Path, live_authorization: Path | None, *, poll_only: bool) -> None:
    authorization_sha256 = require_live_authorization(manifest, live_authorization)
    private, public = load_wallet_indexes(manifest, work, keygen)
    rpc = Rpc(manifest.rpc_url, minimum_interval_ms=manifest.scenario.limits.min_dispatch_interval_ms)
    authenticate_cluster(manifest, rpc)
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
    stop_parser = sub.add_parser("stop")
    stop_parser.add_argument("--reason", required=True)
    cleanup = sub.add_parser("cleanup-keys")
    cleanup.add_argument("--solana-keygen", required=True)
    cleanup.add_argument("--confirm-scenario", required=True)
    return root


def main(argv: Sequence[str] | None = None) -> int:
    arguments = parser().parse_args(argv)
    try:
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
