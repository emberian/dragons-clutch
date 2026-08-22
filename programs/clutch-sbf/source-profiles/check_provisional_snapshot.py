#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Offline consistency gate for the provisional 2026-08-22 devnet snapshot.

The checker deliberately has no RPC client and invokes no subprocess. It reads
the JSON record, the dated review, and the local-clone shell script; a passing
result cannot promote a source release or mutate the compiled registry.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import pathlib
import re
import sys
from typing import Any


SCHEMA = "dragons-clutch/provisional-source-snapshot/v1"
STATUS = "PROVISIONAL_READ_ONLY"
PROMOTION_REFUSAL = {
    "compiled_registry_row": False,
    "release_identity": False,
    "deployment_manifest": False,
    "authorization_to_sign_or_submit": False,
    "promotion_eligibility": "REFUSE",
}
SOURCE_DOCUMENT = "docs/reviews/DEVNET_REAL_SOURCE_SNAPSHOT_2026-08-22.md"
CLONE_SCRIPT = "programs/clutch-sbf/scripts/run_pyth_devnet_clone.sh"
DEFAULT_MANIFEST = (
    "programs/clutch-sbf/source-profiles/"
    "devnet-real-source-snapshot-2026-08-22.json"
)
HEX_32 = re.compile(r"[0-9a-f]{64}\Z")
HEX_COMMIT = re.compile(r"[0-9a-f]{40}\Z")
HEX_DISCRIMINATOR = re.compile(r"[0-9a-f]{16}\Z")
BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
POST_ACCOUNT_ROLES = [
    "payer",
    "encoded_vaa",
    "receiver_config",
    "treasury",
    "price_update_v2",
    "system_program",
    "write_authority",
]


class SnapshotError(ValueError):
    """A fail-closed snapshot validation error."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SnapshotError(message)


def exact_keys(value: dict[str, Any], expected: set[str], where: str) -> None:
    actual = set(value)
    require(actual == expected, f"{where}: keys {sorted(actual)} != {sorted(expected)}")


def require_int(value: Any, where: str, *, minimum: int | None = None) -> int:
    require(type(value) is int, f"{where}: expected integer")
    if minimum is not None:
        require(value >= minimum, f"{where}: must be >= {minimum}")
    return value


def require_bool(value: Any, where: str) -> bool:
    require(type(value) is bool, f"{where}: expected boolean")
    return value


def require_hex(value: Any, pattern: re.Pattern[str], where: str) -> str:
    require(isinstance(value, str) and pattern.fullmatch(value) is not None,
            f"{where}: malformed lowercase hex")
    return value


def base58_bytes(value: Any, where: str) -> bytes:
    require(isinstance(value, str) and value, f"{where}: expected base58 string")
    number = 0
    for char in value:
        require(char in BASE58_ALPHABET, f"{where}: non-base58 character {char!r}")
        number = number * 58 + BASE58_ALPHABET.index(char)
    body = number.to_bytes((number.bit_length() + 7) // 8, "big") if number else b""
    leading_zeroes = len(value) - len(value.lstrip("1"))
    return b"\0" * leading_zeroes + body


def require_pubkey(value: Any, where: str) -> str:
    decoded = base58_bytes(value, where)
    require(len(decoded) == 32, f"{where}: decoded width {len(decoded)} != 32")
    return value


def require_timestamp(value: Any, where: str) -> dt.datetime:
    require(isinstance(value, str) and value.endswith("Z"),
            f"{where}: expected UTC timestamp ending in Z")
    try:
        parsed = dt.datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError as exc:
        raise SnapshotError(f"{where}: malformed timestamp") from exc
    require(parsed.second == 0 and parsed.microsecond == 0,
            f"{where}: snapshot records minute precision only")
    return parsed


def one_match(pattern: str, text: str, where: str, flags: int = 0) -> str:
    matches = re.findall(pattern, text, flags)
    require(len(matches) == 1, f"{where}: expected one match, found {len(matches)}")
    match = matches[0]
    require(isinstance(match, str), f"{where}: internal capture error")
    return match


def markdown_row(document: str, role: str) -> tuple[str, str]:
    prefix = f"| {role} |"
    rows = [line for line in document.splitlines() if line.startswith(prefix)]
    require(len(rows) == 1, f"source document row {role!r}: found {len(rows)}")
    cells = [cell.strip() for cell in rows[0].strip().strip("|").split("|")]
    require(len(cells) == 3, f"source document row {role!r}: malformed table row")
    address = one_match(r"`([^`]+)`", cells[1], f"source document {role} address")
    return address, cells[2]


def parse_script(script: str) -> dict[str, str]:
    names = ["RPC_URL", "EXPECTED_GENESIS", "RECEIVER", "ROUTER", "PUSH_ORACLE", "CONFIG"]
    return {
        name: one_match(
            rf'^\s*{name}="([^"]+)"\s*$', script, f"clone script {name}", re.MULTILINE
        )
        for name in names
    }


def validate_program(
    program: dict[str, Any], where: str, expected_len: int
) -> tuple[str, str, str]:
    exact_keys(
        program,
        {
            "address",
            "executable",
            "owner_kind",
            "data_len",
            "account_body_sha256",
            "programdata",
        },
        where,
    )
    address = require_pubkey(program["address"], f"{where}.address")
    require_bool(program["executable"], f"{where}.executable")
    require(program["executable"] is True, f"{where}: program must be executable")
    require(program["owner_kind"] == "Upgradeable Loader", f"{where}: owner kind drift")
    require(require_int(program["data_len"], f"{where}.data_len") == expected_len,
            f"{where}: unexpected program-account size")
    body_digest = require_hex(
        program["account_body_sha256"], HEX_32, f"{where}.account_body_sha256"
    )

    programdata = program["programdata"]
    require(isinstance(programdata, dict), f"{where}.programdata: expected object")
    exact_keys(
        programdata,
        {"address", "deployment_slot", "upgrade_authority", "data_len", "account_body_sha256"},
        f"{where}.programdata",
    )
    programdata_address = require_pubkey(
        programdata["address"], f"{where}.programdata.address"
    )
    require_int(programdata["deployment_slot"], f"{where}.programdata.deployment_slot", minimum=1)
    require_pubkey(programdata["upgrade_authority"], f"{where}.programdata.upgrade_authority")
    require_int(programdata["data_len"], f"{where}.programdata.data_len", minimum=1)
    require_hex(
        programdata["account_body_sha256"],
        HEX_32,
        f"{where}.programdata.account_body_sha256",
    )
    return address, programdata_address, body_digest


def validate_manifest(data: Any) -> None:
    require(isinstance(data, dict), "manifest: expected JSON object")
    exact_keys(
        data,
        {
            "schema",
            "status",
            "classification",
            "evidence_inputs",
            "retrieval",
            "cluster",
            "reviewed_upstream",
            "accounts",
            "observed_post_update",
            "observed_update_accounts",
        },
        "manifest",
    )
    require(data["schema"] == SCHEMA, "manifest.schema: unsupported schema")
    require(data["status"] == STATUS, "manifest.status: promotion is refused")
    require(data["classification"] == PROMOTION_REFUSAL,
            "manifest.classification: every promotion surface must refuse")
    require(
        data["evidence_inputs"]
        == {"source_document": SOURCE_DOCUMENT, "clone_script": CLONE_SCRIPT},
        "manifest.evidence_inputs: input paths drifted",
    )

    retrieval = data["retrieval"]
    require(isinstance(retrieval, dict), "retrieval: expected object")
    exact_keys(
        retrieval,
        {"rpc_endpoint", "commitment", "window_start_utc", "window_end_utc", "read_only"},
        "retrieval",
    )
    require(retrieval["rpc_endpoint"] == "https://api.devnet.solana.com",
            "retrieval.rpc_endpoint: unexpected endpoint")
    require(retrieval["commitment"] == "finalized", "retrieval.commitment: must be finalized")
    require_bool(retrieval["read_only"], "retrieval.read_only")
    require(retrieval["read_only"] is True, "retrieval.read_only: must remain true")
    started = require_timestamp(retrieval["window_start_utc"], "retrieval.window_start_utc")
    ended = require_timestamp(retrieval["window_end_utc"], "retrieval.window_end_utc")
    require(started < ended, "retrieval: empty or reversed window")

    cluster = data["cluster"]
    require(cluster == {
        "name": "devnet",
        "genesis_hash": "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG",
    }, "cluster: identity drift")
    require(len(base58_bytes(cluster["genesis_hash"], "cluster.genesis_hash")) == 32,
            "cluster.genesis_hash: decoded width must be 32")

    upstream = data["reviewed_upstream"]
    require(isinstance(upstream, dict), "reviewed_upstream: expected object")
    exact_keys(upstream, {"repository", "commit"}, "reviewed_upstream")
    require(upstream["repository"] == "https://github.com/pyth-network/pyth-crosschain",
            "reviewed_upstream.repository: drift")
    require_hex(upstream["commit"], HEX_COMMIT, "reviewed_upstream.commit")

    accounts = data["accounts"]
    require(isinstance(accounts, dict), "accounts: expected object")
    exact_keys(
        accounts,
        {"receiver_program", "router_program", "push_oracle_program", "receiver_config"},
        "accounts",
    )
    receiver, receiver_data, _ = validate_program(
        accounts["receiver_program"], "accounts.receiver_program", 36
    )
    router, router_data, _ = validate_program(
        accounts["router_program"], "accounts.router_program", 36
    )

    push = accounts["push_oracle_program"]
    require(isinstance(push, dict), "accounts.push_oracle_program: expected object")
    exact_keys(push, {"address", "executable", "classification"}, "accounts.push_oracle_program")
    push_address = require_pubkey(push["address"], "accounts.push_oracle_program.address")
    require_bool(push["executable"], "accounts.push_oracle_program.executable")
    require(push["executable"] is True, "accounts.push_oracle_program: must be executable")
    require(push["classification"] == "cloned-dependency-not-admitted-source-identity",
            "accounts.push_oracle_program: classification drift")

    config = accounts["receiver_config"]
    require(isinstance(config, dict), "accounts.receiver_config: expected object")
    exact_keys(
        config,
        {"address", "owner_kind", "executable", "data_len", "account_body_sha256"},
        "accounts.receiver_config",
    )
    config_address = require_pubkey(config["address"], "accounts.receiver_config.address")
    require(config["owner_kind"] == "receiver-owned", "accounts.receiver_config: owner drift")
    require_bool(config["executable"], "accounts.receiver_config.executable")
    require(config["executable"] is False, "accounts.receiver_config: must be non-executable")
    require(require_int(config["data_len"], "accounts.receiver_config.data_len") == 370,
            "accounts.receiver_config: size drift")
    require_hex(config["account_body_sha256"], HEX_32,
                "accounts.receiver_config.account_body_sha256")

    post = data["observed_post_update"]
    require(isinstance(post, dict), "observed_post_update: expected object")
    exact_keys(
        post,
        {
            "instruction",
            "anchor_global_name",
            "discriminator_hex",
            "account_order",
            "push_oracle_cpi_satisfies_top_level_adjacency",
        },
        "observed_post_update",
    )
    require(post["instruction"] == "PostUpdate", "observed_post_update.instruction: drift")
    require(post["anchor_global_name"] == "global:post_update",
            "observed_post_update.anchor_global_name: drift")
    discriminator = require_hex(
        post["discriminator_hex"], HEX_DISCRIMINATOR, "observed_post_update.discriminator_hex"
    )
    derived = hashlib.sha256(post["anchor_global_name"].encode("ascii")).digest()[:8].hex()
    require(discriminator == derived,
            f"observed_post_update.discriminator_hex: {discriminator} != derived {derived}")
    expected_flags = [
        (True, "transaction-boundary"),
        (None, None),
        (False, None),
        (True, None),
        (True, None),
        (False, None),
        (None, "direct-call-or-invoke-signed-pda"),
    ]
    order = post["account_order"]
    require(isinstance(order, list) and len(order) == 7,
            "observed_post_update.account_order: expected seven accounts")
    for index, (entry, role, flags) in enumerate(zip(order, POST_ACCOUNT_ROLES, expected_flags)):
        require(isinstance(entry, dict), f"observed_post_update.account_order[{index}]: object")
        exact_keys(entry, {"index", "role", "writable", "signer"},
                   f"observed_post_update.account_order[{index}]")
        require(entry == {"index": index, "role": role, "writable": flags[0], "signer": flags[1]},
                f"observed_post_update.account_order[{index}]: order or flags drift")
    require_bool(post["push_oracle_cpi_satisfies_top_level_adjacency"],
                 "observed_post_update.push_oracle_cpi_satisfies_top_level_adjacency")
    require(post["push_oracle_cpi_satisfies_top_level_adjacency"] is False,
            "observed_post_update: CPI must not claim top-level adjacency")

    updates = data["observed_update_accounts"]
    require(isinstance(updates, list) and len(updates) == 2,
            "observed_update_accounts: expected exactly two bounded observations")
    all_addresses = [receiver, receiver_data, router, router_data, push_address, config_address]
    for index, update in enumerate(updates):
        where = f"observed_update_accounts[{index}]"
        require(isinstance(update, dict), f"{where}: expected object")
        exact_keys(
            update,
            {
                "address",
                "data_len",
                "feed_id_hex",
                "price",
                "confidence",
                "exponent",
                "publish_time",
                "posted_slot",
                "verification_level",
                "canonical_zero_padding",
                "account_body_sha256",
            },
            where,
        )
        all_addresses.append(require_pubkey(update["address"], f"{where}.address"))
        require(require_int(update["data_len"], f"{where}.data_len") == 134,
                f"{where}: size drift")
        require_hex(update["feed_id_hex"], HEX_32, f"{where}.feed_id_hex")
        require_int(update["price"], f"{where}.price")
        require_int(update["confidence"], f"{where}.confidence", minimum=0)
        require_int(update["exponent"], f"{where}.exponent")
        require_int(update["publish_time"], f"{where}.publish_time", minimum=1)
        require_int(update["posted_slot"], f"{where}.posted_slot", minimum=1)
        require(update["verification_level"] == "Full", f"{where}: verification level drift")
        require_bool(update["canonical_zero_padding"], f"{where}.canonical_zero_padding")
        require(update["canonical_zero_padding"] is True, f"{where}: padding not canonical")
        require_hex(update["account_body_sha256"], HEX_32, f"{where}.account_body_sha256")
    require(len(all_addresses) == len(set(all_addresses)), "accounts: addresses must be distinct")


def validate_script_agreement(data: dict[str, Any], script: str) -> None:
    values = parse_script(script)
    expected = {
        "RPC_URL": data["retrieval"]["rpc_endpoint"],
        "EXPECTED_GENESIS": data["cluster"]["genesis_hash"],
        "RECEIVER": data["accounts"]["receiver_program"]["address"],
        "ROUTER": data["accounts"]["router_program"]["address"],
        "PUSH_ORACLE": data["accounts"]["push_oracle_program"]["address"],
        "CONFIG": data["accounts"]["receiver_config"]["address"],
    }
    require(values == expected, f"clone script identity drift: {values!r} != {expected!r}")


def validate_document_agreement(data: dict[str, Any], document: str) -> None:
    endpoint = one_match(r"exact endpoint `([^`]+)`", document, "source document endpoint")
    genesis = one_match(
        r"Canonical devnet genesis hash observed:\s*```text\s*([^\s`]+)\s*```",
        document,
        "source document genesis",
    )
    upstream = one_match(
        r"reviewed upstream source at commit\s*`([0-9a-f]{40})`",
        document,
        "source document upstream commit",
    )
    window = re.findall(
        r"Retrieval window: (\d{4}-\d{2}-\d{2}) (\d{2}:\d{2})[–-](\d{2}:\d{2}) UTC\.",
        document,
    )
    require(len(window) == 1, f"source document retrieval window: found {len(window)}")
    day, started, ended = window[0]
    require(endpoint == data["retrieval"]["rpc_endpoint"], "source document endpoint drift")
    require(genesis == data["cluster"]["genesis_hash"], "source document genesis drift")
    require(upstream == data["reviewed_upstream"]["commit"], "source document upstream drift")
    require(f"{day}T{started}:00Z" == data["retrieval"]["window_start_utc"],
            "source document retrieval start drift")
    require(f"{day}T{ended}:00Z" == data["retrieval"]["window_end_utc"],
            "source document retrieval end drift")

    accounts = data["accounts"]
    receiver_address, receiver_facts = markdown_row(document, "upgraded receiver")
    require(receiver_address == accounts["receiver_program"]["address"],
            "source document receiver address drift")
    require(one_match(r"(\d+)-byte Program account", receiver_facts,
                      "source document receiver size")
            == str(accounts["receiver_program"]["data_len"]),
            "source document receiver size drift")
    require(one_match(r"full Program-account body SHA-256 `([0-9a-f]{64})`", receiver_facts,
                      "source document receiver digest")
            == accounts["receiver_program"]["account_body_sha256"],
            "source document receiver digest drift")

    router_address, router_facts = markdown_row(document, "upgraded Wormhole/router")
    require(router_address == accounts["router_program"]["address"],
            "source document router address drift")
    require(one_match(r"(\d+)-byte Program account", router_facts,
                      "source document router size") == str(accounts["router_program"]["data_len"]),
            "source document router size drift")
    require(one_match(r"full Program-account body SHA-256 `([0-9a-f]{64})`", router_facts,
                      "source document router digest")
            == accounts["router_program"]["account_body_sha256"],
            "source document router digest drift")

    for row_name, manifest_name in [
        ("receiver ProgramData", "receiver_program"),
        ("router ProgramData", "router_program"),
    ]:
        address, facts = markdown_row(document, row_name)
        programdata = accounts[manifest_name]["programdata"]
        require(address == programdata["address"], f"source document {row_name} address drift")
        require(int(one_match(r"deployment slot `(\d+)`", facts,
                              f"source document {row_name} slot"))
                == programdata["deployment_slot"], f"source document {row_name} slot drift")
        require(one_match(r"authority `([^`]+)`", facts,
                          f"source document {row_name} authority")
                == programdata["upgrade_authority"],
                f"source document {row_name} authority drift")
        require(int(one_match(r"([0-9,]+) bytes", facts,
                              f"source document {row_name} size").replace(",", ""))
                == programdata["data_len"], f"source document {row_name} size drift")
        require(one_match(r"full account-body SHA-256 `([0-9a-f]{64})`", facts,
                          f"source document {row_name} digest")
                == programdata["account_body_sha256"],
                f"source document {row_name} digest drift")

    push_address, push_facts = markdown_row(document, "upgraded push oracle")
    require(push_address == accounts["push_oracle_program"]["address"],
            "source document push-oracle address drift")
    require("not an admitted Clutch source identity" in push_facts,
            "source document push-oracle classification drift")

    config_address, config_facts = markdown_row(document, "receiver Config")
    config = accounts["receiver_config"]
    require(config_address == config["address"], "source document Config address drift")
    require(int(one_match(r"([0-9,]+) bytes", config_facts,
                          "source document Config size").replace(",", "")) == config["data_len"],
            "source document Config size drift")
    require(one_match(r"full body SHA-256 `([0-9a-f]{64})`", config_facts,
                      "source document Config digest") == config["account_body_sha256"],
            "source document Config digest drift")

    post = data["observed_post_update"]
    require(one_match(r"began with bytes `([0-9a-f]{16})`", document,
                      "source document post discriminator") == post["discriminator_hex"],
            "source document post discriminator drift")
    numbered_roles = re.findall(
        r"^\d+\.\s+(.+?)(?=\s+—|;|$)", document, re.MULTILINE
    )
    normalized_roles = [role.strip().replace("`", "").replace("PriceUpdateV2", "price_update_v2")
                        .replace("System Program", "system_program")
                        .replace("write authority", "write_authority")
                        .replace("receiver Config", "receiver_config")
                        .replace("encoded VAA", "encoded_vaa")
                        for role in numbered_roles]
    require(normalized_roles[:7] == POST_ACCOUNT_ROLES,
            f"source document post account order drift: {normalized_roles[:7]!r}")

    update_by_address = {item["address"]: item for item in data["observed_update_accounts"]}
    document_updates: dict[str, list[str]] = {}
    for line in document.splitlines():
        if not line.startswith("| `"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) == 8 and re.fullmatch(r"`[1-9A-HJ-NP-Za-km-z]+`", cells[0]):
            address = cells[0].strip("`")
            document_updates[address] = cells
    require(set(document_updates) == set(update_by_address),
            "source document update-account identities drift")
    for address, cells in document_updates.items():
        update = update_by_address[address]
        require(cells[1].strip("`") == update["feed_id_hex"],
                f"source document update {address}: feed id drift")
        numeric = [int(cell.replace(",", "")) for cell in cells[2:7]]
        require(numeric == [
            update["price"], update["confidence"], update["exponent"],
            update["publish_time"], update["posted_slot"],
        ], f"source document update {address}: numeric fields drift")
        require(cells[7].strip("`") == update["account_body_sha256"],
                f"source document update {address}: digest drift")


def check(root: pathlib.Path, manifest_rel: str = DEFAULT_MANIFEST) -> None:
    manifest_path = root / manifest_rel
    try:
        data = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise SnapshotError(f"manifest unreadable: {manifest_path}: {exc}") from exc
    validate_manifest(data)
    source_document = root / data["evidence_inputs"]["source_document"]
    clone_script = root / data["evidence_inputs"]["clone_script"]
    try:
        document_text = source_document.read_text(encoding="utf-8")
        script_text = clone_script.read_text(encoding="utf-8")
    except OSError as exc:
        raise SnapshotError(f"evidence input unreadable: {exc}") from exc
    validate_document_agreement(data, document_text)
    validate_script_agreement(data, script_text)


def repository_root() -> pathlib.Path:
    return pathlib.Path(__file__).resolve().parents[3]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=pathlib.Path, default=repository_root())
    parser.add_argument("--manifest", default=DEFAULT_MANIFEST)
    args = parser.parse_args(argv)
    try:
        check(args.root.resolve(), args.manifest)
    except SnapshotError as exc:
        print(f"STOP provisional-source-snapshot: {exc}", file=sys.stderr)
        return 1
    print(
        "PASS provisional-source-snapshot "
        "status=PROVISIONAL_READ_ONLY promotion=REFUSE network=unused"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
