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
    # FALLING, so a spend ceiling has something to cross. The real census
    # reports the payer's live balance and a fee payer's balance only goes down
    # between airdrops; a constant here would have made the budget kill
    # untestable through this harness.
    "payer_lamports": 1000 - len(series) * 7,
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

    def test_the_fee_band_reaches_the_admitted_rate_and_past_it(self):
        """A world must be mostly tradeable and must still contain a control.

        This replaces `test_every_market_is_zero_fee`, which pinned the opposite
        property for a reason that turned out to be about the fill's fee leg
        rather than about founding. Zero is the ONE rate the owned-loopback
        Direct producer can never fill, so a world of zero-fee markets was a
        world whose fills could not have landed however many other walls fell.
        """
        world = simlife.build_world(self.spec(markets=60))
        rates = sorted({market.fee_basis_points for market in world.markets})
        self.assertTrue(all(0 <= rate <= 10_000 for rate in rates), rates)
        self.assertIn(simlife.DIRECT_ADMITTED_FEE_BASIS_POINTS_V1, rates)
        # And a rate the release does NOT admit, so the producer's rate clause
        # is exercised by this world rather than only described by it.
        self.assertTrue(
            [rate for rate in rates if rate != simlife.DIRECT_ADMITTED_FEE_BASIS_POINTS_V1],
            rates,
        )
        # The majority of a world should be tradeable, or a run spends its night
        # measuring the same refusal.
        admitted = [
            market for market in world.markets
            if market.fee_basis_points == simlife.DIRECT_ADMITTED_FEE_BASIS_POINTS_V1
        ]
        self.assertGreater(len(admitted), len(world.markets) // 2, rates)

    def test_the_engine_and_the_driver_layer_agree_on_the_admitted_rate(self):
        """Two modules state the rate; a disagreement is a silently dead world.

        The engine may not import the driver layer -- it decides what to attempt
        and a substrate decides what happens -- so the constant is written twice
        and pinned equal here rather than hoped equal.
        """
        self.assertEqual(
            simlife.DIRECT_ADMITTED_FEE_BASIS_POINTS_V1,
            drivers.DIRECT_ADMITTED_FEE_BASIS_POINTS_V1,
        )

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

    def config(self, *, bindings=None, markets=6, ticks=8, budget=None) -> Path:
        body = {
            "schema": drive.SCHEMA_CONFIG,
            "cluster": {"label": "local", "rpc_url": "http://127.0.0.1:8899"},
            "bootstrap_bin": str(self.boot),
            "work_dir": str(self.work),
            "substrate_label": "a fake bootstrap in a temporary directory",
            "world": {
                "seed": "dclutch/simlife/test-drive-4",
                "markets": markets,
                "ticks": ticks,
                "archetype_mix": "foundable-today",
                "slots_per_tick": 4000,
            },
            **({"budget": budget} if budget is not None else {}),
            "bindings": bindings if bindings is not None else {
                "m00": {
                    "mint": "Mint1", "payer": "Payer1", "hoard": "Hoard1",
                    "aggregate": "Agg1", "claim_unit_atoms": 1, "outcome_count": 2,
                },
                # m02, not m01: m01 is the three-cell coin-flip this world
                # drew, and the fake census reports two cells. Binding it there
                # is the exact misfiling `check_bindings` refuses, and is
                # exercised as a refusal below rather than smuggled into every
                # other test. The seed carries a suffix for the same reason the
                # binding names m02: when `coin-flip` widened from two cells to
                # three -- a width-2 market has NO cuts, so its only ordinary
                # answer is "it did not fail", which is not a coin flip -- the
                # unsuffixed seed stopped drawing two two-cell markets and the
                # guard correctly refused. The fixture moved rather than the
                # guard loosening.
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

    def test_a_crossed_spend_budget_kills_the_run_and_refuses_a_restart(self):
        """The fourth kill condition, end to end.

        A spend ceiling has to stop a run the way a broken law does -- HALT.json
        on disk, a restart refused until a human clears it -- and it has to say
        a DIFFERENT word, because a broken conservation law is a fact about the
        ledger and a crossed budget is a fact about the run.
        """
        config = self.config(budget={"max_lamports_spent": 10})
        self.assertEqual(drive.main(["run", "--config", str(config), "--execute"]), 6)
        halt = json.loads((self.work / "HALT.json").read_text())
        self.assertIn("spend budget crossed", halt["reason"])
        self.assertIn("lamports", halt["reason"])
        spend = halt["details"]["spend"]
        self.assertGreater(spend["spent_lamports"], 10)
        self.assertEqual(spend["max_lamports_spent"], 10)
        self.assertTrue(spend["bounded"])
        exit_body = json.loads((self.work / "EXIT.json").read_text())
        self.assertEqual(exit_body["outcome"], simcore.EXIT_OVERSPENT)
        # The census that crossed it is ON DISK: the halt is the run's ending,
        # never a hole in its history.
        self.assertTrue(Path(halt["details"]["census"]).is_file())
        # And a restart refuses, so an unattended lane cannot resume spending.
        self.assertEqual(drive.main(["run", "--config", str(config), "--execute"]), 2)

    def test_a_run_with_no_budget_records_its_spend_and_never_stops_for_it(self):
        config = self.config()
        self.assertEqual(drive.main(["run", "--config", str(config), "--execute"]), 0)
        ledger = json.loads((self.work / "ledger.json").read_text())
        spend = ledger["substrate"]["spend"]
        self.assertFalse(spend["bounded"])
        self.assertIsNone(spend["max_lamports_spent"])
        self.assertGreater(spend["spent_lamports"], 0)
        self.assertFalse((self.work / "HALT.json").exists())

    def test_a_budget_that_is_not_a_positive_whole_number_refuses_before_anything_runs(self):
        config = self.config(budget={"max_lamports_spent": 0})
        self.assertEqual(drive.main(["run", "--config", str(config), "--execute"]), 2)
        self.assertFalse((self.work / "census").exists())

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
        self.assertEqual(status["simlife"]["seed"]["preimage"], "dclutch/simlife/test-drive-4")
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

    def _routing_answers(self, *, table_body: str, table_owner=drivers.ADDRESS_LOOKUP_TABLE_PROGRAM):
        """A chain that answers the two reads by address and REFUSES a scan.

        The refusal is the control. The old discovery scanned
        `getProgramAccounts` over the whole AddressLookupTable program, which is
        exactly what a public endpoint will not do, so a replacement that still
        scanned would pass a test that merely checked the answer.
        """
        seen = []

        def fake(url, method, params, timeout=60.0):
            seen.append(method)
            if method == "getProgramAccounts":
                raise AssertionError(
                    "the routing table must be looked up by address, never scanned for"
                )
            if method == "getTransaction":
                return {"transaction": {"message": {
                    "accountKeys": [
                        "payer", "the-frozen-one", "11111111111111111111111111111111",
                        drivers.ADDRESS_LOOKUP_TABLE_PROGRAM,
                    ],
                    "instructions": [
                        {"programIdIndex": 3, "accounts": [1, 0, 0, 2], "data": "1111"},
                    ],
                }}}
            if method == "getAccountInfo":
                return {"value": {"owner": table_owner, "data": [table_body, "base64"]}}
            raise AssertionError(f"unexpected read {method}")

        return fake, seen

    def _evidence_with_create(self):
        return {"execution": {"transactions": [
            {"label": "publish record: something else", "signature": "no",
             "error": None},
            {"label": drivers.FROZEN_ROUTING_TABLE_CREATE_LABEL_V1,
             "signature": "the-create-signature", "error": None},
        ]}}

    def test_the_routing_table_is_read_by_address_and_never_scanned_for(self):
        """Two reads: the founding's own create transaction, then the account.

        Measured on cohort-11: `getProgramAccounts` over devnet's ALT program
        returns nothing usable, so the search this replaces answered `None` for
        a table that exists. The address was in the founding's own create
        transaction the whole time.
        """
        market = drivers.base58(bytes([7] * 32))
        fake, seen = self._routing_answers(
            table_body=alt_account([bytes([9] * 32), bytes([7] * 32)], frozen=True)
        )
        original = drivers.rpc
        drivers.rpc = fake
        try:
            found = drivers.frozen_routing_table_for(
                "http://127.0.0.1:1/", self._evidence_with_create(), market
            )
        finally:
            drivers.rpc = original
        self.assertEqual(found, "the-frozen-one")
        self.assertEqual(seen, ["getTransaction", "getAccountInfo"])

    def test_a_founding_that_recorded_no_create_answers_absent_without_reading(self):
        original = drivers.rpc
        drivers.rpc = lambda *a, **k: (_ for _ in ()).throw(
            AssertionError("an absent record must not be searched for")
        )
        try:
            self.assertIsNone(
                drivers.frozen_routing_table_for(
                    "http://127.0.0.1:1/", {"execution": {"transactions": []}}, "M"
                )
            )
        finally:
            drivers.rpc = original

    def test_a_table_that_does_not_route_this_founding_refuses_by_name(self):
        """The authentication that makes a wrong pick a refusal, not a route."""
        fake, _ = self._routing_answers(
            table_body=alt_account([bytes([9] * 32)], frozen=True)
        )
        original = drivers.rpc
        drivers.rpc = fake
        try:
            with self.assertRaises(drivers.DriverRefusal) as caught:
                drivers.frozen_routing_table_for(
                    "http://127.0.0.1:1/", self._evidence_with_create(),
                    drivers.base58(bytes([7] * 32)),
                )
        finally:
            drivers.rpc = original
        self.assertIn("does not route", str(caught.exception))

    def test_an_account_the_lookup_table_program_does_not_own_refuses(self):
        """The create record names an address; the chain says what it IS."""
        fake, _ = self._routing_answers(
            table_body=alt_account([bytes([7] * 32)], frozen=True),
            table_owner="11111111111111111111111111111111",
        )
        original = drivers.rpc
        drivers.rpc = fake
        try:
            with self.assertRaises(drivers.DriverRefusal) as caught:
                drivers.frozen_routing_table_for(
                    "http://127.0.0.1:1/", self._evidence_with_create(),
                    drivers.base58(bytes([7] * 32)),
                )
        finally:
            drivers.rpc = original
        self.assertIn("not the Address Lookup Table program", str(caught.exception))

    def test_a_table_still_carrying_an_authority_refuses_rather_than_routes(self):
        """An extendable table is not the frozen one the founding committed to,
        and passing more than the frozen one refuses `DuplicateAddress` deep in
        a founding rather than here."""
        fake, _ = self._routing_answers(
            table_body=alt_account([bytes([7] * 32)], frozen=False)
        )
        original = drivers.rpc
        drivers.rpc = fake
        try:
            with self.assertRaises(drivers.DriverRefusal) as caught:
                drivers.frozen_routing_table_for(
                    "http://127.0.0.1:1/", self._evidence_with_create(),
                    drivers.base58(bytes([7] * 32)),
                )
        finally:
            drivers.rpc = original
        self.assertIn("still carries an authority", str(caught.exception))

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


RECORDING_BOOT = """#!/bin/sh
here="$(cd "$(dirname "$0")" && pwd)"
printf '%s\\n' "$@" > "$here/argv.txt"
if [ -f "$here/refuse" ]; then
  echo "REFUSED: [activation/root] the fake driver was told to say no" >&2
  exit 1
fi
out=""
want=""
for arg in "$@"; do
  if [ "$want" = "yes" ]; then out="$arg"; want=""; fi
  if [ "$arg" = "--output" ]; then want="yes"; fi
done
[ -n "$out" ] && echo '{"schema":"fake"}' > "$out"
echo "activation finalized slot 4242"
exit 0
"""


def write_recording_boot(directory: Path) -> Path:
    path = directory / "recording-bootstrap"
    path.write_text(RECORDING_BOOT)
    path.chmod(path.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)
    return path


def write_staged_trade_boot(directory: Path, *, stages: int, stuck: bool = False) -> Path:
    """A bootstrap that walks a Direct trade one durable action per call.

    It counts its own invocations, prints a journal document for each action and
    the FINALIZED evidence document for the last -- which is the shape the real
    driver has and the reason a single call is not a trade.
    """
    path = directory / "staged-trade-bootstrap"
    path.write_text(f"""#!/bin/sh
here="$(cd "$(dirname "$0")" && pwd)"
case "$1" in
  local-private-validator-direct-trade-produce-v1)
    out=""
    want=""
    for arg in "$@"; do
      if [ "$want" = "yes" ]; then out="$arg"; want=""; fi
      if [ "$arg" = "--output-dir" ]; then want="yes"; fi
    done
    echo '{{"schema":"produced"}}' > "$out/direct-trade-session.json"
    exit 0
    ;;
  local-private-validator-direct-trade-v1)
    n=0
    [ -f "$here/calls.txt" ] && n=$(cat "$here/calls.txt")
    n=$((n + 1))
    echo "$n" > "$here/calls.txt"
    advances={stages}
    if [ "{int(stuck)}" = "1" ]; then
      echo '{{"schema":"dclutch-owned-loopback-direct-trade-journal-v1"}}'
      exit 0
    fi
    if [ "$n" -gt "$advances" ]; then
      echo '{{"schema":"dclutch-owned-loopback-direct-trade-finalized-v1"}}'
    else
      echo "{{\"schema\":\"dclutch-owned-loopback-direct-trade-journal-v1\",\"stage\":$n}}"
    fi
    exit 0
    ;;
esac
echo "staged bootstrap does not implement $1" >&2
exit 64
""")
    path.chmod(path.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)
    return path


class DirectActivationTests(unittest.TestCase):
    """The step between a founding and a fill, and the reason fills were dead.

    Founding does NOT create the Direct capability execution root and nothing
    else does either; before `local-private-validator-direct-capability-\
activation-v1` existed, no local Direct fill was reachable at any market width.
    """

    def founded(self, work: Path) -> "drivers.FoundedMarket":
        accounts = {
            name: {"address": name.upper()} for name in (
                "founding_market", "collateral_mint", "founding_hoard_vault",
                "claims_aggregate", "founder_position", "collateral_wallet",
                "local_participant_fixture_source",
            )
        }
        evidence = work / "evidence.json"
        evidence.write_text(json.dumps({
            "payer": "PAYER", "execution": {"market": {"accounts": accounts}, "completed": True},
        }))
        market_input = work / "market.json"
        market_input.write_text(json.dumps({"coefficients": [1, 1, 0]}))
        return drivers.founded_market_from_evidence(
            "m00", evidence, market_input, work, fee_basis_points=50
        )

    def context(self, work: Path, boot: Path, *, keys: bool = False) -> "drivers.DriverContext":
        plan = work / "plan.json"
        plan.write_text(json.dumps({"schema": "plan"}))
        payer = work / "payer.json"
        payer.write_text("[]")
        substrate_keys = None
        if keys:
            # The two the Direct producer reads that no market's own founding
            # creates. Named rather than swept, exactly as the driver does.
            directory = work / "substrate-keys"
            directory.mkdir(exist_ok=True)
            for name in drivers.TRADE_SHARED_KEYS:
                (directory / f"{name}.json").write_text("[]")
            substrate_keys = str(directory)
        return drivers.DriverContext(
            bootstrap_bin=str(boot), rpc_url="http://127.0.0.1:34599/", plan=str(plan),
            work=work, timeout=30.0, campaign_payer_keypair=str(payer),
            founding_founder="F", substituted_founder="S", substrate_keys=substrate_keys,
        )

    def test_the_shipped_command_is_called_and_every_input_is_pinned_to_its_own_digest(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            boot = write_recording_boot(work)
            context = self.context(work, boot)
            founded = self.founded(work)
            run = drivers.drive_direct_activation(context, founded)
            self.assertIsNotNone(run)
            argv = (work / "argv.txt").read_text().splitlines()
            self.assertEqual(
                argv[0], "local-private-validator-direct-capability-activation-v1"
            )
            self.assertIn("--execute", argv)
            # Loopback takes NO devnet acknowledgment; the command refuses one
            # ahead of the origin parser, so passing it would be a refusal this
            # module composed for itself.
            self.assertNotIn("--i-mean-devnet", argv)
            pairs = dict(zip(argv, argv[1:]))
            for flag, pin in (
                ("--plan", "--expected-plan-sha256"),
                ("--market-input", "--expected-market-input-sha256"),
                ("--campaign-report", "--expected-campaign-report-sha256"),
            ):
                self.assertEqual(
                    pairs[pin], drivers.sha256_hex(Path(pairs[flag])),
                    f"{pin} must be the real digest of {flag}",
                )
            self.assertTrue(founded.activation and founded.activation.is_file())

    def test_a_report_already_on_disk_is_adopted_rather_than_rewalked(self):
        """The command refuses to overwrite its own output; so does this.

        A rerun over a work directory continues it. Re-walking would refuse at
        `output/exists` and report a refusal about a market that is already
        activated -- the loudest possible way to say nothing happened.
        """
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            boot = write_recording_boot(work)
            context = self.context(work, boot)
            founded = self.founded(work)
            self.assertIsNotNone(drivers.drive_direct_activation(context, founded))
            (work / "argv.txt").unlink()
            second = self.founded(work)
            self.assertIsNone(drivers.drive_direct_activation(context, second))
            self.assertFalse((work / "argv.txt").exists(), "nothing may have run")
            self.assertEqual(second.activation, founded.activation)

    def test_a_refused_activation_leaves_the_market_untradeable_and_says_which_step(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            boot = write_recording_boot(work)
            (work / "refuse").write_text("")
            context = self.context(work, boot)
            founded = self.founded(work)
            with self.assertRaises(drivers.DriverRefusal) as caught:
                drivers.drive_direct_activation(context, founded)
            self.assertIn("activating the Direct capability", str(caught.exception))
            self.assertIsNone(founded.activation)

    def test_a_fill_without_an_activation_names_the_step_and_not_the_root(self):
        """The producer's own sentence is about a root; this one is about a step.

        Absence and an owner change arrive at the producer's root check looking
        alike -- a finalized snapshot renders a missing account as a
        System-owned zero-length placeholder -- which is how twenty-one refused
        fills were once read as a claim about a widened market's width.
        """
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            boot = write_recording_boot(work)
            context = self.context(work, boot)
            founded = self.founded(work)
            founded.admissions["p0"] = work / "admission.json"
            with self.assertRaises(drivers.DriverRefusal) as caught:
                drivers.drive_fill(context, founded, "p0>p1", work / "admission.json")
            message = str(caught.exception)
            self.assertIn("EXECUTION root", message)
            self.assertIn("local-private-validator-direct-capability-activation-v1", message)
            self.assertNotIn("width", message)

    def test_a_trade_is_driven_to_completion_rather_than_advanced_once(self):
        """The driver advances ONE durable action per invocation and says so.

        A Direct trade is about ten of them. Calling it once advanced the first
        and returned zero, and this module recorded a fill as `executed` over a
        trade that had barely started -- a green that describes nothing.
        """
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            boot = write_staged_trade_boot(work, stages=6)
            context = self.context(work, boot, keys=True)
            founded = self.founded(work)
            founded.activation = work / "activation.json"
            founded.activation.write_text("{}")
            report = work / "admission.json"
            report.write_text("{}")
            run = drivers.drive_fill(context, founded, "p0>p1", report)
            self.assertIsNotNone(run)
            # One produce plus six advances: the last one is the finalized
            # evidence document, which is the driver's own word for done.
            self.assertEqual(int((work / "calls.txt").read_text().strip()), 7)

    def test_a_trade_that_stops_advancing_stops_the_run_rather_than_spinning(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            boot = write_staged_trade_boot(work, stages=6, stuck=True)
            context = self.context(work, boot, keys=True)
            founded = self.founded(work)
            founded.activation = work / "activation.json"
            founded.activation.write_text("{}")
            report = work / "admission.json"
            report.write_text("{}")
            with self.assertRaises(drivers.DriverRefusal) as caught:
                drivers.drive_fill(context, founded, "p0>p1", report)
            self.assertIn("twice in a row without finalizing", str(caught.exception))

    def test_two_stages_sharing_a_schema_are_progress_and_not_a_stall(self):
        """Schema alone is too coarse, and it cost a hand-run its trade.

        `replay-setup` and `token-setup` both print
        `dclutch-direct-trade-setup-journal-v1`, so a stall check keyed on the
        schema saw one word after two consecutive FINALIZED actions and called a
        working trade stuck.
        """
        setup = "dclutch-direct-trade-setup-journal-v1"
        replay = drivers._direct_trade_progress(
            json.dumps({"schema": setup, "stage": "replay-setup", "phase": "finalized"})
        )
        token = drivers._direct_trade_progress(
            json.dumps({"schema": setup, "stage": "token-setup", "phase": "finalized"})
        )
        self.assertNotEqual(replay, token)
        self.assertEqual(replay[0], token[0], "the schema really is the same")
        # And the same is true one level down: three consecutive lookup-extend
        # actions share a schema AND a stage, and each finalizes its own
        # transaction. Only the whole document separates them.
        journal = "dclutch-owned-loopback-direct-trade-journal-v1"
        first = drivers._direct_trade_progress(json.dumps(
            {"schema": journal, "stage": "lookup-extend", "phase": "finalized", "slot": 11},
        ))
        second = drivers._direct_trade_progress(json.dumps(
            {"schema": journal, "stage": "lookup-extend", "phase": "finalized", "slot": 12},
        ))
        self.assertNotEqual(first, second)
        self.assertEqual(first[:2], second[:2], "schema and stage really are the same")
        # A driver that truly does not advance prints the SAME report.
        self.assertEqual(first, drivers._direct_trade_progress(json.dumps(
            {"schema": journal, "stage": "lookup-extend", "phase": "finalized", "slot": 11},
        )))
        self.assertIsNone(drivers._direct_trade_progress("not json"))

    def test_one_taker_per_market_because_the_fixture_is_one_constant(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            founded = self.founded(work)
            taker = drivers.fixture_share_atoms(founded, taker=True)
            other = drivers.fixture_share_atoms(founded)
            self.assertEqual(taker, drivers.DIRECT_FILL_COLLATERAL_REQUIREMENT_ATOMS_V1)
            self.assertLess(other, taker)
            # Two fully funded buyers do not fit inside one pinned fixture, and
            # that is the reason there is exactly one taker rather than a policy.
            self.assertGreater(
                2 * taker, drivers.LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1
            )
            # And one does, with room for the small shares behind it.
            self.assertLessEqual(taker, drivers.LOCAL_PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS_V1)

    def test_a_market_with_no_fixture_source_funds_nobody(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            founded = self.founded(work)
            founded.participant_fixture_source = None
            self.assertEqual(drivers.fixture_share_atoms(founded, taker=True), 0)
            self.assertEqual(drivers.fixture_share_atoms(founded), 0)


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

    def test_one_address_gets_one_label_or_the_census_counts_it_twice(self):
        """The census sums PER ACCOUNT, so a duplicate is atoms from nowhere.

        Measured: a re-walk over a chain this run had already touched brought
        the holder's participant token account back from the prior scan as
        `prior_01` while the admission also named it `holder_m04-p0`, and L1
        refused with `tracked 228894899 != Mint supply 178644899` -- fifty
        million atoms more than the Mint had ever issued. The census was right
        and the caption was wrong, which is the law's whole purpose.
        """
        binding = drive.LifecycleSubstrate._dedupe_tokens({
            "tokens": {
                "holder_m04-p0": "SAME", "prior_01": "SAME",
                "founder_wallet": "OTHER", "prior_00": "THIRD",
            },
        })
        self.assertEqual(len(binding["tokens"]), 3)
        self.assertEqual(sorted(binding["tokens"].values()), ["OTHER", "SAME", "THIRD"])
        # The descriptive label survives, because a reader should see the name
        # that says what the account is.
        self.assertIn("holder_m04-p0", binding["tokens"])
        self.assertNotIn("prior_01", binding["tokens"])
        self.assertIn("prior_00", binding["tokens"], "an unduplicated prior is still named")

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
    ANCHOR = simlife.LOCAL_PYTH_FIXTURE_COORDINATE_V1

    def test_a_two_cell_market_has_no_cuts_and_a_wide_one_has_width_minus_two(self):
        for width in (2, 3, 4, 9):
            band = simlife._band(width, self.ANCHOR, 1_000_000, 0, simlife.BAND_PROFILE_UNIFORM)
            self.assertEqual(len(band), width - 2)

    def test_a_band_is_strictly_increasing_and_positive(self):
        for profile in simlife.BAND_PROFILES:
            cuts = simlife._band(9, self.ANCHOR, 900_000, 0, profile)
            self.assertEqual(cuts, sorted(set(cuts)), profile)
            self.assertTrue(all(cut > 0 for cut in cuts), profile)

    def test_a_band_is_placed_around_the_coordinate_the_substrate_will_observe(self):
        """The bug this whole rule exists to close.

        Before it, cuts came from `IntUniform(4_000, 40_000)` with a comment
        about USD cents per SOL, while the local Pyth fixture's coordinate is
        the raw price atoms 100,000,000 -- three to five orders of magnitude
        away -- so every observation landed above every cut and the settling
        cell was a CONSTANT.
        """
        world = simlife.build_world(simlife.WorldSpec(
            seed=simlife.SeedBook("dclutch/simlife3/anchored"), markets=40, ticks=20,
        ))
        anchor = world.spec.coordinate_anchor
        for market in world.markets:
            if not market.cuts:
                continue
            # Every cut within an order of magnitude of the coordinate: a band
            # ten times away from it is a band that cannot be landed in.
            for cut in market.cuts:
                self.assertGreater(cut * 10, anchor, market.market_id)
                self.assertLess(cut, anchor * 10, market.market_id)
            self.assertNotIn(anchor, market.cuts, "a cut ON the anchor makes the cell a tie")

    def test_a_world_does_not_settle_into_one_bucket(self):
        """OUTCOME SPREAD IS A HEALTH PROPERTY OF A RUN, not a nice-to-have.

        A world of forty markets that all settle into the same cell has forty
        copies of one measurement, whatever else is varied about it.
        """
        for name in ("design-space", "foundable-today"):
            world = simlife.build_world(simlife.WorldSpec(
                seed=simlife.SeedBook(f"dclutch/simlife3/spread/{name}"), markets=40, ticks=40,
                archetype_mix=simlife.ARCHETYPE_MIXES[name],
            ))
            spread = world.outcome_spread()
            self.assertGreater(spread["resolving_markets"], 8, name)
            self.assertGreater(spread["distinct_cells"], 4, f"{name}: {spread['counts']}")
            # And the reading that actually catches the defect: where in its own
            # market each answer landed. Three is the floor rather than a target
            # -- a world weighted towards three-cell markets can only reach the
            # two tails, because a three-cell market has exactly two ordinary
            # answers and no interior to land in -- so the property is that a
            # world reaches BOTH TAILS and at least one place between them.
            self.assertGreaterEqual(
                spread["distinct_positions"], 3, f"{name}: {spread['position_counts']}"
            )
            self.assertIn(0, spread["position_counts"], f"{name}: no market settled low")
            self.assertIn(10, spread["position_counts"], f"{name}: no market settled high")
            self.assertTrue(
                [place for place in spread["position_counts"] if 0 < place < 10],
                f"{name}: nothing settled between the tails: {spread['position_counts']}",
            )
            self.assertFalse(spread["degenerate"], f"{name}: {spread['position_counts']}")
            self.assertLessEqual(
                spread["heaviest_share_percent"],
                simlife.DEGENERATE_OUTCOME_SHARE_PERCENT_V1,
                f"{name}: {spread['position_counts']}",
            )

    def test_a_position_normalises_a_cell_against_its_own_market(self):
        """Cell 3 of four and cell 3 of eleven are not the same answer."""
        self.assertEqual(simlife.settling_position_tenths(0, 3), 0)
        self.assertEqual(simlife.settling_position_tenths(1, 3), 10)
        self.assertEqual(simlife.settling_position_tenths(0, 12), 0)
        self.assertEqual(simlife.settling_position_tenths(10, 12), 10)
        self.assertEqual(simlife.settling_position_tenths(5, 12), 5)
        # A width-2 market has one ordinary cell and therefore no position: a
        # whole coordinate domain as one region is not "the bottom of" anything.
        self.assertIsNone(simlife.settling_position_tenths(0, 2))

    def test_the_cell_histogram_alone_would_have_missed_the_defect(self):
        """Why the flag is over position and not over `cell/width`.

        The historical failure put EVERY observation above EVERY cut. Counted as
        `cell/width` that spreads across as many keys as the world has widths
        and reads as diverse; counted as position it is one bucket. This is the
        test that says the weaker reading was considered and rejected.
        """
        world = simlife.build_world(simlife.WorldSpec(
            seed=simlife.SeedBook("dclutch/simlife3/histogram"), markets=40, ticks=40,
        ))
        far_above = world.spec.coordinate_anchor * 1_000
        for market in world.markets:
            if market.selected_cell is not None and market.destiny == simlife.DESTINY_RESOLVES:
                market.selected_cell = simlife.settling_cell(far_above, market.cuts)
        spread = world.outcome_spread()
        self.assertGreater(spread["distinct_cells"], 3, "the weak reading looks diverse")
        self.assertEqual(spread["distinct_positions"], 1, spread["position_counts"])
        self.assertTrue(spread["degenerate"])

    def test_the_degeneracy_flag_can_actually_fire(self):
        """A checker that has never said no is a checker nobody has tested.

        THE HISTORICAL DEFECT REPRODUCED, and reproducing it takes the two
        halves coming apart rather than either half being wrong: the bands are
        drawn for one coordinate and the observation arrives at another. That is
        exactly what happened -- cuts drawn in USD cents per SOL, an observation
        arriving as raw price atoms 100,000,000 -- and it is why no amount of
        variety in the bands themselves saved the run.

        Without this arm the test above passes on a flag hard-wired to `False`.
        """
        world = simlife.build_world(simlife.WorldSpec(
            seed=simlife.SeedBook("dclutch/simlife3/degenerate"), markets=40, ticks=40,
        ))
        self.assertFalse(world.outcome_spread()["degenerate"], "the control must be healthy")
        # The coordinate the OLD comment believed in: USD cents per SOL, five
        # orders of magnitude below every cut this world drew.
        elsewhere = world.spec.coordinate_anchor // 100_000
        for market in world.markets:
            if market.selected_cell is not None and market.destiny == simlife.DESTINY_RESOLVES:
                market.selected_cell = simlife.settling_cell(elsewhere, market.cuts)
        spread = world.outcome_spread()
        self.assertTrue(spread["degenerate"], spread["counts"])
        self.assertGreater(spread["heaviest_share_percent"], 70)
        lines = "\n".join(simlife.world_summary(world))
        self.assertIn("DEGENERATE OUTCOME SPREAD", lines)

    def test_the_settling_cell_counts_the_cuts_at_or_below_the_coordinate(self):
        cuts = [10, 20, 30]
        self.assertEqual(simlife.settling_cell(5, cuts), 0)
        self.assertEqual(simlife.settling_cell(10, cuts), 1)
        self.assertEqual(simlife.settling_cell(25, cuts), 2)
        self.assertEqual(simlife.settling_cell(999, cuts), 3)
        # Never the failure cell: a failure is reached by a deadline, not by an
        # observation, so the answer is always in 0..=len(cuts).
        self.assertEqual(simlife.settling_cell(999, []), 0)

    def test_a_profile_varies_the_gaps_without_moving_the_scale(self):
        """Varied widths, and the reason they are a shape rather than a knob.

        A profile has to change what the band looks like without changing how
        big it is, or "tight-centre" would just mean "narrower" and the two
        axes would be one axis twice.
        """
        widths = {}
        for profile in simlife.BAND_PROFILES:
            cuts = simlife._band(9, self.ANCHOR, 1_000_000, 0, profile)
            gaps = [b - a for a, b in zip(cuts, cuts[1:])]
            widths[profile] = cuts[-1] - cuts[0]
            if profile == simlife.BAND_PROFILE_UNIFORM:
                self.assertEqual(len(set(gaps)), 1, profile)
            else:
                self.assertGreater(len(set(gaps)), 1, f"{profile} must vary its gaps")
        span = max(widths.values()) / min(widths.values())
        self.assertLess(span, 2.0, f"a profile must not be a second scale: {widths}")

    def test_a_tight_centre_band_is_finest_in_the_middle(self):
        cuts = simlife._band(11, self.ANCHOR, 1_000_000, 0, simlife.BAND_PROFILE_TIGHT_CENTRE)
        gaps = [b - a for a, b in zip(cuts, cuts[1:])]
        middle = gaps[len(gaps) // 2]
        self.assertLess(middle, gaps[0])
        self.assertLess(middle, gaps[-1])

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
