/-!
# Dragon's Clutch Lean model — arithmetic core

This file owns the exact-integer arithmetic the semantic plane is built from:
the collateral-atom scalar, the one named rounding boundary (ceiling division),
the liability pairing (`dot`), and the maximum-liability functional (`maxOf`).

Nothing here mentions a market, a token, an account, or Rust.  Every function is
total, every theorem is over unbounded `Nat`, and the fixed-width story of the
Rust kernel enters only as an explicit bound (`amountMax`) that transitions
check, never as a silent modulus.

Conventions used throughout the model:

* **Amounts are `Nat`.**  Nonnegativity is therefore structural rather than a
  checked hypothesis.  This is faithful to the Rust kernel, whose `Amount` is
  `u64`; it is *not* faithful to a hypothetical signed encoding, and the
  hypothesis "(H1) nonnegativity" of `DISTRIBUTIONAL_CLAIMS_DESIGN.md` §3.1 is
  therefore discharged by the type rather than proved.  Named, not hidden.
* **Lists are active prefixes.**  A vector over `n` active outcomes is a
  `List Nat` of length `n`.  Length agreement is an explicit hypothesis
  wherever it matters; `dot` truncates rather than failing, so every theorem
  that needs agreement says so.
-/

namespace DragonsClutch

/-- A quantity of collateral atoms, or of claims.  Opaque: no mint, decimal
system, or asset rule is attached to it, exactly as in the Rust kernel.

This is *notation* for `Nat` rather than an `abbrev` on purpose: `omega` — the
linear-arithmetic decision procedure every amount proof leans on — collects no
constraints from hypotheses whose type is a definitional alias, even a reducible
one.  Notation keeps the name in the source and `Nat` in every goal. -/
notation "Amount" => Nat

/-- The stored-amount bound of the Rust kernel (`u64::MAX`).  The model stores
`Nat` and *checks* this bound at every write, which is the modelling of
`checked_add`/`checked_sub`: refusal, never wraparound.  Intermediate
arithmetic is exact `Nat` here and `u128` there; see
`DragonsClutch.Solvency.liability_fits_u128` for the theorem that closes the
gap on the liability path. -/
def amountMax : Amount := 18446744073709551615

theorem amountMax_pos : 0 < amountMax := by decide

/-! ## Ceiling division: the one named rounding boundary -/

/-- `ceilDiv a b = ⌈a / b⌉`, with the total-function convention `ceilDiv a 0 = 0`.

This is the *only* rounding in the model.  It appears in exactly one place —
the collateral requirement — where rounding up is a conservative reservation.
Redemption never rounds: it divides exactly or refuses. -/
def ceilDiv (a b : Nat) : Nat :=
  if b = 0 then 0 else (a + b - 1) / b

@[simp] theorem ceilDiv_zero_denom (a : Nat) : ceilDiv a 0 = 0 := by
  simp [ceilDiv]

/-- The characteristic property of the rounding boundary.  Everything the model
proves about collateral requirements goes through this iff, which is why the
rounding can never be the source of an off-by-one: `⌈a/b⌉ ≤ c` is *exactly*
`a ≤ c·b`. -/
theorem ceilDiv_le_iff {a b c : Nat} (hb : 0 < b) : ceilDiv a b ≤ c ↔ a ≤ c * b := by
  have hb' : b ≠ 0 := by omega
  rw [ceilDiv, if_neg hb', Nat.div_le_iff_le_mul_add_pred hb]
  have hbc : b * c = c * b := Nat.mul_comm _ _
  omega

/-- Exactness at a multiple: no conservative slack is added when the liability
divides evenly.  Used for the tightness half of the solvency theorem. -/
theorem ceilDiv_mul_cancel {c b : Nat} (hb : 0 < b) : ceilDiv (c * b) b = c := by
  have hb' : b ≠ 0 := by omega
  have hsplit : c * b + b - 1 = (b - 1) + b * c := by
    have : c * b = b * c := Nat.mul_comm _ _
    omega
  rw [ceilDiv, if_neg hb', hsplit, Nat.add_mul_div_left _ _ hb,
    Nat.div_eq_of_lt (by omega), Nat.zero_add]

/-- Monotone in the numerator: a smaller liability never requires more
collateral. -/
theorem ceilDiv_le_ceilDiv {a a' b : Nat} (h : a ≤ a') : ceilDiv a b ≤ ceilDiv a' b := by
  rcases Nat.eq_zero_or_pos b with hb | hb
  · simp [hb]
  · have hb' : b ≠ 0 := by omega
    rw [ceilDiv, ceilDiv, if_neg hb', if_neg hb']
    exact Nat.div_le_div_right (by omega)

/-- Shifting the numerator by a whole multiple of the denominator shifts the
ceiling by exactly that multiple.  This is why a uniform supply change moves the
collateral requirement by exactly the same amount, with no rounding drift. -/
theorem ceilDiv_add_mul {a q b : Nat} (hb : 0 < b) : ceilDiv (a + q * b) b = ceilDiv a b + q := by
  have hb' : b ≠ 0 := by omega
  have hsplit : a + q * b + b - 1 = (a + b - 1) + b * q := by
    have : q * b = b * q := Nat.mul_comm _ _
    omega
  rw [ceilDiv, ceilDiv, if_neg hb', if_neg hb', hsplit, Nat.add_mul_div_left _ _ hb]

/-! ## Vectors over active outcomes -/

/-- The liability pairing `⟨T, w⟩ = Σ_i T_i · w_i`.

Truncating on length mismatch keeps the function total.  Every theorem that
depends on the sum being the *whole* pairing carries a length hypothesis. -/
def dot : List Nat → List Nat → Nat
  | [], _ => 0
  | _ :: _, [] => 0
  | x :: xs, y :: ys => x * y + dot xs ys

@[simp] theorem dot_nil_left (ys : List Nat) : dot [] ys = 0 := rfl
@[simp] theorem dot_nil_right (xs : List Nat) : dot xs [] = 0 := by
  cases xs <;> rfl
@[simp] theorem dot_cons (x : Nat) (xs : List Nat) (y : Nat) (ys : List Nat) :
    dot (x :: xs) (y :: ys) = x * y + dot xs ys := rfl

/-- The maximum-liability functional `max_i T_i`, with `max ∅ = 0`. -/
def maxOf : List Nat → Nat
  | [] => 0
  | x :: xs => Nat.max x (maxOf xs)

@[simp] theorem maxOf_nil : maxOf [] = 0 := rfl
@[simp] theorem maxOf_cons (x : Nat) (xs : List Nat) :
    maxOf (x :: xs) = Nat.max x (maxOf xs) := rfl

theorem le_maxOf_of_mem {x : Nat} : ∀ {l : List Nat}, x ∈ l → x ≤ maxOf l
  | y :: ys, h => by
      rcases List.mem_cons.mp h with h | h
      · subst h; exact Nat.le_max_left _ _
      · exact Nat.le_trans (le_maxOf_of_mem h) (Nat.le_max_right _ _)

theorem maxOf_le {l : List Nat} {M : Nat} (h : ∀ x ∈ l, x ≤ M) : maxOf l ≤ M := by
  induction l with
  | nil => simp
  | cons y ys ih =>
      simp only [maxOf_cons]
      exact Nat.max_le.mpr ⟨h y (List.mem_cons_self ..),
        ih (fun x hx => h x (List.mem_cons_of_mem _ hx))⟩

theorem maxOf_cons_cases (x : Nat) (xs : List Nat) :
    maxOf (x :: xs) = x ∨ maxOf (x :: xs) = maxOf xs := by
  simp only [maxOf_cons, Nat.max_def]
  split
  · exact Or.inr rfl
  · exact Or.inl rfl

/-- The maximum of a nonempty list of amounts is attained by one of them.  This
is what makes the Active-phase collateral requirement a *supremum* rather than
merely an upper bound (claim (iv) of the design's §3.2). -/
theorem maxOf_mem_or_zero : ∀ (l : List Nat), maxOf l = 0 ∨ maxOf l ∈ l
  | [] => Or.inl rfl
  | x :: xs => by
      rcases maxOf_cons_cases x xs with h | h
      · exact Or.inr (by rw [h]; simp)
      · rcases maxOf_mem_or_zero xs with h0 | hm
        · exact Or.inl (by rw [h, h0])
        · exact Or.inr (by rw [h]; exact List.mem_cons_of_mem _ hm)

/-- Every entry of a list of amounts is bounded by the list's sum.  This is the
lemma that makes the design's (H1) upper bound `w_i ≤ D` a *consequence* of
(H2) `Σ w_i = D` over unsigned amounts rather than an independent hypothesis. -/
theorem mem_le_sum {x : Nat} : ∀ {l : List Nat}, x ∈ l → x ≤ l.sum
  | y :: ys, h => by
      rcases List.mem_cons.mp h with h | h
      · subst h; simp
      · have := mem_le_sum h
        simp only [List.sum_cons]
        omega

/-- The sum of a constant list. -/
theorem sum_replicate (n q : Nat) : (List.replicate n q).sum = n * q := by
  induction n with
  | zero => simp
  | succ k ih => simp [List.replicate_succ, ih, Nat.succ_mul]; omega

/-- Every entry is bounded by the maximum. -/
theorem mem_le_maxOf {l : List Nat} {x : Nat} (h : x ∈ l) : x ≤ maxOf l :=
  le_maxOf_of_mem h


/-! ## Uniform shifts

`split` and `merge` move every outcome by the same quantity.  These are the two
list operations they use, with the exact effect each has on the two functionals
the solvency invariant is built from. -/

/-- Add `q` to every entry (the `split` supply and balance update). -/
def bumpAll (q : Nat) (xs : List Nat) : List Nat := xs.map (· + q)

/-- Subtract `q` from every entry (the `merge` and complete-set update). -/
def dropAll (q : Nat) (xs : List Nat) : List Nat := xs.map (· - q)

@[simp] theorem length_bumpAll (q : Nat) (xs : List Nat) : (bumpAll q xs).length = xs.length := by
  simp [bumpAll]

@[simp] theorem length_dropAll (q : Nat) (xs : List Nat) : (dropAll q xs).length = xs.length := by
  simp [dropAll]

@[simp] theorem bumpAll_nil (q : Nat) : bumpAll q [] = [] := rfl
@[simp] theorem dropAll_nil (q : Nat) : dropAll q [] = [] := rfl
@[simp] theorem bumpAll_cons (q x : Nat) (xs : List Nat) :
    bumpAll q (x :: xs) = (x + q) :: bumpAll q xs := rfl
@[simp] theorem dropAll_cons (q x : Nat) (xs : List Nat) :
    dropAll q (x :: xs) = (x - q) :: dropAll q xs := rfl

theorem mem_bumpAll {q y : Nat} {xs : List Nat} (h : y ∈ bumpAll q xs) :
    ∃ x ∈ xs, y = x + q := by
  obtain ⟨x, hx, hy⟩ := List.mem_map.mp h
  exact ⟨x, hx, hy.symm⟩

theorem mem_dropAll {q y : Nat} {xs : List Nat} (h : y ∈ dropAll q xs) :
    ∃ x ∈ xs, y = x - q := by
  obtain ⟨x, hx, hy⟩ := List.mem_map.mp h
  exact ⟨x, hx, hy.symm⟩

/-- A uniform increase raises the liability numerator by exactly `q · Σ w`. -/
theorem dot_bumpAll (q : Nat) :
    ∀ (T w : List Nat), T.length = w.length → dot (bumpAll q T) w = dot T w + q * w.sum
  | [], [], _ => by simp
  | [], _ :: _, h => by simp at h
  | _ :: _, [], h => by simp at h
  | t :: ts, u :: us, h => by
      have hl : ts.length = us.length := by simpa using h
      simp only [bumpAll_cons, dot_cons, List.sum_cons, dot_bumpAll q ts us hl,
        Nat.add_mul, Nat.mul_add]
      omega

/-- A uniform decrease lowers it by exactly `q · Σ w`, stated additively so that
truncated subtraction never enters a proof. -/
theorem dot_dropAll (q : Nat) :
    ∀ (T w : List Nat), T.length = w.length → (∀ t ∈ T, q ≤ t) →
      dot (dropAll q T) w + q * w.sum = dot T w
  | [], [], _, _ => by simp
  | [], _ :: _, h, _ => by simp at h
  | _ :: _, [], h, _ => by simp at h
  | t :: ts, u :: us, h, hq => by
      have hl : ts.length = us.length := by simpa using h
      have ht : q ≤ t := hq t (by simp)
      have ih := dot_dropAll q ts us hl (fun x hx => hq x (by simp [hx]))
      simp only [dropAll_cons, dot_cons, List.sum_cons, Nat.mul_add]
      have hsub : (t - q) * u + q * u = t * u := by
        have : (t - q) * u + q * u = ((t - q) + q) * u := by rw [Nat.add_mul]
        rw [this]
        congr 1
        omega
      omega

/-- The maximum of a nonempty list is one of its entries. -/
theorem maxOf_mem : ∀ (l : List Nat), l ≠ [] → maxOf l ∈ l := by
  intro l
  induction l with
  | nil => intro h; exact absurd rfl h
  | cons x xs ih =>
      intro _
      cases xs with
      | nil => simp [maxOf]
      | cons y ys =>
          rcases maxOf_cons_cases x (y :: ys) with h | h
          · rw [h]; simp
          · rw [h]; exact List.mem_cons_of_mem _ (ih (by simp))

/-- A uniform increase raises the maximum by exactly `q` (nonempty). -/
theorem maxOf_bumpAll (q : Nat) (xs : List Nat) (hne : xs ≠ []) :
    maxOf (bumpAll q xs) = maxOf xs + q := by
  refine Nat.le_antisymm ?_ ?_
  · refine maxOf_le ?_
    intro y hy
    obtain ⟨x, hx, rfl⟩ := mem_bumpAll hy
    exact Nat.add_le_add_right (le_maxOf_of_mem hx) q
  · have hmem : maxOf xs ∈ xs := maxOf_mem xs hne
    have : maxOf xs + q ∈ bumpAll q xs := List.mem_map.mpr ⟨maxOf xs, hmem, rfl⟩
    exact le_maxOf_of_mem this

/-- A uniform decrease lowers the maximum by exactly `q` (nonempty, and every
entry at least `q`), stated additively. -/
theorem maxOf_dropAll (q : Nat) (xs : List Nat) (hne : xs ≠ []) (hq : ∀ x ∈ xs, q ≤ x) :
    maxOf (dropAll q xs) + q = maxOf xs := by
  have hmem : maxOf xs ∈ xs := maxOf_mem xs hne
  have hqmax : q ≤ maxOf xs := hq _ hmem
  refine Nat.le_antisymm ?_ ?_
  · have hb : maxOf (dropAll q xs) ≤ maxOf xs - q := by
      refine maxOf_le ?_
      intro y hy
      obtain ⟨x, hx, rfl⟩ := mem_dropAll hy
      have := le_maxOf_of_mem hx
      omega
    omega
  · have : maxOf xs - q ∈ dropAll q xs := List.mem_map.mpr ⟨maxOf xs, hmem, rfl⟩
    have := le_maxOf_of_mem this
    omega

/-- The maximum of a shifted list of values, used for the finite-preset arm of
the Active requirement: `max_j (f_j + q) = (max_j f_j) + q`. -/
theorem maxOf_map_add (q : Nat) :
    ∀ (xs : List Nat), xs ≠ [] → maxOf (xs.map (· + q)) = maxOf xs + q :=
  maxOf_bumpAll q

end DragonsClutch
