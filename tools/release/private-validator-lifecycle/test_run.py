#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest


MODULE_PATH = Path(__file__).with_name("run.py")
SPEC = importlib.util.spec_from_file_location("private_validator_lifecycle", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class PrivateValidatorLifecycleTests(unittest.TestCase):
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
            paths, seeds, through = MODULE.parse(
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
                ]
            )
            self.assertEqual(paths.work, root / "work")
            self.assertEqual((seeds, through), (1, "participant"))

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
        self.assertEqual(MODULE.DEVELOPMENT_FEE_RECIPIENT_ROLE, "founding-source-funder")
        self.assertEqual(
            MODULE.PROTOCOL_CREATED_KEY_ROLES,
            ("collateral-mint", "collateral-wallet", "founding-source-funder"),
        )
        self.assertTrue(
            set(MODULE.LOCAL_AIRDROP_ROLES).isdisjoint(MODULE.PROTOCOL_CREATED_KEY_ROLES)
        )
        self.assertNotIn(MODULE.VALIDATOR_MINT_ROLE, MODULE.PROTOCOL_CREATED_KEY_ROLES)

    def test_pyth_owner_has_exact_eight_action_journal_prefix(self) -> None:
        self.assertEqual(MODULE.PYTH_PROVISION_COMMAND, "local-private-validator-pyth-vaa-provision-v1")
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
        self.assertEqual(argv[argv.index("--account-dir") + 1], "/work/mutable/account-dir")
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
        self.assertEqual(
            MODULE.canonical_resolution_link({"links": [honest]}), honest
        )
        for substitution in (
            {"package": "dclutch-sbf"},
            {"elf": {**honest["elf"], "canonical_path": "elf/dclutch_sbf.so"}},
            {"elf": {**honest["elf"], "bytes": 9_034_536}},
        ):
            hostile = {**honest, **substitution}
            with self.assertRaisesRegex(MODULE.Refusal, "substitution is banished"):
                MODULE.canonical_resolution_link({"links": [hostile]})

    def test_stage_receipt_preserves_failure_without_calling_it_passed(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            run = Path(root_text)
            (run / "stages").mkdir()
            with self.assertRaisesRegex(MODULE.Refusal, "status 7"):
                MODULE.run_stage(run, 1, "hostile", ["/bin/sh", "-c", "echo wall >&2; exit 7"])
            receipt = MODULE.read_unique_json(
                run / "stages" / "01-hostile" / "receipt.json", "receipt"
            )
            self.assertEqual(receipt["exit_status"], 7)
            self.assertTrue((run / "stages" / "01-hostile" / "stderr.bin").is_file())


if __name__ == "__main__":
    unittest.main()
