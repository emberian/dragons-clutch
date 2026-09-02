import DClutchSemantics.AbiCoverage

/-!
# The WindowSpecV1 preimage

The window a market settles on: which source specification, over which closed
time interval, with which staleness and skew bounds, on which schedule, and
with how much cadence tolerance.  One hundred and twelve bytes.

Until this module existed the record had TWO authors and neither could see the
other.  `crates/dclutch-source-contract/src/lib.rs` wrote the magic, the width
and the two time coordinates as literals -- `112`, `*b"DCLTWIN1"`, `48`, `56` --
and left the six coordinates between them as bare arguments inside `decode` and
`to_bytes`.  `SourceScheduledMedianV1` owned the other end, and owned it by
ASSERTION: `windowSpecBytes := 112` and `windowSpecTailOffset := 104` were bare
Lean literals, and the tail schema was specialized from cursor 104 because
someone typed 104.  Nothing related that cursor to the ninety-six bytes in front
of it, in either language.  A record whose first author writes its width and
whose second author writes an offset into it, with no shared object, is two
records that happen to agree.

So the tail's placement is the fact worth recovering.  `windowSpecTailOffset` is
now the offset of `cadenceToleranceSeconds` in THIS schema, which is to say it
is the sum of the eleven fields before it, and `tail_fits_former_reserved` --
which used to compare two literals -- now says that the tail the scheduled
median owns ends exactly where the record does.

The version VALUE is deliberately not here.  `SCHEMA_VERSION` is crate-wide and
five preimages share it, so it has an owner already; what belongs to this record
is the COORDINATE the version is written at, which is a placement like any
other.
-/

namespace DClutch.SourceWindowSpecV1Abi

open DClutch.AbiSchema

/-- `DCLTWIN1`. -/
def magic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x57, 0x49, 0x4e, 0x31]

inductive Field where
  | magic | schemaVersion | kind | headerReserved | sourceSpecId
  | startUnixSeconds | endUnixSeconds | maxAgeSeconds | maxFutureSkewSeconds
  | scheduleId | cadenceToleranceSeconds | tailReserved
  deriving DecidableEq, Repr

/-- The eleven fields before the cadence tolerance.  Named as its own list
because the tolerance's coordinate IS this list's width, which is the fact the
two old authors could not state. -/
def beforeTail : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩, ⟨.schemaVersion, .u16⟩, ⟨.kind, .u8⟩,
  ⟨.headerReserved, .reserved 5⟩, ⟨.sourceSpecId, .bytes 32⟩,
  ⟨.startUnixSeconds, .u64⟩, ⟨.endUnixSeconds, .u64⟩,
  ⟨.maxAgeSeconds, .u32⟩, ⟨.maxFutureSkewSeconds, .u32⟩,
  ⟨.scheduleId, .bytes 32⟩
]

/-- The tail `SourceScheduledMedianV1` owns: the tolerance and the four bytes
of the former reserved span it did not take. -/
def tail : List (FieldSpec Field) := [
  ⟨.cadenceToleranceSeconds, .u32⟩, ⟨.tailReserved, .reserved 4⟩
]

def schema : List (FieldSpec Field) := beforeTail ++ tail

def layout : List (PlacedField Field) := specialize schema
def windowSpecBytes : Nat := schemaWidth schema

/-- Where the tail begins: the width of everything in front of it, never a
number anybody types. -/
def tailOffset : Nat := schemaWidth beforeTail
def tailBytes : Nat := schemaWidth tail

namespace Field

def all : List Field := [
  .magic, .schemaVersion, .kind, .headerReserved, .sourceSpecId,
  .startUnixSeconds, .endUnixSeconds, .maxAgeSeconds, .maxFutureSkewSeconds,
  .scheduleId, .cadenceToleranceSeconds, .tailReserved
]

/-- The two names that already existed are preserved exactly; nothing here
moves, so nothing here is renamed. -/
def rustName : Field → String
  | .magic => "WINDOW_SPEC_MAGIC_OFFSET_V1"
  | .schemaVersion => "WINDOW_SPEC_SCHEMA_VERSION_OFFSET_V1"
  | .kind => "WINDOW_SPEC_KIND_OFFSET_V1"
  | .headerReserved => "WINDOW_SPEC_HEADER_RESERVED_OFFSET_V1"
  | .sourceSpecId => "WINDOW_SPEC_SOURCE_SPEC_ID_OFFSET_V1"
  | .startUnixSeconds => "WINDOW_SPEC_START_UNIX_SECONDS_OFFSET_V1"
  | .endUnixSeconds => "WINDOW_SPEC_END_UNIX_SECONDS_OFFSET_V1"
  | .maxAgeSeconds => "WINDOW_SPEC_MAX_AGE_SECONDS_OFFSET_V1"
  | .maxFutureSkewSeconds => "WINDOW_SPEC_MAX_FUTURE_SKEW_SECONDS_OFFSET_V1"
  | .scheduleId => "WINDOW_SPEC_SCHEDULE_ID_OFFSET_V1"
  | .cadenceToleranceSeconds => "WINDOW_SPEC_CADENCE_TOLERANCE_OFFSET_V1"
  | .tailReserved => "WINDOW_SPEC_CADENCE_TOLERANCE_TAIL_RESERVED_OFFSET_V1"

def doc : Field → String
  | .magic => "Canonical window-specification magic."
  | .schemaVersion => "Crate-wide `SCHEMA_VERSION`, at this record's coordinate."
  | .kind => "Window kind tag."
  | .headerReserved => "Canonical-zero span between the kind tag and the identity."
  | .sourceSpecId => "Identity of the source specification the window reads."
  | .startUnixSeconds => "Closed lower time bound of the window."
  | .endUnixSeconds => "Closed upper time bound of the window."
  | .maxAgeSeconds => "Maximum admissible sample staleness."
  | .maxFutureSkewSeconds => "Maximum admissible sample skew into the future."
  | .scheduleId => "Identity of the schedule the samples must land on."
  | .cadenceToleranceSeconds => "Admission half-width around each scheduled position."
  | .tailReserved => "Canonical-zero remainder of the former reserved tail."

def coordinate (field : Field) : Nat × Nat :=
  (coordinate? field layout).getD (0, 0)

def offset (field : Field) : Nat := (coordinate field).1
def width (field : Field) : Nat := (coordinate field).2

end Field

/-! ## What the layout says -/

theorem schema_well_formed : WellFormed schema := by
  constructor
  · native_decide
  · native_decide

theorem layout_disjoint : layout.Pairwise Before :=
  specializeFrom_pairwise 0 schema

/-- The twelve fields cover the 112 bytes every reader allocates: no gap, and
the last field ends exactly at the declared width.  Two reserved spans in one
record is two places a gap could have hidden. -/
theorem layout_covers_its_declared_width :
    windowSpecBytes = 112 ∧ tiles 0 layout 112 = true := by
  native_decide

/-- Every coordinate, including the six that were bare arguments inside
`decode` and `to_bytes` and had no name in any language. -/
theorem coordinates_are_canonical : coordinates layout = [
    (.magic, 0, 8), (.schemaVersion, 8, 2), (.kind, 10, 1),
    (.headerReserved, 11, 5), (.sourceSpecId, 16, 32),
    (.startUnixSeconds, 48, 8), (.endUnixSeconds, 56, 8),
    (.maxAgeSeconds, 64, 4), (.maxFutureSkewSeconds, 68, 4),
    (.scheduleId, 72, 32), (.cadenceToleranceSeconds, 104, 4),
    (.tailReserved, 108, 4)
  ] := by
  native_decide

/-- The tail begins at the width of everything before it.  `104` was a literal
in Lean and a literal in Rust and the two were unrelated; it is a sum now. -/
theorem the_tail_begins_where_the_head_ends :
    tailOffset = Field.offset .cadenceToleranceSeconds ∧
      tailOffset = 104 ∧ tailBytes = 8 ∧
      tailOffset + tailBytes = windowSpecBytes := by
  native_decide

/-- The window is an interval, so its two bounds are adjacent and equally
wide.  The Rust wrote `48` and `56` in four places between `decode` and
`to_bytes` and never said the second followed the first. -/
theorem the_bounds_are_adjacent_and_equal :
    Field.offset .endUnixSeconds =
        Field.offset .startUnixSeconds + Field.width .startUnixSeconds ∧
      Field.width .startUnixSeconds = Field.width .endUnixSeconds := by
  native_decide

/-- Both identity coordinates are full-width content ids. -/
theorem the_identities_are_content_ids :
    Field.width .sourceSpecId = 32 ∧ Field.width .scheduleId = 32 := by
  native_decide

theorem magic_is_eight_bytes : magic.length = 8 := by native_decide

theorem magic_fills_its_field : magic.length = Field.width .magic := by
  native_decide

theorem rust_names_are_distinct : (Field.all.map Field.rustName).Nodup := by
  native_decide

theorem every_placed_field_is_named :
    Field.all = schema.map (fun field => field.name) := by native_decide

end DClutch.SourceWindowSpecV1Abi
