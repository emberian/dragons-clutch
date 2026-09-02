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

/-!
## V2 — the succession profile

The V2 profile is V1's layout extended by the predecessor's two
artifact-release ids (the succession is content-walkable, like the release
lineage record's predecessor keying) and a reserved tail.  V1 is never
mutated: V2 lives at its own one-seed PDA domain and is written once by the
succession ceremony (`docs/design/PROFILE_UPGRADE_RULING_2026_08_31.md` §5).
-/

/-- Per-Core PDA seed domain for the immutable infrastructure profile.

One seed, so Solana's 32-byte seed bound is the constraint the theorem below
states.  The Rust carried this string by hand while every other coordinate of
the profile derived. -/
def profilePdaDomainTextV1 : String := "dclutch:infrastructure:v1"

def profilePdaDomainV1 : List UInt8 := profilePdaDomainTextV1.toUTF8.toList

/-- The V2 profile's own one-seed PDA domain.

V2 lives at its own domain precisely so V1 is never mutated. The string was
hand-written in `protocol_infrastructure.rs` while its V1 sibling derived,
which is the asymmetry this closes: both domains now have one author. -/
def profilePdaDomainTextV2 : String := "dclutch:infrastructure:v2"

def profilePdaDomainV2 : List UInt8 := profilePdaDomainTextV2.toUTF8.toList

theorem pda_domains_are_admissible_single_seeds :
    profilePdaDomainV1.length <= 32 ∧ profilePdaDomainV2.length <= 32 ∧
      profilePdaDomainV1 ≠ profilePdaDomainV2 := by
  native_decide

def profileMagicV2 : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x49, 0x4e, 0x46, 0x32]
def initializeMagicV2 : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x49, 0x49, 0x4e, 0x32]
def schemaVersionV2 : Nat := 2

inductive ProfileFieldV2 where
  | magic | schemaVersion | artifactProfile | reserved
  | registryProgram | registryArtifactRelease | rentProgram | rentArtifactRelease
  | predecessorRegistryArtifact | predecessorRentArtifact | reservedTail
  deriving DecidableEq, Repr

def profileSchemaV2 : List (FieldSpec ProfileFieldV2) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.reserved, .reserved 4⟩,
  ⟨.registryProgram, .bytes 32⟩,
  ⟨.registryArtifactRelease, .bytes 32⟩,
  ⟨.rentProgram, .bytes 32⟩,
  ⟨.rentArtifactRelease, .bytes 32⟩,
  ⟨.predecessorRegistryArtifact, .bytes 32⟩,
  ⟨.predecessorRentArtifact, .bytes 32⟩,
  ⟨.reservedTail, .reserved 16⟩
]

def profileLayoutV2 : List (PlacedField ProfileFieldV2) := specialize profileSchemaV2
def profileBytesV2 : Nat := schemaWidth profileSchemaV2

namespace ProfileFieldV2

def rustName : ProfileFieldV2 → String
  | .magic => "PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_OFFSET_V2"
  | .schemaVersion => "PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V2"
  | .artifactProfile => "PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V2"
  | .reserved => "PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_OFFSET_V2"
  | .registryProgram => "PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V2"
  | .registryArtifactRelease => "PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_ARTIFACT_OFFSET_V2"
  | .rentProgram => "PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_PROGRAM_OFFSET_V2"
  | .rentArtifactRelease => "PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_ARTIFACT_OFFSET_V2"
  | .predecessorRegistryArtifact =>
      "PROTOCOL_INFRASTRUCTURE_PROFILE_PREDECESSOR_REGISTRY_ARTIFACT_OFFSET_V2"
  | .predecessorRentArtifact =>
      "PROTOCOL_INFRASTRUCTURE_PROFILE_PREDECESSOR_RENT_ARTIFACT_OFFSET_V2"
  | .reservedTail => "PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_TAIL_OFFSET_V2"

end ProfileFieldV2

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

/-- The V2 ceremony instruction reuses the V1 header shape byte for byte;
only the magic and schema version distinguish it, so the same layout is
emitted twice under version-suffixed names rather than duplicated. -/
def rustNameV2 : InitializeField → String
  | .magic => "INITIALIZE_PROTOCOL_INFRASTRUCTURE_MAGIC_OFFSET_V2"
  | .schemaVersion => "INITIALIZE_PROTOCOL_INFRASTRUCTURE_SCHEMA_VERSION_OFFSET_V2"
  | .artifactProfile => "INITIALIZE_PROTOCOL_INFRASTRUCTURE_ARTIFACT_PROFILE_OFFSET_V2"
  | .reserved => "INITIALIZE_PROTOCOL_INFRASTRUCTURE_RESERVED_OFFSET_V2"

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

theorem profile_v2_width_is_exact : profileBytesV2 = 224 := by native_decide
theorem profile_v2_names_are_unique :
    (profileSchemaV2.map fun field => field.name).Nodup := by native_decide
theorem profile_v2_fields_are_disjoint :
    profileLayoutV2.Pairwise Before := specializeFrom_pairwise 0 profileSchemaV2

/-- V2 is exactly V1's layout for its first eight fields: every offset and
width V1 placed is placed identically in V2, so a reader of the shared
prefix cannot diverge between the two versions. -/
theorem profile_v2_preserves_v1_prefix :
    (profileLayoutV2.take profileLayout.length).map
        (fun field => (field.offset, field.spec.kind)) =
      profileLayout.map (fun field => (field.offset, field.spec.kind)) := by
  native_decide

/-- The predecessor artifact ids begin exactly where V1's record ended. -/
theorem profile_v2_extends_at_v1_width :
    (profileLayoutV2.filter fun field =>
        field.spec.name = ProfileFieldV2.predecessorRegistryArtifact).map
        (fun field => field.offset) = [profileBytes] := by
  native_decide

end DClutch.ProtocolInfrastructureProfileAbi
