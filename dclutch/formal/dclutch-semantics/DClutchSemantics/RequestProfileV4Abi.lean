import DClutchSemantics.AbiSchema
import DClutchSemantics.RequestProfileAbi
import Std.Tactic

/-!
# Compact repeated-row RequestProfile V4 ABI

V4 embeds one exact V1 fixed-prefix projector and one canonical row program.
The row program is interpreted exactly `K` times.  `K` is committed by the
finalized artifact, repeated in the request, and supplied independently in a
protected common scalar by authenticated immutable configuration.  It is not
the Product tail width.  Row-local registers begin after the protected prefix,
so a repeated row cannot alias a prefix output or another row.
-/

namespace DClutch.RequestProfileV4Abi

open DClutch
open DClutch.AbiSchema
open DClutch.RequestProfileAbi

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x52, 0x50, 0x30, 0x34]
def schemaVersion : Nat := 4
def artifactProfile : Nat := 4
def finalizedRecordMaxBytes : Nat := RequestProfileAbi.finalizedRecordMaxBytes
def maxRows : Nat := 256
def maxRowBytes : Nat := 4096
def schemaReleasePreimage : List UInt8 :=
  "dclutch/schema/request-profile-v4-authenticated-row-program-v1".toUTF8.toList
def schemaReleaseId : List UInt8 := [
  0x24, 0x8b, 0x83, 0xcc, 0x64, 0x03, 0x2d, 0xf3,
  0xa0, 0x6c, 0xc1, 0x61, 0x97, 0xd6, 0x1a, 0x61,
  0xd2, 0x28, 0xbf, 0xed, 0x08, 0x04, 0x72, 0xb9,
  0xb9, 0x0a, 0x53, 0xd7, 0xd6, 0x85, 0x75, 0x88
]

inductive HeaderField where
  | magic | schemaVersion | artifactProfile | embeddedV1Bytes
  | expectedRowCount | rowBytes | requestRowCountOffset | orderedKeyOffset
  | protectedScalars | rowScalarStride | protectedIdentities | rowIdentityStride
  | rowCountCommonScalar | orderedKeyRowScalar | rowOperationCount | reserved
  deriving DecidableEq, Repr

def headerSchema : List (FieldSpec HeaderField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.embeddedV1Bytes, .u32⟩,
  ⟨.expectedRowCount, .u32⟩,
  ⟨.rowBytes, .u32⟩,
  ⟨.requestRowCountOffset, .u32⟩,
  ⟨.orderedKeyOffset, .u32⟩,
  ⟨.protectedScalars, .u16⟩,
  ⟨.rowScalarStride, .u16⟩,
  ⟨.protectedIdentities, .u16⟩,
  ⟨.rowIdentityStride, .u16⟩,
  ⟨.rowCountCommonScalar, .u16⟩,
  ⟨.orderedKeyRowScalar, .u16⟩,
  ⟨.rowOperationCount, .u16⟩,
  ⟨.reserved, .reserved 18⟩
]

def headerLayout : List (PlacedField HeaderField) := specialize headerSchema
def headerBytes : Nat := schemaWidth headerSchema

namespace HeaderField

def rustName : HeaderField → String
  | .magic => "REQUEST_PROFILE_V4_MAGIC_OFFSET"
  | .schemaVersion => "REQUEST_PROFILE_V4_SCHEMA_VERSION_OFFSET"
  | .artifactProfile => "REQUEST_PROFILE_V4_ARTIFACT_PROFILE_OFFSET"
  | .embeddedV1Bytes => "REQUEST_PROFILE_V4_EMBEDDED_V1_BYTES_OFFSET"
  | .expectedRowCount => "REQUEST_PROFILE_V4_EXPECTED_ROW_COUNT_OFFSET"
  | .rowBytes => "REQUEST_PROFILE_V4_ROW_BYTES_OFFSET"
  | .requestRowCountOffset => "REQUEST_PROFILE_V4_REQUEST_ROW_COUNT_OFFSET_OFFSET"
  | .orderedKeyOffset => "REQUEST_PROFILE_V4_ORDERED_KEY_OFFSET_OFFSET"
  | .protectedScalars => "REQUEST_PROFILE_V4_PROTECTED_SCALARS_OFFSET"
  | .rowScalarStride => "REQUEST_PROFILE_V4_ROW_SCALAR_STRIDE_OFFSET"
  | .protectedIdentities => "REQUEST_PROFILE_V4_PROTECTED_IDENTITIES_OFFSET"
  | .rowIdentityStride => "REQUEST_PROFILE_V4_ROW_IDENTITY_STRIDE_OFFSET"
  | .rowCountCommonScalar => "REQUEST_PROFILE_V4_ROW_COUNT_SCALAR_OFFSET"
  | .orderedKeyRowScalar => "REQUEST_PROFILE_V4_ORDERED_KEY_SCALAR_OFFSET"
  | .rowOperationCount => "REQUEST_PROFILE_V4_ROW_OPERATION_COUNT_OFFSET"
  | .reserved => "REQUEST_PROFILE_V4_RESERVED_OFFSET"

def offset (field : HeaderField) : Nat :=
  (coordinate? field headerLayout).map (·.1) |>.getD 0

end HeaderField

inductive RowOperationField where
  | opcode | reservedHead | requestOffset | targetKind | reservedByte
  | target | immediate | reservedTail
  deriving DecidableEq, Repr

def rowOperationSchema : List (FieldSpec RowOperationField) := [
  ⟨.opcode, .u8⟩,
  ⟨.reservedHead, .reserved 3⟩,
  ⟨.requestOffset, .u32⟩,
  ⟨.targetKind, .u8⟩,
  ⟨.reservedByte, .reserved 1⟩,
  ⟨.target, .u16⟩,
  ⟨.immediate, .u64⟩,
  ⟨.reservedTail, .reserved 4⟩
]

def rowOperationLayout : List (PlacedField RowOperationField) := specialize rowOperationSchema
def rowOperationBytes : Nat := schemaWidth rowOperationSchema

namespace RowOperationField

def rustName : RowOperationField → String
  | .opcode => "REQUEST_PROFILE_V4_ROW_OPCODE_OFFSET"
  | .reservedHead => "REQUEST_PROFILE_V4_ROW_RESERVED_HEAD_OFFSET"
  | .requestOffset => "REQUEST_PROFILE_V4_ROW_REQUEST_OFFSET_OFFSET"
  | .targetKind => "REQUEST_PROFILE_V4_ROW_TARGET_KIND_OFFSET"
  | .reservedByte => "REQUEST_PROFILE_V4_ROW_RESERVED_BYTE_OFFSET"
  | .target => "REQUEST_PROFILE_V4_ROW_TARGET_OFFSET"
  | .immediate => "REQUEST_PROFILE_V4_ROW_IMMEDIATE_OFFSET"
  | .reservedTail => "REQUEST_PROFILE_V4_ROW_RESERVED_TAIL_OFFSET"

def offset (field : RowOperationField) : Nat :=
  (coordinate? field rowOperationLayout).map (·.1) |>.getD 0

end RowOperationField

inductive RowOperationKind where
  | requireU8 | requireU16 | requireU32 | requireU64 | requireZero
  | projectU8 | projectU16 | projectU32 | projectU64 | projectIdentity
  deriving DecidableEq, Repr

namespace RowOperationKind

def tag : RowOperationKind → UInt8
  | .requireU8 => 0 | .requireU16 => 1 | .requireU32 => 2 | .requireU64 => 3
  | .requireZero => 4 | .projectU8 => 5 | .projectU16 => 6 | .projectU32 => 7
  | .projectU64 => 8 | .projectIdentity => 9

def readWidth? : RowOperationKind → Nat → Option Nat
  | .requireU8, _ | .projectU8, _ => some 1
  | .requireU16, _ | .projectU16, _ => some 2
  | .requireU32, _ | .projectU32, _ => some 4
  | .requireU64, _ | .projectU64, _ => some 8
  | .projectIdentity, _ => some 32
  | .requireZero, immediate =>
      if immediate = 0 || immediate ≥ RequestProfileAbi.u32Limit then none else some immediate

def isProjection : RowOperationKind → Bool
  | .projectU8 | .projectU16 | .projectU32 | .projectU64 | .projectIdentity => true
  | _ => false

def isIdentityProjection : RowOperationKind → Bool
  | .projectIdentity => true
  | _ => false

end RowOperationKind

inductive RowTargetKind where | none | scalar | identity
  deriving DecidableEq, Repr

namespace RowTargetKind
def tag : RowTargetKind → UInt8 | .none => 0 | .scalar => 1 | .identity => 2
end RowTargetKind

structure Geometry where
  expectedRowCount : Nat
  rowBytes : Nat
  requestRowCountOffset : Nat
  orderedKeyOffset : Nat
  protectedScalars : Nat
  rowScalarStride : Nat
  protectedIdentities : Nat
  rowIdentityStride : Nat
  rowCountCommonScalar : Nat
  orderedKeyRowScalar : Nat
  deriving DecidableEq, Repr

structure RowOperation where
  kind : RowOperationKind
  requestOffset : Nat
  targetKind : RowTargetKind
  target : Nat
  immediate : Nat
  deriving DecidableEq, Repr

structure Profile where
  embedded : RequestProfileAbi.Profile
  geometry : Geometry
  rowOperations : List RowOperation
  deriving DecidableEq, Repr

def RowOperation.shapeValid (operation : RowOperation) (geometry : Geometry) : Bool :=
  match operation.kind.readWidth? operation.immediate with
  | none => false
  | some width =>
      operation.requestOffset + width ≤ geometry.rowBytes &&
      match operation.kind.isProjection, operation.kind.isIdentityProjection with
      | false, _ => operation.targetKind = .none && operation.target = 0
      | true, false => operation.targetKind = .scalar &&
          operation.target < geometry.rowScalarStride && operation.immediate = 0
      | true, true => operation.targetKind = .identity &&
          operation.target < geometry.rowIdentityStride && operation.immediate = 0

def RowOperation.covers (operation : RowOperation) (byte : Nat) : Bool :=
  match operation.kind.readWidth? operation.immediate with
  | none => false
  | some width => operation.requestOffset ≤ byte && byte < operation.requestOffset + width

def rowProjectionUnique (operations : List RowOperation) : Bool :=
  (List.range operations.length).all fun index =>
    match operations[index]? with
    | none => false
    | some operation =>
        if operation.kind.isProjection then
          (operations.take index).all fun prior =>
            !(prior.kind.isProjection && prior.targetKind = operation.targetKind &&
              prior.target = operation.target)
        else true

def rowCoverageExact (geometry : Geometry) (operations : List RowOperation) : Bool :=
  (List.range geometry.rowBytes).all fun byte =>
    (operations.filter (·.covers byte)).length = 1

def orderedKeyProjected (geometry : Geometry) (operations : List RowOperation) : Bool :=
  operations.any fun operation =>
    operation.kind = .projectU32 && operation.requestOffset = geometry.orderedKeyOffset &&
      operation.targetKind = .scalar && operation.target = geometry.orderedKeyRowScalar

def fixedProjectionWithinPrefix (profile : RequestProfileAbi.Profile)
    (geometry : Geometry) : Bool :=
  profile.fixedOperations.all fun operation =>
    if !operation.kind.isProjection then true
    else if operation.kind.isIdentityProjection then operation.register < geometry.protectedIdentities
    else operation.register < geometry.protectedScalars &&
      operation.register != geometry.rowCountCommonScalar

def Geometry.wellFormed (geometry : Geometry) (embedded : RequestProfileAbi.Profile) : Bool :=
  geometry.expectedRowCount ≠ 0 && geometry.expectedRowCount ≤ maxRows &&
    geometry.rowBytes ≠ 0 && geometry.rowBytes ≤ maxRowBytes &&
    geometry.rowScalarStride ≠ 0 && geometry.rowIdentityStride ≠ 0 &&
    geometry.rowCountCommonScalar < geometry.protectedScalars &&
    geometry.orderedKeyRowScalar < geometry.rowScalarStride &&
    geometry.requestRowCountOffset + 4 ≤ embedded.fixedRequestBytes &&
    geometry.orderedKeyOffset + 4 ≤ geometry.rowBytes &&
    embedded.itemRequestBytes = 0 && embedded.itemOperations = [] &&
    embedded.itemScalarStride = 0 && embedded.itemIdentityStride = 0 &&
    embedded.commonScalars = geometry.protectedScalars +
      geometry.rowScalarStride * geometry.expectedRowCount &&
    embedded.commonIdentities = geometry.protectedIdentities +
      geometry.rowIdentityStride * geometry.expectedRowCount &&
    fixedProjectionWithinPrefix embedded geometry

def Profile.encodedWidth (profile : Profile) : Nat :=
  headerBytes + profile.embedded.encodedWidth + profile.rowOperations.length * rowOperationBytes

def Profile.requestWidth (profile : Profile) : Nat :=
  profile.embedded.fixedRequestBytes + profile.geometry.rowBytes * profile.geometry.expectedRowCount

def Profile.wellFormed (profile : Profile) : Bool :=
  profile.embedded.wellFormed && profile.geometry.wellFormed profile.embedded &&
    profile.rowOperations ≠ [] && profile.rowOperations.length < RequestProfileAbi.u16Limit &&
    profile.encodedWidth ≤ finalizedRecordMaxBytes &&
    profile.rowOperations.all (·.shapeValid profile.geometry) &&
    rowProjectionUnique profile.rowOperations &&
    rowCoverageExact profile.geometry profile.rowOperations &&
    orderedKeyProjected profile.geometry profile.rowOperations

def rowRegisterIndex (base stride row slot : Nat) : Nat :=
  base + stride * row + slot

theorem header_bytes_exact : headerBytes = 64 := by decide
theorem row_operation_bytes_exact : rowOperationBytes = 24 := by decide
theorem schema_coordinates_exact :
    schemaReleasePreimage.length = 62 ∧ schemaReleaseId.length = 32 := by native_decide
theorem row_register_cannot_alias_prefix
    (base stride row slot : Nat) :
    base ≤ rowRegisterIndex base stride row slot := by
  unfold rowRegisterIndex
  exact Nat.le_trans (Nat.le_add_right base (stride * row))
    (Nat.le_add_right (base + stride * row) slot)

end DClutch.RequestProfileV4Abi
