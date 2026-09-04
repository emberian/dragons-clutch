import DClutchSemantics.ProductRuntimeV2
import DClutchSemantics.SourceScheduledMedianV1
import DClutchSemantics.SourceResolutionStateV2Abi
import Std.Tactic

/-!
# Observed-median resolution over an ensemble of declared sources

A market declares `k` sources and a quorum `q ≤ k` under ONE window.  Inside
the window every source is captured independently, each capture a fragment; at
settle the fragments are folded and the selector is taken from the MEDIAN of the
readings, on the cuts' scale.  Fewer than `q` fragments engage the funded
recovery ladder (`SourceResolutionStateV2Abi.Ladder`) exactly as a silent single
source does today.

`docs/design/MECHANISM_ENSEMBLE_RESOLUTION_2026_09_04.md` is the design note this
module states the properties of.  Nothing here is a second model of the parts
that exist: the median is `SourceScheduledMedianV1.median?`, the same
rank-selection scan the on-chain evaluator runs, so the tree keeps ONE median;
the cell is `ProductRuntimeV2.ResultDomain.selectOrdinaryScaled`, the same
selector every route already reaches, so a fold of one fragment is today's
selection to the bit; and the fallback is the ladder's own `advance?`, over the
attempts the ensemble does not consume.

## The fold, in one sentence

Put the readings on the material's one declared scale (a capture whose adapter
disagrees with the statistic's scale refuses at capture and never reaches the
fold), take the median with the tie rule below, and map that one reading through
the scaled selector.  `the_cell_of_the_median_is_the_median_of_the_cells` says
the same selector comes out whether a reader folds readings or folds cells,
because the selector is monotone -- which is also the whole content of the
robustness property: the honest sources bracket the median, the selector is
monotone, so the honest cells bracket the cell.

## The tie rule, exactly

`median?` selects the value of rank `⌊n / 2⌋` (zero-indexed) among `n`
readings: the middle one when `n` is odd and the UPPER of the two middle ones
when `n` is even.  Equal readings are one value at several ranks and
`selects_unique` makes the answer a single number whichever fragment carried it.
The rank is inherited rather than chosen so the two medians in the tree are one;
`exactly_half_can_move_the_cell_up_and_not_down` states the price of the even
case exactly, and the design note recommends `k` and `q` odd.
-/

namespace DClutch.EnsembleResolutionV1

open DClutch
open DClutch.ProductRuntimeV2
open DClutch.SourceScheduledMedianV1

/-! ## The spec -/

/-- The greatest ensemble a material can declare: the primary source plus every
attempt slot of `RecoveryPolicyV2`.  It is the policy's capacity plus one rather
than a fresh number, because a member IS an attempt slot whose deadline is the
primary window's own (`membersShareTheWindow` below). -/
def maxMembers : Nat := SourceRecoveryPolicyV2Abi.maxAttempts + 1

/-- The two numbers an ensemble adds to a material: how many sources, and how
many observations the fold needs. -/
structure Spec where
  members : Nat
  quorum : Nat
  deriving DecidableEq, Repr

def Spec.valid (spec : Spec) : Bool :=
  decide (1 ≤ spec.quorum) && decide (spec.quorum ≤ spec.members) &&
    decide (spec.members ≤ maxMembers)

/-- Today's market: one source, one observation.  A material whose two ensemble
bytes are zero decodes to this, which is what makes the single-source market
byte-identical rather than merely equivalent. -/
def Spec.single : Spec := ⟨1, 1⟩

theorem single_is_valid : Spec.single.valid = true := by native_decide

theorem maxMembers_is_five : maxMembers = 5 := by native_decide

/-- The members take the leading attempt slots, so a valid spec leaves them room. -/
theorem the_members_fit_the_policy (spec : Spec) (canon : spec.valid = true) :
    spec.members - 1 ≤ SourceRecoveryPolicyV2Abi.maxAttempts := by
  simp only [Spec.valid, Bool.and_eq_true, decide_eq_true_eq, maxMembers] at canon
  omega

/-! ## The window and the fragments -/

/-- The one window, as the fold sees it: a capture is admitted from `start`
through the closed `deadline = end + max_age`, the same `primary_deadline`
the sponsored-push capture already refuses past. -/
structure Window where
  start : Nat
  deadline : Nat
  deriving DecidableEq, Repr

/-- One capture: which member answered, its reading normalized on the
material's one scale (an integer, denominator one, as every certificate
producer already pins), and when it was captured. -/
structure Fragment where
  member : Nat
  reading : Int
  observedAt : Nat
  deriving DecidableEq, Repr

/-- A fragment is admitted when it names a member the spec declares and was
captured inside the window.  Both conjuncts are refusals a hostile aims at. -/
def Fragment.admitted (spec : Spec) (window : Window) (fragment : Fragment) : Bool :=
  decide (fragment.member < spec.members) &&
    decide (window.start ≤ fragment.observedAt) &&
    decide (fragment.observedAt ≤ window.deadline)

/-- A fold input is well-formed when every fragment is admitted and no member
answers twice.  On chain the second conjunct is structural -- a member has ONE
head account, seeded by its provider release, and a second capture of the same
source advances that head rather than adding a fragment -- and it is stated
here so that the theorems below cannot be satisfied by counting one source
twice. -/
def wellFormed (spec : Spec) (window : Window) (fragments : List Fragment) : Bool :=
  fragments.all (Fragment.admitted spec window) &&
    decide ((fragments.map Fragment.member).Nodup)

def readings (fragments : List Fragment) : List Int := fragments.map Fragment.reading

/-! ## The fold -/

/-- What a settle decides: a reading and the cell it falls in, or the ladder. -/
inductive Settle where
  | decided (reading : Int) (cell : Nat)
  | ladder
  deriving DecidableEq, Repr

/-- The cell one normalized reading falls in: the scaled selector, applied to
the reading over denominator one.  This is the only place the fold touches a
scale, and it is after the median. -/
def cellOf (domain : ResultDomain) (scale : Scale) (reading : Int) : Nat :=
  domain.selectOrdinaryScaled ⟨reading, 1⟩ scale

/-- **The fold.**  Refuse an ill-formed input; with fewer fragments than the
quorum, the ladder; otherwise the median reading and its cell. -/
def fold (domain : ResultDomain) (scale : Scale) (spec : Spec) (window : Window)
    (fragments : List Fragment) : Option Settle :=
  if !wellFormed spec window fragments then none
  else if fragments.length < spec.quorum then some .ladder
  else
    match median? (readings fragments) with
    | some reading => some (.decided reading (cellOf domain scale reading))
    | none => none

theorem fold_eq_decided {domain : ResultDomain} {scale : Scale} {spec : Spec}
    {window : Window} {fragments : List Fragment} {reading : Int} {cell : Nat}
    (result : fold domain scale spec window fragments = some (.decided reading cell)) :
    wellFormed spec window fragments = true ∧ spec.quorum ≤ fragments.length ∧
      median? (readings fragments) = some reading ∧ cell = cellOf domain scale reading := by
  unfold fold at result
  split at result
  · simp at result
  · rename_i formed
    split at result
    · simp at result
    · rename_i enough
      split at result
      · rename_i found value hit
        simp only [Option.some.injEq, Settle.decided.injEq] at result
        obtain ⟨sameReading, sameCell⟩ := result
        subst sameReading
        exact ⟨by simpa using formed, Nat.not_lt.mp enough, hit, sameCell.symm⟩
      · simp at result

/-! ## The median, completed

`SourceScheduledMedianV1` proves the scan sound and unique.  Two facts it did
not need are needed here: that a nonempty list always HAS a median (liveness
would otherwise be a claim about a partial function), and the honest-majority
bracket for every `n` rather than odd `n` only (an ensemble of odd `k` folds an
even number of fragments the moment one source is dark). -/

private theorem filter_length_mono (weaker stronger : Int → Bool)
    (implies : ∀ value, weaker value = true → stronger value = true) (values : List Int) :
    (values.filter weaker).length ≤ (values.filter stronger).length := by
  induction values with
  | nil => simp
  | cons head rest ih =>
      by_cases weak : weaker head = true
      · rw [List.filter_cons_of_pos weak, List.filter_cons_of_pos (implies head weak)]
        simpa using ih
      · rw [List.filter_cons_of_neg weak]
        by_cases strong : stronger head = true
        · rw [List.filter_cons_of_pos strong]
          simp only [List.length_cons]
          omega
        · rw [List.filter_cons_of_neg strong]
          exact ih

private theorem perm_below {left right : List Int} (perm : left.Perm right) (candidate : Int) :
    below candidate left = below candidate right :=
  (perm.filter _).length_eq

private theorem perm_atMost {left right : List Int} (perm : left.Perm right) (candidate : Int) :
    atMost candidate left = atMost candidate right :=
  (perm.filter _).length_eq

private theorem below_append (candidate : Int) (left right : List Int) :
    below candidate (left ++ right) = below candidate left + below candidate right := by
  simp [below, List.filter_append]

private theorem atMost_append (candidate : Int) (left right : List Int) :
    atMost candidate (left ++ right) = atMost candidate left + atMost candidate right := by
  simp [atMost, List.filter_append]

private theorem atMost_eq_zero {candidate : Int} {values : List Int}
    (above : ∀ value ∈ values, candidate < value) : atMost candidate values = 0 := by
  simp only [atMost, List.length_eq_zero_iff, List.filter_eq_nil_iff, decide_eq_true_eq]
  intro value member
  have strictlyAbove := above value member
  omega

private theorem below_eq_zero {candidate : Int} {values : List Int}
    (above : ∀ value ∈ values, candidate ≤ value) : below candidate values = 0 := by
  simp only [below, List.length_eq_zero_iff, List.filter_eq_nil_iff, decide_eq_true_eq]
  intro value member
  have atLeast := above value member
  omega

private theorem below_eq_length {candidate : Int} {values : List Int}
    (under : ∀ value ∈ values, value < candidate) : below candidate values = values.length := by
  have kept : values.filter (fun value => decide (value < candidate)) = values := by
    rw [List.filter_eq_self]
    intro value member
    simpa using under value member
  simp only [below, kept]

private theorem atMost_eq_length {candidate : Int} {values : List Int}
    (under : ∀ value ∈ values, value ≤ candidate) : atMost candidate values = values.length := by
  have kept : values.filter (fun value => decide (value ≤ candidate)) = values := by
    rw [List.filter_eq_self]
    intro value member
    simpa using under value member
  simp only [atMost, kept]

private theorem atMost_le_length (candidate : Int) (values : List Int) :
    atMost candidate values ≤ values.length :=
  List.length_filter_le _ _

/-- Splitting a list by any predicate splits its counts. -/
private theorem below_split (candidate : Int) (keep : Int → Bool) (values : List Int) :
    below candidate values =
      below candidate (values.filter keep) +
        below candidate (values.filter (fun value => !keep value)) := by
  rw [← below_append, perm_below (List.filter_append_perm keep values)]

private theorem atMost_split (candidate : Int) (keep : Int → Bool) (values : List Int) :
    atMost candidate values =
      atMost candidate (values.filter keep) +
        atMost candidate (values.filter (fun value => !keep value)) := by
  rw [← atMost_append, perm_atMost (List.filter_append_perm keep values)]

/-- **A nonempty list has a value at every rank.**  This is quickselect's
correctness: pivot on the head; if the rank is below the pivot's rank, the
answer is among the strictly smaller values, and if it is past the pivot's last
rank, among the strictly larger ones, with the rank shifted by the values
discarded.  Both sublists are strictly shorter, so the recursion is a proof. -/
theorem exists_selects :
    ∀ (values : List Int) (rank : Nat), rank < values.length → ∃ value, Selects rank values value
  | [], _, bound => by simp at bound
  | pivot :: rest, rank, bound => by
      by_cases low : below pivot (pivot :: rest) ≤ rank
      · by_cases high : rank < atMost pivot (pivot :: rest)
        · exact ⟨pivot, by simp, low, high⟩
        · -- Past the pivot: recurse on the strictly larger values.
          have shorter : ((pivot :: rest).filter (fun value => decide (pivot < value))).length
              < (pivot :: rest).length := by
            have dropHead : (pivot :: rest).filter (fun value => decide (pivot < value)) =
                rest.filter (fun value => decide (pivot < value)) := by simp
            rw [dropHead]
            have := List.length_filter_le (fun value => decide (pivot < value)) rest
            simp only [List.length_cons]
            omega
          have complement : ((pivot :: rest).filter
              (fun value => !decide (pivot < value))).length = atMost pivot (pivot :: rest) := by
            unfold atMost
            congr 1
            apply List.filter_congr
            intro value _
            by_cases order : pivot < value
            · simp [order, Int.not_le.mpr order]
            · simp [order, Int.not_lt.mp order]
          have bound' : rank - atMost pivot (pivot :: rest)
              < ((pivot :: rest).filter (fun value => decide (pivot < value))).length := by
            have total := atMost_split pivot (fun value => decide (pivot < value)) (pivot :: rest)
            have selfAtMost : atMost pivot ((pivot :: rest).filter
                (fun value => decide (pivot < value))) = 0 := by
              apply atMost_eq_zero
              intro value member
              simpa using (List.mem_filter.mp member).2
            have whole := (List.filter_append_perm (fun value => decide (pivot < value))
              (pivot :: rest)).length_eq
            rw [List.length_append] at whole
            omega
          obtain ⟨value, member, belowBound, rankBound⟩ :=
            exists_selects ((pivot :: rest).filter (fun value => decide (pivot < value)))
              (rank - atMost pivot (pivot :: rest)) bound'
          have above : pivot < value := by simpa using (List.mem_filter.mp member).2
          refine ⟨value, (List.mem_filter.mp member).1, ?_, ?_⟩
          · rw [below_split value (fun value => decide (pivot < value))]
            have rest' : below value ((pivot :: rest).filter
                (fun value => !decide (pivot < value))) =
                  ((pivot :: rest).filter (fun value => !decide (pivot < value))).length := by
              apply below_eq_length
              intro sample member'
              have := (List.mem_filter.mp member').2
              simp only [Bool.not_eq_true', decide_eq_false_iff_not, Int.not_lt] at this
              omega
            rw [rest', complement]
            omega
          · rw [atMost_split value (fun value => decide (pivot < value))]
            have rest' : atMost value ((pivot :: rest).filter
                (fun value => !decide (pivot < value))) =
                  ((pivot :: rest).filter (fun value => !decide (pivot < value))).length := by
              apply atMost_eq_length
              intro sample member'
              have := (List.mem_filter.mp member').2
              simp only [Bool.not_eq_true', decide_eq_false_iff_not, Int.not_lt] at this
              omega
            rw [rest', complement]
            omega
      · -- Before the pivot: recurse on the strictly smaller values.
        have shorter : ((pivot :: rest).filter (fun value => decide (value < pivot))).length
            < (pivot :: rest).length := by
          have dropHead : (pivot :: rest).filter (fun value => decide (value < pivot)) =
              rest.filter (fun value => decide (value < pivot)) := by simp
          rw [dropHead]
          have := List.length_filter_le (fun value => decide (value < pivot)) rest
          simp only [List.length_cons]
          omega
        have bound' : rank < ((pivot :: rest).filter (fun value => decide (value < pivot))).length := by
          unfold below at low
          omega
        obtain ⟨value, member, belowBound, rankBound⟩ :=
          exists_selects ((pivot :: rest).filter (fun value => decide (value < pivot))) rank bound'
        have under : value < pivot := by simpa using (List.mem_filter.mp member).2
        refine ⟨value, (List.mem_filter.mp member).1, ?_, ?_⟩
        · rw [below_split value (fun value => decide (value < pivot))]
          have rest' : below value ((pivot :: rest).filter
              (fun value => !decide (value < pivot))) = 0 := by
            apply below_eq_zero
            intro sample member'
            have := (List.mem_filter.mp member').2
            simp only [Bool.not_eq_true', decide_eq_false_iff_not, Int.not_lt] at this
            omega
          omega
        · rw [atMost_split value (fun value => decide (value < pivot))]
          have rest' : atMost value ((pivot :: rest).filter
              (fun value => !decide (value < pivot))) = 0 := by
            apply atMost_eq_zero
            intro sample member'
            have := (List.mem_filter.mp member').2
            simp only [Bool.not_eq_true', decide_eq_false_iff_not, Int.not_lt] at this
            omega
          omega
termination_by values => values.length
decreasing_by
  all_goals
    simp_wf
    first
      | (have := List.length_filter_le (fun value => decide (pivot < value)) rest; omega)
      | (have := List.length_filter_le (fun value => decide (value < pivot)) rest; omega)

/-- **A nonempty list has a median.**  The scan refuses only the empty list. -/
theorem median_some_of_nonempty {values : List Int} (nonempty : 0 < values.length) :
    ∃ value, median? values = some value := by
  obtain ⟨value, selects⟩ := exists_selects values (values.length / 2) (by omega)
  exact ⟨value, median_of_selects selects⟩

/-- Half the fragments, or fewer, cannot push the median below every honest
reading: some honest reading is at most the median. -/
theorem bracketed_below_by_at_most_half
    {values honest manipulated : List Int} {value : Int}
    (split : values.Perm (honest ++ manipulated))
    (atMostHalf : 2 * manipulated.length ≤ values.length)
    (result : median? values = some value) :
    ∃ sample ∈ honest, sample ≤ value := by
  obtain ⟨_, _, rankBound⟩ := median_sound result
  have lengths : values.length = honest.length + manipulated.length := by
    rw [split.length_eq, List.length_append]
  rcases Classical.em (∃ sample ∈ honest, sample ≤ value) with found | missing
  · exact found
  · exfalso
    have above : ∀ sample ∈ honest, value < sample := by
      intro sample member
      rcases Int.lt_trichotomy value sample with order | order | order
      · exact order
      · exact absurd ⟨sample, member, Int.le_of_eq order.symm⟩ missing
      · exact absurd ⟨sample, member, Int.le_of_lt order⟩ missing
    have honestNone : atMost value honest = 0 := atMost_eq_zero above
    have total : atMost value values = atMost value manipulated := by
      rw [perm_atMost split, atMost_append, honestNone, Nat.zero_add]
    have bounded : atMost value manipulated ≤ manipulated.length :=
      atMost_le_length value manipulated
    omega

/-- Fewer than half the fragments cannot push the median above every honest
reading: some honest reading is at least the median.  Strict, and
`exactly_half_can_move_the_cell_up_and_not_down` shows it has to be. -/
theorem bracketed_above_by_fewer_than_half
    {values honest manipulated : List Int} {value : Int}
    (split : values.Perm (honest ++ manipulated))
    (minority : 2 * manipulated.length < values.length)
    (result : median? values = some value) :
    ∃ sample ∈ honest, value ≤ sample := by
  obtain ⟨_, belowBound, _⟩ := median_sound result
  have lengths : values.length = honest.length + manipulated.length := by
    rw [split.length_eq, List.length_append]
  rcases Classical.em (∃ sample ∈ honest, value ≤ sample) with found | missing
  · exact found
  · exfalso
    have under : ∀ sample ∈ honest, sample < value := by
      intro sample member
      rcases Int.lt_trichotomy sample value with order | order | order
      · exact order
      · exact absurd ⟨sample, member, Int.le_of_eq order.symm⟩ missing
      · exact absurd ⟨sample, member, Int.le_of_lt order⟩ missing
    have honestAll : below value honest = honest.length := below_eq_length under
    have total : below value values = honest.length + below value manipulated := by
      rw [perm_below split, below_append, honestAll]
    omega

/-- **The honest-majority bracket, for every `n`.**  `SourceScheduledMedianV1.
median_within_honest_range` states this for an odd window; an ensemble folds an
even number of fragments whenever one of an odd `k` is dark, so the odd
hypothesis is dropped and the statement survives unchanged. -/
theorem the_median_is_bracketed_by_an_honest_majority
    {values honest manipulated : List Int} {value : Int}
    (split : values.Perm (honest ++ manipulated))
    (minority : 2 * manipulated.length < values.length)
    (result : median? values = some value) :
    (∃ sample ∈ honest, sample ≤ value) ∧ (∃ sample ∈ honest, value ≤ sample) :=
  ⟨bracketed_below_by_at_most_half split (Nat.le_of_lt minority) result,
    bracketed_above_by_fewer_than_half split minority result⟩

/-! ## The selector is monotone, so the cells bracket too -/

/-- At a fixed denominator the selector never falls as the numerator rises: a
larger reading is at or above every cut a smaller one is. -/
theorem selectOrdinaryFrom_mono (denominator cutDenominator : Nat) (cuts : List Int)
    {smaller larger : Int} (order : smaller ≤ larger) :
    selectOrdinaryFrom ⟨smaller, denominator⟩ cutDenominator cuts ≤
      selectOrdinaryFrom ⟨larger, denominator⟩ cutDenominator cuts := by
  induction cuts with
  | nil => simp [selectOrdinaryFrom]
  | cons cut rest ih =>
      have scaled : smaller * (cutDenominator : Int) ≤ larger * (cutDenominator : Int) :=
        Int.mul_le_mul_of_nonneg_right order (Int.natCast_nonneg _)
      by_cases smallerBelow : rationalLessCut ⟨smaller, denominator⟩ cutDenominator cut = true
      · simp [selectOrdinaryFrom, smallerBelow]
      · have largerBelow : rationalLessCut ⟨larger, denominator⟩ cutDenominator cut = false := by
          simp only [rationalLessCut, decide_eq_true_eq] at smallerBelow
          simp only [rationalLessCut, decide_eq_false_iff_not]
          omega
        simp only [selectOrdinaryFrom, smallerBelow, largerBelow, Bool.false_eq_true, if_false]
        omega

theorem cellOf_mono (domain : ResultDomain) (scale : Scale) {smaller larger : Int}
    (order : smaller ≤ larger) :
    cellOf domain scale smaller ≤ cellOf domain scale larger := by
  unfold cellOf ResultDomain.selectOrdinaryScaled Coordinate.onCutScale
  exact selectOrdinaryFrom_mono _ _ _ order

/-- A monotone map commutes with the median: the image of the median is the
median of the images.  Proved over the scan's own selection predicate, so it is
a fact about the evaluator and not about an idealised median. -/
theorem median_map_monotone (f : Int → Int)
    (mono : ∀ smaller larger, smaller ≤ larger → f smaller ≤ f larger)
    {values : List Int} {value : Int} (result : median? values = some value) :
    median? (values.map f) = some (f value) := by
  obtain ⟨member, belowBound, rankBound⟩ := median_sound result
  have length : (values.map f).length = values.length := List.length_map f
  refine median_of_selects ⟨List.mem_map.mpr ⟨value, member, rfl⟩, ?_, ?_⟩
  · rw [length]
    have image : below (f value) (values.map f) =
        (values.filter (fun sample => decide (f sample < f value))).length := by
      simp [below, List.filter_map, Function.comp_def]
    rw [image]
    refine Nat.le_trans (filter_length_mono _ _ ?_ values) belowBound
    intro sample imageBelow
    simp only [decide_eq_true_eq] at imageBelow ⊢
    rcases Int.lt_or_le sample value with order | order
    · exact order
    · exact absurd imageBelow (Int.not_lt.mpr (mono value sample order))
  · rw [length]
    have image : atMost (f value) (values.map f) =
        (values.filter (fun sample => decide (f sample ≤ f value))).length := by
      simp [atMost, List.filter_map, Function.comp_def]
    rw [image]
    refine Nat.lt_of_lt_of_le rankBound (filter_length_mono _ _ ?_ values)
    intro sample sampleAtMost
    simp only [decide_eq_true_eq] at sampleAtMost ⊢
    exact mono sample value sampleAtMost

/-- **Folding readings and folding cells agree.**  A reader holding the `k`
fragments may take the median of the readings and select once, or select `k`
times and take the median of the cells; the selector is the same.  This is why
the certificate can carry the median reading in `result_numerator` and its
cell in `selector` and be checkable from either. -/
theorem the_cell_of_the_median_is_the_median_of_the_cells
    (domain : ResultDomain) (scale : Scale) {values : List Int} {value : Int}
    (result : median? values = some value) :
    median? (values.map (fun reading => (cellOf domain scale reading : Int))) =
      some (cellOf domain scale value) := by
  refine median_map_monotone _ ?_ result
  intro smaller larger order
  exact Int.ofNat_le.mpr (cellOf_mono domain scale order)

/-! ## (a) The single-source market is today's -/

theorem median_singleton (reading : Int) : median? [reading] = some reading := by
  simp [median?, scan, below, atMost]

/-- **`k = q = 1` is today.**  One admitted fragment folds to its own reading
and the scaled selector of that reading -- the exact value
`resolve_primary_from_authenticated_domain` commits now -- and at the identity
scale it is the pre-factor selector, by the migration statement
`selectOrdinaryScaled_identity`. -/
theorem the_single_source_market_is_today
    (domain : ResultDomain) (scale : Scale) (window : Window) (fragment : Fragment)
    (admitted : fragment.admitted Spec.single window = true) :
    fold domain scale Spec.single window [fragment] =
        some (.decided fragment.reading (domain.selectOrdinaryScaled ⟨fragment.reading, 1⟩ scale)) ∧
      fold domain Scale.identity Spec.single window [fragment] =
        some (.decided fragment.reading (domain.selectOrdinary ⟨fragment.reading, 1⟩)) := by
  have formed : wellFormed ⟨1, 1⟩ window [fragment] = true := by
    simpa [wellFormed, Spec.single] using admitted
  constructor
  · simp [fold, formed, Spec.single, readings, median_singleton, cellOf]
  · simp [fold, formed, Spec.single, readings, median_singleton, cellOf,
      ResultDomain.selectOrdinaryScaled_identity]

/-! ## (b) Robustness: the bound, exactly -/

/-- **Fewer than half the observed fragments cannot move the cell.**  Split the
folded fragments into honest and manipulated; if the manipulated are a strict
minority of what was OBSERVED and every honest reading falls in one cell, the
fold decides that cell.  Nothing is assumed about the manipulated readings at
all -- they may be any integers. -/
theorem an_attacker_below_the_bound_cannot_move_the_cell
    (domain : ResultDomain) (scale : Scale) (spec : Spec) (window : Window)
    {fragments honest manipulated : List Fragment} {cell : Nat}
    (split : fragments.Perm (honest ++ manipulated))
    (minority : 2 * manipulated.length < fragments.length)
    (agree : ∀ fragment ∈ honest, cellOf domain scale fragment.reading = cell)
    {reading : Int} {selected : Nat}
    (result : fold domain scale spec window fragments = some (.decided reading selected)) :
    selected = cell := by
  obtain ⟨_, _, found, shape⟩ := fold_eq_decided result
  have splitReadings : (readings fragments).Perm (readings honest ++ readings manipulated) := by
    simpa [readings, List.map_append] using split.map Fragment.reading
  have minorityReadings : 2 * (readings manipulated).length < (readings fragments).length := by
    simpa [readings] using minority
  obtain ⟨⟨lower, lowerMember, lowerLe⟩, ⟨upper, upperMember, upperGe⟩⟩ :=
    the_median_is_bracketed_by_an_honest_majority splitReadings minorityReadings found
  obtain ⟨lowerFragment, lowerIn, lowerIs⟩ := List.mem_map.mp lowerMember
  obtain ⟨upperFragment, upperIn, upperIs⟩ := List.mem_map.mp upperMember
  have lowerCell := agree lowerFragment lowerIn
  have upperCell := agree upperFragment upperIn
  have fromBelow := cellOf_mono domain scale (lowerIs ▸ lowerLe)
  have fromAbove := cellOf_mono domain scale (upperIs ▸ upperGe)
  rw [shape]
  omega

/-- **The bound in sources.**  A fold needs at least `q` fragments, so an
attacker who controls fewer than `q / 2` of them -- `2·m < q` -- is a strict
minority of any fold that decides, whatever the honest outage pattern was.
With every honest source observed the same statement holds at `k` in place of
`q`, by the theorem above with `fragments.length = k`. -/
theorem an_attacker_with_fewer_than_half_the_quorum_never_moves_the_cell
    (domain : ResultDomain) (scale : Scale) (spec : Spec) (window : Window)
    {fragments honest manipulated : List Fragment} {cell : Nat}
    (split : fragments.Perm (honest ++ manipulated))
    (belowQuorum : 2 * manipulated.length < spec.quorum)
    (agree : ∀ fragment ∈ honest, cellOf domain scale fragment.reading = cell)
    {reading : Int} {selected : Nat}
    (result : fold domain scale spec window fragments = some (.decided reading selected)) :
    selected = cell := by
  obtain ⟨_, enough, _, _⟩ := fold_eq_decided result
  exact an_attacker_below_the_bound_cannot_move_the_cell domain scale spec window split
    (by omega) agree result

/-- The one-cut domain and the window every witness below shares. -/
def witnessDomain : ResultDomain := ⟨1, [100]⟩
def witnessWindow : Window := ⟨0, 10⟩

/-- **The bound is exact, and the even case is asymmetric.**  Two honest
sources read `50` and `60`, in cell `0`.  Two manipulated sources -- exactly
half -- reading `200` and `300` move the fold to cell `1`; reading `-5` and
`-7` they cannot move it below the honest range.  This is
`bracketed_below_by_at_most_half` and the strictness of
`bracketed_above_by_fewer_than_half` in one witness, and it is the reason the
design note recommends an odd `k` and an odd `q`. -/
theorem exactly_half_can_move_the_cell_up_and_not_down :
    fold witnessDomain Scale.identity ⟨4, 4⟩ witnessWindow
        [⟨0, 50, 1⟩, ⟨1, 60, 1⟩, ⟨2, 200, 1⟩, ⟨3, 300, 1⟩] = some (.decided 200 1) ∧
      fold witnessDomain Scale.identity ⟨4, 4⟩ witnessWindow
        [⟨0, 50, 1⟩, ⟨1, 60, 1⟩, ⟨2, -5, 1⟩, ⟨3, -7, 1⟩] = some (.decided 50 0) := by
  native_decide

/-- Three honest of five hold the cell against two manipulated at either end. -/
theorem a_strict_minority_moves_nothing :
    fold witnessDomain Scale.identity ⟨5, 5⟩ witnessWindow
        [⟨0, 50, 1⟩, ⟨1, 55, 1⟩, ⟨2, 60, 1⟩, ⟨3, 200, 1⟩, ⟨4, 300, 1⟩] = some (.decided 60 0) ∧
      fold witnessDomain Scale.identity ⟨5, 5⟩ witnessWindow
        [⟨0, 50, 1⟩, ⟨1, 55, 1⟩, ⟨2, 60, 1⟩, ⟨3, -5, 1⟩, ⟨4, -7, 1⟩] = some (.decided 50 0) := by
  native_decide

/-! ## (d) Liveness: fewer than `q` is the ladder, never a stall -/

theorem fewer_than_the_quorum_engages_the_ladder
    (domain : ResultDomain) (scale : Scale) (spec : Spec) (window : Window)
    {fragments : List Fragment} (formed : wellFormed spec window fragments = true)
    (short : fragments.length < spec.quorum) :
    fold domain scale spec window fragments = some .ladder := by
  simp [fold, formed, short]

theorem a_quorum_always_decides
    (domain : ResultDomain) (scale : Scale) (spec : Spec) (window : Window)
    {fragments : List Fragment} (formed : wellFormed spec window fragments = true)
    (positive : 1 ≤ spec.quorum) (enough : spec.quorum ≤ fragments.length) :
    ∃ reading, fold domain scale spec window fragments =
      some (.decided reading (cellOf domain scale reading)) := by
  have nonempty : 0 < (readings fragments).length := by
    simp only [readings, List.length_map]
    omega
  obtain ⟨reading, found⟩ := median_some_of_nonempty nonempty
  exact ⟨reading, by simp [fold, formed, Nat.not_lt.mpr enough, found]⟩

/-- **The fold never stalls.**  Every well-formed input under a positive quorum
is decided or handed to the ladder; there is no third outcome and no refusal. -/
theorem the_fold_never_stalls
    (domain : ResultDomain) (scale : Scale) (spec : Spec) (window : Window)
    {fragments : List Fragment} (formed : wellFormed spec window fragments = true)
    (positive : 1 ≤ spec.quorum) :
    (fragments.length < spec.quorum ∧ fold domain scale spec window fragments = some .ladder) ∨
      (spec.quorum ≤ fragments.length ∧ ∃ reading,
        fold domain scale spec window fragments =
          some (.decided reading (cellOf domain scale reading))) := by
  by_cases short : fragments.length < spec.quorum
  · exact Or.inl ⟨short, fewer_than_the_quorum_engages_the_ladder domain scale spec window formed short⟩
  · exact Or.inr ⟨Nat.not_lt.mp short,
      a_quorum_always_decides domain scale spec window formed positive (Nat.not_lt.mp short)⟩

/-! ## The ladder is the fallback -/

/-- The members take the leading `k − 1` attempt slots of the policy; the
rungs are what remains.  `take`/`drop` of ONE list, so the record needs no
second list and the ladder's twelve theorems apply to the rungs verbatim. -/
def members (spec : Spec) (policy : SourceRecoveryPolicyV2Abi.Policy) :
    List SourceRecoveryPolicyV2Abi.Attempt :=
  policy.attempts.take (spec.members - 1)

def rungs (spec : Spec) (policy : SourceRecoveryPolicyV2Abi.Policy) :
    List SourceRecoveryPolicyV2Abi.Attempt :=
  policy.attempts.drop (spec.members - 1)

theorem members_and_rungs_partition_the_attempts
    (spec : Spec) (policy : SourceRecoveryPolicyV2Abi.Policy) :
    members spec policy ++ rungs spec policy = policy.attempts :=
  List.take_append_drop _ _

/-- A member is an attempt whose deadline IS the window's closed deadline: the
same second the primary source's own capture stops being admitted.  Founding
refuses a member that says otherwise. -/
def membersShareTheWindow (spec : Spec) (window : Window)
    (policy : SourceRecoveryPolicyV2Abi.Policy) : Bool :=
  (members spec policy).all fun attempt => decide (attempt.deadline = window.deadline)

/-- The ladder the fold falls back to: the market's window deadline and the
rungs.  For `Spec.single` it is the ladder over the whole policy, today's. -/
def fallback (spec : Spec) (window : Window) (policy : SourceRecoveryPolicyV2Abi.Policy) :
    SourceResolutionStateV2Abi.Ladder :=
  { primaryDeadline := window.deadline
    policy := { policy with attempts := rungs spec policy } }

theorem the_single_source_fallback_is_todays_ladder
    (window : Window) (policy : SourceRecoveryPolicyV2Abi.Policy) :
    (fallback Spec.single window policy).policy = policy := by
  simp [fallback, rungs, Spec.single]

open SourceResolutionStateV2Abi in
/-- **The fallback never stalls either.**  Once the window has closed, a market
with rungs advances onto its first rung (`Ladder.advance?`, the transition the
crank runs) and a market without rungs cannot enter the ladder at all -- which
is the no-recovery market's own terminal, the primary exhaustion.  Together
with `the_fold_never_stalls` there is no fragment count and no rung count at
which a market sits with a closed window and nothing to do. -/
theorem the_ladder_is_the_fallback
    (spec : Spec) (window : Window) (policy : SourceRecoveryPolicyV2Abi.Policy)
    (now : Nat) (closed : window.deadline < now) :
    (rungs spec policy = [] →
        (fallback spec window policy).advance? ⟨Phase.tag .primary, 0⟩ now = none) ∧
      (rungs spec policy ≠ [] →
        (fallback spec window policy).advance? ⟨Phase.tag .primary, 0⟩ now =
          some ⟨Phase.tag .recovery, 0⟩) := by
  constructor
  · intro empty
    apply Ladder.a_ladder_with_no_funded_attempt_cannot_be_entered
    simp [Ladder.attemptCount, fallback, empty]
  · intro present
    have positive : 0 < (rungs spec policy).length := List.length_pos_iff.mpr present
    simp [Ladder.advance?, Ladder.canAdvance, Ladder.windowClosed, Ladder.funded,
      Ladder.nextAttempt, Ladder.attemptCount, fallback, Phase.tag, closed, positive]

/-! ## The hostiles, each a refusal -/

/-- A fragment naming a member the spec does not declare refuses the fold. -/
theorem a_fragment_from_a_source_not_in_the_spec_refuses :
    fold witnessDomain Scale.identity ⟨3, 2⟩ witnessWindow
      [⟨0, 50, 1⟩, ⟨1, 55, 1⟩, ⟨3, 60, 1⟩] = none := by
  native_decide

/-- Two fragments from one member refuse the fold: a source is one vote. -/
theorem two_fragments_from_one_source_refuse :
    fold witnessDomain Scale.identity ⟨3, 2⟩ witnessWindow
      [⟨0, 50, 1⟩, ⟨1, 55, 1⟩, ⟨1, 300, 2⟩] = none := by
  native_decide

/-- A fragment captured after the window's closed deadline, or before its start,
refuses the fold; the capture route refuses it earlier, at `ProviderFreshness`. -/
theorem a_fragment_outside_the_window_refuses :
    fold witnessDomain Scale.identity ⟨3, 2⟩ witnessWindow
        [⟨0, 50, 1⟩, ⟨1, 55, 1⟩, ⟨2, 60, 11⟩] = none ∧
      fold witnessDomain Scale.identity ⟨3, 2⟩ witnessWindow
        [⟨0, 50, 1⟩, ⟨1, 55, 1⟩, ⟨2, 60, 0⟩] = some (.decided 55 0) := by
  native_decide

/-- Fewer fragments than the quorum is the ladder, not a decision and not a
refusal: two of three with `q = 3`. -/
theorem a_fold_with_fewer_than_the_quorum_is_the_ladder :
    fold witnessDomain Scale.identity ⟨3, 3⟩ witnessWindow
      [⟨0, 50, 1⟩, ⟨1, 55, 1⟩] = some .ladder := by
  native_decide

/-- **Why the fold presumes one scale.**  Cohort-15's cuts, `10200` and `10600`
over `100` (US cents) with the statistic's factor `-8`; three readings of
`$103.74`, `$103.75` and `$103.80`.  On one scale they fold to cell `1`, the
cell every one of them falls in.  If two of them arrived as raw mantissas at
exponent `-2` instead -- the same prices, authored in cents -- the fold would
read them as ten-thousandths of a cent and decide cell `0`.  A reading not on
the material's scale is not a smaller or larger price, it is a different
number; which is why a member whose adapter exponent disagrees with the
statistic refuses at capture (`ResolutionError::ProviderScale`) and never
reaches this function. -/
theorem a_median_over_mixed_scales_is_not_a_median :
    fold ⟨100, [10200, 10600]⟩ ⟨-8⟩ ⟨3, 3⟩ witnessWindow
        [⟨0, 10373844866, 1⟩, ⟨1, 10375000000, 1⟩, ⟨2, 10380000000, 1⟩] =
          some (.decided 10375000000 1) ∧
      fold ⟨100, [10200, 10600]⟩ ⟨-8⟩ ⟨3, 3⟩ witnessWindow
        [⟨0, 10374, 1⟩, ⟨1, 10375, 1⟩, ⟨2, 10380000000, 1⟩] =
          some (.decided 10375 0) := by
  native_decide

/-- Cohort-15 market 3's reading, as the one fragment of a single-source
ensemble: the fold commits `10397222400` over one at `-8` to cell `1`, the cell
the chain committed at certificate offset 256. -/
theorem cohort15_market3_as_an_ensemble_of_one :
    fold ⟨100, [10200, 10600]⟩ ⟨-8⟩ Spec.single ⟨1788499895, 1788508895⟩
      [⟨0, 10397222400, 1788499916⟩] = some (.decided 10397222400 1) := by
  native_decide

end DClutch.EnsembleResolutionV1
