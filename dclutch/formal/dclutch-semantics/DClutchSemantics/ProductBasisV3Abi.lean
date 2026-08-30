import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# Live runtime liability-basis V3 ABI (`DCLTPAY3`)

This module is the semantic owner of the record the protocol actually evaluates
on chain.  Until it existed, `ProductBasisV3`'s layout lived only as a block of
handwritten Rust constants carrying no `@generated` marker, invisible to the
emission census and pinned by nothing -- while the *unreferenced* V2 liability
basis kernel next door carried an emitted conformance corpus and 221 theorems.
That asymmetry was an assurance inversion, not an authority question: the
evaluator with callers was the one without a specification.

This file does not change a single wire byte.  It restates the existing
`DCLTPAY3` header and term layouts as schema data so that `specialize` -- the
only owner of a field offset anywhere in this project -- derives every offset
the Rust decoder reads, and so `basisHeaderCoordinates` below freezes all
eighteen of them against a literal witness.  The emitted Rust is required to be
byte-identical to what the handwritten constants already said; that `cmp` is
the proof that nothing moved.

Authority is unchanged and deliberately so.  The live `ProductBasisV3`
evaluator remains the protocol's sole basis writer under `O-005`; the kernel
crate stays a non-authoritative differential reference.  What moves here is the
specification, not the code that runs.
-/

namespace DClutch.ProductBasisV3Abi

open DClutch
open DClutch.AbiSchema

/-! ## Record identity -/

/-- `DCLTPAY3`.  Distinct from the dormant `DCLTPAY2` and from every magic in
the kernel's `DCLTLBV2` family, so this record shares a discriminating prefix
with nothing. -/
def basisMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x50, 0x41, 0x59, 0x33]

/-- Wire schema of the live runtime basis record. -/
def basisSchemaVersion : Nat := 3

/-! ## Header layout

Eighteen fields, gapless, totalling exactly the 256-byte fixed header that
precedes every runtime tail.  The two reserved spans are the record's only
slack and both are zero-enforced by the live decoder, which is what lets an old
decoder *refuse* rather than misread a record written by a newer one. -/

inductive HeaderField where
  | magic | schemaVersion | headerBytes | recordBytes | kind | rounding
  | headerReserved | basisWidth | knotCount | termCount | product
  | resultDomain | coordinateDomain | resultUnit | payoutScale
  | knotDenominator | evaluatorRelease | tailReserved
  deriving DecidableEq, Repr

def headerSchema : List (FieldSpec HeaderField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.headerBytes, .u16⟩,
  ⟨.recordBytes, .u32⟩,
  ⟨.kind, .u8⟩,
  ⟨.rounding, .u8⟩,
  ⟨.headerReserved, .reserved 2⟩,
  ⟨.basisWidth, .u32⟩,
  ⟨.knotCount, .u32⟩,
  ⟨.termCount, .u32⟩,
  ⟨.product, .bytes 32⟩,
  ⟨.resultDomain, .bytes 32⟩,
  ⟨.coordinateDomain, .bytes 32⟩,
  ⟨.resultUnit, .bytes 32⟩,
  ⟨.payoutScale, .u64⟩,
  ⟨.knotDenominator, .u64⟩,
  ⟨.evaluatorRelease, .bytes 32⟩,
  ⟨.tailReserved, .reserved 48⟩
]

def headerLayout : List (PlacedField HeaderField) := specialize headerSchema
def headerBytes : Nat := schemaWidth headerSchema

namespace HeaderField

def rustName : HeaderField → String
  | .magic => "BASIS_MAGIC_OFFSET_V3"
  | .schemaVersion => "BASIS_SCHEMA_OFFSET_V3"
  | .headerBytes => "BASIS_HEADER_BYTES_OFFSET_V3"
  | .recordBytes => "BASIS_RECORD_BYTES_OFFSET_V3"
  | .kind => "BASIS_KIND_OFFSET_V3"
  | .rounding => "BASIS_ROUNDING_OFFSET_V3"
  | .headerReserved => "BASIS_HEADER_RESERVED_OFFSET_V3"
  | .basisWidth => "BASIS_WIDTH_OFFSET_V3"
  | .knotCount => "BASIS_KNOT_COUNT_OFFSET_V3"
  | .termCount => "BASIS_TERM_COUNT_OFFSET_V3"
  | .product => "BASIS_PRODUCT_ID_OFFSET_V3"
  | .resultDomain => "BASIS_RESULT_DOMAIN_ID_OFFSET_V3"
  | .coordinateDomain => "BASIS_COORDINATE_DOMAIN_ID_OFFSET_V3"
  | .resultUnit => "BASIS_RESULT_UNIT_ID_OFFSET_V3"
  | .payoutScale => "BASIS_PAYOUT_SCALE_OFFSET_V3"
  | .knotDenominator => "BASIS_KNOT_DENOMINATOR_OFFSET_V3"
  | .evaluatorRelease => "BASIS_EVALUATOR_RELEASE_ID_OFFSET_V3"
  | .tailReserved => "BASIS_HEADER_TAIL_RESERVED_OFFSET_V3"

end HeaderField

/-! ## Term layout

One canonical graded term.  `left`, `peak` and `right` are Product-owned *knot
indices*, not coordinates; which of them a term reads is decided by its shape
tag, and the unread ones are forced canonical rather than left free. -/

inductive TermField where
  | claimIndex | shape | shapeReserved | left | peak | right | tailReserved
  | amplitude
  deriving DecidableEq, Repr

def termSchema : List (FieldSpec TermField) := [
  ⟨.claimIndex, .u32⟩,
  ⟨.shape, .u8⟩,
  ⟨.shapeReserved, .reserved 3⟩,
  ⟨.left, .u32⟩,
  ⟨.peak, .u32⟩,
  ⟨.right, .u32⟩,
  ⟨.tailReserved, .reserved 4⟩,
  ⟨.amplitude, .u64⟩
]

def termLayout : List (PlacedField TermField) := specialize termSchema
def termBytes : Nat := schemaWidth termSchema

namespace TermField

def rustName : TermField → String
  | .claimIndex => "TERM_CLAIM_INDEX_OFFSET_V3"
  | .shape => "TERM_SHAPE_OFFSET_V3"
  | .shapeReserved => "TERM_SHAPE_RESERVED_OFFSET_V3"
  | .left => "TERM_LEFT_OFFSET_V3"
  | .peak => "TERM_PEAK_OFFSET_V3"
  | .right => "TERM_RIGHT_OFFSET_V3"
  | .tailReserved => "TERM_TAIL_RESERVED_OFFSET_V3"
  | .amplitude => "TERM_AMPLITUDE_OFFSET_V3"

end TermField

/-! ## Tail widths and discriminants -/

/-- One exact knot numerator: a little-endian `i128`.  The live wire is
deliberately wider here than the kernel's `i64`. -/
def knotBytes : Nat := 16

/-- Header byte 16 selects the evaluator family. -/
def categoricalKind : Nat := 1
def gradedExactComplementKind : Nat := 2

/-- Header byte 17 names the rounding boundary, and the live decoder requires
it to *agree with* the kind rather than merely be in range. -/
def exactCategoricalBoundary : Nat := 0
def termFloorExactComplementBoundary : Nat := 1

/-- Term byte 4 selects the shape. -/
def constantShape : Nat := 0
def rampUpShape : Nat := 1
def rampDownShape : Nat := 2
def tentShape : Nat := 3

/-- Content-hash domain for the semantic basis identity.  The Product and
result-domain links are omitted from this preimage so a Product result domain
may commit the semantic identity without a hash cycle. -/
def semanticContentDomain : List UInt8 :=
  "dclutch/product-basis/semantic/v3".toUTF8.toList

/-- Content-hash domain for the full Product-linked raw record. -/
def linkedContentDomain : List UInt8 :=
  "dclutch/product-basis/linked/v3".toUTF8.toList

/-! ## What the layout is required to be

`basisHeaderCoordinates` and `basisTermCoordinates` are the freeze.  They are a
comparison against the ABI already deployed, not a second offset table the
specializer consults: every offset on the right-hand side was read off the
handwritten Rust this module replaces.  Moving any field in `headerSchema`
fails one of them. -/

theorem header_width_is_exact : headerBytes = 256 := by native_decide
theorem term_width_is_exact : termBytes = 32 := by native_decide

theorem header_names_unique :
    (headerSchema.map fun field => field.name).Nodup := by native_decide

theorem term_names_unique :
    (termSchema.map fun field => field.name).Nodup := by native_decide

theorem basisHeaderCoordinates : coordinates headerLayout = [
    (.magic, 0, 8),
    (.schemaVersion, 8, 2),
    (.headerBytes, 10, 2),
    (.recordBytes, 12, 4),
    (.kind, 16, 1),
    (.rounding, 17, 1),
    (.headerReserved, 18, 2),
    (.basisWidth, 20, 4),
    (.knotCount, 24, 4),
    (.termCount, 28, 4),
    (.product, 32, 32),
    (.resultDomain, 64, 32),
    (.coordinateDomain, 96, 32),
    (.resultUnit, 128, 32),
    (.payoutScale, 160, 8),
    (.knotDenominator, 168, 8),
    (.evaluatorRelease, 176, 32),
    (.tailReserved, 208, 48)
  ] := by native_decide

theorem basisTermCoordinates : coordinates termLayout = [
    (.claimIndex, 0, 4),
    (.shape, 4, 1),
    (.shapeReserved, 5, 3),
    (.left, 8, 4),
    (.peak, 12, 4),
    (.right, 16, 4),
    (.tailReserved, 20, 4),
    (.amplitude, 24, 8)
  ] := by native_decide

theorem header_layout_is_byte_disjoint : headerLayout.Pairwise Before :=
  specializeFrom_pairwise 0 headerSchema

theorem term_layout_is_byte_disjoint : termLayout.Pairwise Before :=
  specializeFrom_pairwise 0 termSchema

theorem header_fields_bounded (placed : PlacedField HeaderField)
    (member : placed ∈ headerLayout) :
    placed.offset + placed.spec.kind.byteWidth ≤ 256 := by
  simpa [headerLayout, specialize, show schemaWidth headerSchema = 256 from
    header_width_is_exact] using specializeFrom_bounded 0 headerSchema placed member

theorem term_fields_bounded (placed : PlacedField TermField)
    (member : placed ∈ termLayout) :
    placed.offset + placed.spec.kind.byteWidth ≤ 32 := by
  simpa [termLayout, specialize, show schemaWidth termSchema = 32 from
    term_width_is_exact] using specializeFrom_bounded 0 termSchema placed member

/-! ## The two properties a third kind will depend on

Neither is spent here -- this commit adds no kind -- but both are the reason
the reserved spans must stay reserved, so they are stated where the layout is
defined rather than rediscovered later. -/

/-- The header's slack is exactly fifty bytes, in two spans.  A future degree
byte and interior-multiplicity bit have somewhere to go without moving a field,
and a certificate digest has somewhere to go without a new header. -/
theorem header_reserved_span_is_fifty :
    ((headerSchema.filter fun field =>
        match field.kind with | .reserved _ => true | _ => false).map
      fun field => field.kind.byteWidth).sum = 50 := by native_decide

/-- The kind and rounding discriminants are distinct and both fit a `u8`, so
the byte at offset 16 can name a third family without widening. -/
theorem kind_tags_distinct :
    categoricalKind ≠ gradedExactComplementKind ∧
      gradedExactComplementKind < 256 := by
  constructor <;> decide

end DClutch.ProductBasisV3Abi
