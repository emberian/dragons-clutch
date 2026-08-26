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
def exampleCapabilityProgramId : Nat := 0x23

def configMagic : List UInt8 :=
  [0x44, 0x43, 0x47, 0x43, 0x46, 0x47, 0x30, 0x32] -- `DCGCFG02`

inductive ConfigField where
  | magic | version | artifactProfile | outcomeCount | reserved
  | capacityProfileId | claimBasisId | capabilityProgramId
  | generation | priceScale | collectionSlots | selectionSlots | settlementSlots
  | maxOrdersPerCandidate | maxPagesPerCandidate | continuationRewardLamports
  | selectionPolicyId | quoteSurplusBeneficiary
  deriving DecidableEq, Repr

def configSchema : List (FieldSpec ConfigField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.artifactProfile, .u16⟩,
  ⟨.outcomeCount, .u16⟩, ⟨.reserved, .reserved 2⟩,
  ⟨.capacityProfileId, .bytes 32⟩, ⟨.claimBasisId, .bytes 32⟩,
  ⟨.capabilityProgramId, .bytes 32⟩, ⟨.generation, .u64⟩,
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

structure ConfigDataV2 where
  outcomeCount : Nat
  capacityProfileId : Nat
  claimBasisId : Nat
  capabilityProgramId : Nat
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

def ConfigDataV2.valid (value : ConfigDataV2) : Bool :=
  2 ≤ value.outcomeCount && value.outcomeCount ≤ maxOutcomes &&
  value.capacityProfileId != 0 && fitsUnsigned 32 value.capacityProfileId &&
  value.claimBasisId != 0 && fitsUnsigned 32 value.claimBasisId &&
  value.capabilityProgramId != 0 && fitsUnsigned 32 value.capabilityProgramId &&
  value.selectionPolicyId != 0 && fitsUnsigned 32 value.selectionPolicyId &&
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
  Codec.encodeLE 32 value.capabilityProgramId ++ Codec.encodeLE 8 value.generation ++
  Codec.encodeLE 8 value.priceScale ++ Codec.encodeLE 8 value.collectionSlots ++
  Codec.encodeLE 8 value.selectionSlots ++ Codec.encodeLE 8 value.settlementSlots ++
  Codec.encodeLE 4 value.maxOrdersPerCandidate ++
  Codec.encodeLE 4 value.maxPagesPerCandidate ++
  Codec.encodeLE 8 value.continuationRewardLamports ++
  Codec.encodeLE 32 value.selectionPolicyId ++
  Codec.encodeLE 32 value.quoteSurplusBeneficiary

theorem config_encoding_length (value : ConfigDataV2) :
    (encodeConfig value).length = configBytes := by
  simp [encodeConfig, configBytes, configSchema, schemaWidth, configMagic,
    Codec.encodeLE_length, FieldKind.byteWidth]

def exampleConfig : ConfigDataV2 := {
  outcomeCount := 2
  capacityProfileId := 0x11
  claimBasisId := 0x22
  capabilityProgramId := exampleCapabilityProgramId
  generation := 7
  priceScale := 100
  collectionSlots := 10
  selectionSlots := 11
  settlementSlots := 12
  maxOrdersPerCandidate := 64
  maxPagesPerCandidate := 2
  continuationRewardLamports := 5
  selectionPolicyId := 0x33
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

/-- A capability selects one immutable interpreted policy identity; a batch
cannot choose another objective while initializing its cursor. -/
def selectionPolicyAccepts (config : ConfigDataV2) (policyId : Nat) : Bool :=
  config.valid && policyId = config.selectionPolicyId

theorem substituted_selection_policy_refuses_general
    (config : ConfigDataV2) (policyId : Nat)
    (valid : config.valid = true)
    (substituted : policyId ≠ config.selectionPolicyId) :
    selectionPolicyAccepts config policyId = false := by
  simp [selectionPolicyAccepts, valid, substituted]

/-! ## Mutable General root-state tail

The canonical Trading account prepends its common immutable capability-root
header. This 128-byte value is the descriptor-selected mutable tail; it is not
a second account or a second activation projection.
-/

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

/-! ## Core-authenticated General activation request -/

def activationMagic : List UInt8 :=
  [0x44, 0x43, 0x47, 0x41, 0x43, 0x54, 0x30, 0x32] -- `DCGACT02`

def activationAction : Nat := 1

inductive ActivationField where
  | magic | version | action | reservedHeader | capabilityRoot | configId
  | manifestId | fundingState | rentCredit | entryIndex | reservedEntry
  | currentSlot | exactRootRentLamports | exactFundingRentLamports
  | rootStateBytes | configBytes | fundingBytes | selectedEntryBytes | reservedTail
  deriving DecidableEq, Repr

def activationSchema : List (FieldSpec ActivationField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩, ⟨.capabilityRoot, .bytes 32⟩,
  ⟨.configId, .bytes 32⟩, ⟨.manifestId, .bytes 32⟩,
  ⟨.fundingState, .bytes 32⟩, ⟨.rentCredit, .bytes 32⟩,
  ⟨.entryIndex, .u16⟩, ⟨.reservedEntry, .reserved 6⟩,
  ⟨.currentSlot, .u64⟩, ⟨.exactRootRentLamports, .u64⟩,
  ⟨.exactFundingRentLamports, .u64⟩, ⟨.rootStateBytes, .u32⟩,
  ⟨.configBytes, .u32⟩, ⟨.fundingBytes, .u32⟩,
  ⟨.selectedEntryBytes, .u32⟩,
  ⟨.reservedTail, .reserved 32⟩
]

def activationLayout := specialize activationSchema
def activationBytes := schemaWidth activationSchema

theorem exact_activation_width : activationBytes = 256 := by native_decide

theorem activation_schema_well_formed : WellFormed activationSchema := by
  simp [WellFormed, activationSchema, FieldKind.byteWidth]

theorem activation_layout_is_byte_disjoint : activationLayout.Pairwise Before :=
  specializeFrom_pairwise 0 activationSchema

structure ActivationRequestDataV2 where
  capabilityRoot : Nat
  configId : Nat
  manifestId : Nat
  fundingState : Nat
  rentCredit : Nat
  entryIndex : Nat
  currentSlot : Nat
  exactRootRentLamports : Nat
  exactFundingRentLamports : Nat
  deriving DecidableEq, Repr

def ActivationRequestDataV2.valid (value : ActivationRequestDataV2) : Bool :=
  value.capabilityRoot != 0 && fitsUnsigned 32 value.capabilityRoot &&
  value.configId != 0 && fitsUnsigned 32 value.configId &&
  value.manifestId != 0 && fitsUnsigned 32 value.manifestId &&
  value.fundingState != 0 && fitsUnsigned 32 value.fundingState &&
  value.rentCredit != 0 && fitsUnsigned 32 value.rentCredit &&
  fitsUnsigned 2 value.entryIndex && fitsUnsigned 8 value.currentSlot &&
  value.exactRootRentLamports != 0 && fitsUnsigned 8 value.exactRootRentLamports &&
  value.exactFundingRentLamports != 0 && fitsUnsigned 8 value.exactFundingRentLamports

def encodeActivationRequest (value : ActivationRequestDataV2) : List UInt8 :=
  activationMagic ++ Codec.encodeLE 2 abiVersion ++ [UInt8.ofNat activationAction] ++
  List.replicate 5 0 ++ Codec.encodeLE 32 value.capabilityRoot ++
  Codec.encodeLE 32 value.configId ++ Codec.encodeLE 32 value.manifestId ++
  Codec.encodeLE 32 value.fundingState ++ Codec.encodeLE 32 value.rentCredit ++
  Codec.encodeLE 2 value.entryIndex ++ List.replicate 6 0 ++
  Codec.encodeLE 8 value.currentSlot ++ Codec.encodeLE 8 value.exactRootRentLamports ++
  Codec.encodeLE 8 value.exactFundingRentLamports ++ Codec.encodeLE 4 rootBytes ++
  Codec.encodeLE 4 configBytes ++ Codec.encodeLE 4 320 ++ Codec.encodeLE 4 528 ++
  List.replicate 32 0

theorem activation_request_encoding_length (value : ActivationRequestDataV2) :
    (encodeActivationRequest value).length = activationBytes := by
  simp [encodeActivationRequest, activationBytes, activationSchema, schemaWidth,
    activationMagic, Codec.encodeLE_length, FieldKind.byteWidth]

def exampleActivationRequest : ActivationRequestDataV2 := {
  capabilityRoot := 0x55, configId := 0x66, manifestId := 0x77, fundingState := 0x88,
  rentCredit := 0x99, entryIndex := 3, currentSlot := 44,
  exactRootRentLamports := 100, exactFundingRentLamports := 200
}

theorem example_activation_request_is_valid :
    exampleActivationRequest.valid = true := by native_decide

/-- The Trading controller must leave exact Rent on the one composite
capability-root account after General atomically debits its shared FundingState,
classifies precreation displacement, and routes unsolicited dust to the
Core-authenticated RentCredit destination. -/
def normalizedRootRentAccepts (request : ActivationRequestDataV2)
    (observedLamports : Nat) : Bool :=
  request.valid && observedLamports = request.exactRootRentLamports

theorem underfunded_activation_refuses :
    normalizedRootRentAccepts exampleActivationRequest 99 = false := by native_decide

theorem overfunded_activation_refuses :
    normalizedRootRentAccepts exampleActivationRequest 101 = false := by native_decide

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
