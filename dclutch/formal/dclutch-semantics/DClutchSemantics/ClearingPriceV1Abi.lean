import DClutchSemantics.GeneralRuntimeWireV2

/-!
# The clearing price as a durable chain fact

`MECHANISM_JOINT_CLEARING_2026_09_04.md` finding 4: the price vector of a
cleared batch lived only in the Candidate record, which `CloseCandidate`
reclaims. The forecast -- decision 0032 §2c, "the price series IS the
product" -- was a fact the chain forgot the moment it was paid for.

This module is the single author of `ClearingPriceV1`: a tail appended to the
General batch record (`DCGBTCH2`) at the exact byte where the V1 record
ended, written once by the settlement `Close` and read by the SDK and the
market page's price history. The batch record is the durable per-batch
account the tree already has and the market page already reads; a second PDA
per batch would be one more account creation inside the heaviest frame of the
family for a fact that has a home.

The tail is VACANT -- every byte zero -- until the batch reaches the third
status, `cleared`; the hostile decoder requires exactly that, so a batch that
was never settled cannot present a price. The `batch_id` identity preimage
(`GeneralBatchOccurrenceTermsV1`) is untouched: it commits to the opening
terms and never to this tail, so every order signed against a batch keeps its
identity through the clearing.
-/

namespace DClutch.General.ClearingPriceV1

open DClutch.General.RuntimeWireV2 (Field place recordBytes wellFormed)

/-! ## The V1 batch record this tail follows -/

/-- The V1 batch record's fixed width, as a number the theorems below pin to
the tail's offset. `collection_v1.rs` spelled it `GENERAL_BATCH_BYTES_V1`. -/
def batchV1Bytes : Nat := 224

/-- The clearing tail's fixed fields. -/
def clearingFields : List Field := [
  { name := "cleared_candidate_id", bytes := 32 },
  { name := "cleared_slot", bytes := 8 },
  { name := "sets_move", bytes := 1 },
  { name := "reserved_move", bytes := 7 },
  { name := "sets_quantity", bytes := 8 },
  { name := "filled_lots", bytes := 8 },
  { name := "live_order_count", bytes := 4 },
  { name := "reserved_tail", bytes := 4 }
]

def clearingFixedBytes : Nat := recordBytes clearingFields
def clearingOffset : Nat := batchV1Bytes

/-- One `u64` per outcome, twice: the price vector, then the residual the
close STRANDED at each outcome (decision 0032 §2a; zero wherever the price
is positive, by `JointClearingV1.residual_worth_nothing`). -/
def tailStride : Nat := 8
def tailCount : Nat := 2

def pricesOffset : Nat := clearingOffset + clearingFixedBytes
def residualOffset (outcomeCount : Nat) : Nat := pricesOffset + tailStride * outcomeCount
def batchBytes (outcomeCount : Nat) : Nat :=
  clearingOffset + clearingFixedBytes + tailCount * tailStride * outcomeCount

/-- `DCGBTCH2`, and NOT `DCGBAT02`, which the record's own version-suffix
convention would have chosen.

`DCGBAT02` is already taken, by `general/lifecycle.rs`'s
`BATCH_LIFECYCLE_MAGIC_V2` -- a different persisted object. A wire magic
selects one thing, so the two cannot both hold it, and decision 0007's rule for
refusal codes decides which yields one wire object over: a discriminant that has
never reached a chain or a published reference is not yet a fact and may move,
one that has shipped may not. This record's V2 magic is the new one. -/
def batchMagic : List Nat := [0x44, 0x43, 0x47, 0x42, 0x54, 0x43, 0x48, 0x32]
def batchVersion : Nat := 2

/-- Batch status tags. V1 had the first two; the third is what the close
writes and what makes "a batch clears once" a fact of the record. -/
def statusCollecting : Nat := 1
def statusClosed : Nat := 2
def statusCleared : Nat := 3

/-- Complete-set move tags, shared with the verifier's `RuntimeCompleteSetMoveV2`. -/
def moveNone : Nat := 0
def moveMint : Nat := 1
def moveMerge : Nat := 2

/-! ## What the layout is -/

theorem the_tail_is_seventy_two_fixed_bytes : clearingFixedBytes = 72 := by native_decide

theorem the_tail_begins_where_v1_ended : clearingOffset = 224 := by rfl

theorem prices_begin_at_two_hundred_ninety_six : pricesOffset = 296 := by native_decide

theorem batch_width_is_two_ninety_six_plus_sixteen_per_outcome (outcomeCount : Nat) :
    batchBytes outcomeCount = 296 + 16 * outcomeCount := by
  simp only [batchBytes, clearingOffset, batchV1Bytes, the_tail_is_seventy_two_fixed_bytes,
    tailCount, tailStride]

/-- Every scalar of the tail is aligned when placed at 224: the placements
below are relative, and 224 is a multiple of eight, so alignment survives. -/
theorem every_field_is_aligned : wellFormed clearingFields = true := by native_decide

theorem the_offset_keeps_alignment : clearingOffset % 8 = 0 := by native_decide

/-! ## The vacancy law -/

/-- The clearing tail as decoded. -/
structure ClearingTail where
  clearedCandidateId : Nat
  clearedSlot : Nat
  setsMove : Nat
  setsQuantity : Nat
  filledLots : Nat
  liveOrderCount : Nat
  prices : List Nat
  residual : List Nat
  deriving DecidableEq, Repr

def ClearingTail.vacant (tail : ClearingTail) : Bool :=
  tail.clearedCandidateId = 0 && tail.clearedSlot = 0 && tail.setsMove = moveNone &&
    tail.setsQuantity = 0 && tail.filledLots = 0 && tail.liveOrderCount = 0 &&
    tail.prices.all (· = 0) && tail.residual.all (· = 0)

/-- A cleared tail: a candidate, a slot, a canonical move, prices on the
simplex, and a residual that is zero wherever the price is positive. -/
def ClearingTail.cleared (tail : ClearingTail) (outcomeCount scale : Nat) : Bool :=
  tail.clearedCandidateId != 0 && tail.clearedSlot != 0 &&
    tail.prices.length = outcomeCount && tail.residual.length = outcomeCount &&
    tail.prices.sum = scale &&
    ((tail.setsMove = moveNone && tail.setsQuantity = 0) ||
      ((tail.setsMove = moveMint || tail.setsMove = moveMerge) && 0 < tail.setsQuantity)) &&
    (List.range outcomeCount).all fun outcome =>
      tail.prices[outcome]?.getD 0 = 0 || tail.residual[outcome]?.getD 0 = 0

/-- The record's own status decides which law the tail must satisfy. This is
the hostile decoder's rule: a collecting or closed batch presents a vacant
tail, a cleared batch a cleared one, and nothing else decodes. -/
def tailAdmissible (status : Nat) (tail : ClearingTail) (outcomeCount scale : Nat) : Bool :=
  if status = statusCleared then tail.cleared outcomeCount scale
  else if status = statusCollecting || status = statusClosed then tail.vacant
  else false

/-- PRICES SUM TO ONE BY CONSTRUCTION. The page's derived surface is this
conjunct of the decoder, not a check the page performs. -/
theorem a_cleared_tail_is_on_the_simplex
    (tail : ClearingTail) (outcomeCount scale : Nat)
    (cleared : tail.cleared outcomeCount scale = true) :
    tail.prices.sum = scale := by
  simp only [ClearingTail.cleared, Bool.and_eq_true, decide_eq_true_eq] at cleared
  exact cleared.1.1.2

/-- A residual sits only where the price is zero: the strand is the exact
shape `JointClearingV1.residual_worth_nothing` names, read back off the record. -/
theorem a_cleared_tail_strands_only_at_zero_price
    (tail : ClearingTail) (outcomeCount scale outcome : Nat)
    (cleared : tail.cleared outcomeCount scale = true) (inBounds : outcome < outcomeCount) :
    tail.prices[outcome]?.getD 0 = 0 ∨ tail.residual[outcome]?.getD 0 = 0 := by
  simp only [ClearingTail.cleared, Bool.and_eq_true, decide_eq_true_eq] at cleared
  have := List.all_eq_true.mp cleared.2 outcome (List.mem_range.mpr inBounds)
  simp only [Bool.or_eq_true, decide_eq_true_eq] at this
  exact this

/-! ## Executable witnesses -/

def vacantTail : ClearingTail := {
  clearedCandidateId := 0, clearedSlot := 0, setsMove := 0, setsQuantity := 0,
  filledLots := 0, liveOrderCount := 0, prices := [0, 0, 0], residual := [0, 0, 0] }

/-- The design note's `zeroPriced` witness as a record: ten sets minted,
eight claims of outcome 2 stranded at price zero. -/
def zeroPricedTail : ClearingTail := {
  clearedCandidateId := 0x51, clearedSlot := 1_000, setsMove := moveMint, setsQuantity := 10,
  filledLots := 22, liveOrderCount := 3, prices := [60, 40, 0], residual := [0, 0, 8] }

example : tailAdmissible statusClosed vacantTail 3 100 = true := by native_decide
example : tailAdmissible statusCleared zeroPricedTail 3 100 = true := by native_decide
/-- HOSTILE: a cleared status over a vacant tail, and a vacant status over a
cleared tail, both refuse. -/
example : tailAdmissible statusCleared vacantTail 3 100 = false := by native_decide
example : tailAdmissible statusClosed zeroPricedTail 3 100 = false := by native_decide
/-- HOSTILE: a residual on a priced outcome. -/
example :
    tailAdmissible statusCleared { zeroPricedTail with residual := [1, 0, 8] } 3 100 = false := by
  native_decide
/-- HOSTILE: prices off the simplex. -/
example :
    tailAdmissible statusCleared { zeroPricedTail with prices := [60, 40, 1] } 3 100 = false := by
  native_decide

end DClutch.General.ClearingPriceV1
