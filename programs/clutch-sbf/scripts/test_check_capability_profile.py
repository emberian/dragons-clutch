#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Deterministic adversarial tests for the capability-profile V2 gate."""

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
SYSCALLS = ["abort", "sol_log_"]


def digest(label: str) -> str:
    return hashlib.sha256(f"test-only/{label}".encode("utf-8")).hexdigest()


def capabilities(
    linkage: str = "planned", profile_feature: str = "profile-general-source-v2-point"
) -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    for index, (slot, owner) in enumerate(checker.CAPABILITY_OWNERS, start=1):
        if profile_feature == checker.SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE:
            required_intents = []
            if slot == "relation":
                required_intents.extend(
                    [pair + [0] for pair in checker.SUCCESSOR_CHAIN_ATTACHED_LEGACY_INTENT_PAIRS]
                )
                required_intents.extend(
                    [pair + [0] for pair in checker.SUCCESSOR_CHAIN_ATTACHED_DIRECT_INTENT_PAIRS]
                )
            if slot == "source-plane":
                required_intents.extend(
                    copy.deepcopy(checker.CURRENT_SOURCE_EXTENSION_TRIPLES)
                )
            required_intents.sort()
        else:
            required_intents = [[index, 3, 0]]
            if slot == "source-plane" and profile_feature == "profile-full":
                required_intents.extend(copy.deepcopy(checker.CURRENT_SOURCE_EXTENSION_TRIPLES))
        rows.append(
            {
                "slot": slot,
                "owner": owner,
                "linkage": linkage,
                "semantic_version": f"{slot}-test-v1",
                "semantic_digest_sha256": digest(slot),
                "required_intent_triples": required_intents,
                "required_account_coordinates": [[index, 1]],
            }
        )
    return rows


def linked_coverage(rows: list[dict[str, object]]) -> dict[str, object]:
    intents: list[list[int]] = []
    accounts: list[list[int]] = []
    for row in rows:
        if row["linkage"] == "linked":
            intents.extend(row["required_intent_triples"])  # type: ignore[arg-type]
            accounts.extend(row["required_account_coordinates"])  # type: ignore[arg-type]
    return {
        "semantic_version": "central-registry-test-v1",
        "semantic_digest_sha256": digest("central-registry"),
        "enabled_intent_triples": sorted(intents),
        "linked_account_coordinates": sorted(accounts),
    }


def wire_surface(registry: dict[str, object]) -> dict[str, object]:
    pairs = [
        triple[:2]
        for triple in registry["enabled_intent_triples"]  # type: ignore[index]
        if triple[2] == 0
    ]
    return {
        "schema": checker.WIRE_SURFACE_SCHEMA,
        "legacy_intent_pairs": [
            pair for pair in pairs if pair[0] not in checker.DIRECT_V3_TAGS
        ],
        "dedicated_direct_intent_pairs": [
            pair for pair in pairs if pair[0] in checker.DIRECT_V3_TAGS
        ],
        "outer_request_actions": copy.deepcopy(checker.OUTER_REQUEST_ACTIONS),
        "source_generation_discriminants": [],
    }


def limits() -> dict[str, int]:
    return {
        "max_elf_bytes": 2_000,
        "max_text_bytes": 1_500,
        "programdata_max_len": 2_000,
        "max_persistent_loader_rent_lamports": 10_000,
    }


def manifest(
    *,
    linkage: str = "planned",
    classification: str = "planning",
    measurement_class: str = "planned",
    evidence_path: str | None = None,
    evidence_profile_name: str | None = None,
    source_identity: str = "production-inert",
    collateral_release_identity: str = "production-inert",
    profile_feature: str = "profile-general-source-v2-point",
    budget_limits: dict[str, int] | None = None,
) -> dict[str, object]:
    rows = capabilities(linkage, profile_feature)
    registry = linked_coverage(rows)
    declared_wire_surface = wire_surface(registry)
    declared_limits = limits() if budget_limits is None else budget_limits
    name = "test-profile"
    label = "dragons-clutch/capability-profile/test-fixture/v2"
    build_contract = {
        "cargo_profile_feature": profile_feature,
        "source_identity": source_identity,
        "collateral_release_identity": collateral_release_identity,
        "expected_undefined_dynamic_symbols": SYSCALLS,
    }
    identity = checker.profile_identity(
        name,
        label,
        build_contract,
        rows,
        registry,
        declared_wire_surface,
        declared_limits,
    )
    return {
        "schema": checker.MANIFEST_SCHEMA,
        "release_declaration": False,
        "profile": {
            "name": name,
            "label": label,
            "classification": classification,
            "identity_sha256": identity,
        },
        "build_contract": build_contract,
        "capabilities": rows,
        "central_registry": registry,
        "wire_surface": declared_wire_surface,
        "artifact_budget": {
            "limits": declared_limits,
            "measurement_class": measurement_class,
            "evidence_path": evidence_path,
            "evidence_profile_name": evidence_profile_name,
        },
    }


def loader(elf_bytes: int = 1_000, chosen_max_len: int = 2_000) -> dict[str, object]:
    rate = 2
    overhead = 128

    def account(role: str, data_len: int) -> dict[str, object]:
        billable = data_len + overhead
        return {
            "lifetime": "transient-recyclable" if role == "buffer" else "persistent",
            "data_len_bytes": data_len,
            "storage_overhead_bytes": overhead,
            "billable_bytes": billable,
            "rent_exempt_lamports": billable * rate,
        }

    program = account("program", 36)
    programdata = account("programdata", 45 + chosen_max_len)
    buffer = account("buffer", 37 + chosen_max_len)
    return {
        "current_elf_len_bytes": elf_bytes,
        "chosen_programdata_max_len": chosen_max_len,
        "exact_size_allocation": elf_bytes == chosen_max_len,
        "program": program,
        "programdata": programdata,
        "buffer": buffer,
        "persistent_program_plus_programdata_rent_lamports": (
            program["rent_exempt_lamports"] + programdata["rent_exempt_lamports"]  # type: ignore[operator]
        ),
        "transient_buffer_rent_lamports": buffer["rent_exempt_lamports"],
    }


def measurement_run(
    run: int,
    *,
    build_mode: str = "explicit-profile",
    elf_bytes: int = 1_000,
    text_bytes: int = 800,
    chosen_max_len: int = 2_000,
) -> dict[str, object]:
    return {
        "run": run,
        "build_mode": build_mode,
        "elf_sha256": digest("elf"),
        "elf_bytes": elf_bytes,
        "text_bytes": text_bytes,
        "rodata_bytes": 100,
        "undefined_dynamic_symbols": SYSCALLS,
        "backend_stack_diagnostic_lines": 2,
        "backend_stack_diagnostic_symbols": 2,
        "backend_stack_diagnostic_survivors": 0,
        "final_frame_audit": {
            "final_text_function_symbols": 10,
            "final_text_function_addresses": 10,
            "disassembled_function_regions": 10,
            "direct_r10_references": 4,
            "deepest_direct_r10_offset": 256,
            "deepest_direct_r10_function": "test_function",
            "direct_frame_limit_bytes": 4096,
            "direct_frame_bounds": "PASS",
        },
        "loader": loader(elf_bytes, chosen_max_len),
    }


def measurement_document(value: dict[str, object]) -> dict[str, object]:
    identity = value["profile"]["identity_sha256"]  # type: ignore[index]
    contract = value["build_contract"]
    default_equivalence = None
    if (
        contract["cargo_profile_feature"] == "profile-full"  # type: ignore[index]
        and contract["source_identity"] == "production-inert"  # type: ignore[index]
        and contract["collateral_release_identity"] == "production-inert"  # type: ignore[index]
    ):
        default_equivalence = {
            "capability_profile_identity_sha256": identity,
            "measurement": measurement_run(3, build_mode="cargo-default"),
            "matches_explicit": True,
        }
    return {
        "schema": checker.LINKED_MEASUREMENT_SCHEMA,
        "availability": "available",
        "release_declaration": False,
        "manifest_input_source_clean": True,
        "source": {
            "git_commit": digest("commit")[:40],
            "git_tree": digest("tree")[:40],
            "closure_paths": [
                "crates",
                "programs/clutch-sbf/program",
                "programs/clutch-sbf/scripts/check_capability_profile.py",
                "programs/clutch-sbf/scripts/measure_capability_profiles.py",
            ],
            "closure_file_count": 4,
            "closure_digest_sha256": digest("closure"),
            "measurement_code": [
                {
                    "role": role,
                    "path": path,
                    "sha256": digest(role),
                    "git_blob_oid": digest(f"{role}-blob")[:40],
                }
                for role, path in checker.LINKED_MEASUREMENT_CODE_INPUTS
            ],
            "cleanliness": {
                "tracked_before": [],
                "untracked_before": [],
                "tracked_after": [],
                "untracked_after": [],
            },
        },
        "toolchain": {
            "cargo_build_sbf": {"version": "test", "sha256": digest("builder")},
            "platform_rustc": {"version": "test", "sha256": digest("rustc")},
            "llvm_readobj": {"version": "test", "sha256": digest("readobj")},
            "llvm_objdump": {"version": "test", "sha256": digest("objdump")},
            "platform_tools": "v-test",
            "cargo_profile": "release",
            "lto": "fat",
            "codegen_units": 1,
            "overflow_checks": True,
        },
        "rent_model": {
            "model": "upgradeable-loader-v3",
            "rent_exempt_lamports_per_billable_byte": 2,
            "account_storage_overhead_bytes": 128,
            "program_data_len_bytes": 36,
            "programdata_metadata_data_len_bytes": 45,
            "buffer_metadata_data_len_bytes": 37,
        },
        "profiles": [
            {
                "name": value["profile"]["name"],  # type: ignore[index]
                "label": value["profile"]["label"],  # type: ignore[index]
                "source_identity": contract["source_identity"],  # type: ignore[index]
                "collateral_release_identity": contract["collateral_release_identity"],  # type: ignore[index]
                "cargo_features": checker.cargo_features(contract),  # type: ignore[arg-type]
                "capability_profile_identity_sha256": identity,
                "identity_manifest_sha256": checker.measurement_input_manifest_sha256(
                    value
                ),
                "semantic_owners": copy.deepcopy(value["capabilities"]),
                "central_registry": copy.deepcopy(value["central_registry"]),
                "wire_surface": copy.deepcopy(value["wire_surface"]),
                "wire_surface_sha256": checker.wire_surface_sha256(
                    value["wire_surface"]  # type: ignore[arg-type]
                ),
                "reproducible": True,
                "measurements": [measurement_run(1), measurement_run(2)],
                "default_feature_equivalence": default_equivalence,
                "retained_workdirs": [],
            }
        ],
        "refusals": [],
    }


class CapabilityProfileTests(unittest.TestCase):
    def test_identity_binds_owner_registry_build_identity_and_budget(self) -> None:
        value = manifest()
        original = value["profile"]["identity_sha256"]  # type: ignore[index]
        mutations = []
        for path, replacement in (
            (("capabilities", 0, "semantic_digest_sha256"), digest("changed-owner")),
            (
                ("central_registry", "semantic_digest_sha256"),
                digest("changed-registry"),
            ),
            (("build_contract", "source_identity"), "non-production-real-pyth-lab"),
            (("artifact_budget", "limits", "programdata_max_len"), 2_001),
        ):
            changed = copy.deepcopy(value)
            target: object = changed
            for key in path[:-1]:
                target = target[key]  # type: ignore[index]
            target[path[-1]] = replacement  # type: ignore[index]
            mutations.append(changed)
        for changed in mutations:
            summary = checker.validate_manifest(
                {
                    **changed,
                    "profile": {
                        **changed["profile"],  # type: ignore[arg-type]
                        "identity_sha256": checker.profile_identity(
                            changed["profile"]["name"],  # type: ignore[index]
                            changed["profile"]["label"],  # type: ignore[index]
                            changed["build_contract"],  # type: ignore[arg-type]
                            changed["capabilities"],  # type: ignore[arg-type]
                            changed["central_registry"],  # type: ignore[arg-type]
                            changed["wire_surface"],  # type: ignore[arg-type]
                            changed["artifact_budget"]["limits"],  # type: ignore[index,arg-type]
                        ),
                    },
                },
                repo=ROOT,
            )
            self.assertNotEqual(original, summary["profile_identity_sha256"])

    def test_identity_and_domain_separated_digest_bind_exact_wire_surface(self) -> None:
        value = manifest(linkage="linked")
        original_surface = value["wire_surface"]
        changed_surface = copy.deepcopy(original_surface)
        changed_surface["outer_request_actions"] = [0, 1]
        original_identity = value["profile"]["identity_sha256"]  # type: ignore[index]
        changed_identity = checker.profile_identity(
            value["profile"]["name"],  # type: ignore[index]
            value["profile"]["label"],  # type: ignore[index]
            value["build_contract"],  # type: ignore[arg-type]
            value["capabilities"],  # type: ignore[arg-type]
            value["central_registry"],  # type: ignore[arg-type]
            changed_surface,  # type: ignore[arg-type]
            value["artifact_budget"]["limits"],  # type: ignore[index,arg-type]
        )
        self.assertNotEqual(original_identity, changed_identity)
        self.assertNotEqual(
            checker.wire_surface_sha256(original_surface),  # type: ignore[arg-type]
            checker.wire_surface_sha256(changed_surface),  # type: ignore[arg-type]
        )

    def test_wire_surface_is_canonical_exhaustive_and_decoder_separated(self) -> None:
        value = manifest(linkage="linked")
        value["wire_surface"]["legacy_intent_pairs"].pop()  # type: ignore[index,union-attr]
        with self.assertRaisesRegex(checker.ProfileError, "do not exactly match"):
            checker.validate_manifest(value, repo=ROOT)

        value = manifest(linkage="linked")
        pair = value["wire_surface"]["legacy_intent_pairs"].pop(0)  # type: ignore[index,union-attr]
        value["wire_surface"]["dedicated_direct_intent_pairs"].append(pair)  # type: ignore[index,union-attr]
        with self.assertRaisesRegex(checker.ProfileError, "non-Direct tag"):
            checker.validate_manifest(value, repo=ROOT)

        value = manifest(linkage="linked")
        value["wire_surface"]["legacy_intent_pairs"].reverse()  # type: ignore[index,union-attr]
        with self.assertRaisesRegex(checker.ProfileError, "noncanonical intent-pair"):
            checker.validate_manifest(value, repo=ROOT)

    def test_release_wire_surface_refuses_both_legacy_source_generations(self) -> None:
        for tag, generation in ((23, 1), (70, 2)):
            with self.subTest(tag=tag):
                value = manifest(linkage="linked")
                source_owner = next(
                    row
                    for row in value["capabilities"]  # type: ignore[union-attr]
                    if row["slot"] == "source-plane"
                )
                source_owner["required_intent_triples"].append([tag, 3, 0])
                source_owner["required_intent_triples"].sort()
                value["central_registry"]["enabled_intent_triples"].append([tag, 3, 0])  # type: ignore[index,union-attr]
                value["central_registry"]["enabled_intent_triples"].sort()  # type: ignore[index,union-attr]
                value["wire_surface"]["legacy_intent_pairs"].append([tag, 3])  # type: ignore[index,union-attr]
                value["wire_surface"]["legacy_intent_pairs"].sort()  # type: ignore[index,union-attr]
                value["wire_surface"]["source_generation_discriminants"] = [generation]  # type: ignore[index]
                with self.assertRaisesRegex(
                    checker.ProfileError, "retains legacy Source authority"
                ):
                    checker.validate_manifest(value, repo=ROOT)

    def test_full_successor_source_surface_is_exactly_actions_one_through_four(self) -> None:
        for mutation in ("missing", "reserved"):
            with self.subTest(mutation=mutation):
                value = manifest(profile_feature="profile-full")
                source_owner = next(
                    row
                    for row in value["capabilities"]  # type: ignore[union-attr]
                    if row["slot"] == "source-plane"
                )
                triples = source_owner["required_intent_triples"]
                if mutation == "missing":
                    triples.remove([77, 2, 4])
                else:
                    triples.append([77, 2, 5])
                with self.assertRaisesRegex(
                    checker.ProfileError,
                    "must be exactly 77/v2 actions 1 through 4",
                ):
                    checker.validate_manifest(value, repo=ROOT)

    def test_all_eleven_semantic_owners_are_mandatory_and_ordered(self) -> None:
        value = manifest()
        value["capabilities"].pop()  # type: ignore[union-attr]
        with self.assertRaisesRegex(checker.ProfileError, "missing capability slots"):
            checker.validate_manifest(value, repo=ROOT)

        value = manifest()
        value["capabilities"][0], value["capabilities"][1] = (  # type: ignore[index]
            value["capabilities"][1],  # type: ignore[index]
            value["capabilities"][0],  # type: ignore[index]
        )
        with self.assertRaisesRegex(checker.ProfileError, "noncanonical order"):
            checker.validate_manifest(value, repo=ROOT)

    def test_missing_linked_intent_or_account_coverage_refuses(self) -> None:
        value = manifest(linkage="linked")
        value["central_registry"]["enabled_intent_triples"].pop()  # type: ignore[index,union-attr]
        with self.assertRaisesRegex(
            checker.ProfileError, "missing linked intent triples"
        ):
            checker.validate_manifest(value, repo=ROOT)

        value = manifest(linkage="linked")
        value["central_registry"]["linked_account_coordinates"].pop()  # type: ignore[index,union-attr]
        with self.assertRaisesRegex(
            checker.ProfileError, "missing linked account coordinates"
        ):
            checker.validate_manifest(value, repo=ROOT)

    def test_unowned_enabled_registry_rows_refuse(self) -> None:
        value = manifest()
        value["central_registry"]["enabled_intent_triples"] = [[99, 1, 1]]  # type: ignore[index]
        with self.assertRaisesRegex(
            checker.ProfileError, "lack a linked semantic owner"
        ):
            checker.validate_manifest(value, repo=ROOT)

    def test_model_only_profile_is_valid_planning_but_never_deployable(self) -> None:
        value = manifest()
        summary = checker.validate_manifest(value, repo=ROOT)
        self.assertEqual(
            summary["manifest_canonical_sha256"],
            checker.canonical_json_sha256(value),
        )
        self.assertEqual(summary["linked_capabilities"], [])
        self.assertEqual(
            summary["planned_capabilities"], list(checker.CAPABILITY_SLOTS)
        )
        self.assertFalse(summary["deployment_eligible"])
        with self.assertRaisesRegex(checker.ProfileError, "deployment eligibility"):
            checker.validate_manifest(manifest(), repo=ROOT, require_deployable=True)

    def test_linked_v2_measurement_can_qualify(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            value = manifest(
                linkage="linked",
                classification="deployable",
                measurement_class="linked",
                evidence_path="measurement.json",
                evidence_profile_name="test-profile",
            )
            (repo / "measurement.json").write_text(
                json.dumps(measurement_document(value)), encoding="utf-8"
            )
            summary = checker.validate_manifest(
                value, repo=repo, require_deployable=True
            )
            self.assertTrue(summary["deployment_eligible"])
            self.assertEqual(
                summary["measurement"]["persistent_loader_rent_lamports"], 4_674
            )
            self.assertEqual(
                summary["measurement"]["transient_buffer_rent_lamports"], 4_330
            )
            self.assertFalse(summary["measurement"]["exact_size_allocation"])

    def test_unavailable_v2_record_never_qualifies(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            value = manifest(
                linkage="linked",
                classification="deployable",
                measurement_class="linked",
                evidence_path="measurement.json",
                evidence_profile_name="test-profile",
            )
            evidence = measurement_document(value)
            evidence["availability"] = "unavailable"
            (repo / "measurement.json").write_text(
                json.dumps(evidence), encoding="utf-8"
            )
            with self.assertRaisesRegex(
                checker.ProfileError, "linked evidence is unavailable"
            ):
                checker.validate_manifest(value, repo=repo)

    def test_semantic_owner_and_registry_bodies_must_match_identity_manifest(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            value = manifest(
                linkage="linked",
                classification="deployable",
                measurement_class="linked",
                evidence_path="measurement.json",
                evidence_profile_name="test-profile",
            )
            evidence = measurement_document(value)
            evidence["profiles"][0]["semantic_owners"][0]["semantic_version"] = "forged-v2"  # type: ignore[index]
            (repo / "measurement.json").write_text(
                json.dumps(evidence), encoding="utf-8"
            )
            with self.assertRaisesRegex(
                checker.ProfileError, "semantic-owner manifest mismatch"
            ):
                checker.validate_manifest(value, repo=repo)

    def test_measurement_evidence_must_repeat_exact_wire_surface(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            value = manifest(
                linkage="linked",
                classification="deployable",
                measurement_class="linked",
                evidence_path="measurement.json",
                evidence_profile_name="test-profile",
            )
            evidence = measurement_document(value)
            evidence["profiles"][0]["wire_surface"]["outer_request_actions"] = [0, 1]
            (repo / "measurement.json").write_text(
                json.dumps(evidence), encoding="utf-8"
            )
            with self.assertRaisesRegex(
                checker.ProfileError, "wire-surface manifest mismatch"
            ):
                checker.validate_manifest(value, repo=repo)

    def test_producer_input_manifest_digest_is_recomputed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            value = manifest(
                linkage="linked",
                classification="deployable",
                measurement_class="linked",
                evidence_path="measurement.json",
                evidence_profile_name="test-profile",
            )
            evidence = measurement_document(value)
            evidence["profiles"][0]["identity_manifest_sha256"] = digest(  # type: ignore[index]
                "different-input-manifest"
            )
            (repo / "measurement.json").write_text(
                json.dumps(evidence), encoding="utf-8"
            )
            with self.assertRaisesRegex(
                checker.ProfileError, "producer identity-manifest"
            ):
                checker.validate_manifest(value, repo=repo)

    def test_tracked_or_untracked_dirty_source_evidence_refuses(self) -> None:
        for key in (
            "tracked_before",
            "untracked_before",
            "tracked_after",
            "untracked_after",
        ):
            with self.subTest(key=key), tempfile.TemporaryDirectory() as directory:
                repo = Path(directory)
                value = manifest(
                    linkage="linked",
                    classification="deployable",
                    measurement_class="linked",
                    evidence_path="measurement.json",
                    evidence_profile_name="test-profile",
                )
                evidence = measurement_document(value)
                evidence["source"]["cleanliness"][key] = ["dirty"]  # type: ignore[index]
                (repo / "measurement.json").write_text(
                    json.dumps(evidence), encoding="utf-8"
                )
                with self.assertRaisesRegex(
                    checker.ProfileError, "linked input closure is dirty"
                ):
                    checker.validate_manifest(value, repo=repo)

    def test_measurement_code_must_be_in_closure_with_exact_sha_and_blob_provenance(
        self,
    ) -> None:
        mutations = (
            ("bad-commit", lambda source: source.__setitem__("git_commit", "f")),
            ("zero-tree", lambda source: source.__setitem__("git_tree", "0" * 40)),
            ("mixed-oids", lambda source: source["measurement_code"][0].__setitem__(
                "git_blob_oid", digest("mixed-blob")
            )),
            ("outside-closure", lambda source: source["closure_paths"].remove(
                "programs/clutch-sbf/scripts/check_capability_profile.py"
            )),
            ("missing-code-row", lambda source: source["measurement_code"].pop()),
            ("bad-sha", lambda source: source["measurement_code"][0].__setitem__("sha256", "f")),
            ("bad-blob", lambda source: source["measurement_code"][0].__setitem__("git_blob_oid", "0" * 40)),
            ("swapped-role", lambda source: source["measurement_code"][0].__setitem__("role", "producer")),
        )
        for name, mutate in mutations:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                repo = Path(directory)
                value = manifest(
                    linkage="linked",
                    classification="deployable",
                    measurement_class="linked",
                    evidence_path="measurement.json",
                    evidence_profile_name="test-profile",
                )
                evidence = measurement_document(value)
                mutate(evidence["source"])
                (repo / "measurement.json").write_text(
                    json.dumps(evidence), encoding="utf-8"
                )
                with self.assertRaises(checker.ProfileError):
                    checker.validate_manifest(value, repo=repo)

    def test_linked_source_accepts_uniform_sha256_git_object_format(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            value = manifest(
                linkage="linked",
                classification="deployable",
                measurement_class="linked",
                evidence_path="measurement.json",
                evidence_profile_name="test-profile",
            )
            evidence = measurement_document(value)
            evidence["source"]["git_commit"] = digest("sha256-commit")
            evidence["source"]["git_tree"] = digest("sha256-tree")
            for index, row in enumerate(evidence["source"]["measurement_code"]):
                row["git_blob_oid"] = digest(f"sha256-blob-{index}")
            (repo / "measurement.json").write_text(
                json.dumps(evidence), encoding="utf-8"
            )
            summary = checker.validate_manifest(
                value, repo=repo, require_deployable=True
            )
            self.assertTrue(summary["deployment_eligible"])

    def test_loader_data_lengths_overhead_and_lifetimes_are_recomputed(self) -> None:
        mutations = (
            ("program", "data_len_bytes"),
            ("programdata", "storage_overhead_bytes"),
            ("buffer", "rent_exempt_lamports"),
        )
        for role, field in mutations:
            with self.subTest(
                role=role, field=field
            ), tempfile.TemporaryDirectory() as directory:
                repo = Path(directory)
                value = manifest(
                    linkage="linked",
                    classification="deployable",
                    measurement_class="linked",
                    evidence_path="measurement.json",
                    evidence_profile_name="test-profile",
                )
                evidence = measurement_document(value)
                for run in evidence["profiles"][0]["measurements"]:  # type: ignore[index]
                    run["loader"][role][field] += 1
                (repo / "measurement.json").write_text(
                    json.dumps(evidence), encoding="utf-8"
                )
                with self.assertRaisesRegex(checker.ProfileError, "mismatch"):
                    checker.validate_manifest(value, repo=repo)

    def test_mock_and_real_pyth_lab_identities_cannot_alias(self) -> None:
        mock = manifest(source_identity="non-production-mock-source-lab")
        real = manifest(source_identity="non-production-real-pyth-lab")
        mock_summary = checker.validate_manifest(mock, repo=ROOT)
        real_summary = checker.validate_manifest(real, repo=ROOT)
        self.assertNotEqual(
            mock_summary["profile_identity_sha256"],
            real_summary["profile_identity_sha256"],
        )
        self.assertEqual(
            mock_summary["cargo_features"],
            [
                "custom-heap",
                "profile-general-source-v2-point",
                "non-production-mock-source",
            ],
        )
        self.assertEqual(
            real_summary["cargo_features"],
            [
                "custom-heap",
                "profile-general-source-v2-point",
                "non-production-real-pyth-lab",
            ],
        )

    def test_nonproduction_source_identities_cannot_be_deployable(self) -> None:
        for source_identity in (
            "non-production-mock-source-lab",
            "non-production-real-pyth-lab",
        ):
            with self.subTest(source_identity=source_identity), self.assertRaisesRegex(
                checker.ProfileError, "non-production source identity cannot be deployable"
            ):
                checker.validate_manifest(
                    manifest(
                        source_identity=source_identity,
                        classification="deployable",
                    ),
                    repo=ROOT,
                )

    def test_source_identity_selector_registry_is_exact(self) -> None:
        self.assertEqual(
            checker.SOURCE_IDENTITY_FEATURE,
            {
                "production-inert": None,
                "runtime-real-pyth-release": None,
                "non-production-mock-source-lab": "non-production-mock-source",
                "non-production-real-pyth-lab": "non-production-real-pyth-lab",
            },
        )
        valid = checker.validate_build_contract(
            {
                "cargo_profile_feature": checker.SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE,
                "source_identity": "runtime-real-pyth-release",
                "collateral_release_identity": "production-inert",
                "expected_undefined_dynamic_symbols": SYSCALLS,
            }
        )
        self.assertEqual(
            checker.cargo_features(valid),
            ["custom-heap", checker.SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE],
        )
        with self.assertRaisesRegex(checker.ProfileError, "unknown class"):
            checker.validate_build_contract(
                {
                    "cargo_profile_feature": checker.SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE,
                    "source_identity": "caller-invented-release",
                    "collateral_release_identity": "production-inert",
                    "expected_undefined_dynamic_symbols": SYSCALLS,
                }
            )

    def test_runtime_real_pyth_release_never_enables_a_fixture_feature(self) -> None:
        inert = checker.validate_manifest(
            manifest(profile_feature="profile-full"), repo=ROOT
        )
        runtime = checker.validate_manifest(
            manifest(
                linkage="linked",
                profile_feature=checker.SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE,
                source_identity="runtime-real-pyth-release",
            ),
            repo=ROOT,
        )
        self.assertNotEqual(
            inert["profile_identity_sha256"], runtime["profile_identity_sha256"]
        )
        self.assertEqual(
            runtime["cargo_features"],
            ["custom-heap", checker.SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE],
        )

    def test_observed_release_selector_is_expressible_and_empty_rows_refuse(self) -> None:
        selected = checker.validate_build_contract(
            {
                "cargo_profile_feature": checker.SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE,
                "source_identity": "runtime-real-pyth-release",
                "collateral_release_identity": (
                    "observed-positive-collateral-and-claim-release"
                ),
                "expected_undefined_dynamic_symbols": SYSCALLS,
            }
        )
        self.assertEqual(
            checker.cargo_features(selected),
            [
                "custom-heap",
                checker.SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE,
                "observed-positive-collateral-release-manifest",
            ],
        )
        with self.assertRaisesRegex(
            checker.ProfileError,
            "observed-positive collateral release rows are absent or mismatched",
        ):
            checker.validate_manifest(
                manifest(
                    profile_feature=checker.SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE,
                    source_identity="runtime-real-pyth-release",
                    collateral_release_identity=(
                        "observed-positive-collateral-and-claim-release"
                    ),
                ),
                repo=ROOT,
            )

        with self.assertRaisesRegex(checker.ProfileError, "unknown class"):
            checker.validate_build_contract(
                {
                    "cargo_profile_feature": checker.SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE,
                    "source_identity": "runtime-real-pyth-release",
                    "collateral_release_identity": "caller-invented-release",
                    "expected_undefined_dynamic_symbols": SYSCALLS,
                }
            )

    def test_runtime_real_pyth_release_refuses_legacy_narrow_profile(self) -> None:
        with self.assertRaisesRegex(
            checker.ProfileError,
            "runtime real-Pyth release requires the chain-attached successor profile",
        ):
            checker.validate_manifest(
                manifest(
                    profile_feature="profile-general-source-v2-point",
                    source_identity="runtime-real-pyth-release",
                ),
                repo=ROOT,
            )

    def test_chain_attached_successor_requires_runtime_release_identity(self) -> None:
        for source_identity in (
            "production-inert",
            "non-production-mock-source-lab",
            "non-production-real-pyth-lab",
        ):
            with self.subTest(source_identity=source_identity), self.assertRaisesRegex(
                checker.ProfileError,
                "chain-attached successor requires the runtime real-Pyth release identity",
            ):
                checker.validate_manifest(
                    manifest(
                        linkage="linked",
                        profile_feature=checker.SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE,
                        source_identity=source_identity,
                    ),
                    repo=ROOT,
                )

    def test_chain_attached_successor_source_labs_are_compile_time_excluded(self) -> None:
        program = (ROOT / "programs/clutch-sbf/program/src/lib.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn('feature = "profile-successor-chain-attached-v1"', program)
        for feature in (
            "non-production-mock-source",
            "non-production-real-pyth-lab",
            "laboratory-fixtures",
        ):
            self.assertIn(f'feature = "{feature}"', program)
        self.assertIn(
            "the chain-attached successor cannot include legacy, mock, or real-Pyth Source laboratories",
            program,
        )

    def test_chain_attached_successor_wire_surface_is_exact(self) -> None:
        value = manifest(
            linkage="linked",
            profile_feature=checker.SUCCESSOR_CHAIN_ATTACHED_PROFILE_FEATURE,
            source_identity="runtime-real-pyth-release",
        )
        checked = checker.validate_manifest(value, repo=ROOT)
        self.assertEqual(
            checked["wire_surface"]["legacy_intent_pairs"],
            checker.SUCCESSOR_CHAIN_ATTACHED_LEGACY_INTENT_PAIRS,
        )
        self.assertEqual(
            checked["wire_surface"]["dedicated_direct_intent_pairs"],
            checker.SUCCESSOR_CHAIN_ATTACHED_DIRECT_INTENT_PAIRS,
        )
        self.assertEqual(
            checked["wire_surface"]["source_generation_discriminants"], [],
        )

        hostile = copy.deepcopy(value)
        hostile["wire_surface"]["legacy_intent_pairs"].append([69, 3])
        hostile["central_registry"]["enabled_intent_triples"].append([69, 3, 0])
        hostile["central_registry"]["enabled_intent_triples"].sort()
        hostile["capabilities"][0]["required_intent_triples"].append([69, 3, 0])
        hostile["profile"]["identity_sha256"] = checker.profile_identity(
            hostile["profile"]["name"],
            hostile["profile"]["label"],
            hostile["build_contract"],
            hostile["capabilities"],
            hostile["central_registry"],
            hostile["wire_surface"],
            hostile["artifact_budget"]["limits"],
        )
        with self.assertRaisesRegex(
            checker.ProfileError,
            "chain-attached successor legacy intent set is not exact",
        ):
            checker.validate_manifest(hostile, repo=ROOT)

    def test_fractional_semantic_owner_and_all_or_none_enablement(self) -> None:
        value = manifest()
        fractional = next(
            row
            for row in value["capabilities"]
            if row["slot"] == "fractional-redemption"
        )
        self.assertEqual(
            [triple for triple in fractional["required_intent_triples"] if triple[0] == 79],
            [],
        )
        checker.validate_manifest(value, repo=ROOT)

        fully_enabled = copy.deepcopy(value)
        full_fractional = next(
            row
            for row in fully_enabled["capabilities"]
            if row["slot"] == "fractional-redemption"
        )
        full_fractional["required_intent_triples"].extend(
            copy.deepcopy(checker.CURRENT_FRACTIONAL_EXTENSION_TRIPLES)
        )
        full_fractional["required_intent_triples"].sort()
        fully_enabled["build_contract"]["collateral_release_identity"] = (
            "observed-positive-collateral-and-claim-release"
        )
        fully_enabled["central_registry"] = linked_coverage(
            fully_enabled["capabilities"]
        )
        fully_enabled["wire_surface"] = wire_surface(
            fully_enabled["central_registry"]
        )
        fully_enabled["profile"]["identity_sha256"] = checker.profile_identity(
            fully_enabled["profile"]["name"],
            fully_enabled["profile"]["label"],
            fully_enabled["build_contract"],
            fully_enabled["capabilities"],
            fully_enabled["central_registry"],
            fully_enabled["wire_surface"],
            fully_enabled["artifact_budget"]["limits"],
        )
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            manifest_path = repo / checker.OBSERVED_RELEASE_MANIFEST_PATH
            manifest_path.parent.mkdir(parents=True)
            manifest_path.write_text(
                """
static OBSERVED_COLLATERAL_RELEASES_V2: [AdapterReleaseV2; 1] = [RELEASE];
static OBSERVED_COLLATERAL_RELEASE_MANIFESTS_V2:
    [CompiledCollateralReleaseManifestV2; 1] = [MANIFEST];
const OBSERVED_CLAIM_ISSUANCE_RELEASE_V1:
    Option<CompiledClaimIssuanceReleaseV1> = Some(CLAIM);
""",
                encoding="utf-8",
            )
            checker.validate_manifest(fully_enabled, repo=repo)

        partial_required = copy.deepcopy(value)
        partial_fractional = next(
            row
            for row in partial_required["capabilities"]
            if row["slot"] == "fractional-redemption"
        )
        partial_fractional["required_intent_triples"].append([79, 1, 1])
        partial_fractional["required_intent_triples"].sort()
        partial_required["central_registry"] = linked_coverage(
            partial_required["capabilities"]
        )
        partial_required["wire_surface"] = wire_surface(
            partial_required["central_registry"]
        )
        partial_required["profile"]["identity_sha256"] = checker.profile_identity(
            partial_required["profile"]["name"],
            partial_required["profile"]["label"],
            partial_required["build_contract"],
            partial_required["capabilities"],
            partial_required["central_registry"],
            partial_required["wire_surface"],
            partial_required["artifact_budget"]["limits"],
        )
        with self.assertRaisesRegex(
            checker.ProfileError, "exactly 79/v1 actions 1 through 10"
        ):
            checker.validate_manifest(partial_required, repo=ROOT)

        wrong_owner = copy.deepcopy(value)
        wrong_owner["capabilities"][0]["required_intent_triples"].append([79, 1, 1])
        wrong_owner["capabilities"][0]["required_intent_triples"].sort()
        wrong_owner["central_registry"] = linked_coverage(wrong_owner["capabilities"])
        wrong_owner["wire_surface"] = wire_surface(wrong_owner["central_registry"])
        wrong_owner["profile"]["identity_sha256"] = checker.profile_identity(
            wrong_owner["profile"]["name"],
            wrong_owner["profile"]["label"],
            wrong_owner["build_contract"],
            wrong_owner["capabilities"],
            wrong_owner["central_registry"],
            wrong_owner["wire_surface"],
            wrong_owner["artifact_budget"]["limits"],
        )
        with self.assertRaisesRegex(
            checker.ProfileError, "non-Fractional semantic owner"
        ):
            checker.validate_manifest(wrong_owner, repo=ROOT)

    def test_full_profile_records_cargo_default_identity_marker(self) -> None:
        full = checker.validate_manifest(
            manifest(profile_feature="profile-full"), repo=ROOT
        )
        direct = checker.validate_manifest(
            manifest(profile_feature="profile-direct-v3-source-v2-point"), repo=ROOT
        )
        full_lab = checker.validate_manifest(
            manifest(
                profile_feature="profile-full",
                source_identity="non-production-real-pyth-lab",
            ),
            repo=ROOT,
        )
        self.assertEqual(
            full["cargo_features"],
            ["custom-heap", "default", "profile-full"],
        )
        self.assertEqual(
            direct["cargo_features"],
            ["custom-heap", "profile-direct-v3-source-v2-point"],
        )
        self.assertEqual(
            full_lab["cargo_features"],
            [
                "custom-heap",
                "default",
                "profile-full",
                "non-production-real-pyth-lab",
            ],
        )

    def test_default_equals_explicit_full_only_under_the_same_identity(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            value = manifest(
                linkage="linked",
                classification="deployable",
                measurement_class="linked",
                evidence_path="measurement.json",
                evidence_profile_name="test-profile",
                profile_feature="profile-full",
            )
            evidence = measurement_document(value)
            (repo / "measurement.json").write_text(
                json.dumps(evidence), encoding="utf-8"
            )
            checker.validate_manifest(value, repo=repo, require_deployable=True)

            evidence["profiles"][0]["default_feature_equivalence"][  # type: ignore[index]
                "capability_profile_identity_sha256"
            ] = digest(
                "other-identity"
            )
            (repo / "measurement.json").write_text(
                json.dumps(evidence), encoding="utf-8"
            )
            with self.assertRaisesRegex(
                checker.ProfileError, "identity-manifest mismatch"
            ):
                checker.validate_manifest(value, repo=repo)

    def test_same_elf_hash_backend_or_frame_mismatch_cannot_pass_default_gate(self) -> None:
        mutations = (
            ("backend_stack_diagnostic_lines", 3, "backend_stack_diagnostic_lines"),
            (
                "final_frame_audit",
                {
                    **measurement_run(3, build_mode="cargo-default")["final_frame_audit"],
                    "deepest_direct_r10_offset": 512,
                },
                "final_frame_audit",
            ),
        )
        for key, changed, message in mutations:
            with self.subTest(key=key), tempfile.TemporaryDirectory() as directory:
                repo = Path(directory)
                value = manifest(
                    linkage="linked",
                    classification="deployable",
                    measurement_class="linked",
                    evidence_path="measurement.json",
                    evidence_profile_name="test-profile",
                    profile_feature="profile-full",
                )
                evidence = measurement_document(value)
                default = evidence["profiles"][0]["default_feature_equivalence"]["measurement"]
                self.assertEqual(default["elf_sha256"], evidence["profiles"][0]["measurements"][0]["elf_sha256"])
                default[key] = changed
                (repo / "measurement.json").write_text(
                    json.dumps(evidence), encoding="utf-8"
                )
                with self.assertRaisesRegex(checker.ProfileError, message):
                    checker.validate_manifest(value, repo=repo)

    def test_lab_full_profile_cannot_reuse_default_equivalence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            value = manifest(
                linkage="linked",
                classification="deployable",
                measurement_class="linked",
                evidence_path="measurement.json",
                evidence_profile_name="test-profile",
                profile_feature="profile-full",
                source_identity="non-production-real-pyth-lab",
            )
            evidence = measurement_document(value)
            evidence["profiles"][0]["default_feature_equivalence"] = {  # type: ignore[index]
                "capability_profile_identity_sha256": value["profile"]["identity_sha256"],  # type: ignore[index]
                "measurement": measurement_run(3, build_mode="cargo-default"),
                "matches_explicit": True,
            }
            (repo / "measurement.json").write_text(
                json.dumps(evidence), encoding="utf-8"
            )
            with self.assertRaisesRegex(
                checker.ProfileError, "distinct profile/lab identity"
            ):
                checker.validate_manifest(value, repo=repo)

    def test_historical_v1_measurement_remains_comparison_only(self) -> None:
        evidence = json.loads((ROOT / HISTORICAL_EVIDENCE).read_text(encoding="utf-8"))
        selected = next(
            profile for profile in evidence["profiles"] if profile["name"] == "full"
        )
        measured = selected["measurements"][0]
        value = manifest(
            measurement_class="historical",
            evidence_path=str(HISTORICAL_EVIDENCE),
            evidence_profile_name="full",
            budget_limits={
                "max_elf_bytes": measured["elf_bytes"],
                "max_text_bytes": measured["text_bytes"],
                "programdata_max_len": measured["elf_bytes"],
                "max_persistent_loader_rent_lamports": measured[
                    "total_loader_rent_lamports"
                ],
            },
        )
        summary = checker.validate_manifest(value, repo=ROOT)
        self.assertTrue(summary["budget_evaluated"])
        self.assertFalse(summary["deployment_eligible"])

    def test_duplicate_json_object_key_refuses_at_parse_time(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "duplicate.json"
            path.write_text('{"schema":"one","schema":"two"}', encoding="utf-8")
            with self.assertRaisesRegex(checker.ProfileError, "duplicate object key"):
                checker.load_json(path)


if __name__ == "__main__":
    unittest.main()
