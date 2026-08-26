import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import DClutchSemantics.TransitionVMV2
import Std.Tactic

/-!
# Content-selected request-profile ABI and projection semantics

A RequestProfile validates one fixed request prefix and one item body repeated
for a Product-authenticated runtime count. Fixed-body projections address common
registers; item-body projections address only the corresponding item stride.
The outer adapter remains responsible for finalized-record authentication and
for supplying the authenticated Product count.
-/

namespace DClutch.RequestProfileAbi

open DClutch
open DClutch.AbiSchema

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x52, 0x50, 0x30, 0x31]
def schemaVersion : Nat := 1
def artifactProfile : Nat := 1
def finalizedRecordMaxBytes : Nat := 1312
def schemaReleasePreimage : List UInt8 :=
  "dclutch/schema/request-profile-v1".toUTF8.toList
def schemaReleaseId : List UInt8 := [
  0xaa, 0x02, 0x96, 0xe0, 0xcf, 0xf2, 0x18, 0x8d,
  0x7e, 0x8e, 0x0f, 0xfe, 0x7e, 0x9f, 0xad, 0x92,
  0xb2, 0x25, 0x3f, 0x74, 0x44, 0xa6, 0x93, 0x6c,
  0x40, 0x04, 0x1d, 0x14, 0xd5, 0xca, 0x1c, 0x6c
]

def u16Limit : Nat := 2 ^ 16
def u32Limit : Nat := 2 ^ 32
def u64Limit : Nat := 2 ^ 64

inductive HeaderField where
  | magic | schemaVersion | artifactProfile
  | fixedRequestBytes | itemRequestBytes
  | fixedOperations | itemOperations
  | commonScalars | itemScalarStride
  | commonIdentities | itemIdentityStride
  deriving DecidableEq, Repr

def headerSchema : List (FieldSpec HeaderField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.fixedRequestBytes, .u32⟩,
  ⟨.itemRequestBytes, .u32⟩,
  ⟨.fixedOperations, .u16⟩,
  ⟨.itemOperations, .u16⟩,
  ⟨.commonScalars, .u16⟩,
  ⟨.itemScalarStride, .u16⟩,
  ⟨.commonIdentities, .u16⟩,
  ⟨.itemIdentityStride, .u16⟩
]

def headerLayout : List (PlacedField HeaderField) := specialize headerSchema
def headerBytes : Nat := schemaWidth headerSchema

namespace HeaderField

def rustName : HeaderField → String
  | .magic => "REQUEST_PROFILE_MAGIC_OFFSET"
  | .schemaVersion => "REQUEST_PROFILE_VERSION_OFFSET"
  | .artifactProfile => "REQUEST_PROFILE_ARTIFACT_OFFSET"
  | .fixedRequestBytes => "REQUEST_PROFILE_FIXED_REQUEST_BYTES_OFFSET"
  | .itemRequestBytes => "REQUEST_PROFILE_ITEM_REQUEST_BYTES_OFFSET"
  | .fixedOperations => "REQUEST_PROFILE_FIXED_OPERATIONS_OFFSET"
  | .itemOperations => "REQUEST_PROFILE_ITEM_OPERATIONS_OFFSET"
  | .commonScalars => "REQUEST_PROFILE_COMMON_SCALARS_OFFSET"
  | .itemScalarStride => "REQUEST_PROFILE_ITEM_SCALAR_STRIDE_OFFSET"
  | .commonIdentities => "REQUEST_PROFILE_COMMON_IDENTITIES_OFFSET"
  | .itemIdentityStride => "REQUEST_PROFILE_ITEM_IDENTITY_STRIDE_OFFSET"

def offset (field : HeaderField) : Nat :=
  (coordinate? field headerLayout).map (·.1) |>.getD 0

end HeaderField

inductive OperationField where
  | opcode | requestSpace | registerSpace | reservedByte
  | requestOffset | register | reservedShort | immediate | reserved
  deriving DecidableEq, Repr

def operationSchema : List (FieldSpec OperationField) := [
  ⟨.opcode, .u8⟩,
  ⟨.requestSpace, .u8⟩,
  ⟨.registerSpace, .u8⟩,
  ⟨.reservedByte, .reserved 1⟩,
  ⟨.requestOffset, .u32⟩,
  ⟨.register, .u16⟩,
  ⟨.reservedShort, .reserved 2⟩,
  ⟨.immediate, .u64⟩,
  ⟨.reserved, .reserved 4⟩
]

def operationLayout : List (PlacedField OperationField) := specialize operationSchema
def operationBytes : Nat := schemaWidth operationSchema

namespace OperationField

def rustName : OperationField → String
  | .opcode => "REQUEST_OPERATION_OPCODE_OFFSET"
  | .requestSpace => "REQUEST_OPERATION_REQUEST_SPACE_OFFSET"
  | .registerSpace => "REQUEST_OPERATION_REGISTER_SPACE_OFFSET"
  | .reservedByte => "REQUEST_OPERATION_RESERVED_BYTE_OFFSET"
  | .requestOffset => "REQUEST_OPERATION_REQUEST_OFFSET_OFFSET"
  | .register => "REQUEST_OPERATION_REGISTER_OFFSET"
  | .reservedShort => "REQUEST_OPERATION_RESERVED_SHORT_OFFSET"
  | .immediate => "REQUEST_OPERATION_IMMEDIATE_OFFSET"
  | .reserved => "REQUEST_OPERATION_RESERVED_OFFSET"

def offset (field : OperationField) : Nat :=
  (coordinate? field operationLayout).map (·.1) |>.getD 0

end OperationField

inductive OperationKind where
  | requireU8 | requireU16 | requireU32 | requireU64 | requireZeroRange
  | projectU8 | projectU16 | projectU32 | projectU64 | projectIdentity
  deriving DecidableEq, Repr

namespace OperationKind

def tag : OperationKind → UInt8
  | .requireU8 => 0
  | .requireU16 => 1
  | .requireU32 => 2
  | .requireU64 => 3
  | .requireZeroRange => 4
  | .projectU8 => 5
  | .projectU16 => 6
  | .projectU32 => 7
  | .projectU64 => 8
  | .projectIdentity => 9

def decode : UInt8 → Option OperationKind
  | 0 => some .requireU8
  | 1 => some .requireU16
  | 2 => some .requireU32
  | 3 => some .requireU64
  | 4 => some .requireZeroRange
  | 5 => some .projectU8
  | 6 => some .projectU16
  | 7 => some .projectU32
  | 8 => some .projectU64
  | 9 => some .projectIdentity
  | _ => none

def isProjection : OperationKind → Bool
  | .projectU8 | .projectU16 | .projectU32 | .projectU64 | .projectIdentity => true
  | _ => false

def isIdentityProjection : OperationKind → Bool
  | .projectIdentity => true
  | _ => false

def readWidth? : OperationKind → Nat → Option Nat
  | .requireU8, _ | .projectU8, _ => some 1
  | .requireU16, _ | .projectU16, _ => some 2
  | .requireU32, _ | .projectU32, _ => some 4
  | .requireU64, _ | .projectU64, _ => some 8
  | .projectIdentity, _ => some 32
  | .requireZeroRange, immediate =>
      if immediate = 0 || immediate ≥ u32Limit then none else some immediate

end OperationKind

structure Operation where
  kind : OperationKind
  requestItem : Bool
  registerItem : Bool
  requestOffset : Nat
  register : Nat
  immediate : Nat
  deriving DecidableEq, Repr

structure Profile where
  fixedRequestBytes : Nat
  itemRequestBytes : Nat
  commonScalars : Nat
  itemScalarStride : Nat
  commonIdentities : Nat
  itemIdentityStride : Nat
  fixedOperations : List Operation
  itemOperations : List Operation
  deriving DecidableEq, Repr

def runtimeWidth (common stride count : Nat) : Nat := common + stride * count

def Profile.requestWidth (profile : Profile) (count : Nat) : Nat :=
  runtimeWidth profile.fixedRequestBytes profile.itemRequestBytes count

def Profile.scalarWidth (profile : Profile) (count : Nat) : Nat :=
  runtimeWidth profile.commonScalars profile.itemScalarStride count

def Profile.identityWidth (profile : Profile) (count : Nat) : Nat :=
  runtimeWidth profile.commonIdentities profile.itemIdentityStride count

def Operation.shapeValid (operation : Operation) (profile : Profile) (itemBody : Bool) : Bool :=
  match operation.kind.readWidth? operation.immediate with
  | none => false
  | some width =>
      let requestBound := if itemBody then profile.itemRequestBytes else profile.fixedRequestBytes
      let registerBound :=
        if operation.kind.isIdentityProjection then
          if itemBody then profile.itemIdentityStride else profile.commonIdentities
        else if itemBody then profile.itemScalarStride else profile.commonScalars
      operation.requestItem = itemBody &&
        operation.registerItem = (itemBody && operation.kind.isProjection) &&
        operation.requestOffset + width ≤ requestBound &&
        (if operation.kind.isProjection then
          operation.register < registerBound && operation.immediate = 0
        else operation.register = 0)

def sameProjectionBank (left right : Operation) : Bool :=
  left.kind.isProjection && right.kind.isProjection &&
    left.kind.isIdentityProjection = right.kind.isIdentityProjection &&
    left.registerItem = right.registerItem

def projectionUnique (operations : List Operation) : Bool :=
  (List.range operations.length).all fun index =>
    match operations[index]? with
    | none => false
    | some operation =>
        if operation.kind.isProjection then
          (operations.take index).all fun prior =>
            !(sameProjectionBank operation prior && operation.register = prior.register)
        else true

def Profile.encodedWidth (profile : Profile) : Nat :=
  headerBytes + (profile.fixedOperations.length + profile.itemOperations.length) * operationBytes

def Profile.itemProjectionsLocal (profile : Profile) : Bool :=
  profile.itemOperations.all fun operation =>
    !operation.kind.isProjection || operation.registerItem

def Profile.wellFormed (profile : Profile) : Bool :=
  (profile.fixedRequestBytes ≠ 0 && profile.fixedRequestBytes < u32Limit &&
      profile.itemRequestBytes < u32Limit && profile.fixedOperations ≠ [] &&
      (profile.commonScalars ≠ 0 || profile.itemScalarStride ≠ 0 ||
        profile.commonIdentities ≠ 0 || profile.itemIdentityStride ≠ 0) &&
      ((profile.itemRequestBytes = 0) = (profile.itemOperations = [])) &&
      (profile.itemOperations = [] || profile.itemScalarStride ≠ 0 ||
        profile.itemIdentityStride ≠ 0) &&
      profile.fixedOperations.length < u16Limit &&
      profile.itemOperations.length < u16Limit &&
      profile.commonScalars < u16Limit && profile.itemScalarStride < u16Limit &&
      profile.commonIdentities < u16Limit && profile.itemIdentityStride < u16Limit &&
      profile.encodedWidth ≤ finalizedRecordMaxBytes &&
      profile.fixedOperations.all (·.shapeValid profile false) &&
      profile.itemOperations.all (·.shapeValid profile true) &&
      projectionUnique profile.fixedOperations && projectionUnique profile.itemOperations) &&
    profile.itemProjectionsLocal

def encodeHeader (profile : Profile) : List UInt8 :=
  magic ++ Codec.encodeLE 2 schemaVersion ++ Codec.encodeLE 2 artifactProfile ++
    Codec.encodeLE 4 profile.fixedRequestBytes ++ Codec.encodeLE 4 profile.itemRequestBytes ++
    Codec.encodeLE 2 profile.fixedOperations.length ++
    Codec.encodeLE 2 profile.itemOperations.length ++
    Codec.encodeLE 2 profile.commonScalars ++ Codec.encodeLE 2 profile.itemScalarStride ++
    Codec.encodeLE 2 profile.commonIdentities ++ Codec.encodeLE 2 profile.itemIdentityStride

def encodeOperation (operation : Operation) : List UInt8 :=
  [operation.kind.tag, if operation.requestItem then 1 else 0,
    if operation.registerItem then 1 else 0, 0] ++
    Codec.encodeLE 4 operation.requestOffset ++ Codec.encodeLE 2 operation.register ++
    [0, 0] ++ Codec.encodeLE 8 operation.immediate ++ List.replicate 4 0

def encodeProfile (profile : Profile) : List UInt8 :=
  encodeHeader profile ++ profile.fixedOperations.flatMap encodeOperation ++
    profile.itemOperations.flatMap encodeOperation

def bytesAt (input : List UInt8) (offset width : Nat) : List UInt8 :=
  (input.drop offset).take width

def zeroAt (input : List UInt8) (offset width : Nat) : Bool :=
  (bytesAt input offset width).length = width &&
    (bytesAt input offset width).all (· = 0)

def decodeSpace (byte : UInt8) : Option Bool :=
  match byte with
  | 0 => some false
  | 1 => some true
  | _ => none

def decodeOperation (input : List UInt8) : Option Operation := do
  if input.length != operationBytes ||
      input[OperationField.offset .reservedByte]? != some 0 ||
      !zeroAt input (OperationField.offset .reservedShort) 2 ||
      !zeroAt input (OperationField.offset .reserved) 4 then none else
  let kind ← OperationKind.decode (← input[OperationField.offset .opcode]?)
  let requestItem ← decodeSpace (← input[OperationField.offset .requestSpace]?)
  let registerItem ← decodeSpace (← input[OperationField.offset .registerSpace]?)
  some {
    kind
    requestItem
    registerItem
    requestOffset := Codec.decodeLE (bytesAt input (OperationField.offset .requestOffset) 4)
    register := Codec.decodeLE (bytesAt input (OperationField.offset .register) 2)
    immediate := Codec.decodeLE (bytesAt input (OperationField.offset .immediate) 8)
  }

def decodeMany (count width : Nat) (input : List UInt8) : Option (List Operation) :=
  (List.range count).mapM fun index =>
    decodeOperation (bytesAt input (index * width) width)

def decodeProfile (input : List UInt8) : Option Profile := do
  if input.length < headerBytes || input.length > finalizedRecordMaxBytes ||
      bytesAt input 0 8 != magic ||
      Codec.decodeLE (bytesAt input (HeaderField.offset .schemaVersion) 2) != schemaVersion ||
      Codec.decodeLE (bytesAt input (HeaderField.offset .artifactProfile) 2) != artifactProfile
    then none else
  let fixedRequestBytes := Codec.decodeLE (bytesAt input (HeaderField.offset .fixedRequestBytes) 4)
  let itemRequestBytes := Codec.decodeLE (bytesAt input (HeaderField.offset .itemRequestBytes) 4)
  let fixedCount := Codec.decodeLE (bytesAt input (HeaderField.offset .fixedOperations) 2)
  let itemCount := Codec.decodeLE (bytesAt input (HeaderField.offset .itemOperations) 2)
  let commonScalars := Codec.decodeLE (bytesAt input (HeaderField.offset .commonScalars) 2)
  let itemScalarStride := Codec.decodeLE (bytesAt input (HeaderField.offset .itemScalarStride) 2)
  let commonIdentities := Codec.decodeLE (bytesAt input (HeaderField.offset .commonIdentities) 2)
  let itemIdentityStride :=
    Codec.decodeLE (bytesAt input (HeaderField.offset .itemIdentityStride) 2)
  let expected := headerBytes + (fixedCount + itemCount) * operationBytes
  if input.length != expected then none else
  let body := input.drop headerBytes
  let fixedOperations ← decodeMany fixedCount operationBytes body
  let itemOperations ← decodeMany itemCount operationBytes
    (body.drop (fixedCount * operationBytes))
  let profile : Profile := {
    fixedRequestBytes
    itemRequestBytes
    commonScalars
    itemScalarStride
    commonIdentities
    itemIdentityStride
    fixedOperations
    itemOperations
  }
  if profile.wellFormed then some profile else none

def requestIndex (profile : Profile) (operation : Operation) (item : Option Nat) : Option Nat :=
  if operation.requestItem then do
    let item ← item
    some (profile.fixedRequestBytes + profile.itemRequestBytes * item + operation.requestOffset)
  else some operation.requestOffset

def registerIndex (common stride : Nat) (operation : Operation)
    (item : Option Nat) : Option Nat :=
  if operation.registerItem then do
    let item ← item
    some (common + stride * item + operation.register)
  else some operation.register

def requireState (condition : Bool) (state : TransitionVMV2.State) :
    Option TransitionVMV2.State :=
  if condition then some state else none

def projectOperation (profile : Profile) (request : List UInt8) (item : Option Nat)
    (operation : Operation) (state : TransitionVMV2.State) :
    Option TransitionVMV2.State := do
  let width ← operation.kind.readWidth? operation.immediate
  let start ← requestIndex profile operation item
  let field := bytesAt request start width
  if field.length != width then none else
  match operation.kind with
  | .requireU8 | .requireU16 | .requireU32 | .requireU64 =>
      requireState (Codec.decodeLE field = operation.immediate) state
  | .requireZeroRange => requireState (field.all (· = 0)) state
  | .projectU8 | .projectU16 | .projectU32 | .projectU64 =>
      let register ← registerIndex profile.commonScalars profile.itemScalarStride operation item
      TransitionVMV2.setScalar state register (Codec.decodeLE field)
  | .projectIdentity =>
      let register ← registerIndex profile.commonIdentities profile.itemIdentityStride operation item
      TransitionVMV2.setIdentity state register (Codec.decodeLE field)

def projectOperations (profile : Profile) (request : List UInt8) (item : Option Nat) :
    List Operation → TransitionVMV2.State → Option TransitionVMV2.State
  | [], state => some state
  | operation :: rest, state => do
      let next ← projectOperation profile request item operation state
      projectOperations profile request item rest next

def projectItems (profile : Profile) (request : List UInt8) :
    List Nat → TransitionVMV2.State → Option TransitionVMV2.State
  | [], state => some state
  | item :: rest, state => do
      let next ← projectOperations profile request (some item) profile.itemOperations state
      projectItems profile request rest next

def Profile.stateMatches (profile : Profile) (count : Nat)
    (state : TransitionVMV2.State) : Bool :=
  state.scalars.size = profile.scalarWidth count &&
    state.identities.size = profile.identityWidth count

def project (selectedProfile authenticatedProfile : Nat) (profile : Profile)
    (count : Nat) (request : List UInt8) (input : TransitionVMV2.State) :
    Option TransitionVMV2.State := do
  if selectedProfile = 0 || selectedProfile != authenticatedProfile ||
      count ≥ u32Limit || !profile.wellFormed || !profile.stateMatches count input ||
      request.length != profile.requestWidth count then none else
  let fixed ← projectOperations profile request none profile.fixedOperations input
  projectItems profile request (List.range count) fixed

def commitLast (candidate : TransitionVMV2.State) : Option TransitionVMV2.State →
    TransitionVMV2.State
  | some projected => projected
  | none => candidate

theorem refusal_preserves_candidate (candidate : TransitionVMV2.State) :
    commitLast candidate none = candidate := by rfl

def agreementProfile : Profile := {
  fixedRequestBytes := 16
  itemRequestBytes := 40
  commonScalars := 1
  itemScalarStride := 1
  commonIdentities := 0
  itemIdentityStride := 1
  fixedOperations := [
    ⟨.requireU64, false, false, 0, 0, Codec.decodeLE "DCLTRQ01".toUTF8.toList⟩,
    ⟨.requireU8, false, false, 8, 0, 2⟩,
    ⟨.requireZeroRange, false, false, 9, 0, 7⟩
  ]
  itemOperations := [
    ⟨.projectU64, true, true, 0, 0, 0⟩,
    ⟨.projectIdentity, true, true, 8, 0, 0⟩
  ]
}

def agreementRequestTail2 : List UInt8 :=
  "DCLTRQ01".toUTF8.toList ++ [2] ++ List.replicate 7 0 ++
    Codec.encodeLE 8 3 ++ List.replicate 32 0x31 ++
    Codec.encodeLE 8 4 ++ List.replicate 32 0x41

def agreementInputTail2 : TransitionVMV2.State := {
  scalars := #[9, 9, 9]
  identities := #[Codec.decodeLE (List.replicate 32 9), Codec.decodeLE (List.replicate 32 9)]
}

def agreementOutputTail2 : TransitionVMV2.State := {
  scalars := #[9, 3, 4]
  identities := #[Codec.decodeLE (List.replicate 32 0x31),
    Codec.decodeLE (List.replicate 32 0x41)]
}

def profileRefusalCorpus : List (List UInt8) :=
  let canonical := encodeProfile agreementProfile
  let firstFixed := headerBytes
  let firstItem := headerBytes + agreementProfile.fixedOperations.length * operationBytes
  let secondItem := firstItem + operationBytes
  let zeroRange := headerBytes + 2 * operationBytes
  [
    canonical.set 0 0xff,
    canonical.set (firstFixed + OperationField.offset .opcode) 0xff,
    canonical.set (firstFixed + OperationField.offset .requestSpace) 2,
    canonical.set (firstFixed + OperationField.offset .reservedByte) 1,
    canonical.set (firstFixed + OperationField.offset .reservedShort) 1,
    canonical.set (firstFixed + OperationField.offset .reserved) 1,
    canonical.set (firstItem + OperationField.offset .registerSpace) 0,
    canonical.set (secondItem + OperationField.offset .opcode) 8,
    canonical.set (zeroRange + OperationField.offset .immediate) 0,
    canonical.set (firstItem + OperationField.offset .requestOffset) 39
  ]

def requestRefusalCorpusTail2 : List (List UInt8) := [
  agreementRequestTail2.set 8 3,
  agreementRequestTail2.set 9 1
]

def oversizedProfile : Profile := {
  agreementProfile with
  fixedOperations := List.replicate 54 ⟨.requireU8, false, false, 8, 0, 2⟩
  itemOperations := []
  itemRequestBytes := 0
  itemScalarStride := 0
  itemIdentityStride := 0
}

def oversizedProfileBytes : List UInt8 := encodeProfile oversizedProfile

theorem header_width_is_exact : headerBytes = 32 := by native_decide
theorem operation_width_is_exact : operationBytes = 24 := by native_decide
theorem layouts_are_disjoint :
    headerLayout.Pairwise Before ∧ operationLayout.Pairwise Before := by
  exact ⟨specializeFrom_pairwise 0 headerSchema,
    specializeFrom_pairwise 0 operationSchema⟩
theorem schema_release_coordinates_have_exact_width :
    schemaReleasePreimage.length = 33 ∧ schemaReleaseId.length = 32 := by native_decide
theorem agreement_profile_is_well_formed : agreementProfile.wellFormed = true := by
  native_decide
theorem agreement_profile_width_is_exact :
    (encodeProfile agreementProfile).length = 152 := by native_decide
theorem agreement_profile_round_trip :
    decodeProfile (encodeProfile agreementProfile) = some agreementProfile := by native_decide
theorem agreement_runtime_widths_are_exact :
    agreementProfile.requestWidth 2 = 96 ∧ agreementProfile.scalarWidth 2 = 3 ∧
      agreementProfile.identityWidth 2 = 2 := by native_decide
theorem maximum_runtime_widths_fit_u64 :
    (u32Limit - 1) + (u32Limit - 1) * (u32Limit - 1) < u64Limit ∧
      (u16Limit - 1) + (u16Limit - 1) * (u32Limit - 1) < u64Limit := by
  native_decide
theorem well_formed_item_body_projections_cannot_target_common_registers
    (profile : Profile) (wellFormed : profile.wellFormed = true) :
    profile.itemProjectionsLocal = true := by
  exact (Bool.and_eq_true_iff.mp wellFormed).2
theorem agreement_item_body_projection_policy_is_explicit :
    agreementProfile.itemOperations.all fun operation =>
      !operation.kind.isProjection || operation.registerItem := by native_decide
theorem agreement_projection_is_exact :
    project 1 1 agreementProfile 2 agreementRequestTail2 agreementInputTail2 =
      some agreementOutputTail2 := by native_decide
theorem generated_profile_refusal_corpus_refuses :
    profileRefusalCorpus.all fun hostile => decodeProfile hostile = none := by native_decide
theorem generated_request_refusal_corpus_refuses :
    requestRefusalCorpusTail2.all fun hostile =>
      project 1 1 agreementProfile 2 hostile agreementInputTail2 = none := by native_decide
theorem short_request_refuses :
    project 1 1 agreementProfile 2 (agreementRequestTail2.take 95) agreementInputTail2 = none := by
  native_decide
theorem oversized_profile_refuses :
    oversizedProfileBytes.length = 1328 ∧ decodeProfile oversizedProfileBytes = none := by
  native_decide

end DClutch.RequestProfileAbi
