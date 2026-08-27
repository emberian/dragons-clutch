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
ramp's single boundary, and that is proved rather than asserted:
`cumulativeFloorBoundaryV2_eq_cappedRamp` shows the boundary itself is
`cappedRampComplementFloorBoundaryV2`, and `apportion_width_two` shows the
width-two apportionment is that one floor plus its exact integer complement.

Exactness of the total is not on its own a per-claim guarantee — an
apportionment handing claim zero everything would also sum to `Q`.
`apportion_within_one_atom` supplies the other half: no claim is ever more than
one collateral atom from its exact rational share.  The rule is *not*
reflection symmetric, unlike a largest-remainder rule; that is a real
difference and it is recorded rather than hidden.

**The evaluation layer** is Cox-de-Boor run on integers.  Basis values at one
level are carried as numerators over a common positive denominator, and one
degree-raising step is exactly a convex redistribution: each value `v` under a
weight `p/q` sends `(q-p)*v` left and `p*v` right.  Sum preservation is then a
list induction under `p ≤ q` rather than the usual index-shifting argument, and
no floating point and no rational division occurs anywhere.  The local triangle
covers exactly the `degree+1` basis functions supported on the located span;
every other claim's weight is a real zero, not a rounded one.

Knot multiplicity is admitted rather than refused.  `spanCandidates` keeps only
*non-degenerate* spans, so a repeated knot simply collapses a span and the
locator skips it — and a non-degenerate span is exactly what makes every de
Boor denominator positive, with no special case for multiplicity anywhere.
`locateSpan` is total, so it still has a fallback when a profile has no
non-degenerate span at all; `SplineProfile.admits` is the decidable check that
rules that fallback out, and it is a premise of every theorem below rather than
an assumption buried in a constructor.

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

/-! ### How far the apportionment can move a claim

Exactness of the total says nothing on its own about any individual claim: an
apportionment that gave claim zero everything would still sum to `Q`. The
bound below is what makes the boundary honest per claim.
-/

theorem apportionFrom_within_one_atom
    (scale denominator cumulative carried : Nat) (weights : List Nat)
    (positive : 0 < denominator)
    (carriedExact : cumulativeFloorBoundaryV2 scale cumulative denominator = carried)
    (pair : Nat × Nat)
    (member : pair ∈
      (apportionFrom scale denominator carried cumulative weights).zip weights) :
    pair.1 * denominator < scale * pair.2 + denominator ∧
      scale * pair.2 < (pair.1 + 1) * denominator := by
  induction weights generalizing carried cumulative with
  | nil => simp [apportionFrom] at member
  | cons weight weights induction =>
      simp only [apportionFrom, List.zip_cons_cons, List.mem_cons] at member
      rcases member with rfl | member
      · -- The head claim: one boundary step, bracketed on both sides.
        have lowBefore := cumulativeFloorBoundaryV2_never_rounds_up scale cumulative
          denominator
        have highBefore := cumulativeFloorBoundaryV2_residue_lt_one_atom scale cumulative
          denominator positive
        have lowAfter := cumulativeFloorBoundaryV2_never_rounds_up scale
          (cumulative + weight) denominator
        have highAfter := cumulativeFloorBoundaryV2_residue_lt_one_atom scale
          (cumulative + weight) denominator positive
        have ordered : carried
            ≤ cumulativeFloorBoundaryV2 scale (cumulative + weight) denominator := by
          rw [← carriedExact]
          exact cumulativeFloorBoundaryV2_monotone scale cumulative (cumulative + weight)
            denominator (by omega)
        -- Expand every product `omega` would otherwise treat as an opaque atom.
        have expandBefore :
            (cumulativeFloorBoundaryV2 scale cumulative denominator + 1) * denominator
              = cumulativeFloorBoundaryV2 scale cumulative denominator * denominator
                + denominator := by
          rw [Nat.add_mul, Nat.one_mul]
        have expandAfter :
            (cumulativeFloorBoundaryV2 scale (cumulative + weight) denominator + 1)
                * denominator
              = cumulativeFloorBoundaryV2 scale (cumulative + weight) denominator
                * denominator + denominator := by
          rw [Nat.add_mul, Nat.one_mul]
        have expandStep :
            (cumulativeFloorBoundaryV2 scale (cumulative + weight) denominator - carried)
                * denominator
              = cumulativeFloorBoundaryV2 scale (cumulative + weight) denominator
                * denominator - carried * denominator := by
          rw [Nat.sub_mul]
        have expandPayout :
            (cumulativeFloorBoundaryV2 scale (cumulative + weight) denominator - carried
                + 1) * denominator
              = (cumulativeFloorBoundaryV2 scale (cumulative + weight) denominator
                - carried) * denominator + denominator := by
          rw [Nat.add_mul, Nat.one_mul]
        have carriedProduct : carried * denominator
            = cumulativeFloorBoundaryV2 scale cumulative denominator * denominator := by
          rw [carriedExact]
        have spread : scale * (cumulative + weight) = scale * cumulative + scale * weight :=
          Nat.mul_add _ _ _
        have monotoneProduct :
            carried * denominator
              ≤ cumulativeFloorBoundaryV2 scale (cumulative + weight) denominator
                * denominator :=
          Nat.mul_le_mul_right denominator ordered
        simp only
        omega
      · exact induction _ _ rfl member

/-- The one-atom bound at a single boundary step: the head case of
`apportionFrom_within_one_atom`, and the induction step of the control-polygon
bound below. -/
theorem boundary_step_within_one_atom
    (scale denominator cumulative carried weight : Nat)
    (positive : 0 < denominator)
    (carriedExact : cumulativeFloorBoundaryV2 scale cumulative denominator = carried) :
    (cumulativeFloorBoundaryV2 scale (cumulative + weight) denominator - carried)
        * denominator < scale * weight + denominator ∧
      scale * weight
        < ((cumulativeFloorBoundaryV2 scale (cumulative + weight) denominator - carried)
          + 1) * denominator := by
  have step := apportionFrom_within_one_atom scale denominator cumulative carried
    [weight] positive carriedExact
    (cumulativeFloorBoundaryV2 scale (cumulative + weight) denominator - carried,
      weight) (by simp [apportionFrom])
  simpa using step

/-- **No claim is apportioned more than one atom from its exact share.** The
integer payout `p` of a claim whose exact rational share is `Q * w / D`
satisfies `|p - Q*w/D| < 1`, stated over integers to avoid a rational. This is
the per-claim honesty bound the exact partition sum does not by itself
provide. -/
theorem apportion_within_one_atom
    (scale denominator : Nat) (weights : List Nat) (positive : 0 < denominator)
    (pair : Nat × Nat)
    (member : pair ∈ (apportion scale denominator weights).zip weights) :
    pair.1 * denominator < scale * pair.2 + denominator ∧
      scale * pair.2 < (pair.1 + 1) * denominator :=
  apportionFrom_within_one_atom scale denominator 0 0 weights positive
    (by simp [cumulativeFloorBoundaryV2]) pair member

/-- **A claim with no exact weight is never handed a collateral atom.** This is
what carries B-spline local support through the apportionment: outside the
`degree + 1` claims supported on the located span, the payout is an exact zero
rather than a rounded one. -/
theorem apportion_zero_weight
    (scale denominator : Nat) (weights : List Nat) (positive : 0 < denominator)
    (pair : Nat × Nat)
    (member : pair ∈ (apportion scale denominator weights).zip weights)
    (unweighted : pair.2 = 0) : pair.1 = 0 := by
  obtain ⟨bound, _⟩ := apportion_within_one_atom scale denominator weights positive
    pair member
  rw [unweighted, Nat.mul_zero, Nat.zero_add] at bound
  have scaled : pair.1 * denominator < 1 * denominator := by
    rw [Nat.one_mul]
    exact bound
  have below : pair.1 < 1 := Nat.lt_of_mul_lt_mul_right scaled
  omega

/-! ### The supply vector is a control polygon

`liability T w` over the *exact rational* weights `w/D` is, by definition, the
spline curve whose control points are the outstanding supplies. The theorem
below bounds how far the integer-apportioned liability can sit from `Q` times
that curve, and the answer is at most one atom per outstanding claim — which
is the aggregate form of the per-claim bound above.
-/

/-- The arithmetic of one control point, over abstract variables so that the
products stay transparent: one claim's supply lifts the per-claim one-atom
bound into a per-claim `supply`-atom bound, and the tail bounds add. -/
theorem control_polygon_step
    (scale denominator supply payout weight tailPayout tailWeight tailSum : Nat)
    (lowerHead : payout * denominator < scale * weight + denominator)
    (upperHead : scale * weight < (payout + 1) * denominator)
    (lowerTail : tailPayout * denominator ≤ scale * tailWeight + tailSum * denominator)
    (upperTail : scale * tailWeight ≤ (tailPayout + tailSum) * denominator) :
    (supply * payout + tailPayout) * denominator
        ≤ scale * (supply * weight + tailWeight) + (supply + tailSum) * denominator ∧
      scale * (supply * weight + tailWeight)
        ≤ (supply * payout + tailPayout + (supply + tailSum)) * denominator := by
  have liftLower : supply * (payout * denominator)
      ≤ supply * (scale * weight + denominator) :=
    Nat.mul_le_mul_left supply (by omega)
  have liftUpper : supply * (scale * weight) ≤ supply * ((payout + 1) * denominator) :=
    Nat.mul_le_mul_left supply (by omega)
  have leftLower : supply * (payout * denominator) = supply * payout * denominator :=
    (Nat.mul_assoc _ _ _).symm
  have rightLower : supply * (scale * weight + denominator)
      = scale * (supply * weight) + supply * denominator := by
    rw [Nat.mul_add, Nat.mul_comm supply denominator, ← Nat.mul_assoc,
      Nat.mul_comm supply scale, Nat.mul_assoc]
  have leftUpper : supply * (scale * weight) = scale * (supply * weight) := by
    rw [← Nat.mul_assoc, Nat.mul_comm supply scale, Nat.mul_assoc]
  have rightUpper : supply * ((payout + 1) * denominator)
      = supply * payout * denominator + supply * denominator := by
    rw [Nat.add_mul, Nat.one_mul, Nat.mul_add, ← Nat.mul_assoc]
  have goalLeft : (supply * payout + tailPayout) * denominator
      = supply * payout * denominator + tailPayout * denominator := Nat.add_mul _ _ _
  have goalRight : scale * (supply * weight + tailWeight)
      = scale * (supply * weight) + scale * tailWeight := Nat.mul_add _ _ _
  have goalSum : (supply + tailSum) * denominator
      = supply * denominator + tailSum * denominator := Nat.add_mul _ _ _
  have goalUpper : (supply * payout + tailPayout + (supply + tailSum)) * denominator
      = supply * payout * denominator + tailPayout * denominator
        + supply * denominator + tailSum * denominator := by
    rw [Nat.add_mul, Nat.add_mul, Nat.add_mul]
    omega
  have tailBound : (tailPayout + tailSum) * denominator
      = tailPayout * denominator + tailSum * denominator := Nat.add_mul _ _ _
  omega

theorem liability_apportionFrom_control_polygon
    (scale denominator cumulative carried : Nat) (supplies weights : List Nat)
    (positive : 0 < denominator)
    (carriedExact : cumulativeFloorBoundaryV2 scale cumulative denominator = carried)
    (sameWidth : supplies.length = weights.length) :
    liability supplies (apportionFrom scale denominator carried cumulative weights)
          * denominator
        ≤ scale * liability supplies weights + supplies.sum * denominator ∧
      scale * liability supplies weights
        ≤ (liability supplies
            (apportionFrom scale denominator carried cumulative weights)
          + supplies.sum) * denominator := by
  induction supplies generalizing weights carried cumulative with
  | nil =>
      cases weights with
      | nil => simp [liability]
      | cons _ _ => simp at sameWidth
  | cons supply supplies induction =>
      cases weights with
      | nil => simp at sameWidth
      | cons weight weights =>
          have tailWidth : supplies.length = weights.length := by simpa using sameWidth
          obtain ⟨lowerTail, upperTail⟩ := induction weights
            (carried := cumulativeFloorBoundaryV2 scale (cumulative + weight) denominator)
            (cumulative := cumulative + weight) rfl tailWidth
          obtain ⟨lowerHead, upperHead⟩ := boundary_step_within_one_atom scale denominator
            cumulative carried weight positive carriedExact
          simp only [apportionFrom, liability, List.sum_cons]
          exact control_polygon_step scale denominator supply _ weight _ _ _
            lowerHead upperHead lowerTail upperTail

/-- **The outstanding supply vector is the spline's control polygon.** Exact
terminal liability over the rational basis weights is the spline curve at the
resolved coordinate; the integer-apportioned liability is within one
collateral atom *per outstanding claim* of `Q` times that curve, stated over
integers to avoid a rational.

This is what *"properly shaped dynamics"* means operationally: an issuer picks
knots and a degree, a holder picks supplies, and the payoff is the spline
those control points describe — rather than a choice among fixed bins. -/
theorem liability_apportion_control_polygon
    (scale denominator : Nat) (supplies weights : List Nat)
    (positive : 0 < denominator)
    (sameWidth : supplies.length = weights.length) :
    liability supplies (apportion scale denominator weights) * denominator
        ≤ scale * liability supplies weights + supplies.sum * denominator ∧
      scale * liability supplies weights
        ≤ (liability supplies (apportion scale denominator weights)
          + supplies.sum) * denominator :=
  liability_apportionFrom_control_polygon scale denominator 0 0 supplies weights
    positive (by simp [cumulativeFloorBoundaryV2]) sameWidth

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

/-! ## Local support

A degree-`d` basis function is supported on `d+1` knot spans, so at any one
coordinate exactly `d+1` of the `K` claims can pay anything at all.  The de
Boor triangle computes those `d+1` weights; every other claim's weight is a
real zero rather than a rounded one, and `scatter` places the triangle at the
located span.
-/

/-- Place the locally supported weights inside the runtime-width vector. -/
def scatter (width offset : Nat) (values : List Nat) : List Nat :=
  List.replicate offset 0 ++ values ++
    List.replicate (width - (offset + values.length)) 0

theorem sum_replicate_zero (count : Nat) : (List.replicate count 0).sum = 0 := by
  induction count with
  | zero => rfl
  | succ count induction => simp [List.replicate_succ, induction]

/-- Scattering moves no weight: the claims outside the local support carry
exact zeros. -/
@[simp] theorem scatter_sum (width offset : Nat) (values : List Nat) :
    (scatter width offset values).sum = values.sum := by
  simp only [scatter, List.sum_append, sum_replicate_zero]
  omega

theorem scatter_length
    (width offset : Nat) (values : List Nat)
    (inside : offset + values.length ≤ width) :
    (scatter width offset values).length = width := by
  simp only [scatter, List.length_append, List.length_replicate]
  omega

/-! ## The B-spline profile

Knots are exact signed numerators over one positive common denominator, the
same representation the ramp profile uses.  Repeated knots are admitted: an
interior knot of multiplicity `r` drops the spline's continuity there by `r`,
which is how a corner or a jump is expressed inside a smooth basis at all.
Gen-1 forbade interior multiplicity outright and had to record the consequence
as a limitation — a tent was exact at degree one and inexact at every smooth
degree.  Here degenerate spans are skipped by the span locator instead of
being refused by the profile.
-/

abbrev RationalCoordinate := DClutch.ProductV2.RationalCoordinate

/-- A degree-`1..3` B-spline liability basis over an exact rational knot
vector. `knots` is the full knot vector *with* multiplicity, so `width` claims
need `width + degree + 1` knots. -/
structure SplineProfile where
  degree : Nat
  scale : Nat
  knotDenominator : Nat
  knots : List Int
  degreePositive : 0 < degree
  degreeBounded : degree ≤ 3
  scalePositive : 0 < scale
  knotDenominatorPositive : 0 < knotDenominator
  enoughKnots : 2 * degree + 2 ≤ knots.length

/-- One claim per basis function: `K = |knots| - degree - 1`. -/
def SplineProfile.width (profile : SplineProfile) : Nat :=
  profile.knots.length - profile.degree - 1

theorem SplineProfile.degree_lt_width (profile : SplineProfile) :
    profile.degree < profile.width := by
  have enough := profile.enoughKnots
  unfold SplineProfile.width
  omega

theorem SplineProfile.width_positive (profile : SplineProfile) :
    0 < profile.width :=
  Nat.lt_of_le_of_lt (Nat.zero_le _) profile.degree_lt_width

/-- Total knot read; an out-of-range index reads zero and is ruled out by the
domain checks. -/
def knotAt (knots : List Int) (index : Nat) : Int := knots[index]?.getD 0

/-- Scaled knot, over the common denominator of the knots and the coordinate —
definitionally the ramp lane's `scaledKnot`. -/
def SplineProfile.scaledKnot
    (profile : SplineProfile) (coordinate : RationalCoordinate) (index : Nat) : Int :=
  knotAt profile.knots index * coordinate.denominator

/-- Scaled coordinate — definitionally the ramp lane's `scaledCoordinate`. -/
def SplineProfile.scaledCoordinate
    (profile : SplineProfile) (coordinate : RationalCoordinate) : Int :=
  coordinate.numerator * profile.knotDenominator

/-- **Boundary clamp.** Coordinates below the first knot of the domain and
above the last are pulled onto the domain, so the outermost claims pay their
full weight at and beyond the edge rather than falling off a half-open span.
This is the spline form of the ramp's two explicit tail clamps. -/
def SplineProfile.clampedCoordinate
    (profile : SplineProfile) (coordinate : RationalCoordinate) : Int :=
  max (profile.scaledKnot coordinate profile.degree)
    (min (profile.scaledCoordinate coordinate)
      (profile.scaledKnot coordinate profile.width))

/-- Spans inside the domain that are not degenerate. A knot of multiplicity
`r` collapses `r-1` spans; those are skipped here rather than refused, which
is what admits multiplicity at all. -/
def SplineProfile.spanCandidates
    (profile : SplineProfile) (coordinate : RationalCoordinate) : List Nat :=
  ((List.range (profile.width - profile.degree)).map (fun offset =>
      profile.degree + offset)).filter (fun span =>
    decide (profile.scaledKnot coordinate span
      < profile.scaledKnot coordinate (span + 1)))

/-- The non-degenerate span carrying the clamped coordinate: the last candidate
whose left knot the coordinate has reached. The top of the domain is closed,
so the final coordinate lands in the final span rather than nowhere. -/
def SplineProfile.locateSpan
    (profile : SplineProfile) (coordinate : RationalCoordinate) : Nat :=
  let candidates := profile.spanCandidates coordinate
  match (candidates.filter (fun span =>
      decide (profile.scaledKnot coordinate span
        ≤ profile.clampedCoordinate coordinate))).getLast? with
  | some span => span
  | none => candidates.headD profile.degree

/-- The `k` Cox-de-Boor weights of one level, at knot indices `span+1-k` up to
`span`. The numerator is clamped to the denominator so that no weight can
exceed one; on a correctly located span the clamp is inert, exactly as the
ramp's interior clamp is. -/
def SplineProfile.levelWeights
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (span level : Nat) : List (Nat × Nat) :=
  (List.range level).map (fun offset =>
    let index := span + 1 + offset - level
    let elapsed :=
      (profile.clampedCoordinate coordinate
        - profile.scaledKnot coordinate index).toNat
    let support :=
      (profile.scaledKnot coordinate (index + level)
        - profile.scaledKnot coordinate index).toNat
    (Nat.min elapsed support, support))

@[simp] theorem SplineProfile.levelWeights_length
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (span level : Nat) :
    (profile.levelWeights coordinate span level).length = level := by
  simp [SplineProfile.levelWeights]

theorem SplineProfile.levelWeights_bounded
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (span level : Nat) (weight : Nat × Nat)
    (member : weight ∈ profile.levelWeights coordinate span level) :
    weight.1 ≤ weight.2 := by
  simp only [SplineProfile.levelWeights, List.mem_map] at member
  obtain ⟨_, _, rfl⟩ := member
  exact Nat.min_le_right _ _

/-- The degree levels of the triangle, from level one up to the degree. -/
def SplineProfile.weightLevelsFrom
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (span : Nat) : Nat → Nat → List (List (Nat × Nat))
  | 0, _ => []
  | remaining + 1, level =>
      profile.levelWeights coordinate span level ::
        profile.weightLevelsFrom coordinate span remaining (level + 1)

def SplineProfile.weightLevels
    (profile : SplineProfile) (coordinate : RationalCoordinate) :
    List (List (Nat × Nat)) :=
  profile.weightLevelsFrom coordinate (profile.locateSpan coordinate)
    profile.degree 1

/-- **Evaluator admission.** Three decidable facts: the coordinate is a real
rational, the located span sits inside the domain, and every de Boor
denominator is a non-degenerate knot span. Nothing else is needed — weight
nonnegativity and the `p ≤ q` bound are structural. -/
def SplineProfile.admits
    (profile : SplineProfile) (coordinate : RationalCoordinate) : Bool :=
  decide (0 < coordinate.denominator) &&
    decide (profile.degree ≤ profile.locateSpan coordinate) &&
    decide (profile.locateSpan coordinate < profile.width) &&
    (profile.weightLevels coordinate).all (fun level =>
      level.all (fun weight => decide (0 < weight.2)))

theorem SplineProfile.weightLevelsFrom_wellFormed
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (span remaining level : Nat)
    (positive : ∀ entry ∈ profile.weightLevelsFrom coordinate span remaining level,
      ∀ weight ∈ entry, 0 < weight.2) :
    LevelsWellFormed (profile.weightLevelsFrom coordinate span remaining level) level := by
  induction remaining generalizing level with
  | zero => trivial
  | succ remaining induction =>
      refine ⟨profile.levelWeights_length coordinate span level, ?_, ?_⟩
      · intro weight member
        exact ⟨profile.levelWeights_bounded coordinate span level weight member,
          positive _ (by simp [SplineProfile.weightLevelsFrom]) weight member⟩
      · exact induction (level + 1) (fun entry member =>
          positive entry (by simp [SplineProfile.weightLevelsFrom, member]))

/-- An admitted coordinate gives a well-formed triangle starting from the
single degree-zero value. -/
theorem SplineProfile.wellFormed_of_admits
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (admitted : profile.admits coordinate = true) :
    LevelsWellFormed (profile.weightLevels coordinate) 1 := by
  unfold SplineProfile.admits at admitted
  simp only [Bool.and_eq_true, decide_eq_true_eq, List.all_eq_true] at admitted
  refine profile.weightLevelsFrom_wellFormed coordinate _ _ _ ?_
  intro entry member weight weightMember
  exact admitted.2 entry member weight weightMember

theorem SplineProfile.admits_coordinateDenominator
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (admitted : profile.admits coordinate = true) : 0 < coordinate.denominator := by
  unfold SplineProfile.admits at admitted
  simp only [Bool.and_eq_true, decide_eq_true_eq] at admitted
  exact admitted.1.1.1

theorem SplineProfile.admits_span_inside
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (admitted : profile.admits coordinate = true) :
    profile.degree ≤ profile.locateSpan coordinate ∧
      profile.locateSpan coordinate < profile.width := by
  unfold SplineProfile.admits at admitted
  simp only [Bool.and_eq_true, decide_eq_true_eq] at admitted
  exact ⟨admitted.1.1.2, admitted.1.2⟩

/-! ### The exact rational basis, then the exact integer payouts -/

/-- Local de Boor values, one per basis function supported on the located
span. Their common denominator is `basisDenominator`. -/
def SplineProfile.localNumerators
    (profile : SplineProfile) (coordinate : RationalCoordinate) : List Nat :=
  deBoorLevels (profile.weightLevels coordinate) [1]

/-- The exact common denominator of one evaluation. -/
def SplineProfile.basisDenominator
    (profile : SplineProfile) (coordinate : RationalCoordinate) : Nat :=
  levelsProduct (profile.weightLevels coordinate)

/-- The exact rational basis weights, as numerators over `basisDenominator`,
placed at their claim coordinates. -/
def SplineProfile.basisNumerators
    (profile : SplineProfile) (coordinate : RationalCoordinate) : List Nat :=
  scatter profile.width (profile.locateSpan coordinate - profile.degree)
    (profile.localNumerators coordinate)

/-- **The evaluator.** Exact rational B-spline weights, apportioned into
integer collateral atoms by the single cumulative floor boundary. -/
def SplineProfile.evaluate
    (profile : SplineProfile) (coordinate : RationalCoordinate) : List Nat :=
  apportion profile.scale (profile.basisDenominator coordinate)
    (profile.basisNumerators coordinate)

theorem SplineProfile.weightLevelsFrom_length
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (span remaining level : Nat) :
    (profile.weightLevelsFrom coordinate span remaining level).length = remaining := by
  induction remaining generalizing level with
  | zero => rfl
  | succ remaining induction =>
      simp only [SplineProfile.weightLevelsFrom, List.length_cons]
      rw [induction (level + 1)]

@[simp] theorem SplineProfile.weightLevels_length
    (profile : SplineProfile) (coordinate : RationalCoordinate) :
    (profile.weightLevels coordinate).length = profile.degree :=
  profile.weightLevelsFrom_length coordinate _ _ _

/-- Exactly `degree + 1` claims are locally supported, which is the B-spline
locality property in its integer form. -/
theorem SplineProfile.localNumerators_length
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (admitted : profile.admits coordinate = true) :
    (profile.localNumerators coordinate).length = profile.degree + 1 := by
  have wellFormed := profile.wellFormed_of_admits coordinate admitted
  have stepped := deBoorLevels_length (profile.weightLevels coordinate) [1] 1
    (by simp) wellFormed
  simp only [SplineProfile.localNumerators]
  rw [stepped, profile.weightLevels_length coordinate]
  omega

theorem SplineProfile.basisDenominator_positive
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (admitted : profile.admits coordinate = true) :
    0 < profile.basisDenominator coordinate :=
  levelsProduct_positive _ 1 (profile.wellFormed_of_admits coordinate admitted)

/-- **Partition of unity, in exact rational form.** The B-spline weights at an
admitted coordinate sum to exactly one — as integer numerators, to exactly
their own common denominator. No approximation and no rounding has happened
yet at this point. -/
theorem SplineProfile.basisNumerators_sum
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (admitted : profile.admits coordinate = true) :
    (profile.basisNumerators coordinate).sum = profile.basisDenominator coordinate := by
  have wellFormed := profile.wellFormed_of_admits coordinate admitted
  simp only [SplineProfile.basisNumerators, scatter_sum, SplineProfile.localNumerators,
    SplineProfile.basisDenominator]
  rw [deBoorLevels_sum (profile.weightLevels coordinate) [1] 1 (by simp) wellFormed]
  simp

theorem SplineProfile.basisNumerators_length
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (admitted : profile.admits coordinate = true) :
    (profile.basisNumerators coordinate).length = profile.width := by
  obtain ⟨spanLow, spanHigh⟩ := profile.admits_span_inside coordinate admitted
  refine scatter_length _ _ _ ?_
  rw [profile.localNumerators_length coordinate admitted]
  omega

theorem SplineProfile.evaluate_length
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (admitted : profile.admits coordinate = true) :
    (profile.evaluate coordinate).length = profile.width := by
  simp only [SplineProfile.evaluate, apportion_length]
  exact profile.basisNumerators_length coordinate admitted

/-- **Exact partition sum.** The integer payouts of a degree-`1..3` B-spline
basis sum to exactly the collateral scale `Q` at every admitted coordinate.
This is the theorem `M-4` names: the B-spline partition-of-unity property
holding over the integer-apportioned form, at every runtime width, with one
named rounding boundary and no residue. -/
theorem SplineProfile.evaluate_partition
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (admitted : profile.admits coordinate = true) :
    (profile.evaluate coordinate).sum = profile.scale :=
  apportion_sum profile.scale (profile.basisDenominator coordinate) _
    (profile.basisDenominator_positive coordinate admitted)
    (profile.basisNumerators_sum coordinate admitted)

theorem SplineProfile.evaluate_bounded
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (admitted : profile.admits coordinate = true)
    (payout : Nat) (member : payout ∈ profile.evaluate coordinate) :
    payout ≤ profile.scale :=
  apportion_le_scale profile.scale (profile.basisDenominator coordinate) _
    (profile.basisDenominator_positive coordinate admitted)
    (profile.basisNumerators_sum coordinate admitted) payout member

/-! ### The B-spline family as a liability basis

Instantiating `Basis` is what carries the whole supply algebra across:
complete-set split, complete-set merge, claim transfer and terminal
redemption, plus the `Q * peak(T)` solvency envelope, are all inherited from
`LiabilityBasisV2` rather than re-proved here.
-/

/-- Admitted terminal results of one profile. -/
def SplineProfile.Admitted (profile : SplineProfile) : Type :=
  { coordinate : RationalCoordinate // profile.admits coordinate = true }

/-- **The degree-`1..3` B-spline liability basis.** -/
def SplineProfile.basis (profile : SplineProfile) : Basis profile.Admitted := {
  width := profile.width
  scale := profile.scale
  widthPositive := profile.width_positive
  scalePositive := profile.scalePositive
  evaluate := fun result => profile.evaluate result.val
  exactWidth := fun result => profile.evaluate_length result.val result.property
  payoutBounded := fun result => profile.evaluate_bounded result.val result.property
  partitionUnity := fun result => profile.evaluate_partition result.val result.property
}

@[simp] theorem SplineProfile.basis_width (profile : SplineProfile) :
    profile.basis.width = profile.width := rfl

@[simp] theorem SplineProfile.basis_scale (profile : SplineProfile) :
    profile.basis.scale = profile.scale := rfl

/-- The hostile partition checker never refuses an honest spline evaluation. -/
theorem SplineProfile.validPartition_evaluate
    (profile : SplineProfile) (result : profile.Admitted) :
    validPartition (profile.evaluate result.val) profile.scale = true :=
  profile.basis.validPartition_evaluate result

/-- **Per-claim honesty.** Every claim's integer payout is within one collateral
atom of `Q` times its exact rational B-spline weight. -/
theorem SplineProfile.evaluate_within_one_atom
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (admitted : profile.admits coordinate = true) (pair : Nat × Nat)
    (member : pair ∈
      (profile.evaluate coordinate).zip (profile.basisNumerators coordinate)) :
    pair.1 * profile.basisDenominator coordinate
        < profile.scale * pair.2 + profile.basisDenominator coordinate ∧
      profile.scale * pair.2
        < (pair.1 + 1) * profile.basisDenominator coordinate :=
  apportion_within_one_atom profile.scale (profile.basisDenominator coordinate) _
    (profile.basisDenominator_positive coordinate admitted) pair member

/-- **The outstanding supply vector is this spline's control polygon.**
`liability supplies (basisNumerators x)` over the exact rational weights *is*
the spline curve at the resolved coordinate, with the supplies as control
points. The integer payouts move terminal liability by at most one collateral
atom per outstanding claim away from `Q` times that curve.

This is what the ledger's *"properly shaped dynamics"* means operationally: an
issuer picks knots and a degree, a holder picks supplies, and the payoff is
the spline those control points describe — instead of a choice among fixed
bins. -/
theorem SplineProfile.liability_is_the_control_polygon_curve
    (profile : SplineProfile) (coordinate : RationalCoordinate) (supplies : List Nat)
    (admitted : profile.admits coordinate = true)
    (sameWidth : supplies.length = profile.width) :
    liability supplies (profile.evaluate coordinate)
          * profile.basisDenominator coordinate
        ≤ profile.scale * liability supplies (profile.basisNumerators coordinate)
          + supplies.sum * profile.basisDenominator coordinate ∧
      profile.scale * liability supplies (profile.basisNumerators coordinate)
        ≤ (liability supplies (profile.evaluate coordinate) + supplies.sum)
          * profile.basisDenominator coordinate := by
  refine liability_apportion_control_polygon profile.scale
    (profile.basisDenominator coordinate) supplies _
    (profile.basisDenominator_positive coordinate admitted) ?_
  rw [profile.basisNumerators_length coordinate admitted]
  exact sameWidth

/-- **Local support survives apportionment.** A claim whose exact B-spline
weight is zero — every claim outside the `degree + 1` supported on the located
span — receives an exact zero payout, never a rounded atom. -/
theorem SplineProfile.evaluate_zero_outside_support
    (profile : SplineProfile) (coordinate : RationalCoordinate)
    (admitted : profile.admits coordinate = true) (pair : Nat × Nat)
    (member : pair ∈
      (profile.evaluate coordinate).zip (profile.basisNumerators coordinate))
    (unweighted : pair.2 = 0) : pair.1 = 0 :=
  apportion_zero_weight profile.scale (profile.basisDenominator coordinate) _
    (profile.basisDenominator_positive coordinate admitted) pair member unweighted

end DClutch.LiabilityBasisV2.Spline
