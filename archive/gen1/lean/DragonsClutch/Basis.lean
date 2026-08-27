import DragonsClutch.Basic
/-!
# The payout basis: partition of unity as a hypothesis on a weight map

This file is the mathematical content of
`docs/implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md` §2–§3.1, and *only* that
content.  It models:

* a payout vector as an exact integer weight list over a common denominator;
* **admissibility** — hypotheses (H1) and (H2) of §3.1;
* a **basis family** as a total weight map `X → PayoutVector` from an admitted
  value domain `X`, every value of which is admissible.

Deliberately **not** modelled here: knots, degrees, panes, de Boor evaluation,
edge policy, `u128` freeze-time bounds, the `TermsAccount` encoding.  Those
choose *which* weight map a market freezes; every theorem in this model holds
for **every** map satisfying (H1) and (H2), so the B-spline construction is a
supplier of `WeightMap` values, not a hypothesis of the theory.  `X` *is* the
admitted value domain: "admissible `x̂`" is not a side condition here, it is
membership in the domain of a total function.
-/
namespace DragonsClutch

/-- A payout vector: exact integer weights over a common denominator, in the
active-outcome order.  The Rust kernel's `PayoutVector` is the fixed-width
encoding of this with a zero-padded `[u64; MAX_OUTCOMES]`; the list here is the
active prefix. -/
structure PayoutVector where
  denominator : Nat
  weights : List Nat
deriving Repr, DecidableEq, Inhabited

namespace PayoutVector

/-- The all-zero vector.  The Rust kernel uses it as the "no vector installed"
sentinel in the two resolution slots; this model uses an inductive `Resolution`
instead and needs the sentinel only for correspondence discussion. -/
def zero : PayoutVector := { denominator := 0, weights := [] }

/-- **Admissibility** — hypotheses (H1) and (H2) of `DISTRIBUTIONAL_CLAIMS_DESIGN.md`
§3.1, over `n` active outcomes.

```text
(H1) nonnegativity:            0 ≤ w_i ≤ D
(H2) exact partition of unity: Σ_{i<n} w_i = D
```

Three fields, not four, and the discrepancy is deliberate:

* `0 ≤ w_i` is structural — amounts are `Nat` (and `u64` in the kernel).  It is
  discharged by the type, not proved.  A signed encoding would have to prove it.
* `w_i ≤ D` is **derivable** from (H2) over unsigned weights and is proved as
  `Admissible.bounded` below rather than assumed.  The Rust kernel checks it
  anyway (`PayoutVector::validate`), which is defence in depth against an
  encoding whose sum check could pass with a wrapped addend — not an independent
  hypothesis of the theory.

This predicate is decidable, so the model can *run* it. -/
def Admissible (v : PayoutVector) (n : Nat) : Prop :=
  0 < v.denominator ∧ v.weights.length = n ∧ v.weights.sum = v.denominator

instance (v : PayoutVector) (n : Nat) : Decidable (v.Admissible n) := by
  unfold Admissible; infer_instance

theorem Admissible.denom_pos {v : PayoutVector} {n : Nat} (h : v.Admissible n) :
    0 < v.denominator := h.1

theorem Admissible.arity {v : PayoutVector} {n : Nat} (h : v.Admissible n) :
    v.weights.length = n := h.2.1

/-- (H2), the partition of unity itself. -/
theorem Admissible.pou {v : PayoutVector} {n : Nat} (h : v.Admissible n) :
    v.weights.sum = v.denominator := h.2.2

/-- (H1)'s upper half, **derived** from (H2).  Recorded as a finding: the design
states `0 ≤ w_i ≤ D` as an assumed hypothesis; over unsigned weights only the
sum is an assumption. -/
theorem Admissible.bounded {v : PayoutVector} {n : Nat} (h : v.Admissible n) :
    ∀ w ∈ v.weights, w ≤ v.denominator := by
  intro w hw
  have := mem_le_sum hw
  rw [h.pou] at this
  exact this

end PayoutVector

/-- A **basis family**: the frozen `(degree, knots, edge policy, D)` of the
design, seen only through what every theorem actually uses — a total map from
the admitted value domain `X` to admissible weight vectors over a single frozen
denominator `D`.

`X` carries the edge policy: under `EDGE-CLAMP-01` it is the whole admitted
value domain, under `EDGE-REFUSE-02` it is the in-range subset.  Either way the
map is total on it, and "admissible `x̂`" means exactly `x : X`. -/
structure WeightMap (X : Type) (n D : Nat) where
  /-- The weight map `w : X → Z^n` of §2.2 composed with the `WEIGHT-ROUND-01`
  integerization of §2.3. -/
  map : X → PayoutVector
  /-- (H1) and (H2) at every admissible value. -/
  admissible : ∀ x, (map x).Admissible n
  /-- One frozen common denominator, shared with the market's payout set. -/
  common : ∀ x, (map x).denominator = D

namespace WeightMap

variable {X : Type} {n D : Nat}

theorem denom_pos (B : WeightMap X n D) (x : X) : 0 < D := by
  have := (B.admissible x).denom_pos
  rw [B.common x] at this
  exact this

theorem pou (B : WeightMap X n D) (x : X) : (B.map x).weights.sum = D := by
  rw [(B.admissible x).pou, B.common x]

end WeightMap

/-! ## The finite preset set

`PayoutSet` is the kernel's immutable finite payout set.  It survives unchanged
in derived-basis markets, where it anchors the common denominator `D` and holds
the named vectors (at minimum the frozen failure-refund vector). -/

/-- Bounds transcribed from the kernel's public constants. -/
def maxOutcomes : Nat := 16
def maxPayouts : Nat := 8
def minOutcomes : Nat := 2

structure PayoutSet where
  outcomes : Nat
  vectors : List PayoutVector
deriving Repr, DecidableEq, Inhabited

namespace PayoutSet

/-- The frozen common denominator: the first vector's, which `Valid` proves is
every vector's.  `0` for the empty set, which `Valid` forbids. -/
def denominator (P : PayoutSet) : Nat :=
  match P.vectors with
  | [] => 0
  | v :: _ => v.denominator

/-- Structural validity of the finite payout set, mirroring
`PayoutSet::validate`: a bounded outcome count, a nonempty bounded vector list,
every vector admissible over the active outcomes, and one common denominator. -/
def Valid (P : PayoutSet) : Prop :=
  minOutcomes ≤ P.outcomes ∧ P.outcomes ≤ maxOutcomes ∧
  0 < P.vectors.length ∧ P.vectors.length ≤ maxPayouts ∧
  (∀ v ∈ P.vectors, v.Admissible P.outcomes) ∧
  (∀ v ∈ P.vectors, v.denominator = P.denominator)

instance (P : PayoutSet) : Decidable P.Valid := by unfold Valid; infer_instance

theorem Valid.denom_pos {P : PayoutSet} (h : P.Valid) : 0 < P.denominator := by
  obtain ⟨_, _, hne, _, hadm, _⟩ := h
  match hv : P.vectors with
  | [] => rw [hv] at hne; simp at hne
  | v :: _ =>
      have : v.Admissible P.outcomes := hadm v (by rw [hv]; simp)
      have hd : P.denominator = v.denominator := by unfold denominator; rw [hv]
      rw [hd]; exact this.denom_pos

end PayoutSet

end DragonsClutch
