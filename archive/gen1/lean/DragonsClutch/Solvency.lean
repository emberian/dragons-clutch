import DragonsClutch.Basis
/-!
# P-SOLV-01: partition-of-unity maximum-liability solvency

This file proves the central theorem of
`docs/implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md` §3.2, which that document
carries as a **proof sketch**.  Everything here is about the mathematics of
liability under a partition of unity; no market state, transition, or refusal
appears until `DragonsClutch.Kernel`.

The liability functionals of §3.1 (DEF):

```text
required_active(T)      := max_i T_i
required_resolved(T, w) := ⌈ Σ_i T_i · w_i / D ⌉
```

The results, in the order the design states them:

* `P_SOLV_01_resolution_bound` — claim (i): resolution can never raise the
  requirement, for **every** admissible weight vector.
* `P_SOLV_01_sup_bound` — claim (i) over a whole basis family: the bound holds
  at every admissible value `x̂`, so it bounds the supremum.
* `P_SOLV_01_required_active_is_exact_sup` — claim (iv): the bound is attained,
  so `max_i T_i` is the exact supremum over the frozen simplex lattice and not
  a chosen over-reservation.
* `P_PAY_02_complete_set_liability_exact` — claim (ii): a complete set is worth
  exactly `q` at every admissible value, remainders never.
* `P_PAY_01_liability_fits_u128` — the arithmetic-width corollary: the same
  bound is what makes the kernel's `u128` liability accumulator unable to
  overflow.
-/

namespace DragonsClutch

/-! ## Liability functionals -/

/-- `⟨T, w⟩ = Σ_i T_i · w_i`, the *undivided* liability numerator. -/
def liability (T : List Amount) (v : PayoutVector) : Nat := dot T v.weights

/-- The Active-phase collateral requirement of a derived-basis market:
`max_i T_i` (design §3.1 DEF, §4 piece 3). -/
def requiredActive (T : List Amount) : Amount := maxOf T

/-- The Resolved-phase collateral requirement under a fixed payout vector:
`⌈ Σ_i T_i · w_i / D ⌉`.  The ceiling is the one named rounding boundary and is
a conservative reservation. -/
def requiredResolved (T : List Amount) (v : PayoutVector) : Amount :=
  ceilDiv (liability T v) v.denominator

/-! ## The pairing bound -/

/-- The one inequality the whole theorem rests on: if every supply entry is at
most `M`, the liability numerator is at most `M · Σ w`.

No length hypothesis: `dot` truncates on mismatch, which can only lower the
left-hand side. -/
theorem dot_le_bound_mul_sum {M : Nat} :
    ∀ (T w : List Nat), (∀ t ∈ T, t ≤ M) → dot T w ≤ M * w.sum
  | [], _, _ => by simp
  | _ :: _, [], _ => by simp
  | t :: ts, u :: us, h => by
      have ht : t ≤ M := h t (by simp)
      have ih := dot_le_bound_mul_sum ts us (fun x hx => h x (by simp [hx]))
      simp only [dot_cons, List.sum_cons, Nat.mul_add]
      exact Nat.add_le_add (Nat.mul_le_mul ht (Nat.le_refl u)) ih

/-- `dot` against an all-zero weight list is zero. -/
theorem dot_replicate_zero : ∀ (T : List Nat) (k : Nat), dot T (List.replicate k 0) = 0
  | [], _ => by simp
  | _ :: _, 0 => by simp
  | _ :: ts, k + 1 => by
      simp [List.replicate_succ, dot_replicate_zero ts k]

/-- A **complete set** of `q` units of every outcome pairs with any weight list
of matching length to exactly `q · Σ w`. -/
theorem dot_replicate_left (q : Nat) :
    ∀ (w : List Nat) (n : Nat), w.length = n → dot (List.replicate n q) w = q * w.sum
  | [], n, hn => by
      cases n with
      | zero => simp
      | succ k => simp at hn
  | u :: us, n, hn => by
      cases n with
      | zero => simp at hn
      | succ k =>
          have hk : us.length = k := by simpa using hn
          simp [List.replicate_succ, dot_replicate_left q us k hk, Nat.mul_add,
            Nat.mul_comm q u]

/-- The dual of the pairing bound: if every supply entry is at least `q`, the
liability numerator is at least `q · Σ w`.  Length agreement is required here —
truncation would break this direction. -/
theorem bound_mul_sum_le_dot {q : Nat} :
    ∀ (T w : List Nat), T.length = w.length → (∀ t ∈ T, q ≤ t) → q * w.sum ≤ dot T w
  | [], [], _, _ => by simp
  | [], _ :: _, h, _ => by simp at h
  | _ :: _, [], h, _ => by simp at h
  | t :: ts, u :: us, h, hq => by
      have hl : ts.length = us.length := by simpa using h
      have ht : q ≤ t := hq t (by simp)
      have ih := bound_mul_sum_le_dot ts us hl (fun x hx => hq x (by simp [hx]))
      simp only [dot_cons, List.sum_cons, Nat.mul_add]
      exact Nat.add_le_add (Nat.mul_le_mul_right u ht) ih

/-- The liability numerator under an admissible vector is at most
`(max_i T_i) · D`.  This is the substance of the design's claim (i) proof; the
ceiling step is separate and exact. -/
theorem liability_le_max_mul_denominator {T : List Amount} {v : PayoutVector} {n : Nat}
    (hv : v.Admissible n) : liability T v ≤ maxOf T * v.denominator := by
  have h := dot_le_bound_mul_sum (M := maxOf T) T v.weights (fun _ ht => mem_le_maxOf ht)
  rw [hv.pou] at h
  exact h

/-! ## Claim (i): resolution can never breach the invariant -/

/-- **P-SOLV-01 (claim i).** For every supply vector `T` and every admissible
payout vector `w` over the frozen denominator `D`:

```text
required_resolved(T, w) ≤ required_active(T)
```

Hypotheses, all explicit: `w` is admissible over `n` active outcomes — that is,
`D > 0`, `w` has `n` entries, and `Σ_i w_i = D` exactly.  Nonnegativity is
structural (`Nat`).  Nothing about knots, degree, evidence, or provenance
enters.  The supply vector `T` is arbitrary: no length agreement, no bound, no
solvency assumption.

*Proof.* `Σ_i T_i·w_i ≤ (max_j T_j)·Σ_i w_i = (max_j T_j)·D` by the pairing
bound and (H2); `⌈a/D⌉ ≤ c ↔ a ≤ c·D` for `D > 0`, and the right-hand side is
exactly that with `c = max_j T_j`. ∎ -/
theorem P_SOLV_01_resolution_bound {T : List Amount} {v : PayoutVector} {n : Nat}
    (hv : v.Admissible n) : requiredResolved T v ≤ requiredActive T :=
  (ceilDiv_le_iff hv.denom_pos).mpr (liability_le_max_mul_denominator hv)

/-- **P-SOLV-01 (claim i), basis form.** Over a whole basis family the bound
holds at *every* admissible value `x̂ ∈ X`, so `max_i T_i` bounds the supremum
of the resolved requirement over the admitted value domain. -/
theorem P_SOLV_01_sup_bound {X : Type} {n D : Nat} (B : WeightMap X n D)
    (T : List Amount) (x : X) : requiredResolved T (B.map x) ≤ requiredActive T :=
  P_SOLV_01_resolution_bound (B.admissible x)

/-! ## Claim (iv): the bound is exactly attained -/

/-- Some admissible weight vector attains the maximum: for any nonempty supply
vector and any positive denominator there is a one-hot vector at an argmax.

This is why `required_active` is a **supremum** and not merely a sound
over-reservation: over the frozen simplex lattice `{w : Σ w_i = D}` — which is
exactly the set the kernel admits, since it checks shape and not provenance
(design §3.3) — the bound is tight. -/
theorem exists_attaining_weights (D : Nat) :
    ∀ (T : List Amount), T ≠ [] →
      ∃ w : List Nat, w.length = T.length ∧ w.sum = D ∧ dot T w = maxOf T * D := by
  intro T
  induction T with
  | nil => intro h; exact absurd rfl h
  | cons x xs ih =>
      intro _
      cases xs with
      | nil => exact ⟨[D], by simp, by simp, by simp⟩
      | cons y ys =>
          rcases maxOf_cons_cases x (y :: ys) with hmax | hmax
          · refine ⟨D :: List.replicate (y :: ys).length 0, by simp, by simp, ?_⟩
            simp [dot_replicate_zero, hmax]
          · obtain ⟨w', hl, hs, hd⟩ := ih (by simp)
            exact ⟨0 :: w', by simp [hl], by simp [hs], by simp [hd, hmax]⟩

/-- **P-SOLV-01 (claim iv).** `required_active(T) = max_i T_i` is the exact
supremum of `required_resolved(T, ·)` over the frozen simplex lattice: it bounds
every admissible vector, and some admissible vector attains it.

Hypotheses: a positive frozen denominator and at least one active outcome.  The
attaining vector is one-hot, so for a *particular* basis family the supremum is
attained exactly when that family reaches a one-hot weight vector — true for
degree 0 everywhere and degree 1 at every knot, false for the interior of
degree ≥ 2, where `max_i T_i` is a sound over-reservation.  The kernel's Active
requirement must cover the whole lattice regardless, because the kernel admits
any shape-valid vector. -/
theorem P_SOLV_01_required_active_is_exact_sup {T : List Amount} {D : Nat}
    (hD : 0 < D) (hT : T ≠ []) :
    (∀ v : PayoutVector, v.Admissible T.length → v.denominator = D →
        requiredResolved T v ≤ requiredActive T) ∧
      (∃ v : PayoutVector, v.Admissible T.length ∧ v.denominator = D ∧
        requiredResolved T v = requiredActive T) := by
  refine ⟨fun v hv _ => P_SOLV_01_resolution_bound hv, ?_⟩
  obtain ⟨w, hl, hs, hd⟩ := exists_attaining_weights D T hT
  refine ⟨{ denominator := D, weights := w }, ⟨hD, hl, hs⟩, rfl, ?_⟩
  show ceilDiv (dot T w) D = maxOf T
  rw [hd, ceilDiv_mul_cancel hD]

/-! ## Claim (ii): complete-set exactness -/

/-- **P-PAY-02 (claim ii).** A complete set of `q` units of every active outcome
has liability numerator exactly `q · D` at every admissible payout vector — so
`redeem_complete_set` pays exactly `q`, and its remainder refusal is unreachable
for a validated vector.

The partition of unity does all of the work: the identity is `Σ_i q·w_i = q·D`,
which is (H2) multiplied by `q`.  It is independent of degree, knot grid, and
resolved value. -/
theorem P_PAY_02_complete_set_liability_exact {v : PayoutVector} {n q : Nat}
    (hv : v.Admissible n) : liability (List.replicate n q) v = q * v.denominator := by
  unfold liability
  rw [dot_replicate_left q v.weights n hv.arity, hv.pou]

/-- The complete-set requirement after the division: exactly `q`, with no
ceiling slack. -/
theorem P_PAY_02_complete_set_required_exact {v : PayoutVector} {n q : Nat}
    (hv : v.Admissible n) : requiredResolved (List.replicate n q) v = q := by
  unfold requiredResolved
  rw [P_PAY_02_complete_set_liability_exact hv, ceilDiv_mul_cancel hv.denom_pos]

/-! ## Arithmetic width -/

/-- `u128::MAX`, the width of the kernel's liability accumulator. -/
def u128Max : Nat := 340282366920938463463374607431768211455

theorem amountMax_sq_le_u128Max : amountMax * amountMax ≤ u128Max := by decide

/-- **P-PAY-01.** The liability numerator of any reachable state fits in `u128`:
with every supply entry and the denominator bounded by `u64::MAX`, the partition
of unity bounds `Σ_i T_i·w_i` by `(max_i T_i)·D ≤ (2^64−1)^2 < 2^128`.

This is the same inequality as claim (i), read as an arithmetic-width fact.  It
matters because the naive bound — sixteen products each of size up to
`(2^64−1)^2` — *does* exceed `u128`, so the kernel's `checked_add` accumulator
is not obviously unable to fail; the partition of unity is exactly what makes it
unable to fail.  Partial sums are bounded by the total, so no intermediate
accumulation overflows either. -/
theorem P_PAY_01_liability_fits_u128 {T : List Amount} {v : PayoutVector} {n : Nat}
    (hv : v.Admissible n) (hT : ∀ t ∈ T, t ≤ amountMax) (hD : v.denominator ≤ amountMax) :
    liability T v ≤ u128Max := by
  have h1 : liability T v ≤ maxOf T * v.denominator := liability_le_max_mul_denominator hv
  have h2 : maxOf T ≤ amountMax := maxOf_le hT
  have h3 : maxOf T * v.denominator ≤ amountMax * amountMax := Nat.mul_le_mul h2 hD
  exact Nat.le_trans h1 (Nat.le_trans h3 amountMax_sq_le_u128Max)

end DragonsClutch
