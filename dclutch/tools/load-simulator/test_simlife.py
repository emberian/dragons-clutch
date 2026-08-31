#!/usr/bin/env python3
"""Tests for the simlife behavior engine and its driver layer.

Two things are being proven and they are different.

THE ENGINE is pure, so it is tested by ASSERTION rather than by execution: the
same seed draws the same world byte for byte, independent streams stay
independent when the world around them changes, an exact total stays exact
through a Dirichlet split, and the schedule's ordering is total.

THE DRIVER LAYER is tested against a FAKE bootstrap binary that honours the real
`ledger-census` contract -- cumulative observation array, nonzero exit on a
violated law -- exactly as `test_simulator.py` does.  That proves the conductor's
state machine, the halt discipline, the per-market census chains and the
preflight refusal without a validator.

Run: `python3 test_simlife.py`
"""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import simcore  # noqa: E402
import simlife  # noqa: E402

SPEC = importlib.util.spec_from_file_location("dclutch_simlife_drive", HERE / "simlife_drive.py")
assert SPEC is not None and SPEC.loader is not None
drive = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = drive
SPEC.loader.exec_module(drive)


FAKE_BOOT = r"""#!/usr/bin/env bash
# Fake successor bootstrap. Honours the ledger-census contract: `--prior` is
# reloaded, the new observation appended, and the whole chain re-serialized, so
# the newest file is the whole series exactly as the real driver leaves it.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
cmd="${1:-}"; shift || true
output=""; prior=""; stage=""; aggregate=""
while [ "$#" -gt 0 ]; do case "$1" in
  --output) output="$2"; shift 2 ;;
  --prior) prior="$2"; shift 2 ;;
  --stage) stage="$2"; shift 2 ;;
  --aggregate) aggregate="$2"; shift 2 ;;
  *) shift ;;
esac; done
case "$cmd" in
  ledger-census)
    if [ -f "$here/census-violation" ]; then
      echo "conservation law violated: hoard delta -3" >&2; exit 4
    fi
    if [ -f "$here/backpressure" ]; then
      echo "HTTP 429 Too Many Requests" >&2; exit 1
    fi
    DCLUTCH_PRIOR="$prior" DCLUTCH_STAGE="$stage" DCLUTCH_OUTPUT="$output" \
    DCLUTCH_AGG="$aggregate" python3 - <<'PY'
import json, os
prior, stage = os.environ["DCLUTCH_PRIOR"], os.environ["DCLUTCH_STAGE"]
output, aggregate = os.environ["DCLUTCH_OUTPUT"], os.environ["DCLUTCH_AGG"]
series = json.load(open(prior)) if prior else []
series.append({
    "stage": stage,
    "slot": 1000 + len(series) * 7,
    "aggregate": aggregate,
    "aggregate_supply": [500, 500],
    "hoard_atoms": 1000,
    "tracked_collateral": 1000,
    "mint_supply": 1000,
    "payer_lamports": 999,
    "outcome_count": 2,
    "position_balances": {},
    "position_totals": [0, 0],
    "token_atoms": {"hoard": 1000},
    "accounts": {},
    "verdicts": [
        {"law": "L1", "status": "holds", "detail": "tracked == mint supply"},
        {"law": "L4", "status": "holds", "detail": "hoard covers the worst outcome"},
    ],
})
open(output, "w").write(json.dumps(series, indent=2))
PY
    ;;
  *) echo "fake bootstrap does not implement $cmd" >&2; exit 64 ;;
esac
"""


def write_fake_boot(directory: Path) -> Path:
    path = directory / "fake-bootstrap"
    path.write_text(FAKE_BOOT)
    path.chmod(path.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)
    return path


# ---------------------------------------------------------------------------


class SeedTests(unittest.TestCase):
    def test_streams_are_independent_across_domains(self):
        book = simlife.SeedBook("a run worth naming")
        first = [book.stream("market", 3).random() for _ in range(3)]
        second = [book.stream("market", 3).random() for _ in range(3)]
        self.assertEqual(first, second, "the same named stream must replay identically")
        self.assertNotEqual(first, [book.stream("market", 4).random() for _ in range(3)])

    def test_domain_separator_prevents_collisions(self):
        """`("market", 12)` and `("market1", 2)` must not be the same stream."""
        book = simlife.SeedBook("separator")
        self.assertNotEqual(
            book.stream("market", 12).random(),
            book.stream("market1", 2).random(),
        )

    def test_digest_is_of_the_preimage(self):
        book = simlife.SeedBook("x")
        import hashlib
        self.assertEqual(book.digest, hashlib.sha256(b"x").hexdigest())


class DistributionTests(unittest.TestCase):
    def test_log_int_uniform_reaches_both_ends(self):
        book = simlife.SeedBook("log")
        dist = simlife.LogIntUniform(100, 100_000)
        drawn = [dist.draw(book.stream("d", i)) for i in range(400)]
        self.assertTrue(all(100 <= v <= 100_000 for v in drawn))
        # The point of a log draw is that the SMALL end is not vanishingly
        # rare. A linear draw over this range would put under 1% below 1000.
        small = len([v for v in drawn if v < 1_000])
        self.assertGreater(small, 40, "a log-uniform must reach its small end")

    def test_categorical_respects_weights(self):
        book = simlife.SeedBook("cat")
        dist = simlife.Categorical((("a", 9), ("b", 1)))
        drawn = [dist.draw(book.stream("c", i)) for i in range(400)]
        self.assertGreater(drawn.count("a"), drawn.count("b") * 3)

    def test_categorical_refuses_a_zero_weight(self):
        with self.assertRaises(simlife.Refusal):
            simlife.Categorical((("a", 0),))

    def test_split_is_exact_and_never_zero(self):
        book = simlife.SeedBook("split")
        for concentration in (20, 100, 400):
            split = simlife.DirichletSplit(parts=5, concentration_percent=concentration)
            for index in range(50):
                shares = split.split(book.stream("s", index), 1_000_000_007)
                self.assertEqual(sum(shares), 1_000_000_007, "a split must conserve its total")
                self.assertTrue(all(share >= 1 for share in shares))

    def test_split_refuses_an_impossible_total(self):
        with self.assertRaises(simlife.Refusal):
            simlife.DirichletSplit(parts=5).split(simlife.SeedBook("x").stream("s"), 3)

    def test_every_distribution_describes_itself(self):
        for dist in (
            simlife.Constant(1), simlife.IntUniform(1, 2), simlife.LogIntUniform(1, 2),
            simlife.Categorical((("a", 1),)), simlife.Bernoulli(50), simlife.DirichletSplit(2),
        ):
            body = dist.describe()
            self.assertIn("kind", body)
            json.dumps(body)  # a description that will not serialise is not a record


class WorldTests(unittest.TestCase):
    def spec(self, **overrides):
        base = dict(
            seed=simlife.SeedBook("dclutch/simlife/test"),
            markets=12,
            ticks=30,
        )
        base.update(overrides)
        return simlife.WorldSpec(**base)

    def test_the_same_seed_draws_the_same_world(self):
        left = simlife.build_world(self.spec()).body()
        right = simlife.build_world(self.spec()).body()
        self.assertEqual(
            simcore.canonical_json_bytes(left), simcore.canonical_json_bytes(right)
        )

    def test_a_different_seed_draws_a_different_world(self):
        other = self.spec(seed=simlife.SeedBook("dclutch/simlife/other"))
        self.assertNotEqual(
            simlife.build_world(self.spec()).body()["plan_digest"],
            simlife.build_world(other).body()["plan_digest"],
        )

    def test_widening_the_world_does_not_reshuffle_the_markets_already_in_it(self):
        """The property independent streams are FOR.

        Market 3 of an eight-market world and market 3 of an eighty-market world
        are the same market. Without per-site streams every draw after the
        change would shift, and two runs of a slightly edited world would never
        be comparable again.
        """
        small = simlife.build_world(self.spec(markets=8))
        large = simlife.build_world(self.spec(markets=80))
        for index in range(8):
            self.assertEqual(
                small.markets[index].body(),
                large.markets[index].body(),
                f"market {index} changed when the world grew around it",
            )

    def test_the_world_is_heterogeneous(self):
        world = simlife.build_world(self.spec(markets=24))
        self.assertGreater(len({m.archetype for m in world.markets}), 3)
        self.assertGreater(len({m.outcome_count for m in world.markets}), 3)
        self.assertGreater(len({m.destiny for m in world.markets}), 1)
        personas = {p.persona for m in world.markets for p in m.participants}
        self.assertGreater(len(personas), 3)

    def test_every_destiny_and_every_persona_is_reachable(self):
        """A behaviour nothing ever draws is a behaviour that does not exist."""
        world = simlife.build_world(self.spec(markets=120, ticks=40))
        destinies = {m.destiny for m in world.markets}
        self.assertEqual(destinies, {
            simlife.DESTINY_RESOLVES, simlife.DESTINY_FAILS, simlife.DESTINY_SLEEPY,
        })
        personas = {p.persona for m in world.markets for p in m.participants}
        self.assertEqual(personas, set(simlife.PERSONAS_BY_NAME))
        bases = {m.basis for m in world.markets}
        self.assertEqual(bases, set(simlife.ALL_BASIS_KINDS))

    def test_stakes_sum_to_the_founding_collateral(self):
        world = simlife.build_world(self.spec(markets=40))
        for market in world.markets:
            total = sum(p.stake_atoms for p in market.participants)
            self.assertEqual(total, market.founding_collateral_atoms, market.market_id)

    def test_every_market_is_zero_fee(self):
        world = simlife.build_world(self.spec(markets=60))
        self.assertTrue(all(m.fee_basis_points == 0 for m in world.markets))

    def test_the_schedule_is_totally_ordered_and_numbered(self):
        world = simlife.build_world(self.spec(markets=20))
        self.assertEqual([e.sequence for e in world.events], list(range(len(world.events))))
        keys = [(e.tick, e.market_id, simlife.ROUTE_RANK[e.route], e.subject) for e in world.events]
        self.assertEqual(keys, sorted(keys))

    def test_a_market_is_founded_before_anything_else_touches_it(self):
        world = simlife.build_world(self.spec(markets=30))
        first_found = {}
        for event in world.events:
            if event.route == simlife.ROUTE_FOUND:
                first_found[event.market_id] = event.sequence
        for event in world.events:
            if event.route == simlife.ROUTE_FOUND:
                continue
            founded = first_found.get(event.market_id)
            if founded is None:
                continue
            self.assertGreater(event.sequence, founded, f"{event.route} preceded its founding")

    def test_no_event_falls_outside_the_horizon(self):
        world = simlife.build_world(self.spec(markets=30, ticks=15))
        self.assertTrue(all(0 <= e.tick < 15 for e in world.events))

    def test_a_sleeper_never_redeems_and_a_stranger_compacts_the_dormant(self):
        world = simlife.build_world(self.spec(markets=60, ticks=40, slots_per_tick=4000))
        sleepers = {
            p.participant_id for m in world.markets for p in m.participants
            if p.persona == "sleeper"
        }
        dormant = {
            p.participant_id for m in world.markets for p in m.participants if not p.redeems
        }
        self.assertTrue(sleepers)
        self.assertTrue(sleepers < dormant, "a sleeper is dormant, and so is a crank who holds")
        redeemed = {e.subject for e in world.events if e.route == simlife.ROUTE_REDEEM}
        self.assertEqual(redeemed & dormant, set(), "somebody who never redeems redeemed")
        compactions = [e for e in world.events if e.route == simlife.ROUTE_COMPACT]
        self.assertTrue(compactions, "no dormant claim check was ever compacted")
        compacted_sleepers = 0
        for event in compactions:
            self.assertIn(event.detail["dormant_holder"], dormant)
            self.assertNotEqual(
                event.detail["compacted_by"], event.detail["dormant_holder"],
                "compacting your own claim check is redemption wearing another name",
            )
            if event.detail["dormant_persona"] == "sleeper":
                compacted_sleepers += 1
        self.assertGreater(compacted_sleepers, 0, "no actual sleeper was ever compacted")

    def test_a_failure_branch_is_driven_by_a_crank_who_holds_nothing_in_it(self):
        world = simlife.build_world(self.spec(markets=80, ticks=40, slots_per_tick=4000))
        walks = [e for e in world.events if e.route == simlife.ROUTE_DEADLINE_FAILURE]
        self.assertTrue(walks, "no market ever reached its failure branch")
        for walk in walks:
            driver = walk.detail["driven_by"]
            if driver is None:
                continue
            self.assertTrue(driver.startswith("m"))

    def test_fills_arrive_in_bursts_rather_than_evenly(self):
        world = simlife.build_world(self.spec(markets=40, ticks=40, slots_per_tick=4000))
        fills = [e for e in world.events if e.route == simlife.ROUTE_FILL]
        self.assertTrue(fills)
        ticks = [e.tick for e in fills]
        # Clustering: strictly fewer distinct ticks than fills means at least one
        # tick carries more than one fill, which a steady rate never produces.
        self.assertLess(len(set(ticks)), len(ticks))
        for fill in fills:
            self.assertNotEqual(fill.detail["maker"], fill.detail["taker"])
            self.assertGreaterEqual(fill.detail["quantity_atoms"], 1)

    def test_a_quiet_corner_never_trades(self):
        world = simlife.build_world(self.spec(markets=60, ticks=40))
        quiet = {m.market_id for m in world.markets if m.archetype == "quiet-corner"}
        self.assertTrue(quiet)
        for event in world.events:
            if event.route == simlife.ROUTE_FILL:
                self.assertNotIn(event.market_id, quiet)

    def test_a_world_refuses_to_be_empty(self):
        with self.assertRaises(simlife.Refusal):
            simlife.build_world(self.spec(markets=0))
        with self.assertRaises(simlife.Refusal):
            simlife.build_world(self.spec(ticks=0))

    def test_the_summary_says_when_nothing_will_resolve(self):
        lines = simlife.world_summary(simlife.build_world(self.spec(markets=6, ticks=3)))
        self.assertTrue(any("horizon" in line for line in lines))


class ConductorTests(unittest.TestCase):
    def world(self, **overrides):
        base = dict(seed=simlife.SeedBook("conductor"), markets=8, ticks=20)
        base.update(overrides)
        return simlife.build_world(simlife.WorldSpec(**base))

    def test_a_rehearsal_executes_nothing_and_says_so_everywhere(self):
        world = self.world()
        ledger = simlife.Conductor(world, simlife.RehearsalSubstrate()).run()
        self.assertEqual(len(ledger["entries"]), len(world.events))
        outcomes = {entry["result"]["outcome"] for entry in ledger["entries"]}
        self.assertEqual(outcomes, {simlife.OUTCOME_UNATTEMPTED})
        self.assertEqual(ledger["markets_founded"], [])
        for entry in ledger["entries"]:
            self.assertIsNone(entry["result"]["observation"])

    def test_a_substrate_without_founding_blocks_nothing_downstream_by_accident(self):
        """A route the substrate lacks is `unattempted`; a thing DOWNSTREAM of a
        route it lacks is `blocked`. The two words must not swap."""

        class CensusOnly(simlife.Substrate):
            name, label = "t", "a test substrate"
            routes = frozenset({simlife.ROUTE_CENSUS})
            basis_kinds = frozenset(simlife.ALL_BASIS_KINDS)

            def execute(self, event, market):
                return simlife.EventResult(simlife.OUTCOME_EXECUTED, "observed")

        world = self.world()
        conductor = simlife.Conductor(world, CensusOnly())
        conductor.run()
        tally = conductor.tally()
        self.assertEqual(tally[simlife.ROUTE_FOUND][simlife.OUTCOME_UNATTEMPTED],
                         tally[simlife.ROUTE_FOUND][simlife.OUTCOME_UNATTEMPTED])
        self.assertEqual(tally[simlife.ROUTE_FOUND][simlife.OUTCOME_BLOCKED], 0)
        # The census never executes, because no founding did.
        self.assertEqual(tally[simlife.ROUTE_CENSUS][simlife.OUTCOME_EXECUTED], 0)
        self.assertGreater(tally[simlife.ROUTE_CENSUS][simlife.OUTCOME_BLOCKED], 0)

    def test_a_substrate_that_cannot_express_a_basis_says_which_and_why(self):
        class CategoricalOnly(simlife.Substrate):
            name, label = "t", "a categorical-only substrate"
            routes = frozenset(simlife.ALL_ROUTES)
            basis_kinds = frozenset({simlife.BASIS_CATEGORICAL})

            def execute(self, event, market):
                return simlife.EventResult(simlife.OUTCOME_EXECUTED, "did it")

        world = self.world(markets=24)
        conductor = simlife.Conductor(world, CategoricalOnly())
        conductor.run()
        graded = {m.market_id for m in world.markets if m.basis != simlife.BASIS_CATEGORICAL}
        self.assertTrue(graded)
        for entry in conductor.entries:
            if entry.event.market_id not in graded:
                continue
            self.assertNotEqual(entry.result.outcome, simlife.OUTCOME_EXECUTED)
            if entry.event.route == simlife.ROUTE_FOUND:
                self.assertEqual(entry.result.outcome, simlife.OUTCOME_UNATTEMPTED)
                self.assertIn("founding", entry.result.detail.lower() + " founding")

    def test_a_full_substrate_walks_the_whole_arc(self):
        class Everything(simlife.Substrate):
            name, label = "t", "a substrate with every route"
            routes = frozenset(simlife.ALL_ROUTES)
            basis_kinds = frozenset(simlife.ALL_BASIS_KINDS)

            def execute(self, event, market):
                return simlife.EventResult(simlife.OUTCOME_EXECUTED, "did it")

        world = self.world(markets=40, ticks=40, slots_per_tick=4000)
        conductor = simlife.Conductor(world, Everything())
        conductor.run()
        tally = conductor.tally()
        for route in (simlife.ROUTE_FOUND, simlife.ROUTE_ADMIT, simlife.ROUTE_FILL,
                      simlife.ROUTE_RESOLVE, simlife.ROUTE_DEADLINE_FAILURE,
                      simlife.ROUTE_REDEEM, simlife.ROUTE_COMPACT, simlife.ROUTE_RETIRE,
                      simlife.ROUTE_CENSUS):
            self.assertGreater(
                tally.get(route, {}).get(simlife.OUTCOME_EXECUTED, 0), 0,
                f"{route} never executed even against a substrate that has every route",
            )

    def test_a_refused_founding_blocks_that_market_and_no_other(self):
        class OneRefuses(simlife.Substrate):
            name, label = "t", "a substrate that refuses one founding"
            routes = frozenset(simlife.ALL_ROUTES)
            basis_kinds = frozenset(simlife.ALL_BASIS_KINDS)

            def execute(self, event, market):
                if event.route == simlife.ROUTE_FOUND and event.market_id == "m00":
                    return simlife.EventResult(simlife.OUTCOME_REFUSED, "custom program error 0x5182")
                return simlife.EventResult(simlife.OUTCOME_EXECUTED, "did it")

        world = self.world(markets=8, ticks=20)
        conductor = simlife.Conductor(world, OneRefuses())
        conductor.run()
        by_market: dict = {}
        for entry in conductor.entries:
            by_market.setdefault(entry.event.market_id, set()).add(entry.result.outcome)
        self.assertEqual(by_market["m00"], {simlife.OUTCOME_REFUSED, simlife.OUTCOME_BLOCKED})
        for market_id, outcomes in by_market.items():
            if market_id == "m00":
                continue
            self.assertNotIn(simlife.OUTCOME_BLOCKED, outcomes, market_id)

    def test_an_event_result_refuses_a_word_that_is_not_one_of_the_four(self):
        with self.assertRaises(simlife.Refusal):
            simlife.EventResult(outcome="ok", detail="")

    def test_the_ledger_redacts_a_credential_in_a_detail(self):
        entry = simlife.EventResult(
            outcome=simlife.OUTCOME_REFUSED,
            detail="child said: --rpc-url https://devnet.example.com/?api-key=SECRETKEY failed",
        ).body()
        self.assertNotIn("SECRETKEY", entry["detail"])


# ---------------------------------------------------------------------------
# The driver layer, against a fake bootstrap
# ---------------------------------------------------------------------------


class DriveTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.boot = write_fake_boot(self.root)
        self.work = self.root / "work"

    def tearDown(self):
        self.tmp.cleanup()

    def config(self, *, bindings=None, markets=6, ticks=8) -> Path:
        body = {
            "schema": drive.SCHEMA_CONFIG,
            "cluster": {"label": "local", "rpc_url": "http://127.0.0.1:8899"},
            "bootstrap_bin": str(self.boot),
            "work_dir": str(self.work),
            "substrate_label": "a fake bootstrap in a temporary directory",
            "world": {
                "seed": "dclutch/simlife/test-drive",
                "markets": markets,
                "ticks": ticks,
                "archetype_mix": "foundable-today",
                "slots_per_tick": 4000,
            },
            "bindings": bindings if bindings is not None else {
                "m00": {
                    "mint": "Mint1", "payer": "Payer1", "hoard": "Hoard1",
                    "aggregate": "Agg1", "claim_unit_atoms": 1, "outcome_count": 2,
                },
                # m02, not m01: m01 is the seven-cell wide-field this world drew,
                # and the fake census reports two cells. Binding it there is the
                # exact misfiling `check_bindings` refuses, and is exercised as a
                # refusal below rather than smuggled into every other test.
                "m02": {
                    "mint": "Mint2", "payer": "Payer2", "hoard": "Hoard2",
                    "aggregate": "Agg2", "claim_unit_atoms": 1, "outcome_count": 2,
                },
            },
        }
        path = self.root / "config.json"
        path.write_text(json.dumps(body, indent=2))
        return path

    def test_a_config_with_a_wrong_schema_is_refused(self):
        path = self.root / "bad.json"
        path.write_text(json.dumps({"schema": "something-else"}))
        with self.assertRaises(drive.Refusal):
            drive.load_config(path)

    def test_a_non_loopback_local_rpc_is_refused(self):
        path = self.root / "bad.json"
        path.write_text(json.dumps({
            "schema": drive.SCHEMA_CONFIG,
            "cluster": {"label": "local", "rpc_url": "http://10.0.0.5:8899"},
            "bootstrap_bin": str(self.boot), "work_dir": str(self.work),
            "world": {"seed": "s"},
        }))
        with self.assertRaises(drive.Refusal):
            drive.load_config(path)

    def test_mainnet_is_refused_unconditionally(self):
        path = self.root / "bad.json"
        path.write_text(json.dumps({
            "schema": drive.SCHEMA_CONFIG,
            "cluster": {"label": "devnet", "rpc_url": "https://api.mainnet-beta.solana.com",
                        "devnet_genesis": "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"},
            "bootstrap_bin": str(self.boot), "work_dir": str(self.work),
            "world": {"seed": "s"},
        }))
        with self.assertRaises(drive.Refusal):
            drive.load_config(path)

    def test_an_unnamed_run_is_refused(self):
        path = self.root / "bad.json"
        path.write_text(json.dumps({
            "schema": drive.SCHEMA_CONFIG,
            "cluster": {"label": "local", "rpc_url": "http://127.0.0.1:8899"},
            "bootstrap_bin": str(self.boot), "work_dir": str(self.work),
            "world": {"seed": ""},
        }))
        with self.assertRaises(drive.Refusal):
            drive.load_config(path)

    def test_preflight_composes_the_command_and_runs_nothing(self):
        code = drive.main(["run", "--config", str(self.config())])
        self.assertEqual(code, 0)
        ledger = json.loads((self.work / "ledger.json").read_text())
        census = [e for e in ledger["entries"] if e["route"] == simlife.ROUTE_CENSUS]
        self.assertTrue(census)
        # Nothing ran. A bound market's census composed its command and stopped;
        # an unbound market's is blocked, because nothing exists to observe.
        self.assertEqual(
            {e["result"]["outcome"] for e in census},
            {simlife.OUTCOME_UNATTEMPTED, simlife.OUTCOME_BLOCKED},
        )
        bound = [e for e in census if e["market_id"] in ("m00", "m02")]
        self.assertTrue(bound)
        for entry in bound:
            self.assertEqual(entry["result"]["outcome"], simlife.OUTCOME_UNATTEMPTED)
            self.assertIn("ledger-census", entry["result"]["detail"])
        for directory in (self.work / "census").glob("*"):
            self.assertEqual(list(directory.glob("cycle-*.json")), [], "preflight wrote a census")

    def test_an_executed_run_observes_every_bound_market_and_only_those(self):
        code = drive.main(["run", "--config", str(self.config()), "--execute"])
        self.assertEqual(code, 0)
        ledger = json.loads((self.work / "ledger.json").read_text())
        executed = [e for e in ledger["entries"]
                    if e["result"]["outcome"] == simlife.OUTCOME_EXECUTED]
        self.assertTrue(executed)
        self.assertEqual({e["market_id"] for e in executed}, {"m00", "m02"})
        for entry in executed:
            self.assertIsNotNone(entry["result"]["observation"]["slot"])
        # A planned market with nothing bound to it is never observed.
        unbound = [e for e in ledger["entries"]
                   if e["route"] == simlife.ROUTE_CENSUS and e["market_id"] not in ("m00", "m02")]
        self.assertTrue(unbound)
        self.assertTrue(all(
            e["result"]["outcome"] in (simlife.OUTCOME_UNATTEMPTED, simlife.OUTCOME_BLOCKED)
            for e in unbound
        ))

    def test_each_market_keeps_its_own_census_chain(self):
        drive.main(["run", "--config", str(self.config()), "--execute"])
        for market_id in ("m00", "m02"):
            files = sorted((self.work / "census" / market_id).glob("cycle-*.json"))
            self.assertTrue(files, market_id)
            series = json.loads(files[-1].read_text())
            # Every observation in a chain belongs to its own market: the fake
            # echoes `--aggregate`, so a crossed chain is visible here.
            self.assertEqual({o["aggregate"] for o in series}, {"Agg1" if market_id == "m00" else "Agg2"})
            self.assertTrue(all(o["stage"].startswith(f"simlife-{market_id}-") for o in series))

    def test_the_newest_census_file_is_the_whole_chain(self):
        drive.main(["run", "--config", str(self.config()), "--execute"])
        files = sorted((self.work / "census" / "m00").glob("cycle-*.json"))
        newest = json.loads(files[-1].read_text())
        self.assertGreater(len(newest), 1)
        for older in files[:-1]:
            prefix = json.loads(older.read_text())
            self.assertEqual(prefix, newest[:len(prefix)])

    def test_a_violated_law_halts_the_run_and_refuses_a_restart(self):
        (self.root / "census-violation").write_text("")
        config = self.config()
        self.assertEqual(drive.main(["run", "--config", str(config), "--execute"]), 3)
        self.assertTrue((self.work / "HALT.json").exists())
        halt = json.loads((self.work / "HALT.json").read_text())
        self.assertIn("conservation law", halt["reason"])
        # And the work dir now refuses to start again until a human clears it.
        self.assertEqual(drive.main(["run", "--config", str(config), "--execute"]), 2)

    def test_backpressure_stops_rather_than_hammers(self):
        (self.root / "backpressure").write_text("")
        self.assertEqual(
            drive.main(["run", "--config", str(self.config()), "--execute"]), 5
        )
        exit_record = json.loads((self.work / "EXIT.json").read_text())
        self.assertEqual(exit_record["outcome"], simcore.EXIT_SIGNALLED)

    def test_a_rerun_continues_the_chain_rather_than_restarting_it(self):
        config = self.config(ticks=6)
        drive.main(["run", "--config", str(config), "--execute"])
        first = len(json.loads(
            sorted((self.work / "census" / "m00").glob("cycle-*.json"))[-1].read_text()
        ))
        drive.main(["run", "--config", str(config), "--execute"])
        second = len(json.loads(
            sorted((self.work / "census" / "m00").glob("cycle-*.json"))[-1].read_text()
        ))
        self.assertGreater(second, first, "a rerun restarted the chain instead of extending it")

    def test_the_status_artifact_carries_the_seed_and_the_tally(self):
        drive.main(["run", "--config", str(self.config()), "--execute"])
        status = json.loads((self.work / "status.json").read_text())
        self.assertEqual(status["schema"], simcore.SCHEMA_STATUS)
        self.assertIn("simlife", status)
        self.assertEqual(status["simlife"]["seed"]["preimage"], "dclutch/simlife/test-drive")
        self.assertIn("expected_next_update_by", status["heartbeat"])
        self.assertEqual(status["simlife"]["markets_bound"], 2)

    def test_a_binding_of_the_wrong_width_is_refused_before_anything_runs(self):
        """The one join where a caption could come apart from its chart.

        m01 is this world's seven-cell wide-field. Binding a two-cell market to
        it would draw two bars under a caption promising seven, or lay one
        market's cells under another market's names.
        """
        config = self.config(bindings={
            "m01": {"mint": "M", "payer": "P", "hoard": "H", "aggregate": "A",
                    "claim_unit_atoms": 1, "outcome_count": 2},
        })
        self.assertEqual(drive.main(["run", "--config", str(config), "--execute"]), 2)
        self.assertFalse((self.work / "census").exists())

    def test_a_binding_naming_no_planned_market_is_refused(self):
        config = self.config(bindings={
            "m99": {"mint": "M", "payer": "P", "hoard": "H", "aggregate": "A",
                    "claim_unit_atoms": 1, "outcome_count": 2},
        })
        self.assertEqual(drive.main(["run", "--config", str(config), "--execute"]), 2)

    def test_a_binding_without_a_width_is_refused(self):
        path = self.root / "nowidth.json"
        path.write_text(json.dumps({
            "schema": drive.SCHEMA_CONFIG,
            "cluster": {"label": "local", "rpc_url": "http://127.0.0.1:8899"},
            "bootstrap_bin": str(self.boot), "work_dir": str(self.work),
            "world": {"seed": "s"},
            "bindings": {"m00": {"mint": "M", "payer": "P", "hoard": "H",
                                 "aggregate": "A", "claim_unit_atoms": 1}},
        }))
        with self.assertRaises(drive.Refusal):
            drive.load_config(path)

    def test_the_world_document_is_written_and_names_its_own_digest(self):
        drive.main(["run", "--config", str(self.config()), "--execute"])
        world = json.loads((self.work / "world.json").read_text())
        self.assertEqual(world["schema"], simlife.SCHEMA_WORLD)
        self.assertEqual(
            world["plan_digest"],
            simcore.digest_of({"markets": world["markets"], "events": world["events"]}),
        )

    def test_a_substrate_that_founds_nothing_claims_no_basis_it_can_express(self):
        """`basis_kinds_absent: []` on a substrate that cannot found anything
        would read as `it can found every shape`, which is the opposite."""
        substrate = drive.LedgerCensusSubstrate(
            drive.load_config(self.config()), self.work, execute=False,
        )
        described = substrate.describe()
        self.assertEqual(described["basis_kinds"], [])
        self.assertEqual(sorted(described["basis_kinds_absent"]), sorted(simlife.ALL_BASIS_KINDS))
        self.assertIn(simlife.ROUTE_FOUND, described["routes_absent"])

    def test_every_route_has_a_named_driver(self):
        """A route with no driver named is a route nobody can go and run."""
        for route in simlife.ALL_ROUTES:
            self.assertIn(route, drive.ROUTE_DRIVERS)
            self.assertGreater(len(drive.ROUTE_DRIVERS[route]), 40)


# ---------------------------------------------------------------------------
# The driver layer
# ---------------------------------------------------------------------------

DRIVERS_SPEC = importlib.util.spec_from_file_location(
    "dclutch_simlife_drivers", HERE / "simlife_drivers.py"
)
assert DRIVERS_SPEC is not None and DRIVERS_SPEC.loader is not None
drivers = importlib.util.module_from_spec(DRIVERS_SPEC)
sys.modules[DRIVERS_SPEC.name] = drivers
DRIVERS_SPEC.loader.exec_module(drivers)


def alt_account(addresses, *, frozen: bool) -> str:
    """One AddressLookupTable account body, in the layout the runtime writes."""
    import base64
    raw = bytearray(56)
    raw[0:4] = (1).to_bytes(4, "little")
    raw[21] = 0 if frozen else 1
    if not frozen:
        raw[22:54] = bytes(range(32))
    for entry in addresses:
        raw += entry
    return base64.b64encode(bytes(raw)).decode()


class DriverPrimitiveTests(unittest.TestCase):
    """The three pure things the driver layer computes for itself.

    Everything else it does is a subprocess. These are the exceptions, and each
    one is a READ of something already on disk or on chain rather than a
    reimplementation of something the protocol does.
    """

    def test_a_keypair_files_public_half_is_its_second_thirty_two_bytes(self):
        with tempfile.TemporaryDirectory() as work:
            path = Path(work) / "key.json"
            # A recognisable secret half, then a public half of all-ones bytes.
            path.write_text(json.dumps(list(range(32)) + [1] * 32))
            self.assertEqual(drivers.keypair_pubkey(path), drivers.base58(bytes([1] * 32)))

    def test_a_keypair_file_of_the_wrong_width_is_refused_rather_than_truncated(self):
        with tempfile.TemporaryDirectory() as work:
            path = Path(work) / "key.json"
            path.write_text(json.dumps(list(range(32))))
            with self.assertRaises(drivers.DriverRefusal):
                drivers.keypair_pubkey(path)

    def test_base58_keeps_leading_zero_bytes_as_ones(self):
        self.assertTrue(drivers.base58(bytes([0, 0, 1])).startswith("11"))

    def test_the_frozen_table_containing_the_market_is_the_one_chosen(self):
        """A founding creates five routing tables and freezes exactly one.

        The admission must route through the FROZEN one -- passing all five
        refuses `DuplicateAddress` -- and the founding evidence does not record
        its address. Both facts needed to find it are on the chain: authority
        `None`, and the market in its own address list.
        """
        market = bytes([7] * 32)
        other = bytes([9] * 32)
        answers = {
            "result": [
                # Still extendable, and it holds the market: NOT the one.
                {"pubkey": "still-writable", "account": {"data": [alt_account([market], frozen=False), "base64"]}},
                # Frozen, and it holds a different market: NOT the one.
                {"pubkey": "another-founding", "account": {"data": [alt_account([other], frozen=True), "base64"]}},
                {"pubkey": "the-frozen-one", "account": {"data": [alt_account([other, market], frozen=True), "base64"]}},
            ]
        }
        original = drivers.rpc
        drivers.rpc = lambda url, method, params, timeout=60.0: answers["result"]
        try:
            found = drivers.frozen_routing_table_for("http://127.0.0.1:1/", drivers.base58(market))
        finally:
            drivers.rpc = original
        self.assertEqual(found, "the-frozen-one")

    def test_a_founding_evidence_without_the_market_is_refused_not_guessed(self):
        with tempfile.TemporaryDirectory() as work:
            evidence = Path(work) / "evidence.json"
            evidence.write_text(json.dumps({
                "payer": "P", "execution": {"market": {"accounts": {"collateral_mint": {"address": "M"}}}},
            }))
            market_input = Path(work) / "market.json"
            market_input.write_text(json.dumps({"coefficients": [1, 0]}))
            with self.assertRaises(drivers.DriverRefusal) as caught:
                drivers.founded_market_from_evidence("m00", evidence, market_input, Path(work))
            self.assertIn("refuses to guess", str(caught.exception))

    def test_a_census_binding_names_the_fixture_source_or_L1_fails_by_construction(self):
        with tempfile.TemporaryDirectory() as work:
            accounts = {
                name: {"address": name.upper()} for name in (
                    "founding_market", "collateral_mint", "founding_hoard_vault",
                    "claims_aggregate", "founder_position", "collateral_wallet",
                    "local_participant_fixture_source",
                )
            }
            evidence = Path(work) / "evidence.json"
            evidence.write_text(json.dumps({"payer": "P", "execution": {"market": {"accounts": accounts}}}))
            market_input = Path(work) / "market.json"
            market_input.write_text(json.dumps({"coefficients": [1, 1, 0]}))
            founded = drivers.founded_market_from_evidence("m00", evidence, market_input, Path(work))
            self.assertEqual(founded.outcome_count, 3)
            binding = founded.census_binding()
            self.assertIn("participant_fixture_source", binding["tokens"])
            self.assertEqual(binding["outcome_count"], 3)
            # The claim unit is the CHAIN's, not the plan's: the basis compiler
            # hard-wires it and the census must be told the truth.
            self.assertEqual(binding["claim_unit_atoms"], 1)


class LifecycleSubstrateTests(unittest.TestCase):
    """The substrate's outcome mapping, which is the whole vocabulary.

    A driver that refuses is `refused` -- a reading about this chain -- and a
    route with no driver anywhere is `unattempted`. Collapsing the two would
    turn one wall into a hundred failures, which is the failure this module's
    four words exist to prevent.
    """

    def config(self, work: Path, boot: Path) -> dict:
        return {
            "schema": drive.SCHEMA_CONFIG,
            "substrate": "lifecycle",
            "cluster": {"label": "local", "rpc_url": "http://127.0.0.1:34599/"},
            "bootstrap_bin": str(boot),
            "work_dir": str(work),
            "lifecycle": {
                "plan": str(work / "plan.json"),
                "campaign_payer_keypair": str(work / "payer.json"),
                "founding_founder": "F",
                "substituted_founder": "S",
            },
            "world": {"seed": "dclutch/simlife2/test", "markets": 3, "ticks": 4},
            "bindings": {},
        }

    def test_a_substrate_declares_every_route_but_the_one_with_no_driver(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            boot = write_fake_boot(work)
            substrate = drive.LifecycleSubstrate(self.config(work, boot), work, execute=False)
            self.assertIn(simlife.ROUTE_FOUND, substrate.routes)
            self.assertIn(simlife.ROUTE_RETIRE, substrate.routes)
            self.assertNotIn(simlife.ROUTE_COMPACT, substrate.routes)
            self.assertIn("has NO CLI anywhere", substrate.why_not(simlife.ROUTE_COMPACT))

    def test_compaction_is_sized_rather_than_called_impossible(self):
        # An impossibility is a refusal; a size is an estimate with a number.
        self.assertIn("hours", drivers.COMPACTION_ABSENT)
        self.assertIn("ProgramTest", drivers.COMPACTION_ABSENT)

    def test_a_driver_that_refuses_is_refused_and_not_unattempted(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            boot = write_fake_boot(work)
            substrate = drive.LifecycleSubstrate(self.config(work, boot), work, execute=True)
            world = simlife.build_world(simlife.WorldSpec(
                seed=simlife.SeedBook("dclutch/simlife2/test"), markets=3, ticks=4,
            ))
            market = world.markets[0]
            event = simlife.PlannedEvent(
                tick=0, route=simlife.ROUTE_ADMIT, market_id=market.market_id,
                subject="p0", detail={"stake_atoms": 1},
            )
            # Nothing was founded, so the driver layer refuses before it
            # composes a command line for a market that does not exist.
            result = substrate.execute(event, market)
            self.assertEqual(result.outcome, simlife.OUTCOME_REFUSED)
            self.assertIn("never founded by this run", result.detail)

    def test_a_preflight_run_attempts_no_mutation_and_names_the_driver(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            boot = write_fake_boot(work)
            substrate = drive.LifecycleSubstrate(self.config(work, boot), work, execute=False)
            world = simlife.build_world(simlife.WorldSpec(
                seed=simlife.SeedBook("dclutch/simlife2/test"), markets=3, ticks=4,
            ))
            market = world.markets[0]
            event = simlife.PlannedEvent(
                tick=0, route=simlife.ROUTE_FOUND, market_id=market.market_id,
                subject=market.market_id, detail={},
            )
            result = substrate.execute(event, market)
            self.assertEqual(result.outcome, simlife.OUTCOME_UNATTEMPTED)
            self.assertIn("local-private-validator-market-v1", result.detail)

    def test_a_lifecycle_config_without_its_founding_identities_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            boot = write_fake_boot(work)
            config = self.config(work, boot)
            del config["lifecycle"]["founding_founder"]
            with self.assertRaises(drive.Refusal) as caught:
                drive.LifecycleSubstrate(config, work, execute=False)
            self.assertIn("will not invent one", str(caught.exception))

    def test_the_substrate_a_config_names_is_the_one_built(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            boot = write_fake_boot(work)
            config = self.config(work, boot)
            self.assertIsInstance(
                drive.build_substrate(config, work, execute=False), drive.LifecycleSubstrate
            )
            # The DEFAULT is the read-only one: mutating a chain is opt-in in a
            # file somebody reviewed, not a consequence of upgrading a module.
            del config["substrate"]
            self.assertIsInstance(
                drive.build_substrate(config, work, execute=False), drive.LedgerCensusSubstrate
            )
            config["substrate"] = "whatever"
            with self.assertRaises(drive.Refusal):
                drive.build_substrate(config, work, execute=False)


class BandTests(unittest.TestCase):
    def test_a_two_cell_market_has_no_cuts_and_a_wide_one_has_width_minus_two(self):
        for width in (2, 3, 4, 9):
            self.assertEqual(len(simlife._band(width, 20_000, 1_000)), width - 2)

    def test_a_band_is_strictly_increasing_and_positive(self):
        cuts = simlife._band(9, 3_000, 900)
        self.assertEqual(cuts, sorted(set(cuts)))
        self.assertTrue(all(cut > 0 for cut in cuts))

    def test_the_failure_cell_never_pays_and_something_always_does(self):
        import random as _random
        for seed in range(40):
            coefficients = simlife._payoff(_random.Random(seed), 5)
            self.assertEqual(len(coefficients), 5)
            self.assertEqual(coefficients[-1], 0, "the explicit failure cell must pay nothing")
            self.assertTrue(any(coefficients[:-1]), "a portfolio worth zero everywhere is not a claim")

    def test_two_markets_of_one_width_are_not_the_same_market_twice(self):
        world = simlife.build_world(simlife.WorldSpec(
            seed=simlife.SeedBook("dclutch/simlife2/bands"), markets=24, ticks=8,
        ))
        by_width: dict = {}
        for market in world.markets:
            by_width.setdefault(market.outcome_count, set()).add(tuple(market.cuts))
        widened = [width for width, bands in by_width.items() if width > 2 and len(bands) > 1]
        self.assertTrue(widened, "some width must have drawn two different bands")

    def test_the_deadline_in_slots_becomes_a_window_in_seconds(self):
        world = simlife.build_world(simlife.WorldSpec(
            seed=simlife.SeedBook("dclutch/simlife2/clock"), markets=4, ticks=8,
        ))
        for market in world.markets:
            seconds = drivers.terminal_window_seconds(market)
            self.assertGreaterEqual(seconds, 1)
            # 400 ms a slot, by the cluster's own target.
            self.assertEqual(seconds, max(1, (market.deadline_slots * 2) // 5))


if __name__ == "__main__":
    unittest.main(verbosity=2)
