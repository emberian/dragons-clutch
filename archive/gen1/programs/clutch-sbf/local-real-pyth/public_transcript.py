#!/usr/bin/env python3
"""Build and audit a public-safe five-file local-real-Pyth transcript."""

from __future__ import annotations

import argparse
import getpass
import hashlib
import ipaddress
import json
import os
import pwd
import re
import sys
from collections import Counter
from pathlib import Path
from typing import Any


PUBLIC_FILES = (
    "campaign.json",
    "result.json",
    "probe-evidence.json",
    "probe-before.txt",
    "probe-after.txt",
)
CLAIM = "NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE"
PROBE_SCHEMA = "dragons-clutch/operator/public-loopback-listener-probe/v1"
PROBE_EVIDENCE_SCHEMA = "dragons-clutch/operator/public-loopback-probe-evidence/v1"
LOWER_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
IPV4_LITERAL = re.compile(r"(?<![0-9.])(?:[0-9]{1,3}\.){3}[0-9]{1,3}(?![0-9.])")
IPV6_LITERAL = re.compile(
    r"(?<![0-9A-Fa-f:])(?:[0-9A-Fa-f]{0,4}:){2,7}[0-9A-Fa-f]{0,4}(?![0-9A-Fa-f:])"
)
USER_HOME_PATH = re.compile(
    r"(?:/Users/|/home/|/root(?:/|\b)|/private/var/folders/|[A-Za-z]:\\Users\\)"
)
PID_LINE = re.compile(r"(?im)^\s*(?:pid|process[_ -]?id)\s*:")
LSOF_ROW = re.compile(r"(?m)^\S+\s+\d+\s+\S+\s+\d+[A-Za-z]*\s+IPv[46]\b")
SECRET_TEXT = re.compile(
    r"(?i)(?:private[-_ ]?key|secret[-_ ]?key|seed[-_ ]?phrase|mnemonic|keypair|"
    r"-----BEGIN [A-Z ]*PRIVATE KEY-----)"
)
SECRET_KEYS = {
    "private_key",
    "privatekey",
    "secret",
    "secret_key",
    "secretkey",
    "seed_phrase",
    "seedphrase",
    "mnemonic",
    "keypair",
}
PID_KEYS = {"pid", "process_id", "processid", "fd", "file_descriptor"}


class PublicTranscriptError(ValueError):
    """The candidate public transcript is incomplete or unsafe."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise PublicTranscriptError(message)


def sha256_bytes(body: bytes) -> str:
    return hashlib.sha256(body).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def read_json(path: Path) -> Any:
    def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        value: dict[str, Any] = {}
        for key, item in pairs:
            require(key not in value, f"{path.name} has duplicate JSON key {key!r}")
            value[key] = item
        return value

    try:
        return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=reject_duplicates)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise PublicTranscriptError(f"cannot read canonical JSON {path}: {error}") from error


def canonical_json(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True, ensure_ascii=True) + "\n").encode()


def checked_port(value: str, role: str) -> int:
    require(value.isascii() and value.isdecimal(), f"{role} port is not decimal")
    port = int(value)
    require(1024 <= port <= 65535, f"{role} port is outside 1024..65535")
    return port


def checked_sha256(value: str, role: str) -> str:
    require(LOWER_SHA256.fullmatch(value) is not None, f"{role} is not lowercase SHA-256")
    return value


def endpoint(port: int) -> str:
    return f"127.0.0.1:{port}"


def local_username() -> str:
    try:
        return pwd.getpwuid(os.getuid()).pw_name
    except (KeyError, OSError):
        return getpass.getuser()


def parse_raw_probe(
    path: Path,
    rpc_port: int,
    websocket_port: int,
    faucet_port: int,
    gossip_port: int,
) -> dict[str, Any]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError) as error:
        raise PublicTranscriptError(f"cannot read raw probe {path}: {error}") from error
    require(lines and lines[0] == "loopback listener probe: PASS", f"{path.name} did not pass")
    expected_headers = {
        "rpc": endpoint(rpc_port),
        "websocket": endpoint(websocket_port),
        "faucet": endpoint(faucet_port),
    }
    for role, expected in expected_headers.items():
        require(f"{role}: {expected}" in lines, f"{path.name} does not bind exact {role} endpoint")

    observed: dict[str, list[str]] = {"tcp": [], "udp": []}
    section: str | None = None
    for line in lines:
        if line == "tcp listeners:":
            section = "tcp"
            continue
        if line == "udp sockets:":
            section = "udp"
            continue
        if section is None or not line:
            continue
        match = re.search(r"\b(TCP|UDP)\s+(\S+?)(?:\s+\(LISTEN\))?\s*$", line)
        require(match is not None, f"{path.name} has an unparseable {section} socket row")
        require(match.group(1).lower() == section, f"{path.name} socket protocol/section mismatch")
        socket_endpoint = match.group(2)
        endpoint_match = re.fullmatch(r"127\.0\.0\.1:([0-9]+)", socket_endpoint)
        require(endpoint_match is not None, f"{path.name} contains a non-loopback socket")
        checked_port(endpoint_match.group(1), f"observed {section}")
        observed[section].append(socket_endpoint)

    required_tcp = {
        endpoint(rpc_port),
        endpoint(websocket_port),
        endpoint(faucet_port),
        endpoint(gossip_port),
    }
    require(required_tcp.issubset(observed["tcp"]), f"{path.name} is missing a named TCP listener")
    require(endpoint(gossip_port) in observed["udp"], f"{path.name} is missing the gossip UDP socket")
    require(observed["udp"], f"{path.name} has no UDP socket observations")
    return {
        "result": "PASS",
        "tcp": Counter(observed["tcp"]),
        "udp": Counter(observed["udp"]),
        "raw_sha256": sha256_file(path),
    }


def render_probe_summary(
    probe: dict[str, Any],
    validator_sha256: str,
    rpc_port: int,
    websocket_port: int,
    faucet_port: int,
    gossip_port: int,
    dynamic_port_range: str,
) -> bytes:
    lines = [
        f"schema: {PROBE_SCHEMA}",
        "result: PASS",
        f"selected_validator_sha256: {validator_sha256}",
        f"rpc: {endpoint(rpc_port)}",
        f"websocket: {endpoint(websocket_port)}",
        f"faucet: {endpoint(faucet_port)}",
        f"gossip: {endpoint(gossip_port)}",
        f"configured_dynamic_port_range: {dynamic_port_range}",
        f"ephemeral_raw_probe_sha256: {probe['raw_sha256']}",
        "ephemeral_raw_probe_retained: false",
        f"tcp_socket_observation_count: {sum(probe['tcp'].values())}",
        f"tcp_unique_endpoint_count: {len(probe['tcp'])}",
    ]
    for socket_endpoint, count in sorted(probe["tcp"].items()):
        lines.append(f"tcp_endpoint: {socket_endpoint} observations={count}")
    lines.extend(
        [
            f"udp_socket_observation_count: {sum(probe['udp'].values())}",
            f"udp_unique_endpoint_count: {len(probe['udp'])}",
        ]
    )
    for socket_endpoint, count in sorted(probe["udp"].items()):
        lines.append(f"udp_endpoint: {socket_endpoint} observations={count}")
    lines.append(
        "scope: exact observed child sockets were loopback-bound; raw process, descriptor, "
        "host-user, filesystem-path, and interface-address rows remain ephemeral"
    )
    return ("\n".join(lines) + "\n").encode()


def json_nodes(value: Any, path: tuple[str, ...] = ()):
    if isinstance(value, dict):
        for key, item in value.items():
            yield path + (key,), key, item
            yield from json_nodes(item, path + (key,))
    elif isinstance(value, list):
        for index, item in enumerate(value):
            yield from json_nodes(item, path + (str(index),))


def public_safety_violations(name: str, body: bytes, username: str | None = None) -> list[str]:
    violations: list[str] = []
    try:
        text = body.decode("utf-8")
    except UnicodeError:
        return [f"{name}: not UTF-8 text"]
    if USER_HOME_PATH.search(text):
        violations.append(f"{name}: contains a local absolute/home path")
    if PID_LINE.search(text) or LSOF_ROW.search(text):
        violations.append(f"{name}: contains PID/FD process metadata")
    if SECRET_TEXT.search(text):
        violations.append(f"{name}: contains a keypair/private-key/secret phrase marker")
    if username and len(username) >= 3:
        username_token = re.compile(rf"(?i)(?<![A-Za-z0-9]){re.escape(username)}(?![A-Za-z0-9])")
        if username_token.search(text):
            violations.append(f"{name}: contains the local username token")
    for literal in IPV4_LITERAL.findall(text):
        try:
            address = ipaddress.ip_address(literal)
        except ValueError:
            violations.append(f"{name}: contains malformed IPv4-like literal")
            continue
        if not address.is_loopback:
            violations.append(f"{name}: contains non-loopback literal IP address")
            break
    for literal in IPV6_LITERAL.findall(text):
        try:
            address = ipaddress.ip_address(literal)
        except ValueError:
            continue
        if not address.is_loopback:
            violations.append(f"{name}: contains non-loopback literal IP address")
            break

    if name.endswith(".json"):
        try:
            value = json.loads(text)
        except json.JSONDecodeError:
            violations.append(f"{name}: invalid JSON")
        else:
            for path, key, item in json_nodes(value):
                normalized_key = key.lower().replace("-", "_")
                if normalized_key in SECRET_KEYS:
                    violations.append(f"{name}: secret-bearing field {'/'.join(path)}")
                if normalized_key in PID_KEYS:
                    violations.append(f"{name}: PID/FD field {'/'.join(path)}")
                if isinstance(item, str) and Path(item).is_absolute():
                    violations.append(f"{name}: absolute filesystem path at {'/'.join(path)}")
    return sorted(set(violations))


def check_public_directory(directory: Path, username: str | None = None) -> None:
    if username is None:
        username = local_username()
    for name in PUBLIC_FILES:
        path = directory / name
        require(path.is_file() and not path.is_symlink(), f"public transcript lacks regular {name}")
        violations = public_safety_violations(name, path.read_bytes(), username)
        require(not violations, "; ".join(violations))

    campaign = read_json(directory / "campaign.json")
    result = read_json(directory / "result.json")
    evidence = read_json(directory / "probe-evidence.json")
    require(campaign.get("claim") == CLAIM, "public campaign truth label differs")
    require(result.get("claim") == CLAIM, "public result truth label differs")
    require(evidence.get("claim") == CLAIM, "public probe truth label differs")
    require(evidence.get("schema") == PROBE_EVIDENCE_SCHEMA, "public probe evidence schema differs")
    validator_sha256 = checked_sha256(
        evidence.get("selected_validator_sha256", ""), "selected validator hash"
    )
    require(
        campaign.get("validator_binary_sha256") == validator_sha256,
        "public campaign and probe evidence validator hashes differ",
    )
    validator_binary = campaign.get("validator_binary")
    require(
        isinstance(validator_binary, str)
        and validator_binary
        and Path(validator_binary).name == validator_binary,
        "public campaign validator identity is not a basename",
    )
    for moment in ("before", "after"):
        summary_path = directory / f"probe-{moment}.txt"
        summary = summary_path.read_text(encoding="utf-8")
        require(f"schema: {PROBE_SCHEMA}\n" in summary, f"public {moment} probe schema differs")
        require("result: PASS\n" in summary, f"public {moment} probe did not pass")
        require(
            f"selected_validator_sha256: {validator_sha256}\n" in summary,
            f"public {moment} probe validator hash differs",
        )
        record = evidence.get(f"probe_{moment}", {})
        require(record.get("result") == "PASS", f"probe evidence {moment} result differs")
        require(
            evidence.get(f"probe_{moment}_sha256") == record.get("ephemeral_raw_sha256"),
            f"probe evidence {moment} raw hash aliases differ",
        )
        require(
            record.get("public_summary_sha256") == sha256_file(summary_path),
            f"probe evidence {moment} summary hash differs",
        )
        require(
            isinstance(record.get("tcp_socket_observation_count"), int)
            and record["tcp_socket_observation_count"] >= 4,
            f"probe evidence {moment} TCP count is invalid",
        )
        require(
            isinstance(record.get("udp_socket_observation_count"), int)
            and record["udp_socket_observation_count"] >= 1,
            f"probe evidence {moment} UDP count is invalid",
        )


def build_public_transcript(args: argparse.Namespace) -> None:
    work = args.work.resolve()
    output = args.output.resolve()
    require(work != output, "public output must differ from the ephemeral work directory")
    output.mkdir(parents=True, exist_ok=True)
    for name in PUBLIC_FILES:
        target = output / name
        require(
            not target.exists() and not target.is_symlink(),
            f"refusing to overwrite public {name}",
        )

    validator_sha256 = checked_sha256(args.validator_sha256, "selected validator hash")
    validator_log_sha256 = checked_sha256(args.validator_log_sha256, "validator log hash")
    rpc_port = checked_port(args.rpc_port, "RPC")
    websocket_port = checked_port(args.websocket_port, "WebSocket")
    faucet_port = checked_port(args.faucet_port, "faucet")
    gossip_port = checked_port(args.gossip_port, "gossip")
    require(
        len({rpc_port, websocket_port, faucet_port, gossip_port}) == 4,
        "named public endpoints are not distinct",
    )
    require(
        re.fullmatch(r"[0-9]+-[0-9]+", args.dynamic_port_range) is not None,
        "dynamic port range is malformed",
    )

    campaign = read_json(work / "campaign.json")
    result = read_json(work / "result.json")
    require(campaign.get("validator_binary_sha256") == validator_sha256, "campaign validator hash differs")
    validator_binary = campaign.get("validator_binary")
    require(isinstance(validator_binary, str) and validator_binary, "campaign validator path is absent")
    campaign["validator_binary"] = Path(validator_binary).name

    before = parse_raw_probe(
        work / "probe-before.txt", rpc_port, websocket_port, faucet_port, gossip_port
    )
    after = parse_raw_probe(
        work / "probe-after.txt", rpc_port, websocket_port, faucet_port, gossip_port
    )
    before_summary = render_probe_summary(
        before,
        validator_sha256,
        rpc_port,
        websocket_port,
        faucet_port,
        gossip_port,
        args.dynamic_port_range,
    )
    after_summary = render_probe_summary(
        after,
        validator_sha256,
        rpc_port,
        websocket_port,
        faucet_port,
        gossip_port,
        args.dynamic_port_range,
    )
    evidence = {
        "claim": CLAIM,
        "schema": PROBE_EVIDENCE_SCHEMA,
        "selected_validator_sha256": validator_sha256,
        "rpc": endpoint(rpc_port),
        "websocket": endpoint(websocket_port),
        "faucet": endpoint(faucet_port),
        "gossip": endpoint(gossip_port),
        "configured_dynamic_port_range": args.dynamic_port_range,
        # Preserve the established probe-evidence fields and their meaning:
        # these pin the stronger raw lsof observations, which remain ephemeral.
        "probe_before_sha256": before["raw_sha256"],
        "probe_after_sha256": after["raw_sha256"],
        "validator_log_sha256": validator_log_sha256,
        "validator_log_retained": False,
        "probe_before": {
            "result": "PASS",
            "public_summary_sha256": sha256_bytes(before_summary),
            "ephemeral_raw_sha256": before["raw_sha256"],
            "ephemeral_raw_retained": False,
            "tcp_socket_observation_count": sum(before["tcp"].values()),
            "tcp_unique_endpoint_count": len(before["tcp"]),
            "udp_socket_observation_count": sum(before["udp"].values()),
            "udp_unique_endpoint_count": len(before["udp"]),
        },
        "probe_after": {
            "result": "PASS",
            "public_summary_sha256": sha256_bytes(after_summary),
            "ephemeral_raw_sha256": after["raw_sha256"],
            "ephemeral_raw_retained": False,
            "tcp_socket_observation_count": sum(after["tcp"].values()),
            "tcp_unique_endpoint_count": len(after["tcp"]),
            "udp_socket_observation_count": sum(after["udp"].values()),
            "udp_unique_endpoint_count": len(after["udp"]),
        },
        "scope": (
            "proves the exact observed child TCP/UDP socket endpoints were loopback-bound; "
            "raw process, descriptor, host-user, filesystem-path, interface-address, and "
            "validator-log rows remain only in the ephemeral campaign work directory"
        ),
    }

    bodies = {
        "campaign.json": canonical_json(campaign),
        "result.json": canonical_json(result),
        "probe-evidence.json": canonical_json(evidence),
        "probe-before.txt": before_summary,
        "probe-after.txt": after_summary,
    }
    username = local_username()
    for name, body in bodies.items():
        violations = public_safety_violations(name, body, username)
        require(not violations, "; ".join(violations))
    for name, body in bodies.items():
        (output / name).write_bytes(body)
    check_public_directory(output, username)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    build = subparsers.add_parser("build", help="normalize ephemeral campaign output")
    build.add_argument("--work", type=Path, required=True)
    build.add_argument("--output", type=Path, required=True)
    build.add_argument("--validator-sha256", required=True)
    build.add_argument("--validator-log-sha256", required=True)
    build.add_argument("--rpc-port", required=True)
    build.add_argument("--websocket-port", required=True)
    build.add_argument("--faucet-port", required=True)
    build.add_argument("--gossip-port", required=True)
    build.add_argument("--dynamic-port-range", required=True)
    check = subparsers.add_parser("check", help="reject unsafe retained transcript content")
    check.add_argument("--directory", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "build":
            build_public_transcript(args)
        else:
            check_public_directory(args.directory)
    except PublicTranscriptError as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
