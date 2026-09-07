import DClutchSemantics.GeneralRuntimeWireV2

/-!
# The General order record, re-digested for the joint clearing

`MECHANISM_JOINT_CLEARING_2026_09_04.md` §1.1 states an order as three
things -- a signed claim flow per lot, a quantity, and a limit -- and §1.4
rules that `PlaceOrder` admits single-outcome and interval orders only. The V1
record (`collection_v1.rs`, `DCGORD01`) carried an arbitrary portfolio as
`16N` bytes of interleaved `(receive, deliver)` rows and nothing else named
the shape: the verifier bound the rows to the record, but no conjunct could
say "this is a buy of outcome 2" because the record did not say so either.

This module is the single author of the V2 layout (`DCGORD02`). Four shape
fields land in the twenty-four bytes between the V1 header's end and the state
window: a side, an inclusive outcome interval, and the claims one lot moves at
every coordinate of that interval. The first 160 bytes are BYTE-IDENTICAL to
V1 (`the_v1_header_is_a_prefix`), so every fixed-offset EffectProgram write
of a header field keeps its coordinate; only the state window and the rows
move, each by exactly the twenty-four bytes the shape occupies.

The identity is the digest of the header alone. The rows are DERIVED from the
shape (`rowsOf`) and the hostile decoder refuses a record whose rows disagree
with its own shape, so the rows are transport for the emitted PlaceOrder
effect program rather than a second author of the portfolio. Deleting the rows
from the wire is the named follow-on; it moves the emitted transition program
and is not this cohort's.

Semantics live elsewhere: `JointClearingV1` owns what a limit MEANS and
`GeneralClearing` owns the rounding boundary; this file owns bytes and the
shape rule.
-/

namespace DClutch.General.OrderV2

open DClutch.General.RuntimeWireV2 (Field place recordBytes wellFormed prologueFields)

/-! ## The layout -/

/-- The V1 header, field for field. Kept here only so the prefix theorem
below is a statement about V1's real field list and not a number. -/
def orderHeaderV1Fields : List Field := prologueFields ++ [
  { name := "outcome_count", bytes := 4 },
  { name := "nonce", bytes := 8 },
  { name := "min_quote_credit_per_lot", bytes := 8 },
  { name := "owner_id", bytes := 32 },
  { name := "market", bytes := 32 },
  { name := "batch_id", bytes := 32 },
  { name := "generation", bytes := 8 },
  { name := "max_lots", bytes := 8 },
  { name := "max_quote_debit_per_lot", bytes := 8 },
  { name := "valid_until_slot", bytes := 8 }
]

/-- The four shape fields the joint clearing adds. `side` is one byte and
the interval is two `u32`s; `claims_per_lot` is the magnitude the V1 rows
carried at each coordinate, now stated once. -/
def shapeFields : List Field := [
  { name := "side", bytes := 1 },
  { name := "reserved_shape", bytes := 3 },
  { name := "outcome_lo", bytes := 4 },
  { name := "outcome_hi", bytes := 4 },
  { name := "reserved_tail", bytes := 4 },
  { name := "claims_per_lot", bytes := 8 }
]

/-- The signed immutable terms: exactly what the maker signs and exactly the
identity preimage. -/
def orderHeaderFields : List Field := orderHeaderV1Fields ++ shapeFields

/-- The mutable escrow window, masked out of the identity. Same four fields
as V1, twenty-four bytes later. -/
def orderStateFields : List Field := [
  { name := "state_phase", bytes := 1 },
  { name := "reserved_state", bytes := 7 },
  { name := "state_admitted_slot", bytes := 8 },
  { name := "state_released_slot", bytes := 8 },
  { name := "reserved_state_tail", bytes := 8 }
]

/-- One `(receive, deliver)` row per outcome, interleaved as V1 laid them. -/
def rowStride : Nat := 16

def orderFixedFields : List Field := orderHeaderFields ++ orderStateFields

def orderHeaderBytes : Nat := recordBytes orderHeaderFields
def orderStateOffset : Nat := orderHeaderBytes
def orderRowBase : Nat := recordBytes orderFixedFields
def orderBytes (outcomeCount : Nat) : Nat := orderRowBase + rowStride * outcomeCount

/-- `DCGORD02`. -/
def orderMagic : List Nat := [0x44, 0x43, 0x47, 0x4f, 0x52, 0x44, 0x30, 0x32]
def orderVersion : Nat := 2
def orderPhase : Nat := 21

/-- Side tags. Zero is not a side, so an all-zero shape window -- which is
what a V1 record decoded at V2 offsets would present -- refuses. -/
def sideBuy : Nat := 1
def sideSell : Nat := 2

/-! ## What the layout is -/

theorem header_is_one_hundred_eighty_four_bytes : orderHeaderBytes = 184 := by
  native_decide

theorem state_window_is_thirty_two_bytes : recordBytes orderStateFields = 32 := by
  native_decide

theorem rows_begin_at_two_hundred_sixteen : orderRowBase = 216 := by native_decide

theorem order_width_is_rows_after_the_fixed_prefix (outcomeCount : Nat) :
    orderBytes outcomeCount = 216 + 16 * outcomeCount := by
  simp [orderBytes, rows_begin_at_two_hundred_sixteen, rowStride]

/-- THE V1 HEADER IS A PREFIX. Every field the V1 record had sits at the byte
it always did; the shape is appended, not interleaved. This is the statement
that lets the OpenBatch/PlaceOrder account rules keep their header coordinates
through the re-digest and move only the state window and the row base. -/
theorem the_v1_header_is_a_prefix :
    orderHeaderFields.take orderHeaderV1Fields.length = orderHeaderV1Fields := by
  native_decide

theorem the_v1_header_was_one_hundred_sixty_bytes :
    recordBytes orderHeaderV1Fields = 160 := by native_decide

theorem the_shape_is_the_twenty_four_bytes_between :
    recordBytes shapeFields = 24 := by native_decide

theorem every_field_is_aligned : wellFormed orderFixedFields = true := by native_decide

theorem the_record_begins_with_the_prologue :
    orderFixedFields.take prologueFields.length = prologueFields := by native_decide

/-! ## The shape, and the rows it derives -/

inductive Side where
  | buy
  | sell
  deriving DecidableEq, Repr

def Side.tag : Side → Nat
  | .buy => sideBuy
  | .sell => sideSell

def Side.ofTag? : Nat → Option Side
  | 1 => some .buy
  | 2 => some .sell
  | _ => none

theorem side_tags_round_trip (side : Side) : Side.ofTag? side.tag = some side := by
  cases side <;> rfl

/-- The shape half of a signed order. -/
structure Shape where
  side : Side
  outcomeLo : Nat
  outcomeHi : Nat
  claimsPerLot : Nat
  deriving DecidableEq, Repr

/-- The interval rule of the design note's §1.4: a nonempty inclusive interval
inside the outcome width, moving a positive number of claims per lot. -/
def Shape.isInterval (shape : Shape) (outcomeCount : Nat) : Bool :=
  shape.outcomeLo ≤ shape.outcomeHi && shape.outcomeHi < outcomeCount &&
    0 < shape.claimsPerLot

/-- What cohort-18 admits: the interval is one outcome. A wider interval is
an interval order, and the verifier's minimality conjunct is exact only for
single-outcome books (the dual optimal face of an interval book is a polytope,
not a box). Refused by name at `PlaceOrder` until that conjunct exists. -/
def Shape.isSingleOutcome (shape : Shape) : Bool :=
  shape.outcomeLo = shape.outcomeHi

def Shape.covers (shape : Shape) (outcome : Nat) : Bool :=
  shape.outcomeLo ≤ outcome && outcome ≤ shape.outcomeHi

/-- The rows the shape derives: a buy receives `claimsPerLot` on its interval
and delivers nothing; a sell the reverse. -/
def Shape.row (shape : Shape) (outcome : Nat) : Nat × Nat :=
  if shape.covers outcome then
    match shape.side with
    | .buy => (shape.claimsPerLot, 0)
    | .sell => (0, shape.claimsPerLot)
  else (0, 0)

def rowsOf (shape : Shape) (outcomeCount : Nat) : List (Nat × Nat) :=
  (List.range outcomeCount).map shape.row

/-- Signed claim flow per lot, the `a_o` of the design note. -/
def Shape.flow (shape : Shape) (outcome : Nat) : Int :=
  let (receive, deliver) := shape.row outcome
  (receive : Int) - (deliver : Int)

theorem a_buy_delivers_nothing (shape : Shape) (outcome : Nat) (buy : shape.side = .buy) :
    (shape.row outcome).2 = 0 := by
  unfold Shape.row
  split
  · rw [buy]
  · rfl

theorem a_sell_receives_nothing (shape : Shape) (outcome : Nat) (sell : shape.side = .sell) :
    (shape.row outcome).1 = 0 := by
  unfold Shape.row
  split
  · rw [sell]
  · rfl

theorem no_flow_outside_the_interval
    (shape : Shape) (outcome : Nat) (outside : shape.covers outcome = false) :
    shape.row outcome = (0, 0) := by
  simp [Shape.row, outside]

/-- Exactly one of the pair is nonzero on the interval, and it is the shape's
magnitude: the rows are a function of the shape and nothing else. -/
theorem the_rows_are_the_shape
    (shape : Shape) (outcome : Nat) (inside : shape.covers outcome = true) :
    shape.row outcome = match shape.side with
      | .buy => (shape.claimsPerLot, 0)
      | .sell => (0, shape.claimsPerLot) := by
  simp [Shape.row, inside]

/-! ## The escrow the shape reserves

A buy escrows quote and no claims; a sell escrows claims and no quote. These
are the worst cases `PlaceOrder` moves at admission (decision 0010 §2), stated
on the shape so the physical escrow and the verifier read one author. -/

def quoteReserve (maxQuoteDebitPerLot maxLots : Nat) : Nat :=
  maxQuoteDebitPerLot * maxLots

def claimReserve (shape : Shape) (maxLots outcome : Nat) : Nat :=
  (shape.row outcome).2 * maxLots

theorem a_buy_reserves_no_claims
    (shape : Shape) (maxLots outcome : Nat) (buy : shape.side = .buy) :
    claimReserve shape maxLots outcome = 0 := by
  simp [claimReserve, a_buy_delivers_nothing shape outcome buy]

theorem a_sell_reserves_its_whole_interval
    (shape : Shape) (maxLots outcome : Nat) (sell : shape.side = .sell)
    (inside : shape.covers outcome = true) :
    claimReserve shape maxLots outcome = shape.claimsPerLot * maxLots := by
  simp [claimReserve, the_rows_are_the_shape shape outcome inside, sell]

/-! ## Executable witnesses -/

def buyTwo : Shape := { side := .buy, outcomeLo := 2, outcomeHi := 2, claimsPerLot := 1000 }
def sellInterval : Shape := { side := .sell, outcomeLo := 3, outcomeHi := 5, claimsPerLot := 1 }
def emptyInterval : Shape := { side := .buy, outcomeLo := 4, outcomeHi := 3, claimsPerLot := 1 }
def zeroMagnitude : Shape := { side := .buy, outcomeLo := 0, outcomeHi := 0, claimsPerLot := 0 }

example : buyTwo.isInterval 3 = true ∧ buyTwo.isSingleOutcome = true := by native_decide
example : rowsOf buyTwo 3 = [(0, 0), (0, 0), (1000, 0)] := by native_decide
example : sellInterval.isInterval 6 = true ∧ sellInterval.isSingleOutcome = false := by
  native_decide
example : rowsOf sellInterval 6 = [(0, 0), (0, 0), (0, 0), (0, 1), (0, 1), (0, 1)] := by
  native_decide
/-- HOSTILE: an empty interval and a zero magnitude both fail the shape rule. -/
example : emptyInterval.isInterval 6 = false ∧ zeroMagnitude.isInterval 6 = false := by
  native_decide
/-- HOSTILE: an interval past the width. -/
example : buyTwo.isInterval 2 = false := by native_decide

end DClutch.General.OrderV2
