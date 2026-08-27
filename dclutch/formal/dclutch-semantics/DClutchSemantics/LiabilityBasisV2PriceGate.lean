import DClutchSemantics.LiabilityBasisV2Spline

/-!
# The degree-`≥ 2` arbitrage gate

`LiabilityBasisV2` is the *claim* plane: it certifies that a basis pays a
nonnegative integer partition of one collateral scale `Q`.  This module is the
*price* plane, and it exists because at degree `≥ 2` those two planes come
apart.

At degree `≤ 1` a claim attains the whole complete set somewhere, so
`p ≥ 0, sum p = Q` — the simplex condition — is the whole no-arbitrage
condition.  At degree `≥ 2` it stops being: an interior basis function peaks
strictly below one (`3/4` at degree two, `2/3` at degree three, both pinned as
`decide` witnesses in `LiabilityBasisV2SplineExamples`), so the portfolio
*three complete sets, short four units of the interior claim* has a globally
nonnegative payoff and, at the simplex-admissible price `Q * e_j`, a strictly
negative price.  That is an executable arbitrage, not a theoretical one.

## What this module certifies

The gate is **integer hull membership**.  A price vector `p` is admitted when
a certificate exhibits it as a nonnegative integer mixture of *actually
attainable* payout vectors:

```text
0 < W,  every weight positive,  sum weights = W
W * p_i = sum over atoms of weight * evaluate(result)_i     for every claim i
```

Every atom is **recomputed by the basis's own evaluator** and never supplied
by the caller.  The certificate needs exactly one thing from a basis — a
deterministic integer evaluator whose payouts sum to a fixed scale — which is
precisely what `LiabilityBasisV2.Basis` is.  It asks for no uniformity, no
knot vector, no degree and no span decomposition, so it is indifferent to
every axis on which the `Spline` family is more general than its predecessors.

`Certificate.no_arbitrage` is the theorem: **a price with a valid certificate
admits no arbitrage against the reachable payout set.**

## Provenance

The requirement and the *shape* of the certificate are generation two's, from
`crates/clutch-price-measure/src/atom_mixture_v1.rs` on `dragons-clutch`
(`verify_quantized_atom_mixture_v1`, 2026-08-23/24), which had 48 Rust tests
and zero theorems.  See `docs/compost/PRICE_GATE_HULL_2026_08_27.md`.  The
mathematics below was written against `LiabilityBasisV2.Basis` and shares no
line with it.
-/

namespace DClutch.LiabilityBasisV2.PriceGate

open DClutch.LiabilityBasisV2

/-! ## Signed portfolios

A portfolio holds an integer quantity of each elementary claim; a complete set
is one unit of every claim.  Holdings are signed because a short position is
what makes an arbitrage expressible at all, and the payouts it is valued
against are not.
-/

/-- Value of one signed portfolio against one nonnegative payout vector.
Ragged input truncates, exactly as `liability` does; every theorem below
carries the width premise that rules it out. -/
def portfolioValue : List Int → List Nat → Int
  | holding :: holdings, payout :: payouts =>
      holding * (payout : Int) + portfolioValue holdings payouts
  | _, _ => 0

@[simp] theorem portfolioValue_nil_payouts (portfolio : List Int) :
    portfolioValue portfolio [] = 0 := by
  cases portfolio <;> rfl

@[simp] theorem portfolioValue_nil_portfolio (payouts : List Nat) :
    portfolioValue [] payouts = 0 := by
  cases payouts <;> rfl

/-- Scale one payout vector by a nonnegative factor. -/
def scaleVector (factor : Nat) (values : List Nat) : List Nat :=
  values.map (fun value => factor * value)

@[simp] theorem scaleVector_length (factor : Nat) (values : List Nat) :
    (scaleVector factor values).length = values.length := by
  simp [scaleVector]

@[simp] theorem scaleVector_nil (factor : Nat) : scaleVector factor [] = [] := rfl

@[simp] theorem scaleVector_cons (factor value : Nat) (values : List Nat) :
    scaleVector factor (value :: values) = factor * value :: scaleVector factor values := rfl

theorem scaleVector_sum (factor : Nat) (values : List Nat) :
    (scaleVector factor values).sum = factor * values.sum :=
  Spline.sum_map_mul_left factor values

/-- Regrouping helper: the two associativity rewrites every bilinearity proof
below needs, closed once by `omega` on opaque atoms. -/
private theorem int_regroup (left right lower upper : Int) :
    left + right + (lower + upper) = left + lower + (right + upper) := by omega

theorem portfolioValue_scaleVector
    (portfolio : List Int) (factor : Nat) (values : List Nat) :
    portfolioValue portfolio (scaleVector factor values)
      = (factor : Int) * portfolioValue portfolio values := by
  induction portfolio generalizing values with
  | nil => simp
  | cons holding holdings induction =>
      cases values with
      | nil => simp
      | cons value values =>
          have cast : ((factor * value : Nat) : Int) = (factor : Int) * (value : Int) := by
            omega
          simp only [scaleVector_cons, portfolioValue, induction values, cast, Int.mul_add]
          rw [← Int.mul_assoc, Int.mul_comm holding (factor : Int), Int.mul_assoc]

theorem pointwiseAdd_length
    (left right : List Nat) (sameWidth : left.length = right.length) :
    (pointwiseAdd left right).length = left.length := by
  induction left generalizing right with
  | nil => rfl
  | cons _ lefts induction =>
      cases right with
      | nil => simp at sameWidth
      | cons _ rights =>
          simp only [pointwiseAdd, List.length_cons]
          rw [induction rights (by simpa using sameWidth)]

theorem pointwiseAdd_sum
    (left right : List Nat) (sameWidth : left.length = right.length) :
    (pointwiseAdd left right).sum = left.sum + right.sum := by
  induction left generalizing right with
  | nil =>
      cases right with
      | nil => simp [pointwiseAdd]
      | cons _ _ => simp at sameWidth
  | cons leftValue lefts induction =>
      cases right with
      | nil => simp at sameWidth
      | cons rightValue rights =>
          simp only [pointwiseAdd, List.sum_cons]
          rw [induction rights (by simpa using sameWidth)]
          omega

theorem portfolioValue_add
    (portfolio : List Int) (left right : List Nat)
    (sameWidth : left.length = right.length) :
    portfolioValue portfolio (pointwiseAdd left right)
      = portfolioValue portfolio left + portfolioValue portfolio right := by
  induction portfolio generalizing left right with
  | nil => simp
  | cons holding holdings induction =>
      cases left with
      | nil =>
          cases right with
          | nil => simp [pointwiseAdd]
          | cons _ _ => simp at sameWidth
      | cons leftValue lefts =>
          cases right with
          | nil => simp at sameWidth
          | cons rightValue rights =>
              have cast : ((leftValue + rightValue : Nat) : Int)
                  = (leftValue : Int) + (rightValue : Int) := by omega
              simp only [pointwiseAdd, portfolioValue,
                induction lefts rights (by simpa using sameWidth), cast, Int.mul_add]
              exact int_regroup _ _ _ _

theorem portfolioValue_replicate_zero (portfolio : List Int) (width : Nat) :
    portfolioValue portfolio (List.replicate width 0) = 0 := by
  induction portfolio generalizing width with
  | nil => simp
  | cons _ holdings induction =>
      cases width with
      | zero => simp
      | succ width =>
          rw [List.replicate_succ]
          simp only [portfolioValue]
          rw [induction width]
          simp

/-! ### Total coordinate reads distribute

`entryAt` is `LiabilityBasisV2`'s total read: an out-of-range coordinate reads
zero.  Both distribution facts below hold unconditionally past the end because
equal-width operands keep the result at that same width.
-/

theorem entryAt_scaleVector (factor : Nat) (values : List Nat) (index : Nat) :
    entryAt (scaleVector factor values) index = factor * entryAt values index := by
  induction values generalizing index with
  | nil => simp [entryAt]
  | cons value values induction =>
      cases index with
      | zero => simp
      | succ index => simpa using induction index

theorem entryAt_pointwiseAdd
    (left right : List Nat) (index : Nat) (sameWidth : left.length = right.length) :
    entryAt (pointwiseAdd left right) index = entryAt left index + entryAt right index := by
  induction left generalizing right index with
  | nil =>
      cases right with
      | nil => simp [pointwiseAdd]
      | cons _ _ => simp at sameWidth
  | cons leftValue lefts induction =>
      cases right with
      | nil => simp at sameWidth
      | cons rightValue rights =>
          cases index with
          | zero => simp [pointwiseAdd]
          | succ index =>
              simpa [pointwiseAdd] using induction rights index (by simpa using sameWidth)

theorem entryAt_replicate_zero (width index : Nat) :
    entryAt (List.replicate width 0) index = 0 := by
  induction width generalizing index with
  | zero => simp [entryAt]
  | succ width induction =>
      cases index with
      | zero => simp [List.replicate_succ]
      | succ index => simpa [List.replicate_succ] using induction index

theorem sum_nonneg_of_mem_nonneg (values : List Int) (nonneg : ∀ value ∈ values, 0 ≤ value) :
    0 ≤ values.sum := by
  induction values with
  | nil => simp
  | cons value values induction =>
      have head : 0 ≤ value := nonneg value (by simp)
      have tail : 0 ≤ values.sum := induction (fun entry member => nonneg entry (by simp [member]))
      simpa using Int.add_nonneg head tail

/-! ## The mixture

One atom is an admitted terminal result together with a positive integer
weight.  The mixture is the weighted sum of the *recomputed* payout vectors:
nothing about an atom's payouts is ever taken from the certificate.
-/

/-- The exact integer mixture a weighted atom list defines, over a **bare
evaluator**.  Every payout vector here is produced by that evaluator, never
supplied by a caller.

The physical boundary decides admission before it has a `Basis` instance to
name — an atom's coordinate is admitted or refused by a check, not carried as
a proof — so the mixture is stated at this level and `mixture` below is its
specialization. -/
def rawMixture (width : Nat) (evaluate : Result → List Nat) :
    List (Result × Nat) → List Nat
  | [] => List.replicate width 0
  | atom :: atoms =>
      pointwiseAdd (scaleVector atom.2 (evaluate atom.1)) (rawMixture width evaluate atoms)

/-- The mixture over one certified basis. -/
def mixture (basis : Basis Result) (atoms : List (Result × Nat)) : List Nat :=
  rawMixture basis.width basis.evaluate atoms

/-- Every payout vector a mixture consumes has the mixture's own width.  The
hypothesis is scoped to the atoms actually present, because an evaluator is
free to be partial: `SplineProfile.evaluate` returns a well-formed partition
only at an *admitted* coordinate, and the physical boundary checks admission
per atom rather than assuming it everywhere. -/
theorem rawMixture_length
    (width : Nat) (evaluate : Result → List Nat) (atoms : List (Result × Nat))
    (exactWidth : ∀ atom ∈ atoms, (evaluate atom.1).length = width) :
    (rawMixture width evaluate atoms).length = width := by
  induction atoms with
  | nil => simp [rawMixture]
  | cons atom atoms induction =>
      have head : (evaluate atom.1).length = width := exactWidth atom (by simp)
      have tail : (rawMixture width evaluate atoms).length = width :=
        induction (fun entry member => exactWidth entry (by simp [member]))
      simp only [rawMixture]
      rw [pointwiseAdd_length _ _ (by simp [head, tail]), scaleVector_length, head]

@[simp] theorem mixture_length (basis : Basis Result) (atoms : List (Result × Nat)) :
    (mixture basis atoms).length = basis.width :=
  rawMixture_length _ _ atoms (fun atom _ => basis.exactWidth atom.1)

/-- **The mixture's mass is the total weight times the collateral scale**, at
the bare-evaluator level: each atom paying exactly `scale` is all it takes. -/
theorem rawMixture_sum
    (width scale : Nat) (evaluate : Result → List Nat) (atoms : List (Result × Nat))
    (exactWidth : ∀ atom ∈ atoms, (evaluate atom.1).length = width)
    (partition : ∀ atom ∈ atoms, (evaluate atom.1).sum = scale) :
    (rawMixture width evaluate atoms).sum = (atoms.map Prod.snd).sum * scale := by
  induction atoms with
  | nil => simp [rawMixture]
  | cons atom atoms induction =>
      have head : (evaluate atom.1).length = width := exactWidth atom (by simp)
      have tail : (rawMixture width evaluate atoms).length = width :=
        rawMixture_length _ _ _ (fun entry member => exactWidth entry (by simp [member]))
      simp only [rawMixture, List.map_cons, List.sum_cons]
      rw [pointwiseAdd_sum _ _ (by simp [head, tail]), scaleVector_sum,
        partition atom (by simp),
        induction (fun entry member => exactWidth entry (by simp [member]))
          (fun entry member => partition entry (by simp [member]))]
      exact (Nat.add_mul _ _ _).symm

/-- **The mixture's mass is the total weight times the collateral scale.**
Every atom is itself a partition of `Q`, so the mixture is a partition of
`(sum of weights) * Q`. -/
theorem mixture_sum (basis : Basis Result) (atoms : List (Result × Nat)) :
    (mixture basis atoms).sum = (atoms.map Prod.snd).sum * basis.scale :=
  rawMixture_sum _ _ _ atoms (fun atom _ => basis.exactWidth atom.1)
    (fun atom _ => basis.partitionUnity atom.1)

/-- **Bilinearity.** A portfolio prices a mixture by pricing every atom and
taking the same weighted sum.  This is the whole soundness argument in one
line, and it is why the gate needs nothing from an evaluator beyond a fixed
width on the atoms it actually uses. -/
theorem portfolioValue_rawMixture
    (width : Nat) (evaluate : Result → List Nat) (portfolio : List Int)
    (atoms : List (Result × Nat))
    (exactWidth : ∀ atom ∈ atoms, (evaluate atom.1).length = width) :
    portfolioValue portfolio (rawMixture width evaluate atoms)
      = (atoms.map (fun atom =>
          (atom.2 : Int) * portfolioValue portfolio (evaluate atom.1))).sum := by
  induction atoms with
  | nil => simp [rawMixture, portfolioValue_replicate_zero]
  | cons atom atoms induction =>
      have head : (evaluate atom.1).length = width := exactWidth atom (by simp)
      have tail : (rawMixture width evaluate atoms).length = width :=
        rawMixture_length _ _ _ (fun entry member => exactWidth entry (by simp [member]))
      simp only [rawMixture, List.map_cons, List.sum_cons]
      rw [portfolioValue_add _ _ _ (by simp [head, tail]), portfolioValue_scaleVector,
        induction (fun entry member => exactWidth entry (by simp [member]))]

theorem portfolioValue_mixture
    (basis : Basis Result) (portfolio : List Int) (atoms : List (Result × Nat)) :
    portfolioValue portfolio (mixture basis atoms)
      = (atoms.map (fun atom =>
          (atom.2 : Int) * portfolioValue portfolio (basis.evaluate atom.1))).sum :=
  portfolioValue_rawMixture _ _ portfolio atoms (fun atom _ => basis.exactWidth atom.1)

/-- **The coordinate reconstruction.**  One claim's mixture entry is the
weighted sum of that claim's payouts, which is the equation
`verify_quantized_atom_mixture_v1` checks componentwise. -/
theorem entryAt_rawMixture
    (width : Nat) (evaluate : Result → List Nat) (atoms : List (Result × Nat))
    (claim : Nat) (exactWidth : ∀ atom ∈ atoms, (evaluate atom.1).length = width) :
    entryAt (rawMixture width evaluate atoms) claim
      = (atoms.map (fun atom => atom.2 * entryAt (evaluate atom.1) claim)).sum := by
  induction atoms with
  | nil => simp [rawMixture, entryAt_replicate_zero]
  | cons atom atoms induction =>
      have head : (evaluate atom.1).length = width := exactWidth atom (by simp)
      have tail : (rawMixture width evaluate atoms).length = width :=
        rawMixture_length _ _ _ (fun entry member => exactWidth entry (by simp [member]))
      simp only [rawMixture, List.map_cons, List.sum_cons]
      rw [entryAt_pointwiseAdd _ _ _ (by simp [head, tail]), entryAt_scaleVector,
        induction (fun entry member => exactWidth entry (by simp [member]))]

theorem entryAt_mixture
    (basis : Basis Result) (atoms : List (Result × Nat)) (claim : Nat) :
    entryAt (mixture basis atoms) claim
      = (atoms.map (fun atom => atom.2 * entryAt (basis.evaluate atom.1) claim)).sum :=
  entryAt_rawMixture _ _ atoms claim (fun atom _ => basis.exactWidth atom.1)

/-- **Soundness, at the bare-evaluator level.**  A positive mass, an exact
integer reconstruction, and a portfolio that pays nonnegatively at every atom
are all soundness needs.  No partition of unity, no bound, no degree.  This is
the theorem the physical boundary carries, and `Certificate.no_arbitrage`
below is its specialization to a certified basis. -/
theorem nonneg_price_of_raw_mixture
    (width : Nat) (evaluate : Result → List Nat)
    (portfolio : List Int) (price : List Nat) (mass : Nat)
    (atoms : List (Result × Nat))
    (exactWidth : ∀ atom ∈ atoms, (evaluate atom.1).length = width)
    (massPositive : 0 < mass)
    (reconstructs : scaleVector mass price = rawMixture width evaluate atoms)
    (nonneg : ∀ atom ∈ atoms, 0 ≤ portfolioValue portfolio (evaluate atom.1)) :
    0 ≤ portfolioValue portfolio price := by
  have scaled : 0 ≤ (mass : Int) * portfolioValue portfolio price := by
    rw [← portfolioValue_scaleVector, reconstructs,
      portfolioValue_rawMixture _ _ portfolio atoms exactWidth]
    refine sum_nonneg_of_mem_nonneg _ ?_
    intro value member
    simp only [List.mem_map] at member
    obtain ⟨atom, atomMember, rfl⟩ := member
    exact Int.mul_nonneg (Int.natCast_nonneg _) (nonneg atom atomMember)
  have massInt : (0 : Int) < (mass : Int) := by omega
  have folded : (mass : Int) * 0 ≤ (mass : Int) * portfolioValue portfolio price := by
    simpa using scaled
  exact Int.le_of_mul_le_mul_left folded massInt

/-! ## The certificate -/

/-- One nonnegative-integer mixture certificate for a claimed price vector.

`price` is on the basis's own payout scale: the components are integers that
sum to `Q`.  That is deliberately narrower than an arbitrary rational price
scale — it is the choice generation two made, and it means the certificate can
represent exactly the prices that live on the collateral lattice. -/
structure Certificate (Result : Type) where
  /-- The claimed price vector, one component per elementary claim. -/
  price : List Nat
  /-- Positive common denominator of the mixture weights. -/
  mass : Nat
  /-- Sparse support: admitted terminal results with positive weights. -/
  atoms : List (Result × Nat)

/-- **Certificate validity.**  Exactly the conjuncts
`verify_quantized_atom_mixture_v1` checks, minus the physical envelope: a
positive mass, a nonempty support of positive weights summing to that mass, a
price of the basis's width, and exact componentwise integer reconstruction. -/
def Certificate.Valid (certificate : Certificate Result) (basis : Basis Result) : Prop :=
  0 < certificate.mass ∧
    certificate.atoms ≠ [] ∧
    (∀ atom ∈ certificate.atoms, 0 < atom.2) ∧
    (certificate.atoms.map Prod.snd).sum = certificate.mass ∧
    certificate.price.length = basis.width ∧
    scaleVector certificate.mass certificate.price = mixture basis certificate.atoms

/-- The decidable checker.  `check_eq_true_iff` proves it is exactly
`Certificate.Valid` and not an approximation of it. -/
def Certificate.check (certificate : Certificate Result) (basis : Basis Result) : Bool :=
  decide (0 < certificate.mass) &&
    !certificate.atoms.isEmpty &&
    certificate.atoms.all (fun atom => decide (0 < atom.2)) &&
    decide ((certificate.atoms.map Prod.snd).sum = certificate.mass) &&
    decide (certificate.price.length = basis.width) &&
    decide (scaleVector certificate.mass certificate.price = mixture basis certificate.atoms)

/-- **The checker decides validity exactly.**  A hostile implementation that
weakened any conjunct would fail here rather than silently admit. -/
theorem Certificate.check_eq_true_iff
    (certificate : Certificate Result) (basis : Basis Result) :
    certificate.check basis = true ↔ certificate.Valid basis := by
  unfold Certificate.check Certificate.Valid
  simp only [Bool.and_eq_true, decide_eq_true_eq, List.all_eq_true,
    Bool.not_eq_eq_eq_not, Bool.not_true, List.isEmpty_eq_false_iff]
  constructor
  · rintro ⟨⟨⟨⟨⟨mass, nonempty⟩, positive⟩, weights⟩, width⟩, reconstruction⟩
    exact ⟨mass, nonempty, fun atom member => by simpa using positive atom member,
      weights, width, reconstruction⟩
  · rintro ⟨mass, nonempty, positive, weights, width, reconstruction⟩
    exact ⟨⟨⟨⟨⟨mass, nonempty⟩, fun atom member => by simpa using positive atom member⟩,
      weights⟩, width⟩, reconstruction⟩

instance (certificate : Certificate Result) (basis : Basis Result) :
    Decidable (certificate.Valid basis) :=
  decidable_of_iff _ (certificate.check_eq_true_iff basis)

/-! ## Soundness -/

theorem Certificate.mass_mul_portfolioValue
    (certificate : Certificate Result) (basis : Basis Result) (portfolio : List Int)
    (valid : certificate.Valid basis) :
    (certificate.mass : Int) * portfolioValue portfolio certificate.price
      = (certificate.atoms.map (fun atom =>
          (atom.2 : Int) * portfolioValue portfolio (basis.evaluate atom.1))).sum := by
  obtain ⟨_, _, _, _, _, reconstruction⟩ := valid
  rw [← portfolioValue_scaleVector, reconstruction, portfolioValue_mixture]

/-- **The gate's theorem, in its sharpest form.**  A portfolio that pays
nonnegatively at every atom the certificate names cannot have a negative
price.  The support is finite, so this hypothesis is decidable. -/
theorem Certificate.nonneg_price_of_nonneg_on_support
    (certificate : Certificate Result) (basis : Basis Result) (portfolio : List Int)
    (valid : certificate.Valid basis)
    (nonneg : ∀ atom ∈ certificate.atoms,
      0 ≤ portfolioValue portfolio (basis.evaluate atom.1)) :
    0 ≤ portfolioValue portfolio certificate.price :=
  nonneg_price_of_raw_mixture basis.width basis.evaluate portfolio
    certificate.price certificate.mass certificate.atoms
    (fun atom _ => basis.exactWidth atom.1) valid.1 valid.2.2.2.2.2 nonneg

/-- **No arbitrage.**  A price with a valid certificate admits no portfolio
whose payoff is nonnegative at every terminal result and whose price is
strictly negative.  This is the statement generation one's moment cone was
*wrong* about and generation two never wrote down. -/
theorem Certificate.no_arbitrage
    (certificate : Certificate Result) (basis : Basis Result) (portfolio : List Int)
    (valid : certificate.Valid basis)
    (nonnegative : ∀ result, 0 ≤ portfolioValue portfolio (basis.evaluate result)) :
    0 ≤ portfolioValue portfolio certificate.price :=
  certificate.nonneg_price_of_nonneg_on_support basis portfolio valid
    (fun atom _ => nonnegative atom.1)

/-! ### The gate strictly strengthens the simplex condition -/

theorem sum_pos_of_mem_pos
    (values : List Nat) (nonempty : values ≠ [])
    (positive : ∀ value ∈ values, 0 < value) : 0 < values.sum := by
  cases values with
  | nil => exact absurd rfl nonempty
  | cons value values =>
      have head : 0 < value := positive value (by simp)
      simp only [List.sum_cons]
      omega

theorem sum_map_le_sum_map
    (atoms : List (Result × Nat)) (lower upper : Result × Nat → Nat)
    (pointwise : ∀ atom ∈ atoms, lower atom ≤ upper atom) :
    (atoms.map lower).sum ≤ (atoms.map upper).sum := by
  induction atoms with
  | nil => simp
  | cons atom atoms induction =>
      have head : lower atom ≤ upper atom := pointwise atom (by simp)
      have tail : (atoms.map lower).sum ≤ (atoms.map upper).sum :=
        induction (fun entry member => pointwise entry (by simp [member]))
      simp only [List.map_cons, List.sum_cons]
      omega

theorem sum_map_mul_right (atoms : List (Result × Nat)) (factor : Nat) :
    (atoms.map (fun atom => atom.2 * factor)).sum = (atoms.map Prod.snd).sum * factor := by
  induction atoms with
  | nil => simp
  | cons atom atoms induction =>
      simp only [List.map_cons, List.sum_cons, induction]
      exact (Nat.add_mul _ _ _).symm

theorem mem_le_sum (values : List Nat) (value : Nat) (member : value ∈ values) :
    value ≤ values.sum := by
  induction values with
  | nil => simp at member
  | cons head tail induction =>
      rcases List.mem_cons.1 member with rfl | member
      · simp
      · have := induction member
        simp only [List.sum_cons]
        omega

/-- **A certified price sums to exactly the collateral scale.**  The simplex
condition is not an extra premise of this gate: it is a consequence, so a gate
certificate can only ever refuse more than `p ≥ 0, sum p = Q`, never less. -/
theorem Certificate.price_sum
    (certificate : Certificate Result) (basis : Basis Result)
    (valid : certificate.Valid basis) : certificate.price.sum = basis.scale := by
  obtain ⟨massPositive, _, _, weights, _, reconstruction⟩ := valid
  have left : (scaleVector certificate.mass certificate.price).sum
      = certificate.mass * certificate.price.sum := scaleVector_sum _ _
  have right : (mixture basis certificate.atoms).sum = certificate.mass * basis.scale := by
    rw [mixture_sum, weights]
  rw [reconstruction, right] at left
  exact Nat.eq_of_mul_eq_mul_left massPositive left.symm

/-- The hostile partition checker `LiabilityBasisV2` already owns never
refuses a certified price. -/
theorem Certificate.validPartition_price
    (certificate : Certificate Result) (basis : Basis Result)
    (valid : certificate.Valid basis) :
    validPartition certificate.price basis.scale = true := by
  have sums := certificate.price_sum basis valid
  have nonempty : certificate.price ≠ [] := by
    intro empty
    have width := valid.2.2.2.2.1
    rw [empty, List.length_nil] at width
    have := basis.widthPositive
    omega
  unfold validPartition
  simp only [Bool.and_eq_true, decide_eq_true_eq, List.all_eq_true,
    Bool.not_eq_eq_eq_not, Bool.not_true, List.isEmpty_eq_false_iff]
  refine ⟨⟨⟨nonempty, basis.scalePositive⟩, ?_⟩, sums⟩
  intro component member
  have := mem_le_sum certificate.price component member
  omega

/-- The componentwise reconstruction, as `verify_quantized_atom_mixture_v1`
states it: `p_i * W = sum over atoms of weight * atom_i`. -/
theorem Certificate.reconstruction_at
    (certificate : Certificate Result) (basis : Basis Result) (claim : Nat)
    (valid : certificate.Valid basis) :
    entryAt certificate.price claim * certificate.mass
      = (certificate.atoms.map (fun atom =>
          atom.2 * entryAt (basis.evaluate atom.1) claim)).sum := by
  obtain ⟨_, _, _, _, _, reconstruction⟩ := valid
  have reconstructed := congrArg (fun values => entryAt values claim) reconstruction
  simp only [entryAt_scaleVector, entryAt_mixture] at reconstructed
  rw [Nat.mul_comm]
  exact reconstructed

/-! ## Constructive completeness of the hull side

Every positive-integer mixture of admitted payout vectors is admitted.  The
narrow but load-bearing instance is a single atom at weight one: a price that
*is* an attainable payout vector is always certified — the case generation
one's moment cone wrongly refused
(`dragons-clutch crates/clutch-price-measure/tests/adversarial.rs:281`).
-/

/-- Aggregating a width-`w` vector with the zero vector of that width is the
identity, which is the base case of every single-atom certificate. -/
theorem pointwiseAdd_replicate_zero_right
    (values : List Nat) (width : Nat) (sameWidth : values.length = width) :
    pointwiseAdd values (List.replicate width 0) = values := by
  induction values generalizing width with
  | nil => cases width <;> simp [pointwiseAdd]
  | cons value values induction =>
      cases width with
      | zero => simp at sameWidth
      | succ width =>
          rw [List.replicate_succ]
          simp only [pointwiseAdd, Nat.add_zero]
          rw [induction width (by simpa using sameWidth)]

/-- The certificate a single attainable payout vector carries. -/
def singleAtom (basis : Basis Result) (result : Result) : Certificate Result where
  price := basis.evaluate result
  mass := 1
  atoms := [(result, 1)]

/-- **Every attainable payout vector is its own price certificate.**  A market
that trades at exactly what some terminal coordinate pays is never refused. -/
theorem singleAtom_valid (basis : Basis Result) (result : Result) :
    (singleAtom basis result).Valid basis := by
  refine ⟨Nat.one_pos, by simp [singleAtom], ?_, by simp [singleAtom], ?_, ?_⟩
  · intro atom member
    simp only [singleAtom, List.mem_singleton] at member
    subst member
    exact Nat.one_pos
  · simpa [singleAtom] using basis.exactWidth result
  · simp only [singleAtom, mixture, rawMixture]
    exact (pointwiseAdd_replicate_zero_right _ _ (by simp [basis.exactWidth])).symm

/-- Any weighted atom list whose reconstruction closes is a valid certificate:
the checker admits *every* honest mixture, not a distinguished normal form. -/
theorem mixture_valid
    (basis : Basis Result) (price : List Nat) (mass : Nat)
    (atoms : List (Result × Nat))
    (nonempty : atoms ≠ []) (positive : ∀ atom ∈ atoms, 0 < atom.2)
    (weights : (atoms.map Prod.snd).sum = mass)
    (width : price.length = basis.width)
    (reconstructs : scaleVector mass price = mixture basis atoms) :
    ({ price, mass, atoms } : Certificate Result).Valid basis := by
  refine ⟨?_, nonempty, positive, weights, width, reconstructs⟩
  rw [← weights]
  refine sum_pos_of_mem_pos _ (by simpa using nonempty) ?_
  intro value member
  simp only [List.mem_map] at member
  obtain ⟨atom, atomMember, rfl⟩ := member
  exact positive atom atomMember

/-! ## Why the gate has teeth, and when it has none

Both directions are proved once, over an arbitrary `Basis`, so neither depends
on the spline family or on any degree.
-/

/-- **The capped-claim refusal.**  If some claim can never pay more than
`cap / multiplier` of a complete set, with `cap < multiplier`, then the
simplex-admissible price that pays that claim the whole scale has **no valid
certificate at all**.

The economics: the portfolio *`cap` complete sets, short `multiplier` units of
that claim* pays `cap * Q - multiplier * a_j ≥ 0` everywhere and costs
`(cap - multiplier) * Q < 0` at that price.  With `cap/multiplier = 3/4` this
is exactly the arbitrage `LiabilityBasisV2SplineExamples` pins as arithmetic
at the degree-two interior peak. -/
theorem no_certificate_of_capped_claim
    (basis : Basis Result) (certificate : Certificate Result)
    (claim cap multiplier : Nat)
    (capped : ∀ result,
      multiplier * entryAt (basis.evaluate result) claim ≤ cap * basis.scale)
    (tight : cap < multiplier)
    (full : entryAt certificate.price claim = basis.scale)
    (valid : certificate.Valid basis) : False := by
  have massPositive : 0 < certificate.mass := valid.1
  have weights : (certificate.atoms.map Prod.snd).sum = certificate.mass := valid.2.2.2.1
  have reconstruction := certificate.reconstruction_at basis claim valid
  rw [full] at reconstruction
  -- Multiply the claim-`j` reconstruction through by the cap's multiplier.
  have scaled : multiplier * (basis.scale * certificate.mass)
      = (certificate.atoms.map (fun atom =>
          multiplier * (atom.2 * entryAt (basis.evaluate atom.1) claim))).sum := by
    rw [reconstruction, ← Spline.sum_map_mul_left, List.map_map]
    rfl
  -- Every atom's contribution is capped, so the whole mixture is.
  have bounded := sum_map_le_sum_map certificate.atoms
    (fun atom => multiplier * (atom.2 * entryAt (basis.evaluate atom.1) claim))
    (fun atom => atom.2 * (cap * basis.scale))
    (fun atom _ => by
      calc multiplier * (atom.2 * entryAt (basis.evaluate atom.1) claim)
          = atom.2 * (multiplier * entryAt (basis.evaluate atom.1) claim) := by
            rw [← Nat.mul_assoc, Nat.mul_comm multiplier atom.2, Nat.mul_assoc]
        _ ≤ atom.2 * (cap * basis.scale) := Nat.mul_le_mul_left _ (capped atom.1))
  rw [sum_map_mul_right, weights, ← scaled] at bounded
  -- Cancel the two positive factors and contradict `cap < multiplier`.
  have folded : certificate.mass * (basis.scale * multiplier)
      ≤ certificate.mass * (basis.scale * cap) := by
    calc certificate.mass * (basis.scale * multiplier)
        = multiplier * (basis.scale * certificate.mass) := by
          simp [Nat.mul_comm, Nat.mul_left_comm]
      _ ≤ certificate.mass * (cap * basis.scale) := bounded
      _ = certificate.mass * (basis.scale * cap) := by rw [Nat.mul_comm cap basis.scale]
  have cancelled := Nat.le_of_mul_le_mul_left folded massPositive
  have final := Nat.le_of_mul_le_mul_left cancelled basis.scalePositive
  omega

/-- **And why degree `≤ 1` is exempt.**  No claim that attains a whole complete
set at some admitted result can be capped, so `no_certificate_of_capped_claim`
has no instance against it.

At degree one a hat attains the whole complete set at its own knot — LB-SPLINE
pinned it as `hats.evaluate (at' 1 1) = [100, 0]` in
`LiabilityBasisV2SplineExamples`, and that witness is the citation this lane
rests the exemption on rather than restating.  At degree `≥ 2` an interior
basis function peaks at `3/4` (degree two) or `2/3` (degree three), also
pinned there, and the cap exists. -/
theorem no_cap_of_attained_scale
    (basis : Basis Result) (claim cap multiplier : Nat)
    (attained : ∃ result, entryAt (basis.evaluate result) claim = basis.scale)
    (capped : ∀ result,
      multiplier * entryAt (basis.evaluate result) claim ≤ cap * basis.scale) :
    multiplier ≤ cap := by
  obtain ⟨result, attains⟩ := attained
  have := capped result
  rw [attains] at this
  exact Nat.le_of_mul_le_mul_right (by simpa [Nat.mul_comm] using this) basis.scalePositive

/-! ## The admission rule

This is the conjunct the evaluator boundary gains.  It is stated over the
degree because that is the fact a Market selects, and it is checked in the
kernel today; the layout slice that would let a Market select a spline basis
at all does not exist yet and is out of scope by Frontier 2's own gate.
-/

/-- Highest degree exempt from the price gate.  Not a physical capacity: it is
the degree at which `no_cap_of_attained_scale` still has an instance for every
claim, so the simplex condition is still the whole no-arbitrage condition. -/
def exemptDegree : Nat := 1

/-- **The admission conjunct.**  A basis of degree `> exemptDegree` may be
evaluated for sale only alongside a certificate that the checker admits.  A
certificate supplied at an exempt degree is still checked — an input that is
present is never silently ignored. -/
def admits
    (basis : Basis Result) (degree : Nat) (certificate : Option (Certificate Result)) : Bool :=
  match certificate with
  | some certificate => certificate.check basis
  | none => decide (degree ≤ exemptDegree)

theorem admits_of_exempt
    (basis : Basis Result) (degree : Nat) (exempt : degree ≤ exemptDegree) :
    admits basis degree none = true := by
  simp [admits, exempt]

/-- **Nothing at degree `≥ 2` is admitted without a certificate.** -/
theorem not_admits_of_graded_without_certificate
    (basis : Basis Result) (degree : Nat) (graded : exemptDegree < degree) :
    admits basis degree none = false := by
  simp only [admits, decide_eq_false_iff_not]
  omega

/-- **Whatever the degree, an admitted certificate is a valid one**, so the
soundness theorem applies to every admission this rule grants. -/
theorem valid_of_admits
    (basis : Basis Result) (degree : Nat) (certificate : Certificate Result)
    (admitted : admits basis degree (some certificate) = true) :
    certificate.Valid basis :=
  (certificate.check_eq_true_iff basis).1 admitted

end DClutch.LiabilityBasisV2.PriceGate
