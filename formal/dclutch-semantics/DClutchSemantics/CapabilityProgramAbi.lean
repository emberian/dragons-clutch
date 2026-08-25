import DClutchSemantics.AbiSchema
import DClutchSemantics.CapabilityExecutionAbi
import DClutchSemantics.TransitionVMV2

/-!
# Data-defined capability-program and Trading child-root ABIs

The descriptor is finalized, account-resident Registry material whose complete
content identity is the capability manifest entry's semantic `release_id`.
The Trading-owned root account has one immutable header embedding the exact
activation projection followed by a descriptor-sized mutable state tail. The
header is not a second semantic owner. The manifest child schema names the
tail, and hot actions authenticate the root rather than carrying the projection
again.
-/

namespace DClutch.CapabilityProgramAbi

open DClutch.AbiSchema

def descriptorMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x50, 0x52, 0x31]
def rootMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x52, 0x54, 0x31]
def schemaVersion : Nat := 1
def descriptorArtifactProfile : Nat := 2
def rootArtifactProfile : Nat := 1

inductive DescriptorField where
  | magic | schemaVersion | artifactProfile | reserved
  | kind | configSchema | requestSchema | rootSchema | accountProfile
  | derivationPolicy | capacityProfile | effectSchema | rootStateBytes
  | bodyReserved
  deriving DecidableEq, Repr

def descriptorSchema : List (FieldSpec DescriptorField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.reserved, .reserved 4⟩,
  ⟨.kind, .bytes 32⟩,
  ⟨.configSchema, .bytes 32⟩,
  ⟨.requestSchema, .bytes 32⟩,
  ⟨.rootSchema, .bytes 32⟩,
  ⟨.accountProfile, .bytes 32⟩,
  ⟨.derivationPolicy, .bytes 32⟩,
  ⟨.capacityProfile, .bytes 32⟩,
  ⟨.effectSchema, .bytes 32⟩,
  ⟨.rootStateBytes, .u32⟩,
  ⟨.bodyReserved, .reserved 4⟩
]

def descriptorLayout : List (PlacedField DescriptorField) := specialize descriptorSchema
def descriptorHeaderBytes : Nat := schemaWidth descriptorSchema
def finalizedRecordMaxBytes : Nat := 1312
def transitionMaxInstructions : Nat :=
  (finalizedRecordMaxBytes - descriptorHeaderBytes - TransitionVMV2.Codec.headerBytes) /
    TransitionVMV2.Codec.instructionBytes
def transitionMaxBytes : Nat := TransitionVMV2.Codec.headerBytes +
  transitionMaxInstructions * TransitionVMV2.Codec.instructionBytes
def descriptorMaxBytes : Nat := descriptorHeaderBytes +
  transitionMaxBytes
def rootStateMaxBytes : Nat := 4096

namespace DescriptorField

def rustName : DescriptorField → String
  | .magic => "CAPABILITY_PROGRAM_MAGIC_OFFSET"
  | .schemaVersion => "CAPABILITY_PROGRAM_SCHEMA_VERSION_OFFSET"
  | .artifactProfile => "CAPABILITY_PROGRAM_PROFILE_OFFSET"
  | .reserved => "CAPABILITY_PROGRAM_RESERVED_OFFSET"
  | .kind => "CAPABILITY_PROGRAM_KIND_OFFSET"
  | .configSchema => "CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET"
  | .requestSchema => "CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET"
  | .rootSchema => "CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET"
  | .accountProfile => "CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET"
  | .derivationPolicy => "CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET"
  | .capacityProfile => "CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET"
  | .effectSchema => "CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET"
  | .rootStateBytes => "CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET"
  | .bodyReserved => "CAPABILITY_PROGRAM_BODY_RESERVED_OFFSET"

end DescriptorField

inductive RootField where
  | magic | schemaVersion | artifactProfile | reserved
  | releaseSet | market | generation | selection
  deriving DecidableEq, Repr

def rootHeaderSchema : List (FieldSpec RootField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.reserved, .reserved 4⟩,
  ⟨.releaseSet, .bytes 32⟩,
  ⟨.market, .bytes 32⟩,
  ⟨.generation, .u64⟩,
  ⟨.selection, .bytes CapabilityExecutionAbi.bytes⟩
]

def rootHeaderLayout : List (PlacedField RootField) := specialize rootHeaderSchema
def rootHeaderBytes : Nat := schemaWidth rootHeaderSchema
def rootAccountMaxBytes : Nat := rootHeaderBytes + rootStateMaxBytes

namespace RootField

def rustName : RootField → String
  | .magic => "CAPABILITY_ROOT_MAGIC_OFFSET"
  | .schemaVersion => "CAPABILITY_ROOT_SCHEMA_VERSION_OFFSET"
  | .artifactProfile => "CAPABILITY_ROOT_PROFILE_OFFSET"
  | .reserved => "CAPABILITY_ROOT_RESERVED_OFFSET"
  | .releaseSet => "CAPABILITY_ROOT_RELEASE_SET_OFFSET"
  | .market => "CAPABILITY_ROOT_MARKET_OFFSET"
  | .generation => "CAPABILITY_ROOT_GENERATION_OFFSET"
  | .selection => "CAPABILITY_ROOT_SELECTION_OFFSET"

end RootField

theorem descriptor_header_width_is_exact : descriptorHeaderBytes = 280 := by native_decide
theorem transition_max_instructions_is_exact : transitionMaxInstructions = 42 := by native_decide
theorem transition_max_width_is_exact : transitionMaxBytes = 1024 := by native_decide
theorem descriptor_max_width_is_exact : descriptorMaxBytes = 1304 := by native_decide
theorem descriptor_fits_finalized_record_bound :
    descriptorMaxBytes ≤ finalizedRecordMaxBytes := by native_decide
theorem descriptor_names_are_unique :
    (descriptorSchema.map fun field => field.name).Nodup := by native_decide
theorem descriptor_fields_are_disjoint :
    descriptorLayout.Pairwise Before := specializeFrom_pairwise 0 descriptorSchema
theorem root_header_width_is_exact : rootHeaderBytes = 232 := by native_decide
theorem root_account_max_width_is_exact : rootAccountMaxBytes = 4328 := by native_decide
theorem root_names_are_unique :
    (rootHeaderSchema.map fun field => field.name).Nodup := by native_decide
theorem root_fields_are_disjoint :
    rootHeaderLayout.Pairwise Before := specializeFrom_pairwise 0 rootHeaderSchema

end DClutch.CapabilityProgramAbi
