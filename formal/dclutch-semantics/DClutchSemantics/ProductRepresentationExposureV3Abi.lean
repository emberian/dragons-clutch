import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# Product-result to Claims-representation exposure V3

This model owns only the fixed layout and the canonical rank-one execution
view of a finalized composition DAG. Product coordinates range over `N` and
Claims roots range over `K`; neither dimension is reinterpreted as the other.
The safe Rust kernel remains the hostile decoder and checked-arithmetic owner.
-/

namespace DClutch.ProductRepresentationExposureV3Abi

open DClutch.AbiSchema

def version : Nat := 3
def minProductWidth : Nat := 1
def maxProductWidth : Nat := 512
def maxRepresentationWidth : Nat := 256
def maxTerms : Nat := 65536

def magic : List UInt8 := "DCRCEX03".toUTF8.toList
def schemaPreimage : List UInt8 :=
  "dclutch/schema/product-representation-exposure-bundle-v3".toUTF8.toList
def schemaId : List UInt8 := [
  0xc8, 0xbf, 0x29, 0xb9, 0x97, 0x67, 0x94, 0xa7,
  0x7d, 0x32, 0xbe, 0xd9, 0xd7, 0xfc, 0x93, 0x3d,
  0xcb, 0xfc, 0x78, 0x75, 0x91, 0x0c, 0x99, 0xc8,
  0x0d, 0xe7, 0x18, 0xc3, 0xc0, 0x10, 0x07, 0x5a]
def capacityPreimage : List UInt8 :=
  "dclutch/capacity/product-representation-exposure-v3/product512/representation256/terms65536/u128".toUTF8.toList
def capacityId : List UInt8 := [
  0x44, 0x0b, 0x9a, 0x61, 0x16, 0x31, 0xa2, 0x3e,
  0x68, 0x74, 0xaa, 0x94, 0x54, 0x07, 0xe2, 0x35,
  0x7a, 0xea, 0xab, 0x3f, 0xea, 0x4d, 0xd0, 0xd8,
  0xc7, 0x31, 0x00, 0x9b, 0xdc, 0x83, 0x63, 0x9a]

inductive HeaderField where
  | magic | version | reservedHeader | market | resultDomain | releaseSet
  | productBasis | representationBasis | graphId | capacityProfile
  | productWidth | representationWidth | rowCount | termCount | reservedTail
  deriving DecidableEq, Repr

def headerSchema : List (FieldSpec HeaderField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.reservedHeader, .reserved 6⟩,
  ⟨.market, .bytes 32⟩, ⟨.resultDomain, .bytes 32⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.productBasis, .bytes 32⟩,
  ⟨.representationBasis, .bytes 32⟩, ⟨.graphId, .bytes 32⟩,
  ⟨.capacityProfile, .bytes 32⟩, ⟨.productWidth, .u32⟩,
  ⟨.representationWidth, .u32⟩, ⟨.rowCount, .u32⟩,
  ⟨.termCount, .u32⟩, ⟨.reservedTail, .reserved 48⟩]

inductive RowField where
  | nodeId | representationCoordinate | rank | firstTerm | termCount | denominator
  deriving DecidableEq, Repr

def rowSchema : List (FieldSpec RowField) := [
  ⟨.nodeId, .bytes 32⟩, ⟨.representationCoordinate, .u32⟩,
  ⟨.rank, .u32⟩, ⟨.firstTerm, .u32⟩, ⟨.termCount, .u32⟩,
  ⟨.denominator, .u64⟩]

inductive TermField where | productCoordinate | reserved | numerator
  deriving DecidableEq, Repr

def termSchema : List (FieldSpec TermField) := [
  ⟨.productCoordinate, .u32⟩, ⟨.reserved, .reserved 4⟩,
  ⟨.numerator, .u64⟩]

def headerLayout := specialize headerSchema
def rowLayout := specialize rowSchema
def termLayout := specialize termSchema
def headerBytes := schemaWidth headerSchema
def rowBytes := schemaWidth rowSchema
def termBytes := schemaWidth termSchema

namespace HeaderField
def rustName : HeaderField → String
  | .magic => "COMPOSITION_EXPOSURE_MAGIC_OFFSET_V3"
  | .version => "COMPOSITION_EXPOSURE_VERSION_OFFSET_V3"
  | .reservedHeader => "COMPOSITION_EXPOSURE_RESERVED_HEADER_OFFSET_V3"
  | .market => "COMPOSITION_EXPOSURE_MARKET_OFFSET_V3"
  | .resultDomain => "COMPOSITION_EXPOSURE_RESULT_DOMAIN_OFFSET_V3"
  | .releaseSet => "COMPOSITION_EXPOSURE_RELEASE_SET_OFFSET_V3"
  | .productBasis => "COMPOSITION_EXPOSURE_PRODUCT_BASIS_OFFSET_V3"
  | .representationBasis => "COMPOSITION_EXPOSURE_REPRESENTATION_BASIS_OFFSET_V3"
  | .graphId => "COMPOSITION_EXPOSURE_GRAPH_ID_OFFSET_V3"
  | .capacityProfile => "COMPOSITION_EXPOSURE_CAPACITY_PROFILE_OFFSET_V3"
  | .productWidth => "COMPOSITION_EXPOSURE_PRODUCT_WIDTH_OFFSET_V3"
  | .representationWidth => "COMPOSITION_EXPOSURE_REPRESENTATION_WIDTH_OFFSET_V3"
  | .rowCount => "COMPOSITION_EXPOSURE_ROW_COUNT_OFFSET_V3"
  | .termCount => "COMPOSITION_EXPOSURE_TERM_COUNT_OFFSET_V3"
  | .reservedTail => "COMPOSITION_EXPOSURE_RESERVED_TAIL_OFFSET_V3"
end HeaderField

namespace RowField
def rustName : RowField → String
  | .nodeId => "COMPOSITION_EXPOSURE_ROW_NODE_ID_OFFSET_V3"
  | .representationCoordinate => "COMPOSITION_EXPOSURE_ROW_COORDINATE_OFFSET_V3"
  | .rank => "COMPOSITION_EXPOSURE_ROW_RANK_OFFSET_V3"
  | .firstTerm => "COMPOSITION_EXPOSURE_ROW_FIRST_TERM_OFFSET_V3"
  | .termCount => "COMPOSITION_EXPOSURE_ROW_TERM_COUNT_OFFSET_V3"
  | .denominator => "COMPOSITION_EXPOSURE_ROW_DENOMINATOR_OFFSET_V3"
end RowField

namespace TermField
def rustName : TermField → String
  | .productCoordinate => "COMPOSITION_EXPOSURE_TERM_PRODUCT_COORDINATE_OFFSET_V3"
  | .reserved => "COMPOSITION_EXPOSURE_TERM_RESERVED_OFFSET_V3"
  | .numerator => "COMPOSITION_EXPOSURE_TERM_NUMERATOR_OFFSET_V3"
end TermField

structure ExposureTerm where
  productCoordinate : Nat
  numerator : Nat
  deriving DecidableEq, Repr

structure ExposureRow where
  representationCoordinate : Nat
  rank : Nat
  denominator : Nat
  terms : List ExposureTerm
  deriving DecidableEq, Repr

def CanonicalRow (N K : Nat) (row : ExposureRow) : Prop :=
  row.representationCoordinate < K ∧ row.rank = 1 ∧ row.denominator > 0 ∧
  row.terms ≠ [] ∧
  (∀ term ∈ row.terms, term.productCoordinate < N ∧ term.numerator > 0) ∧
  row.terms.Pairwise (fun left right => left.productCoordinate < right.productCoordinate)

def CanonicalExposure (N K : Nat) (rows : List ExposureRow) : Prop :=
  minProductWidth ≤ N ∧ N ≤ maxProductWidth ∧ 0 < K ∧
  K ≤ maxRepresentationWidth ∧ rows.length = K ∧
  (∀ row ∈ rows, CanonicalRow N K row) ∧
  rows.map (fun row => row.representationCoordinate) = List.range K

theorem canonical_edges_strictly_lower_rank
    (canonical : CanonicalRow N K row) (term : ExposureTerm)
    (_member : term ∈ row.terms) : 0 < row.rank := by
  simp [canonical.2.1]

theorem product_coordinates_cannot_be_reinterpreted_as_claims_coordinates
    (canonical : CanonicalRow N K row) (term : ExposureTerm)
    (member : term ∈ row.terms) :
    term.productCoordinate < N ∧ row.representationCoordinate < K := by
  exact ⟨(canonical.2.2.2.2.1 term member).1, canonical.1⟩

def n1Rows : List ExposureRow := [
  ⟨0, 1, 1, [⟨0, 1⟩]⟩, ⟨1, 1, 1, [⟨0, 2⟩]⟩,
  ⟨2, 1, 1, [⟨0, 3⟩]⟩]
def n258Rows : List ExposureRow := [
  ⟨0, 1, 1, [⟨0, 1⟩]⟩, ⟨1, 1, 1, [⟨128, 1⟩]⟩,
  ⟨2, 1, 1, [⟨257, 1⟩]⟩]

theorem k3_n1_is_canonical : CanonicalExposure 1 3 n1Rows := by
  simp [CanonicalExposure, CanonicalRow, n1Rows, minProductWidth,
    maxProductWidth, maxRepresentationWidth, List.range]
  native_decide
theorem k3_n258_is_canonical : CanonicalExposure 258 3 n258Rows := by
  simp [CanonicalExposure, CanonicalRow, n258Rows, minProductWidth,
    maxProductWidth, maxRepresentationWidth, List.range]
  native_decide

def zeros (count : Nat) : List UInt8 := List.replicate count 0
def repeated (value count : Nat) : List UInt8 := List.replicate count (UInt8.ofNat value)

def encodeHeader (productWidth representationWidth termCount : Nat) : List UInt8 :=
  magic ++ DClutch.Codec.encodeLE 2 version ++ zeros 6 ++
  repeated 1 32 ++ repeated 2 32 ++ repeated 3 32 ++ repeated 4 32 ++
  repeated 5 32 ++ repeated 6 32 ++ capacityId ++
  DClutch.Codec.encodeLE 4 productWidth ++
  DClutch.Codec.encodeLE 4 representationWidth ++
  DClutch.Codec.encodeLE 4 representationWidth ++
  DClutch.Codec.encodeLE 4 termCount ++ zeros 48

def encodeRow (nodeId coordinate firstTerm termCount denominator : Nat) : List UInt8 :=
  repeated nodeId 32 ++ DClutch.Codec.encodeLE 4 coordinate ++
  DClutch.Codec.encodeLE 4 1 ++ DClutch.Codec.encodeLE 4 firstTerm ++
  DClutch.Codec.encodeLE 4 termCount ++ DClutch.Codec.encodeLE 8 denominator

def encodeTerm (coordinate numerator : Nat) : List UInt8 :=
  DClutch.Codec.encodeLE 4 coordinate ++ zeros 4 ++ DClutch.Codec.encodeLE 8 numerator

def k3n1Witness : List UInt8 := encodeHeader 1 3 3 ++
  encodeRow 10 0 0 1 1 ++ encodeRow 11 1 1 1 1 ++ encodeRow 12 2 2 1 1 ++
  encodeTerm 0 1 ++ encodeTerm 0 2 ++ encodeTerm 0 3

def k3n258Witness : List UInt8 := encodeHeader 258 3 3 ++
  encodeRow 20 0 0 1 1 ++ encodeRow 21 1 1 1 1 ++ encodeRow 22 2 2 1 1 ++
  encodeTerm 0 1 ++ encodeTerm 128 1 ++ encodeTerm 257 1

def patch (bytes : List UInt8) (offset : Nat) (replacement : List UInt8) : List UInt8 :=
  bytes.take offset ++ replacement ++ bytes.drop (offset + replacement.length)

def rankCycleRefusal := patch k3n258Witness (headerBytes + 36) (DClutch.Codec.encodeLE 4 0)
def widthRefusal := patch k3n258Witness 240 (DClutch.Codec.encodeLE 4 2)
def releaseTransplantRefusal := patch k3n258Witness 80 (repeated 99 32)

theorem fixed_layout_is_exact :
    headerBytes = 304 ∧ rowBytes = 56 ∧ termBytes = 16 := by native_decide

theorem layouts_are_ordered_and_nonoverlapping :
    headerLayout.Pairwise Before ∧ rowLayout.Pairwise Before ∧ termLayout.Pairwise Before := by
  exact ⟨specializeFrom_pairwise 0 headerSchema,
    specializeFrom_pairwise 0 rowSchema, specializeFrom_pairwise 0 termSchema⟩

theorem corpus_widths_are_exact :
    k3n1Witness.length = 520 ∧ k3n258Witness.length = 520 ∧
    rankCycleRefusal.length = 520 ∧ widthRefusal.length = 520 ∧
    releaseTransplantRefusal.length = 520 := by native_decide

end DClutch.ProductRepresentationExposureV3Abi
