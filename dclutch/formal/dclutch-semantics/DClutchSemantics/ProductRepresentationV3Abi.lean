import DClutchSemantics.AbiSchema

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

inductive Field where
  | magic | version | basisKind | reservedHeader
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
  ⟨.reservedHeader, .reserved 5⟩,
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
  .magic, .version, .basisKind, .reservedHeader,
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
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl |
      rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl |
      rfl | rfl | rfl | rfl <;> decide

end DClutch.ProductRepresentationV3Abi
