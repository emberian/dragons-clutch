import DClutchSemantics.AbiSchema

/-!
# Fixed-role capability execution-selection ABI

This schema is the sole byte-layout owner for the derived projection from one
authenticated capability-manifest entry to the canonical Trading execution
role. The wire deliberately contains no role, Program, ProgramData, artifact,
or family tag. Activation and closure carry it before the exact child-owned
request; hot actions consume the persisted child-root selection instead.
-/

namespace DClutch.CapabilityExecutionAbi

open DClutch.AbiSchema

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x45, 0x52, 0x31]
def schemaVersion : Nat := 1
def artifactProfile : Nat := 1

inductive Field where
  | magic | schemaVersion | artifactProfile | entryIndex | reserved
  | manifest | kind | capabilityRelease | config
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.entryIndex, .u16⟩,
  ⟨.reserved, .reserved 2⟩,
  ⟨.manifest, .bytes 32⟩,
  ⟨.kind, .bytes 32⟩,
  ⟨.capabilityRelease, .bytes 32⟩,
  ⟨.config, .bytes 32⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

namespace Field

def rustName : Field → String
  | .magic => "CAPABILITY_EXECUTION_SELECTION_MAGIC_OFFSET"
  | .schemaVersion => "CAPABILITY_EXECUTION_SELECTION_SCHEMA_VERSION_OFFSET"
  | .artifactProfile => "CAPABILITY_EXECUTION_SELECTION_PROFILE_OFFSET"
  | .entryIndex => "CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET"
  | .reserved => "CAPABILITY_EXECUTION_SELECTION_RESERVED_OFFSET"
  | .manifest => "CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET"
  | .kind => "CAPABILITY_EXECUTION_SELECTION_KIND_OFFSET"
  | .capabilityRelease => "CAPABILITY_EXECUTION_SELECTION_RELEASE_OFFSET"
  | .config => "CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET"

end Field

theorem width_is_exact : bytes = 144 := by native_decide
theorem names_are_unique : (schema.map fun field => field.name).Nodup := by native_decide
theorem fields_are_disjoint : layout.Pairwise Before := specializeFrom_pairwise 0 schema
theorem identity_fields_are_exactly_32_bytes :
    (schema.filter fun field =>
      field.name = .manifest || field.name = .kind ||
      field.name = .capabilityRelease || field.name = .config).all
      (fun field => field.kind.byteWidth = 32) := by native_decide

end DClutch.CapabilityExecutionAbi
