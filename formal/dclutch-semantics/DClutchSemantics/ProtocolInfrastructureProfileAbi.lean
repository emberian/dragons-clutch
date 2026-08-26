import DClutchSemantics.AbiSchema

/-!
# Protocol infrastructure profile ABI

This schema is the sole byte-layout owner for the immutable per-Core bootstrap
root selecting the exact Registry and Rent artifact releases.  The profile is
initialized once by the current Core ProgramData upgrade-authority signer; it
is not a Registry-owned release set and has no update or close transition.
-/

namespace DClutch.ProtocolInfrastructureProfileAbi

open DClutch.AbiSchema

def profileMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x49, 0x4e, 0x46, 0x31]
def initializeMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x49, 0x49, 0x4e, 0x31]
def schemaVersion : Nat := 1
def artifactProfile : Nat := 1

inductive ProfileField where
  | magic | schemaVersion | artifactProfile | reserved
  | registryProgram | registryArtifactRelease | rentProgram | rentArtifactRelease
  deriving DecidableEq, Repr

def profileSchema : List (FieldSpec ProfileField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.reserved, .reserved 4⟩,
  ⟨.registryProgram, .bytes 32⟩,
  ⟨.registryArtifactRelease, .bytes 32⟩,
  ⟨.rentProgram, .bytes 32⟩,
  ⟨.rentArtifactRelease, .bytes 32⟩
]

def profileLayout : List (PlacedField ProfileField) := specialize profileSchema
def profileBytes : Nat := schemaWidth profileSchema

namespace ProfileField

def rustName : ProfileField → String
  | .magic => "PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_OFFSET_V1"
  | .schemaVersion => "PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V1"
  | .artifactProfile => "PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V1"
  | .reserved => "PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_OFFSET_V1"
  | .registryProgram => "PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V1"
  | .registryArtifactRelease => "PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_ARTIFACT_OFFSET_V1"
  | .rentProgram => "PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_PROGRAM_OFFSET_V1"
  | .rentArtifactRelease => "PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_ARTIFACT_OFFSET_V1"

end ProfileField

inductive InitializeField where
  | magic | schemaVersion | artifactProfile | reserved
  deriving DecidableEq, Repr

def initializeSchema : List (FieldSpec InitializeField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.reserved, .reserved 4⟩
]

def initializeLayout : List (PlacedField InitializeField) := specialize initializeSchema
def initializeBytes : Nat := schemaWidth initializeSchema

namespace InitializeField

def rustName : InitializeField → String
  | .magic => "INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_OFFSET_V1"
  | .schemaVersion => "INITIALIZE_PROTOCOL_INFRASTRUCTURE_SCHEMA_VERSION_OFFSET_V1"
  | .artifactProfile => "INITIALIZE_PROTOCOL_INFRASTRUCTURE_ARTIFACT_PROFILE_OFFSET_V1"
  | .reserved => "INITIALIZE_PROTOCOL_INFRASTRUCTURE_RESERVED_OFFSET_V1"

end InitializeField

theorem profile_width_is_exact : profileBytes = 144 := by native_decide
theorem initialize_width_is_exact : initializeBytes = 16 := by native_decide
theorem profile_names_are_unique :
    (profileSchema.map fun field => field.name).Nodup := by native_decide
theorem profile_fields_are_disjoint :
    profileLayout.Pairwise Before := specializeFrom_pairwise 0 profileSchema
theorem initialize_names_are_unique :
    (initializeSchema.map fun field => field.name).Nodup := by native_decide
theorem initialize_fields_are_disjoint :
    initializeLayout.Pairwise Before := specializeFrom_pairwise 0 initializeSchema

end DClutch.ProtocolInfrastructureProfileAbi
