import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# AccountProfile V2 fixed ABI

The V2 artifact's byte vocabulary: magic, schema identity, the artifact-profile
numbers, the four header cut points, the rule/operation/dynamic-span record
layouts, and the twenty-one operation opcodes.

This module owns LAYOUT and NUMBERING only.  It also owns the prestate ADMISSIBILITY RELATION, which is finite -- thirteen
artifact profiles against six tags -- and therefore a table, however much a
nested match made it look like control flow.

One thing is deliberately outside it, because it is the interpreter's meaning
rather than the wire's shape and is not finite in the same way: what an opcode
requires of its OPERANDS -- identity versus scalar register space, whether a
data offset and stride are canonically zero, whether a selected window must
precede it. Those depend on the register geometry of the profile being
decoded and on the order of earlier operations, so they are not a relation
over two small finite sets. That is the unit after this one.

`AccountProfileV2Profile13` and `AccountProfileV2Profile14` are instances over
this vocabulary, not definitions of it; neither defines an operation opcode.
Note that `AccountProfileAbi.OperationKind` is the V1 vocabulary and is NOT
this one: it names seven of these operations and tags them one through seven,
where V2 tags the same seven zero through six.
-/

namespace DClutch.AccountProfileV2Abi

open DClutch.AbiSchema

/-! ## Artifact identity -/

def magic : List UInt8 := "DCLTAP02".toUTF8.toList
def schemaVersion : Nat := 2
def schemaReleasePreimage : List UInt8 :=
  "dclutch/schema/account-profile-v2".toUTF8.toList
def schemaReleaseId : List UInt8 := [
  0x4b, 0x66, 0x56, 0x93, 0x89, 0x0c, 0x76, 0x23,
  0xb5, 0x65, 0x2b, 0x82, 0xe8, 0x5b, 0x26, 0x4a,
  0xc1, 0xa5, 0x26, 0xe7, 0x6a, 0x3d, 0x8e, 0x3c,
  0x8c, 0x1d, 0xd4, 0xd4, 0x6c, 0xc8, 0xe7, 0xfc
]

/-! ## Artifact profiles

Each successor profile admits everything its predecessor did and one further
coordinate.  The numbers are append-only; a withdrawn profile is never reused.
-/

def artifactProfile : Nat := 2
def selectedWindowArtifactProfile : Nat := 3
def typedScalarArtifactProfile : Nat := 4
def trustedEnvironmentArtifactProfile : Nat := 5
def lifecyclePrestateArtifactProfile : Nat := 6
def adapterAuthenticatedVariableDataArtifactProfile : Nat := 7
def trustedExecutingProgramArtifactProfile : Nat := 8
def adapterAuthenticatedVariableDataAliasArtifactProfile : Nat := 9
def nonzeroU64TailCountArtifactProfile : Nat := 10
def authenticatedRouteAliasArtifactProfile : Nat := 11
def nonzeroU64TailRowsArtifactProfile : Nat := 12
def dynamicFixedSpanArtifactProfile : Nat := 13
def fixedDataPredicateArtifactProfile : Nat := 14

def artifactProfiles : List Nat := [
  artifactProfile, selectedWindowArtifactProfile, typedScalarArtifactProfile,
  trustedEnvironmentArtifactProfile, lifecyclePrestateArtifactProfile,
  adapterAuthenticatedVariableDataArtifactProfile,
  trustedExecutingProgramArtifactProfile,
  adapterAuthenticatedVariableDataAliasArtifactProfile,
  nonzeroU64TailCountArtifactProfile, authenticatedRouteAliasArtifactProfile,
  nonzeroU64TailRowsArtifactProfile, dynamicFixedSpanArtifactProfile,
  fixedDataPredicateArtifactProfile
]

/-! ## Header

One layout, read to one of four cut points.  A profile that predates a trusted
coordinate stops before it and requires the remainder to be zero, so the four
header widths are not four numbers -- they are four prefixes of this schema.
-/

inductive HeaderField where
  | magic | schemaVersion | artifactProfile
  | fixedAccounts | itemAccountStride | fixedOperations | itemOperations
  | commonScalars | itemScalarStride | commonIdentities | itemIdentityStride
  | trustedEnvironmentScalar | trustedEnvironmentKind | trustedEnvironmentReserved
  | trustedExecutingProgramIdentity | trustedExecutingProgramKind
  | trustedExecutingProgramReserved
  | trustedBuiltinIdentity | trustedBuiltinKind | trustedBuiltinReserved
  | dynamicFixedSpanCount | spanHeaderReserved
  deriving DecidableEq, Repr

def headerSchema : List (FieldSpec HeaderField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.fixedAccounts, .u16⟩,
  ⟨.itemAccountStride, .u16⟩,
  ⟨.fixedOperations, .u16⟩,
  ⟨.itemOperations, .u16⟩,
  ⟨.commonScalars, .u16⟩,
  ⟨.itemScalarStride, .u16⟩,
  ⟨.commonIdentities, .u16⟩,
  ⟨.itemIdentityStride, .u16⟩,
  ⟨.trustedEnvironmentScalar, .u16⟩,
  ⟨.trustedEnvironmentKind, .u8⟩,
  ⟨.trustedEnvironmentReserved, .reserved 1⟩,
  ⟨.trustedExecutingProgramIdentity, .u16⟩,
  ⟨.trustedExecutingProgramKind, .u8⟩,
  ⟨.trustedExecutingProgramReserved, .reserved 1⟩,
  ⟨.trustedBuiltinIdentity, .u16⟩,
  ⟨.trustedBuiltinKind, .u8⟩,
  ⟨.trustedBuiltinReserved, .reserved 1⟩,
  ⟨.dynamicFixedSpanCount, .u16⟩,
  ⟨.spanHeaderReserved, .reserved 6⟩
]

def headerLayout : List (PlacedField HeaderField) := specialize headerSchema

namespace HeaderField

def rustName : HeaderField → String
  | .magic => "ACCOUNT_PROFILE_V2_MAGIC_OFFSET"
  | .schemaVersion => "ACCOUNT_PROFILE_V2_VERSION_OFFSET"
  | .artifactProfile => "ACCOUNT_PROFILE_V2_ARTIFACT_PROFILE_OFFSET"
  | .fixedAccounts => "ACCOUNT_PROFILE_V2_FIXED_ACCOUNTS_OFFSET"
  | .itemAccountStride => "ACCOUNT_PROFILE_V2_ITEM_ACCOUNT_STRIDE_OFFSET"
  | .fixedOperations => "ACCOUNT_PROFILE_V2_FIXED_OPERATIONS_OFFSET"
  | .itemOperations => "ACCOUNT_PROFILE_V2_ITEM_OPERATIONS_OFFSET"
  | .commonScalars => "ACCOUNT_PROFILE_V2_COMMON_SCALARS_OFFSET"
  | .itemScalarStride => "ACCOUNT_PROFILE_V2_ITEM_SCALAR_STRIDE_OFFSET"
  | .commonIdentities => "ACCOUNT_PROFILE_V2_COMMON_IDENTITIES_OFFSET"
  | .itemIdentityStride => "ACCOUNT_PROFILE_V2_ITEM_IDENTITY_STRIDE_OFFSET"
  | .trustedEnvironmentScalar => "ACCOUNT_PROFILE_V2_TRUSTED_ENVIRONMENT_SCALAR_OFFSET"
  | .trustedEnvironmentKind => "ACCOUNT_PROFILE_V2_TRUSTED_ENVIRONMENT_KIND_OFFSET"
  | .trustedEnvironmentReserved => "ACCOUNT_PROFILE_V2_TRUSTED_ENVIRONMENT_RESERVED_OFFSET"
  | .trustedExecutingProgramIdentity => "ACCOUNT_PROFILE_V2_TRUSTED_EXECUTING_PROGRAM_IDENTITY_OFFSET"
  | .trustedExecutingProgramKind => "ACCOUNT_PROFILE_V2_TRUSTED_EXECUTING_PROGRAM_KIND_OFFSET"
  | .trustedExecutingProgramReserved => "ACCOUNT_PROFILE_V2_TRUSTED_EXECUTING_PROGRAM_RESERVED_OFFSET"
  | .trustedBuiltinIdentity => "ACCOUNT_PROFILE_V2_TRUSTED_BUILTIN_IDENTITY_OFFSET"
  | .trustedBuiltinKind => "ACCOUNT_PROFILE_V2_TRUSTED_BUILTIN_KIND_OFFSET"
  | .trustedBuiltinReserved => "ACCOUNT_PROFILE_V2_TRUSTED_BUILTIN_RESERVED_OFFSET"
  | .dynamicFixedSpanCount => "ACCOUNT_PROFILE_V2_DYNAMIC_FIXED_SPAN_COUNT_OFFSET"
  | .spanHeaderReserved => "ACCOUNT_PROFILE_V2_DYNAMIC_FIXED_SPAN_RESERVED_OFFSET"

def offset (field : HeaderField) : Nat :=
  (coordinate? field headerLayout).map (·.1) |>.getD 0

end HeaderField

/-- Header width for profiles 2 through 7: everything through the trusted
environment coordinate. -/
def headerBytes : Nat := HeaderField.offset .trustedExecutingProgramIdentity
/-- Header width for profiles 8 through 10. -/
def trustedExecutingProgramHeaderBytes : Nat := HeaderField.offset .trustedBuiltinIdentity
/-- Header width for profiles 11 and 12. -/
def authenticatedRouteAliasHeaderBytes : Nat := HeaderField.offset .dynamicFixedSpanCount
/-- Header width for profile 13.  Profile 14 is the same width and spends the
two bytes after the span count on its predicate count; `AccountProfileV2Profile14`
owns that tail. -/
def dynamicFixedSpanHeaderBytes : Nat := schemaWidth headerSchema

/-! ## Account rule -/

inductive RuleField where
  | privileges | effectPermissions | aliasKind | prestate | aliasIndex | reserved
  | dataLength | dataItemStride
  deriving DecidableEq, Repr

def ruleSchema : List (FieldSpec RuleField) := [
  ⟨.privileges, .u8⟩,
  ⟨.effectPermissions, .u8⟩,
  ⟨.aliasKind, .u8⟩,
  ⟨.prestate, .u8⟩,
  ⟨.aliasIndex, .u16⟩,
  ⟨.reserved, .reserved 2⟩,
  ⟨.dataLength, .u32⟩,
  ⟨.dataItemStride, .u32⟩
]

def ruleLayout : List (PlacedField RuleField) := specialize ruleSchema
def ruleBytes : Nat := schemaWidth ruleSchema

namespace RuleField

def rustName : RuleField → String
  | .privileges => "ACCOUNT_RULE_V2_PRIVILEGES_OFFSET"
  | .effectPermissions => "ACCOUNT_RULE_V2_EFFECT_PERMISSIONS_OFFSET"
  | .aliasKind => "ACCOUNT_RULE_V2_ALIAS_KIND_OFFSET"
  | .prestate => "ACCOUNT_RULE_V2_PRESTATE_OFFSET"
  | .aliasIndex => "ACCOUNT_RULE_V2_ALIAS_INDEX_OFFSET"
  | .reserved => "ACCOUNT_RULE_V2_RESERVED_OFFSET"
  | .dataLength => "ACCOUNT_RULE_V2_DATA_LENGTH_OFFSET"
  | .dataItemStride => "ACCOUNT_RULE_V2_DATA_ITEM_STRIDE_OFFSET"

def offset (field : RuleField) : Nat :=
  (coordinate? field ruleLayout).map (·.1) |>.getD 0

end RuleField

/-! ## Operation record -/

inductive OperationField where
  | opcode | accountSpace | account | registerSpace | reserved | register
  | dataOffset | dataStride
  deriving DecidableEq, Repr

def operationSchema : List (FieldSpec OperationField) := [
  ⟨.opcode, .u8⟩,
  ⟨.accountSpace, .u8⟩,
  ⟨.account, .u16⟩,
  ⟨.registerSpace, .u8⟩,
  ⟨.reserved, .reserved 1⟩,
  ⟨.register, .u16⟩,
  ⟨.dataOffset, .u32⟩,
  ⟨.dataStride, .u32⟩
]

def operationLayout : List (PlacedField OperationField) := specialize operationSchema
def operationBytes : Nat := schemaWidth operationSchema

namespace OperationField

def rustName : OperationField → String
  | .opcode => "ACCOUNT_OPERATION_V2_OPCODE_OFFSET"
  | .accountSpace => "ACCOUNT_OPERATION_V2_ACCOUNT_SPACE_OFFSET"
  | .account => "ACCOUNT_OPERATION_V2_ACCOUNT_OFFSET"
  | .registerSpace => "ACCOUNT_OPERATION_V2_REGISTER_SPACE_OFFSET"
  | .reserved => "ACCOUNT_OPERATION_V2_RESERVED_OFFSET"
  | .register => "ACCOUNT_OPERATION_V2_REGISTER_OFFSET"
  | .dataOffset => "ACCOUNT_OPERATION_V2_DATA_OFFSET_OFFSET"
  | .dataStride => "ACCOUNT_OPERATION_V2_DATA_STRIDE_OFFSET"

def offset (field : OperationField) : Nat :=
  (coordinate? field operationLayout).map (·.1) |>.getD 0

end OperationField

/-! ## Dynamic fixed-span entry -/

inductive DynamicSpanField where
  | insertion | countScalar | ruleStart | ruleStride | minimum | maximum | step
  deriving DecidableEq, Repr

def dynamicSpanSchema : List (FieldSpec DynamicSpanField) := [
  ⟨.insertion, .u16⟩,
  ⟨.countScalar, .u16⟩,
  ⟨.ruleStart, .u16⟩,
  ⟨.ruleStride, .u16⟩,
  ⟨.minimum, .u32⟩,
  ⟨.maximum, .u32⟩,
  ⟨.step, .u32⟩
]

def dynamicSpanLayout : List (PlacedField DynamicSpanField) := specialize dynamicSpanSchema
def dynamicSpanEntryBytes : Nat := schemaWidth dynamicSpanSchema

namespace DynamicSpanField

def rustName : DynamicSpanField → String
  | .insertion => "DYNAMIC_FIXED_SPAN_V2_ENTRY_INSERTION_OFFSET"
  | .countScalar => "DYNAMIC_FIXED_SPAN_V2_ENTRY_COUNT_SCALAR_OFFSET"
  | .ruleStart => "DYNAMIC_FIXED_SPAN_V2_ENTRY_RULE_START_OFFSET"
  | .ruleStride => "DYNAMIC_FIXED_SPAN_V2_ENTRY_RULE_STRIDE_OFFSET"
  | .minimum => "DYNAMIC_FIXED_SPAN_V2_ENTRY_MIN_OFFSET"
  | .maximum => "DYNAMIC_FIXED_SPAN_V2_ENTRY_MAX_OFFSET"
  | .step => "DYNAMIC_FIXED_SPAN_V2_ENTRY_STEP_OFFSET"

def offset (field : DynamicSpanField) : Nat :=
  (coordinate? field dynamicSpanLayout).map (·.1) |>.getD 0

end DynamicSpanField

/-! ## Operation opcodes

Twenty-one opcodes, densely numbered from zero.  This is the V2 vocabulary and
is not `AccountProfileAbi.OperationKind`, which numbers its seven from one.
-/

inductive OperationKind where
  | requireKey | requireOwner | projectKey | projectOwner | projectLamports
  | projectDataU64 | projectDataIdentity | projectDataU32 | projectTailCountU32
  | projectDataU64Affine | projectDataIdentityAffine | selectDataWindow
  | projectDataU64Selected | projectDataIdentitySelected
  | projectDataU64SelectedAffine | projectDataIdentitySelectedAffine
  | projectDataU16 | projectDataU8 | projectNonzeroU64TailCount
  | projectNonzeroU64TailRows | projectDataDigest
  deriving DecidableEq, Repr

namespace OperationKind

def tag : OperationKind → Nat
  | .requireKey => 0
  | .requireOwner => 1
  | .projectKey => 2
  | .projectOwner => 3
  | .projectLamports => 4
  | .projectDataU64 => 5
  | .projectDataIdentity => 6
  | .projectDataU32 => 7
  | .projectTailCountU32 => 8
  | .projectDataU64Affine => 9
  | .projectDataIdentityAffine => 10
  | .selectDataWindow => 11
  | .projectDataU64Selected => 12
  | .projectDataIdentitySelected => 13
  | .projectDataU64SelectedAffine => 14
  | .projectDataIdentitySelectedAffine => 15
  | .projectDataU16 => 16
  | .projectDataU8 => 17
  | .projectNonzeroU64TailCount => 18
  | .projectNonzeroU64TailRows => 19
  | .projectDataDigest => 20

def rustName : OperationKind → String
  | .requireKey => "OP_REQUIRE_KEY_V2"
  | .requireOwner => "OP_REQUIRE_OWNER_V2"
  | .projectKey => "OP_PROJECT_KEY_V2"
  | .projectOwner => "OP_PROJECT_OWNER_V2"
  | .projectLamports => "OP_PROJECT_LAMPORTS_V2"
  | .projectDataU64 => "OP_PROJECT_DATA_U64_V2"
  | .projectDataIdentity => "OP_PROJECT_DATA_IDENTITY_V2"
  | .projectDataU32 => "OP_PROJECT_DATA_U32_V2"
  | .projectTailCountU32 => "OP_PROJECT_TAIL_COUNT_U32_V2"
  | .projectDataU64Affine => "OP_PROJECT_DATA_U64_AFFINE_V2"
  | .projectDataIdentityAffine => "OP_PROJECT_DATA_IDENTITY_AFFINE_V2"
  | .selectDataWindow => "OP_SELECT_DATA_WINDOW_V2"
  | .projectDataU64Selected => "OP_PROJECT_DATA_U64_SELECTED_V2"
  | .projectDataIdentitySelected => "OP_PROJECT_DATA_IDENTITY_SELECTED_V2"
  | .projectDataU64SelectedAffine => "OP_PROJECT_DATA_U64_SELECTED_AFFINE_V2"
  | .projectDataIdentitySelectedAffine => "OP_PROJECT_DATA_IDENTITY_SELECTED_AFFINE_V2"
  | .projectDataU16 => "OP_PROJECT_DATA_U16_V2"
  | .projectDataU8 => "OP_PROJECT_DATA_U8_V2"
  | .projectNonzeroU64TailCount => "OP_PROJECT_NONZERO_U64_TAIL_COUNT_V2"
  | .projectNonzeroU64TailRows => "OP_PROJECT_NONZERO_U64_TAIL_ROWS_V2"
  | .projectDataDigest => "OP_PROJECT_DATA_DIGEST_V2"

end OperationKind

def operationKinds : List OperationKind := [
  .requireKey, .requireOwner, .projectKey, .projectOwner, .projectLamports,
  .projectDataU64, .projectDataIdentity, .projectDataU32, .projectTailCountU32,
  .projectDataU64Affine, .projectDataIdentityAffine, .selectDataWindow,
  .projectDataU64Selected, .projectDataIdentitySelected,
  .projectDataU64SelectedAffine, .projectDataIdentitySelectedAffine,
  .projectDataU16, .projectDataU8, .projectNonzeroU64TailCount,
  .projectNonzeroU64TailRows, .projectDataDigest
]

/-! ## Tag vocabularies

Each tag's NUMBER is here.  Which artifact profile admits which tag is not.
-/

def trustedEnvironmentNone : Nat := 0
def trustedEnvironmentCurrentSlot : Nat := 1
def trustedExecutingProgramNone : Nat := 0
def trustedExecutingProgramCurrent : Nat := 1
def trustedBuiltinNone : Nat := 0
def trustedBuiltinSystemProgram : Nat := 1

def aliasSelfCoordinate : Nat := 0
def aliasFixed : Nat := 1
def aliasSameItem : Nat := 2

def registerSpaceCommon : Nat := 0
def registerSpaceItem : Nat := 1

def prestateExact : Nat := 0
def prestateLifecycleBound : Nat := 1
def prestateAdapterAuthenticatedVariableData : Nat := 2
def prestateAdapterAuthenticatedVariableDataAlias : Nat := 3
def prestateAuthenticatedRouteAlias : Nat := 4
def prestateAuthenticatedOpaqueReadonlyData : Nat := 5

def prestateTags : List Nat := [
  prestateExact, prestateLifecycleBound,
  prestateAdapterAuthenticatedVariableData,
  prestateAdapterAuthenticatedVariableDataAlias,
  prestateAuthenticatedRouteAlias, prestateAuthenticatedOpaqueReadonlyData
]

/-! ## Prestate admissibility

Which prestate tag a rule may carry depends on the artifact profile that
declares it.  The relation is NOT monotone in the profile number: profiles 13
and 14 admit the route-alias and opaque-readonly tags while refusing the
variable-data-alias tag that 9 through 12 admit.  A nested match hides that; a
table states it, and the theorem below makes removing it a visible change.
-/

/-- Which refusal an inadmissible tag earns.  This is a function of the
artifact profile alone, not of the cell. -/
inductive PrestateRefusal where
  | nonCanonicalReserved | invalidLifecyclePrestate | invalidVariableDataPrestate
  deriving DecidableEq, Repr

namespace PrestateRefusal

def tag : PrestateRefusal → Nat
  | .nonCanonicalReserved => 0
  | .invalidLifecyclePrestate => 1
  | .invalidVariableDataPrestate => 2

end PrestateRefusal

def prestateRefusal (profile : Nat) : PrestateRefusal :=
  if profile = lifecyclePrestateArtifactProfile then
    .invalidLifecyclePrestate
  else if adapterAuthenticatedVariableDataArtifactProfile ≤ profile
      && profile ≤ fixedDataPredicateArtifactProfile then
    .invalidVariableDataPrestate
  else
    .nonCanonicalReserved

/-- The admitted tags of one artifact profile, in increasing order. -/
def admissiblePrestates (profile : Nat) : List Nat :=
  if profile = lifecyclePrestateArtifactProfile then
    [prestateExact, prestateLifecycleBound]
  else if profile = adapterAuthenticatedVariableDataArtifactProfile
      || profile = trustedExecutingProgramArtifactProfile then
    [prestateExact, prestateLifecycleBound, prestateAdapterAuthenticatedVariableData]
  else if profile = adapterAuthenticatedVariableDataAliasArtifactProfile
      || profile = nonzeroU64TailCountArtifactProfile
      || profile = nonzeroU64TailRowsArtifactProfile then
    [prestateExact, prestateLifecycleBound, prestateAdapterAuthenticatedVariableData,
      prestateAdapterAuthenticatedVariableDataAlias]
  else if profile = authenticatedRouteAliasArtifactProfile then
    [prestateExact, prestateLifecycleBound, prestateAdapterAuthenticatedVariableData,
      prestateAdapterAuthenticatedVariableDataAlias, prestateAuthenticatedRouteAlias]
  else if profile = dynamicFixedSpanArtifactProfile
      || profile = fixedDataPredicateArtifactProfile then
    [prestateExact, prestateLifecycleBound, prestateAdapterAuthenticatedVariableData,
      prestateAuthenticatedRouteAlias, prestateAuthenticatedOpaqueReadonlyData]
  else
    [prestateExact]

def admissible (profile tag : Nat) : Bool := (admissiblePrestates profile).contains tag

/-- One row of the emitted table per artifact profile, one column per tag. -/
def admissibilityRows : List (List Bool) :=
  artifactProfiles.map fun profile => prestateTags.map (admissible profile)

def refusalClasses : List Nat :=
  artifactProfiles.map fun profile => (prestateRefusal profile).tag

/-! ## Theorems -/

theorem record_widths_are_exact :
    schemaWidth headerSchema = 48 ∧ ruleBytes = 16 ∧ operationBytes = 16 ∧
      dynamicSpanEntryBytes = 20 := by
  native_decide

/-- The four header widths are four prefixes of one schema, not four numbers. -/
theorem header_cut_points_are_prefixes_of_one_layout :
    [headerBytes, trustedExecutingProgramHeaderBytes,
      authenticatedRouteAliasHeaderBytes, dynamicFixedSpanHeaderBytes] =
      [32, 36, 40, 48] := by
  native_decide

theorem header_coordinates_are_canonical : coordinates headerLayout = [
    (.magic, 0, 8), (.schemaVersion, 8, 2), (.artifactProfile, 10, 2),
    (.fixedAccounts, 12, 2), (.itemAccountStride, 14, 2),
    (.fixedOperations, 16, 2), (.itemOperations, 18, 2),
    (.commonScalars, 20, 2), (.itemScalarStride, 22, 2),
    (.commonIdentities, 24, 2), (.itemIdentityStride, 26, 2),
    (.trustedEnvironmentScalar, 28, 2), (.trustedEnvironmentKind, 30, 1),
    (.trustedEnvironmentReserved, 31, 1),
    (.trustedExecutingProgramIdentity, 32, 2),
    (.trustedExecutingProgramKind, 34, 1),
    (.trustedExecutingProgramReserved, 35, 1),
    (.trustedBuiltinIdentity, 36, 2), (.trustedBuiltinKind, 38, 1),
    (.trustedBuiltinReserved, 39, 1),
    (.dynamicFixedSpanCount, 40, 2), (.spanHeaderReserved, 42, 6)
  ] := by native_decide

theorem rule_coordinates_are_canonical : coordinates ruleLayout = [
    (.privileges, 0, 1), (.effectPermissions, 1, 1), (.aliasKind, 2, 1),
    (.prestate, 3, 1), (.aliasIndex, 4, 2), (.reserved, 6, 2),
    (.dataLength, 8, 4), (.dataItemStride, 12, 4)
  ] := by native_decide

theorem operation_coordinates_are_canonical : coordinates operationLayout = [
    (.opcode, 0, 1), (.accountSpace, 1, 1), (.account, 2, 2),
    (.registerSpace, 4, 1), (.reserved, 5, 1), (.register, 6, 2),
    (.dataOffset, 8, 4), (.dataStride, 12, 4)
  ] := by native_decide

theorem dynamic_span_coordinates_are_canonical : coordinates dynamicSpanLayout = [
    (.insertion, 0, 2), (.countScalar, 2, 2), (.ruleStart, 4, 2),
    (.ruleStride, 6, 2), (.minimum, 8, 4), (.maximum, 12, 4), (.step, 16, 4)
  ] := by native_decide

theorem header_fields_are_disjoint : headerLayout.Pairwise Before :=
  specializeFrom_pairwise 0 headerSchema

theorem rule_fields_are_disjoint : ruleLayout.Pairwise Before :=
  specializeFrom_pairwise 0 ruleSchema

theorem operation_fields_are_disjoint : operationLayout.Pairwise Before :=
  specializeFrom_pairwise 0 operationSchema

theorem dynamic_span_fields_are_disjoint : dynamicSpanLayout.Pairwise Before :=
  specializeFrom_pairwise 0 dynamicSpanSchema

/-- The opcodes are dense from zero with no gap and no duplicate.  This is the
statement that fails if an opcode is renumbered or a new one skips a value. -/
theorem opcodes_are_dense_from_zero :
    operationKinds.map OperationKind.tag = List.range 21 := by
  native_decide

/-- V2 numbers the seven V1 operations one lower than V1 does.  Stated so that
reusing the V1 vocabulary here is a visible contradiction rather than a silent
off-by-one. -/
theorem v2_opcodes_are_not_the_v1_tags :
    (operationKinds.take 7).map OperationKind.tag = [0, 1, 2, 3, 4, 5, 6] := by
  native_decide

theorem artifact_profiles_are_dense_from_two :
    artifactProfiles = (List.range 13).map (· + 2) := by
  native_decide

theorem prestate_tags_are_dense_from_zero :
    prestateTags = List.range 6 := by
  native_decide

theorem schema_identity_has_exact_width :
    magic.length = 8 ∧ schemaReleasePreimage.length = 33 ∧
      schemaReleaseId.length = 32 := by
  native_decide

/-! ### Prestate admissibility -/

/-- The relation is total: one row per artifact profile, one column per tag,
with no profile and no tag left undecided. -/
theorem admissibility_is_total_over_profiles_and_tags :
    admissibilityRows.length = artifactProfiles.length ∧
      admissibilityRows.all (fun row => row.length = prestateTags.length) ∧
      refusalClasses.length = artifactProfiles.length := by
  native_decide

/-- The exact table, pinned. -/
theorem admissibility_table_is_exact : admissibilityRows = [
    [true, false, false, false, false, false],
    [true, false, false, false, false, false],
    [true, false, false, false, false, false],
    [true, false, false, false, false, false],
    [true, true, false, false, false, false],
    [true, true, true, false, false, false],
    [true, true, true, false, false, false],
    [true, true, true, true, false, false],
    [true, true, true, true, false, false],
    [true, true, true, true, true, false],
    [true, true, true, true, false, false],
    [true, true, true, false, true, true],
    [true, true, true, false, true, true]
  ] := by native_decide

theorem refusal_classes_are_exact :
    refusalClasses = [0, 0, 0, 0, 1, 2, 2, 2, 2, 2, 2, 2, 2] := by
  native_decide

/-- Every artifact profile admits the exact prestate, which is what makes a
profile-agnostic rule expressible at all. -/
theorem every_profile_admits_exact :
    artifactProfiles.all (fun profile => admissible profile prestateExact) := by
  native_decide

/-- The relation is not monotone in the profile number, and this is the
exception.  Profiles 13 and 14 refuse the tag that 9 through 12 admit while
admitting two that those four do not.  Stated so that "tidying" the table into
a prefix relation is a red theorem rather than a silent widening. -/
theorem span_profiles_refuse_the_alias_tag_their_predecessors_admit :
    [9, 10, 11, 12].all (fun profile =>
        admissible profile prestateAdapterAuthenticatedVariableDataAlias) ∧
      [13, 14].all (fun profile =>
        !admissible profile prestateAdapterAuthenticatedVariableDataAlias
          && admissible profile prestateAuthenticatedRouteAlias
          && admissible profile prestateAuthenticatedOpaqueReadonlyData) := by
  native_decide

end DClutch.AccountProfileV2Abi
