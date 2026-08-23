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


def capabilities(linkage: str = "planned") -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    for index, (slot, owner) in enumerate(checker.CAPABILITY_OWNERS, start=1):
        rows.append(
            {
                "slot": slot,
                "owner": owner,
                "linkage": linkage,
                "semantic_version": f"{slot}-test-v1",
                "semantic_digest_sha256": digest(slot),
                "required_intent_triples": [[index, 3, 0]],
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
    profile_feature: str = "profile-general-source-v2-point",
    budget_limits: dict[str, int] | None = None,
) -> dict[str, object]:
    rows = capabilities(linkage)
    registry = linked_coverage(rows)
    declared_limits = limits() if budget_limits is None else budget_limits
    name = "test-profile"
    label = "dragons-clutch/capability-profile/test-fixture/v2"
    build_contract = {
        "cargo_profile_feature": profile_feature,
        "source_identity": source_identity,
        "expected_undefined_dynamic_symbols": SYSCALLS,
    }
    identity = checker.profile_identity(
        name, label, build_contract, rows, registry, declared_limits
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
                "cargo_features": checker.cargo_features(contract),  # type: ignore[arg-type]
                "capability_profile_identity_sha256": identity,
                "identity_manifest_sha256": checker.measurement_input_manifest_sha256(
                    value
                ),
                "semantic_owners": copy.deepcopy(value["capabilities"]),
                "central_registry": copy.deepcopy(value["central_registry"]),
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
                            changed["artifact_budget"]["limits"],  # type: ignore[index,arg-type]
                        ),
                    },
                },
                repo=ROOT,
            )
            self.assertNotEqual(original, summary["profile_identity_sha256"])

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
        summary = checker.validate_manifest(manifest(), repo=ROOT)
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
        self.assertIn("non-production-mock-source", mock_summary["cargo_features"])
        self.assertIn("non-production-real-pyth-lab", real_summary["cargo_features"])

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
