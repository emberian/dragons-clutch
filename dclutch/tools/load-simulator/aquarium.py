#!/usr/bin/env python3
"""Supervise a bounded, public devnet aquarium of already-founded markets.

The aquarium deliberately does *not* create a market, create a wallet, discover
a user's key, or construct a transaction.  Its only mutating child is the
existing ``simulator.py run --config … --cycles N --execute`` driver.  That
child owns the signed admission and Direct-session journals; this process owns
the inventory, epoch selection, process lifetime, and a credential-free public
status document.

An epoch is finite and names a distinct checked simulator config.  Direct
tickets are single-use evidence, so using ``--sustain`` and cycling a short
ticket list would turn a bounded demonstration into an ambiguous replay.  New
activity is therefore precommitted as another epoch.  When the queue is empty
the aquarium keeps observing the configured market inventory and says that it
needs an operator-provisioned epoch; it never fabricates activity.

Usage:
  python3 aquarium.py check --config /absolute/aquarium.json
  python3 aquarium.py run --config /absolute/aquarium.json [--execute]

``--execute`` is the sole opt-in to signing.  Without it, each selected child
runs the current driver's preflight and no private key is opened.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time
from typing import Any, Optional

HERE = Path(__file__).resolve().parent
if str(HERE) not in sys.path:
    sys.path.insert(0, str(HERE))
import simcore  # noqa: E402


SCHEMA_CONFIG = "dclutch-aquarium-config-v1"
SCHEMA_STATUS = "dclutch-aquarium-status-v1"
SCHEMA_JOURNAL = "dclutch-aquarium-epoch-journal-v1"
COHORT_SCHEMA = "dclutch-cohort-manifest-v1"
DEVNET_GENESIS = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
# PROVISIONAL operational caps. Their lifting plan is
# docs/operators/AQUARIUM_V1.md "Bounded epochs": measure child process count,
# storage, authoring time and browser list handling, then change these constants
# with their hostile bound tests and the document in one revision.
MAX_ACTIVE_MARKETS_HARD = 32
MAX_EPOCH_CYCLES_HARD = 256
MAX_WALLETS_HARD = 512
COHORT_ROLES = ("registry", "rent", "custody", "resolution", "claims", "trading", "core")
REPRODUCIBLE_GATE_SCHEMA = "dclutch-reproducible-release-gate-v1"
STATES = frozenset({"preflight", "running", "stopping", "stopped", "halted", "stale"})
HEX64 = re.compile(r"^[0-9a-f]{64}$")
HEX40 = re.compile(r"^[0-9a-f]{40}$")
ENVIRONMENT_NAME = re.compile(r"^[A-Z_][A-Z0-9_]*$")
BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


class Refusal(RuntimeError):
    """The configuration is not a safe, checkable aquarium plan."""


def whole(value: Any, field: str, *, maximum: Optional[int] = None) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise Refusal(f"{field} must be a non-negative whole number")
    if maximum is not None and value > maximum:
        raise Refusal(f"{field} exceeds its hard bound {maximum}")
    return value


def text(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value:
        raise Refusal(f"{field} must be a non-empty string")
    return value


def absolute(value: Any, field: str) -> Path:
    result = Path(text(value, field))
    if not result.is_absolute():
        raise Refusal(f"{field} must be an absolute path")
    return result


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def base58_decode(value: str) -> bytes:
    number = 0
    for character in value:
        index = BASE58.find(character)
        if index < 0:
            raise ValueError(character)
        number = number * 58 + index
    raw = number.to_bytes((number.bit_length() + 7) // 8, "big") if number else b""
    return b"\x00" * (len(value) - len(value.lstrip("1"))) + raw


def base58_encode(raw: bytes) -> str:
    zeros = len(raw) - len(raw.lstrip(b"\x00"))
    number = int.from_bytes(raw, "big")
    encoded = ""
    while number:
        number, remainder = divmod(number, 58)
        encoded = BASE58[remainder] + encoded
    return "1" * zeros + (encoded or ("" if zeros else "1"))


def address(value: Any, field: str) -> str:
    candidate = text(value, field)
    try:
        raw = base58_decode(candidate)
    except ValueError as error:
        raise Refusal(f"{field} must be a canonical Solana address") from error
    if len(raw) != 32 or base58_encode(raw) != candidate:
        raise Refusal(f"{field} must be a canonical Solana address")
    return candidate


def utc_after(seconds: float) -> str:
    return (dt.datetime.now(dt.timezone.utc) + dt.timedelta(seconds=seconds)).isoformat(
        timespec="seconds"
    )


def read_json(path: Path, field: str) -> dict:
    try:
        result = json.loads(path.read_text())
    except (OSError, ValueError) as error:
        raise Refusal(f"{field} cannot be read as JSON: {error}") from error
    if not isinstance(result, dict):
        raise Refusal(f"{field} must contain one JSON object")
    return result


def verify_release_gate(path: Path, digest: str) -> None:
    """Delegate every named-file check to the release gate's semantic owner.

    Checking that a JSON object merely *says* this commit is a release gate is
    the same self-authentication error as a manifest timestamp. The release
    verifier rehashes every link and refuses a missing, substituted, or
    diagnostic-bearing release input. It performs no RPC call and opens no key.
    """
    verifier = HERE.parent / "release" / "artifact_provenance.py"
    if not verifier.is_file():
        raise Refusal(f"release gate verifier is absent: {verifier}")
    child = subprocess.run(
        [sys.executable, str(verifier), "verify-reproducible-gate", "--root", str(path.parent),
         "--gate-sha256", digest],
        stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False,
    )
    if child.returncode != 0:
        detail = (child.stdout or b"").decode("utf-8", errors="replace").strip()
        raise Refusal(f"cohort.release_gate failed its release-owned reauthentication: {simcore.redact_text(detail[-800:])}")


def simulator_config(path: Path, market_address: str, maximum_spend: int) -> dict:
    """Validate only public/configuration facts; never read a keypair path."""
    body = read_json(path, "epoch simulator_config")
    if body.get("schema") != "dclutch-load-simulator-config-v1":
        raise Refusal(f"{path} is not a dclutch-load-simulator-config-v1")
    cluster = body.get("cluster") or {}
    if cluster.get("label") != "devnet" or cluster.get("devnet_genesis") != DEVNET_GENESIS:
        raise Refusal(f"{path} does not acknowledge Solana devnet")
    rpc_url = text(cluster.get("rpc_url"), f"{path} cluster.rpc_url")
    if not rpc_url.startswith("https://") or "mainnet" in rpc_url:
        raise Refusal(f"{path} must name credential-free HTTPS devnet, never mainnet")
    carried = simcore.endpoint_credential(rpc_url)
    if carried is not None:
        raise Refusal(f"{path} cluster.rpc_url carries {carried}; credentials never enter an aquarium config")
    if body.get("market_address") != market_address:
        raise Refusal(f"{path} names a different market than its aquarium inventory row")
    trade = body.get("trade") or {}
    if trade.get("mode") != "devnet":
        raise Refusal(f"{path} must use the current devnet Direct driver")
    budget = (body.get("budget") or {}).get("max_lamports_spent")
    spend = whole(budget, f"{path} budget.max_lamports_spent")
    if spend > maximum_spend:
        raise Refusal(f"{path} spends {spend}, above this aquarium's per-epoch ceiling")
    return body


def validate_config(path: Path) -> dict:
    body = read_json(path, "aquarium config")
    if body.get("schema") != SCHEMA_CONFIG:
        raise Refusal(f"config schema must be {SCHEMA_CONFIG}")
    work = absolute(body.get("work_dir"), "work_dir")
    public_status = absolute(body.get("public_status"), "public_status")
    if public_status.name != "aquarium-status-v1.json":
        raise Refusal("public_status must be named aquarium-status-v1.json")
    limits = body.get("limits")
    if not isinstance(limits, dict):
        raise Refusal("limits must be an object")
    max_active = whole(limits.get("max_active_markets"), "limits.max_active_markets",
                       maximum=MAX_ACTIVE_MARKETS_HARD)
    min_active = whole(limits.get("min_active_markets"), "limits.min_active_markets",
                       maximum=MAX_ACTIVE_MARKETS_HARD)
    if max_active < 1 or min_active < 1 or min_active > max_active:
        raise Refusal("limits require 1 <= min_active_markets <= max_active_markets")
    max_wallets = whole(limits.get("max_wallets"), "limits.max_wallets", maximum=MAX_WALLETS_HARD)
    max_spend = whole(limits.get("max_lamports_spent"), "limits.max_lamports_spent")
    if max_wallets < 1 or max_spend < 1:
        raise Refusal("limits.max_wallets and limits.max_lamports_spent must be positive")
    heartbeat = whole(limits.get("heartbeat_seconds", 90), "limits.heartbeat_seconds", maximum=3600)
    if heartbeat < 1:
        raise Refusal("limits.heartbeat_seconds must be positive")

    cohort_cfg = body.get("cohort")
    if not isinstance(cohort_cfg, dict):
        raise Refusal("cohort must be an object")
    manifest_path = absolute(cohort_cfg.get("manifest"), "cohort.manifest")
    manifest = read_json(manifest_path, "cohort.manifest")
    if manifest.get("schema") != COHORT_SCHEMA:
        raise Refusal("cohort.manifest has another schema")
    cohort_number = whole(manifest.get("cohort"), "cohort manifest cohort")
    if cohort_number < 1:
        raise Refusal("cohort manifest cohort must be positive")
    stated_manifest_digest = text(cohort_cfg.get("manifest_sha256"), "cohort.manifest_sha256")
    if not HEX64.fullmatch(stated_manifest_digest) or sha256_file(manifest_path) != stated_manifest_digest:
        raise Refusal("cohort.manifest_sha256 does not authenticate the current manifest bytes")
    deploy_commit = text(manifest.get("deploy_commit"), "cohort manifest deploy_commit")
    if not HEX40.fullmatch(deploy_commit) or cohort_cfg.get("deploy_commit") != deploy_commit:
        raise Refusal("cohort deploy_commit must be the manifest's exact lowercase commit")
    checked_at = text(cohort_cfg.get("checked_at"), "cohort.checked_at")
    try:
        dt.datetime.fromisoformat(checked_at.replace("Z", "+00:00"))
    except ValueError as error:
        raise Refusal("cohort.checked_at must be an ISO timestamp") from error
    roles = manifest.get("roles")
    if not isinstance(roles, list) or tuple(roles) != COHORT_ROLES:
        raise Refusal(f"cohort manifest roles must be the exact current ordered set {list(COHORT_ROLES)}")
    programs = manifest.get("programs") or {}
    if not isinstance(programs, dict) or set(programs) != set(COHORT_ROLES):
        raise Refusal("cohort manifest programs must contain exactly the current cohort roles")
    expected_programs = cohort_cfg.get("program_ids")
    if not isinstance(expected_programs, dict) or set(expected_programs) != set(COHORT_ROLES):
        raise Refusal("cohort.program_ids must repeat the checked manifest program ids")
    for role in COHORT_ROLES:
        actual = address(programs.get(role), f"cohort manifest programs.{role}")
        if expected_programs.get(role) != actual:
            raise Refusal(f"cohort.program_ids.{role} does not match the checked manifest")
    accelerator = manifest.get("general_accelerator")
    expected_accelerator = cohort_cfg.get("general_accelerator")
    if not isinstance(accelerator, dict) or not isinstance(expected_accelerator, dict):
        raise Refusal("cohort must carry and pin its separately deployed general_accelerator")
    accelerator_id = address(accelerator.get("program_id"), "cohort manifest general_accelerator.program_id")
    accelerator_slot_text = text(accelerator.get("deployment_slot"), "general_accelerator.deployment_slot")
    if not accelerator_slot_text.isdecimal():
        raise Refusal("general_accelerator.deployment_slot must be a canonical non-negative decimal")
    accelerator_slot = whole(int(accelerator_slot_text), "general_accelerator.deployment_slot")
    accelerator_elf = text(accelerator.get("elf_sha256"), "general_accelerator.elf_sha256")
    accelerator_semantic = text(accelerator.get("semantic_release_id"), "general_accelerator.semantic_release_id")
    if not HEX64.fullmatch(accelerator_elf) or not HEX64.fullmatch(accelerator_semantic):
        raise Refusal("general_accelerator ELF and semantic release digests must be lowercase SHA-256")
    expected_accelerator_body = {"program_id": accelerator_id, "deployment_slot": accelerator_slot,
                                 "elf_sha256": accelerator_elf, "semantic_release_id": accelerator_semantic}
    if expected_accelerator != expected_accelerator_body:
        raise Refusal("cohort.general_accelerator does not match the checked manifest")
    gate_cfg = cohort_cfg.get("release_gate")
    if not isinstance(gate_cfg, dict):
        raise Refusal("cohort.release_gate must bind the manifest to a reproducible release gate")
    gate_path = absolute(gate_cfg.get("path"), "cohort.release_gate.path")
    gate_digest = text(gate_cfg.get("sha256"), "cohort.release_gate.sha256")
    if gate_path.name != "RELEASE_GATE.json" or not HEX64.fullmatch(gate_digest) or sha256_file(gate_path) != gate_digest:
        raise Refusal("cohort.release_gate must name and hash the exact RELEASE_GATE.json")
    gate = read_json(gate_path, "cohort.release_gate")
    if gate.get("schema") != REPRODUCIBLE_GATE_SCHEMA or gate.get("source_revision") != deploy_commit:
        raise Refusal("cohort.release_gate does not authenticate this manifest's deploy commit")
    verify_release_gate(gate_path, gate_digest)
    prior_path = absolute(cohort_cfg.get("prior_manifest"), "cohort.prior_manifest")
    prior = read_json(prior_path, "cohort.prior_manifest")
    if prior.get("schema") != COHORT_SCHEMA:
        raise Refusal("cohort.prior_manifest has another schema")
    if whole(prior.get("cohort"), "prior cohort") >= cohort_number:
        raise Refusal("cohort.prior_manifest is not older than this cohort")
    for role in COHORT_ROLES:
        previous = (prior.get("programs") or {}).get(role)
        if previous and previous == programs[role]:
            raise Refusal(f"cohort {cohort_number} reused prior {role} program identity")

    actors = body.get("synthetic_actors")
    if not isinstance(actors, list) or not actors:
        raise Refusal("synthetic_actors must name the finite configured actor population")
    actor_addresses = [address(entry, f"synthetic_actors[{index}]") for index, entry in enumerate(actors)]
    if len(actor_addresses) != len(set(actor_addresses)) or len(actor_addresses) > max_wallets:
        raise Refusal("synthetic_actors must be unique and within limits.max_wallets")

    markets = body.get("markets")
    if not isinstance(markets, list) or not markets:
        raise Refusal("markets must be a non-empty inventory")
    if len(markets) > max_active:
        raise Refusal("markets exceeds limits.max_active_markets")
    seen_market_ids, seen_addresses, seen_ticket_digests, work_dirs = set(), set(), set(), set()
    parsed_markets = []
    manifest_markets = {str(entry.get("label")): entry for entry in (manifest.get("markets") or [])}
    for index, row in enumerate(markets):
        if not isinstance(row, dict):
            raise Refusal(f"markets[{index}] must be an object")
        market_id = text(row.get("market_id"), f"markets[{index}].market_id")
        market_address = address(row.get("address"), f"markets[{index}].address")
        source_label = text(row.get("source_market_label"), f"markets[{index}].source_market_label")
        if market_id in seen_market_ids or market_address in seen_addresses:
            raise Refusal("market_id and address are each unique in the active inventory")
        seen_market_ids.add(market_id); seen_addresses.add(market_address)
        source = manifest_markets.get(source_label)
        if source is None or source.get("kind") != "direct" or source.get("address") != market_address:
            raise Refusal(f"markets[{index}] is not a checked live Direct market from this manifest")
        # A public admission route may exist elsewhere, but the checked release
        # material accepted by this v1 schema has no published first-admission
        # linked-basis binding for this market. Configured actor keypair paths
        # are intentionally never read here and are never a visitor join route.
        if row.get("join_open", False) is not False:
            raise Refusal("join_open=true is unavailable until a checked release binding proves public first admission")
        epochs = row.get("epochs")
        if not isinstance(epochs, list) or not epochs:
            raise Refusal(f"markets[{index}].epochs must precommit at least one bounded epoch")
        parsed_epochs = []
        for epoch_index, epoch in enumerate(epochs):
            if not isinstance(epoch, dict):
                raise Refusal(f"markets[{index}].epochs[{epoch_index}] must be an object")
            epoch_id = text(epoch.get("epoch_id"), "epoch_id")
            cycles = whole(epoch.get("cycles"), f"epoch {epoch_id} cycles", maximum=MAX_EPOCH_CYCLES_HARD)
            if cycles < 1:
                raise Refusal(f"epoch {epoch_id} needs at least one cycle")
            sim_path = absolute(epoch.get("simulator_config"), f"epoch {epoch_id} simulator_config")
            sim = simulator_config(sim_path, market_address, max_spend)
            sim_work = absolute(sim.get("work_dir"), f"epoch {epoch_id} simulator work_dir")
            if str(sim_work) in work_dirs:
                raise Refusal("each epoch needs a distinct simulator work_dir; journals cannot be reused")
            work_dirs.add(str(sim_work))
            pairs = (((sim.get("trade") or {}).get("devnet") or {}).get("pairs") or [])
            if len(pairs) < cycles:
                raise Refusal(f"epoch {epoch_id} has {cycles} cycles but only {len(pairs)} pinned ticket pairs")
            for pair_index, pair in enumerate(pairs[:cycles]):
                if not isinstance(pair, dict):
                    raise Refusal(f"epoch {epoch_id} pair {pair_index} must be an object")
                seller, buyer = pair.get("seller_ticket_sha256"), pair.get("buyer_ticket_sha256")
                if not isinstance(seller, str) or not isinstance(buyer, str) or not HEX64.fullmatch(seller) or not HEX64.fullmatch(buyer):
                    raise Refusal(f"epoch {epoch_id} pair {pair_index} needs lowercase pinned ticket digests")
                for side, digest in (("seller", seller), ("buyer", buyer)):
                    if digest in seen_ticket_digests:
                        raise Refusal(f"a Direct {side} ticket digest appears in more than one aquarium cycle")
                    seen_ticket_digests.add(digest)
            parsed_epochs.append({"epoch_id": epoch_id, "cycles": cycles, "simulator_config": sim_path})
        parsed_markets.append({"market_id": market_id, "address": market_address, "epochs": parsed_epochs})
    if len(parsed_markets) < min_active:
        raise Refusal("inventory is below limits.min_active_markets")
    # Every child owns an independent payer-spend ledger. The aquarium owns
    # their sum: otherwise a row of individually bounded epochs is an
    # unbounded population at the supervisor layer.
    total_reserved_spend = sum(
        int((read_json(epoch["simulator_config"], "epoch simulator config").get("budget") or {})[
            "max_lamports_spent"
        ])
        for market in parsed_markets for epoch in market["epochs"]
    )
    if total_reserved_spend > max_spend:
        raise Refusal(
            f"precommitted child spend {total_reserved_spend} exceeds limits.max_lamports_spent {max_spend}"
        )
    return {"body": body, "work": work, "public_status": public_status, "limits": limits,
            "cohort": {"number": cohort_number, "digest": stated_manifest_digest,
                       "deploy_commit": deploy_commit, "checked_at": checked_at,
                       "release_gate_sha256": gate_digest, "general_accelerator": expected_accelerator_body},
            "actors": actor_addresses, "markets": parsed_markets,
            "reserved_lamports": total_reserved_spend,
            "ticket_digests": frozenset(seen_ticket_digests)}


class Aquarium:
    def __init__(self, config: dict, *, execute: bool):
        self.config, self.execute = config, execute
        self.work: Path = config["work"]
        self.public_status: Path = config["public_status"]
        self.started_at = simcore.utc_now_iso()
        self.stopping = False
        self.last_event: Optional[dict] = None
        self.counts = dict.fromkeys(("found", "admitted", "fill", "resolved", "deadline_failure",
                                     "redeemed", "retired", "census", "refused", "unattempted", "blocked"), 0)
        self.completed: set[tuple[str, str]] = set()
        self.observed_spend = 0

    def install_signals(self) -> None:
        def stop(_signum, _frame):
            self.stopping = True
        signal.signal(signal.SIGTERM, stop)
        signal.signal(signal.SIGINT, stop)

    def journal_path(self, market_id: str, epoch_id: str) -> Path:
        return self.work / "epochs" / market_id / epoch_id / "journal.json"

    def plan(self, market: dict, epoch: dict) -> dict:
        return {"market_id": market["market_id"], "market_address": market["address"],
                "epoch_id": epoch["epoch_id"], "cycles": epoch["cycles"],
                "simulator_config_sha256": sha256_file(epoch["simulator_config"]),
                "mode": "execute" if self.execute else "preflight"}

    def record(self, market: dict, epoch: dict, phase: str, **extra: Any) -> dict:
        path = self.journal_path(market["market_id"], epoch["epoch_id"])
        path.parent.mkdir(parents=True, exist_ok=True)
        plan = self.plan(market, epoch)
        existing = read_json(path, "epoch journal") if path.exists() else None
        if existing is not None and existing.get("plan_digest") != simcore.digest_of(plan):
            raise simcore.JournalConflict(f"{path} describes another aquarium epoch plan")
        body = {"schema": SCHEMA_JOURNAL, "phase": phase, "plan": plan,
                "plan_digest": simcore.digest_of(plan), "recorded_at": simcore.utc_now_iso()}
        body.update(extra)
        simcore.write_json_atomic(path, body)
        return body

    def market_rows(self) -> list:
        rows = []
        for market in self.config["markets"]:
            epochs = market["epochs"]
            done = sum((market["market_id"], epoch["epoch_id"]) in self.completed for epoch in epochs)
            rows.append({"market_id": market["market_id"], "address": market["address"],
                         "state": "activity-running" if self.last_event and self.last_event.get("market_id") == market["market_id"] else "active",
                         "join_open": False,
                         "observed_at": self.last_event.get("at") if self.last_event and self.last_event.get("market_id") == market["market_id"] else None,
                         "epochs_completed": done, "epochs_precommitted": len(epochs)})
        return rows

    def status(self, state: str, failure: Optional[str] = None) -> dict:
        if state not in STATES:
            raise Refusal(f"unknown aquarium state {state}")
        heartbeat = float(self.config["limits"].get("heartbeat_seconds", 90))
        body = {"schema": SCHEMA_STATUS,
                "cohort": {"number": self.config["cohort"]["number"], "manifest_sha256": self.config["cohort"]["digest"],
                           "deployment_commit": self.config["cohort"]["deploy_commit"], "checked_at": self.config["cohort"]["checked_at"],
                           "release_gate_sha256": self.config["cohort"]["release_gate_sha256"],
                           "general_accelerator": self.config["cohort"]["general_accelerator"]},
                "state": state,
                "run": {"started_at": self.started_at, "updated_at": simcore.utc_now_iso(),
                        "expected_next_update_by": None if state in ("stopped", "halted") else utc_after(heartbeat),
                        "planned_market_count": len(self.config["markets"]),
                        "active_market_target": self.config["limits"]["min_active_markets"],
                        "joined_wallet_target": len(self.config["actors"]),
                        "max_wallets": self.config["limits"]["max_wallets"],
                        "max_lamports_spent": self.config["limits"]["max_lamports_spent"],
                        "lamports_spent_observed": self.observed_spend},
                "activity": {"synthetic_actors": True, "last_event_at": None if self.last_event is None else self.last_event["at"],
                             "last_event_kind": None if self.last_event is None else self.last_event["kind"],
                             "counts": self.counts, "active_markets": self.market_rows(),
                             "join_note": "Configured synthetic actors only. A public noncustodial admission entrance is not yet delivered."},
                "limits": {"max_active_markets": self.config["limits"]["max_active_markets"]},
                "artifacts": {"driver": "tools/load-simulator/simulator.py",
                              "epoch_journal_schema": SCHEMA_JOURNAL,
                              "child_journal_owner": "dclutch-load-simulator-cycle-v1"},
                "failure": None if failure is None else {"kind": "driver-exit", "at": simcore.utc_now_iso(),
                                                          "detail": simcore.redact_text(failure)}}
        return body

    def publish(self, state: str, failure: Optional[str] = None) -> None:
        self.public_status.parent.mkdir(parents=True, exist_ok=True)
        simcore.write_json_atomic(self.public_status, self.status(state, failure))

    @staticmethod
    def child_status(epoch: dict) -> Optional[dict]:
        try:
            cfg = read_json(epoch["simulator_config"], "epoch simulator config")
            return read_json(Path(cfg["work_dir"]) / "status.json", "child status")
        except Refusal:
            return None

    def run_epoch(self, market: dict, epoch: dict) -> int:
        journal_path = self.journal_path(market["market_id"], epoch["epoch_id"])
        if journal_path.exists():
            existing = read_json(journal_path, "epoch journal")
            plan = self.plan(market, epoch)
            if existing.get("plan_digest") != simcore.digest_of(plan):
                raise simcore.JournalConflict(f"{journal_path} describes another aquarium epoch plan")
            if existing.get("phase") == "finalized":
                self.completed.add((market["market_id"], epoch["epoch_id"]))
                return 0
        self.record(market, epoch, "executing")
        command = [sys.executable, str(HERE / "simulator.py"), "run", "--config",
                   str(epoch["simulator_config"]), "--cycles", str(epoch["cycles"])]
        if self.execute:
            command.append("--execute")
        # No shell; the driver owns its private, signed journals. The command is
        # stored credential-free as a fixed argv shape, never as an operator string.
        log = journal_path.parent / "driver.log"
        proc = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, text=True)
        while proc.poll() is None:
            self.last_event = {"at": simcore.utc_now_iso(), "kind": "driver-running", "market_id": market["market_id"]}
            self.publish("stopping" if self.stopping else "running")
            if self.stopping:
                proc.terminate()
            time.sleep(0.2)
        if proc.stdout is None:
            output = ""
        else:
            output = proc.stdout.read()
            proc.stdout.close()
        log.write_text(output)
        child = self.child_status(epoch)
        child_spend = ((child or {}).get("spend") or {}).get("spent_lamports")
        if isinstance(child_spend, int) and not isinstance(child_spend, bool) and child_spend >= 0:
            self.observed_spend += child_spend
        event_kind = "fill" if (child or {}).get("trades", {}).get("landed", 0) else "census"
        self.counts[event_kind] += 1
        self.last_event = {"at": simcore.utc_now_iso(), "kind": event_kind, "market_id": market["market_id"]}
        if proc.returncode == 0:
            self.record(market, epoch, "finalized", child_exit=0)
            self.completed.add((market["market_id"], epoch["epoch_id"]))
            return 0
        self.counts["refused"] += 1
        self.record(market, epoch, "halted", child_exit=proc.returncode,
                    detail=simcore.redact_text(output[-1000:]))
        return proc.returncode

    def run(self) -> int:
        self.work.mkdir(parents=True, exist_ok=True)
        os.chmod(self.work, 0o700)
        self.install_signals()
        self.publish("preflight" if not self.execute else "running")
        for market in self.config["markets"]:
            for epoch in market["epochs"]:
                if self.stopping:
                    self.publish("stopped")
                    return 0
                code = self.run_epoch(market, epoch)
                if code != 0:
                    self.publish("halted", f"current driver exited {code}; its journal and log are retained")
                    return code
                # Preflight has only one meaningful driver invocation; it must
                # never burn through an entire future activity queue.
                if not self.execute:
                    self.publish("stopped")
                    return 0
        self.publish("stopped")
        return 0


# ---------------------------------------------------------------------------
# Epoch preparation: offline ticket authoring, then review before admission.

SCHEMA_PREPARE = "dclutch-aquarium-epoch-preparation-v1"


def decimal(value: Any, field: str, *, maximum: Optional[int] = None) -> int:
    raw = text(value, field)
    if not raw.isdecimal():
        raise Refusal(f"{field} must be a canonical non-negative decimal")
    return whole(int(raw), field, maximum=maximum)


def ticket_author(value: Any, field: str, market: str, side: str) -> dict:
    if not isinstance(value, dict):
        raise Refusal(f"{field} must be an object")
    env = text(value.get("keypair_env"), f"{field}.keypair_env")
    if not ENVIRONMENT_NAME.fullmatch(env):
        raise Refusal(f"{field}.keypair_env must name one uppercase environment variable")
    lifecycle = text(value.get("lifecycle"), f"{field}.lifecycle")
    if lifecycle != "fok":
        raise Refusal(f"{field}.lifecycle must be fok for a bounded aquarium epoch")
    return {
        "keypair_env": env,
        "maker": address(value.get("maker"), f"{field}.maker"),
        "market": market,
        "side": side,
        "lifecycle": lifecycle,
        "outcome": decimal(value.get("outcome"), f"{field}.outcome", maximum=(1 << 32) - 1),
        "generation": decimal(value.get("generation"), f"{field}.generation"),
        "nonce": decimal(value.get("nonce"), f"{field}.nonce"),
        "valid_from": decimal(value.get("valid_from"), f"{field}.valid_from"),
        "valid_through": decimal(value.get("valid_through"), f"{field}.valid_through"),
        "maximum_fill": decimal(value.get("maximum_fill"), f"{field}.maximum_fill"),
        "limit_price": decimal(value.get("limit_price"), f"{field}.limit_price"),
        "fee_basis_points": decimal(value.get("fee_basis_points"), f"{field}.fee_basis_points", maximum=10_000),
        "collateral_account": address(value.get("collateral_account"), f"{field}.collateral_account"),
    }


def prepare_spec(path: Path) -> dict:
    body = read_json(path, "epoch preparation")
    if body.get("schema") != SCHEMA_PREPARE:
        raise Refusal(f"epoch preparation schema must be {SCHEMA_PREPARE}")
    aquarium_path = absolute(body.get("aquarium_config"), "epoch preparation aquarium_config")
    aquarium = validate_config(aquarium_path)
    market_id = text(body.get("market_id"), "epoch preparation market_id")
    market = next((row for row in aquarium["markets"] if row["market_id"] == market_id), None)
    if market is None:
        raise Refusal("epoch preparation market_id is not in the checked active inventory")
    epoch_id = text(body.get("epoch_id"), "epoch preparation epoch_id")
    if any(epoch["epoch_id"] == epoch_id for epoch in market["epochs"]):
        raise Refusal("epoch preparation epoch_id already exists; use a new epoch identity")
    cycles = whole(body.get("cycles"), "epoch preparation cycles", maximum=MAX_EPOCH_CYCLES_HARD)
    if cycles < 1:
        raise Refusal("epoch preparation cycles must be positive")
    output_dir = absolute(body.get("output_dir"), "epoch preparation output_dir")
    simulator_work = absolute(body.get("simulator_work_dir"), "epoch preparation simulator_work_dir")
    if output_dir.exists() and not (output_dir / "prepared-epoch.json").is_file():
        raise Refusal("epoch preparation output_dir already exists without a sealed preparation; inspect it, then use a new epoch identity")
    if simulator_work.exists():
        raise Refusal("epoch preparation simulator_work_dir already exists; a fresh epoch may not bypass an old driver journal")
    occupied = {
        str(read_json(epoch["simulator_config"], "existing epoch simulator config")["work_dir"])
        for row in aquarium["markets"] for epoch in row["epochs"]
    }
    if str(simulator_work) in occupied:
        raise Refusal("epoch preparation simulator_work_dir reuses an existing signed-journal root")
    template_path = absolute(body.get("simulator_template"), "epoch preparation simulator_template")
    template = simulator_config(template_path, market["address"], aquarium["limits"]["max_lamports_spent"])
    candidate_spend = whole((template.get("budget") or {}).get("max_lamports_spent"),
                            "epoch preparation template budget.max_lamports_spent")
    cumulative_spend = aquarium["reserved_lamports"] + candidate_spend
    if cumulative_spend > aquarium["limits"]["max_lamports_spent"]:
        raise Refusal("epoch preparation would exceed limits.max_lamports_spent after existing reserved epochs")
    bootstrap = absolute(template.get("bootstrap_bin"), "epoch preparation bootstrap_bin")
    if not os.access(bootstrap, os.X_OK):
        raise Refusal("epoch preparation bootstrap_bin is not executable")
    pairs = body.get("ticket_pairs")
    if not isinstance(pairs, list) or len(pairs) != cycles:
        raise Refusal("epoch preparation needs exactly one explicit ticket pair per cycle")
    parsed_pairs = []
    for index, pair in enumerate(pairs):
        if not isinstance(pair, dict):
            raise Refusal(f"ticket_pairs[{index}] must be an object")
        seller = ticket_author(pair.get("seller"), f"ticket_pairs[{index}].seller", market["address"], "sell")
        buyer = ticket_author(pair.get("buyer"), f"ticket_pairs[{index}].buyer", market["address"], "buy")
        for field in ("outcome", "generation", "valid_from", "valid_through", "maximum_fill", "limit_price", "fee_basis_points"):
            if seller[field] != buyer[field]:
                raise Refusal(f"ticket_pairs[{index}] seller and buyer disagree on {field}")
        if seller["maker"] == buyer["maker"]:
            raise Refusal(f"ticket_pairs[{index}] needs distinct seller and buyer makers")
        if seller["valid_from"] > seller["valid_through"]:
            raise Refusal(f"ticket_pairs[{index}] has an empty validity interval")
        parsed_pairs.append({"seller": seller, "buyer": buyer})
    return {"aquarium": aquarium, "market": market, "epoch_id": epoch_id, "cycles": cycles,
            "output_dir": output_dir, "simulator_work": simulator_work, "template": template,
            "template_path": template_path, "bootstrap": bootstrap, "pairs": parsed_pairs,
            "reserved_lamports_before": aquarium["reserved_lamports"],
            "candidate_lamports": candidate_spend, "reserved_lamports_after": cumulative_spend,
            "existing_ticket_digests": aquarium["ticket_digests"]}


def preparation_plan(spec: dict) -> dict:
    """The exact offline signing plan; key paths and values are absent by design."""
    return {
        "market_id": spec["market"]["market_id"], "market_address": spec["market"]["address"],
        "epoch_id": spec["epoch_id"], "cycles": spec["cycles"],
        "template_sha256": sha256_file(spec["template_path"]),
        "reserved_lamports_before": spec["reserved_lamports_before"],
        "candidate_lamports": spec["candidate_lamports"],
        "reserved_lamports_after": spec["reserved_lamports_after"],
        "simulator_work_dir": str(spec["simulator_work"]),
        "pairs": [{role: {key: value for key, value in ticket.items() if key != "keypair_env"}
                   for role, ticket in pair.items()} for pair in spec["pairs"]],
    }


def ticket_command(bootstrap: Path, ticket: dict, output: Path) -> list[str]:
    return [str(bootstrap), "direct-intent-ticket-author-v1", "--keypair-env", ticket["keypair_env"],
            "--maker", ticket["maker"], "--market", ticket["market"], "--side", ticket["side"],
            "--lifecycle", ticket["lifecycle"], "--outcome", str(ticket["outcome"]),
            "--generation", str(ticket["generation"]), "--nonce", str(ticket["nonce"]),
            "--valid-from", str(ticket["valid_from"]), "--valid-through", str(ticket["valid_through"]),
            "--maximum-fill", str(ticket["maximum_fill"]), "--limit-price", str(ticket["limit_price"]),
            "--fee-basis-points", str(ticket["fee_basis_points"]), "--collateral-account", ticket["collateral_account"],
            "--out", str(output)]


def author_ticket(bootstrap: Path, ticket: dict, output: Path, receipt: Path) -> str:
    """Author one portable ticket. Explicit caller configuration names its env.

    The supervisor does not inspect the environment entry or its key-file path;
    the single shared ticket author consumes that named variable. Nothing here
    opens a socket, builds a transaction, or submits a packet.
    """
    if output.exists() or receipt.exists():
        raise Refusal(
            f"ticket output {output} already exists; ticket authoring is one-shot and an "
            "interrupted preparation must be inspected before a fresh epoch is named"
        )
    argv = ticket_command(bootstrap, ticket, output)
    child = subprocess.run(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                           stderr=subprocess.STDOUT, check=False)
    transcript = (child.stdout or b"").decode("utf-8", errors="replace")
    if child.returncode != 0:
        raise Refusal(f"ticket author refused {ticket['side']} ticket: {simcore.redact_text(transcript[-800:])}")
    if not output.is_file():
        raise Refusal(f"ticket author succeeded without writing {output}")
    simcore.write_atomic(receipt, transcript.encode("utf-8"))
    return sha256_file(output)


def cmd_prepare_epoch(args: argparse.Namespace) -> int:
    spec = prepare_spec(Path(args.spec))
    plan = preparation_plan(spec)
    if not args.author:
        print(json.dumps({"schema": SCHEMA_PREPARE, "phase": "preflight", "plan": plan,
                          "plan_digest": simcore.digest_of(plan)}, indent=2, sort_keys=True))
        return 0
    output = spec["output_dir"]
    prepared = output / "prepared-epoch.json"
    if prepared.exists():
        existing = read_json(prepared, "prepared epoch")
        if existing.get("plan_digest") != simcore.digest_of(plan):
            raise simcore.JournalConflict("prepared epoch describes another ticket-authoring plan")
        print(f"prepared epoch already sealed: {prepared}")
        return 0
    output.mkdir(parents=True, exist_ok=False)
    simcore.write_json_atomic(output / "prepare-journal.json", {"schema": SCHEMA_PREPARE, "phase": "authoring",
        "plan": plan, "plan_digest": simcore.digest_of(plan), "recorded_at": simcore.utc_now_iso()})
    generated_pairs = []
    occupied_digests = set(spec["existing_ticket_digests"])
    for index, pair in enumerate(spec["pairs"]):
        pair_dir = output / f"pair-{index:03d}"
        pair_dir.mkdir()
        seller = author_ticket(spec["bootstrap"], pair["seller"], pair_dir / "seller.json", pair_dir / "seller.receipt.json")
        buyer = author_ticket(spec["bootstrap"], pair["buyer"], pair_dir / "buyer.json", pair_dir / "buyer.receipt.json")
        for side, digest in (("seller", seller), ("buyer", buyer)):
            if digest in occupied_digests:
                raise Refusal(f"newly authored {side} ticket duplicates a ticket in the checked aquarium plan")
            occupied_digests.add(digest)
        generated_pairs.append({"seller_ticket": str(pair_dir / "seller.json"), "seller_ticket_sha256": seller,
                                "buyer_ticket": str(pair_dir / "buyer.json"), "buyer_ticket_sha256": buyer})
    simulator = json.loads(json.dumps(spec["template"]))
    simulator["work_dir"] = str(spec["simulator_work"])
    simulator["trade"]["devnet"]["pairs"] = generated_pairs
    simulator_path = output / "simulator-config.json"
    simcore.write_json_atomic(simulator_path, simulator)
    result = {"schema": SCHEMA_PREPARE, "phase": "prepared", "plan": plan,
              "plan_digest": simcore.digest_of(plan), "prepared_at": simcore.utc_now_iso(),
              "epoch": {"epoch_id": spec["epoch_id"], "cycles": spec["cycles"],
                        "simulator_config": str(simulator_path)}, "ticket_pairs": generated_pairs}
    simcore.write_json_atomic(prepared, result)
    simcore.write_json_atomic(output / "prepare-journal.json", result)
    print(json.dumps({"prepared_epoch": str(prepared), "epoch": result["epoch"]}, indent=2, sort_keys=True))
    return 0


def cmd_check(args: argparse.Namespace) -> int:
    config = validate_config(Path(args.config))
    print(f"aquarium: cohort {config['cohort']['number']} checked; {len(config['markets'])} active Direct markets, "
          f"{sum(len(m['epochs']) for m in config['markets'])} bounded epochs")
    return 0


def cmd_run(args: argparse.Namespace) -> int:
    return Aquarium(validate_config(Path(args.config)), execute=args.execute).run()


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    for name in ("check", "run"):
        child = sub.add_parser(name)
        child.add_argument("--config", required=True)
        if name == "run":
            child.add_argument("--execute", action="store_true")
    prepare = sub.add_parser("prepare-epoch", help="preflight or explicitly author one bounded replacement epoch")
    prepare.add_argument("--spec", required=True)
    prepare.add_argument("--author", action="store_true",
                         help="authorize local portable-ticket signatures; submits no transaction")
    args = parser.parse_args(argv)
    try:
        if args.command == "check":
            return cmd_check(args)
        if args.command == "prepare-epoch":
            return cmd_prepare_epoch(args)
        return cmd_run(args)
    except (Refusal, simcore.JournalConflict, OSError, ValueError, KeyError) as error:
        print(f"REFUSED: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
