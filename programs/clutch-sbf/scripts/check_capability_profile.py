#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Offline gate for semantic capability profiles and their ELF budgets.

This checker reads local JSON only.  It does not build, measure, deploy, sign,
or contact an RPC endpoint.  Historical V1 capability measurements may be used
for comparison, but cannot qualify a profile as deployable because they do not
bind the semantic capability-profile identity introduced here.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import sys
from typing import Any


MANIFEST_SCHEMA = "dragons-clutch/capability-profile-manifest/v1"
IDENTITY_DOMAIN = "dragons-clutch/capability-profile-identity/v1"
HISTORICAL_MEASUREMENT_SCHEMA = "dragons-clutch/capability-profile-measurement/v1"
LINKED_MEASUREMENT_SCHEMA = "dragons-clutch/capability-profile-measurement/v2"

CAPABILITY_OWNERS: tuple[tuple[str, str], ...] = (
    ("score", "dragons-clutch/semantic-owner/score"),
    ("candidate-lifecycle", "dragons-clutch/semantic-owner/candidate-lifecycle"),
    ("clear-work", "dragons-clutch/semantic-owner/clear-work"),
    ("source-plane", "dragons-clutch/semantic-owner/source-plane"),
    ("retirement", "dragons-clutch/semantic-owner/retirement"),
    ("structured-claim", "dragons-clutch/semantic-owner/structured-claim"),
    ("liquidity", "dragons-clutch/semantic-owner/liquidity"),
)
CAPABILITY_SLOTS = tuple(slot for slot, _owner in CAPABILITY_OWNERS)
KNOWN_OWNERS = frozenset(owner for _slot, owner in CAPABILITY_OWNERS)
EXPECTED_OWNER = dict(CAPABILITY_OWNERS)

HEX_32 = re.compile(r"[0-9a-f]{64}\Z")
SEMANTIC_VERSION = re.compile(r"[a-z0-9][a-z0-9._/+:-]{0,127}\Z")
PROFILE_LABEL = re.compile(
    r"dragons-clutch/capability-profile/[a-z0-9][a-z0-9._/-]*/v[1-9][0-9]*\Z"
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


def require_positive_int(value: Any, where: str) -> int:
    require(type(value) is int and value > 0, f"{where}: expected positive integer")
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


def validate_capabilities(value: Any) -> list[dict[str, str]]:
    require(isinstance(value, list), "capabilities: expected array")
    rows: list[dict[str, str]] = []
    seen_slots: set[str] = set()
    seen_owners: set[str] = set()
    for index, row in enumerate(value):
        where = f"capabilities[{index}]"
        require(isinstance(row, dict), f"{where}: expected object")
        exact_keys(
            row,
            {"slot", "owner", "linkage", "semantic_version", "semantic_digest_sha256"},
            where,
        )
        slot = require_string(row["slot"], f"{where}.slot")
        owner = require_string(row["owner"], f"{where}.owner")
        linkage = require_string(row["linkage"], f"{where}.linkage")
        version = require_string(row["semantic_version"], f"{where}.semantic_version")
        digest = require_hex32(row["semantic_digest_sha256"], f"{where}.semantic_digest_sha256")

        require(slot in CAPABILITY_SLOTS, f"{where}.slot: unknown capability slot {slot!r}")
        require(slot not in seen_slots, f"{where}.slot: duplicate capability slot {slot!r}")
        require(owner in KNOWN_OWNERS, f"{where}.owner: unknown capability owner {owner!r}")
        require(owner not in seen_owners, f"{where}.owner: duplicate capability owner {owner!r}")
        require(
            owner == EXPECTED_OWNER[slot],
            f"{where}.owner: owner {owner!r} does not own slot {slot!r}",
        )
        require(linkage in {"linked", "planned"}, f"{where}.linkage: unknown state")
        require(
            SEMANTIC_VERSION.fullmatch(version) is not None,
            f"{where}.semantic_version: malformed semantic version",
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
            }
        )

    missing_slots = sorted(set(CAPABILITY_SLOTS) - seen_slots)
    missing_owners = sorted(KNOWN_OWNERS - seen_owners)
    require(
        not missing_slots and not missing_owners,
        "capabilities: missing capability slots "
        f"{missing_slots}; missing capability owners {missing_owners}",
    )
    observed_order = tuple(row["slot"] for row in rows)
    require(
        observed_order == CAPABILITY_SLOTS,
        f"capabilities: noncanonical order {observed_order!r}; expected {CAPABILITY_SLOTS!r}",
    )
    return rows


def validate_limits(value: Any) -> dict[str, int]:
    require(isinstance(value, dict), "artifact_budget.limits: expected object")
    exact_keys(
        value,
        {"max_elf_bytes", "max_text_bytes", "max_total_loader_rent_lamports"},
        "artifact_budget.limits",
    )
    limits = {
        "max_elf_bytes": require_positive_int(
            value["max_elf_bytes"], "artifact_budget.limits.max_elf_bytes"
        ),
        "max_text_bytes": require_positive_int(
            value["max_text_bytes"], "artifact_budget.limits.max_text_bytes"
        ),
        "max_total_loader_rent_lamports": require_positive_int(
            value["max_total_loader_rent_lamports"],
            "artifact_budget.limits.max_total_loader_rent_lamports",
        ),
    }
    require(
        limits["max_text_bytes"] <= limits["max_elf_bytes"],
        "artifact_budget.limits: text ceiling exceeds ELF ceiling",
    )
    return limits


def profile_identity(label: str, capabilities: list[dict[str, str]], limits: dict[str, int]) -> str:
    preimage = {
        "domain": IDENTITY_DOMAIN,
        "label": label,
        "capabilities": capabilities,
        "artifact_budget_limits": limits,
    }
    encoded = json.dumps(
        preimage, ensure_ascii=True, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def resolve_repository_path(repo: Path, value: Any, where: str) -> Path:
    text = require_string(value, where)
    relative = PurePosixPath(text)
    require(
        text != "" and not relative.is_absolute(),
        f"{where}: expected repository-relative path",
    )
    require(".." not in relative.parts and "." not in relative.parts, f"{where}: noncanonical path")
    require(str(relative) == text, f"{where}: noncanonical path")
    root = repo.resolve()
    resolved = (root / Path(*relative.parts)).resolve()
    try:
        resolved.relative_to(root)
    except ValueError as exc:
        raise ProfileError(f"{where}: path escapes repository") from exc
    require(resolved.is_file(), f"{where}: evidence file does not exist")
    return resolved


def extract_measurement(
    evidence: Any,
    *,
    evidence_class: str,
    profile_name: str,
    computed_identity: str,
) -> dict[str, int | str]:
    require(isinstance(evidence, dict), "measurement evidence: expected object")
    schema = evidence.get("schema")
    expected_schema = (
        HISTORICAL_MEASUREMENT_SCHEMA
        if evidence_class == "historical"
        else LINKED_MEASUREMENT_SCHEMA
    )
    require(
        schema == expected_schema,
        f"measurement evidence: {evidence_class} requires {expected_schema}",
    )
    require(
        evidence.get("release_declaration") is False,
        "measurement evidence: measurement input must not declare a release",
    )
    if evidence_class == "linked":
        require(
            evidence.get("manifest_input_source_clean") is True,
            "measurement evidence: linked source closure is not clean",
        )

    rent_model = evidence.get("rent_model")
    require(isinstance(rent_model, dict), "measurement evidence.rent_model: expected object")
    require(
        rent_model.get("model") == "upgradeable-loader-v3-program-plus-programdata",
        "measurement evidence.rent_model: unsupported loader model",
    )
    rent_lamports_per_byte = require_positive_int(
        rent_model.get("rent_lamports_per_byte"),
        "measurement evidence.rent_model.rent_lamports_per_byte",
    )
    program_account_bytes = require_positive_int(
        rent_model.get("program_account_bytes"),
        "measurement evidence.rent_model.program_account_bytes",
    )
    programdata_metadata_bytes = require_positive_int(
        rent_model.get("programdata_metadata_bytes"),
        "measurement evidence.rent_model.programdata_metadata_bytes",
    )

    profiles = evidence.get("profiles")
    require(isinstance(profiles, list), "measurement evidence.profiles: expected array")
    names: list[str] = []
    selected: list[dict[str, Any]] = []
    for index, profile in enumerate(profiles):
        require(
            isinstance(profile, dict),
            f"measurement evidence.profiles[{index}]: expected object",
        )
        name = require_string(
            profile.get("name"), f"measurement evidence.profiles[{index}].name"
        )
        require(name not in names, f"measurement evidence: duplicate profile name {name!r}")
        names.append(name)
        if name == profile_name:
            selected.append(profile)
    require(len(selected) == 1, f"measurement evidence: expected one profile {profile_name!r}")
    profile = selected[0]
    require(
        profile.get("reproducible") is True,
        "measurement evidence: profile is not reproducible",
    )
    if evidence_class == "linked":
        bound = require_hex32(
            profile.get("capability_profile_identity_sha256"),
            "measurement evidence.capability_profile_identity_sha256",
        )
        require(
            bound == computed_identity,
            "measurement evidence: capability profile identity mismatch",
        )
    else:
        require(
            "capability_profile_identity_sha256" not in profile,
            "measurement evidence: V1 historical record unexpectedly claims a semantic binding",
        )

    measurements = profile.get("measurements")
    require(
        isinstance(measurements, list) and len(measurements) == 2,
        "measurement evidence: exactly two fresh-run measurements are required",
    )
    comparable = ("elf_sha256", "elf_bytes", "text_bytes", "total_loader_rent_lamports")
    runs: list[dict[str, int | str]] = []
    for index, measurement in enumerate(measurements):
        where = f"measurement evidence.measurements[{index}]"
        require(isinstance(measurement, dict), f"{where}: expected object")
        run = require_positive_int(measurement.get("run"), f"{where}.run")
        parsed: dict[str, int | str] = {
            "run": run,
            "elf_sha256": require_hex32(measurement.get("elf_sha256"), f"{where}.elf_sha256"),
            "elf_bytes": require_positive_int(measurement.get("elf_bytes"), f"{where}.elf_bytes"),
            "text_bytes": require_positive_int(
                measurement.get("text_bytes"), f"{where}.text_bytes"
            ),
            "total_loader_rent_lamports": require_positive_int(
                measurement.get("total_loader_rent_lamports"),
                f"{where}.total_loader_rent_lamports",
            ),
        }
        require(
            int(parsed["text_bytes"]) <= int(parsed["elf_bytes"]),
            f"{where}: executable text exceeds ELF size",
        )
        expected_rent = (
            int(parsed["elf_bytes"])
            + program_account_bytes
            + programdata_metadata_bytes
        ) * rent_lamports_per_byte
        require(
            int(parsed["total_loader_rent_lamports"]) == expected_rent,
            f"{where}: loader rent does not match recorded model",
        )
        runs.append(parsed)
    require(
        sorted(int(run["run"]) for run in runs) == [1, 2],
        "measurement evidence: runs must be 1 and 2",
    )
    for key in comparable:
        require(runs[0][key] == runs[1][key], f"measurement evidence: non-reproducible {key}")
    return {key: runs[0][key] for key in comparable}


def check_budget(measurement: dict[str, int | str], limits: dict[str, int]) -> None:
    checks = (
        ("elf_bytes", "max_elf_bytes"),
        ("text_bytes", "max_text_bytes"),
        ("total_loader_rent_lamports", "max_total_loader_rent_lamports"),
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
        {"schema", "release_declaration", "profile", "capabilities", "artifact_budget"},
        "manifest",
    )
    require(data["schema"] == MANIFEST_SCHEMA, "manifest.schema: unsupported schema")
    require(
        data["release_declaration"] is False,
        "manifest: profile input must not declare a release",
    )

    profile = data["profile"]
    require(isinstance(profile, dict), "profile: expected object")
    exact_keys(profile, {"label", "classification", "identity_sha256"}, "profile")
    label = require_string(profile["label"], "profile.label")
    require(PROFILE_LABEL.fullmatch(label) is not None, "profile.label: malformed canonical label")
    classification = require_string(profile["classification"], "profile.classification")
    require(classification in {"planning", "deployable"}, "profile.classification: unknown state")

    capabilities = validate_capabilities(data["capabilities"])
    artifact_budget = data["artifact_budget"]
    require(isinstance(artifact_budget, dict), "artifact_budget: expected object")
    exact_keys(
        artifact_budget,
        {"limits", "measurement_class", "evidence_path", "evidence_profile_name"},
        "artifact_budget",
    )
    limits = validate_limits(artifact_budget["limits"])
    computed_identity = profile_identity(label, capabilities, limits)
    declared_identity = require_hex32(profile["identity_sha256"], "profile.identity_sha256")
    require(
        declared_identity == computed_identity,
        "profile.identity_sha256: canonical preimage mismatch",
    )

    linked = [row["slot"] for row in capabilities if row["linkage"] == "linked"]
    planned = [row["slot"] for row in capabilities if row["linkage"] == "planned"]
    measurement_class = require_string(
        artifact_budget["measurement_class"], "artifact_budget.measurement_class"
    )
    require(
        measurement_class in {"planned", "historical", "linked"},
        "artifact_budget.measurement_class: unknown class",
    )

    measurement: dict[str, int | str] | None = None
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
            artifact_budget["evidence_profile_name"], "artifact_budget.evidence_profile_name"
        )
        require(evidence_profile_name != "", "artifact_budget.evidence_profile_name: empty")
        measurement = extract_measurement(
            load_json(evidence_path),
            evidence_class=measurement_class,
            profile_name=evidence_profile_name,
            computed_identity=computed_identity,
        )
        check_budget(measurement, limits)

    if classification == "deployable":
        require(not planned, "profile: deployable classification contains planned capabilities")
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
        "profile_identity_sha256": computed_identity,
        "classification": classification,
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
    parser.add_argument("manifest", type=Path, help="repository-relative or absolute manifest path")
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
    manifest_path = args.manifest if args.manifest.is_absolute() else repo / args.manifest
    try:
        summary = validate_manifest(
            load_json(manifest_path), repo=repo, require_deployable=args.require_deployable
        )
    except ProfileError as exc:
        print(f"REFUSE: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
