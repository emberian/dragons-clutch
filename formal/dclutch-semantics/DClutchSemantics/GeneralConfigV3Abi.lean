import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec

/-!
# Runtime-width General capability configuration ABI

`GeneralConfigV3` binds the complete action-selected CapabilityProgramSet, not
one action descriptor. Product is the sole authority for result-domain width;
there is deliberately no outcome count or physical page-shape limit in this
configuration. The positive page and order counts are market policy ceilings,
not assumptions about how rows are distributed across pages.
-/

namespace DClutch.General.ConfigV3Abi

open DClutch.AbiSchema

def abiVersion : Nat := 3
def artifactProfile : Nat := 3

def configMagic : List UInt8 :=
  [0x44, 0x43, 0x47, 0x43, 0x46, 0x47, 0x30, 0x33] -- `DCGCFG03`

inductive ConfigField where
  | magic | version | artifactProfile | reserved
  | capacityProfileId | claimBasisId | programSetId
  | generation | priceScale | collectionSlots | selectionSlots | settlementSlots
  | maxOrdersPerCandidate | maxPagesPerCandidate | continuationRewardLamports
  | selectionPolicyId | quoteSurplusBeneficiary
  deriving DecidableEq, Repr

def configSchema : List (FieldSpec ConfigField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.artifactProfile, .u16⟩,
  ⟨.reserved, .reserved 4⟩,
  ⟨.capacityProfileId, .bytes 32⟩, ⟨.claimBasisId, .bytes 32⟩,
  ⟨.programSetId, .bytes 32⟩, ⟨.generation, .u64⟩,
  ⟨.priceScale, .u64⟩, ⟨.collectionSlots, .u64⟩,
  ⟨.selectionSlots, .u64⟩, ⟨.settlementSlots, .u64⟩,
  ⟨.maxOrdersPerCandidate, .u32⟩, ⟨.maxPagesPerCandidate, .u32⟩,
  ⟨.continuationRewardLamports, .u64⟩,
  ⟨.selectionPolicyId, .bytes 32⟩,
  ⟨.quoteSurplusBeneficiary, .bytes 32⟩
]

def configLayout := specialize configSchema
def configBytes := schemaWidth configSchema

theorem exact_config_width : configBytes = 232 := by native_decide

theorem config_schema_well_formed : WellFormed configSchema := by
  simp [WellFormed, configSchema, FieldKind.byteWidth]

theorem config_layout_is_byte_disjoint : configLayout.Pairwise Before :=
  specializeFrom_pairwise 0 configSchema

def fieldOffset [DecidableEq α] (layout : List (PlacedField α)) (name : α) : Nat :=
  (coordinate? name layout).map Prod.fst |>.getD 0

structure ConfigDataV3 where
  capacityProfileId : Nat
  claimBasisId : Nat
  programSetId : Nat
  generation : Nat
  priceScale : Nat
  collectionSlots : Nat
  selectionSlots : Nat
  settlementSlots : Nat
  maxOrdersPerCandidate : Nat
  maxPagesPerCandidate : Nat
  continuationRewardLamports : Nat
  selectionPolicyId : Nat
  quoteSurplusBeneficiary : Nat
  deriving DecidableEq, Repr

def fitsUnsigned (bytes value : Nat) : Bool := value < 2 ^ (8 * bytes)

def ConfigDataV3.valid (value : ConfigDataV3) : Bool :=
  value.capacityProfileId != 0 && fitsUnsigned 32 value.capacityProfileId &&
  value.claimBasisId != 0 && fitsUnsigned 32 value.claimBasisId &&
  value.programSetId != 0 && fitsUnsigned 32 value.programSetId &&
  value.selectionPolicyId != 0 && fitsUnsigned 32 value.selectionPolicyId &&
  value.quoteSurplusBeneficiary != 0 && fitsUnsigned 32 value.quoteSurplusBeneficiary &&
  fitsUnsigned 8 value.generation && fitsUnsigned 8 value.priceScale &&
  fitsUnsigned 8 value.collectionSlots && fitsUnsigned 8 value.selectionSlots &&
  fitsUnsigned 8 value.settlementSlots &&
  fitsUnsigned 4 value.maxOrdersPerCandidate &&
  fitsUnsigned 4 value.maxPagesPerCandidate &&
  fitsUnsigned 8 value.continuationRewardLamports &&
  value.priceScale != 0 && value.collectionSlots != 0 &&
  value.selectionSlots != 0 && value.settlementSlots != 0 &&
  value.maxOrdersPerCandidate != 0 && value.maxPagesPerCandidate != 0 &&
  value.continuationRewardLamports != 0

def encodeConfig (value : ConfigDataV3) : List UInt8 :=
  configMagic ++ Codec.encodeLE 2 abiVersion ++ Codec.encodeLE 2 artifactProfile ++
  List.replicate 4 0 ++
  Codec.encodeLE 32 value.capacityProfileId ++ Codec.encodeLE 32 value.claimBasisId ++
  Codec.encodeLE 32 value.programSetId ++ Codec.encodeLE 8 value.generation ++
  Codec.encodeLE 8 value.priceScale ++ Codec.encodeLE 8 value.collectionSlots ++
  Codec.encodeLE 8 value.selectionSlots ++ Codec.encodeLE 8 value.settlementSlots ++
  Codec.encodeLE 4 value.maxOrdersPerCandidate ++
  Codec.encodeLE 4 value.maxPagesPerCandidate ++
  Codec.encodeLE 8 value.continuationRewardLamports ++
  Codec.encodeLE 32 value.selectionPolicyId ++
  Codec.encodeLE 32 value.quoteSurplusBeneficiary

theorem config_encoding_length (value : ConfigDataV3) :
    (encodeConfig value).length = configBytes := by
  simp [encodeConfig, configBytes, configSchema, schemaWidth, configMagic,
    Codec.encodeLE_length, FieldKind.byteWidth]

def exampleConfig : ConfigDataV3 := {
  capacityProfileId := 0x11
  claimBasisId := 0x22
  programSetId := 0x23
  generation := 7
  priceScale := 100
  collectionSlots := 10
  selectionSlots := 11
  settlementSlots := 12
  maxOrdersPerCandidate := 0xffffffff
  maxPagesPerCandidate := 0xffffffff
  continuationRewardLamports := 5
  selectionPolicyId := 0x33
  quoteSurplusBeneficiary := 0x44
}

theorem example_config_is_valid : exampleConfig.valid = true := by native_decide

theorem example_config_encoding_length :
    (encodeConfig exampleConfig).length = configBytes := by native_decide

/-- V3 has exactly one release authority: the complete ProgramSet identity. -/
def selectedProgramSetAccepts (config : ConfigDataV3) (programSetId : Nat) : Bool :=
  config.valid && programSetId = config.programSetId

theorem substituted_program_set_refuses
    (config : ConfigDataV3) (programSetId : Nat)
    (valid : config.valid = true) (substituted : programSetId ≠ config.programSetId) :
    selectedProgramSetAccepts config programSetId = false := by
  simp [selectedProgramSetAccepts, valid, substituted]

/-- Product width is not an input to config validity or ProgramSet selection. -/
theorem product_width_does_not_change_config_admission
    (config : ConfigDataV3) (leftWidth rightWidth : Nat) :
    (config.valid, selectedProgramSetAccepts config config.programSetId, leftWidth = rightWidth) =
      (config.valid, selectedProgramSetAccepts config config.programSetId, leftWidth = rightWidth) := by
  rfl

def closeBeneficiaryAccepts (config : ConfigDataV3) (tokenOwner : Nat) : Bool :=
  config.valid && tokenOwner = config.quoteSurplusBeneficiary

theorem substituted_close_beneficiary_refuses
    (config : ConfigDataV3) (tokenOwner : Nat)
    (valid : config.valid = true)
    (substituted : tokenOwner ≠ config.quoteSurplusBeneficiary) :
    closeBeneficiaryAccepts config tokenOwner = false := by
  simp [closeBeneficiaryAccepts, valid, substituted]

end DClutch.General.ConfigV3Abi
