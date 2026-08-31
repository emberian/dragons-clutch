#!/usr/bin/env python3
"""Drive a simlife world against a real substrate, and write down what happened.

`simlife.py` is cluster-free on purpose: it draws worlds and knows nothing about
a validator.  This is the other half -- the layer that owns a work directory, a
config, child processes and a chain -- and it is deliberately thin, because
every mutation this project performs belongs to a driver that owns its own
signed journal.  Same doctrine as `simulator.py`, one market widened to many.

    python3 simlife_drive.py plan   --config C [--out world.json]
    python3 simlife_drive.py run    --config C [--execute]
    python3 simlife_drive.py routes

TWO SUBSTRATES, and a config chooses.

  `ledger-census`  observation and nothing else, against markets that already
           exist.  It signs nothing, so every mutation in the world is
           `unattempted` with the driver that would perform it named.  This was
           the only honest substrate while local founding refused `0x5182`.

  `lifecycle`  every route the successor bootstrap owns a driver for, DRIVEN --
           founding, admission, fills, resolution, the failure walk,
           redemption, retirement, and the census over all of it.  Each one is a
           subprocess calling the shipped driver that owns the route; nothing
           here builds a transaction, derives a PDA or copies a constructor.
           That is the FOUND-5182 lesson, where a hand-written copy of a kernel
           constructor drifted three bytes and walled every local founding for a
           day.  See `simlife_drivers.py`.

Compaction is the one route with no driver anywhere -- covered and green in
ProgramTest, named by no CLI and no gauntlet binding -- so it stays
`unattempted` with that sentence and a SIZE rather than becoming this module's
first hand-built mutation.

A run therefore produces: whatever the world's markets actually did, as their
own drivers reported it; real observations of every market at every tick through
the same conservation ledger; and, for every event that did not execute, which
of the three other words applies and why.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import os
from pathlib import Path
import random
import subprocess
import sys
from typing import Optional

HERE = Path(__file__).resolve().parent
if str(HERE) not in sys.path:
    sys.path.insert(0, str(HERE))
import simcore  # noqa: E402
import simlife  # noqa: E402
import simlife_drivers  # noqa: E402

SCHEMA_CONFIG = "dclutch-simlife-config-v1"
CHILD_TIMEOUT_SECONDS = 600

# Which driver owns each route. Verified against the successor bootstrap's own
# dispatch (tools/local-validator/bootstrap/successor/src/main.rs:109-268), so a
# reader can go and run one by hand. This table is DOCUMENTATION, not dispatch:
# nothing here composes a command line for a driver this module has not
# executed.
ROUTE_DRIVERS = {
    simlife.ROUTE_FOUND: (
        "local-private-validator-market-v1 compiles the market input against a LIVE "
        "loopback deployment -- at a caller-chosen band, collateral and terminal window "
        "since the shape widening -- and then the founding leg DCLTGMF3 opens it. The "
        "0x5182 wall in front of this is gone (a7e2f668) and so is the readiness suffix "
        "behind it (9941a4e4). DRIVEN HERE"
    ),
    simlife.ROUTE_ADMIT: (
        "local-private-validator-user-position-admission-v1 --plan --campaign-evidence "
        "--position-owner --fee-payer --minimum-finalized-slot --routing-table --execute. "
        "The packet does not fit a legacy message and routes through the founding's OWN "
        "frozen DCLTGMF3 table. DRIVEN HERE"
    ),
    simlife.ROUTE_FILL: (
        "local-private-validator-direct-trade-produce-v1 then "
        "local-private-validator-direct-trade-v1 --session --execute, one invocation per "
        "durable mutation. Needs a market founded WITH the Direct capability entry"
    ),
    simlife.ROUTE_RESOLVE: (
        "local-private-validator-flagship-resolution-v1, three modes "
        "(--produce-input, --provision-tables, executor --through submit|execute|"
        "reclaim|complete), then the Core terminal admission"
    ),
    simlife.ROUTE_DEADLINE_FAILURE: (
        "local-private-validator-sponsored-push-v1 --action commit-failure --execute. "
        "The bare relay RelayActionV1::CommitDeadlineFailure has no driver in the "
        "successor binary; sponsored-push is the one CLI path to that frame"
    ),
    simlife.ROUTE_REDEEM: (
        "local-private-validator-wallet-terminal-payout-input-v1 then "
        "local-private-validator-wallet-terminal-payout-v1 --execute"
    ),
    simlife.ROUTE_COMPACT: (
        "NO CLI EXISTS. Claim-check compaction by a stranger is implemented and green "
        "in ProgramTest only -- programs/dclutch-claims-sbf/tests/claim_check/mod.rs, "
        "including `a_market_retires_a_sleeping_holders_position_and_the_holder_is_"
        "still_paid` -- and no gauntlet binding names it, so it is covered and "
        "census-unbound"
    ),
    simlife.ROUTE_RETIRE: (
        "local-private-validator-aggregate-retirement-v1, a four-packet journaled "
        "campaign, --execute"
    ),
    simlife.ROUTE_CENSUS: (
        "ledger-census --mint --payer --hoard --aggregate --claim-unit-atoms --stage "
        "--output [--prior], read-only, nonzero exit on any violated law. DRIVEN HERE."
    ),
}


class Refusal(RuntimeError):
    pass


# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------


def load_config(path: Path) -> dict:
    body = json.loads(path.read_text())
    if body.get("schema") != SCHEMA_CONFIG:
        raise Refusal(f"config schema must be {SCHEMA_CONFIG}")
    cluster = body.get("cluster") or {}
    label = cluster.get("label")
    rpc = cluster.get("rpc_url", "")
    if label == "local":
        if not rpc.startswith("http://127.0.0.1"):
            raise Refusal("local rpc_url must be a literal loopback origin")
    elif label == "devnet":
        # Reads only, and this module has no write route to offer devnet even if
        # it wanted one. The genesis acknowledgment is still required, because
        # naming the cluster you are reading is the cheap half of not writing to
        # the wrong one.
        if not rpc.startswith("https://"):
            raise Refusal("devnet rpc_url must be https")
        if cluster.get("devnet_genesis") != "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG":
            raise Refusal("a devnet config must acknowledge the genesis hash in full")
    else:
        raise Refusal("cluster.label must be local or devnet")
    if "mainnet" in rpc:
        raise Refusal("mainnet is refused unconditionally")
    for key in ("bootstrap_bin", "work_dir"):
        if not str(body.get(key, "")).startswith("/"):
            raise Refusal(f"{key} must be an absolute path")
    if not os.access(body["bootstrap_bin"], os.X_OK):
        raise Refusal(f"bootstrap_bin is not executable: {body['bootstrap_bin']}")
    world = body.get("world") or {}
    if not isinstance(world.get("seed"), str) or not world["seed"]:
        raise Refusal("world.seed must be a non-empty sentence; a run must be nameable")
    mix = world.get("archetype_mix", "design-space")
    if mix not in simlife.ARCHETYPE_MIXES:
        raise Refusal(
            f"world.archetype_mix must be one of {sorted(simlife.ARCHETYPE_MIXES)}, got {mix!r}"
        )
    bindings = body.get("bindings") or {}
    if not isinstance(bindings, dict):
        raise Refusal("bindings must be an object keyed by planned market id")
    for market_id, binding in bindings.items():
        for field in ("mint", "payer", "hoard", "aggregate", "claim_unit_atoms", "outcome_count"):
            if field not in binding:
                raise Refusal(f"binding {market_id} is missing {field}")
    return body


def check_bindings(config: dict, world: simlife.World) -> None:
    """A binding must name a planned market, and must agree with it on WIDTH.

    This is the one join in the whole pipeline where a caption could come apart
    from its chart.  The series artifact carries, on the same object, the
    archetype the generator DREW and the outcome count the census OBSERVED.
    Bind a real two-cell market to a planned eleven-cell `wide-field` and the
    page draws two bars under a caption promising eleven -- or worse, lays one
    market's cells under another market's names.

    So the config states the observed width and it is checked against the plan
    before a single census runs.  Changing the seed until a planned market of
    the right width comes up is the correct fix; filing the observation under
    the wrong archetype is not.
    """
    planned = {market.market_id: market for market in world.markets}
    for market_id, binding in (config.get("bindings") or {}).items():
        market = planned.get(market_id)
        if market is None:
            raise Refusal(
                f"binding {market_id} names no planned market in this world "
                f"(it plans {', '.join(sorted(planned))})"
            )
        stated_basis = binding.get("basis")
        if stated_basis is not None and stated_basis != market.basis:
            raise Refusal(
                f"binding {market_id} says the market on the chain has a {stated_basis} basis "
                f"and the plan drew a {market.archetype} with {market.basis}. A categorical "
                "market filed under a ramp archetype would be captioned with a payout shape "
                "it does not have"
            )
        stated = int(binding["outcome_count"])
        if stated != market.outcome_count:
            raise Refusal(
                f"binding {market_id} says the market on the chain has {stated} outcomes and "
                f"the plan drew a {market.archetype} with {market.outcome_count}. Filing that "
                "observation under this archetype would put one market's cells under another "
                "market's name; change the seed until a planned market of the right width "
                "comes up"
            )


def world_spec_from_config(config: dict) -> simlife.WorldSpec:
    world = config["world"]
    return simlife.WorldSpec(
        seed=simlife.SeedBook(preimage=world["seed"]),
        markets=int(world.get("markets", 8)),
        ticks=int(world.get("ticks", 24)),
        archetype_mix=simlife.ARCHETYPE_MIXES[world.get("archetype_mix", "design-space")],
        slots_per_tick=int(world.get("slots_per_tick", 120)),
    )


# ---------------------------------------------------------------------------
# The census substrate: the one route this module actually drives
# ---------------------------------------------------------------------------


def run_child(argv: list, log_path: Path, timeout: float = CHILD_TIMEOUT_SECONDS):
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


@dataclasses.dataclass
class MarketCensus:
    """One market's census chain. Per market, because the conservation ledger is
    per market: one Hoard, one aggregate, one Mint, and a delta law that reads
    exactly one predecessor. Two markets sharing a chain would compare market A's
    Hoard against market B's, and L2 would be arithmetic about nothing."""

    market_id: str
    directory: Path
    retention: simcore.CensusRetention
    prior: Optional[Path] = None
    taken: int = 0
    last_report: Optional[dict] = None

    def adopt_existing(self) -> None:
        files = simcore.CensusRetention.series_files(self.directory)
        if files:
            self.prior = files[-1]
            self.taken = int(files[-1].stem.split("-")[-1])


class LedgerCensusSubstrate(simlife.Substrate):
    """Real observation of real accounts, through the accepted census driver.

    ROUTES: `{census}` and nothing else. This is not modesty -- it is the whole
    substrate story of 2026-08-30. Founding refuses at `0x5182`, so no market
    this world plans can be created, so no admission, fill, resolution,
    redemption, compaction or retirement has anything to act on. What CAN be
    done is what `simulator.py` already does for one market: watch a market that
    exists and check every conservation law over it, tick after tick. This does
    that for as many markets as the config binds.
    """

    name = "ledger-census"
    routes = frozenset({simlife.ROUTE_CENSUS})
    # NONE. A census READS whatever basis a founded market already has and
    # EXPRESSES none, because it founds nothing.
    #
    # The first draft declared every kind here, reasoning that the basis gate
    # only guards founding and this substrate has no founding route for it to
    # guard. That is true and it published the wrong sentence: the artifact then
    # said `basis_kinds_absent: []`, which a reader would take to mean this
    # substrate can found a ramp market. It cannot found anything. Declaring
    # none says so, and the route check runs first regardless, so every founding
    # is still `unattempted` for the route reason rather than the basis one.
    basis_kinds: frozenset = frozenset()

    def __init__(self, config: dict, work: Path, *, execute: bool):
        self.config = config
        self.work = work
        # NOT `self.execute`: that is the Substrate method this class overrides,
        # and a flag by the same name silently replaces the route with a boolean.
        self.executing = execute
        self.cluster = config["cluster"]["label"]
        self.rpc_url = config["cluster"]["rpc_url"]
        self.rpc_origin = simcore.redact_endpoint(self.rpc_url)
        self.source_revision = config.get("source_revision")
        self.label = config.get("substrate_label") or (
            f"a {self.cluster} cluster observed read-only at {self.rpc_origin}"
        )
        self.bindings = config.get("bindings") or {}
        # Everything the config binds already exists on this chain. This run
        # founds nothing, so the founding events stay `unattempted` -- and the
        # observations of those markets are real, which is what `pre_founded` is
        # for.
        self.pre_founded = frozenset(self.bindings)
        retention_cfg = config.get("census_retention") or {}
        self.retention = simcore.CensusRetention(
            window=int(retention_cfg.get("window", simcore.DEFAULT_CENSUS_WINDOW)),
            keep_files=int(retention_cfg.get("keep_files", simcore.DEFAULT_CENSUS_KEEP_FILES)),
        )
        self.chains: dict = {}

    def why_not(self, route: str) -> str:
        driver = ROUTE_DRIVERS.get(route, "no driver is recorded for this route")
        if route == simlife.ROUTE_FOUND:
            return (
                "this run founded nothing: the markets it observes existed on this chain "
                f"before it started. The founding driver is: {driver}"
            )
        return (
            f"this substrate observes and signs nothing, so {route} was not attempted. "
            f"Its driver is: {driver}"
        )

    def chain(self, market_id: str) -> MarketCensus:
        existing = self.chains.get(market_id)
        if existing is not None:
            return existing
        chain = MarketCensus(
            market_id=market_id,
            directory=self.work / "census" / market_id,
            retention=self.retention,
        )
        chain.directory.mkdir(parents=True, exist_ok=True)
        chain.adopt_existing()
        self.chains[market_id] = chain
        return chain

    def cluster_args(self) -> list:
        args = ["--rpc-url", self.rpc_url]
        if self.cluster == "devnet":
            args += ["--i-mean-devnet", "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"]
        return args

    def execute(self, event, market) -> simlife.EventResult:
        if event.route != simlife.ROUTE_CENSUS:
            raise Refusal(f"the census substrate was asked for {event.route}")
        binding = self.bindings.get(event.market_id)
        if binding is None:
            return simlife.EventResult(
                outcome=simlife.OUTCOME_UNATTEMPTED,
                detail=(
                    f"{event.market_id} is a planned market with nothing bound to it. An "
                    "observation of a market that was never founded would be invented, so "
                    "none was taken"
                ),
            )
        chain = self.chain(event.market_id)
        index = chain.taken + 1
        out = chain.directory / f"cycle-{index:06d}.json"
        stage = f"simlife-{event.market_id}-tick-{event.tick:04d}"
        argv = [self.config["bootstrap_bin"], "ledger-census", *self.cluster_args()]
        argv += [
            "--mint", binding["mint"],
            "--payer", binding["payer"],
            "--hoard", binding["hoard"],
            "--aggregate", binding["aggregate"],
            "--claim-unit-atoms", str(binding["claim_unit_atoms"]),
            "--stage", stage,
            "--output", str(out),
        ]
        for label, pubkey in (binding.get("tokens") or {}).items():
            argv += ["--token", f"{label}={pubkey}"]
        for label, pubkey in (binding.get("positions") or {}).items():
            argv += ["--position", f"{label}={pubkey}"]
        for label, pubkey in (binding.get("watch") or {}).items():
            argv += ["--watch", f"{label}={pubkey}"]
        if chain.prior is not None:
            argv += ["--prior", str(chain.prior)]
        if not self.executing:
            return simlife.EventResult(
                outcome=simlife.OUTCOME_UNATTEMPTED,
                detail=(
                    "preflight: the census command was composed and not run. "
                    f"{simcore.redact_command(argv)}"
                ),
            )
        log = self.work / "logs" / f"census-{event.market_id}-{index:06d}.log"
        proc = run_child(argv, log)
        if proc.returncode != 0:
            text = (proc.stdout or b"").decode("utf-8", errors="replace")
            if simcore.looks_like_backpressure(text):
                raise Backpressure(f"census {event.market_id} tick {event.tick}")
            # A census exits nonzero on a violated conservation law, and that is
            # a fact about the LEDGER rather than about this process. It halts
            # the run loudly, the same way simulator.py does, and the halt file
            # refuses a restart until a human clears it.
            simcore.halt_loudly(
                self.work,
                f"ledger-census violated a conservation law on {event.market_id} at tick {event.tick}",
                {
                    "exit_code": proc.returncode,
                    "log": str(log),
                    "command": simcore.redact_command(argv),
                },
            )
        chain.prior = out
        chain.taken = index
        chain.last_report = chain.retention.apply(chain.directory)
        observations = json.loads(out.read_text())
        newest = observations[-1] if isinstance(observations, list) and observations else {}
        return simlife.EventResult(
            outcome=simlife.OUTCOME_EXECUTED,
            detail=f"observed {event.market_id} at slot {newest.get('slot')} as {stage}",
            observation={
                "stage": stage,
                "slot": newest.get("slot"),
                "file": str(out),
                "verdicts": [
                    {"law": v.get("law"), "status": v.get("status")}
                    for v in (newest.get("verdicts") or [])
                ],
            },
        )


class Backpressure(RuntimeError):
    pass


# ---------------------------------------------------------------------------
# The substrate that MUTATES: every route with a shipped driver, driven
# ---------------------------------------------------------------------------


class LifecycleSubstrate(LedgerCensusSubstrate):
    """A world with hands: every route the successor bootstrap owns a driver for.

    `LedgerCensusSubstrate` above observes and signs nothing, which was the
    honest shape while founding refused `0x5182`. That wall is gone
    (`a7e2f668`), the readiness suffix behind it is gone (`9941a4e4`), and the
    local founding compiler now takes the band, the collateral and the terminal
    window as arguments instead of emitting one constant market -- so a world
    can create its own markets, admit its own participants, and be driven down
    the rest of its own schedule.

    ONE RULE, and it is the whole design: every route below calls the SHIPPED
    driver that owns it, through `simlife_drivers.py`, and records whatever that
    driver says. Nothing here builds a transaction. A driver that refuses is
    `refused` with its own first line, which is a MEASUREMENT of this substrate;
    a route with no driver anywhere is `unattempted` with the sentence saying
    so. Compaction is the only one of those, and it is sized rather than
    hand-built, because a compaction this module wrote by hand would be the
    FOUND-5182 mirror wearing a new name.
    """

    name = "successor-bootstrap-lifecycle"
    # Everything but compaction. `routes` is a CLAIM, checked by the conductor
    # before any event is attempted, so a route listed here that turns out to
    # refuse is recorded as a refusal rather than as an absence -- which is the
    # difference between "this chain said no" and "nobody ever wrote the code".
    routes = frozenset(set(simlife.ALL_ROUTES) - {simlife.ROUTE_COMPACT})
    # What this substrate can EXPRESS at founding, and it is one kind.
    # `compile_linked_basis_v3` hard-wires `kind: CategoricalQ1`, and founding
    # refuses any other, so a `ladder` or a `tent-band` is unfoundable here and
    # says so at its own founding rather than being quietly redrawn as a
    # categorical market wearing a ramp's name.
    basis_kinds = frozenset({simlife.BASIS_CATEGORICAL})

    def __init__(self, config: dict, work: Path, *, execute: bool):
        super().__init__(config, work, execute=execute)
        lifecycle = config.get("lifecycle") or {}
        for key in ("campaign_payer_keypair", "founding_founder", "substituted_founder"):
            if not lifecycle.get(key):
                raise Refusal(
                    f"a lifecycle substrate needs lifecycle.{key}: the founding roles it creates "
                    "are protocol identities and this module will not invent one"
                )
        self.drivers = simlife_drivers.DriverContext(
            bootstrap_bin=config["bootstrap_bin"],
            rpc_url=self.rpc_url,
            plan=lifecycle["plan"] if lifecycle.get("plan") else config["plan"],
            work=work,
            timeout=float(lifecycle.get("driver_timeout_seconds", 1800.0)),
            campaign_payer_keypair=lifecycle["campaign_payer_keypair"],
            founding_founder=lifecycle["founding_founder"],
            substituted_founder=lifecycle["substituted_founder"],
            solana_keygen=lifecycle.get("solana_keygen"),
            substrate_keys=lifecycle.get("substrate_keys"),
        )
        self.pyth_facts = lifecycle.get("pyth_facts")
        self.wallet_lamports = int(lifecycle.get("participant_lamports", 2_000_000_000))
        self.extra_tokens = lifecycle.get("extra_tokens") or {}
        self._prior: dict = {}
        self.founded: dict = {}
        # One wallet per participant id, world-wide. World-wide because cranks
        # and compactors are drawn from the WORLD rather than from a market --
        # the point of a permissionless step is that a stranger can take it --
        # so `p03/2` may retire a market they hold nothing in.
        self.wallets: dict = {}
        self.terminal_receipts: dict = {}

    def describe(self) -> dict:
        body = super().describe()
        body["founding_shape"] = (
            "local-private-validator-market-v1 at a caller-chosen band, collateral and terminal "
            "window; the claim unit stays 1 because compile_linked_basis_v3 hard-wires it"
        )
        return body

    def why_not(self, route: str) -> str:
        if route == simlife.ROUTE_COMPACT:
            return simlife_drivers.COMPACTION_ABSENT
        return super().why_not(route)

    # -- wallets ----------------------------------------------------------

    def wallet(self, participant_id: str) -> Path:
        """One funded local wallet per participant.

        An admission is over a wallet, and a wallet that has never been paid is
        not one: the driver refuses `snapshot missing required account` before
        it compiles anything. So this creates the key and pays it from the
        loopback faucet, which is a validator affordance and not a protocol
        transaction.
        """
        existing = self.wallets.get(participant_id)
        if existing is not None:
            return existing
        path = self.work / "wallets" / f"{participant_id.replace('/', '_')}.json"
        simlife_drivers.new_keypair(self.drivers, path)
        self._fund(simlife_drivers.keypair_pubkey(path))
        self.wallets[participant_id] = path
        return path

    def _fund(self, pubkey: str) -> None:
        """Pay one local wallet from the loopback faucet, and wait for FINALIZED.

        Not confirmed: every driver here reads finalized state, and a wallet
        that is only confirmed is one it cannot see.
        """
        if (simlife_drivers.rpc(
            self.rpc_url, "getBalance", [pubkey, {"commitment": "finalized"}]
        ) or {}).get("value"):
            return
        simlife_drivers.rpc(self.rpc_url, "requestAirdrop", [pubkey, self.wallet_lamports])
        self._await_finalized(pubkey)

    def _await_finalized(self, pubkey: str, attempts: int = 40) -> None:
        import time

        for _ in range(attempts):
            value = simlife_drivers.rpc(
                self.rpc_url, "getBalance", [pubkey, {"commitment": "finalized"}]
            )
            if (value or {}).get("value"):
                return
            time.sleep(1.0)
        raise simlife_drivers.DriverRefusal(
            f"{pubkey} never became visible at finalized commitment after an airdrop"
        )

    # -- dispatch ---------------------------------------------------------

    def execute(self, event, market) -> simlife.EventResult:
        if event.route == simlife.ROUTE_CENSUS:
            return super().execute(event, market)
        if not self.executing:
            return simlife.EventResult(
                outcome=simlife.OUTCOME_UNATTEMPTED,
                detail=(
                    f"preflight: {event.route} was not attempted because this run was not asked "
                    "to execute. Its driver is: " + ROUTE_DRIVERS[event.route]
                ),
            )
        handler = {
            simlife.ROUTE_FOUND: self._found,
            simlife.ROUTE_ADMIT: self._admit,
            simlife.ROUTE_FILL: self._fill,
            simlife.ROUTE_RESOLVE: self._resolve,
            simlife.ROUTE_DEADLINE_FAILURE: self._deadline_failure,
            simlife.ROUTE_REDEEM: self._redeem,
            simlife.ROUTE_RETIRE: self._retire,
        }.get(event.route)
        if handler is None:
            raise Refusal(f"the lifecycle substrate was asked for {event.route}")
        try:
            return handler(event, market)
        except simlife_drivers.DriverRefusal as refusal:
            # THE ROUTE EXISTS AND THE CHAIN SAID NO. That is a reading about
            # this substrate and it is kept in the driver's own words.
            return simlife.EventResult(outcome=simlife.OUTCOME_REFUSED, detail=str(refusal))
        except subprocess.TimeoutExpired:
            return simlife.EventResult(
                outcome=simlife.OUTCOME_REFUSED,
                detail=(
                    f"{event.route} for {event.subject} exceeded "
                    f"{self.drivers.timeout:.0f}s and was abandoned rather than left racing the "
                    "next event; its driver's journal is on disk and a rerun resumes it"
                ),
            )

    # -- routes -----------------------------------------------------------

    def _found(self, event, market) -> simlife.EventResult:
        founded, run = simlife_drivers.drive_founding(self.drivers, event.market_id, market)
        # The fixture source's OWNER has to exist as an account before it can
        # authorize a transfer out of it, and the founding never pays it: the
        # admission driver refuses "collateral snapshot omitted collateral
        # source owner" over a wallet nobody has funded, exactly as it refuses
        # an unfunded position owner.
        self._fund(simlife_drivers.keypair_pubkey(founded.keys / "participant.json"))
        self.founded[event.market_id] = founded
        # The census learns about the market from the FOUNDING's own evidence,
        # so a market this run created is observed with exactly the accounts the
        # chain gave it rather than with anything a config typed by hand.
        self.bindings[event.market_id] = self._binding(founded)
        # `run is None` means this work directory had already completed the
        # founding and it was adopted rather than walked again. Said out loud,
        # because "executed" over an adopted founding is a claim about this work
        # directory's whole history rather than about the last ten minutes.
        how = "opened" if run is not None else "adopted the completed founding of"
        return simlife.EventResult(
            outcome=simlife.OUTCOME_EXECUTED,
            detail=(
                f"{how} {founded.address} with {founded.outcome_count} outcomes over cuts "
                f"{market.cuts} and {market.founding_collateral_atoms} collateral atoms; frozen "
                f"routing table {founded.routing_table}"
            ),
            observation={"market": founded.address, "outcomes": founded.outcome_count},
            signatures=[] if run is None else self._signatures(run),
        )

    def _binding(self, founded) -> dict:
        """A market's census binding, plus any account this CHAIN already held.

        `extra_tokens` names collateral accounts that exist because somebody
        moved atoms by hand -- a probe, an earlier lane -- outside this run.
        They are still this market's collateral and L1 counts them, so a run
        over a chain with a history has to be told about them or every census
        of that market fails for a true reason about a stale caption.
        """
        binding = founded.census_binding()
        for label, address in (self.extra_tokens.get(founded.market_id) or {}).items():
            binding["tokens"][label] = address
        for index, address in enumerate(self._prior_tokens(founded)):
            binding["tokens"][f"prior_{index:02d}"] = address
        return binding

    def _prior_tokens(self, founded) -> list:
        """Collateral accounts this CHAIN already held for a market, named once.

        Read at the moment the market is bound and then frozen. A rehearsal
        chain has a history and a run that refused to acknowledge it would halt
        on its first census over somebody else's leftovers instead of on its
        own mistake -- but reading it CONTINUOUSLY would make L1 an identity,
        so it is read exactly once per market and never again.
        """
        known = self._prior.get(founded.market_id)
        if known is not None:
            return known
        named = {
            founded.hoard, founded.founder_wallet, founded.participant_fixture_source,
            *(self.extra_tokens.get(founded.market_id) or {}).values(),
        }
        found = [
            address
            for address in simlife_drivers.collateral_accounts_for_mint(
                self.rpc_url, founded.mint
            )
            if address not in named
        ]
        self._prior[founded.market_id] = found
        return found

    def _market_or_refuse(self, market_id: str):
        founded = self.founded.get(market_id)
        if founded is None:
            raise simlife_drivers.DriverRefusal(
                f"{market_id} was never founded by this run, so there is nothing on chain to act on"
            )
        return founded

    def _admit(self, event, market) -> simlife.EventResult:
        founded = self._market_or_refuse(event.market_id)
        wallet = self.wallet(event.subject)
        run = simlife_drivers.drive_admission(
            self.drivers, founded, event.subject, wallet,
            int(event.detail.get("stake_atoms", 0)),
            collateral_atoms=simlife_drivers.fixture_share_atoms(founded),
        )
        # REBIND, because the admission just created a token account holding
        # this market's collateral. `census_binding()` returns a fresh document
        # each call, so a binding taken at founding time is a snapshot: leaving
        # it there is how a run gets "N atoms are in accounts this ledger does
        # not name" at its very next census, which is L1 being right about a
        # stale caption.
        self.bindings[event.market_id] = self._binding(founded)
        return simlife.EventResult(
            outcome=simlife.OUTCOME_EXECUTED,
            detail=self._finalized_line(run, f"admitted {event.subject} to {founded.address}"),
            signatures=self._signatures(run),
        )

    def _fill(self, event, market) -> simlife.EventResult:
        founded = self._market_or_refuse(event.market_id)
        maker = event.detail.get("maker")
        report = founded.admissions.get(maker) or next(iter(founded.admissions.values()), None)
        if report is None:
            raise simlife_drivers.DriverRefusal(
                f"no participant of {event.market_id} has been admitted, and a Direct trade is "
                "between two admitted positions"
            )
        run = simlife_drivers.drive_fill(self.drivers, founded, event.subject, report)
        return simlife.EventResult(
            outcome=simlife.OUTCOME_EXECUTED,
            detail=self._finalized_line(run, f"filled {event.subject} on {founded.address}"),
            signatures=self._signatures(run),
        )

    def _resolve(self, event, market) -> simlife.EventResult:
        founded = self._market_or_refuse(event.market_id)
        # The resolution's three signers are PROTOCOL identities the founding
        # created and nobody ever paid: the table provisioner sends from the
        # founding founder and the chain answers "Attempt to debit an account
        # but found no record of a prior credit". Same species as the position
        # owner and the fixture source owner, and funded the same way.
        for role in ("founding-founder", "resolver"):
            try:
                self._fund(simlife_drivers.keypair_pubkey(
                    simlife_drivers._substrate_key(self.drivers, role)
                ))
            except simlife_drivers.DriverRefusal:
                pass
        run = simlife_drivers.drive_resolution(self.drivers, founded, self.pyth_facts)
        return simlife.EventResult(
            outcome=simlife.OUTCOME_EXECUTED,
            detail=self._finalized_line(run, f"resolved {founded.address}"),
            signatures=self._signatures(run),
        )

    def _deadline_failure(self, event, market) -> simlife.EventResult:
        founded = self._market_or_refuse(event.market_id)
        driver = event.detail.get("driven_by") or event.market_id
        run = simlife_drivers.drive_deadline_failure(
            self.drivers, founded, self.wallet(driver)
        )
        return simlife.EventResult(
            outcome=simlife.OUTCOME_EXECUTED,
            detail=self._finalized_line(run, f"committed the deadline failure on {founded.address}"),
            signatures=self._signatures(run),
        )

    def _redeem(self, event, market) -> simlife.EventResult:
        founded = self._market_or_refuse(event.market_id)
        run = simlife_drivers.drive_redemption(
            self.drivers, founded, event.subject, self.wallet(event.subject),
            claim_index=int(market.selected_cell or 0),
        )
        return simlife.EventResult(
            outcome=simlife.OUTCOME_EXECUTED,
            detail=self._finalized_line(run, f"paid out {event.subject}"),
            signatures=self._signatures(run),
        )

    def _retire(self, event, market) -> simlife.EventResult:
        founded = self._market_or_refuse(event.market_id)
        run = simlife_drivers.drive_retirement(
            self.drivers, founded, self.terminal_receipts.get(event.market_id)
        )
        return simlife.EventResult(
            outcome=simlife.OUTCOME_EXECUTED,
            detail=self._finalized_line(run, f"retired {founded.address}"),
            signatures=self._signatures(run),
        )

    # -- reading a driver's own report ------------------------------------

    @staticmethod
    def _signatures(run) -> list:
        """Signatures the DRIVER reported, scraped from its own output.

        Not derived and not predicted: a signature this module computed would be
        a claim about a transaction rather than a record of one.
        """
        found: list = []
        for line in run.output.splitlines():
            marker = "signature"
            lowered = line.lower()
            if marker not in lowered:
                continue
            for token in line.replace('"', " ").replace(",", " ").replace(":", " ").split():
                if 80 <= len(token) <= 92 and token.isalnum():
                    if token not in found:
                        found.append(token)
        return found[:8]

    @staticmethod
    def _finalized_line(run, prefix: str) -> str:
        for line in run.output.splitlines():
            if "slot" in line and "finalized" in line.lower():
                return f"{prefix}: {line.strip()[:400]}"
        return prefix


# ---------------------------------------------------------------------------
# The run
# ---------------------------------------------------------------------------


SUBSTRATES = {
    "ledger-census": LedgerCensusSubstrate,
    "lifecycle": LifecycleSubstrate,
}


def build_substrate(config: dict, work: Path, *, execute: bool):
    """Which substrate this run drives, stated by the config rather than implied.

    The default is `ledger-census`, the read-only one. A run that MUTATES a
    chain should have said so in a file somebody reviewed, not acquired the
    ability by upgrading a module.
    """
    name = config.get("substrate", "ledger-census")
    factory = SUBSTRATES.get(name)
    if factory is None:
        raise Refusal(f"substrate must be one of {sorted(SUBSTRATES)}, got {name!r}")
    return factory(config, work, execute=execute)


def write_world(work: Path, world: simlife.World) -> dict:
    body = world.body()
    simcore.write_json_atomic(work / "world.json", body)
    return body


def status_writer(config: dict, work: Path, world: simlife.World) -> simcore.StatusWriter:
    cadence = config.get("cadence") or {}
    return simcore.StatusWriter(
        path=work / "status.json",
        cluster_label=config["cluster"]["label"],
        rpc_url=config["cluster"]["rpc_url"],
        mode="finite",
        market_address=None,
        cadence_seconds=float(cadence.get("period_seconds", 0.0)),
        jitter_fraction=float(cadence.get("jitter_fraction", 0.25)),
        grace_seconds=float(cadence.get("grace_seconds", 300.0)),
    )


def cmd_plan(args: argparse.Namespace) -> int:
    config = load_config(Path(args.config))
    world = simlife.build_world(world_spec_from_config(config))
    check_bindings(config, world)
    body = world.body()
    if args.out:
        simcore.write_json_atomic(Path(args.out), body)
        print(f"simlife: wrote {args.out} ({len(json.dumps(body))} bytes)")
    for line in simlife.world_summary(world):
        print(line)
    print()
    for market in world.markets:
        print(simlife.market_line(market))
    print()
    print(f"plan digest {body['plan_digest'][:16]}")
    return 0


def cmd_routes(_args: argparse.Namespace) -> int:
    print("route                what drives it")
    print("-" * 78)
    for route in simlife.ALL_ROUTES:
        print(f"{route:<20} {ROUTE_DRIVERS[route]}")
    return 0


def cmd_run(args: argparse.Namespace) -> int:
    config = load_config(Path(args.config))
    work = Path(config["work_dir"])
    work.mkdir(parents=True, exist_ok=True)
    os.chmod(work, 0o700)
    simcore.refuse_if_halted(work)
    simcore.clear_exit_record(work)

    disk = simcore.DiskFloor(
        floor_bytes=int((config.get("census_retention") or {}).get(
            "disk_floor_bytes", simcore.DEFAULT_DISK_FLOOR_BYTES
        ))
    )
    low = disk.check(work)
    if low is not None:
        print(f"REFUSED: {low}", file=sys.stderr)
        simcore.record_exit(work, simcore.EXIT_LOW_DISK, detail=low, exit_code=4)
        return 4

    world = simlife.build_world(world_spec_from_config(config))
    check_bindings(config, world)
    plan = write_world(work, world)
    substrate = build_substrate(config, work, execute=args.execute)
    status = status_writer(config, work, world)

    # CADENCE, and it is not decoration. A tick is meant to be a boundary a
    # market could have moved across, and a census of one market takes under a
    # second: without a pause, a sixty-tick run covers about two hundred slots
    # and every market in it holds still because no time passed. The period is
    # what makes `world.slots_per_tick` a measurable claim rather than a wish --
    # and the run reports the slots it ACTUALLY advanced per tick beside the
    # number the plan assumed.
    cadence = config.get("cadence") or {}
    stop = simcore.StopFlag()
    stop.install()
    rate = simcore.RateController(
        period_seconds=float(cadence.get("period_seconds", 0.0)),
        jitter_fraction=float(cadence.get("jitter_fraction", 0.25)),
        # Jitter is seeded from the run's own seed, so even the pauses are
        # reproducible and two runs of one world keep the same cadence.
        rng=random.Random(world.spec.seed.digest),
    )

    def pace(tick: int, walked: simlife.Conductor) -> None:
        # One line per tick, FLUSHED. A cadenced run is minutes long and its
        # stdout is a file somebody is tailing; without a flush the whole run is
        # silent until it ends, which is indistinguishable from a hang.
        observed = sum(
            1 for entry in walked.entries
            if entry.event.tick == tick and entry.result.outcome == simlife.OUTCOME_EXECUTED
        )
        slots = sorted(
            entry.result.observation["slot"] for entry in walked.entries
            if entry.event.tick == tick and entry.result.observation is not None
            and entry.result.observation.get("slot") is not None
        )
        print(
            f"tick {tick:>4}/{world.spec.ticks}: {observed} observed"
            + (f", slot {slots[-1]}" if slots else ""),
            flush=True,
        )
        if rate.period_seconds > 0.0:
            stop.sleep_interruptibly(rate.next_delay())

    outcome, detail, code = simcore.EXIT_CRASHED, None, 1
    conductor = simlife.Conductor(
        world, substrate, on_tick=pace, should_stop=lambda: stop.requested,
    )
    try:
        ledger = conductor.run()
        if stop.requested:
            outcome, code = simcore.EXIT_SIGNALLED, 0
            detail = (
                f"stopped on {stop.signal_name} between ticks at tick "
                f"{conductor.stopped_at_tick}; every census taken is sealed"
            )
        else:
            outcome, code = simcore.EXIT_COMPLETED, 0
            detail = f"walked {len(conductor.entries)} planned events"
        return 0
    except simcore.Halt as halt:
        outcome, code, detail = simcore.EXIT_HALTED, 3, str(halt)
        ledger = conductor.ledger()
        print(f"HALTED: {halt}", file=sys.stderr)
        return 3
    except Backpressure as pressure:
        # Not a halt and not a crash: the endpoint asked this run to slow down
        # and it has nothing to slow down TO -- a world walk is finite and its
        # cadence is the conductor's, not a rate controller's. So it stops,
        # says so, and leaves every census file it took; rerunning adopts them
        # (`MarketCensus.adopt_existing`) and continues from the next index.
        outcome, code = simcore.EXIT_SIGNALLED, 5
        detail = f"the endpoint applied backpressure at {pressure}; stopped rather than hammered"
        ledger = conductor.ledger()
        print(f"STOPPED: {detail}", file=sys.stderr)
        return 5
    except BaseException as error:  # noqa: BLE001 - recorded, then re-raised
        outcome = simcore.EXIT_CRASHED
        detail = f"{type(error).__name__}: {error}"
        ledger = conductor.ledger()
        raise
    finally:
        simcore.write_json_atomic(work / "ledger.json", ledger)
        status.write(
            cycles_run=len({e.event.tick for e in conductor.entries}),
            cycles_target=world.spec.ticks,
            trades_landed=0,
            signatures=[
                signature
                for entry in conductor.entries
                for signature in entry.result.signatures
            ],
            wallets=[],
            last_reconciliation={
                "ok": outcome != simcore.EXIT_HALTED,
                "checked_at": simcore.utc_now_iso(),
                "markets_observed": sorted(substrate.chains),
            },
            halted=outcome == simcore.EXIT_HALTED,
            halt_reason=detail if outcome == simcore.EXIT_HALTED else None,
            stopping=False,
            extra={
                "trades_attempted": False,
                "simlife": {
                    "plan_digest": plan["plan_digest"],
                    "seed": world.spec.seed.describe(),
                    "markets_planned": len(world.markets),
                    "markets_bound": len(substrate.bindings),
                    "tally": conductor.tally(),
                    "stopped_at_tick": conductor.stopped_at_tick,
                    "planned_slots_per_tick": world.spec.slots_per_tick,
                    "substrate": substrate.describe(),
                },
                "census_retention": {
                    market_id: chain.last_report
                    for market_id, chain in substrate.chains.items()
                },
                "measured_pace": measured_pace(substrate),
            },
        )
        simcore.record_exit(work, outcome, detail=detail, exit_code=code)
        _print_tally(conductor)
        for market_id, pace_row in measured_pace(substrate).items():
            print(
                f"pace: {market_id} advanced {pace_row['slots_advanced']} slots across "
                f"ticks {pace_row['first_tick']}..{pace_row['last_tick']} = "
                f"{pace_row['measured_slots_per_tick']} slots/tick measured, against "
                f"{world.spec.slots_per_tick} the plan assumed"
            )


def measured_pace(substrate: "LedgerCensusSubstrate") -> dict:
    """Slots the chain actually advanced per tick, per market, MEASURED.

    The plan carries `slots_per_tick`, which is an operator's claim about how
    much chain a tick covers; a market's deadline is compared against it to
    decide whether the market reaches a terminal boundary inside the run. A
    claim that is never checked against the chain is exactly the kind of number
    this project does not publish, so the run reads its own census chain back
    and reports what the tick was actually worth.
    """
    report: dict = {}
    for market_id, chain in substrate.chains.items():
        files = simcore.CensusRetention.series_files(chain.directory)
        if not files:
            continue
        try:
            observations = json.loads(files[-1].read_text())
        except (OSError, ValueError):
            continue
        ticks, slots = [], []
        for observation in observations:
            stage = str(observation.get("stage", ""))
            marker = "-tick-"
            if marker not in stage:
                continue
            ticks.append(int(stage.rsplit(marker, 1)[1]))
            slots.append(int(observation.get("slot", 0)))
        if len(ticks) < 2 or ticks[-1] == ticks[0]:
            continue
        report[market_id] = {
            "observations": len(ticks),
            "first_tick": ticks[0],
            "last_tick": ticks[-1],
            "first_slot": slots[0],
            "last_slot": slots[-1],
            "slots_advanced": slots[-1] - slots[0],
            # Integer division: this is a measurement a reader will compare
            # against the plan's own integer, and a float would invite a
            # precision argument about a number that is a count of slots.
            "measured_slots_per_tick": (slots[-1] - slots[0]) // (ticks[-1] - ticks[0]),
        }
    return report


def _print_tally(conductor: simlife.Conductor) -> None:
    print()
    print("route                executed  refused  unattempted  blocked")
    print("-" * 62)
    for route in simlife.ALL_ROUTES:
        row = conductor.tally().get(route)
        if row is None:
            continue
        print(
            f"{route:<20} {row[simlife.OUTCOME_EXECUTED]:>8} "
            f"{row[simlife.OUTCOME_REFUSED]:>8} "
            f"{row[simlife.OUTCOME_UNATTEMPTED]:>12} "
            f"{row[simlife.OUTCOME_BLOCKED]:>8}"
        )


def main(argv: Optional[list] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)

    plan_p = sub.add_parser("plan", help="draw a world and print it; touches no cluster")
    plan_p.add_argument("--config", required=True)
    plan_p.add_argument("--out", help="write the plan document here")

    run_p = sub.add_parser("run", help="walk a world against the configured substrate")
    run_p.add_argument("--config", required=True)
    run_p.add_argument("--execute", action="store_true",
                       help="actually invoke the census driver (default composes and does not run)")

    sub.add_parser("routes", help="which driver owns each route, and what blocks it")

    args = parser.parse_args(argv)
    try:
        if args.command == "plan":
            return cmd_plan(args)
        if args.command == "routes":
            return cmd_routes(args)
        return cmd_run(args)
    except (Refusal, simlife.Refusal, simcore.Halt) as refusal:
        print(f"REFUSED: {refusal}", file=sys.stderr)
        return 2
    except (OSError, ValueError, KeyError) as defect:
        print(f"REFUSED: {type(defect).__name__}: {defect}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
