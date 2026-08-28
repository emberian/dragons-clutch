#!/usr/bin/env python3
"""Black-box failure injection for one owned-loopback lifecycle session.

The protocol commands remain the semantic owners.  This supervisor starts the
same opaque session command in a fresh case directory, watches its durable
journals, kills only the command process group, proxies only a literal
loopback RPC endpoint, and compares an independently produced account snapshot.

The accepted private-validator driver writes the small projection described in
README.md.  No wallet material appears in that projection or in this tool.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import contextlib
import dataclasses
import hashlib
import http.server
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import threading
import time
from typing import Any, Iterable, Mapping, Sequence
from urllib import error as urlerror
from urllib import request as urlrequest


SPEC_SCHEMA = "dclutch-lifecycle-chaos-spec-v1"
SUMMARY_SCHEMA = "dclutch-lifecycle-chaos-summary-v1"
SNAPSHOT_SCHEMA = "dclutch-lifecycle-chaos-snapshot-v1"
STAGE_JOURNAL_SCHEMA = "dclutch-lifecycle-chaos-stage-projection-v1"
CONTROL_SCHEMA = "dclutch-lifecycle-chaos-control-v1"
RPC_TRACE_SCHEMA = "dclutch-lifecycle-chaos-rpc-trace-v1"
BOUNDARIES = (
    "founding",
    "participant",
    "alt",
    "seal",
    "hot",
    "resolution",
    "payout",
    "retire",
)
RPC_CASES = ("rpc-timeout", "duplicate-send", "blockhash-expiry")
REFUSAL_CASES = (
    "corrupted-evidence",
    "replaced-evidence",
    "wallet-underfund",
    "wallet-surplus",
    "late-child-refusal",
)
ALL_CASES = (
    "baseline",
    *(f"kill-{boundary}" for boundary in BOUNDARIES),
    *RPC_CASES,
    *REFUSAL_CASES,
)
MAX_JSON_BYTES = 16 * 1024 * 1024
POLL_SECONDS = 0.01
BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
MAX_TRANSACTION_BYTES = 1232


class Refusal(RuntimeError):
    """Fail-closed harness refusal."""


@dataclasses.dataclass(frozen=True)
class CommandTemplate:
    argv: tuple[str, ...]
    cwd: Path
    environment: tuple[tuple[str, str], ...]


@dataclasses.dataclass(frozen=True)
class Spec:
    path: Path
    source_revision: str
    command: CommandTemplate
    observer: CommandTemplate
    teardown: CommandTemplate
    session_relative: Path
    journal_relative: Path
    session_schema: str
    rpc_upstream: str | None
    evidence_relative: Path
    replacement_relative: Path
    case_timeout_seconds: float
    journal_timeout_seconds: float


@dataclasses.dataclass(frozen=True)
class ProcessResult:
    returncode: int
    stdout: bytes
    stderr: bytes
    elapsed_nanoseconds: int


@dataclasses.dataclass(frozen=True)
class RpcRule:
    kind: str
    method: str
    stage: str


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def base58_encode(value: bytes) -> str:
    zeroes = len(value) - len(value.lstrip(b"\0"))
    number = int.from_bytes(value, "big")
    encoded = ""
    while number:
        number, remainder = divmod(number, 58)
        encoded = BASE58_ALPHABET[remainder] + encoded
    return "1" * zeroes + encoded


def base58_decode(value: str, label: str) -> bytes:
    if not value:
        raise Refusal(f"{label} is empty")
    number = 0
    for char in value:
        try:
            digit = BASE58_ALPHABET.index(char)
        except ValueError as exc:
            raise Refusal(f"{label} is not base58") from exc
        number = number * 58 + digit
    width = (number.bit_length() + 7) // 8
    body = number.to_bytes(width, "big") if width else b""
    return b"\0" * (len(value) - len(value.lstrip("1"))) + body


def short_vec(body: bytes, offset: int, label: str) -> tuple[int, int]:
    value = 0
    shift = 0
    for _ in range(3):
        if offset >= len(body):
            raise Refusal(f"{label} ended inside a compact length")
        byte = body[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            return value, offset
        shift += 7
    raise Refusal(f"{label} carried an overlong compact length")


def frozen_transaction(call: Mapping[str, Any]) -> tuple[str, str, str]:
    params = call.get("params")
    if not isinstance(params, list) or not 1 <= len(params) <= 2 or not isinstance(params[0], str):
        raise Refusal("sendTransaction params are malformed")
    config = params[1] if len(params) == 2 else {}
    if not isinstance(config, dict):
        raise Refusal("sendTransaction config is malformed")
    encoding = config.get("encoding", "base58")
    try:
        if encoding == "base64":
            wire = base64.b64decode(params[0], validate=True)
        elif encoding == "base58":
            wire = base58_decode(params[0], "sendTransaction wire")
        else:
            raise Refusal("sendTransaction encoding is neither base64 nor base58")
    except (ValueError, binascii.Error) as exc:
        raise Refusal(f"sendTransaction wire is malformed: {exc}") from exc
    if not 0 < len(wire) <= MAX_TRANSACTION_BYTES:
        raise Refusal("sendTransaction wire is outside the Solana packet bound")

    signature_count, message_offset = short_vec(wire, 0, "sendTransaction signatures")
    signatures_end = message_offset + signature_count * 64
    if signature_count == 0 or signatures_end >= len(wire):
        raise Refusal("sendTransaction omitted its first signature or message")
    signature = base58_encode(wire[message_offset : message_offset + 64])
    message = wire[signatures_end:]
    if message[0] & 0x80:
        if message[0] != 0x80:
            raise Refusal("sendTransaction used an unsupported message version")
        header_offset = 1
    else:
        header_offset = 0
    key_count_offset = header_offset + 3
    if key_count_offset >= len(message):
        raise Refusal("sendTransaction message omitted its header")
    key_count, keys_offset = short_vec(message, key_count_offset, "message account keys")
    blockhash_offset = keys_offset + key_count * 32
    blockhash_end = blockhash_offset + 32
    if blockhash_end > len(message):
        raise Refusal("sendTransaction message omitted its recent blockhash")
    blockhash = base58_encode(message[blockhash_offset:blockhash_end])
    return sha256_bytes(wire), signature, blockhash


def require_accepted_send(status: int, response: bytes, signature: str) -> None:
    value = unique_json_bytes(response, "sendTransaction response")
    if (
        status != 200
        or not isinstance(value, dict)
        or value.get("result") != signature
        or value.get("error") is not None
    ):
        raise Refusal("upstream did not accept the exact frozen transaction signature")


def status_signature_values(call: Mapping[str, Any]) -> list[str] | None:
    if call.get("method") != "getSignatureStatuses":
        return None
    params = call.get("params")
    if (
        not isinstance(params, list)
        or not params
        or not isinstance(params[0], list)
        or not all(isinstance(value, str) for value in params[0])
    ):
        raise Refusal("getSignatureStatuses params are malformed")
    return params[0]


def unique_json_bytes(data: bytes, label: str) -> Any:
    if not 0 < len(data) <= MAX_JSON_BYTES:
        raise Refusal(f"{label} is outside the 1..{MAX_JSON_BYTES} byte bound")

    def pairs(rows: list[tuple[str, Any]]) -> dict[str, Any]:
        output: dict[str, Any] = {}
        for key, value in rows:
            if key in output:
                raise Refusal(f"{label} duplicated JSON key {key!r}")
            output[key] = value
        return output

    try:
        return json.loads(data, object_pairs_hook=pairs)
    except (UnicodeError, json.JSONDecodeError) as exc:
        raise Refusal(f"{label} is not exact JSON: {exc}") from exc


def read_unique_json(path: Path, label: str) -> Any:
    if path.is_symlink() or not path.is_file():
        raise Refusal(f"{label} is not one regular non-symlink file: {path}")
    try:
        return unique_json_bytes(path.read_bytes(), label)
    except OSError as exc:
        raise Refusal(f"read {label}: {exc}") from exc


def write_new(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as target:
            target.write(data)
            target.flush()
            os.fsync(target.fileno())
        directory = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    except Exception:
        with contextlib.suppress(OSError):
            os.close(descriptor)
        raise


def write_json_new(path: Path, value: Any) -> None:
    write_new(path, (json.dumps(value, sort_keys=True, indent=2) + "\n").encode())


def atomic_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    write_new(temporary, (json.dumps(value, sort_keys=True, indent=2) + "\n").encode())
    os.replace(temporary, path)
    directory = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


def absolute_directory(value: Any, label: str) -> Path:
    if not isinstance(value, str):
        raise Refusal(f"{label} must be an absolute directory string")
    path = Path(value)
    if not path.is_absolute() or path.is_symlink() or not path.is_dir():
        raise Refusal(f"{label} must be one existing absolute non-symlink directory")
    return path.resolve(strict=True)


def relative_path(value: Any, label: str) -> Path:
    if not isinstance(value, str):
        raise Refusal(f"{label} must be a relative path string")
    path = Path(value)
    if path.is_absolute() or not path.parts or ".." in path.parts:
        raise Refusal(f"{label} must remain below the case directory")
    return path


def command_template(value: Any, label: str) -> CommandTemplate:
    if not isinstance(value, dict) or set(value) != {"argv", "cwd", "environment"}:
        raise Refusal(f"{label} must contain exactly argv, cwd, and environment")
    argv = value["argv"]
    environment = value["environment"]
    if (
        not isinstance(argv, list)
        or not argv
        or not all(isinstance(item, str) and item for item in argv)
        or not isinstance(environment, dict)
        or not all(isinstance(key, str) and isinstance(item, str) for key, item in environment.items())
    ):
        raise Refusal(f"{label} argv/environment is malformed")
    executable = Path(argv[0])
    if not executable.is_absolute() or executable.is_symlink() or not executable.is_file():
        raise Refusal(f"{label} executable must be one absolute non-symlink file")
    return CommandTemplate(
        tuple(argv),
        absolute_directory(value["cwd"], f"{label} cwd"),
        tuple(sorted(environment.items())),
    )


def loopback_rpc(value: Any) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str) or not value.startswith("http://127.0.0.1:"):
        raise Refusal("RPC upstream must be a literal IPv4 loopback HTTP URL")
    suffix = value.removeprefix("http://127.0.0.1:")
    if not suffix.isdigit() or not 1 <= int(suffix) <= 65535:
        raise Refusal("RPC upstream carried an invalid loopback port")
    return value


def parse_spec(path: Path) -> Spec:
    document = read_unique_json(path, "chaos spec")
    expected = {
        "schema",
        "cluster",
        "sourceRevision",
        "command",
        "observer",
        "teardown",
        "session",
        "journalDir",
        "sessionSchema",
        "rpcUpstream",
        "evidence",
        "replacementEvidence",
        "caseTimeoutSeconds",
        "journalTimeoutSeconds",
        "boundaries",
    }
    if not isinstance(document, dict) or set(document) != expected:
        raise Refusal("chaos spec has unknown, omitted, or defaulted fields")
    if document["schema"] != SPEC_SCHEMA or document["cluster"] != "owned-loopback":
        raise Refusal("chaos spec is not the exact owned-loopback schema")
    revision = document["sourceRevision"]
    if not isinstance(revision, str) or len(revision) != 40 or any(
        char not in "0123456789abcdef" for char in revision
    ):
        raise Refusal("chaos spec source revision is not one lowercase commit digest")
    if document["boundaries"] != list(BOUNDARIES):
        raise Refusal("chaos spec boundaries are not the exact ordered lifecycle")
    case_timeout = document["caseTimeoutSeconds"]
    journal_timeout = document["journalTimeoutSeconds"]
    if (
        not isinstance(case_timeout, (int, float))
        or not 1 <= case_timeout <= 3600
        or not isinstance(journal_timeout, (int, float))
        or not 1 <= journal_timeout <= case_timeout
    ):
        raise Refusal("chaos spec timeouts are outside their bounded range")
    schema = document["sessionSchema"]
    if not isinstance(schema, str) or not schema:
        raise Refusal("chaos spec omitted its exact session schema")
    return Spec(
        path.resolve(strict=True),
        revision,
        command_template(document["command"], "session command"),
        command_template(document["observer"], "observer command"),
        command_template(document["teardown"], "teardown command"),
        relative_path(document["session"], "session path"),
        relative_path(document["journalDir"], "journal directory"),
        schema,
        loopback_rpc(document["rpcUpstream"]),
        relative_path(document["evidence"], "evidence path"),
        relative_path(document["replacementEvidence"], "replacement evidence path"),
        float(case_timeout),
        float(journal_timeout),
    )


def expand(value: str, *, case: str, case_work: Path, rpc_url: str) -> str:
    replacements = {
        "{case}": case,
        "{caseWork}": str(case_work),
        "{rpcUrl}": rpc_url,
    }
    output = value
    for source, target in replacements.items():
        output = output.replace(source, target)
    if "{" in output or "}" in output:
        raise Refusal(f"command contains an unknown placeholder: {value}")
    return output


def expanded_command(
    template: CommandTemplate,
    *,
    case: str,
    case_work: Path,
    rpc_url: str,
    control: Path,
) -> tuple[list[str], Path, dict[str, str]]:
    argv = [expand(item, case=case, case_work=case_work, rpc_url=rpc_url) for item in template.argv]
    environment = os.environ.copy()
    environment.update(
        {
            key: expand(value, case=case, case_work=case_work, rpc_url=rpc_url)
            for key, value in template.environment
        }
    )
    environment["DCLUTCH_LIFECYCLE_CHAOS_CONTROL"] = str(control)
    environment["DCLUTCH_LIFECYCLE_CHAOS_CASE"] = case
    environment["DCLUTCH_LIFECYCLE_CHAOS_RPC_URL"] = rpc_url
    return argv, template.cwd, environment


def terminate_group(child: subprocess.Popen[bytes], *, force: bool) -> None:
    if child.poll() is not None:
        return
    selected = signal.SIGKILL if force else signal.SIGTERM
    with contextlib.suppress(ProcessLookupError):
        os.killpg(child.pid, selected)
    try:
        child.wait(timeout=5)
    except subprocess.TimeoutExpired:
        with contextlib.suppress(ProcessLookupError):
            os.killpg(child.pid, signal.SIGKILL)
        child.wait(timeout=5)


def stable_journal(path: Path, expected_stage: str) -> dict[str, Any] | None:
    try:
        first = path.read_bytes()
        first_stat = path.stat()
        second = path.read_bytes()
        second_stat = path.stat()
    except FileNotFoundError:
        return None
    except OSError as exc:
        raise Refusal(f"read {expected_stage} journal: {exc}") from exc
    if first != second or first_stat.st_ino != second_stat.st_ino or first_stat.st_size != second_stat.st_size:
        return None
    value = unique_json_bytes(first, f"{expected_stage} journal")
    if (
        not isinstance(value, dict)
        or set(value) != {"schema", "stage", "phase", "intentSha256"}
        or value["schema"] != STAGE_JOURNAL_SCHEMA
        or value["stage"] != expected_stage
        or value["phase"] not in {"planned", "signed-not-submitted", "submitted", "finalized"}
        or not isinstance(value["intentSha256"], str)
        or len(value["intentSha256"]) != 64
    ):
        raise Refusal(f"{expected_stage} journal projection is malformed")
    return value


def wait_for_submitted(
    child: subprocess.Popen[bytes], journal: Path, stage: str, timeout: float
) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if child.poll() is not None:
            raise Refusal(f"session exited before durable Submitted at {stage}")
        value = stable_journal(journal, stage)
        if value is not None:
            if value["phase"] == "submitted":
                return value
            if value["phase"] == "finalized":
                raise Refusal(f"{stage} finalized before the kill could observe durable Submitted")
        time.sleep(POLL_SECONDS)
    raise Refusal(f"session did not expose durable Submitted at {stage} within the bound")


def run_process(
    argv: Sequence[str], cwd: Path, environment: Mapping[str, str], timeout: float
) -> ProcessResult:
    started = time.monotonic_ns()
    child = subprocess.Popen(
        list(argv),
        cwd=cwd,
        env=dict(environment),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    try:
        stdout, stderr = child.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        terminate_group(child, force=True)
        stdout, stderr = child.communicate()
        raise Refusal(f"command exceeded its {timeout:g} second bound")
    return ProcessResult(child.returncode, stdout, stderr, time.monotonic_ns() - started)


def record_attempt(path: Path, argv: Sequence[str], result: ProcessResult) -> None:
    path.mkdir(parents=True, exist_ok=False)
    write_new(path / "stdout.bin", result.stdout)
    write_new(path / "stderr.bin", result.stderr)
    write_json_new(
        path / "receipt.json",
        {
            "schema": "dclutch-lifecycle-chaos-attempt-v1",
            "argv": list(argv),
            "elapsedNanoseconds": result.elapsed_nanoseconds,
            "exitStatus": result.returncode,
            "stdoutSha256": sha256_bytes(result.stdout),
            "stderrSha256": sha256_bytes(result.stderr),
        },
    )


def kill_then_resume(
    spec: Spec,
    case: str,
    case_work: Path,
    rpc_url: str,
    control: Path,
    boundary: str,
    proxy: "RpcFaultProxy",
) -> tuple[ProcessResult, str]:
    argv, cwd, environment = expanded_command(
        spec.command,
        case=case,
        case_work=case_work,
        rpc_url=rpc_url,
        control=control,
    )
    started = time.monotonic_ns()
    child = subprocess.Popen(
        argv,
        cwd=cwd,
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    try:
        wait_prepared(control, child, spec.journal_timeout_seconds)
        atomic_json(control / "GO.json", {"schema": CONTROL_SCHEMA, "state": "go"})
        submitted = wait_for_submitted(
            child,
            case_work / spec.journal_relative / f"{boundary}.json",
            boundary,
            spec.journal_timeout_seconds,
        )
        if not proxy.state.delivery_done.wait(spec.journal_timeout_seconds):
            raise Refusal(f"session did not deliver the frozen {boundary} packet after Submitted")
    except Exception:
        terminate_group(child, force=True)
        proxy.state.release_delivery.set()
        raise
    terminate_group(child, force=True)
    proxy.state.release_delivery.set()
    if not proxy.state.injection_done.wait(spec.journal_timeout_seconds):
        raise Refusal(f"{boundary} delivery handler did not stop after process death")
    stdout, stderr = child.communicate()
    killed = ProcessResult(child.returncode, stdout, stderr, time.monotonic_ns() - started)
    record_attempt(case_work / "attempt-1-killed", argv, killed)
    resumed = run_process(argv, cwd, environment, spec.case_timeout_seconds)
    record_attempt(case_work / "attempt-2-resumed", argv, resumed)
    return resumed, submitted["intentSha256"]


def validate_snapshot(data: bytes, label: str) -> dict[str, Any]:
    value = unique_json_bytes(data, label)
    if not isinstance(value, dict) or set(value) != {"schema", "accounts", "totals"}:
        raise Refusal(f"{label} must contain exactly schema, accounts, and totals")
    if value["schema"] != SNAPSHOT_SCHEMA or not isinstance(value["accounts"], list):
        raise Refusal(f"{label} has another snapshot schema")
    addresses: set[str] = set()
    lamports = 0
    for row in value["accounts"]:
        if not isinstance(row, dict) or set(row) != {
            "address",
            "owner",
            "lamports",
            "executable",
            "dataBase64",
            "dataSha256",
        }:
            raise Refusal(f"{label} account row is not exact")
        address = row["address"]
        if not isinstance(address, str) or not address or address in addresses:
            raise Refusal(f"{label} account identities are empty or duplicated")
        addresses.add(address)
        if not isinstance(row["lamports"], int) or row["lamports"] < 0:
            raise Refusal(f"{label} carried invalid lamports")
        if not isinstance(row["executable"], bool) or not isinstance(row["owner"], str):
            raise Refusal(f"{label} carried invalid owner/executable facts")
        try:
            body = base64.b64decode(row["dataBase64"], validate=True)
        except Exception as exc:
            raise Refusal(f"{label} carried invalid account data: {exc}") from exc
        if sha256_bytes(body) != row["dataSha256"]:
            raise Refusal(f"{label} account body digest disagreed")
        lamports += row["lamports"]
    totals = value["totals"]
    if not isinstance(totals, dict) or set(totals) != {"accountCount", "lamports"}:
        raise Refusal(f"{label} totals are not exact")
    if totals != {"accountCount": len(addresses), "lamports": lamports}:
        raise Refusal(f"{label} totals disagree with its ordered account set")
    ordered = sorted(value["accounts"], key=lambda row: row["address"])
    if ordered != value["accounts"]:
        raise Refusal(f"{label} account order is not canonical")
    return value


def observe(
    spec: Spec,
    case: str,
    case_work: Path,
    rpc_url: str,
    control: Path,
    label: str,
) -> dict[str, Any]:
    argv, cwd, environment = expanded_command(
        spec.observer,
        case=case,
        case_work=case_work,
        rpc_url=rpc_url,
        control=control,
    )
    environment["DCLUTCH_LIFECYCLE_CHAOS_OBSERVATION"] = label
    result = run_process(argv, cwd, environment, spec.case_timeout_seconds)
    record_attempt(case_work / f"observation-{label}", argv, result)
    if result.returncode != 0:
        raise Refusal(f"{case} observer failed with status {result.returncode}")
    return validate_snapshot(result.stdout, f"{case} snapshot")


def teardown_case(
    spec: Spec,
    case: str,
    case_work: Path,
    rpc_url: str,
    control: Path,
) -> None:
    argv, cwd, environment = expanded_command(
        spec.teardown,
        case=case,
        case_work=case_work,
        rpc_url=rpc_url,
        control=control,
    )
    result = run_process(argv, cwd, environment, spec.journal_timeout_seconds)
    record_attempt(case_work / "teardown", argv, result)
    if result.returncode != 0:
        raise Refusal(f"{case} exact teardown failed with status {result.returncode}")


def validate_session(spec: Spec, case_work: Path) -> dict[str, Any]:
    value = read_unique_json(case_work / spec.session_relative, "lifecycle session")
    if not isinstance(value, dict) or set(value) != {"schema", "status", "stages"}:
        raise Refusal("lifecycle session has unknown or omitted fields")
    stages = value["stages"]
    if (
        value["schema"] != spec.session_schema
        or value["status"] != "finalized"
        or not isinstance(stages, list)
        or [row.get("stage") for row in stages] != list(BOUNDARIES)
        or any(set(row) != {"stage", "status", "intentSha256"} for row in stages)
        or any(row["status"] != "finalized" for row in stages)
    ):
        raise Refusal("lifecycle session is not the exact finalized eight-stage sequence")
    return value


def validate_refusal_journals(spec: Spec, case: str, case_work: Path) -> None:
    root = case_work / spec.journal_relative
    actual = tuple(
        (value["phase"] if (value := stable_journal(root / f"{stage}.json", stage)) else None)
        for stage in BOUNDARIES
    )
    if case in {
        "corrupted-evidence",
        "replaced-evidence",
        "wallet-underfund",
        "wallet-surplus",
    }:
        expected: tuple[str | None, ...] = (None,) * len(BOUNDARIES)
    elif case == "blockhash-expiry":
        expected = (
            "finalized",
            "finalized",
            "finalized",
            "finalized",
            "submitted",
            None,
            None,
            None,
        )
    elif case == "late-child-refusal":
        expected = (*(("finalized",) * 7), None)
    else:
        raise Refusal(f"{case} has no frozen refusal journal boundary")
    if actual != expected:
        raise Refusal(
            f"{case} refusal journal prefix disagreed: expected {expected}, observed {actual}"
        )


def corrupt_evidence(path: Path) -> str:
    if path.is_symlink() or not path.is_file():
        raise Refusal("corruption target is not one regular evidence file")
    body = bytearray(path.read_bytes())
    if not body:
        raise Refusal("corruption target is empty")
    body[len(body) // 2] ^= 1
    before = sha256_file(path)
    temporary = path.with_name(f".{path.name}.corrupted")
    write_new(temporary, bytes(body))
    os.replace(temporary, path)
    return before


def replace_evidence(path: Path, replacement: Path) -> str:
    if any(item.is_symlink() or not item.is_file() for item in (path, replacement)):
        raise Refusal("replacement evidence target/source must be regular non-symlink files")
    before = sha256_file(path)
    body = replacement.read_bytes()
    if sha256_bytes(body) == before:
        raise Refusal("replacement evidence is byte-identical to its target")
    temporary = path.with_name(f".{path.name}.replacement")
    write_new(temporary, body)
    os.replace(temporary, path)
    return before


class _ProxyState:
    def __init__(self, upstream: str, rule: RpcRule, journal_root: Path, expiry_timeout: float):
        self.upstream = upstream
        self.rule = rule
        self.journal_root = journal_root
        self.expiry_timeout = expiry_timeout
        self.lock = threading.Lock()
        self.trace: list[dict[str, Any]] = []
        self.blockhash_heights: dict[str, int] = {}
        self.delivery_done = threading.Event()
        self.injection_done = threading.Event()
        self.release_delivery = threading.Event()
        self.fired = False
        self.failure: str | None = None

    def stage(self) -> tuple[str | None, str | None, str | None]:
        for stage in reversed(BOUNDARIES):
            value = stable_journal(self.journal_root / f"{stage}.json", stage)
            if value is not None:
                return stage, value["phase"], value["intentSha256"]
        return None, None, None

    def append(self, row: dict[str, Any]) -> None:
        with self.lock:
            self.trace.append(row)

    def remember_blockhash(self, call: Mapping[str, Any], response: bytes) -> None:
        if call.get("method") != "getLatestBlockhash":
            return
        value = unique_json_bytes(response, "getLatestBlockhash response")
        result = value.get("result") if isinstance(value, dict) else None
        facts = result.get("value") if isinstance(result, dict) else None
        if not isinstance(facts, dict):
            raise Refusal("getLatestBlockhash response omitted result.value")
        blockhash = facts.get("blockhash")
        last_valid = facts.get("lastValidBlockHeight")
        if not isinstance(blockhash, str) or not isinstance(last_valid, int) or last_valid < 0:
            raise Refusal("getLatestBlockhash response carried malformed expiry facts")
        with self.lock:
            self.blockhash_heights[blockhash] = last_valid

    def last_valid_height(self, blockhash: str) -> int:
        with self.lock:
            try:
                return self.blockhash_heights[blockhash]
            except KeyError as exc:
                raise Refusal("frozen send used a blockhash not observed through this proxy") from exc


class _RpcHandler(http.server.BaseHTTPRequestHandler):
    server: "_RpcServer"

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def do_POST(self) -> None:  # noqa: N802 - stdlib hook name
        try:
            width = int(self.headers.get("content-length", "0"))
            if not 0 < width <= 2 * 1024 * 1024:
                raise Refusal("RPC request width is outside its bound")
            body = self.rfile.read(width)
            call = unique_json_bytes(body, "proxied RPC request")
            method = call.get("method") if isinstance(call, dict) else None
            if not isinstance(method, str):
                raise Refusal("proxied RPC request omitted method")
            stage, phase, intent = self.server.state.stage()
            packet = sha256_bytes(body)
            if method == "sendTransaction" and stage == self.server.state.rule.stage and phase != "submitted":
                raise Refusal("sendTransaction reached RPC before durable Submitted")
            matching = (
                not self.server.state.fired
                and method == self.server.state.rule.method
                and stage == self.server.state.rule.stage
                and phase == "submitted"
            )
            row = {
                "source": "client",
                "method": method,
                "stage": stage,
                "phase": phase,
                "intentSha256": intent,
                "requestSha256": packet,
            }
            statuses = status_signature_values(call)
            if statuses is not None:
                row["signatureStatusValues"] = statuses
            self.server.state.append(row)
            if matching:
                self.server.state.fired = True
                try:
                    self._inject(call, body, method, stage, intent)
                finally:
                    self.server.state.injection_done.set()
                return
            status, response = self._forward(body)
            self.server.state.remember_blockhash(call, response)
            self._reply(status, response)
        except Exception as exc:  # server thread cannot surface directly
            self.server.state.failure = str(exc)
            with contextlib.suppress(BrokenPipeError):
                self.send_error(502, str(exc))

    def _forward(self, body: bytes) -> tuple[int, bytes]:
        request = urlrequest.Request(
            self.server.state.upstream,
            data=body,
            headers={"content-type": "application/json"},
        )
        try:
            with urlrequest.urlopen(request, timeout=5) as response:  # noqa: S310 loopback checked
                return response.status, response.read()
        except urlerror.HTTPError as exc:
            return exc.code, exc.read()

    def _inject(
        self,
        call: Mapping[str, Any],
        body: bytes,
        method: str,
        stage: str | None,
        intent: str | None,
    ) -> None:
        kind = self.server.state.rule.kind
        wire_sha256, signature, blockhash = frozen_transaction(call)
        if kind == "kill-delivery":
            status, response = self._forward(body)
            require_accepted_send(status, response, signature)
            self.server.state.append(
                {
                    "source": "injected-forward",
                    "method": method,
                    "stage": stage,
                    "intentSha256": intent,
                    "requestSha256": sha256_bytes(body),
                    "wireSha256": wire_sha256,
                    "transactionSignature": signature,
                    "recentBlockhash": blockhash,
                    "responseSha256": sha256_bytes(response),
                    "status": status,
                }
            )
            self.server.state.delivery_done.set()
            if not self.server.state.release_delivery.wait(self.server.state.expiry_timeout):
                raise Refusal("supervisor did not release the delivered kill packet")
            with contextlib.suppress(BrokenPipeError, ConnectionResetError):
                self._reply(status, response)
            return
        if kind == "rpc-timeout":
            status, response = self._forward(body)
            require_accepted_send(status, response, signature)
            self.server.state.append(
                {
                    "source": "injected-forward",
                    "method": method,
                    "stage": stage,
                    "intentSha256": intent,
                    "requestSha256": sha256_bytes(body),
                    "wireSha256": wire_sha256,
                    "transactionSignature": signature,
                    "recentBlockhash": blockhash,
                    "responseSha256": sha256_bytes(response),
                    "status": status,
                }
            )
            self.server.state.delivery_done.set()
            if not self.server.state.release_delivery.wait(self.server.state.expiry_timeout):
                raise Refusal("client RPC did not time out within the bounded case timeout")
            with contextlib.suppress(BrokenPipeError, ConnectionResetError):
                self._reply(status, response)
            return
        if kind == "duplicate-send":
            first_status, first = self._forward(body)
            second_status, second = self._forward(body)
            require_accepted_send(first_status, first, signature)
            require_accepted_send(second_status, second, signature)
            for ordinal, status, response in ((1, first_status, first), (2, second_status, second)):
                self.server.state.append(
                    {
                        "source": "injected-forward",
                        "ordinal": ordinal,
                        "method": method,
                        "stage": stage,
                        "intentSha256": intent,
                        "requestSha256": sha256_bytes(body),
                        "wireSha256": wire_sha256,
                        "transactionSignature": signature,
                        "recentBlockhash": blockhash,
                        "responseSha256": sha256_bytes(response),
                        "status": status,
                    }
                )
            self._reply(first_status, first)
            return
        if kind == "blockhash-expiry":
            last_valid = self.server.state.last_valid_height(blockhash)
            observed = self._wait_until_expired(last_valid)
            status, encoded = self._forward(body)
            response = unique_json_bytes(encoded, "expired send response")
            error = response.get("error") if isinstance(response, dict) else None
            error_data = error.get("data") if isinstance(error, dict) else None
            if (
                status != 200
                or not isinstance(error, dict)
                or "Blockhash" not in str(error.get("message", ""))
                or not isinstance(error_data, dict)
                or error_data.get("err") != "BlockhashNotFound"
            ):
                raise Refusal("upstream accepted the transaction after its blockhash expired")
            self.server.state.append(
                {
                    "source": "injected-expired-forward",
                    "method": method,
                    "stage": stage,
                    "intentSha256": intent,
                    "requestSha256": sha256_bytes(body),
                    "wireSha256": wire_sha256,
                    "transactionSignature": signature,
                    "recentBlockhash": blockhash,
                    "lastValidBlockHeight": last_valid,
                    "observedBlockHeight": observed,
                    "responseSha256": sha256_bytes(encoded),
                    "status": status,
                }
            )
            with contextlib.suppress(BrokenPipeError, ConnectionResetError):
                self._reply(status, encoded)
            return
        raise Refusal(f"unknown RPC injection {kind}")

    def _wait_until_expired(self, last_valid: int) -> int:
        deadline = time.monotonic() + self.server.state.expiry_timeout
        request_id = 0
        while time.monotonic() < deadline:
            request_id += 1
            body = json.dumps(
                {
                    "jsonrpc": "2.0",
                    "id": f"dclutch-lifecycle-chaos-expiry-{request_id}",
                    "method": "getBlockHeight",
                    "params": [{"commitment": "finalized"}],
                },
                separators=(",", ":"),
            ).encode()
            status, response = self._forward(body)
            value = unique_json_bytes(response, "getBlockHeight response")
            height = value.get("result") if isinstance(value, dict) else None
            self.server.state.append(
                {
                    "source": "supervisor-expiry-poll",
                    "method": "getBlockHeight",
                    "requestSha256": sha256_bytes(body),
                    "responseSha256": sha256_bytes(response),
                    "status": status,
                    "blockHeight": height,
                }
            )
            if status != 200 or not isinstance(height, int) or height < 0:
                raise Refusal("getBlockHeight did not return one nonnegative height")
            if height > last_valid:
                return height
            time.sleep(0.05)
        raise Refusal("blockhash did not expire within the bounded case timeout")

    def _reply(self, status: int, body: bytes) -> None:
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


class _RpcServer(http.server.ThreadingHTTPServer):
    allow_reuse_address = False
    daemon_threads = True

    def __init__(self, state: _ProxyState):
        super().__init__(("127.0.0.1", 0), _RpcHandler)
        self.state = state


class RpcFaultProxy:
    def __init__(self, upstream: str, rule: RpcRule, journal_root: Path, expiry_timeout: float):
        self.state = _ProxyState(upstream, rule, journal_root, expiry_timeout)
        self.server = _RpcServer(self.state)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    @property
    def url(self) -> str:
        host, port = self.server.server_address
        return f"http://{host}:{port}"

    def __enter__(self) -> "RpcFaultProxy":
        self.thread.start()
        return self

    def __exit__(self, *_args: object) -> None:
        self.state.release_delivery.set()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)


def rpc_rule(case: str) -> RpcRule:
    if case.startswith("kill-") and case.removeprefix("kill-") in BOUNDARIES:
        return RpcRule("kill-delivery", "sendTransaction", case.removeprefix("kill-"))
    if case not in RPC_CASES:
        raise Refusal(f"{case} is not an RPC chaos case")
    return RpcRule(case, "sendTransaction", "hot")


def validate_poll_only(trace: Sequence[dict[str, Any]], injected_case: str) -> None:
    client_sends: dict[tuple[str | None, str | None], int] = {}
    for row in trace:
        if row.get("source") == "client" and row.get("method") == "sendTransaction":
            key = (row.get("stage"), row.get("intentSha256"))
            client_sends[key] = client_sends.get(key, 0) + 1
    repeated = {key: count for key, count in client_sends.items() if count > 1}
    if repeated:
        raise Refusal(f"{injected_case} recovery resent a frozen intent instead of polling: {repeated}")
    client_rows = [row for row in trace if row.get("source") == "client"]
    methods = [row.get("method") for row in client_rows]
    if injected_case in {"rpc-timeout", "blockhash-expiry"} or injected_case.startswith("kill-"):
        try:
            send_index = methods.index("sendTransaction")
        except ValueError as exc:
            raise Refusal(f"{injected_case} did not reach its frozen send") from exc
        injected = next(
            (
                row
                for row in trace
                if row.get("source") in {"injected-forward", "injected-expired-forward"}
                and row.get("method") == "sendTransaction"
            ),
            None,
        )
        if injected is None or not isinstance(injected.get("transactionSignature"), str):
            raise Refusal(f"{injected_case} omitted its frozen transaction signature")
        signature = injected["transactionSignature"]
        polls = [
            row
            for row in client_rows[send_index + 1 :]
            if row.get("method") == "getSignatureStatuses"
        ]
        if not polls:
            raise Refusal(f"{injected_case} recovery did not poll the frozen signature")
        if not any(signature in row.get("signatureStatusValues", []) for row in polls):
            raise Refusal(f"{injected_case} recovery polled another signature")


def copy_fixture(spec: Spec, case_work: Path) -> None:
    fixture = spec.path.parent / "fixture"
    if fixture.is_symlink() or not fixture.is_dir():
        raise Refusal("chaos spec sibling fixture must be one non-symlink directory")
    if any(path.is_symlink() for path in fixture.rglob("*")):
        raise Refusal("chaos fixture contains a symlink")
    shutil.copytree(fixture, case_work, symlinks=False)


def wait_prepared(control: Path, child: subprocess.Popen[bytes], timeout: float) -> None:
    deadline = time.monotonic() + timeout
    prepared = control / "PREPARED.json"
    while time.monotonic() < deadline:
        if child.poll() is not None:
            raise Refusal("session exited before the chaos prestate handshake")
        if prepared.exists():
            value = read_unique_json(prepared, "chaos prepared handshake")
            if value != {"schema": CONTROL_SCHEMA, "state": "prepared"}:
                raise Refusal("chaos prepared handshake is malformed")
            return
        time.sleep(POLL_SECONDS)
    raise Refusal("session did not expose its prepared prestate within the bound")


def wait_fault_armed(
    control: Path,
    child: subprocess.Popen[bytes],
    case: str,
    timeout: float,
) -> None:
    deadline = time.monotonic() + timeout
    armed = control / "FAULT_ARMED.json"
    expected = {"schema": CONTROL_SCHEMA, "state": "fault-armed", "fault": case}
    while time.monotonic() < deadline:
        if child.poll() is not None:
            raise Refusal(f"session exited before the {case} fault boundary")
        if armed.exists():
            if read_unique_json(armed, "chaos fault-armed handshake") != expected:
                raise Refusal("chaos fault-armed handshake is malformed")
            return
        time.sleep(POLL_SECONDS)
    raise Refusal(f"session did not expose the {case} fault boundary within the bound")


def prestart_fault(spec: Spec, case: str, case_work: Path) -> str | None:
    if case == "corrupted-evidence":
        return corrupt_evidence(case_work / spec.evidence_relative)
    if case == "replaced-evidence":
        return replace_evidence(
            case_work / spec.evidence_relative,
            case_work / spec.replacement_relative,
        )
    if case in {
        "wallet-underfund",
        "wallet-surplus",
        "blockhash-expiry",
        "late-child-refusal",
    }:
        # The opaque driver owns the local-only mutation.  The harness states the
        # fault in the fsynced control file and later proves that a refusal moved
        # no tracked account.  No private key or packet crosses this seam.
        atomic_json(
            case_work / "control" / "FAULT.json",
            {"schema": CONTROL_SCHEMA, "state": "fault", "fault": case},
        )
    return None


def run_with_handshake(
    spec: Spec,
    case: str,
    case_work: Path,
    rpc_url: str,
    control: Path,
) -> tuple[ProcessResult, str | None, dict[str, Any]]:
    argv, cwd, environment = expanded_command(
        spec.command,
        case=case,
        case_work=case_work,
        rpc_url=rpc_url,
        control=control,
    )
    started = time.monotonic_ns()
    child = subprocess.Popen(
        argv,
        cwd=cwd,
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    try:
        wait_prepared(control, child, spec.journal_timeout_seconds)
        before = prestart_fault(spec, case, case_work)
        go = {"schema": CONTROL_SCHEMA, "state": "go"}
        if case in {"wallet-underfund", "wallet-surplus"}:
            wait_fault_armed(control, child, case, spec.journal_timeout_seconds)
            before_snapshot = observe(spec, case, case_work, rpc_url, control, "before")
            atomic_json(control / "GO.json", go)
        elif case in {"blockhash-expiry", "late-child-refusal"}:
            atomic_json(control / "GO.json", go)
            wait_fault_armed(control, child, case, spec.journal_timeout_seconds)
            before_snapshot = observe(spec, case, case_work, rpc_url, control, "before")
            atomic_json(
                control / "FAULT_GO.json",
                {"schema": CONTROL_SCHEMA, "state": "fault-go", "fault": case},
            )
        else:
            before_snapshot = observe(spec, case, case_work, rpc_url, control, "before")
            atomic_json(control / "GO.json", go)
        stdout, stderr = child.communicate(timeout=spec.case_timeout_seconds)
    except subprocess.TimeoutExpired:
        terminate_group(child, force=True)
        stdout, stderr = child.communicate()
        raise Refusal(f"{case} exceeded its bounded session timeout")
    except Exception:
        terminate_group(child, force=True)
        child.communicate()
        raise
    result = ProcessResult(child.returncode, stdout, stderr, time.monotonic_ns() - started)
    record_attempt(case_work / "attempt-1", argv, result)
    return result, before, before_snapshot


def case_result(
    spec: Spec,
    case: str,
    case_work: Path,
    baseline: dict[str, Any] | None,
) -> tuple[dict[str, Any], dict[str, Any] | None]:
    copy_fixture(spec, case_work)
    control = case_work / "control"
    control.mkdir(exist_ok=False)
    rpc_url = spec.rpc_upstream or "http://127.0.0.1:1"
    proxy: RpcFaultProxy | None = None
    killed_intent: str | None = None
    evidence_before: str | None = None
    before_snapshot: dict[str, Any] | None = None
    cleanup_attempted = False
    try:
        if case in RPC_CASES or case.startswith("kill-"):
            if spec.rpc_upstream is None:
                raise Refusal(f"{case} requires a checked loopback upstream")
            proxy = RpcFaultProxy(
                spec.rpc_upstream,
                rpc_rule(case),
                case_work / spec.journal_relative,
                spec.case_timeout_seconds * 0.9,
            )
            proxy.__enter__()
            rpc_url = proxy.url

        if case.startswith("kill-"):
            # Kill cases do not use the prestate handshake: the exact session
            # advances until the selected semantic owner has fsynced Submitted.
            boundary = case.removeprefix("kill-")
            result, killed_intent = kill_then_resume(
                spec, case, case_work, rpc_url, control, boundary, proxy
            )
        else:
            result, evidence_before, before_snapshot = run_with_handshake(
                spec, case, case_work, rpc_url, control
            )

        expected_refusal = case in REFUSAL_CASES or case == "blockhash-expiry"
        if expected_refusal:
            if result.returncode == 0:
                raise Refusal(f"{case} accepted a hostile lifecycle")
        elif result.returncode != 0:
            raise Refusal(f"{case} failed with status {result.returncode}")

        if proxy is not None:
            if proxy.state.rule.kind == "rpc-timeout":
                proxy.state.release_delivery.set()
            if not proxy.state.injection_done.wait(spec.case_timeout_seconds):
                raise Refusal(f"{case} RPC injection did not finish within its bound")
            if proxy.state.failure is not None:
                raise Refusal(f"{case} RPC proxy failed: {proxy.state.failure}")
            if not proxy.state.fired:
                raise Refusal(f"{case} never reached its selected RPC injection")
            validate_poll_only(proxy.state.trace, case)
            write_json_new(
                case_work / "RPC_TRACE.json",
                {"schema": RPC_TRACE_SCHEMA, "case": case, "rows": proxy.state.trace},
            )

        snapshot = observe(spec, case, case_work, rpc_url, control, "after")
        expected_snapshot = before_snapshot if expected_refusal else baseline
        if expected_snapshot is not None and snapshot != expected_snapshot:
            comparison = "fault prestate" if expected_refusal else "clean baseline"
            raise Refusal(f"{case} changed canonical account bytes or lamports from {comparison}")
        if expected_refusal and (case_work / spec.session_relative).exists():
            raise Refusal(f"{case} wrote a terminal finalized session on refusal")
        if expected_refusal:
            validate_refusal_journals(spec, case, case_work)
        session_digest = None
        if not expected_refusal:
            session = validate_session(spec, case_work)
            session_digest = sha256_bytes(
                (json.dumps(session, separators=(",", ":"), sort_keys=True)).encode()
            )
        cleanup_attempted = True
        teardown_case(
            spec,
            case,
            case_work,
            spec.rpc_upstream or rpc_url,
            control,
        )
        output = {
            "case": case,
            "status": "passed",
            "expectedRefusal": expected_refusal,
            "exitStatus": result.returncode,
            "snapshotSha256": sha256_bytes(
                (json.dumps(snapshot, separators=(",", ":"), sort_keys=True)).encode()
            ),
            "sessionSha256": session_digest,
            "killedIntentSha256": killed_intent,
            "evidencePreFaultSha256": evidence_before,
            "pollOnlyRecovery": True,
        }
        write_json_new(case_work / "RESULT.json", output)
        return output, snapshot
    finally:
        try:
            if proxy is not None:
                proxy.__exit__()
        finally:
            if not cleanup_attempted:
                teardown_case(
                    spec,
                    case,
                    case_work,
                    spec.rpc_upstream or rpc_url,
                    control,
                )


def run(spec: Spec, work: Path, selected: Sequence[str]) -> dict[str, Any]:
    if not work.is_absolute() or work.exists() or work.is_symlink():
        raise Refusal("chaos work must be one fresh absolute path")
    if not selected or len(set(selected)) != len(selected) or any(case not in ALL_CASES for case in selected):
        raise Refusal("selected chaos cases are empty, duplicated, or unknown")
    if selected[0] != "baseline":
        raise Refusal("baseline must run first and own the comparison snapshot")
    work.mkdir(parents=True)
    (work / "cases").mkdir()
    rows: list[dict[str, Any]] = []
    baseline: dict[str, Any] | None = None
    for case in selected:
        row, snapshot = case_result(spec, case, work / "cases" / case, baseline)
        rows.append(row)
        if case == "baseline":
            baseline = snapshot
    summary = {
        "schema": SUMMARY_SCHEMA,
        "sourceRevision": spec.source_revision,
        "spec": str(spec.path),
        "specSha256": sha256_file(spec.path),
        "status": "passed",
        "passCount": len(rows),
        "caseCount": len(selected),
        "cases": rows,
    }
    write_json_new(work / "SUMMARY.json", summary)
    return summary


def parse(argv: Sequence[str]) -> tuple[Spec, Path, tuple[str, ...]]:
    parser = argparse.ArgumentParser()
    parser.add_argument("--spec", required=True)
    parser.add_argument("--work", required=True)
    parser.add_argument("--case", action="append", dest="cases")
    args = parser.parse_args(argv)
    spec_path = Path(args.spec)
    if not spec_path.is_absolute() or spec_path.is_symlink() or not spec_path.is_file():
        raise Refusal("--spec must be one existing absolute non-symlink file")
    work = Path(args.work)
    selected = tuple(args.cases) if args.cases else ALL_CASES
    if selected and selected[0] != "baseline":
        selected = ("baseline", *selected)
    return parse_spec(spec_path), work, selected


def main(argv: Sequence[str] | None = None) -> int:
    try:
        spec, work, selected = parse(list(argv) if argv is not None else os.sys.argv[1:])
        summary = run(spec, work, selected)
        print(json.dumps(summary, sort_keys=True))
        return 0
    except (Refusal, OSError, subprocess.SubprocessError) as exc:
        print(f"lifecycle-chaos: refusing: {exc}", file=os.sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
