#!/usr/bin/env python3
"""End-to-end tests of the load-simulator orchestration against a FAKE
bootstrap binary.  The fake stands in for dclutch-local-successor-bootstrap:
it honors the same subcommand names and the same durable-artifact contract
(producer journal, session file, completion file, census exit codes), so
these tests prove the orchestrator's state machine -- production, step
pulsing, resume-never-resend, backpressure backoff, census halt -- without a
validator.  The real drivers are exercised by run-local.sh.
"""

from __future__ import annotations

import datetime as dt
import importlib.util
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("dclutch_load_simulator", HERE / "simulator.py")
assert SPEC is not None and SPEC.loader is not None
simulator = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = simulator
SPEC.loader.exec_module(simulator)
simcore = simulator.simcore

FAKE_BOOT = r"""#!/usr/bin/env bash
# Fake successor bootstrap for orchestrator tests.  Behavior is steered by
# env markers dropped next to the binary by the test.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
cmd="${1:-}"; shift || true
outdir=""; session=""; output=""; prior=""; stage=""; tokens=""
while [ "$#" -gt 0 ]; do case "$1" in
  --output-dir) outdir="$2"; shift 2 ;;
  --session) session="$2"; shift 2 ;;
  --output) output="$2"; shift 2 ;;
  --prior) prior="$2"; shift 2 ;;
  --stage) stage="$2"; shift 2 ;;
  --token) tokens="$tokens $2"; shift 2 ;;
  *) shift ;;
esac; done
case "$cmd" in
  local-private-validator-direct-trade-produce-v1)
    echo '{"schema":"fake-session"}' > "$outdir/direct-trade-session.json"
    # The real public manifest carries the two Direct token PDAs
    # `direct_token_setup_v1` creates, under `tokenSetup`. The census binding
    # reads them from here rather than deriving a protocol PDA in Python.
    cat > "$outdir/direct-trade-public.json" <<'MANIFEST'
{"schema":"fake-public",
 "tokenSetup":{"sellerToken":"SellerDirectTokenPda","feeToken":"VenueFeeTokenPda"}}
MANIFEST
    echo '{"schema":"fake-producer","phase":"finalized"}' > "$outdir/direct-trade-producer.json"
    ;;
  local-private-validator-direct-trade-v1)
    sdir="$(dirname "$session")"
    count_file="$sdir/.steps"
    count=$(cat "$count_file" 2>/dev/null || echo 0)
    count=$((count+1)); echo "$count" > "$count_file"
    if [ -f "$here/backpressure-once" ] && [ ! -f "$sdir/.bp-done" ]; then
      touch "$sdir/.bp-done"; echo "HTTP 429 Too Many Requests"; exit 1
    fi
    threshold=$(cat "$here/steps-needed" 2>/dev/null || echo 3)
    if [ "$count" -ge "$threshold" ]; then
      printf '{"schema":"dclutch-devnet-direct-trade-finalized-v1","market":"Mkt","signature":"sig-final-%s","mutations":[{"kind":"hot","signature":"sig-hot-%s"}]}\n' "$count" "$count" > "$sdir/direct-trade-completion.json"
    fi
    ;;
  ledger-census)
    # Every --token this census was given, one per line, newest run last.
    for entry in $tokens; do echo "$entry" >> "$here/census-tokens.log"; done
    echo "--- $stage" >> "$here/census-tokens.log"
    if [ -f "$here/census-violation" ]; then
      echo "conservation law violated: hoard delta -3" >&2; exit 4
    fi
    # CUMULATIVE, like the real one: `--prior` is reloaded and the new
    # observation appended, so the newest file is the whole series and the
    # directory grows as the SUM of its files. That growth is the fault the
    # retention bound exists to stop, so the fake has to reproduce it or the
    # bound is never exercised. Encoding matches serde_json::to_vec_pretty.
    DCLUTCH_PRIOR="$prior" DCLUTCH_STAGE="$stage" DCLUTCH_OUTPUT="$output" python3 - <<'PY'
import json, os
prior, stage, output = os.environ["DCLUTCH_PRIOR"], os.environ["DCLUTCH_STAGE"], os.environ["DCLUTCH_OUTPUT"]
series = json.load(open(prior)) if prior else []
series.append({
    "stage": stage,
    "slot": 1000 + len(series),
    "hoard_atoms": 0,
    "tracked_collateral": 0,
    "aggregate_supply": [0, 0],
    "accounts": {},
    "position_balances": {},
    "token_atoms": {},
    "verdicts": [{"law": "L1", "status": "holds", "detail": "fake census"}],
    # Padding so one observation has a realistic weight and the growth this
    # bounds is visible at test scale rather than only at devnet scale.
    "padding": "x" * 512,
})
open(output, "w").write(json.dumps(series, indent=2))
PY
    ;;
  local-private-validator-user-position-admission-v1)
    echo '{"schema":"fake-admission","phase":"finalized"}' > "$output"
    ;;
  *) echo "fake boot: unknown command $cmd" >&2; exit 9 ;;
esac
"""


class SimulatorHarness(unittest.TestCase):
    """The fake-driver rig, with no assertions of its own, so the suites below
    share it without re-running each other's cases."""

    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.boot = self.root / "fake-boot"
        self.boot.write_text(FAKE_BOOT)
        self.boot.chmod(self.boot.stat().st_mode | stat.S_IEXEC)
        self.work = self.root / "work"

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def config(self, **overrides) -> dict:
        body = {
            "schema": "dclutch-load-simulator-config-v1",
            "cluster": {"label": "local", "rpc_url": "http://127.0.0.1:19999/"},
            "bootstrap_bin": str(self.boot),
            "work_dir": str(self.work),
            "market_address": "MktAddr",
            "cadence": {"period_seconds": 0.0, "jitter_fraction": 0.0},
            "trade": {
                "mode": "local",
                "max_steps_per_session": 6,
                "step_pause_seconds": 0.0,
                "local": {
                    "plan": "/dev/null", "market_input": "/dev/null",
                    "campaign_report": "/dev/null", "participant_report": "/dev/null",
                    "key_dir": "/dev/null",
                },
            },
            "census": {
                "mint": "M", "payer": "P", "hoard": "H", "aggregate": "A",
                "claim_unit_atoms": 7, "tokens": {}, "positions": {}, "watch": {},
            },
            "wallets": [],
        }
        body.update(overrides)
        return body

    def write_config(self, body: dict) -> Path:
        path = self.root / "config.json"
        path.write_text(json.dumps(body))
        return path

    def run_sim(self, *argv: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(HERE / "simulator.py"), *argv],
            capture_output=True, text=True, timeout=120,
        )


class DirectTokenCensusBindingTest(SimulatorHarness):
    """The two accounts a fill pays, and the law that goes red without them.

    `direct_token_setup_v1` creates one Direct token PDA for the seller and one
    for the venue fee, and a fill moves collateral into them. Until 2026-09-02
    nothing named either, so L1 -- tracked atoms == Mint supply -- would have
    reported a shortfall of exactly the traded atoms and blamed the run for a
    gap that is really an account nobody bound.
    """

    def census_tokens(self) -> list:
        log = self.root / "census-tokens.log"
        return log.read_text().splitlines() if log.is_file() else []

    def test_the_census_names_both_direct_token_accounts_after_a_session(self) -> None:
        (self.root / "steps-needed").write_text("1")
        cfg = self.write_config(self.config())
        proc = self.run_sim("run", "--config", str(cfg), "--cycles", "1", "--execute")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        rows = self.census_tokens()
        self.assertIn("direct_seller_token=SellerDirectTokenPda", rows)
        self.assertIn("direct_venue_fee_token=VenueFeeTokenPda", rows)

    def test_a_restarted_run_adopts_the_bindings_off_disk(self) -> None:
        """The accounts outlive the cycle that created them.

        A resumed run that stopped naming them would violate L1 over collateral
        sitting exactly where its own predecessor put it. Asserted at
        CONSTRUCTION, because a run that merely produces a new session would
        rebind them anyway and prove nothing about resume.
        """
        session = self.work / "sessions" / "cycle-000001"
        session.mkdir(parents=True)
        (session / "direct-trade-public.json").write_text(json.dumps(
            {"tokenSetup": {"sellerToken": "SellerFromDisk", "feeToken": "FeeFromDisk"}}))
        sim = simulator.Simulator(self.config(), execute=False, sustain=False, cycles=1)
        self.assertEqual(sim.direct_token_bindings, {
            "direct_seller_token": "SellerFromDisk",
            "direct_venue_fee_token": "FeeFromDisk",
        })

    def test_an_explicitly_configured_label_is_not_overridden(self) -> None:
        """An operator who names an address is not second-guessed by a file."""
        (self.root / "steps-needed").write_text("1")
        body = self.config()
        body["census"]["tokens"] = {"direct_seller_token": "OperatorSaidThis"}
        cfg = self.write_config(body)
        self.assertEqual(self.run_sim(
            "run", "--config", str(cfg), "--cycles", "1", "--execute").returncode, 0)
        rows = self.census_tokens()
        self.assertIn("direct_seller_token=OperatorSaidThis", rows)
        self.assertNotIn("direct_seller_token=SellerDirectTokenPda", rows)

    def test_one_label_naming_two_addresses_refuses(self) -> None:
        """Silently rebinding a label stops tracking the first account, which
        is the exact shape of the L1 shortfall this binding exists to close."""
        sim = simulator.Simulator(self.config(), execute=False, sustain=False, cycles=1)
        first = self.root / "s1"
        first.mkdir()
        (first / "direct-trade-public.json").write_text(json.dumps(
            {"tokenSetup": {"sellerToken": "A", "feeToken": "B"}}))
        sim.adopt_direct_token_bindings(first)
        self.assertEqual(sim.direct_token_bindings["direct_seller_token"], "A")
        second = self.root / "s2"
        second.mkdir()
        (second / "direct-trade-public.json").write_text(json.dumps(
            {"tokenSetup": {"sellerToken": "C", "feeToken": "B"}}))
        with self.assertRaises(simulator.Refusal) as caught:
            sim.adopt_direct_token_bindings(second)
        self.assertIn("cannot track two accounts under one label", str(caught.exception))

    def test_a_manifest_without_the_block_binds_nothing(self) -> None:
        """A world with no trade names no trade accounts, rather than binding a
        placeholder the census would then fail to read."""
        sim = simulator.Simulator(self.config(), execute=False, sustain=False, cycles=1)
        out = self.root / "s0"
        out.mkdir()
        (out / "direct-trade-public.json").write_text(json.dumps({"schema": "no-setup"}))
        sim.adopt_direct_token_bindings(out)
        self.assertEqual(sim.direct_token_bindings, {})
        sim.adopt_direct_token_bindings(self.root / "does-not-exist")
        self.assertEqual(sim.direct_token_bindings, {})


class SimulatorLoopTest(SimulatorHarness):
    def test_preflight_runs_one_cycle_and_sends_nothing(self) -> None:
        cfg = self.write_config(self.config())
        proc = self.run_sim("run", "--config", str(cfg), "--cycles", "3")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertIn("preflight completed for cycle 1", proc.stdout)
        status = json.loads((self.work / "status.json").read_text())
        self.assertEqual(status["trades"]["landed"], 0)

    def test_execute_runs_cycles_lands_trades_and_reconciles(self) -> None:
        (self.root / "steps-needed").write_text("3")
        cfg = self.write_config(self.config())
        proc = self.run_sim("run", "--config", str(cfg), "--cycles", "2", "--execute")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        status = json.loads((self.work / "status.json").read_text())
        self.assertEqual(status["cycles"]["run"], 2)
        self.assertEqual(status["trades"]["landed"], 2)
        self.assertTrue(status["last_reconciliation"]["ok"])
        self.assertTrue(any(s.startswith("sig-hot") for s in status["trades"]["signatures"]))
        # Census chained: cycle 2's census exists alongside cycle 1's.
        self.assertTrue((self.work / "census" / "cycle-000001.json").exists())
        self.assertTrue((self.work / "census" / "cycle-000002.json").exists())

    def test_census_only_reconciles_every_cycle_and_attempts_no_trade(self) -> None:
        # A market can be worth watching before it can be traded.
        body = self.config()
        body["trade"] = {"mode": "none"}
        cfg = self.write_config(body)
        proc = self.run_sim("run", "--config", str(cfg), "--cycles", "2", "--execute")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        status = json.loads((self.work / "status.json").read_text())
        self.assertEqual(status["cycles"]["run"], 2)
        self.assertEqual(status["trades"]["landed"], 0)
        self.assertEqual(status["trades"]["signatures"], [])
        # Said out loud, so zero trades is never mistaken for a stall.
        self.assertFalse(status["trades_attempted"])
        self.assertTrue(status["last_reconciliation"]["ok"])
        # The census still chains cycle to cycle.
        self.assertTrue((self.work / "census" / "cycle-000001.json").exists())
        self.assertTrue((self.work / "census" / "cycle-000002.json").exists())

    def test_an_unknown_trade_mode_is_refused(self) -> None:
        body = self.config()
        body["trade"] = {"mode": "whatever"}
        cfg = self.write_config(body)
        proc = self.run_sim("run", "--config", str(cfg), "--cycles", "1")
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("trade.mode", proc.stderr + proc.stdout)

    def test_rerun_over_finalized_journals_is_a_noop(self) -> None:
        (self.root / "steps-needed").write_text("2")
        cfg = self.write_config(self.config())
        first = self.run_sim("run", "--config", str(cfg), "--cycles", "2", "--execute")
        self.assertEqual(first.returncode, 0, first.stderr)
        journal_bytes = {
            p: p.read_bytes() for p in (self.work / "journal").rglob("cycle.json")
        }
        steps_before = {
            p: p.read_text() for p in (self.work / "sessions").rglob(".steps")
        }
        second = self.run_sim("run", "--config", str(cfg), "--cycles", "2", "--execute")
        self.assertEqual(second.returncode, 0, second.stderr)
        for p, before in journal_bytes.items():
            self.assertEqual(before, p.read_bytes(), f"journal rewritten: {p}")
        for p, before in steps_before.items():
            self.assertEqual(before, p.read_text(), f"driver re-invoked: {p}")
        status = json.loads((self.work / "status.json").read_text())
        self.assertEqual(status["cycles"]["run"], 2)
        self.assertEqual(status["trades"]["landed"], 2)

    def test_backpressure_backs_off_and_the_cycle_still_lands(self) -> None:
        (self.root / "steps-needed").write_text("2")
        (self.root / "backpressure-once").write_text("1")
        cfg = self.write_config(self.config())
        proc = self.run_sim("run", "--config", str(cfg), "--cycles", "1", "--execute")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertIn("backpressure", proc.stderr)
        status = json.loads((self.work / "status.json").read_text())
        self.assertEqual(status["trades"]["landed"], 1)
        self.assertFalse(status["halted"])

    def test_census_violation_halts_loudly_and_blocks_restart(self) -> None:
        (self.root / "steps-needed").write_text("1")
        (self.root / "census-violation").write_text("1")
        cfg = self.write_config(self.config())
        proc = self.run_sim("run", "--config", str(cfg), "--cycles", "1", "--execute")
        self.assertEqual(proc.returncode, 3, proc.stderr)
        self.assertIn("HALTED", proc.stderr)
        self.assertTrue((self.work / "HALT.json").exists())
        status = json.loads((self.work / "status.json").read_text())
        self.assertTrue(status["halted"])
        again = self.run_sim("run", "--config", str(cfg), "--cycles", "1", "--execute")
        self.assertEqual(again.returncode, 2)
        self.assertIn("halted", again.stderr)

    def test_devnet_config_requires_full_genesis_acknowledgment(self) -> None:
        body = self.config()
        body["cluster"] = {"label": "devnet", "rpc_url": "https://api.devnet.solana.com"}
        cfg = self.write_config(body)
        proc = self.run_sim("run", "--config", str(cfg), "--cycles", "1")
        self.assertEqual(proc.returncode, 2)
        self.assertIn("acknowledge", proc.stderr)

    def test_loopback_config_refuses_devnet_acknowledgment(self) -> None:
        body = self.config()
        body["cluster"]["devnet_genesis"] = simulator.DEVNET_GENESIS
        cfg = self.write_config(body)
        proc = self.run_sim("run", "--config", str(cfg), "--cycles", "1")
        self.assertEqual(proc.returncode, 2)

    def test_sigterm_finishes_inflight_cycle_and_seals(self) -> None:
        (self.root / "steps-needed").write_text("2")
        cfg = self.write_config(self.config())
        import signal as sig
        import threading
        proc = subprocess.Popen(
            [sys.executable, str(HERE / "simulator.py"), "run", "--config", str(cfg),
             "--sustain", "--execute"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )

        def fire() -> None:
            # let at least one cycle start, then request stop
            import time
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline:
                if (self.work / "journal" / "cycle-000001" / "cycle.json").exists():
                    break
                time.sleep(0.1)
            proc.send_signal(sig.SIGTERM)

        t = threading.Thread(target=fire)
        t.start()
        out, err = proc.communicate(timeout=90)
        t.join()
        self.assertEqual(proc.returncode, 0, err)
        self.assertIn("stopped cleanly", out)
        status = json.loads((self.work / "status.json").read_text())
        self.assertFalse(status["halted"])
        # every recorded journal is sealed (finalized), none abandoned mid-phase
        for p in (self.work / "journal").rglob("cycle.json"):
            body = json.loads(p.read_text())
            self.assertEqual(body["phase"], "finalized", p)
        # And it said so: a stop it can observe is a stop it records.
        exit_record = json.loads((self.work / "EXIT.json").read_text())
        self.assertEqual(exit_record["outcome"], simcore.EXIT_SIGNALLED)
        self.assertEqual(exit_record["exit_code"], 0)


class DeathHonestyTest(SimulatorHarness):
    """The second fault: the run was killed mid-cycle 124 inside the ENOSPC
    window and left an artifact still reading `halted: false` with no halt
    record beside it.  Nothing in the file contradicted the claim."""

    def test_a_finished_run_records_how_it_ended(self) -> None:
        (self.root / "steps-needed").write_text("1")
        cfg = self.write_config(self.config())
        proc = self.run_sim("run", "--config", str(cfg), "--cycles", "2", "--execute")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        body = json.loads((self.work / "EXIT.json").read_text())
        self.assertEqual(body["outcome"], simcore.EXIT_COMPLETED)
        self.assertEqual(body["cycles_run"], 2)

    def test_a_halt_records_both_and_they_say_different_things(self) -> None:
        (self.root / "steps-needed").write_text("1")
        (self.root / "census-violation").write_text("1")
        cfg = self.write_config(self.config())
        proc = self.run_sim("run", "--config", str(cfg), "--cycles", "1", "--execute")
        self.assertEqual(proc.returncode, 3, proc.stderr)
        # HALT.json: the LEDGER diverged, and a human must clear it.
        self.assertTrue((self.work / "HALT.json").exists())
        # EXIT.json: the PROCESS ended, and here is how.
        self.assertEqual(
            json.loads((self.work / "EXIT.json").read_text())["outcome"], simcore.EXIT_HALTED,
        )

    def test_a_kill_leaves_no_record_and_the_artifact_says_it_expected_one(self) -> None:
        """THE CASE THAT HAPPENED.  SIGKILL runs no handler, so nothing is
        written on the way down -- and pretending otherwise is the fault, not
        the fix.  What must hold is that the artifact left behind is READABLE
        AS DEAD: its own deadline passes, with no exit record beside it."""
        (self.root / "steps-needed").write_text("3")
        body = self.config()
        body["cadence"] = {"period_seconds": 0.2, "jitter_fraction": 0.25, "grace_seconds": 1.0}
        cfg = self.write_config(body)
        import signal as sig
        import time
        proc = subprocess.Popen(
            [sys.executable, str(HERE / "simulator.py"), "run", "--config", str(cfg),
             "--sustain", "--execute"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )
        try:
            deadline = time.monotonic() + 30
            status_path = self.work / "status.json"
            while time.monotonic() < deadline and not status_path.exists():
                time.sleep(0.05)
            self.assertTrue(status_path.exists(), "the run never wrote a status to kill it after")
            proc.send_signal(sig.SIGKILL)
            proc.communicate(timeout=30)
        finally:
            if proc.poll() is None:
                proc.kill()

        status = json.loads(status_path.read_text())
        # Exactly the dead run's artifact: nothing in the flags is alarming.
        self.assertFalse(status["halted"])
        self.assertFalse(status["stopping"])
        self.assertIsNone(status["halt_reason"])
        # It could not record an ending, and it did not invent one.
        self.assertFalse((self.work / "EXIT.json").exists())
        # But it stamped the instant by which a living run must have written
        # again, and a reader with a clock can evaluate that unaided.
        expected = dt.datetime.fromisoformat(status["heartbeat"]["expected_next_update_by"])
        self.assertLess(
            (expected - dt.datetime.fromisoformat(status["updated_at"])).total_seconds(), 10.0,
        )
        time.sleep(2.0)
        self.assertLess(expected, dt.datetime.now(dt.timezone.utc), "the deadline never expires")


class WalletRowTest(SimulatorHarness):
    """Participant balances reach the status artifact without the endpoint
    reaching the process table."""

    def test_a_wallet_that_does_not_answer_is_null_and_never_zero(self) -> None:
        (self.root / "steps-needed").write_text("1")
        body = self.config()
        # The loopback endpoint in the test config has nothing listening, so
        # this exercises the real unreachable path rather than a mocked one.
        body["wallets"] = [{"address": "5oGySWQAKZ3fLmAwUbG6WifP7dCF6FRtriawtgxoCZXf",
                            "source": "staged"}]
        cfg = self.write_config(body)
        proc = self.run_sim("run", "--config", str(cfg), "--cycles", "1", "--execute")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        rows = json.loads((self.work / "status.json").read_text())["wallets"]
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["role"], "participant")
        self.assertEqual(rows[0]["source"], "staged")
        # Null, not zero. A fabricated zero would be a claim about the wallet.
        self.assertIsNone(rows[0]["sol_lamports"])

    def test_the_balance_read_puts_no_endpoint_on_a_command_line(self) -> None:
        """It used to shell `solana balance --url <rpc_url>`, which shows the
        credential to every `ps` on the machine for as long as the child
        lives. Redacting the files we write while handing the key to the
        process table is not a redaction story."""
        source = (HERE / "simulator.py").read_text()
        # The old argv literal, gone. (The name still appears once, in the
        # docstring saying why it is gone; that is a comment, not a call.)
        self.assertNotIn('"solana", "balance"', source)
        self.assertIn("urllib.request.urlopen", source)
        # And no child process is spawned for a balance at all.
        import inspect
        body = inspect.getsource(simulator.Simulator.wallet_balance)
        self.assertNotIn("subprocess", body)
        self.assertIn("urlopen", body)


class StorageBoundTest(SimulatorHarness):
    """The first fault, end to end: the census directory that filled the
    machine's data volume and took every lane's shell down with it."""

    def test_a_running_loop_holds_its_census_inside_the_stated_bound(self) -> None:
        (self.root / "steps-needed").write_text("1")
        body = self.config()
        body["cadence"] = {"period_seconds": 0.01}
        body["census_retention"] = {"window": 4, "keep_files": 2}
        cfg = self.write_config(body)
        proc = self.run_sim("run", "--config", str(cfg), "--cycles", "25", "--execute")
        self.assertEqual(proc.returncode, 0, proc.stderr)

        census = self.work / "census"
        files = sorted(census.glob("cycle-*.json"))
        # Twenty-five cycles ran; two census files remain, not twenty-five.
        self.assertEqual([p.name for p in files], ["cycle-000024.json", "cycle-000025.json"])
        status = json.loads((self.work / "status.json").read_text())
        self.assertEqual(status["cycles"]["run"], 25)

        # The newest file is still the whole series a reader needs, ending at
        # the cycle that wrote it -- the property simulator-series.mjs mines.
        newest = json.loads(files[-1].read_bytes())
        self.assertEqual(len(newest), 4)
        self.assertEqual(newest[-1]["stage"], "load-sim-cycle-000025")

        # And the artifact states the ceiling it is under, as a number.
        report = status["census_retention"]
        actual = sum(p.stat().st_size for p in files)
        self.assertEqual(report["bytes_on_disk"], actual)
        self.assertLessEqual(actual, report["bytes_bound"])
        self.assertEqual(
            report["bytes_bound"], 2 * 4 * report["bytes_per_observation"],
        )

    def test_the_low_disk_floor_stops_between_cycles_rather_than_mid_write(self) -> None:
        """The other half of not repeating the outage: whatever else is
        filling the volume, this process is not the one that takes the last of
        it, and it stops while it still has room to say so."""
        (self.root / "steps-needed").write_text("1")
        body = self.config()
        body["census_retention"] = {"disk_floor_bytes": 1 << 62}
        cfg = self.write_config(body)
        proc = self.run_sim("run", "--config", str(cfg), "--cycles", "3", "--execute")
        self.assertEqual(proc.returncode, 4, proc.stderr)
        self.assertIn("stopping", proc.stderr)
        record = json.loads((self.work / "EXIT.json").read_text())
        self.assertEqual(record["outcome"], simcore.EXIT_LOW_DISK)
        # It stopped BEFORE spending a cycle, and it is not halted: a full
        # volume is an environment fact, not a conservation divergence, so a
        # restart with room needs no human to clear anything.
        self.assertFalse((self.work / "HALT.json").exists())
        self.assertEqual(json.loads((self.work / "status.json").read_text())["cycles"]["run"], 0)


if __name__ == "__main__":
    unittest.main()
