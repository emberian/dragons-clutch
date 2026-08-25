import Std.Tactic

/-!
# Total signed-rational Product payoffs

This module owns the V2 semantic correction needed to join Product payoff
evidence to canonical Source resolution certificates. Coordinates are exact
signed rationals. Knots are signed numerators over one positive Product-owned
denominator. Ramp and tent tails clamp explicitly, so every coordinate with a
positive denominator has a payout.

`interpolationFloor` is the sole rounding boundary. It floors only the final
nonnegative rational interpolation into the Product's scaled payout unit; no
intermediate integer-coordinate quantization is permitted.

The theorems below cover total semantic evaluation and the conservative
sum-of-amplitudes bound. Fixed-width codecs, 256-bit arithmetic refinement,
account authentication, SBF execution, CPI, and rollback remain separately
named translation/runtime boundaries.
-/

namespace DClutch.ProductV2

/-- Exact signed-rational coordinate carried by Source evidence. -/
structure RationalCoordinate where
  numerator : Int
  denominator : Nat
  deriving DecidableEq, Repr

def RationalCoordinate.valid (coordinate : RationalCoordinate) : Bool :=
  0 < coordinate.denominator

/-- Product-owned signed knots over one positive common denominator. -/
structure ResultLine where
  domainId : Nat
  coordinateUnitId : Nat
  knotDenominator : Nat
  knots : List Int
  deriving DecidableEq, Repr

def strictlyIncreasing : List Int → Bool
  | [] | [_] => true
  | left :: right :: rest => left < right && strictlyIncreasing (right :: rest)

def ResultLine.valid (line : ResultLine) : Bool :=
  line.domainId != 0 && line.coordinateUnitId != 0 &&
    0 < line.knotDenominator && 2 ≤ line.knots.length &&
    strictlyIncreasing line.knots

def valueAt (values : List Int) (index : Nat) : Int :=
  values[index]?.getD 0

/-- Exact rational comparison without coordinate quantization. -/
def coordinateLeKnot
    (coordinate : RationalCoordinate) (line : ResultLine) (knot : Int) : Bool :=
  coordinate.numerator * line.knotDenominator ≤
    knot * coordinate.denominator

/-- Finite payoff shapes refer only to Product-owned knot indices. -/
inductive Shape where
  | constant
  | rampUp (left right : Nat)
  | rampDown (left right : Nat)
  | tent (left peak right : Nat)
  deriving DecidableEq, Repr

/-- One nonnegative scaled payout basis term. -/
structure Term where
  shape : Shape
  amplitude : Nat
  deriving DecidableEq, Repr

def Shape.validFor (shape : Shape) (line : ResultLine) : Bool :=
  match shape with
  | .constant => true
  | .rampUp left right | .rampDown left right =>
      left < right && right < line.knots.length
  | .tent left peak right =>
      left < peak && peak < right && right < line.knots.length

/-- **The sole V2 rounding boundary.** The admitted interior supplies positive
`elapsed < width`; defensive invalid inputs map to zero. -/
def interpolationFloor (amplitude : Nat) (elapsed width : Int) : Nat :=
  if elapsed ≤ 0 || width ≤ 0 then 0
  else Nat.min amplitude (Int.toNat ((amplitude * elapsed) / width))

def rampUp
    (amplitude : Nat) (left right : Int) (line : ResultLine)
    (coordinate : RationalCoordinate) : Nat :=
  if coordinateLeKnot coordinate line left then 0
  else if coordinateLeKnot coordinate line right then
    interpolationFloor amplitude
      (coordinate.numerator * line.knotDenominator -
        left * coordinate.denominator)
      ((right - left) * coordinate.denominator)
  else amplitude

def rampDown
    (amplitude : Nat) (left right : Int) (line : ResultLine)
    (coordinate : RationalCoordinate) : Nat :=
  if coordinateLeKnot coordinate line left then amplitude
  else if coordinateLeKnot coordinate line right then
    interpolationFloor amplitude
      (right * coordinate.denominator -
        coordinate.numerator * line.knotDenominator)
      ((right - left) * coordinate.denominator)
  else 0

def tent
    (amplitude : Nat) (left peak right : Int) (line : ResultLine)
    (coordinate : RationalCoordinate) : Nat :=
  Nat.min
    (rampUp amplitude left peak line coordinate)
    (rampDown amplitude peak right line coordinate)

def Term.evaluate
    (line : ResultLine) (term : Term) (coordinate : RationalCoordinate) : Nat :=
  match term.shape with
  | .constant => term.amplitude
  | .rampUp left right =>
      rampUp term.amplitude (valueAt line.knots left)
        (valueAt line.knots right) line coordinate
  | .rampDown left right =>
      rampDown term.amplitude (valueAt line.knots left)
        (valueAt line.knots right) line coordinate
  | .tent left peak right =>
      tent term.amplitude (valueAt line.knots left)
        (valueAt line.knots peak) (valueAt line.knots right) line coordinate

/-- A finite, width-independent payoff program. -/
structure Payoff where
  payoutScale : Nat
  terms : List Term
  deriving DecidableEq, Repr

/-- Product owns the result line and interpreted payoff data. -/
structure Product where
  productId : Nat
  line : ResultLine
  payoff : Payoff
  deriving DecidableEq, Repr

def Product.valid (product : Product) : Bool :=
  product.productId != 0 && product.line.valid &&
    0 < product.payoff.payoutScale && !product.payoff.terms.isEmpty &&
    product.payoff.terms.all fun term =>
      0 < term.amplitude && term.shape.validFor product.line

/-- Total exact-rational evaluation. Admission separately requires valid
Product data and a positive coordinate denominator. -/
def Product.evaluate
    (product : Product) (coordinate : RationalCoordinate) : Nat :=
  product.payoff.terms.map (fun term =>
    term.evaluate product.line coordinate) |>.sum

/-- Conservative structural liability bound; it is not claimed minimal. -/
def Payoff.liabilityBound (payoff : Payoff) : Nat :=
  payoff.terms.map Term.amplitude |>.sum

theorem interpolationFloor_le
    (amplitude : Nat) (elapsed width : Int) :
    interpolationFloor amplitude elapsed width ≤ amplitude := by
  unfold interpolationFloor
  split
  · omega
  · exact Nat.min_le_left _ _

theorem rampUp_le
    (amplitude : Nat) (left right : Int) (line : ResultLine)
    (coordinate : RationalCoordinate) :
    rampUp amplitude left right line coordinate ≤ amplitude := by
  unfold rampUp
  split
  · omega
  · split
    · exact interpolationFloor_le _ _ _
    · omega

theorem rampDown_le
    (amplitude : Nat) (left right : Int) (line : ResultLine)
    (coordinate : RationalCoordinate) :
    rampDown amplitude left right line coordinate ≤ amplitude := by
  unfold rampDown
  split
  · omega
  · split
    · exact interpolationFloor_le _ _ _
    · omega

theorem tent_le
    (amplitude : Nat) (left peak right : Int) (line : ResultLine)
    (coordinate : RationalCoordinate) :
    tent amplitude left peak right line coordinate ≤ amplitude := by
  exact Nat.le_trans (Nat.min_le_left _ _)
    (rampUp_le amplitude left peak line coordinate)

theorem Term.evaluate_le_amplitude
    (line : ResultLine) (term : Term) (coordinate : RationalCoordinate) :
    term.evaluate line coordinate ≤ term.amplitude := by
  cases term with
  | mk shape amplitude =>
      cases shape <;> simp only [Term.evaluate]
      · omega
      · exact rampUp_le _ _ _ _ _
      · exact rampDown_le _ _ _ _ _
      · exact tent_le _ _ _ _ _ _

theorem termList_evaluate_le_amplitudes
    (line : ResultLine) (terms : List Term)
    (coordinate : RationalCoordinate) :
    (terms.map fun term => term.evaluate line coordinate).sum ≤
      (terms.map Term.amplitude).sum := by
  induction terms with
  | nil => simp
  | cons head tail induction =>
      simp only [List.map_cons, List.sum_cons]
      exact Nat.add_le_add
        (head.evaluate_le_amplitude line coordinate) induction

/-- Every signed-rational coordinate is covered by the conservative bound. -/
theorem Product.evaluate_le_liabilityBound
    (product : Product) (coordinate : RationalCoordinate) :
    product.evaluate coordinate ≤ product.payoff.liabilityBound := by
  exact termList_evaluate_le_amplitudes
    product.line product.payoff.terms coordinate

/-- Semantic totality: evaluation always produces a payout; callers need not
invent an integral-coordinate or bounded-domain fallback. -/
theorem Product.evaluate_total
    (product : Product) (coordinate : RationalCoordinate) :
    ∃ payout, product.evaluate coordinate = payout := by
  exact ⟨product.evaluate coordinate, rfl⟩

end DClutch.ProductV2
