import DClutchSemantics.SourceWindowSpecV1Abi
import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# `OddScheduledMedian` with a cadence tolerance

`CHAIN_STATE_SOURCES_2026_08.md` §6.4 makes the odd scheduled median the
family-general mechanism for a chain-state price Source, and then surfaces the
hazard in the same breath: the statistic as shipped requires **strict equal
cadence**, so under Solana congestion one submitter that misses its schedule
second breaks the whole window and the statistic refuses.  That was recorded as
a *provisional judgement with no measurement*, which `AGENTS.md` admits only
with a lifting plan.  This module is the lift, and it is a designed one rather
than a measured one: it states the tolerance and proves that the properties the
median was chosen for survive it.

## The lift

A schedule is `count` nominal slots `slot i = start + cadence · i`, derived from
the *window* and never from the samples — if the samples chose the cadence, an
attacker would choose the cadence.  One tolerance `τ` widens each slot into an
admission interval `[slot i − τ, slot i + τ]`, and the tolerance is bounded by

```text
2 · τ < cadence
```

Everything the median is for follows from that one inequality:

* `admission_windows_are_disjoint` — no instant satisfies two slots, so a
  submitter cannot answer two scheduled positions with one observation, and an
  attacker cannot collapse the window.
* `admitted_times_strictly_increase` — the strictly-increasing-timestamps check
  the shipped evaluator already performs is *implied*, not an extra premise.
* `admitted_samples_stay_separated` — consecutive admitted samples are still at
  least `cadence − 2τ` apart, so §5.1's atomicity bound survives with a stated
  and smaller margin.  This is the real cost of the lift and it is visible in
  the inequality rather than hidden in prose.
* `tolerance_bound_is_tight` — at `2τ = cadence` two slots admit one instant.
  The strict inequality is not decoration.

## Conservativity

`admits_of_zero_tolerance` says `τ = 0` admits exactly the strict-cadence
schedule.  Every window written before the tolerance existed carries `τ = 0` in
a reserved coordinate, so the lift changes no existing record's meaning.

## The statistic itself

The tolerance moves *times*.  The median is a function of *values*, so it is
untouched — but "untouched" is the claim that has to be proved, not assumed, and
the two properties that matter are proved here over the exact rank-selection
scan the on-chain evaluator runs:

* `selects_unique` — at most one value satisfies the selection, so the scan has
  one answer regardless of which equal candidate it happens to reach first;
* `median_permutation_invariant` — the answer does not depend on the order the
  admitted samples were submitted in;
* `median_within_honest_range` — with an odd window and any minority of
  manipulated samples, the median still lies between the smallest and largest
  honest sample.  This is the precise form of §6.4's claim; the median does not
  *ignore* a minority, it is *bracketed* by the honest majority.
-/

namespace DClutch.SourceScheduledMedianV1

open DClutch

/-! ## The tolerated schedule -/

/-- A committed finite schedule.  `start` and `cadence` are derived from the
window; `tolerance` is the new coordinate. -/
structure Schedule where
  start : Int
  cadence : Int
  count : Nat
  tolerance : Int
  deriving DecidableEq, Repr

/-- Nominal time of one scheduled position. -/
def Schedule.slot (schedule : Schedule) (index : Nat) : Int :=
  schedule.start + schedule.cadence * (index : Int)

/-- Every condition the constructor enforces.  The odd count of at least three
is §6.4's; `2 · tolerance < cadence` is this module's. -/
def Schedule.WellFormed (schedule : Schedule) : Prop :=
  0 < schedule.cadence ∧ 3 ≤ schedule.count ∧ schedule.count % 2 = 1 ∧
    0 ≤ schedule.tolerance ∧ 2 * schedule.tolerance < schedule.cadence

/-- Whether an observation time answers one scheduled position. -/
def Schedule.admits (schedule : Schedule) (index : Nat) (time : Int) : Bool :=
  decide (schedule.slot index - schedule.tolerance ≤ time) &&
    decide (time ≤ schedule.slot index + schedule.tolerance)

theorem Schedule.slot_succ (schedule : Schedule) (index : Nat) :
    schedule.slot (index + 1) = schedule.slot index + schedule.cadence := by
  simp only [Schedule.slot, Int.natCast_add, Int.natCast_one, Int.mul_add, Int.mul_one]
  omega

/-- A later slot is at least one full cadence later. -/
theorem Schedule.slot_step (schedule : Schedule) (positive : 0 < schedule.cadence)
    {left right : Nat} (order : left < right) :
    schedule.slot left + schedule.cadence ≤ schedule.slot right := by
  have castOrder : (left : Int) < (right : Int) := by exact_mod_cast order
  have oneLe : (1 : Int) ≤ (right : Int) - (left : Int) := by omega
  have scaled := Int.mul_le_mul_of_nonneg_left oneLe (Int.le_of_lt positive)
  rw [Int.mul_one, Int.mul_sub] at scaled
  simp only [Schedule.slot]
  omega

/-- **The disjointness the tolerance is bounded for.**  No instant answers two
scheduled positions, so the tolerance cannot be used to satisfy the window with
fewer distinct observations than it commits to. -/
theorem admission_windows_are_disjoint (schedule : Schedule)
    (wellFormed : schedule.WellFormed) {left right : Nat} (distinct : left < right)
    (time : Int) (admittedLeft : schedule.admits left time = true) :
    schedule.admits right time = false := by
  obtain ⟨positive, _, _, nonnegative, bounded⟩ := wellFormed
  have step := schedule.slot_step positive distinct
  simp only [Schedule.admits, Bool.and_eq_true, decide_eq_true_eq] at admittedLeft ⊢
  simp only [Bool.and_eq_false_iff, decide_eq_false_iff_not]
  omega

/-- The strictly-increasing-timestamp check the evaluator already performs is a
consequence of the schedule, not an independent premise. -/
theorem admitted_times_strictly_increase (schedule : Schedule)
    (wellFormed : schedule.WellFormed) {left right : Nat} (order : left < right)
    {earlier later : Int}
    (admittedLeft : schedule.admits left earlier = true)
    (admittedRight : schedule.admits right later = true) :
    earlier < later := by
  obtain ⟨positive, _, _, nonnegative, bounded⟩ := wellFormed
  have step := schedule.slot_step positive order
  simp only [Schedule.admits, Bool.and_eq_true, decide_eq_true_eq]
    at admittedLeft admittedRight
  omega

/-- **The atomicity bound survives, with a stated margin.**  Consecutive
admitted samples remain at least `cadence − 2τ` apart, and that quantity is
positive.  §5.1's argument — that an attacker must hold a manipulated price
across separate slots and cannot do it inside one bundle — therefore still
applies, against the smaller separation rather than the nominal cadence. -/
theorem admitted_samples_stay_separated (schedule : Schedule)
    (wellFormed : schedule.WellFormed) (index : Nat) {earlier later : Int}
    (admittedEarlier : schedule.admits index earlier = true)
    (admittedLater : schedule.admits (index + 1) later = true) :
    schedule.cadence - 2 * schedule.tolerance ≤ later - earlier ∧
      0 < schedule.cadence - 2 * schedule.tolerance := by
  obtain ⟨positive, _, _, nonnegative, bounded⟩ := wellFormed
  simp only [Schedule.admits, Bool.and_eq_true, decide_eq_true_eq, schedule.slot_succ index]
    at admittedEarlier admittedLater
  omega

/-- **The bound is tight.**  Relaxing `2τ < cadence` to `2τ ≤ cadence` lets one
instant answer two adjacent positions, which is exactly the collapse the bound
exists to forbid. -/
theorem tolerance_bound_is_tight (schedule : Schedule)
    (positive : 0 < schedule.cadence)
    (relaxed : schedule.cadence ≤ 2 * schedule.tolerance) (index : Nat) :
    schedule.admits index (schedule.slot index + schedule.tolerance) = true ∧
      schedule.admits (index + 1) (schedule.slot index + schedule.tolerance) = true := by
  constructor <;>
    simp only [Schedule.admits, Bool.and_eq_true, decide_eq_true_eq,
      schedule.slot_succ index] <;>
    omega

/-- **Conservativity.**  A zero tolerance admits exactly the strict equal
cadence the shipped evaluator requires, so no existing window changes meaning. -/
theorem admits_of_zero_tolerance (schedule : Schedule)
    (strict : schedule.tolerance = 0) (index : Nat) (time : Int) :
    schedule.admits index time = true ↔ time = schedule.slot index := by
  simp only [Schedule.admits, strict, Bool.and_eq_true, decide_eq_true_eq]
  omega

/-! ## The exact median

`below` and `atMost` are the two counts the on-chain scan computes (it computes
`below` and `equal`; `atMost = below + equal`). -/

def below (candidate : Int) (atoms : List Int) : Nat :=
  (atoms.filter fun value => decide (value < candidate)).length

def atMost (candidate : Int) (atoms : List Int) : Nat :=
  (atoms.filter fun value => decide (value ≤ candidate)).length

/-- The rank-selection predicate the scan tests, stated once. -/
def Selects (rank : Nat) (atoms : List Int) (candidate : Int) : Prop :=
  candidate ∈ atoms ∧ below candidate atoms ≤ rank ∧ rank < atMost candidate atoms

/-- The scan itself: try every sample, in submission order, exactly as the
`no_alloc` evaluator does. -/
def scan (rank : Nat) (atoms : List Int) : List Int → Option Int
  | [] => none
  | candidate :: rest =>
      if below candidate atoms ≤ rank ∧ rank < atMost candidate atoms then
        some candidate
      else
        scan rank atoms rest

def median? (atoms : List Int) : Option Int :=
  scan (atoms.length / 2) atoms atoms

private theorem filter_length_mono {α : Type} (weaker stronger : α → Bool)
    (implies : ∀ value, weaker value = true → stronger value = true) (values : List α) :
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

/-- Everything at most a smaller candidate is strictly below a larger one. -/
theorem atMost_le_below (atoms : List Int) {smaller larger : Int}
    (order : smaller < larger) : atMost smaller atoms ≤ below larger atoms := by
  refine filter_length_mono _ _ ?_ atoms
  intro value admitted
  simp only [decide_eq_true_eq] at admitted ⊢
  omega

/-- **One answer.**  At most one value satisfies the selection, so the scan's
result does not depend on which of several equal candidates it reaches first. -/
theorem selects_unique {rank : Nat} {atoms : List Int} {left right : Int}
    (selectsLeft : Selects rank atoms left) (selectsRight : Selects rank atoms right) :
    left = right := by
  obtain ⟨_, belowLeft, rankLeft⟩ := selectsLeft
  obtain ⟨_, belowRight, rankRight⟩ := selectsRight
  rcases Int.lt_trichotomy left right with order | equal | order
  · have := atMost_le_below atoms order
    omega
  · exact equal
  · have := atMost_le_below atoms order
    omega

theorem scan_sound {rank : Nat} {atoms candidates : List Int} {value : Int}
    (result : scan rank atoms candidates = some value) :
    value ∈ candidates ∧ below value atoms ≤ rank ∧ rank < atMost value atoms := by
  induction candidates with
  | nil => simp [scan] at result
  | cons head rest ih =>
      by_cases selected : below head atoms ≤ rank ∧ rank < atMost head atoms
      · rw [scan, if_pos selected] at result
        have headIsValue : head = value := by simpa using result
        subst headIsValue
        exact ⟨by simp, selected.1, selected.2⟩
      · rw [scan, if_neg selected] at result
        obtain ⟨member, bounds⟩ := ih result
        exact ⟨by simp [member], bounds⟩

theorem scan_of_witness {rank : Nat} {atoms candidates : List Int} {value : Int}
    (member : value ∈ candidates)
    (bounds : below value atoms ≤ rank ∧ rank < atMost value atoms) :
    ∃ result, scan rank atoms candidates = some result := by
  induction candidates with
  | nil => simp at member
  | cons head rest ih =>
      by_cases selected : below head atoms ≤ rank ∧ rank < atMost head atoms
      · exact ⟨head, by rw [scan, if_pos selected]⟩
      · rcases List.mem_cons.1 member with rfl | tail
        · exact absurd bounds selected
        · obtain ⟨result, found⟩ := ih tail
          exact ⟨result, by rw [scan, if_neg selected]; exact found⟩

/-- The scan is sound against the selection predicate. -/
theorem median_sound {atoms : List Int} {value : Int}
    (result : median? atoms = some value) :
    Selects (atoms.length / 2) atoms value := by
  obtain ⟨member, bounds⟩ := scan_sound result
  exact ⟨member, bounds⟩

/-- A witness for the selection is always found: the scan refuses only when no
sample satisfies the rank condition at all. -/
theorem median_of_selects {atoms : List Int} {value : Int}
    (selects : Selects (atoms.length / 2) atoms value) :
    median? atoms = some value := by
  obtain ⟨member, bounds⟩ := selects
  obtain ⟨result, found⟩ := scan_of_witness member bounds
  have soundness := median_sound (atoms := atoms) found
  have sameValue := selects_unique soundness ⟨member, bounds⟩
  unfold median?
  rw [found, sameValue]

private theorem perm_below {left right : List Int} (perm : left.Perm right)
    (candidate : Int) : below candidate left = below candidate right :=
  (perm.filter _).length_eq

private theorem perm_atMost {left right : List Int} (perm : left.Perm right)
    (candidate : Int) : atMost candidate left = atMost candidate right :=
  (perm.filter _).length_eq

/-- **Order independence.**  The admitted samples are a set of observations, not
a sequence; the statistic must not depend on the order they were submitted in,
and it does not. -/
theorem median_permutation_invariant {left right : List Int} (perm : left.Perm right) :
    median? left = median? right := by
  have transport : ∀ {first second : List Int}, first.Perm second → ∀ value : Int,
      median? first = some value → median? second = some value := by
    intro first second step value result
    obtain ⟨member, bounds⟩ := median_sound result
    refine median_of_selects ⟨step.mem_iff.1 member, ?_, ?_⟩
    · rw [← perm_below step, ← step.length_eq]; exact bounds.1
    · rw [← perm_atMost step, ← step.length_eq]; exact bounds.2
  cases onLeft : median? left with
  | none =>
      cases onRight : median? right with
      | none => rfl
      | some value =>
          have mirrored := transport perm.symm value onRight
          rw [onLeft] at mirrored
          exact absurd mirrored (by simp)
  | some value => exact (transport perm value onLeft).symm

/-! ## What a minority of manipulated samples can do -/

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

private theorem below_eq_length {candidate : Int} {values : List Int}
    (under : ∀ value ∈ values, value < candidate) : below candidate values = values.length := by
  have kept : values.filter (fun value => decide (value < candidate)) = values := by
    rw [List.filter_eq_self]
    intro value member
    simpa using under value member
  simp only [below, kept]

private theorem atMost_le_length (candidate : Int) (values : List Int) :
    atMost candidate values ≤ values.length :=
  List.length_filter_le _ _

/-- **The manipulation property, stated exactly.**  Split the admitted samples
into honest and manipulated ones.  If the window is odd and the manipulated
samples are a strict minority, then the median is bracketed by the honest
samples: some honest sample is at most the median and some honest sample is at
least it.  An attacker who holds a manipulated price across a minority of the
scheduled positions therefore cannot push the answer outside the honest range at
all — which is what makes the median, rather than a mean, the right statistic
here.

The tolerance does not appear in this statement, and that is the point: it moves
*when* a sample may land, never *which* samples count.  `admission_windows_are_
disjoint` is what forbids the tolerance from turning one observation into two
admitted samples and so changing the split this theorem quantifies over. -/
theorem median_within_honest_range
    {atoms honest manipulated : List Int} {value : Int}
    (split : atoms.Perm (honest ++ manipulated))
    (oddWindow : atoms.length % 2 = 1)
    (minority : 2 * manipulated.length < atoms.length)
    (result : median? atoms = some value) :
    (∃ sample ∈ honest, sample ≤ value) ∧ (∃ sample ∈ honest, value ≤ sample) := by
  obtain ⟨_, belowBound, rankBound⟩ := median_sound result
  have lengths : atoms.length = honest.length + manipulated.length := by
    rw [split.length_eq, List.length_append]
  have rank : 2 * (atoms.length / 2) + 1 = atoms.length := by omega
  constructor
  · rcases Classical.em (∃ sample ∈ honest, sample ≤ value) with found | missing
    · exact found
    · exfalso
      have above : ∀ sample ∈ honest, value < sample := by
        intro sample member
        rcases Int.lt_trichotomy value sample with order | order | order
        · exact order
        · exact absurd ⟨sample, member, Int.le_of_eq order.symm⟩ missing
        · exact absurd ⟨sample, member, Int.le_of_lt order⟩ missing
      have honestNone : atMost value honest = 0 := atMost_eq_zero above
      have total : atMost value atoms = atMost value manipulated := by
        rw [perm_atMost split, atMost_append, honestNone, Nat.zero_add]
      have bounded : atMost value manipulated ≤ manipulated.length :=
        atMost_le_length value manipulated
      omega
  · rcases Classical.em (∃ sample ∈ honest, value ≤ sample) with found | missing
    · exact found
    · exfalso
      have under : ∀ sample ∈ honest, sample < value := by
        intro sample member
        rcases Int.lt_trichotomy sample value with order | order | order
        · exact order
        · exact absurd ⟨sample, member, Int.le_of_eq order.symm⟩ missing
        · exact absurd ⟨sample, member, Int.le_of_lt order⟩ missing
      have honestAll : below value honest = honest.length := below_eq_length under
      have total : below value atoms = honest.length + below value manipulated := by
        rw [perm_below split, below_append, honestAll]
      omega

/-! ## The evaluated statistic

The evaluator's obligation, assembled: exactly `count` samples, each admitted at
its own scheduled position, and then the median of their atoms. -/

structure Sample where
  unixSeconds : Int
  atoms : Int
  deriving DecidableEq, Repr

def admittedSchedule (schedule : Schedule) (samples : List Sample) : Bool :=
  samples.length == schedule.count &&
    (samples.zipIdx.all fun indexed => schedule.admits indexed.2 indexed.1.unixSeconds)

def evaluate (schedule : Schedule) (samples : List Sample) : Option Int :=
  if admittedSchedule schedule samples then median? (samples.map Sample.atoms) else none

/-- A window written before the tolerance existed carries `τ = 0`, and at `τ = 0`
the admitted schedule is byte-for-byte the strict cadence the shipped evaluator
demanded.  Stated on the exhibited three-sample window rather than universally,
because the universal statement is `admits_of_zero_tolerance` above. -/
def strictExample : Schedule :=
  { start := 1000, cadence := 60, count := 3, tolerance := 0 }

def toleratedExample : Schedule :=
  { strictExample with tolerance := 20 }

theorem examples_are_well_formed :
    strictExample.WellFormed ∧ toleratedExample.WellFormed := by
  constructor <;> exact ⟨by decide, by decide, by decide, by decide, by decide⟩

/-- The congestion case the lift exists for: the middle submitter is eleven
seconds late.  The strict schedule refuses the whole window; the tolerated one
answers it. -/
def lateMiddleWindow : List Sample := [
  { unixSeconds := 1000, atoms := 7 },
  { unixSeconds := 1071, atoms := 9 },
  { unixSeconds := 1120, atoms := 8 }
]

theorem congestion_case :
    evaluate strictExample lateMiddleWindow = none ∧
      evaluate toleratedExample lateMiddleWindow = some 8 := by
  constructor <;> native_decide

/-- A sample beyond the tolerance is still refused; the lift widens the window,
it does not remove it. -/
def tooLateWindow : List Sample := [
  { unixSeconds := 1000, atoms := 7 },
  { unixSeconds := 1081, atoms := 9 },
  { unixSeconds := 1120, atoms := 8 }
]

theorem beyond_tolerance_still_refuses :
    evaluate toleratedExample tooLateWindow = none := by native_decide

/-! ## The tolerance's wire coordinate

The tolerance belongs to the **window**, not to the statistic: `[start, end]`
says what the observation must be *about* and the schedule falls out of it, so
the width of each scheduled position's admission belongs beside them.
`WindowSpecV1` is 112 bytes with an eight-byte reserved tail at 104, and the
tolerance takes the first four of them.  The width does not move, and a window
written before the tolerance existed reads `τ = 0`. -/

open DClutch.AbiSchema

-- These three were bare literals, and `tail_fits_former_reserved` below
-- therefore compared two numbers somebody had typed.  `SourceWindowSpecV1Abi`
-- places the whole 112-byte record, so the width is a sum of twelve fields and
-- the tail's cursor is the width of the eleven in front of it.  The theorem now
-- says the tail ends exactly where the record does, which is what it was always
-- meant to say and could not.
def windowSpecBytes : Nat := DClutch.SourceWindowSpecV1Abi.windowSpecBytes
def windowSpecTailOffset : Nat := DClutch.SourceWindowSpecV1Abi.tailOffset
def windowSpecTailBytes : Nat := DClutch.SourceWindowSpecV1Abi.tailBytes

inductive TailField where
  | cadenceToleranceSeconds | tailReserved
  deriving DecidableEq, Repr

def tailSchema : List (FieldSpec TailField) := [
  ⟨.cadenceToleranceSeconds, .u32⟩,
  ⟨.tailReserved, .reserved 4⟩
]

def tailLayout : List (PlacedField TailField) :=
  specializeFrom windowSpecTailOffset tailSchema

namespace TailField

def rustName : TailField → String
  | .cadenceToleranceSeconds => "WINDOW_SPEC_CADENCE_TOLERANCE_OFFSET_V1"
  | .tailReserved => "WINDOW_SPEC_CADENCE_TOLERANCE_TAIL_RESERVED_OFFSET_V1"

end TailField

theorem tail_fits_former_reserved :
    schemaWidth tailSchema = windowSpecTailBytes ∧
      windowSpecTailOffset + schemaWidth tailSchema = windowSpecBytes := by
  constructor <;> native_decide

theorem tail_coordinates_are_pinned : coordinates tailLayout = [
    (.cadenceToleranceSeconds, 104, 4),
    (.tailReserved, 108, 4)
  ] := by
  native_decide

theorem tail_layout_is_byte_disjoint : tailLayout.Pairwise Before :=
  specializeFrom_pairwise windowSpecTailOffset tailSchema

/-- The lifting plan the tolerance discharges: §6.4's provisional judgement
becomes a designed bound with proofs, and the *measurement* that would replace
the designed default with a per-cluster one is named here rather than left
implicit. -/
def liftingPlanPreimage : List UInt8 :=
  "dclutch/lifting-plan/source-scheduled-median-cadence-tolerance/v1".toUTF8.toList
def liftingPlanId : List UInt8 := [
  0xb0, 0xf5, 0x02, 0x49, 0x5d, 0x7e, 0x92, 0xf2,
  0xb2, 0xe1, 0xab, 0xb3, 0xd8, 0xa6, 0x24, 0xf4,
  0xcd, 0x9d, 0x58, 0xc2, 0xd6, 0x45, 0x01, 0xad,
  0xe0, 0x64, 0x97, 0x01, 0x0f, 0x0f, 0xf6, 0x1b
]

/-! ## Corpora -/

/-- Schedule cases: a schedule and the observation times offered against it. -/
def scheduleCases : List (Schedule × List Int) := [
  -- Strict cadence, exactly on the nominal slots: admitted with or without τ.
  (strictExample, [1000, 1060, 1120]),
  (toleratedExample, [1000, 1060, 1120]),
  -- The congestion case: one late submitter.
  (strictExample, [1000, 1071, 1120]),
  (toleratedExample, [1000, 1071, 1120]),
  -- Late by exactly the tolerance, and by one second more.
  (toleratedExample, [1000, 1080, 1120]),
  (toleratedExample, [1000, 1081, 1120]),
  -- Early by exactly the tolerance, and by one second more.
  (toleratedExample, [1000, 1040, 1120]),
  (toleratedExample, [1000, 1039, 1120]),
  -- The first and last positions tolerate too, and step outside [start, end].
  (toleratedExample, [980, 1060, 1140]),
  (toleratedExample, [979, 1060, 1140]),
  -- Two observations inside one slot's window: refused because the count and
  -- the positions are both fixed by the schedule.
  (toleratedExample, [1000, 1015, 1120]),
  -- Wrong sample count.
  (toleratedExample, [1000, 1060]),
  (toleratedExample, [1000, 1060, 1120, 1180]),
  -- Out of submission order.
  (toleratedExample, [1120, 1060, 1000]),
  -- A five-slot window at a wider cadence with the maximum admissible τ.
  ({ start := 0, cadence := 300, count := 5, tolerance := 149 },
    [149, 300, 600, 900, 1200]),
  ({ start := 0, cadence := 300, count := 5, tolerance := 149 },
    [150, 300, 600, 900, 1200])
]

/-- Median cases over the admitted atoms, including the ties that make the
one-answer theorem load-bearing. -/
def medianCases : List (List Int) := [
  [7, 9, 8],
  [8, 8, 8],
  [9, 7, 8],
  [-5, 0, 5],
  [1, 1, 2],
  [2, 1, 1],
  [1, 2, 2],
  [5, 4, 3, 2, 1],
  [1, 1, 1, 2, 2],
  [-9223372036854775808, 0, 9223372036854775807],
  [3]
]

theorem median_cases_all_answer :
    medianCases.all fun atoms => (median? atoms).isSome := by native_decide

/-- Order independence, decided on the corpus as well as proved in general:
each case and its reversal agree. -/
theorem median_cases_are_order_independent :
    medianCases.all fun atoms => median? atoms == median? atoms.reverse := by native_decide

end DClutch.SourceScheduledMedianV1
