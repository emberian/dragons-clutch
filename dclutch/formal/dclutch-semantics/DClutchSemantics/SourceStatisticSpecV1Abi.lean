import DClutchSemantics.AbiCoverage

/-!
# The StatisticSpecV1 preimage

The record that relates a source unit to a result unit: which two units, by
what decimal shift, under which statistic family and rounding boundary, over
how many samples, against which threshold, capacity profile and evaluator
release.  One hundred and seventy-six bytes.

This module exists because of what the record's twelve coordinates cost when
nobody owned them.  `crates/dclutch-source-contract/src/lib.rs` wrote every one
of them as a bare argument inside `decode` and `to_bytes` -- `16`, `48`, `80`,
`96`, `112`, `144`, and two reserved spans at `(12, 4)` and `(82, 14)` -- while
its sibling `WindowSpecV1` had been Lean-owned since
`SourceWindowSpecV1Abi.lean`.  The asymmetry was not academic.  On 2026-09-03
`4cd2b9cb5` put `source_scale_exponent` into the first of those two reserved
spans, closing the defect that mis-paid cohort-14 market B, and the field
landed **at an offset nothing emitted**: the number that decides which cell a
market pays sat in a hand-written layout, four bytes chosen by reading a
`zero(bytes, 12, 4)` call and trusting it.

So the fact worth recovering is the one the migration rests on.  The factor
occupies exactly the span that was reserved and enforced zero, which is what
makes every statistic founded before it decode at the identity and re-encode
byte-for-byte -- `the_factor_fills_the_span_that_was_reserved` says the shift
begins where the rounding tag ends and ends where the first identity begins,
so it can neither have taken a byte from a field nor left a gap behind.  A
record that had two reserved spans now has one, and
`exactly_one_reserved_span_remains` is that statement rather than a comment.

The version VALUE is deliberately not here, for the same reason it is not in
the window's module: `SCHEMA_VERSION` is crate-wide and five preimages share
it.  What belongs to this record is the COORDINATE the version is written at.
-/

namespace DClutch.SourceStatisticSpecV1Abi

open DClutch.AbiSchema

/-- `DCLTSTA1`. -/
def magic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x53, 0x54, 0x41, 0x31]

inductive Field where
  | magic | schemaVersion | kind | rounding | sourceScaleExponent
  | sourceUnitId | resultUnitId | requiredSamples | bodyReserved
  | thresholdAtoms | capacityProfileId | evaluatorReleaseId
  deriving DecidableEq, Repr

/-- The header: the two coordinates every Source preimage shares, then this
record's two one-byte tags, then the shift.  The shift is *in* the header
because that is physically where the reserved span it replaced was, and the
migration statement is exactly that placement. -/
def header : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩, ⟨.schemaVersion, .u16⟩,
  ⟨.kind, .u8⟩, ⟨.rounding, .u8⟩,
  ⟨.sourceScaleExponent, .u32⟩
]

/-- The two unit identities the shift relates, adjacent and equally wide
because they are one pair and not two unrelated names. -/
def units : List (FieldSpec Field) := [
  ⟨.sourceUnitId, .bytes 32⟩, ⟨.resultUnitId, .bytes 32⟩
]

/-- The statistic's own configuration and the two releases it is evaluated
under.  The reserved span here is the one this record still has. -/
def body : List (FieldSpec Field) := [
  ⟨.requiredSamples, .u16⟩, ⟨.bodyReserved, .reserved 14⟩,
  ⟨.thresholdAtoms, .bytes 16⟩,
  ⟨.capacityProfileId, .bytes 32⟩, ⟨.evaluatorReleaseId, .bytes 32⟩
]

def schema : List (FieldSpec Field) := header ++ units ++ body

def layout : List (PlacedField Field) := specialize schema
def statisticSpecBytes : Nat := schemaWidth schema

/-- Where the two unit identities begin: the width of the header in front of
them, never a number anybody types. -/
def unitsOffset : Nat := schemaWidth header
def unitsBytes : Nat := schemaWidth units

namespace Field

def all : List Field := [
  .magic, .schemaVersion, .kind, .rounding, .sourceScaleExponent,
  .sourceUnitId, .resultUnitId, .requiredSamples, .bodyReserved,
  .thresholdAtoms, .capacityProfileId, .evaluatorReleaseId
]

def rustName : Field → String
  | .magic => "STATISTIC_SPEC_MAGIC_OFFSET_V1"
  | .schemaVersion => "STATISTIC_SPEC_SCHEMA_VERSION_OFFSET_V1"
  | .kind => "STATISTIC_SPEC_KIND_OFFSET_V1"
  | .rounding => "STATISTIC_SPEC_ROUNDING_OFFSET_V1"
  | .sourceScaleExponent => "STATISTIC_SPEC_SOURCE_SCALE_EXPONENT_OFFSET_V1"
  | .sourceUnitId => "STATISTIC_SPEC_SOURCE_UNIT_ID_OFFSET_V1"
  | .resultUnitId => "STATISTIC_SPEC_RESULT_UNIT_ID_OFFSET_V1"
  | .requiredSamples => "STATISTIC_SPEC_REQUIRED_SAMPLES_OFFSET_V1"
  | .bodyReserved => "STATISTIC_SPEC_BODY_RESERVED_OFFSET_V1"
  | .thresholdAtoms => "STATISTIC_SPEC_THRESHOLD_ATOMS_OFFSET_V1"
  | .capacityProfileId => "STATISTIC_SPEC_CAPACITY_PROFILE_ID_OFFSET_V1"
  | .evaluatorReleaseId => "STATISTIC_SPEC_EVALUATOR_RELEASE_ID_OFFSET_V1"

def doc : Field → String
  | .magic => "Canonical statistic-specification magic."
  | .schemaVersion => "Crate-wide `SCHEMA_VERSION`, at this record's coordinate."
  | .kind => "Statistic family tag."
  | .rounding => "The one named statistic-to-result rounding boundary."
  | .sourceScaleExponent =>
      "Declared source-to-result decimal shift; zero is the identity every pre-factor record states."
  | .sourceUnitId => "Identity of the unit the observation is counted in."
  | .resultUnitId => "Identity of the unit the mapping release consumes."
  | .requiredSamples => "Exact required observation count."
  | .bodyReserved => "Canonical-zero span between the sample count and the threshold."
  | .thresholdAtoms => "Exact signed threshold atoms, zero for non-threshold families."
  | .capacityProfileId => "Identity of the capacity profile bounding the sample count."
  | .evaluatorReleaseId => "Identity of the release that evaluates this statistic."

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

/-- The twelve fields cover the 176 bytes every reader allocates: no gap, and
the last field ends exactly at the declared width. -/
theorem layout_covers_its_declared_width :
    statisticSpecBytes = 176 ∧ tiles 0 layout 176 = true := by
  native_decide

/-- Every coordinate, including the eight that were bare arguments inside
`decode` and `to_bytes` and had no name in any language. -/
theorem coordinates_are_canonical : coordinates layout = [
    (.magic, 0, 8), (.schemaVersion, 8, 2), (.kind, 10, 1), (.rounding, 11, 1),
    (.sourceScaleExponent, 12, 4),
    (.sourceUnitId, 16, 32), (.resultUnitId, 48, 32),
    (.requiredSamples, 80, 2), (.bodyReserved, 82, 14),
    (.thresholdAtoms, 96, 16),
    (.capacityProfileId, 112, 32), (.evaluatorReleaseId, 144, 32)
  ] := by
  native_decide

/-- **The migration statement, as a placement.**  The shift begins exactly
where the rounding tag ends and ends exactly where the first unit identity
begins, so the four bytes it occupies are precisely the four `decode` used to
require canonically zero -- it took nothing from a neighbour and left no gap.
That is what makes a statistic founded before `4cd2b9cb5` decode at the
identity and re-encode byte-for-byte, and it is why the record's width did not
move. -/
theorem the_factor_fills_the_span_that_was_reserved :
    Field.offset .sourceScaleExponent =
        Field.offset .rounding + Field.width .rounding ∧
      Field.offset .sourceScaleExponent + Field.width .sourceScaleExponent =
        Field.offset .sourceUnitId ∧
      Field.offset .sourceScaleExponent = 12 ∧
      Field.width .sourceScaleExponent = 4 := by
  native_decide

/-- The record had two canonical-zero spans and the factor consumed the first,
so it has one.  A reserved span is where the next field will land, and a claim
about how many there are belongs in the schema rather than in a comment beside
a `zero` call. -/
theorem exactly_one_reserved_span_remains :
    schema.filter (fun field => isReserved field.kind) =
      [⟨.bodyReserved, .reserved 14⟩] := by
  native_decide

/-- The two units are adjacent and equally wide.  They are the pair the shift
relates: equal identities declare no conversion and admit only a zero shift,
different ones declare a conversion.  The Rust wrote `16` and `48` in four
places and never said the second followed the first. -/
theorem the_units_are_an_adjacent_pair :
    unitsOffset = Field.offset .sourceUnitId ∧
      Field.offset .resultUnitId =
        Field.offset .sourceUnitId + Field.width .sourceUnitId ∧
      Field.width .sourceUnitId = Field.width .resultUnitId ∧
      unitsOffset + unitsBytes = Field.offset .requiredSamples := by
  native_decide

/-- All four identity coordinates are full-width content ids. -/
theorem the_identities_are_content_ids :
    Field.width .sourceUnitId = 32 ∧ Field.width .resultUnitId = 32 ∧
      Field.width .capacityProfileId = 32 ∧
      Field.width .evaluatorReleaseId = 32 := by
  native_decide

/-- The threshold is a full signed-atom coordinate, and the record ends with
the two releases it is evaluated under. -/
theorem the_threshold_is_a_full_atom_coordinate :
    Field.width .thresholdAtoms = 16 ∧
      Field.offset .evaluatorReleaseId + Field.width .evaluatorReleaseId =
        statisticSpecBytes := by
  native_decide

theorem magic_is_eight_bytes : magic.length = 8 := by native_decide

theorem magic_fills_its_field : magic.length = Field.width .magic := by
  native_decide

theorem rust_names_are_distinct : (Field.all.map Field.rustName).Nodup := by
  native_decide

theorem every_placed_field_is_named :
    Field.all = schema.map (fun field => field.name) := by native_decide

end DClutch.SourceStatisticSpecV1Abi
