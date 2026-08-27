import DClutchSemantics.LiabilityBasisV2Spline

/-!
# Decided witnesses for the B-spline liability basis

The theorems in `LiabilityBasisV2Spline` are universally quantified, and a
universally quantified theorem about a partition sum can be true of an
evaluator that computes the wrong curve.  Everything below is a concrete
`decide`-checked fact pinning the evaluator to values derived independently of
it, so a refactor that keeps every theorem green and changes the mathematics
fails here.

No `native_decide`: each fact is closed by the kernel.

Three of these values were computed by the generation-one stack, from a
different algorithm in a different language, and are reproduced exactly:

* the clamped cubic is the cubic Bernstein basis;
* the interior degree-two basis function peaks at `3/4`;
* the interior degree-three basis function peaks at `2/3`.

The last two are the entries of generation one's *exact ceiling table*, and
they are the reason a degree-`≥ 2` claim basis needs a price-plane admission
gate that this tree does not yet have — see
`docs/research/BSPLINE_ECLIPSE_SCORECARD_2026_08_27.md`.
-/

namespace DClutch.LiabilityBasisV2.Spline.Examples

open DClutch.LiabilityBasisV2
open DClutch.LiabilityBasisV2.Spline

/-- A concrete profile, with the side conditions discharged by computation. -/
def profile
    (degree scale knotDenominator : Nat) (knots : List Int)
    (degreePositive : 0 < degree := by decide)
    (degreeBounded : degree ≤ 3 := by decide)
    (scalePositive : 0 < scale := by decide)
    (knotDenominatorPositive : 0 < knotDenominator := by decide)
    (enoughKnots : 2 * degree + 2 ≤ knots.length := by decide) : SplineProfile :=
  { degree, scale, knotDenominator, knots, degreePositive, degreeBounded,
    scalePositive, knotDenominatorPositive, enoughKnots }

def at' (numerator : Int) (denominator : Nat) : RationalCoordinate :=
  { numerator, denominator }

/-! ## Degree one: the hat basis -/

/-- Two hats over four knots. Claim `i` peaks at knot `t_{i+1}`, so a portfolio
holding `g(t_i)` of claim `i` pays the piecewise-linear interpolant of `g`. -/
def hats : SplineProfile := profile 1 100 1 [0, 1, 2, 3]

example : hats.width = 2 := by decide
example : hats.admits (at' 3 2) = true := by decide

/-- Exactly half each at the midpoint of the domain. -/
example : hats.evaluate (at' 3 2) = [50, 50] := by decide

/-- Each hat attains the whole scale at its own knot. -/
example : hats.evaluate (at' 1 1) = [100, 0] := by decide
example : hats.evaluate (at' 2 1) = [0, 100] := by decide

/-- **Boundary clamp.** Coordinates far below and far above the domain pay the
outermost claim in full rather than falling off a half-open span. -/
example : hats.evaluate (at' (-1000000) 1) = [100, 0] := by decide
example : hats.evaluate (at' 1000000 1) = [0, 100] := by decide

/-! ## Degree three: the clamped cubic is the Bernstein basis -/

/-- Endpoints at multiplicity four, one span: the basis is exactly the cubic
Bernstein polynomials. -/
def bezier : SplineProfile := profile 3 1000 1 [0, 0, 0, 0, 1, 1, 1, 1]

example : bezier.width = 4 := by decide

/-- **The exact rational weights are cubic Bernstein at `t = 1/2`:**
`[1/8, 3/8, 3/8, 1/8]`, as integer numerators over their own denominator, with
no rounding anywhere before the apportionment. -/
example : bezier.basisNumerators (at' 1 2) = [8, 24, 24, 8] := by decide
example : bezier.basisDenominator (at' 1 2) = 64 := by decide

/-- At `t = 1/4` the same basis, over a denominator the triangle happened to
accumulate: `[1728, 1728, 576, 64] / 4096` is `[27/64, 27/64, 9/64, 1/64]`.
**No greatest common divisor is ever computed** — the weights are carried
unreduced and only the apportionment ever divides, which is what keeps the
whole evaluation to multiplication, subtraction and one floor per claim. -/
example : bezier.basisNumerators (at' 1 4) = [1728, 1728, 576, 64] := by decide
example : bezier.basisDenominator (at' 1 4) = 4096 := by decide
example : 1728 * 64 = 27 * 4096 := by decide

/-- The apportionment at the midpoint is exact — every share is a whole number
of atoms — so no rounding is visible. -/
example : bezier.evaluate (at' 1 2) = [125, 375, 375, 125] := by decide

/-- At a quarter it is not, and the cumulative floor is **not** reflection
symmetric: these two are not mirror images of each other. Every claim is still
within one atom of its exact share, and both still sum to exactly `Q`. -/
example : bezier.evaluate (at' 1 4) = [421, 422, 141, 16] := by decide
example : bezier.evaluate (at' 3 4) = [15, 141, 422, 422] := by decide
example : (bezier.evaluate (at' 1 4)).sum = 1000 := by decide
example : (bezier.evaluate (at' 3 4)).sum = 1000 := by decide

/-- A clamped end pays its own claim the whole complete set. -/
example : bezier.evaluate (at' 0 1) = [1000, 0, 0, 0] := by decide
example : bezier.evaluate (at' 1 1) = [0, 0, 0, 1000] := by decide

/-! ## Uniform clamped cubic -/

def uniformCubic : SplineProfile :=
  profile 3 1200 1 [0, 0, 0, 0, 1, 2, 3, 4, 5, 5, 5, 5]

example : uniformCubic.width = 8 := by decide

/-- **The uniform cubic B-spline at a span midpoint is `[1/48, 23/48, 23/48,
1/48]`**, the classical value, reproduced as exact integers. -/
example : uniformCubic.basisNumerators (at' 5 2) = [0, 0, 144, 3312, 3312, 144, 0, 0] := by
  decide
example : uniformCubic.basisDenominator (at' 5 2) = 6912 := by decide

/-- At an interior knot it is `[1/6, 2/3, 1/6]`. -/
example : uniformCubic.evaluate (at' 2 1) = [0, 0, 200, 800, 200, 0, 0, 0] := by decide

/-- **Local support.** Exactly `degree + 1` of the eight claims can pay
anything; the rest are exact zeros, not rounded ones. -/
example :
    ((uniformCubic.evaluate (at' 5 2)).filter (fun payout => payout != 0)).length = 4 := by
  decide

/-! ## Interior knot multiplicity

Generation one forbade a repeated interior knot outright, and recorded the
consequence: a tent or a corner was exact at degree one and inexact at every
smooth degree. Here a repeated knot simply collapses a span, the locator skips
it, and the partition stays exact.
-/

def doubleKnot : SplineProfile :=
  profile 3 1000 1 [0, 0, 0, 0, 2, 2, 4, 4, 4, 4]

example : doubleKnot.width = 6 := by decide

/-- At the double knot itself, continuity has dropped to `C^1` and two claims
split the complete set exactly. -/
example : doubleKnot.evaluate (at' 2 1) = [0, 0, 500, 500, 0, 0] := by decide

/-- On either side of it the partition is still exact. -/
example : (doubleKnot.evaluate (at' 1 1)).sum = 1000 := by decide
example : (doubleKnot.evaluate (at' 3 1)).sum = 1000 := by decide
example : doubleKnot.admits (at' 2 1) = true := by decide

/-! ## The degree-two and degree-three interior peaks

These are the two entries of generation one's exact ceiling table that matter,
recomputed here by an unrelated algorithm.

They are also the whole reason a degree-`≥ 2` claim basis is not safe to sell
without a price-plane gate. At degree `≥ 2` an interior claim can never pay the
whole complete set, so `p ≥ 0, sum p = Q` stops being the no-arbitrage
condition on a price vector: the portfolio *three complete sets, short four of
the interior claim* has a globally nonnegative payoff, and at the price
`p = Q * e_interior` it costs `-Q`.

`LiabilityBasisV2Spline` is the claim plane. Nothing here or anywhere else in
this tree gates the price plane, and nothing may select a degree-`≥ 2` basis
until something does.
-/

/-- Degree two, five claims, uniform interior knots. -/
def quadratic : SplineProfile := profile 2 1200 1 [0, 0, 0, 2, 4, 6, 6, 6]

example : quadratic.width = 5 := by decide

/-- **The interior degree-two claim peaks at exactly `3/4`** — `900` of a
`1200` complete set — which is generation one's exact ceiling table entry. -/
example : quadratic.evaluate (at' 3 1) = [0, 150, 900, 150, 0] := by decide

/-- **The executable arbitrage at that peak.** Three complete sets pay `3*Q`;
four units of the interior claim pay at most `4 * (3/4) * Q = 3*Q`. So the
payoff of *three sets short four interior claims* is exactly zero here and
strictly positive elsewhere, while a simplex-admissible price `Q * e_2` values
it at `3*Q - 4*Q = -Q`. -/
example : 4 * (quadratic.evaluate (at' 3 1))[2]! = 3 * quadratic.scale := by decide

/-- Away from the peak the same portfolio pays strictly more than nothing,
which is what makes it an arbitrage rather than a zero-cost trade. -/
example : 4 * (quadratic.evaluate (at' 2 1))[2]! < 3 * quadratic.scale := by decide
example : 4 * (quadratic.evaluate (at' 4 1))[2]! < 3 * quadratic.scale := by decide

/-- **The interior degree-three claim peaks at exactly `2/3`** — `800` of a
`1200` complete set — generation one's other exact ceiling table entry. The
same argument applies with a tighter multiplier. -/
example : uniformCubic.evaluate (at' 3 1) = [0, 0, 0, 200, 800, 200, 0, 0] := by decide

/-- **Degree one is different, and that is why a degree-one wave needs no
price-plane work.** A hat attains the whole complete set at its own knot, so
the same portfolio pays exactly zero there and the arbitrage does not exist. -/
example : hats.evaluate (at' 1 1) = [100, 0] := by decide

end DClutch.LiabilityBasisV2.Spline.Examples
