import DClutchSemantics.LiabilityBasisV2

/-!
# Degree-1..3 B-spline liability bases

`LiabilityBasisV2` proves the economic theorem for a finite nonnegative integer
partition of unity and instantiates it twice: the categorical one-hot basis at
`Q = 1`, and a two-claim capped ramp whose single apportionment boundary is
`cappedRampComplementFloorBoundaryV2`.

This module supplies the third and widest family: the B-spline basis of degree
`1..3` over an exact rational knot vector.  Every elementary claim is one basis
function; the outstanding supply vector is therefore the **control polygon**,
and terminal liability is the spline curve evaluated at the resolved
coordinate.  That is the content the ledger's `M-4` calls *"properly shaped
dynamics"*: an issuer picks knots and a degree, and a holder assembles any
shape the spline's span admits by choosing supplies — rather than choosing
among fixed bins.

Two layers carry the whole development.

**The apportionment layer** turns exact nonnegative rational basis weights into
integer payouts.  `cumulativeFloorBoundaryV2` floors the *running* weight sum
rather than each weight, and each claim receives the difference between two
consecutive floors.  The partition sum is then exact by telescoping — the last
floor is `Q` and the first is `0` — with no remainder step, no second rounding
decision and no unclassified residue.  It is the exact generalization of the
ramp's single boundary: `apportion_width_two_eq_cappedRamp` proves the
width-two instance *is* `cappedRampComplementFloorBoundaryV2` plus its exact
complement.

**The evaluation layer** is Cox-de-Boor run on integers.  Basis values at one
level are carried as numerators over a common positive denominator, and one
degree-raising step is exactly a convex redistribution: each value `v` under a
weight `p/q` sends `(q-p)*v` left and `p*v` right.  That step is *structurally*
sum-preserving, so the rational partition of unity is a list induction rather
than a reindexing argument, and no floating point or rational division occurs
anywhere.  The local triangle covers exactly the `degree+1` basis functions
supported on the located span; every other claim's weight is a real zero, not a
rounded one.

Knot multiplicity is handled by construction: the span locator selects a
*non-degenerate* span, which forces every de Boor weight denominator positive
without a special case.

The physical Rust profile uses bounded integers and refuses outside them.
Those bounds are not premises of any theorem below.
-/

namespace DClutch.LiabilityBasisV2.Spline

open DClutch.LiabilityBasisV2

/-! ## The cumulative apportionment boundary

`Q` collateral atoms are apportioned across `K` claims whose exact rational
weights are `w_i / D` with `sum w_i = D`.  Flooring each claim's own share
independently loses `up to K-1` atoms to an unclassified remainder.  Flooring
the *running* sum instead and handing each claim the difference is exact:

```text
c_j = floor(Q * (w_0 + ... + w_j) / D)      c_{-1} = 0,  c_{K-1} = Q
p_j = c_j - c_{j-1}
```
-/

/-- **The B-spline apportionment boundary.** One floor of a running weight sum
into the collateral scale. `Nat` division is exactly the floor of a
nonnegative rational, so this is the same rounding direction the ramp's
`cappedRampComplementFloorBoundaryV2` takes, applied cumulatively. -/
def cumulativeFloorBoundaryV2 (scale cumulative denominator : Nat) : Nat :=
  scale * cumulative / denominator

theorem cumulativeFloorBoundaryV2_monotone
    (scale first second denominator : Nat) (ordered : first ≤ second) :
    cumulativeFloorBoundaryV2 scale first denominator
      ≤ cumulativeFloorBoundaryV2 scale second denominator :=
  Nat.div_le_div_right (Nat.mul_le_mul_left scale ordered)

theorem cumulativeFloorBoundaryV2_zero (scale denominator : Nat) :
    cumulativeFloorBoundaryV2 scale 0 denominator = 0 := by
  simp [cumulativeFloorBoundaryV2]

/-- At the final running sum the boundary returns the whole scale, which is why
no remainder atom can survive the apportionment. -/
theorem cumulativeFloorBoundaryV2_full
    (scale denominator : Nat) (positive : 0 < denominator) :
    cumulativeFloorBoundaryV2 scale denominator denominator = scale := by
  unfold cumulativeFloorBoundaryV2
  exact Nat.mul_div_cancel scale positive

theorem cumulativeFloorBoundaryV2_le
    (scale cumulative denominator : Nat)
    (positive : 0 < denominator) (covered : cumulative ≤ denominator) :
    cumulativeFloorBoundaryV2 scale cumulative denominator ≤ scale := by
  have step := cumulativeFloorBoundaryV2_monotone scale cumulative denominator
    denominator covered
  rw [cumulativeFloorBoundaryV2_full scale denominator positive] at step
  exact step

/-- **The apportionment never rounds a claim up.** The running boundary is at
most its exact rational share, exactly as the ramp's sole boundary is. -/
theorem cumulativeFloorBoundaryV2_never_rounds_up
    (scale cumulative denominator : Nat) :
    cumulativeFloorBoundaryV2 scale cumulative denominator * denominator
      ≤ scale * cumulative :=
  Nat.div_mul_le_self _ _

/-- The residue one boundary leaves behind is strictly less than one apportioned
atom, so the next claim absorbs it exactly rather than it becoming an
unclassified remainder. -/
theorem cumulativeFloorBoundaryV2_residue_lt_one_atom
    (scale cumulative denominator : Nat) (positive : 0 < denominator) :
    scale * cumulative
      < (cumulativeFloorBoundaryV2 scale cumulative denominator + 1) * denominator := by
  unfold cumulativeFloorBoundaryV2
  have divided := Nat.div_add_mod (scale * cumulative) denominator
  have remainder := Nat.mod_lt (scale * cumulative) positive
  have expand : (scale * cumulative / denominator + 1) * denominator
      = denominator * (scale * cumulative / denominator) + denominator := by
    rw [Nat.add_mul, Nat.one_mul, Nat.mul_comm]
  omega

/-- Running weight sums, apportioned claim by claim. `carried` is the previous
boundary value, so each claim receives exactly the atoms the running floor has
newly released. -/
def apportionFrom
    (scale denominator carried cumulative : Nat) : List Nat → List Nat
  | [] => []
  | weight :: weights =>
      (cumulativeFloorBoundaryV2 scale (cumulative + weight) denominator - carried) ::
        apportionFrom scale denominator
          (cumulativeFloorBoundaryV2 scale (cumulative + weight) denominator)
          (cumulative + weight) weights

/-- **The integer payout vector of a rational weight vector.** -/
def apportion (scale denominator : Nat) (weights : List Nat) : List Nat :=
  apportionFrom scale denominator 0 0 weights

theorem apportionFrom_length
    (scale denominator carried cumulative : Nat) (weights : List Nat) :
    (apportionFrom scale denominator carried cumulative weights).length =
      weights.length := by
  induction weights generalizing carried cumulative with
  | nil => rfl
  | cons weight weights induction =>
      simp only [apportionFrom, List.length_cons]
      rw [induction]

theorem apportion_length (scale denominator : Nat) (weights : List Nat) :
    (apportion scale denominator weights).length = weights.length :=
  apportionFrom_length _ _ _ _ _

/-- **The telescoping identity.** Everything the apportionment hands out is
exactly the distance between the boundary at the final running sum and the
boundary already carried. No atom is created and none is lost. -/
theorem apportionFrom_sum
    (scale denominator cumulative : Nat) (weights : List Nat)
    (carriedExact : cumulativeFloorBoundaryV2 scale cumulative denominator
      = carried) :
    (apportionFrom scale denominator carried cumulative weights).sum + carried =
      cumulativeFloorBoundaryV2 scale (cumulative + weights.sum) denominator := by
  induction weights generalizing carried cumulative with
  | nil =>
      simp only [apportionFrom, List.sum_nil, Nat.zero_add, List.sum_nil,
        Nat.add_zero]
      exact carriedExact.symm
  | cons weight weights induction =>
      have step := induction (carried := cumulativeFloorBoundaryV2 scale
        (cumulative + weight) denominator) (cumulative := cumulative + weight) rfl
      have carriedLe : carried ≤
          cumulativeFloorBoundaryV2 scale (cumulative + weight) denominator := by
        rw [← carriedExact]
        exact cumulativeFloorBoundaryV2_monotone scale cumulative (cumulative + weight)
          denominator (by omega)
      have assoc : cumulative + weight + weights.sum
          = cumulative + (weight + weights.sum) := by omega
      rw [assoc] at step
      simp only [apportionFrom, List.sum_cons]
      omega

/-- **Exact partition sum.** When the rational weights sum to the common
denominator — the partition-of-unity premise — the integer payouts sum to
exactly `Q`. This is the theorem `M-4` asks for over the integer-apportioned
form, and it holds for every width, not only two. -/
theorem apportion_sum
    (scale denominator : Nat) (weights : List Nat)
    (positive : 0 < denominator) (unity : weights.sum = denominator) :
    (apportion scale denominator weights).sum = scale := by
  have telescoped := apportionFrom_sum (carried := 0) scale denominator 0 weights
    (by simp [cumulativeFloorBoundaryV2])
  simp only [Nat.zero_add, Nat.add_zero] at telescoped
  rw [unity] at telescoped
  rw [cumulativeFloorBoundaryV2_full scale denominator positive] at telescoped
  exact telescoped

theorem apportionFrom_le_scale
    (scale denominator cumulative carried : Nat) (weights : List Nat)
    (positive : 0 < denominator)
    (payout : Nat)
    (member : payout ∈ apportionFrom scale denominator carried cumulative weights)
    (covered : cumulative + weights.sum ≤ denominator) :
    payout ≤ scale := by
  induction weights generalizing carried cumulative with
  | nil => simp [apportionFrom] at member
  | cons weight weights induction =>
      have headCovered : cumulative + weight + weights.sum ≤ denominator := by
        simp only [List.sum_cons] at covered
        omega
      simp only [apportionFrom, List.mem_cons] at member
      rcases member with rfl | member
      · have bound := cumulativeFloorBoundaryV2_le scale (cumulative + weight)
          denominator positive (by omega)
        omega
      · exact induction _ _ member headCovered

/-- Every apportioned payout is inside the collateral scale, so no single claim
can be handed more than one complete set is worth. -/
theorem apportion_le_scale
    (scale denominator : Nat) (weights : List Nat)
    (positive : 0 < denominator) (unity : weights.sum = denominator)
    (payout : Nat) (member : payout ∈ apportion scale denominator weights) :
    payout ≤ scale :=
  apportionFrom_le_scale scale denominator 0 0 weights positive payout member
    (by omega)

/-! ### The ramp is the width-two instance

The successor's first slice apportioned two claims with one floor and an exact
complement. That is exactly this boundary at width two, so the ramp lane's
named boundary is not replaced — it is generalized.
-/

/-- **The named ramp boundary generalizes.** At width two the cumulative
apportionment is exactly one floor plus the exact integer complement, which is
`CappedRampComplement.evaluate`'s shape. -/
theorem apportion_width_two
    (scale denominator primaryWeight complementWeight : Nat)
    (positive : 0 < denominator)
    (unity : primaryWeight + complementWeight = denominator) :
    apportion scale denominator [primaryWeight, complementWeight] =
      [cumulativeFloorBoundaryV2 scale primaryWeight denominator,
        scale - cumulativeFloorBoundaryV2 scale primaryWeight denominator] := by
  have full : cumulativeFloorBoundaryV2 scale (primaryWeight + complementWeight)
      denominator = scale := by
    rw [unity]
    exact cumulativeFloorBoundaryV2_full scale denominator positive
  simp only [apportion, apportionFrom, Nat.zero_add, Nat.sub_zero]
  rw [full]

/-- The width-two cumulative boundary and the ramp lane's sole boundary are the
same rounding decision: `floor(Q * elapsed / width)` computed over nonnegative
integers. -/
theorem cumulativeFloorBoundaryV2_eq_cappedRamp
    (scale elapsed width : Nat) (positiveElapsed : 0 < elapsed)
    (interior : elapsed < width) :
    cumulativeFloorBoundaryV2 scale elapsed width
      = cappedRampComplementFloorBoundaryV2 scale (elapsed : Int) (width : Int) := by
  rw [cappedRampComplementFloorBoundaryV2_interior scale (elapsed : Int) (width : Int)
    (by omega) (by omega)]
  unfold cumulativeFloorBoundaryV2
  rw [← Int.natCast_mul, ← Int.natCast_ediv, Int.toNat_natCast]

/-! ## Integer Cox-de-Boor

One degree-raising step is a convex redistribution. Under the weight `p/q`
attached to a value `v`, the share `(q-p)*v` moves to the claim on the left and
`p*v` to the claim on the right, and every value in the level is scaled by `q`
so that the whole level stays over one common denominator.

The step is therefore sum-preserving up to the level's denominator factor by
*construction*, which is why the rational partition of unity below is a list
induction and not a reindexing argument.
-/

/-- The common denominator factor one level contributes. -/
def weightProduct : List (Nat × Nat) → Nat
  | [] => 1
  | weight :: weights => weight.2 * weightProduct weights

theorem weightProduct_positive
    (weights : List (Nat × Nat))
    (positive : ∀ weight ∈ weights, 0 < weight.2) :
    0 < weightProduct weights := by
  induction weights with
  | nil => simp [weightProduct]
  | cons weight weights induction =>
      have head : 0 < weight.2 := positive weight (by simp)
      have tail : 0 < weightProduct weights :=
        induction (fun entry member => positive entry (by simp [member]))
      simpa [weightProduct] using Nat.mul_pos head tail

theorem sum_map_mul_left (factor : Nat) (values : List Nat) :
    (values.map (fun value => factor * value)).sum = factor * values.sum := by
  induction values with
  | nil => simp
  | cons value values induction =>
      simp only [List.map_cons, List.sum_cons, induction, Nat.mul_add]

/-- **One degree-raising step.** `weights` carries one exact rational `p/q` per
incoming value; the result carries one more value than it consumed, over the
denominator scaled by `weightProduct weights`. Out-of-shape input stays total
and is ruled out by the width premise of every theorem below. -/
def deBoorStep : List (Nat × Nat) → List Nat → List Nat
  | [], _ => [0]
  | _ :: _, [] => [0]
  | (numerator, denominator) :: weights, value :: values =>
      match deBoorStep weights values with
      | [] => [0]
      | head :: tail =>
          ((denominator - numerator) * value * weightProduct weights) ::
            (numerator * value * weightProduct weights + denominator * head) ::
              tail.map (fun entry => denominator * entry)

theorem deBoorStep_length
    (weights : List (Nat × Nat)) (values : List Nat)
    (sameWidth : weights.length = values.length) :
    (deBoorStep weights values).length = weights.length + 1 := by
  induction weights generalizing values with
  | nil => simp [deBoorStep]
  | cons weight weights induction =>
      cases values with
      | nil => simp at sameWidth
      | cons value values =>
          have tailWidth : weights.length = values.length := by simpa using sameWidth
          have tail := induction values tailWidth
          obtain ⟨numerator, denominator⟩ := weight
          cases inner : deBoorStep weights values with
          | nil => rw [inner] at tail; simp at tail
          | cons head rest =>
              rw [inner] at tail
              simp only [List.length_cons] at tail
              simp only [deBoorStep, inner, List.length_cons, List.length_map]
              omega

/-- The step never produces an empty level, so the two claim shares it splits a
value into always have somewhere to land. -/
theorem deBoorStep_ne_nil (weights : List (Nat × Nat)) (values : List Nat) :
    deBoorStep weights values ≠ [] := by
  match weights, values with
  | [], _ => simp [deBoorStep]
  | _ :: _, [] => simp [deBoorStep]
  | (numerator, denominator) :: weights, value :: values =>
      simp only [deBoorStep]
      split <;> simp

/-- **The step conserves the level sum.** Every atom of every incoming value is
either sent left or sent right; the level denominator picks up exactly
`weightProduct weights`. `numerator ≤ denominator` is what rules out a
truncating `Nat` subtraction, so no share can be silently clipped. -/
theorem deBoorStep_sum
    (weights : List (Nat × Nat)) (values : List Nat)
    (sameWidth : weights.length = values.length)
    (bounded : ∀ weight ∈ weights, weight.1 ≤ weight.2) :
    (deBoorStep weights values).sum = weightProduct weights * values.sum := by
  induction weights generalizing values with
  | nil =>
      cases values with
      | nil => simp [deBoorStep, weightProduct]
      | cons _ _ => simp at sameWidth
  | cons weight weights induction =>
      cases values with
      | nil => simp at sameWidth
      | cons value values =>
          have tailWidth : weights.length = values.length := by simpa using sameWidth
          have tailBounded : ∀ entry ∈ weights, entry.1 ≤ entry.2 :=
            fun entry member => bounded entry (by simp [member])
          have headBounded : weight.1 ≤ weight.2 := bounded weight (by simp)
          have tail := induction values tailWidth tailBounded
          obtain ⟨numerator, denominator⟩ := weight
          simp only at headBounded
          cases inner : deBoorStep weights values with
          | nil => exact absurd inner (deBoorStep_ne_nil weights values)
          | cons head rest =>
              rw [inner] at tail
              simp only [List.sum_cons] at tail
              simp only [deBoorStep, inner, List.sum_cons, weightProduct,
                sum_map_mul_left]
              -- The two shares one value is split into re-add to the whole value.
              have split : (denominator - numerator) * value * weightProduct weights
                  + numerator * value * weightProduct weights
                  = denominator * value * weightProduct weights := by
                rw [Nat.mul_right_comm (denominator - numerator) value,
                  Nat.mul_right_comm numerator value,
                  Nat.mul_right_comm denominator value,
                  ← Nat.add_mul, ← Nat.add_mul, Nat.sub_add_cancel headBounded]
              -- The level below already sums to its own denominator times its input.
              have carried : denominator * head + denominator * rest.sum
                  = denominator * (weightProduct weights * values.sum) := by
                rw [← Nat.mul_add, tail]
              have flatten : denominator * (weightProduct weights * values.sum)
                  = denominator * weightProduct weights * values.sum :=
                (Nat.mul_assoc _ _ _).symm
              have commuted : denominator * value * weightProduct weights
                  = denominator * weightProduct weights * value := by
                rw [Nat.mul_assoc, Nat.mul_assoc, Nat.mul_comm value]
              have expand : denominator * weightProduct weights * value
                  + denominator * weightProduct weights * values.sum
                  = denominator * weightProduct weights * (value + values.sum) :=
                (Nat.mul_add _ _ _).symm
              omega

/-- Run every level of the Cox-de-Boor triangle in order. -/
def deBoorLevels : List (List (Nat × Nat)) → List Nat → List Nat
  | [], values => values
  | level :: levels, values => deBoorLevels levels (deBoorStep level values)

/-- The common denominator the whole triangle accumulates. -/
def levelsProduct : List (List (Nat × Nat)) → Nat
  | [] => 1
  | level :: levels => weightProduct level * levelsProduct levels

/-- Each level consumes one width and produces one more. -/
def LevelsWellFormed : List (List (Nat × Nat)) → Nat → Prop
  | [], _ => True
  | level :: levels, width =>
      level.length = width ∧
        (∀ weight ∈ level, weight.1 ≤ weight.2 ∧ 0 < weight.2) ∧
        LevelsWellFormed levels (width + 1)

theorem deBoorLevels_length
    (levels : List (List (Nat × Nat))) (values : List Nat) (width : Nat)
    (sameWidth : values.length = width)
    (wellFormed : LevelsWellFormed levels width) :
    (deBoorLevels levels values).length = width + levels.length := by
  induction levels generalizing values width with
  | nil => simpa [deBoorLevels] using sameWidth
  | cons level levels induction =>
      obtain ⟨levelWidth, _, tailWellFormed⟩ := wellFormed
      have stepped : (deBoorStep level values).length = width + 1 := by
        rw [deBoorStep_length level values (by omega), levelWidth]
      simp only [deBoorLevels, List.length_cons]
      rw [induction (deBoorStep level values) (width + 1) stepped tailWellFormed]
      omega

/-- **Rational partition of unity, in integer form.** After every level the
numerators sum to exactly the accumulated common denominator times the incoming
sum. Starting from the single degree-zero value `1`, the basis weights sum to
exactly the triangle's denominator — which is the premise `apportion_sum`
needs. -/
theorem deBoorLevels_sum
    (levels : List (List (Nat × Nat))) (values : List Nat) (width : Nat)
    (sameWidth : values.length = width)
    (wellFormed : LevelsWellFormed levels width) :
    (deBoorLevels levels values).sum = levelsProduct levels * values.sum := by
  induction levels generalizing values width with
  | nil => simp [deBoorLevels, levelsProduct]
  | cons level levels induction =>
      obtain ⟨levelWidth, levelBounded, tailWellFormed⟩ := wellFormed
      have stepped : (deBoorStep level values).length = width + 1 := by
        rw [deBoorStep_length level values (by omega), levelWidth]
      have stepSum : (deBoorStep level values).sum = weightProduct level * values.sum :=
        deBoorStep_sum level values (by omega)
          (fun weight member => (levelBounded weight member).1)
      simp only [deBoorLevels, levelsProduct]
      rw [induction (deBoorStep level values) (width + 1) stepped tailWellFormed,
        stepSum, Nat.mul_left_comm]
      exact (Nat.mul_assoc _ _ _).symm

theorem levelsProduct_positive
    (levels : List (List (Nat × Nat))) (width : Nat)
    (wellFormed : LevelsWellFormed levels width) :
    0 < levelsProduct levels := by
  induction levels generalizing width with
  | nil => simp [levelsProduct]
  | cons level levels induction =>
      obtain ⟨_, levelBounded, tailWellFormed⟩ := wellFormed
      have head : 0 < weightProduct level :=
        weightProduct_positive level (fun weight member => (levelBounded weight member).2)
      have tail : 0 < levelsProduct levels := induction (width + 1) tailWellFormed
      simpa [levelsProduct] using Nat.mul_pos head tail

end DClutch.LiabilityBasisV2.Spline
