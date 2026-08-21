import DragonsClutch.BSpline
/-!
# A generic executable evaluator for the uniform open-clamped smooth basis

`BSpline.lean` proves the exact constructions and, in
`bsplineRefinementFixtures`, exhibits eight rows whose `Split` literals were
derived by hand.  That is why there are eight of them: each row costs a human
derivation, and its assumption manifest says so
(`verus/bspline/BSPLINE_REFINEMENT_ASSUMPTIONS.md` item 2 — "associating each
fixture's concrete `Split` literals with its CSV knot/value row is reviewed and
finite, not a general parser/refinement theorem").

This file removes that per-row cost for the *uniform* family.  It assembles the
already-proved pieces — `locateUniformPane`, `expandedKnotAt`,
`uniformBasisFunsCell`, `refineOne/Two/Three`, `openClampedLeft/Right`,
`RationalBasis.pad`, and the canonical largest-remainder selection — into one
total, computable function from a stored uniform grid and an observed value to
the exact global rational vector, and proves that function `Exact` for every
positive uniform grid, every degree one through three, and every observed
value including both closed endpoints and everything outside the span.

Two things this file does **not** claim.  It is not a refinement theorem about
Rust: it makes a much larger *finite* differential corpus mechanically
generable, which is a different and weaker kind of evidence.  And it covers the
uniform stored grids only — `uniform_rust_expanded_knot_linkage` is the
underlying linkage, and it is stated for `uniformStoredKnots`.
-/

namespace DragonsClutch

/-! ## Total split construction

The executable evaluator cannot assume a nondegenerate knot bracketing: it has
to observe one.  `splitOf?` is the observation, and `paneSplit?_of_cell` below
is the proof that on an admitted uniform grid the observation always succeeds
with exactly the distances `BasisFunsCell.toSplit` records. -/

/-- A `Split` from two checked knot distances, or `none` when the bracketing
degenerates.  The `none` branch is what a caller-supplied invalid `Split` would
otherwise hide. -/
def splitOf? (low high : Nat) : Option Split :=
  if h : 0 < low + high then some { low, high, positive := h } else none

theorem splitOf?_of_pos {low high : Nat} (h : 0 < low + high) :
    splitOf? low high = some { low, high, positive := h } := by
  simp [splitOf?, h]

/-- The two unsigned distances Rust's `BasisFuns` row consumes, observed
rather than assumed. -/
def paneSplit? (U : Nat → Nat) (span column row x : Nat) : Option Split :=
  splitOf? (x - U (basisFunsLeftIndex span column row))
    (U (basisFunsRightIndex span row) - x)

theorem paneSplit?_of_cell {U : Nat → Nat} {span column row x : Nat}
    (cell : BasisFunsCell U span column row x) :
    paneSplit? U span column row x = some cell.toSplit := by
  unfold paneSplit?
  exact splitOf?_of_pos cell.toSplit.positive

/-! ## The `BasisFuns` columns, as total functions

Each column consumes the previous column's common-denominator numerators.  The
`match` on the numerator list is the arity obligation: a column that did not
receive exactly `k` numerators cannot be refined into `k + 1`. -/

/-- Column one: one coefficient to two, over the single split of row zero.
The starting coefficient is `1/1`; every later floor and remainder is
invariant under rescaling a common denominator, so no larger seed is needed. -/
def columnOne? (U : Nat → Nat) (span x : Nat) : Option RationalBasis :=
  match paneSplit? U span 1 0 x with
  | some s => some (refineOne 1 s)
  | none => none

/-- Column two, from a two-numerator column one. -/
def columnTwo? (U : Nat → Nat) (span x : Nat) (prev : RationalBasis) :
    Option RationalBasis :=
  match paneSplit? U span 2 0 x, paneSplit? U span 2 1 x, prev.numerators with
  | some s0, some s1, [a, b] => some (refineTwo prev.denominator a b s0 s1)
  | _, _, _ => none

/-- Column three, from a three-numerator column two. -/
def columnThree? (U : Nat → Nat) (span x : Nat) (prev : RationalBasis) :
    Option RationalBasis :=
  match paneSplit? U span 3 0 x, paneSplit? U span 3 1 x,
      paneSplit? U span 3 2 x, prev.numerators with
  | some s0, some s1, some s2, [a, b, c] =>
      some (refineThree prev.denominator a b c s0 s1 s2)
  | _, _, _, _ => none

/-- The local `degree + 1` block on the pane whose span index is `span`. -/
def localBlock? (U : Nat → Nat) (degree span x : Nat) : Option RationalBasis :=
  match columnOne? U span x with
  | none => none
  | some c1 =>
      match degree with
      | 1 => some c1
      | 2 => columnTwo? U span x c1
      | 3 =>
          match columnTwo? U span x c1 with
          | none => none
          | some c2 => columnThree? U span x c2
      | _ => none

/-! ## The whole evaluation

The control flow mirrors the production entry point exactly: edge clamp, the
two closed-endpoint branches, uniform pane location, the recurrence, then zero
padding into the global outcome vector.  `openClampedLeft/Right` are used for
the endpoints rather than a degenerate recurrence call, which is the point of
their existing theorems. -/

/-- Stored-grid last knot. -/
def uniformLast (origin gap count : Nat) : Nat := origin + (count - 1) * gap

/-- Global outcome count: `n = K - 1 + d` for `d ≥ 1`. -/
def uniformOutcomes (count degree : Nat) : Nat := count - 1 + degree

/-- The interior branch: the recurrence on one located pane, padded into the
global outcome vector at that pane's offset. -/
def uniformInterior? (origin gap count degree pane x : Nat) : Option RationalBasis :=
  match localBlock? (expandedKnotAt (uniformStoredKnots origin gap count) degree)
      degree (degree + pane) x with
  | some block => some (block.pad pane (count - 2 - pane))
  | none => none

/-- `EDGE-CLAMP-01`: the observed value clamped into the closed knot span. -/
def uniformClamp (origin gap count x : Nat) : Nat :=
  max origin (min x (uniformLast origin gap count))

/-- The three branches of the production entry point at an already clamped
coordinate: closed low endpoint, closed high endpoint, interior pane. -/
def uniformSmoothBasisAt? (origin gap count degree c : Nat) : Option RationalBasis :=
  if c ≤ origin then some (openClampedLeft (uniformOutcomes count degree) 1)
  else if uniformLast origin gap count ≤ c then
    some (openClampedRight (uniformOutcomes count degree) 1)
  else uniformInterior? origin gap count degree (locateUniformPane origin gap count c) c

/-- The exact global rational vector at one observed value, or `none` when the
grid or degree is inadmissible or a bracketing degenerated.

The value is clamped into `[origin, last]` before anything else, so the map is
total on every `x`. -/
def uniformSmoothBasis? (origin gap count degree x : Nat) : Option RationalBasis :=
  if gap = 0 ∨ count < 2 ∨ degree < 1 ∨ 3 < degree then none
  else uniformSmoothBasisAt? origin gap count degree (uniformClamp origin gap count x)

/-- The integer payout weights the production evaluator must reproduce. -/
def uniformSmoothWeights? (origin gap count degree D x : Nat) : Option (List Nat) :=
  match uniformSmoothBasis? origin gap count degree x with
  | some r => some (refinementCanonicalWeights D r)
  | none => none

/-! ## Exactness on every admitted uniform input

This is the statement that makes a generated corpus non-vacuous: every row the
emitter below prints came from an exact rational partition of unity, so a
disagreement with Rust is a disagreement about the semantics and not about
whether the Lean side computed anything meaningful. -/

private theorem two_numerators {r : RationalBasis} (h : r.numerators.length = 2) :
    ∃ a b, r.numerators = [a, b] := by
  cases hn : r.numerators with
  | nil => rw [hn] at h; simp at h
  | cons a tail =>
    cases tail with
    | nil => rw [hn] at h; simp at h
    | cons b rest =>
      cases rest with
      | nil => exact ⟨a, b, rfl⟩
      | cons c more => rw [hn] at h; simp at h

private theorem three_numerators {r : RationalBasis} (h : r.numerators.length = 3) :
    ∃ a b c, r.numerators = [a, b, c] := by
  cases hn : r.numerators with
  | nil => rw [hn] at h; simp at h
  | cons a tail =>
    cases tail with
    | nil => rw [hn] at h; simp at h
    | cons b rest =>
      cases rest with
      | nil => rw [hn] at h; simp at h
      | cons c more =>
        cases more with
        | nil => exact ⟨a, b, c, rfl⟩
        | cons d extra => rw [hn] at h; simp at h

private theorem columnOne?_exact {U : Nat → Nat} {span x : Nat}
    (cell : BasisFunsCell U span 1 0 x) :
    ∃ r, columnOne? U span x = some r ∧ r.Exact ∧ r.numerators.length = 2 := by
  refine ⟨refineOne 1 cell.toSplit, ?_, refineOne_exact (by decide) _, by simp [refineOne]⟩
  simp [columnOne?, paneSplit?_of_cell cell]

private theorem columnTwo?_exact {U : Nat → Nat} {span x : Nat} {prev : RationalBasis}
    (hprev : prev.Exact) (harity : prev.numerators.length = 2)
    (cell0 : BasisFunsCell U span 2 0 x) (cell1 : BasisFunsCell U span 2 1 x) :
    ∃ r, columnTwo? U span x prev = some r ∧ r.Exact ∧ r.numerators.length = 3 := by
  obtain ⟨a, b, hab⟩ := two_numerators harity
  have hsum : a + b = prev.denominator := by
    have hexact := hprev.2
    rw [hab] at hexact
    simp only [List.sum_cons, List.sum_nil, Nat.add_zero] at hexact
    omega
  refine ⟨refineTwo prev.denominator a b cell0.toSplit cell1.toSplit, ?_,
    refineTwo_exact hprev.1 hsum _ _, by simp [refineTwo]⟩
  simp [columnTwo?, paneSplit?_of_cell cell0, paneSplit?_of_cell cell1, hab]

private theorem columnThree?_exact {U : Nat → Nat} {span x : Nat} {prev : RationalBasis}
    (hprev : prev.Exact) (harity : prev.numerators.length = 3)
    (cell0 : BasisFunsCell U span 3 0 x) (cell1 : BasisFunsCell U span 3 1 x)
    (cell2 : BasisFunsCell U span 3 2 x) :
    ∃ r, columnThree? U span x prev = some r ∧ r.Exact ∧ r.numerators.length = 4 := by
  obtain ⟨a, b, c, habc⟩ := three_numerators harity
  have hsum : a + b + c = prev.denominator := by
    have hexact := hprev.2
    rw [habc] at hexact
    simp only [List.sum_cons, List.sum_nil, Nat.add_zero] at hexact
    omega
  refine ⟨refineThree prev.denominator a b c cell0.toSplit cell1.toSplit cell2.toSplit,
    ?_, refineThree_exact hprev.1 hsum _ _ _, by simp [refineThree]⟩
  simp [columnThree?, paneSplit?_of_cell cell0, paneSplit?_of_cell cell1,
    paneSplit?_of_cell cell2, habc]

/-- The recurrence itself, abstract in the knot function and the span index.
The three cases chain `refineOne_exact`/`refineTwo_exact`/`refineThree_exact`:
each column's own exactness discharges the `a + b = q` hypothesis of the next,
so nothing here assumes the conclusion at any column. -/
private theorem localBlock?_exact {U : Nat → Nat} {degree span x : Nat}
    (hlow : 1 ≤ degree) (hhigh : degree ≤ 3)
    (hcell : ∀ column row, 1 ≤ column → column ≤ degree → row < column →
      BasisFunsCell U span column row x) :
    ∃ b, localBlock? U degree span x = some b ∧ b.Exact := by
  have hdeg : degree = 1 ∨ degree = 2 ∨ degree = 3 := by omega
  rcases hdeg with rfl | rfl | rfl
  · obtain ⟨c1, hc1, hc1exact, _⟩ :=
      columnOne?_exact (hcell 1 0 (by omega) (by omega) (by omega))
    exact ⟨c1, by simp [localBlock?, hc1], hc1exact⟩
  · obtain ⟨c1, hc1, hc1exact, hc1len⟩ :=
      columnOne?_exact (hcell 1 0 (by omega) (by omega) (by omega))
    obtain ⟨c2, hc2, hc2exact, _⟩ := columnTwo?_exact hc1exact hc1len
      (hcell 2 0 (by omega) (by omega) (by omega))
      (hcell 2 1 (by omega) (by omega) (by omega))
    exact ⟨c2, by simp [localBlock?, hc1, hc2], hc2exact⟩
  · obtain ⟨c1, hc1, hc1exact, hc1len⟩ :=
      columnOne?_exact (hcell 1 0 (by omega) (by omega) (by omega))
    obtain ⟨c2, hc2, hc2exact, hc2len⟩ := columnTwo?_exact hc1exact hc1len
      (hcell 2 0 (by omega) (by omega) (by omega))
      (hcell 2 1 (by omega) (by omega) (by omega))
    obtain ⟨c3, hc3, hc3exact, _⟩ := columnThree?_exact hc2exact hc2len
      (hcell 3 0 (by omega) (by omega) (by omega))
      (hcell 3 1 (by omega) (by omega) (by omega))
      (hcell 3 2 (by omega) (by omega) (by omega))
    exact ⟨c3, by simp [localBlock?, hc1, hc2, hc3], hc3exact⟩

/-- The interior branch: an exact local block padded into the global outcome
vector at its pane offset. -/
private theorem uniformInterior?_exact {origin gap count degree pane c : Nat}
    (hgap : 0 < gap) (hcount : 2 ≤ count) (hlow : 1 ≤ degree) (hhigh : degree ≤ 3)
    (hpane : pane + 1 < count)
    (hlo : origin + pane * gap ≤ c) (hhi : c ≤ origin + (pane + 1) * gap) :
    ∃ r, uniformInterior? origin gap count degree pane c = some r ∧ r.Exact := by
  obtain ⟨b, hb, hbexact⟩ := localBlock?_exact (U :=
      expandedKnotAt (uniformStoredKnots origin gap count) degree)
    (span := degree + pane) (x := c) hlow hhigh
    (fun column row h1 h2 h3 =>
      uniformBasisFunsCell hgap hcount hlow hhigh hpane hlo hhi h1 h2 h3)
  exact ⟨b.pad pane (count - 2 - pane), by rw [uniformInterior?, hb],
    hbexact.pad pane (count - 2 - pane)⟩

/-- Every admitted uniform input evaluates, and its exact rational vector is a
partition of unity — including both closed endpoints and every value the edge
policy clamps. -/
theorem uniformSmoothBasis?_exact {origin gap count degree x : Nat}
    (hgap : 0 < gap) (hcount : 2 ≤ count)
    (hlow : 1 ≤ degree) (hhigh : degree ≤ 3) :
    ∃ r, uniformSmoothBasis? origin gap count degree x = some r ∧ r.Exact := by
  have hguard : ¬ (gap = 0 ∨ count < 2 ∨ degree < 1 ∨ 3 < degree) := by omega
  have houtcomes : 0 < uniformOutcomes count degree := by
    unfold uniformOutcomes; omega
  rw [uniformSmoothBasis?, if_neg hguard]
  generalize hc : uniformClamp origin gap count x = c
  by_cases hleft : c ≤ origin
  · exact ⟨openClampedLeft (uniformOutcomes count degree) 1,
      by rw [uniformSmoothBasisAt?, if_pos hleft],
      openClampedLeft_exact houtcomes (by decide)⟩
  by_cases hright : uniformLast origin gap count ≤ c
  · exact ⟨openClampedRight (uniformOutcomes count degree) 1,
      by rw [uniformSmoothBasisAt?, if_neg hleft, if_pos hright],
      openClampedRight_exact houtcomes (by decide)⟩
  -- Interior.  The clamp bounds are exactly the pane bracket hypotheses.
  rw [uniformSmoothBasisAt?, if_neg hleft, if_neg hright]
  have hlo : origin ≤ c := by omega
  have hhi : c < origin + (count - 1) * gap := by
    have hlast := hright
    unfold uniformLast at hlast
    omega
  obtain ⟨hpane, hpanelo, hpanehi⟩ :=
    locateUniformPane_bracket (origin := origin) (gap := gap) (count := count)
      (x := c) hgap hcount hlo hhi
  exact uniformInterior?_exact hgap hcount hlow hhigh hpane hpanelo
    (Nat.le_of_lt hpanehi)

/-- Every emitted weight vector is an exact integer partition of unity at a
positive scale.  This is `quantizeLargest_admissible` transported along the
generic evaluator, so a generated corpus row is never a vacuous comparison. -/
theorem uniformSmoothWeights?_sum {origin gap count degree D x : Nat}
    (hgap : 0 < gap) (hcount : 2 ≤ count)
    (hlow : 1 ≤ degree) (hhigh : degree ≤ 3) (hD : 0 < D) :
    ∃ w, uniformSmoothWeights? origin gap count degree D x = some w ∧ w.sum = D := by
  obtain ⟨r, hr, hrexact⟩ :=
    uniformSmoothBasis?_exact (origin := origin) (gap := gap) (count := count)
      (degree := degree) (x := x) hgap hcount hlow hhigh
  refine ⟨refinementCanonicalWeights D r, ?_, ?_⟩
  · rw [uniformSmoothWeights?, hr]
  · have hadm := quantizeLargest_admissible hD hrexact
      (canonicalLargestRemainderSelection D r hrexact)
    have hweights :
        (quantizeLargest D r (canonicalLargestRemainderSelection D r hrexact)).weights
          = refinementCanonicalWeights D r := rfl
    have hsum := hadm.2.2
    rw [hweights] at hsum
    exact hsum

/-! ## Agreement with the hand-derived witness vectors

`BSpline.lean`'s eight refinement fixtures were built by writing each pane's
`Split` literals out by hand and proving the resulting rational vector's value
with `decide`.  The generic evaluator above never sees those literals: it
recovers each split from `expandedKnotAt` and `locateUniformPane`.  So the
four smooth rows agreeing is a real cross-check of the indexing, not a
restatement — a wrong `span`, pane, or expansion would show up here while
leaving every hand-derived theorem in `BSpline.lean` untouched. -/

/-- Scale a common-denominator vector without changing the rational value.

Used only to *state* the degree-three agreement below, never to compute: the
generic evaluator and the hand derivation reach the same rational vector over
different common denominators, and this names the relation between the two
representations.  That `refinementCanonicalWeights` is invariant under it is
believed and observed on the corpus rows, but is not proved here and nothing in
this file depends on it. -/
def RationalBasis.scale (k : Nat) (r : RationalBasis) : RationalBasis :=
  { denominator := k * r.denominator, numerators := r.numerators.map (k * ·) }

/-- Stored knots `[0,4,8,12]`, degree two, `x = 2`; the hand-derived
`refinementDegreeTwoFirst` is `(32,80,16,0,0)/128`.  The generic evaluator
reaches the identical common denominator here, so this is a literal equality. -/
theorem corpus_matches_degreeTwoFirst :
    uniformSmoothBasis? 0 4 4 2 2 = some refinementDegreeTwoFirst := by
  decide

/-- The same grid at the interior point `x = 6`, whose pane is not the first;
`refinementDegreeTwoInterior` is `(0,32,192,32,0)/256` embedded at index one. -/
theorem corpus_matches_degreeTwoInterior :
    uniformSmoothBasis? 0 4 4 2 6 = some refinementDegreeTwoInterior := by
  decide

/-- Stored knots `[0,4,8]`, degree three, `x = 2`.  The hand-derived
`refinementDegreeThreeFirst` seeded column three from the *reduced* pair
`(16,40,8)/64`; the generic evaluator never reduces, so it lands on exactly
twice that common denominator with exactly twice each numerator. -/
theorem corpus_matches_degreeThreeFirst :
    uniformSmoothBasis? 0 4 3 3 2 =
      some (RationalBasis.scale 2 refinementDegreeThreeFirst) := by
  decide

/-- The cubic internal knot `x = 4` on the same grid, where one split
degenerates to `low = 0`: `refinementDegreeThreeBoundary` is the exact global
`(0,1,2,1,0)/4`.  Here the hand derivation did not reduce, so the generic
evaluator reproduces its three `Split` literals and its denominator exactly. -/
theorem corpus_matches_degreeThreeBoundary :
    uniformSmoothBasis? 0 4 3 3 4 = some refinementDegreeThreeBoundary := by
  decide

/-- Both closed endpoints of a degree-three grid take the open-clamped branch
rather than a degenerate interior evaluation, and the edge policy carries
every exterior value onto them. -/
theorem corpus_endpoints_are_open_clamped :
    uniformSmoothBasis? 4 4 3 3 0 = some (openClampedLeft 5 1) ∧
      uniformSmoothBasis? 4 4 3 3 4 = some (openClampedLeft 5 1) ∧
      uniformSmoothBasis? 4 4 3 3 12 = some (openClampedRight 5 1) ∧
      uniformSmoothBasis? 4 4 3 3 99 = some (openClampedRight 5 1) := by
  decide

/-! ## The generated differential corpus

Every row is `(degree, D, x, uniform grid)`; the expected weights are computed
by `uniformSmoothWeights?` above, never copied from Rust.  The emitter in
`verus/bspline/emit_degree_corpus.lean` renders these into the production
oracle driver's CSV grammar. -/

/-- One corpus grid: origin, `log2` gap, and stored knot count. -/
structure CorpusGrid where
  origin : Nat
  shift : Nat
  count : Nat
deriving Repr, DecidableEq

def CorpusGrid.gap (g : CorpusGrid) : Nat := 2 ^ g.shift

def CorpusGrid.last (g : CorpusGrid) : Nat := uniformLast g.origin g.gap g.count

def CorpusGrid.knots (g : CorpusGrid) : List Nat :=
  uniformStoredKnots g.origin g.gap g.count

/-- Grids chosen to reach every structural case at degrees two and three: a
single-pane grid where both clamped end effects overlap, short grids where the
end panes still touch, and grids long enough to contain a genuinely interior
pane whose four expanded knots are all distinct. -/
def corpusGrids : List CorpusGrid :=
  [ { origin := 0, shift := 2, count := 2 },
    { origin := 0, shift := 2, count := 3 },
    { origin := 0, shift := 2, count := 5 },
    { origin := 8, shift := 1, count := 4 },
    { origin := 0, shift := 3, count := 6 },
    { origin := 100, shift := 4, count := 3 } ]

/-- Scales spanning the exact-shift path (`D` a power of two at least the
gap), coarse scales where the residual is maximal, and odd scales coprime to
every pane denominator. -/
def corpusDenominators : List Nat := [1, 2, 3, 7, 8, 16, 63, 1000]

/-- Observed values: every integer from two below the low endpoint to two
above the high one, so each row set contains both clamped exteriors, both
closed endpoints, every internal knot, and every interior point. -/
def corpusValues (g : CorpusGrid) : List Nat :=
  (List.range (g.last + 5)).filter (fun x => g.origin ≤ x + 2)

/-- Degrees under test.  Degree one is included on purpose: production
evaluates it through a *different* function (`evaluate_degree_one`, the
division/shift specialization) than degrees two and three, so these rows check
the specialization against the same generic recurrence. -/
def corpusDegrees : List Nat := [1, 2, 3]

/-- A corpus row: the grid/degree/scale/value it came from, and the
Lean-computed weights. -/
structure CorpusRow where
  degree : Nat
  denominator : Nat
  value : Nat
  grid : CorpusGrid
  weights : List Nat
deriving Repr

/-- Admissibility of one candidate row against the production evaluator's own
freeze-time rules, so the corpus never contains a row the evaluator would
refuse for a shape reason: `2 ≤ n ≤ 16`, `K ≤ 16`, and `0 < D`. -/
def corpusRowAdmissible (g : CorpusGrid) (degree D : Nat) : Bool :=
  let n := uniformOutcomes g.count degree
  decide (2 ≤ n ∧ n ≤ 16 ∧ g.count ≤ 16 ∧ 0 < D ∧ 2 ≤ g.count)

/-- One row rendered into the production oracle driver's CSV input grammar
`degree,D,value,edge,log2 spacing,knot count,knots…`.  This is a renderer, not
a second evaluator: it touches no weight. -/
def CorpusRow.driverInput (r : CorpusRow) : String :=
  String.intercalate ","
    ([toString r.degree, toString r.denominator, toString r.value, "c",
      toString r.grid.shift, toString r.grid.count] ++ r.grid.knots.map toString)

def corpusRows : List CorpusRow :=
  corpusGrids.flatMap fun g =>
    corpusDegrees.flatMap fun degree =>
      corpusDenominators.flatMap fun D =>
        if corpusRowAdmissible g degree D then
          (corpusValues g).filterMap fun x =>
            match uniformSmoothWeights? g.origin g.gap g.count degree D x with
            | some w =>
                some { degree, denominator := D, value := x, grid := g, weights := w }
            | none => none
        else []

end DragonsClutch
