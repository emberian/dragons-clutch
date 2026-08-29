#!/usr/bin/env python3

from __future__ import annotations

import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import preflight


SOURCE_REPO = Path(__file__).resolve().parents[3]


def required_paths() -> set[str]:
    paths = {
        preflight.RUNNER,
        preflight.MAIN,
        *(row.help_path for row in preflight.EXPOSURES),
        *(row.owner_path for row in preflight.EXPOSURES if row.owner_path is not None),
        *(owner for _, owner in preflight.SCHEMA_OWNERS),
        f"{preflight.SUCCESSOR}/market.rs",
        f"{preflight.SUCCESSOR}/founding_submission_journal.rs",
        f"{preflight.SUCCESSOR}/private_activity.rs",
        f"{preflight.SUCCESSOR}/private_lifecycle.rs",
        f"{preflight.SUCCESSOR}/direct_trade.rs",
        f"{preflight.SUCCESSOR}/terminal_lifecycle.rs",
        f"{preflight.SUCCESSOR}/user_position_admission.rs",
        f"{preflight.SUCCESSOR}/terminal_exterior_pyth.rs",
        f"{preflight.SUCCESSOR}/flagship_resolution.rs",
        f"{preflight.SUCCESSOR}/wallet_terminal_payout_exterior.rs",
        f"{preflight.SUCCESSOR}/terminal_sequence.rs",
        f"{preflight.SUCCESSOR}/aggregate_retirement_exterior.rs",
        "crates/dclutch-operator/src/wallet_terminal_payout_v3.rs",
    }
    return {path for path in paths if path is not None}


class OfflinePreflightTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="dclutch-lifecycle-preflight-")
        self.repo = Path(self.temporary.name).resolve()
        for relative in required_paths():
            source = SOURCE_REPO / relative
            target = self.repo / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def mutate(self, relative: str, old: str, new: str, *, all_matches: bool = False) -> None:
        path = self.repo / relative
        source = path.read_text()
        self.assertIn(old, source, f"test fixture no longer contains {old!r}")
        changed = source.replace(old, new) if all_matches else source.replace(old, new, 1)
        path.write_text(changed)

    def assert_refuses(self, pattern: str, through: str = "full-probe") -> None:
        with self.assertRaisesRegex(preflight.Refusal, pattern):
            preflight.run_preflight(self.repo, through)

    def test_current_source_derives_exact_eight_stage_model(self) -> None:
        report = preflight.run_preflight(self.repo, "full-probe")
        self.assertEqual(report["status"], "accepted")
        self.assertEqual(report["stage_vocabulary"]["private"], [
            "founding", "participant", "alt", "seal", "direct", "resolution", "payout", "retirement",
        ])
        completion = [row["stage"] for row in report["expected_execution"] if row["completion_stage"]]
        self.assertEqual(completion, report["stage_vocabulary"]["private"])
        self.assertLessEqual(report["founding_geometry"]["complete_transaction_keys"], 64)
        self.assertEqual(report["transaction_geometry"]["direct_hot"], {
            "static": 4, "loaded": 57, "unique": 61, "wire_bytes": 1_159, "poststates": 10,
        })
        self.assertFalse(report["validator_started"])
        self.assertFalse(report["rpc_used"])
        self.assertFalse(report["keys_read"])
        self.assertFalse(report["build_run"])
        self.assertEqual(len(report["model_sha256"]), 64)

    def test_participant_mode_excludes_terminal_commands_and_stages(self) -> None:
        report = preflight.run_preflight(self.repo, "participant")
        self.assertEqual(report["expected_execution"][-1]["stage"], "participant")
        commands = {row["command"] for row in report["command_exposures"]}
        self.assertNotIn(preflight.DIRECT_EXECUTE_COMMAND if hasattr(preflight, "DIRECT_EXECUTE_COMMAND") else "local-private-validator-direct-trade-v1", commands)

    def test_missing_dispatch_refuses_before_binary_or_validator(self) -> None:
        self.mutate(
            preflight.MAIN,
            'Some("local-private-validator-direct-trade-v1")',
            'Some("local-private-validator-direct-trade-v9")',
        )
        self.assert_refuses("absent from the successor dispatch")

    def test_dispatched_but_hidden_help_refuses(self) -> None:
        self.mutate(
            f"{preflight.SUCCESSOR}/direct_trade.rs",
            "local-private-validator-direct-trade-v1",
            "local-private-validator-direct-trade-hidden-v1",
            all_matches=True,
        )
        self.assert_refuses("absent from its accepted help")

    def test_literal_patch_marker_in_help_refuses(self) -> None:
        self.mutate(
            f"{preflight.SUCCESSOR}/aggregate_retirement_exterior.rs",
            r"\n     --rpc-url",
            r"\n+     --rpc-url",
        )
        self.assert_refuses("literal patch-marker prefix")

    def test_runner_owner_schema_drift_refuses(self) -> None:
        self.mutate(
            preflight.RUNNER,
            'DIRECT_FINALIZED_SCHEMA = "dclutch-owned-loopback-direct-trade-finalized-v1"',
            'DIRECT_FINALIZED_SCHEMA = "dclutch-owned-loopback-direct-trade-finalized-v9"',
        )
        self.assert_refuses("DIRECT_FINALIZED_SCHEMA differs from semantic owner")

    def test_zero_or_changed_fixture_supply_refuses(self) -> None:
        self.mutate(
            preflight.RUNNER,
            "PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS = 100_000_000",
            "PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS = 0",
        )
        self.assert_refuses("bankroll or participant fixture quantity changed")

    def test_airdrop_role_refuses(self) -> None:
        self.mutate(
            preflight.RUNNER,
            "LOCAL_AIRDROP_ROLES: tuple[str, ...] = ()",
            'LOCAL_AIRDROP_ROLES: tuple[str, ...] = ("campaign-payer",)',
        )
        self.assert_refuses("reintroduced an airdrop role")

    def test_missing_supply_partition_join_refuses(self) -> None:
        self.mutate(
            preflight.RUNNER,
            "founding_atoms + PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS",
            "founding_atoms + 1",
        )
        self.assert_refuses(r"founding_atoms \+ PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS")

    def test_founding_above_devnet_lock_limit_refuses(self) -> None:
        market = f"{preflight.SUCCESSOR}/market.rs"
        source = (self.repo / market).read_text()
        match = preflight.re.search(
            r"const (GENERIC_MARKET_FOUNDING_COMPLETE_KEYS_V\d+): usize = ([0-9_]+);",
            source,
        )
        self.assertIsNotNone(match)
        assert match is not None
        self.mutate(market, match.group(0), f"const {match.group(1)}: usize = 65;")
        self.assert_refuses("exceeds devnet's 64-key limit")

    def test_founding_route_rename_without_runner_join_refuses(self) -> None:
        market = f"{preflight.SUCCESSOR}/market.rs"
        source = (self.repo / market).read_text()
        match = preflight.re.search(
            r'const GENERIC_MARKET_FOUNDING_MAGIC_V\d+: \[u8; 8\] = \*b"(DCLTGMF\d+)";',
            source,
        )
        self.assertIsNotNone(match)
        assert match is not None
        magic = match.group(1)
        replacement = "DCLTGMF9" if magic != "DCLTGMF9" else "DCLTGMF8"
        self.mutate(
            preflight.RUNNER,
            f"found the Market atomically: Lock, Found, Realize, Claims, Open ({magic})",
            f"found the Market atomically: Lock, Found, Realize, Claims, Open ({replacement})",
        )
        self.assert_refuses("mutation/journal vocabulary differs")

    def test_private_stage_reorder_refuses(self) -> None:
        path = f"{preflight.SUCCESSOR}/private_lifecycle.rs"
        self.mutate(path, '    "alt",\n    "seal",', '    "seal",\n    "alt",')
        self.assert_refuses("vocabularies no longer form the exact")

    def test_pyth_journal_action_reorder_refuses(self) -> None:
        self.mutate(
            preflight.RUNNER,
            '    "router-initialize",\n    "receiver-initialize",',
            '    "receiver-initialize",\n    "router-initialize",',
        )
        self.assert_refuses("Pyth journal file order differs")

    def test_direct_geometry_drift_refuses(self) -> None:
        self.mutate(
            f"{preflight.SUCCESSOR}/direct_trade.rs",
            "evidence.unique_message_account_count != 61",
            "evidence.unique_message_account_count != 62",
            all_matches=True,
        )
        self.assert_refuses("Direct terminal geometry")

    def test_direct_terminal_vocabulary_omission_refuses(self) -> None:
        self.mutate(preflight.RUNNER, '        "hot": 6,', '        "execute": 6,')
        self.assert_refuses("Direct controller vocabulary")

    def test_zero_payout_burn_semantics_omission_refuses(self) -> None:
        self.mutate(
            "crates/dclutch-operator/src/wallet_terminal_payout_v3.rs",
            "Exact collateral atoms paid; zero is a real burn outcome.",
            "Exact collateral atoms paid.",
        )
        self.assert_refuses("wallet zero-payout semantics")

    def test_resolution_receipt_order_omission_refuses(self) -> None:
        self.mutate(
            preflight.RUNNER,
            '        "core-terminal-accept-v1",\n        "reclaim",',
            '        "reclaim",\n        "core-terminal-accept-v1",',
        )
        self.assert_refuses("core-terminal-accept-v1")

    def test_terminal_handoff_rename_refuses(self) -> None:
        self.mutate(
            f"{preflight.SUCCESSOR}/terminal_sequence.rs",
            "15-retirement-replay-handoff.json",
            "16-retirement-replay-handoff.json",
            all_matches=True,
        )
        self.assert_refuses("terminal prelude geometry")

    def test_output_is_create_new_and_no_clobber(self) -> None:
        report = preflight.run_preflight(self.repo, "participant")
        output = self.repo / "preflight.json"
        preflight.write_new(output, report)
        self.assertEqual(json.loads(output.read_text())["model_sha256"], report["model_sha256"])
        with self.assertRaisesRegex(preflight.Refusal, "absent path"):
            preflight.write_new(output, report)


if __name__ == "__main__":
    unittest.main()
