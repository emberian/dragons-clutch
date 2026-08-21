import DragonsClutch.BSplineCorpus

/-!
# The price moment cone, and the admission condition above degree one

`docs/research/DUAL_IS_THE_MEASURE.md` §7.4 refutes the measure half of the
clearing conjecture above degree one: an interior quadratic claim peaks at
`3/4`, so the coordinate vector `S * e_j` passes the exact simplex gate while
no probability measure has it as a moment vector — and the portfolio "three
complete sets short four units of claim `j`" has a nonnegative payoff at every
resolved value and a strictly negative price there.  §7.6 derives the exact
membership condition and the finite certified family the relation enforces as
its V1b stage (`crates/clutch-batch/src/relation_v1.rs`).

This file is the **model plane** of that stage:

* `momentConeAdmits` is the stage, as a decidable Boolean over exact integers,
  mirroring the Rust ladder condition for condition;
* `ceilingCertificate` / `butterflyCertificate` are the portfolios whose
  no-arbitrage inequalities the stage checks;
* the theorems below are `decide`-checked **against the model basis itself**
  (`uniformSmoothBasis?`, the same exact rational evaluator the differential
  corpus uses), in both directions: the certificates really do pay nonnegative
  at every admitted integer resolved value; the refused price really does buy
  them for less than nothing; every point mass's own moment vector is admitted;
  and below degree two the stage is the constant `true` — proved for *all*
  price vectors, not merely decided on witnesses, because that is the
  regression anchor for every landed degree-0 and degree-1 verdict.

Nothing here is a claim about the Rust code: it is the same condition stated
over the model, with witnesses the kernel checked.  Zero `sorry`, core Lean only.
-/

namespace DragonsClutch

/-! ## The stage -/

/-- `max_x N_j(x)` for the open-clamped uniform basis as a rational upper bound
`(num, den)`.  Mirrors `relation_v1.rs::claim_ceiling`; the table and its
derivation are `DUAL_IS_THE_MEASURE.md` §7.6.5. -/
def claimCeiling (degree outcomes claim : Nat) : Nat × Nat :=
  if claim = 0 ∨ claim + 1 = outcomes then (1, 1)
  else if degree = 2 then
    if outcomes = 3 then (1, 2)
    else if claim = 1 ∨ claim + 2 = outcomes then (2, 3)
    else (3, 4)
  else
    if outcomes = 4 then (4, 9)
    else if outcomes = 5 ∧ claim = 2 then (1, 2)
    else if claim ≤ 2 ∨ outcomes ≤ claim + 3 then (3, 5)
    else (2, 3)

/-- The butterfly weight `k` with `k*(N_{j-1} + N_{j+1}) - N_j ≥ 0` everywhere,
as a rational upper bound `(num, den)`; `none` where no finite weight exists,
which is exactly the situation at degree at most one.  Mirrors
`relation_v1.rs::butterfly_weight`. -/
def butterflyWeight (degree outcomes claim : Nat) : Option (Nat × Nat) :=
  if claim = 0 ∨ claim + 1 = outcomes then none
  else if degree = 2 then
    some (if outcomes = 3 then (1, 1)
      else if claim = 1 ∨ claim + 2 = outcomes then (2, 1)
      else (3, 1))
  else
    some (if outcomes = 4 then (7, 8)
      else if claim = 1 ∨ claim + 2 = outcomes then (8, 5)
      else if claim = 2 ∨ claim + 3 = outcomes then (3, 2)
      else (2, 1))

/-- V1b: the moment-cone admission of a candidate price vector, on the exact
scaled simplex `Σ p = scale`.  Assumes the simplex gate already passed. -/
def momentConeAdmits (degree scale : Nat) (prices : List Nat) : Bool :=
  if degree ≤ 1 then true
  else
    let n := prices.length
    let p := fun i => prices.getD i 0
    let ceilings := (List.range n).all fun j =>
      let (a, b) := claimCeiling degree n j
      decide (p j * b ≤ scale * a)
    let butterflies := (List.range n).all fun j =>
      match butterflyWeight degree n j with
      | none => true
      | some (kn, kd) => decide (p j * kd ≤ (p (j - 1) + p (j + 1)) * kn)
    let spans :=
      if n = degree + 1 then
        if degree = 2 then decide (p 1 * p 1 ≤ 4 * (p 0 * p 2))
        else decide (p 1 * p 1 ≤ 3 * (p 0 * p 2)) &&
          decide (p 2 * p 2 ≤ 3 * (p 1 * p 3))
      else true
    ceilings && butterflies && spans

/-- **The reduction.**  At degree zero and degree one the stage is the constant
`true`, for every scale and every price vector: `M_0 = M_1 = Δ`
(`DUAL_IS_THE_MEASURE.md` §7.1, §7.2), the ceiling of a cell indicator or a hat
is `1` — which the simplex gate already enforces — and no finite butterfly
weight exists, because at a point where a hat attains `1` both neighbours
vanish.  Every landed degree-0 and degree-1 verdict is therefore unchanged by
the stage's existence. -/
theorem momentConeAdmits_of_degree_le_one {degree : Nat} (h : degree ≤ 1)
    (scale : Nat) (prices : List Nat) :
    momentConeAdmits degree scale prices = true := by
  unfold momentConeAdmits
  simp [h]

/-! ## Portfolios, payoffs, and prices

A portfolio is an integer coefficient vector in claim space: `+1` is one unit
of a claim, and the complete set is the all-ones vector, which the split/merge
technology makes executable at exactly one collateral unit (§2.3).  Its payoff
at a resolved value is `Σ c_i N_i(x)`, and since the model basis carries one
common denominator, the *sign* of the payoff is the sign of the numerator sum
computed here. -/

/-- The integer numerator of a portfolio's payoff at one exact basis vector. -/
def payoffNumerator (coefficients : List Int) (r : RationalBasis) : Int :=
  (List.zipWith (fun (c : Int) (n : Nat) => c * (n : Int)) coefficients r.numerators).sum

/-- A portfolio's price at an exact integer price vector. -/
def portfolioPrice (coefficients : List Int) (prices : List Nat) : Int :=
  (List.zipWith (fun (c : Int) (p : Nat) => c * (p : Int)) coefficients prices).sum

/-- The ceiling certificate: `a` complete sets short `b` units of claim `j`.
Executable as written — split `a` sets, sell `b` units of claim `j`, keep the
rest — which is the §7.4 split-and-sell position at a general index. -/
def ceilingCertificate (outcomes claim a b : Nat) : List Int :=
  (List.range outcomes).map fun i => if i = claim then (a : Int) - (b : Int) else (a : Int)

/-- The butterfly certificate: `k` units of each neighbour claim, short `1` unit
of claim `j` (scaled by `kd` to stay integral). -/
def butterflyCertificate (outcomes claim kn kd : Nat) : List Int :=
  (List.range outcomes).map fun i =>
    if i = claim then -(kd : Int)
    else if i + 1 = claim ∨ claim + 1 = i then (kn : Int)
    else 0

/-! ## The witnessed grid

Stored knots `[0, 4, 8, 12, 16]`, degree two: six claims, four spans, and a
genuinely interior claim (index three) whose peak is the `3/4` of §7.4.  The
observed values below cover every integer of the closed span and four clamped
exteriors on each side, which is every resolved value the kernel can ever pay
on this grid. -/

/-- The model basis at one integer resolved value of the witnessed grid. -/
def witnessBasis? (x : Nat) : Option RationalBasis := uniformSmoothBasis? 0 4 5 2 x

/-- Every integer resolved value of the witnessed grid, clamped exteriors
included. -/
def witnessValues : List Nat := List.range 21

/-- The refused price vector of §7.4: all price mass on the interior claim
three, `S = 10000`. -/
def peakedPrices : List Nat := [0, 0, 0, 10000, 0, 0]

/-- The ceiling certificate at claim three: three complete sets short four
units of claim three. -/
def peakCeilingPortfolio : List Int := ceilingCertificate 6 3 3 4

/-- The butterfly certificate at claim three: three units of each neighbour,
short one unit of claim three. -/
def peakButterflyPortfolio : List Int := butterflyCertificate 6 3 3 1

/-- **Certificate soundness, decided against the model basis.**  The ceiling
portfolio pays nonnegative at *every* integer resolved value of the grid — the
whole closed span and both clamped exteriors — so it is a nonnegative-payoff
position, not merely a nonnegative-payoff position at the sampled points. -/
theorem peak_ceiling_certificate_never_pays_negative :
    witnessValues.all (fun x =>
      match witnessBasis? x with
      | some r => decide (0 ≤ payoffNumerator peakCeilingPortfolio r)
      | none => false) = true := by
  decide

/-- The same for the butterfly certificate at the same claim. -/
theorem peak_butterfly_certificate_never_pays_negative :
    witnessValues.all (fun x =>
      match witnessBasis? x with
      | some r => decide (0 ≤ payoffNumerator peakButterflyPortfolio r)
      | none => false) = true := by
  decide

/-- One certificate against a whole grid: does it ever pay negative at an
integer resolved value?  Payouts happen only at integer coordinates, so this is
the set on which a position is actually executable. -/
def certificateNeverPaysNegative (origin gap count degree : Nat) (values : List Nat)
    (coefficients : List Int) : Bool :=
  values.all fun x =>
    match uniformSmoothBasis? origin gap count degree x with
    | some r => decide (0 ≤ payoffNumerator coefficients r)
    | none => false

/-- **The rest of the ceiling and butterfly tables, decided.**  Six certificates
covering every position class the tables distinguish above the two already
witnessed: at degree three the interior `2/3` ceiling and its `k = 2` butterfly,
and the two near-clamped-end classes whose exact maxima are irrational
(`(18 + 8·√2)/49` and `(33 + 18·√2)/98`, both bounded by `3/5`) with their
`8/5` and `3/2` butterflies; and at degree two the second-from-end `2/3`
ceiling with its `k = 2` butterfly.  A wrong table entry in either direction
shows up here as a negative payoff. -/
theorem the_table_certificates_never_pay_negative :
    certificateNeverPaysNegative 0 8 6 3 (List.range 45) (ceilingCertificate 8 3 2 3) = true ∧
      certificateNeverPaysNegative 0 8 6 3 (List.range 45) (ceilingCertificate 8 1 3 5) = true ∧
      certificateNeverPaysNegative 0 8 6 3 (List.range 45) (ceilingCertificate 8 2 3 5) = true ∧
      certificateNeverPaysNegative 0 8 6 3 (List.range 45)
          (butterflyCertificate 8 3 2 1) = true ∧
      certificateNeverPaysNegative 0 8 6 3 (List.range 45)
          (butterflyCertificate 8 1 8 5) = true ∧
      certificateNeverPaysNegative 0 8 6 3 (List.range 45)
          (butterflyCertificate 8 2 3 2) = true ∧
      certificateNeverPaysNegative 0 4 5 2 witnessValues (ceilingCertificate 6 1 2 3) = true ∧
      certificateNeverPaysNegative 0 4 5 2 witnessValues
          (butterflyCertificate 6 1 2 1) = true := by
  decide

/-- **And the table is not slack.**  One notch tighter in either family — the
interior degree-three ceiling at `1/2` instead of `2/3`, its butterfly at
`k = 1` instead of `2`, the near-end ceiling at `7/12` instead of `3/5` — and
the certificate does pay negative somewhere on the same grid.  So the theorem
above is a real constraint on the table entries, not a vacuous one: they cannot
be lowered and stay sound. -/
theorem the_table_certificates_are_not_slack :
    certificateNeverPaysNegative 0 8 6 3 (List.range 45) (ceilingCertificate 8 3 1 2) = false ∧
      certificateNeverPaysNegative 0 8 6 3 (List.range 45)
          (butterflyCertificate 8 3 1 1) = false ∧
      certificateNeverPaysNegative 0 8 6 3 (List.range 45)
          (ceilingCertificate 8 1 7 12) = false := by
  decide

/-- **The arbitrage.**  At the refused price both certificates cost strictly
less than nothing: a position with a nonnegative payoff everywhere, acquired
for a negative outlay.  Together with the two theorems above this is the §7.4
counterexample, machine-checked over the model basis. -/
theorem peaked_price_buys_a_nonnegative_payoff_for_less_than_nothing :
    portfolioPrice peakCeilingPortfolio peakedPrices = -10000 ∧
      portfolioPrice peakButterflyPortfolio peakedPrices = -10000 := by
  decide

/-- The stage refuses it at degree two, and at degree three. -/
theorem momentConeAdmits_refuses_the_peaked_price :
    momentConeAdmits 2 10000 peakedPrices = false ∧
      momentConeAdmits 3 10000 peakedPrices = false := by
  decide

/-- The discriminating pair: the same price vector at degree one, where every
simplex vector is a hat-moment vector and the same position pays exactly zero
at the knot. -/
theorem momentConeAdmits_accepts_the_peaked_price_at_degree_one :
    momentConeAdmits 1 10000 peakedPrices = true ∧
      momentConeAdmits 0 10000 peakedPrices = true := by
  decide

/-- Both clamped end claims attain `1`, so their coordinate vectors *are*
moment vectors and the stage must admit them (`DUAL_IS_THE_MEASURE.md` Lemma
7.6.2). -/
theorem momentConeAdmits_accepts_the_clamped_end_coordinates :
    momentConeAdmits 2 10000 [10000, 0, 0, 0, 0, 0] = true ∧
      momentConeAdmits 2 10000 [0, 0, 0, 0, 0, 10000] = true := by
  decide

/-! ## No false refusals: every point mass's own moment vector is admitted -/

/-- The price vector of the point mass at one resolved value, at a scale the
basis denominator divides exactly. -/
def atomPrices (scale : Nat) (r : RationalBasis) : Option (List Nat) :=
  if r.denominator = 0 then none
  else if scale % r.denominator ≠ 0 then none
  else some (r.numerators.map fun n => n * (scale / r.denominator))

/-- **The completeness direction, on the witnessed grid.**  For every integer
resolved value, the exact moment vector of the point mass there — the price
vector a market that is certain of that outcome would publish — is admitted by
the stage.  A stage that refused one of these would be refusing a genuine
measure, and would cost the venue clearings it should have made. -/
theorem momentConeAdmits_accepts_every_point_mass :
    witnessValues.all (fun x =>
      match witnessBasis? x with
      | some r =>
        match atomPrices 1048576 r with
        | some prices => momentConeAdmits 2 1048576 prices
        | none => false
      | none => false) = true := by
  decide

/-- The interior peak, spelled out: the point mass at the midpoint of the third
span has moment vector `(0, 0, 1/8, 3/4, 1/8, 0)`, whose price vector at
`S = 8192` is `[0, 0, 1024, 6144, 1024, 0]`.  The stage admits it and refuses
the vector one atom above it: the peak sits exactly on the cone boundary, where
both certificates are tight. -/
theorem the_interior_peak_is_the_boundary :
    momentConeAdmits 2 8192 [0, 0, 1024, 6144, 1024, 0] = true ∧
      momentConeAdmits 2 8192 [0, 0, 1023, 6145, 1024, 0] = false := by
  decide

/-- And it really is the model basis's own value there, up to the common
denominator the recurrence reaches. -/
theorem the_interior_peak_is_the_model_basis :
    atomPrices 8192 <$> witnessBasis? 10 = some (some [0, 0, 1024, 6144, 1024, 0]) := by
  decide

/-! ## The single-span grids, where the stage is exact

At `outcome_count = degree + 1` the basis is the Bernstein basis of one span and
the two Hankel quadrics of Corollary 7.6.6 are *exactly* moment-cone
membership.  Both are tight at every point mass. -/

theorem single_span_quadrics_are_tight_at_the_point_masses :
    momentConeAdmits 2 4096 [1024, 2048, 1024] = true ∧
      momentConeAdmits 2 4096 [1023, 2049, 1024] = false ∧
      momentConeAdmits 3 4096 [512, 1536, 1536, 512] = true ∧
      momentConeAdmits 3 4096 [511, 1537, 1536, 512] = false := by
  decide

/-- The single-span degree-two basis at the midpoint of `[0,4]` is the
Bernstein `(1,2,1)/4`, so `[1024, 2048, 1024]` above is a real moment vector and
not a convenient triple. -/
theorem single_span_midpoint_is_bernstein :
    atomPrices 4096 <$> uniformSmoothBasis? 0 4 2 2 2 = some (some [1024, 2048, 1024]) := by
  decide

end DragonsClutch
