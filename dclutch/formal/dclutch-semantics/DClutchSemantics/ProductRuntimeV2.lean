import Std.Tactic

/-!
# Runtime-width Product semantics

This module owns the semantic shape of the runtime-tail Product successor. A
finite ordered list of exact rational cuts induces one exhaustive, disjoint,
ordered partition of the whole signed-rational line. The failure outcome is a
separate final coordinate and can never alias an ordinary region.

Portfolio coefficients are nonnegative exact rationals over one positive
denominator. `representationFloor` is the single named conversion from that
exact representation to integer claim atoms. Wire decoding, bounded machine
integer refinement, content hashing, account authentication, and SBF execution
remain separately named runtime boundaries.
-/

namespace DClutch.ProductRuntimeV2

structure Coordinate where
  numerator : Int
  denominator : Nat
  deriving DecidableEq, Repr

def Coordinate.valid (coordinate : Coordinate) : Bool :=
  0 < coordinate.denominator

def strictlyIncreasing : List Int → Bool
  | [] | [_] => true
  | left :: right :: rest => left < right && strictlyIncreasing (right :: rest)

/-- Product-owned runtime-width partition of the exact coordinate line. -/
structure ResultDomain where
  cutDenominator : Nat
  cuts : List Int
  deriving DecidableEq, Repr

def ResultDomain.valid (domain : ResultDomain) : Bool :=
  0 < domain.cutDenominator && strictlyIncreasing domain.cuts

def ResultDomain.regionCount (domain : ResultDomain) : Nat :=
  domain.cuts.length + 1

/-- Ordinary regions plus one explicit final failure outcome. -/
def ResultDomain.outcomeCount (domain : ResultDomain) : Nat :=
  domain.regionCount + 1

def rationalLessCut
    (coordinate : Coordinate) (cutDenominator : Nat) (cut : Int) : Bool :=
  coordinate.numerator * cutDenominator < cut * coordinate.denominator

/-- The half-open selector: below the first cut, between adjacent cuts, or at
or above the final cut. Empty cuts describe the whole line as region zero. -/
def selectOrdinaryFrom
    (coordinate : Coordinate) (cutDenominator : Nat) : List Int → Nat
  | [] => 0
  | cut :: rest =>
      if rationalLessCut coordinate cutDenominator cut then 0
      else 1 + selectOrdinaryFrom coordinate cutDenominator rest

def ResultDomain.selectOrdinary
    (domain : ResultDomain) (coordinate : Coordinate) : Nat :=
  selectOrdinaryFrom coordinate domain.cutDenominator domain.cuts

def ResultDomain.failureSelector (domain : ResultDomain) : Nat :=
  domain.regionCount

theorem selectOrdinaryFrom_le_length
    (coordinate : Coordinate) (cutDenominator : Nat) (cuts : List Int) :
    selectOrdinaryFrom coordinate cutDenominator cuts ≤ cuts.length := by
  induction cuts with
  | nil => simp [selectOrdinaryFrom]
  | cons cut rest induction =>
      simp only [selectOrdinaryFrom, List.length_cons]
      split
      · omega
      · omega

/-- Every exact rational coordinate selects one ordinary region. -/
theorem ResultDomain.selection_exhaustive
    (domain : ResultDomain) (coordinate : Coordinate) :
    ∃ selector, selector < domain.regionCount ∧
      domain.selectOrdinary coordinate = selector := by
  refine ⟨domain.selectOrdinary coordinate, ?_, rfl⟩
  unfold ResultDomain.selectOrdinary ResultDomain.regionCount
  have bound := selectOrdinaryFrom_le_length
    coordinate domain.cutDenominator domain.cuts
  omega

/-- A functional selector cannot place one coordinate in two distinct
ordinary regions. This is the disjointness contract consumed by the wire
validator after it establishes strict cut ordering. -/
theorem ResultDomain.selection_disjoint
    (domain : ResultDomain) (coordinate : Coordinate) (left right : Nat)
    (leftSelected : domain.selectOrdinary coordinate = left)
    (rightSelected : domain.selectOrdinary coordinate = right) :
    left = right := by
  rw [← leftSelected, ← rightSelected]

theorem ResultDomain.failure_distinct
    (domain : ResultDomain) (coordinate : Coordinate) :
    domain.selectOrdinary coordinate ≠ domain.failureSelector := by
  obtain ⟨selector, selectorBound, selected⟩ :=
    domain.selection_exhaustive coordinate
  rw [selected]
  unfold ResultDomain.failureSelector
  omega

/-! ## The declared source-to-result scale

An observation is authored on the source's scale and the cuts are authored on
the result's. Nothing above relates the two: `selectOrdinary` compares the two
ratios as they are, which is correct exactly when the scales agree and silently
wrong otherwise. The relation is one signed decimal shift, and this section
makes it an explicit argument of the mapping from an observation to a cell so
that no route can leave it unstated.
-/

/-- Largest absolute decimal shift this release admits between a source scale
and a result scale.

Mathematical bound: the shift multiplies a denominator, the physical refinement
carries denominators in an unsigned 64-bit integer, and `10 ^ 19` is the last
power of ten below `2 ^ 64`. Eighteen is that bound less one decade, and it is
six decades beyond the widest exponent any admitted feed publishes. -/
def maxScaleExponent : Nat := 18

/-- A declared source-to-result decimal shift: the one number saying how an
observation authored on the source's scale relates to cuts authored on the
result's.

Zero is the identity, and it is what a record founded before this factor
existed declares. That is deliberate: it makes the pre-factor reading the
identity reading rather than an undefined one, so a migration has a name for
what an old market means instead of a silence. -/
structure Scale where
  exponent : Int
  deriving DecidableEq, Repr

/-- The scale a record declares when it declares nothing. -/
def Scale.identity : Scale := ⟨0⟩

def Scale.valid (scale : Scale) : Bool := scale.exponent.natAbs ≤ maxScaleExponent

/-- Applying the shift is a rewrite of *one side's denominator*, chosen by the
sign so that the factor is always a multiplication.

This is the whole reason the scaled selector stays exact. Dividing either
numerator by a power of ten would round, and rounding inside a partition test
moves cells; multiplying a denominator cannot. It is also why the physical
refinement needs no wider integer than the unscaled comparison already used. -/
def Scale.observationDenominator (scale : Scale) (denominator : Nat) : Nat :=
  if scale.exponent < 0 then denominator * 10 ^ scale.exponent.natAbs else denominator

def Scale.cutDenominator (scale : Scale) (denominator : Nat) : Nat :=
  if scale.exponent < 0 then denominator else denominator * 10 ^ scale.exponent.natAbs

/-- The observation restated on the cuts' scale. -/
def Coordinate.onCutScale (coordinate : Coordinate) (scale : Scale) : Coordinate :=
  ⟨coordinate.numerator, scale.observationDenominator coordinate.denominator⟩

/-- The sole mapping from an observation to an ordinary cell. It takes the
scale as an argument because the observation alone does not determine a cell:
the same numerator names different cells under different declared factors, and
a route that omits the argument has not chosen the identity, it has failed to
state a choice. -/
def ResultDomain.selectOrdinaryScaled
    (domain : ResultDomain) (coordinate : Coordinate) (scale : Scale) : Nat :=
  selectOrdinaryFrom (coordinate.onCutScale scale)
    (scale.cutDenominator domain.cutDenominator) domain.cuts

@[simp] theorem Scale.observationDenominator_identity (denominator : Nat) :
    Scale.identity.observationDenominator denominator = denominator := by
  simp [Scale.observationDenominator, Scale.identity]

@[simp] theorem Scale.cutDenominator_identity (denominator : Nat) :
    Scale.identity.cutDenominator denominator = denominator := by
  simp [Scale.cutDenominator, Scale.identity]

/-- A record declaring no factor selects exactly what the unscaled selector
selected. This is the migration statement as a theorem: every market founded
before the factor keeps the cell it was paid, and the scaled selector is a
conservative extension of the one it replaces. -/
theorem ResultDomain.selectOrdinaryScaled_identity
    (domain : ResultDomain) (coordinate : Coordinate) :
    domain.selectOrdinaryScaled coordinate Scale.identity =
      domain.selectOrdinary coordinate := by
  unfold ResultDomain.selectOrdinaryScaled ResultDomain.selectOrdinary
    Coordinate.onCutScale
  simp

/-- A positive scale is exactly a positive shift of the cut denominator, and a
negative one exactly a shift of the observation's. Stated so a reader can check
the physical refinement branch by branch. -/
theorem Scale.denominators_split (scale : Scale) (observation cut : Nat) :
    (0 ≤ scale.exponent →
        scale.observationDenominator observation = observation ∧
        scale.cutDenominator cut = cut * 10 ^ scale.exponent.natAbs) ∧
    (scale.exponent < 0 →
        scale.observationDenominator observation =
          observation * 10 ^ scale.exponent.natAbs ∧
        scale.cutDenominator cut = cut) := by
  constructor
  · intro nonnegative
    simp [Scale.observationDenominator, Scale.cutDenominator,
      Int.not_lt.mpr nonnegative]
  · intro negative
    simp [Scale.observationDenominator, Scale.cutDenominator, negative]

/-- `selectOrdinaryFrom` counts the leading cuts the coordinate is at or above.
Every cut before the selector is at or below the coordinate, and the cut at the
selector — when there is one — is strictly above it.

This is the interval characterisation the partition sweeps check numerically:
the selector is not merely *a* number below the region count, it is the index
of the one cell whose lower boundary the coordinate meets and whose upper
boundary it does not. -/
theorem selectOrdinaryFrom_cell
    (coordinate : Coordinate) (cutDenominator : Nat) (cuts : List Int) :
    (∀ cut ∈ cuts.take (selectOrdinaryFrom coordinate cutDenominator cuts),
        ¬ rationalLessCut coordinate cutDenominator cut) ∧
    (∀ cut, (cuts.drop (selectOrdinaryFrom coordinate cutDenominator cuts)).head? = some cut →
        rationalLessCut coordinate cutDenominator cut) := by
  induction cuts with
  | nil => simp [selectOrdinaryFrom]
  | cons cut rest induction =>
      by_cases below : rationalLessCut coordinate cutDenominator cut
      · simp [selectOrdinaryFrom, below]
      · obtain ⟨lower, upper⟩ := induction
        have step : selectOrdinaryFrom coordinate cutDenominator (cut :: rest)
            = selectOrdinaryFrom coordinate cutDenominator rest + 1 := by
          simp [selectOrdinaryFrom, below, Nat.add_comm]
        rw [step]
        refine ⟨?_, ?_⟩
        · intro candidate member
          rw [List.take_succ_cons, List.mem_cons] at member
          rcases member with rfl | member
          · exact below
          · exact lower candidate member
        · intro candidate head
          rw [List.drop_succ_cons] at head
          exact upper candidate head

/-- **The law the defect broke.** Once the declared factor has put the
observation and the cuts on one scale, the observation falls in exactly one
ordinary cell: the selector is below the region count, it is a function of the
observation, every cut beneath it is at or below the reading, and the next cut
is strictly above.

The defect was not that this failed. It was that the two sides were never put
on one scale, so the hypothesis of this theorem was never established and its
conclusion was read anyway. -/
theorem ResultDomain.scaled_selection_in_one_cell
    (domain : ResultDomain) (coordinate : Coordinate) (scale : Scale) :
    ∃ selector,
      selector < domain.regionCount ∧
      domain.selectOrdinaryScaled coordinate scale = selector ∧
      (∀ cut ∈ domain.cuts.take selector,
          ¬ rationalLessCut (coordinate.onCutScale scale)
              (scale.cutDenominator domain.cutDenominator) cut) ∧
      (∀ cut, (domain.cuts.drop selector).head? = some cut →
          rationalLessCut (coordinate.onCutScale scale)
              (scale.cutDenominator domain.cutDenominator) cut) := by
  refine ⟨domain.selectOrdinaryScaled coordinate scale, ?_, rfl, ?_, ?_⟩
  · unfold ResultDomain.selectOrdinaryScaled ResultDomain.regionCount
    have bound := selectOrdinaryFrom_le_length (coordinate.onCutScale scale)
      (scale.cutDenominator domain.cutDenominator) domain.cuts
    omega
  · exact (selectOrdinaryFrom_cell (coordinate.onCutScale scale)
      (scale.cutDenominator domain.cutDenominator) domain.cuts).left
  · exact (selectOrdinaryFrom_cell (coordinate.onCutScale scale)
      (scale.cutDenominator domain.cutDenominator) domain.cuts).right

/-- The scaled selector can never alias the failure outcome either. -/
theorem ResultDomain.scaled_failure_distinct
    (domain : ResultDomain) (coordinate : Coordinate) (scale : Scale) :
    domain.selectOrdinaryScaled coordinate scale ≠ domain.failureSelector := by
  obtain ⟨selector, selectorBound, selected, _, _⟩ :=
    domain.scaled_selection_in_one_cell coordinate scale
  rw [selected]
  unfold ResultDomain.failureSelector
  omega

/-! ### Cohort-14 market B, as a twin

`DUVcCGfjXzp1fBktTCjsAomgrn9S6sxSDziQHoyRiu8A` settled 2026-09-03 with cuts
`9900, 10300` over `100` — dollars, authored in cents — against an observation
of `10062091764` over `1`, a raw Pyth SOL/USD mantissa at exponent `-8`. The
reading is $100.62 and the band is $99 to $103, so the price was inside it. The
market paid the outside cell.

Both selectors below are correct arithmetic. They differ only in whether a
factor was declared, which is the entire finding. -/

def cohort14MarketBDomain : ResultDomain := ⟨100, [9900, 10300]⟩

def cohort14MarketBObservation : Coordinate := ⟨10062091764, 1⟩

/-- What the deployed program computed: no declared factor, so the identity. -/
example :
    cohort14MarketBDomain.selectOrdinaryScaled
      cohort14MarketBObservation Scale.identity = 2 := by decide

/-- What the feed's own exponent says the reading is: inside the band. -/
example :
    cohort14MarketBDomain.selectOrdinaryScaled
      cohort14MarketBObservation ⟨-8⟩ = 1 := by decide

/-- And the factor the founding should have written is a valid one. -/
example : Scale.valid ⟨-8⟩ = true := by decide

/-- Runtime-width exact rational claim recipe. Content identities live in the
physical header; this structure owns only the mathematical coefficients. -/
structure Portfolio where
  denominator : Nat
  coefficients : List Nat
  deriving DecidableEq, Repr

def gcdAll (denominator : Nat) (coefficients : List Nat) : Nat :=
  coefficients.foldl Nat.gcd denominator

def Portfolio.validFor (portfolio : Portfolio) (domain : ResultDomain) : Bool :=
  0 < portfolio.denominator &&
    portfolio.coefficients.length = domain.outcomeCount &&
    portfolio.coefficients.any (· != 0) &&
    gcdAll portfolio.denominator portfolio.coefficients = 1

/-- The sole integer conversion boundary for exact Portfolio coefficients. -/
def representationFloor
    (coefficient scale denominator : Nat) : Nat :=
  if denominator = 0 then 0 else coefficient * scale / denominator

def Portfolio.materialize (portfolio : Portfolio) (scale : Nat) : List Nat :=
  portfolio.coefficients.map fun coefficient =>
    representationFloor coefficient scale portfolio.denominator

theorem Portfolio.materialize_width
    (portfolio : Portfolio) (scale : Nat) :
    (portfolio.materialize scale).length = portfolio.coefficients.length := by
  simp [Portfolio.materialize]

theorem representationFloor_mul_le
    (coefficient scale denominator : Nat) (positive : 0 < denominator) :
    representationFloor coefficient scale denominator * denominator ≤
      coefficient * scale := by
  simp only [representationFloor]
  split
  · omega
  · exact Nat.div_mul_le_self _ _

end DClutch.ProductRuntimeV2
