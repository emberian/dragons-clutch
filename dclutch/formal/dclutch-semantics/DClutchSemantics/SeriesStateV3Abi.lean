import DClutchSemantics.AbiCoverage

/-!
# The Series root tail, and the phase it persists

Sixty-four bytes of Trading-owned account after the 232-byte capability-root
header: the Series lifecycle phase, whether the current occurrence has its
prepared Ticket, the next occurrence, the outstanding ticket accounts, the
replay revision and the close-rent principal.  Until this module the layout
had one author -- `crates/dclutch-trading/src/series/replay.rs`, which wrote
`output[12]`, `output[16..20]`, `all_zero(bytes, 40, 24)` and their decode
twins as bare literals, twice each -- and the SDK's state-machine generator,
which recovers a machine's tag coordinate by matching Rust expressions, could
not see a root phase machine at all: `series-ticket` is the only Series row it
has, and its phase-byte expression matched THIS record's `encode` first.

What is deliberately not here: the magic (`SERIES_STATE_MAGIC_V3` is
`EmitSeriesOccurrenceV3Rust.lean`'s) and the schema and profile VALUES (the
family's `SERIES_TEMPLATE_SCHEMA_V3` / `_PROFILE_V3`).  What belongs to this
record is the coordinate of every word, its width, and the two phase tags.
-/

namespace DClutch.SeriesStateV3Abi

open DClutch.AbiSchema

/-- The two phases a Series root moves through. -/
inductive Phase where
  | active | terminal
  deriving DecidableEq, Repr

namespace Phase

def all : List Phase := [.active, .terminal]

/-- The wire tag persisted in the phase byte. -/
def tag : Phase → Nat
  | .active => 0
  | .terminal => 1

def rustName : Phase → String
  | .active => "SERIES_PHASE_ACTIVE_V3"
  | .terminal => "SERIES_PHASE_TERMINAL_V3"

def doc : Phase → String
  | .active => "One scheduled occurrence remains to be settled."
  | .terminal => "Every occurrence settled; only terminal ticket retirement remains."

end Phase

/-- One past the greatest tag: every phase indexes its own bit of a `u8`. -/
def phaseLimit : Nat := 2

inductive Field where
  | magic | schemaVersion | profile | phase | currentTicketPrepared | headReserved
  | nextOccurrence | outstandingTicketAccounts | revision | closeRentRemaining | tailReserved
  deriving DecidableEq, Repr

/-- The header every Series V3 record shares -- magic, schema, profile -- then
this record's phase byte, its prepared flag, and the canonical-zero pad. -/
def header : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩, ⟨.schemaVersion, .u16⟩, ⟨.profile, .u16⟩,
  ⟨.phase, .u8⟩, ⟨.currentTicketPrepared, .u8⟩, ⟨.headReserved, .reserved 2⟩
]

/-- The replay body: the occurrence cursor, the live ticket-account count, the
revision every act checks, the close-rent principal, and the tail span. -/
def body : List (FieldSpec Field) := [
  ⟨.nextOccurrence, .u32⟩, ⟨.outstandingTicketAccounts, .u32⟩,
  ⟨.revision, .u64⟩, ⟨.closeRentRemaining, .u64⟩, ⟨.tailReserved, .reserved 24⟩
]

def schema : List (FieldSpec Field) := header ++ body

def layout : List (PlacedField Field) := specialize schema
def seriesStateBytes : Nat := schemaWidth schema

/-- Where the replay body begins: the width of the header in front of it. -/
def bodyOffset : Nat := schemaWidth header

namespace Field

def all : List Field := [
  .magic, .schemaVersion, .profile, .phase, .currentTicketPrepared, .headReserved,
  .nextOccurrence, .outstandingTicketAccounts, .revision, .closeRentRemaining, .tailReserved
]

def rustName : Field → String
  | .magic => "SERIES_STATE_MAGIC_OFFSET_V3"
  | .schemaVersion => "SERIES_STATE_SCHEMA_OFFSET_V3"
  | .profile => "SERIES_STATE_PROFILE_OFFSET_V3"
  | .phase => "SERIES_STATE_PHASE_OFFSET_V3"
  | .currentTicketPrepared => "SERIES_STATE_CURRENT_TICKET_PREPARED_OFFSET_V3"
  | .headReserved => "SERIES_STATE_HEAD_RESERVED_OFFSET_V3"
  | .nextOccurrence => "SERIES_STATE_NEXT_OCCURRENCE_OFFSET_V3"
  | .outstandingTicketAccounts => "SERIES_STATE_OUTSTANDING_TICKET_ACCOUNTS_OFFSET_V3"
  | .revision => "SERIES_STATE_REVISION_OFFSET_V3"
  | .closeRentRemaining => "SERIES_STATE_CLOSE_RENT_REMAINING_OFFSET_V3"
  | .tailReserved => "SERIES_STATE_TAIL_RESERVED_OFFSET_V3"

def doc : Field → String
  | .magic => "Canonical Series root-tail magic."
  | .schemaVersion => "Family-wide `SERIES_TEMPLATE_SCHEMA_V3`, at this record's coordinate."
  | .profile => "Family-wide `SERIES_TEMPLATE_PROFILE_V3`, at this record's coordinate."
  | .phase => "The persisted `SeriesPhaseV3` wire tag."
  | .currentTicketPrepared => "Whether the current occurrence already owns its prepared Ticket (0 or 1)."
  | .headReserved => "Canonical-zero span padding the prepared flag to the occurrence cursor."
  | .nextOccurrence => "Next occurrence that may be prepared or settled."
  | .outstandingTicketAccounts => "Number of live terminal or prepared ticket accounts."
  | .revision => "Current replay revision."
  | .closeRentRemaining => "Separately classified close-rent principal."
  | .tailReserved => "Canonical-zero tail span."

def coordinate (field : Field) : Nat × Nat :=
  (coordinate? field layout).getD (0, 0)

def offset (field : Field) : Nat := (coordinate field).1
def width (field : Field) : Nat := (coordinate field).2

end Field

def isReserved : FieldKind → Bool
  | .reserved _ => true
  | _ => false

/-! ## What the layout says -/

theorem schema_well_formed : WellFormed schema := by
  constructor
  · native_decide
  · native_decide

theorem layout_disjoint : layout.Pairwise Before :=
  specializeFrom_pairwise 0 schema

/-- Eleven fields cover the sixty-four bytes every reader allocates. -/
theorem layout_covers_its_declared_width :
    seriesStateBytes = 64 ∧ tiles 0 layout 64 = true := by
  native_decide

/-- Every coordinate `replay.rs` wrote as a literal, placed. -/
theorem coordinates_are_canonical : coordinates layout = [
    (.magic, 0, 8), (.schemaVersion, 8, 2), (.profile, 10, 2),
    (.phase, 12, 1), (.currentTicketPrepared, 13, 1), (.headReserved, 14, 2),
    (.nextOccurrence, 16, 4), (.outstandingTicketAccounts, 20, 4),
    (.revision, 24, 8), (.closeRentRemaining, 32, 8), (.tailReserved, 40, 24)
  ] := by
  native_decide

/-- The two canonical-zero spans are exactly the two `all_zero` calls `decode`
makes: `(14, 2)` and `(40, 24)`. -/
theorem the_two_reserved_spans_are_the_ones_decode_enforces :
    schema.filter (fun field => isReserved field.kind) =
      [⟨.headReserved, .reserved 2⟩, ⟨.tailReserved, .reserved 24⟩] ∧
      Field.offset .headReserved = 14 ∧ Field.width .headReserved = 2 ∧
      Field.offset .tailReserved = 40 ∧ Field.width .tailReserved = 24 := by
  native_decide

/-- The head span pads the prepared flag to the occurrence cursor, and the body
begins there. -/
theorem the_head_span_pads_the_flag_to_the_cursor :
    Field.offset .headReserved = Field.offset .currentTicketPrepared + 1 ∧
      Field.offset .headReserved + Field.width .headReserved =
        Field.offset .nextOccurrence ∧
      bodyOffset = Field.offset .nextOccurrence := by
  native_decide

/-- The phase byte sits where the ticket state's does -- both records open with
the same three header words -- which is exactly why the SDK generator's regex
matched the wrong record. -/
theorem the_phase_follows_the_two_header_words :
    Field.offset .phase = Field.offset .profile + Field.width .profile ∧
      Field.width .phase = 1 ∧ Field.offset .phase = 12 := by
  native_decide

/-- The two tags are distinct bit indices of a `u8`. -/
theorem the_tags_are_distinct_bit_indices :
    (Phase.all.map Phase.tag) = [0, 1] ∧ (Phase.all.map Phase.tag).Nodup ∧
      Phase.all.all (fun phase => Phase.tag phase < phaseLimit) = true ∧
      phaseLimit ≤ 8 := by
  native_decide

theorem rust_names_are_distinct : (Field.all.map Field.rustName).Nodup := by
  native_decide

theorem phase_rust_names_are_distinct : (Phase.all.map Phase.rustName).Nodup := by
  native_decide

theorem every_placed_field_is_named :
    Field.all = schema.map (fun field => field.name) := by native_decide

end DClutch.SeriesStateV3Abi
