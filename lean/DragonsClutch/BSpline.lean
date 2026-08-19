import DragonsClutch.Solvency
/-!
# Exact low-degree clamped B-spline constructions

`Basis.lean` deliberately treats a basis as an abstract admissible weight map.
This module closes the next link: it constructs exact nonnegative rational
weights, integerizes them, and manufactures `PayoutVector.Admissible` rather
than accepting admissibility as a caller hypothesis.

There are two levels, with the boundary explicit:

* `clampedDegreeOne/Two/Three` are the exact Bernstein weights on one span.
  These are precisely the open-clamped B-splines for the knot vector whose two
  endpoints each have multiplicity `degree + 1`.
* `refineOne/Two/Three` are the exact Piegl--Tiller `BasisFuns` column steps.
  They cover arbitrary open-clamped panes once the evaluator supplies the
  positive left/right knot distances.  Their partition-of-unity theorem does
  not assume the conclusion: it follows by expanding each old coefficient
  into two nonnegative adjacent contributions with a common denominator.

The connection from a stored knot vector to those positive distance records is
an integration theorem still owed by the executable evaluator.  This file does
not pretend that a caller-supplied valid `Split` proves the knot indexing.
-/

namespace DragonsClutch

/-! ## Exact rational vectors -/

/-- Nonnegative rational weights over one positive common denominator.

`numerators` are `Nat`, so nonnegativity is structural.  No reduction or
coprimality is required: this is a common-denominator semantic value, not a
codec. -/
structure RationalBasis where
  denominator : Nat
  numerators : List Nat
deriving Repr, DecidableEq, Inhabited

namespace RationalBasis

/-- Exact rational partition of unity. -/
def Exact (r : RationalBasis) : Prop :=
  0 < r.denominator ∧ r.numerators.sum = r.denominator

instance (r : RationalBasis) : Decidable r.Exact := by
  unfold Exact
  infer_instance

/-- Number of nonzero rational coefficients. -/
def supportSize (r : RationalBasis) : Nat :=
  r.numerators.countP (fun n => n != 0)

/-- Every rational numerator is bounded by the common denominator. -/
theorem Exact.bounded {r : RationalBasis} (h : r.Exact) :
    ∀ n ∈ r.numerators, n ≤ r.denominator := by
  intro n hn
  have := mem_le_sum hn
  rw [h.2] at this
  exact this

/-- Embed one local basis block into an arbitrary global outcome vector.
The zeros make the locality claim positional, not merely a support count. -/
def pad (left right : Nat) (r : RationalBasis) : RationalBasis :=
  { denominator := r.denominator
    numerators := List.replicate left 0 ++ r.numerators ++ List.replicate right 0 }

@[simp] theorem pad_denominator (left right : Nat) (r : RationalBasis) :
    (pad left right r).denominator = r.denominator := rfl

@[simp] theorem pad_length (left right : Nat) (r : RationalBasis) :
    (pad left right r).numerators.length = left + r.numerators.length + right := by
  simp [pad, Nat.add_assoc]

@[simp] theorem pad_sum (left right : Nat) (r : RationalBasis) :
    (pad left right r).numerators.sum = r.numerators.sum := by
  simp [pad, List.sum_append]

@[simp] theorem pad_supportSize (left right : Nat) (r : RationalBasis) :
    (pad left right r).supportSize = r.supportSize := by
  have zeroes : ∀ n, List.countP (fun x : Nat => x != 0) (List.replicate n 0) = 0 := by
    intro n
    induction n with
    | zero => simp
    | succ k ih => simp [List.replicate_succ, ih]
  simp [pad, supportSize, List.countP_append, zeroes]

theorem Exact.pad {r : RationalBasis} (h : r.Exact) (left right : Nat) :
    (r.pad left right).Exact := by
  exact ⟨by simpa using h.1, by simpa using h.2⟩

end RationalBasis

/-! ## One-span open-clamped B-splines, degrees one through three -/

/-- Degree-one open-clamped weights on a span of width `h`, at offset `u`.
Exact value: `[(h-u)/h, u/h]`. -/
def clampedDegreeOne (h u : Nat) : RationalBasis :=
  { denominator := h
    numerators := [h - u, u] }

/-- Degree-two open-clamped weights on one span (the quadratic Bernstein
basis).  Coefficients are written as repeated products rather than numeric
binomial coefficients so the dependency-free proof can normalize by
associativity/commutativity alone. -/
def clampedDegreeTwo (h u : Nat) : RationalBasis :=
  let v := h - u
  { denominator := h * h
    numerators := [v * v, u * v + u * v, u * u] }

/-- Degree-three open-clamped weights on one span (the cubic Bernstein basis). -/
def clampedDegreeThree (h u : Nat) : RationalBasis :=
  let v := h - u
  { denominator := h * h * h
    numerators :=
      [ v * v * v,
        u * v * v + u * v * v + u * v * v,
        u * u * v + u * u * v + u * u * v,
        u * u * u ] }

@[simp] theorem clampedDegreeOne_length (h u : Nat) :
    (clampedDegreeOne h u).numerators.length = 2 := by
  simp [clampedDegreeOne]

@[simp] theorem clampedDegreeTwo_length (h u : Nat) :
    (clampedDegreeTwo h u).numerators.length = 3 := by
  simp [clampedDegreeTwo]

@[simp] theorem clampedDegreeThree_length (h u : Nat) :
    (clampedDegreeThree h u).numerators.length = 4 := by
  simp [clampedDegreeThree]

/-- Exact rational partition of unity for every point of a positive linear
span, including both closed endpoints. -/
theorem clampedDegreeOne_exact {h u : Nat} (hh : 0 < h) (hu : u ≤ h) :
    (clampedDegreeOne h u).Exact := by
  refine ⟨hh, ?_⟩
  simp [clampedDegreeOne]
  omega

/-- Exact rational partition of unity for the quadratic clamped construction. -/
theorem clampedDegreeTwo_exact {h u : Nat} (hh : 0 < h) (hu : u ≤ h) :
    (clampedDegreeTwo h u).Exact := by
  refine ⟨Nat.mul_pos hh hh, ?_⟩
  have hv : h - u + u = h := Nat.sub_add_cancel hu
  have hsq : h * h = (h - u + u) * (h - u + u) := by rw [hv]
  simp only [clampedDegreeTwo, List.sum_cons, List.sum_nil, Nat.add_zero]
  rw [hsq]
  simp only [Nat.add_mul, Nat.mul_add]
  ac_rfl

/-- Exact rational partition of unity for the cubic clamped construction. -/
theorem clampedDegreeThree_exact {h u : Nat} (hh : 0 < h) (hu : u ≤ h) :
    (clampedDegreeThree h u).Exact := by
  have hh2 : 0 < h * h := Nat.mul_pos hh hh
  refine ⟨Nat.mul_pos hh2 hh, ?_⟩
  have hv : h - u + u = h := Nat.sub_add_cancel hu
  have hcube : h * h * h = (h - u + u) * (h - u + u) * (h - u + u) := by
    rw [hv]
  simp only [clampedDegreeThree, List.sum_cons, List.sum_nil, Nat.add_zero]
  rw [hcube]
  simp only [Nat.add_mul, Nat.mul_add]
  ac_rfl

/-- Degree-one local support is at most two adjacent coefficients. -/
theorem clampedDegreeOne_local_support (h u : Nat) :
    (clampedDegreeOne h u).supportSize ≤ 2 := by
  exact Nat.le_trans List.countP_le_length (by simp)

/-- Degree-two local support is at most three adjacent coefficients. -/
theorem clampedDegreeTwo_local_support (h u : Nat) :
    (clampedDegreeTwo h u).supportSize ≤ 3 := by
  exact Nat.le_trans List.countP_le_length (by simp)

/-- Degree-three local support is at most four adjacent coefficients. -/
theorem clampedDegreeThree_local_support (h u : Nat) :
    (clampedDegreeThree h u).supportSize ≤ 4 := by
  exact Nat.le_trans List.countP_le_length (by simp)

/-- The closed low edge is exactly one-hot in degree one. -/
theorem clampedDegreeOne_closed_left (h : Nat) :
    (clampedDegreeOne h 0).numerators = [h, 0] := by
  simp [clampedDegreeOne]

/-- The closed high edge is exactly one-hot in degree one. -/
theorem clampedDegreeOne_closed_right (h : Nat) :
    (clampedDegreeOne h h).numerators = [0, h] := by
  simp [clampedDegreeOne]

/-- The closed low edge is exactly one-hot in degree two. -/
theorem clampedDegreeTwo_closed_left (h : Nat) :
    (clampedDegreeTwo h 0).numerators = [h * h, 0, 0] := by
  simp [clampedDegreeTwo]

/-- The closed high edge is exactly one-hot in degree two. -/
theorem clampedDegreeTwo_closed_right (h : Nat) :
    (clampedDegreeTwo h h).numerators = [0, 0, h * h] := by
  simp [clampedDegreeTwo]

/-- The closed low edge is exactly one-hot in degree three. -/
theorem clampedDegreeThree_closed_left (h : Nat) :
    (clampedDegreeThree h 0).numerators = [h * h * h, 0, 0, 0] := by
  simp [clampedDegreeThree]

/-- The closed high edge is exactly one-hot in degree three. -/
theorem clampedDegreeThree_closed_right (h : Nat) :
    (clampedDegreeThree h h).numerators = [0, 0, 0, h * h * h] := by
  simp [clampedDegreeThree]

/-! ## Open-clamped endpoints at arbitrary arity

The executable evaluator handles the two closed endpoints before entering
`BasisFuns`.  The functions below model those branches for an arbitrary number
of claims.  This matters for multi-pane grids: the first and last panes are not
silently replaced by an interior cardinal polynomial. -/

/-- The exact rational vector returned at or below the closed low endpoint. -/
def openClampedLeft (n denominator : Nat) : RationalBasis :=
  { denominator
    numerators := denominator :: List.replicate (n - 1) 0 }

/-- The exact rational vector returned at or above the closed high endpoint. -/
def openClampedRight (n denominator : Nat) : RationalBasis :=
  { denominator
    numerators := List.replicate (n - 1) 0 ++ [denominator] }

theorem openClampedLeft_exact {n denominator : Nat} (_hn : 0 < n)
    (hd : 0 < denominator) : (openClampedLeft n denominator).Exact := by
  refine ⟨hd, ?_⟩
  simp [openClampedLeft]

theorem openClampedRight_exact {n denominator : Nat} (_hn : 0 < n)
    (hd : 0 < denominator) : (openClampedRight n denominator).Exact := by
  refine ⟨hd, ?_⟩
  simp [openClampedRight]

@[simp] theorem openClampedLeft_length {n denominator : Nat} (hn : 0 < n) :
    (openClampedLeft n denominator).numerators.length = n := by
  simp [openClampedLeft]
  omega

@[simp] theorem openClampedRight_length {n denominator : Nat} (hn : 0 < n) :
    (openClampedRight n denominator).numerators.length = n := by
  simp [openClampedRight]
  omega

theorem openClampedLeft_local_support (n denominator : Nat) :
    (openClampedLeft n denominator).supportSize ≤ 1 := by
  have zeros : ∀ k, List.countP (fun x : Nat => x != 0)
      (List.replicate k 0) = 0 := by
    intro k
    induction k with
    | zero => simp
    | succ k ih => simp [List.replicate_succ, ih]
  unfold openClampedLeft RationalBasis.supportSize
  rw [List.countP_cons, zeros]
  split <;> simp_all

theorem openClampedRight_local_support (n denominator : Nat) :
    (openClampedRight n denominator).supportSize ≤ 1 := by
  have zeros : ∀ k, List.countP (fun x : Nat => x != 0)
      (List.replicate k 0) = 0 := by
    intro k
    induction k with
    | zero => simp
    | succ k ih => simp [List.replicate_succ, ih]
  unfold openClampedRight RationalBasis.supportSize
  rw [List.countP_append, zeros]
  rw [List.countP_cons]
  split <;> simp_all

theorem clampedDegreeOne_left_is_open (h : Nat) :
    clampedDegreeOne h 0 = openClampedLeft 2 h := by
  simp [clampedDegreeOne, openClampedLeft]

theorem clampedDegreeOne_right_is_open (h : Nat) :
    clampedDegreeOne h h = openClampedRight 2 h := by
  simp [clampedDegreeOne, openClampedRight]

theorem clampedDegreeTwo_left_is_open (h : Nat) :
    clampedDegreeTwo h 0 = openClampedLeft 3 (h * h) := by
  simp [clampedDegreeTwo, openClampedLeft]

theorem clampedDegreeTwo_right_is_open (h : Nat) :
    clampedDegreeTwo h h = openClampedRight 3 (h * h) := by
  simp [clampedDegreeTwo, openClampedRight]

theorem clampedDegreeThree_left_is_open (h : Nat) :
    clampedDegreeThree h 0 = openClampedLeft 4 (h * h * h) := by
  simp [clampedDegreeThree, openClampedLeft]

theorem clampedDegreeThree_right_is_open (h : Nat) :
    clampedDegreeThree h h = openClampedRight 4 (h * h * h) := by
  simp [clampedDegreeThree, openClampedRight]

/-! ## Exact `BasisFuns` refinement columns

`Split.low` is the distance from the left knot and `Split.high` the distance
to the right knot.  The old coefficient is divided into a low-index
contribution `high/(low+high)` and a high-index contribution
`low/(low+high)`.  Positivity is precisely the nonzero-divisor obligation of
the executable recurrence. -/

structure Split where
  low : Nat
  high : Nat
  positive : 0 < low + high
deriving Repr

namespace Split

def denominator (s : Split) : Nat := s.low + s.high

theorem denominator_pos (s : Split) : 0 < s.denominator := s.positive

end Split

/-- First `BasisFuns` column, from one coefficient to two. -/
def refineOne (q : Nat) (s : Split) : RationalBasis :=
  { denominator := q * s.denominator
    numerators := [q * s.high, q * s.low] }

/-- Second `BasisFuns` column.  Inputs `a,b` share denominator `q`; the output
uses the product of the two split denominators as a common denominator. -/
def refineTwo (q a b : Nat) (s0 s1 : Split) : RationalBasis :=
  let z0 := s0.denominator
  let z1 := s1.denominator
  { denominator := q * z0 * z1
    numerators :=
      [ a * s0.high * z1,
        a * s0.low * z1 + b * s1.high * z0,
        b * s1.low * z0 ] }

/-- Third `BasisFuns` column. -/
def refineThree (q a b c : Nat) (s0 s1 s2 : Split) : RationalBasis :=
  let z0 := s0.denominator
  let z1 := s1.denominator
  let z2 := s2.denominator
  { denominator := q * z0 * z1 * z2
    numerators :=
      [ a * s0.high * z1 * z2,
        a * s0.low * z1 * z2 + b * s1.high * z0 * z2,
        b * s1.low * z0 * z2 + c * s2.high * z0 * z1,
        c * s2.low * z0 * z1 ] }

theorem refineOne_exact {q : Nat} (hq : 0 < q) (s : Split) :
    (refineOne q s).Exact := by
  refine ⟨Nat.mul_pos hq s.denominator_pos, ?_⟩
  simp only [refineOne, List.sum_cons, List.sum_nil, Nat.add_zero, Split.denominator]
  rw [Nat.mul_add]
  ac_rfl

theorem refineTwo_exact {q a b : Nat} (hq : 0 < q) (hab : a + b = q)
    (s0 s1 : Split) : (refineTwo q a b s0 s1).Exact := by
  have hz0 := s0.denominator_pos
  have hz1 := s1.denominator_pos
  refine ⟨Nat.mul_pos (Nat.mul_pos hq hz0) hz1, ?_⟩
  simp only [refineTwo, List.sum_cons, List.sum_nil, Nat.add_zero]
  rw [← hab]
  simp only [Split.denominator, Nat.add_mul, Nat.mul_add]
  ac_rfl

theorem refineThree_exact {q a b c : Nat} (hq : 0 < q) (habc : a + b + c = q)
    (s0 s1 s2 : Split) : (refineThree q a b c s0 s1 s2).Exact := by
  have hz0 := s0.denominator_pos
  have hz1 := s1.denominator_pos
  have hz2 := s2.denominator_pos
  refine ⟨Nat.mul_pos (Nat.mul_pos (Nat.mul_pos hq hz0) hz1) hz2, ?_⟩
  simp only [refineThree, List.sum_cons, List.sum_nil, Nat.add_zero]
  rw [← habc]
  simp only [Split.denominator, Nat.add_mul, Nat.mul_add]
  ac_rfl

theorem refineOne_local_support (q : Nat) (s : Split) :
    (refineOne q s).supportSize ≤ 2 := by
  have h := List.countP_le_length
    (p := fun n : Nat => n != 0) (l := (refineOne q s).numerators)
  simpa [refineOne, RationalBasis.supportSize] using h

theorem refineTwo_local_support (q a b : Nat) (s0 s1 : Split) :
    (refineTwo q a b s0 s1).supportSize ≤ 3 := by
  have h := List.countP_le_length
    (p := fun n : Nat => n != 0) (l := (refineTwo q a b s0 s1).numerators)
  simpa [refineTwo, RationalBasis.supportSize] using h

theorem refineThree_local_support (q a b c : Nat) (s0 s1 s2 : Split) :
    (refineThree q a b c s0 s1 s2).supportSize ≤ 4 := by
  have h := List.countP_le_length
    (p := fun n : Nat => n != 0) (l := (refineThree q a b c s0 s1 s2).numerators)
  simpa [refineThree, RationalBasis.supportSize] using h

/-! ## Explicit knot-vector-to-`Split` linkage

The Rust `BasisFuns` loop at column `j`, row `r` uses

```text
left  = x - U[span + 1 - j + r]
right = U[span + r + 1] - x
```

and divides by their sum. `BasisFunsCell` records precisely the three checked
facts which make those unsigned subtractions and that division valid.  The
column theorems below are generic in `span`, hence apply to every pane, not only
to the one-pane Bernstein special case.

The remaining correspondence residue is deliberately named
`RustExpandedKnotLinkage`: prove that expansion of every admitted stored,
strictly increasing distinct-knot vector (endpoint multiplicity `degree + 1`)
produces the required cells for every reached `(span,column,row)`, and that the
Rust recurrence's reduced `Fraction` values denote the common-denominator
numerators here.  This file proves the mathematical column transformation once
those indexing facts are supplied; it does not claim the Rust array adapter is
verified.
-/

def basisFunsLeftIndex (span column row : Nat) : Nat :=
  span + 1 - column + row

def basisFunsRightIndex (span row : Nat) : Nat :=
  span + row + 1

/-- Checked knot bracketing for one row of one `BasisFuns` column. -/
structure BasisFunsCell (U : Nat → Nat) (span column row x : Nat) : Prop where
  left_le : U (basisFunsLeftIndex span column row) ≤ x
  le_right : x ≤ U (basisFunsRightIndex span row)
  distinct : U (basisFunsLeftIndex span column row) <
    U (basisFunsRightIndex span row)

/-- Convert Rust's two checked knot distances into the abstract refinement
split used above. -/
def BasisFunsCell.toSplit {U : Nat → Nat} {span column row x : Nat}
    (cell : BasisFunsCell U span column row x) : Split :=
  { low := x - U (basisFunsLeftIndex span column row)
    high := U (basisFunsRightIndex span row) - x
    positive := by
      have hsum := Nat.sub_add_sub_cancel cell.le_right cell.left_le
      have hpos := Nat.sub_pos_of_lt cell.distinct
      rw [← hsum] at hpos
      simpa [Nat.add_comm] using hpos }

@[simp] theorem BasisFunsCell.toSplit_low {U : Nat → Nat}
    {span column row x : Nat} (cell : BasisFunsCell U span column row x) :
    cell.toSplit.low = x - U (basisFunsLeftIndex span column row) := rfl

@[simp] theorem BasisFunsCell.toSplit_high {U : Nat → Nat}
    {span column row x : Nat} (cell : BasisFunsCell U span column row x) :
    cell.toSplit.high = U (basisFunsRightIndex span row) - x := rfl

theorem BasisFunsCell.toSplit_denominator {U : Nat → Nat}
    {span column row x : Nat} (cell : BasisFunsCell U span column row x) :
    cell.toSplit.denominator =
      U (basisFunsRightIndex span row) - U (basisFunsLeftIndex span column row) := by
  have hsum := Nat.sub_add_sub_cancel cell.le_right cell.left_le
  simpa [BasisFunsCell.toSplit, Split.denominator, Nat.add_comm] using hsum

/-- Exact first recurrence column on any pane. -/
theorem basisFuns_columnOne_exact {U : Nat → Nat} {span x q : Nat}
    (hq : 0 < q) (cell : BasisFunsCell U span 1 0 x) :
    (refineOne q cell.toSplit).Exact :=
  refineOne_exact hq cell.toSplit

/-- Exact second recurrence column on any pane. -/
theorem basisFuns_columnTwo_exact {U : Nat → Nat} {span x q a b : Nat}
    (hq : 0 < q) (hab : a + b = q)
    (cell0 : BasisFunsCell U span 2 0 x)
    (cell1 : BasisFunsCell U span 2 1 x) :
    (refineTwo q a b cell0.toSplit cell1.toSplit).Exact :=
  refineTwo_exact hq hab cell0.toSplit cell1.toSplit

/-- Exact third recurrence column on any pane. -/
theorem basisFuns_columnThree_exact {U : Nat → Nat} {span x q a b c : Nat}
    (hq : 0 < q) (habc : a + b + c = q)
    (cell0 : BasisFunsCell U span 3 0 x)
    (cell1 : BasisFunsCell U span 3 1 x)
    (cell2 : BasisFunsCell U span 3 2 x) :
    (refineThree q a b c cell0.toSplit cell1.toSplit cell2.toSplit).Exact :=
  refineThree_exact hq habc cell0.toSplit cell1.toSplit cell2.toSplit

/-- Lean transcription of Rust's `expanded_knots`: repeat the first and last
distinct stored knots `degree + 1` times and retain each interior knot once. -/
def expandOpenClamped (degree : Nat) : List Nat → List Nat
  | [] => []
  | first :: rest =>
      List.replicate (degree + 1) first ++ rest.dropLast ++
        List.replicate (degree + 1) (rest.getLastD first)

/-- Rust's exact cursor equation `expanded_len = knot_count + 2*degree`. -/
theorem expandOpenClamped_length {stored : List Nat} {degree : Nat}
    (hstored : 2 ≤ stored.length) :
    (expandOpenClamped degree stored).length = stored.length + 2 * degree := by
  cases stored with
  | nil => simp at hstored
  | cons first rest =>
      simp only [List.length_cons] at hstored
      simp only [expandOpenClamped, List.length_append, List.length_replicate,
        List.length_dropLast, List.length_cons]
      omega

def expandedKnotAt (stored : List Nat) (degree index : Nat) : Nat :=
  (expandOpenClamped degree stored).getD index 0

/-- Exact statement still owed at the Rust/Lean adapter boundary.  It is a
named proposition about the actual expanded list above, not an assumed
theorem. `pane` is Rust's distinct-knot pane and `span = degree + pane`. -/
def RustExpandedKnotLinkage (stored : List Nat) (degree : Nat) : Prop :=
  1 ≤ degree ∧ degree ≤ 3 ∧ 2 ≤ stored.length ∧
  (∀ i, i + 1 < stored.length → stored.getD i 0 < stored.getD (i + 1) 0) ∧
  (∀ pane x,
    pane + 1 < stored.length →
    stored.getD pane 0 ≤ x → x ≤ stored.getD (pane + 1) 0 →
    ∀ column row, 1 ≤ column → column ≤ degree → row < column →
      BasisFunsCell (expandedKnotAt stored degree) (degree + pane) column row x)

/-! ## Deterministic largest-remainder `WEIGHT-ROUND-01`

The executable evaluator floors every exact scaled coefficient and awards the
residual atoms to the largest exact fractional remainders. Equal remainders are
ordered by the lowest global outcome index.  The previous fixed-terminal
residual definitions are intentionally absent: they describe neither the Rust
kernel nor its accuracy policy.

`LargestRemainderSelection` is the exact finite semantic certificate for that
deterministic subset.  It records the cardinality, nonzero-remainder rule,
largest-first rule, and low-index tie break.  `quantizeLargest` then proves the
load-bearing fact: the result is an admissible exact integer partition of
unity.  Construction of this certificate by the Rust selection loop is part of
the unverified adapter residue named above; no axiom imports it into Lean.
-/

/-- Floor error is always in `[0, denominator)`. -/
theorem floor_error_bounds (a denominator : Nat) (hd : 0 < denominator) :
    a / denominator * denominator ≤ a ∧
      a < (a / denominator + 1) * denominator := by
  constructor
  · exact Nat.div_mul_le_self _ _
  · have heq : a / denominator * denominator + a % denominator = a := by
      calc
        a / denominator * denominator + a % denominator =
            denominator * (a / denominator) + a % denominator := by ac_rfl
        _ = a := Nat.div_add_mod a denominator
    have hm := Nat.mod_lt a hd
    simp only [Nat.add_mul, Nat.one_mul]
    omega

/-- Coordinatewise floors of an exact rational vector after scaling by `D`. -/
def floorWeights (D denominator : Nat) (numerators : List Nat) : List Nat :=
  numerators.map (fun n => D * n / denominator)

/-- Exact division remainders paired with `floorWeights`. -/
def floorRemainders (D denominator : Nat) (numerators : List Nat) : List Nat :=
  numerators.map (fun n => D * n % denominator)

@[simp] theorem floorWeights_length (D denominator : Nat) (numerators : List Nat) :
    (floorWeights D denominator numerators).length = numerators.length := by
  simp [floorWeights]

@[simp] theorem floorRemainders_length (D denominator : Nat) (numerators : List Nat) :
    (floorRemainders D denominator numerators).length = numerators.length := by
  simp [floorRemainders]

/-- Quotient/remainder decomposition summed over a whole common-denominator
vector.  This is the accounting identity behind every residual bound. -/
theorem floor_decomposition (D denominator : Nat) : ∀ numerators : List Nat,
    (floorWeights D denominator numerators).sum * denominator +
        (floorRemainders D denominator numerators).sum = D * numerators.sum
  | [] => by simp [floorWeights, floorRemainders]
  | n :: ns => by
      have ih := floor_decomposition D denominator ns
      simp only [floorWeights, floorRemainders] at ih
      have hn : D * n / denominator * denominator + D * n % denominator = D * n := by
        calc
          D * n / denominator * denominator + D * n % denominator =
              denominator * (D * n / denominator) + D * n % denominator := by ac_rfl
          _ = D * n := Nat.div_add_mod _ _
      simp only [floorWeights, floorRemainders, List.map_cons, List.sum_cons,
        Nat.add_mul, Nat.mul_add]
      omega

/-- Each remainder is at most `denominator - 1`, hence their sum is bounded
without any probabilistic or spline hypothesis. -/
theorem floorRemainders_sum_le (D : Nat) {denominator : Nat} (hd : 0 < denominator) :
    ∀ numerators : List Nat,
    (floorRemainders D denominator numerators).sum ≤
      numerators.length * (denominator - 1)
  | [] => by simp [floorRemainders]
  | n :: ns => by
      have ih := floorRemainders_sum_le D hd ns
      have hr : D * n % denominator ≤ denominator - 1 := by
        have := Nat.mod_lt (D * n) hd
        omega
      simp only [floorRemainders] at ih ⊢
      simp only [List.map_cons, List.sum_cons, List.length_cons, Nat.succ_mul]
      omega

/-- Independent flooring never exceeds `D` when the exact rational
coefficients form a partition of unity. -/
theorem floorWeights_sum_le {D denominator : Nat} {numerators : List Nat}
    (hd : 0 < denominator) (hexact : numerators.sum = denominator) :
    (floorWeights D denominator numerators).sum ≤ D := by
  have hdec := floor_decomposition D denominator numerators
  rw [hexact] at hdec
  apply Nat.le_of_mul_le_mul_right (c := denominator) ?_ hd
  omega

/-- The number of residual atoms is strictly smaller than the number of local
coefficients.  In degrees one through three this is respectively at most
`1`, `2`, or `3`. -/
theorem weightRoundResidual_lt_length {D denominator : Nat} {numerators : List Nat}
    (hd : 0 < denominator) (hne : numerators ≠ [])
    (hexact : numerators.sum = denominator) :
    D - (floorWeights D denominator numerators).sum < numerators.length := by
  let floors := (floorWeights D denominator numerators).sum
  let remainders := (floorRemainders D denominator numerators).sum
  have hf : floors ≤ D := floorWeights_sum_le hd hexact
  have hdec := floor_decomposition D denominator numerators
  rw [hexact] at hdec
  have hdec' : floors * denominator + remainders = D * denominator := by
    dsimp [floors, remainders]
    exact hdec
  have hbudget : (D - floors) * denominator + floors * denominator =
      D * denominator := by
    rw [← Nat.add_mul]
    congr 1
    omega
  have hrem : (D - floors) * denominator = remainders := by
    omega
  have hlen : 0 < numerators.length := by
    cases numerators with
    | nil => exact absurd rfl hne
    | cons n ns => simp
  have hremBound := floorRemainders_sum_le D hd numerators
  have hstrict : remainders < numerators.length * denominator := by
    have hstep : numerators.length * (denominator - 1) <
        numerators.length * denominator := by
      exact (Nat.mul_lt_mul_left hlen).mpr (by omega)
    exact Nat.lt_of_le_of_lt hremBound hstep
  have hmul : (D - floors) * denominator < numerators.length * denominator := by
    rw [hrem]
    exact hstrict
  exact Nat.lt_of_mul_lt_mul_right hmul

def boolNat (value : Bool) : Nat := if value then 1 else 0

def awardCount (awarded : List Bool) : Nat :=
  (awarded.map boolNat).sum

/-- Apply a zero/one award mask coordinatewise.  Unequal lengths are rejected
at the semantic-certificate boundary, so the fallback equations are inert. -/
def applyAwards : List Nat → List Bool → List Nat
  | floor :: floors, award :: awards =>
      (floor + boolNat award) :: applyAwards floors awards
  | _, _ => []

theorem applyAwards_length : ∀ {floors : List Nat} {awards : List Bool},
    floors.length = awards.length →
      (applyAwards floors awards).length = floors.length
  | [], [], _ => by simp [applyAwards]
  | [], _ :: _, h => by simp at h
  | _ :: _, [], h => by simp at h
  | _ :: floors, _ :: awards, h => by
      have htail : floors.length = awards.length := by
        simpa using Nat.add_right_cancel h
      simp [applyAwards, applyAwards_length htail]

theorem applyAwards_sum : ∀ {floors : List Nat} {awards : List Bool},
    floors.length = awards.length →
      (applyAwards floors awards).sum = floors.sum + awardCount awards
  | [], [], _ => by simp [applyAwards, awardCount]
  | [], _ :: _, h => by simp at h
  | _ :: _, [], h => by simp at h
  | floor :: floors, award :: awards, h => by
      have htail : floors.length = awards.length := by
        simpa using Nat.add_right_cancel h
      simp only [applyAwards, List.sum_cons, awardCount, List.map_cons]
      rw [applyAwards_sum htail]
      simp only [awardCount]
      omega

def weightResidual (D : Nat) (r : RationalBasis) : Nat :=
  D - (floorWeights D r.denominator r.numerators).sum

def remainderAt (D : Nat) (r : RationalBasis) (index : Nat) : Nat :=
  (floorRemainders D r.denominator r.numerators).getD index 0

/-- Exact semantic certificate for Rust's deterministic largest-remainder
subset.  `priority` says every awarded coordinate outranks every unawarded
coordinate; equality is resolved toward the lower index. -/
structure LargestRemainderSelection (D : Nat) (r : RationalBasis) where
  awarded : List Bool
  arity : awarded.length = r.numerators.length
  cardinality : awardCount awarded = weightResidual D r
  positive : ∀ i, i < r.numerators.length → awarded.getD i false = true →
    0 < remainderAt D r i
  priority : ∀ i j,
    i < r.numerators.length → j < r.numerators.length →
    awarded.getD i false = false → awarded.getD j false = true →
    remainderAt D r i < remainderAt D r j ∨
      (remainderAt D r i = remainderAt D r j ∧ j < i)

/-- When scaling is exact, the deterministic largest-remainder selection is
the all-false mask.  This supplies a general, executable nonvacuity witness for
the important exact-grid path. -/
def zeroResidualSelection {D : Nat} {r : RationalBasis}
    (hz : weightResidual D r = 0) : LargestRemainderSelection D r where
  awarded := List.replicate r.numerators.length false
  arity := by simp
  cardinality := by simp [awardCount, boolNat, hz]
  positive := by
    intro i hi haw
    simp [List.getD, hi] at haw
  priority := by
    intro i j hi hj hui haj
    simp [List.getD, hj] at haj

/-- Largest-remainder integerization of one exact rational basis vector. -/
def quantizeLargest (D : Nat) (r : RationalBasis)
    (selection : LargestRemainderSelection D r) : PayoutVector :=
  { denominator := D
    weights := applyAwards
      (floorWeights D r.denominator r.numerators) selection.awarded }

@[simp] theorem quantizeLargest_length (D : Nat) (r : RationalBasis)
    (selection : LargestRemainderSelection D r) :
    (quantizeLargest D r selection).weights.length = r.numerators.length := by
  simpa [quantizeLargest] using
    (applyAwards_length
      (floors := floorWeights D r.denominator r.numerators)
      (awards := selection.awarded) (by simpa using selection.arity.symm))

/-- The consensus-critical result: deterministic largest-remainder
integerization preserves an exact partition of unity. -/
theorem quantizeLargest_admissible {D : Nat} {r : RationalBasis}
    (hD : 0 < D) (hr : r.Exact) (selection : LargestRemainderSelection D r) :
    (quantizeLargest D r selection).Admissible r.numerators.length := by
  have hfloor : (floorWeights D r.denominator r.numerators).sum ≤ D :=
    floorWeights_sum_le hr.1 hr.2
  refine ⟨hD, quantizeLargest_length D r selection, ?_⟩
  unfold quantizeLargest
  rw [applyAwards_sum]
  · rw [selection.cardinality]
    simp only [weightResidual]
    omega
  · simpa using selection.arity.symm

/-- A largest-remainder result has the same local arity as the exact rational
block.  This is the support bound needed for degrees one through three. -/
theorem quantizeLargest_local_support (D : Nat) (r : RationalBasis)
    (selection : LargestRemainderSelection D r) :
    (quantizeLargest D r selection).weights.countP (fun n => n != 0) ≤
      r.numerators.length := by
  exact Nat.le_trans List.countP_le_length (by simp)

/-- Rust's `residual > degree` refusal is unreachable for each exact local
degree-one basis block. -/
theorem clampedDegreeOne_residual_le {h u D : Nat} (hh : 0 < h) (hu : u ≤ h) :
    weightResidual D (clampedDegreeOne h u) ≤ 1 := by
  have hlt := weightRoundResidual_lt_length (D := D)
    (clampedDegreeOne_exact hh hu).1 (by simp [clampedDegreeOne])
    (clampedDegreeOne_exact hh hu).2
  change D - (floorWeights D h [h - u, u]).sum ≤ 1
  simp only [clampedDegreeOne, List.length_cons, List.length_nil] at hlt
  omega

/-- Rust's residual bound for exact quadratic local blocks. -/
theorem clampedDegreeTwo_residual_le {h u D : Nat} (hh : 0 < h) (hu : u ≤ h) :
    weightResidual D (clampedDegreeTwo h u) ≤ 2 := by
  have hlt := weightRoundResidual_lt_length (D := D)
    (clampedDegreeTwo_exact hh hu).1 (by simp [clampedDegreeTwo])
    (clampedDegreeTwo_exact hh hu).2
  change D - (floorWeights D (h * h)
    [(h - u) * (h - u), u * (h - u) + u * (h - u), u * u]).sum ≤ 2
  simp only [clampedDegreeTwo, List.length_cons, List.length_nil] at hlt
  omega

/-- Rust's residual bound for exact cubic local blocks. -/
theorem clampedDegreeThree_residual_le {h u D : Nat} (hh : 0 < h) (hu : u ≤ h) :
    weightResidual D (clampedDegreeThree h u) ≤ 3 := by
  have hlt := weightRoundResidual_lt_length (D := D)
    (clampedDegreeThree_exact hh hu).1 (by simp [clampedDegreeThree])
    (clampedDegreeThree_exact hh hu).2
  change D - (floorWeights D (h * h * h)
    [(h - u) * (h - u) * (h - u),
      u * (h - u) * (h - u) + u * (h - u) * (h - u) + u * (h - u) * (h - u),
      u * u * (h - u) + u * u * (h - u) + u * u * (h - u),
      u * u * u]).sum ≤ 3
  simp only [clampedDegreeThree, List.length_cons, List.length_nil] at hlt
  omega

/-! ### Checked nonvacuity and tie-breaking fixtures

These are not differential tests for Rust.  They make the semantic certificate
inhabited on all three smooth degrees and pin two largest-remainder ties inside
Lean itself. -/

def linearExactGridSelection :
    LargestRemainderSelection 8 (clampedDegreeOne 8 3) :=
  zeroResidualSelection (by decide)

theorem linearExactGrid_quantized :
    (quantizeLargest 8 (clampedDegreeOne 8 3)
      linearExactGridSelection).weights = [5, 3] := by
  decide

def quadraticMidpointSelection :
    LargestRemainderSelection 7 (clampedDegreeTwo 4 2) where
  awarded := [true, false, true]
  arity := by decide
  cardinality := by decide
  positive := by
    intro i hi _haw
    simp [clampedDegreeTwo] at hi
    have hi' : i = 0 ∨ i = 1 ∨ i = 2 := by omega
    rcases hi' with rfl | rfl | rfl <;> decide
  priority := by
    intro i j hi hj hui haj
    simp [clampedDegreeTwo] at hi hj
    have hi' : i = 0 ∨ i = 1 ∨ i = 2 := by omega
    have hj' : j = 0 ∨ j = 1 ∨ j = 2 := by omega
    rcases hi' with rfl | rfl | rfl <;>
      rcases hj' with rfl | rfl | rfl <;>
      simp [List.getD, remainderAt, floorRemainders, clampedDegreeTwo] at hui haj ⊢

/-- Exact midpoint weights `(1/4,1/2,1/4)` at scale seven floor to
`(1,3,1)`; largest remainder awards the tied edge remainders in index order,
giving `(2,3,2)` rather than a fixed-terminal distortion. -/
theorem quadraticMidpoint_quantized :
    (quantizeLargest 7 (clampedDegreeTwo 4 2)
      quadraticMidpointSelection).weights = [2, 3, 2] := by
  decide

def cubicMidpointSelection :
    LargestRemainderSelection 7 (clampedDegreeThree 4 2) where
  awarded := [true, true, false, true]
  arity := by decide
  cardinality := by decide
  positive := by
    intro i hi _haw
    simp [clampedDegreeThree] at hi
    have hi' : i = 0 ∨ i = 1 ∨ i = 2 ∨ i = 3 := by omega
    rcases hi' with rfl | rfl | rfl | rfl <;> decide
  priority := by
    intro i j hi hj hui haj
    simp [clampedDegreeThree] at hi hj
    have hi' : i = 0 ∨ i = 1 ∨ i = 2 ∨ i = 3 := by omega
    have hj' : j = 0 ∨ j = 1 ∨ j = 2 ∨ j = 3 := by omega
    rcases hi' with rfl | rfl | rfl | rfl <;>
      rcases hj' with rfl | rfl | rfl | rfl <;>
      simp [List.getD, remainderAt, floorRemainders, clampedDegreeThree] at hui haj ⊢

/-- Cubic midpoint exact weights `(1,3,3,1)/8` at scale seven have three
residual atoms. The two outer ties both win, then the lower middle index wins
its exact tie, producing `(1,3,2,1)`. -/
theorem cubicMidpoint_quantized :
    (quantizeLargest 7 (clampedDegreeThree 4 2)
      cubicMidpointSelection).weights = [1, 3, 2, 1] := by
  decide

/-! ## Degrees one through three: integer admissibility and solvency -/

theorem clampedDegreeOne_integer_admissible {h u D : Nat}
    (hh : 0 < h) (hu : u ≤ h) (hD : 0 < D)
    (selection : LargestRemainderSelection D (clampedDegreeOne h u)) :
    (quantizeLargest D (clampedDegreeOne h u) selection).Admissible 2 := by
  simpa using quantizeLargest_admissible hD (clampedDegreeOne_exact hh hu) selection

theorem clampedDegreeTwo_integer_admissible {h u D : Nat}
    (hh : 0 < h) (hu : u ≤ h) (hD : 0 < D)
    (selection : LargestRemainderSelection D (clampedDegreeTwo h u)) :
    (quantizeLargest D (clampedDegreeTwo h u) selection).Admissible 3 := by
  simpa using quantizeLargest_admissible hD (clampedDegreeTwo_exact hh hu) selection

theorem clampedDegreeThree_integer_admissible {h u D : Nat}
    (hh : 0 < h) (hu : u ≤ h) (hD : 0 < D)
    (selection : LargestRemainderSelection D (clampedDegreeThree h u)) :
    (quantizeLargest D (clampedDegreeThree h u) selection).Admissible 4 := by
  simpa using quantizeLargest_admissible hD (clampedDegreeThree_exact hh hu) selection

theorem exactBasis_resolution_bound {D : Nat} {r : RationalBasis}
    (hD : 0 < D) (hr : r.Exact) (selection : LargestRemainderSelection D r)
    (T : List Amount) :
    requiredResolved T (quantizeLargest D r selection) ≤ requiredActive T :=
  P_SOLV_01_resolution_bound (quantizeLargest_admissible hD hr selection)

theorem exactBasis_complete_set_exact {D q : Nat} {r : RationalBasis}
    (hD : 0 < D) (hr : r.Exact) (selection : LargestRemainderSelection D r) :
    requiredResolved (List.replicate r.numerators.length q)
      (quantizeLargest D r selection) = q :=
  P_PAY_02_complete_set_required_exact (quantizeLargest_admissible hD hr selection)

theorem clampedDegreeOne_resolution_bound {h u D : Nat}
    (hh : 0 < h) (hu : u ≤ h) (hD : 0 < D)
    (selection : LargestRemainderSelection D (clampedDegreeOne h u))
    (T : List Amount) :
    requiredResolved T (quantizeLargest D (clampedDegreeOne h u) selection) ≤
      requiredActive T :=
  P_SOLV_01_resolution_bound
    (clampedDegreeOne_integer_admissible hh hu hD selection)

theorem clampedDegreeTwo_resolution_bound {h u D : Nat}
    (hh : 0 < h) (hu : u ≤ h) (hD : 0 < D)
    (selection : LargestRemainderSelection D (clampedDegreeTwo h u))
    (T : List Amount) :
    requiredResolved T (quantizeLargest D (clampedDegreeTwo h u) selection) ≤
      requiredActive T :=
  P_SOLV_01_resolution_bound
    (clampedDegreeTwo_integer_admissible hh hu hD selection)

theorem clampedDegreeThree_resolution_bound {h u D : Nat}
    (hh : 0 < h) (hu : u ≤ h) (hD : 0 < D)
    (selection : LargestRemainderSelection D (clampedDegreeThree h u))
    (T : List Amount) :
    requiredResolved T (quantizeLargest D (clampedDegreeThree h u) selection) ≤
      requiredActive T :=
  P_SOLV_01_resolution_bound
    (clampedDegreeThree_integer_admissible hh hu hD selection)

theorem clampedDegreeOne_complete_set_exact {h u D q : Nat}
    (hh : 0 < h) (hu : u ≤ h) (hD : 0 < D)
    (selection : LargestRemainderSelection D (clampedDegreeOne h u)) :
    requiredResolved (List.replicate 2 q)
      (quantizeLargest D (clampedDegreeOne h u) selection) = q :=
  P_PAY_02_complete_set_required_exact
    (clampedDegreeOne_integer_admissible hh hu hD selection)

theorem clampedDegreeTwo_complete_set_exact {h u D q : Nat}
    (hh : 0 < h) (hu : u ≤ h) (hD : 0 < D)
    (selection : LargestRemainderSelection D (clampedDegreeTwo h u)) :
    requiredResolved (List.replicate 3 q)
      (quantizeLargest D (clampedDegreeTwo h u) selection) = q :=
  P_PAY_02_complete_set_required_exact
    (clampedDegreeTwo_integer_admissible hh hu hD selection)

theorem clampedDegreeThree_complete_set_exact {h u D q : Nat}
    (hh : 0 < h) (hu : u ≤ h) (hD : 0 < D)
    (selection : LargestRemainderSelection D (clampedDegreeThree h u)) :
    requiredResolved (List.replicate 4 q)
      (quantizeLargest D (clampedDegreeThree h u) selection) = q :=
  P_PAY_02_complete_set_required_exact
    (clampedDegreeThree_integer_admissible hh hu hD selection)

end DragonsClutch
