import Std.Tactic

/-!
# General V2 runtime wire layouts

The successor General vertical persists two fixed-layout records that had no
Lean author: the selection cursor the permissionless fold reads its incumbent
back out of, and the verified-candidate certificate that fold consumes. Every
neighbouring V2/V3 record in this tree is generated; these two were spelled out
twice in Rust, once in a typed `*LayoutV2` projection and once as bare numeric
literals inside the encoder and the hostile decoder.

This module is their single author. Offsets are never written down: a record is
a sequence of named fields with exact byte widths, and `place` walks it. A field
therefore cannot be moved without moving every field after it, which is the
property a wire layout actually needs and the one a table of literals cannot
express.

What this module does NOT own: the semantics of any field. `GeneralClearing`
and `GeneralV5Assurance` own what the selection cursor's key MEANS; this file
owns only where the bytes sit and which tags distinguish the records. The
version tag is shared by the whole V2 record family -- Candidate, Execution,
Page and the settlement cursor also carry it -- so this module authors the tag
while modelling two of the family's records.
-/

namespace DClutch.General.RuntimeWireV2

/-- One fixed field of a runtime wire record. -/
structure Field where
  name : String
  bytes : Nat
  deriving Repr, DecidableEq

/-- Assign offsets by walking the field sequence. Offsets are derived, never
declared, so every field begins exactly where its predecessor ends. -/
def placeFrom : Nat → List Field → List (String × Nat × Nat)
  | _, [] => []
  | cursor, field :: rest =>
      (field.name, cursor, field.bytes) :: placeFrom (cursor + field.bytes) rest

def place (fields : List Field) : List (String × Nat × Nat) := placeFrom 0 fields

/-- Total fixed width of a record. -/
def recordBytes (fields : List Field) : Nat := (fields.map Field.bytes).sum

/-- Where the walk that assigns offsets finishes. -/
def lastEnd : Nat → List Field → Nat
  | cursor, [] => cursor
  | cursor, field :: rest => lastEnd (cursor + field.bytes) rest

theorem last_end_is_record_width (fields : List Field) (cursor : Nat) :
    lastEnd cursor fields = cursor + recordBytes fields := by
  induction fields generalizing cursor with
  | nil => simp [lastEnd, recordBytes]
  | cons field rest induction =>
      simp [lastEnd, recordBytes, induction (cursor + field.bytes)]
      omega

/-- The record is exactly covered: the walk that assigns offsets ends at the
declared width, so there is no gap between fields and no field beyond the end.
This is the statement a table of numeric literals cannot make about itself. -/
theorem placement_covers_the_record (fields : List Field) :
    lastEnd 0 fields = recordBytes fields := by
  simpa using last_end_is_record_width fields 0

/-- Widths this module treats as scalars rather than byte arrays. -/
def scalarWidths : List Nat := [2, 4, 8]

/-- A scalar sits at a multiple of its own width; a byte array only has to be
nonempty. An SBF decoder reads scalars with fixed-width loads, so a misaligned
scalar is a layout defect even where the language would tolerate it. -/
def aligned (placement : String × Nat × Nat) : Bool :=
  if scalarWidths.contains placement.2.2 then placement.2.1 % placement.2.2 == 0
  else 0 < placement.2.2

def wellFormed (fields : List Field) : Bool :=
  (place fields).all aligned && fields.all (fun field => 0 < field.bytes)

/-! ## The prologue every V2 record carries

Six records share the same first twelve bytes: an eight-byte ASCII magic, a
two-byte version, a one-byte record phase, and one canonical zero. Rust had
that prologue in one helper pair -- `header` and `write_header` -- and the
helper spelled 8, 10 and 11 as literals, so the four records that are nothing
BUT that helper plus their own fields had no author for their first twelve
bytes at all. Naming the prologue as a field list and proving every record
begins with it is what makes those three numbers derive. -/

def prologueFields : List Field := [
  { name := "magic", bytes := 8 },
  { name := "version", bytes := 2 },
  { name := "phase", bytes := 1 },
  { name := "reserved", bytes := 1 }
]

/-! ## The permissionless selection cursor -/

def selectionCursorFields : List Field := prologueFields ++ [
  { name := "outcome_count", bytes := 4 },
  { name := "revision", bytes := 8 },
  { name := "submitted_count", bytes := 4 },
  { name := "best_candidate_coordinate", bytes := 4 },
  { name := "best_verified_revision", bytes := 8 },
  { name := "price_scale", bytes := 8 },
  { name := "product_id", bytes := 32 },
  { name := "batch_id", bytes := 32 },
  { name := "policy_id", bytes := 32 },
  { name := "best_candidate_id", bytes := 32 },
  { name := "best_verified_digest", bytes := 32 },
  { name := "best_filled_lots", bytes := 8 },
  { name := "best_quote_surplus", bytes := 8 }
]

/-! ## The verified-candidate certificate -/

def verifiedCandidateHeaderFields : List Field := prologueFields ++ [
  { name := "outcome_count", bytes := 4 },
  { name := "page_count", bytes := 4 },
  { name := "candidate_coordinate", bytes := 4 },
  { name := "revision", bytes := 8 },
  { name := "candidate_id", bytes := 32 },
  { name := "product_id", bytes := 32 },
  { name := "batch_id", bytes := 32 },
  { name := "filled_lots", bytes := 8 },
  { name := "quote_debit", bytes := 8 },
  { name := "quote_credit", bytes := 8 },
  { name := "price_scale", bytes := 8 }
]

/-- One eight-byte cell per outcome. -/
def tailStride : Nat := 8

/-- Two runtime-width tails follow the header: claim inputs, then claim
outputs. -/
def tailCount : Nat := 2

def verifiedCandidateBytes (outcomeCount : Nat) : Nat :=
  recordBytes verifiedCandidateHeaderFields + tailCount * tailStride * outcomeCount


/-! ## The four records the prologue helper serves

`CandidateV2`, `ExecutionV2`, `PageV2` and `SettlementCursorV2` are each the
shared prologue plus their own fields. They were the reason the prologue had no
author: every one of them reached byte 8, 10 and 11 through a helper that spelled
those numbers itself. -/

def candidateFields : List Field := prologueFields ++ [
  { name := "outcome_count", bytes := 4 },
  { name := "page_count", bytes := 4 },
  { name := "candidate_coordinate", bytes := 4 },
  { name := "price_scale", bytes := 8 },
  { name := "candidate_id", bytes := 32 },
  { name := "product_id", bytes := 32 },
  { name := "batch_id", bytes := 32 }
]

def executionFields : List Field := prologueFields ++ [
  { name := "outcome_count", bytes := 4 },
  { name := "page_coordinate", bytes := 4 },
  { name := "execution_coordinate", bytes := 4 },
  { name := "nonce", bytes := 8 },
  { name := "order_id", bytes := 32 },
  { name := "owner_id", bytes := 32 },
  { name := "max_lots", bytes := 8 },
  { name := "lots", bytes := 8 }
]

def pageFields : List Field := prologueFields ++ [
  { name := "outcome_count", bytes := 4 },
  { name := "page_coordinate", bytes := 4 },
  { name := "page_count", bytes := 4 },
  { name := "revision", bytes := 8 },
  { name := "candidate_id", bytes := 32 }
]

def settlementCursorFields : List Field := prologueFields ++ [
  { name := "outcome_count", bytes := 4 },
  { name := "order_count", bytes := 4 },
  { name := "next_order", bytes := 4 },
  { name := "revision", bytes := 8 },
  { name := "candidate_id", bytes := 32 },
  { name := "quote_inventory", bytes := 8 },
  { name := "complete_set_quantity", bytes := 8 },
  { name := "terminal_coordinate", bytes := 8 }
]

/-! ## Record tags -/

/-- `DCGCAN02`. -/
def candidateMagic : List Nat := [0x44, 0x43, 0x47, 0x43, 0x41, 0x4e, 0x30, 0x32]

/-- `DCGEXE02`. -/
def executionMagic : List Nat := [0x44, 0x43, 0x47, 0x45, 0x58, 0x45, 0x30, 0x32]

/-- `DCGPAG02`. -/
def pageMagic : List Nat := [0x44, 0x43, 0x47, 0x50, 0x41, 0x47, 0x30, 0x32]

/-- `DCGSET02`. -/
def settlementCursorMagic : List Nat := [0x44, 0x43, 0x47, 0x53, 0x45, 0x54, 0x30, 0x32]

/-- `DCGSEL02`. -/
def selectionMagic : List Nat := [0x44, 0x43, 0x47, 0x53, 0x45, 0x4c, 0x30, 0x32]

/-- `DCGVER02`. -/
def verifiedMagic : List Nat := [0x44, 0x43, 0x47, 0x56, 0x45, 0x52, 0x30, 0x32]

/-- Every record magic, in the order the records are declared above. -/
def recordMagics : List (List Nat) :=
  [candidateMagic, executionMagic, pageMagic, settlementCursorMagic, selectionMagic,
    verifiedMagic]

def candidatePhase : Nat := 1
def executionPhase : Nat := 2
def pagePhase : Nat := 3

def wireVersion : Nat := 2
def selectionPhaseOpen : Nat := 1
def selectionPhaseFrozen : Nat := 2
def verifiedPhase : Nat := 9

/-! ## What the layouts are -/

theorem selection_cursor_is_two_hundred_twenty_four_bytes :
    recordBytes selectionCursorFields = 224 := by native_decide

theorem verified_candidate_header_is_one_hundred_sixty_bytes :
    recordBytes verifiedCandidateHeaderFields = 160 := by native_decide

theorem verified_candidate_width_is_header_plus_sixteen_per_outcome
    (outcomeCount : Nat) :
    verifiedCandidateBytes outcomeCount = 160 + 16 * outcomeCount := by
  simp [verifiedCandidateBytes, tailCount, tailStride,
    verified_candidate_header_is_one_hundred_sixty_bytes]

/-- Every record, in declaration order. -/
def allRecords : List (List Field) :=
  [candidateFields, executionFields, pageFields, settlementCursorFields,
    selectionCursorFields, verifiedCandidateHeaderFields]

/-- Every scalar in every record is aligned to its own width and no field is
empty. Moving one field by anything that is not a multiple of the next
scalar's width fails here, in Lean, before an emitter can print it. -/
theorem every_record_is_well_formed :
    allRecords.all wellFormed = true := by native_decide

/-- Every record begins with the same twelve-byte prologue. This is the fact
`header` and `write_header` implement for all six and that nothing stated:
their offsets 8, 10 and 11 are the prologue's placements, not three numbers
that happen to agree six times. -/
theorem every_record_begins_with_the_prologue :
    allRecords.all (fun fields => fields.take prologueFields.length == prologueFields) = true := by
  native_decide

/-- The four records the prologue helper serves are exactly as wide as the
Rust constants they replace. -/
theorem the_four_helper_records_have_their_declared_widths :
    recordBytes candidateFields = 128 ∧ recordBytes executionFields = 112 ∧
      recordBytes pageFields = 64 ∧ recordBytes settlementCursorFields = 88 := by
  native_decide

/-- No record can be mistaken for another: all six magics are pairwise
distinct. The phase byte is deliberately NOT globally distinct -- a Candidate's
phase 1 and an open selection cursor's phase 1 are different alphabets -- which
is exactly why the magic, and not the phase, is what identifies a record. -/
theorem record_magics_are_pairwise_distinct :
    recordMagics.eraseDups.length = recordMagics.length := by native_decide

/-- Within each record its phase tags are nonzero and distinct, so a blank
account reads as no phase at all. -/
theorem phase_tags_are_nonzero_and_distinct_within_a_record :
    candidatePhase ≠ 0 ∧ executionPhase ≠ 0 ∧ pagePhase ≠ 0 ∧ verifiedPhase ≠ 0 ∧
      selectionPhaseOpen ≠ selectionPhaseFrozen ∧
      selectionPhaseOpen ≠ 0 ∧ selectionPhaseFrozen ≠ 0 := by
  refine ⟨by decide, by decide, by decide, by decide, by decide, by decide, by decide⟩

/-- Both magics are eight printable ASCII bytes, which is what makes a hostile
account dump readable and what the decoder's fixed eight-byte compare
assumes. -/
theorem magics_are_eight_printable_bytes :
    recordMagics.all (fun magic =>
      (magic.length == 8) &&
        magic.all (fun byte => decide (0x20 ≤ byte && byte ≤ 0x7e))) = true := by
  native_decide

/-- The exact placement of both records, pinned in order. This is the table the
Rust used to hold twice; it is derived here and emitted once. -/
theorem runtime_wire_placements_are_exact :
    (place selectionCursorFields).map (fun entry => (entry.1, entry.2.1)) =
      [("magic", 0), ("version", 8), ("phase", 10), ("reserved", 11),
       ("outcome_count", 12), ("revision", 16), ("submitted_count", 24),
       ("best_candidate_coordinate", 28), ("best_verified_revision", 32),
       ("price_scale", 40), ("product_id", 48), ("batch_id", 80),
       ("policy_id", 112), ("best_candidate_id", 144),
       ("best_verified_digest", 176), ("best_filled_lots", 208),
       ("best_quote_surplus", 216)] ∧
    (place verifiedCandidateHeaderFields).map (fun entry => (entry.1, entry.2.1)) =
      [("magic", 0), ("version", 8), ("phase", 10), ("reserved", 11),
       ("outcome_count", 12), ("page_count", 16), ("candidate_coordinate", 20),
       ("revision", 24), ("candidate_id", 32), ("product_id", 64),
       ("batch_id", 96), ("filled_lots", 128), ("quote_debit", 136),
       ("quote_credit", 144), ("price_scale", 152)] := by
  native_decide

end DClutch.General.RuntimeWireV2
