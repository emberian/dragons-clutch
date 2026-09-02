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

/-! ## The permissionless selection cursor -/

def selectionCursorFields : List Field := [
  { name := "magic", bytes := 8 },
  { name := "version", bytes := 2 },
  { name := "phase", bytes := 1 },
  { name := "reserved", bytes := 1 },
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

def verifiedCandidateHeaderFields : List Field := [
  { name := "magic", bytes := 8 },
  { name := "version", bytes := 2 },
  { name := "phase", bytes := 1 },
  { name := "reserved", bytes := 1 },
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

/-! ## Record tags -/

/-- `DCGSEL02`. -/
def selectionMagic : List Nat := [0x44, 0x43, 0x47, 0x53, 0x45, 0x4c, 0x30, 0x32]

/-- `DCGVER02`. -/
def verifiedMagic : List Nat := [0x44, 0x43, 0x47, 0x56, 0x45, 0x52, 0x30, 0x32]

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

/-- Every scalar in both records is aligned to its own width and no field is
empty. Moving one field by anything that is not a multiple of the next
scalar's width fails here, in Lean, before an emitter can print it. -/
theorem both_records_are_well_formed :
    wellFormed selectionCursorFields = true ∧
      wellFormed verifiedCandidateHeaderFields = true := by native_decide

/-- The two records cannot be mistaken for one another, and neither phase tag
is the zero a blank account carries. -/
theorem record_tags_separate_the_two_records :
    selectionMagic ≠ verifiedMagic ∧
      selectionPhaseOpen ≠ selectionPhaseFrozen ∧
      selectionPhaseOpen ≠ 0 ∧ selectionPhaseFrozen ≠ 0 ∧
      verifiedPhase ≠ selectionPhaseOpen ∧ verifiedPhase ≠ selectionPhaseFrozen := by
  refine ⟨by decide, by decide, by decide, by decide, by decide, by decide⟩

/-- Both magics are eight printable ASCII bytes, which is what makes a hostile
account dump readable and what the decoder's fixed eight-byte compare
assumes. -/
theorem magics_are_eight_printable_bytes :
    selectionMagic.length = 8 ∧ verifiedMagic.length = 8 ∧
      selectionMagic.all (fun byte => decide (0x20 ≤ byte && byte ≤ 0x7e)) = true ∧
      verifiedMagic.all (fun byte => decide (0x20 ≤ byte && byte ≤ 0x7e)) = true := by
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
