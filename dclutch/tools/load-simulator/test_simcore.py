#!/usr/bin/env python3
"""Hostile-leaning tests for the load simulator core.

No cluster, no subprocess: everything runs in a temp dir.  The properties
under test are the ones the simulator's honesty depends on -- resume is a
refusal when the plan changed, halts are durable, status writes are atomic.
"""

from __future__ import annotations

import datetime as dt
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


# The real thing: one observation the live market18 devnet census wrote, kept
# byte for byte.  Retention re-serializes observations, so what it has to
# round-trip is the actual encoding of the actual tool, not a hand-written
# approximation of it.
REAL_OBSERVATION = Path(__file__).resolve().parent / "testdata" / "market18-census-observation.json"


def cumulative_series(count: int, per_observation_padding: int = 3800) -> list:
    """A census series shaped like the real one: element N is an observation,
    and the FILE at cycle N holds elements 1..N."""
    return [
        {"stage": f"load-sim-cycle-{index:06d}", "slot": 1000 + index,
         "verdicts": [{"law": "L5", "status": "holds", "detail": "x" * per_observation_padding}]}
        for index in range(1, count + 1)
    ]


class CensusRetentionTest(unittest.TestCase):
    """The fault that filled the machine's data volume on 2026-08-30, and the
    bound that replaces it."""

    def write_series(self, census: Path, cycle: int, series: list) -> Path:
        census.mkdir(parents=True, exist_ok=True)
        path = census / f"cycle-{cycle:06d}.json"
        path.write_bytes(simcore.CensusRetention.serialize(series))
        return path

    def test_the_real_encoding_round_trips_byte_for_byte(self) -> None:
        """Truncation drops whole ELEMENTS and never edits one.  That claim is
        only worth anything if re-serializing what is kept reproduces the
        census tool's own bytes, so it is checked against them."""
        raw = REAL_OBSERVATION.read_bytes()
        self.assertEqual(simcore.CensusRetention.serialize(json.loads(raw)), raw)

    def test_growth_is_quadratic_before_and_constant_after(self) -> None:
        """The measured fault, reproduced, then bounded -- with numbers."""
        window, keep = 20, 2
        retention = simcore.CensusRetention(window=window, keep_files=keep)
        with tempfile.TemporaryDirectory() as tmp:
            unbounded = Path(tmp) / "unbounded"
            bounded = Path(tmp) / "bounded"
            cycles = 60
            for cycle in range(1, cycles + 1):
                self.write_series(unbounded, cycle, cumulative_series(cycle))
                self.write_series(bounded, cycle, cumulative_series(cycle))
                report = retention.apply(bounded)

            unbounded_bytes = sum(p.stat().st_size for p in unbounded.glob("cycle-*.json"))
            bounded_bytes = sum(p.stat().st_size for p in bounded.glob("cycle-*.json"))

            # Unbounded really is the sum of its own history.
            self.assertEqual(len(list(unbounded.glob("cycle-*.json"))), cycles)
            self.assertGreater(unbounded_bytes, 20 * bounded_bytes)

            # Bounded holds exactly what it says it holds.
            self.assertEqual(report["files"], keep)
            self.assertEqual(report["observations"], window)
            self.assertEqual(bounded_bytes, report["bytes_on_disk"])
            self.assertLessEqual(bounded_bytes, report["bytes_bound"])
            self.assertEqual(
                report["bytes_bound"], keep * window * report["bytes_per_observation"]
            )

    def test_the_bound_stops_growing_while_the_run_does_not(self) -> None:
        """A ceiling that only holds for a while is not a ceiling: disk after
        the window fills is identical to disk three times further on."""
        retention = simcore.CensusRetention(window=10, keep_files=2)
        with tempfile.TemporaryDirectory() as tmp:
            census = Path(tmp) / "census"
            sizes = {}
            for cycle in range(1, 91):
                self.write_series(census, cycle, cumulative_series(cycle))
                retention.apply(census)
                sizes[cycle] = sum(p.stat().st_size for p in census.glob("cycle-*.json"))
            self.assertEqual(sizes[30], sizes[90])
            self.assertEqual(sizes[30], sizes[60])

    def test_the_newest_file_is_still_the_whole_window_a_reader_needs(self) -> None:
        """The property `scripts/simulator-series.mjs` mines: the newest file
        alone is the series, newest observation last."""
        retention = simcore.CensusRetention(window=5, keep_files=1)
        with tempfile.TemporaryDirectory() as tmp:
            census = Path(tmp) / "census"
            for cycle in range(1, 13):
                self.write_series(census, cycle, cumulative_series(cycle))
                retention.apply(census)
            files = simcore.CensusRetention.series_files(census)
            self.assertEqual([p.name for p in files], ["cycle-000012.json"])
            kept = json.loads(files[-1].read_bytes())
            self.assertEqual(
                [entry["stage"] for entry in kept],
                [f"load-sim-cycle-{i:06d}" for i in range(8, 13)],
            )

    def test_the_one_observation_every_law_reads_survives_exactly(self) -> None:
        """Losslessness is not a hope about the ledger: L2/L5/L6/L7 each read
        `observations.last()` and nothing reads the prefix, so what truncation
        must preserve is the final element, unedited."""
        retention = simcore.CensusRetention(window=3, keep_files=1)
        with tempfile.TemporaryDirectory() as tmp:
            census = Path(tmp) / "census"
            full = cumulative_series(40)
            path = self.write_series(census, 40, full)
            retention.apply(census)
            kept = json.loads(path.read_bytes())
            self.assertEqual(kept[-1], full[-1])
            self.assertEqual(kept, full[-3:])

    def test_an_unbounded_directory_is_repaired_in_one_pass(self) -> None:
        """Pointed at the 123-file directory the dead run left behind, it
        reclaims rather than only preventing."""
        retention = simcore.CensusRetention(window=8, keep_files=2)
        with tempfile.TemporaryDirectory() as tmp:
            census = Path(tmp) / "census"
            for cycle in range(1, 124):
                self.write_series(census, cycle, cumulative_series(cycle))
            before = sum(p.stat().st_size for p in census.glob("cycle-*.json"))
            report = retention.apply(census)
            after = sum(p.stat().st_size for p in census.glob("cycle-*.json"))
            self.assertEqual(report["removed_files"], 121)
            self.assertEqual(report["files"], 2)
            # Reclaims better than 98% of what the unbounded run was holding.
            self.assertLess(after, before // 50)

    def test_a_census_that_is_not_a_series_refuses_rather_than_skipping(self) -> None:
        """A bound that quietly does nothing when the shape surprises it is not
        a bound."""
        with tempfile.TemporaryDirectory() as tmp:
            census = Path(tmp) / "census"
            census.mkdir()
            (census / "cycle-000001.json").write_text('{"schema":"not-a-series"}')
            with self.assertRaises(ValueError):
                simcore.CensusRetention().apply(census)

    def test_an_empty_directory_reports_zero_rather_than_guessing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            report = simcore.CensusRetention().apply(Path(tmp) / "census")
            self.assertEqual(report["files"], 0)
            self.assertIsNone(report["bytes_bound"])


class HeartbeatTest(unittest.TestCase):
    """The artifact says when it expects to have written again, so a reader
    needs no rule of thumb about a cadence they cannot see."""

    def writer(self, path: Path, **kwargs) -> simcore.StatusWriter:
        defaults = dict(
            path=path, cluster_label="devnet", rpc_url="https://devnet.example.com/",
            mode="sustain", cadence_seconds=20.0, jitter_fraction=0.25, grace_seconds=300.0,
        )
        defaults.update(kwargs)
        return simcore.StatusWriter(**defaults)

    def status(self, writer: simcore.StatusWriter, **kwargs) -> dict:
        return writer.write(
            cycles_run=1, cycles_target=None, trades_landed=0, signatures=[], wallets=[],
            last_reconciliation=None, **kwargs,
        )

    def test_the_deadline_is_derived_from_the_cadence_it_actually_keeps(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            body = self.status(self.writer(Path(tmp) / "status.json"))
            beat = body["heartbeat"]
            self.assertEqual(beat["budget_seconds"], 20.0 * 1.25 + 0.0 + 300.0)
            span = (
                dt.datetime.fromisoformat(beat["expected_next_update_by"])
                - dt.datetime.fromisoformat(body["updated_at"])
            ).total_seconds()
            self.assertEqual(span, beat["budget_seconds"])

    def test_a_throttled_run_widens_its_own_deadline_by_exactly_the_backoff(self) -> None:
        """A run backing off 120s under 429s is late for a reason it knows, so
        it says so rather than being read as dead."""
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "status.json"
            calm = self.status(self.writer(path))["heartbeat"]
            throttled = self.status(self.writer(path), backoff_seconds=120.0)["heartbeat"]
            self.assertEqual(
                throttled["budget_seconds"] - calm["budget_seconds"], 120.0
            )

    def test_the_dead_runs_own_artifact_would_have_been_caught_by_itself(self) -> None:
        """The regression, stated as the case that happened: killed mid-cycle
        at 16:50:41Z, still reading `halted: false` at 17:07 when the wave
        noticed.  With the stamp, the file itself is past its deadline."""
        with tempfile.TemporaryDirectory() as tmp:
            body = self.status(self.writer(Path(tmp) / "status.json"))
            self.assertFalse(body["halted"])
            deadline = dt.datetime.fromisoformat(body["heartbeat"]["expected_next_update_by"])
            sixteen_minutes_on = (
                dt.datetime.fromisoformat(body["updated_at"]) + dt.timedelta(minutes=16)
            )
            self.assertLess(deadline, sixteen_minutes_on)


class ExitRecordTest(unittest.TestCase):
    def test_an_ending_it_can_observe_is_written_down(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            simcore.record_exit(work, simcore.EXIT_SIGNALLED, detail="SIGTERM", cycles_run=7)
            body = json.loads((work / "EXIT.json").read_text())
            self.assertEqual(body["schema"], simcore.SCHEMA_EXIT)
            self.assertEqual(body["outcome"], simcore.EXIT_SIGNALLED)
            self.assertEqual(body["cycles_run"], 7)

    def test_a_previous_runs_record_is_cleared_so_absence_stays_a_live_claim(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            simcore.record_exit(work, simcore.EXIT_COMPLETED)
            simcore.clear_exit_record(work)
            self.assertFalse((work / "EXIT.json").exists())
            simcore.clear_exit_record(work)  # idempotent: no raise

    def test_a_halt_record_and_an_exit_record_say_different_things(self) -> None:
        """HALT.json is about the LEDGER and refuses a restart; EXIT.json is
        about the PROCESS and never does."""
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            simcore.record_exit(work, simcore.EXIT_CRASHED, detail="boom")
            simcore.refuse_if_halted(work)  # no raise


class DiskFloorTest(unittest.TestCase):
    def test_room_reads_as_room(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            self.assertIsNone(simcore.DiskFloor(floor_bytes=1).check(Path(tmp)))

    def test_a_floor_breach_names_its_numbers(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            floor = simcore.DiskFloor(floor_bytes=1 << 62)
            sentence = floor.check(Path(tmp))
            self.assertIsNotNone(sentence)
            self.assertIn(str(1 << 62), sentence)
            self.assertIn("stopping between cycles", sentence)


class CommandRedactionTest(unittest.TestCase):
    """d17aa1a4 took the provider key out of status.json and the cycle plan.
    It did not reach HALT.json, which quotes the failing command -- and the
    command is `--rpc-url https://…?api-key=<the live key>`."""

    KEY = "abc123SECRET"
    URL = f"https://devnet.helius-rpc.com/?api-key={KEY}"

    def test_a_recorded_command_line_carries_no_credential(self) -> None:
        line = simcore.redact_command(
            ["/bin/boot", "ledger-census", "--rpc-url", self.URL, "--stage", "cycle-1"]
        )
        self.assertNotIn(self.KEY, line)
        self.assertIn("devnet.helius-rpc.com", line)
        self.assertIn("--stage cycle-1", line)

    def test_free_text_from_a_child_or_an_exception_is_scrubbed_too(self) -> None:
        scrubbed = simcore.redact_text(f"connection refused talking to {self.URL} after 3 tries")
        self.assertNotIn(self.KEY, scrubbed)
        self.assertIn("after 3 tries", scrubbed)

    def test_the_halt_record_itself_refuses_to_store_it(self) -> None:
        """Redaction at the point of STORAGE, so no future caller can forget."""
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            with self.assertRaises(simcore.Halt):
                simcore.halt_loudly(
                    work,
                    f"census refused against {self.URL}",
                    {"command": f"/bin/boot ledger-census --rpc-url {self.URL}", "exit_code": 4},
                )
            self.assertNotIn(self.KEY, (work / "HALT.json").read_text())

    def test_the_exit_record_refuses_to_store_it(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            simcore.record_exit(work, simcore.EXIT_CRASHED, detail=f"OSError talking to {self.URL}")
            self.assertNotIn(self.KEY, (work / "EXIT.json").read_text())

    def test_a_status_artifact_still_carries_none_of_it(self) -> None:
        """The file /pulse renders, checked whole rather than field by field."""
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "status.json"
            writer = simcore.StatusWriter(
                path=path, cluster_label="devnet", rpc_url=self.URL, mode="sustain",
                cadence_seconds=20.0,
            )
            writer.write(
                cycles_run=1, cycles_target=None, trades_landed=0, signatures=[],
                wallets=[], last_reconciliation=None,
            )
            self.assertNotIn(self.KEY, path.read_text())


class SpendLedgerTests(unittest.TestCase):
    """The fourth kill condition, and the one that was missing.

    HALT.json catches a broken law, EXIT.json catches an ending, the heartbeat
    deadline catches a SIGKILL -- and nothing caught a run that was merely
    expensive. A long unattended world with no ceiling is bounded only by
    cadence times fee, which is a number you compute rather than read.
    """

    def test_spend_is_cumulative_outflow_and_a_refill_does_not_forgive_it(self) -> None:
        ledger = simcore.SpendLedger(max_lamports=1_000)
        ledger.observe("P", 10_000)
        ledger.observe("P", 9_400)          # spent 600
        ledger.observe("P", 100_000)        # airdropped: credited, never subtracted
        ledger.observe("P", 99_700)         # spent another 300
        self.assertEqual(ledger.spent_lamports, 900)
        self.assertEqual(ledger.credited_lamports, 90_600)
        self.assertIsNone(ledger.exceeded(), "900 is under the 1000 ceiling")
        ledger.observe("P", 99_500)         # 200 more, now 1100
        self.assertEqual(ledger.spent_lamports, 1_100)
        reason = ledger.exceeded()
        self.assertIsNotNone(reason)
        self.assertIn("1100", reason)
        self.assertIn("1000", reason)
        # A first-observation delta would read 10000 -> 99500 as a CREDIT of
        # 89500 and never fire at all. That is the bug this shape avoids and
        # the assertion that says so.
        self.assertGreater(ledger.latest["P"], 10_000)

    def test_a_run_with_no_ceiling_says_so_rather_than_leaving_a_null(self) -> None:
        ledger = simcore.SpendLedger()
        ledger.observe("P", 10)
        ledger.observe("P", 1)
        self.assertEqual(ledger.spent_lamports, 9)
        self.assertIsNone(ledger.exceeded())
        self.assertFalse(ledger.describe()["bounded"])

    def test_several_payers_are_summed_and_each_is_reported(self) -> None:
        ledger = simcore.SpendLedger(max_lamports=100)
        for payer, balances in (("A", (500, 460)), ("B", (700, 630))):
            for balance in balances:
                ledger.observe(payer, balance)
        self.assertEqual(ledger.spent_lamports, 110)
        self.assertIsNotNone(ledger.exceeded())
        self.assertEqual(ledger.describe()["payers"], {"A": 460, "B": 630})

    def test_a_census_that_could_not_read_the_payer_is_a_gap_not_a_spend(self) -> None:
        ledger = simcore.SpendLedger(max_lamports=10)
        ledger.observe("P", 100)
        for unreadable in (None, "600", -1, True, 3.5):
            ledger.observe("P", unreadable)
        ledger.observe("P", 95)
        self.assertEqual(ledger.spent_lamports, 5)
        self.assertEqual(ledger.observations, 2)
        ledger.observe("", 1)
        self.assertEqual(ledger.observations, 2, "a nameless payer is not an observation")

    def test_a_budget_that_is_not_a_positive_whole_number_refuses(self) -> None:
        self.assertIsNone(simcore.SpendLedger.from_config({}).max_lamports)
        self.assertIsNone(simcore.SpendLedger.from_config({"budget": {}}).max_lamports)
        self.assertEqual(
            simcore.SpendLedger.from_config({"budget": {"max_lamports_spent": 5}}).max_lamports, 5
        )
        for bad in (0, -1, True, "1000", 1.5):
            with self.assertRaises(ValueError):
                simcore.SpendLedger.from_config({"budget": {"max_lamports_spent": bad}})


if __name__ == "__main__":
    unittest.main()
