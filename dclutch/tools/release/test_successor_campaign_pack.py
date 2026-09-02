from __future__ import annotations

import argparse
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock
from unittest import mock


MODULE_PATH = Path(__file__).with_name("successor_campaign_pack.py")
SPEC = importlib.util.spec_from_file_location("successor_campaign_pack", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
pack_tool = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(pack_tool)


class SuccessorCampaignPackTests(unittest.TestCase):
    def test_the_gate_link_count_is_the_shipped_set_and_not_a_restated_number(self) -> None:
        """The count `e6b7bf1a` moved, and nothing here was watching.

        This module carried the literal 13 in three places. The shipped set went
        to twelve when `dclutch-dealer-sbf` was deleted; the two Rust readers
        were swept in `aa7f8892` and the shell gate in `0f0ec379`, and these were
        the ones left. Nothing went red -- a full release candidate built every
        link, emitted its checked Upgrade gate, and refused at the last stage.
        """
        import importlib.util as _util

        provenance_path = MODULE_PATH.with_name("artifact_provenance.py")
        spec = _util.spec_from_file_location("artifact_provenance_probe", provenance_path)
        assert spec is not None and spec.loader is not None
        provenance = _util.module_from_spec(spec)
        spec.loader.exec_module(provenance)
        shipped = len(provenance.SHIPPED_LINKS)

        # The authority is the set in artifact_provenance, read here rather than
        # restated, so a link appearing or disappearing moves both together.
        self.assertEqual(pack_tool.SHIPPED_LINK_COUNT, shipped)
        self.assertEqual(
            pack_tool.SHIPPED_LABELS,
            frozenset(label for label, _p, _a in provenance.SHIPPED_LINKS),
        )
        self.assertEqual(
            pack_tool.ARTIFACT_ROLES,
            tuple(label for label, _p, produces in provenance.SHIPPED_LINKS if produces),
        )
        # The deleted program is gone from every one of them. This is the exact
        # residue `e6b7bf1a` left: a label set naming a program that no longer
        # ships, which refuses every candidate built after the deletion.
        self.assertNotIn("dclutch-dealer-sbf", pack_tool.SHIPPED_LABELS)
        self.assertNotIn("dclutch-dealer-sbf", pack_tool.ARTIFACT_ROLES)

        exact = {"link_count": shipped, "links": [{"label": f"l{i}"} for i in range(shipped)]}
        self.assertEqual(pack_tool.require_shipped_link_count(exact), exact["links"])

        for wrong in (shipped - 1, shipped + 1):
            with self.assertRaises(pack_tool.Refusal):
                pack_tool.require_shipped_link_count(
                    {"link_count": wrong, "links": [{"label": f"l{i}"} for i in range(wrong)]}
                )
        # A gate whose declared count disagrees with its own list refuses too.
        with self.assertRaises(pack_tool.Refusal):
            pack_tool.require_shipped_link_count(
                {"link_count": shipped, "links": [{"label": "l0"}]}
            )
        # A string of exactly the right LENGTH still is not a link list, and
        # only the type check can say so -- the count check is satisfied by it.
        not_a_list = "x" * shipped
        self.assertEqual(len(not_a_list), shipped)
        with self.assertRaises(pack_tool.Refusal):
            pack_tool.require_shipped_link_count({"link_count": shipped, "links": not_a_list})

    def test_zero_public_key_has_canonical_base58(self) -> None:
        self.assertEqual(pack_tool.base58_32("00" * 32), "1" * 32)

    def test_json_duplicate_key_refuses(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "duplicate.json"
            path.write_text('{"same":1,"same":2}\n')
            with self.assertRaises(pack_tool.Refusal):
                pack_tool.read_json(path, "duplicate fixture")

    def test_kv_allows_only_named_repeatable_fields(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "projection.txt"
            path.write_text("format=one\nassumption=first\nassumption=second\n")
            self.assertEqual(
                pack_tool.read_kv(
                    path,
                    "projection",
                    repeatable=frozenset({"assumption"}),
                )["assumption"],
                "first",
            )
            with self.assertRaises(pack_tool.Refusal):
                pack_tool.read_kv(path, "projection")

    def test_evidence_rehash_refuses_one_byte_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            path = root / "fact"
            path.write_bytes(b"exact")
            recorded = pack_tool.evidence(root, "fact", "fact")
            pack_tool.verify_evidence(root, recorded, "fact")
            path.write_bytes(b"Exact")
            with self.assertRaises(pack_tool.Refusal):
                pack_tool.verify_evidence(root, recorded, "fact")

    def test_evidence_refuses_symlink_alias(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            (root / "real").write_bytes(b"fact")
            (root / "alias").symlink_to(root / "real")
            with self.assertRaises(pack_tool.Refusal):
                pack_tool.evidence(root, "alias", "aliased fact")

    def test_host_substrate_must_execute_source_pinned_rustc(self) -> None:
        summary = {
            "host_rustc_version": "rustc 1.97.1 (fixture)",
            "host_rustc_verbose_sha256": "12" * 32,
            "host_cargo_version": "cargo 1.97.1 (fixture)",
            "host_cc_version": "cc fixture",
            "host_linker_version": "ld fixture",
            "host_libc_version": "libc fixture",
            "host_os": "Linux",
            "host_arch": "x86_64",
            "host_kernel": "fixture-kernel",
        }
        self.assertEqual(
            pack_tool.host_substrate_value(summary, "1.97.1")["arch"],
            "x86_64",
        )
        summary["host_rustc_version"] = "rustc 1.96.0 (substituted)"
        with self.assertRaisesRegex(pack_tool.Refusal, "source toolchain pin"):
            pack_tool.host_substrate_value(summary, "1.97.1")

    def test_resolution_identity_is_derived_from_source_preimage(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source = root / "source/crates/dclutch-resolution-codec/src"
            source.mkdir(parents=True)
            source.joinpath("lib.rs").write_text(
                'pub const RESOLUTION_CONTROLLER_RELEASE_PREIMAGE_V4: &[u8] =\n'
                '    b"one/exact/release";\n'
            )
            self.assertEqual(
                pack_tool.resolution_semantic_id(root),
                pack_tool.sha256_bytes(b"one/exact/release"),
            )

    def materialization_fixture(self, root: Path) -> tuple[Path, dict]:
        pack_path = root / pack_tool.PACK_BASENAME
        pack_path.write_text("{}\n")
        (root / "source/tools/local-validator").mkdir(parents=True)
        launcher = root / "source/tools/local-validator/dclutch-successor-validator"
        launcher.write_text("#!/bin/sh\n")
        elf_dir = root / "elf"
        log_dir = root / "logs"
        elf_dir.mkdir()
        log_dir.mkdir()
        roles = []
        for index, role in enumerate(pack_tool.CAMPAIGN_ROLES, start=1):
            elf = elf_dir / f"{role}.so"
            log = log_dir / f"{role}.log"
            elf.write_bytes(b"\x7fELF" + bytes([index]))
            log.write_text(f"build {role}\n")
            roles.append(
                {
                    "role": role,
                    "spec_key": "rent_credit" if role == "rent" else role,
                    "program_id": pack_tool.base58_32(f"{index:02x}" * 32),
                    "semantic_release_id": f"{index + 16:02x}" * 32,
                    "semantic_source": "fixture",
                    "elf": pack_tool.evidence(root, f"elf/{role}.so", f"{role} ELF"),
                    "build_log": pack_tool.evidence(root, f"logs/{role}.log", f"{role} log"),
                    "package": f"dclutch-{role}-sbf",
                }
            )
        pack = {
            "source": {"revision": "12" * 20, "tree_sha256": "34" * 32},
            "toolchains": {
                "cargo_build_sbf": "cargo-build-sbf 4.0.0",
                "platform_tools": "v1.53",
                "sbf_rustc": "rustc 1.89.0",
                "solana_cli": "solana-cli 4.0.2",
            },
            "campaign": {
                "launcher": pack_tool.evidence(
                    root,
                    "source/tools/local-validator/dclutch-successor-validator",
                    "launcher",
                ),
                "roles": roles,
            },
        }
        return pack_path, pack

    def test_materialized_spec_is_directly_bound_to_pack_roles(self) -> None:
        with tempfile.TemporaryDirectory(dir="/private/tmp") as directory:
            root = Path(directory).resolve()
            pack_path, pack = self.materialization_fixture(root)
            market = root / "market.json"
            market.write_text('{"fixture":"market"}\n')
            run_root = root / "run"
            arguments = argparse.Namespace(
                pack=str(pack_path),
                market=str(market),
                run_root=str(run_root),
                rpc_port=31890,
                record_publication="transaction",
            )
            with mock.patch.object(pack_tool, "verify_pack", return_value=(root, pack)):
                pack_tool.materialize(arguments)
            spec = json.loads((run_root / "spec.json").read_text())
            self.assertEqual(spec["schema"], pack_tool.SPEC_SCHEMA)
            self.assertEqual(spec["rpc_url"], "http://127.0.0.1:31890/")
            self.assertEqual(spec["registry"]["genesis_deployment_slot"], 11)
            self.assertEqual(spec["rent_credit"]["genesis_deployment_slot"], 31)
            for role in pack["campaign"]["roles"]:
                item = spec[role["spec_key"]]
                self.assertEqual(item["elf_sha256"], role["elf"]["sha256"])
                self.assertEqual(item["semantic_release_id"], role["semantic_release_id"])

    def test_materialized_spec_refuses_role_substitution(self) -> None:
        with tempfile.TemporaryDirectory(dir="/private/tmp") as directory:
            root = Path(directory).resolve()
            pack_path, pack = self.materialization_fixture(root)
            market = root / "market.json"
            market.write_text('{"fixture":"market"}\n')
            run_root = root / "run"
            arguments = argparse.Namespace(
                pack=str(pack_path),
                market=str(market),
                run_root=str(run_root),
                rpc_port=31890,
                record_publication="genesis",
            )
            with mock.patch.object(pack_tool, "verify_pack", return_value=(root, pack)):
                pack_tool.materialize(arguments)
            spec_path = run_root / "spec.json"
            spec = json.loads(spec_path.read_text())
            spec["trading"]["elf_sha256"] = "ff" * 32
            spec_path.write_text(json.dumps(spec) + "\n")
            with self.assertRaises(pack_tool.Refusal):
                pack_tool.validate_materialized_spec(root, pack_path, pack, spec_path)

    def lineage_fixture(
        self, root: Path, pack_path: Path, pack: dict
    ) -> Path:
        gate = root / "CHECKED_UPGRADE_GATE.json"
        gate.write_text('{"fixture":"gate"}\n')
        campaign_evidence = root / "campaign-evidence.json"
        campaign_evidence.write_text('{"fixture":"campaign"}\n')
        pack["checked_upgrade_gate"] = pack_tool.evidence(
            root, "CHECKED_UPGRADE_GATE.json", "gate"
        )
        pack["source"] = {
            "revision": "12" * 20,
            "tree_sha256": "34" * 32,
        }
        pack["toolchains"]["solana_cli"] = "solana-cli 4.0.2"
        pack["release"] = {
            "execution_release_set_id": "41" * 32,
            "checked_execution_release_set_id": "42" * 32,
            "checked_execution_release_set": {
                "canonical_path": "unused",
                "bytes": 1,
                "sha256": "43" * 32,
            },
        }
        checked = []
        for role in pack["campaign"]["roles"]:
            checked.append(
                {
                    "role": role["role"],
                    "program": role["program_id"],
                    "programData": pack_tool.base58_32("77" * 32),
                    "checkedCandidateElfPath": str(
                        root / role["elf"]["canonical_path"]
                    ),
                    "checkedCandidateElfSha256": role["elf"]["sha256"],
                    "genesisLiveElfSha256": role["elf"]["sha256"],
                    "genesisProgramDataAccountSha256": "78" * 32,
                    "genesisDeploymentSlot": 10 + len(checked),
                    "semanticReleaseId": role["semantic_release_id"],
                }
            )
        lineage = {
            "schema": pack_tool.LINEAGE_SCHEMA,
            "evidenceLevel": "local-validator-finalized-chain-state",
            "cluster": "owned-loopback",
            "genesisHash": pack_tool.base58_32("55" * 32),
            "planSha256": "56" * 32,
            "campaignEvidencePath": str(campaign_evidence),
            "source": {
                "revision": pack["source"]["revision"],
                "treeSha256": pack["source"]["tree_sha256"],
                "checkedReleaseGatePath": str(gate),
                "checkedReleaseGateSha256": pack["checked_upgrade_gate"]["sha256"],
                "checkedLocalMutableSetSha256": "57" * 32,
                "solanaCliVersion": pack["toolchains"]["solana_cli"],
            },
            "checkedArtifacts": checked,
            "profiles": {
                "predecessorV1": {
                    "address": pack_tool.base58_32("58" * 32),
                    "account": {},
                    "registryArtifactReleaseId": "59" * 32,
                    "rentArtifactReleaseId": "5a" * 32,
                },
                "successorV2": {
                    "address": pack_tool.base58_32("5b" * 32),
                    "account": {},
                    "registryArtifactReleaseId": "5c" * 32,
                    "rentArtifactReleaseId": "5a" * 32,
                    "predecessorRegistryArtifactReleaseId": "59" * 32,
                    "predecessorRentArtifactReleaseId": "5a" * 32,
                },
                "v1PreservedByteIdentical": True,
            },
            "artifactLineage": {},
            "activation": {
                "releaseSetId": pack["release"]["execution_release_set_id"],
                "checkedExecutionReleaseSetId": pack["release"][
                    "checked_execution_release_set_id"
                ],
                "checkedMultiprogramEnvelopeSha256": pack["release"][
                    "checked_execution_release_set"
                ]["sha256"],
                "account": {},
                "roles": [],
            },
            "migration": {},
        }
        lineage_path = root / "infrastructure-lineage.json"
        lineage_path.write_text(json.dumps(lineage) + "\n")
        return lineage_path

    def test_lineage_binding_joins_exact_pack_source_roles_and_profiles(self) -> None:
        with tempfile.TemporaryDirectory(dir="/private/tmp") as directory:
            root = Path(directory).resolve()
            pack_path, pack = self.materialization_fixture(root)
            lineage_path = self.lineage_fixture(root, pack_path, pack)
            binding = pack_tool.lineage_binding_value(
                root, pack_path, pack, lineage_path
            )
            self.assertEqual(binding["schema"], pack_tool.LINEAGE_BINDING_SCHEMA)
            self.assertEqual(
                binding["execution_release_set_id"],
                pack["release"]["execution_release_set_id"],
            )
            self.assertTrue(binding["profiles"]["v1_preserved_byte_identical"])

    def test_lineage_binding_refuses_substituted_checked_elf(self) -> None:
        with tempfile.TemporaryDirectory(dir="/private/tmp") as directory:
            root = Path(directory).resolve()
            pack_path, pack = self.materialization_fixture(root)
            lineage_path = self.lineage_fixture(root, pack_path, pack)
            lineage = json.loads(lineage_path.read_text())
            lineage["checkedArtifacts"][3]["checkedCandidateElfSha256"] = "ff" * 32
            lineage_path.write_text(json.dumps(lineage) + "\n")
            with self.assertRaises(pack_tool.Refusal):
                pack_tool.lineage_binding_value(root, pack_path, pack, lineage_path)

    def reproduction_fixture(self) -> dict:
        artifacts = []
        for index, role in enumerate(pack_tool.ARTIFACT_ROLES, start=1):
            artifacts.append(
                {
                    "role": role,
                    "package": f"package-{role}",
                    "program_id_hex": f"{index:02x}" * 32,
                    "semantic_release_id": f"{index + 16:02x}" * 32,
                    "checked_release_id": f"{index + 32:02x}" * 32,
                    "elf": {"bytes": index, "sha256": f"{index + 48:02x}" * 32},
                    "checked_manifest": {
                        "bytes": index + 100,
                        "sha256": f"{index + 64:02x}" * 32,
                    },
                }
            )
        frames = [
            {
                "label": f"link-{index}",
                "package": f"package-{index}",
                "frame_count": index + 1,
                "deepest_frame_bytes": 1000 + index,
            }
            for index in range(13)
        ]
        return {
            "source": {
                "revision": "11" * 20,
                "tree_sha256": "12" * 32,
                "root_cargo_lock_sha256": "13" * 32,
                "cargo_lock_set_sha256": "14" * 32,
            },
            "toolchains": {
                "host_rust_channel": "1.97.1",
                "sbf_rustc": "rustc 1.89.0",
                "solana_cli": "solana-cli 4.0.2",
                "cargo_build_sbf": "cargo-build-sbf 4.0.0",
                "platform_tools": "1.53",
                "target_triple": "sbpf-solana-solana",
                "actual_builder": "local",
                "actual_builder_scheduler": "direct",
                "host_substrate": {
                    "rustc": "rustc 1.97.1 (fixture)",
                    "rustc_verbose_sha256": "18" * 32,
                    "cargo": "cargo 1.97.1 (fixture)",
                    "cc": "cc fixture",
                    "linker": "ld fixture",
                    "libc": "libc fixture",
                    "os": "Linux",
                    "arch": "x86_64",
                    "kernel": "fixture-kernel",
                },
                "node": {
                    "node_version": "v26.4.0",
                    "npm_version": "11.17.0",
                    "archive_source": "https://nodejs.org/dist/v26.4.0/node-v26.4.0-linux-x64.tar.xz",
                    "archive": {
                        "canonical_path": "toolchain/node-v26.4.0-linux-x64.tar.xz",
                        "bytes": 1,
                        "sha256": "19" * 32,
                    },
                    "node_binary_sha256": "1a" * 32,
                    "npm_cli_sha256": "1b" * 32,
                },
            },
            "artifacts": artifacts,
            "release": {
                "execution_release_set_id": "21" * 32,
                "checked_execution_release_set_id": "22" * 32,
                "execution_release_set": {"sha256": "23" * 32},
                "checked_execution_release_set": {"sha256": "24" * 32},
                "predecessor_infrastructure_profile_sha256": "29" * 32,
                "infrastructure_profile_sha256": "25" * 32,
                "infrastructure_profile_pda_hex": "26" * 32,
                "checked_infrastructure_id": "27" * 32,
                "checked_infrastructure": {"sha256": "28" * 32},
            },
            "ceilings": {
                "compute_units": 1_400_000,
                "packet_bytes": 1232,
                "frame_bytes": 4096,
                "frames": frames,
            },
            "compliance": {
                "repository_license": "AGPL-3.0-or-later",
                "workspace_manifest": {"sha256": "31" * 32},
                "sbom": {"sha256": "32" * 32},
                "notices": {"sha256": "33" * 32},
                "sbom_verifier": {"sha256": "34" * 32},
            },
            "product_handoff": {
                "schema": "dclutch/product-spline-handoff-smoke/v1",
                "fixture_sha256": "39" * 32,
                "compiler_report_sha256": "3a" * 32,
                "semantic_basis_id": "3b" * 32,
                "found_records": {
                    "productRecord": "one",
                    "resultDomainRecord": "two",
                    "portfolioRecord": "three",
                    "linkedBasisRecord": "four",
                    "priceGateRecord": "five",
                },
                "source": {
                    "runner": {"sha256": "3c" * 32},
                    "verifier": {"sha256": "3d" * 32},
                    "fixture": {"sha256": "3e" * 32},
                    "sdk_lock": {"sha256": "3f" * 32},
                    "cli_lock": {"sha256": "40" * 32},
                    "successor_lock": {"sha256": "41" * 32},
                },
                "build": {
                    "cli_bundle": {"sha256": "42" * 32},
                    "successor": {"sha256": "43" * 32},
                },
                "execution": {
                    "products": {
                        "portfolio.bin": {"sha256": "44" * 32},
                        "price-gate.bin": {"sha256": "45" * 32},
                        "product-basis.bin": {"sha256": "46" * 32},
                        "product.bin": {"sha256": "47" * 32},
                        "result-domain.bin": {"sha256": "48" * 32},
                    },
                },
            },
            "verifier": {
                "pack": {"sha256": "35" * 32},
                "artifact_provenance": {"sha256": "36" * 32},
                "public_route_campaign": {"sha256": "37" * 32},
                "devnet_direct_lifecycle": {"sha256": "38" * 32},
            },
        }

    def test_reproduction_projection_excludes_only_declared_host_nondeterminism(self) -> None:
        left = self.reproduction_fixture()
        right = copy.deepcopy(left)
        right["toolchains"]["actual_builder"] = "hbox"
        right["toolchains"]["actual_builder_scheduler"] = "swarm-build"
        right["toolchains"]["host_substrate"]["cc"] = "independent cc"
        right["toolchains"]["host_substrate"]["kernel"] = "independent kernel"
        right["product_handoff"]["build"]["successor"]["sha256"] = "ef" * 32
        self.assertEqual(
            pack_tool.reproduction_projection(left),
            pack_tool.reproduction_projection(right),
        )
        right["artifacts"][0]["elf"]["sha256"] = "ff" * 32
        self.assertNotEqual(
            pack_tool.reproduction_projection(left),
            pack_tool.reproduction_projection(right),
        )
        right = copy.deepcopy(left)
        right["toolchains"]["node"]["archive"]["sha256"] = "ee" * 32
        self.assertNotEqual(
            pack_tool.reproduction_projection(left),
            pack_tool.reproduction_projection(right),
        )
        right = copy.deepcopy(left)
        right["product_handoff"]["build"]["cli_bundle"]["sha256"] = "dd" * 32
        self.assertNotEqual(
            pack_tool.reproduction_projection(left),
            pack_tool.reproduction_projection(right),
        )

    def test_reproduction_report_is_recomputed_from_both_packs(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(strict=True)
            left_path = root / "left" / pack_tool.PACK_BASENAME
            right_path = root / "right" / pack_tool.PACK_BASENAME
            left_path.parent.mkdir()
            right_path.parent.mkdir()
            left_path.write_text("{}\n")
            right_path.write_text("{}\n")
            output = root / "reproduction.json"
            left = self.reproduction_fixture()
            right = copy.deepcopy(left)
            right["toolchains"]["actual_builder"] = "hbox"
            right["toolchains"]["actual_builder_scheduler"] = "swarm-build"

            def verified(path: Path) -> tuple[Path, dict]:
                if path == left_path:
                    return left_path.parent, left
                if path == right_path:
                    return right_path.parent, right
                raise AssertionError(f"unexpected pack path {path}")

            with mock.patch.object(pack_tool, "verify_pack", side_effect=verified):
                pack_tool.compare_packs(
                    argparse.Namespace(
                        left=str(left_path), right=str(right_path), output=str(output)
                    )
                )
                pack_tool.verify_reproduction(argparse.Namespace(report=str(output)))

                report = json.loads(output.read_text())
                report["right"]["builder"] = "persvati"
                output.write_bytes(pack_tool.canonical_json(report))
                with self.assertRaises(pack_tool.Refusal):
                    pack_tool.verify_reproduction(argparse.Namespace(report=str(output)))


if __name__ == "__main__":
    unittest.main()
