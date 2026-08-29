#!/usr/bin/env python3
"""Key-free hostile tests for finite Activity-v3 orchestration."""

from __future__ import annotations

import datetime as dt
import hashlib
import importlib.util
import json
from pathlib import Path
import stat
import sys
import tempfile
import types
import unittest
from unittest import mock


MODULE_PATH = Path(__file__).with_name("ongoing_v1.py")
SPEC = importlib.util.spec_from_file_location("dclutch_activity_ongoing_v1", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
ongoing = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = ongoing
SPEC.loader.exec_module(ongoing)

HELPER_PATH = Path(__file__).with_name("test_activity.py")
HELPER_SPEC = importlib.util.spec_from_file_location(
    "dclutch_activity_test_helpers_ongoing", HELPER_PATH
)
assert HELPER_SPEC is not None and HELPER_SPEC.loader is not None
helpers = importlib.util.module_from_spec(HELPER_SPEC)
sys.modules[HELPER_SPEC.name] = helpers
HELPER_SPEC.loader.exec_module(helpers)


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def executable(path: Path, body: str = "#!/bin/sh\nexit 0\n") -> Path:
    path.write_text(body, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)
    return path


def base58(value: bytes) -> str:
    alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    number = int.from_bytes(value, "big")
    output = ""
    while number:
        number, remainder = divmod(number, 58)
        output = alphabet[remainder] + output
    return "1" * (len(value) - len(value.lstrip(b"\0"))) + (output or "1")


def replace_alice(value: object) -> object:
    return json.loads(json.dumps(value).replace("alice", "ash"))


class OngoingFixture(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve(strict=True)
        self.work_base = self.root / "runs"
        self.work_base.mkdir()
        self.checked_release = self.root / "checked-release.json"
        self.market = self.root / "market.json"
        self.source = self.root / "caller-source.json"
        for path, label in (
            (self.checked_release, "release"),
            (self.market, "market"),
            (self.source, "source"),
        ):
            write_json(path, {"fixture": label})
        self.binaries = [
            executable(self.root / role, f"#!/bin/sh\n# {role}\nexit 0\n")
            for role in ongoing.BINARY_ROLES
        ]
        self.manifest_paths: list[Path] = []
        self.rent_paths: list[Path] = []
        self.write_cycle(0, 1_000_000)
        self.write_cycle(1, 2_000_000)
        self.ongoing_path = self.root / "ongoing.json"
        self.write_ongoing()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def scenario_value(self) -> dict[str, object]:
        scenario = replace_alice(helpers.v3_devnet_scenario_value())
        assert isinstance(scenario, dict)
        body = scenario["body"]
        assert isinstance(body, dict)
        body["evidenceLevel"] = "authenticated-activity-v3"
        wallets = body["wallets"]
        assert isinstance(wallets, list)
        by_id = {
            row["id"]: row for row in wallets if isinstance(row, dict)
        }
        by_id["deployer"]["fundingLamports"] = "360000000"
        by_id["ash"]["fundingLamports"] = "50000000"
        for wallet_id in ("birch", "cobalt", "dahlia"):
            model = json.loads(json.dumps(by_id["ash"]))
            model["id"] = wallet_id
            model["fundingLamports"] = "50000000"
            model["collateralAccountRef"] = (
                f"token.test-lifecycle.{wallet_id}.collateral"
            )
            model["claimAccountRefs"] = [
                f"token.test-lifecycle.{wallet_id}.claim.{index}"
                for index in range(4)
            ]
            model["positionAccountRef"] = f"position.test-lifecycle.{wallet_id}"
            wallets.append(model)
            body["accounts"].append(
                {
                    "id": f"wallet.{wallet_id}",
                    "kind": "wallet",
                    "address": None,
                    "expectedOwnerRef": "solana-system-program",
                    "mintRef": None,
                    "tokenAuthorityWalletRef": None,
                }
            )
        for operation in body["operations"]:
            operation["mutationExpected"] = True
            operation["callerAvailability"] = "public-executable"
        scenario["bodySha256"] = hashlib.sha256(
            json.dumps(body, separators=(",", ":"), ensure_ascii=False).encode()
        ).hexdigest()
        return scenario

    def manifest_value(
        self, scenario_path: Path, direct_session: Path, terminal_session: Path
    ) -> dict[str, object]:
        value = replace_alice(
            helpers.v3_devnet_manifest_value(
                scenario_path, self.checked_release, self.market
            )
        )
        assert isinstance(value, dict)
        value["inputs"] = [
            {
                "id": "checked-release",
                "path": str(self.checked_release),
                "sha256": digest(self.checked_release),
            },
            {"id": "market", "path": str(self.market), "sha256": digest(self.market)},
            {"id": "caller-source", "path": str(self.source), "sha256": digest(self.source)},
            {
                "id": "direct-session",
                "path": str(direct_session),
                "sha256": digest(direct_session),
            },
            {
                "id": "terminal-session",
                "path": str(terminal_session),
                "sha256": digest(terminal_session),
            },
        ]
        campaign = value["campaign"]
        assert isinstance(campaign, dict)
        campaign["initialFunding"]["transferLamports"] = "360000000"
        campaign["postInitFunding"] = [
            {
                "id": f"fund-{wallet_id}",
                "walletRef": wallet_id,
                "transferLamports": "50000000",
                "afterAdapter": "founding",
            }
            for wallet_id in ("ash", "birch", "cobalt", "dahlia")
        ]
        adapters = value["adapters"]
        assert isinstance(adapters, list)
        founding = adapters[0]
        participant = adapters[1]
        direct = adapters[2]
        terminal = adapters[3]
        direct["argv"] = [
            "devnet-direct-trade-v1",
            "--session",
            "{{input.direct-session}}",
            "--execute",
        ]
        direct["progressive"] = {
            "maxSteps": 8,
            "sourceInput": "caller-source",
            "sessionInput": "direct-session",
            "marketInput": "market",
        }
        terminal["id"] = "live-terminal"
        terminal["covers"] = ["resolve", "redeem", "retire"]
        terminal["argv"] = [
            "devnet-terminal-sequence-v1",
            "--session",
            "{{input.terminal-session}}",
            "--execute",
        ]
        terminal["dependsOn"] = [direct["id"]]
        terminal["progressive"] = {
            "maxSteps": 16,
            "sourceInput": "caller-source",
            "sessionInput": "terminal-session",
            "marketInput": "market",
        }
        value["adapters"] = [founding, participant, direct, terminal]
        return value

    def write_cycle(self, ordinal: int, rent: int) -> None:
        scenario_path = self.root / f"scenario-{ordinal}.json"
        direct_session = self.root / f"direct-session-{ordinal}.json"
        terminal_session = self.root / f"terminal-session-{ordinal}.json"
        write_json(scenario_path, self.scenario_value())
        write_json(direct_session, {"cycle": ordinal, "kind": "direct-session"})
        write_json(terminal_session, {"cycle": ordinal, "kind": "terminal-session"})
        manifest_path = self.root / f"manifest-{ordinal}.json"
        write_json(
            manifest_path,
            self.manifest_value(scenario_path, direct_session, terminal_session),
        )
        manifest = ongoing.activity.parse_manifest(manifest_path)
        rent_path = self.root / f"rent-{ordinal}.json"
        write_json(
            rent_path,
            {
                "schema": ongoing.RENT_ENVELOPE_SCHEMA,
                "manifestSha256": manifest.sha256,
                "scenarioSha256": manifest.scenario.sha256,
                "devnetGenesisHash": ongoing.activity.DEVNET_GENESIS_HASH,
                "observedSlot": str(1_000 + ordinal),
                "rentSysvarSha256": f"{ordinal + 1:x}" * 64,
                "entries": [
                    {
                        "accountRef": f"rent.cycle-{ordinal}",
                        "lamports": str(rent),
                    }
                ],
                "totalRentLamports": str(rent),
            },
        )
        self.manifest_paths.append(manifest_path)
        self.rent_paths.append(rent_path)

    def write_ongoing(self) -> None:
        write_json(
            self.ongoing_path,
            {
                "schema": ongoing.ONGOING_MANIFEST_SCHEMA,
                "runId": "test-ongoing",
                "workBase": str(self.work_base),
                "maxCycles": len(self.manifest_paths),
                "economicAuthority": {
                    "path": str(ongoing.CANONICAL_ECONOMIC_AUTHORITY),
                    "sha256": digest(ongoing.CANONICAL_ECONOMIC_AUTHORITY),
                },
                "acceptedHarness": {
                    "path": str(ongoing.ACTIVITY_PATH),
                    "sha256": digest(ongoing.ACTIVITY_PATH),
                    "sourceCommit": "e" * 40,
                },
                "binaries": [
                    {"role": role, "path": str(path), "sha256": digest(path)}
                    for role, path in zip(
                        ongoing.BINARY_ROLES, self.binaries, strict=True
                    )
                ],
                "cycles": [
                    {
                        "manifest": {
                            "path": str(manifest),
                            "sha256": digest(manifest),
                        },
                        "rentEnvelope": {
                            "path": str(rent),
                            "sha256": digest(rent),
                        },
                    }
                    for manifest, rent in zip(
                        self.manifest_paths, self.rent_paths, strict=True
                    )
                ],
            },
        )

    def plan(self):
        return ongoing.parse_ongoing_manifest(
            self.ongoing_path, digest(self.ongoing_path)
        )

    def authorization_value(self, plan, verifier: Path) -> dict[str, object]:
        now = dt.datetime.now(dt.timezone.utc)
        signer = base58(bytes([7]) * 32)
        body = ongoing.authorization_body(
            plan,
            run_nonce="a" * 64,
            not_before=(now - dt.timedelta(minutes=1)).isoformat(),
            expires_at=(now + dt.timedelta(hours=1)).isoformat(),
            signer_public_key=signer,
            accepted_verifier_sha256=digest(verifier),
        )
        return {
            "schema": ongoing.V4_AUTHORIZATION_SCHEMA,
            "body": body,
            "signedBodySha256": ongoing.sha256_bytes(ongoing.canonical_json(body)),
            "publicKeyBase58": signer,
            "signatureBase58": base58(bytes([9]) * 64),
        }

    def mock_verified(self, plan, verifier: Path):
        authorization_path = self.root / "authorization.json"
        value = self.authorization_value(plan, verifier)
        write_json(authorization_path, value)
        body = value["body"]
        assert isinstance(body, dict)
        result = {
            "schema": ongoing.VERIFIER_RESULT_SCHEMA,
            "messageSha256": value["signedBodySha256"],
            "publicKeyBase58": value["publicKeyBase58"],
            "signatureBase58": value["signatureBase58"],
            "verified": True,
        }
        completed = types.SimpleNamespace(
            returncode=0,
            stdout=json.dumps(result, separators=(",", ":")).encode(),
            stderr=b"",
        )
        with mock.patch.object(ongoing.subprocess, "run", return_value=completed) as run:
            verified = ongoing.verify_authorization(
                authorization_path,
                plan,
                verifier,
                str(value["publicKeyBase58"]),
            )
        self.assertEqual(run.call_count, 1)
        return verified

    def completion_artifacts(
        self, plan, cycle, addresses: list[str]
    ) -> tuple[Path, Path, Path]:
        reconciliation = self.root / f"reconciliation-{cycle.ordinal}.json"
        ongoing.activity.atomic_write_json(
            reconciliation,
            {
                "schema": ongoing.activity.RECONCILIATION_SCHEMA,
                "manifestSha256": cycle.manifest.sha256,
                "scenarioSha256": cycle.manifest.scenario.sha256,
                "untrustedProjectionUsed": False,
            },
            mode=0o644,
        )
        status = self.root / f"status-{cycle.ordinal}.json"
        ongoing.activity.atomic_write_json(
            status,
            {
                "schema": ongoing.activity.V3_SUPERVISOR_STATUS_SCHEMA,
                "manifestSha256": cycle.manifest.sha256,
                "scenarioSha256": cycle.manifest.scenario.sha256,
                "cycleId": cycle.cycle_id,
                "status": "complete-reconciled-live-send",
                "reconciliationSha256": digest(reconciliation),
            },
            mode=0o644,
        )
        wallet = self.root / f"wallet-{cycle.ordinal}.json"
        write_json(
            wallet,
            {
                "schema": ongoing.activity.WALLET_LEDGER_SCHEMA,
                "manifestSha256": cycle.manifest.sha256,
                "scenarioSha256": cycle.manifest.scenario.sha256,
                "wallets": [
                    {"id": slot["walletRef"], "address": address}
                    for slot, address in zip(
                        cycle.wallet_slots, addresses, strict=True
                    )
                ],
            },
        )
        return status, wallet, reconciliation


class OngoingTests(OngoingFixture):
    def test_plan_derives_disjoint_cycles_and_exact_aggregate_envelope(self) -> None:
        plan = self.plan()
        self.assertEqual(len(plan.cycles), 2)
        self.assertEqual(len({row.cycle_id for row in plan.cycles}), 2)
        self.assertEqual(
            len(
                {
                    slot["keySlotId"]
                    for cycle in plan.cycles
                    for slot in cycle.wallet_slots
                }
            ),
            20,
        )
        self.assertEqual(
            len(
                {
                    slot["sessionSlotId"]
                    for cycle in plan.cycles
                    for slot in cycle.session_slots
                }
            ),
            4,
        )
        self.assertEqual(plan.aggregate_envelope["payerFundingLamports"], "720000000")
        self.assertEqual(plan.aggregate_envelope["feeEnvelopeLamports"], "40000000")
        self.assertEqual(plan.aggregate_envelope["rentEnvelopeLamports"], "3000000")
        self.assertEqual(
            plan.aggregate_envelope["maximumPayerDebitLamports"], "443000000"
        )
        self.assertEqual(
            plan.aggregate_envelope["minimumPayerResidualLamports"], "277000000"
        )

    def test_reused_session_or_excess_rent_refuses(self) -> None:
        second = json.loads(self.manifest_paths[1].read_text())
        first = json.loads(self.manifest_paths[0].read_text())
        second_direct = next(
            row for row in second["inputs"] if row["id"] == "direct-session"
        )
        first_direct = next(
            row for row in first["inputs"] if row["id"] == "direct-session"
        )
        second_direct.update(first_direct)
        write_json(self.manifest_paths[1], second)
        rent = json.loads(self.rent_paths[1].read_text())
        rent["manifestSha256"] = digest(self.manifest_paths[1])
        write_json(self.rent_paths[1], rent)
        self.write_ongoing()
        with self.assertRaisesRegex(ongoing.Refusal, "reuse progressive session"):
            self.plan()

        # Restore a distinct cycle, then exceed the fixture-owned payer funding.
        self.manifest_paths.pop()
        self.rent_paths.pop()
        self.write_cycle(1, 150_000_000)
        self.write_ongoing()
        with self.assertRaisesRegex(ongoing.Refusal, "exceeds payer funding"):
            self.plan()

    def test_forged_authorization_refuses_before_work_creation(self) -> None:
        plan = self.plan()
        verifier = executable(self.root / "reject-verifier", "#!/bin/sh\nexit 2\n")
        authorization_path = self.root / "authorization.json"
        value = self.authorization_value(plan, verifier)
        write_json(authorization_path, value)
        with self.assertRaisesRegex(ongoing.Refusal, "verifier rejected"):
            ongoing.verify_authorization(
                authorization_path,
                plan,
                verifier,
                str(value["publicKeyBase58"]),
            )
        self.assertEqual(list(self.work_base.iterdir()), [])

    def test_verified_body_is_write_ahead_and_active_cycle_resumes(self) -> None:
        plan = self.plan()
        verifier = executable(self.root / "accepted-verifier")
        verified = self.mock_verified(plan, verifier)
        journal_path = ongoing.prepare_run(plan, verified)
        self.assertTrue(journal_path.is_file())
        action, first, work = ongoing.begin_or_resume_cycle(
            journal_path, plan, verified
        )
        self.assertEqual((action, first.ordinal), ("start", 0))
        self.assertTrue((work / "cycle-work.json").is_file())
        action, same, same_work = ongoing.begin_or_resume_cycle(
            journal_path, plan, verified
        )
        self.assertEqual((action, same.ordinal, same_work), ("resume", 0, work))

        addresses = [base58(bytes([index + 1]) * 32) for index in range(10)]
        status, wallet, reconciliation = self.completion_artifacts(
            plan, first, addresses
        )
        ongoing.admit_active_wallet_ledger(
            journal_path, plan, verified, wallet
        )
        ongoing.complete_active_cycle(
            journal_path,
            plan,
            verified,
            supervisor_status_path=status,
            wallet_ledger_path=wallet,
            reconciliation_path=reconciliation,
        )
        action, second, _ = ongoing.begin_or_resume_cycle(journal_path, plan, verified)
        self.assertEqual((action, second.ordinal), ("start", 1))

        repeated_status, repeated_wallet, repeated_reconciliation = (
            self.completion_artifacts(plan, second, addresses)
        )
        with self.assertRaisesRegex(ongoing.Refusal, "reused a disposable wallet"):
            ongoing.admit_active_wallet_ledger(
                journal_path, plan, verified, repeated_wallet
            )

    def test_signature_verification_pins_body_signer_and_verifier_hash(self) -> None:
        plan = self.plan()
        verifier = executable(self.root / "accepted-verifier")
        authorization_path = self.root / "authorization.json"
        value = self.authorization_value(plan, verifier)
        body = value["body"]
        assert isinstance(body, dict)
        body["maxCycles"] = 3
        write_json(authorization_path, value)
        with self.assertRaisesRegex(ongoing.Refusal, "canonical run envelope"):
            ongoing.verify_authorization(
                authorization_path,
                plan,
                verifier,
                str(value["publicKeyBase58"]),
            )

        value = self.authorization_value(plan, verifier)
        write_json(authorization_path, value)
        with self.assertRaisesRegex(ongoing.Refusal, "accepted public key"):
            ongoing.verify_authorization(
                authorization_path,
                plan,
                verifier,
                base58(bytes([8]) * 32),
            )


if __name__ == "__main__":
    unittest.main()
