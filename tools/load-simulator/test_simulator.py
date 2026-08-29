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
outdir=""; session=""; output=""
while [ "$#" -gt 0 ]; do case "$1" in
  --output-dir) outdir="$2"; shift 2 ;;
  --session) session="$2"; shift 2 ;;
  --output) output="$2"; shift 2 ;;
  *) shift ;;
esac; done
case "$cmd" in
  local-private-validator-direct-trade-produce-v1)
    echo '{"schema":"fake-session"}' > "$outdir/direct-trade-session.json"
    echo '{"schema":"fake-public"}' > "$outdir/direct-trade-public.json"
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
    if [ -f "$here/census-violation" ]; then
      echo "conservation law violated: hoard delta -3" >&2; exit 4
    fi
    echo '{"schema":"fake-census","ok":true}' > "$output"
    ;;
  local-private-validator-user-position-admission-v1)
    echo '{"schema":"fake-admission","phase":"finalized"}' > "$output"
    ;;
  *) echo "fake boot: unknown command $cmd" >&2; exit 9 ;;
esac
"""


class SimulatorLoopTest(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
