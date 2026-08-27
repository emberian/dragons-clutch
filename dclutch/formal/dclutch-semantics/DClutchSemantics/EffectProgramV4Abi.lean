import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# EffectProgram V4 / DCE5 fixed ABI assurance

This module owns the DCE5 successor's fixed header, dynamic-span, and borrowed
range coordinates.  It models the production Product-tail-affine semantic and
child partition, including the Dealer selector-9 witness introduced by
`d19af10`, and the canonical fixed-topology zero-table envelope.

The safe Rust kernel remains the executable owner of hostile decoding, DCE4
embedding, invocation selection, scalar authentication, and checked runtime
resolution.  Lean emits only constants plus canonical and hostile byte
witnesses for differential translation tests.  Hot and family adapters remain
outside this model.
-/

namespace DClutch.EffectProgramV4Abi

open DClutch.AbiSchema

def magic : List UInt8 := "DCE5".toUTF8.toList
def version : Nat := 5
def disjointExactCoveragePolicy : Nat := 0
def identicalReuseExactCoveragePolicy : Nat := 1
def semanticRangeRoute : Nat := 65535
def fixedCoordinateKind : Nat := 0
def commonScalarCoordinateKind : Nat := 1
def productTailAffineCoordinateKind : Nat := 2
def maxExtension : Nat := 63
def maxU32 : Nat := 2 ^ 32 - 1

def schemaReleasePreimage : List UInt8 :=
  "dclutch/schema/effect-program-v5-scalar-spans-and-borrowed-ranges-v2-tail-affine-semantic".toUTF8.toList
def schemaReleaseId : List UInt8 := [
  0x28, 0xe4, 0xa6, 0xc2, 0x95, 0x9d, 0x49, 0x76,
  0x12, 0x35, 0xb7, 0x79, 0x9a, 0xa4, 0xee, 0xcf,
  0x28, 0x45, 0x05, 0x29, 0xb2, 0xa5, 0x0c, 0xb9,
  0x2b, 0x77, 0x69, 0x6d, 0x2f, 0xfe, 0xd4, 0x8c
]

inductive HeaderField where
  | magic | version | policy | spanCount | rangeCount | reservedHeader
  | baseBytes | semanticPrefixBytes | reservedTail
  deriving DecidableEq, Repr

def headerSchema : List (FieldSpec HeaderField) := [
  ⟨.magic, .bytes 4⟩, ⟨.version, .u8⟩, ⟨.policy, .u8⟩,
  ⟨.spanCount, .u16⟩, ⟨.rangeCount, .u16⟩,
  ⟨.reservedHeader, .reserved 2⟩, ⟨.baseBytes, .u32⟩,
  ⟨.semanticPrefixBytes, .u32⟩, ⟨.reservedTail, .reserved 4⟩
]

inductive DynamicSpanField where
  | route | selectorCommonScalar | baseFixedAccountCount | reserved
  | allowedExtensions
  deriving DecidableEq, Repr

def dynamicSpanSchema : List (FieldSpec DynamicSpanField) := [
  ⟨.route, .u16⟩, ⟨.selectorCommonScalar, .u16⟩,
  ⟨.baseFixedAccountCount, .u16⟩, ⟨.reserved, .reserved 2⟩,
  ⟨.allowedExtensions, .u64⟩
]

inductive BorrowedRangeField where
  | route | offsetKind | lengthKind | offsetValue | lengthValue | reserved
  deriving DecidableEq, Repr

def borrowedRangeSchema : List (FieldSpec BorrowedRangeField) := [
  ⟨.route, .u16⟩, ⟨.offsetKind, .u8⟩, ⟨.lengthKind, .u8⟩,
  ⟨.offsetValue, .u32⟩, ⟨.lengthValue, .u32⟩,
  ⟨.reserved, .reserved 4⟩
]

def headerLayout := specialize headerSchema
def dynamicSpanLayout := specialize dynamicSpanSchema
def borrowedRangeLayout := specialize borrowedRangeSchema
def headerBytes := schemaWidth headerSchema
def dynamicSpanBytes := schemaWidth dynamicSpanSchema
def borrowedRangeBytes := schemaWidth borrowedRangeSchema

namespace HeaderField
def rustName : HeaderField → String
  | .magic => "EFFECT_V4_MAGIC_OFFSET"
  | .version => "EFFECT_V4_VERSION_OFFSET"
  | .policy => "EFFECT_V4_POLICY_OFFSET"
  | .spanCount => "EFFECT_V4_SPAN_COUNT_OFFSET"
  | .rangeCount => "EFFECT_V4_RANGE_COUNT_OFFSET"
  | .reservedHeader => "EFFECT_V4_RESERVED_HEADER_OFFSET"
  | .baseBytes => "EFFECT_V4_BASE_BYTES_OFFSET"
  | .semanticPrefixBytes => "EFFECT_V4_SEMANTIC_PREFIX_BYTES_OFFSET"
  | .reservedTail => "EFFECT_V4_RESERVED_TAIL_OFFSET"
end HeaderField

namespace DynamicSpanField
def rustName : DynamicSpanField → String
  | .route => "EFFECT_V4_SPAN_ROUTE_OFFSET"
  | .selectorCommonScalar => "EFFECT_V4_SPAN_SELECTOR_COMMON_SCALAR_OFFSET"
  | .baseFixedAccountCount => "EFFECT_V4_SPAN_BASE_FIXED_ACCOUNT_COUNT_OFFSET"
  | .reserved => "EFFECT_V4_SPAN_RESERVED_OFFSET"
  | .allowedExtensions => "EFFECT_V4_SPAN_ALLOWED_EXTENSIONS_OFFSET"
end DynamicSpanField

namespace BorrowedRangeField
def rustName : BorrowedRangeField → String
  | .route => "EFFECT_V4_RANGE_ROUTE_OFFSET"
  | .offsetKind => "EFFECT_V4_RANGE_OFFSET_KIND_OFFSET"
  | .lengthKind => "EFFECT_V4_RANGE_LENGTH_KIND_OFFSET"
  | .offsetValue => "EFFECT_V4_RANGE_OFFSET_VALUE_OFFSET"
  | .lengthValue => "EFFECT_V4_RANGE_LENGTH_VALUE_OFFSET"
  | .reserved => "EFFECT_V4_RANGE_RESERVED_OFFSET"
end BorrowedRangeField

/-! ## Protected range-resolution and partition model -/

inductive RequestCoordinate where
  | fixed (value : Nat)
  | commonScalar (index : Nat)
  | productTailAffine (base stride : Nat)
  deriving DecidableEq, Repr

structure BorrowedRangeDeclaration where
  route : Nat
  offset : RequestCoordinate
  length : RequestCoordinate
  deriving DecidableEq, Repr

structure ResolvedRange where
  route : Nat
  start : Nat
  length : Nat
  deriving DecidableEq, Repr

def checkedU32 (value : Nat) : Option Nat :=
  if value ≤ maxU32 then some value else none

def RequestCoordinate.resolve (coordinate : RequestCoordinate)
    (scalars : List Nat) (productTailCount : Nat) : Option Nat :=
  match coordinate with
  | .fixed value => checkedU32 value
  | .commonScalar index => scalars[index]?
  | .productTailAffine base stride =>
      if stride = 0 then none else checkedU32 (base + stride * productTailCount)

def BorrowedRangeDeclaration.resolve (declaration : BorrowedRangeDeclaration)
    (scalars : List Nat) (productTailCount : Nat) : Option ResolvedRange := do
  let start ← declaration.offset.resolve scalars productTailCount
  let length ← declaration.length.resolve scalars productTailCount
  if length = 0 then none else
  pure ⟨declaration.route, start, length⟩

def RangeInBounds (requestBytes : Nat) (range : ResolvedRange) : Prop :=
  0 < range.length ∧ range.start + range.length ≤ requestBytes

def Nonoverlapping (left right : ResolvedRange) : Prop :=
  left.start + left.length ≤ right.start ∨
    right.start + right.length ≤ left.start

def ContainsByte (range : ResolvedRange) (byte : Nat) : Prop :=
  range.start ≤ byte ∧ byte < range.start + range.length

def dealerSemanticDeclaration : BorrowedRangeDeclaration :=
  ⟨semanticRangeRoute, .fixed 384, .productTailAffine 0 8⟩

def dealerClaimsDeclaration : BorrowedRangeDeclaration :=
  ⟨4, .productTailAffine 384 8, .commonScalar 1⟩

def dealerSemanticRange (productTailCount : Nat) : ResolvedRange :=
  ⟨semanticRangeRoute, 384, 8 * productTailCount⟩

def dealerClaimsRange (productTailCount witnessBytes : Nat) : ResolvedRange :=
  ⟨4, 384 + 8 * productTailCount, witnessBytes⟩

def dealerRequestBytes (productTailCount witnessBytes : Nat) : Nat :=
  384 + 8 * productTailCount + witnessBytes

theorem dealer_semantic_coordinate_resolves_exactly
    (positiveTail : 0 < productTailCount) (boundedTail : productTailCount ≤ 256) :
    dealerSemanticDeclaration.resolve [0, witnessBytes] productTailCount =
      some (dealerSemanticRange productTailCount) := by
  have affineBound : 8 * productTailCount ≤ maxU32 := by
    have maximum : 8 * 256 ≤ maxU32 := by native_decide
    exact Nat.le_trans (Nat.mul_le_mul_left 8 boundedTail) maximum
  have fixedBound : 384 ≤ maxU32 := by native_decide
  have affineNonzero : 8 * productTailCount ≠ 0 := by omega
  simp [BorrowedRangeDeclaration.resolve, dealerSemanticDeclaration,
    RequestCoordinate.resolve, checkedU32, dealerSemanticRange, affineBound,
    fixedBound, affineNonzero]

theorem dealer_child_coordinate_resolves_exactly
    (positiveWitness : 0 < witnessBytes)
    (boundedTail : productTailCount ≤ 256) :
    dealerClaimsDeclaration.resolve [0, witnessBytes] productTailCount =
      some (dealerClaimsRange productTailCount witnessBytes) := by
  have affineBound : 384 + 8 * productTailCount ≤ maxU32 := by
    have maximum : 384 + 8 * 256 ≤ maxU32 := by native_decide
    omega
  have witnessNonzero : witnessBytes ≠ 0 := by omega
  simp [BorrowedRangeDeclaration.resolve, dealerClaimsDeclaration,
    RequestCoordinate.resolve, checkedU32, dealerClaimsRange, affineBound, witnessNonzero]

theorem dealer_ranges_are_in_bounds
    (positiveTail : 0 < productTailCount) (positiveWitness : 0 < witnessBytes) :
    RangeInBounds (dealerRequestBytes productTailCount witnessBytes)
        (dealerSemanticRange productTailCount) ∧
      RangeInBounds (dealerRequestBytes productTailCount witnessBytes)
        (dealerClaimsRange productTailCount witnessBytes) := by
  simp [RangeInBounds, dealerRequestBytes, dealerSemanticRange, dealerClaimsRange]
  omega

theorem dealer_ranges_are_ordered_and_nonoverlapping
    (positiveTail : 0 < productTailCount) :
    (dealerSemanticRange productTailCount).start <
        (dealerClaimsRange productTailCount witnessBytes).start ∧
      Nonoverlapping (dealerSemanticRange productTailCount)
        (dealerClaimsRange productTailCount witnessBytes) := by
  simp [Nonoverlapping, dealerSemanticRange, dealerClaimsRange]
  omega

theorem dealer_ranges_cover_exact_declared_request
    (productTailCount witnessBytes : Nat) :
    (dealerSemanticRange productTailCount).start = 384 ∧
      (dealerSemanticRange productTailCount).start +
          (dealerSemanticRange productTailCount).length =
        (dealerClaimsRange productTailCount witnessBytes).start ∧
      (dealerClaimsRange productTailCount witnessBytes).start +
          (dealerClaimsRange productTailCount witnessBytes).length =
        dealerRequestBytes productTailCount witnessBytes := by
  simp [dealerSemanticRange, dealerClaimsRange, dealerRequestBytes]

theorem semantic_bytes_cannot_be_reinterpreted_as_child_packet_bytes
    (semantic : ContainsByte (dealerSemanticRange productTailCount) byte) :
    ¬ ContainsByte (dealerClaimsRange productTailCount witnessBytes) byte := by
  simp [ContainsByte, dealerSemanticRange, dealerClaimsRange] at semantic ⊢
  omega

def ZeroTableTopology (spanCount rangeCount tableBytes : Nat) : Prop :=
  spanCount = 0 ∧ rangeCount = 0 ∧ tableBytes = 0

theorem fixed_topology_has_one_canonical_zero_table
    (canonical : ZeroTableTopology spanCount rangeCount tableBytes) :
    spanCount = 0 ∧ rangeCount = 0 ∧
      tableBytes = spanCount * dynamicSpanBytes + rangeCount * borrowedRangeBytes := by
  rcases canonical with ⟨rfl, rfl, rfl⟩
  decide

/-! ## Canonical byte witnesses and refusal corpus -/

def zeros (count : Nat) : List UInt8 := List.replicate count 0

def encodeHeader (policy spanCount rangeCount baseBytes semanticPrefixBytes : Nat) :
    List UInt8 :=
  magic ++ [UInt8.ofNat version, UInt8.ofNat policy] ++
  DClutch.Codec.encodeLE 2 spanCount ++ DClutch.Codec.encodeLE 2 rangeCount ++ zeros 2 ++
  DClutch.Codec.encodeLE 4 baseBytes ++ DClutch.Codec.encodeLE 4 semanticPrefixBytes ++ zeros 4

def coordinateKindValue : RequestCoordinate → Nat × Nat
  | .fixed value => (fixedCoordinateKind, value)
  | .commonScalar index => (commonScalarCoordinateKind, index)
  | .productTailAffine base stride =>
      (productTailAffineCoordinateKind, base + stride * 65536)

def encodeBorrowedRange (range : BorrowedRangeDeclaration) : List UInt8 :=
  let offset := coordinateKindValue range.offset
  let length := coordinateKindValue range.length
  DClutch.Codec.encodeLE 2 range.route ++
  [UInt8.ofNat offset.1, UInt8.ofNat length.1] ++
  DClutch.Codec.encodeLE 4 offset.2 ++ DClutch.Codec.encodeLE 4 length.2 ++ zeros 4

def zeroTableHeaderWitness : List UInt8 :=
  encodeHeader disjointExactCoveragePolicy 0 0 192 1

def dealerHeaderWitness : List UInt8 :=
  encodeHeader disjointExactCoveragePolicy 0 2 192 384

def dealerRangeTableWitness : List UInt8 :=
  encodeBorrowedRange dealerSemanticDeclaration ++
    encodeBorrowedRange dealerClaimsDeclaration

def patch (bytes : List UInt8) (offset : Nat) (replacement : List UInt8) : List UInt8 :=
  bytes.take offset ++ replacement ++ bytes.drop (offset + replacement.length)

def hostileZeroSpanCount : List UInt8 :=
  patch zeroTableHeaderWitness 6 (DClutch.Codec.encodeLE 2 1)
def hostileHeaderReserved : List UInt8 := List.set zeroTableHeaderWitness 20 1
def hostileAffineZeroStride : List UInt8 :=
  patch dealerRangeTableWitness 8 (DClutch.Codec.encodeLE 4 0)
def hostileReversedRanges : List UInt8 :=
  encodeBorrowedRange dealerClaimsDeclaration ++ encodeBorrowedRange dealerSemanticDeclaration
def hostileChildOverlap : List UInt8 :=
  patch dealerRangeTableWitness (borrowedRangeBytes + 4)
    (DClutch.Codec.encodeLE 4 (383 + 8 * 65536))
def hostileChildGap : List UInt8 :=
  patch dealerRangeTableWitness (borrowedRangeBytes + 4)
    (DClutch.Codec.encodeLE 4 (385 + 8 * 65536))

theorem fixed_layout_widths_are_exact :
    headerBytes = 24 ∧ dynamicSpanBytes = 16 ∧ borrowedRangeBytes = 16 := by
  native_decide

theorem fixed_layouts_are_pairwise_disjoint :
    headerLayout.Pairwise Before ∧ dynamicSpanLayout.Pairwise Before ∧
      borrowedRangeLayout.Pairwise Before := by
  exact ⟨specializeFrom_pairwise 0 headerSchema,
    specializeFrom_pairwise 0 dynamicSpanSchema,
    specializeFrom_pairwise 0 borrowedRangeSchema⟩

theorem schema_release_coordinates_are_exact :
    schemaReleasePreimage.length = 89 ∧ schemaReleaseId.length = 32 := by native_decide

theorem canonical_witness_widths_are_exact :
    zeroTableHeaderWitness.length = headerBytes ∧
      dealerHeaderWitness.length = headerBytes ∧
      dealerRangeTableWitness.length = 2 * borrowedRangeBytes := by native_decide

theorem dealer_wire_table_has_exact_semantic_then_child_order :
    dealerRangeTableWitness.take borrowedRangeBytes =
        encodeBorrowedRange dealerSemanticDeclaration ∧
      dealerRangeTableWitness.drop borrowedRangeBytes =
        encodeBorrowedRange dealerClaimsDeclaration := by native_decide

theorem hostile_corpus_preserves_exact_table_widths :
    hostileZeroSpanCount.length = zeroTableHeaderWitness.length ∧
      hostileHeaderReserved.length = zeroTableHeaderWitness.length ∧
      hostileAffineZeroStride.length = dealerRangeTableWitness.length ∧
      hostileReversedRanges.length = dealerRangeTableWitness.length ∧
      hostileChildOverlap.length = dealerRangeTableWitness.length ∧
      hostileChildGap.length = dealerRangeTableWitness.length := by native_decide

end DClutch.EffectProgramV4Abi
