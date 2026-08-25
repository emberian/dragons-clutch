import Std.Tactic

/-!
# Exact finite-basis Product payoffs

This file gives the successor Product semantic basis.  A Product owns one
canonical, ordered result partition.  Payoff programs refer to its knots by
index, so a ramp or tent cannot smuggle in a second copy of the result
ontology.  The program is finite data interpreted by one width-independent
evaluator.

Coordinates and payouts are exact natural numbers in explicitly named units.
`payoffInterpolationFloor` is the sole rounding boundary: it rounds an
interior rational linear interpolation down to a scaled payout integer.  There
are no floating-point values.

The structural liability bound is conservative rather than purportedly
minimal.  It is nevertheless sufficient collateral for every result, proved
below.  Fixed-width overflow checks, account authenticity, serialization,
Solana execution, and transaction rollback remain adapter obligations.
-/

namespace DClutch.Product

def valueAt (values : List Nat) (index : Nat) : Nat :=
  values[index]?.getD 0

def strictlyIncreasing : List Nat → Bool
  | [] | [_] => true
  | left :: right :: rest => left < right && strictlyIncreasing (right :: rest)

/-! ## Canonical result partition -/

/-- The sole owner of result coordinates and their canonical partition.

`coordinateUnitId` identifies the exact integer coordinate unit.  The knots
include both domain endpoints.  Adjacent knots define ordered cells; an
interior knot belongs to the cell on its right, and the last endpoint belongs
to the final cell. -/
structure ResultDomain where
  domainId : Nat
  coordinateUnitId : Nat
  knots : List Nat
  deriving DecidableEq, Repr

def ResultDomain.valid (domain : ResultDomain) : Bool :=
  domain.domainId != 0 && domain.coordinateUnitId != 0 &&
    2 ≤ domain.knots.length && strictlyIncreasing domain.knots

def ResultDomain.lower (domain : ResultDomain) : Nat :=
  domain.knots.head?.getD 0

def ResultDomain.upper (domain : ResultDomain) : Nat :=
  domain.knots.getLast?.getD 0

def ResultDomain.inDomain (domain : ResultDomain) (coordinate : Nat) : Bool :=
  domain.lower ≤ coordinate && coordinate ≤ domain.upper

def ResultDomain.segmentCount (domain : ResultDomain) : Nat :=
  domain.knots.length - 1

def ResultDomain.interiorCuts (domain : ResultDomain) : List Nat :=
  domain.knots.drop 1 |>.dropLast

/-- Total canonical cell selector.  The `min` is a defensive totalization for
invalid domains and out-of-domain coordinates; valid Products admit only
coordinates accepted by `inDomain`. -/
def ResultDomain.cellIndex (domain : ResultDomain) (coordinate : Nat) : Nat :=
  Nat.min
    (domain.interiorCuts.countP fun cut => cut ≤ coordinate)
    (domain.segmentCount - 1)

/-- Exact finite coordinate enumeration used by an offchain compiler to check
a categorical error certificate.  This is semantic/compiler machinery, not a
claim that an onchain adapter should allocate or scan this list. -/
def ResultDomain.coordinates (domain : ResultDomain) : List Nat :=
  (List.range (domain.upper - domain.lower + 1)).map fun offset =>
    domain.lower + offset

def ResultDomain.assignedTo
    (domain : ResultDomain) (coordinate cell : Nat) : Prop :=
  domain.cellIndex coordinate = cell

theorem ResultDomain.valid_has_segment
    (domain : ResultDomain) (valid : domain.valid = true) :
    0 < domain.segmentCount := by
  simp only [ResultDomain.valid, Bool.and_eq_true, decide_eq_true_eq,
    bne_iff_ne] at valid
  unfold ResultDomain.segmentCount
  omega

/-- Every coordinate has exactly one canonical selector result.  Admission
separately requires that the coordinate is inside a valid domain. -/
theorem ResultDomain.partition_exhaustive
    (domain : ResultDomain) (coordinate : Nat) :
    ∃ cell, domain.assignedTo coordinate cell := by
  exact ⟨domain.cellIndex coordinate, rfl⟩

theorem ResultDomain.partition_disjoint
    (domain : ResultDomain) (coordinate left right : Nat)
    (leftAssigned : domain.assignedTo coordinate left)
    (rightAssigned : domain.assignedTo coordinate right) :
    left = right := by
  exact leftAssigned.symm.trans rightAssigned

theorem ResultDomain.cellIndex_bounded
    (domain : ResultDomain) (valid : domain.valid = true) (coordinate : Nat) :
    domain.cellIndex coordinate < domain.segmentCount := by
  have positive := domain.valid_has_segment valid
  unfold ResultDomain.cellIndex
  have bounded :
      Nat.min
          (domain.interiorCuts.countP fun cut => cut ≤ coordinate)
          (domain.segmentCount - 1) ≤
        domain.segmentCount - 1 := Nat.min_le_right _ _
  omega

theorem ResultDomain.coordinates_complete
    (domain : ResultDomain) (coordinate : Nat)
    (inside : domain.inDomain coordinate = true) :
    coordinate ∈ domain.coordinates := by
  simp only [ResultDomain.inDomain, Bool.and_eq_true, decide_eq_true_eq] at inside
  unfold ResultDomain.coordinates
  rw [List.mem_map]
  refine ⟨coordinate - domain.lower, ?_, ?_⟩
  · simp only [List.mem_range]
    omega
  · omega

/-! ## Finite exact payoff basis -/

/-- Piecewise-linear shapes refer only to Product-owned knot indices. -/
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

def Shape.validFor (shape : Shape) (domain : ResultDomain) : Bool :=
  match shape with
  | .constant => true
  | .rampUp left right | .rampDown left right =>
      left < right && right < domain.knots.length
  | .tent left peak right =>
      left < peak && peak < right && right < domain.knots.length

/-- A finite, width-independent payoff program.  `payoutScale` is the exact
integer denominator understood by the collateral Realm; evaluation returns
amounts in those scaled units and does not round at that unit boundary. -/
structure Payoff where
  payoutScale : Nat
  terms : List Term
  deriving DecidableEq, Repr

/-- A Product owns both its result partition and its interpreted payoff data.
There is no separate per-venue copy of either fact. -/
structure Product where
  productId : Nat
  domain : ResultDomain
  payoff : Payoff
  deriving DecidableEq, Repr

def Product.valid (product : Product) : Bool :=
  product.productId != 0 && product.domain.valid &&
    0 < product.payoff.payoutScale && !product.payoff.terms.isEmpty &&
    product.payoff.terms.all fun term => term.shape.validFor product.domain

/-- **The one named rounding boundary.**  This is rational linear
interpolation rounded toward zero (down, because all values are nonnegative).
The `min` is a defensive clamp; admitted ramp calls pass `elapsed ≤ width`,
where the unclamped quotient already lies below `amplitude`. -/
def payoffInterpolationFloor
    (amplitude elapsed width : Nat) : Nat :=
  if width = 0 then 0
  else Nat.min amplitude ((amplitude * elapsed) / width)

def rampUp
    (amplitude left right coordinate : Nat) : Nat :=
  if coordinate ≤ left then 0
  else if right ≤ coordinate then amplitude
  else payoffInterpolationFloor amplitude (coordinate - left) (right - left)

def rampDown
    (amplitude left right coordinate : Nat) : Nat :=
  if coordinate ≤ left then amplitude
  else if right ≤ coordinate then 0
  else payoffInterpolationFloor amplitude (right - coordinate) (right - left)

def tent
    (amplitude left peak right coordinate : Nat) : Nat :=
  Nat.min
    (rampUp amplitude left peak coordinate)
    (rampDown amplitude peak right coordinate)

def Term.evaluate (domain : ResultDomain) (term : Term) (coordinate : Nat) : Nat :=
  match term.shape with
  | .constant => term.amplitude
  | .rampUp left right =>
      rampUp term.amplitude (valueAt domain.knots left)
        (valueAt domain.knots right) coordinate
  | .rampDown left right =>
      rampDown term.amplitude (valueAt domain.knots left)
        (valueAt domain.knots right) coordinate
  | .tent left peak right =>
      tent term.amplitude (valueAt domain.knots left)
        (valueAt domain.knots peak) (valueAt domain.knots right) coordinate

def Payoff.evaluate
    (domain : ResultDomain) (payoff : Payoff) (coordinate : Nat) : Nat :=
  payoff.terms.map (fun term => term.evaluate domain coordinate) |>.sum

def Product.evaluate? (product : Product) (coordinate : Nat) : Option Nat :=
  if product.valid && product.domain.inDomain coordinate then
    some (product.payoff.evaluate product.domain coordinate)
  else
    none

/-- A sound structural liability bound.  This is deliberately not called a
minimal or optimal bound. -/
def Payoff.liabilityBound (payoff : Payoff) : Nat :=
  payoff.terms.map Term.amplitude |>.sum

/-- Executable collateral criterion for one unit of this Product. -/
def Product.collateralizedBy (product : Product) (available : Nat) : Bool :=
  product.valid && product.payoff.liabilityBound ≤ available

theorem payoffInterpolationFloor_le
    (amplitude elapsed width : Nat) :
    payoffInterpolationFloor amplitude elapsed width ≤ amplitude := by
  unfold payoffInterpolationFloor
  split
  · exact Nat.zero_le _
  · exact Nat.min_le_left _ _

/-- On every admitted interior segment the defensive clamp is inactive, so
the evaluator is exactly the floor of the rational linear interpolation. -/
theorem payoffInterpolationFloor_eq_quotient
    (amplitude elapsed width : Nat)
    (widthPositive : 0 < width) (elapsedBound : elapsed ≤ width) :
    payoffInterpolationFloor amplitude elapsed width =
      (amplitude * elapsed) / width := by
  have multiplied := Nat.mul_le_mul_left amplitude elapsedBound
  have quotientBound : (amplitude * elapsed) / width ≤ amplitude := by
    apply Nat.div_le_of_le_mul
    simpa [Nat.mul_comm] using multiplied
  simp [payoffInterpolationFloor, Nat.ne_of_gt widthPositive, quotientBound]

theorem rampUp_le
    (amplitude left right coordinate : Nat) :
    rampUp amplitude left right coordinate ≤ amplitude := by
  unfold rampUp
  split
  · exact Nat.zero_le _
  · split
    · exact Nat.le_refl _
    · exact payoffInterpolationFloor_le ..

theorem rampDown_le
    (amplitude left right coordinate : Nat) :
    rampDown amplitude left right coordinate ≤ amplitude := by
  unfold rampDown
  split
  · exact Nat.le_refl _
  · split
    · exact Nat.zero_le _
    · exact payoffInterpolationFloor_le ..

theorem tent_le
    (amplitude left peak right coordinate : Nat) :
    tent amplitude left peak right coordinate ≤ amplitude := by
  exact Nat.le_trans (Nat.min_le_left _ _)
    (rampUp_le amplitude left peak coordinate)

theorem Term.evaluate_le_amplitude
    (domain : ResultDomain) (term : Term) (coordinate : Nat) :
    term.evaluate domain coordinate ≤ term.amplitude := by
  cases term with
  | mk shape amplitude =>
      cases shape <;> simp only [Term.evaluate]
      · exact Nat.le_refl _
      · exact rampUp_le ..
      · exact rampDown_le ..
      · exact tent_le ..

theorem termList_evaluate_le_amplitudes
    (domain : ResultDomain) (terms : List Term) (coordinate : Nat) :
    (terms.map fun term => term.evaluate domain coordinate).sum ≤
      (terms.map Term.amplitude).sum := by
  induction terms with
  | nil => simp
  | cons term rest induction =>
      simp only [List.map_cons, List.sum_cons]
      exact Nat.add_le_add (term.evaluate_le_amplitude domain coordinate) induction

theorem Payoff.evaluate_le_liabilityBound
    (domain : ResultDomain) (payoff : Payoff) (coordinate : Nat) :
    payoff.evaluate domain coordinate ≤ payoff.liabilityBound := by
  exact termList_evaluate_le_amplitudes domain payoff.terms coordinate

/-- The bounded-liability theorem used by a collateral adapter. -/
theorem Product.collateral_sufficient
    (product : Product) (available coordinate : Nat)
    (criterion : product.collateralizedBy available = true) :
    product.payoff.evaluate product.domain coordinate ≤ available := by
  simp only [Product.collateralizedBy, Bool.and_eq_true,
    decide_eq_true_eq] at criterion
  exact Nat.le_trans
    (product.payoff.evaluate_le_liabilityBound product.domain coordinate)
    criterion.2

/-! ## Honest categorical approximation -/

/-- A categorical projection is explicitly tied to the Product-owned domain.
It has one scaled payout per canonical cell. -/
structure CategoricalApproximation where
  domainId : Nat
  values : List Nat
  deriving DecidableEq, Repr

def CategoricalApproximation.validFor
    (approximation : CategoricalApproximation) (product : Product) : Bool :=
  approximation.domainId = product.domain.domainId &&
    approximation.values.length = product.domain.segmentCount &&
    approximation.values.all fun value => value ≤ product.payoff.liabilityBound

/-- Total categorical projection.  `validFor` is the admission boundary which
requires every submitted value to lie below the proved structural bound. -/
def CategoricalApproximation.evaluate
    (approximation : CategoricalApproximation) (product : Product)
    (coordinate : Nat) : Nat :=
  valueAt approximation.values (product.domain.cellIndex coordinate)

theorem valueAt_le_of_all
    (values : List Nat) (bound index : Nat)
    (bounded : values.all (fun value => value ≤ bound) = true) :
    valueAt values index ≤ bound := by
  induction values generalizing index with
  | nil => simp [valueAt]
  | cons head tail induction =>
      simp only [List.all_cons, Bool.and_eq_true, decide_eq_true_eq] at bounded
      cases index with
      | zero => simpa [valueAt] using bounded.1
      | succ index =>
          simpa [valueAt] using induction index bounded.2

def absoluteError (exact approximate : Nat) : Nat :=
  if exact ≤ approximate then approximate - exact else exact - approximate

theorem absoluteError_le_of_bounded
    (exact approximate bound : Nat)
    (exactBound : exact ≤ bound) (approximateBound : approximate ≤ bound) :
    absoluteError exact approximate ≤ bound := by
  simp only [absoluteError]
  split <;> omega

/-- Sound categorical approximation statement.

Every domain-bound categorical projection has a mechanically proved global
error bound equal to the Product's structural liability bound.  A compiler may
certify a much tighter per-Product tolerance, but it may not claim categorical
identity merely because the widths match. -/
theorem categorical_approximation_sound
    (product : Product) (approximation : CategoricalApproximation)
    (coordinate : Nat)
    (_productValid : product.valid = true)
    (_domainBound : product.domain.inDomain coordinate = true)
    (approximationValid : approximation.validFor product = true) :
    absoluteError
        (product.payoff.evaluate product.domain coordinate)
        (approximation.evaluate product coordinate) ≤
      product.payoff.liabilityBound := by
  apply absoluteError_le_of_bounded
  · exact product.payoff.evaluate_le_liabilityBound product.domain coordinate
  · simp only [CategoricalApproximation.validFor, Bool.and_eq_true,
      decide_eq_true_eq] at approximationValid
    exact valueAt_le_of_all approximation.values product.payoff.liabilityBound
      (product.domain.cellIndex coordinate) approximationValid.2

/-- Exact, executable compiler-side check for a sharper categorical error
tolerance over every integer coordinate in the Product-owned domain. -/
def CategoricalApproximation.certifiesError
    (approximation : CategoricalApproximation) (product : Product)
    (tolerance : Nat) : Bool :=
  approximation.validFor product && product.valid &&
    product.domain.coordinates.all fun coordinate =>
      absoluteError
          (product.payoff.evaluate product.domain coordinate)
          (approximation.evaluate product coordinate) ≤ tolerance

/-- A successful exhaustive finite-domain certificate implies its advertised
pointwise error bound.  The theorem does not trust a same-width vector or an
unchecked compiler assertion. -/
theorem checked_categorical_approximation_sound
    (product : Product) (approximation : CategoricalApproximation)
    (tolerance coordinate : Nat)
    (certificate : approximation.certifiesError product tolerance = true)
    (inside : product.domain.inDomain coordinate = true) :
    absoluteError
        (product.payoff.evaluate product.domain coordinate)
        (approximation.evaluate product coordinate) ≤ tolerance := by
  simp only [CategoricalApproximation.certifiesError, Bool.and_eq_true] at certificate
  have member := product.domain.coordinates_complete coordinate inside
  have point := (List.all_eq_true.mp certificate.2) coordinate member
  exact of_decide_eq_true point

end DClutch.Product
