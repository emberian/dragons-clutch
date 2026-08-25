import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec

/-!
# Immutable General capability configuration ABI

This is the sole fixed-layout source for `GeneralConfigV2`. The collateral
surplus beneficiary is an immutable token-owner authority selected by the
capability configuration. It is deliberately not a candidate field and is
unrelated to any lamport RentCredit beneficiary.
-/

namespace DClutch.General.ConfigAbi

open DClutch.AbiSchema

def abiVersion : Nat := 2
def artifactProfile : Nat := 2
def maxOutcomes : Nat := 16
def maxExecutionsPerPage : Nat := 32
def physicalMaxPagesPerCandidate : Nat := 64
def reviewedCapabilityReleaseId : Nat :=
  0x8bfb89c68ca14dedc1132c6639dee97d898a4271867c7476294f4287ac07e013

def configMagic : List UInt8 :=
  [0x44, 0x43, 0x47, 0x43, 0x46, 0x47, 0x30, 0x32] -- `DCGCFG02`

inductive ConfigField where
  | magic | version | artifactProfile | outcomeCount | reserved
  | capacityProfileId | claimBasisId | capabilityReleaseId
  | generation | priceScale | collectionSlots | selectionSlots | settlementSlots
  | maxOrdersPerCandidate | maxPagesPerCandidate | continuationRewardLamports
  | quoteSurplusBeneficiary
  deriving DecidableEq, Repr

def configSchema : List (FieldSpec ConfigField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.artifactProfile, .u16⟩,
  ⟨.outcomeCount, .u16⟩, ⟨.reserved, .reserved 2⟩,
  ⟨.capacityProfileId, .bytes 32⟩, ⟨.claimBasisId, .bytes 32⟩,
  ⟨.capabilityReleaseId, .bytes 32⟩, ⟨.generation, .u64⟩,
  ⟨.priceScale, .u64⟩, ⟨.collectionSlots, .u64⟩,
  ⟨.selectionSlots, .u64⟩, ⟨.settlementSlots, .u64⟩,
  ⟨.maxOrdersPerCandidate, .u32⟩, ⟨.maxPagesPerCandidate, .u32⟩,
  ⟨.continuationRewardLamports, .u64⟩,
  ⟨.quoteSurplusBeneficiary, .bytes 32⟩
]

def configLayout := specialize configSchema
def configBytes := schemaWidth configSchema

theorem exact_config_width : configBytes = 200 := by native_decide

theorem config_schema_well_formed : WellFormed configSchema := by
  simp [WellFormed, configSchema, FieldKind.byteWidth]

theorem config_layout_is_byte_disjoint : configLayout.Pairwise Before :=
  specializeFrom_pairwise 0 configSchema

def fieldOffset [DecidableEq α] (layout : List (PlacedField α)) (name : α) : Nat :=
  (coordinate? name layout).map Prod.fst |>.getD 0

structure ConfigDataV2 where
  outcomeCount : Nat
  capacityProfileId : Nat
  claimBasisId : Nat
  capabilityReleaseId : Nat
  generation : Nat
  priceScale : Nat
  collectionSlots : Nat
  selectionSlots : Nat
  settlementSlots : Nat
  maxOrdersPerCandidate : Nat
  maxPagesPerCandidate : Nat
  continuationRewardLamports : Nat
  quoteSurplusBeneficiary : Nat
  deriving DecidableEq, Repr

def fitsUnsigned (bytes value : Nat) : Bool := value < 2 ^ (8 * bytes)

def ConfigDataV2.valid (value : ConfigDataV2) : Bool :=
  2 ≤ value.outcomeCount && value.outcomeCount ≤ maxOutcomes &&
  value.capacityProfileId != 0 && fitsUnsigned 32 value.capacityProfileId &&
  value.claimBasisId != 0 && fitsUnsigned 32 value.claimBasisId &&
  value.capabilityReleaseId = reviewedCapabilityReleaseId &&
  value.quoteSurplusBeneficiary != 0 && fitsUnsigned 32 value.quoteSurplusBeneficiary &&
  fitsUnsigned 8 value.generation && fitsUnsigned 8 value.priceScale &&
  fitsUnsigned 8 value.collectionSlots && fitsUnsigned 8 value.selectionSlots &&
  fitsUnsigned 8 value.settlementSlots &&
  fitsUnsigned 4 value.maxOrdersPerCandidate && fitsUnsigned 4 value.maxPagesPerCandidate &&
  fitsUnsigned 8 value.continuationRewardLamports &&
  value.priceScale != 0 && value.collectionSlots != 0 &&
  value.selectionSlots != 0 && value.settlementSlots != 0 &&
  value.maxOrdersPerCandidate != 0 && value.maxPagesPerCandidate != 0 &&
  value.continuationRewardLamports != 0 &&
  value.maxPagesPerCandidate ≤ physicalMaxPagesPerCandidate &&
  value.maxOrdersPerCandidate ≤ value.maxPagesPerCandidate * maxExecutionsPerPage

def encodeConfig (value : ConfigDataV2) : List UInt8 :=
  configMagic ++ Codec.encodeLE 2 abiVersion ++ Codec.encodeLE 2 artifactProfile ++
  Codec.encodeLE 2 value.outcomeCount ++ List.replicate 2 0 ++
  Codec.encodeLE 32 value.capacityProfileId ++ Codec.encodeLE 32 value.claimBasisId ++
  Codec.encodeLE 32 value.capabilityReleaseId ++ Codec.encodeLE 8 value.generation ++
  Codec.encodeLE 8 value.priceScale ++ Codec.encodeLE 8 value.collectionSlots ++
  Codec.encodeLE 8 value.selectionSlots ++ Codec.encodeLE 8 value.settlementSlots ++
  Codec.encodeLE 4 value.maxOrdersPerCandidate ++
  Codec.encodeLE 4 value.maxPagesPerCandidate ++
  Codec.encodeLE 8 value.continuationRewardLamports ++
  Codec.encodeLE 32 value.quoteSurplusBeneficiary

theorem config_encoding_length (value : ConfigDataV2) :
    (encodeConfig value).length = configBytes := by
  simp [encodeConfig, configBytes, configSchema, schemaWidth, configMagic,
    Codec.encodeLE_length, FieldKind.byteWidth]

def exampleConfig : ConfigDataV2 := {
  outcomeCount := 2
  capacityProfileId := 0x11
  claimBasisId := 0x22
  capabilityReleaseId := reviewedCapabilityReleaseId
  generation := 7
  priceScale := 100
  collectionSlots := 10
  selectionSlots := 11
  settlementSlots := 12
  maxOrdersPerCandidate := 64
  maxPagesPerCandidate := 2
  continuationRewardLamports := 5
  quoteSurplusBeneficiary := 0x44
}

theorem example_config_is_valid : exampleConfig.valid = true := by native_decide

theorem example_config_encoding_length :
    (encodeConfig exampleConfig).length = configBytes := by native_decide

/-- Close routes collateral only to the immutable capability authority. The
operational token account remains replaceable and is authenticated by its
parsed token owner at the physical boundary. -/
def closeBeneficiaryAccepts (config : ConfigDataV2) (tokenOwner : Nat) : Bool :=
  config.valid && tokenOwner = config.quoteSurplusBeneficiary

theorem substituted_close_beneficiary_refuses :
    closeBeneficiaryAccepts exampleConfig 0x45 = false := by native_decide

theorem substituted_close_beneficiary_refuses_general
    (config : ConfigDataV2) (tokenOwner : Nat)
    (valid : config.valid = true)
    (substituted : tokenOwner ≠ config.quoteSurplusBeneficiary) :
    closeBeneficiaryAccepts config tokenOwner = false := by
  simp [closeBeneficiaryAccepts, valid, substituted]

/-! ## Minimal activated General root -/

def rootMagic : List UInt8 :=
  [0x44, 0x43, 0x47, 0x52, 0x4f, 0x54, 0x30, 0x32] -- `DCGROT02`

inductive RootField where
  | magic | version | lifecycle | reservedHeader | market | configId
  | generation | revision | nextBatchSequence | openBatches | reservedTail
  deriving DecidableEq, Repr

def rootSchema : List (FieldSpec RootField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.lifecycle, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩, ⟨.market, .bytes 32⟩,
  ⟨.configId, .bytes 32⟩, ⟨.generation, .u64⟩, ⟨.revision, .u64⟩,
  ⟨.nextBatchSequence, .u64⟩, ⟨.openBatches, .u64⟩,
  ⟨.reservedTail, .reserved 16⟩
]

def rootLayout := specialize rootSchema
def rootBytes := schemaWidth rootSchema

theorem exact_root_width : rootBytes = 128 := by native_decide

theorem root_schema_well_formed : WellFormed rootSchema := by
  simp [WellFormed, rootSchema, FieldKind.byteWidth]

theorem root_layout_is_byte_disjoint : rootLayout.Pairwise Before :=
  specializeFrom_pairwise 0 rootSchema

structure RootDataV2 where
  lifecycle : Nat
  market : Nat
  configId : Nat
  generation : Nat
  revision : Nat
  nextBatchSequence : Nat
  openBatches : Nat
  deriving DecidableEq, Repr

def RootDataV2.valid (value : RootDataV2) : Bool :=
  1 ≤ value.lifecycle && value.lifecycle ≤ 3 &&
  value.market != 0 && fitsUnsigned 32 value.market &&
  value.configId != 0 && fitsUnsigned 32 value.configId &&
  fitsUnsigned 8 value.generation && fitsUnsigned 8 value.revision &&
  fitsUnsigned 8 value.nextBatchSequence && fitsUnsigned 8 value.openBatches &&
  value.revision != 0 && (value.lifecycle != 3 || value.openBatches = 0)

def encodeRoot (value : RootDataV2) : List UInt8 :=
  rootMagic ++ Codec.encodeLE 2 abiVersion ++ [UInt8.ofNat value.lifecycle] ++
  List.replicate 5 0 ++ Codec.encodeLE 32 value.market ++
  Codec.encodeLE 32 value.configId ++ Codec.encodeLE 8 value.generation ++
  Codec.encodeLE 8 value.revision ++ Codec.encodeLE 8 value.nextBatchSequence ++
  Codec.encodeLE 8 value.openBatches ++ List.replicate 16 0

theorem root_encoding_length (value : RootDataV2) :
    (encodeRoot value).length = rootBytes := by
  simp [encodeRoot, rootBytes, rootSchema, schemaWidth, rootMagic,
    Codec.encodeLE_length, FieldKind.byteWidth]

def exampleRoot : RootDataV2 := {
  lifecycle := 1, market := 0x11, configId := 0x22, generation := 7,
  revision := 1, nextBatchSequence := 0, openBatches := 0
}

theorem example_root_is_valid : exampleRoot.valid = true := by native_decide

def activateRoot (existing : Option RootDataV2) (expected : RootDataV2) :
    Option RootDataV2 :=
  if !expected.valid then none else
  match existing with
  | none => some expected
  | some present => if present = expected then some present else none

theorem exact_activation_replay_is_idempotent
    (root : RootDataV2) (valid : root.valid = true) :
    activateRoot (some root) root = some root := by
  simp [activateRoot, valid]

theorem substituted_activation_replay_refuses
    (present expected : RootDataV2) (valid : expected.valid = true)
    (substituted : present ≠ expected) :
    activateRoot (some present) expected = none := by
  simp [activateRoot, valid, substituted]

structure DustSafeCreation where
  topUpFromPrepaid : Nat
  displacedPrepaidToRentRefund : Nat
  dustSurplusToRentRefund : Nat
  deriving DecidableEq, Repr

def dustSafeCreation (exactRent precreation : Nat) : DustSafeCreation := {
  topUpFromPrepaid := exactRent - precreation
  displacedPrepaidToRentRefund := min precreation exactRent
  dustSurplusToRentRefund := precreation - exactRent
}

theorem prepaid_root_rent_is_exactly_conserved (exactRent precreation : Nat) :
    (dustSafeCreation exactRent precreation).topUpFromPrepaid +
      (dustSafeCreation exactRent precreation).displacedPrepaidToRentRefund = exactRent := by
  simp [dustSafeCreation]
  omega

theorem precreation_dust_is_exactly_partitioned (exactRent precreation : Nat) :
    (dustSafeCreation exactRent precreation).displacedPrepaidToRentRefund +
      (dustSafeCreation exactRent precreation).dustSurplusToRentRefund = precreation := by
  simp [dustSafeCreation]
  omega

end DClutch.General.ConfigAbi
