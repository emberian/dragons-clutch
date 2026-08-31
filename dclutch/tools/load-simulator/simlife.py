#!/usr/bin/env python3
"""Market life for the dClutch load simulator: many markets at once, each with
its own personality, driven by participants who are not all the same person.

WHAT WAS MISSING.  `simulator.py` is a faithful sustain loop around ONE market
and ONE canned walk: a fixed pair list cycled modulo the cycle number, a census
every cycle, and a status artifact.  Every quantity it observes is therefore
expected to hold still, which is why the site's only time axis drew four
identical bands (`docs/evidence/SIMULATOR_SERIES_VIZ_2026_08_30.md`).  Nothing
about that was dishonest.  It simply had no LIFE in it: no second market with a
different shape, no participant who behaves unlike the last one, no market that
fails rather than resolving, nobody asleep at the moment their claim comes due.

WHAT THIS IS.  A seeded world generator and a conductor.  The generator draws a
population of markets from ARCHETYPES and a population of participants from
PERSONAS, both parameterised by named distributions, and interleaves their
lifecycles into one ordered event schedule.  The conductor walks that schedule
against a SUBSTRATE, records what each event actually did, and re-censuses every
live market at every tick through the existing conservation ledger.

THE LINE THIS MODULE WILL NOT CROSS, and the whole reason it is shaped this way:

    the engine decides WHAT to attempt and WHEN.  The census decides WHAT IS
    TRUE.

No number this module invents ever reaches a series point.  A planned event that
executed contributes its signature and its slot; a planned event the substrate
could not route contributes the word `unattempted` and the reason; a planned
event whose prerequisite never executed contributes `blocked`.  The market
quantities on every series point come from `ledger-census` observing accounts on
a chain, exactly as they did before.  A world plan is a plan, and a plan is not
evidence.

REPRODUCIBILITY.  Everything is drawn from one recorded seed PREIMAGE (a
sentence, so a run can be named and re-run by typing its name) through named,
INDEPENDENT streams.  Independence is the property worth paying for: with one
shared `Random`, adding a market or reordering a persona reshuffles every draw
after it, so no two runs of a changed world are ever comparable.  Here each draw
site derives its own generator from `sha256(seed || domain || index)`, so market
7's deadline is the same number whether the world holds eight markets or eighty.

WHAT A PLAN RECORDS.  Not just the value drawn but the DISTRIBUTION it came
from.  `deadline_slots = 4096` says nothing about whether that was inevitable;
`log-uniform over [512, 32768] -> 4096` says what the world was actually like.
Every draw crosses into the plan as both.
"""

from __future__ import annotations

import dataclasses
import hashlib
import json
import math
from pathlib import Path
import random
import sys
from typing import Any, Callable, Optional, Sequence

HERE = Path(__file__).resolve().parent
if str(HERE) not in sys.path:
    sys.path.insert(0, str(HERE))
import simcore  # noqa: E402

SCHEMA_WORLD = "dclutch-simlife-world-v1"
SCHEMA_LEDGER = "dclutch-simlife-ledger-v1"

# ---------------------------------------------------------------------------
# Routes: the named things a substrate may or may not be able to do.
#
# A route is not a driver and not an instruction. It is the smallest unit of
# market life the conductor knows how to ask for, and every substrate answers
# the same question about each one: can you do this, yes or no. A substrate
# that says no is not broken and its events are not failures -- they are
# `unattempted`, which is a third word this module needs and refuses to spell
# as either of the other two.
# ---------------------------------------------------------------------------

ROUTE_FOUND = "found"
ROUTE_ADMIT = "admit"
ROUTE_FILL = "fill"
ROUTE_RESOLVE = "resolve"
ROUTE_DEADLINE_FAILURE = "deadline-failure"
ROUTE_REDEEM = "redeem"
ROUTE_COMPACT = "compact"
ROUTE_RETIRE = "retire"
ROUTE_CENSUS = "census"

ALL_ROUTES = (
    ROUTE_FOUND,
    ROUTE_ADMIT,
    ROUTE_FILL,
    ROUTE_RESOLVE,
    ROUTE_DEADLINE_FAILURE,
    ROUTE_REDEEM,
    ROUTE_COMPACT,
    ROUTE_RETIRE,
    ROUTE_CENSUS,
)

# How one planned event ended. Four words, and the distinction between the last
# three is the whole honesty of the artifact.
OUTCOME_EXECUTED = "executed"      # a transaction landed, or an account was read
OUTCOME_REFUSED = "refused"        # the route exists and the chain said no
OUTCOME_UNATTEMPTED = "unattempted"  # the substrate has no such route
OUTCOME_BLOCKED = "blocked"        # a prerequisite of this event did not execute

TERMINAL_OUTCOMES = (OUTCOME_EXECUTED, OUTCOME_REFUSED, OUTCOME_UNATTEMPTED, OUTCOME_BLOCKED)


class Refusal(RuntimeError):
    """A world that cannot be built, or a substrate that cannot be trusted."""


# ---------------------------------------------------------------------------
# Seeds
# ---------------------------------------------------------------------------


@dataclasses.dataclass(frozen=True)
class SeedBook:
    """One run seed, and an independent generator per named draw site.

    The preimage is a SENTENCE rather than an integer on purpose: a run that can
    be named (`dclutch/simlife/2026-08-30/first-light`) can be re-run by typing
    its name, and two runs that differ only in their seed are told apart by a
    reader rather than by a hex digest.  The digest is recorded beside it so the
    artifact still has one exact identifier.

    `stream(domain, index)` is `sha256(preimage || 0x00 || domain || 0x00 ||
    index)` reduced to a 256-bit seed.  The separators matter: without them
    `("market", 12)` and `("market1", 2)` would be the same stream, and two draw
    sites would silently share a sequence.
    """

    preimage: str

    @property
    def digest(self) -> str:
        return hashlib.sha256(self.preimage.encode("utf-8")).hexdigest()

    def stream(self, domain: str, index: int = 0) -> random.Random:
        material = b"\x00".join(
            (self.preimage.encode("utf-8"), domain.encode("utf-8"), str(index).encode("ascii"))
        )
        return random.Random(int.from_bytes(hashlib.sha256(material).digest(), "big"))

    def describe(self) -> dict:
        return {"preimage": self.preimage, "sha256": self.digest}


# ---------------------------------------------------------------------------
# Distributions
#
# Each one draws a value and can DESCRIBE itself. The description is what turns
# a plan from a list of numbers into a statement about the world those numbers
# came out of, and it is the difference between "deadline 4096" and "log-uniform
# over 512..32768, which is where 4096 came from".
# ---------------------------------------------------------------------------


class Distribution:
    kind = "distribution"

    def draw(self, rng: random.Random) -> Any:  # pragma: no cover - abstract
        raise NotImplementedError

    def describe(self) -> dict:  # pragma: no cover - abstract
        raise NotImplementedError


@dataclasses.dataclass(frozen=True)
class Constant(Distribution):
    """A value the world does not vary. Recorded as a distribution anyway, so a
    reader can see that it was FIXED rather than that it happened to come out
    the same twice."""

    value: Any
    kind: str = "constant"

    def draw(self, rng: random.Random) -> Any:
        return self.value

    def describe(self) -> dict:
        return {"kind": self.kind, "value": self.value}


@dataclasses.dataclass(frozen=True)
class IntUniform(Distribution):
    """Inclusive on both ends, because an exclusive upper bound in a config file
    is a bug nobody sees until the maximum never occurs."""

    low: int
    high: int
    kind: str = "int-uniform"

    def __post_init__(self) -> None:
        if self.high < self.low:
            raise Refusal(f"int-uniform needs low <= high, got [{self.low}, {self.high}]")

    def draw(self, rng: random.Random) -> int:
        return rng.randint(self.low, self.high)

    def describe(self) -> dict:
        return {"kind": self.kind, "low": self.low, "high": self.high}


@dataclasses.dataclass(frozen=True)
class LogIntUniform(Distribution):
    """Uniform in the LOGARITHM, rounded to an integer.

    For every quantity in this world that spans orders of magnitude -- deadlines
    in slots, collateral in atoms, a burst's size.  A linear draw over
    [512, 32768] puts nine tenths of its mass above 3,000 and a world drawn that
    way has no short-fuse markets in it at all, which is exactly the
    heterogeneity this module exists to produce.
    """

    low: int
    high: int
    kind: str = "log-int-uniform"

    def __post_init__(self) -> None:
        if self.low < 1:
            raise Refusal("log-int-uniform needs a positive low bound")
        if self.high < self.low:
            raise Refusal(f"log-int-uniform needs low <= high, got [{self.low}, {self.high}]")

    def draw(self, rng: random.Random) -> int:
        drawn = math.exp(rng.uniform(math.log(self.low), math.log(self.high)))
        return max(self.low, min(self.high, int(round(drawn))))

    def describe(self) -> dict:
        return {"kind": self.kind, "low": self.low, "high": self.high}


@dataclasses.dataclass(frozen=True)
class Categorical(Distribution):
    """A weighted choice over named options.

    Weights are integers, not floats: a world config is read and edited by
    people, and `3` beside `1` is a statement anyone can check, where
    `0.7499999` is a rounding argument.
    """

    options: tuple  # tuple[tuple[str, int], ...]
    kind: str = "categorical"

    def __post_init__(self) -> None:
        if not self.options:
            raise Refusal("a categorical distribution needs at least one option")
        for name, weight in self.options:
            if not isinstance(weight, int) or weight < 1:
                raise Refusal(f"categorical option {name!r} needs a positive integer weight")

    def draw(self, rng: random.Random) -> str:
        total = sum(weight for _name, weight in self.options)
        cut = rng.randint(1, total)
        running = 0
        for name, weight in self.options:
            running += weight
            if cut <= running:
                return name
        raise Refusal("categorical draw fell off the end, which cannot happen")

    def describe(self) -> dict:
        return {"kind": self.kind, "options": [{"name": n, "weight": w} for n, w in self.options]}


@dataclasses.dataclass(frozen=True)
class Bernoulli(Distribution):
    """A coin with an integer-percent bias, for the same reason weights are
    integers."""

    percent: int
    kind: str = "bernoulli"

    def __post_init__(self) -> None:
        if not 0 <= self.percent <= 100:
            raise Refusal("a bernoulli needs a percentage in [0, 100]")

    def draw(self, rng: random.Random) -> bool:
        return rng.randint(1, 100) <= self.percent

    def describe(self) -> dict:
        return {"kind": self.kind, "percent": self.percent}


@dataclasses.dataclass(frozen=True)
class DirichletSplit(Distribution):
    """A split of one exact integer total into `parts`, drawn from a Dirichlet.

    WHY NOT EQUAL SHARES.  A cohort where everyone stakes the same amount is a
    cohort with no story in it; the interesting picture is one participant
    holding most of a market and three holding the rest.  `concentration` is the
    Dirichlet's symmetric alpha as a percent: below 100 the mass piles onto one
    or two participants, at 100 the split is uniform over the simplex, well
    above 100 everyone converges on an equal share.

    EXACT.  The total is conserved: shares are floored and the remainder is
    handed out largest-fractional-part first, so the parts sum to the total
    exactly and no atom is invented or lost by rounding.  A part is never zero,
    because a participant with no stake is not a participant.
    """

    parts: int
    concentration_percent: int = 100
    kind: str = "dirichlet-split"

    def __post_init__(self) -> None:
        if self.parts < 1:
            raise Refusal("a split needs at least one part")
        if self.concentration_percent < 1:
            raise Refusal("concentration must be a positive percent")

    def split(self, rng: random.Random, total: int) -> list:
        if total < self.parts:
            raise Refusal(
                f"cannot split {total} into {self.parts} non-empty parts; "
                "a participant with no stake is not a participant"
            )
        alpha = self.concentration_percent / 100.0
        weights = [rng.gammavariate(alpha, 1.0) for _ in range(self.parts)]
        # A Dirichlet draw of all-zeros is possible in principle at tiny alpha
        # and would divide by zero; fall back to the uniform split rather than
        # crash a whole world on a measure-zero event.
        mass = sum(weights)
        if mass <= 0.0:
            weights = [1.0] * self.parts
            mass = float(self.parts)
        # Reserve one atom per part first, then split what is left. This is what
        # makes "never zero" a property rather than a hope.
        spare = total - self.parts
        exact = [weight / mass * spare for weight in weights]
        shares = [1 + int(math.floor(value)) for value in exact]
        remainder = total - sum(shares)
        order = sorted(range(self.parts), key=lambda i: (-(exact[i] % 1.0), i))
        for position in range(remainder):
            shares[order[position % self.parts]] += 1
        if sum(shares) != total:
            raise Refusal("split did not conserve its total, which is a defect here")
        return shares

    def draw(self, rng: random.Random) -> list:
        return self.split(rng, self.parts)

    def describe(self) -> dict:
        return {
            "kind": self.kind,
            "parts": self.parts,
            "concentration_percent": self.concentration_percent,
        }


def draw_recorded(name: str, distribution: Distribution, rng: random.Random) -> tuple:
    """One draw, and the record of it. Returns `(value, record)`."""
    value = distribution.draw(rng)
    return value, {"name": name, "drawn": value, "from": distribution.describe()}


# ---------------------------------------------------------------------------
# Market archetypes
# ---------------------------------------------------------------------------

# THE BASIS SHAPES, AND THE ONE THAT CAN ACTUALLY BE FOUNDED TODAY.
#
# `BasisKindV3` (crates/dclutch-product-payoff-v2-codec/src/runtime_v3.rs:122)
# admits `CategoricalQ1` -- the runtime-width indicator basis, degree 0 -- and
# `GradedExactComplement`, whose term shapes are `Constant`, `RampUp`,
# `RampDown` and `Tent` (:177-203). Degrees 0 and 1 are exempt from the price
# gate by proof, so the graded shapes are decodable, evaluable and
# terminal-settleable on today's wire.
#
# THEY ARE NOT FOUNDABLE. `compile_linked_basis_v3`
# (tools/local-validator/bootstrap/successor/src/market.rs:1683) hard-wires
# `kind: CategoricalQ1, payout_scale: 1`, zero knots and zero terms, and
# founding refuses anything else outright at market.rs:3487. All four
# capability compilers -- Direct, General, Rational, Structured -- funnel
# through the same base, so every market this repository can found is a
# categorical one. Every construction site for a graded basis outside that path
# is inside a `#[cfg(test)]` module.
#
# So these three names are kept and drawn from anyway, because an archetype
# table that only contains what today's compiler emits is a table that cannot
# say what is missing. A substrate declares which kinds it can EXPRESS
# (`Substrate.basis_kinds`), and a founding it cannot express is `unattempted`
# with that sentence -- never a failure, and never quietly redrawn as a
# categorical market wearing a ladder's name.
BASIS_CATEGORICAL = "categorical-degree-0"
BASIS_RAMP = "ramp-degree-1"
BASIS_TENT = "tent-degree-1"

ALL_BASIS_KINDS = (BASIS_CATEGORICAL, BASIS_RAMP, BASIS_TENT)

# One sentence per shape a substrate does not have, said where the fact lives.
BASIS_ABSENCE = {
    BASIS_RAMP: (
        "a degree-1 ramp basis decodes and settles on today's wire but no founding "
        "driver emits one: compile_linked_basis_v3 (market.rs:1683) hard-wires "
        "CategoricalQ1 and founding refuses any other kind at market.rs:3487"
    ),
    BASIS_TENT: (
        "a tent is two degree-1 ramps and reaches the wire the same way a ramp does "
        "-- through GradedExactComplement, which the local founding compiler never "
        "emits (market.rs:1683)"
    ),
}

# What a market is FOR, in the world's own terms. A destiny is not a prediction
# about the chain -- it is what the conductor will try to drive, and the chain
# is free to refuse it.
DESTINY_RESOLVES = "resolves-clean"
DESTINY_FAILS = "commit-deadline-failure"
DESTINY_SLEEPY = "founded-then-sleepy"


@dataclasses.dataclass(frozen=True)
class MarketArchetype:
    """One KIND of market, as a bundle of distributions rather than as a market.

    An archetype is drawn from, never instantiated directly: two markets of the
    same archetype differ in every number, and that is the point.  The fee rate
    is a `Constant(0)` on every archetype in this module and the comment beside
    it says why -- fee-bearing founding does not fit in one transaction today
    (`docs/evidence/FEE_SECOND_TRANSACTION_FOUNDATION_2026_08_30.md`), and a
    world that draws a nonzero rate would be a world whose markets cannot be
    founded at all.
    """

    name: str
    blurb: str
    outcomes: Distribution
    basis: Distribution
    deadline_slots: Distribution
    destiny: Distribution
    claim_unit_atoms: Distribution
    founding_collateral_atoms: Distribution
    participants: Distribution
    stake_concentration_percent: Distribution
    # Fills the archetype attempts across its open window, when the substrate
    # has a fill route at all.
    fill_bursts: Distribution
    fills_per_burst: Distribution
    # Zero-fee only. See the class docstring.
    fee_basis_points: Distribution = dataclasses.field(default_factory=lambda: Constant(0))

    def describe(self) -> dict:
        return {
            "name": self.name,
            "blurb": self.blurb,
            "distributions": {
                "outcomes": self.outcomes.describe(),
                "basis": self.basis.describe(),
                "deadline_slots": self.deadline_slots.describe(),
                "destiny": self.destiny.describe(),
                "claim_unit_atoms": self.claim_unit_atoms.describe(),
                "founding_collateral_atoms": self.founding_collateral_atoms.describe(),
                "participants": self.participants.describe(),
                "stake_concentration_percent": self.stake_concentration_percent.describe(),
                "fill_bursts": self.fill_bursts.describe(),
                "fills_per_burst": self.fills_per_burst.describe(),
                "fee_basis_points": self.fee_basis_points.describe(),
            },
        }


ARCHETYPES: tuple = (
    MarketArchetype(
        name="coin-flip",
        blurb="Two outcomes, an indicator basis, and a deadline in the middle "
              "distance. The plainest market this protocol can hold, and the "
              "one every other archetype is a departure from.",
        outcomes=Constant(2),
        basis=Constant(BASIS_CATEGORICAL),
        deadline_slots=LogIntUniform(2_000, 20_000),
        destiny=Categorical(((DESTINY_RESOLVES, 8), (DESTINY_FAILS, 1), (DESTINY_SLEEPY, 1))),
        claim_unit_atoms=Constant(1),
        founding_collateral_atoms=LogIntUniform(100_000_000, 2_000_000_000),
        participants=IntUniform(2, 4),
        stake_concentration_percent=Constant(100),
        fill_bursts=IntUniform(1, 3),
        fills_per_burst=IntUniform(1, 3),
    ),
    MarketArchetype(
        name="short-fuse",
        blurb="A deadline close enough that missing it is the likely story. "
              "This is the archetype that exercises the failure branch on "
              "purpose, and the one whose census window is shortest.",
        # Two to four, because a market's WIDTH has nothing to do with its
        # fuse, and because four is a width this repository actually founds:
        # the graduation-market planner produced one on 2026-08-30 and no
        # archetype in the first draft of this table could describe it.
        outcomes=IntUniform(2, 4),
        basis=Constant(BASIS_CATEGORICAL),
        deadline_slots=LogIntUniform(120, 1_200),
        destiny=Categorical(((DESTINY_FAILS, 6), (DESTINY_RESOLVES, 3), (DESTINY_SLEEPY, 1))),
        claim_unit_atoms=Constant(1),
        founding_collateral_atoms=LogIntUniform(10_000_000, 200_000_000),
        participants=IntUniform(2, 3),
        stake_concentration_percent=Constant(60),
        fill_bursts=IntUniform(1, 2),
        fills_per_burst=IntUniform(1, 2),
    ),
    MarketArchetype(
        name="ladder",
        blurb="A banded numeric domain read through a degree-1 ramp: the payout "
              "rises across the band rather than switching at its edge. Wide, "
              "slow, and the archetype whose odds path has somewhere to go.",
        outcomes=IntUniform(4, 8),
        basis=Constant(BASIS_RAMP),
        deadline_slots=LogIntUniform(8_000, 60_000),
        destiny=Categorical(((DESTINY_RESOLVES, 7), (DESTINY_SLEEPY, 2), (DESTINY_FAILS, 1))),
        claim_unit_atoms=Categorical((("1", 3), ("10", 1))),
        founding_collateral_atoms=LogIntUniform(400_000_000, 4_000_000_000),
        participants=IntUniform(3, 6),
        stake_concentration_percent=Constant(45),
        fill_bursts=IntUniform(2, 5),
        fills_per_burst=IntUniform(1, 4),
    ),
    MarketArchetype(
        name="tent-band",
        blurb="A peaked domain: two ramps meeting at the band the market thinks "
              "is likeliest. Same degree-1 vocabulary as the ladder, a payout "
              "shape that is not monotone.",
        outcomes=IntUniform(3, 6),
        basis=Constant(BASIS_TENT),
        deadline_slots=LogIntUniform(4_000, 30_000),
        destiny=Categorical(((DESTINY_RESOLVES, 6), (DESTINY_FAILS, 2), (DESTINY_SLEEPY, 2))),
        claim_unit_atoms=Constant(1),
        founding_collateral_atoms=LogIntUniform(200_000_000, 2_000_000_000),
        participants=IntUniform(3, 5),
        stake_concentration_percent=Constant(70),
        fill_bursts=IntUniform(1, 4),
        fills_per_burst=IntUniform(1, 3),
    ),
    MarketArchetype(
        name="wide-field",
        blurb="Many mutually exclusive answers and a long horizon: the "
              "archetype where the odds vector is worth drawing as a shape "
              "rather than as a pair of numbers.",
        outcomes=IntUniform(6, 12),
        basis=Constant(BASIS_CATEGORICAL),
        deadline_slots=LogIntUniform(10_000, 80_000),
        destiny=Categorical(((DESTINY_RESOLVES, 5), (DESTINY_SLEEPY, 4), (DESTINY_FAILS, 1))),
        claim_unit_atoms=Constant(1),
        founding_collateral_atoms=LogIntUniform(600_000_000, 6_000_000_000),
        participants=IntUniform(4, 8),
        stake_concentration_percent=Constant(35),
        fill_bursts=IntUniform(2, 6),
        fills_per_burst=IntUniform(1, 5),
    ),
    MarketArchetype(
        name="quiet-corner",
        blurb="Founded and then left alone. Nobody trades it, nobody resolves "
              "it inside this run's horizon, and the census watches it hold "
              "still -- which is a real thing markets do and the control every "
              "other archetype is measured against.",
        outcomes=Constant(2),
        basis=Constant(BASIS_CATEGORICAL),
        deadline_slots=LogIntUniform(40_000, 200_000),
        destiny=Constant(DESTINY_SLEEPY),
        claim_unit_atoms=Constant(1),
        founding_collateral_atoms=LogIntUniform(50_000_000, 500_000_000),
        participants=IntUniform(1, 2),
        stake_concentration_percent=Constant(100),
        fill_bursts=Constant(0),
        fills_per_burst=Constant(0),
    ),
)

ARCHETYPES_BY_NAME = {archetype.name: archetype for archetype in ARCHETYPES}

# How often each archetype comes up when a world draws a market. Deliberately
# not uniform: a world that is one sixth short-fuse markets is a world whose
# failure branch is exercised without being the whole story.
DEFAULT_ARCHETYPE_MIX = Categorical((
    ("coin-flip", 4),
    ("short-fuse", 2),
    ("ladder", 3),
    ("tent-band", 2),
    ("wide-field", 2),
    ("quiet-corner", 2),
))

# The same world, restricted to the markets a real substrate can actually
# express today. `ladder` and `tent-band` are absent because their basis is not
# foundable (see BASIS_ABSENCE); nothing else about them is wrong, and the day a
# founding driver emits a graded basis this preset should be deleted rather than
# edited.
FOUNDABLE_ARCHETYPE_MIX = Categorical((
    ("coin-flip", 4),
    ("short-fuse", 2),
    ("wide-field", 3),
    ("quiet-corner", 2),
))

ARCHETYPE_MIXES = {
    "design-space": DEFAULT_ARCHETYPE_MIX,
    "foundable-today": FOUNDABLE_ARCHETYPE_MIX,
}


# ---------------------------------------------------------------------------
# Participant personas
# ---------------------------------------------------------------------------


@dataclasses.dataclass(frozen=True)
class Persona:
    """One KIND of participant, as a bundle of distributions.

    `redeems` is the field that earns its keep.  A holder who never redeems is
    not an inactive account -- their claim check sits on the chain occupying
    rent that somebody else can recover by compacting it, and that compaction is
    a permissionless act by a stranger.  A world with no sleepers never
    exercises it, and a world that models sleepers as "did nothing" never
    notices that somebody else did something to them.
    """

    name: str
    blurb: str
    # Ticks after the market's founding before this participant is admitted.
    admission_delay: Distribution
    # Relative appetite for taking part in a fill. Weights a participant's
    # chance of being drawn as either side of one.
    activity_weight: Distribution
    # Ticks after the market reaches a terminal answer before redeeming.
    redeem_delay: Distribution
    # Whether this participant ever redeems inside the run at all.
    redeems: Distribution
    # Whether this participant will compact a STRANGER's abandoned claim check.
    compacts_strangers: Distribution
    # Whether this participant will drive a market's permissionless steps --
    # the deadline failure walk, the retirement -- for a market they hold
    # nothing in.
    cranks: Distribution

    def describe(self) -> dict:
        return {
            "name": self.name,
            "blurb": self.blurb,
            "distributions": {
                "admission_delay": self.admission_delay.describe(),
                "activity_weight": self.activity_weight.describe(),
                "redeem_delay": self.redeem_delay.describe(),
                "redeems": self.redeems.describe(),
                "compacts_strangers": self.compacts_strangers.describe(),
                "cranks": self.cranks.describe(),
            },
        }


PERSONAS: tuple = (
    Persona(
        name="eager-maker",
        blurb="In at the first opportunity and busy afterwards. The participant "
              "whose fills, if the substrate has a fill route, are most of a "
              "market's volume.",
        admission_delay=IntUniform(0, 1),
        activity_weight=IntUniform(6, 10),
        redeem_delay=IntUniform(0, 2),
        redeems=Bernoulli(95),
        compacts_strangers=Bernoulli(20),
        cranks=Bernoulli(30),
    ),
    Persona(
        name="patient-maker",
        blurb="Waits, then takes a position and mostly sits on it. Their "
              "admission is the reason a market's holder count is not a step "
              "function at founding.",
        admission_delay=IntUniform(2, 8),
        activity_weight=IntUniform(2, 5),
        redeem_delay=IntUniform(1, 5),
        redeems=Bernoulli(90),
        compacts_strangers=Bernoulli(10),
        cranks=Bernoulli(15),
    ),
    Persona(
        name="prompt-redeemer",
        blurb="Holds quietly and then collects the moment the answer lands. "
              "The clean payback path, and the one a market's terminal "
              "boundary is supposed to look like.",
        admission_delay=IntUniform(1, 4),
        activity_weight=IntUniform(1, 3),
        redeem_delay=Constant(0),
        redeems=Bernoulli(100),
        compacts_strangers=Bernoulli(5),
        cranks=Bernoulli(5),
    ),
    Persona(
        name="sleeper",
        blurb="Takes a position and is never heard from again. Their claim "
              "check outlives their attention, and recovering its rent is "
              "somebody else's permissionless business.",
        admission_delay=IntUniform(0, 6),
        activity_weight=IntUniform(1, 4),
        redeem_delay=IntUniform(0, 0),
        redeems=Bernoulli(0),
        compacts_strangers=Bernoulli(0),
        cranks=Bernoulli(0),
    ),
    Persona(
        name="crank",
        blurb="Holds nothing and drives everything: the deadline walk on a "
              "market that missed its commitment, the retirement of a market "
              "nobody is left in. Permissionless by construction, so nobody "
              "has to have appointed them.",
        admission_delay=IntUniform(0, 3),
        activity_weight=Constant(0),
        redeem_delay=Constant(0),
        redeems=Bernoulli(0),
        compacts_strangers=Bernoulli(60),
        cranks=Bernoulli(100),
    ),
    Persona(
        name="compactor",
        blurb="Makes a living off other people's abandoned claim checks. Exists "
              "so that a world with sleepers in it has somebody for whom the "
              "sleepers are an opportunity rather than a loose end.",
        admission_delay=IntUniform(1, 5),
        activity_weight=IntUniform(0, 2),
        redeem_delay=IntUniform(0, 1),
        redeems=Bernoulli(80),
        compacts_strangers=Bernoulli(100),
        cranks=Bernoulli(70),
    ),
)

PERSONAS_BY_NAME = {persona.name: persona for persona in PERSONAS}

# Who turns up. Sleepers are common on purpose: an abandoned claim check is the
# ordinary case in every market that has ever existed, and a world where it is
# rare would make compaction look like an edge case rather than a job.
DEFAULT_PERSONA_MIX = Categorical((
    ("eager-maker", 4),
    ("patient-maker", 4),
    ("prompt-redeemer", 3),
    ("sleeper", 4),
    ("crank", 2),
    ("compactor", 2),
))


# ---------------------------------------------------------------------------
# The world plan
# ---------------------------------------------------------------------------


@dataclasses.dataclass
class PlannedParticipant:
    market_id: str
    participant_id: str
    persona: str
    admission_tick: int
    activity_weight: int
    redeem_delay: int
    redeems: bool
    compacts_strangers: bool
    cranks: bool
    stake_atoms: int
    draws: list

    def body(self) -> dict:
        return {
            "participant_id": self.participant_id,
            "market_id": self.market_id,
            "persona": self.persona,
            "admission_tick": self.admission_tick,
            "activity_weight": self.activity_weight,
            "redeem_delay": self.redeem_delay,
            "redeems": self.redeems,
            "compacts_strangers": self.compacts_strangers,
            "cranks": self.cranks,
            "stake_atoms": self.stake_atoms,
            "draws": self.draws,
        }


@dataclasses.dataclass
class PlannedMarket:
    market_id: str
    archetype: str
    outcome_count: int
    basis: str
    fee_basis_points: int
    claim_unit_atoms: int
    founding_collateral_atoms: int
    deadline_slots: int
    # The BAND: the cuts this market's Product partitions its coordinate domain
    # at, the denominator they are stated over, and what each outcome pays.
    # `outcome_count == len(cuts) + 2`, always.
    cuts: list
    cut_denominator: int
    coefficients: list
    destiny: str
    founding_tick: int
    deadline_tick: int
    terminal_tick: Optional[int]
    selected_cell: Optional[int]
    failure_cell: Optional[int]
    participants: list
    draws: list

    def body(self) -> dict:
        return {
            "market_id": self.market_id,
            "archetype": self.archetype,
            "outcome_count": self.outcome_count,
            "basis": self.basis,
            "fee_basis_points": self.fee_basis_points,
            "claim_unit_atoms": self.claim_unit_atoms,
            "founding_collateral_atoms": self.founding_collateral_atoms,
            "deadline_slots": self.deadline_slots,
            "cuts": self.cuts,
            "cut_denominator": self.cut_denominator,
            "coefficients": self.coefficients,
            "destiny": self.destiny,
            "founding_tick": self.founding_tick,
            "deadline_tick": self.deadline_tick,
            "terminal_tick": self.terminal_tick,
            "selected_cell": self.selected_cell,
            "failure_cell": self.failure_cell,
            "participants": [p.body() for p in self.participants],
            "draws": self.draws,
        }


@dataclasses.dataclass
class PlannedEvent:
    """One thing the conductor will try, at one tick.

    `sequence` is assigned after the whole schedule is sorted and is the event's
    identity in the ledger.  Sorting is total and deterministic --
    `(tick, market_id, route rank, subject)` -- so that two runs of the same
    world execute the same events in the same order and their ledgers can be
    compared line by line.
    """

    tick: int
    route: str
    market_id: str
    subject: str
    detail: dict
    sequence: int = -1

    def body(self) -> dict:
        return {
            "sequence": self.sequence,
            "tick": self.tick,
            "route": self.route,
            "market_id": self.market_id,
            "subject": self.subject,
            "detail": self.detail,
        }


# Routes are ordered within a tick by what has to be true before what: a market
# is founded before anybody is admitted to it, admitted before it is filled,
# and censused last so that the census sees the tick's own work.
ROUTE_RANK = {route: rank for rank, route in enumerate((
    ROUTE_FOUND,
    ROUTE_ADMIT,
    ROUTE_FILL,
    ROUTE_DEADLINE_FAILURE,
    ROUTE_RESOLVE,
    ROUTE_REDEEM,
    ROUTE_COMPACT,
    ROUTE_RETIRE,
    ROUTE_CENSUS,
))}


@dataclasses.dataclass(frozen=True)
class WorldSpec:
    """What kind of world to draw. Small on purpose: the interesting variety
    lives in the archetypes and personas, not in this."""

    seed: SeedBook
    markets: int = 8
    ticks: int = 24
    # How far apart foundings are staggered, in ticks. A world whose markets are
    # all founded at tick 0 has no arrivals in it.
    founding_stagger: Distribution = dataclasses.field(default_factory=lambda: IntUniform(0, 6))
    archetype_mix: Categorical = DEFAULT_ARCHETYPE_MIX
    persona_mix: Categorical = DEFAULT_PERSONA_MIX
    # Slots per tick, used only to turn a market's deadline in SLOTS into a
    # deadline in ticks. Measured, not assumed: pass the run's own observed slot
    # rate. The default is the rate SIMVIZ measured on the devnet run
    # (6.03 slots/s) times a 20s cadence, rounded.
    slots_per_tick: int = 120

    def describe(self) -> dict:
        return {
            "markets": self.markets,
            "ticks": self.ticks,
            "slots_per_tick": self.slots_per_tick,
            "founding_stagger": self.founding_stagger.describe(),
            "archetype_mix": self.archetype_mix.describe(),
            "persona_mix": self.persona_mix.describe(),
        }


@dataclasses.dataclass
class World:
    spec: WorldSpec
    markets: list
    events: list

    def body(self) -> dict:
        """The whole plan as one document, digest included.

        The digest covers the markets and the events but NOT the spec's prose,
        so a world can gain a blurb without every journal on disk refusing to
        resume.  What it does cover is every number the conductor will act on.
        """
        markets = [market.body() for market in self.markets]
        events = [event.body() for event in self.events]
        return {
            "schema": SCHEMA_WORLD,
            "seed": self.spec.seed.describe(),
            "spec": self.spec.describe(),
            "archetypes": [a.describe() for a in ARCHETYPES],
            "personas": [p.describe() for p in PERSONAS],
            "markets": markets,
            "events": events,
            "plan_digest": simcore.digest_of({"markets": markets, "events": events}),
        }


def build_world(spec: WorldSpec) -> World:
    """Draw a whole world. PURE: no clock, no filesystem, no cluster.

    Called twice with the same spec it returns the same plan, byte for byte,
    which `test_simlife.py` asserts rather than assumes.
    """
    if spec.markets < 1:
        raise Refusal("a world needs at least one market")
    if spec.ticks < 1:
        raise Refusal("a world needs at least one tick")

    markets: list = []
    for index in range(spec.markets):
        markets.append(_draw_market(spec, index))
    events = _schedule(spec, markets)
    return World(spec=spec, markets=markets, events=events)


# The coordinate domain is USD cents per SOL, so a cut of 12,000 over a
# denominator of 100 reads "120.00". These two bound where a band sits and how
# wide its regions are; they are the difference between two markets of one
# width being two products and being one product twice.
BAND_CENTER = IntUniform(4_000, 40_000)
BAND_SPACING = IntUniform(400, 6_000)
CUT_DENOMINATOR = 100


def _band(outcome_count: int, center: int, spacing: int) -> list:
    """`outcome_count - 2` strictly increasing cuts, centred and evenly spaced.

    Evenly spaced is a CHOICE and a modest one: the interesting variation
    between two markets of one width is where the band sits and how wide it is,
    and an uneven band would add a second axis of difference that no consumer of
    this plan reads. It is recorded as a distribution beside the value so a
    reader can see that the spacing was uniform rather than assume it.
    """
    count = max(0, outcome_count - 2)
    if count == 0:
        return []
    first = center - spacing * (count - 1) // 2
    # Keep the whole band positive: a negative USD-cents-per-SOL cut is a
    # coordinate no price can land under, which would make one region dead.
    first = max(spacing, first)
    return [first + spacing * step for step in range(count)]


def _payoff(rng: random.Random, outcome_count: int) -> list:
    """One coefficient per outcome; the failure cell pays nothing.

    At least one ordinary cell must pay, or the portfolio is worth zero in every
    state and the market is not a claim on anything.
    """
    ordinary = max(1, outcome_count - 1)
    coefficients = [rng.randint(0, 1) for _ in range(ordinary)]
    if not any(coefficients):
        coefficients[rng.randrange(ordinary)] = 1
    return coefficients + [0]


def _draw_market(spec: WorldSpec, index: int) -> PlannedMarket:
    market_id = f"m{index:02d}"
    draws: list = []

    def record(name: str, distribution: Distribution, domain: str) -> Any:
        value, entry = draw_recorded(name, distribution, spec.seed.stream(domain, index))
        draws.append(entry)
        return value

    archetype_name = record("archetype", spec.archetype_mix, "world/archetype")
    archetype = ARCHETYPES_BY_NAME[archetype_name]

    outcome_count = record("outcome_count", archetype.outcomes, f"market/{market_id}/outcomes")
    basis = record("basis", archetype.basis, f"market/{market_id}/basis")
    fee = record("fee_basis_points", archetype.fee_basis_points, f"market/{market_id}/fee")
    if fee != 0:
        # Not a style rule: fee-bearing founding does not fit in one transaction
        # on today's wire, so a world that drew a rate would be a world whose
        # markets cannot be founded. Refuse where the number is drawn rather
        # than let a conductor discover it a validator later.
        raise Refusal(
            f"{market_id} drew a {fee} bp fee; only zero-fee markets found in one "
            "transaction today (docs/evidence/FEE_SECOND_TRANSACTION_FOUNDATION_2026_08_30.md)"
        )
    unit = int(record("claim_unit_atoms", archetype.claim_unit_atoms, f"market/{market_id}/unit"))
    collateral = record(
        "founding_collateral_atoms",
        archetype.founding_collateral_atoms,
        f"market/{market_id}/collateral",
    )
    deadline_slots = record(
        "deadline_slots", archetype.deadline_slots, f"market/{market_id}/deadline"
    )
    destiny = record("destiny", archetype.destiny, f"market/{market_id}/destiny")
    participant_count = record(
        "participants", archetype.participants, f"market/{market_id}/participants"
    )
    concentration = record(
        "stake_concentration_percent",
        archetype.stake_concentration_percent,
        f"market/{market_id}/concentration",
    )
    founding_tick = record(
        "founding_tick", spec.founding_stagger, f"market/{market_id}/founding-tick"
    )

    # THE BAND, and it is drawn rather than derived, because the width alone
    # does not describe a market. Two `wide-field`s of nine cells whose cuts sit
    # at the same nine numbers are the same market twice; what makes them
    # different products is where the band sits and how wide its regions are.
    #
    # A categorical market's outcome count is `cuts + 2`: one region per cut
    # boundary, the two open tails, and the explicit failure outcome. So a
    # width-2 market has NO cuts -- the whole coordinate domain as one region
    # plus failure -- and that is a value here rather than a degenerate case.
    band_center = record("band_center", BAND_CENTER, f"market/{market_id}/band-center")
    band_spacing = record("band_spacing", BAND_SPACING, f"market/{market_id}/band-spacing")
    cuts = _band(int(outcome_count), int(band_center), int(band_spacing))
    # The payoff. `coefficients[i]` is what outcome `i` pays, and the LAST
    # outcome is the market's explicit failure cell, which pays nothing: a
    # market that paid on its own failure is not a market anybody would buy
    # protection from.
    payoff_rng = spec.seed.stream(f"market/{market_id}/payoff")
    coefficients = _payoff(payoff_rng, int(outcome_count))
    draws.append({
        "name": "band",
        "drawn": {"cuts": cuts, "cut_denominator": CUT_DENOMINATOR, "coefficients": coefficients},
        "from": (
            f"{len(cuts)} cuts spaced {band_spacing} around {band_center} over denominator "
            f"{CUT_DENOMINATOR}; payoff drawn over the non-failure cells with the failure cell "
            "pinned to zero"
        ),
    })

    # The deadline in TICKS is derived, not drawn: it is the market's own
    # deadline in slots divided by the run's slots-per-tick. A market whose
    # deadline falls outside the run's horizon simply does not reach it inside
    # this run, which is a true thing about a short run and not a defect.
    deadline_tick = founding_tick + max(1, deadline_slots // max(1, spec.slots_per_tick))

    split = DirichletSplit(parts=int(participant_count), concentration_percent=int(concentration))
    stakes = split.split(spec.seed.stream(f"market/{market_id}/stakes"), int(collateral))
    draws.append({
        "name": "stake_split",
        "drawn": stakes,
        "from": split.describe(),
    })

    participants: list = []
    for slot_index in range(int(participant_count)):
        participants.append(
            _draw_participant(spec, market_id, index, slot_index, founding_tick, stakes[slot_index])
        )

    terminal_tick: Optional[int] = None
    selected_cell: Optional[int] = None
    failure_cell: Optional[int] = None
    if destiny == DESTINY_RESOLVES:
        settle_rng = spec.seed.stream(f"market/{market_id}/settlement")
        terminal_tick = deadline_tick + settle_rng.randint(0, 2)
        selected_cell = settle_rng.randrange(int(outcome_count))
    elif destiny == DESTINY_FAILS:
        # The failure branch selects the market's own disclosed failure cell,
        # which by protocol convention is the last one. The engine records which
        # cell it EXPECTS; the certificate on chain is what decides.
        terminal_tick = deadline_tick
        failure_cell = int(outcome_count) - 1
        selected_cell = failure_cell

    return PlannedMarket(
        market_id=market_id,
        archetype=archetype_name,
        outcome_count=int(outcome_count),
        basis=basis,
        fee_basis_points=int(fee),
        claim_unit_atoms=unit,
        founding_collateral_atoms=int(collateral),
        deadline_slots=int(deadline_slots),
        cuts=cuts,
        cut_denominator=CUT_DENOMINATOR,
        coefficients=coefficients,
        destiny=destiny,
        founding_tick=int(founding_tick),
        deadline_tick=int(deadline_tick),
        terminal_tick=terminal_tick,
        selected_cell=selected_cell,
        failure_cell=failure_cell,
        participants=participants,
        draws=draws,
    )


def _draw_participant(
    spec: WorldSpec,
    market_id: str,
    market_index: int,
    slot_index: int,
    founding_tick: int,
    stake_atoms: int,
) -> PlannedParticipant:
    participant_id = f"{market_id}-p{slot_index}"
    draws: list = []
    domain = f"participant/{participant_id}"

    def record(name: str, distribution: Distribution, sub: str) -> Any:
        value, entry = draw_recorded(name, distribution, spec.seed.stream(f"{domain}/{sub}"))
        draws.append(entry)
        return value

    persona_name = record("persona", spec.persona_mix, "persona")
    persona = PERSONAS_BY_NAME[persona_name]
    admission_delay = record("admission_delay", persona.admission_delay, "admission")
    activity = record("activity_weight", persona.activity_weight, "activity")
    redeem_delay = record("redeem_delay", persona.redeem_delay, "redeem-delay")
    redeems = record("redeems", persona.redeems, "redeems")
    compacts = record("compacts_strangers", persona.compacts_strangers, "compacts")
    cranks = record("cranks", persona.cranks, "cranks")

    return PlannedParticipant(
        market_id=market_id,
        participant_id=participant_id,
        persona=persona_name,
        admission_tick=int(founding_tick) + int(admission_delay),
        activity_weight=int(activity),
        redeem_delay=int(redeem_delay),
        redeems=bool(redeems),
        compacts_strangers=bool(compacts),
        cranks=bool(cranks),
        stake_atoms=int(stake_atoms),
        draws=draws,
    )


def _weighted_pick(rng: random.Random, candidates: Sequence, weight: Callable) -> Optional[Any]:
    """One weighted pick, or None when every candidate has zero weight.

    None is a real answer here: a market whose only participants are cranks has
    nobody to fill against, and inventing a filler would be inventing a trade.
    """
    weights = [max(0, int(weight(candidate))) for candidate in candidates]
    total = sum(weights)
    if total == 0:
        return None
    cut = rng.randint(1, total)
    running = 0
    for candidate, value in zip(candidates, weights):
        running += value
        if cut <= running:
            return candidate
    return None


def _schedule(spec: WorldSpec, markets: list) -> list:
    """Interleave every market's lifecycle into one ordered event stream."""
    events: list = []
    # Cranks are drawn from the whole world, not from one market: the point of a
    # permissionless step is that a stranger can take it, so the retirement of
    # market 3 is driven by whoever in the world cranks, which is usually not
    # somebody holding market 3.
    world_cranks = [
        participant for market in markets for participant in market.participants
        if participant.cranks
    ]
    world_compactors = [
        participant for market in markets for participant in market.participants
        if participant.compacts_strangers
    ]

    for market in markets:
        horizon = spec.ticks
        if market.founding_tick >= horizon:
            # Founded past the end of the run. It contributes nothing, and the
            # plan says so by carrying the market with no events rather than by
            # quietly dropping it.
            continue
        events.append(PlannedEvent(
            tick=market.founding_tick,
            route=ROUTE_FOUND,
            market_id=market.market_id,
            subject=market.market_id,
            detail={
                "archetype": market.archetype,
                "outcome_count": market.outcome_count,
                "basis": market.basis,
                "claim_unit_atoms": market.claim_unit_atoms,
                "collateral_atoms": market.founding_collateral_atoms,
                "deadline_slots": market.deadline_slots,
                "fee_basis_points": market.fee_basis_points,
                "cuts": market.cuts,
                "cut_denominator": market.cut_denominator,
                "coefficients": market.coefficients,
            },
        ))
        for participant in market.participants:
            if participant.admission_tick >= horizon:
                continue
            events.append(PlannedEvent(
                tick=participant.admission_tick,
                route=ROUTE_ADMIT,
                market_id=market.market_id,
                subject=participant.participant_id,
                detail={"persona": participant.persona, "stake_atoms": participant.stake_atoms},
            ))

        events.extend(_schedule_fills(spec, market, horizon))

        terminal = market.terminal_tick
        if terminal is not None and terminal < horizon:
            if market.destiny == DESTINY_FAILS:
                driver = _weighted_pick(
                    spec.seed.stream(f"market/{market.market_id}/failure-driver"),
                    world_cranks,
                    lambda p: 1,
                )
                events.append(PlannedEvent(
                    tick=terminal,
                    route=ROUTE_DEADLINE_FAILURE,
                    market_id=market.market_id,
                    subject=market.market_id,
                    detail={
                        "driven_by": None if driver is None else driver.participant_id,
                        "expected_failure_cell": market.failure_cell,
                        "deadline_slots": market.deadline_slots,
                    },
                ))
            else:
                events.append(PlannedEvent(
                    tick=terminal,
                    route=ROUTE_RESOLVE,
                    market_id=market.market_id,
                    subject=market.market_id,
                    detail={"expected_selected_cell": market.selected_cell},
                ))
            events.extend(
                _schedule_paybacks(spec, market, terminal, horizon, world_compactors, world_cranks)
            )

        # The census runs for every tick the market is plausibly alive, from its
        # founding to the end of the run. It is scheduled unconditionally --
        # including for a market whose founding refuses -- and the conductor
        # blocks it there, so the ledger records that the observation was
        # WANTED and why it could not be taken.
        for tick in range(market.founding_tick, horizon):
            events.append(PlannedEvent(
                tick=tick,
                route=ROUTE_CENSUS,
                market_id=market.market_id,
                subject=market.market_id,
                detail={},
            ))

    events.sort(key=lambda e: (e.tick, e.market_id, ROUTE_RANK[e.route], e.subject))
    for sequence, event in enumerate(events):
        event.sequence = sequence
    return events


def _schedule_fills(spec: WorldSpec, market: PlannedMarket, horizon: int) -> list:
    """Fills, in BURSTS rather than at a steady rate.

    Real market activity is clustered: nothing happens for a while and then
    several things happen at once.  A steady one-per-tick schedule draws a
    volume chart that is a straight line, which is the same failure as the flat
    supply chart this whole module exists to fix.
    """
    rng = spec.seed.stream(f"market/{market.market_id}/fills")
    archetype = ARCHETYPES_BY_NAME[market.archetype]
    bursts = int(archetype.fill_bursts.draw(rng))
    if bursts <= 0:
        return []
    # A fill can only happen between admission and the market's terminal
    # boundary. A window that does not exist yields no fills, honestly.
    opens = min(participant.admission_tick for participant in market.participants) + 1
    closes = min(horizon, market.terminal_tick if market.terminal_tick is not None else horizon)
    if closes <= opens:
        return []

    active = [p for p in market.participants if p.activity_weight > 0]
    if len(active) < 2:
        return []

    events: list = []
    for burst in range(bursts):
        tick = rng.randrange(opens, closes)
        for step in range(int(archetype.fills_per_burst.draw(rng))):
            maker = _weighted_pick(rng, active, lambda p: p.activity_weight)
            others = [p for p in active if maker is None or p.participant_id != maker.participant_id]
            taker = _weighted_pick(rng, others, lambda p: p.activity_weight)
            if maker is None or taker is None:
                continue
            # The quantity is a fraction of the smaller side's stake, so a fill
            # is never larger than what the participant brought.
            ceiling = max(1, min(maker.stake_atoms, taker.stake_atoms) // 4)
            events.append(PlannedEvent(
                tick=tick,
                route=ROUTE_FILL,
                market_id=market.market_id,
                subject=f"{maker.participant_id}->{taker.participant_id}#{burst}.{step}",
                detail={
                    "maker": maker.participant_id,
                    "taker": taker.participant_id,
                    "cell": rng.randrange(market.outcome_count),
                    "quantity_atoms": rng.randint(1, ceiling),
                    "burst": burst,
                },
            ))
    return events


def _schedule_paybacks(
    spec: WorldSpec,
    market: PlannedMarket,
    terminal: int,
    horizon: int,
    world_compactors: list,
    world_cranks: list,
) -> list:
    """Who collects, who does not, and who cleans up after the ones who do not."""
    events: list = []
    rng = spec.seed.stream(f"market/{market.market_id}/paybacks")
    dormant: list = []
    last_payback = terminal
    for participant in market.participants:
        if not participant.redeems:
            # DORMANT, not necessarily a `sleeper`. What makes a claim check
            # somebody else's business is that nobody came back for it, and a
            # crank who took a position and only ever cared about the
            # permissionless steps leaves exactly the same account behind. The
            # persona is recorded so a reader can tell the two apart; the
            # schedule does not need to.
            dormant.append(participant)
            continue
        tick = terminal + participant.redeem_delay
        if tick >= horizon:
            # Meant to redeem, ran out of run. Not a sleeper: the difference is
            # recorded because one is a behaviour and the other is a horizon.
            continue
        last_payback = max(last_payback, tick)
        events.append(PlannedEvent(
            tick=tick,
            route=ROUTE_REDEEM,
            market_id=market.market_id,
            subject=participant.participant_id,
            detail={"persona": participant.persona, "stake_atoms": participant.stake_atoms},
        ))

    # Every abandoned claim check is somebody else's rent. The compactor is
    # drawn from the WORLD and is required not to be the holder themselves --
    # compacting your own check is redemption, and calling it compaction would
    # hide the permissionless step this world exists to exercise.
    for participant in dormant:
        strangers = [
            candidate for candidate in world_compactors
            if candidate.participant_id != participant.participant_id
        ]
        compactor = _weighted_pick(rng, strangers, lambda p: 1)
        if compactor is None:
            continue
        tick = terminal + rng.randint(1, 4)
        if tick >= horizon:
            continue
        last_payback = max(last_payback, tick)
        events.append(PlannedEvent(
            tick=tick,
            route=ROUTE_COMPACT,
            market_id=market.market_id,
            subject=participant.participant_id,
            detail={
                "dormant_holder": participant.participant_id,
                "dormant_persona": participant.persona,
                "compacted_by": compactor.participant_id,
                "compactor_persona": compactor.persona,
            },
        ))

    # Retirement last, and only when there is a stranger to drive it.
    driver = _weighted_pick(rng, world_cranks, lambda p: 1)
    retire_tick = last_payback + rng.randint(1, 3)
    if driver is not None and retire_tick < horizon:
        events.append(PlannedEvent(
            tick=retire_tick,
            route=ROUTE_RETIRE,
            market_id=market.market_id,
            subject=market.market_id,
            detail={"driven_by": driver.participant_id, "driver_persona": driver.persona},
        ))
    return events


# ---------------------------------------------------------------------------
# Substrates
# ---------------------------------------------------------------------------


@dataclasses.dataclass
class EventResult:
    """What one event actually did. The only path by which anything reaches the
    ledger, and the reason the ledger cannot carry a number nobody measured."""

    outcome: str
    detail: str
    signatures: list = dataclasses.field(default_factory=list)
    observation: Optional[dict] = None

    def __post_init__(self) -> None:
        if self.outcome not in TERMINAL_OUTCOMES:
            raise Refusal(f"{self.outcome!r} is not one of {TERMINAL_OUTCOMES}")

    def body(self) -> dict:
        return {
            "outcome": self.outcome,
            "detail": simcore.redact_text(self.detail),
            "signatures": list(self.signatures),
            "observation": self.observation,
        }


class Substrate:
    """Where a world is actually driven.

    A substrate declares its ROUTES up front and the conductor never asks it for
    one it does not have.  That declaration is the honest seam of this whole
    module: today's local founding refuses (`0x5182`), so a substrate that
    cannot found says so once, and every founding, admission and fill in the
    world is recorded as `unattempted` with that reason instead of as a failure
    somebody has to interpret.
    """

    name = "substrate"
    label = "unnamed substrate"
    routes: frozenset = frozenset()
    # Which basis kinds this substrate can express at founding. Separate from
    # `routes` because "I can found markets" and "I can found THAT market" are
    # different claims, and today every real substrate answers yes to the first
    # and no to two thirds of the second.
    basis_kinds: frozenset = frozenset({BASIS_CATEGORICAL})
    cluster = "local"
    # Planned market ids that ALREADY EXIST on this substrate, independent of
    # anything this run does.
    #
    # This is the seam between a plan and a chain, and it needs its own word.
    # When a config binds planned market `m03` to a market somebody founded
    # yesterday, this run does not found it -- the founding event is
    # `unattempted` and honestly so -- but every observation of it is real, and
    # blocking those on "m03 was never founded" would be the artifact lying
    # about a market it is looking straight at.
    pre_founded: frozenset = frozenset()
    rpc_origin: Optional[str] = None
    source_revision: Optional[str] = None

    def describe(self) -> dict:
        return {
            "name": self.name,
            "label": self.label,
            "cluster": self.cluster,
            "rpc_origin": self.rpc_origin,
            "source_revision": self.source_revision,
            "routes": sorted(self.routes),
            "routes_absent": sorted(set(ALL_ROUTES) - set(self.routes)),
            "basis_kinds": sorted(self.basis_kinds),
            "basis_kinds_absent": sorted(set(ALL_BASIS_KINDS) - set(self.basis_kinds)),
            "pre_founded": sorted(self.pre_founded),
        }

    def why_not(self, route: str) -> str:
        """One sentence for a route this substrate does not have. Overridden by
        substrates that know something more specific than the default."""
        return f"{self.label} has no {route} route"

    def why_not_basis(self, basis: str) -> str:
        return BASIS_ABSENCE.get(basis, f"{self.label} cannot express a {basis} basis")

    def execute(self, event: PlannedEvent, market: PlannedMarket) -> EventResult:  # pragma: no cover
        raise NotImplementedError


class RehearsalSubstrate(Substrate):
    """A substrate with no routes at all.

    NOT A MOCK, and deliberately not one.  It executes nothing, signs nothing
    and observes nothing, so a rehearsal produces a plan and a ledger in which
    every single event is `unattempted` -- and NO series points, because a
    series point is a census observation and no census was taken.  It exists so
    that a world can be drawn, read and argued about before any substrate is
    available to drive it, without a single number in the output being one
    nobody measured.
    """

    name = "rehearsal"
    label = "a rehearsal that executes nothing"
    routes = frozenset()
    basis_kinds = frozenset()

    def why_not(self, route: str) -> str:
        return "this was a rehearsal: the world was drawn and nothing was driven"

    def execute(self, event: PlannedEvent, market: PlannedMarket) -> EventResult:
        raise Refusal("the rehearsal substrate has no routes and must never be asked to execute")


class CensusOnlySubstrate(Substrate):
    """Observation and nothing else, against markets that already exist.

    The route set is exactly `{census}`.  Every mutation in the world is
    therefore `unattempted` and every series point is a real observation of real
    accounts on a real chain -- which is the honest thing to run when the
    founding route is closed, and is what today's `simulator.py` already does
    for one market, generalised to as many as the caller can name.

    `binding` maps a planned market id to the census arguments for a market that
    EXISTS.  A planned market with no binding is not censused: inventing an
    observation for it would be the exact fabrication this module refuses.
    """

    name = "census-only"
    routes = frozenset({ROUTE_CENSUS})

    def __init__(
        self,
        *,
        label: str,
        cluster: str,
        rpc_origin: str,
        bindings: dict,
        observe: Callable,
        source_revision: Optional[str] = None,
        absent_route_reason: Optional[str] = None,
    ) -> None:
        self.label = label
        self.cluster = cluster
        self.rpc_origin = rpc_origin
        self.bindings = bindings
        self._observe = observe
        self.source_revision = source_revision
        self._absent_reason = absent_route_reason

    def why_not(self, route: str) -> str:
        if self._absent_reason:
            return self._absent_reason
        return (
            f"{self.label} observes accounts and signs nothing, so it has no {route} route"
        )

    def execute(self, event: PlannedEvent, market: PlannedMarket) -> EventResult:
        if event.route != ROUTE_CENSUS:
            raise Refusal(f"census-only substrate was asked for {event.route}")
        binding = self.bindings.get(event.market_id)
        if binding is None:
            return EventResult(
                outcome=OUTCOME_UNATTEMPTED,
                detail=(
                    f"{event.market_id} is a planned market with no existing market bound to "
                    "it; an observation of a market that was never founded would be invented"
                ),
            )
        return self._observe(event, market, binding)


# ---------------------------------------------------------------------------
# The conductor
# ---------------------------------------------------------------------------


@dataclasses.dataclass
class LedgerEntry:
    event: PlannedEvent
    result: EventResult

    def body(self) -> dict:
        entry = self.event.body()
        entry["result"] = self.result.body()
        return entry


class Conductor:
    """Walk a world's schedule against a substrate, and write down what happened.

    Two rules and everything else follows from them.

    ONE: a route the substrate does not have is never attempted, and its events
    are `unattempted` with the substrate's own sentence.

    TWO: an event whose PREREQUISITE did not execute is `blocked`, not failed.
    A market whose founding refuses has no admissions, no fills and no census to
    take; recording those as failures would suggest ten things went wrong when
    one did, and recording them as successes would be a lie.  The prerequisite
    graph is small and explicit: everything in a market depends on that market's
    founding, a redemption or a compaction depends on the market reaching a
    terminal answer, and a retirement depends on the same.
    """

    def __init__(
        self,
        world: World,
        substrate: Substrate,
        *,
        on_tick: Optional[Callable] = None,
        should_stop: Optional[Callable] = None,
    ):
        self.world = world
        self.substrate = substrate
        self.entries: list = []
        self.on_tick = on_tick
        self.should_stop = should_stop
        self.stopped_at_tick: Optional[int] = None
        self._markets = {market.market_id: market for market in world.markets}
        # A market the substrate says already exists is founded as far as this
        # run is concerned. `_founded_here` keeps the two apart so the ledger can
        # still say which foundings THIS run performed, which is none of them
        # when the substrate is an existing chain.
        self._founded: set = set(substrate.pre_founded)
        self._founded_here: set = set()
        self._terminal: set = set()
        self._retired: set = set()
        # Markets whose basis this substrate cannot express. Distinct from
        # "not founded yet": the founding was never attempted and never will be
        # in this run, and everything under it says so with that reason rather
        # than with the generic one.
        #
        # Only a substrate that FOUNDS has a basis to express, and only a market
        # this run would have founded can be blocked on one. A substrate with no
        # founding route expresses no basis at all, and reading that as "every
        # market here is unfoundable" would file every observation of a market
        # that already exists under a shape complaint about a founding nobody
        # was ever going to attempt.
        self._unfoundable: set = set() if ROUTE_FOUND not in substrate.routes else {
            market.market_id for market in world.markets
            if market.basis not in substrate.basis_kinds
            and market.market_id not in substrate.pre_founded
        }

    def _prerequisite_failure(self, event: PlannedEvent) -> Optional[str]:
        market_id = event.market_id
        if event.route == ROUTE_FOUND:
            return None
        if market_id in self._unfoundable:
            return (
                f"{market_id} asks for a basis this substrate cannot express, so it was "
                "never founded and nothing downstream of it happened"
            )
        if market_id not in self._founded:
            return f"{market_id} was never founded, so there is nothing here to act on"
        if market_id in self._retired and event.route != ROUTE_CENSUS:
            return f"{market_id} was already retired at this point in the run"
        # A census AFTER retirement is not blocked, and that is deliberate: L6 is
        # the rent-conservation law and it is only applicable at a boundary where
        # a watched account CLOSED. Refusing to observe a retired market would
        # throw away the one observation that law exists for.
        if event.route in (ROUTE_REDEEM, ROUTE_COMPACT, ROUTE_RETIRE):
            if market_id not in self._terminal:
                return (
                    f"{market_id} never reached a terminal answer, so there is nothing to "
                    "collect and nothing to clean up"
                )
        return None

    def _record(self, event: PlannedEvent, result: EventResult) -> None:
        self.entries.append(LedgerEntry(event=event, result=result))
        if result.outcome != OUTCOME_EXECUTED:
            return
        if event.route == ROUTE_FOUND:
            self._founded.add(event.market_id)
            self._founded_here.add(event.market_id)
        elif event.route in (ROUTE_RESOLVE, ROUTE_DEADLINE_FAILURE):
            self._terminal.add(event.market_id)
        elif event.route == ROUTE_RETIRE:
            self._retired.add(event.market_id)

    def run(self) -> dict:
        ticks = sorted({event.tick for event in self.world.events})
        for tick in ticks:
            # Stopping happens BETWEEN ticks, where it is still a choice. A run
            # interrupted mid-tick would leave one market censused at this
            # boundary and its neighbour not, and the two would then be drawn on
            # a shared x-axis as if they had been read together.
            if self.should_stop is not None and self.should_stop():
                self.stopped_at_tick = tick
                break
            for event in self.world.events:
                if event.tick != tick:
                    continue
                market = self._markets[event.market_id]
                if event.route not in self.substrate.routes:
                    self._record(event, EventResult(
                        outcome=OUTCOME_UNATTEMPTED,
                        detail=self.substrate.why_not(event.route),
                    ))
                    continue
                if event.route == ROUTE_FOUND and event.market_id in self._unfoundable:
                    self._record(event, EventResult(
                        outcome=OUTCOME_UNATTEMPTED,
                        detail=self.substrate.why_not_basis(market.basis),
                    ))
                    continue
                blocked = self._prerequisite_failure(event)
                if blocked is not None:
                    self._record(event, EventResult(outcome=OUTCOME_BLOCKED, detail=blocked))
                    continue
                self._record(event, self.substrate.execute(event, market))
            if self.on_tick is not None:
                self.on_tick(tick, self)
        return self.ledger()

    def tally(self) -> dict:
        """Counts by route and by outcome. The one summary a reader should be
        able to trust without reading the whole ledger."""
        table: dict = {}
        for entry in self.entries:
            row = table.setdefault(entry.event.route, dict.fromkeys(TERMINAL_OUTCOMES, 0))
            row[entry.result.outcome] += 1
        return table

    def ledger(self) -> dict:
        entries = [entry.body() for entry in self.entries]
        return {
            "schema": SCHEMA_LEDGER,
            "recorded_at": simcore.utc_now_iso(),
            "seed": self.world.spec.seed.describe(),
            "plan_digest": self.world.body()["plan_digest"],
            "substrate": self.substrate.describe(),
            "tally": self.tally(),
            # A run that was stopped early has a plan longer than its ledger.
            # Nothing is lost -- `world.json` still holds every planned event --
            # but a reader comparing the two counts deserves to be told why they
            # differ rather than left to infer it.
            "stopped_at_tick": self.stopped_at_tick,
            "ticks_planned": self.world.spec.ticks,
            "markets_pre_founded": sorted(self.substrate.pre_founded),
            "markets_founded_by_this_run": sorted(self._founded_here),
            "markets_founded": sorted(self._founded),
            "markets_terminal": sorted(self._terminal),
            "markets_retired": sorted(self._retired),
            "entries": entries,
        }


# ---------------------------------------------------------------------------
# Reading a world out loud
# ---------------------------------------------------------------------------


def world_summary(world: World) -> list:
    """A few lines a person can read. Not the artifact; the thing that tells you
    whether the artifact is worth opening."""
    lines: list = []
    lines.append(
        f"seed {world.spec.seed.preimage!r} ({world.spec.seed.digest[:12]}), "
        f"{len(world.markets)} markets over {world.spec.ticks} ticks"
    )
    by_archetype: dict = {}
    by_destiny: dict = {}
    by_persona: dict = {}
    for market in world.markets:
        by_archetype[market.archetype] = by_archetype.get(market.archetype, 0) + 1
        by_destiny[market.destiny] = by_destiny.get(market.destiny, 0) + 1
        for participant in market.participants:
            by_persona[participant.persona] = by_persona.get(participant.persona, 0) + 1
    lines.append("archetypes: " + ", ".join(f"{k} x{v}" for k, v in sorted(by_archetype.items())))
    lines.append("destinies:  " + ", ".join(f"{k} x{v}" for k, v in sorted(by_destiny.items())))
    lines.append("personas:   " + ", ".join(f"{k} x{v}" for k, v in sorted(by_persona.items())))
    by_route: dict = {}
    for event in world.events:
        by_route[event.route] = by_route.get(event.route, 0) + 1
    lines.append("events:     " + ", ".join(f"{k} x{v}" for k, v in sorted(by_route.items())))
    widths = sorted({market.outcome_count for market in world.markets})
    lines.append(f"outcome widths drawn: {widths}")
    # THE COVERAGE LINE, and it is the one to read before running anything.
    #
    # A market's deadline is in CHAIN SLOTS, which is the protocol's unit and
    # not this run's. A run's horizon is `ticks * slots_per_tick`. Those are two
    # different clocks and a world can easily be drawn where no market reaches
    # its deadline before the run ends -- every resolution, failure walk,
    # redemption and retirement then falls outside the horizon and the run
    # watches ten markets hold still, which is precisely the flat picture this
    # module exists to stop drawing by accident.
    #
    # Neither clock is adjusted to flatter the other. The mismatch is REPORTED,
    # and the knob that fixes it (`slots_per_tick`, set from the substrate's own
    # measured slot rate times the run's cadence) is named beside it.
    horizon = world.spec.ticks * world.spec.slots_per_tick
    reached = [m for m in world.markets if m.terminal_tick is not None
               and m.terminal_tick < world.spec.ticks]
    lines.append(
        f"horizon: {world.spec.ticks} ticks x {world.spec.slots_per_tick} slots/tick "
        f"= {horizon} slots; {len(reached)} of {len(world.markets)} markets reach a "
        f"terminal boundary inside it"
    )
    if not reached:
        lines.append(
            "  NOTHING RESOLVES IN THIS WORLD. Every market's deadline is past the "
            "horizon, so the run will observe foundings and holdings and no answers. "
            "Raise slots_per_tick or ticks, or draw an archetype mix with shorter fuses."
        )
    return lines


def market_line(market: PlannedMarket) -> str:
    return (
        f"{market.market_id} {market.archetype:<12} {market.outcome_count:>2} cells  "
        f"{market.basis:<20} deadline {market.deadline_slots:>6} slots  "
        f"{market.destiny:<24} {len(market.participants)} participants  "
        f"{market.founding_collateral_atoms} atoms"
    )
