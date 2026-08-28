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
        required = (
            *MODULE.FOUNDING_PARTICIPANT_COMMANDS,
            MODULE.PYTH_PROVISION_COMMAND,
            MODULE.FLAGSHIP_RESOLUTION_COMMAND,
            MODULE.PAYOUT_INPUT_COMMAND,
            MODULE.PAYOUT_EXECUTE_COMMAND,
            MODULE.TERMINAL_RETIREMENT_COMMAND,
            *MODULE.FINAL_EVIDENCE_COMMANDS,
        )
        with tempfile.TemporaryDirectory() as root_text:
            bootstrap = Path(root_text) / "fake-bootstrap"

            def install(commands: tuple[str, ...]) -> None:
                bootstrap.write_text(
                    "#!/bin/sh\nprintf '%s\\n' "
                    + " ".join(f"'{command}'" for command in commands)
                    + "\n"
                )
                bootstrap.chmod(0o755)

            install(required)
            self.assertEqual(len(MODULE.command_surface(bootstrap, "full-probe")), 64)
            for omitted in MODULE.FINAL_EVIDENCE_COMMANDS:
                with self.subTest(omitted=omitted):
                    install(
                        tuple(command for command in required if command != omitted)
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
            "campaign_keypairs": {
                "founding-founder": "/owned/founder.json",
                "substituted-founder": "/owned/substituted.json",
            }
        }
        self.assertEqual(
            MODULE.key_flags(report),
            [
                "--keypair-founding-founder",
                "/owned/founder.json",
                "--keypair-substituted-founder",
                "/owned/substituted.json",
            ],
        )
        with self.assertRaisesRegex(MODULE.Refusal, "Rust-owned"):
            MODULE.key_flags({"keypairs": report["campaign_keypairs"]})

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
            with self.assertRaisesRegex(
                MODULE.Refusal, "owned-loopback Direct producer"
            ):
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
        self.assertEqual(
            MODULE.DEVELOPMENT_FEE_RECIPIENT_ROLE, "founding-source-funder"
        )
        self.assertEqual(
            MODULE.PROTOCOL_CREATED_KEY_ROLES,
            ("collateral-mint", "collateral-wallet", "founding-source-funder"),
        )
        self.assertTrue(
            set(MODULE.LOCAL_AIRDROP_ROLES).isdisjoint(
                MODULE.PROTOCOL_CREATED_KEY_ROLES
            )
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
            "feeLamports": 5_000,
            "computeUnitsConsumed": 123,
        }
        self.assertEqual(
            MODULE.finalized_fact(honest, "honest"),
            {
                "signature": SIGNATURES[0],
                "slot": 9,
                "fee_lamports": 5_000,
                "compute_units_consumed": 123,
            },
        )
        for field, value in (
            ("signature", "not-base58"),
            ("slot", 0),
            ("feeLamports", True),
            ("computeUnitsConsumed", None),
        ):
            with self.assertRaises(MODULE.Refusal):
                MODULE.finalized_fact({**honest, field: value}, "hostile")

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

    def test_resolution_v2_requires_three_cu_bound_mutating_receipts(self) -> None:
        receipts = [
            {
                "stage": stage,
                "signature": SIGNATURES[index],
                "slot": 100 + index,
                "feeLamports": 5_000,
                "computeUnitsConsumed": 100_000 + index,
            }
            for index, stage in enumerate(("submit", "execute", "reclaim"))
        ]
        checkpoint = {
            "format": MODULE.RESOLUTION_CHECKPOINT_SCHEMA,
            "inputSha256": "a" * 64,
            "stagePlan": None,
            "receipts": receipts,
            "verifiedTerminal": True,
        }
        self.assertEqual(len(MODULE.authenticate_resolution_checkpoint(checkpoint)), 3)
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
        with self.assertRaisesRegex(MODULE.Refusal, "submit/execute/reclaim"):
            MODULE.authenticate_resolution_checkpoint(
                {**checkpoint, "receipts": receipts[:2]}
            )

    def test_resolution_table_v2_refuses_old_schema_and_missing_cu(self) -> None:
        receipt = {
            "signature": SIGNATURES[0],
            "slot": 10,
            "feeLamports": 5_000,
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
                    "format": "dclutch-owned-loopback-flagship-resolution-alt-journal-v1",
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

    def test_terminal_completion_checks_mutation_order_and_exact_arithmetic(
        self,
    ) -> None:
        kinds = [
            "core-begin-retiring",
            "direct-begin-retiring",
            "resolution-close-fund",
            "direct-close-capability",
            "retirement-replay-handoff",
            "aggregate-retirement",
        ]
        journals = [
            {
                "schema": MODULE.TERMINAL_JOURNAL_SCHEMA,
                "mutation": {"kind": kind},
                "phase": "finalized",
                "signature": SIGNATURES[index],
                "finalizedSlot": str(50 + index),
                "transactionFeeLamports": "5000",
                "computeUnitsConsumed": str(100 + index),
            }
            for index, kind in enumerate(kinds)
        ]
        completion = {
            "schema": MODULE.TERMINAL_COMPLETION_SCHEMA,
            "status": "finalized",
            "cluster": "owned-loopback",
            "genesisHash": PUBKEY_A,
            "invocation": {"command": MODULE.TERMINAL_RETIREMENT_COMMAND},
            "session": {},
            "journalDirectory": "retirement/journals",
            "market": PUBKEY_B,
            "payer": PUBKEY_C,
            "lookupTable": PUBKEY_A,
            "journals": journals,
            "finalizedSlot": "55",
            "transactionFeesLamports": str(6 * 5000),
            "computeUnitsConsumed": str(sum(range(100, 106))),
        }
        self.assertEqual(
            MODULE.authenticate_terminal_completion(
                completion, market=PUBKEY_B, payer=PUBKEY_C
            ),
            completion,
        )
        with self.assertRaisesRegex(MODULE.Refusal, "six protocol mutations"):
            MODULE.authenticate_terminal_completion(
                {**completion, "journals": list(reversed(journals))},
                market=PUBKEY_B,
                payer=PUBKEY_C,
            )
        with self.assertRaisesRegex(MODULE.Refusal, "arithmetic"):
            MODULE.authenticate_terminal_completion(
                {**completion, "computeUnitsConsumed": "616"},
                market=PUBKEY_B,
                payer=PUBKEY_C,
            )

    def test_terminal_stdout_joins_exact_completion_path_and_hash(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text)
            journal_dir = root / "journals"
            journal_dir.mkdir()
            completion_path = root / "completion.json"
            completion_path.write_text('{"status":"finalized"}\n')
            completion = {"lookupTable": PUBKEY_A}
            summary = {
                "status": "complete",
                "market": PUBKEY_B,
                "lookupTable": PUBKEY_A,
                "journalDirectory": str(journal_dir),
                "completion": str(completion_path),
                "completionSha256": MODULE.sha256_file(completion_path),
                "message": "Every exact terminal journal reverified at finalized and the aggregate Market account is closed.",
            }
            self.assertEqual(
                MODULE.authenticate_terminal_stdout(
                    summary,
                    completion=completion,
                    completion_path=completion_path,
                    journal_dir=journal_dir,
                    market=PUBKEY_B,
                ),
                summary,
            )
            with self.assertRaisesRegex(MODULE.Refusal, "completion path, hash"):
                MODULE.authenticate_terminal_stdout(
                    {**summary, "completionSha256": "00" * 32},
                    completion=completion,
                    completion_path=completion_path,
                    journal_dir=journal_dir,
                    market=PUBKEY_B,
                )
            with self.assertRaisesRegex(MODULE.Refusal, "fields changed"):
                MODULE.authenticate_terminal_stdout(
                    {**summary, "extra": True},
                    completion=completion,
                    completion_path=completion_path,
                    journal_dir=journal_dir,
                    market=PUBKEY_B,
                )

    def test_full_probe_is_distinct_one_seed_and_still_refused(self) -> None:
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
            with self.assertRaisesRegex(MODULE.Refusal, "Direct"):
                MODULE.parse([*common, "--seeds", "1"])
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
