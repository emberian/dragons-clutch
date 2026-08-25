import DClutchSemantics.AbiSchema
import DClutchSemantics.MarketCore

/-!
# Fixed Market Core physical ABI

The fixed 1,416-byte header contains only scalar and immutable Core facts.  The
Product-owned outcome count remains runtime data; economic claim vectors are
separate exact-length slices and therefore introduce no semantic N ceiling.
Offsets are specialized from field data rather than maintained by hand.
-/

namespace DClutch.MarketCoreAbi

open DClutch.AbiSchema

def stateMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x4f, 0x52, 0x31]
def requestMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x52, 0x51, 0x31]
def version : Nat := 1

inductive StateField where
  | magic | version | phase | readiness | terminalWinner
  | realmId | collateralMint | tokenProgram | collateralRelease
  | productId | resultDomain | claimBasis | capacityProfile | compilerRelease
  | outcomeCount | productReserved | scalarLimit
  | marketId | identityRealm | identityProduct | identityResultDomain
  | resolutionPolicy | selectedReleaseSet | generation
  | releaseSetId
  | coreProgram | coreArtifact | coreSemantic
  | claimsProgram | claimsArtifact | claimsSemantic
  | tradingProgram | tradingArtifact | tradingSemantic
  | resolutionProgram | resolutionArtifact | resolutionSemantic
  | custodyProgram | custodyArtifact | custodySemantic
  | derivationAuthenticated | coordinateReserved
  | coordinateMarket | coordinateHoard | coordinateFund | coordinateReadiness
  | coordinateCustody | coordinateRentCredit
  | marketRent | marketDonation | hoardRent | hoardDonation
  | fundRent | fundDonation | readinessRent | readinessDonation
  | custodyRent | custodyDonation | deferredCustodyRent | rentCredit
  | fundingAllocation | initialWorkCapital
  | terminalReceipt | terminalFundingRemaining | hoardPrincipal
  deriving DecidableEq, Repr

def stateSchema : List (FieldSpec StateField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.phase, .u8⟩, ⟨.readiness, .u8⟩,
  ⟨.terminalWinner, .u32⟩,
  ⟨.realmId, .bytes 32⟩, ⟨.collateralMint, .bytes 32⟩,
  ⟨.tokenProgram, .bytes 32⟩, ⟨.collateralRelease, .bytes 32⟩,
  ⟨.productId, .bytes 32⟩, ⟨.resultDomain, .bytes 32⟩,
  ⟨.claimBasis, .bytes 32⟩, ⟨.capacityProfile, .bytes 32⟩,
  ⟨.compilerRelease, .bytes 32⟩, ⟨.outcomeCount, .u32⟩,
  ⟨.productReserved, .reserved 4⟩, ⟨.scalarLimit, .u64⟩,
  ⟨.marketId, .bytes 32⟩, ⟨.identityRealm, .bytes 32⟩,
  ⟨.identityProduct, .bytes 32⟩, ⟨.identityResultDomain, .bytes 32⟩,
  ⟨.resolutionPolicy, .bytes 32⟩,
  ⟨.selectedReleaseSet, .bytes 32⟩, ⟨.generation, .u64⟩,
  ⟨.releaseSetId, .bytes 32⟩,
  ⟨.coreProgram, .bytes 32⟩, ⟨.coreArtifact, .bytes 32⟩, ⟨.coreSemantic, .bytes 32⟩,
  ⟨.claimsProgram, .bytes 32⟩, ⟨.claimsArtifact, .bytes 32⟩,
  ⟨.claimsSemantic, .bytes 32⟩,
  ⟨.tradingProgram, .bytes 32⟩, ⟨.tradingArtifact, .bytes 32⟩,
  ⟨.tradingSemantic, .bytes 32⟩,
  ⟨.resolutionProgram, .bytes 32⟩, ⟨.resolutionArtifact, .bytes 32⟩,
  ⟨.resolutionSemantic, .bytes 32⟩,
  ⟨.custodyProgram, .bytes 32⟩, ⟨.custodyArtifact, .bytes 32⟩,
  ⟨.custodySemantic, .bytes 32⟩,
  ⟨.derivationAuthenticated, .u8⟩, ⟨.coordinateReserved, .reserved 7⟩,
  ⟨.coordinateMarket, .bytes 32⟩, ⟨.coordinateHoard, .bytes 32⟩,
  ⟨.coordinateFund, .bytes 32⟩, ⟨.coordinateReadiness, .bytes 32⟩,
  ⟨.coordinateCustody, .bytes 32⟩, ⟨.coordinateRentCredit, .bytes 32⟩,
  ⟨.marketRent, .u64⟩, ⟨.marketDonation, .u64⟩,
  ⟨.hoardRent, .u64⟩, ⟨.hoardDonation, .u64⟩,
  ⟨.fundRent, .u64⟩, ⟨.fundDonation, .u64⟩,
  ⟨.readinessRent, .u64⟩, ⟨.readinessDonation, .u64⟩,
  ⟨.custodyRent, .u64⟩, ⟨.custodyDonation, .u64⟩,
  ⟨.deferredCustodyRent, .u64⟩, ⟨.rentCredit, .u64⟩,
  ⟨.fundingAllocation, .bytes 32⟩, ⟨.initialWorkCapital, .u64⟩,
  ⟨.terminalReceipt, .bytes 32⟩, ⟨.terminalFundingRemaining, .u64⟩,
  ⟨.hoardPrincipal, .u64⟩
]

def stateLayout : List (PlacedField StateField) := specialize stateSchema
def stateBytes : Nat := schemaWidth stateSchema

namespace StateField

def rustName : StateField → String
  | .magic => "STATE_MAGIC_OFFSET" | .version => "STATE_VERSION_OFFSET"
  | .phase => "STATE_PHASE_OFFSET" | .readiness => "STATE_READINESS_OFFSET"
  | .terminalWinner => "STATE_TERMINAL_WINNER_OFFSET"
  | .realmId => "STATE_REALM_ID_OFFSET" | .collateralMint => "STATE_COLLATERAL_MINT_OFFSET"
  | .tokenProgram => "STATE_TOKEN_PROGRAM_OFFSET"
  | .collateralRelease => "STATE_COLLATERAL_RELEASE_OFFSET"
  | .productId => "STATE_PRODUCT_ID_OFFSET" | .resultDomain => "STATE_RESULT_DOMAIN_OFFSET"
  | .claimBasis => "STATE_CLAIM_BASIS_OFFSET" | .capacityProfile => "STATE_CAPACITY_PROFILE_OFFSET"
  | .compilerRelease => "STATE_COMPILER_RELEASE_OFFSET" | .outcomeCount => "STATE_OUTCOME_COUNT_OFFSET"
  | .productReserved => "STATE_PRODUCT_RESERVED_OFFSET" | .scalarLimit => "STATE_SCALAR_LIMIT_OFFSET"
  | .marketId => "STATE_MARKET_ID_OFFSET" | .identityRealm => "STATE_IDENTITY_REALM_OFFSET"
  | .identityProduct => "STATE_IDENTITY_PRODUCT_OFFSET"
  | .identityResultDomain => "STATE_IDENTITY_RESULT_DOMAIN_OFFSET"
  | .resolutionPolicy => "STATE_RESOLUTION_POLICY_OFFSET"
  | .selectedReleaseSet => "STATE_SELECTED_RELEASE_SET_OFFSET" | .generation => "STATE_GENERATION_OFFSET"
  | .releaseSetId => "STATE_RELEASE_SET_ID_OFFSET"
  | .coreProgram => "STATE_CORE_PROGRAM_OFFSET" | .coreArtifact => "STATE_CORE_ARTIFACT_OFFSET"
  | .coreSemantic => "STATE_CORE_SEMANTIC_OFFSET" | .claimsProgram => "STATE_CLAIMS_PROGRAM_OFFSET"
  | .claimsArtifact => "STATE_CLAIMS_ARTIFACT_OFFSET" | .claimsSemantic => "STATE_CLAIMS_SEMANTIC_OFFSET"
  | .tradingProgram => "STATE_TRADING_PROGRAM_OFFSET" | .tradingArtifact => "STATE_TRADING_ARTIFACT_OFFSET"
  | .tradingSemantic => "STATE_TRADING_SEMANTIC_OFFSET"
  | .resolutionProgram => "STATE_RESOLUTION_PROGRAM_OFFSET"
  | .resolutionArtifact => "STATE_RESOLUTION_ARTIFACT_OFFSET"
  | .resolutionSemantic => "STATE_RESOLUTION_SEMANTIC_OFFSET"
  | .custodyProgram => "STATE_CUSTODY_PROGRAM_OFFSET" | .custodyArtifact => "STATE_CUSTODY_ARTIFACT_OFFSET"
  | .custodySemantic => "STATE_CUSTODY_SEMANTIC_OFFSET"
  | .derivationAuthenticated => "STATE_DERIVATION_AUTHENTICATED_OFFSET"
  | .coordinateReserved => "STATE_COORDINATE_RESERVED_OFFSET"
  | .coordinateMarket => "STATE_COORDINATE_MARKET_OFFSET" | .coordinateHoard => "STATE_COORDINATE_HOARD_OFFSET"
  | .coordinateFund => "STATE_COORDINATE_FUND_OFFSET"
  | .coordinateReadiness => "STATE_COORDINATE_READINESS_OFFSET"
  | .coordinateCustody => "STATE_COORDINATE_CUSTODY_OFFSET"
  | .coordinateRentCredit => "STATE_COORDINATE_RENT_CREDIT_OFFSET"
  | .marketRent => "STATE_MARKET_RENT_OFFSET" | .marketDonation => "STATE_MARKET_DONATION_OFFSET"
  | .hoardRent => "STATE_HOARD_RENT_OFFSET" | .hoardDonation => "STATE_HOARD_DONATION_OFFSET"
  | .fundRent => "STATE_FUND_RENT_OFFSET" | .fundDonation => "STATE_FUND_DONATION_OFFSET"
  | .readinessRent => "STATE_READINESS_RENT_OFFSET"
  | .readinessDonation => "STATE_READINESS_DONATION_OFFSET"
  | .custodyRent => "STATE_CUSTODY_RENT_OFFSET" | .custodyDonation => "STATE_CUSTODY_DONATION_OFFSET"
  | .deferredCustodyRent => "STATE_DEFERRED_CUSTODY_RENT_OFFSET" | .rentCredit => "STATE_RENT_CREDIT_OFFSET"
  | .fundingAllocation => "STATE_FUNDING_ALLOCATION_OFFSET"
  | .initialWorkCapital => "STATE_INITIAL_WORK_CAPITAL_OFFSET"
  | .terminalReceipt => "STATE_TERMINAL_RECEIPT_OFFSET"
  | .terminalFundingRemaining => "STATE_TERMINAL_FUNDING_REMAINING_OFFSET"
  | .hoardPrincipal => "STATE_HOARD_PRINCIPAL_OFFSET"

end StateField

theorem state_schema_width : stateBytes = 1416 := by native_decide
theorem state_schema_unique : (stateSchema.map fun field => field.name).Nodup := by native_decide
theorem state_fields_disjoint : stateLayout.Pairwise Before := specializeFrom_pairwise 0 stateSchema
theorem state_fields_bounded (placed : PlacedField StateField) (member : placed ∈ stateLayout) :
    placed.offset + placed.spec.kind.byteWidth ≤ stateBytes := by
  simpa [stateLayout, stateBytes, specialize] using
    specializeFrom_bounded 0 stateSchema placed member

inductive Action where
  | found | activateFund | openMarket | admitTerminal
  | split | redeem | beginRetiring | retire
  deriving DecidableEq, Repr

def Action.tag : Action → Nat
  | .found => 0 | .activateFund => 1 | .openMarket => 2 | .admitTerminal => 3
  | .split => 4 | .redeem => 5 | .beginRetiring => 6 | .retire => 7

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
    [Action.found, .activateFund, .openMarket, .admitTerminal, .split, .redeem,
      .beginRetiring, .retire].all (fun action => action.tag < 256) = true := by native_decide

theorem action_tags_unique :
    ([Action.found, .activateFund, .openMarket, .admitTerminal, .split, .redeem,
      .beginRetiring, .retire].map Action.tag).Nodup := by native_decide

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
