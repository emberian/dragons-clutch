#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Offline gate for identity-linked capability profiles and ELF evidence.

This checker reads local JSON only. It never builds, deploys, signs, or contacts
RPC. V2 evidence is eligible only when it repeats the exact semantic-owner,
central-registry, and exhaustive wire-surface manifest whose canonical digest
defines the profile identity. Historical V1 measurements remain comparison-only
evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import sys
from typing import Any


MANIFEST_SCHEMA = "dragons-clutch/capability-profile-manifest/v2"
IDENTITY_DOMAIN = "dragons-clutch/capability-profile-identity/v2"
HISTORICAL_MEASUREMENT_SCHEMA = "dragons-clutch/capability-profile-measurement/v1"
LINKED_MEASUREMENT_SCHEMA = "dragons-clutch/capability-profile-measurement/v2"
WIRE_SURFACE_SCHEMA = "dragons-clutch/wire-surface/v1"
WIRE_SURFACE_IDENTITY_DOMAIN = "dragons-clutch/wire-surface-identity/v1"
LOADER_V3_MAX_PERMITTED_DATA_LENGTH = 10 * 1024 * 1024
PROGRAMDATA_METADATA_DATA_LEN_BYTES = 45

OUTER_REQUEST_ACTIONS = [0, 1, 2]
DIRECT_V3_TAGS = frozenset(range(36, 47))
SOURCE_V1_TAGS = frozenset(range(23, 27))
SOURCE_V2_TAGS = frozenset(range(70, 74))
CURRENT_SOURCE_EXTENSION_TRIPLES = [[77, 2, action] for action in range(1, 5)]
SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE = "profile-successor-chain-attached-v1"
# This is the complete local-action-zero wire surface of the first
# chain-attached successor.  It intentionally excludes legacy market founding,
# retired Source generations, old Direct generations, and every General value
# or clearing action.  Current Direct V4 owns shared tags 7/14 and its
# dedicated 36..=46 decoder; current Collateral owns 2..=5 and 15..=17.
SUCCESSOR_CHAIN_ATTACHED_LEGACY_INTENT_PAIRS = [
    [tag, 3]
    for tag in [2, 3, 4, 5, 7, 10, 11, 14, 15, 16, 17, 18, 19, 20, 21, 68]
]
SUCCESSOR_CHAIN_ATTACHED_DIRECT_INTENT_PAIRS = [
    [tag, 3] for tag in range(36, 47)
]

CAPABILITY_OWNERS: tuple[tuple[str, str], ...] = (
    ("relation", "dragons-clutch/semantic-owner/relation"),
    ("score", "dragons-clutch/semantic-owner/score"),
    ("price-measure", "dragons-clutch/semantic-owner/price-measure"),
    ("candidate-lifecycle", "dragons-clutch/semantic-owner/candidate-lifecycle"),
    ("clear-work-feed", "dragons-clutch/semantic-owner/clear-work-feed"),
    ("retirement", "dragons-clutch/semantic-owner/retirement"),
    ("source-plane", "dragons-clutch/semantic-owner/source-plane"),
    ("series-products", "dragons-clutch/semantic-owner/series-products"),
    ("recovery", "dragons-clutch/semantic-owner/recovery"),
    ("structured-claim", "dragons-clutch/semantic-owner/structured-claim"),
    ("liquidity-dealer", "dragons-clutch/semantic-owner/liquidity-dealer"),
)
CAPABILITY_SLOTS = tuple(slot for slot, _owner in CAPABILITY_OWNERS)
KNOWN_OWNERS = frozenset(owner for _slot, owner in CAPABILITY_OWNERS)
EXPECTED_OWNER = dict(CAPABILITY_OWNERS)

PROFILE_FEATURES = frozenset(
    {
        "profile-full",
        "profile-direct-v3-source-v2-point",
        "profile-general-source-v2-point",
        SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE,
    }
)
SOURCE_IDENTITY_FEATURE: dict[str, str | None] = {
    "production-inert": None,
    "non-production-mock-source-lab": "non-production-mock-source",
    "non-production-real-pyth-lab": "non-production-real-pyth-lab",
}

HEX_32 = re.compile(r"[0-9a-f]{64}\Z")
GIT_OBJECT_ID = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})\Z")
SEMANTIC_VERSION = re.compile(r"[a-z0-9][a-z0-9._/+:-]{0,127}\Z")
PROFILE_NAME = re.compile(r"[a-z0-9][a-z0-9-]{0,63}\Z")
PROFILE_LABEL = re.compile(
    r"dragons-clutch/capability-profile/[a-z0-9][a-z0-9._/-]*/v[1-9][0-9]*\Z"
)
SYSCALL_NAME = re.compile(r"(?:abort|sol_[a-z0-9_]+)\Z")

# Every first-party Python body executed by the linked schema-V2 producer must
# be both inside its clean tracked closure and named in the evidence. Changing
# either body therefore invalidates source_clean before any build begins.
LINKED_MEASUREMENT_CODE_INPUTS: tuple[tuple[str, str], ...] = (
    ("checker", "programs/clutch-sbf/scripts/check_capability_profile.py"),
    ("producer", "programs/clutch-sbf/scripts/measure_capability_profiles.py"),
)


class ProfileError(ValueError):
    """A deterministic, fail-closed profile refusal."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ProfileError(message)


def exact_keys(value: dict[str, Any], expected: set[str], where: str) -> None:
    actual = set(value)
    require(actual == expected, f"{where}: keys {sorted(actual)} != {sorted(expected)}")


def require_string(value: Any, where: str) -> str:
    require(isinstance(value, str), f"{where}: expected string")
    return value


def require_hex32(value: Any, where: str, *, nonzero: bool = True) -> str:
    text = require_string(value, where)
    require(HEX_32.fullmatch(text) is not None, f"{where}: malformed lowercase sha256")
    if nonzero:
        require(text != "0" * 64, f"{where}: zero digest is not an identity")
    return text


def require_git_object_id(value: Any, where: str) -> str:
    text = require_string(value, where)
    require(
        GIT_OBJECT_ID.fullmatch(text) is not None,
        f"{where}: malformed lowercase Git object identity",
    )
    require(set(text) != {"0"}, f"{where}: zero Git object identity")
    return text


def require_positive_int(value: Any, where: str) -> int:
    require(type(value) is int and value > 0, f"{where}: expected positive integer")
    return value


def require_nonnegative_int(value: Any, where: str) -> int:
    require(type(value) is int and value >= 0, f"{where}: expected nonnegative integer")
    return value


def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise ProfileError(f"json: duplicate object key {key!r}")
        value[key] = item
    return value


def load_json(path: Path) -> Any:
    try:
        with path.open("r", encoding="utf-8") as source:
            return json.load(source, object_pairs_hook=reject_duplicate_keys)
    except (OSError, json.JSONDecodeError) as exc:
        raise ProfileError(f"{path}: cannot read canonical JSON: {exc}") from exc


def canonical_json_sha256(value: Any) -> str:
    encoded = json.dumps(
        value, ensure_ascii=True, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


IntentTriple = tuple[int, int, int]
AccountCoordinate = tuple[int, int]
IntentPair = tuple[int, int]


def validate_intent_triples(value: Any, where: str) -> list[list[int]]:
    """Validate canonical `(outer tag, version, local action)` coordinates.

    Legacy two-byte intents use local action zero. Successor envelopes use a
    nonzero family-local action. Allocation validity is owned by the pinned
    central-registry digest, not duplicated as Python tag ranges here.
    """
    require(isinstance(value, list), f"{where}: expected array")
    parsed: list[IntentTriple] = []
    for index, item in enumerate(value):
        item_where = f"{where}[{index}]"
        require(
            isinstance(item, list) and len(item) == 3, f"{item_where}: expected triple"
        )
        fields = tuple(
            require_nonnegative_int(field, f"{item_where}[{field_index}]")
            for field_index, field in enumerate(item)
        )
        require(
            all(field <= 255 for field in fields),
            f"{item_where}: coordinate exceeds one byte",
        )
        require(fields[0] > 0 and fields[1] > 0, f"{item_where}: zero tag/version")
        parsed.append(fields)  # type: ignore[arg-type]
    require(len(set(parsed)) == len(parsed), f"{where}: duplicate intent triple")
    require(parsed == sorted(parsed), f"{where}: noncanonical intent-triple order")
    return [list(item) for item in parsed]


def validate_account_coordinates(value: Any, where: str) -> list[list[int]]:
    require(isinstance(value, list), f"{where}: expected array")
    parsed: list[AccountCoordinate] = []
    for index, item in enumerate(value):
        item_where = f"{where}[{index}]"
        require(
            isinstance(item, list) and len(item) == 2, f"{item_where}: expected pair"
        )
        fields = tuple(
            require_positive_int(field, f"{item_where}[{field_index}]")
            for field_index, field in enumerate(item)
        )
        require(
            all(field <= 255 for field in fields),
            f"{item_where}: coordinate exceeds one byte",
        )
        parsed.append(fields)  # type: ignore[arg-type]
    require(len(set(parsed)) == len(parsed), f"{where}: duplicate account coordinate")
    require(parsed == sorted(parsed), f"{where}: noncanonical account-coordinate order")
    return [list(item) for item in parsed]


def validate_intent_pairs(value: Any, where: str) -> list[list[int]]:
    require(isinstance(value, list), f"{where}: expected array")
    parsed: list[IntentPair] = []
    for index, item in enumerate(value):
        item_where = f"{where}[{index}]"
        require(
            isinstance(item, list) and len(item) == 2,
            f"{item_where}: expected pair",
        )
        pair = tuple(
            require_positive_int(field, f"{item_where}[{field_index}]")
            for field_index, field in enumerate(item)
        )
        require(
            all(field <= 255 for field in pair),
            f"{item_where}: coordinate exceeds one byte",
        )
        parsed.append(pair)  # type: ignore[arg-type]
    require(len(set(parsed)) == len(parsed), f"{where}: duplicate intent pair")
    require(parsed == sorted(parsed), f"{where}: noncanonical intent-pair order")
    return [list(item) for item in parsed]


def validate_byte_discriminants(value: Any, where: str) -> list[int]:
    require(isinstance(value, list), f"{where}: expected array")
    parsed = [require_nonnegative_int(item, f"{where}[{index}]") for index, item in enumerate(value)]
    require(all(item <= 255 for item in parsed), f"{where}: value exceeds one byte")
    require(parsed == sorted(set(parsed)), f"{where}: noncanonical byte order")
    return parsed


def wire_surface_sha256(value: dict[str, Any]) -> str:
    return canonical_json_sha256(
        {"domain": WIRE_SURFACE_IDENTITY_DOMAIN, "wire_surface": value}
    )


def validate_wire_surface(
    value: Any,
    *,
    build_contract: dict[str, Any],
    capabilities: list[dict[str, Any]],
    central_registry: dict[str, Any],
) -> dict[str, Any]:
    require(isinstance(value, dict), "wire_surface: expected object")
    exact_keys(
        value,
        {
            "schema",
            "legacy_intent_pairs",
            "dedicated_direct_intent_pairs",
            "outer_request_actions",
            "source_generation_discriminants",
        },
        "wire_surface",
    )
    require(
        value["schema"] == WIRE_SURFACE_SCHEMA,
        "wire_surface.schema: unsupported schema",
    )
    legacy = validate_intent_pairs(
        value["legacy_intent_pairs"], "wire_surface.legacy_intent_pairs"
    )
    direct = validate_intent_pairs(
        value["dedicated_direct_intent_pairs"],
        "wire_surface.dedicated_direct_intent_pairs",
    )
    outer = validate_byte_discriminants(
        value["outer_request_actions"], "wire_surface.outer_request_actions"
    )
    generations = validate_byte_discriminants(
        value["source_generation_discriminants"],
        "wire_surface.source_generation_discriminants",
    )
    require(
        outer == OUTER_REQUEST_ACTIONS,
        f"wire_surface.outer_request_actions: expected {OUTER_REQUEST_ACTIONS}",
    )

    legacy_set = {tuple(pair) for pair in legacy}
    direct_set = {tuple(pair) for pair in direct}
    require(
        not legacy_set.intersection(direct_set),
        "wire_surface: legacy and dedicated Direct intent pairs overlap",
    )
    require(
        all(pair[0] not in DIRECT_V3_TAGS for pair in legacy_set),
        "wire_surface.legacy_intent_pairs: dedicated Direct V3 tag misclassified",
    )
    require(
        all(pair[0] in DIRECT_V3_TAGS for pair in direct_set),
        "wire_surface.dedicated_direct_intent_pairs: non-Direct tag",
    )
    enabled_pairs = {
        (tag, version)
        for tag, version, local_action in central_registry["enabled_intent_triples"]
        if local_action == 0
    }
    require(
        legacy_set.union(direct_set) == enabled_pairs,
        "wire_surface: intent pairs do not exactly match central-registry legacy/dedicated coverage",
    )

    source_tags = {tag for tag, _version in legacy_set if tag in SOURCE_V1_TAGS | SOURCE_V2_TAGS}
    expected_generations: list[int] = []
    if source_tags.intersection(SOURCE_V1_TAGS):
        expected_generations.append(1)
    if source_tags.intersection(SOURCE_V2_TAGS):
        expected_generations.append(2)
    require(
        generations == expected_generations,
        "wire_surface.source_generation_discriminants: legacy Source generation mismatch",
    )

    source_identity = build_contract["source_identity"]
    if source_identity in {"production-inert", "runtime-real-pyth-release"}:
        require(
            not source_tags and generations == [],
            "wire_surface: release-class profile retains legacy Source authority",
        )

    source_owner = next(row for row in capabilities if row["slot"] == "source-plane")
    required_source_extensions = [
        triple
        for triple in source_owner["required_intent_triples"]
        if triple[0] == 77 and triple[1] == 2 and triple[2] != 0
    ]
    all_required_source_extensions = [
        triple
        for row in capabilities
        for triple in row["required_intent_triples"]
        if triple[0] == 77 and triple[1] == 2 and triple[2] != 0
    ]
    require(
        all_required_source_extensions == required_source_extensions,
        "wire_surface: Source V3 actions have a non-Source semantic owner",
    )
    enabled_source_extensions = [
        triple
        for triple in central_registry["enabled_intent_triples"]
        if triple[0] == 77 and triple[1] == 2 and triple[2] != 0
    ]
    profile_feature = build_contract["cargo_profile_feature"]
    if profile_feature in {"profile-full", SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE}:
        require(
            required_source_extensions == CURRENT_SOURCE_EXTENSION_TRIPLES,
            "wire_surface: current Source requirements must be exactly 77/v2 actions 1 through 4",
        )
        if source_owner["linkage"] == "linked":
            require(
                enabled_source_extensions == CURRENT_SOURCE_EXTENSION_TRIPLES,
                "wire_surface: linked current Source must enable exactly 77/v2 actions 1 through 4",
            )
        else:
            require(
                enabled_source_extensions == [],
                "wire_surface: planned Source owner unexpectedly enables Source V3 actions",
            )
    else:
        require(
            required_source_extensions == [] and enabled_source_extensions == [],
            "wire_surface: narrow profile unexpectedly retains Source V3 actions",
        )

    if profile_feature == SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE:
        require(
            legacy == SUCCESSOR_CHAIN_ATTACHED_LEGACY_INTENT_PAIRS,
            "wire_surface: chain-attached successor legacy intent set is not exact",
        )
        require(
            direct == SUCCESSOR_CHAIN_ATTACHED_DIRECT_INTENT_PAIRS,
            "wire_surface: chain-attached successor Direct intent set is not exact",
        )
        require(
            generations == [],
            "wire_surface: chain-attached successor retains a legacy Source generation",
        )

    return {
        "schema": WIRE_SURFACE_SCHEMA,
        "legacy_intent_pairs": legacy,
        "dedicated_direct_intent_pairs": direct,
        "outer_request_actions": outer,
        "source_generation_discriminants": generations,
    }


def validate_capabilities(value: Any) -> list[dict[str, Any]]:
    require(isinstance(value, list), "capabilities: expected array")
    rows: list[dict[str, Any]] = []
    seen_slots: set[str] = set()
    seen_owners: set[str] = set()
    for index, row in enumerate(value):
        where = f"capabilities[{index}]"
        require(isinstance(row, dict), f"{where}: expected object")
        exact_keys(
            row,
            {
                "slot",
                "owner",
                "linkage",
                "semantic_version",
                "semantic_digest_sha256",
                "required_intent_triples",
                "required_account_coordinates",
            },
            where,
        )
        slot = require_string(row["slot"], f"{where}.slot")
        owner = require_string(row["owner"], f"{where}.owner")
        linkage = require_string(row["linkage"], f"{where}.linkage")
        version = require_string(row["semantic_version"], f"{where}.semantic_version")
        digest = require_hex32(
            row["semantic_digest_sha256"], f"{where}.semantic_digest_sha256"
        )
        require(
            slot in CAPABILITY_SLOTS, f"{where}.slot: unknown capability slot {slot!r}"
        )
        require(
            slot not in seen_slots, f"{where}.slot: duplicate capability slot {slot!r}"
        )
        require(
            owner in KNOWN_OWNERS, f"{where}.owner: unknown capability owner {owner!r}"
        )
        require(
            owner not in seen_owners,
            f"{where}.owner: duplicate capability owner {owner!r}",
        )
        require(
            owner == EXPECTED_OWNER[slot],
            f"{where}.owner: wrong owner for slot {slot!r}",
        )
        require(linkage in {"linked", "planned"}, f"{where}.linkage: unknown state")
        require(
            SEMANTIC_VERSION.fullmatch(version) is not None,
            f"{where}.semantic_version: malformed semantic version",
        )
        intents = validate_intent_triples(
            row["required_intent_triples"], f"{where}.required_intent_triples"
        )
        accounts = validate_account_coordinates(
            row["required_account_coordinates"], f"{where}.required_account_coordinates"
        )
        seen_slots.add(slot)
        seen_owners.add(owner)
        rows.append(
            {
                "slot": slot,
                "owner": owner,
                "linkage": linkage,
                "semantic_version": version,
                "semantic_digest_sha256": digest,
                "required_intent_triples": intents,
                "required_account_coordinates": accounts,
            }
        )

    missing_slots = sorted(set(CAPABILITY_SLOTS) - seen_slots)
    missing_owners = sorted(KNOWN_OWNERS - seen_owners)
    require(
        not missing_slots and not missing_owners,
        f"capabilities: missing capability slots {missing_slots}; missing capability owners {missing_owners}",
    )
    observed_order = tuple(row["slot"] for row in rows)
    require(
        observed_order == CAPABILITY_SLOTS,
        f"capabilities: noncanonical order {observed_order!r}; expected {CAPABILITY_SLOTS!r}",
    )
    return rows


def validate_registry(value: Any, capabilities: list[dict[str, Any]]) -> dict[str, Any]:
    require(isinstance(value, dict), "central_registry: expected object")
    exact_keys(
        value,
        {
            "semantic_version",
            "semantic_digest_sha256",
            "enabled_intent_triples",
            "linked_account_coordinates",
        },
        "central_registry",
    )
    version = require_string(
        value["semantic_version"], "central_registry.semantic_version"
    )
    require(
        SEMANTIC_VERSION.fullmatch(version) is not None,
        "central_registry.semantic_version: malformed semantic version",
    )
    digest = require_hex32(
        value["semantic_digest_sha256"], "central_registry.semantic_digest_sha256"
    )
    enabled = validate_intent_triples(
        value["enabled_intent_triples"], "central_registry.enabled_intent_triples"
    )
    accounts = validate_account_coordinates(
        value["linked_account_coordinates"],
        "central_registry.linked_account_coordinates",
    )

    enabled_set = {tuple(item) for item in enabled}
    account_set = {tuple(item) for item in accounts}
    required_intents: set[IntentTriple] = set()
    required_accounts: set[AccountCoordinate] = set()
    for row in capabilities:
        if row["linkage"] != "linked":
            continue
        required_intents.update(tuple(item) for item in row["required_intent_triples"])
        required_accounts.update(
            tuple(item) for item in row["required_account_coordinates"]
        )
    missing_intents = sorted(required_intents - enabled_set)
    missing_accounts = sorted(required_accounts - account_set)
    unowned_intents = sorted(enabled_set - required_intents)
    unowned_accounts = sorted(account_set - required_accounts)
    require(
        not missing_intents,
        f"central_registry: missing linked intent triples {missing_intents}",
    )
    require(
        not missing_accounts,
        f"central_registry: missing linked account coordinates {missing_accounts}",
    )
    require(
        not unowned_intents,
        f"central_registry: enabled intent triples lack a linked semantic owner {unowned_intents}",
    )
    require(
        not unowned_accounts,
        f"central_registry: linked account coordinates lack a linked semantic owner {unowned_accounts}",
    )
    return {
        "semantic_version": version,
        "semantic_digest_sha256": digest,
        "enabled_intent_triples": enabled,
        "linked_account_coordinates": accounts,
    }


def validate_build_contract(value: Any) -> dict[str, Any]:
    require(isinstance(value, dict), "build_contract: expected object")
    exact_keys(
        value,
        {
            "cargo_profile_feature",
            "source_identity",
            "expected_undefined_dynamic_symbols",
        },
        "build_contract",
    )
    feature = require_string(
        value["cargo_profile_feature"], "build_contract.cargo_profile_feature"
    )
    require(
        feature in PROFILE_FEATURES,
        "build_contract.cargo_profile_feature: unknown profile",
    )
    source_identity = require_string(
        value["source_identity"], "build_contract.source_identity"
    )
    require(
        source_identity in SOURCE_IDENTITY_FEATURE,
        "build_contract.source_identity: unknown class",
    )
    symbols = value["expected_undefined_dynamic_symbols"]
    require(
        isinstance(symbols, list),
        "build_contract.expected_undefined_dynamic_symbols: expected array",
    )
    parsed_symbols: list[str] = []
    for index, symbol in enumerate(symbols):
        text = require_string(
            symbol, f"build_contract.expected_undefined_dynamic_symbols[{index}]"
        )
        require(
            SYSCALL_NAME.fullmatch(text) is not None,
            f"build_contract: malformed syscall {text!r}",
        )
        parsed_symbols.append(text)
    require(
        len(set(parsed_symbols)) == len(parsed_symbols),
        "build_contract: duplicate syscall",
    )
    require(
        parsed_symbols == sorted(parsed_symbols),
        "build_contract: noncanonical syscall order",
    )
    return {
        "cargo_profile_feature": feature,
        "source_identity": source_identity,
        "expected_undefined_dynamic_symbols": parsed_symbols,
    }


def cargo_features(build_contract: dict[str, Any]) -> list[str]:
    profile_feature = str(build_contract["cargo_profile_feature"])
    features = ["custom-heap"]
    # Cargo's default route enables the named ``default`` feature in addition
    # to every feature it expands to.  That marker has no cfg-gated behavior in
    # the program, but rustc still includes the complete feature set in crate
    # identity.  Preserve the marker for the full profile so the explicit and
    # Cargo-default routes compile the same crate identity and therefore the
    # same deployable bytes.  Narrow profiles must keep defaults disabled.
    if profile_feature == "profile-full":
        features.append("default")
    features.append(profile_feature)
    source_feature = SOURCE_IDENTITY_FEATURE[str(build_contract["source_identity"])]
    if source_feature is not None:
        features.append(source_feature)
    return features


def validate_limits(value: Any) -> dict[str, int]:
    require(isinstance(value, dict), "artifact_budget.limits: expected object")
    exact_keys(
        value,
        {
            "max_elf_bytes",
            "max_text_bytes",
            "programdata_max_len",
            "max_persistent_loader_rent_lamports",
        },
        "artifact_budget.limits",
    )
    limits = {
        "max_elf_bytes": require_positive_int(
            value["max_elf_bytes"], "artifact_budget.limits.max_elf_bytes"
        ),
        "max_text_bytes": require_positive_int(
            value["max_text_bytes"], "artifact_budget.limits.max_text_bytes"
        ),
        "programdata_max_len": require_positive_int(
            value["programdata_max_len"], "artifact_budget.limits.programdata_max_len"
        ),
        "max_persistent_loader_rent_lamports": require_positive_int(
            value["max_persistent_loader_rent_lamports"],
            "artifact_budget.limits.max_persistent_loader_rent_lamports",
        ),
    }
    require(
        limits["max_text_bytes"] <= limits["max_elf_bytes"],
        "artifact_budget.limits: text ceiling exceeds ELF ceiling",
    )
    require(
        limits["max_elf_bytes"] <= limits["programdata_max_len"],
        "artifact_budget.limits: ELF ceiling exceeds chosen ProgramData max_len",
    )
    require(
        limits["programdata_max_len"] + PROGRAMDATA_METADATA_DATA_LEN_BYTES
        <= LOADER_V3_MAX_PERMITTED_DATA_LENGTH,
        "artifact_budget.limits: chosen ProgramData max_len exceeds loader-v3 data limit",
    )
    return limits


def profile_identity(
    name: str,
    label: str,
    build_contract: dict[str, Any],
    capabilities: list[dict[str, Any]],
    central_registry: dict[str, Any],
    wire_surface: dict[str, Any],
    limits: dict[str, int],
) -> str:
    return canonical_json_sha256(
        {
            "domain": IDENTITY_DOMAIN,
            "name": name,
            "label": label,
            "build_contract": build_contract,
            "capabilities": capabilities,
            "central_registry": central_registry,
            "wire_surface": wire_surface,
            "artifact_budget_limits": limits,
        }
    )


def measurement_input_manifest_sha256(data: dict[str, Any]) -> str:
    """Hash the canonical planning form consumed by the measurement producer.

    A post-measurement manifest changes only its classification and evidence
    pointer. Normalizing those fields lets the checker authenticate the exact
    producer input without making measurement evidence circular.
    """
    return canonical_json_sha256(
        {
            **data,
            "profile": {**data["profile"], "classification": "planning"},
            "artifact_budget": {
                **data["artifact_budget"],
                "measurement_class": "planned",
                "evidence_path": None,
                "evidence_profile_name": None,
            },
        }
    )


def resolve_repository_path(repo: Path, value: Any, where: str) -> Path:
    text = require_string(value, where)
    relative = PurePosixPath(text)
    require(
        text != "" and not relative.is_absolute(),
        f"{where}: expected repository-relative path",
    )
    require(
        ".." not in relative.parts and "." not in relative.parts,
        f"{where}: noncanonical path",
    )
    require(str(relative) == text, f"{where}: noncanonical path")
    root = repo.resolve()
    resolved = (root / Path(*relative.parts)).resolve()
    try:
        resolved.relative_to(root)
    except ValueError as exc:
        raise ProfileError(f"{where}: path escapes repository") from exc
    require(resolved.is_file(), f"{where}: evidence file does not exist")
    return resolved


def validate_loader_model(value: Any) -> dict[str, int | str]:
    require(isinstance(value, dict), "measurement evidence.rent_model: expected object")
    exact_keys(
        value,
        {
            "model",
            "rent_exempt_lamports_per_billable_byte",
            "account_storage_overhead_bytes",
            "program_data_len_bytes",
            "programdata_metadata_data_len_bytes",
            "buffer_metadata_data_len_bytes",
        },
        "measurement evidence.rent_model",
    )
    require(
        value["model"] == "upgradeable-loader-v3",
        "measurement evidence.rent_model: unsupported loader model",
    )
    parsed: dict[str, int | str] = {"model": value["model"]}
    for key in (
        "rent_exempt_lamports_per_billable_byte",
        "account_storage_overhead_bytes",
        "program_data_len_bytes",
        "programdata_metadata_data_len_bytes",
        "buffer_metadata_data_len_bytes",
    ):
        parsed[key] = require_positive_int(
            value[key], f"measurement evidence.rent_model.{key}"
        )
    require(
        parsed["account_storage_overhead_bytes"] == 128,
        "measurement evidence.rent_model: account overhead must be 128 bytes",
    )
    require(
        parsed["program_data_len_bytes"] == 36,
        "measurement evidence.rent_model: Program data length must be 36",
    )
    require(
        parsed["programdata_metadata_data_len_bytes"] == 45,
        "measurement evidence.rent_model: ProgramData metadata length must be 45",
    )
    require(
        parsed["buffer_metadata_data_len_bytes"] == 37,
        "measurement evidence.rent_model: Buffer metadata length must be 37",
    )
    return parsed


def validate_frame_audit(value: Any, where: str) -> dict[str, Any]:
    require(isinstance(value, dict), f"{where}: expected object")
    exact_keys(
        value,
        {
            "final_text_function_symbols",
            "final_text_function_addresses",
            "disassembled_function_regions",
            "direct_r10_references",
            "deepest_direct_r10_offset",
            "deepest_direct_r10_function",
            "direct_frame_limit_bytes",
            "direct_frame_bounds",
        },
        where,
    )
    parsed: dict[str, Any] = {}
    for key in (
        "final_text_function_symbols",
        "final_text_function_addresses",
        "disassembled_function_regions",
    ):
        parsed[key] = require_positive_int(value[key], f"{where}.{key}")
    for key in ("direct_r10_references", "deepest_direct_r10_offset"):
        parsed[key] = require_nonnegative_int(value[key], f"{where}.{key}")
    deepest = value["deepest_direct_r10_function"]
    require(
        deepest is None or isinstance(deepest, str),
        f"{where}.deepest_direct_r10_function: expected string or null",
    )
    parsed["deepest_direct_r10_function"] = deepest
    parsed["direct_frame_limit_bytes"] = require_positive_int(
        value["direct_frame_limit_bytes"], f"{where}.direct_frame_limit_bytes"
    )
    require(
        parsed["direct_frame_limit_bytes"] == 4096,
        f"{where}: unexpected direct-frame limit",
    )
    require(
        value["direct_frame_bounds"] == "PASS",
        f"{where}: final frame audit did not pass",
    )
    parsed["direct_frame_bounds"] = "PASS"
    require(
        parsed["deepest_direct_r10_offset"] <= parsed["direct_frame_limit_bytes"],
        f"{where}: direct frame exceeds limit",
    )
    if parsed["direct_r10_references"] == 0:
        require(
            deepest is None and parsed["deepest_direct_r10_offset"] == 0,
            f"{where}: inconsistent empty frame audit",
        )
    else:
        require(
            isinstance(deepest, str) and deepest != "",
            f"{where}: deepest frame function absent",
        )
    return parsed


def validate_loader_measurement(
    value: Any,
    *,
    where: str,
    elf_bytes: int,
    expected_max_len: int,
    rent_model: dict[str, int | str],
) -> dict[str, Any]:
    require(isinstance(value, dict), f"{where}: expected object")
    exact_keys(
        value,
        {
            "current_elf_len_bytes",
            "chosen_programdata_max_len",
            "exact_size_allocation",
            "program",
            "programdata",
            "buffer",
            "persistent_program_plus_programdata_rent_lamports",
            "transient_buffer_rent_lamports",
        },
        where,
    )
    current = require_positive_int(
        value["current_elf_len_bytes"], f"{where}.current_elf_len_bytes"
    )
    chosen = require_positive_int(
        value["chosen_programdata_max_len"], f"{where}.chosen_programdata_max_len"
    )
    require(current == elf_bytes, f"{where}: current ELF length mismatch")
    require(chosen == expected_max_len, f"{where}: chosen ProgramData max_len mismatch")
    require(
        current <= chosen, f"{where}: current ELF exceeds chosen ProgramData max_len"
    )
    require(
        value["exact_size_allocation"] == (current == chosen),
        f"{where}: exact-size flag mismatch",
    )

    overhead = int(rent_model["account_storage_overhead_bytes"])
    rate = int(rent_model["rent_exempt_lamports_per_billable_byte"])
    expected_data_lengths = {
        "program": int(rent_model["program_data_len_bytes"]),
        "programdata": int(rent_model["programdata_metadata_data_len_bytes"]) + chosen,
        "buffer": int(rent_model["buffer_metadata_data_len_bytes"]) + chosen,
    }
    parsed_accounts: dict[str, dict[str, int | str]] = {}
    for role in ("program", "programdata", "buffer"):
        account = value[role]
        account_where = f"{where}.{role}"
        require(isinstance(account, dict), f"{account_where}: expected object")
        exact_keys(
            account,
            {
                "lifetime",
                "data_len_bytes",
                "storage_overhead_bytes",
                "billable_bytes",
                "rent_exempt_lamports",
            },
            account_where,
        )
        expected_lifetime = "persistent" if role != "buffer" else "transient-recyclable"
        require(
            account["lifetime"] == expected_lifetime, f"{account_where}: wrong lifetime"
        )
        data_len = require_positive_int(
            account["data_len_bytes"], f"{account_where}.data_len_bytes"
        )
        stored_overhead = require_positive_int(
            account["storage_overhead_bytes"], f"{account_where}.storage_overhead_bytes"
        )
        billable = require_positive_int(
            account["billable_bytes"], f"{account_where}.billable_bytes"
        )
        rent = require_positive_int(
            account["rent_exempt_lamports"], f"{account_where}.rent_exempt_lamports"
        )
        require(
            data_len == expected_data_lengths[role],
            f"{account_where}: data length mismatch",
        )
        require(
            stored_overhead == overhead, f"{account_where}: storage overhead mismatch"
        )
        require(
            billable == data_len + overhead, f"{account_where}: billable bytes mismatch"
        )
        require(rent == billable * rate, f"{account_where}: rent arithmetic mismatch")
        parsed_accounts[role] = {
            "lifetime": expected_lifetime,
            "data_len_bytes": data_len,
            "storage_overhead_bytes": stored_overhead,
            "billable_bytes": billable,
            "rent_exempt_lamports": rent,
        }
    persistent = require_positive_int(
        value["persistent_program_plus_programdata_rent_lamports"],
        f"{where}.persistent_program_plus_programdata_rent_lamports",
    )
    transient = require_positive_int(
        value["transient_buffer_rent_lamports"],
        f"{where}.transient_buffer_rent_lamports",
    )
    require(
        persistent
        == int(parsed_accounts["program"]["rent_exempt_lamports"])
        + int(parsed_accounts["programdata"]["rent_exempt_lamports"]),
        f"{where}: persistent loader rent mismatch",
    )
    require(
        transient == int(parsed_accounts["buffer"]["rent_exempt_lamports"]),
        f"{where}: transient Buffer rent mismatch",
    )
    return {
        "current_elf_len_bytes": current,
        "chosen_programdata_max_len": chosen,
        "exact_size_allocation": current == chosen,
        **parsed_accounts,
        "persistent_program_plus_programdata_rent_lamports": persistent,
        "transient_buffer_rent_lamports": transient,
    }


def validate_v2_run(
    value: Any,
    *,
    where: str,
    expected_run: int,
    expected_mode: str,
    expected_syscalls: list[str],
    expected_max_len: int,
    rent_model: dict[str, int | str],
) -> dict[str, Any]:
    require(isinstance(value, dict), f"{where}: expected object")
    exact_keys(
        value,
        {
            "run",
            "build_mode",
            "elf_sha256",
            "elf_bytes",
            "text_bytes",
            "rodata_bytes",
            "undefined_dynamic_symbols",
            "backend_stack_diagnostic_lines",
            "backend_stack_diagnostic_symbols",
            "backend_stack_diagnostic_survivors",
            "final_frame_audit",
            "loader",
        },
        where,
    )
    run = require_positive_int(value["run"], f"{where}.run")
    require(run == expected_run, f"{where}: unexpected run number")
    require(value["build_mode"] == expected_mode, f"{where}: unexpected build mode")
    elf_hash = require_hex32(value["elf_sha256"], f"{where}.elf_sha256")
    elf_bytes = require_positive_int(value["elf_bytes"], f"{where}.elf_bytes")
    text_bytes = require_positive_int(value["text_bytes"], f"{where}.text_bytes")
    rodata_bytes = require_positive_int(value["rodata_bytes"], f"{where}.rodata_bytes")
    require(text_bytes <= elf_bytes, f"{where}: executable text exceeds ELF size")
    require(rodata_bytes <= elf_bytes, f"{where}: rodata exceeds ELF size")
    symbols = value["undefined_dynamic_symbols"]
    require(
        isinstance(symbols, list), f"{where}.undefined_dynamic_symbols: expected array"
    )
    parsed_symbols = [
        require_string(item, f"{where}.undefined_dynamic_symbols") for item in symbols
    ]
    require(
        parsed_symbols == sorted(set(parsed_symbols)),
        f"{where}: noncanonical syscall surface",
    )
    require(
        parsed_symbols == expected_syscalls,
        f"{where}: undefined dynamic-symbol surface mismatch",
    )
    diagnostic_lines = require_nonnegative_int(
        value["backend_stack_diagnostic_lines"],
        f"{where}.backend_stack_diagnostic_lines",
    )
    diagnostic_symbols = require_nonnegative_int(
        value["backend_stack_diagnostic_symbols"],
        f"{where}.backend_stack_diagnostic_symbols",
    )
    diagnostic_survivors = require_nonnegative_int(
        value["backend_stack_diagnostic_survivors"],
        f"{where}.backend_stack_diagnostic_survivors",
    )
    require(
        diagnostic_survivors == 0,
        f"{where}: backend stack-diagnostic symbol survived final LTO",
    )
    frame = validate_frame_audit(
        value["final_frame_audit"], f"{where}.final_frame_audit"
    )
    loader = validate_loader_measurement(
        value["loader"],
        where=f"{where}.loader",
        elf_bytes=elf_bytes,
        expected_max_len=expected_max_len,
        rent_model=rent_model,
    )
    return {
        "run": run,
        "build_mode": expected_mode,
        "elf_sha256": elf_hash,
        "elf_bytes": elf_bytes,
        "text_bytes": text_bytes,
        "rodata_bytes": rodata_bytes,
        "undefined_dynamic_symbols": parsed_symbols,
        "backend_stack_diagnostic_lines": diagnostic_lines,
        "backend_stack_diagnostic_symbols": diagnostic_symbols,
        "backend_stack_diagnostic_survivors": diagnostic_survivors,
        "final_frame_audit": frame,
        "loader": loader,
    }


def validate_source(value: Any) -> dict[str, str]:
    require(isinstance(value, dict), "measurement evidence.source: expected object")
    exact_keys(
        value,
        {
            "git_commit",
            "git_tree",
            "closure_paths",
            "closure_file_count",
            "closure_digest_sha256",
            "measurement_code",
            "cleanliness",
        },
        "measurement evidence.source",
    )
    commit_oid = require_git_object_id(
        value["git_commit"], "measurement evidence.source.git_commit"
    )
    tree_oid = require_git_object_id(
        value["git_tree"], "measurement evidence.source.git_tree"
    )
    git_object_id_lengths = {len(commit_oid), len(tree_oid)}
    require_hex32(
        value["closure_digest_sha256"],
        "measurement evidence.source.closure_digest_sha256",
    )
    require_positive_int(
        value["closure_file_count"], "measurement evidence.source.closure_file_count"
    )
    paths = value["closure_paths"]
    require(
        isinstance(paths, list) and paths,
        "measurement evidence.source.closure_paths: expected nonempty array",
    )
    require(
        all(isinstance(path, str) and path for path in paths),
        "measurement evidence.source.closure_paths: malformed path",
    )
    require(
        paths == sorted(set(paths)),
        "measurement evidence.source.closure_paths: noncanonical order",
    )
    required_code_paths = {path for _role, path in LINKED_MEASUREMENT_CODE_INPUTS}
    require(
        required_code_paths.issubset(paths),
        "measurement evidence.source.closure_paths: measurement code is outside tracked closure",
    )
    code = value["measurement_code"]
    require(
        isinstance(code, list),
        "measurement evidence.source.measurement_code: expected array",
    )
    require(
        len(code) == len(LINKED_MEASUREMENT_CODE_INPUTS),
        "measurement evidence.source.measurement_code: incomplete execution provenance",
    )
    for index, ((expected_role, expected_path), row) in enumerate(
        zip(LINKED_MEASUREMENT_CODE_INPUTS, code, strict=True)
    ):
        where = f"measurement evidence.source.measurement_code[{index}]"
        require(isinstance(row, dict), f"{where}: expected object")
        exact_keys(row, {"role", "path", "sha256", "git_blob_oid"}, where)
        require(row["role"] == expected_role, f"{where}: unexpected role")
        require(row["path"] == expected_path, f"{where}: unexpected path")
        require_hex32(row["sha256"], f"{where}.sha256")
        blob_oid = require_git_object_id(
            row["git_blob_oid"], f"{where}.git_blob_oid"
        )
        git_object_id_lengths.add(len(blob_oid))
    require(
        len(git_object_id_lengths) == 1,
        "measurement evidence.source: mixed Git object identity formats",
    )
    cleanliness = value["cleanliness"]
    require(
        isinstance(cleanliness, dict),
        "measurement evidence.source.cleanliness: expected object",
    )
    exact_keys(
        cleanliness,
        {"tracked_before", "untracked_before", "tracked_after", "untracked_after"},
        "measurement evidence.source.cleanliness",
    )
    for key in (
        "tracked_before",
        "untracked_before",
        "tracked_after",
        "untracked_after",
    ):
        require(
            cleanliness[key] == [],
            f"measurement evidence.source.cleanliness.{key}: linked input closure is dirty",
        )
    return {"git_commit": commit_oid, "git_tree": tree_oid}


def validate_toolchain(value: Any) -> None:
    require(isinstance(value, dict), "measurement evidence.toolchain: expected object")
    exact_keys(
        value,
        {
            "cargo_build_sbf",
            "platform_rustc",
            "llvm_readobj",
            "llvm_objdump",
            "platform_tools",
            "cargo_profile",
            "lto",
            "codegen_units",
            "overflow_checks",
        },
        "measurement evidence.toolchain",
    )
    for key in ("cargo_build_sbf", "platform_rustc", "llvm_readobj", "llvm_objdump"):
        tool = value[key]
        require(
            isinstance(tool, dict),
            f"measurement evidence.toolchain.{key}: expected object",
        )
        exact_keys(tool, {"version", "sha256"}, f"measurement evidence.toolchain.{key}")
        require_string(tool["version"], f"measurement evidence.toolchain.{key}.version")
        require_hex32(tool["sha256"], f"measurement evidence.toolchain.{key}.sha256")
    require_string(
        value["platform_tools"], "measurement evidence.toolchain.platform_tools"
    )
    require(
        value["cargo_profile"] == "release",
        "measurement evidence.toolchain: wrong Cargo profile",
    )
    require(value["lto"] == "fat", "measurement evidence.toolchain: wrong LTO mode")
    require(
        value["codegen_units"] == 1,
        "measurement evidence.toolchain: wrong codegen unit count",
    )
    require(
        value["overflow_checks"] is True,
        "measurement evidence.toolchain: overflow checks disabled",
    )


def extract_historical_measurement(
    evidence: Any, profile_name: str
) -> dict[str, int | str]:
    require(isinstance(evidence, dict), "measurement evidence: expected object")
    require(
        evidence.get("schema") == HISTORICAL_MEASUREMENT_SCHEMA,
        f"measurement evidence: historical requires {HISTORICAL_MEASUREMENT_SCHEMA}",
    )
    require(
        evidence.get("release_declaration") is False,
        "measurement evidence: measurement input must not declare a release",
    )
    rent_model = evidence.get("rent_model")
    require(
        isinstance(rent_model, dict), "measurement evidence.rent_model: expected object"
    )
    require(
        rent_model.get("model") == "upgradeable-loader-v3-program-plus-programdata",
        "measurement evidence.rent_model: unsupported historical loader model",
    )
    rate = require_positive_int(
        rent_model.get("rent_lamports_per_byte"),
        "measurement evidence.rent_model.rent_lamports_per_byte",
    )
    program_billable = require_positive_int(
        rent_model.get("program_account_bytes"),
        "measurement evidence.rent_model.program_account_bytes",
    )
    programdata_extra = require_positive_int(
        rent_model.get("programdata_metadata_bytes"),
        "measurement evidence.rent_model.programdata_metadata_bytes",
    )
    profiles = evidence.get("profiles")
    require(isinstance(profiles, list), "measurement evidence.profiles: expected array")
    selected = [
        profile
        for profile in profiles
        if isinstance(profile, dict) and profile.get("name") == profile_name
    ]
    require(
        len(selected) == 1,
        f"measurement evidence: expected one profile {profile_name!r}",
    )
    profile = selected[0]
    require(
        "capability_profile_identity_sha256" not in profile,
        "measurement evidence: V1 historical record unexpectedly claims a semantic binding",
    )
    require(
        profile.get("reproducible") is True,
        "measurement evidence: profile is not reproducible",
    )
    measurements = profile.get("measurements")
    require(
        isinstance(measurements, list) and len(measurements) == 2,
        "measurement evidence: exactly two fresh-run measurements are required",
    )
    parsed_runs: list[dict[str, int | str]] = []
    for index, measurement in enumerate(measurements):
        where = f"measurement evidence.measurements[{index}]"
        require(isinstance(measurement, dict), f"{where}: expected object")
        run = require_positive_int(measurement.get("run"), f"{where}.run")
        elf_bytes = require_positive_int(
            measurement.get("elf_bytes"), f"{where}.elf_bytes"
        )
        total = require_positive_int(
            measurement.get("total_loader_rent_lamports"),
            f"{where}.total_loader_rent_lamports",
        )
        require(
            total == (elf_bytes + program_billable + programdata_extra) * rate,
            f"{where}: loader rent does not match recorded historical model",
        )
        parsed_runs.append(
            {
                "run": run,
                "elf_sha256": require_hex32(
                    measurement.get("elf_sha256"), f"{where}.elf_sha256"
                ),
                "elf_bytes": elf_bytes,
                "text_bytes": require_positive_int(
                    measurement.get("text_bytes"), f"{where}.text_bytes"
                ),
                "persistent_loader_rent_lamports": total,
            }
        )
    require(
        [int(run["run"]) for run in parsed_runs] == [1, 2],
        "measurement evidence: runs must be 1 and 2",
    )
    for key in (
        "elf_sha256",
        "elf_bytes",
        "text_bytes",
        "persistent_loader_rent_lamports",
    ):
        require(
            parsed_runs[0][key] == parsed_runs[1][key],
            f"measurement evidence: non-reproducible {key}",
        )
    return {
        key: parsed_runs[0][key]
        for key in (
            "elf_sha256",
            "elf_bytes",
            "text_bytes",
            "persistent_loader_rent_lamports",
        )
    }


def extract_linked_measurement(
    evidence: Any,
    *,
    profile_name: str,
    computed_identity: str,
    expected_identity_manifest_sha256: str,
    label: str,
    build_contract: dict[str, Any],
    capabilities: list[dict[str, Any]],
    central_registry: dict[str, Any],
    wire_surface: dict[str, Any],
    expected_wire_surface_sha256: str,
    limits: dict[str, int],
) -> dict[str, Any]:
    require(isinstance(evidence, dict), "measurement evidence: expected object")
    require(
        evidence.get("schema") == LINKED_MEASUREMENT_SCHEMA,
        f"measurement evidence: linked requires {LINKED_MEASUREMENT_SCHEMA}",
    )
    require(
        evidence.get("availability") == "available",
        "measurement evidence: linked evidence is unavailable",
    )
    exact_keys(
        evidence,
        {
            "schema",
            "availability",
            "release_declaration",
            "manifest_input_source_clean",
            "source",
            "toolchain",
            "rent_model",
            "profiles",
            "refusals",
        },
        "measurement evidence",
    )
    require(
        evidence["release_declaration"] is False,
        "measurement evidence: measurement input must not declare a release",
    )
    require(
        evidence["manifest_input_source_clean"] is True,
        "measurement evidence: linked source closure is not clean",
    )
    require(
        evidence["refusals"] == [],
        "measurement evidence: available evidence carries refusals",
    )
    source = validate_source(evidence["source"])
    validate_toolchain(evidence["toolchain"])
    rent_model = validate_loader_model(evidence["rent_model"])

    profiles = evidence["profiles"]
    require(isinstance(profiles, list), "measurement evidence.profiles: expected array")
    selected = [
        profile
        for profile in profiles
        if isinstance(profile, dict) and profile.get("name") == profile_name
    ]
    require(
        len(selected) == 1,
        f"measurement evidence: expected one profile {profile_name!r}",
    )
    profile = selected[0]
    exact_keys(
        profile,
        {
            "name",
            "label",
            "source_identity",
            "cargo_features",
            "capability_profile_identity_sha256",
            "identity_manifest_sha256",
            "semantic_owners",
            "central_registry",
            "wire_surface",
            "wire_surface_sha256",
            "reproducible",
            "measurements",
            "default_feature_equivalence",
            "retained_workdirs",
        },
        "measurement evidence.profile",
    )
    require(profile["label"] == label, "measurement evidence: profile label mismatch")
    require(
        profile["source_identity"] == build_contract["source_identity"],
        "measurement evidence: source/lab identity mismatch",
    )
    require(
        profile["cargo_features"] == cargo_features(build_contract),
        "measurement evidence: Cargo feature identity mismatch",
    )
    require(
        require_hex32(
            profile["capability_profile_identity_sha256"],
            "measurement evidence.capability_profile_identity_sha256",
        )
        == computed_identity,
        "measurement evidence: capability profile identity mismatch",
    )
    manifest_digest = require_hex32(
        profile["identity_manifest_sha256"],
        "measurement evidence.identity_manifest_sha256",
    )
    require(
        manifest_digest == expected_identity_manifest_sha256,
        "measurement evidence: producer identity-manifest digest mismatch",
    )
    require(
        profile["semantic_owners"] == capabilities,
        "measurement evidence: semantic-owner manifest mismatch",
    )
    require(
        profile["central_registry"] == central_registry,
        "measurement evidence: central-registry manifest mismatch",
    )
    require(
        profile["wire_surface"] == wire_surface,
        "measurement evidence: wire-surface manifest mismatch",
    )
    require(
        require_hex32(
            profile["wire_surface_sha256"],
            "measurement evidence.wire_surface_sha256",
        )
        == expected_wire_surface_sha256,
        "measurement evidence: wire-surface identity mismatch",
    )
    require(
        profile["reproducible"] is True,
        "measurement evidence: profile is not reproducible",
    )
    retained = profile["retained_workdirs"]
    require(
        isinstance(retained, list) and all(isinstance(item, str) for item in retained),
        "measurement evidence: malformed retained workdirs",
    )

    measurements = profile["measurements"]
    require(
        isinstance(measurements, list) and len(measurements) == 2,
        "measurement evidence: exactly two explicit fresh-run measurements are required",
    )
    expected_syscalls = build_contract["expected_undefined_dynamic_symbols"]
    runs = [
        validate_v2_run(
            measurement,
            where=f"measurement evidence.measurements[{index}]",
            expected_run=index + 1,
            expected_mode="explicit-profile",
            expected_syscalls=expected_syscalls,
            expected_max_len=limits["programdata_max_len"],
            rent_model=rent_model,
        )
        for index, measurement in enumerate(measurements)
    ]
    comparable = (
        "elf_sha256",
        "elf_bytes",
        "text_bytes",
        "rodata_bytes",
        "undefined_dynamic_symbols",
        "backend_stack_diagnostic_lines",
        "backend_stack_diagnostic_symbols",
        "backend_stack_diagnostic_survivors",
        "final_frame_audit",
        "loader",
    )
    for key in comparable:
        require(
            runs[0][key] == runs[1][key],
            f"measurement evidence: non-reproducible {key}",
        )

    default_equivalence = profile["default_feature_equivalence"]
    needs_default_equivalence = (
        build_contract["cargo_profile_feature"] == "profile-full"
        and build_contract["source_identity"] == "production-inert"
    )
    if needs_default_equivalence:
        require(
            isinstance(default_equivalence, dict),
            "measurement evidence: default/full equivalence evidence is required",
        )
        exact_keys(
            default_equivalence,
            {"capability_profile_identity_sha256", "measurement", "matches_explicit"},
            "measurement evidence.default_feature_equivalence",
        )
        require(
            require_hex32(
                default_equivalence["capability_profile_identity_sha256"],
                "measurement evidence.default_feature_equivalence.capability_profile_identity_sha256",
            )
            == computed_identity,
            "measurement evidence: default/full identity-manifest mismatch",
        )
        default_run = validate_v2_run(
            default_equivalence["measurement"],
            where="measurement evidence.default_feature_equivalence.measurement",
            expected_run=3,
            expected_mode="cargo-default",
            expected_syscalls=expected_syscalls,
            expected_max_len=limits["programdata_max_len"],
            rent_model=rent_model,
        )
        require(
            default_equivalence["matches_explicit"] is True,
            "measurement evidence: default/full equivalence did not pass",
        )
        for key in comparable:
            require(
                default_run[key] == runs[0][key],
                f"measurement evidence: Cargo default differs from explicit full in {key}",
            )
    else:
        require(
            default_equivalence is None,
            "measurement evidence: default equivalence cannot cross a distinct profile/lab identity",
        )

    loader = runs[0]["loader"]
    return {
        "source_git_commit": source["git_commit"],
        "source_git_tree": source["git_tree"],
        "elf_sha256": runs[0]["elf_sha256"],
        "elf_bytes": runs[0]["elf_bytes"],
        "text_bytes": runs[0]["text_bytes"],
        "rodata_bytes": runs[0]["rodata_bytes"],
        "undefined_dynamic_symbols": runs[0]["undefined_dynamic_symbols"],
        "persistent_loader_rent_lamports": loader[
            "persistent_program_plus_programdata_rent_lamports"
        ],
        "transient_buffer_rent_lamports": loader["transient_buffer_rent_lamports"],
        "chosen_programdata_max_len": loader["chosen_programdata_max_len"],
        "exact_size_allocation": loader["exact_size_allocation"],
    }


def check_budget(measurement: dict[str, Any], limits: dict[str, int]) -> None:
    checks = (
        ("elf_bytes", "max_elf_bytes"),
        ("text_bytes", "max_text_bytes"),
        ("persistent_loader_rent_lamports", "max_persistent_loader_rent_lamports"),
    )
    for measured, maximum in checks:
        actual = int(measurement[measured])
        ceiling = limits[maximum]
        require(
            actual <= ceiling,
            f"artifact budget: {measured} {actual} exceeds {maximum} {ceiling}",
        )


def validate_manifest(
    data: Any, *, repo: Path, require_deployable: bool = False
) -> dict[str, Any]:
    require(isinstance(data, dict), "manifest: expected object")
    exact_keys(
        data,
        {
            "schema",
            "release_declaration",
            "profile",
            "build_contract",
            "capabilities",
            "central_registry",
            "wire_surface",
            "artifact_budget",
        },
        "manifest",
    )
    require(data["schema"] == MANIFEST_SCHEMA, "manifest.schema: unsupported schema")
    require(
        data["release_declaration"] is False,
        "manifest: profile input must not declare a release",
    )

    profile = data["profile"]
    require(isinstance(profile, dict), "profile: expected object")
    exact_keys(
        profile, {"name", "label", "classification", "identity_sha256"}, "profile"
    )
    name = require_string(profile["name"], "profile.name")
    require(
        PROFILE_NAME.fullmatch(name) is not None,
        "profile.name: malformed canonical name",
    )
    label = require_string(profile["label"], "profile.label")
    require(
        PROFILE_LABEL.fullmatch(label) is not None,
        "profile.label: malformed canonical label",
    )
    classification = require_string(profile["classification"], "profile.classification")
    require(
        classification in {"planning", "deployable"},
        "profile.classification: unknown state",
    )

    build_contract = validate_build_contract(data["build_contract"])
    if classification == "deployable":
        require(
            build_contract["source_identity"]
            in {"production-inert", "runtime-real-pyth-release"},
            "profile: non-production source identity cannot be deployable",
        )
    if build_contract["source_identity"] == "runtime-real-pyth-release":
        require(
            build_contract["cargo_profile_feature"]
            == SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE,
            "profile: runtime real-Pyth release requires the chain-attached successor profile",
        )
    if (
        build_contract["cargo_profile_feature"]
        == SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE
    ):
        require(
            build_contract["source_identity"] == "runtime-real-pyth-release",
            "profile: chain-attached successor requires the runtime real-Pyth release identity",
        )
    capabilities = validate_capabilities(data["capabilities"])
    central_registry = validate_registry(data["central_registry"], capabilities)
    wire_surface = validate_wire_surface(
        data["wire_surface"],
        build_contract=build_contract,
        capabilities=capabilities,
        central_registry=central_registry,
    )
    computed_wire_surface_sha256 = wire_surface_sha256(wire_surface)
    artifact_budget = data["artifact_budget"]
    require(isinstance(artifact_budget, dict), "artifact_budget: expected object")
    exact_keys(
        artifact_budget,
        {"limits", "measurement_class", "evidence_path", "evidence_profile_name"},
        "artifact_budget",
    )
    limits = validate_limits(artifact_budget["limits"])
    computed_identity = profile_identity(
        name,
        label,
        build_contract,
        capabilities,
        central_registry,
        wire_surface,
        limits,
    )
    declared_identity = require_hex32(
        profile["identity_sha256"], "profile.identity_sha256"
    )
    require(
        declared_identity == computed_identity,
        "profile.identity_sha256: canonical preimage mismatch",
    )
    expected_identity_manifest_sha256 = measurement_input_manifest_sha256(data)

    linked = [row["slot"] for row in capabilities if row["linkage"] == "linked"]
    planned = [row["slot"] for row in capabilities if row["linkage"] == "planned"]
    measurement_class = require_string(
        artifact_budget["measurement_class"], "artifact_budget.measurement_class"
    )
    require(
        measurement_class in {"planned", "historical", "linked"},
        "artifact_budget.measurement_class: unknown class",
    )

    measurement: dict[str, Any] | None = None
    if measurement_class == "planned":
        require(
            artifact_budget["evidence_path"] is None,
            "artifact_budget: planned evidence_path must be null",
        )
        require(
            artifact_budget["evidence_profile_name"] is None,
            "artifact_budget: planned evidence_profile_name must be null",
        )
    else:
        evidence_path = resolve_repository_path(
            repo, artifact_budget["evidence_path"], "artifact_budget.evidence_path"
        )
        evidence_profile_name = require_string(
            artifact_budget["evidence_profile_name"],
            "artifact_budget.evidence_profile_name",
        )
        require(
            evidence_profile_name != "", "artifact_budget.evidence_profile_name: empty"
        )
        evidence = load_json(evidence_path)
        if measurement_class == "historical":
            measurement = extract_historical_measurement(
                evidence, evidence_profile_name
            )
        else:
            measurement = extract_linked_measurement(
                evidence,
                profile_name=evidence_profile_name,
                computed_identity=computed_identity,
                expected_identity_manifest_sha256=expected_identity_manifest_sha256,
                label=label,
                build_contract=build_contract,
                capabilities=capabilities,
                central_registry=central_registry,
                wire_surface=wire_surface,
                expected_wire_surface_sha256=computed_wire_surface_sha256,
                limits=limits,
            )
        check_budget(measurement, limits)

    if classification == "deployable":
        require(
            not planned,
            "profile: deployable classification contains planned capabilities",
        )
        require(
            measurement_class == "linked",
            "profile: deployable classification requires identity-linked V2 measurement evidence",
        )
    deployment_eligible = (
        classification == "deployable" and not planned and measurement_class == "linked"
    )
    if require_deployable:
        require(deployment_eligible, "profile: deployment eligibility required")

    return {
        "schema": MANIFEST_SCHEMA,
        "manifest_canonical_sha256": canonical_json_sha256(data),
        "profile_identity_sha256": computed_identity,
        "classification": classification,
        "source_identity": build_contract["source_identity"],
        "cargo_features": cargo_features(build_contract),
        "capabilities": capabilities,
        "central_registry": central_registry,
        "wire_surface": wire_surface,
        "wire_surface_sha256": computed_wire_surface_sha256,
        "limits": limits,
        "linked_capabilities": linked,
        "planned_capabilities": planned,
        "measurement_class": measurement_class,
        "measurement": measurement,
        "budget_evaluated": measurement is not None,
        "budget_within_limits": True if measurement is not None else None,
        "deployment_eligible": deployment_eligible,
        "release_declaration": False,
    }


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "manifest", type=Path, help="repository-relative or absolute manifest path"
    )
    parser.add_argument(
        "--repo",
        type=Path,
        default=Path(__file__).resolve().parents[3],
        help="repository root used to resolve evidence paths",
    )
    parser.add_argument(
        "--require-deployable",
        action="store_true",
        help="refuse planning profiles and comparison-only historical measurements",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    repo = args.repo.resolve()
    manifest_path = (
        args.manifest if args.manifest.is_absolute() else repo / args.manifest
    )
    try:
        summary = validate_manifest(
            load_json(manifest_path),
            repo=repo,
            require_deployable=args.require_deployable,
        )
    except ProfileError as exc:
        print(f"REFUSE: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
