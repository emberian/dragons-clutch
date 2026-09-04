import DClutchSemantics.AbiCoverage

/-!
# The occurrence-ticket replay state, and the three tags it persists

Sixty-four bytes of Trading-owned account holding one occurrence ticket's
replay phase, its revision and the ticket record it settles.  `TicketPhaseV3`
is the eighth machine the route census gates on, and until this module it was
one of FOUR with no Lean owner at all: `crates/dclutch-series-v3-kernel/src/replay.rs`
authored the three discriminants, and `packages/dclutch-sdk/scripts/generate-state-machines-v1.mjs`
scraped them back out of the decoder's match arms because there was nothing
else to read.

## What the scrape could not see, and this module states

The generator could not find a Rust constant naming the phase byte's
coordinate, because there is none: `encode` writes `output[12]` and `decode`
reads `read_u8(bytes, 12)`, both bare.  So it took the offset from the two
expressions AGREEING -- the strongest thing available to a reader of Rust
text, and still not a statement anybody could check, because **the encode
expression it matched belongs to a different record**.  `SeriesStateV3::encode`
writes `output[12] = self.phase as u8;` at line 151 and `TicketStateV3::encode`
writes the identical line at 397; a first-match regex reads the former.  The
two agree today, which is exactly why nothing noticed.  Here the coordinate is
`Field.offset .phase`, placed by `specialize` behind the two header words, and
it belongs to this record and no other.

## Why `Prepared = 0` is worth saying out loud

A `LifecycleBound` account arrives as sixty-four zeros, and
`SERIES_PREPARE_TICKET_COORDINATE_V5` declares exactly that width with write
permission and never writes it -- the producer-missing debt `replay.rs` records
at length.  Zero is `Prepared`'s wire tag.  So the phase byte alone cannot
separate a fresh account from a prepared ticket, and the magic is the whole
partition; `the_prepared_tag_is_zero_so_the_magic_is_the_partition` is that
statement rather than a remark.

## What is deliberately not here

The magic.  `SERIES_TICKET_STATE_MAGIC_V3` has been Lean-emitted since
`EmitSeriesOccurrenceV3Rust.lean` and belongs to that module; re-emitting it
here would be a second author for one fact.  Same for the schema and profile
VALUES, which are the family's and are already `SERIES_TEMPLATE_SCHEMA_V3` and
`SERIES_TEMPLATE_PROFILE_V3`.  What belongs to this record is the COORDINATES
those words are written at, and its own width.
-/

namespace DClutch.SeriesTicketStateV3Abi

open DClutch.AbiSchema

/-- The three phases one occurrence ticket's replay state moves through. -/
inductive Phase where
  | prepared | consumed | expired
  deriving DecidableEq, Repr

namespace Phase

def all : List Phase := [.prepared, .consumed, .expired]

/-- The wire tag persisted in the phase byte. -/
def tag : Phase → Nat
  | .prepared => 0
  | .consumed => 1
  | .expired => 2

def rustName : Phase → String
  | .prepared => "SERIES_TICKET_PHASE_PREPARED_V3"
  | .consumed => "SERIES_TICKET_PHASE_CONSUMED_V3"
  | .expired => "SERIES_TICKET_PHASE_EXPIRED_V3"

def doc : Phase → String
  | .prepared =>
      "Exact custody is prepared and the occurrence remains retryable."
  | .consumed => "The ticket was atomically consumed into its exact Found Market."
  | .expired => "The retry window elapsed and every compartment was refunded."

/-- No economic retry remains possible.  `TicketPhaseV3::terminal` is the
complement of `prepared`, and four callers read it as a predicate. -/
def terminal : Phase → Bool
  | .prepared => false
  | .consumed | .expired => true

end Phase

/-- One past the greatest tag, which is the bound `ticket_admission_v1.rs`
writes as `STATE_COUNT` and asserts fits a `u8` bitset. -/
def phaseLimit : Nat := 3

inductive Field where
  | magic | schemaVersion | profile | phase | headReserved
  | revision | ticketRecordId | tailReserved
  deriving DecidableEq, Repr

/-- The header every Series V3 record shares -- magic, schema, profile -- then
this record's phase byte and the canonical-zero span that pads it to eight. -/
def header : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩, ⟨.schemaVersion, .u16⟩, ⟨.profile, .u16⟩,
  ⟨.phase, .u8⟩, ⟨.headReserved, .reserved 3⟩
]

/-- The replay body: the revision the settlement checks and the ticket record
it settles, then the tail span. -/
def body : List (FieldSpec Field) := [
  ⟨.revision, .u64⟩, ⟨.ticketRecordId, .bytes 32⟩, ⟨.tailReserved, .reserved 8⟩
]

def schema : List (FieldSpec Field) := header ++ body

def layout : List (PlacedField Field) := specialize schema
def ticketStateBytes : Nat := schemaWidth schema

/-- Where the replay body begins: the width of the shared header in front of
it, never a number anybody types. -/
def bodyOffset : Nat := schemaWidth header

namespace Field

def all : List Field := [
  .magic, .schemaVersion, .profile, .phase, .headReserved,
  .revision, .ticketRecordId, .tailReserved
]

def rustName : Field → String
  | .magic => "SERIES_TICKET_STATE_MAGIC_OFFSET_V3"
  | .schemaVersion => "SERIES_TICKET_STATE_SCHEMA_OFFSET_V3"
  | .profile => "SERIES_TICKET_STATE_PROFILE_OFFSET_V3"
  | .phase => "SERIES_TICKET_STATE_PHASE_OFFSET_V3"
  | .headReserved => "SERIES_TICKET_STATE_HEAD_RESERVED_OFFSET_V3"
  | .revision => "SERIES_TICKET_STATE_REVISION_OFFSET_V3"
  | .ticketRecordId => "SERIES_TICKET_STATE_RECORD_ID_OFFSET_V3"
  | .tailReserved => "SERIES_TICKET_STATE_TAIL_RESERVED_OFFSET_V3"

def doc : Field → String
  | .magic => "Canonical ticket-state magic, the only partition against a zeroed account."
  | .schemaVersion => "Family-wide `SERIES_TEMPLATE_SCHEMA_V3`, at this record's coordinate."
  | .profile => "Family-wide `SERIES_TEMPLATE_PROFILE_V3`, at this record's coordinate."
  | .phase => "The persisted `TicketPhaseV3` wire tag."
  | .headReserved => "Canonical-zero span padding the phase byte to the revision."
  | .revision => "Replay revision the settlement transition checks."
  | .ticketRecordId => "Identity of the ticket record this state settles."
  | .tailReserved => "Canonical-zero tail span."

def coordinate (field : Field) : Nat × Nat :=
  (coordinate? field layout).getD (0, 0)

def offset (field : Field) : Nat := (coordinate field).1
def width (field : Field) : Nat := (coordinate field).2

end Field

/-- Physical predicate a schema-level statement can be made about. -/
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

/-- The eight fields cover the sixty-four bytes every reader allocates: no gap,
and the last field ends exactly at the declared width. -/
theorem layout_covers_its_declared_width :
    ticketStateBytes = 64 ∧ tiles 0 layout 64 = true := by
  native_decide

/-- Every coordinate, including the phase byte the SDK generator had to infer
from two agreeing bare expressions. -/
theorem coordinates_are_canonical : coordinates layout = [
    (.magic, 0, 8), (.schemaVersion, 8, 2), (.profile, 10, 2),
    (.phase, 12, 1), (.headReserved, 13, 3),
    (.revision, 16, 8), (.ticketRecordId, 24, 32), (.tailReserved, 56, 8)
  ] := by
  native_decide

/-- **The coordinate the Rust never named.**  The phase byte begins exactly
where the profile word ends and is exactly one byte wide, so `12` is a
placement rather than a literal that happened to be written twice -- and, as it
turned out, twice in two different records. -/
theorem the_phase_follows_the_two_header_words :
    Field.offset .phase = Field.offset .profile + Field.width .profile ∧
      Field.width .phase = 1 ∧
      Field.offset .phase = 12 := by
  native_decide

/-- The two canonical-zero spans, which are exactly the two `decode` refuses a
nonzero byte in: `(13, 3)` and `(56, 8)`.  They were the arguments of two
`all_zero` calls and had no name in any language. -/
theorem the_two_reserved_spans_are_the_ones_decode_enforces :
    schema.filter (fun field => isReserved field.kind) =
      [⟨.headReserved, .reserved 3⟩, ⟨.tailReserved, .reserved 8⟩] ∧
      Field.offset .headReserved = 13 ∧ Field.width .headReserved = 3 ∧
      Field.offset .tailReserved = 56 ∧ Field.width .tailReserved = 8 := by
  native_decide

/-- The head span pads the phase byte to the revision's coordinate, so the
record has no unowned byte between them. -/
theorem the_head_span_pads_the_phase_to_the_revision :
    Field.offset .headReserved = Field.offset .phase + Field.width .phase ∧
      Field.offset .headReserved + Field.width .headReserved =
        Field.offset .revision ∧
      bodyOffset = Field.offset .revision := by
  native_decide

/-- The three tags are distinct and every one of them indexes its own bit of
the `u8` bitset `SeriesTicketAdmissionV1` is.  This is what
`ticket_admission_v1.rs` writes as `STATE_COUNT <= 8` plus one `assert!` on the
last variant; here it is the whole enumeration. -/
theorem the_tags_are_distinct_bit_indices :
    (Phase.all.map Phase.tag) = [0, 1, 2] ∧
      (Phase.all.map Phase.tag).Nodup ∧
      Phase.all.all (fun phase => Phase.tag phase < phaseLimit) = true ∧
      phaseLimit ≤ 8 := by
  native_decide

/-- **Zero is `Prepared`.**  A `LifecycleBound` account presents as sixty-four
zeros, so the phase byte of an unwritten account decodes as a prepared ticket
and the magic is the only thing that refuses it.  That is why `decode` checks
the magic first and why the producer-missing debt in `replay.rs` is a refusal
rather than a corruption. -/
theorem the_prepared_tag_is_zero_so_the_magic_is_the_partition :
    Phase.tag .prepared = 0 ∧ Field.offset .magic = 0 ∧
      Field.width .magic = 8 := by
  native_decide

/-- `terminal` is exactly the complement of `prepared`, which is the predicate
`settle` requires of its target phase. -/
theorem terminal_is_the_complement_of_prepared :
    Phase.all.filter Phase.terminal = [.consumed, .expired] := by
  native_decide

theorem rust_names_are_distinct : (Field.all.map Field.rustName).Nodup := by
  native_decide

theorem phase_rust_names_are_distinct : (Phase.all.map Phase.rustName).Nodup := by
  native_decide

theorem every_placed_field_is_named :
    Field.all = schema.map (fun field => field.name) := by native_decide

end DClutch.SeriesTicketStateV3Abi
