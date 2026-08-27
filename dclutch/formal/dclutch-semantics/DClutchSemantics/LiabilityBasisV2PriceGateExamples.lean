import DClutchSemantics.LiabilityBasisV2PriceGate
import DClutchSemantics.LiabilityBasisV2SplineExamples

/-!
# Decided witnesses for the degree-`≥ 2` arbitrage gate

`LiabilityBasisV2PriceGate` is universally quantified, and a universally
quantified soundness theorem is true of a gate that refuses everything.  This
file pins the gate against generation one's and generation two's own numbers.

The centrepiece is **generation two's adversarial pair**, which refutes
generation one's moment cone in both directions.  Both halves are on
`dragons-clutch` `main` in `crates/clutch-price-measure/tests/adversarial.rs`:

* line `262`, `continuous_checker_refuses_the_named_v1b_false_acceptance` —
  generation one's gate **accepts** `(4,8,0,0,0)/12`, and the portfolio
  `(1,-2,10,40,64)` costs exactly `-S` there.  Generation one was **unsound**.
* line `281`, `quantized_live_point_that_v1b_refuses_has_an_exact_runtime_certificate`
  — a live quantized point generation one's gate **refuses** has an exact
  single-atom certificate.  Generation one also **over-refused**.

Both are reproduced below against *this* tree's evaluator, which shares no
line with either predecessor.  One reproduction is worth stating separately:
generation two recorded that its degree-two basis at coordinate `85` pays
`[1128, 6667, 2205, 0, 0]` out of `10000`.  This tree's from-scratch integer
Cox-de-Boor and cumulative-floor apportionment return the same vector, though
generation one and two rounded by largest remainder and this tree floors a
running sum.  Two independent implementations, two different rounding rules,
one answer.

No `native_decide`: every fact below is closed by the kernel.
-/

namespace DClutch.LiabilityBasisV2.PriceGate.Examples

open DClutch.LiabilityBasisV2
open DClutch.LiabilityBasisV2.Spline
open DClutch.LiabilityBasisV2.Spline.Examples
open DClutch.LiabilityBasisV2.PriceGate

/-! ## Generation one's gate, transcribed

`v1bDegreeTwoAccepts` is generation one's degree-two moment-cone gate `V1b`,
transcribed from `dragons-clutch
crates/clutch-price-measure/tests/adversarial.rs:815`, which is generation
two's own transcription of generation one's `lean/DragonsClutch/MomentCone.lean`
ceiling table.  It is reproduced here **only** so that the two refutation
directions can be decided rather than asserted.  Nothing in this tree calls
it, and nothing should.
-/

/-- The exact interior ceiling `V1b` applies at one claim: `1/2` for the
narrowest basis, `2/3` at the two claims next to the ends, `3/4` in the deep
interior.  These are generation one's `decide`-checked table entries. -/
def v1bCeiling (width claim : Nat) : Nat × Nat :=
  if width = 3 then (1, 2)
  else if claim = 1 ∨ claim + 2 = width then (2, 3)
  else (3, 4)

/-- The neighbour multiplier `V1b` applies at the same claim. -/
def v1bWeight (width claim : Nat) : Nat :=
  if width = 3 then 1
  else if claim = 1 ∨ claim + 2 = width then 2
  else 3

/-- **Generation one's degree-two gate.** -/
def v1bDegreeTwoAccepts (prices : List Nat) (scale : Nat) : Bool :=
  let width := prices.length
  decide (3 ≤ width) && decide (prices.sum = scale) &&
    ((List.range width).all (fun claim =>
      if 1 ≤ claim ∧ claim + 1 < width then
        decide (entryAt prices claim * (v1bCeiling width claim).2
            ≤ scale * (v1bCeiling width claim).1) &&
          decide (entryAt prices claim
            ≤ v1bWeight width claim
              * (entryAt prices (claim - 1) + entryAt prices (claim + 1)))
      else true)) &&
    (if width = 3 then
        decide (entryAt prices 1 * entryAt prices 1
          ≤ 4 * (entryAt prices 0 * entryAt prices 2))
      else true)

/-! ## Direction one: generation one accepted an executable arbitrage

Degree two, five claims, breakpoints `[0,1,2,3]`.  In this tree's knot vector
that is `[0,0,0,1,2,3,3,3]` — the same basis, written with multiplicity at the
clamped ends instead of as a breakpoint list.

The pinned price is `p/S = (1/3, 2/3, 0, 0, 0)`, which generation one's own
`docs/research/DUAL_IS_THE_MEASURE.md` §7.6.7 recorded as a **false
acceptance** of its gate.
-/

/-- Generation one's pinned counterexample basis, at its own scale `S = 12`. -/
def gen1Basis : SplineProfile := profile 2 12 1 [0, 0, 0, 1, 2, 3, 3, 3]

example : gen1Basis.width = 5 := by decide

/-- The price generation one's gate wrongly admitted. -/
def gen1Price : List Nat := [4, 8, 0, 0, 0]

/-- **Generation one accepts it.**  This is the unsoundness, decided rather
than recalled. -/
example : v1bDegreeTwoAccepts gen1Price 12 = true := by decide

/-- It is simplex-admissible, so nothing weaker than a hull gate could have
refused it. -/
example : validPartition gen1Price 12 = true := by decide

/-- **The arbitrage portfolio.**  These are exactly the B-spline coefficients
of `(3x-1)^2` over `[0,0,0,1,2,3,3,3]`: by the blossom identity the coefficient
at claim `i` is `(3*t_{i+1} - 1)*(3*t_{i+2} - 1)`, giving
`(1, -2, 10, 40, 64)`.  Generation two priced the same vector at `-12` in
`adversarial.rs:270-278`. -/
def gen1Arbitrage : List Int := [1, -2, 10, 40, 64]

/-- **It costs exactly `-S`.** -/
example : portfolioValue gen1Arbitrage gen1Price = -12 := by decide

/-- **The exact rational identity behind it**, at four coordinates: the
portfolio's value against the *unrounded* B-spline weights is
`(3n - d)^2 / d^2` of a complete set, so it is a square and cannot be
negative.  `basisNumerators` is the exact rational basis before any
apportionment; `basisDenominator` is its common denominator. -/
example :
    portfolioValue gen1Arbitrage (gen1Basis.basisNumerators (at' 1 3)) * 9
      = 0 * gen1Basis.basisDenominator (at' 1 3) := by decide

example :
    portfolioValue gen1Arbitrage (gen1Basis.basisNumerators (at' 1 1)) * 1
      = 4 * gen1Basis.basisDenominator (at' 1 1) := by decide

example :
    portfolioValue gen1Arbitrage (gen1Basis.basisNumerators (at' 5 2)) * 4
      = 169 * gen1Basis.basisDenominator (at' 5 2) := by decide

example :
    portfolioValue gen1Arbitrage (gen1Basis.basisNumerators (at' 7 3)) * 9
      = 324 * gen1Basis.basisDenominator (at' 7 3) := by decide

/-! ### The refutation, and exactly how far it reaches

`Certificate.nonneg_price_of_nonneg_on_support` turns the arbitrage into a
refusal: a certificate whose atoms all pay `gen1Arbitrage` nonnegatively
cannot price it at `-12`.  The sweep below decides that hypothesis on a named
grid — every coordinate `n/d` in the domain `[0,3]` for
`d ∈ {1,2,3,4,6,12}`.

**What this does and does not establish.**  It establishes that no valid
certificate for `gen1Price` is supported on that grid.  It does *not*
establish that no certificate exists at any real coordinate: that needs
`0 ≤ portfolioValue gen1Arbitrage (evaluate x)` for **every** admitted `x`,
which is a statement about an infinite rational domain.  Generation one
asserted the continuous form of it analytically and never machine-checked it;
generation two checked one supplied moment witness against its per-span cone,
which refuses that witness rather than every witness.  Neither predecessor
closed it either, and the scorecard names it as the residual.
-/

/-- The denominators the sweep covers. -/
def gridDenominators : List Nat := [1, 2, 3, 4, 6, 12]

/-- Every coordinate `n/d` in `[0, 3]` for each swept denominator: 90 of
them. -/
def grid : List RationalCoordinate :=
  gridDenominators.flatMap (fun denominator =>
    (List.range (3 * denominator + 1)).map (fun numerator =>
      at' (Int.ofNat numerator) denominator))

example : grid.length = 90 := by decide

/-- Every swept coordinate is admitted, so the sweep below is about real
payout vectors and not about refusals. -/
example : grid.all (fun coordinate => gen1Basis.admits coordinate) = true := by decide

/-- **The sweep.**  The arbitrage portfolio pays nonnegatively at every swept
coordinate, after apportionment — the integer rounding never pushes the square
below zero on this grid. -/
example :
    grid.all (fun coordinate =>
      decide (0 ≤ portfolioValue gen1Arbitrage (gen1Basis.evaluate coordinate))) = true := by
  decide

theorem gen1_arbitrage_nonneg_on_grid
    (coordinate : RationalCoordinate) (swept : coordinate ∈ grid) :
    0 ≤ portfolioValue gen1Arbitrage (gen1Basis.evaluate coordinate) := by
  have sweep : grid.all (fun coordinate =>
      decide (0 ≤ portfolioValue gen1Arbitrage (gen1Basis.evaluate coordinate))) = true := by
    decide
  rw [List.all_eq_true] at sweep
  simpa using sweep coordinate swept

/-- **The refutation.**  No valid certificate for generation one's pinned
price is supported on the swept grid — so the price generation one *accepted*
is one this gate refuses, on exactly the coordinates a certificate would most
naturally use. -/
theorem gen1_price_has_no_certificate_on_grid
    (certificate : Certificate gen1Basis.Admitted)
    (priced : certificate.price = gen1Price)
    (supported : ∀ atom ∈ certificate.atoms, atom.1.val ∈ grid)
    (valid : certificate.Valid gen1Basis.basis) : False := by
  have nonneg : 0 ≤ portfolioValue gen1Arbitrage certificate.price :=
    certificate.nonneg_price_of_nonneg_on_support gen1Basis.basis gen1Arbitrage valid
      (fun atom member => gen1_arbitrage_nonneg_on_grid atom.1.val (supported atom member))
  rw [priced] at nonneg
  have priced : portfolioValue gen1Arbitrage gen1Price = -12 := by decide
  omega

/-! ## Direction two: generation one refused a price that is simply attainable

Generation two pinned a live quantized point that `V1b` refuses:
`basis(2, [0,128,256,384], 10_000).evaluate(85) = [1128, 6667, 2205, 0, 0]`,
refused because `3 * 6667 = 20001 > 2 * 10000`.  It gave that point an exact
single-atom certificate.

This tree reproduces the payout vector exactly.
-/

/-- Generation two's over-refusal basis, in this tree's knot vector. -/
def gen2LiveBasis : SplineProfile := profile 2 10000 1 [0, 0, 0, 128, 256, 384, 384, 384]

example : gen2LiveBasis.width = 5 := by decide

/-- **The cross-generational agreement.**  Generation two's evaluator, which
rounds by largest remainder with a lowest-index tie break, and this tree's,
which floors a running cumulative sum, return the same vector at coordinate
`85`. -/
example : gen2LiveBasis.evaluate (at' 85 1) = [1128, 6667, 2205, 0, 0] := by decide

/-- **Generation one refuses it** — the over-refusal, decided. -/
example : v1bDegreeTwoAccepts (gen2LiveBasis.evaluate (at' 85 1)) 10000 = false := by decide

/-- The live coordinate, with its admission discharged by computation. -/
def gen2LivePoint : gen2LiveBasis.Admitted := ⟨at' 85 1, by decide⟩

/-- **This gate admits it**, by the single-atom certificate — decided through
the checker, not asserted. -/
example : (singleAtom gen2LiveBasis.basis gen2LivePoint).check gen2LiveBasis.basis = true := by
  decide

/-- And the same fact as a theorem rather than a computation, for every
attainable payout vector at once. -/
example : (singleAtom gen2LiveBasis.basis gen2LivePoint).Valid gen2LiveBasis.basis :=
  singleAtom_valid gen2LiveBasis.basis gen2LivePoint

/-- The mirror point generation two did not pin.  `V1b` refuses exactly two of
the 385 integer coordinates in this basis's domain, and they are reflections
of each other — which is what an over-refusal caused by a *ceiling* rather
than by geometry looks like. -/
example : gen2LiveBasis.evaluate (at' 299 1) = [0, 0, 2204, 6667, 1129] := by decide

example : v1bDegreeTwoAccepts (gen2LiveBasis.evaluate (at' 299 1)) 10000 = false := by decide

/-! ## The pinned degree-two peak, as a gate refusal

`LiabilityBasisV2SplineExamples` pins the arbitrage as arithmetic:
`quadratic.evaluate (at' 3 1) = [0, 150, 900, 150, 0]` and
`4 * 900 = 3 * 1200`.  `no_certificate_of_capped_claim` turns that into a
refusal.
-/

/-- The peak itself, restated only so the ceiling premise below is legible. -/
example : quadratic.evaluate (at' 3 1) = [0, 150, 900, 150, 0] := by decide

example : 4 * 900 = 3 * quadratic.scale := by decide

/-- **The price `Q * e_2` has no certificate.**

The ceiling is a premise, not a theorem, and deliberately so: `4 * a_2 ≤ 3 * Q`
at *every* admitted coordinate quantifies over an infinite rational domain.
Generation one `decide`-checked the `3/4` entry of its exact ceiling table and
LB-SPLINE reproduced it independently, but neither proved the universal bound,
and neither does this lane.  What is proved here is the implication: **given
the ceiling, the gate refuses the peak price** — and that implication is the
whole reason the gate is not vacuous at degree two. -/
theorem quadratic_peak_price_has_no_certificate
    (certificate : Certificate quadratic.Admitted)
    (ceiling : ∀ result : quadratic.Admitted,
      4 * entryAt (quadratic.evaluate result.val) 2 ≤ 3 * quadratic.scale)
    (full : entryAt certificate.price 2 = quadratic.scale)
    (valid : certificate.Valid quadratic.basis) : False :=
  no_certificate_of_capped_claim quadratic.basis certificate 2 3 4 ceiling (by decide) full valid

/-! ## Degree one: the gate is vacuous, and that is decided

`no_cap_of_attained_scale` says no claim that attains a whole complete set can
be capped, so `no_certificate_of_capped_claim` has no instance at degree one.
LB-SPLINE pinned the attainment (`hats.evaluate (at' 1 1) = [100, 0]`); this
lane cites it rather than restating it.

The sweep below is the stronger, executable form of the same fact: **every**
simplex-admissible price on the two-claim hat basis has a valid certificate.
A degree-one wave therefore needs no price-plane work, which is what the
scorecard has said since the ramp landed.
-/

/-- The two knots at which each hat attains the whole complete set. -/
def hatLeft : hats.Admitted := ⟨at' 1 1, by decide⟩

def hatRight : hats.Admitted := ⟨at' 2 1, by decide⟩

/-- The certificate a two-claim simplex price carries: mix the two attaining
coordinates in the price's own proportions. -/
def hatCertificate (left : Nat) : Certificate hats.Admitted where
  price := [left, 100 - left]
  mass := 100
  atoms := [(hatLeft, left), (hatRight, 100 - left)]

/-- **Every interior simplex price on the hat basis is admitted.**  Ninety-nine
prices, each checked through the gate's own decidable checker. -/
example :
    ((List.range 99).map (· + 1)).all
      (fun left => (hatCertificate left).check hats.basis) = true := by decide

/-- The two endpoints are the single-atom certificates. -/
example : (singleAtom hats.basis hatLeft).check hats.basis = true := by decide

example : (singleAtom hats.basis hatRight).check hats.basis = true := by decide

/-! ## The admission conjunct

`admits` is the rule the evaluator boundary gains: degree `≥ 2` requires a
certificate the checker accepts; degree `≤ 1` does not, but a certificate that
*is* supplied is still checked.
-/

example : admits gen1Basis.basis 1 none = true := by decide

example : admits gen2LiveBasis.basis 2 none = false := by decide

example : admits gen2LiveBasis.basis 3 none = false := by decide

example :
    admits gen2LiveBasis.basis 2 (some (singleAtom gen2LiveBasis.basis gen2LivePoint)) = true := by
  decide

/-- A degree-one basis handed a broken certificate still refuses it: an input
that is present is never silently ignored. -/
example :
    admits hats.basis 1 (some { price := [50, 50], mass := 0, atoms := [(hatLeft, 1)] })
      = false := by decide

end DClutch.LiabilityBasisV2.PriceGate.Examples
