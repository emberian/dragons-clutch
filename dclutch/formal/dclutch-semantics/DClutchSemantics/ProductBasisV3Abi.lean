import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import DClutchSemantics.LiabilityBasisV2PriceGate
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
  | splineDegree | splineFlags | basisWidth | knotCount | termCount | product
  | resultDomain | coordinateDomain | resultUnit | payoutScale
  | knotDenominator | evaluatorRelease | priceGateDigest | tailReserved
  deriving DecidableEq, Repr

def headerSchema : List (FieldSpec HeaderField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.headerBytes, .u16⟩,
  ⟨.recordBytes, .u32⟩,
  ⟨.kind, .u8⟩,
  ⟨.rounding, .u8⟩,
  ⟨.splineDegree, .u8⟩,
  ⟨.splineFlags, .u8⟩,
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
  ⟨.priceGateDigest, .bytes 32⟩,
  ⟨.tailReserved, .reserved 16⟩
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
  | .splineDegree => "BASIS_SPLINE_DEGREE_OFFSET_V3"
  | .splineFlags => "BASIS_SPLINE_FLAGS_OFFSET_V3"
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
  | .priceGateDigest => "BASIS_PRICE_GATE_DIGEST_OFFSET_V3"
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

/-- Header byte 16 value 3: the degree-2-to-3 B-spline family.

**Allocating a tag is not admitting a record.**  No decoder in this tree
accepts this byte, no encoder emits it into a `DCLTPAY3` record, and no
evaluator exists for the family it names.  The allocation buys two things.  It
takes the byte out of circulation, so a later record family cannot claim 3 and
collide with a partially-landed spline.  And it forces every exhaustive match
over the kind to state, at compile time, what it does with a family it cannot
evaluate — which is how a refusal becomes a build failure rather than a
runtime surprise.

The reserved span at offset 18 **is** spent here, and that is what turns this
from an allocation into a wire migration.  Byte 18 carries the degree and byte
19 the flags; both are zero for the two shipping kinds, which a deployed
decoder already enforces, so an old decoder confronted with a curved record
refuses it rather than misreading one.  The schema identity bumps in the same
commit for the converse direction. -/
def splineDegree2To3Kind : Nat := 3

/-- The closed degree interval the third family names.  Degree 0 is `Constant`
and degree 1 is the ramp/tent family, both of which ship today through
`BasisShapeV3` under `gradedExactComplementKind`; neither belongs to this kind.
Above 3 is not a capacity limit — it is the interval the kernel's de Boor
development and its price gate are proved over. -/
def splineMinimumDegree : Nat := 2
def splineMaximumDegree : Nat := 3

/-- Header byte 19, bit 0: this basis permits repeated **interior** knots.

Interior multiplicity is how a spline lowers continuity -- a knot of
multiplicity `r` collapses `r - 1` spans and puts a corner inside an otherwise
smooth basis.  It is a permission rather than a fact so that the relaxation is
*declared by the record* and visible in its digest, not inferred by an
evaluator noticing a repeat.  The two shipping kinds keep their strictly
increasing knot rule unconditionally; this bit does not exist for them and
their flags byte is forced zero.

Bits 1-7 are unallocated and required zero, so the byte keeps refusing what it
does not understand. -/
def splineInteriorMultiplicityFlag : Nat := 1

/-- Header byte 17 names the rounding boundary, and the live decoder requires
it to *agree with* the kind rather than merely be in range. -/
def exactCategoricalBoundary : Nat := 0
def termFloorExactComplementBoundary : Nat := 1

/-- Header byte 17 value 2: **cumulative-floor**, the spline family's rounding.

The orchestrator ruled this on measurement (WAVE `76e2ca3f`).  The graded
family's rule -- floor each primary term, hand the residue to the last claim --
is well defined only because that family *structurally reserves* its last
claim.  A spline reserves nothing: every one of its claims carries a de Boor
weight, and the claims outside the local support carry an exact zero.
Transliterating the graded rule would pay rounding residue to a claim the basis
says is unsupported.

It is a distinct boundary tag rather than a reuse of tag 1 because the decoder
requires the rounding byte to *agree with* the kind rather than merely be in
range.  Giving the blessed rule its own value is what makes "which rounding
did this record use" a question the wire answers. -/
def cumulativeFloorBoundary : Nat := 2

theorem rounding_boundaries_distinct :
    exactCategoricalBoundary ≠ termFloorExactComplementBoundary ∧
      exactCategoricalBoundary ≠ cumulativeFloorBoundary ∧
      termFloorExactComplementBoundary ≠ cumulativeFloorBoundary := by
  refine ⟨?_, ?_, ?_⟩ <;> decide

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
    (.splineDegree, 18, 1),
    (.splineFlags, 19, 1),
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
    (.priceGateDigest, 208, 32),
    (.tailReserved, 240, 16)
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

/-! ## The reserved spans, and what spending them bought

The fifty reserved bytes existed for exactly this.  They are now spent, by the
commit that makes the third kind evaluable rather than merely allocated:

- **offset 18** becomes the spline degree, and **offset 19** its flags byte,
  whose bit 0 permits repeated interior knots.  Both are kind-inactive for the
  two shipping kinds and forced canonical there, which is the same discipline
  `payout_scale == 1` already takes for a categorical record.
- **offset 208** becomes the 32-byte `DCLTPGT1` certificate digest, leaving
  **sixteen** reserved bytes at 240.

No offset moved and no width changed -- every field a deployed decoder reads is
where it was.  What changed is that two spans an old decoder refuses on nonzero
now carry meaning, which is exactly why the schema identity had to bump in the
same commit: see `ProductGradedBasisAdmissionV3Abi`.  An old decoder confronted
with a new record refuses rather than misreads it, and a new decoder confronted
with a record finalized under the old identity refuses it outright. -/

/-- The header's remaining slack is sixteen bytes, in one span.

This was fifty, in two spans, and the theorem said so as the reason the spans
had to stay reserved until a lane genuinely needed them.  The degree byte, the
flags byte and the certificate digest are that need.  Sixteen bytes are left,
and leaving *some* is deliberate: a record with no slack at all is the
`CoreState` situation, where adding a field is an account-size migration. -/
theorem header_reserved_span_is_sixteen :
    ((headerSchema.filter fun field =>
        match field.kind with | .reserved _ => true | _ => false).map
      fun field => field.kind.byteWidth).sum = 16 := by native_decide

/-- The spend is exact: degree, flags and digest account for the whole
difference, so nothing was quietly taken beyond what this commit names. -/
theorem header_reserved_spend_is_thirty_four :
    50 - 16 = 1 + 1 + 32 := by decide

/-- The three kind discriminants are pairwise distinct and all fit a `u8`, so
the byte at offset 16 names three families without widening. -/
theorem kind_tags_distinct :
    categoricalKind ≠ gradedExactComplementKind ∧
      categoricalKind ≠ splineDegree2To3Kind ∧
      gradedExactComplementKind ≠ splineDegree2To3Kind ∧
      splineDegree2To3Kind < 256 := by
  refine ⟨?_, ?_, ?_, ?_⟩ <;> decide

/-! ## Why the Rust admission conjunct takes no degree comparison

The price gate exempts degree `≤ 1` by proof: at that degree the simplex
condition is still the whole no-arbitrage condition, so a basis needs no
certificate.  Above it, `not_admits_of_graded_without_certificate` says
admission without a certificate is `false`.

The interval this kind names starts at 2, which is strictly above the exempt
degree.  So the conjunct in `runtime_v3.rs` demands a certificate for
`SplineDegree2To3` *unconditionally* rather than comparing a degree at runtime,
and the theorem below is why that is not a shortcut: the comparison it would
perform has no false branch anywhere in the interval. -/

open DClutch.LiabilityBasisV2 (Basis)
open DClutch.LiabilityBasisV2.PriceGate (admits exemptDegree)

/-- The exempt degree is below every degree this kind admits.  This is the one
arithmetic fact the unconditional Rust conjunct rests on. -/
theorem exempt_degree_below_spline_interval :
    exemptDegree < splineMinimumDegree ∧ splineMinimumDegree ≤ splineMaximumDegree := by
  constructor <;> decide

/-- **A basis at any degree this kind names is refused without a certificate.**
Instantiating the price gate's own admission theorem across the whole interval,
so the Rust refusal is a consequence rather than a restatement. -/
theorem spline_degrees_require_a_certificate {Result : Type}
    (basis : Basis Result) (degree : Nat)
    (lower : splineMinimumDegree ≤ degree) :
    admits basis degree none = false := by
  refine DClutch.LiabilityBasisV2.PriceGate.not_admits_of_graded_without_certificate
    basis degree ?_
  have : exemptDegree < splineMinimumDegree := exempt_degree_below_spline_interval.left
  omega

end DClutch.ProductBasisV3Abi
