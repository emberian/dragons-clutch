#!/usr/bin/env python3

from __future__ import annotations

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


class PrivateValidatorLifecycleTests(unittest.TestCase):
    def test_full_help_requires_every_dispatched_final_evidence_command(self) -> None:
        probe_required = (
            *MODULE.FOUNDING_PARTICIPANT_COMMANDS,
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
                {"compute_units": {"founding-dcltgmf2": 10, "pyth-verify": 21}},
                {"compute_units": {"founding-dcltgmf2": 12, "pyth-verify": 22}},
            ]
        )
        self.assertEqual(
            report["founding-dcltgmf2"],
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
            ["--keypair-core-upgrade-authority", "/owned/core.json"],
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
        self.assertNotIn("--keypair-campaign-payer", admin)
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
            paths, seeds, through, hold_participant = MODULE.parse(
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

    def test_full_mode_refuses_until_all_callers_are_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            fake = root / "fake"
            fake.write_text("x")
            fake.chmod(0o755)
            repo = root / "repo"
            release = root / "release"
            repo.mkdir()
            release.mkdir()
            with self.assertRaisesRegex(MODULE.Refusal, "seventeen-case"):
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
                    ]
                )

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
                    MODULE.PayoutTarget(PUBKEY_B, 1, PUBKEY_C),
                    MODULE.PayoutTarget(PUBKEY_A, 0, PUBKEY_B),
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

    def test_resolution_v3_requires_four_cu_bound_mutating_receipts(self) -> None:
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
            "inputSha256": "a" * 64,
            "stagePlan": None,
            "receipts": receipts,
            "verifiedTerminal": True,
        }
        self.assertEqual(len(MODULE.authenticate_resolution_checkpoint(checkpoint)), 4)
        with self.assertRaisesRegex(MODULE.Refusal, "owned-loopback|verified terminal"):
            MODULE.authenticate_resolution_checkpoint(
                {
                    **checkpoint,
                    "format": "dclutch-owned-loopback-flagship-resolution-checkpoint-v1",
                }
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
                }
            )
        with self.assertRaisesRegex(MODULE.Refusal, "provider-execute/Core-accept"):
            MODULE.authenticate_resolution_checkpoint(
                {**checkpoint, "receipts": receipts[:2]}
            )
        with self.assertRaisesRegex(MODULE.Refusal, "advance slots"):
            MODULE.authenticate_resolution_checkpoint(
                {
                    **checkpoint,
                    "receipts": [
                        {**row, "slot": 100 if index == 2 else row["slot"]}
                        for index, row in enumerate(receipts)
                    ],
                }
            )

    def test_resolution_table_v3_refuses_old_schema_and_missing_cu(self) -> None:
        receipt = {
            "signature": SIGNATURES[0],
            "slot": 10,
            "feeLamports": 0,
            "computeUnitsConsumed": 42,
        }
        journal = {
            "format": MODULE.RESOLUTION_TABLE_SCHEMA,
            "producerIdentitySha256": "b" * 64,
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
                journal, require_complete=True
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
            )

    def test_payout_input_joins_exact_target_and_refuses_lookup_substitution(
        self,
    ) -> None:
        target = MODULE.PayoutTarget(PUBKEY_A, 1, PUBKEY_B)
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
            _paths, seeds, through, hold = MODULE.parse([*common, "--seeds", "1"])
            self.assertEqual((seeds, through, hold), (1, "full-probe", False))
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


if __name__ == "__main__":
    unittest.main()
