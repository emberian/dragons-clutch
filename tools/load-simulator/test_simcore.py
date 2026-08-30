#!/usr/bin/env python3
"""Hostile-leaning tests for the load simulator core.

No cluster, no subprocess: everything runs in a temp dir.  The properties
under test are the ones the simulator's honesty depends on -- resume is a
refusal when the plan changed, halts are durable, status writes are atomic.
"""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

MODULE_PATH = Path(__file__).with_name("simcore.py")
SPEC = importlib.util.spec_from_file_location("dclutch_load_simcore", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
simcore = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = simcore
SPEC.loader.exec_module(simcore)


class CycleJournalTest(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def test_finalized_journal_survives_rerun_byte_identically(self) -> None:
        plan = {"participant": "A", "action": "trade", "lamports": 7}
        j = simcore.CycleJournal.open(self.root, 1)
        j.record(simcore.PHASE_PLANNED, plan)
        j.record(simcore.PHASE_FINALIZED, plan, signatures=["sig1"])
        before = j.path.read_bytes()

        # A resumed run sees the same plan and must not rewrite anything.
        j2 = simcore.CycleJournal.open(self.root, 1)
        body = j2.assert_same_plan_or_absent(plan)
        self.assertIsNotNone(body)
        self.assertTrue(j2.is_finalized())
        self.assertEqual(before, j2.path.read_bytes())

    def test_changed_plan_refuses_rather_than_resumes(self) -> None:
        plan = {"participant": "A", "action": "trade"}
        j = simcore.CycleJournal.open(self.root, 3)
        j.record(simcore.PHASE_FINALIZED, plan)
        with self.assertRaises(simcore.JournalConflict):
            simcore.CycleJournal.open(self.root, 3).assert_same_plan_or_absent(
                {"participant": "B", "action": "trade"}
            )

    def test_plan_digest_is_order_insensitive(self) -> None:
        a = {"x": 1, "y": [1, 2]}
        b = {"y": [1, 2], "x": 1}
        self.assertEqual(simcore.digest_of(a), simcore.digest_of(b))

    def test_executing_phase_is_visible_to_resume(self) -> None:
        plan = {"action": "join"}
        j = simcore.CycleJournal.open(self.root, 5)
        j.record(simcore.PHASE_EXECUTING, plan)
        body = simcore.CycleJournal.open(self.root, 5).assert_same_plan_or_absent(plan)
        assert body is not None
        self.assertEqual(body["phase"], simcore.PHASE_EXECUTING)
        self.assertFalse(simcore.CycleJournal.open(self.root, 5).is_finalized())


class RateControllerTest(unittest.TestCase):
    def test_jitter_stays_inside_the_band(self) -> None:
        rc = simcore.RateController(period_seconds=10.0, jitter_fraction=0.25)
        rc.rng.seed(7)
        for _ in range(200):
            d = rc.next_delay()
            self.assertGreaterEqual(d, 7.5)
            self.assertLessEqual(d, 12.5)

    def test_backpressure_doubles_and_caps_and_resets(self) -> None:
        rc = simcore.RateController(
            period_seconds=1.0, backoff_initial=5.0, backoff_max=40.0
        )
        self.assertEqual(rc.on_backpressure(), 5.0)
        self.assertEqual(rc.on_backpressure(), 10.0)
        self.assertEqual(rc.on_backpressure(), 20.0)
        self.assertEqual(rc.on_backpressure(), 40.0)
        self.assertEqual(rc.on_backpressure(), 40.0)  # capped
        rc.on_clean_cycle()
        self.assertEqual(rc.on_backpressure(), 5.0)

    def test_backpressure_marker_detection(self) -> None:
        self.assertTrue(simcore.looks_like_backpressure("HTTP 429 Too Many Requests"))
        self.assertTrue(simcore.looks_like_backpressure("server said RATE LIMIT hit"))
        self.assertFalse(simcore.looks_like_backpressure("custom program error: 0x4003"))


class RedactionTest(unittest.TestCase):
    """The status artifact says secrets never enter it. These are the teeth."""

    def test_provider_keys_never_survive_redaction(self) -> None:
        cases = {
            # Helius and friends: the key is a query parameter.
            "https://devnet.helius-rpc.com/?api-key=abc123SECRET":
                "https://devnet.helius-rpc.com/?<redacted>",
            "https://rpc.example.com/?apiKey=abc&other=1":
                "https://rpc.example.com/?<redacted>",
            # Some providers put the credential in the path instead.
            "https://x.quiknode.pro/abc123SECRET/":
                "https://x.quiknode.pro/<redacted>",
            # Nothing to hide stays legible.
            "https://api.devnet.solana.com": "https://api.devnet.solana.com",
            "http://127.0.0.1:8899/": "http://127.0.0.1:8899/",
        }
        for raw, expected in cases.items():
            self.assertEqual(simcore.redact_endpoint(raw), expected, raw)
        # Anything unparseable is withheld rather than passed through.
        self.assertEqual(simcore.redact_endpoint("not a url"), "<redacted>")

    def test_the_status_writer_cannot_be_made_to_hold_a_key(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "status.json"
            w = simcore.StatusWriter(
                path=path,
                cluster_label="devnet",
                rpc_url="https://devnet.helius-rpc.com/?api-key=abc123SECRET",
                mode="sustain",
            )
            # Redacted at the field, so no write path can reintroduce it.
            self.assertNotIn("abc123SECRET", w.rpc_url)
            w.write(
                cycles_run=1,
                cycles_target=None,
                trades_landed=0,
                signatures=[],
                wallets=[],
                last_reconciliation=None,
            )
            self.assertNotIn("abc123SECRET", path.read_text())
            self.assertEqual(
                json.loads(path.read_text())["cluster"]["rpc_url"],
                "https://devnet.helius-rpc.com/?<redacted>",
            )


class StatusWriterTest(unittest.TestCase):
    def test_status_is_complete_and_caps_signatures(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "status.json"
            w = simcore.StatusWriter(
                path=path,
                cluster_label="local",
                rpc_url="http://127.0.0.1:8899",
                mode="finite",
                market_address="MktAddr111",
                max_signatures=3,
            )
            w.write(
                cycles_run=4,
                cycles_target=5,
                trades_landed=4,
                signatures=["s1", "s2", "s3", "s4", "s5"],
                wallets=[{"address": "W1", "sol_lamports": 12345, "source": "staged"}],
                last_reconciliation={"ok": True, "checked_at": "now", "checks": []},
            )
            body = json.loads(path.read_text())
            self.assertEqual(body["schema"], simcore.SCHEMA_STATUS)
            self.assertEqual(body["trades"]["signatures"], ["s3", "s4", "s5"])
            self.assertEqual(body["market"]["address"], "MktAddr111")
            self.assertFalse(body["halted"])
            # No temp file left behind: the write was atomic.
            self.assertEqual(
                [p.name for p in Path(tmp).iterdir()], ["status.json"]
            )


class HaltTest(unittest.TestCase):
    def test_halt_is_durable_and_blocks_restart(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            with self.assertRaises(simcore.Halt):
                simcore.halt_loudly(work, "conservation divergence", {"delta": -3})
            self.assertTrue((work / "HALT.json").exists())
            with self.assertRaises(simcore.Halt):
                simcore.refuse_if_halted(work)
            # A deliberate human removal re-arms the work dir.
            (work / "HALT.json").unlink()
            simcore.refuse_if_halted(work)  # no raise


if __name__ == "__main__":
    unittest.main()
