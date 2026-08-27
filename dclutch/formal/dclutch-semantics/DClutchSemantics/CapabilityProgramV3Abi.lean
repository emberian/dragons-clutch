import DClutchSemantics.AbiSchema

/-!
# Capability Program V3 descriptor ABI

The fixed descriptor selects independently finalized AccountProfile,
RequestProfile, EffectProgram, and Transition artifacts. Its manifest release
identity is the SHA-256 digest of these exact 408 bytes; no embedded VM body or
family tag participates in physical authority.
-/

namespace DClutch.CapabilityProgramV3Abi

open DClutch.AbiSchema

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x50, 0x52, 0x33]
def schemaVersion : Nat := 3
def artifactProfile : Nat := 3
def transitionSchemaVersion : Nat := 3
def requestProfileSchemaVersion : Nat := 1

inductive Field where
  | magic | schemaVersion | artifactProfile | transitionSchemaVersion | requestProfileSchemaVersion
  | kind | configSchema | requestSchema | rootSchema | accountProfile
  | derivationPolicy | capacityProfile | effectProgram | requestProfileSchema | requestProfileProgram
  | transitionSchema | transitionProgram | rootStateBytes | tailReserved
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.transitionSchemaVersion, .u16⟩,
  ⟨.requestProfileSchemaVersion, .u16⟩,
  ⟨.kind, .bytes 32⟩,
  ⟨.configSchema, .bytes 32⟩,
  ⟨.requestSchema, .bytes 32⟩,
  ⟨.rootSchema, .bytes 32⟩,
  ⟨.accountProfile, .bytes 32⟩,
  ⟨.derivationPolicy, .bytes 32⟩,
  ⟨.capacityProfile, .bytes 32⟩,
  ⟨.effectProgram, .bytes 32⟩,
  ⟨.requestProfileSchema, .bytes 32⟩,
  ⟨.requestProfileProgram, .bytes 32⟩,
  ⟨.transitionSchema, .bytes 32⟩,
  ⟨.transitionProgram, .bytes 32⟩,
  ⟨.rootStateBytes, .u32⟩,
  ⟨.tailReserved, .reserved 4⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

namespace Field

def rustName : Field → String
  | .magic => "CAPABILITY_PROGRAM_V3_MAGIC_OFFSET"
  | .schemaVersion => "CAPABILITY_PROGRAM_V3_SCHEMA_VERSION_OFFSET"
  | .artifactProfile => "CAPABILITY_PROGRAM_V3_ARTIFACT_PROFILE_OFFSET"
  | .transitionSchemaVersion => "CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION_OFFSET"
  | .requestProfileSchemaVersion => "CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION_OFFSET"
  | .kind => "CAPABILITY_PROGRAM_V3_KIND_OFFSET"
  | .configSchema => "CAPABILITY_PROGRAM_V3_CONFIG_SCHEMA_OFFSET"
  | .requestSchema => "CAPABILITY_PROGRAM_V3_REQUEST_SCHEMA_OFFSET"
  | .rootSchema => "CAPABILITY_PROGRAM_V3_ROOT_SCHEMA_OFFSET"
  | .accountProfile => "CAPABILITY_PROGRAM_V3_ACCOUNT_PROFILE_OFFSET"
  | .derivationPolicy => "CAPABILITY_PROGRAM_V3_DERIVATION_POLICY_OFFSET"
  | .capacityProfile => "CAPABILITY_PROGRAM_V3_CAPACITY_PROFILE_OFFSET"
  | .effectProgram => "CAPABILITY_PROGRAM_V3_EFFECT_PROGRAM_OFFSET"
  | .requestProfileSchema => "CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_OFFSET"
  | .requestProfileProgram => "CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_PROGRAM_OFFSET"
  | .transitionSchema => "CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_OFFSET"
  | .transitionProgram => "CAPABILITY_PROGRAM_V3_TRANSITION_PROGRAM_OFFSET"
  | .rootStateBytes => "CAPABILITY_PROGRAM_V3_ROOT_STATE_BYTES_OFFSET"
  | .tailReserved => "CAPABILITY_PROGRAM_V3_TAIL_RESERVED_OFFSET"

end Field

theorem width_is_exact : bytes = 408 := by native_decide
theorem names_are_unique : (schema.map fun field => field.name).Nodup := by native_decide
theorem fields_are_disjoint : layout.Pairwise Before := specializeFrom_pairwise 0 schema

end DClutch.CapabilityProgramV3Abi
