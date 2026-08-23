#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Deterministic adversarial tests for the offline capability-profile gate."""

from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest


SCRIPTS = Path(__file__).resolve().parent
ROOT = SCRIPTS.parents[2]
sys.path.insert(0, str(SCRIPTS))
import check_capability_profile as checker  # noqa: E402


HISTORICAL_EVIDENCE = Path(
    "programs/clutch-sbf/audit/evidence/2026-08-22-capability-profiles.json"
)


def semantic_digest(slot: str) -> str:
    """Return an unmistakably synthetic test-only semantic digest."""
    return hashlib.sha256(f"test-only/{slot}".encode("utf-8")).hexdigest()


def capabilities(linkage: str = "planned") -> list[dict[str, str]]:
    return [
        {
            "slot": slot,
            "owner": owner,
            "linkage": linkage,
            "semantic_version": f"{slot}-test-v1",
            "semantic_digest_sha256": semantic_digest(slot),
        }
        for slot, owner in checker.CAPABILITY_OWNERS
    ]


def limits() -> dict[str, int]:
    # Fixture arithmetic only; these values are not program measurements.
    return {
        "max_elf_bytes": 2_000,
        "max_text_bytes": 1_500,
        "max_total_loader_rent_lamports": 20_000,
    }


def manifest(
    *,
    linkage: str = "planned",
    classification: str = "planning",
    measurement_class: str = "planned",
    evidence_path: str | None = None,
    evidence_profile_name: str | None = None,
    budget_limits: dict[str, int] | None = None,
) -> dict[str, object]:
    rows = capabilities(linkage)
    declared_limits = limits() if budget_limits is None else budget_limits
    label = "dragons-clutch/capability-profile/test-fixture/v1"
    identity = checker.profile_identity(label, rows, declared_limits)
    return {
        "schema": checker.MANIFEST_SCHEMA,
        "release_declaration": False,
        "profile": {
            "label": label,
            "classification": classification,
            "identity_sha256": identity,
        },
        "capabilities": rows,
        "artifact_budget": {
            "limits": declared_limits,
            "measurement_class": measurement_class,
            "evidence_path": evidence_path,
            "evidence_profile_name": evidence_profile_name,
        },
    }


def measurement_document(
    profile_identity: str,
    *,
    elf_bytes: int = 1_000,
    text_bytes: int = 800,
) -> dict[str, object]:
    rent_model = {
        "model": "upgradeable-loader-v3-program-plus-programdata",
        "rent_lamports_per_byte": 2,
        "program_account_bytes": 100,
        "programdata_metadata_bytes": 50,
    }
    measurement = {
        "elf_sha256": hashlib.sha256(b"test-only-elf").hexdigest(),
        "elf_bytes": elf_bytes,
        "text_bytes": text_bytes,
        "total_loader_rent_lamports": (
            elf_bytes
            + rent_model["program_account_bytes"]
            + rent_model["programdata_metadata_bytes"]
        )
        * rent_model["rent_lamports_per_byte"],
    }
    return {
        "schema": checker.LINKED_MEASUREMENT_SCHEMA,
        "release_declaration": False,
        "manifest_input_source_clean": True,
        "rent_model": rent_model,
        "profiles": [
            {
                "name": "test-profile",
                "capability_profile_identity_sha256": profile_identity,
                "reproducible": True,
                "measurements": [
                    {"run": 1, **measurement},
                    {"run": 2, **measurement},
                ],
            }
        ],
    }


class CapabilityProfileTests(unittest.TestCase):
    def test_identity_is_deterministic_and_binds_every_semantic_slot_and_budget(self) -> None:
        value = manifest()
        rows = value["capabilities"]
        declared_limits = value["artifact_budget"]["limits"]  # type: ignore[index]
        label = value["profile"]["label"]  # type: ignore[index]
        first = checker.profile_identity(label, rows, declared_limits)  # type: ignore[arg-type]
        self.assertEqual(
            first,
            checker.profile_identity(label, rows, declared_limits),  # type: ignore[arg-type]
        )

        for index in range(len(rows)):  # type: ignore[arg-type]
            changed = copy.deepcopy(rows)
            changed[index]["semantic_digest_sha256"] = hashlib.sha256(
                f"changed/{index}".encode("utf-8")
            ).hexdigest()
            self.assertNotEqual(
                first,
                checker.profile_identity(label, changed, declared_limits),  # type: ignore[arg-type]
            )

        changed_version = copy.deepcopy(rows)
        changed_version[0]["semantic_version"] = "score-test-v2"
        self.assertNotEqual(
            first,
            checker.profile_identity(
                label, changed_version, declared_limits  # type: ignore[arg-type]
            ),
        )
        changed_linkage = copy.deepcopy(rows)
        changed_linkage[0]["linkage"] = "linked"
        self.assertNotEqual(
            first,
            checker.profile_identity(
                label, changed_linkage, declared_limits  # type: ignore[arg-type]
            ),
        )

        changed_limits = dict(declared_limits)  # type: ignore[arg-type]
        changed_limits["max_elf_bytes"] += 1
        self.assertNotEqual(
            first,
            checker.profile_identity(label, rows, changed_limits),  # type: ignore[arg-type]
        )

    def test_planned_profile_is_valid_but_never_deployment_eligible(self) -> None:
        summary = checker.validate_manifest(manifest(), repo=ROOT)
        self.assertEqual(summary["linked_capabilities"], [])
        self.assertEqual(summary["planned_capabilities"], list(checker.CAPABILITY_SLOTS))
        self.assertFalse(summary["budget_evaluated"])
        self.assertIsNone(summary["budget_within_limits"])
        self.assertFalse(summary["deployment_eligible"])
        with self.assertRaisesRegex(checker.ProfileError, "deployment eligibility"):
            checker.validate_manifest(manifest(), repo=ROOT, require_deployable=True)

    def test_missing_unknown_and_duplicate_capability_owners_refuse(self) -> None:
        missing = manifest()
        missing["capabilities"].pop()  # type: ignore[union-attr]
        with self.assertRaisesRegex(checker.ProfileError, "missing capability slots"):
            checker.validate_manifest(missing, repo=ROOT)

        unknown = manifest()
        unknown["capabilities"][0]["owner"] = (  # type: ignore[index]
            "dragons-clutch/semantic-owner/unknown"
        )
        with self.assertRaisesRegex(checker.ProfileError, "unknown capability owner"):
            checker.validate_manifest(unknown, repo=ROOT)

        duplicate = manifest()
        duplicate["capabilities"][1]["owner"] = (  # type: ignore[index]
            duplicate["capabilities"][0]["owner"]  # type: ignore[index]
        )
        with self.assertRaisesRegex(checker.ProfileError, "duplicate capability owner"):
            checker.validate_manifest(duplicate, repo=ROOT)

    def test_unknown_duplicate_and_noncanonical_capability_slots_refuse(self) -> None:
        unknown = manifest()
        unknown["capabilities"][0]["slot"] = "unknown"  # type: ignore[index]
        with self.assertRaisesRegex(checker.ProfileError, "unknown capability slot"):
            checker.validate_manifest(unknown, repo=ROOT)

        duplicate = manifest()
        duplicate["capabilities"][1]["slot"] = (  # type: ignore[index]
            duplicate["capabilities"][0]["slot"]  # type: ignore[index]
        )
        with self.assertRaisesRegex(checker.ProfileError, "duplicate capability slot"):
            checker.validate_manifest(duplicate, repo=ROOT)

        reordered = manifest()
        reordered["capabilities"][0], reordered["capabilities"][1] = (  # type: ignore[index]
            reordered["capabilities"][1],  # type: ignore[index]
            reordered["capabilities"][0],  # type: ignore[index]
        )
        with self.assertRaisesRegex(checker.ProfileError, "noncanonical order"):
            checker.validate_manifest(reordered, repo=ROOT)

    def test_semantic_version_and_digest_are_mandatory(self) -> None:
        missing = manifest()
        del missing["capabilities"][0]["semantic_version"]  # type: ignore[index]
        with self.assertRaisesRegex(checker.ProfileError, "keys"):
            checker.validate_manifest(missing, repo=ROOT)

        zero = manifest()
        zero["capabilities"][0]["semantic_digest_sha256"] = "0" * 64  # type: ignore[index]
        with self.assertRaisesRegex(checker.ProfileError, "zero digest"):
            checker.validate_manifest(zero, repo=ROOT)

    def test_identity_drift_refuses_before_evidence_is_considered(self) -> None:
        changed = manifest()
        changed["capabilities"][0]["semantic_version"] = "score-test-v2"  # type: ignore[index]
        with self.assertRaisesRegex(checker.ProfileError, "canonical preimage mismatch"):
            checker.validate_manifest(changed, repo=ROOT)

    def test_linked_clean_v2_measurement_can_qualify_a_deployable_profile(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            value = manifest(
                linkage="linked",
                classification="deployable",
                measurement_class="linked",
                evidence_path="measurement.json",
                evidence_profile_name="test-profile",
            )
            identity = value["profile"]["identity_sha256"]  # type: ignore[index]
            (repo / "measurement.json").write_text(
                json.dumps(
                    measurement_document(identity), sort_keys=True  # type: ignore[arg-type]
                ),
                encoding="utf-8",
            )
            summary = checker.validate_manifest(value, repo=repo, require_deployable=True)
            self.assertTrue(summary["budget_evaluated"])
            self.assertTrue(summary["budget_within_limits"])
            self.assertTrue(summary["deployment_eligible"])
            self.assertEqual(summary["planned_capabilities"], [])

    def test_deployable_profile_refuses_one_planned_component(self) -> None:
        value = manifest(classification="deployable")
        with self.assertRaisesRegex(checker.ProfileError, "planned capabilities"):
            checker.validate_manifest(value, repo=ROOT)

    def test_linked_evidence_must_bind_the_exact_profile_identity(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            value = manifest(
                linkage="linked",
                classification="deployable",
                measurement_class="linked",
                evidence_path="measurement.json",
                evidence_profile_name="test-profile",
            )
            wrong = hashlib.sha256(b"wrong-profile").hexdigest()
            (repo / "measurement.json").write_text(
                json.dumps(measurement_document(wrong), sort_keys=True), encoding="utf-8"
            )
            with self.assertRaisesRegex(checker.ProfileError, "identity mismatch"):
                checker.validate_manifest(value, repo=repo)

    def test_each_exact_budget_boundary_passes_and_one_byte_over_refuses(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            boundary_limits = {
                "max_elf_bytes": 2_000,
                "max_text_bytes": 1_500,
                "max_total_loader_rent_lamports": 4_300,
            }
            value = manifest(
                linkage="linked",
                classification="deployable",
                measurement_class="linked",
                evidence_path="measurement.json",
                evidence_profile_name="test-profile",
                budget_limits=boundary_limits,
            )
            identity = value["profile"]["identity_sha256"]  # type: ignore[index]
            (repo / "measurement.json").write_text(
                json.dumps(
                    measurement_document(
                        identity,  # type: ignore[arg-type]
                        elf_bytes=2_000,
                        text_bytes=1_500,
                    ),
                    sort_keys=True,
                ),
                encoding="utf-8",
            )
            checker.validate_manifest(value, repo=repo, require_deployable=True)

            (repo / "measurement.json").write_text(
                json.dumps(
                    measurement_document(
                        identity,  # type: ignore[arg-type]
                        elf_bytes=2_001,
                        text_bytes=1_500,
                    ),
                    sort_keys=True,
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(checker.ProfileError, "exceeds max_elf_bytes"):
                checker.validate_manifest(value, repo=repo)

    def test_nonreproducible_two_run_measurement_refuses(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            value = manifest(
                linkage="linked",
                classification="deployable",
                measurement_class="linked",
                evidence_path="measurement.json",
                evidence_profile_name="test-profile",
            )
            identity = value["profile"]["identity_sha256"]  # type: ignore[index]
            evidence = measurement_document(identity)  # type: ignore[arg-type]
            evidence["profiles"][0]["measurements"][1]["text_bytes"] += 1  # type: ignore[index]
            (repo / "measurement.json").write_text(
                json.dumps(evidence, sort_keys=True), encoding="utf-8"
            )
            with self.assertRaisesRegex(checker.ProfileError, "non-reproducible text_bytes"):
                checker.validate_manifest(value, repo=repo)

    def test_loader_rent_is_recomputed_from_the_recorded_model(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            value = manifest(
                linkage="linked",
                classification="deployable",
                measurement_class="linked",
                evidence_path="measurement.json",
                evidence_profile_name="test-profile",
            )
            identity = value["profile"]["identity_sha256"]  # type: ignore[index]
            evidence = measurement_document(identity)  # type: ignore[arg-type]
            for run in evidence["profiles"][0]["measurements"]:  # type: ignore[index]
                run["total_loader_rent_lamports"] += 1
            (repo / "measurement.json").write_text(
                json.dumps(evidence, sort_keys=True), encoding="utf-8"
            )
            with self.assertRaisesRegex(checker.ProfileError, "rent does not match"):
                checker.validate_manifest(value, repo=repo)

    def test_existing_v1_measurement_is_historical_only_without_copying_sizes(self) -> None:
        evidence = json.loads((ROOT / HISTORICAL_EVIDENCE).read_text(encoding="utf-8"))
        selected = next(profile for profile in evidence["profiles"] if profile["name"] == "full")
        measured = selected["measurements"][0]
        historical_limits = {
            "max_elf_bytes": measured["elf_bytes"],
            "max_text_bytes": measured["text_bytes"],
            "max_total_loader_rent_lamports": measured["total_loader_rent_lamports"],
        }
        value = manifest(
            measurement_class="historical",
            evidence_path=str(HISTORICAL_EVIDENCE),
            evidence_profile_name="full",
            budget_limits=historical_limits,
        )
        summary = checker.validate_manifest(value, repo=ROOT)
        self.assertEqual(summary["measurement_class"], "historical")
        self.assertTrue(summary["budget_evaluated"])
        self.assertTrue(summary["budget_within_limits"])
        self.assertFalse(summary["deployment_eligible"])
        with self.assertRaisesRegex(checker.ProfileError, "deployment eligibility"):
            checker.validate_manifest(value, repo=ROOT, require_deployable=True)

    def test_v1_measurement_cannot_be_relabelled_as_linked(self) -> None:
        value = manifest(
            linkage="linked",
            measurement_class="linked",
            evidence_path=str(HISTORICAL_EVIDENCE),
            evidence_profile_name="full",
        )
        with self.assertRaisesRegex(checker.ProfileError, "linked requires"):
            checker.validate_manifest(value, repo=ROOT)

    def test_duplicate_json_object_key_refuses_at_parse_time(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "duplicate.json"
            path.write_text('{"schema":"one","schema":"two"}', encoding="utf-8")
            with self.assertRaisesRegex(checker.ProfileError, "duplicate object key"):
                checker.load_json(path)


if __name__ == "__main__":
    unittest.main()
