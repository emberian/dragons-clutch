import DClutchSemantics.AbiSchema
import DClutchSemantics.ClaimsRepresentation

/-!
# Physical ABI for data-specialized claim representations

This module owns one variable-width descriptor header, one fixed action, one
fixed capability state, and the action-rule table consumed by the safe Rust
interpreter.  The only variable tail is `outcomeCount` consecutive little-
endian `u64` claim weights.  No semantic or physical N-specific program family
is generated.

The physical ABI refines abstract Lean identities to nonzero 32-byte content or
account identities and abstract scalars to checked `u64`.  Registry receipt
authentication, Economic-state execution, Token-2022, CPI, persistence, and
transaction rollback remain named adapter boundaries.
-/

namespace DClutch.ClaimsRepresentationAbi

open DClutch
open DClutch.AbiSchema

def descriptorMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x57, 0x52, 0x50, 0x44, 0x31] -- `DCLWRPD1`
def actionMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x57, 0x52, 0x50, 0x41, 0x31] -- `DCLWRPA1`
def stateMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x57, 0x52, 0x50, 0x53, 0x31] -- `DCLWRPS1`

def version : Nat := 1
def claimAtomBytes : Nat := 8

def zeros (count : Nat) : List UInt8 := List.replicate count 0

inductive DescriptorField where
  | magic | version | reservedHeader | descriptorId | marketId | productId
  | resultDomainId | adapterAssetId | releaseSetId | outcomeCount
  | reservedBody | receiptUnitsPerLot
  deriving DecidableEq, Repr

def descriptorHeaderSchema : List (FieldSpec DescriptorField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.reservedHeader, .reserved 6⟩,
  ⟨.descriptorId, .bytes 32⟩,
  ⟨.marketId, .bytes 32⟩,
  ⟨.productId, .bytes 32⟩,
  ⟨.resultDomainId, .bytes 32⟩,
  ⟨.adapterAssetId, .bytes 32⟩,
  ⟨.releaseSetId, .bytes 32⟩,
  ⟨.outcomeCount, .u32⟩,
  ⟨.reservedBody, .reserved 4⟩,
  ⟨.receiptUnitsPerLot, .u64⟩
]

def descriptorHeaderLayout : List (PlacedField DescriptorField) :=
  specialize descriptorHeaderSchema

def descriptorHeaderBytes : Nat := schemaWidth descriptorHeaderSchema

def descriptorBytes (outcomeCount : Nat) : Nat :=
  descriptorHeaderBytes + outcomeCount * claimAtomBytes

namespace DescriptorField

def all : List DescriptorField := [
  .magic, .version, .reservedHeader, .descriptorId, .marketId, .productId,
  .resultDomainId, .adapterAssetId, .releaseSetId, .outcomeCount,
  .reservedBody, .receiptUnitsPerLot
]

def rustName : DescriptorField → String
  | .magic => "DESCRIPTOR_MAGIC_OFFSET"
  | .version => "DESCRIPTOR_VERSION_OFFSET"
  | .reservedHeader => "DESCRIPTOR_RESERVED_HEADER_OFFSET"
  | .descriptorId => "DESCRIPTOR_ID_OFFSET"
  | .marketId => "DESCRIPTOR_MARKET_ID_OFFSET"
  | .productId => "DESCRIPTOR_PRODUCT_ID_OFFSET"
  | .resultDomainId => "DESCRIPTOR_RESULT_DOMAIN_ID_OFFSET"
  | .adapterAssetId => "DESCRIPTOR_ADAPTER_ASSET_ID_OFFSET"
  | .releaseSetId => "DESCRIPTOR_RELEASE_SET_ID_OFFSET"
  | .outcomeCount => "DESCRIPTOR_OUTCOME_COUNT_OFFSET"
  | .reservedBody => "DESCRIPTOR_RESERVED_BODY_OFFSET"
  | .receiptUnitsPerLot => "DESCRIPTOR_RECEIPT_UNITS_PER_LOT_OFFSET"

def offset (field : DescriptorField) : Nat :=
  ((coordinate? field descriptorHeaderLayout).getD (0, 0)).1

end DescriptorField

inductive ActionField where
  | magic | version | action | reserved | descriptorId | expectedReleaseSetId
  | claimant | expectedNextNonce | expectedIssuedLots | lots
  deriving DecidableEq, Repr

def actionSchema : List (FieldSpec ActionField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.action, .u8⟩,
  ⟨.reserved, .reserved 5⟩,
  ⟨.descriptorId, .bytes 32⟩,
  ⟨.expectedReleaseSetId, .bytes 32⟩,
  ⟨.claimant, .bytes 32⟩,
  ⟨.expectedNextNonce, .u64⟩,
  ⟨.expectedIssuedLots, .u64⟩,
  ⟨.lots, .u64⟩
]

def actionLayout : List (PlacedField ActionField) := specialize actionSchema
def actionBytes : Nat := schemaWidth actionSchema

namespace ActionField

def all : List ActionField := [
  .magic, .version, .action, .reserved, .descriptorId,
  .expectedReleaseSetId, .claimant, .expectedNextNonce,
  .expectedIssuedLots, .lots
]

def rustName : ActionField → String
  | .magic => "ACTION_MAGIC_OFFSET"
  | .version => "ACTION_VERSION_OFFSET"
  | .action => "ACTION_TAG_OFFSET"
  | .reserved => "ACTION_RESERVED_OFFSET"
  | .descriptorId => "ACTION_DESCRIPTOR_ID_OFFSET"
  | .expectedReleaseSetId => "ACTION_EXPECTED_RELEASE_SET_ID_OFFSET"
  | .claimant => "ACTION_CLAIMANT_OFFSET"
  | .expectedNextNonce => "ACTION_EXPECTED_NEXT_NONCE_OFFSET"
  | .expectedIssuedLots => "ACTION_EXPECTED_ISSUED_LOTS_OFFSET"
  | .lots => "ACTION_LOTS_OFFSET"

def offset (field : ActionField) : Nat :=
  ((coordinate? field actionLayout).getD (0, 0)).1

end ActionField

inductive StateField where
  | magic | version | retired | reserved | descriptorId | nextNonce | issuedLots
  deriving DecidableEq, Repr

def stateSchema : List (FieldSpec StateField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.retired, .u8⟩,
  ⟨.reserved, .reserved 5⟩,
  ⟨.descriptorId, .bytes 32⟩,
  ⟨.nextNonce, .u64⟩,
  ⟨.issuedLots, .u64⟩
]

def stateLayout : List (PlacedField StateField) := specialize stateSchema
def stateBytes : Nat := schemaWidth stateSchema

namespace StateField

def all : List StateField := [
  .magic, .version, .retired, .reserved, .descriptorId, .nextNonce, .issuedLots
]

def rustName : StateField → String
  | .magic => "STATE_MAGIC_OFFSET"
  | .version => "STATE_VERSION_OFFSET"
  | .retired => "STATE_RETIRED_OFFSET"
  | .reserved => "STATE_RESERVED_OFFSET"
  | .descriptorId => "STATE_DESCRIPTOR_ID_OFFSET"
  | .nextNonce => "STATE_NEXT_NONCE_OFFSET"
  | .issuedLots => "STATE_ISSUED_LOTS_OFFSET"

def offset (field : StateField) : Nat :=
  ((coordinate? field stateLayout).getD (0, 0)).1

end StateField

inductive Action where
  | issue | redeem | redeemTerminal | retire
  deriving DecidableEq, Repr

def Action.tag : Action → Nat
  | .issue => 1
  | .redeem => 2
  | .redeemTerminal => 3
  | .retire => 4

inductive LotEffect where
  | add | subtract | retire
  deriving DecidableEq, Repr

inductive EconomicStyle where
  | materialize | dematerialize | terminal | none
  deriving DecidableEq, Repr

inductive AdapterStyle where
  | mint | burn | retire
  deriving DecidableEq, Repr

def phaseOpen : Nat := 1
def phaseTerminal : Nat := 2
def phaseRetiring : Nat := 4
def phaseRetired : Nat := 8

structure Rule where
  action : Action
  allowedPhases : Nat
  lotEffect : LotEffect
  requiresPositiveLots : Bool
  economicStyle : EconomicStyle
  adapterStyle : AdapterStyle
  deriving DecidableEq, Repr

def rules : List Rule := [
  ⟨.issue, phaseOpen, .add, true, .materialize, .mint⟩,
  ⟨.redeem, phaseOpen ||| phaseTerminal ||| phaseRetiring,
    .subtract, true, .dematerialize, .burn⟩,
  ⟨.redeemTerminal, phaseTerminal ||| phaseRetiring,
    .subtract, true, .terminal, .burn⟩,
  ⟨.retire, phaseTerminal ||| phaseRetiring ||| phaseRetired,
    .retire, false, .none, .retire⟩
]

abbrev Id32 := List UInt8

def encodeId32 (identity : Id32) : List UInt8 := identity.take 32

structure DescriptorV1 where
  descriptorId : Id32
  marketId : Id32
  productId : Id32
  resultDomainId : Id32
  adapterAssetId : Id32
  releaseSetId : Id32
  receiptUnitsPerLot : Nat
  claimAtomsPerLot : List Nat

def encodeDescriptorV1 (descriptor : DescriptorV1) : List UInt8 :=
  descriptorMagic ++ Codec.encodeLE 2 version ++ zeros 6 ++
  encodeId32 descriptor.descriptorId ++ encodeId32 descriptor.marketId ++
  encodeId32 descriptor.productId ++ encodeId32 descriptor.resultDomainId ++
  encodeId32 descriptor.adapterAssetId ++ encodeId32 descriptor.releaseSetId ++
  Codec.encodeLE 4 descriptor.claimAtomsPerLot.length ++ zeros 4 ++
  Codec.encodeLE 8 descriptor.receiptUnitsPerLot ++
  descriptor.claimAtomsPerLot.flatMap (Codec.encodeLE 8)

structure ActionV1 where
  action : Action
  descriptorId : Id32
  expectedReleaseSetId : Id32
  claimant : Id32
  expectedNextNonce : Nat
  expectedIssuedLots : Nat
  lots : Nat

def encodeActionV1 (action : ActionV1) : List UInt8 :=
  actionMagic ++ Codec.encodeLE 2 version ++ [UInt8.ofNat action.action.tag] ++
  zeros 5 ++ encodeId32 action.descriptorId ++
  encodeId32 action.expectedReleaseSetId ++ encodeId32 action.claimant ++
  Codec.encodeLE 8 action.expectedNextNonce ++
  Codec.encodeLE 8 action.expectedIssuedLots ++ Codec.encodeLE 8 action.lots

structure StateV1 where
  retired : Bool
  descriptorId : Id32
  nextNonce : Nat
  issuedLots : Nat

def encodeStateV1 (state : StateV1) : List UInt8 :=
  stateMagic ++ Codec.encodeLE 2 version ++ [if state.retired then 1 else 0] ++
  zeros 5 ++ encodeId32 state.descriptorId ++
  Codec.encodeLE 8 state.nextNonce ++ Codec.encodeLE 8 state.issuedLots

theorem descriptor_header_width : descriptorHeaderBytes = 224 := by native_decide
theorem action_width : actionBytes = 136 := by native_decide
theorem state_width : stateBytes = 64 := by native_decide

theorem descriptor_coordinates : coordinates descriptorHeaderLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.reservedHeader, 10, 6),
    (.descriptorId, 16, 32), (.marketId, 48, 32), (.productId, 80, 32),
    (.resultDomainId, 112, 32), (.adapterAssetId, 144, 32),
    (.releaseSetId, 176, 32), (.outcomeCount, 208, 4),
    (.reservedBody, 212, 4), (.receiptUnitsPerLot, 216, 8)
  ] := by native_decide

theorem action_coordinates : coordinates actionLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.action, 10, 1),
    (.reserved, 11, 5), (.descriptorId, 16, 32),
    (.expectedReleaseSetId, 48, 32), (.claimant, 80, 32),
    (.expectedNextNonce, 112, 8), (.expectedIssuedLots, 120, 8),
    (.lots, 128, 8)
  ] := by native_decide

theorem state_coordinates : coordinates stateLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.retired, 10, 1),
    (.reserved, 11, 5), (.descriptorId, 16, 32),
    (.nextNonce, 48, 8), (.issuedLots, 56, 8)
  ] := by native_decide

theorem action_tags_unique : (rules.map (fun rule => rule.action.tag)).Nodup := by
  native_decide

theorem every_rule_has_nonzero_phase_mask :
    rules.all (fun rule => rule.allowedPhases != 0) = true := by
  native_decide

namespace Examples

def id (byte : UInt8) : Id32 := List.replicate 32 byte

def fractionalDescriptor : DescriptorV1 := {
  descriptorId := id 1
  marketId := id 2
  productId := id 3
  resultDomainId := id 4
  adapterAssetId := id 5
  releaseSetId := id 6
  receiptUnitsPerLot := 10
  claimAtomsPerLot := [1, 2, 1]
}

def issue : ActionV1 := {
  action := .issue
  descriptorId := id 1
  expectedReleaseSetId := id 6
  claimant := id 7
  expectedNextNonce := 0
  expectedIssuedLots := 0
  lots := 3
}

def emptyState : StateV1 := {
  retired := false
  descriptorId := id 1
  nextNonce := 0
  issuedLots := 0
}

theorem descriptor_example_width :
    (encodeDescriptorV1 fractionalDescriptor).length = descriptorBytes 3 := by
  native_decide

theorem action_example_width : (encodeActionV1 issue).length = actionBytes := by
  native_decide

theorem state_example_width : (encodeStateV1 emptyState).length = stateBytes := by
  native_decide

end Examples

end DClutch.ClaimsRepresentationAbi
