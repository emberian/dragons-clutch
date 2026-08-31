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
# WHERE A BAND SITS IS RELATIVE TO THE COORDINATE THE SUBSTRATE WILL OBSERVE,
# and getting that wrong once made every market in every world resolve into the
# same cell.
#
# The old rule drew `BAND_CENTER = IntUniform(4_000, 40_000)` and
# `BAND_SPACING = IntUniform(400, 6_000)` with a comment reading "the coordinate
# domain is USD cents per SOL, so a cut of 12,000 over a denominator of 100
# reads 120.00". That framing describes a devnet SOL/USD feed and it is not what
# any local chain does. `PythAdapterConfigV1::validate_update` returns
# `Ok(i128::from(price))` -- the RAW signed price atoms, with no rescaling to any
# denominator -- and the committed local fixture
# (`fixtures/pyth/local-upgraded-2026-08-22`) carries price `100000000` at
# exponent `-8`. So the observed coordinate on this substrate is 100,000,000
# exactly, on every chain, forever; every cut the old rule could draw was three
# to five orders of magnitude below it; and the observation therefore landed
# above the top cut in one hundred percent of markets.
#
# That is not a skew a bigger sample fixes. It is a units mismatch that made the
# outcome a CONSTANT, and the world's own `selected_cell` -- drawn uniformly and
# independently of the band -- hid it, because the plan's expectation and the
# chain's answer were never compared.
LOCAL_PYTH_FIXTURE_COORDINATE_V1 = 100_000_000

# When one cell taking this share of a world's resolving markets stops being a
# draw and starts being a defect. Seventy is a threshold rather than a law: a
# world of four markets can legitimately put three in one cell, and a world of
# forty cannot. It is stated so a run can FLAG itself rather than leave the
# reading to whoever notices the histogram.
DEGENERATE_OUTCOME_SHARE_PERCENT_V1 = 70

# The window a band's width is stated against. A market's band should be wide
# enough that the coordinate could plausibly be anywhere in it by the deadline,
# so the width scales with the SQUARE ROOT of the window -- the random walk's
# own scaling, and the only one that does not make a long market's band either
# absurdly narrow or absurdly wide.
BAND_WINDOW_REFERENCE_SLOTS_V1 = 10_000

# How far the coordinate is assumed to be able to travel over the reference
# window, in basis points of the anchor. Drawn per market: two markets on one
# feed with different views of how volatile it is are two different products,
# and this is the axis that says so.
BAND_VOLATILITY_BPS = IntUniform(40, 900)

# The band's PLACEMENT, in tenths of its own spacing, measured from the anchor.
# Drawn wide enough that the anchor lands in every cell of the market including
# both open tails: with `count` cuts there are `count + 1` ordinary cells, and an
# offset spread over +/- (count + 1)/2 spacings reaches all of them.
BAND_PLACEMENT_TENTHS_PER_CELL_V1 = 5

# Cuts are stated over this denominator. It is 1 rather than 100 because the
# coordinate is raw price atoms and a denominator is a display convention: a
# denominator of 100 over an atom coordinate would have printed every cut a
# hundred times too small.
CUT_DENOMINATOR = 1

# How the gaps between cuts vary across a band. Evenly spaced was the old rule
# and it made two markets of one width one product twice at two scales; a
# profile is what makes a nine-cell market about "roughly where" different from
# a nine-cell market about "exactly where near spot".
BAND_PROFILE_UNIFORM = "uniform"
BAND_PROFILE_TIGHT_CENTRE = "tight-centre"
BAND_PROFILE_TIGHT_EDGES = "tight-edges"
BAND_PROFILE_RAGGED = "ragged"
BAND_PROFILES = (
    BAND_PROFILE_UNIFORM,
    BAND_PROFILE_TIGHT_CENTRE,
    BAND_PROFILE_TIGHT_EDGES,
    BAND_PROFILE_RAGGED,
)
BAND_PROFILE = Categorical((
    (BAND_PROFILE_TIGHT_CENTRE, 4),
    (BAND_PROFILE_UNIFORM, 3),
    (BAND_PROFILE_RAGGED, 2),
    (BAND_PROFILE_TIGHT_EDGES, 1),
))

# The one profile that is a pattern rather than a rule: a fixed irregular
# sequence, so a `ragged` band is reproducibly uneven rather than randomly so.
_RAGGED_PATTERN_V1 = (7, 13, 9, 17, 11, 6, 15, 12, 8)

# How steeply a shaped profile's gaps grow, in tenths of the spacing, per
# half-step away from the middle of the band.
_PROFILE_SLOPE_TENTHS_V1 = 4




@dataclasses.dataclass(frozen=True)
class MarketArchetype:
    """One KIND of market, as a bundle of distributions rather than as a market.

    An archetype is drawn from, never instantiated directly: two markets of the
    same archetype differ in every number, and that is the point.

    THE FEE RATE IS A BAND. It was a `Constant(0)` everywhere, on the reading
    that fee-bearing founding did not fit in one transaction; that reading came
    from a document about the FILL's fee leg and not about founding, and it made
    every market this world drew untradeable -- because the owned-loopback Direct
    producer has no ticket to read, authors its own terms, and admits exactly
    `DIRECT_ADMITTED_FEE_BASIS_POINTS_V1`. Zero was the one rate that could never
    trade.

    So each archetype now draws a rate, weighted towards the admitted one and
    carrying its own controls. A market drawn off that rate is founded, opened,
    activated, admitted to and censused like any other, and its fill refuses
    naming the rate it read, the rate the release admits, and the program that
    does the refusing. That refusal is a measurement worth having and it is
    deliberately reachable.
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
    # See the class docstring. The default is the admitted rate rather than
    # zero, so an archetype added later without a fee band is tradeable by
    # default rather than silently untradeable.
    fee_basis_points: Distribution = dataclasses.field(
        default_factory=lambda: Constant(DIRECT_ADMITTED_FEE_BASIS_POINTS_V1)
    )
    # HOW FAR THE ARCHETYPE THINKS ITS COORDINATE CAN TRAVEL over the reference
    # window, in basis points of the anchor, and HOW ITS GAPS VARY across the
    # band. These two are what make two markets of one width two products: a
    # three-cell market at 40 bp on a tight-centre profile asks "is it moving at
    # all", and a three-cell market at 900 bp on a tight-edges profile asks a
    # different question with the same number of answers.
    band_volatility_bps: Distribution = dataclasses.field(
        default_factory=lambda: BAND_VOLATILITY_BPS
    )
    band_profile: Distribution = dataclasses.field(default_factory=lambda: BAND_PROFILE)

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
                "band_volatility_bps": self.band_volatility_bps.describe(),
                "band_profile": self.band_profile.describe(),
            },
        }


# The one Direct fee rate the deployed setup release admits, mirrored from
# `simlife_drivers.DIRECT_ADMITTED_FEE_BASIS_POINTS_V1` so this module can draw a
# band around it without importing the driver layer. The engine must not depend
# on the drivers -- it decides what to ATTEMPT and a substrate decides what
# happens -- so the number is stated twice on purpose and pinned equal by test.
DIRECT_ADMITTED_FEE_BASIS_POINTS_V1 = 50


# Rates a market may be founded at, and only one of them can be filled today.
# The weights say what a world is mostly made of; the tail is the control that
# makes the producer's rate clause reachable rather than theoretical.
TRADEABLE_FEE_BAND = Categorical((("50", 6), ("0", 1), ("25", 1)))
WIDE_FEE_BAND = Categorical((("50", 4), ("0", 1), ("100", 1)))
NARROW_FEE_BAND = Categorical((("50", 3), ("100", 1)))
GRADED_FEE_BAND = Categorical((("50", 3), ("25", 1)))
UNTRADED_FEE_BAND = Categorical((("0", 1), ("50", 1)))


ARCHETYPES: tuple = (
    MarketArchetype(
        name="coin-flip",
        blurb="Two sides of one line, plus the explicit failure cell: above the "
              "cut or below it. The plainest market this protocol can hold that "
              "still has two answers, and the one every other archetype is a "
              "departure from.",
        # THREE, not two, and the difference is the whole archetype. A width-2
        # market has NO cuts -- the whole coordinate domain as one region plus
        # failure -- so its only ordinary answer is "it did not fail", which is
        # not a coin flip and cannot land in two places. One cut gives two
        # regions, which is what the name has always claimed. Width 2 stays
        # reachable through `quiet-corner`, which is about a market nobody
        # trades rather than about a market with two sides.
        outcomes=Constant(3),
        basis=Constant(BASIS_CATEGORICAL),
        deadline_slots=LogIntUniform(2_000, 20_000),
        destiny=Categorical(((DESTINY_RESOLVES, 8), (DESTINY_FAILS, 1), (DESTINY_SLEEPY, 1))),
        claim_unit_atoms=Constant(1),
        founding_collateral_atoms=LogIntUniform(100_000_000, 2_000_000_000),
        participants=IntUniform(2, 4),
        stake_concentration_percent=Constant(100),
        fill_bursts=IntUniform(1, 3),
        fills_per_burst=IntUniform(1, 3),
        fee_basis_points=TRADEABLE_FEE_BAND,
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
        fee_basis_points=NARROW_FEE_BAND,
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
        fee_basis_points=GRADED_FEE_BAND,
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
        fee_basis_points=GRADED_FEE_BAND,
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
        fee_basis_points=WIDE_FEE_BAND,
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
        fee_basis_points=UNTRADED_FEE_BAND,
        # A market nobody trades is still a market about something, and the
        # thing it is about is a wide slow question: a low volatility over a
        # very long window still gives a broad band.
        band_volatility_bps=IntUniform(60, 300),
        band_profile=Constant(BAND_PROFILE_UNIFORM),
    ),
    MarketArchetype(
        name="hairline",
        blurb="Three answers around a band so tight the question is whether "
              "the coordinate moved at all. The archetype whose cuts sit "
              "closest together, and the one where a small move changes the "
              "answer.",
        outcomes=Constant(3),
        basis=Constant(BASIS_CATEGORICAL),
        deadline_slots=LogIntUniform(600, 6_000),
        destiny=Categorical(((DESTINY_RESOLVES, 8), (DESTINY_FAILS, 1), (DESTINY_SLEEPY, 1))),
        claim_unit_atoms=Constant(1),
        founding_collateral_atoms=LogIntUniform(80_000_000, 900_000_000),
        participants=IntUniform(2, 4),
        stake_concentration_percent=Constant(80),
        fill_bursts=IntUniform(1, 3),
        fills_per_burst=IntUniform(1, 2),
        fee_basis_points=TRADEABLE_FEE_BAND,
        # Deliberately the narrowest band any archetype draws. At this width the
        # placement draw moves the answer between all three cells easily, which
        # is the point: a hairline market is one whose outcome is genuinely in
        # doubt rather than one whose band is so wide the middle always wins.
        band_volatility_bps=IntUniform(15, 120),
        band_profile=Constant(BAND_PROFILE_UNIFORM),
    ),
    MarketArchetype(
        name="long-tail",
        blurb="Five to seven answers whose regions widen away from the middle: "
              "fine resolution where the coordinate probably lands and coarse "
              "buckets for the moves nobody expects. The archetype about HOW "
              "FAR rather than about WHERE.",
        outcomes=IntUniform(5, 7),
        basis=Constant(BASIS_CATEGORICAL),
        deadline_slots=LogIntUniform(3_000, 40_000),
        destiny=Categorical(((DESTINY_RESOLVES, 6), (DESTINY_SLEEPY, 3), (DESTINY_FAILS, 1))),
        claim_unit_atoms=Constant(1),
        founding_collateral_atoms=LogIntUniform(300_000_000, 3_000_000_000),
        participants=IntUniform(3, 6),
        stake_concentration_percent=Constant(50),
        fill_bursts=IntUniform(2, 4),
        fills_per_burst=IntUniform(1, 3),
        fee_basis_points=TRADEABLE_FEE_BAND,
        band_volatility_bps=IntUniform(200, 900),
        band_profile=Constant(BAND_PROFILE_TIGHT_CENTRE),
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
    ("hairline", 3),
    ("long-tail", 3),
))

# The same world, restricted to the markets a real substrate can actually
# express today. `ladder` and `tent-band` are absent because their basis is not
# foundable (see BASIS_ABSENCE); nothing else about them is wrong, and the day a
# founding driver emits a graded basis this preset should be deleted rather than
# edited.
#
# Six archetypes rather than four, and the two added are categorical by
# construction: `hairline` and `long-tail` differ from the rest in their BAND
# rather than in their basis -- how far the coordinate is assumed to travel and
# how the gaps vary across the width -- which is the one axis of variety a
# substrate that can express exactly one basis kind still has.
FOUNDABLE_ARCHETYPE_MIX = Categorical((
    ("coin-flip", 4),
    ("short-fuse", 2),
    ("wide-field", 3),
    ("quiet-corner", 2),
    ("hairline", 3),
    ("long-tail", 3),
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
    # THE COORDINATE THIS WORLD'S SUBSTRATE WILL OBSERVE, and every band is
    # placed relative to it. It is a fact about the CHAIN rather than about the
    # world: a local validator resolves against a captured Pyth fixture whose
    # price is one constant, so a world drawn for one substrate and run against
    # another would have its bands in the wrong place. The default is the
    # committed local fixture's own coordinate; a devnet world states its own.
    coordinate_anchor: int = LOCAL_PYTH_FIXTURE_COORDINATE_V1

    def describe(self) -> dict:
        return {
            "markets": self.markets,
            "ticks": self.ticks,
            "slots_per_tick": self.slots_per_tick,
            "coordinate_anchor": self.coordinate_anchor,
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
            "outcome_spread": self.outcome_spread(),
            "plan_digest": simcore.digest_of({"markets": markets, "events": events}),
        }

    def outcome_spread(self) -> dict:
        """WHICH CELL each market that can resolve settles into, counted.

        A world whose markets all settle into one cell is a world that measures
        nothing about outcomes, however many markets it has and however varied
        everything else about them is -- and that is exactly the state this
        engine was in before the band was drawn relative to the observed
        coordinate. So it is a health property of a RUN rather than a fact
        somebody has to go looking for: counted here, carried in `world.json`,
        printed by `plan`, and failed by a test.

        Cells are counted as `i/n` -- cell index over the market's own width --
        because cell 3 of four and cell 3 of eleven are not the same answer and
        summing them would be the caption-disagrees-with-its-chart species one
        level down.
        """
        counts: dict = {}
        positions: dict = {}
        for market in self.markets:
            if market.destiny != DESTINY_RESOLVES or market.selected_cell is None:
                continue
            counts[f"{market.selected_cell}/{market.outcome_count}"] = 1 + counts.get(
                f"{market.selected_cell}/{market.outcome_count}", 0
            )
            place = settling_position_tenths(market.selected_cell, market.outcome_count)
            if place is not None:
                positions[place] = positions.get(place, 0) + 1
        total = sum(counts.values())
        ordered = sorted(counts.items(), key=lambda row: (-row[1], row[0]))
        # THE DEGENERACY FLAG IS OVER POSITION, NOT OVER `cell/width`, and the
        # difference is what lets it catch the defect it exists for. Keyed by
        # `cell/width`, "every market landed in its bottom cell" spreads across
        # as many keys as the world has widths and looks diverse; the historical
        # failure -- every observation above every cut -- would have passed.
        # Position normalises the cell to where in its own market it sits, so
        # "always the bottom" and "always the top" are each ONE bucket.
        placed = sum(positions.values())
        by_position = sorted(positions.items(), key=lambda row: (-row[1], row[0]))
        heaviest = by_position[0] if by_position else None
        # A share stated as an exact percentage over integers, never a float:
        # a threshold a reader can check by hand is worth more than one they
        # have to trust.
        share_percent = (100 * heaviest[1]) // placed if heaviest and placed else 0
        return {
            "resolving_markets": total,
            "distinct_cells": len(counts),
            "counts": dict(ordered),
            "coordinate_anchor": self.spec.coordinate_anchor,
            # Markets with a single ordinary cell are counted above and NOT
            # here: a market whose whole coordinate domain is one region has no
            # position to take, and counting it as "the bottom" would make a
            # world of narrow markets look degenerate for being narrow.
            "positioned_markets": placed,
            "position_counts": dict(by_position),
            "distinct_positions": len(positions),
            "heaviest_position_tenths": None if heaviest is None else heaviest[0],
            "heaviest_share_percent": share_percent,
            "degenerate_threshold_percent": DEGENERATE_OUTCOME_SHARE_PERCENT_V1,
            "degenerate": bool(
                placed > 0 and share_percent > DEGENERATE_OUTCOME_SHARE_PERCENT_V1
            ),
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


def _gap_tenths(profile: str, gaps: int) -> list:
    """`gaps` gap multipliers, in tenths of the band's spacing.

    A PROFILE IS A SHAPE OVER THE GAPS AND NOT A SECOND SCALE, and that is
    enforced by construction rather than by hand-tuned tables: whatever shape a
    profile asks for is rescaled so the multipliers average ten. Without the
    rescale "tight-centre" would also mean "narrower", the two axes would be one
    axis twice, and a reader comparing two markets could not tell which of the
    two they were looking at.
    """
    if gaps <= 0:
        return []
    if profile == BAND_PROFILE_UNIFORM:
        return [10] * gaps
    if profile == BAND_PROFILE_RAGGED:
        raw = [_RAGGED_PATTERN_V1[index % len(_RAGGED_PATTERN_V1)] for index in range(gaps)]
    else:
        # Distance from the band's middle in HALF-STEPS, so an even gap count is
        # symmetric with no rounding. The shape is a property of the band rather
        # than of which end it was built from.
        distance = [abs(2 * index - (gaps - 1)) for index in range(gaps)]
        farthest = max(distance)
        if profile == BAND_PROFILE_TIGHT_CENTRE:
            raw = [10 + _PROFILE_SLOPE_TENTHS_V1 * step for step in distance]
        elif profile == BAND_PROFILE_TIGHT_EDGES:
            raw = [10 + _PROFILE_SLOPE_TENTHS_V1 * (farthest - step) for step in distance]
        else:
            raise Refusal(f"unknown band profile {profile!r}")
    total = sum(raw)
    return [max(1, value * 10 * gaps // total) for value in raw]


def _band(outcome_count: int, anchor: int, spacing: int, offset: int, profile: str) -> list:
    """`outcome_count - 2` strictly increasing cuts, placed around an anchor.

    `anchor` is the coordinate the substrate will actually observe; `offset` is
    how far the band's middle sits from it, in the band's own units; `profile`
    decides how the gaps vary across the band. The cuts come back strictly
    increasing and positive, which the founding compiler requires and refuses.
    """
    count = max(0, outcome_count - 2)
    if count == 0:
        return []
    spacing = max(1, int(spacing))
    gaps = [max(1, spacing * tenths // 10) for tenths in _gap_tenths(profile, max(0, count - 1))]
    width = sum(gaps)
    first = anchor + offset - width // 2
    # Keep the whole band positive AND clear of the anchor's own value: a cut
    # exactly at the anchor would make the settling cell depend on whether the
    # chain's comparison is strict, which is a question this module must not
    # have an opinion about.
    lowest = max(1, spacing)
    if first < lowest:
        first = lowest
    cuts = [first]
    for gap in gaps:
        cuts.append(cuts[-1] + gap)
    if anchor in cuts:
        # The WHOLE band moves, not the offending cut. Nudging one cut would
        # change two gaps and make a `uniform` profile stop being uniform for a
        # reason that has nothing to do with the profile.
        cuts = [cut + 1 for cut in cuts]
    return cuts


def settling_cell(coordinate: int, cuts: Sequence[int]) -> int:
    """Which ordinary cell an observed coordinate falls in.

    `outcome_count == len(cuts) + 2`: one open tail below the first cut, one
    region per gap, one open tail above the last, and the explicit failure cell
    last. So an observation at or above `k` cuts is cell `k`, and the answer is
    in `0 ..= len(cuts)` -- never the failure cell, which is reached by a
    deadline rather than by an observation.
    """
    return sum(1 for cut in cuts if coordinate >= cut)


def settling_position_tenths(cell: int, outcome_count: int) -> Optional[int]:
    """WHERE IN ITS OWN MARKET a settled cell sits, in tenths, or `None`.

    Cell 3 of four and cell 3 of eleven are not the same answer, so a histogram
    over raw cell indices compares numbers that do not mean the same thing. This
    normalises: 0 is the bottom open tail, 10 is the top one, and everything
    between is interior.

    `None` for a market with a single ordinary cell -- a width-2 market has no
    cuts, so its whole coordinate domain is one region and there is no position
    for an observation to take. Filing that as "the bottom" would make a world
    of deliberately narrow markets read as degenerate for being narrow.
    """
    ordinary = max(0, int(outcome_count) - 1)
    if ordinary < 2:
        return None
    return (10 * int(cell)) // (ordinary - 1)


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
    fee = int(record("fee_basis_points", archetype.fee_basis_points, f"market/{market_id}/fee"))
    # A RATE IS A BAND NOW, and the refusal that used to stand here is deleted.
    #
    # It read "only zero-fee markets found in one transaction today" and cited
    # FEE_SECOND_TRANSACTION_FOUNDATION_2026_08_30.md. That document is about the
    # Direct HOT FILL's fee leg -- two Custody CPIs the transition co-enables,
    # whose measured floor sat over the 1,400,000 CU ceiling -- and says nothing
    # about founding at all. Fee-bearing foundings were measured landing on a
    # loopback validator on 2026-08-30, at four cells and at six.
    #
    # What survives is only the compiler's own domain. Which rates can be FILLED
    # is a different question and is not decided here: the owned-loopback Direct
    # producer admits exactly one rate, so a market drawn at any other is one
    # this world founds, opens, activates and censuses and never trades -- and
    # the fill says which rate it read and which the release admits.
    if not 0 <= fee <= 10_000:
        raise Refusal(
            f"{market_id} drew a {fee} bp fee, which is outside the 0..10000 a rate can be"
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
    anchor = int(spec.coordinate_anchor)
    volatility_bps = int(record(
        "band_volatility_bps", archetype.band_volatility_bps, f"market/{market_id}/volatility"
    ))
    profile = record("band_profile", archetype.band_profile, f"market/{market_id}/band-profile")
    # WIDTH SCALES WITH THE SQUARE ROOT OF THE WINDOW, which is the random
    # walk's own scaling: a market with twenty times the horizon gets a band
    # about four and a half times as wide, not twenty times. Integer throughout
    # -- `isqrt`, floor division -- because a plan a reader can reproduce by hand
    # is worth more than a fractional basis point.
    cell_count = max(1, int(outcome_count) - 1)
    span_bps = volatility_bps * math.isqrt(max(1, int(deadline_slots))) \
        // max(1, math.isqrt(BAND_WINDOW_REFERENCE_SLOTS_V1))
    band_spacing = max(1, anchor * max(1, span_bps) // (10_000 * cell_count))
    # PLACEMENT, in tenths of the band's own spacing, drawn wide enough to put
    # the anchor in any cell of this market including both open tails. This is
    # the draw that makes outcomes distribute: the boundaries are relative to
    # the coordinate that will actually be observed, and where they sit around
    # it is the random variable.
    reach = BAND_PLACEMENT_TENTHS_PER_CELL_V1 * cell_count
    placement_tenths = int(record(
        "band_placement_tenths", IntUniform(-reach, reach), f"market/{market_id}/band-placement"
    ))
    band_offset = band_spacing * placement_tenths // 10
    cuts = _band(int(outcome_count), anchor, band_spacing, band_offset, profile)
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
            f"{len(cuts)} cuts on a {profile} profile at spacing {band_spacing}, placed "
            f"{band_offset} from the anchor coordinate {anchor}; the spacing is "
            f"{volatility_bps} bp of the anchor scaled by the square root of a "
            f"{deadline_slots}-slot window against a {BAND_WINDOW_REFERENCE_SLOTS_V1}-slot "
            f"reference, over {cell_count} ordinary cells; payoff drawn over the non-failure "
            "cells with the failure cell pinned to zero"
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
        # THE CELL IS DERIVED FROM THE BAND, NOT DRAWN BESIDE IT.
        #
        # It used to be `settle_rng.randrange(outcome_count)` -- a uniform draw
        # with no relation to where the cuts sat -- so the plan's expectation and
        # the chain's answer were two unrelated numbers and nothing in the run
        # ever compared them. That is what let a band three orders of magnitude
        # away from the observed coordinate look healthy: the plan said "cell 4"
        # for reasons of its own while the chain said "top cell" every time.
        #
        # Now it is the cell the anchor falls in, which is what the certificate
        # will say if this substrate resolves this market. The chain still
        # decides; this is the engine stating a checkable expectation instead of
        # an unrelated wish.
        selected_cell = settling_cell(anchor, cuts)
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
    # THE OUTCOME SPREAD, and it is the second line to read before running
    # anything. A world of forty markets that all settle into the same cell has
    # forty copies of one measurement.
    spread = world.outcome_spread()
    lines.append(
        f"outcome spread: {spread['resolving_markets']} resolving markets over "
        f"{spread['distinct_cells']} distinct cells at coordinate "
        f"{world.spec.coordinate_anchor}; {spread['positioned_markets']} of them over "
        f"{spread['distinct_positions']} positions, heaviest "
        f"{spread['heaviest_position_tenths']}/10 at {spread['heaviest_share_percent']}%"
    )
    lines.append("  cells: " + ", ".join(f"{k} x{v}" for k, v in spread["counts"].items()))
    if spread["degenerate"]:
        lines.append(
            f"  DEGENERATE OUTCOME SPREAD. One cell takes "
            f"{spread['heaviest_share_percent']}% of the markets that can resolve, over the "
            f"{spread['degenerate_threshold_percent']}% threshold. The bands are drawn in the "
            "wrong place for the coordinate this substrate observes -- check "
            "WorldSpec.coordinate_anchor against what the chain's source adapter returns."
        )
    return lines


def market_line(market: PlannedMarket) -> str:
    return (
        f"{market.market_id} {market.archetype:<12} {market.outcome_count:>2} cells  "
        f"{market.basis:<20} deadline {market.deadline_slots:>6} slots  "
        f"{market.destiny:<24} {len(market.participants)} participants  "
        f"{market.founding_collateral_atoms} atoms"
    )
