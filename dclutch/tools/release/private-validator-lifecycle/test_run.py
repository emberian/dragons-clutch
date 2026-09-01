#!/usr/bin/env python3

from __future__ import annotations

import copy
import dataclasses
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

MODULE_PATH = Path(__file__).with_name("run.py")
SPEC = importlib.util.spec_from_file_location(
    "private_validator_lifecycle", MODULE_PATH
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def base58(data: bytes) -> str:
    number = int.from_bytes(data, "big")
    output = ""
    while number:
        number, remainder = divmod(number, 58)
        output = MODULE.BASE58_ALPHABET[remainder] + output
    zeros = len(data) - len(data.lstrip(b"\0"))
    return "1" * zeros + (output or "")


PUBKEY_A = base58(bytes([1]) * 32)
PUBKEY_B = base58(bytes([2]) * 32)
PUBKEY_C = base58(bytes([3]) * 32)
SIGNATURES = [base58(bytes([index]) * 64) for index in range(1, 20)]


def offline_preflight_fixture(
    paths: MODULE.Paths, commit: str, tree: str, through: str
) -> dict[str, object]:
    source_sha256 = {
        str(MODULE.RUNNER_RELATIVE_PATH): MODULE.sha256_file(
            paths.repo / MODULE.RUNNER_RELATIVE_PATH
        ),
        str(MODULE.OFFLINE_PREFLIGHT_RELATIVE_PATH): MODULE.sha256_file(
            paths.repo / MODULE.OFFLINE_PREFLIGHT_RELATIVE_PATH
        ),
    }
    report: dict[str, object] = {
        "schema": MODULE.OFFLINE_PREFLIGHT_SCHEMA,
        "status": "accepted",
        "evidence_level": MODULE.OFFLINE_PREFLIGHT_EVIDENCE_LEVEL,
        "through": through,
        "validator_started": False,
        "rpc_used": False,
        "keys_read": False,
        "build_run": False,
        "repository": {
            "head": commit,
            "tree": tree,
            "source_set_sha256": MODULE.sha256_bytes(
                MODULE.json.dumps(
                    source_sha256, sort_keys=True, separators=(",", ":")
                ).encode()
            ),
        },
        "command_exposures": [],
        "recovery_exposure": {},
        "schema_handoffs": [],
        "stage_vocabulary": [],
        "constants": {},
        "economic_owner": {},
        "founding_geometry": {},
        "transaction_geometry": {},
        "expected_execution": [],
        "source_sha256": source_sha256,
    }
    report["model_sha256"] = MODULE.sha256_bytes(
        MODULE.json.dumps(report, sort_keys=True, separators=(",", ":")).encode()
    )
    return report


def direct_activation_report(*, market: str = PUBKEY_A, root: str = PUBKEY_B) -> dict[str, object]:
    return {
        "verdict": "ACTIVATED",
        "facts": {
            "schema": "dclutch-local-private-validator-direct-capability-activation-report-v1",
            "market": market,
            "generation": 1,
            "entryIndex": 0,
            "root": root,
            "foundingPermitRoot": PUBKEY_C,
            "fundingLedger": base58(bytes([4]) * 32),
            "callerAuthority": base58(bytes([5]) * 32),
            "contextSha256": "a" * 64,
            "roleRequestSha256": "b" * 64,
            "activationDeadlineSlot": 100,
            "observedSlot": 10,
            "instructionAccounts": 37,
            "instructionDataBytes": 320,
        },
        "activationSignature": SIGNATURES[0],
        "activationSlot": 11,
        "feeLamports": 75_000,
        "computeUnitsConsumed": 30_000,
        "rootLamports": 1_000_000,
        "rootBytes": 128,
        "rootPhase": "Open",
        "ledgerLamportsAfter": 1_000_000,
        "tableTransactions": [
            {"label": "create DIRECT-ACT table", "signature": SIGNATURES[1], "slot": 9}
        ],
    }


class PrivateValidatorLifecycleTests(unittest.TestCase):
    def test_mixed_gate_uses_shared_authenticator_and_explicit_pins(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            tree = root / "source-tree.txt"
            tree.write_bytes(b"candidate source tree\n")
            tree_sha256 = MODULE.sha256_file(tree)
            labels = [MODULE.CANONICAL_RESOLUTION_GATE_LABEL] + [
                f"other-{index}" for index in range(12)
            ]
            links = []
            for label in labels:
                row: dict[str, object] = {
                    "label": label,
                    "sbf_diagnostics_count": 0,
                    "frame_bound_bytes": 4096,
                    "frames_at_or_over_bound": 0,
                    "deepest_frame_bytes": 4032,
                }
                if label == MODULE.CANONICAL_RESOLUTION_GATE_LABEL:
                    row.update(
                        {
                            "package": MODULE.CANONICAL_RESOLUTION_PACKAGE,
                            "compile_marker": (
                                "Compiling " + MODULE.CANONICAL_RESOLUTION_PACKAGE
                            ),
                            "checked_manifest": {
                                "canonical_path": "evidence/resolution/checked.bin"
                            },
                            "artifact_provenance": {
                                "canonical_path": "provenance/resolution.json"
                            },
                            "elf": {
                                "canonical_path": MODULE.CANONICAL_RESOLUTION_ELF_PATH,
                                "bytes": 1234,
                            },
                        }
                    )
                links.append(row)
            source_revision = "11" * 20
            gate = {
                "schema": MODULE.MIXED_GATE_SCHEMA,
                "source_revision": source_revision,
                "source_tree_sha256": tree_sha256,
                "source_tree_manifest": {
                    "canonical_path": tree.name,
                    "sha256": tree_sha256,
                },
                "link_count": 13,
                "links": links,
            }
            gate_path = root / "CHECKED_UPGRADE_GATE.json"
            gate_path.write_text(MODULE.json.dumps(gate))
            gate_sha256 = MODULE.sha256_file(gate_path)
            paths = MODULE.Paths(
                repo=MODULE_PATH.parents[3],
                release_root=root,
                expected_release_gate_sha256=gate_sha256,
                expected_release_source_revision=source_revision,
                expected_release_source_tree_sha256=tree_sha256,
                bootstrap=root / "bootstrap",
                reuse_bootstrap_work=None,
                validator=MODULE_PATH,
                solana=MODULE_PATH,
                work=root / "unused-work",
            )
            resolution = links[0]

            class Authenticator:
                Refusal = RuntimeError

                @staticmethod
                def authenticate_existing_gate(
                    observed_gate: Path,
                    observed_sha256: str,
                    observed_revision: str,
                    observed_tree: str,
                    selected_link: str,
                ) -> dict[str, object]:
                    self.assertEqual(observed_gate, gate_path)
                    self.assertEqual(observed_sha256, gate_sha256)
                    self.assertEqual(observed_revision, source_revision)
                    self.assertEqual(observed_tree, tree_sha256)
                    self.assertEqual(
                        selected_link, MODULE.CANONICAL_RESOLUTION_GATE_LABEL
                    )
                    return {
                        "schema": "dclutch-checked-mixed-gate-link-selection-v1",
                        "gate_path": str(gate_path),
                        "gate_sha256": gate_sha256,
                        "source_revision": source_revision,
                        "source_tree_sha256": tree_sha256,
                        "label": resolution["label"],
                        "package": resolution["package"],
                        "elf": resolution["elf"],
                        "checked_manifest": resolution["checked_manifest"],
                        "artifact_provenance": resolution["artifact_provenance"],
                    }

            with mock.patch.object(
                MODULE,
                "load_mixed_gate_authenticator",
                return_value=Authenticator,
            ):
                observed_path, observed_gate, observed_sha256 = MODULE.checked_gate(
                    paths, "22" * 20
                )
            self.assertEqual(
                (observed_path, observed_gate, observed_sha256),
                (gate_path, gate, gate_sha256),
            )

            without_pin = dataclasses.replace(
                paths, expected_release_gate_sha256=None
            )
            with self.assertRaisesRegex(
                MODULE.Refusal, "requires explicit gate"
            ):
                MODULE.checked_gate(without_pin, "22" * 20)

    def test_offline_preflight_authenticates_exact_source_set_and_model(self) -> None:
        repo = MODULE_PATH.parents[3]
        paths = MODULE.Paths(
            repo=repo,
            release_root=repo,
            expected_release_gate_sha256=None,
            expected_release_source_revision=None,
            expected_release_source_tree_sha256=None,
            bootstrap=repo / "unused-bootstrap",
            reuse_bootstrap_work=None,
            validator=MODULE_PATH,
            solana=MODULE_PATH,
            work=repo / "unused-preflight-work",
        )
        commit = "11" * 20
        tree = "22" * 20
        report = offline_preflight_fixture(paths, commit, tree, "full-probe")
        self.assertEqual(
            MODULE.authenticate_offline_preflight(
                report,
                paths=paths,
                commit=commit,
                tree=tree,
                through="full-probe",
            ),
            report,
        )
        hostiles: list[tuple[str, dict[str, object], str]] = []
        changed_source = copy.deepcopy(report)
        changed_source["source_sha256"][str(MODULE.RUNNER_RELATIVE_PATH)] = "33" * 32
        changed_source["repository"]["source_set_sha256"] = MODULE.sha256_bytes(
            MODULE.json.dumps(
                changed_source["source_sha256"],
                sort_keys=True,
                separators=(",", ":"),
            ).encode()
        )
        changed_source["model_sha256"] = MODULE.sha256_bytes(
            MODULE.json.dumps(
                {key: value for key, value in changed_source.items() if key != "model_sha256"},
                sort_keys=True,
                separators=(",", ":"),
            ).encode()
        )
        hostiles.append(("source", changed_source, "did not bind exact"))
        changed_mode = copy.deepcopy(report)
        changed_mode["through"] = "participant"
        changed_mode["model_sha256"] = MODULE.sha256_bytes(
            MODULE.json.dumps(
                {key: value for key, value in changed_mode.items() if key != "model_sha256"},
                sort_keys=True,
                separators=(",", ":"),
            ).encode()
        )
        hostiles.append(("mode", changed_mode, "did not accept"))
        changed_runtime = copy.deepcopy(report)
        changed_runtime["validator_started"] = True
        changed_runtime["model_sha256"] = MODULE.sha256_bytes(
            MODULE.json.dumps(
                {key: value for key, value in changed_runtime.items() if key != "model_sha256"},
                sort_keys=True,
                separators=(",", ":"),
            ).encode()
        )
        hostiles.append(("runtime", changed_runtime, "did not accept"))
        changed_model = copy.deepcopy(report)
        changed_model["model_sha256"] = "44" * 32
        hostiles.append(("model", changed_model, "model digest changed"))
        for label, hostile, refusal in hostiles:
            with self.subTest(label=label), self.assertRaisesRegex(
                MODULE.Refusal, refusal
            ):
                MODULE.authenticate_offline_preflight(
                    hostile,
                    paths=paths,
                    commit=commit,
                    tree=tree,
                    through="full-probe",
                )

    def test_main_runs_preflight_before_gate_and_work_creation(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            work = root / "work"
            paths = MODULE.Paths(
                repo=MODULE_PATH.parents[3],
                release_root=root,
                expected_release_gate_sha256=None,
                expected_release_source_revision=None,
                expected_release_source_tree_sha256=None,
                bootstrap=work / "bootstrap",
                reuse_bootstrap_work=None,
                validator=MODULE_PATH,
                solana=MODULE_PATH,
                work=work,
            )

            def preflight(*_args: object) -> tuple[bytes, dict[str, object]]:
                self.assertFalse(work.exists())
                return b'{"accepted":true}\n', {
                    "model_sha256": "11" * 32,
                    "repository": {"source_set_sha256": "22" * 32},
                }

            def gate(*_args: object) -> object:
                self.assertFalse(work.exists())
                raise MODULE.Refusal("stop before work")

            with (
                mock.patch.object(
                    MODULE, "parse", return_value=(paths, 1, "full-probe", False, False)
                ),
                mock.patch.object(MODULE, "clean_commit", return_value="33" * 20),
                mock.patch.object(MODULE, "clean_tree", return_value="44" * 20),
                mock.patch.object(MODULE, "run_offline_preflight", side_effect=preflight),
                mock.patch.object(MODULE, "checked_gate", side_effect=gate),
                self.assertRaisesRegex(MODULE.Refusal, "stop before work"),
            ):
                MODULE.main([])
            self.assertFalse(work.exists())

    def test_main_persists_exact_preflight_stdout_before_build(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            work = root / "work"
            paths = MODULE.Paths(
                repo=MODULE_PATH.parents[3],
                release_root=root,
                expected_release_gate_sha256=None,
                expected_release_source_revision=None,
                expected_release_source_tree_sha256=None,
                bootstrap=work / "bootstrap",
                reuse_bootstrap_work=None,
                validator=MODULE_PATH,
                solana=MODULE_PATH,
                work=work,
            )
            captured = b'{"schema":"preflight-test"}\n'
            report = {
                "model_sha256": "11" * 32,
                "repository": {"source_set_sha256": "22" * 32},
            }

            def build(observed: MODULE.Paths, *_args: object) -> MODULE.Paths:
                self.assertEqual(
                    (work / MODULE.OFFLINE_PREFLIGHT_RECEIPT).read_bytes(), captured
                )
                self.assertTrue((work / "runs").is_dir())
                self.assertFalse((work / "SUMMARY.json").exists())
                raise MODULE.Refusal("stop after receipt")

            with (
                mock.patch.object(
                    MODULE, "parse", return_value=(paths, 1, "full-probe", False, False)
                ),
                mock.patch.object(MODULE, "clean_commit", return_value="33" * 20),
                mock.patch.object(MODULE, "clean_tree", return_value="44" * 20),
                mock.patch.object(
                    MODULE, "run_offline_preflight", return_value=(captured, report)
                ),
                mock.patch.object(
                    MODULE,
                    "checked_gate",
                    return_value=(root / "gate.json", {}, "55" * 32),
                ),
                mock.patch.object(MODULE, "build_bootstrap", side_effect=build),
                self.assertRaisesRegex(MODULE.Refusal, "stop after receipt"),
            ):
                MODULE.main([])

    def test_full_help_requires_every_dispatched_final_evidence_command(self) -> None:
        probe_required = (
            *MODULE.FOUNDING_PARTICIPANT_COMMANDS,
            MODULE.DIRECT_CAPABILITY_ACTIVATION_COMMAND,
            MODULE.DIRECT_PRODUCER_COMMAND,
            MODULE.DIRECT_EXECUTE_COMMAND,
            MODULE.DIRECT_PAYOUT_SCHEDULE_COMMAND,
            MODULE.PYTH_PROVISION_COMMAND,
            MODULE.FLAGSHIP_RESOLUTION_COMMAND,
            MODULE.PAYOUT_INPUT_COMMAND,
            MODULE.PAYOUT_EXECUTE_COMMAND,
            MODULE.TERMINAL_SEQUENCE_COMMAND,
            MODULE.TERMINAL_RETIREMENT_COMMAND,
        )
        full_required = (*probe_required, *MODULE.FINAL_EVIDENCE_COMMANDS)
        with tempfile.TemporaryDirectory() as root_text:
            bootstrap = Path(root_text) / "fake-bootstrap"

            def install(commands: tuple[str, ...]) -> None:
                bootstrap.write_text(
                    "#!/bin/sh\nprintf '%s\\n' "
                    + " ".join(f"'{command}'" for command in commands)
                    + "\n"
                )
                bootstrap.chmod(0o755)

            install(probe_required)
            self.assertEqual(len(MODULE.command_surface(bootstrap, "full-probe")), 64)
            for omitted in MODULE.FINAL_EVIDENCE_COMMANDS:
                with self.subTest(omitted=omitted):
                    install(
                        tuple(command for command in full_required if command != omitted)
                    )
                    with self.assertRaisesRegex(MODULE.Refusal, omitted):
                        MODULE.command_surface(bootstrap, "full")

    def test_direct_activation_stage_pins_inputs_and_shifts_producer_ordinal(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            run = root / "run"
            (run / "direct").mkdir(parents=True)
            plan = root / "plan.json"
            market = root / "market.json"
            campaign = root / "founding.json"
            payer_key = root / "validator-mint.json"
            for path in (plan, market, payer_key):
                path.write_text("{}\n")
            campaign.write_text(
                MODULE.json.dumps({"founding_targets": {"open_market": PUBKEY_A}})
            )
            output = run / "direct" / "direct-capability-activation.json"
            stdout = (MODULE.json.dumps(direct_activation_report(), indent=2) + "\n").encode()
            output.write_bytes(stdout)
            paths = mock.Mock(bootstrap=root / "bootstrap", solana=root / "solana")
            mutable = {"keypairs": {MODULE.VALIDATOR_MINT_ROLE: str(payer_key)}}
            with mock.patch.object(
                MODULE, "run_stage", return_value=mock.Mock(stdout=stdout)
            ) as stage, mock.patch.object(MODULE, "key_address", return_value=PUBKEY_C):
                activation, next_ordinal = MODULE.run_direct_capability_activation(
                    run,
                    paths,
                    mutable,
                    "http://127.0.0.1:8899",
                    plan,
                    market,
                    campaign,
                    9,
                )
            self.assertEqual(next_ordinal, 10)
            self.assertEqual(stage.call_args.args[1:3], (9, "direct-capability-activation"))
            argv = stage.call_args.args[3]
            self.assertEqual(argv[1], MODULE.DIRECT_CAPABILITY_ACTIVATION_COMMAND)
            self.assertEqual(argv[argv.index("--expected-plan-sha256") + 1], MODULE.sha256_file(plan))
            self.assertEqual(
                argv[argv.index("--expected-market-input-sha256") + 1],
                MODULE.sha256_file(market),
            )
            self.assertEqual(
                argv[argv.index("--expected-campaign-report-sha256") + 1],
                MODULE.sha256_file(campaign),
            )
            self.assertEqual(argv[argv.index("--payer") + 1], PUBKEY_C)
            self.assertEqual(argv[argv.index("--payer-keypair") + 1], str(payer_key))
            self.assertEqual(argv[-1], "--execute")
            self.assertEqual(activation["root"], PUBKEY_B)
            self.assertEqual(activation["finalized"]["slot"], 11)

    def test_direct_activation_authenticator_refuses_hostile_shape_and_nonfinal_fields(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            plan = root / "plan.json"
            market = root / "market.json"
            campaign = root / "founding.json"
            for path in (plan, market):
                path.write_text("{}\n")
            campaign.write_text(
                MODULE.json.dumps({"founding_targets": {"open_market": PUBKEY_A}})
            )

            def authenticate(document: dict[str, object]) -> None:
                MODULE.authenticate_direct_capability_activation(
                    document,
                    plan=plan,
                    market_input=market,
                    campaign_report=campaign,
                    expected_market=PUBKEY_A,
                )

            authenticate(direct_activation_report())
            hostile_cases = []
            schema = direct_activation_report()
            schema["facts"] = {**schema["facts"], "schema": "foreign-schema"}
            hostile_cases.append(schema)
            root_substitution = direct_activation_report()
            root_substitution["facts"] = {**root_substitution["facts"], "root": "not-a-pubkey"}
            hostile_cases.append(root_substitution)
            market_substitution = direct_activation_report()
            market_substitution["facts"] = {**market_substitution["facts"], "market": PUBKEY_C}
            hostile_cases.append(market_substitution)
            nonfinal = direct_activation_report()
            nonfinal["activationSlot"] = 0
            hostile_cases.append(nonfinal)
            for hostile in hostile_cases:
                with self.subTest(hostile=hostile):
                    with self.assertRaises(MODULE.Refusal):
                        authenticate(hostile)

    def test_named_seeds_are_stable_distinct_and_named(self) -> None:
        first = MODULE.named_seed(1)
        last = MODULE.named_seed(20)
        self.assertEqual(first[0], "seed-01")
        self.assertEqual(last[0], "seed-20")
        self.assertNotEqual(first[1], last[1])
        self.assertEqual(first, MODULE.named_seed(1))

    def test_mean_is_exact_and_does_not_hide_remainder(self) -> None:
        self.assertEqual(
            MODULE.arithmetic_mean([10, 11, 12]),
            {"numerator": 33, "denominator": 3, "floor": 11, "remainder": 0},
        )

    def test_compute_report_names_pass_count_and_exact_mean(self) -> None:
        report = MODULE.compute_unit_report(
            [
                {"compute_units": {"founding-dcltgmf3": 10, "pyth-verify": 21}},
                {"compute_units": {"founding-dcltgmf3": 12, "pyth-verify": 22}},
            ]
        )
        self.assertEqual(
            report["founding-dcltgmf3"],
            {
                "pass_count": 2,
                "arithmetic_mean": {
                    "numerator": 22,
                    "denominator": 2,
                    "floor": 11,
                    "remainder": 0,
                },
            },
        )
        self.assertEqual(report["pyth-verify"]["pass_count"], 2)
        self.assertEqual(report["pyth-verify"]["arithmetic_mean"]["remainder"], 1)
        self.assertEqual(
            MODULE.arithmetic_mean([1, 2]),
            {"numerator": 3, "denominator": 2, "floor": 1, "remainder": 1},
        )

    def test_campaign_flags_are_owned_by_the_rust_prepare_report(self) -> None:
        report = {
            "campaign_administration_keypairs": {
                "core-upgrade-authority": "/owned/core.json",
                "campaign-payer": "/owned/admin-payer.json",
            },
            "campaign_founding_keypairs": {
                role: f"/owned/{role}.json"
                for role in MODULE.CAMPAIGN_FOUNDING_KEY_ROLES
            },
        }
        self.assertEqual(
            MODULE.key_flags(report),
            [
                item
                for role in sorted(MODULE.CAMPAIGN_FOUNDING_KEY_ROLES)
                for item in (f"--keypair-{role}", f"/owned/{role}.json")
            ],
        )
        self.assertEqual(
            MODULE.key_flags(report, "campaign_administration_keypairs"),
            [
                "--keypair-campaign-payer",
                "/owned/admin-payer.json",
                "--keypair-core-upgrade-authority",
                "/owned/core.json",
            ],
        )
        with self.assertRaisesRegex(MODULE.Refusal, "Rust-owned"):
            MODULE.key_flags({"campaign_keypairs": {"wrong": "/wrong"}})
        with self.assertRaisesRegex(MODULE.Refusal, "frozen mode"):
            MODULE.key_flags(report, "campaign_keypairs")
        with self.assertRaisesRegex(MODULE.Refusal, "exact Rust-owned"):
            MODULE.key_flags(
                {
                    **report,
                    "campaign_founding_keypairs": {
                        role: path
                        for role, path in report["campaign_founding_keypairs"].items()
                        if role != MODULE.PARTICIPANT_ROLE
                    },
                }
            )

    def test_campaign_public_identities_are_exact_distinct_and_key_free(self) -> None:
        founder = base58(bytes([7]) * 32)
        substituted = base58(bytes([8]) * 32)
        report = {
            "campaign_public_identities": {
                "founding-founder": founder,
                "substituted-founder": substituted,
            }
        }
        self.assertEqual(MODULE.campaign_public_identities(report), report["campaign_public_identities"])
        with self.assertRaisesRegex(MODULE.Refusal, "exact two"):
            MODULE.campaign_public_identities(
                {"campaign_public_identities": {"founding-founder": founder}}
            )
        with self.assertRaisesRegex(MODULE.Refusal, "alias"):
            MODULE.campaign_public_identities(
                {
                    "campaign_public_identities": {
                        "founding-founder": founder,
                        "substituted-founder": founder,
                    }
                }
            )

    def test_campaign_mode_argv_is_disjoint_and_uses_owned_projections(self) -> None:
        founder = base58(bytes([7]) * 32)
        substituted = base58(bytes([8]) * 32)
        report = {
            "campaign_administration_keypairs": {
                "core-upgrade-authority": "/owned/core.json",
                "campaign-payer": "/owned/admin-payer.json",
            },
            "campaign_founding_keypairs": {
                role: f"/owned/{role}.json"
                for role in MODULE.CAMPAIGN_FOUNDING_KEY_ROLES
            },
            "campaign_public_identities": {
                "founding-founder": founder,
                "substituted-founder": substituted,
            },
        }
        admin = MODULE.administration_campaign_argv(
            Path("/bootstrap"),
            "http://127.0.0.1:8899",
            Path("/plan.json"),
            Path("/administration.json"),
            Path("/infrastructure-lineage.json"),
            report,
        )
        founding = MODULE.founding_campaign_argv(
            Path("/bootstrap"),
            "http://127.0.0.1:8899",
            Path("/plan.json"),
            Path("/market.json"),
            Path("/founding.json"),
            report,
        )
        self.assertNotIn("--founding-only", admin)
        self.assertIn("--keypair-core-upgrade-authority", admin)
        self.assertIn("--keypair-campaign-payer", admin)
        self.assertEqual(
            admin[admin.index("--infrastructure-lineage-evidence") + 1],
            "/infrastructure-lineage.json",
        )
        self.assertIn("--founding-only", founding)
        self.assertIn("--keypair-campaign-payer", founding)
        self.assertNotIn("--keypair-core-upgrade-authority", founding)
        self.assertEqual(founding[founding.index("--founding-founder") + 1], founder)
        self.assertEqual(
            founding[founding.index("--substituted-founder") + 1], substituted
        )

    def test_campaign_completion_binds_mode_and_six_founding_mutations(self) -> None:
        plan = Path("/plan.json")
        market_path = Path("/market.json")
        admin = {
            "schema": "dclutch-successor-campaign-report-v1",
            "cluster": "loopback",
            "genesis_hash": PUBKEY_A,
            "mode": "execute",
            "execution_intent": {
                "authorized_mutation": True,
                "campaign_mode": "administration",
                "through_stage": "activation",
                "plan": str(plan),
                "market": None,
            },
            "execution": {"completed": True, "market": None, "transactions": []},
        }
        self.assertIs(
            MODULE.authenticate_campaign_completion(
                admin, "administration", plan, None
            ),
            admin["execution"],
        )
        ledger = {"address": PUBKEY_A}
        founding = {
            **admin,
            "rpc_url": "http://127.0.0.1:8899",
            "plan_sha256": "11" * 32,
            "market_sha256": "22" * 32,
            "evidence_output": "/founding.json",
            "payer": PUBKEY_C,
            "execution_intent": {
                **admin["execution_intent"],
                "campaign_mode": "founding-only",
                "through_stage": "founding",
                "market": str(market_path),
            },
            "execution": {
                "completed": True,
                "recoveredFinalizedFounding": False,
                "transactions": [
                    {
                        "label": label,
                        "signature": SIGNATURES[index],
                        "slot": index + 1,
                        "fee_lamports": 0,
                        "compute_units_consumed": 100_000 + index,
                        "error": None,
                        "transaction_metadata_available": True,
                    }
                    for index, label in enumerate(MODULE.FOUNDING_SUCCESS_MUTATIONS)
                ],
                "market": {"accounts": {"resolution_funding_ledger": ledger}},
            },
            "foundingSubmissionJournals": [
                {
                    "schema": MODULE.FOUNDING_JOURNAL_SCHEMA,
                    "cluster": "loopback",
                    "genesisHash": PUBKEY_A,
                    "evidencePath": "/founding.json",
                    "rpcUrl": "http://127.0.0.1:8899",
                    "planSha256": "11" * 32,
                    "marketSha256": "22" * 32,
                    "payer": PUBKEY_C,
                    "operation": operation,
                    "phase": "finalized",
                    "expectedSignature": SIGNATURES[index],
                    "finalizedSlot": index + 1,
                    "feeLamports": 0,
                    "computeUnitsConsumed": 100_000 + index,
                    "intentSha256": "31" * 32,
                    "signedPacketSha256": "32" * 32,
                    "transactionSha256": "33" * 32,
                    "finalizedPoststatesSha256": "34" * 32,
                    "stateSha256": "35" * 32,
                }
                for index, operation in enumerate(MODULE.FOUNDING_JOURNAL_OPERATIONS)
            ],
        }
        self.assertIs(
            MODULE.authenticate_campaign_completion(
                founding, "founding-only", plan, market_path
            ),
            founding["execution"],
        )
        self.assertEqual(
            MODULE.founding_compute_units(founding),
            {
                metric: 100_000 + index
                for index, metric in enumerate(MODULE.FOUNDING_COMPUTE_LABELS)
            },
        )
        hostile = {
            **founding,
            "execution": {
                **founding["execution"],
                "transactions": list(reversed(founding["execution"]["transactions"])),
            },
        }
        with self.assertRaisesRegex(MODULE.Refusal, "six-mutation"):
            MODULE.authenticate_campaign_completion(
                hostile, "founding-only", plan, market_path
            )
        missing_ledger = {
            **founding,
            "execution": {
                **founding["execution"],
                "market": {"accounts": {}},
            },
        }
        with self.assertRaisesRegex(MODULE.Refusal, "Resolution funding ledger"):
            MODULE.authenticate_campaign_completion(
                missing_ledger, "founding-only", plan, market_path
            )
        wrong_journal = {
            **founding,
            "foundingSubmissionJournals": [
                *founding["foundingSubmissionJournals"][:5],
                {
                    **founding["foundingSubmissionJournals"][5],
                    "phase": "submitted",
                },
            ],
        }
        with self.assertRaisesRegex(MODULE.Refusal, "does not join"):
            MODULE.authenticate_campaign_completion(
                wrong_journal, "founding-only", plan, market_path
            )

    def test_infrastructure_lineage_joins_source_profiles_artifacts_and_activation(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            plan = root / "plan.json"
            plan.write_bytes(b"exact plan\n")
            lineage_path = root / "infrastructure-lineage.json"
            artifact_ids = [f"{index:02x}" * 32 for index in range(1, 12)]
            programs = {
                role: base58(bytes([index + 10]) * 32)
                for index, role in enumerate(MODULE.ROLE_ORDER)
            }
            programdata = {
                role: base58(bytes([index + 30]) * 32)
                for index, role in enumerate(MODULE.ROLE_ORDER)
            }
            loader = base58(bytes([50]) * 32)

            def account(address: str, data_sha256: str) -> dict[str, object]:
                return {
                    "address": address,
                    "owner": programs["registry"],
                    "lamports": 1,
                    "executable": False,
                    "data_len": 224,
                    "data_sha256": data_sha256,
                    "account_sha256": "aa" * 32,
                }

            def release(
                role: str, artifact_id: str, slot: int
            ) -> dict[str, object]:
                return {
                    "artifactReleaseId": artifact_id,
                    "program": programs[role],
                    "loaderProgram": loader,
                    "programData": programdata[role],
                    "semanticReleaseId": f"{MODULE.ROLE_ORDER.index(role) + 60:02x}" * 32,
                    "elfSha256": f"{MODULE.ROLE_ORDER.index(role) + 70:02x}" * 32,
                    "deploymentSlot": slot,
                    "upgradePolicy": "exact-authority",
                    "upgradeAuthority": PUBKEY_C,
                }

            def record(row: dict[str, object]) -> dict[str, object]:
                artifact_id = str(row["artifactReleaseId"])
                return {
                    "record": row,
                    "rawAccount": account(PUBKEY_A, artifact_id),
                    "stagingAddress": PUBKEY_B,
                    "stagingAbsentAfterFinalize": True,
                }

            predecessor_registry = release("registry", artifact_ids[0], 1)
            successor_registry = release("registry", artifact_ids[1], 10)
            rent_release = release("rent", artifact_ids[2], 2)
            execution_roles = ("core", "claims", "trading", "resolution", "custody")
            activated = [
                {"role": role, "release": release(role, artifact_ids[index + 3], index + 3)}
                for index, role in enumerate(execution_roles)
            ]
            checked = [
                {
                    "role": role,
                    "program": programs[role],
                    "programData": programdata[role],
                    "checkedCandidateElfPath": f"/release/{role}.so",
                    "checkedCandidateElfSha256": (
                        successor_registry["elfSha256"]
                        if role == "registry"
                        else rent_release["elfSha256"]
                        if role == "rent"
                        else next(row["release"]["elfSha256"] for row in activated if row["role"] == role)
                    ),
                    "genesisLiveElfSha256": (
                        successor_registry["elfSha256"]
                        if role == "registry"
                        else rent_release["elfSha256"]
                        if role == "rent"
                        else next(row["release"]["elfSha256"] for row in activated if row["role"] == role)
                    ),
                    "genesisProgramDataAccountSha256": "bb" * 32,
                    "genesisDeploymentSlot": MODULE.ROLE_ORDER.index(role) + 1,
                    "semanticReleaseId": (
                        successor_registry["semanticReleaseId"]
                        if role == "registry"
                        else rent_release["semanticReleaseId"]
                        if role == "rent"
                        else next(row["release"]["semanticReleaseId"] for row in activated if row["role"] == role)
                    ),
                }
                for role in MODULE.ROLE_ORDER
            ]
            lineage = {
                "schema": "dclutch-current-source-infrastructure-lineage-v1",
                "evidenceLevel": "local-validator-finalized-chain-state",
                "cluster": "owned-loopback",
                "genesisHash": PUBKEY_A,
                "planSha256": MODULE.sha256_file(plan),
                "campaignEvidencePath": "/administration.json",
                "source": {
                    "revision": "11" * 20,
                    "treeSha256": "12" * 32,
                    "checkedReleaseGatePath": "/release/CHECKED_UPGRADE_GATE.json",
                    "checkedReleaseGateSha256": "13" * 32,
                    "checkedLocalMutableSetSha256": "14" * 32,
                    "solanaCliVersion": "4.0.2",
                },
                "checkedArtifacts": checked,
                "profiles": {
                    "predecessorV1": {
                        "address": PUBKEY_A,
                        "account": account(PUBKEY_A, "21" * 32),
                        "registryArtifactReleaseId": artifact_ids[0],
                        "rentArtifactReleaseId": artifact_ids[2],
                    },
                    "successorV2": {
                        "address": PUBKEY_B,
                        "account": account(PUBKEY_B, "22" * 32),
                        "registryArtifactReleaseId": artifact_ids[1],
                        "rentArtifactReleaseId": artifact_ids[2],
                        "predecessorRegistryArtifactReleaseId": artifact_ids[0],
                        "predecessorRentArtifactReleaseId": artifact_ids[2],
                    },
                    "v1PreservedByteIdentical": True,
                },
                "artifactLineage": {
                    "registry": {
                        "movedForward": True,
                        "predecessor": record(predecessor_registry),
                        "successor": record(successor_registry),
                    },
                    "rent": {"carriedForward": True, "release": record(rent_release)},
                },
                "activation": {
                    "releaseSetId": "31" * 32,
                    "checkedExecutionReleaseSetId": "32" * 32,
                    "checkedMultiprogramEnvelopeSha256": "33" * 32,
                    "account": account(PUBKEY_C, "34" * 32),
                    "roles": activated,
                },
                "migration": {
                    "preexistingMarketsMigrated": 0,
                    "marketsSilentlyRebound": False,
                    "scope": "fresh validator",
                    "consumerRule": "V2 consumers",
                },
            }
            lineage_path.write_text(MODULE.json.dumps(lineage))
            administration = {
                "genesis_hash": PUBKEY_A,
                "evidence_output": "/administration.json",
                "infrastructureLineageEvidence": {
                    "path": str(lineage_path),
                    "sha256": MODULE.sha256_file(lineage_path),
                    "schema": lineage["schema"],
                },
            }
            self.assertEqual(
                MODULE.authenticate_infrastructure_lineage(
                    lineage_path, administration, plan
                ),
                lineage,
            )
            hostile = copy.deepcopy(lineage)
            hostile["profiles"]["successorV2"]["predecessorRegistryArtifactReleaseId"] = "ff" * 32
            lineage_path.write_text(MODULE.json.dumps(hostile))
            administration["infrastructureLineageEvidence"]["sha256"] = MODULE.sha256_file(lineage_path)
            with self.assertRaisesRegex(MODULE.Refusal, "join exactly"):
                MODULE.authenticate_infrastructure_lineage(
                    lineage_path, administration, plan
                )

    def test_external_rpc_is_structurally_refused(self) -> None:
        with self.assertRaisesRegex(MODULE.Refusal, "escaped loopback"):
            MODULE.rpc("https://api.devnet.solana.com", "getHealth")
        with self.assertRaisesRegex(MODULE.Refusal, "escaped loopback"):
            MODULE.rpc("https://api.mainnet-beta.solana.com", "getHealth")

    def test_work_must_be_fresh_and_exactly_twenty_seeds(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            fake = root / "fake"
            fake.write_text("x")
            fake.chmod(0o755)
            repo = root / "repo"
            release = root / "release"
            repo.mkdir()
            release.mkdir()
            with self.assertRaisesRegex(MODULE.Refusal, "exactly 20"):
                MODULE.parse(
                    [
                        "--repo",
                        str(repo),
                        "--release-root",
                        str(release),
                        "--validator",
                        str(fake),
                        "--solana",
                        str(fake),
                        "--work",
                        str(root / "work"),
                        "--seeds",
                        "1",
                    ]
                )
            paths, seeds, through, hold_participant, hot_cu_profile = MODULE.parse(
                [
                    "--repo",
                    str(repo),
                    "--release-root",
                    str(release),
                    "--validator",
                    str(fake),
                    "--solana",
                    str(fake),
                    "--work",
                    str(root / "work"),
                    "--seeds",
                    "1",
                    "--through",
                    "participant",
                    "--hold-after-participant",
                ]
            )
            self.assertEqual(paths.work, root / "work")
            self.assertEqual((seeds, through), (1, "participant"))
            self.assertTrue(hold_participant)
            self.assertFalse(hot_cu_profile)

    def test_participant_handoff_is_fsync_new_and_reauthenticated_after_resume(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            keys = root / "keys"
            keys.mkdir()
            plan = root / "plan.json"
            market = root / "market.json"
            founding = root / "founding.json"
            participant = root / "participant.json"
            for path, value in (
                (plan, b"plan\n"),
                (market, b"market\n"),
                (founding, b"founding\n"),
                (participant, b"participant\n"),
            ):
                path.write_bytes(value)
            document = MODULE.participant_handoff_document(
                source_revision="11" * 20,
                checked_release_gate_sha256="22" * 32,
                rpc_url="http://127.0.0.1:46088",
                validator_pid=1234,
                plan=plan,
                market=market,
                founding=founding,
                participant=participant,
                key_directory=keys,
            )
            self.assertEqual(
                set(document),
                {
                    "schema",
                    "status",
                    "sourceRevision",
                    "checkedReleaseGateSha256",
                    "rpcUrl",
                    "validatorPid",
                    "plan",
                    "marketInput",
                    "foundingEvidence",
                    "participantEvidence",
                    "participantSha256",
                    "keyDirectory",
                },
            )
            self.assertEqual(document["status"], "ready")
            self.assertEqual(
                document["participantSha256"], MODULE.sha256_file(participant)
            )

            class Validator:
                pid = 1234

                @staticmethod
                def poll() -> None:
                    return None

            receipt = root / "participant-handoff.json"
            with (
                mock.patch.object(MODULE.os, "kill") as stopped,
                mock.patch.object(MODULE.os, "getpgid", return_value=1234),
                mock.patch.object(MODULE, "rpc", return_value="ok"),
            ):
                MODULE.hold_after_participant(receipt, document, Validator())
            stopped.assert_called_once_with(MODULE.os.getpid(), MODULE.signal.SIGSTOP)
            self.assertEqual(receipt.stat().st_mode & 0o777, 0o600)

            participant.write_bytes(b"substituted participant\n")
            with (
                mock.patch.object(MODULE.os, "getpgid", return_value=1234),
                mock.patch.object(MODULE, "rpc", return_value="ok"),
                self.assertRaisesRegex(MODULE.Refusal, "participant evidence changed"),
            ):
                MODULE.authenticate_participant_handoff(
                    receipt, document, Validator()
                )

            substituted = dict(document)
            substituted["participantSha256"] = "33" * 32
            receipt.unlink()
            MODULE.write_json_new(receipt, substituted)
            with (
                mock.patch.object(MODULE.os, "getpgid", return_value=1234),
                mock.patch.object(MODULE, "rpc", return_value="ok"),
                self.assertRaisesRegex(MODULE.Refusal, "changed while"),
            ):
                MODULE.authenticate_participant_handoff(
                    receipt, document, Validator()
                )

    def test_participant_handoff_refuses_nonparticipant_mode(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            fake = root / "fake"
            fake.write_text("x")
            fake.chmod(0o755)
            repo = root / "repo"
            release = root / "release"
            repo.mkdir()
            release.mkdir()
            with self.assertRaisesRegex(MODULE.Refusal, "requires --through participant"):
                MODULE.parse(
                    [
                        "--repo",
                        str(repo),
                        "--release-root",
                        str(release),
                        "--validator",
                        str(fake),
                        "--solana",
                        str(fake),
                        "--work",
                        str(root / "work"),
                        "--hold-after-participant",
                    ]
                )

    def test_full_mode_admits_exact_twenty_seed_release_campaign(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            fake = root / "fake"
            fake.write_text("x")
            fake.chmod(0o755)
            repo = root / "repo"
            release = root / "release"
            repo.mkdir()
            release.mkdir()
            paths, seeds, through, hold, hot_cu_profile = MODULE.parse(
                [
                    "--repo",
                    str(repo),
                    "--release-root",
                    str(release),
                    "--validator",
                    str(fake),
                    "--solana",
                    str(fake),
                    "--work",
                    str(root / "work"),
                ]
            )
            self.assertEqual(paths.repo, repo.resolve())
            self.assertEqual(seeds, 20)
            self.assertEqual(through, "full")
            self.assertFalse(hold)
            self.assertFalse(hot_cu_profile)

    def test_fee_profile_is_exactly_half_a_percent(self) -> None:
        self.assertEqual(MODULE.DEVELOPMENT_FEE_BASIS_POINTS, 50)
        self.assertEqual(MODULE.FEE_BASIS_POINTS_DENOMINATOR, 10_000)

    def test_participant_fixture_liquidity_is_exact_and_authority_removed(self) -> None:
        fixture = {
            "sourceTokenAccount": "source",
            "sourceOwner": "participant",
            "quantityAtoms": 100_000_000,
            "foundingCollateralAtoms": 1_000_000_000,
            "totalSupplyAtoms": 1_100_000_000,
            "mint": "mint",
            "mintAuthorityRemoved": True,
            "transactionSignature": "signature",
            "finalizedSlot": 42,
            "computeUnitsConsumed": 123,
        }
        campaign = {
            "execution": {"localParticipantFixtureLiquidity": fixture},
            "founding_targets": {"collateral_mint": "mint"},
        }
        market = {"initial_collateral_atoms": 1_000_000_000}
        self.assertEqual(
            MODULE.authenticate_participant_fixture_liquidity(
                campaign, market, "participant", "source"
            ),
            fixture,
        )
        for field, value in (
            ("sourceOwner", "substituted"),
            ("quantityAtoms", 99_999_999),
            ("totalSupplyAtoms", 1_100_000_001),
            ("mintAuthorityRemoved", False),
        ):
            hostile = {**fixture, field: value}
            with self.assertRaisesRegex(MODULE.Refusal, "authority-removed"):
                MODULE.authenticate_participant_fixture_liquidity(
                    {
                        **campaign,
                        "execution": {"localParticipantFixtureLiquidity": hostile},
                    },
                    market,
                    "participant",
                    "source",
                )

    def test_funding_never_precreates_protocol_accounts(self) -> None:
        self.assertEqual(MODULE.LOCAL_AIRDROP_ROLES, ())
        self.assertEqual(MODULE.VALIDATOR_MINT_ROLE, "core-upgrade-authority")
        self.assertEqual(MODULE.CAMPAIGN_PAYER_ROLE, "campaign-payer")
        self.assertEqual(
            MODULE.DEVELOPMENT_FEE_RECIPIENT_ROLE, "founding-source-funder"
        )
        self.assertEqual(
            MODULE.PROTOCOL_CREATED_KEY_ROLES,
            (
                "collateral-mint",
                "collateral-wallet",
                "founding-beneficiary",
                "founding-projection-witness",
                "founding-source-funder",
            ),
        )
        self.assertTrue(
            set(MODULE.LOCAL_AIRDROP_ROLES).isdisjoint(
                MODULE.PROTOCOL_CREATED_KEY_ROLES
            )
        )

    def test_local_bankroll_is_one_exact_non_airdrop_transfer(self) -> None:
        argv = MODULE.local_bankroll_transfer_argv(
            Path("/solana"),
            "http://127.0.0.1:8899",
            Path("/genesis.json"),
            PUBKEY_B,
        )
        self.assertIn("transfer", argv)
        self.assertNotIn("airdrop", argv)
        self.assertEqual(argv[argv.index("transfer") + 1 : argv.index("--from")], [PUBKEY_B, "100"])
        self.assertEqual(argv[argv.index("--from") + 1], "/genesis.json")
        self.assertEqual(argv[argv.index("--fee-payer") + 1], "/genesis.json")
        self.assertIn("--allow-unfunded-recipient", argv)

    def test_local_bankroll_snapshot_is_one_finalized_vacancy_closure(self) -> None:
        rows = [
            {
                "owner": MODULE.SYSTEM_PROGRAM_ADDRESS,
                "executable": False,
                "data": ["", "base64"],
                "lamports": MODULE.LOCAL_TEST_BANKROLL_LAMPORTS + 50_000,
            },
            None,
            None,
            None,
            None,
            None,
            None,
        ]
        vacant = tuple(
            (role, base58(bytes([20 + index]) * 32))
            for index, role in enumerate(MODULE.PROTOCOL_CREATED_KEY_ROLES)
        )
        with mock.patch.object(
            MODULE,
            "rpc",
            return_value={"context": {"slot": 11}, "value": rows},
        ):
            snapshot = MODULE.local_bankroll_snapshot(
                "http://127.0.0.1:8899", PUBKEY_A, PUBKEY_B, vacant
            )
        self.assertEqual(snapshot["finalizedSlot"], "11")
        self.assertIsNone(snapshot["campaignPayerLamports"])
        self.assertEqual(
            snapshot["sourceLamports"],
            str(MODULE.LOCAL_TEST_BANKROLL_LAMPORTS + 50_000),
        )
        self.assertEqual(
            snapshot["vacantProtocolRoles"],
            [{"role": role, "address": address} for role, address in vacant],
        )
        rows[-1] = {
            "owner": MODULE.SYSTEM_PROGRAM_ADDRESS,
            "executable": False,
            "data": ["", "base64"],
            "lamports": 1,
        }
        with mock.patch.object(
            MODULE,
            "rpc",
            return_value={"context": {"slot": 11}, "value": rows},
        ), self.assertRaisesRegex(MODULE.Refusal, "already exists"):
            MODULE.local_bankroll_snapshot(
                "http://127.0.0.1:8899", PUBKEY_A, PUBKEY_B, vacant
            )

    def test_local_bankroll_transaction_binds_instruction_fee_cu_and_deltas(self) -> None:
        signature = SIGNATURES[0]
        fee = 0
        source_post = 123_456
        source_pre = source_post + MODULE.LOCAL_TEST_BANKROLL_LAMPORTS + fee
        value = {
            "slot": 12,
            "transaction": {
                "signatures": [signature],
                "message": {
                    "accountKeys": [
                        {
                            "pubkey": PUBKEY_A,
                            "signer": True,
                            "writable": True,
                        },
                        {
                            "pubkey": PUBKEY_B,
                            "signer": False,
                            "writable": True,
                        },
                        {
                            "pubkey": MODULE.SYSTEM_PROGRAM_ADDRESS,
                            "signer": False,
                            "writable": False,
                        },
                    ],
                    "instructions": [
                        {
                            "program": "system",
                            "programId": MODULE.SYSTEM_PROGRAM_ADDRESS,
                            "parsed": {
                                "type": "transfer",
                                "info": {
                                    "source": PUBKEY_A,
                                    "destination": PUBKEY_B,
                                    "lamports": MODULE.LOCAL_TEST_BANKROLL_LAMPORTS,
                                },
                            },
                        }
                    ],
                },
            },
            "meta": {
                "err": None,
                "fee": fee,
                "computeUnitsConsumed": 150,
                "innerInstructions": None,
                "preBalances": [source_pre, 0, 1],
                "postBalances": [
                    source_post,
                    MODULE.LOCAL_TEST_BANKROLL_LAMPORTS,
                    1,
                ],
            },
        }
        with mock.patch.object(MODULE, "rpc", return_value=value):
            fact = MODULE.finalized_local_bankroll_transaction(
                "http://127.0.0.1:8899", signature, PUBKEY_A, PUBKEY_B
            )
        self.assertEqual(fact["feeLamports"], str(fee))
        self.assertEqual(fact["computeUnitsConsumed"], "150")
        hostile = {
            **value,
            "meta": {
                **value["meta"],
                "postBalances": [source_post + 1, MODULE.LOCAL_TEST_BANKROLL_LAMPORTS, 1],
            },
        }
        with mock.patch.object(MODULE, "rpc", return_value=hostile), self.assertRaisesRegex(
            MODULE.Refusal, "conservation"
        ):
            MODULE.finalized_local_bankroll_transaction(
                "http://127.0.0.1:8899", signature, PUBKEY_A, PUBKEY_B
            )
        for hostile_fee in (True, -1, None):
            hostile = {**value, "meta": {**value["meta"], "fee": hostile_fee}}
            with mock.patch.object(
                MODULE, "rpc", return_value=hostile
            ), self.assertRaisesRegex(MODULE.Refusal, "omitted exact fee"):
                MODULE.finalized_local_bankroll_transaction(
                    "http://127.0.0.1:8899", signature, PUBKEY_A, PUBKEY_B
                )

    def test_local_bankroll_receipt_is_create_new(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            (root / "provisioning-poststate.json").write_text("{}")
            with self.assertRaisesRegex(MODULE.Refusal, "already exists"):
                MODULE.provision_disposable_funding(
                    root,
                    None,  # refusal precedes all tool/key access
                    {},
                    "http://127.0.0.1:8899",
                )

    def test_local_bankroll_owner_writes_one_finalized_joined_receipt(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            solana = root / "solana"
            solana.write_bytes(b"pinned-solana")
            key_roles = (
                MODULE.VALIDATOR_MINT_ROLE,
                MODULE.CAMPAIGN_PAYER_ROLE,
                *MODULE.PROTOCOL_CREATED_KEY_ROLES,
            )
            report = {
                "keypairs": {role: str(root / f"{role}.json") for role in key_roles}
            }
            addresses = {
                role: base58(bytes([30 + index]) * 32)
                for index, role in enumerate(key_roles)
            }
            vacant = [
                {"role": role, "address": addresses[role]}
                for role in MODULE.PROTOCOL_CREATED_KEY_ROLES
            ]
            pre = {
                "finalizedSlot": "10",
                "sourceLamports": "100000010000",
                "campaignPayerLamports": None,
                "vacantProtocolRoles": vacant,
            }
            transaction = {
                "signature": SIGNATURES[0],
                "finalizedSlot": "11",
                "feeLamports": "5000",
                "computeUnitsConsumed": "150",
                "sourcePreLamports": "100000010000",
                "sourcePostLamports": "5000",
                "campaignPayerPreLamports": "0",
                "campaignPayerPostLamports": "100000000000",
            }
            post = {
                "finalizedSlot": "12",
                "sourceLamports": "5000",
                "campaignPayerLamports": "100000000000",
                "vacantProtocolRoles": vacant,
            }
            paths = mock.Mock(solana=solana)

            def address_for(_solana: Path, keypair: Path) -> str:
                return addresses[keypair.stem]

            completed = mock.Mock(stdout=(
                '{"signature":"' + SIGNATURES[0] + '"}\n'
            ).encode())
            with mock.patch.object(MODULE, "key_address", side_effect=address_for), mock.patch.object(
                MODULE, "local_bankroll_snapshot", side_effect=[pre, post]
            ), mock.patch.object(MODULE, "run_stage", return_value=completed) as run_stage, mock.patch.object(
                MODULE, "finalized_local_bankroll_transaction", return_value=transaction
            ), mock.patch.object(MODULE, "rpc", return_value=PUBKEY_C):
                receipt = MODULE.provision_disposable_funding(
                    root, paths, report, "http://127.0.0.1:8899"
                )
            self.assertEqual(receipt["schema"], MODULE.LOCAL_TEST_BANKROLL_SCHEMA)
            self.assertEqual(receipt["amountLamports"], "100000000000")
            self.assertEqual(receipt["transaction"], transaction)
            self.assertEqual(receipt["prestate"], pre)
            self.assertEqual(receipt["poststate"], post)
            self.assertFalse(receipt["externalWrites"])
            self.assertEqual(run_stage.call_args.args[1:3], (3, "local-test-bankroll"))
            self.assertEqual(
                MODULE.read_unique_json(
                    root / "provisioning-poststate.json", "test bankroll receipt"
                ),
                receipt,
            )
        self.assertNotIn(MODULE.VALIDATOR_MINT_ROLE, MODULE.PROTOCOL_CREATED_KEY_ROLES)

    def test_pyth_owner_has_exact_eight_action_journal_prefix(self) -> None:
        self.assertEqual(
            MODULE.PYTH_PROVISION_COMMAND,
            "local-private-validator-pyth-vaa-provision-v1",
        )
        self.assertEqual(len(MODULE.PYTH_JOURNAL_FILES), 8)
        self.assertEqual(MODULE.PYTH_JOURNAL_FILES[0], "00-router-initialize.json")
        self.assertEqual(MODULE.PYTH_JOURNAL_FILES[-1], "07-encoded-vaa-verify.json")
        self.assertEqual(len(set(MODULE.PYTH_JOURNAL_FILES)), 8)

    def test_validator_uses_only_plan_owned_eighteen_account_genesis(self) -> None:
        argv = MODULE.validator_argv(
            Path("/tools/solana-test-validator"),
            Path("/work/ledger"),
            "/work/mutable/account-dir",
            "Mint111111111111111111111111111111111111111",
            20890,
        )
        self.assertEqual(argv.count("--account-dir"), 1)
        self.assertEqual(
            argv[argv.index("--account-dir") + 1], "/work/mutable/account-dir"
        )
        self.assertNotIn("--upgradeable-program", argv)
        self.assertNotIn("receiver.so", " ".join(argv))
        self.assertNotIn("router.so", " ".join(argv))
        self.assertEqual(argv[argv.index("--rpc-port") + 1], "20890")

    def test_checked_mutable_slot_floor_comes_from_the_seven_roles(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            path = Path(root_text) / "plan.json"
            path.write_text(
                '{"checked_local_mutable_set":{"roles":['
                + ",".join(
                    f'{{"role":"{role}","deployment_slot":{index}}}'
                    for index, role in enumerate(MODULE.ROLE_ORDER, start=1)
                )
                + "]}}"
            )
            self.assertEqual(MODULE.checked_mutable_slot_floor(path), 7)

    def test_resolution_can_only_come_from_the_canonical_checked_role(self) -> None:
        honest = {
            "label": "resolution",
            "package": "dclutch-resolution-proof-sbf",
            "compile_marker": "Compiling dclutch-resolution-proof-sbf from pinned source",
            "checked_manifest": {"canonical_path": "evidence/resolution/checked.bin"},
            "elf": {
                "canonical_path": "elf/resolution.so",
                "bytes": 631_640,
                "sha256": "a" * 64,
            },
        }
        self.assertEqual(MODULE.canonical_resolution_link({"links": [honest]}), honest)
        for substitution in (
            {"package": "dclutch-sbf"},
            {"elf": {**honest["elf"], "canonical_path": "elf/dclutch_sbf.so"}},
            {"elf": {**honest["elf"], "bytes": 9_034_536}},
        ):
            hostile = {**honest, **substitution}
            with self.assertRaisesRegex(MODULE.Refusal, "substitution is banished"):
                MODULE.canonical_resolution_link({"links": [hostile]})

    def test_finalized_fact_requires_exact_signature_slot_fee_and_compute(self) -> None:
        honest = {
            "signature": SIGNATURES[0],
            "slot": 9,
            "feeLamports": 0,
            "computeUnitsConsumed": 123,
        }
        self.assertEqual(
            MODULE.finalized_fact(honest, "honest"),
            {
                "signature": SIGNATURES[0],
                "slot": 9,
                "fee_lamports": 0,
                "compute_units_consumed": 123,
            },
        )
        for field, value in (
            ("signature", "not-base58"),
            ("slot", 0),
            ("feeLamports", True),
            ("feeLamports", -1),
            ("feeLamports", None),
            ("computeUnitsConsumed", None),
        ):
            with self.assertRaises(MODULE.Refusal):
                MODULE.finalized_fact({**honest, field: value}, "hostile")
        decimal = {
            **honest,
            "slot": "9",
            "feeLamports": "0",
            "computeUnitsConsumed": "123",
        }
        self.assertEqual(
            MODULE.finalized_fact(decimal, "decimal", decimal_text=True)[
                "fee_lamports"
            ],
            0,
        )
        for hostile_fee in (None, 0, -1, "-1", "00"):
            with self.assertRaises(MODULE.Refusal):
                MODULE.finalized_fact(
                    {**decimal, "feeLamports": hostile_fee},
                    "hostile decimal",
                    decimal_text=True,
                )

    def test_direct_payout_schedule_is_bounded_unique_and_canonical(self) -> None:
        ordered = tuple(
            sorted(
                (
                    MODULE.PayoutTarget(PUBKEY_B, 1, PUBKEY_C, 100),
                    MODULE.PayoutTarget(PUBKEY_A, 0, PUBKEY_B, 900),
                ),
                key=lambda row: (
                    MODULE.base58_bytes(row.owner, 32, "owner"),
                    row.claim_index,
                    MODULE.base58_bytes(row.recipient, 32, "recipient"),
                ),
            )
        )
        self.assertEqual(MODULE.canonical_payout_schedule(ordered), ordered)
        with self.assertRaisesRegex(MODULE.Refusal, "canonical"):
            MODULE.canonical_payout_schedule(tuple(reversed(ordered)))
        with self.assertRaisesRegex(MODULE.Refusal, "repeats"):
            MODULE.canonical_payout_schedule((ordered[0], ordered[0]))
        with self.assertRaisesRegex(MODULE.Refusal, "one through 32"):
            MODULE.canonical_payout_schedule(())

    def test_typed_direct_schedule_reopens_mutations_and_claims(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            evidence = root / "direct-finalized.json"
            evidence.write_text("{}\n")
            kinds = (
                "replay-setup",
                "token-setup",
                "lookup-create",
                "lookup-extend",
                "lookup-freeze",
                "capability-seal",
                "hot",
            )
            mutations = []
            for index, kind in enumerate(kinds):
                journal = root / f"journal-{index}.json"
                journal.write_text(f'{{"phase":"finalized","index":{index}}}\n')
                mutations.append(
                    {
                        "kind": kind,
                        "prefixLen": "32" if kind == "lookup-extend" else None,
                        "path": str(journal),
                        "sha256": MODULE.sha256_file(journal),
                        "intentSha256": f"{index + 1:02x}" * 32,
                        "schema": "dclutch-test-direct-journal-v1",
                        "completionPointer": "/phase",
                        "completionValue": "finalized",
                        "signature": SIGNATURES[index],
                        "slot": str(index + 1),
                        "feePayer": PUBKEY_A,
                        "feeLamports": "0",
                        "computeUnitsConsumed": str(100 + index),
                    }
                )
            claims = [
                {
                    "owner": PUBKEY_A,
                    "position": PUBKEY_C,
                    "recipientToken": PUBKEY_B,
                    "claimIndex": "0",
                    "quantityAtoms": "900",
                },
                {
                    "owner": PUBKEY_B,
                    "position": PUBKEY_C,
                    "recipientToken": PUBKEY_A,
                    "claimIndex": "1",
                    "quantityAtoms": "100",
                },
            ]
            schedule = {
                "schema": MODULE.DIRECT_PAYOUT_SCHEDULE_SCHEMA,
                "status": "finalized",
                "cluster": "owned-loopback",
                "directEvidence": {
                    "path": str(evidence),
                    "sha256": MODULE.sha256_file(evidence),
                    "schema": MODULE.DIRECT_FINALIZED_SCHEMA,
                    "evidenceSha256": "11" * 32,
                },
                "market": PUBKEY_C,
                "planSha256": "22" * 32,
                "marketInputSha256": "33" * 32,
                "finalizedSlot": "10",
                "mutations": mutations,
                "claims": claims,
                "scheduleSetSha256": MODULE.sha256_bytes(
                    (
                        MODULE.json.dumps(
                            claims, sort_keys=True, separators=(",", ":")
                        )
                        + "\n"
                    ).encode()
                ),
            }
            schedule_path = root / "schedule.json"
            schedule_path.write_text(MODULE.json.dumps(schedule, sort_keys=True) + "\n")
            targets, metrics, decoded = MODULE.accepted_direct_payout_schedule(
                schedule_path, evidence
            )
            self.assertEqual(len(targets), 2)
            self.assertEqual(
                {target.quantity_atoms for target in targets}, {100, 900}
            )
            self.assertEqual(len(metrics), 7)
            self.assertEqual(decoded["status"], "finalized")

            schedule["claims"] = list(reversed(claims))
            schedule["scheduleSetSha256"] = MODULE.sha256_bytes(
                (
                    MODULE.json.dumps(
                        schedule["claims"], sort_keys=True, separators=(",", ":")
                    )
                    + "\n"
                ).encode()
            )
            hostile = root / "hostile.json"
            hostile.write_text(MODULE.json.dumps(schedule, sort_keys=True) + "\n")
            with self.assertRaisesRegex(MODULE.Refusal, "canonical"):
                MODULE.accepted_direct_payout_schedule(hostile, evidence)

            for label, quantity in (
                ("missing", None),
                ("noncanonical", "0900"),
                ("overflow", str(1 << 64)),
            ):
                with self.subTest(quantity=label):
                    hostile_claims = [dict(row) for row in claims]
                    if quantity is None:
                        del hostile_claims[0]["quantityAtoms"]
                    else:
                        hostile_claims[0]["quantityAtoms"] = quantity
                    schedule["claims"] = hostile_claims
                    schedule["scheduleSetSha256"] = MODULE.sha256_bytes(
                        (
                            MODULE.json.dumps(
                                hostile_claims,
                                sort_keys=True,
                                separators=(",", ":"),
                            )
                            + "\n"
                        ).encode()
                    )
                    hostile.write_text(
                        MODULE.json.dumps(schedule, sort_keys=True) + "\n"
                    )
                    with self.assertRaises(MODULE.Refusal):
                        MODULE.accepted_direct_payout_schedule(hostile, evidence)

    def test_resolution_v3_requires_four_cu_bound_mutating_receipts(self) -> None:
        root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        input_path = root / "resolution-input.json"
        input_path.write_bytes(b"exact Resolution input\n")
        receipts = [
            {
                "stage": stage,
                "signature": SIGNATURES[index],
                "slot": 100 + index,
                "feeLamports": 0,
                "computeUnitsConsumed": 100_000 + index,
            }
            for index, stage in enumerate(
                (
                    "submit",
                    "resolution-provider-execute-v1",
                    "core-terminal-accept-v1",
                    "reclaim",
                )
            )
        ]
        checkpoint = {
            "format": MODULE.RESOLUTION_CHECKPOINT_SCHEMA,
            "inputSha256": MODULE.sha256_file(input_path),
            "stagePlan": None,
            "receipts": receipts,
            "verifiedTerminal": True,
        }
        self.assertEqual(
            len(
                MODULE.authenticate_resolution_checkpoint(
                    checkpoint, input_path=input_path
                )
            ),
            4,
        )
        with self.assertRaisesRegex(MODULE.Refusal, "owned-loopback|verified terminal"):
            MODULE.authenticate_resolution_checkpoint(
                {
                    **checkpoint,
                    "format": "dclutch-owned-loopback-flagship-resolution-checkpoint-v1",
                },
                input_path=input_path,
            )
        with self.assertRaisesRegex(MODULE.Refusal, "compute units"):
            MODULE.authenticate_resolution_checkpoint(
                {
                    **checkpoint,
                    "receipts": [
                        {
                            key: value
                            for key, value in row.items()
                            if key != "computeUnitsConsumed"
                        }
                        for row in receipts
                    ],
                },
                input_path=input_path,
            )
        with self.assertRaisesRegex(MODULE.Refusal, "provider-execute/Core-accept"):
            MODULE.authenticate_resolution_checkpoint(
                {**checkpoint, "receipts": receipts[:2]}, input_path=input_path
            )
        with self.assertRaisesRegex(MODULE.Refusal, "advance slots"):
            MODULE.authenticate_resolution_checkpoint(
                {
                    **checkpoint,
                    "receipts": [
                        {**row, "slot": 100 if index == 2 else row["slot"]}
                        for index, row in enumerate(receipts)
                    ],
                },
                input_path=input_path,
            )

    def test_resolution_table_v3_refuses_old_schema_and_missing_cu(self) -> None:
        producer = {
            "planSha256": "1" * 64,
            "campaignEvidenceSha256": "2" * 64,
            "pythFactsSha256": "3" * 64,
            "market": PUBKEY_A,
            "generation": 0,
            "payer": PUBKEY_B,
            "authority": PUBKEY_B,
            "tables": {"submit": {}, "execute": {}, "reclaim": {}},
            "plannedInput": {"accounts": {"market": PUBKEY_A}},
        }
        receipt = {
            "signature": SIGNATURES[0],
            "slot": 10,
            "feeLamports": 0,
            "computeUnitsConsumed": 42,
        }
        journal = {
            "format": MODULE.RESOLUTION_TABLE_SCHEMA,
            "producerIdentitySha256": MODULE.resolution_producer_identity_sha256(
                producer
            ),
            "phase": "finalized",
            "intent": None,
            "intentSha256": None,
            "signedTransactionBase64": None,
            "signedTransactionSha256": None,
            "expectedSignature": None,
            "finalized": None,
            "receipts": [receipt],
        }
        self.assertEqual(
            MODULE.authenticate_resolution_table_journal(
                journal, require_complete=True, producer=producer
            )[0]["compute_units_consumed"],
            42,
        )
        with self.assertRaisesRegex(MODULE.Refusal, "another owned-loopback"):
            MODULE.authenticate_resolution_table_journal(
                {
                    **journal,
                    "format": "dclutch-owned-loopback-flagship-resolution-alt-journal-v2",
                },
                require_complete=True,
                producer=producer,
            )
        with self.assertRaisesRegex(MODULE.Refusal, "compute units"):
            MODULE.authenticate_resolution_table_journal(
                {
                    **journal,
                    "receipts": [
                        {
                            key: value
                            for key, value in receipt.items()
                            if key != "computeUnitsConsumed"
                        }
                    ],
                },
                require_complete=True,
                producer=producer,
            )

    def test_resolution_artifact_chain_refuses_self_consistent_substitutions(
        self,
    ) -> None:
        root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        plan = root / "plan.json"
        campaign = root / "campaign.json"
        pyth = root / "pyth.json"
        input_path = root / "input.json"
        for path, body in (
            (plan, b"plan\n"),
            (campaign, b"campaign\n"),
            (pyth, b"pyth\n"),
            (input_path, b"input\n"),
        ):
            path.write_bytes(body)
        planned_input = {
            "format": MODULE.RESOLUTION_INPUT_SCHEMA,
            "accounts": {"market": PUBKEY_A},
        }
        producer = {
            "format": MODULE.RESOLUTION_PRODUCER_SCHEMA,
            "planSha256": MODULE.sha256_file(plan),
            "campaignEvidenceSha256": MODULE.sha256_file(campaign),
            "pythFactsSha256": MODULE.sha256_file(pyth),
            "observationSlot": 7,
            "observationUnixTimestamp": 8,
            "market": PUBKEY_A,
            "generation": 0,
            "payer": PUBKEY_B,
            "authority": PUBKEY_B,
            "tables": {"submit": {}, "execute": {}, "reclaim": {}},
            "routes": {
                "submit": {"action": "complete"},
                "execute": {"action": "complete"},
                "reclaim": {"action": "complete"},
            },
            "plannedInput": planned_input,
            "flagshipInput": planned_input,
        }
        self.assertEqual(
            MODULE.authenticate_resolution_producer(
                producer,
                require_complete=True,
                plan=plan,
                campaign_evidence=campaign,
                pyth_facts=pyth,
            ),
            producer,
        )

        for field, body in (
            ("planSha256", b"substituted plan\n"),
            ("campaignEvidenceSha256", b"substituted campaign\n"),
            ("pythFactsSha256", b"substituted pyth\n"),
        ):
            with self.subTest(field=field):
                substituted = {**producer, field: MODULE.sha256_bytes(body)}
                with self.assertRaisesRegex(MODULE.Refusal, "exact source file"):
                    MODULE.authenticate_resolution_producer(
                        substituted,
                        require_complete=True,
                        plan=plan,
                        campaign_evidence=campaign,
                        pyth_facts=pyth,
                    )

                table = {
                    "format": MODULE.RESOLUTION_TABLE_SCHEMA,
                    "producerIdentitySha256": (
                        MODULE.resolution_producer_identity_sha256(substituted)
                    ),
                    "phase": "finalized",
                    "intent": None,
                    "intentSha256": None,
                    "signedTransactionBase64": None,
                    "signedTransactionSha256": None,
                    "expectedSignature": None,
                    "finalized": None,
                    "receipts": [],
                }
                with self.assertRaisesRegex(
                    MODULE.Refusal, "another producer identity"
                ):
                    MODULE.authenticate_resolution_table_journal(
                        table, require_complete=True, producer=producer
                    )

        checkpoint = {
            "format": MODULE.RESOLUTION_CHECKPOINT_SCHEMA,
            "inputSha256": MODULE.sha256_bytes(b"substituted input\n"),
            "stagePlan": None,
            "receipts": [],
            "verifiedTerminal": True,
        }
        with self.assertRaisesRegex(MODULE.Refusal, "another exact input file"):
            MODULE.authenticate_resolution_checkpoint(
                checkpoint, input_path=input_path
            )

    def test_payout_input_joins_exact_target_and_refuses_lookup_substitution(
        self,
    ) -> None:
        target = MODULE.PayoutTarget(PUBKEY_A, 1, PUBKEY_B, 7)
        document = {
            "format": MODULE.PAYOUT_INPUT_SCHEMA,
            "market": PUBKEY_C,
            "owner": PUBKEY_A,
            "recipientOwner": PUBKEY_A,
            "recipient": PUBKEY_B,
            "collateralMint": PUBKEY_A,
            "tokenProgram": PUBKEY_B,
            "quantity": "7",
            "claimIndex": 1,
            "transferIndex": 0,
            "parentContext": "00" * 32,
            "custodyContext": "11" * 32,
            "releaseSet": "22" * 32,
            "terminalCertificate": PUBKEY_C,
            "programs": {},
            "records": {},
        }
        self.assertEqual(
            MODULE.authenticate_payout_input(document, target, PUBKEY_C), document
        )
        with self.assertRaisesRegex(MODULE.Refusal, "fields changed"):
            MODULE.authenticate_payout_input(
                {**document, "lookupTable": PUBKEY_C}, target, PUBKEY_C
            )
        with self.assertRaisesRegex(MODULE.Refusal, "target identity"):
            MODULE.authenticate_payout_input(
                {**document, "claimIndex": 2}, target, PUBKEY_C
            )
        with self.assertRaisesRegex(MODULE.Refusal, "frozen schedule"):
            MODULE.authenticate_payout_input(
                {**document, "quantity": "8"}, target, PUBKEY_C
            )
        with self.assertRaisesRegex(MODULE.Refusal, "canonical unsigned decimal"):
            MODULE.authenticate_payout_input(
                {**document, "quantity": "07"}, target, PUBKEY_C
            )
        with self.assertRaisesRegex(MODULE.Refusal, "fields changed"):
            MODULE.authenticate_payout_input(
                {key: value for key, value in document.items() if key != "quantity"},
                target,
                PUBKEY_C,
            )

    def test_resumed_payout_input_remains_bound_to_frozen_schedule_quantity(
        self,
    ) -> None:
        target = MODULE.PayoutTarget(PUBKEY_A, 1, PUBKEY_B, 7)
        document = {
            "format": MODULE.PAYOUT_INPUT_SCHEMA,
            "market": PUBKEY_C,
            "owner": PUBKEY_A,
            "recipientOwner": PUBKEY_A,
            "recipient": PUBKEY_B,
            "collateralMint": PUBKEY_A,
            "tokenProgram": PUBKEY_B,
            "quantity": "7",
            "claimIndex": 1,
            "transferIndex": 0,
            "parentContext": "00" * 32,
            "custodyContext": "11" * 32,
            "releaseSet": "22" * 32,
            "terminalCertificate": PUBKEY_C,
            "programs": {},
            "records": {},
        }
        self.assertEqual(
            MODULE.authenticate_payout_input(document, target, PUBKEY_C), document
        )
        # A resumed producer may observe the Position after an unrelated mutation,
        # but it cannot replace the Direct-owned schedule quantity.
        with self.assertRaisesRegex(MODULE.Refusal, "frozen schedule"):
            MODULE.authenticate_payout_input(
                {**document, "quantity": "6"}, target, PUBKEY_C
            )

    def test_terminal_completion_requires_exact_four_phase_conservation(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            journal_dir = Path(root_text)
            plan = journal_dir / "plan.json"
            evidence = journal_dir / "evidence.json"
            plan.write_text("plan\n")
            evidence.write_text("evidence\n")
            classified = {
                "market": 10,
                "rentCredit": 20,
                "claimsRefund": 30,
                "custodyReplay": 40,
                "hoardVault": 50,
                "expectedRefundDelta": 150,
                "refundWalletBefore": 1_000,
            }
            def account(address: str, lamports: int) -> dict[str, object]:
                row: dict[str, object] = {
                    "address": address,
                    "owner": PUBKEY_A,
                    "lamports": lamports,
                    "executable": False,
                    "dataLen": 0,
                    "dataSha256": MODULE.sha256_bytes(b""),
                    "accountSha256": "",
                }
                row["accountSha256"] = MODULE.semantic_owner_digest(
                    row, "accountSha256", "fixture account"
                )
                return row

            operations = []
            for operation in MODULE.TERMINAL_AGGREGATE_OPERATIONS:
                data = operation.encode()
                operations.append(
                    {
                        "operation": operation,
                        "programId": PUBKEY_A,
                        "accounts": [
                            {"address": PUBKEY_A, "signer": False, "writable": False}
                            for _ in range(35)
                        ],
                        "dataBase64": MODULE.base64.b64encode(data).decode(),
                        "dataSha256": MODULE.sha256_bytes(data),
                        "expectedWireBytes": 800,
                        "exactProtocolAndPayerKeys": 36,
                    }
                )
            campaign = {
                "schema": MODULE.TERMINAL_CAMPAIGN_SCHEMA,
                "cluster": "owned-loopback",
                "genesisHash": PUBKEY_A,
                "rpcUrl": "http://127.0.0.1:8899",
                "planSha256": MODULE.sha256_file(plan),
                "evidenceSha256": MODULE.sha256_file(evidence),
                "payer": PUBKEY_C,
                "lookupTable": PUBKEY_A,
                "lookupTableSha256": "33" * 32,
                "coreProgram": PUBKEY_A,
                "claimsProgram": PUBKEY_B,
                "market": account(PUBKEY_B, 10),
                "rentCredit": account(PUBKEY_A, 20),
                "checkpoint": account(PUBKEY_A, 30),
                "custodyReplay": account(PUBKEY_A, 40),
                "hoardVault": account(PUBKEY_A, 50),
                "sourceReceipt": account(PUBKEY_B, 0),
                "refundWallet": account(PUBKEY_A, 1_000),
                "classifiedLamports": classified,
                "operations": operations,
                "campaignSha256": "",
            }
            campaign["campaignSha256"] = MODULE.semantic_owner_digest(
                campaign, "campaignSha256", "fixture campaign"
            )
            self.assertIs(
                MODULE.authenticate_terminal_campaign(
                    campaign,
                    url="http://127.0.0.1:8899",
                    plan=plan,
                    evidence=evidence,
                    market=PUBKEY_B,
                    payer=PUBKEY_C,
                    source_receipt=PUBKEY_B,
                    lookup_table=PUBKEY_A,
                    genesis_hash=PUBKEY_A,
                ),
                campaign,
            )
            compact = []
            predecessors = (
                "ready",
                "claims-closed",
                "hoard-vault-closed",
                "custody-replay-closed",
            )
            successors = (
                "claims-closed",
                "hoard-vault-closed",
                "custody-replay-closed",
                "complete",
            )
            for index, operation in enumerate(MODULE.TERMINAL_AGGREGATE_OPERATIONS):
                finalization = {
                    "signature": SIGNATURES[index],
                    "finalizedSlot": 50 + index,
                    "packetSha256": f"{index + 1:02x}" * 32,
                    "feeLamports": 0,
                    "computeUnitsConsumed": 100 + index,
                    "poststateSha256": f"{index + 5:02x}" * 32,
                    "checkpointHistorySha256": None,
                }
                journal = {
                    "schema": MODULE.TERMINAL_AGGREGATE_JOURNAL_SCHEMA,
                    "campaignSha256": campaign["campaignSha256"],
                    "operation": operation,
                    "phase": "finalized",
                    "predecessor": predecessors[index],
                    "successor": successors[index],
                    "plannedPrestateSha256": "44" * 32,
                    "intentSha256": "55" * 32,
                    "packet": {"signed": {"packetSha256": finalization["packetSha256"]}},
                    "finalization": finalization,
                    "stateSha256": "",
                }
                journal["stateSha256"] = MODULE.semantic_owner_digest(
                    journal, "stateSha256", "fixture journal"
                )
                (journal_dir / f"{index:02d}-{operation}.json").write_text(
                    MODULE.json.dumps(journal, indent=2) + "\n"
                )
                compact.append(
                    {
                        "operation": operation,
                        "journalSha256": journal["stateSha256"],
                        **{
                            key: value
                            for key, value in finalization.items()
                            if key != "checkpointHistorySha256"
                        },
                    }
                )
            completion = {
                "schema": MODULE.TERMINAL_COMPLETION_SCHEMA,
                "status": "finalized",
                "campaignSha256": campaign["campaignSha256"],
                "market": PUBKEY_B,
                "checkpoint": PUBKEY_A,
                "rentCredit": PUBKEY_A,
                "refundWallet": PUBKEY_A,
                "payer": PUBKEY_C,
                "classifiedLamports": classified,
                "totalTransactionFeesLamports": 0,
                "terminalRefundWalletLamports": 1_150,
                "journals": compact,
                "receiptSha256": "",
            }
            completion["receiptSha256"] = MODULE.semantic_owner_digest(
                completion, "receiptSha256", "fixture completion"
            )
            self.assertEqual(
                len(
                    MODULE.authenticate_terminal_completion(
                        completion,
                        campaign=campaign,
                        journal_dir=journal_dir,
                        market=PUBKEY_B,
                        payer=PUBKEY_C,
                    )["facts"]
                ),
                4,
            )
            for hostile in (
                {**completion, "journals": list(reversed(compact))},
                {**completion, "totalTransactionFeesLamports": 1},
                {**completion, "journals": [{"mutation": {"kind": "aggregate-retirement"}}]},
            ):
                with self.assertRaises(MODULE.Refusal):
                    MODULE.authenticate_terminal_completion(
                        hostile,
                        campaign=campaign,
                        journal_dir=journal_dir,
                        market=PUBKEY_B,
                        payer=PUBKEY_C,
                    )
            sequence_dir = journal_dir / "sequence"
            sequence_dir.mkdir()
            sequence = (
                ("00-alt-create.json", {"kind": "lookup-create"}),
                ("01-alt-extend-032.json", {"kind": "lookup-extend", "prefixLen": 32}),
                ("02-alt-freeze.json", {"kind": "lookup-freeze"}),
                ("10-core-begin-retiring.json", {"kind": "core-begin-retiring"}),
                ("11-direct-begin-retiring.json", {"kind": "direct-begin-retiring"}),
                ("13-resolution-close-fund.json", {"kind": "resolution-close-fund"}),
                ("14-direct-close-capability.json", {"kind": "direct-close-capability"}),
                ("15-retirement-replay-handoff.json", {"kind": "retirement-replay-handoff"}),
            )
            for index, (name, mutation) in enumerate(sequence):
                intent = {"mutation": mutation}
                finalized = {
                    "signature": SIGNATURES[index + 4],
                    "slot": 100 + index,
                    "feeLamports": 0,
                    "computeUnitsConsumed": 200 + index,
                    "packetSha256": "66" * 32,
                    "poststate": {},
                }
                journal = {
                    "schema": MODULE.TERMINAL_JOURNAL_SCHEMA,
                    "cluster": "owned-loopback",
                    "rpcUrl": "http://127.0.0.1:8899",
                    "authorizedMutation": True,
                    "stateSha256": "",
                    "phase": "finalized",
                    "intentSha256": MODULE.sha256_bytes(
                        MODULE.json.dumps(
                            intent, ensure_ascii=False, separators=(",", ":")
                        ).encode()
                    ),
                    "intent": intent,
                    "signedPacketBase64": "AQ==",
                    "expectedSignature": finalized["signature"],
                    "finalized": finalized,
                }
                journal["stateSha256"] = MODULE.semantic_owner_digest(
                    journal, "stateSha256", "sequence fixture"
                )
                (sequence_dir / name).write_text(
                    MODULE.json.dumps(journal, indent=2) + "\n"
                )
            self.assertEqual(
                len(
                    MODULE.terminal_sequence_finalized_history(
                        sequence_dir, url="http://127.0.0.1:8899"
                    )
                ),
                8,
            )

    def test_terminal_stdout_joins_exact_completion_path_and_hash(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            journal_dir = root / "journals"
            journal_dir.mkdir()
            completion_path = root / "completion.json"
            completion_path.write_text('{"status":"finalized"}\n')
            campaign_path = root / "campaign.json"
            campaign_path.write_text('{"schema":"campaign"}\n')
            campaign = {"campaignSha256": "11" * 32}
            summary = {
                "schema": MODULE.TERMINAL_PROGRESS_SCHEMA,
                "status": "finalized",
                "campaign": str(campaign_path),
                "campaignSha256": campaign["campaignSha256"],
                "journalDirectory": str(journal_dir),
                "completion": str(completion_path),
                "completionSha256": MODULE.sha256_file(completion_path),
                "message": "Aggregate retirement finalized through prepare, close-vault, close-replay, and finish; exact rent/refund conservation reverified.",
            }
            self.assertEqual(
                MODULE.authenticate_terminal_stdout(
                    summary,
                    campaign=campaign,
                    campaign_path=campaign_path,
                    completion_path=completion_path,
                    journal_dir=journal_dir,
                ),
                summary,
            )
            with self.assertRaisesRegex(MODULE.Refusal, "completion path, hash"):
                MODULE.authenticate_terminal_stdout(
                    {**summary, "completionSha256": "00" * 32},
                    campaign=campaign,
                    campaign_path=campaign_path,
                    completion_path=completion_path,
                    journal_dir=journal_dir,
                )
            with self.assertRaisesRegex(MODULE.Refusal, "fields changed"):
                MODULE.authenticate_terminal_stdout(
                    {**summary, "extra": True},
                    campaign=campaign,
                    campaign_path=campaign_path,
                    completion_path=completion_path,
                    journal_dir=journal_dir,
                )

    def test_full_probe_is_distinct_and_admits_exactly_one_seed(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            fake = root / "fake"
            fake.write_text("x")
            fake.chmod(0o755)
            repo = root / "repo"
            release = root / "release"
            repo.mkdir()
            release.mkdir()
            common = [
                "--repo",
                str(repo),
                "--release-root",
                str(release),
                "--validator",
                str(fake),
                "--solana",
                str(fake),
                "--work",
                str(root / "work"),
                "--through",
                "full-probe",
            ]
            with self.assertRaisesRegex(MODULE.Refusal, "exactly one"):
                MODULE.parse([*common, "--seeds", "2"])
            _paths, seeds, through, hold, hot_cu_profile = MODULE.parse(
                [*common, "--seeds", "1", "--hot-cu-profile"]
            )
            self.assertEqual((seeds, through, hold), (1, "full-probe", False))
            self.assertTrue(hot_cu_profile)
            ordinary = [*common, "--seeds", "1"]
            _paths, seeds, through, hold, hot_cu_profile = MODULE.parse(ordinary)
            self.assertEqual((seeds, through, hold), (1, "full-probe", False))
            self.assertFalse(hot_cu_profile)
            with self.assertRaisesRegex(MODULE.Refusal, "diagnostic-only"):
                MODULE.parse(
                    [
                        *ordinary,
                        "--through",
                        "participant",
                        "--hot-cu-profile",
                    ]
                )
            self.assertNotEqual(MODULE.FULL_PROBE_SCHEMA, MODULE.SCHEMA)
            self.assertNotEqual(MODULE.FULL_PROBE_RUN_SCHEMA, MODULE.RUN_SCHEMA)

    def test_stage_receipt_preserves_failure_without_calling_it_passed(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            run = Path(root_text)
            (run / "stages").mkdir()
            with self.assertRaisesRegex(MODULE.Refusal, "status 7"):
                MODULE.run_stage(
                    run, 1, "hostile", ["/bin/sh", "-c", "echo wall >&2; exit 7"]
                )
            receipt = MODULE.read_unique_json(
                run / "stages" / "01-hostile" / "receipt.json", "receipt"
            )
            self.assertEqual(receipt["exit_status"], 7)
            self.assertTrue((run / "stages" / "01-hostile" / "stderr.bin").is_file())

    def test_stage_supervisor_observes_real_sigkill_and_restarts_exact_argv(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            run = Path(root_text).resolve()
            (run / "stages").mkdir()
            journal = run / "target.json"
            journal.write_text('{"phase":"dispatching"}')
            receipt_path = run / "fault.json"
            signature = SIGNATURES[0]
            script = """
import hashlib, json, os, pathlib, time
journal = pathlib.Path(os.environ.get("DCLUTCH_CHAOS_FAULT_JOURNAL_V1", %r))
receipt = os.environ.get("DCLUTCH_CHAOS_FAULT_RECEIPT_V1")
if receipt is None:
    journal.write_text('{"phase":"finalized"}')
    print("recovered")
    raise SystemExit(0)
body = journal.read_bytes()
row = {
    "schema": "dclutch-owned-loopback-chaos-fault-boundary-v1",
    "status": "armed",
    "caseId": os.environ["DCLUTCH_CHAOS_FAULT_CASE_V1"],
    "targetMutation": os.environ["DCLUTCH_CHAOS_FAULT_MUTATION_V1"],
    "boundary": os.environ["DCLUTCH_CHAOS_FAULT_BOUNDARY_V1"],
    "processId": os.getpid(),
    "durablePhase": "dispatching",
    "journalPath": str(journal),
    "journalBeforeKillSha256": hashlib.sha256(body).hexdigest(),
    "intentSha256": "11" * 32,
    "packetSha256": "22" * 32,
    "signature": %r,
    "sendCountBeforeKill": 0,
}
path = pathlib.Path(receipt)
with path.open("x") as target:
    json.dump(row, target, separators=(",", ":"))
    target.flush()
    os.fsync(target.fileno())
while True:
    time.sleep(1)
""" % (str(journal), signature)
            supervisor = MODULE.ChaosStageSupervisor(
                case_id="founding:dispatching-before-send",
                mutation="dcltgmf3",
                boundary="dispatching-before-send",
                journal=journal,
                receipt=receipt_path,
            )
            prior = MODULE._ACTIVE_CHAOS_SUPERVISOR
            MODULE._ACTIVE_CHAOS_SUPERVISOR = supervisor
            try:
                result = MODULE.run_stage(
                    run, 1, "faulted", [sys.executable, "-c", script]
                )
            finally:
                MODULE._ACTIVE_CHAOS_SUPERVISOR = prior
            self.assertEqual(result.returncode, 0)
            self.assertEqual(result.stdout.strip(), b"recovered")
            self.assertTrue(supervisor.fired)
            self.assertEqual(supervisor.facts["exitCode"], -MODULE.signal.SIGKILL)
            stage = run / "stages" / "01-faulted"
            self.assertTrue((stage / "fault-stdout.bin").is_file())
            receipt = MODULE.read_unique_json(stage / "receipt.json", "stage receipt")
            self.assertEqual(
                receipt["chaos_fault"]["signal"], MODULE.signal.SIGKILL
            )
            self.assertEqual(receipt["chaos_recovery"]["sendCountAfterRestart"], 1)
            self.assertNotEqual(
                receipt["chaos_recovery"]["journalAfterFinalizationSha256"],
                receipt["chaos_fault"]["journalBeforeKillSha256"],
            )

    def test_finalized_chaos_target_facts_authenticate_schema_and_packet(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            packet = b"one exact signed packet"
            packet_base64 = MODULE.base64.b64encode(packet).decode("ascii")
            packet_sha256 = MODULE.sha256_bytes(packet)
            direct = root / "0007-hot.json"
            MODULE.write_json_new(
                direct,
                {
                    "schema": "dclutch-owned-loopback-direct-trade-journal-v1",
                    "stage": "hot",
                    "phase": "finalized",
                    "intentSha256": "11" * 32,
                    "signedPacketBase64": packet_base64,
                    "expectedSignature": SIGNATURES[0],
                    "finalizedSlot": 91,
                },
            )
            facts = MODULE.finalized_chaos_target_facts("hot", direct)
            self.assertEqual(facts["packetSha256"], packet_sha256)
            self.assertEqual(facts["signature"], SIGNATURES[0])
            self.assertEqual(facts["finalizedSlot"], 91)

            hostile = root / "hostile.json"
            MODULE.write_json_new(
                hostile,
                {
                    "schema": "dclutch-devnet-direct-trade-journal-v1",
                    "stage": "hot",
                    "phase": "finalized",
                    "intentSha256": "11" * 32,
                    "signedPacketBase64": packet_base64,
                    "expectedSignature": SIGNATURES[0],
                    "finalizedSlot": 91,
                },
            )
            with self.assertRaisesRegex(MODULE.Refusal, "schema or stage"):
                MODULE.finalized_chaos_target_facts("hot", hostile)

    def test_control_chaos_case_uses_authenticated_aggregate_finish(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            journal = root / "retirement" / "aggregate-journals" / "03-finish.json"
            journal.parent.mkdir(parents=True)
            MODULE.write_json_new(
                journal,
                {
                    "schema": "dclutch-owned-loopback-aggregate-retirement-journal-v1",
                    "operation": "finish",
                    "phase": "finalized",
                    "intentSha256": "11" * 32,
                    "packet": {
                        "signed": {
                            "packetSha256": "22" * 32,
                            "signature": SIGNATURES[0],
                        }
                    },
                    "finalization": {"finalizedSlot": 123},
                },
            )
            result = root / "RESULT.json"
            MODULE.write_json_new(result, {"status": "passed"})
            contract = MODULE.load_chaos_contract(
                type("RepoPaths", (), {"repo": MODULE_PATH.parents[3]})()
            )
            case = MODULE.build_chaos_case(
                contract=contract,
                spec=contract.MATRIX[0],
                index=1,
                run=root,
                result_path=result,
                name="chaos-01",
                genesis_hash=PUBKEY_A,
                session_identity_sha256="33" * 32,
                source_revision="44" * 20,
                checked_release_gate_sha256="55" * 32,
                supervisor=None,
            )
            self.assertEqual(case["targetMutation"], "complete-life")
            self.assertEqual(case["targetPacketSha256"], "22" * 32)
            self.assertEqual(case["targetSendCount"], 1)
            self.assertIsNone(case["fault"])
            self.assertIsNone(case["recovery"])

    def test_final_activity_uses_historical_direct_owner_and_exact_stage_order(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            run = Path(root_text).resolve()
            (run / "stages").mkdir()
            direct_journals = run / "direct" / "direct-trade-journal"
            direct_journals.mkdir(parents=True)

            def source(path: Path) -> Path:
                path.parent.mkdir(parents=True, exist_ok=True)
                MODULE.write_json_new(path, {"source": path.name})
                return path

            plan = source(run / "plan.json")
            profile = source(run / "validator-profile.json")
            founding = source(run / "founding.json")
            source(run / "participant.json")
            source(direct_journals / "0001-lookup-freeze.json")
            source(direct_journals / "0002-capability-seal.json")
            direct_evidence = source(run / "direct" / "finalized.json")
            fee = source(run / "direct" / "fee.json")
            resolution_input = source(run / "resolution" / "input.json")
            resolution_checkpoint = source(run / "resolution" / "checkpoint.json")
            payout_input = source(run / "payouts" / "000" / "input.json")
            payout_evidence = source(run / "payouts" / "000" / "evidence.json")
            retirement = source(run / "retirement" / "completion.json")
            direct = {
                "finalized_evidence": str(direct_evidence),
                "fee_settlement": {"evidence": str(fee)},
            }
            post_direct = {
                "resolution": {
                    "input": str(resolution_input),
                    "checkpoint": str(resolution_checkpoint),
                },
                "payouts": [
                    {"input": str(payout_input), "evidence": str(payout_evidence)}
                ],
                "retirement": {"completion": str(retirement)},
            }
            observed_sources: dict[str, list[tuple[str, Path]]] = {}

            def wrap(
                _run: Path,
                _paths: MODULE.Paths,
                _url: str,
                stage: str,
                sources: list[tuple[str, Path]],
                ordinal: int,
            ) -> tuple[Path, int]:
                observed_sources[stage] = sources
                output = run / "activity" / f"{stage}.json"
                MODULE.write_json_new(
                    output,
                    {
                        "schema": "dclutch-owned-loopback-activity-stage-completion-v1",
                        "stage": stage,
                        "status": "finalized",
                    },
                )
                return output, ordinal + 1

            def stage_command(
                _run: Path, _ordinal: int, label: str, argv: list[str]
            ) -> dict[str, object]:
                output = Path(argv[argv.index("--output") + 1])
                documents: dict[str, dict[str, object]] = {
                    "activity-manifest": {
                        "schema": "dclutch-owned-loopback-activity-reconcile-manifest-v1"
                    },
                    "activity-finalized-capture": {
                        "schema": "dclutch-owned-loopback-captured-finalized-rpc-v1",
                        "commitment": "finalized",
                    },
                    "pyth-provider-closure": {
                        "schema": "dclutch-owned-loopback-pyth-provider-closure-v1",
                        "status": "finalized",
                    },
                    "activity-lifecycle-session": {
                        "schema": "dclutch-owned-loopback-private-lifecycle-session-v1",
                        "status": "finalized",
                    },
                }
                document = documents[label]
                MODULE.write_json_new(output, document)
                return document

            paths = MODULE.Paths(
                repo=MODULE_PATH.parents[3],
                release_root=run,
                expected_release_gate_sha256=None,
                expected_release_source_revision=None,
                expected_release_source_tree_sha256=None,
                bootstrap=source(run / "bootstrap"),
                reuse_bootstrap_work=None,
                validator=Path("/bin/true"),
                solana=Path("/bin/true"),
                work=run,
            )
            with mock.patch.object(
                MODULE, "run_activity_stage_wrapper", side_effect=wrap
            ), mock.patch.object(MODULE, "run_json_stage", side_effect=stage_command):
                controller, ordinal = MODULE.run_final_activity_evidence(
                    run,
                    paths,
                    "http://127.0.0.1:31432",
                    plan,
                    profile,
                    direct,
                    post_direct,
                    20,
                )
            self.assertEqual(ordinal, 30)
            self.assertEqual(observed_sources["direct"][0], ("campaign", founding))
            self.assertEqual(
                [role for role, _path in observed_sources["payout"]],
                ["direct-evidence", "input-000", "evidence-000"],
            )
            descriptors = MODULE.read_unique_json(
                Path(controller["stage_descriptors"]), "activity descriptors"
            )
            self.assertEqual(
                tuple(row["semanticRole"] for row in descriptors["journals"]),
                MODULE.ACTIVITY_DESCRIPTOR_ROLES,
            )

    def test_final_receipt_emits_exact_eleven_role_order(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            run = root / "run"
            (run / "stages").mkdir(parents=True)
            (run / "activity").mkdir()
            direct_journals = run / "direct" / "direct-trade-journal"
            direct_journals.mkdir(parents=True)

            def source(path: Path) -> Path:
                path.parent.mkdir(parents=True, exist_ok=True)
                MODULE.write_json_new(path, {"source": path.name})
                return path

            wrappers = {
                stage: source(run / "activity" / f"{stage}.json")
                for stage in (
                    "founding",
                    "participant",
                    "direct",
                    "resolution",
                    "payout",
                    "retirement",
                )
            }
            source(direct_journals / "0001-lookup-freeze.json")
            source(direct_journals / "0002-capability-seal.json")
            pyth = source(run / "pyth-provisioning.json")
            session = source(run / "activity" / "lifecycle-session.json")
            capture = source(run / "activity" / "capture.json")
            manifest = source(run / "activity" / "manifest.json")
            provider = source(run / "activity" / "provider.json")
            plan = source(run / "plan.json")
            facts = source(run / "pyth-facts.json")
            gate = source(root / "gate.json")
            chaos = source(root / "chaos.json")
            MODULE.write_json_new(
                run / "RESULT.json",
                {
                    "plan": str(plan),
                    "final_activity": {
                        "stage_wrappers": {
                            stage: {
                                "path": str(path),
                                "sha256": MODULE.sha256_file(path),
                            }
                            for stage, path in wrappers.items()
                        },
                        "session": str(session),
                        "capture": str(capture),
                        "manifest": str(manifest),
                        "provider_closure": str(provider),
                    },
                    "post_direct": {"pyth": {"facts": str(pyth)}},
                },
            )
            paths = MODULE.Paths(
                repo=MODULE_PATH.parents[3],
                release_root=root,
                expected_release_gate_sha256=None,
                expected_release_source_revision=None,
                expected_release_source_tree_sha256=None,
                bootstrap=source(root / "bootstrap"),
                reuse_bootstrap_work=None,
                validator=Path("/bin/true"),
                solana=Path("/bin/true"),
                work=root,
            )
            observed_roles: tuple[str, ...] | None = None

            def stage_command(
                _run: Path, _ordinal: int, label: str, argv: list[str]
            ) -> dict[str, object]:
                nonlocal observed_roles
                self.assertEqual(label, "lifecycle-receipt")
                descriptors = Path(argv[argv.index("--stage-journal-descriptors") + 1])
                rows = MODULE.read_unique_json(descriptors, "final descriptors")["journals"]
                observed_roles = tuple(row["semanticRole"] for row in rows)
                output = Path(argv[argv.index("--output") + 1])
                document: dict[str, object] = {
                    "schema": "dclutch-owned-loopback-reconcile-session-receipt-v1",
                    "status": "finalized",
                }
                MODULE.write_json_new(output, document)
                return document

            with mock.patch.object(MODULE, "run_json_stage", side_effect=stage_command):
                receipt = MODULE.finalize_lifecycle_receipt(
                    run=run,
                    paths=paths,
                    gate_path=gate,
                    gate_digest="11" * 32,
                    source_revision="22" * 20,
                    chaos_session_path=chaos,
                )
            self.assertEqual(observed_roles, MODULE.FINAL_LIFECYCLE_DESCRIPTOR_ROLES)
            self.assertEqual(receipt["run"], "run")

    def test_maker_close_binds_direct_generation_root_and_source_digest(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            direct = root / "direct.json"
            MODULE.write_json_new(direct, {"generation": 7})
            evidence = {
                "schema": "dclutch-direct-close-maker-evidence-v1",
                "cluster": "owned-loopback",
                "market": PUBKEY_C,
                "generation": 7,
                "directRoot": PUBKEY_A,
                "directEvidenceSha256": MODULE.sha256_file(direct),
                "makerReplay": PUBKEY_B,
                "plan": {
                    "maker": PUBKEY_C,
                    "rentOwner": PUBKEY_A,
                    "rentPrincipal": 100,
                    "unclassifiedDonation": 0,
                    "totalCredit": 100,
                    "beneficiaryLamportsAfter": 100,
                    "remainingOpenMakerRoots": 0,
                    "requestDigest": "11" * 32,
                    "expectedPostRootDigest": "22" * 32,
                    "expectedReceipt": "00",
                },
                "alreadyClosed": False,
                "landed": {
                    "signature": SIGNATURES[0],
                    "slot": 1,
                    "computeUnitsConsumed": 1,
                    "feeLamports": 5_000,
                },
            }
            accepted = MODULE.authenticate_direct_close_maker(
                evidence,
                child={"maker": PUBKEY_C, "replay": PUBKEY_B},
                market=PUBKEY_C,
                direct_evidence=direct,
                direct_root=PUBKEY_A,
                remaining_roots=0,
            )
            self.assertEqual(accepted["finalized"]["slot"], 1)
            for field, hostile in (
                ("generation", 8),
                ("directRoot", PUBKEY_B),
                ("directEvidenceSha256", "33" * 32),
            ):
                substituted = copy.deepcopy(evidence)
                substituted[field] = hostile
                with self.assertRaisesRegex(MODULE.Refusal, "generation, root"):
                    MODULE.authenticate_direct_close_maker(
                        substituted,
                        child={"maker": PUBKEY_C, "replay": PUBKEY_B},
                        market=PUBKEY_C,
                        direct_evidence=direct,
                        direct_root=PUBKEY_A,
                        remaining_roots=0,
                    )


class FrozenRoutingTableTests(unittest.TestCase):
    """The table `--through participant` could not complete without.

    The admission message does not fit a legacy transaction: without a routing
    table it refuses `admission message compilation: PacketTooLarge` AFTER the
    prefund transfer has landed, which is exactly where the participant probe
    stopped on 2026-08-31.
    """

    @staticmethod
    def table(addresses, *, frozen: bool) -> str:
        import base64

        raw = bytearray(MODULE.ALT_HEADER_BYTES_V1)
        raw[0:4] = (1).to_bytes(4, "little")
        raw[MODULE.ALT_AUTHORITY_FLAG_OFFSET_V1] = 0 if frozen else 1
        if not frozen:
            raw[22:54] = bytes(range(32))
        for entry in addresses:
            raw += entry
        return base64.b64encode(bytes(raw)).decode()

    def accounts(self, rows):
        return [
            {"pubkey": pubkey, "account": {"data": [body, "base64"]}}
            for pubkey, body in rows
        ]

    def test_the_frozen_table_containing_the_market_is_the_one_chosen(self):
        market = bytes([9]) * 32
        other = bytes([8]) * 32
        rows = self.accounts([
            # Still extendable, and it does contain the market: an authority
            # that can still add an address is not the table the founding
            # committed to.
            (PUBKEY_A, self.table([market, other], frozen=False)),
            # Frozen, but about another market entirely.
            (PUBKEY_B, self.table([other], frozen=True)),
            (PUBKEY_C, self.table([other, market], frozen=True)),
        ])
        with mock.patch.object(MODULE, "rpc", return_value=rows):
            self.assertEqual(
                MODULE.frozen_founding_routing_table("http://127.0.0.1:1/", base58(market)),
                PUBKEY_C,
            )

    def test_no_frozen_table_refuses_rather_than_compiling_a_packet_that_cannot_fit(self):
        market = bytes([9]) * 32
        rows = self.accounts([(PUBKEY_A, self.table([market], frozen=False))])
        with mock.patch.object(MODULE, "rpc", return_value=rows):
            with self.assertRaises(MODULE.Refusal) as caught:
                MODULE.frozen_founding_routing_table("http://127.0.0.1:1/", base58(market))
        self.assertIn("does not fit a legacy transaction", str(caught.exception))

    def test_two_candidate_tables_refuse_rather_than_pick_one(self):
        """Passing all five founding tables refuses `DuplicateAddress`, so the
        contract is exactly one table and an ambiguity is not a coin toss."""
        market = bytes([9]) * 32
        rows = self.accounts([
            (PUBKEY_A, self.table([market], frozen=True)),
            (PUBKEY_B, self.table([market], frozen=True)),
        ])
        with mock.patch.object(MODULE, "rpc", return_value=rows):
            with self.assertRaises(MODULE.Refusal) as caught:
                MODULE.frozen_founding_routing_table("http://127.0.0.1:1/", base58(market))
        self.assertIn("2 frozen lookup tables", str(caught.exception))

    def test_a_short_or_undecodable_account_is_skipped_rather_than_crashing(self):
        market = bytes([9]) * 32
        rows = [
            {"pubkey": PUBKEY_A, "account": {"data": ["", "base64"]}},
            {"pubkey": PUBKEY_B, "account": {}},
        ] + self.accounts([(PUBKEY_C, self.table([market], frozen=True))])
        with mock.patch.object(MODULE, "rpc", return_value=rows):
            self.assertEqual(
                MODULE.frozen_founding_routing_table("http://127.0.0.1:1/", base58(market)),
                PUBKEY_C,
            )


class ValidatorLaunchTests(unittest.TestCase):
    def test_validator_profile_binds_exact_loopback_loader_facts(self):
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            ledger = root / "ledger"
            accounts = root / "accounts"
            ledger.mkdir()
            accounts.mkdir()
            plan = root / "plan.json"
            plan.write_text('{"schema":"test-plan"}')
            paths = MODULE.Paths(
                repo=MODULE_PATH.parents[3],
                release_root=root,
                expected_release_gate_sha256=None,
                expected_release_source_revision=None,
                expected_release_source_tree_sha256=None,
                bootstrap=root / "bootstrap",
                reuse_bootstrap_work=None,
                validator=Path("/bin/true"),
                solana=Path("/bin/true"),
                work=root / "work",
            )
            completed = MODULE.subprocess.CompletedProcess(
                args=["/bin/true", "--version"],
                returncode=0,
                stdout=b"solana-test-validator 2.2.1\n",
                stderr=b"",
            )
            with mock.patch.object(MODULE.subprocess, "run", return_value=completed):
                profile_path = MODULE.write_local_validator_profile(
                    paths,
                    ledger=ledger,
                    plan=plan,
                    account_dir=str(accounts),
                    port=31_432,
                )
            profile = MODULE.read_unique_json(profile_path, "validator profile")
            self.assertEqual(
                profile["schema"], "dclutch-successor-local-validator-profile-v1"
            )
            self.assertEqual(profile["network"]["rpc_url"], "http://127.0.0.1:31432")
            self.assertEqual(profile["network"]["faucet_port"], 31_434)
            self.assertEqual(profile["network"]["dynamic_port_range"], "31442-31473")
            self.assertEqual(
                profile["validator"]["version"], "solana-test-validator 2.2.1"
            )
            self.assertEqual(
                [row["name"] for row in profile["programs"][:7]],
                [
                    "registry",
                    "core",
                    "claims",
                    "trading",
                    "resolution",
                    "custody",
                    "rent-credit",
                ],
            )
            provider_rows = profile["programs"][7:]
            self.assertEqual(
                [row["name"] for row in provider_rows],
                ["pyth-receiver", "pyth-router"],
            )
            for row in provider_rows:
                self.assertIsNone(row["upgrade_authority"])
                self.assertEqual(row["deployment_slot"], 0)
                self.assertRegex(row["programdata_sha256"], r"^[0-9a-f]{64}$")
                self.assertRegex(row["elf_sha256"], r"^[0-9a-f]{64}$")

    def test_the_session_keeps_its_transaction_history(self):
        """A purge between two stages strands a journal permanently.

        Every driver here re-verifies its earlier stages from transaction
        history, and the Direct trade and the flagship resolution advance one
        durable action per invocation with minutes between them. Under the
        validator's own default those roots are purged in multi-thousand-slot
        chunks and the later stage can no longer authenticate the earlier one --
        a failure no retry recovers, because the history is gone rather than
        late.
        """
        argv = MODULE.validator_argv(
            Path("/bin/true"), Path("/tmp/ledger"), "/tmp/accounts", PUBKEY_A, 31432,
        )
        self.assertIn("--limit-ledger-size", argv)
        cap = argv[argv.index("--limit-ledger-size") + 1]
        self.assertEqual(cap, str(MODULE.VALIDATOR_LEDGER_SHRED_CAP_V1))
        self.assertGreater(int(cap), 1_000_000)
        # The port block is still derived from one base, which is what makes a
        # ledger addressable by path alone.
        self.assertEqual(argv[argv.index("--rpc-port") + 1], "31432")
        self.assertEqual(argv[argv.index("--faucet-port") + 1], "31434")


if __name__ == "__main__":
    unittest.main()
