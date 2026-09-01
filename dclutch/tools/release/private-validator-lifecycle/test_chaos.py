#!/usr/bin/env python3
"""Adversarial tests for the exact private lifecycle chaos matrix."""

from __future__ import annotations

import copy
import hashlib
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest


MODULE_PATH = Path(__file__).with_name("chaos.py")
SPEC = importlib.util.spec_from_file_location("dclutch_private_chaos", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
CHAOS = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CHAOS
SPEC.loader.exec_module(CHAOS)


def digest(label: str) -> str:
    return hashlib.sha256(label.encode()).hexdigest()


SOURCE_REVISION = hashlib.sha1(b"source").hexdigest()


def signature(index: int) -> str:
    # All-ones is valid base58 spelling for a zero-prefixed byte string.  The
    # chaos contract validates textual closure; finalized RPC authentication
    # in run.py validates the real 64-byte signature.
    return "1" * (64 + index % 8)


def case(spec: CHAOS.FaultSpec, index: int) -> dict:
    row = {
        "schema": CHAOS.CASE_SCHEMA_V1,
        "caseId": spec.case_id,
        "stage": spec.stage,
        "boundary": spec.boundary,
        "targetMutation": CHAOS.TARGET_MUTATIONS[spec.stage],
        "status": "finalized",
        "namedSeed": f"chaos-{index:02d}",
        "genesisHash": "1" * 32,
        "sessionIdentitySha256": digest(f"session-{index}"),
        "sourceRevision": SOURCE_REVISION,
        "checkedReleaseGateSha256": digest("gate"),
        "terminalResultSha256": digest(f"terminal-{index}"),
        "completedStages": list(CHAOS.STAGES),
        "targetIntentSha256": digest(f"intent-{index}"),
        "targetPacketSha256": digest(f"packet-{index}"),
        "targetSignature": signature(index),
        "targetSigningCount": 1,
        "targetDistinctSignatureCount": 1,
        "targetSendCount": 1,
        "fault": None,
        "recovery": None,
        "caseSha256": "0" * 64,
    }
    if spec.interrupted:
        journal = digest(f"journal-{index}")
        row["fault"] = {
            "receiptSha256": digest(f"fault-{index}"),
            "journalBeforeKillSha256": journal,
            "durablePhase": "dispatching",
            "exitCode": -9,
            "signal": 9,
            "sendCountBeforeKill": (
                0 if spec.boundary == CHAOS.PRE_SEND_BOUNDARY else 1
            ),
            "intentSha256": row["targetIntentSha256"],
            "packetSha256": row["targetPacketSha256"],
            "signature": row["targetSignature"],
        }
        row["recovery"] = {
            "sameGenesis": True,
            "sameSessionIdentity": True,
            "journalBeforeRestartSha256": journal,
            "journalAfterFinalizationSha256": digest(f"final-journal-{index}"),
            "intentSha256": row["targetIntentSha256"],
            "packetSha256": row["targetPacketSha256"],
            "signature": row["targetSignature"],
            "pollCount": 1,
            "sendCountAfterRestart": (
                1 if spec.boundary == CHAOS.PRE_SEND_BOUNDARY else 0
            ),
            "signingCountAfterRestart": 0,
            "finalizedSlot": 100 + index,
        }
    row["caseSha256"] = CHAOS._case_digest(row)
    return row


def session() -> dict:
    return CHAOS.build_session(
        source_revision=SOURCE_REVISION,
        source_tree_sha256=digest("tree"),
        checked_release_gate_sha256=digest("gate"),
        cases=[case(spec, index) for index, spec in enumerate(CHAOS.MATRIX, start=1)],
    )


class ChaosContractTests(unittest.TestCase):
    def test_matrix_is_exactly_control_plus_two_boundaries_for_eight_stages(self) -> None:
        self.assertEqual(len(CHAOS.MATRIX), 17)
        self.assertEqual(CHAOS.MATRIX[0].case_id, "control")
        self.assertEqual(
            [(row.stage, row.boundary) for row in CHAOS.MATRIX[1:]],
            [
                (stage, boundary)
                for stage in CHAOS.STAGES
                for boundary in CHAOS.BOUNDARIES
            ],
        )

    def test_exact_session_round_trips_and_writes_no_clobber(self) -> None:
        accepted = session()
        self.assertEqual(CHAOS.authenticate_session(accepted), accepted)
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "CHAOS.json"
            CHAOS.write_session_new(path, accepted)
            self.assertEqual(CHAOS.read_session(path), accepted)
            with self.assertRaisesRegex(CHAOS.Refusal, "new absolute path"):
                CHAOS.write_session_new(path, accepted)

    def test_missing_reordered_or_relabelled_case_refuses(self) -> None:
        original = session()
        missing = copy.deepcopy(original)
        missing["cases"].pop()
        missing["sessionSha256"] = CHAOS._session_digest(missing)
        with self.assertRaisesRegex(CHAOS.Refusal, "seventeen"):
            CHAOS.authenticate_session(missing)

        reordered = copy.deepcopy(original)
        reordered["cases"][1], reordered["cases"][2] = (
            reordered["cases"][2],
            reordered["cases"][1],
        )
        reordered["sessionSha256"] = CHAOS._session_digest(reordered)
        with self.assertRaisesRegex(CHAOS.Refusal, "changed identity"):
            CHAOS.authenticate_session(reordered)

    def test_restart_may_not_resign_or_send_after_landed_boundary(self) -> None:
        original = session()
        # Case two for each stage is the lost-response/landed boundary.  Use
        # founding's, matrix index 2.
        hostile = copy.deepcopy(original)
        row = hostile["cases"][2]
        row["recovery"]["signingCountAfterRestart"] = 1
        row["recovery"]["sendCountAfterRestart"] = 1
        row["caseSha256"] = CHAOS._case_digest(row)
        hostile["sessionSha256"] = CHAOS._session_digest(hostile)
        with self.assertRaisesRegex(CHAOS.Refusal, "sends after restart"):
            CHAOS.authenticate_session(hostile)

    def test_packet_signature_or_dead_journal_substitution_refuses(self) -> None:
        original = session()
        for mutation, message in (
            (("recovery", "packetSha256"), "exact packetSha256"),
            (("recovery", "signature"), "exact signature"),
            (("recovery", "journalBeforeRestartSha256"), "while the process was dead"),
        ):
            hostile = copy.deepcopy(original)
            row = hostile["cases"][1]
            row[mutation[0]][mutation[1]] = (
                "1" * 64 if mutation[1] == "signature" else digest("substituted")
            )
            row["caseSha256"] = CHAOS._case_digest(row)
            hostile["sessionSha256"] = CHAOS._session_digest(hostile)
            with self.assertRaisesRegex(CHAOS.Refusal, message):
                CHAOS.authenticate_session(hostile)

    def test_control_cannot_carry_fault_theater(self) -> None:
        original = session()
        hostile = copy.deepcopy(original)
        hostile["cases"][0]["fault"] = copy.deepcopy(hostile["cases"][1]["fault"])
        hostile["cases"][0]["caseSha256"] = CHAOS._case_digest(hostile["cases"][0])
        hostile["sessionSha256"] = CHAOS._session_digest(hostile)
        with self.assertRaisesRegex(CHAOS.Refusal, "fault or recovery theater"):
            CHAOS.authenticate_session(hostile)

    def test_execute_matrix_invokes_every_case_once(self) -> None:
        seen: list[tuple[str, int]] = []

        def execute(spec: CHAOS.FaultSpec, index: int) -> dict:
            seen.append((spec.case_id, index))
            return case(spec, index)

        accepted = CHAOS.execute_matrix(
            execute,
            source_revision=SOURCE_REVISION,
            source_tree_sha256=digest("tree"),
            checked_release_gate_sha256=digest("gate"),
        )
        self.assertEqual(len(seen), 17)
        self.assertEqual(accepted["matrix"]["caseCount"], 17)


if __name__ == "__main__":
    unittest.main()
