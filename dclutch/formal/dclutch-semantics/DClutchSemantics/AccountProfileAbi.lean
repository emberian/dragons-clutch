import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import DClutchSemantics.TransitionVMV2
import Std.Tactic

/-!
# Content-selected account-profile ABI and projection semantics

An AccountProfile contains no expected key or owner literal. Its relations
read identities from an immutable authenticated TransitionVM V2 input bank.
The profile describes an exact suffix-account frame and projects accepted
observations into a runtime-width candidate bank. The outer adapter remains
responsible for finalized-record content authentication, hashing, PDA
derivation, and runtime account access.
-/

namespace DClutch.AccountProfileAbi

open DClutch
open DClutch.AbiSchema

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x41, 0x50, 0x30, 0x31]
def schemaVersion : Nat := 1
def artifactProfile : Nat := 1
def finalizedRecordMaxBytes : Nat := 1312
def schemaReleasePreimage : List UInt8 :=
  "dclutch/schema/account-profile-v1".toUTF8.toList
def schemaReleaseId : List UInt8 := [
  0xa8, 0xc0, 0x78, 0x71, 0x82, 0xca, 0x87, 0x1d,
  0xca, 0x15, 0xfe, 0x24, 0xef, 0xca, 0xaa, 0xea,
  0xd7, 0xac, 0x28, 0x61, 0xeb, 0xe5, 0xfa, 0x1d,
  0xa8, 0x99, 0x29, 0xee, 0xac, 0x48, 0x22, 0xb9
]
def effectPermissionDebitLamports : Nat := 1
def effectPermissionCreditLamports : Nat := 2
def effectPermissionWriteData : Nat := 4

inductive HeaderField where
  | magic | schemaVersion | artifactProfile | accountCount | operationCount
  | scalarCount | identityCount | reserved
  deriving DecidableEq, Repr

def headerSchema : List (FieldSpec HeaderField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.accountCount, .u16⟩,
  ⟨.operationCount, .u16⟩,
  ⟨.scalarCount, .u16⟩,
  ⟨.identityCount, .u16⟩,
  ⟨.reserved, .reserved 12⟩
]

def headerLayout : List (PlacedField HeaderField) := specialize headerSchema
def headerBytes : Nat := schemaWidth headerSchema

namespace HeaderField

def rustName : HeaderField → String
  | .magic => "ACCOUNT_PROFILE_MAGIC_OFFSET"
  | .schemaVersion => "ACCOUNT_PROFILE_VERSION_OFFSET"
  | .artifactProfile => "ACCOUNT_PROFILE_ARTIFACT_OFFSET"
  | .accountCount => "ACCOUNT_PROFILE_ACCOUNT_COUNT_OFFSET"
  | .operationCount => "ACCOUNT_PROFILE_OPERATION_COUNT_OFFSET"
  | .scalarCount => "ACCOUNT_PROFILE_SCALAR_COUNT_OFFSET"
  | .identityCount => "ACCOUNT_PROFILE_IDENTITY_COUNT_OFFSET"
  | .reserved => "ACCOUNT_PROFILE_RESERVED_OFFSET"

def offset (field : HeaderField) : Nat :=
  (coordinate? field headerLayout).map (·.1) |>.getD 0

end HeaderField

inductive RuleField where
  | privileges | effectPermissions | aliasOf | dataLength | reserved
  deriving DecidableEq, Repr

def ruleSchema : List (FieldSpec RuleField) := [
  ⟨.privileges, .u8⟩,
  ⟨.effectPermissions, .u8⟩,
  ⟨.aliasOf, .u16⟩,
  ⟨.dataLength, .u32⟩,
  ⟨.reserved, .reserved 8⟩
]

def ruleLayout : List (PlacedField RuleField) := specialize ruleSchema
def ruleBytes : Nat := schemaWidth ruleSchema

namespace RuleField

def rustName : RuleField → String
  | .privileges => "ACCOUNT_RULE_PRIVILEGES_OFFSET"
  | .effectPermissions => "ACCOUNT_RULE_EFFECT_PERMISSIONS_OFFSET"
  | .aliasOf => "ACCOUNT_RULE_ALIAS_OF_OFFSET"
  | .dataLength => "ACCOUNT_RULE_DATA_LENGTH_OFFSET"
  | .reserved => "ACCOUNT_RULE_RESERVED_OFFSET"

def offset (field : RuleField) : Nat :=
  (coordinate? field ruleLayout).map (·.1) |>.getD 0

end RuleField

inductive OperationField where
  | opcode | reservedByte | account | register | reservedShort | dataOffset | reserved
  deriving DecidableEq, Repr

def operationSchema : List (FieldSpec OperationField) := [
  ⟨.opcode, .u8⟩,
  ⟨.reservedByte, .reserved 1⟩,
  ⟨.account, .u16⟩,
  ⟨.register, .u16⟩,
  ⟨.reservedShort, .reserved 2⟩,
  ⟨.dataOffset, .u32⟩,
  ⟨.reserved, .reserved 4⟩
]

def operationLayout : List (PlacedField OperationField) := specialize operationSchema
def operationBytes : Nat := schemaWidth operationSchema

namespace OperationField

def rustName : OperationField → String
  | .opcode => "ACCOUNT_OPERATION_OPCODE_OFFSET"
  | .reservedByte => "ACCOUNT_OPERATION_RESERVED_BYTE_OFFSET"
  | .account => "ACCOUNT_OPERATION_ACCOUNT_OFFSET"
  | .register => "ACCOUNT_OPERATION_REGISTER_OFFSET"
  | .reservedShort => "ACCOUNT_OPERATION_RESERVED_SHORT_OFFSET"
  | .dataOffset => "ACCOUNT_OPERATION_DATA_OFFSET"
  | .reserved => "ACCOUNT_OPERATION_RESERVED_OFFSET"

def offset (field : OperationField) : Nat :=
  (coordinate? field operationLayout).map (·.1) |>.getD 0

end OperationField

structure AccountRule where
  privileges : Nat
  effectPermissions : Nat
  aliasOf : Nat
  dataLength : Nat
  deriving DecidableEq, Repr

inductive OperationKind where
  | requireKeyEqIdentity
  | requireOwnerEqIdentity
  | projectKey
  | projectOwner
  | projectLamports
  | projectDataU64
  | projectDataIdentity
  deriving DecidableEq, Repr

namespace OperationKind

def tag : OperationKind → UInt8
  | .requireKeyEqIdentity => 1
  | .requireOwnerEqIdentity => 2
  | .projectKey => 3
  | .projectOwner => 4
  | .projectLamports => 5
  | .projectDataU64 => 6
  | .projectDataIdentity => 7

def decode : UInt8 → Option OperationKind
  | 1 => some .requireKeyEqIdentity
  | 2 => some .requireOwnerEqIdentity
  | 3 => some .projectKey
  | 4 => some .projectOwner
  | 5 => some .projectLamports
  | 6 => some .projectDataU64
  | 7 => some .projectDataIdentity
  | _ => none

def isRequirement : OperationKind → Bool
  | .requireKeyEqIdentity | .requireOwnerEqIdentity => true
  | _ => false

def isIdentityProjection : OperationKind → Bool
  | .projectKey | .projectOwner | .projectDataIdentity => true
  | _ => false

def isScalarProjection : OperationKind → Bool
  | .projectLamports | .projectDataU64 => true
  | _ => false

def fieldWidth : OperationKind → Nat
  | .projectDataU64 => 8
  | .projectDataIdentity => 32
  | _ => 0

end OperationKind

structure Operation where
  kind : OperationKind
  account : Nat
  register : Nat
  dataOffset : Nat
  deriving DecidableEq, Repr

structure Profile where
  scalarWidth : Nat
  identityWidth : Nat
  accounts : List AccountRule
  operations : List Operation
  deriving DecidableEq, Repr

def sameProjectionBank (left right : Operation) : Bool :=
  (left.kind.isIdentityProjection && right.kind.isIdentityProjection) ||
    (left.kind.isScalarProjection && right.kind.isScalarProjection)

def Operation.shapeValid (operation : Operation) (profile : Profile) : Bool :=
  operation.account < profile.accounts.length &&
    (if operation.kind.isRequirement || operation.kind.isIdentityProjection then
      operation.register < profile.identityWidth
    else operation.register < profile.scalarWidth) &&
    (if operation.kind.fieldWidth = 0 then operation.dataOffset = 0
    else
      operation.dataOffset + operation.kind.fieldWidth ≤
        ((profile.accounts[operation.account]?).map (·.dataLength)).getD 0)

def runtimeWritable (privileges : Nat) : Bool :=
  privileges = 2 || privileges = 3 || privileges = 6 || privileges = 7

def AccountRule.shapeValid (rule : AccountRule) (index : Nat) (profile : Profile) : Bool :=
  rule.privileges < 8 && rule.effectPermissions < 8 && rule.aliasOf ≤ index &&
    (rule.effectPermissions = 0 || runtimeWritable rule.privileges) &&
    ((profile.accounts[rule.aliasOf]?).map (·.aliasOf == rule.aliasOf)).getD false

def enumerate {alpha : Type} (values : List alpha) : List (Nat × alpha) :=
  (List.range values.length).zip values

def projectionUnique (profile : Profile) : Bool :=
  enumerate profile.operations |>.all fun indexed =>
    let index := indexed.1
    let operation := indexed.2
    if operation.kind.isIdentityProjection || operation.kind.isScalarProjection then
      (profile.operations.take index).all fun prior =>
        !(sameProjectionBank operation prior && operation.register = prior.register)
    else true

def authorityPreserved (profile : Profile) : Bool :=
  profile.operations.all fun projected =>
    if projected.kind.isIdentityProjection then
      profile.operations.all fun required =>
        !(required.kind.isRequirement && required.register = projected.register)
    else true

def representativesAnchored (profile : Profile) : Bool :=
  enumerate profile.accounts |>.all fun indexed =>
    let index := indexed.1
    let rule := indexed.2
    if rule.aliasOf = index then
      profile.operations.any fun operation =>
        operation.account = index && operation.kind.isRequirement
    else true

def aliasRulesConsistent (profile : Profile) : Bool :=
  enumerate profile.accounts |>.all fun indexed =>
    let index := indexed.1
    let rule := indexed.2
    if rule.aliasOf = index then true
    else
      match profile.accounts[rule.aliasOf]? with
      | some representative =>
          rule.privileges = representative.privileges &&
            rule.effectPermissions = representative.effectPermissions &&
            rule.dataLength = representative.dataLength
      | none => false

def effectAuthorityValid (profile : Profile) : Bool :=
  enumerate profile.accounts |>.all fun indexed =>
    let rule := indexed.2
    if rule.effectPermissions = 0 ||
        rule.effectPermissions = effectPermissionCreditLamports then true
    else
      profile.operations.any fun operation =>
        operation.account = rule.aliasOf && operation.kind = .requireOwnerEqIdentity

def Profile.wellFormed (profile : Profile) : Bool :=
  profile.accounts ≠ [] && profile.operations ≠ [] &&
    (profile.scalarWidth ≠ 0 || profile.identityWidth ≠ 0) &&
    profile.accounts.length < TransitionVMV2.u16Limit &&
    profile.operations.length < TransitionVMV2.u16Limit &&
    profile.scalarWidth < TransitionVMV2.u16Limit &&
    profile.identityWidth < TransitionVMV2.u16Limit &&
    (enumerate profile.accounts |>.all fun indexed =>
      indexed.2.shapeValid indexed.1 profile) &&
    profile.operations.all (·.shapeValid profile) &&
    (profile.operations.any fun operation =>
      operation.kind.isIdentityProjection || operation.kind.isScalarProjection) &&
    projectionUnique profile && authorityPreserved profile &&
    representativesAnchored profile && aliasRulesConsistent profile &&
    effectAuthorityValid profile

def encodeHeader (profile : Profile) : List UInt8 :=
  magic ++ Codec.encodeLE 2 schemaVersion ++ Codec.encodeLE 2 artifactProfile ++
    Codec.encodeLE 2 profile.accounts.length ++
    Codec.encodeLE 2 profile.operations.length ++
    Codec.encodeLE 2 profile.scalarWidth ++ Codec.encodeLE 2 profile.identityWidth ++
    List.replicate 12 0

def encodeRule (rule : AccountRule) : List UInt8 :=
  [UInt8.ofNat rule.privileges, UInt8.ofNat rule.effectPermissions] ++
    Codec.encodeLE 2 rule.aliasOf ++
    Codec.encodeLE 4 rule.dataLength ++ List.replicate 8 0

def encodeOperation (operation : Operation) : List UInt8 :=
  [operation.kind.tag, 0] ++ Codec.encodeLE 2 operation.account ++
    Codec.encodeLE 2 operation.register ++ [0, 0] ++
    Codec.encodeLE 4 operation.dataOffset ++ List.replicate 4 0

def encodeProfile (profile : Profile) : List UInt8 :=
  encodeHeader profile ++ profile.accounts.flatMap encodeRule ++
    profile.operations.flatMap encodeOperation

def bytesAt (input : List UInt8) (offset width : Nat) : List UInt8 :=
  (input.drop offset).take width

def zeroAt (input : List UInt8) (offset width : Nat) : Bool :=
  (bytesAt input offset width).length = width &&
    (bytesAt input offset width).all (· = 0)

def decodeRule (input : List UInt8) : Option AccountRule := do
  if input.length != ruleBytes || !zeroAt input (RuleField.offset .reserved) 8 then none else
  let privileges := Codec.decodeLE (bytesAt input (RuleField.offset .privileges) 1)
  let effectPermissions := Codec.decodeLE (bytesAt input (RuleField.offset .effectPermissions) 1)
  if privileges ≥ 8 || effectPermissions ≥ 8 then none else
  some {
    privileges
    effectPermissions
    aliasOf := Codec.decodeLE (bytesAt input (RuleField.offset .aliasOf) 2)
    dataLength := Codec.decodeLE (bytesAt input (RuleField.offset .dataLength) 4)
  }

def decodeOperation (input : List UInt8) : Option Operation := do
  if input.length != operationBytes ||
      input[OperationField.offset .reservedByte]? != some 0 ||
      !zeroAt input (OperationField.offset .reservedShort) 2 ||
      !zeroAt input (OperationField.offset .reserved) 4 then none else
  let tag ← input[OperationField.offset .opcode]?
  let kind ← OperationKind.decode tag
  let dataOffset := Codec.decodeLE (bytesAt input (OperationField.offset .dataOffset) 4)
  if kind.fieldWidth = 0 && dataOffset != 0 then none else
  some {
    kind
    account := Codec.decodeLE (bytesAt input (OperationField.offset .account) 2)
    register := Codec.decodeLE (bytesAt input (OperationField.offset .register) 2)
    dataOffset
  }

def decodeMany {α : Type} (count width : Nat) (decoder : List UInt8 → Option α)
    (input : List UInt8) : Option (List α) :=
  (List.range count).mapM fun index => decoder (bytesAt input (index * width) width)

def decodeProfile (input : List UInt8) : Option Profile := do
  if input.length < headerBytes || input.length > finalizedRecordMaxBytes ||
      bytesAt input 0 8 != magic ||
      Codec.decodeLE (bytesAt input (HeaderField.offset .schemaVersion) 2) != schemaVersion ||
      Codec.decodeLE (bytesAt input (HeaderField.offset .artifactProfile) 2) != artifactProfile ||
      !zeroAt input (HeaderField.offset .reserved) 12 then none else
  let accountCount := Codec.decodeLE (bytesAt input (HeaderField.offset .accountCount) 2)
  let operationCount := Codec.decodeLE (bytesAt input (HeaderField.offset .operationCount) 2)
  let scalarWidth := Codec.decodeLE (bytesAt input (HeaderField.offset .scalarCount) 2)
  let identityWidth := Codec.decodeLE (bytesAt input (HeaderField.offset .identityCount) 2)
  if accountCount = 0 || operationCount = 0 || (scalarWidth = 0 && identityWidth = 0) ||
      input.length != headerBytes + accountCount * ruleBytes + operationCount * operationBytes
    then none else
  let ruleBody := (input.drop headerBytes).take (accountCount * ruleBytes)
  let operationBody := input.drop (headerBytes + accountCount * ruleBytes)
  let accounts ← decodeMany accountCount ruleBytes decodeRule ruleBody
  let operations ← decodeMany operationCount operationBytes decodeOperation operationBody
  let profile : Profile := { scalarWidth, identityWidth, accounts, operations }
  if profile.wellFormed then some profile else none

structure AccountObservation where
  key : Nat
  owner : Nat
  lamports : Nat
  data : List UInt8
  signer : Bool
  writable : Bool
  executable : Bool
  deriving DecidableEq, Repr

def privilegeBits (account : AccountObservation) : Nat :=
  (if account.signer then 1 else 0) +
    (if account.writable then 2 else 0) +
    (if account.executable then 4 else 0)

def aliasesValid (profile : Profile) (accounts : List AccountObservation) : Bool :=
  enumerate accounts |>.all fun left =>
    enumerate accounts |>.all fun right =>
      if left.1 < right.1 then
        let leftRule := profile.accounts[left.1]?
        let rightRule := profile.accounts[right.1]?
        match leftRule, rightRule with
        | some l, some r =>
            (left.2.key = right.2.key) = (l.aliasOf = r.aliasOf) &&
              (!(l.aliasOf = r.aliasOf) || left.2 = right.2)
        | _, _ => false
      else true

def AccountObservation.matchesRule (account : AccountObservation) (rule : AccountRule) : Bool :=
  privilegeBits account = rule.privileges && account.data.length = rule.dataLength

def requirementsAccept (profile : Profile) (accounts : List AccountObservation)
    (input : TransitionVMV2.State) : Bool :=
  profile.operations.all fun operation =>
    match operation.kind with
    | .requireKeyEqIdentity =>
        match accounts[operation.account]?, input.identities[operation.register]? with
        | some account, some expected => account.key = expected
        | _, _ => false
    | .requireOwnerEqIdentity =>
        match accounts[operation.account]?, input.identities[operation.register]? with
        | some account, some expected => account.owner = expected
        | _, _ => false
    | _ => true

def projectOperation (accounts : List AccountObservation) (operation : Operation)
    (state : TransitionVMV2.State) : Option TransitionVMV2.State := do
  let account ← accounts[operation.account]?
  match operation.kind with
  | .requireKeyEqIdentity | .requireOwnerEqIdentity => some state
  | .projectKey => TransitionVMV2.setIdentity state operation.register account.key
  | .projectOwner => TransitionVMV2.setIdentity state operation.register account.owner
  | .projectLamports => TransitionVMV2.setScalar state operation.register account.lamports
  | .projectDataU64 =>
      if operation.dataOffset + 8 ≤ account.data.length then
        TransitionVMV2.setScalar state operation.register
          (Codec.decodeLE (bytesAt account.data operation.dataOffset 8))
      else none
  | .projectDataIdentity =>
      if operation.dataOffset + 32 ≤ account.data.length then
        TransitionVMV2.setIdentity state operation.register
          (Codec.decodeLE (bytesAt account.data operation.dataOffset 32))
      else none

def projectOperations (accounts : List AccountObservation) :
    List Operation → TransitionVMV2.State → Option TransitionVMV2.State
  | [], state => some state
  | operation :: rest, state => do
      let next ← projectOperation accounts operation state
      projectOperations accounts rest next

def Profile.stateMatches (profile : Profile) (state : TransitionVMV2.State) : Bool :=
  state.scalars.size = profile.scalarWidth && state.identities.size = profile.identityWidth

structure EffectPermission where
  mayDebitLamports : Bool
  mayCreditLamports : Bool
  mayWriteData : Bool
  deriving DecidableEq, Repr

def AccountRule.effectPermission (rule : AccountRule) : EffectPermission := {
  mayDebitLamports := rule.effectPermissions = effectPermissionDebitLamports ||
    rule.effectPermissions = effectPermissionDebitLamports + effectPermissionCreditLamports ||
    rule.effectPermissions = effectPermissionDebitLamports + effectPermissionWriteData ||
    rule.effectPermissions = effectPermissionDebitLamports + effectPermissionCreditLamports +
      effectPermissionWriteData
  mayCreditLamports := rule.effectPermissions = effectPermissionCreditLamports ||
    rule.effectPermissions = effectPermissionDebitLamports + effectPermissionCreditLamports ||
    rule.effectPermissions = effectPermissionCreditLamports + effectPermissionWriteData ||
    rule.effectPermissions = effectPermissionDebitLamports + effectPermissionCreditLamports +
      effectPermissionWriteData
  mayWriteData := rule.effectPermissions ≥ effectPermissionWriteData
}

def Profile.effectPermissionBank (profile : Profile) : List EffectPermission :=
  profile.accounts.map (·.effectPermission)

def project (selectedProfile authenticatedProfile : Nat) (profile : Profile)
    (accounts : List AccountObservation) (input : TransitionVMV2.State) :
    Option TransitionVMV2.State := do
  if selectedProfile = 0 || selectedProfile != authenticatedProfile ||
      !profile.wellFormed || !profile.stateMatches input ||
      accounts.length != profile.accounts.length ||
      !((accounts.zip profile.accounts).all fun pair => pair.1.matchesRule pair.2) ||
      !aliasesValid profile accounts || !requirementsAccept profile accounts input
    then none else
  projectOperations accounts profile.operations input

def commitLast (candidate : TransitionVMV2.State) : Option TransitionVMV2.State →
    TransitionVMV2.State
  | some projected => projected
  | none => candidate

theorem refusal_preserves_candidate (candidate : TransitionVMV2.State) :
    commitLast candidate none = candidate := by rfl

def activationResourceProfile : Profile := {
  scalarWidth := 20
  identityWidth := 16
  accounts := [
    ⟨0, 0, 0, 64⟩,
    ⟨0, 0, 1, 232⟩,
    ⟨2, 1, 2, 0⟩,
    ⟨0, 0, 3, 40⟩
  ]
  operations := [
    ⟨.requireOwnerEqIdentity, 0, 8, 0⟩,
    ⟨.requireOwnerEqIdentity, 1, 8, 0⟩,
    ⟨.requireKeyEqIdentity, 0, 9, 0⟩,
    ⟨.requireKeyEqIdentity, 1, 10, 0⟩,
    ⟨.requireKeyEqIdentity, 2, 6, 0⟩,
    ⟨.requireKeyEqIdentity, 3, 11, 0⟩,
    ⟨.requireOwnerEqIdentity, 2, 7, 0⟩,
    ⟨.projectKey, 2, 12, 0⟩,
    ⟨.projectOwner, 2, 13, 0⟩,
    ⟨.projectLamports, 2, 14, 0⟩,
    ⟨.projectDataU64, 1, 15, 112⟩,
    ⟨.projectDataU64, 1, 16, 120⟩,
    ⟨.projectDataU64, 3, 17, 0⟩,
    ⟨.projectDataIdentity, 1, 14, 16⟩,
    ⟨.projectDataIdentity, 1, 15, 80⟩
  ]
}

def activationResourceProfileContentId : List UInt8 := [
  0x6c, 0x69, 0x0f, 0x04, 0x19, 0xd3, 0xee, 0x02,
  0x80, 0x0c, 0xa9, 0x18, 0xbc, 0x02, 0xf4, 0xce,
  0xa6, 0x7a, 0xf3, 0xd8, 0x70, 0x3b, 0x71, 0x9e,
  0xce, 0x3c, 0xb5, 0x5b, 0x75, 0x94, 0x88, 0x5f
]

def aliasAgreementProfile : Profile := {
  scalarWidth := 1
  identityWidth := 1
  accounts := [
    ⟨0, 0, 0, 8⟩,
    ⟨0, 0, 0, 8⟩
  ]
  operations := [
    ⟨.requireKeyEqIdentity, 0, 0, 0⟩,
    ⟨.projectLamports, 1, 0, 0⟩
  ]
}

def refusalCorpus : List (List UInt8) :=
  let canonical := encodeProfile activationResourceProfile
  let operationsStart := headerBytes + activationResourceProfile.accounts.length * ruleBytes
  [
    canonical.set operationsStart 0xff,
    canonical.set (operationsStart + OperationField.offset .reservedByte) 1,
    canonical.set (headerBytes + ruleBytes + RuleField.offset .aliasOf) 2,
    canonical.set (operationsStart + OperationField.offset .register) 16,
    canonical.set (headerBytes + RuleField.offset .effectPermissions) 8,
    canonical.set (headerBytes + RuleField.offset .effectPermissions) 1,
    canonical.set (operationsStart + 6 * operationBytes + OperationField.offset .opcode) 1
  ]

theorem header_width_is_exact : headerBytes = 32 := by native_decide
theorem schema_release_coordinates_have_exact_width :
    schemaReleasePreimage.length = 33 ∧ schemaReleaseId.length = 32 := by native_decide
theorem activation_profile_content_identity_has_exact_width :
    activationResourceProfileContentId.length = 32 := by native_decide
theorem rule_width_is_exact : ruleBytes = 16 := by native_decide
theorem operation_width_is_exact : operationBytes = 16 := by native_decide
theorem layouts_are_disjoint :
    headerLayout.Pairwise Before ∧ ruleLayout.Pairwise Before ∧
      operationLayout.Pairwise Before := by
  exact ⟨specializeFrom_pairwise 0 headerSchema,
    specializeFrom_pairwise 0 ruleSchema, specializeFrom_pairwise 0 operationSchema⟩
theorem activation_profile_is_family_neutral_and_well_formed :
    activationResourceProfile.wellFormed = true := by native_decide
theorem activation_profile_width_is_exact :
    (encodeProfile activationResourceProfile).length = 336 := by native_decide
theorem activation_profile_round_trip :
    decodeProfile (encodeProfile activationResourceProfile) = some activationResourceProfile := by
  native_decide
theorem alias_agreement_profile_is_well_formed :
    aliasAgreementProfile.wellFormed = true := by native_decide
theorem alias_agreement_profile_round_trip :
    decodeProfile (encodeProfile aliasAgreementProfile) = some aliasAgreementProfile := by
  native_decide
theorem generated_refusal_corpus_refuses :
    refusalCorpus.all fun hostile => decodeProfile hostile = none := by native_decide

theorem effect_permission_bank_has_exact_account_width (profile : Profile) :
    profile.effectPermissionBank.length = profile.accounts.length := by
  simp [Profile.effectPermissionBank]

end DClutch.AccountProfileAbi
