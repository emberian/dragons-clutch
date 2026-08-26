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
