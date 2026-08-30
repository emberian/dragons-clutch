#!/usr/bin/env python3
"""dClutch load simulator: sustained, rate-controlled, multi-wallet activity
against a live cluster (owned loopback validator or public devnet).

This is an ORCHESTRATION layer only.  Every mutation is performed by an
existing accepted driver that owns its own signed journal:

  * admission        -> dclutch-local-successor-bootstrap
                        {local-private-validator,devnet}-user-position-admission-v1
  * Direct sessions  -> ...-direct-trade-produce-v1 then ...-direct-trade-v1,
                        one invocation per durable mutation (replay-setup,
                        token-setup, lookup-*, capability-seal, hot)
  * wallet minting + funding -> tools/release/devnet-activity.sh (the activity
                        harness's own keygen, exact-target envelope funding,
                        signature markers; never the deployer as participant)
  * reconciliation   -> dclutch-local-successor-bootstrap ledger-census
                        (read-only, exits nonzero on any violated conservation
                        law; --prior chains delta laws across cycles)

The simulator adds: the sustain loop, cadence with jitter, backpressure
backoff, the per-cycle write-ahead journal (resume-never-resend), the halt
discipline (a census violation stops the run loudly), and the status.json
artifact a web surface can render.

Default is a DRY plan: nothing is signed or sent without --execute.
SIGTERM/SIGINT finish the in-flight cycle, seal journals, and write a final
status.  Rerunning over finalized cycle journals is a byte-identical no-op.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
from typing import Any, Optional
import urllib.request

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import simcore  # noqa: E402

DEVNET_GENESIS = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
SCHEMA_CONFIG = "dclutch-load-simulator-config-v1"
CHILD_TIMEOUT_SECONDS = 600


class Refusal(RuntimeError):
    pass


def load_config(path: Path) -> dict:
    body = json.loads(path.read_text())
    if body.get("schema") != SCHEMA_CONFIG:
        raise Refusal(f"config schema must be {SCHEMA_CONFIG}")
    label = body["cluster"]["label"]
    rpc = body["cluster"]["rpc_url"]
    if label == "devnet":
        if not rpc.startswith("https://"):
            raise Refusal("devnet rpc_url must be https")
        if body["cluster"].get("devnet_genesis") != DEVNET_GENESIS:
            raise Refusal(
                "devnet config must acknowledge the genesis hash in full "
                f"(cluster.devnet_genesis = {DEVNET_GENESIS})"
            )
    elif label == "local":
        if not rpc.startswith("http://127.0.0.1"):
            raise Refusal("local rpc_url must be a literal loopback origin")
        if body["cluster"].get("devnet_genesis"):
            raise Refusal("a loopback config must not carry a devnet acknowledgment")
    else:
        raise Refusal("cluster.label must be local or devnet")
    for key in ("bootstrap_bin", "work_dir"):
        if not str(body.get(key, "")).startswith("/"):
            raise Refusal(f"{key} must be an absolute path")
    if not os.access(body["bootstrap_bin"], os.X_OK):
        raise Refusal(f"bootstrap_bin is not executable: {body['bootstrap_bin']}")
    if "mainnet" in rpc:
        raise Refusal("mainnet is refused unconditionally")
    trade_mode = (body.get("trade") or {}).get("mode")
    if trade_mode not in ("local", "devnet", "none"):
        raise Refusal("trade.mode must be local, devnet, or none")
    return body


def run_child(
    argv: list, log_path: Path, timeout: float = CHILD_TIMEOUT_SECONDS
) -> subprocess.CompletedProcess:
    """One child driver invocation.  stdout+stderr are teed to a log file;
    stdin is closed; no shell.  The child owns its own journal."""
    log_path.parent.mkdir(parents=True, exist_ok=True)
    proc = subprocess.run(
        [str(a) for a in argv],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout,
        check=False,
    )
    log_path.write_bytes(proc.stdout or b"")
    return proc


def child_text(proc: subprocess.CompletedProcess) -> str:
    return (proc.stdout or b"").decode("utf-8", errors="replace")


class Simulator:
    def __init__(self, config: dict, execute: bool, sustain: bool, cycles: int):
        self.config = config
        self.execute = execute
        self.sustain = sustain
        self.cycles_target = None if sustain else cycles
        self.work = Path(config["work_dir"])
        self.work.mkdir(parents=True, exist_ok=True)
        os.chmod(self.work, 0o700)
        self.journal_root = self.work / "journal"
        self.stop = simcore.StopFlag()
        cadence = config.get("cadence", {})
        period_seconds = float(cadence.get("period_seconds", 8.0))
        jitter_fraction = float(cadence.get("jitter_fraction", 0.25))
        self.rate = simcore.RateController(
            period_seconds=period_seconds,
            jitter_fraction=jitter_fraction,
        )
        # The status artifact carries the deadline by which a living run must
        # have written again, so it is handed the same cadence the loop keeps
        # rather than a number a reader has to guess at.
        self.status = simcore.StatusWriter(
            path=self.work / "status.json",
            cluster_label=config["cluster"]["label"],
            rpc_url=config["cluster"]["rpc_url"],
            mode="sustain" if sustain else "finite",
            market_address=config.get("market_address"),
            cadence_seconds=period_seconds,
            jitter_fraction=jitter_fraction,
            grace_seconds=float(cadence.get("grace_seconds", 300.0)),
        )
        retention = config.get("census_retention", {})
        self.retention = simcore.CensusRetention(
            window=int(retention.get("window", simcore.DEFAULT_CENSUS_WINDOW)),
            keep_files=int(retention.get("keep_files", simcore.DEFAULT_CENSUS_KEEP_FILES)),
        )
        self.retention_report: Optional[dict] = None
        self.disk = simcore.DiskFloor(
            floor_bytes=int(retention.get("disk_floor_bytes", simcore.DEFAULT_DISK_FLOOR_BYTES)),
        )
        self.signatures: list = []
        self.trades_landed = 0
        # A market can be worth watching before it can be traded: the census
        # half needs only a cluster and the market's own accounts.
        self.census_only = (config.get("trade") or {}).get("mode") == "none"
        self.prior_census: Optional[Path] = None
        # Resume: adopt the newest already-finalized census as --prior.
        census_dir = self.work / "census"
        if census_dir.is_dir():
            finals = sorted(census_dir.glob("cycle-*.json"))
            if finals:
                self.prior_census = finals[-1]

    # ---------- cluster argv helpers ----------

    def cluster_args(self) -> list:
        args = ["--rpc-url", self.config["cluster"]["rpc_url"]]
        if self.config["cluster"]["label"] == "devnet":
            args += ["--i-mean-devnet", DEVNET_GENESIS]
        return args

    def boot(self) -> str:
        return self.config["bootstrap_bin"]

    # ---------- admission (once per participant, idempotent) ----------

    def ensure_admissions(self) -> None:
        for adm in self.config.get("admissions", []):
            name = adm["name"]
            output = Path(adm["output"])
            marker = self.work / "admissions" / f"{name}.done"
            if marker.exists():
                continue
            cmd = (
                "local-private-validator-user-position-admission-v1"
                if self.config["cluster"]["label"] == "local"
                else "devnet-user-position-admission-v1"
            )
            argv = [self.boot(), cmd, *self.cluster_args()]
            argv += [
                "--plan", adm["plan"],
                "--campaign-evidence", adm["campaign_evidence"],
                "--position-owner", adm["position_owner"],
                "--position-owner-keypair", adm["position_owner_keypair"],
                "--fee-payer", adm["fee_payer"],
                "--fee-payer-keypair", adm["fee_payer_keypair"],
                "--minimum-finalized-slot", str(adm["minimum_finalized_slot"]),
                "--output", str(output),
            ]
            collateral = adm.get("collateral")
            if collateral:
                argv += [
                    "--collateral-source-owner", collateral["source_owner"],
                    "--collateral-source-owner-keypair", collateral["source_owner_keypair"],
                    "--collateral-source-account", collateral["source_account"],
                    "--collateral-quantity-atoms", str(collateral["quantity_atoms"]),
                ]
            if self.execute:
                argv.append("--execute")
            proc = run_child(argv, self.work / "logs" / f"admission-{name}.log")
            if proc.returncode != 0:
                text = child_text(proc)
                if simcore.looks_like_backpressure(text):
                    raise BackpressureSignal(f"admission {name}")
                raise Refusal(
                    f"admission {name} refused (exit {proc.returncode}); "
                    f"log: {self.work / 'logs' / f'admission-{name}.log'}"
                )
            if self.execute:
                marker.parent.mkdir(parents=True, exist_ok=True)
                simcore.write_atomic(marker, (simcore.utc_now_iso() + "\n").encode())

    # ---------- Direct session per cycle ----------

    def session_dir(self, cycle: int) -> Path:
        return self.work / "sessions" / f"cycle-{cycle:06d}"

    def produce_session(self, cycle: int) -> Path:
        """Produce this cycle's Direct session with the accepted offline
        producer.  A finalized producer journal on disk is adopted, never
        reproduced (the producer refuses a non-empty output dir anyway)."""
        out = self.session_dir(cycle)
        producer_journal = out / "direct-trade-producer.json"
        if producer_journal.exists():
            body = json.loads(producer_journal.read_text())
            if body.get("phase") == "finalized":
                return out
            raise Refusal(
                f"cycle {cycle} producer journal exists in phase "
                f"{body.get('phase')!r}; refusing to reproduce over it"
            )
        out.mkdir(parents=True, exist_ok=True)
        trade = self.config["trade"]
        if self.config["cluster"]["label"] == "local":
            local = trade["local"]
            participant_report = local["participant_report"]
            key_dir = local["key_dir"]
            pair = self.pair_for_cycle(cycle)
            if pair:
                participant_report = pair.get("participant_report", participant_report)
                key_dir = pair.get("key_dir", key_dir)
            argv = [
                self.boot(), "local-private-validator-direct-trade-produce-v1",
                "--rpc-url", self.config["cluster"]["rpc_url"],
                "--plan", local["plan"],
                "--market-input", local["market_input"],
                "--campaign-report", local["campaign_report"],
                "--participant-report", participant_report,
                "--key-dir", key_dir,
                "--output-dir", str(out),
            ]
        else:
            dev = trade["devnet"]
            pair = self.pair_for_cycle(cycle)
            if pair is None:
                raise Refusal("devnet trade config needs at least one ticket pair")
            argv = [self.boot(), "devnet-direct-trade-produce-v1", *self.cluster_args()]
            for flag, key in (
                ("--plan", "plan"),
                ("--market-input", "market_input"),
                ("--campaign-report", "campaign_report"),
                ("--buyer-participant", "buyer_participant"),
                ("--checked-execution-release", "checked_execution_release"),
            ):
                argv += [flag, dev[key], f"--expected-{flag[2:]}-sha256", dev[f"{key}_sha256"]]
            argv += [
                "--seller-ticket", pair["seller_ticket"],
                "--expected-seller-ticket-sha256", pair["seller_ticket_sha256"],
                "--buyer-ticket", pair["buyer_ticket"],
                "--expected-buyer-ticket-sha256", pair["buyer_ticket_sha256"],
                "--payer", dev["payer"],
                "--payer-keypair", dev["payer_keypair"],
                "--output-dir", str(out),
            ]
        proc = run_child(argv, self.work / "logs" / f"produce-{cycle:06d}.log")
        if proc.returncode != 0:
            text = child_text(proc)
            if simcore.looks_like_backpressure(text):
                raise BackpressureSignal(f"produce cycle {cycle}")
            raise Refusal(
                f"cycle {cycle} session production refused (exit {proc.returncode}); "
                f"log: {self.work / 'logs' / f'produce-{cycle:06d}.log'}"
            )
        return out

    def session_completion(self, out: Path) -> Optional[dict]:
        """The finalized completion document, if the session has one."""
        for candidate in sorted(out.glob("*.json")):
            try:
                body = json.loads(candidate.read_text())
            except (ValueError, OSError):
                continue
            schema = str(body.get("schema", ""))
            if "direct-trade-finalized" in schema:
                return body
        return None

    def pulse_session(self, cycle: int, out: Path) -> dict:
        """Advance the session one durable mutation per invocation until its
        finalized completion exists.  Bounded by max_steps_per_session."""
        trade = self.config["trade"]
        session_file = out / "direct-trade-session.json"
        if not session_file.exists():
            raise Refusal(f"cycle {cycle}: {session_file} is absent after production")
        cmd = (
            "local-private-validator-direct-trade-v1"
            if self.config["cluster"]["label"] == "local"
            else "devnet-direct-trade-v1"
        )
        max_steps = int(trade.get("max_steps_per_session", 32))
        pause = float(trade.get("step_pause_seconds", 2.0))
        for step in range(1, max_steps + 1):
            done = self.session_completion(out)
            if done is not None:
                return done
            argv = [self.boot(), cmd, *self.cluster_args(), "--session", str(session_file)]
            if self.execute:
                argv.append("--execute")
            proc = run_child(
                argv, self.work / "logs" / f"direct-{cycle:06d}-{step:03d}.log"
            )
            if proc.returncode != 0:
                text = child_text(proc)
                if simcore.looks_like_backpressure(text):
                    raise BackpressureSignal(f"direct step {step} cycle {cycle}")
                raise Refusal(
                    f"cycle {cycle} direct step {step} refused (exit {proc.returncode}); "
                    f"log: {self.work / 'logs' / f'direct-{cycle:06d}-{step:03d}.log'}"
                )
            if not self.execute:
                return {"preflight": True}
            self.stop.sleep_interruptibly(pause)
        done = self.session_completion(out)
        if done is not None:
            return done
        raise Refusal(
            f"cycle {cycle} session did not finalize within {max_steps} steps; "
            "durable session journal remains, rerun resumes it"
        )

    # ---------- reconciliation ----------

    def run_census(self, cycle: int) -> dict:
        census_cfg = self.config.get("census")
        if not census_cfg:
            return {"ok": True, "skipped": "no census configured", "checked_at": simcore.utc_now_iso()}
        out = self.work / "census" / f"cycle-{cycle:06d}.json"
        out.parent.mkdir(parents=True, exist_ok=True)
        if out.exists():
            self.prior_census = out
            return {"ok": True, "resumed": True, "checked_at": simcore.utc_now_iso(), "output": str(out)}
        argv = [self.boot(), "ledger-census", *self.cluster_args()]
        argv += [
            "--mint", census_cfg["mint"],
            "--payer", census_cfg["payer"],
            "--hoard", census_cfg["hoard"],
            "--aggregate", census_cfg["aggregate"],
            "--claim-unit-atoms", str(census_cfg["claim_unit_atoms"]),
            "--stage", f"load-sim-cycle-{cycle:06d}",
            "--output", str(out),
        ]
        for label, pubkey in census_cfg.get("tokens", {}).items():
            argv += ["--token", f"{label}={pubkey}"]
        for label, pubkey in census_cfg.get("positions", {}).items():
            argv += ["--position", f"{label}={pubkey}"]
        for label, pubkey in census_cfg.get("watch", {}).items():
            argv += ["--watch", f"{label}={pubkey}"]
        if self.prior_census is not None:
            argv += ["--prior", str(self.prior_census)]
        proc = run_child(argv, self.work / "logs" / f"census-{cycle:06d}.log")
        if proc.returncode != 0:
            text = child_text(proc)
            if simcore.looks_like_backpressure(text):
                raise BackpressureSignal(f"census cycle {cycle}")
            # A census violation is a conservation divergence: halt loudly.
            simcore.halt_loudly(
                self.work,
                f"ledger-census violated a conservation law at cycle {cycle}",
                {
                    "exit_code": proc.returncode,
                    "log": str(self.work / "logs" / f"census-{cycle:06d}.log"),
                    # NOT shlex.join of the raw argv: it holds
                    # `--rpc-url https://…?api-key=<the live key>`.
                    "command": simcore.redact_command(argv),
                },
            )
        self.prior_census = out
        # Bound the series before the next cycle reads it back as `--prior`.
        # Superseded files are strict prefixes of this one and the window drop
        # is lossless for every conservation law (see CensusRetention), so the
        # newest file stays the whole series a reader needs while the
        # directory stops growing as the sum of its own history.
        self.retention_report = self.retention.apply(out.parent)
        return {"ok": True, "checked_at": simcore.utc_now_iso(), "output": str(out)}

    # ---------- wallets for status ----------

    def wallet_balance(self, address: str) -> Optional[int]:
        """One participant's lamports, read over JSON-RPC directly.

        NOT `solana balance --url <rpc_url>`, which is what this did before.
        That put the live endpoint -- credential and all -- into a process
        ARGUMENT LIST, where `ps` shows it to every user on the machine for as
        long as the child lives.

        BUT SAY THE WHOLE TRUTH, because half of it would be worse than none:
        this does NOT mean the credential never reaches a command line. Every
        successor driver takes `--rpc-url` (see `cluster_args`), so the census
        child is handed it once per cycle and `mint-wallets` hands it to the
        activity harness. That is inherent to the driver interface and is not
        something this function can fix. What it fixes is the one spawn we did
        not need: a balance read is four lines of JSON-RPC, so paying a CLI
        exposure for it bought nothing. Fewer processes carry the key, for
        less of the time; the exposure is narrowed, not closed. Closing it
        would mean the drivers reading the endpoint from the environment or a
        mode-0600 file instead of argv, which is a change to their interface
        and belongs to whoever owns it.

        A balance that does not answer is recorded as null, never as zero: the
        page renders "did not answer" and a fabricated zero would be a claim
        about someone's wallet.
        """
        payload = json.dumps({
            "jsonrpc": "2.0", "id": 1, "method": "getBalance", "params": [address],
        }).encode("utf-8")
        request = urllib.request.Request(
            self.config["cluster"]["rpc_url"], data=payload,
            headers={"Content-Type": "application/json"},
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                body = json.loads(response.read().decode("utf-8"))
        except (OSError, ValueError):
            return None
        value = (body.get("result") or {}).get("value")
        return value if isinstance(value, int) and value >= 0 else None

    def wallet_rows(self) -> list:
        return [
            {
                "address": w["address"],
                "role": "participant",
                "source": w.get("source", "staged"),
                "sol_lamports": self.wallet_balance(w["address"]),
            }
            for w in self.config.get("wallets", [])
        ]

    # ---------- the loop ----------

    def pair_for_cycle(self, cycle: int) -> Optional[dict]:
        trade = self.config["trade"]
        if self.config["cluster"]["label"] == "local":
            pairs = (trade.get("local") or {}).get("pairs") or []
        else:
            pairs = (trade.get("devnet") or {}).get("pairs") or []
        if not pairs:
            return None
        return pairs[(cycle - 1) % len(pairs)]

    def cycle_plan(self, cycle: int) -> dict:
        return {
            "cycle": cycle,
            "cluster": self.config["cluster"]["label"],
            # The plan is hashed into the cycle journal and read back on
            # resume, so it records the endpoint's identity, never its key.
            "rpc_url": simcore.redact_endpoint(self.config["cluster"]["rpc_url"]),
            "market": self.config.get("market_address"),
            "mode": "execute" if self.execute else "preflight",
            "trade_mode": self.config["cluster"]["label"],
            "pair": self.pair_for_cycle(cycle),
        }

    def write_status(self, cycles_run: int, recon: Optional[dict], stopping: bool = False,
                     halted: bool = False, halt_reason: Optional[str] = None) -> None:
        self.status.write(
            cycles_run=cycles_run,
            cycles_target=self.cycles_target,
            trades_landed=self.trades_landed,
            signatures=self.signatures,
            wallets=self.wallet_rows(),
            last_reconciliation=recon,
            halted=halted,
            halt_reason=halt_reason,
            stopping=stopping,
            backoff_seconds=self.rate.current_backoff,
            extra={
                # Said out loud so a reader never has to infer "zero trades"
                # from an empty list: this run is not attempting any.
                "trades_attempted": not self.census_only,
                # The storage this run is actually holding, and the ceiling it
                # cannot pass, both as numbers. A bound nobody can read is not
                # a bound, and this one is the fault that took the machine
                # down on 2026-08-30.
                "census_retention": self.retention_report,
            },
        )

    def run(self) -> int:
        """The loop, wrapped so that every ending this process can observe is
        written down.  `record_exit` in the `finally` is the honest half of
        the halt discipline: HALT.json means the LEDGER diverged and a human
        must clear it, EXIT.json means the PROCESS ended and says how.  The
        endings it cannot observe -- SIGKILL, ENOSPC on its own write -- leave
        no record at all, on purpose, and are read off the heartbeat deadline
        the status artifact stamps."""
        # Before anything is cleared: a work dir already halted refuses to
        # start, and that refusal must not erase the previous run's record of
        # how it ended -- the two together are the whole story a reader needs.
        simcore.refuse_if_halted(self.work)
        simcore.clear_exit_record(self.work)
        outcome, detail, code = simcore.EXIT_CRASHED, None, 1
        cycles_run = 0
        try:
            code, outcome, detail, cycles_run = self._run_cycles()
            return code
        except BaseException as error:  # noqa: BLE001 - recorded, then re-raised
            outcome = simcore.EXIT_CRASHED
            detail = f"{type(error).__name__}: {error}"
            raise
        finally:
            simcore.record_exit(
                self.work, outcome, detail=detail, cycles_run=cycles_run, exit_code=code,
            )

    def _run_cycles(self):
        self.stop.install()
        self.ensure_admissions()
        cycles_run = 0
        recon: Optional[dict] = None
        cycle = 1
        while True:
            if self.stop.requested:
                break
            if self.cycles_target is not None and cycle > self.cycles_target:
                break
            # Checked BETWEEN cycles, where stopping is still a choice. A
            # writer that discovers the volume is full mid-write cannot record
            # anything, which is exactly how this run died on 2026-08-30.
            low_disk = self.disk.check(self.work)
            if low_disk is not None:
                print(f"stopping: {low_disk}", file=sys.stderr)
                self.write_status(cycles_run, recon, stopping=True)
                return 4, simcore.EXIT_LOW_DISK, low_disk, cycles_run
            plan = self.cycle_plan(cycle)
            journal = simcore.CycleJournal.open(self.journal_root, cycle)
            existing = journal.assert_same_plan_or_absent(plan)
            if existing and journal.is_finalized():
                # Resume over a finalized cycle: byte-identical no-op.
                cycles_run = cycle
                for sig in existing.get("signatures", []):
                    if sig not in self.signatures:
                        self.signatures.append(sig)
                self.trades_landed = max(self.trades_landed, existing.get("trades_landed_total", 0))
                cycle += 1
                continue
            journal.record(simcore.PHASE_PLANNED, plan)
            try:
                journal.record(simcore.PHASE_EXECUTING, plan)
                sigs = []
                if self.census_only:
                    # No trade is attempted, so none is reported. The cycle is
                    # still a real cycle: it reads the market off the cluster
                    # and reconciles it against the conservation laws.
                    pass
                else:
                    out = self.produce_session(cycle)
                    completion = self.pulse_session(cycle, out)
                    for mutation in completion.get("mutations", []) or []:
                        sig = mutation.get("signature")
                        if sig:
                            sigs.append(sig)
                    if completion.get("signature"):
                        sigs.append(completion["signature"])
                    if self.execute and not completion.get("preflight"):
                        self.trades_landed += 1
                    self.signatures.extend(s for s in sigs if s not in self.signatures)
                recon = self.run_census(cycle)
                journal.record(
                    simcore.PHASE_FINALIZED, plan,
                    signatures=sigs,
                    trades_landed_total=self.trades_landed,
                    reconciliation=recon,
                )
                cycles_run = cycle
                self.write_status(cycles_run, recon, stopping=self.stop.requested)
                self.rate.on_clean_cycle()
                if not self.execute:
                    print(f"preflight completed for cycle {cycle}; rerun with --execute")
                    return 0, simcore.EXIT_PREFLIGHT, "preflight signed nothing", cycles_run
                cycle += 1
                if self.stop.requested:
                    break
                if self.cycles_target is None or cycle <= self.cycles_target:
                    self.stop.sleep_interruptibly(self.rate.next_delay())
            except BackpressureSignal as backpressure:
                wait = self.rate.on_backpressure()
                journal.record(
                    simcore.PHASE_PLANNED, plan,
                    note=f"backpressure ({backpressure}); retrying after {wait:.0f}s",
                )
                print(f"backpressure: {backpressure}; backing off {wait:.0f}s", file=sys.stderr)
                self.write_status(cycles_run, recon, stopping=self.stop.requested)
                self.stop.sleep_interruptibly(wait)
                # same cycle retries; durable child journals make it a resume
            except simcore.Halt as halt:
                journal.record(simcore.PHASE_HALTED, plan, reason=str(halt))
                self.write_status(cycles_run, recon, halted=True, halt_reason=str(halt))
                print(f"HALTED: {halt}", file=sys.stderr)
                return 3, simcore.EXIT_HALTED, str(halt), cycles_run
        self.write_status(cycles_run, recon, stopping=False)
        if self.stop.requested:
            print(f"stopped cleanly on {self.stop.signal_name}; journals sealed at cycle {cycles_run}")
            return 0, simcore.EXIT_SIGNALLED, (
                f"finished the in-flight cycle on {self.stop.signal_name} and sealed its journal"
            ), cycles_run
        return 0, simcore.EXIT_COMPLETED, (
            f"ran the {cycles_run} cycle(s) it was asked for"
        ), cycles_run


class BackpressureSignal(RuntimeError):
    pass


def cmd_mint_wallets(args: argparse.Namespace) -> int:
    """Mint + fund additional disposable participants THROUGH the activity
    harness (tools/release/devnet-activity.sh): its keygen, its exact-target
    envelope funding, its signature markers.  Devnet only; the payer is the
    campaign payer, never the deployer."""
    config = load_config(Path(args.config))
    if config["cluster"]["label"] != "devnet":
        raise Refusal("mint-wallets is devnet-only; local participants come from the probe fixtures")
    work = Path(config["work_dir"]) / "minted-wallets"
    script = Path(config.get("activity_sh", str(HERE.parent / "release" / "devnet-activity.sh")))
    if not script.is_file():
        raise Refusal(f"activity harness not found at {script}; set activity_sh in config")
    argv = [
        "bash", str(script),
        "--rpc-url", config["cluster"]["rpc_url"],
        "--i-mean-devnet", DEVNET_GENESIS,
        "--state-dir", str(work),
        "--participants", str(args.count),
        "--wallet-lamports", str(args.envelope_lamports),
        "--payer-keypair", args.payer_keypair,
    ]
    if args.execute:
        argv.append("--execute")
    proc = subprocess.run(argv, stdin=subprocess.DEVNULL, check=False)
    if proc.returncode == 0 and (work / "participants.tsv").exists():
        print(f"minted-wallet manifest: {work / 'participants.tsv'}")
    return proc.returncode


def main(argv: Optional[list] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)

    run_p = sub.add_parser("run", help="run the load loop (default: preflight one cycle)")
    run_p.add_argument("--config", required=True, help="absolute config JSON path")
    run_p.add_argument("--cycles", type=int, default=3, help="finite cycle count (default 3)")
    run_p.add_argument("--sustain", action="store_true", help="run continuously until SIGTERM")
    run_p.add_argument("--execute", action="store_true", help="sign and send (default preflight)")

    mint_p = sub.add_parser("mint-wallets", help="mint+fund extra devnet participants via the activity harness")
    mint_p.add_argument("--config", required=True)
    mint_p.add_argument("--count", type=int, required=True)
    mint_p.add_argument("--envelope-lamports", type=int, default=20_000_000,
                        help="exact-target funding envelope per wallet (harness default 20000000)")
    mint_p.add_argument("--payer-keypair", required=True,
                        help="campaign payer keypair (NEVER the deployer)")
    mint_p.add_argument("--execute", action="store_true")

    args = parser.parse_args(argv)
    try:
        if args.command == "mint-wallets":
            return cmd_mint_wallets(args)
        config = load_config(Path(args.config))
        sim = Simulator(config, execute=args.execute, sustain=args.sustain, cycles=args.cycles)
        return sim.run()
    except (Refusal, simcore.Halt, simcore.JournalConflict) as refusal:
        print(f"REFUSED: {refusal}", file=sys.stderr)
        return 2
    except (OSError, ValueError, KeyError) as defect:
        print(f"REFUSED: {type(defect).__name__}: {defect}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
