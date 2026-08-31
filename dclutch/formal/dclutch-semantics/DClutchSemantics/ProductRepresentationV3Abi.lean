import DClutchSemantics.AbiSchema
import DClutchSemantics.ProductBasisV3Abi

/-!
# Product Representation V3 admission ABI

This schema is the fixed-layout, ephemeral receipt produced after joining one
authenticated ProductBasisV3 record to one rational representation descriptor
and graph. It is never Registry finality or Claims state. The adapter must
authenticate those authorities independently before constructing the receipt.
-/

namespace DClutch.ProductRepresentationV3Abi

open DClutch.AbiSchema

def magic : List UInt8 :=
  [0x44, 0x43, 0x52, 0x50, 0x41, 0x44, 0x56, 0x33] -- `DCRPADV3`

def version : Nat := 3
def categoricalKind : Nat := 1
def gradedKind : Nat := 2

/-- The degree-2-to-3 spline family's admission tag, allocated to match
`DClutchSemantics.ProductBasisV3Abi.splineDegree2To3Kind`.

`DCRPADV3` is the *second* Rust author of the basis-kind byte, and until this
allocation the two authors could have drifted: nothing re-ran this emitter and
nothing compared its output to the accepted file.  The tag is allocated here so
the two byte-16 vocabularies stay one vocabulary, and refused by the decoder so
allocating it admits nothing. -/
def splineDegree2To3Kind : Nat := 3

theorem kind_tags_distinct :
    categoricalKind ≠ gradedKind ∧ categoricalKind ≠ splineDegree2To3Kind ∧
      gradedKind ≠ splineDegree2To3Kind := by
  refine ⟨?_, ?_, ?_⟩ <;> decide

/-- The two records agree on what each byte at their kind offsets means.  This
is the whole content of "one vocabulary, two authors", and it is checked here
rather than assumed because the `DCRPADV3` emitter is re-run by nothing. -/
theorem kind_tags_agree_with_the_basis_record :
    categoricalKind = ProductBasisV3Abi.categoricalKind ∧
      gradedKind = ProductBasisV3Abi.gradedExactComplementKind ∧
      splineDegree2To3Kind = ProductBasisV3Abi.splineDegree2To3Kind := by
  refine ⟨?_, ?_, ?_⟩ <;> decide

inductive Field where
  | magic | version | basisKind | splineDegree | splineFlags | reservedHeader
  | descriptorId | graphId | graphDigest
  | productId | resultDomainId | coordinateDomainId | resultUnitId
  | semanticBasisId | linkedBasisRecordDigest
  | marketId | releaseSetId | receiptMint | tokenProgram
  | representationAuthority | evaluatorReleaseId
  | basisWidth | reservedScalars | payoutScale | denominator | graphScale
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.basisKind, .u8⟩,
  ⟨.splineDegree, .u8⟩,
  ⟨.splineFlags, .u8⟩,
  ⟨.reservedHeader, .reserved 3⟩,
  ⟨.descriptorId, .bytes 32⟩,
  ⟨.graphId, .bytes 32⟩,
  ⟨.graphDigest, .bytes 32⟩,
  ⟨.productId, .bytes 32⟩,
  ⟨.resultDomainId, .bytes 32⟩,
  ⟨.coordinateDomainId, .bytes 32⟩,
  ⟨.resultUnitId, .bytes 32⟩,
  ⟨.semanticBasisId, .bytes 32⟩,
  ⟨.linkedBasisRecordDigest, .bytes 32⟩,
  ⟨.marketId, .bytes 32⟩,
  ⟨.releaseSetId, .bytes 32⟩,
  ⟨.receiptMint, .bytes 32⟩,
  ⟨.tokenProgram, .bytes 32⟩,
  ⟨.representationAuthority, .bytes 32⟩,
  ⟨.evaluatorReleaseId, .bytes 32⟩,
  ⟨.basisWidth, .u32⟩,
  ⟨.reservedScalars, .reserved 4⟩,
  ⟨.payoutScale, .u64⟩,
  ⟨.denominator, .u64⟩,
  ⟨.graphScale, .u64⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

namespace Field

def all : List Field := [
  .magic, .version, .basisKind, .splineDegree, .splineFlags, .reservedHeader,
  .descriptorId, .graphId, .graphDigest,
  .productId, .resultDomainId, .coordinateDomainId, .resultUnitId,
  .semanticBasisId, .linkedBasisRecordDigest,
  .marketId, .releaseSetId, .receiptMint, .tokenProgram,
  .representationAuthority, .evaluatorReleaseId,
  .basisWidth, .reservedScalars, .payoutScale, .denominator, .graphScale
]

def rustName : Field → String
  | .magic => "ADMISSION_MAGIC_OFFSET_V3"
  | .version => "ADMISSION_VERSION_OFFSET_V3"
  | .basisKind => "ADMISSION_BASIS_KIND_OFFSET_V3"
  | .splineDegree => "ADMISSION_SPLINE_DEGREE_OFFSET_V3"
  | .splineFlags => "ADMISSION_SPLINE_FLAGS_OFFSET_V3"
  | .reservedHeader => "ADMISSION_RESERVED_HEADER_OFFSET_V3"
  | .descriptorId => "ADMISSION_DESCRIPTOR_ID_OFFSET_V3"
  | .graphId => "ADMISSION_GRAPH_ID_OFFSET_V3"
  | .graphDigest => "ADMISSION_GRAPH_DIGEST_OFFSET_V3"
  | .productId => "ADMISSION_PRODUCT_ID_OFFSET_V3"
  | .resultDomainId => "ADMISSION_RESULT_DOMAIN_ID_OFFSET_V3"
  | .coordinateDomainId => "ADMISSION_COORDINATE_DOMAIN_ID_OFFSET_V3"
  | .resultUnitId => "ADMISSION_RESULT_UNIT_ID_OFFSET_V3"
  | .semanticBasisId => "ADMISSION_SEMANTIC_BASIS_ID_OFFSET_V3"
  | .linkedBasisRecordDigest => "ADMISSION_LINKED_BASIS_DIGEST_OFFSET_V3"
  | .marketId => "ADMISSION_MARKET_ID_OFFSET_V3"
  | .releaseSetId => "ADMISSION_RELEASE_SET_ID_OFFSET_V3"
  | .receiptMint => "ADMISSION_RECEIPT_MINT_OFFSET_V3"
  | .tokenProgram => "ADMISSION_TOKEN_PROGRAM_OFFSET_V3"
  | .representationAuthority => "ADMISSION_REPRESENTATION_AUTHORITY_OFFSET_V3"
  | .evaluatorReleaseId => "ADMISSION_EVALUATOR_RELEASE_ID_OFFSET_V3"
  | .basisWidth => "ADMISSION_BASIS_WIDTH_OFFSET_V3"
  | .reservedScalars => "ADMISSION_RESERVED_SCALARS_OFFSET_V3"
  | .payoutScale => "ADMISSION_PAYOUT_SCALE_OFFSET_V3"
  | .denominator => "ADMISSION_DENOMINATOR_OFFSET_V3"
  | .graphScale => "ADMISSION_GRAPH_SCALE_OFFSET_V3"

def offset (field : Field) : Nat :=
  ((coordinate? field layout).getD (0, 0)).1

end Field

theorem schema_wellFormed : WellFormed schema := by
  constructor
  · decide
  · intro field member
    simp [schema] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl |
      rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl |
      rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> decide

end DClutch.ProductRepresentationV3Abi
