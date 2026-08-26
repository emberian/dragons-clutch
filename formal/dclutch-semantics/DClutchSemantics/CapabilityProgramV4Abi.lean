import DClutchSemantics.AbiSchema

/-!
# Capability Program V4 descriptor ABI

The fixed descriptor selects six independently finalized executable artifact
classes through explicit schema and content identities. Its manifest release
identity is the SHA-256 digest of these exact 600 bytes; no raw-byte magic,
family tag, or caller schema hint participates in authority.
-/

namespace DClutch.CapabilityProgramV4Abi

open DClutch.AbiSchema

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x50, 0x52, 0x34]
def schemaVersion : Nat := 4
def artifactProfile : Nat := 4

inductive Field where
  | magic | schemaVersion | artifactProfile | headerReserved
  | kind | configSchema | requestSchema | rootSchema | derivationPolicy | capacityProfile
  | accountProfileSchema | accountProfileProgram
  | requestProfileSchema | requestProfileProgram
  | lifecycleSchema | lifecycleProgram
  | strategySchema | strategyProgram
  | transitionSchema | transitionProgram
  | effectSchema | effectProgram
  | rootStateBytes | tailReserved
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.headerReserved, .reserved 4⟩,
  ⟨.kind, .bytes 32⟩,
  ⟨.configSchema, .bytes 32⟩,
  ⟨.requestSchema, .bytes 32⟩,
  ⟨.rootSchema, .bytes 32⟩,
  ⟨.derivationPolicy, .bytes 32⟩,
  ⟨.capacityProfile, .bytes 32⟩,
  ⟨.accountProfileSchema, .bytes 32⟩,
  ⟨.accountProfileProgram, .bytes 32⟩,
  ⟨.requestProfileSchema, .bytes 32⟩,
  ⟨.requestProfileProgram, .bytes 32⟩,
  ⟨.lifecycleSchema, .bytes 32⟩,
  ⟨.lifecycleProgram, .bytes 32⟩,
  ⟨.strategySchema, .bytes 32⟩,
  ⟨.strategyProgram, .bytes 32⟩,
  ⟨.transitionSchema, .bytes 32⟩,
  ⟨.transitionProgram, .bytes 32⟩,
  ⟨.effectSchema, .bytes 32⟩,
  ⟨.effectProgram, .bytes 32⟩,
  ⟨.rootStateBytes, .u32⟩,
  ⟨.tailReserved, .reserved 4⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

namespace Field

def rustName : Field → String
  | .magic => "CAPABILITY_PROGRAM_V4_MAGIC_OFFSET"
  | .schemaVersion => "CAPABILITY_PROGRAM_V4_SCHEMA_VERSION_OFFSET"
  | .artifactProfile => "CAPABILITY_PROGRAM_V4_ARTIFACT_PROFILE_OFFSET"
  | .headerReserved => "CAPABILITY_PROGRAM_V4_HEADER_RESERVED_OFFSET"
  | .kind => "CAPABILITY_PROGRAM_V4_KIND_OFFSET"
  | .configSchema => "CAPABILITY_PROGRAM_V4_CONFIG_SCHEMA_OFFSET"
  | .requestSchema => "CAPABILITY_PROGRAM_V4_REQUEST_SCHEMA_OFFSET"
  | .rootSchema => "CAPABILITY_PROGRAM_V4_ROOT_SCHEMA_OFFSET"
  | .derivationPolicy => "CAPABILITY_PROGRAM_V4_DERIVATION_POLICY_OFFSET"
  | .capacityProfile => "CAPABILITY_PROGRAM_V4_CAPACITY_PROFILE_OFFSET"
  | .accountProfileSchema => "CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_SCHEMA_OFFSET"
  | .accountProfileProgram => "CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_PROGRAM_OFFSET"
  | .requestProfileSchema => "CAPABILITY_PROGRAM_V4_REQUEST_PROFILE_SCHEMA_OFFSET"
  | .requestProfileProgram => "CAPABILITY_PROGRAM_V4_REQUEST_PROFILE_PROGRAM_OFFSET"
  | .lifecycleSchema => "CAPABILITY_PROGRAM_V4_LIFECYCLE_SCHEMA_OFFSET"
  | .lifecycleProgram => "CAPABILITY_PROGRAM_V4_LIFECYCLE_PROGRAM_OFFSET"
  | .strategySchema => "CAPABILITY_PROGRAM_V4_STRATEGY_SCHEMA_OFFSET"
  | .strategyProgram => "CAPABILITY_PROGRAM_V4_STRATEGY_PROGRAM_OFFSET"
  | .transitionSchema => "CAPABILITY_PROGRAM_V4_TRANSITION_SCHEMA_OFFSET"
  | .transitionProgram => "CAPABILITY_PROGRAM_V4_TRANSITION_PROGRAM_OFFSET"
  | .effectSchema => "CAPABILITY_PROGRAM_V4_EFFECT_SCHEMA_OFFSET"
  | .effectProgram => "CAPABILITY_PROGRAM_V4_EFFECT_PROGRAM_OFFSET"
  | .rootStateBytes => "CAPABILITY_PROGRAM_V4_ROOT_STATE_BYTES_OFFSET"
  | .tailReserved => "CAPABILITY_PROGRAM_V4_TAIL_RESERVED_OFFSET"

end Field

theorem width_is_exact : bytes = 600 := by native_decide
theorem names_are_unique : (schema.map fun field => field.name).Nodup := by native_decide
theorem fields_are_disjoint : layout.Pairwise Before := specializeFrom_pairwise 0 schema

end DClutch.CapabilityProgramV4Abi
