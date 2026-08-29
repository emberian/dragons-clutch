#!/usr/bin/env python3
"""Offline decision-0012 Upgrade/open-market plan assembler.

This module never opens a keypair, creates an RPC client, or invokes a Solana
binary.  It joins already-captured public facts into a fail-closed operator
plan.  The existing successor Upgrade commands remain the authority for every
read and mutation.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
import sys
from pathlib import Path
from typing import Any


TEMPLATE_SCHEMA = "dclutch-decision-0012-devnet-upgrade-dryplan-v1"
INPUTS_SCHEMA = "dclutch-decision-0012-devnet-upgrade-inputs-v1"
GATE_SCHEMA = "dclutch-checked-upgrade-gate-v1"
PROVENANCE_SCHEMA = "dclutch-sbf-link-provenance-v1"
CAPTURE_SCHEMA = "dclutch-devnet-permanent-substrate-snapshot-v1"
BASELINE_SCHEMA = "dclutch-devnet-upgrade-baseline-v1"
DEVNET_GENESIS = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
DEVNET_ENDPOINT = "https://api.devnet.solana.com/"
CAPTURE_ENDPOINT = "https://api.devnet.solana.com"
AUTHORITY = "4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP"
LOADER = "BPFLoaderUpgradeab1e11111111111111111111111"
BUFFER_METADATA_BYTES = 37
PROGRAMDATA_METADATA_BYTES = 45
DEPLOY1_PROGRAMDATA_LAMPORTS = 31_772_309_520

ROLES = (
    ("registry", "carry-forward", "Hies39GBowHUMZw9rVCfaDTAXNorkQqMGKnukY2MD4Qj", "ENRSwrUEymWaXyrNtyD4QXXXk3tsTmcTGPTUFvnpsRVz", 489100383),
    ("rent", "carry-forward", "DgfYeuorJUmnktxgCmUXy65f6MFBGcc1aMQoauxoJCY3", "78MW6W4iPzBVLceAwTL51CtyLcpcFM2iGVMDbzZtUFmy", 489100242),
    ("custody", "upgrade", "34dhZkSUUhhFPL98KpWXaoG9aMs3EinZo5xN5epJEgGH", "EhB7hHJ7vsCW3nCeqbxbJrn5Jsi6gbqwpVhoLMPZ8ENf", 489100460),
    ("resolution", "upgrade", "2GHmxBawHTmwDRzqXuqdeC9A9Gj2HzucRd29wGpfgzmd", "2QFBQJdLBXAnJWTVK8KeeUtWZEFhQqqN2CbkrWjMjY6f", 489100560),
    ("claims", "upgrade", "85hwTeQGabwFRs71Hafvngb1UmHb6dQoumBv3VV4epNN", "4La2511ddSxUcAQfdhKvEeGEasih3TStbQWVFEQKd34j", 489100803),
    ("trading", "upgrade", "5ywjTNdo6DGTe7bC8p9CgFYWFrBNePx61xeXp8Cdhbkk", "AE1cWbCvXedE23XH3otSxvDQ7xVx7WLNMYDc8y8rqkrn", 489100942),
    ("core", "upgrade", "HezRkcMGTZ5EY2LZk3i4uJbrAjUSDcamAw9B5v68z33N", "AD6mb5SP6yqc5GFexf3xhpr1wKaZQhS7Hrt41iZhKxaN", 489100672),
)
# Finalized DEPLOY-1/DEVNET_ITERATION_2 pre-write facts. A fresh capture must
# still match these exact pins before this specific dryplan can advance.
DEPLOY1_FACTS = {
    "registry": (207_072, 1_442_425_200, "e1f4a20f0fefb60ad8f809f153c4403363d298d5eb11b88e29abe404048ac6e1", "f8aaca90165d50f5020fa2f7f3377674813bbc5a5cda7c361fb110d752d653e0"),
    "rent": (137_608, 958_955_760, "3b857b2236522c29e17b7d73cf27df6e6028fd8298a52df386753638f915ff79", "acf94e2340067e3ededfeaa9c36d7d877d307a32740539fc2cb5244b844812ee"),
    "custody": (360_328, 2_509_086_960, "d171cf742391dcc6ff152171657187d6a62538f38cedc9ce048af457b16746f1", "d83020025e037ef42e01cf88a2368219c10b7a2f6011254edbe7a71336a303c2"),
    "resolution": (588_336, 4_096_022_640, "03842494bc1604b7f4806962157f93529056848f51499a4e0de771d1b8ab1fbf", "47082d659047011c046b7c26b5af4b0fe402b4255fbe81a4a3763140cbff734d"),
    "claims": (1_010_496, 7_034_256_240, "51967830f17ab6ebad074fbaf178482c027910bc9d14a8ade070e17004b84b8a", "bff08ee426a9a6c98a1d11b0ab92fb774f70b1cab1aac0735ea6c990dc96e1b8"),
    "trading": (1_325_848, 9_229_106_160, "7facb8e58e45843f46b9d3d572ced5e45507bfcbfb2250e865b5427baa1b9d3c", "e9b65886b144a556bd68cf201a736d7af64c61ebcbd75a5f71f4543f7c71b5a7"),
    "core": (934_088, 6_502_456_560, "e0cc7109da7a7b2b94cfa5a0f00a63c40ce44519f7d0186b6c1fbfe39b68f0ee", "b76f7a0c2886a32a21986ecf5038b51aa9b28fcf6f37f940c1a11fc35d2ea100"),
}
UPGRADE_ROLES = tuple(row[0] for row in ROLES if row[1] == "upgrade")
ACTIVATION_ROLES = ("core", "claims", "trading", "resolution", "custody")
ACTIVITY_OPERATION_COUNTS = {
    "found": 1,
    "participant": 4,
    "direct": 4,
    "resolve": 1,
    "redeem": 14,
    "retire": 1,
}
ACTIVITY_AUTHORITY_REPO_PATH = "tools/economic-lifecycle-ledger/fixtures/activity-v3-canonical.json"
ACTIVITY_AUTHORITY_SHA256 = "a018a4012ffe981482e67e24958cfefcc4f2ff296802ac02626b6d425e61aacd"
PACKAGES = {
    "core": "dclutch-core-sbf",
    "claims": "dclutch-claims-sbf",
    "trading": "dclutch-trading-sbf",
    "resolution": "dclutch-resolution-proof-sbf",
    "custody": "dclutch-custody-sbf",
}
ARTIFACT_STEMS = {
    "core": "dclutch_core_sbf",
    "claims": "dclutch_claims_sbf",
    "trading": "dclutch_trading_sbf",
    "resolution": "dclutch_resolution_proof_sbf",
    "custody": "dclutch_custody_sbf",
}
HEX = set("0123456789abcdef")
BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


class Refusal(ValueError):
    pass


def refuse(condition: bool, message: str) -> None:
    if not condition:
        raise Refusal(message)


def strict_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise Refusal(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_json_bytes(raw: bytes, label: str) -> dict[str, Any]:
    try:
        value = json.loads(raw, object_pairs_hook=strict_object)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise Refusal(f"{label} is not strict JSON: {error}") from error
    refuse(isinstance(value, dict), f"{label} must be a JSON object")
    return value


def read_canonical_file(path: Path, label: str) -> bytes:
    refuse(path.is_absolute(), f"{label} path must be absolute")
    try:
        metadata = path.lstat()
    except OSError as error:
        raise Refusal(f"cannot read {label}: {error}") from error
    refuse(stat.S_ISREG(metadata.st_mode), f"{label} must be a regular file, not a symlink")
    refuse(path.resolve() == path, f"{label} path must already be canonical")
    return path.read_bytes()


def exact_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    observed = set(value)
    refuse(observed == expected, f"{label} fields are {sorted(observed)}, expected {sorted(expected)}")


def hex_digest(value: Any, label: str) -> str:
    refuse(isinstance(value, str) and len(value) == 64 and set(value) <= HEX, f"{label} must be 64 lowercase hex")
    return value


def unsigned(value: Any, label: str) -> int:
    refuse(isinstance(value, int) and not isinstance(value, bool) and value >= 0, f"{label} must be an unsigned integer")
    return value


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def sha256(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def state_digest(plan: dict[str, Any]) -> str:
    copy = dict(plan)
    copy["stateSha256"] = ""
    raw = canonical_bytes(copy)
    hasher = hashlib.sha256()
    hasher.update(b"dclutch/decision-0012/devnet-upgrade-dryplan/v1\0")
    hasher.update(len(raw).to_bytes(8, "little"))
    hasher.update(raw)
    return hasher.hexdigest()


def read_reference(reference: dict[str, Any], label: str) -> tuple[Path, bytes]:
    exact_keys(reference, {"canonicalPath", "sha256"}, label)
    path_text = reference["canonicalPath"]
    refuse(isinstance(path_text, str) and Path(path_text).is_absolute(), f"{label} path must be absolute")
    path = Path(path_text)
    try:
        metadata = path.lstat()
    except OSError as error:
        raise Refusal(f"cannot read {label}: {error}") from error
    refuse(stat.S_ISREG(metadata.st_mode), f"{label} must be a regular file, not a symlink")
    refuse(path.resolve() == path, f"{label} path must already be canonical")
    raw = path.read_bytes()
    expected = hex_digest(reference["sha256"], f"{label} SHA-256")
    refuse(sha256(raw) == expected, f"{label} bytes do not match pinned SHA-256")
    return path, raw


def gate_file(root: Path, value: Any, expected_path: str, label: str) -> tuple[Path, bytes]:
    refuse(isinstance(value, dict), f"{label} reference must be an object")
    exact_keys(value, {"canonical_path", "bytes", "sha256"}, f"{label} reference")
    refuse(value["canonical_path"] == expected_path, f"{label} path must be {expected_path}")
    unsigned(value["bytes"], f"{label} bytes")
    hex_digest(value["sha256"], f"{label} SHA-256")
    candidate = root / expected_path
    refuse(candidate.parent.resolve().is_relative_to(root), f"{label} escapes the gate root")
    try:
        metadata = candidate.lstat()
    except OSError as error:
        raise Refusal(f"cannot read {label}: {error}") from error
    refuse(stat.S_ISREG(metadata.st_mode), f"{label} must be a regular file, not a symlink")
    refuse(candidate.resolve() == candidate, f"{label} path must already be canonical")
    raw = candidate.read_bytes()
    refuse(len(raw) == value["bytes"], f"{label} byte length changed")
    refuse(sha256(raw) == value["sha256"], f"{label} digest changed")
    return candidate, raw


def validate_provenance(raw: bytes, link: dict[str, Any], role: str, root: Path, gate: dict[str, Any]) -> None:
    value = load_json_bytes(raw, f"{role} provenance")
    exact_keys(value, {
        "schema", "label", "package", "artifact_stem", "source_revision",
        "source_tree_sha256", "build_run_id", "plain_build", "shipped_elf",
        "frame_measurement",
    }, f"{role} provenance")
    refuse(value["schema"] == PROVENANCE_SCHEMA, f"{role} provenance schema changed")
    refuse(value["label"] == role and value["package"] == PACKAGES[role], f"{role} provenance identity changed")
    refuse(value["artifact_stem"] == ARTIFACT_STEMS[role], f"{role} artifact stem changed")
    refuse(value["source_revision"] == gate["source_revision"], f"{role} provenance source revision changed")
    refuse(value["source_tree_sha256"] == gate["source_tree_sha256"], f"{role} provenance source tree changed")
    refuse(value["build_run_id"] == gate["build_run_id"], f"{role} provenance build run changed")
    exact_keys(value["plain_build"], {"invocation", "log", "compile_marker", "sbf_diagnostics_count"}, f"{role} plain_build")
    exact_keys(value["frame_measurement"], {"invocation", "build_log", "compile_marker", "object", "report"}, f"{role} frame_measurement")
    refuse(value["plain_build"]["sbf_diagnostics_count"] == 0, f"{role} has SBF diagnostics")
    refuse(value["shipped_elf"] == link["elf"], f"{role} provenance does not bind the shipped ELF")
    # Rehash every descriptor file object. The Upgrade executor still owns the
    # complete thirteen-link admission; this catches substitution in the five
    # mutable role chains used by this plan.
    for field, item in value["plain_build"].items():
        if field in {"invocation", "log"}:
            refuse(isinstance(item, dict), f"{role} plain_build.{field} must be a file object")
            gate_file(root, item, item.get("canonical_path", ""), f"{role} plain_build.{field}")
    for field in ("invocation", "build_log", "object", "report"):
        item = value["frame_measurement"][field]
        refuse(isinstance(item, dict), f"{role} frame_measurement.{field} must be a file object")
        gate_file(root, item, item.get("canonical_path", ""), f"{role} frame_measurement.{field}")


def validate_gate(reference: dict[str, Any]) -> dict[str, Any]:
    path, raw = read_reference(reference, "checked release gate")
    gate = load_json_bytes(raw, "checked release gate")
    refuse(gate.get("schema") == GATE_SCHEMA, "checked release gate schema changed")
    refuse(isinstance(gate.get("source_revision"), str) and len(gate["source_revision"]) == 40 and set(gate["source_revision"]) <= HEX, "gate source revision is invalid")
    hex_digest(gate.get("source_tree_sha256"), "gate source tree SHA-256")
    links = gate.get("links")
    refuse(isinstance(links, list) and gate.get("link_count") == len(links) == 13, "gate must bind exactly thirteen checked links")
    root = path.parent
    selected: dict[str, Any] = {}
    for role in UPGRADE_ROLES:
        matches = [link for link in links if isinstance(link, dict) and link.get("label") == role]
        refuse(len(matches) == 1, f"gate must contain exactly one {role} link")
        link = matches[0]
        refuse(link.get("package") == PACKAGES[role], f"gate package changed for {role}")
        elf = link.get("elf")
        provenance = link.get("artifact_provenance")
        _, elf_raw = gate_file(root, elf, f"elf/{role}.so", f"{role} ELF")
        refuse(elf_raw.startswith(b"\x7fELF"), f"{role} checked artifact is not an ELF")
        _, provenance_raw = gate_file(root, provenance, f"provenance/{role}.json", f"{role} provenance")
        validate_provenance(provenance_raw, link, role, root, gate)
        selected[role] = {
            "elf": {"canonicalPath": str((root / f"elf/{role}.so").resolve()), "bytes": len(elf_raw), "sha256": sha256(elf_raw)},
            "artifactProvenance": {"canonicalPath": str((root / f"provenance/{role}.json").resolve()), "bytes": len(provenance_raw), "sha256": sha256(provenance_raw)},
        }
    return {"sourceRevision": gate["source_revision"], "sourceTreeSha256": gate["source_tree_sha256"], "roles": selected}


def base58_bytes(value: str, label: str) -> bytes:
    refuse(isinstance(value, str) and value, f"{label} must be base58")
    number = 0
    try:
        for character in value:
            number = number * 58 + BASE58_ALPHABET.index(character)
    except ValueError as error:
        raise Refusal(f"{label} is not base58") from error
    leading = len(value) - len(value.lstrip("1"))
    raw = (b"" if number == 0 else number.to_bytes((number.bit_length() + 7) // 8, "big"))
    raw = b"\0" * leading + raw
    refuse(len(raw) == 32, f"{label} is not a 32-byte public key")
    return raw


def validate_capture(reference: dict[str, Any]) -> dict[str, Any]:
    _, raw = read_reference(reference, "permanent substrate capture")
    capture = load_json_bytes(raw, "permanent substrate capture")
    exact_keys(capture, {
        "schema", "endpoint", "commitment", "rpc_method", "context_slot",
        "expected_upgrade_authority", "fee_payer", "fee_payer_lamports",
        "canonical_role_order", "roles", "program_lamports_total",
        "programdata_lamports_total", "snapshot_sha256",
    }, "permanent substrate capture")
    refuse(capture.get("schema") == CAPTURE_SCHEMA, "permanent substrate capture schema changed")
    refuse(capture.get("endpoint") == CAPTURE_ENDPOINT and capture.get("commitment") == "finalized", "capture is not canonical finalized devnet")
    refuse(capture.get("rpc_method") == "getMultipleAccounts", "capture was not one getMultipleAccounts context")
    refuse(capture.get("expected_upgrade_authority") == AUTHORITY, "capture retained authority changed")
    refuse(unsigned(capture.get("context_slot"), "capture context slot") > 0, "capture context slot is zero")
    base58_bytes(capture.get("fee_payer"), "capture fee payer")
    unsigned(capture.get("fee_payer_lamports"), "capture fee payer lamports")
    refuse(capture.get("canonical_role_order") == [row[0] for row in ROLES], "capture role order changed")
    rows = capture.get("roles")
    refuse(isinstance(rows, list) and len(rows) == 7, "capture must contain seven roles")
    program_total = 0
    programdata_total = 0
    for ordinal, (observed, expected) in enumerate(zip(rows, ROLES)):
        role, _, program, programdata, _ = expected
        deploy1_live_bytes, deploy1_lamports, deploy1_live_sha, deploy1_account_sha = DEPLOY1_FACTS[role]
        exact_keys(observed, {
            "ordinal", "role", "program_id", "programdata_id",
            "program_lamports", "program_data_sha256", "programdata_lamports",
            "programdata_account_bytes", "programdata_account_sha256",
            "deployment_slot", "live_elf_bytes", "live_elf_sha256",
        }, f"{role} capture row")
        refuse(observed.get("ordinal") == ordinal and observed.get("role") == role, f"capture ordinal changed for {role}")
        refuse(observed.get("program_id") == program and observed.get("programdata_id") == programdata, f"capture Loader pair changed for {role}")
        refuse(unsigned(observed.get("deployment_slot"), f"{role} deployment slot") == expected[4], f"{role} deployment slot differs from the admitted DEPLOY-1 prestate")
        refuse(observed.get("live_elf_bytes") == deploy1_live_bytes and observed.get("live_elf_sha256") == deploy1_live_sha, f"{role} live payload differs from the admitted DEPLOY-1 prestate")
        refuse(observed.get("programdata_lamports") == deploy1_lamports, f"{role} ProgramData lamports differ from the admitted DEPLOY-1 prestate")
        refuse(observed.get("programdata_account_sha256") == deploy1_account_sha, f"{role} ProgramData account digest differs from the admitted DEPLOY-1 prestate")
        refuse(unsigned(observed.get("programdata_account_bytes"), f"{role} ProgramData bytes") == unsigned(observed.get("live_elf_bytes"), f"{role} live ELF bytes") + PROGRAMDATA_METADATA_BYTES, f"{role} ProgramData width is inconsistent")
        for field in ("program_data_sha256", "programdata_account_sha256", "live_elf_sha256"):
            hex_digest(observed.get(field), f"{role} {field}")
        program_total += unsigned(observed.get("program_lamports"), f"{role} Program lamports")
        programdata_total += unsigned(observed.get("programdata_lamports"), f"{role} ProgramData lamports")
    refuse(program_total == capture.get("program_lamports_total"), "capture Program lamport total is inconsistent")
    refuse(programdata_total == capture.get("programdata_lamports_total"), "capture ProgramData lamport total is inconsistent")
    # Authenticate the Rust capture's domain-separated self digest.
    expected = hex_digest(capture.get("snapshot_sha256"), "capture self digest")
    digest_input = dict(capture)
    digest_input["snapshot_sha256"] = ""
    # Rust's derived Serialize order is the declared struct field order. JSON
    # object order from the strict decoder retains those bytes' semantic order.
    body = json.dumps(digest_input, separators=(",", ":"), ensure_ascii=False).encode()
    hasher = hashlib.sha256()
    hasher.update(b"dclutch/devnet-permanent-substrate-snapshot/v1\n")
    hasher.update(len(body).to_bytes(8, "little"))
    hasher.update(body)
    refuse(hasher.hexdigest() == expected, "capture self digest changed")
    return capture


def hash_text(hasher: Any, value: Any, label: str) -> None:
    refuse(isinstance(value, str), f"{label} must be text")
    raw = value.encode()
    hasher.update(len(raw).to_bytes(8, "little"))
    hasher.update(raw)


def baseline_digest(baseline: dict[str, Any], role: str) -> str:
    observation = baseline["observation"]
    hasher = hashlib.sha256()
    hasher.update(b"dclutch/devnet-upgrade-baseline/v1\0")
    hash_text(hasher, baseline["schema"], f"{role} baseline schema")
    for item in baseline["canonical_role_order"]:
        hash_text(hasher, item, f"{role} canonical role")
    ordinal = unsigned(baseline["role_ordinal"], f"{role} ordinal")
    refuse(ordinal <= 255, f"{role} ordinal is wider than u8")
    hasher.update(bytes([ordinal]))
    hash_text(hasher, baseline["role"], f"{role} role")
    hasher.update(base58_bytes(baseline["program_id"], f"{role} Program"))
    hasher.update(base58_bytes(baseline["programdata_id"], f"{role} ProgramData"))
    hasher.update(base58_bytes(baseline["expected_upgrade_authority"], f"{role} authority"))
    hash_text(hasher, baseline["genesis_hash"], f"{role} genesis")
    hasher.update(unsigned(baseline["context_slot"], f"{role} context slot").to_bytes(8, "little"))
    hasher.update(unsigned(observation["program_lamports"], f"{role} Program lamports").to_bytes(8, "little"))
    hash_text(hasher, observation["program_owner"], f"{role} Program owner")
    refuse(isinstance(observation["program_executable"], bool), f"{role} Program executable must be boolean")
    hasher.update(bytes([int(observation["program_executable"])]))
    hasher.update(unsigned(observation["program_data_bytes"], f"{role} Program bytes").to_bytes(8, "little"))
    hash_text(hasher, observation["program_account_sha256"], f"{role} Program digest")
    hasher.update(unsigned(observation["programdata_lamports"], f"{role} ProgramData lamports").to_bytes(8, "little"))
    hash_text(hasher, observation["programdata_owner"], f"{role} ProgramData owner")
    refuse(isinstance(observation["programdata_executable"], bool), f"{role} ProgramData executable must be boolean")
    hasher.update(bytes([int(observation["programdata_executable"])]))
    hasher.update(unsigned(observation["programdata_data_bytes"], f"{role} ProgramData bytes").to_bytes(8, "little"))
    hash_text(hasher, observation["programdata_account_sha256"], f"{role} ProgramData digest")
    hasher.update(unsigned(observation["deployment_slot"], f"{role} deployment slot").to_bytes(8, "little"))
    hash_text(hasher, observation["upgrade_authority"], f"{role} observed authority")
    hasher.update(unsigned(observation["live_elf_bytes"], f"{role} live ELF bytes").to_bytes(8, "little"))
    hash_text(hasher, observation["live_elf_sha256"], f"{role} live ELF digest")
    for field in (
        "target_live_elf_bytes",
        "extension_additional_bytes",
        "current_rent_exempt_minimum_lamports",
        "target_rent_exempt_minimum_lamports",
        "extension_lamport_top_up",
    ):
        hasher.update(unsigned(baseline[field], f"{role} {field}").to_bytes(8, "little"))
    return hasher.hexdigest()


def validate_baseline(reference: dict[str, Any], role: str, target_bytes: int, capture_role: dict[str, Any], capture_context_slot: int) -> dict[str, Any]:
    _, raw = read_reference(reference, f"{role} baseline")
    baseline = load_json_bytes(raw, f"{role} baseline")
    exact_keys(baseline, {
        "schema", "canonical_role_order", "role_ordinal", "role",
        "program_id", "programdata_id", "expected_upgrade_authority",
        "rpc_origin_redacted", "genesis_hash", "context_slot", "observation",
        "target_live_elf_bytes", "extension_additional_bytes",
        "current_rent_exempt_minimum_lamports",
        "target_rent_exempt_minimum_lamports", "extension_lamport_top_up",
        "baseline_sha256",
    }, f"{role} baseline")
    refuse(baseline.get("schema") == BASELINE_SCHEMA, f"{role} baseline schema changed")
    expected = next(row for row in ROLES if row[0] == role)
    ordinal = [row[0] for row in ROLES].index(role)
    refuse(baseline.get("canonical_role_order") == [row[0] for row in ROLES], f"{role} baseline role order changed")
    refuse(baseline.get("role") == role and baseline.get("role_ordinal") == ordinal, f"{role} baseline identity changed")
    refuse(baseline.get("program_id") == expected[2] and baseline.get("programdata_id") == expected[3], f"{role} baseline Loader pair changed")
    refuse(baseline.get("expected_upgrade_authority") == AUTHORITY and baseline.get("genesis_hash") == DEVNET_GENESIS, f"{role} baseline authority or genesis changed")
    observation = baseline.get("observation")
    refuse(isinstance(observation, dict), f"{role} baseline observation is absent")
    exact_keys(observation, {
        "program_lamports", "program_owner", "program_executable",
        "program_data_bytes", "program_account_sha256", "programdata_lamports",
        "programdata_owner", "programdata_executable", "programdata_data_bytes",
        "deployment_slot", "upgrade_authority", "live_elf_bytes",
        "live_elf_sha256", "programdata_account_sha256",
    }, f"{role} baseline observation")
    refuse(unsigned(baseline.get("context_slot"), f"{role} baseline context slot") >= capture_context_slot, f"{role} baseline predates the one-context capture")
    refuse(observation.get("upgrade_authority") == AUTHORITY, f"{role} observed authority changed")
    refuse(observation.get("program_owner") == LOADER and observation.get("programdata_owner") == LOADER, f"{role} Loader owner changed")
    refuse(observation.get("program_executable") is True and observation.get("programdata_executable") is False, f"{role} Loader privileges changed")
    refuse(observation.get("programdata_account_sha256") == capture_role["programdata_account_sha256"], f"{role} baseline is not the pre-write capture state")
    refuse(observation.get("deployment_slot") == capture_role["deployment_slot"], f"{role} baseline slot is not the pre-write capture slot")
    refuse(observation.get("program_lamports") == capture_role["program_lamports"], f"{role} Program lamports changed after the pre-write capture")
    refuse(observation.get("program_account_sha256") == capture_role["program_data_sha256"], f"{role} Program account changed after the pre-write capture")
    refuse(observation.get("programdata_lamports") == capture_role["programdata_lamports"], f"{role} ProgramData lamports changed after the pre-write capture")
    refuse(observation.get("programdata_data_bytes") == capture_role["programdata_account_bytes"], f"{role} ProgramData width changed after the pre-write capture")
    refuse(observation.get("live_elf_bytes") == capture_role["live_elf_bytes"] and observation.get("live_elf_sha256") == capture_role["live_elf_sha256"], f"{role} live ELF changed after the pre-write capture")
    current_space = unsigned(observation.get("programdata_data_bytes"), f"{role} current ProgramData bytes")
    live_bytes = unsigned(observation.get("live_elf_bytes"), f"{role} current live ELF bytes")
    refuse(current_space == live_bytes + PROGRAMDATA_METADATA_BYTES, f"{role} baseline ProgramData width is inconsistent")
    admitted_target_live = max(target_bytes, live_bytes)
    target_space = max(current_space, admitted_target_live + PROGRAMDATA_METADATA_BYTES)
    additional = target_space - current_space
    refuse(baseline.get("target_live_elf_bytes") == admitted_target_live, f"{role} target live width changed")
    refuse(baseline.get("extension_additional_bytes") == additional, f"{role} extension width changed")
    target_minimum = unsigned(baseline.get("target_rent_exempt_minimum_lamports"), f"{role} target rent minimum")
    programdata_lamports = unsigned(observation.get("programdata_lamports"), f"{role} ProgramData lamports")
    refuse(baseline.get("extension_lamport_top_up") == max(target_minimum - programdata_lamports, 0), f"{role} rent top-up changed")
    expected_digest = hex_digest(baseline.get("baseline_sha256"), f"{role} baseline self digest")
    refuse(baseline_digest(baseline, role) == expected_digest, f"{role} baseline self digest changed")
    return baseline


def reference_from_input(value: Any, label: str) -> dict[str, Any]:
    refuse(isinstance(value, dict), f"{label} must be an object")
    exact_keys(value, {"canonicalPath", "sha256"}, label)
    return value


def activity_v3_plan() -> dict[str, Any]:
    repo_root = Path(__file__).resolve().parents[3]
    path = repo_root / ACTIVITY_AUTHORITY_REPO_PATH
    try:
        metadata = path.lstat()
    except OSError as error:
        raise Refusal(f"cannot read Activity-v3 economic authority: {error}") from error
    refuse(stat.S_ISREG(metadata.st_mode) and path.resolve() == path, "Activity-v3 economic authority must be a canonical regular file")
    raw = path.read_bytes()
    refuse(sha256(raw) == ACTIVITY_AUTHORITY_SHA256, "Activity-v3 economic authority bytes changed")
    fixture = load_json_bytes(raw, "Activity-v3 economic authority")
    exact_keys(fixture, {
        "schema", "fixtureId", "sourceAuthority", "outcomeCount",
        "collateralMintSupplyAtoms", "hoardCollateralAccount",
        "feeCollateralAccount", "activityV3Authority", "initial", "stages",
        "lamportContract",
    }, "Activity-v3 economic authority")
    refuse(fixture["schema"] == "dclutch-exact-economic-lifecycle-fixture-v1", "Activity-v3 economic authority schema changed")
    refuse(fixture["fixtureId"] == "activity-v3-canonical-four-outcome", "Activity-v3 economic fixture identity changed")
    authority = fixture["activityV3Authority"]
    refuse(isinstance(authority, dict), "Activity-v3 authority body is absent")
    exact_keys(authority, {
        "clusterTarget", "payerWallet", "wallets", "authorization",
        "allLifecycleMutationsExpected", "feeBasisPointsPerSide",
        "feeDenominator",
    }, "Activity-v3 authority body")
    refuse(authority["clusterTarget"] == "devnet" and authority["payerWallet"] == "deployer", "Activity-v3 authority is not the frozen devnet payer partition")
    expected_wallets = [
        ("deployer", "campaign-payer", "360000000", "0"),
        ("collateral-mint", "collateral-mint", "0", "0"),
        ("collateral-wallet", "collateral-wallet", "0", "0"),
        ("founding-beneficiary", "founding-beneficiary", "0", "0"),
        ("founding-projection-witness", "founding-projection-witness", "0", "0"),
        ("founding-source-funder", "founding-source-funder", "0", "0"),
        ("ash", "participant-ash", "0", "50000000"),
        ("birch", "participant-birch", "0", "50000000"),
        ("cobalt", "participant-cobalt", "0", "50000000"),
        ("dahlia", "participant-dahlia", "0", "50000000"),
    ]
    wallets = authority["wallets"]
    refuse(isinstance(wallets, list) and len(wallets) == len(expected_wallets), "Activity-v3 authority must contain ten wallets")
    for row, expected in zip(wallets, expected_wallets):
        exact_keys(row, {"id", "role", "initialFundingLamports", "postInitFundingLamports"}, f"Activity-v3 wallet {expected[0]}")
        refuse(tuple(row.values()) == expected, f"Activity-v3 wallet authority changed for {expected[0]}")
    authorization = authority["authorization"]
    exact_keys(authorization, {
        "initialFundingLamports", "maxPostInitTransferLamports",
        "maxPostInitFeeLamports", "maxFeeLamports", "maxSpendLamports",
        "guaranteedPreLifecycleResidualLamports",
    }, "Activity-v3 authorization")
    expected_authorization = {
        "initialFundingLamports": "360000000",
        "maxPostInitTransferLamports": "200000000",
        "maxPostInitFeeLamports": "10000000",
        "maxFeeLamports": "10000000",
        "maxSpendLamports": "210000000",
        "guaranteedPreLifecycleResidualLamports": "150000000",
    }
    refuse(authorization == expected_authorization, "Activity-v3 authorization caps changed")
    refuse(authority["allLifecycleMutationsExpected"] is True, "Activity-v3 lifecycle reintroduced a nonmutating gap")
    refuse(authority["feeBasisPointsPerSide"] == 50 and authority["feeDenominator"] == "10000", "Activity-v3 fee authority changed")
    observed_counts = {key: 0 for key in ACTIVITY_OPERATION_COUNTS}
    stages = fixture["stages"]
    refuse(isinstance(stages, list), "Activity-v3 stages are absent")
    for stage in stages:
        stage_id = stage.get("id") if isinstance(stage, dict) else None
        events = stage.get("events") if isinstance(stage, dict) else None
        refuse(isinstance(stage_id, str) and isinstance(events, list), "Activity-v3 stage is malformed")
        if stage_id == "founding":
            observed_counts["found"] += 1
        elif stage_id.startswith("participant-"):
            observed_counts["participant"] += 1
        elif stage_id.startswith("direct-"):
            observed_counts["direct"] += 1
        elif stage_id == "resolution":
            observed_counts["resolve"] += 1
        elif stage_id == "payouts":
            observed_counts["redeem"] += sum(1 for event in events if event.get("kind") == "redeem")
        elif stage_id == "aggregate-retirement":
            observed_counts["retire"] += 1
    refuse(observed_counts == ACTIVITY_OPERATION_COUNTS, "Activity-v3 operation ensemble changed")
    return {
        "stage": "semantic-authority-frozen-live-artifacts-pending",
        "semanticAuthority": {
            "repoPath": ACTIVITY_AUTHORITY_REPO_PATH,
            "sha256": ACTIVITY_AUTHORITY_SHA256,
            "schema": fixture["schema"],
            "fixtureId": fixture["fixtureId"],
        },
        "target": {"kind": "devnet", "genesisHash": DEVNET_GENESIS},
        "scenarioSchemaRequired": "dclutch-devnet-economic-scenario-v1",
        "manifestSchemaRequired": "dclutch-devnet-activity-manifest-v3",
        "derived": {
            "walletCount": len(wallets),
            "initialFundingLamports": int(authorization["initialFundingLamports"]),
            "postInitTransferLamports": int(authorization["maxPostInitTransferLamports"]),
            "maxPostInitFeeLamports": int(authorization["maxPostInitFeeLamports"]),
            "maxActivityFeeLamports": int(authorization["maxFeeLamports"]),
            "maxSpendLamports": int(authorization["maxSpendLamports"]),
            "guaranteedPreLifecycleResidualLamports": int(authorization["guaranteedPreLifecycleResidualLamports"]),
            "feeBasisPointsPerSide": authority["feeBasisPointsPerSide"],
            "feeDenominator": int(authority["feeDenominator"]),
            "operationCounts": observed_counts,
            "allOperationsMutationExpected": True,
        },
        "oldFlagshipFixture": {
            "path": "tools/devnet-scenarios/fixtures/flagship.json",
            "status": "refused-scenario-only",
            "reasons": [
                "Direct, redeem, and retire operations are nonmutating gaps",
                "four 50,000,000-lamport post-init transfers exceed its 150,000,000-lamport deployer bankroll before fees",
            ],
        },
        "artifactsRequired": [
            "new canonical scenario envelope with all twenty-five operations mutationExpected true and accepted caller schemas",
            "Activity-v3 manifest binding the real Market, checked release, exact caller completions, ten-wallet partition, and the semantic authority digest",
            "bounded v3 live authorization binding both distinct 10,000,000-lamport fee ceilings, exact initial closure, post-init plan digest, and at-most-six-hour window",
        ],
    }


def template() -> dict[str, Any]:
    refuse(
        sum(facts[1] for facts in DEPLOY1_FACTS.values())
        == DEPLOY1_PROGRAMDATA_LAMPORTS,
        "DEPLOY-1 ProgramData lamport pins do not close",
    )
    roles = []
    for ordinal, (role, disposition, program, programdata, slot) in enumerate(ROLES):
        live_bytes, programdata_lamports, live_sha256, programdata_sha256 = DEPLOY1_FACTS[role]
        roles.append({
            "ordinal": ordinal,
            "role": role,
            "disposition": disposition,
            "programId": program,
            "programDataId": programdata,
            "deploy1Slot": slot,
            "deploy1LiveElfBytes": live_bytes,
            "deploy1LiveElfSha256": live_sha256,
            "deploy1ProgramDataBytes": live_bytes + PROGRAMDATA_METADATA_BYTES,
            "deploy1ProgramDataLamports": programdata_lamports,
            "deploy1ProgramDataAccountSha256": programdata_sha256,
        })
    plan: dict[str, Any] = {
        "schema": TEMPLATE_SCHEMA,
        "stage": "template",
        "mutationPermitted": False,
        "cluster": {"kind": "devnet", "endpoint": DEVNET_ENDPOINT, "genesisHash": DEVNET_GENESIS, "commitment": "finalized"},
        "decision": {"id": "0012", "programIdentityPolicy": "retain-and-upgrade", "slotPolicy": "full-elf-at-activation;exact-programdata-slot-pin-at-use"},
        "retainedUpgradeAuthority": AUTHORITY,
        "roles": roles,
        "authorities": {"checkedReleaseGate": None, "permanentSubstrateCapture": None, "baselines": []},
        "upgradeOperations": [],
        "activationOrder": list(ACTIVATION_ROLES),
        "accounting": {
            "unit": "lamports",
            "deploy1ProgramDataLamports": DEPLOY1_PROGRAMDATA_LAMPORTS,
            "bufferDataBytesFormula": "rawElfBytes + 37",
            "targetProgramDataBytesFormula": "max(currentProgramDataBytes, max(rawElfBytes,currentLiveElfBytes) + 45)",
            "programDataRentAfterFormula": "preWriteProgramDataLamports + sum(extensionRentTopUpLamports)",
            "ordinaryUpgradeNetDebitFormula": "authenticatedBufferUploadFeesLamports + finalizedUpgradeFeeLamports",
            "peakTransientFormula": "max(each role: extensionRentTopUpLamports + extensionFee + bufferRentLamports + uploadFeeReserve + upgradeFeeReserve)",
            "classification": "ProgramData rent stays program rent; Buffer rent is transient and refunded; fees stay fees; hoard principal is never used or relabeled",
        },
        "recovery": [
            "one persistent Buffer identity and one process lease per role",
            "fsync each unsigned message and signed packet before send; send once with maxRetries=0",
            "Dispatching or Submitted recovery polls the exact signature before any resend",
            "a present signature is poll-only even after blockhash expiry",
            "only finalized height past lastValidBlockHeight plus history-null signature plus exact unchanged prestate permits a newly signed packet",
            "never recycle a Program, close a ProgramData account, change authority, infer success from account state alone, or use an unrelated payer transaction inside the exclusive window",
        ],
        "downstreamSequence": [
            "capture-registry-and-rent-carry-forward",
            "capture-five-updated-programdata-bodies",
            "prepare-checked-deployment-set",
            "publish-release-records",
            "initialize-infrastructure-and-release-profile",
            *[f"activate-{role}" for role in ACTIVATION_ROLES],
            "rerun-founding-frame-and-packet-census",
            "preflight-dcltgmf2-and-dcltpcb2",
            "create-found-and-open-devnet-market",
            "run-bounded-multiwallet-devnet-activity",
            "reconcile-finalized-activity-and-wallet-ledgers",
            "sync-site-market-manifest-and-trigger-manual-pages-workflow",
            "cold-browser-acceptance-against-finalized-devnet",
        ],
        "activityV3Plan": activity_v3_plan(),
        "missingAuthoritativeInputs": [
            "fresh checked release gate with five role ELF and provenance chains",
            "one fresh finalized permanent-substrate capture immediately before the first write",
            "five fresh key-free role baselines derived from those checked raw ELF widths",
            "five new public Buffer addresses and the exclusive fee-payer public address",
            "later receipts carrying exact extension, upload, and Upgrade transaction fees",
            "post-Upgrade captures, checked release records, Market address, activity closure, and site acceptance evidence",
        ],
        "stateSha256": "",
    }
    plan["stateSha256"] = state_digest(plan)
    return plan


def assemble(inputs_path: Path) -> dict[str, Any]:
    raw = read_canonical_file(inputs_path, "dryplan inputs")
    inputs = load_json_bytes(raw, "dryplan inputs")
    exact_keys(inputs, {"schema", "checkedReleaseGate", "permanentSubstrateCapture", "baselines", "feePayer", "buffers"}, "dryplan inputs")
    refuse(inputs["schema"] == INPUTS_SCHEMA, "dryplan input schema changed")
    fee_payer = inputs["feePayer"]
    base58_bytes(fee_payer, "fee payer")
    gate_ref = reference_from_input(inputs["checkedReleaseGate"], "checked release gate input")
    capture_ref = reference_from_input(inputs["permanentSubstrateCapture"], "permanent substrate input")
    gate = validate_gate(gate_ref)
    capture = validate_capture(capture_ref)
    refuse(capture.get("fee_payer") == fee_payer, "fee payer does not match the one-context capture")
    buffers = inputs["buffers"]
    refuse(isinstance(buffers, list) and len(buffers) == 5, "exactly five Buffer public identities are required")
    buffer_map: dict[str, str] = {}
    reserved_addresses = {AUTHORITY, fee_payer}
    for _, _, program, programdata, _ in ROLES:
        reserved_addresses.update((program, programdata))
    for row, role in zip(buffers, UPGRADE_ROLES):
        exact_keys(row, {"role", "publicKey"}, f"{role} Buffer")
        refuse(row["role"] == role, f"Buffer order must be {list(UPGRADE_ROLES)}")
        base58_bytes(row["publicKey"], f"{role} Buffer public key")
        refuse(row["publicKey"] not in buffer_map.values(), "Buffer public identities must be unique")
        refuse(row["publicKey"] not in reserved_addresses, f"{role} Buffer collides with a permanent or signer identity")
        buffer_map[role] = row["publicKey"]
    baseline_refs = inputs["baselines"]
    refuse(isinstance(baseline_refs, list) and len(baseline_refs) == 5, "exactly five baselines are required")
    capture_map = {row["role"]: row for row in capture["roles"]}
    operations = []
    authorities = []
    extension_total = 0
    for row, role in zip(baseline_refs, UPGRADE_ROLES):
        exact_keys(row, {"role", "canonicalPath", "sha256"}, f"{role} baseline input")
        refuse(row["role"] == role, f"baseline order must be {list(UPGRADE_ROLES)}")
        reference = {"canonicalPath": row["canonicalPath"], "sha256": row["sha256"]}
        baseline = validate_baseline(reference, role, gate["roles"][role]["elf"]["bytes"], capture_map[role], capture["context_slot"])
        extension_total += baseline["extension_lamport_top_up"]
        operation = {
            "ordinal": len(operations),
            "role": role,
            "programId": next(item[2] for item in ROLES if item[0] == role),
            "programDataId": next(item[3] for item in ROLES if item[0] == role),
            "preWriteSlot": baseline["observation"]["deployment_slot"],
            "checkedRawElf": gate["roles"][role]["elf"],
            "artifactProvenance": gate["roles"][role]["artifactProvenance"],
            "baseline": reference,
            "bufferPublicKey": buffer_map[role],
            "bufferDataBytes": gate["roles"][role]["elf"]["bytes"] + BUFFER_METADATA_BYTES,
            "currentProgramDataBytes": baseline["observation"]["programdata_data_bytes"],
            "targetProgramDataBytes": baseline["observation"]["programdata_data_bytes"] + baseline["extension_additional_bytes"],
            "extensionAdditionalBytes": baseline["extension_additional_bytes"],
            "extensionRentTopUpLamports": baseline["extension_lamport_top_up"],
            "orderedPhases": (["extend-programdata", "recapture-baseline"] if baseline["extension_additional_bytes"] else []) + ["preflight", "create-and-write-buffer", "loader-upgrade", "finalized-postcapture", "publish-live-dump-and-receipt"],
            "terminalPinsRequired": ["advancedDeploymentSlot", "retainedAuthority", "exactProgramToProgramDataLink", "exactPaddedLiveElfDigest", "unchangedProgramDataLamportsAfterAnyExtension", "exactFinalizedSignatureAndFee", "exactBufferRentRefund"],
        }
        operations.append(operation)
        authorities.append({"role": role, **reference})
    result = template()
    result["stage"] = "captured"
    result["authorities"] = {
        "checkedReleaseGate": gate_ref,
        "permanentSubstrateCapture": capture_ref,
        "baselines": authorities,
        "sourceRevision": gate["sourceRevision"],
        "sourceTreeSha256": gate["sourceTreeSha256"],
        "captureContextSlot": capture["context_slot"],
        "feePayer": fee_payer,
    }
    result["upgradeOperations"] = operations
    result["accounting"]["preWriteProgramDataLamports"] = capture["programdata_lamports_total"]
    result["accounting"]["extensionRentTopUpLamports"] = extension_total
    result["accounting"]["postExtensionProgramDataLamports"] = capture["programdata_lamports_total"] + extension_total
    result["missingAuthoritativeInputs"] = [
        "key-bearing execution remains forbidden by this document; use the existing per-role successor journals",
        "exact Buffer rent and transaction fees arrive only in authenticated role receipts",
        "post-Upgrade slots/digests and deployment-set audit",
        "CarryForward closure, release preparation/publication/activation receipts, founding/open receipts, activity closure, and site acceptance",
    ]
    result["stateSha256"] = state_digest(result)
    return result


def validate_plan(plan: dict[str, Any], required_stage: str | None = None) -> None:
    refuse(plan.get("schema") == TEMPLATE_SCHEMA, "dryplan schema changed")
    refuse(plan.get("stage") in {"template", "captured"}, "dryplan stage changed")
    if required_stage is not None:
        refuse(plan["stage"] == required_stage, f"dryplan stage is {plan['stage']}, required {required_stage}")
    refuse(plan.get("mutationPermitted") is False, "a dryplan may never permit mutation")
    refuse(plan.get("cluster") == {"kind": "devnet", "endpoint": DEVNET_ENDPOINT, "genesisHash": DEVNET_GENESIS, "commitment": "finalized"}, "cluster pin changed")
    refuse(plan.get("retainedUpgradeAuthority") == AUTHORITY, "retained authority changed")
    expected_template = template()
    expected_roles = expected_template["roles"]
    refuse(plan.get("roles") == expected_roles, "permanent role identities or dispositions changed")
    refuse(plan.get("activationOrder") == list(ACTIVATION_ROLES), "activation order changed")
    refuse(plan.get("decision") == expected_template["decision"], "decision-0012 policy changed")
    refuse(plan.get("recovery") == expected_template["recovery"], "crash/retry policy changed")
    refuse(plan.get("downstreamSequence") == expected_template["downstreamSequence"], "release/market/activity suffix changed")
    refuse(plan.get("activityV3Plan") == expected_template["activityV3Plan"], "Activity-v3 funding or lifecycle authority changed")
    for key, expected in expected_template["accounting"].items():
        refuse(plan.get("accounting", {}).get(key) == expected, f"accounting rule changed: {key}")
    digest = hex_digest(plan.get("stateSha256"), "dryplan state digest")
    refuse(state_digest(plan) == digest, "dryplan state digest changed")
    if plan["stage"] == "template":
        refuse(plan.get("upgradeOperations") == [], "template must not invent Upgrade operations")
    else:
        operations = plan.get("upgradeOperations")
        refuse(isinstance(operations, list) and [row.get("role") for row in operations] == list(UPGRADE_ROLES), "Upgrade operations are not canonical")
        for ordinal, row in enumerate(operations):
            refuse(row.get("ordinal") == ordinal, "Upgrade operation ordinal changed")
            refuse(row.get("bufferDataBytes") == row["checkedRawElf"]["bytes"] + BUFFER_METADATA_BYTES, f"{row.get('role')} Buffer width changed")
            refuse(row.get("targetProgramDataBytes") == row["currentProgramDataBytes"] + row["extensionAdditionalBytes"], f"{row.get('role')} ProgramData width changed")


def write_new(path: Path, value: dict[str, Any]) -> None:
    refuse(path.is_absolute(), "output path must be absolute")
    refuse(not path.exists(), "output already exists")
    path.parent.mkdir(parents=True, exist_ok=True)
    refuse(path.parent.resolve() == path.parent, "output parent must already be canonical")
    with path.open("x") as output:
        output.write(json.dumps(value, indent=2, sort_keys=True) + "\n")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    template_parser = sub.add_parser("template")
    template_parser.add_argument("--output", type=Path, required=True)
    assemble_parser = sub.add_parser("assemble")
    assemble_parser.add_argument("--inputs", type=Path, required=True)
    assemble_parser.add_argument("--output", type=Path, required=True)
    verify_parser = sub.add_parser("verify")
    verify_parser.add_argument("--plan", type=Path, required=True)
    verify_parser.add_argument("--require-stage", choices=("template", "captured"))
    args = parser.parse_args(argv)
    try:
        if args.command == "template":
            value = template()
            validate_plan(value, "template")
            write_new(args.output, value)
        elif args.command == "assemble":
            refuse(args.inputs.is_absolute(), "inputs path must be absolute")
            value = assemble(args.inputs)
            validate_plan(value, "captured")
            write_new(args.output, value)
        else:
            refuse(args.plan.is_absolute(), "plan path must be absolute")
            value = load_json_bytes(read_canonical_file(args.plan, "dryplan"), "dryplan")
            validate_plan(value, args.require_stage)
        return 0
    except (OSError, Refusal) as error:
        print(f"refusing: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
