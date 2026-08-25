import DClutchSemantics.AbiSchema
import DClutchSemantics.MarketCore

/-!
# Fixed Market Core physical ABI V2

The fixed 352-byte header contains only lifecycle and immutable Market identity
references. The Product-owned outcome count remains runtime data, while all
mutable claim vectors and Hoard principal belong exclusively to the Claims
aggregate deterministically derived by the selected Claims program. Source/Resolution exclusively owns
action funding, work balances, and closure. Core stores neither parallel child
ledger nor economic tail and therefore introduces no semantic N ceiling.
Offsets are specialized from field data rather than maintained by hand.
-/

namespace DClutch.MarketCoreAbi

open DClutch.AbiSchema

def stateMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x4f, 0x52, 0x32]
def requestMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x52, 0x51, 0x32]
def version : Nat := 2

inductive StateField where
  | magic | version | phase | readiness | terminalWinner
  | marketId | identityRealm | identityProduct | identityResultDomain
  | resolutionPolicy | capabilityManifest | selectedReleaseSet | registryProgram | generation
  | outstandingCapabilities
  | rentBeneficiary
  | terminalReceipt
  deriving DecidableEq, Repr

def stateSchema : List (FieldSpec StateField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.phase, .u8⟩, ⟨.readiness, .u8⟩,
  ⟨.terminalWinner, .u32⟩,
  ⟨.marketId, .bytes 32⟩, ⟨.identityRealm, .bytes 32⟩,
  ⟨.identityProduct, .bytes 32⟩, ⟨.identityResultDomain, .bytes 32⟩,
  ⟨.resolutionPolicy, .bytes 32⟩,
  ⟨.capabilityManifest, .bytes 32⟩,
  ⟨.selectedReleaseSet, .bytes 32⟩, ⟨.registryProgram, .bytes 32⟩,
  ⟨.generation, .u64⟩,
  ⟨.outstandingCapabilities, .u64⟩,
  ⟨.rentBeneficiary, .bytes 32⟩,
  ⟨.terminalReceipt, .bytes 32⟩
]

def stateLayout : List (PlacedField StateField) := specialize stateSchema
def stateBytes : Nat := schemaWidth stateSchema

namespace StateField

def rustName : StateField → String
  | .magic => "STATE_MAGIC_OFFSET" | .version => "STATE_VERSION_OFFSET"
  | .phase => "STATE_PHASE_OFFSET" | .readiness => "STATE_READINESS_OFFSET"
  | .terminalWinner => "STATE_TERMINAL_WINNER_OFFSET"
  | .marketId => "STATE_MARKET_ID_OFFSET" | .identityRealm => "STATE_IDENTITY_REALM_OFFSET"
  | .identityProduct => "STATE_IDENTITY_PRODUCT_OFFSET"
  | .identityResultDomain => "STATE_IDENTITY_RESULT_DOMAIN_OFFSET"
  | .resolutionPolicy => "STATE_RESOLUTION_POLICY_OFFSET"
  | .capabilityManifest => "STATE_CAPABILITY_MANIFEST_OFFSET"
  | .selectedReleaseSet => "STATE_SELECTED_RELEASE_SET_OFFSET"
  | .registryProgram => "STATE_REGISTRY_PROGRAM_OFFSET" | .generation => "STATE_GENERATION_OFFSET"
  | .outstandingCapabilities => "STATE_OUTSTANDING_CAPABILITIES_OFFSET"
  | .rentBeneficiary => "STATE_RENT_BENEFICIARY_OFFSET"
  | .terminalReceipt => "STATE_TERMINAL_RECEIPT_OFFSET"

end StateField

theorem state_schema_width : stateBytes = 352 := by native_decide
theorem state_schema_unique : (stateSchema.map fun field => field.name).Nodup := by native_decide
theorem state_fields_disjoint : stateLayout.Pairwise Before := specializeFrom_pairwise 0 stateSchema
theorem state_fields_bounded (placed : PlacedField StateField) (member : placed ∈ stateLayout) :
    placed.offset + placed.spec.kind.byteWidth ≤ stateBytes := by
  simpa [stateLayout, stateBytes, specialize] using
    specializeFrom_bounded 0 stateSchema placed member

inductive Action where
  | found | verifyReadiness | openMarket | admitTerminal
  | split | redeem | beginRetiring | retire | activateCapability | closeCapability
  deriving DecidableEq, Repr

def Action.tag : Action → Nat
  | .found => 0 | .verifyReadiness => 1 | .openMarket => 2 | .admitTerminal => 3
  | .split => 4 | .redeem => 5 | .beginRetiring => 6 | .retire => 7
  | .activateCapability => 8 | .closeCapability => 9

def phaseFoundingTag : Nat := 0
def phaseOpenTag : Nat := 1
def phaseTerminalTag : Nat := 2
def phaseRetiringTag : Nat := 3
def phaseRetiredTag : Nat := 4

def readinessPrepaidTag : Nat := 0
def readinessReadyTag : Nat := 1
def readinessConsumedTag : Nat := 2

def holderNoneTag : Nat := 0
def holderSourceTag : Nat := 1
def holderDestinationTag : Nat := 2

def representationNoneTag : Nat := 0
def representationNativeTag : Nat := 1
def representationMaterializedTag : Nat := 2

theorem action_tags_fit_u8 :
    [Action.found, .verifyReadiness, .openMarket, .admitTerminal, .split, .redeem,
      .beginRetiring, .retire, .activateCapability, .closeCapability].all
      (fun action => action.tag < 256) = true := by native_decide

theorem action_tags_unique :
    ([Action.found, .verifyReadiness, .openMarket, .admitTerminal, .split, .redeem,
      .beginRetiring, .retire, .activateCapability, .closeCapability].map Action.tag).Nodup := by
  native_decide

inductive RequestField where
  | magic | version | action | holder | representation | reservedA
  | outcome | reservedB | quantity | generation | market
  deriving DecidableEq, Repr

def requestSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩,
  ⟨.holder, .u8⟩, ⟨.representation, .u8⟩, ⟨.reservedA, .reserved 3⟩,
  ⟨.outcome, .u32⟩, ⟨.reservedB, .reserved 4⟩, ⟨.quantity, .u64⟩,
  ⟨.generation, .u64⟩, ⟨.market, .bytes 32⟩
]

def requestLayout : List (PlacedField RequestField) := specialize requestSchema
def requestBytes : Nat := schemaWidth requestSchema

namespace RequestField

def rustName : RequestField → String
  | .magic => "REQUEST_MAGIC_OFFSET" | .version => "REQUEST_VERSION_OFFSET"
  | .action => "REQUEST_ACTION_OFFSET" | .holder => "REQUEST_HOLDER_OFFSET"
  | .representation => "REQUEST_REPRESENTATION_OFFSET" | .reservedA => "REQUEST_RESERVED_A_OFFSET"
  | .outcome => "REQUEST_OUTCOME_OFFSET" | .reservedB => "REQUEST_RESERVED_B_OFFSET"
  | .quantity => "REQUEST_QUANTITY_OFFSET" | .generation => "REQUEST_GENERATION_OFFSET"
  | .market => "REQUEST_MARKET_OFFSET"

end RequestField

theorem request_schema_width : requestBytes = 72 := by native_decide
theorem request_schema_unique : (requestSchema.map fun field => field.name).Nodup := by native_decide
theorem request_fields_disjoint : requestLayout.Pairwise Before := specializeFrom_pairwise 0 requestSchema
theorem request_fields_bounded (placed : PlacedField RequestField) (member : placed ∈ requestLayout) :
    placed.offset + placed.spec.kind.byteWidth ≤ requestBytes := by
  simpa [requestLayout, requestBytes, specialize] using
    specializeFrom_bounded 0 requestSchema placed member

end DClutch.MarketCoreAbi
