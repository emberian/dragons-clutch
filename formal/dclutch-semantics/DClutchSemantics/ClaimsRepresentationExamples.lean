import DClutchSemantics.ClaimsRepresentation

/-!
# Executable representation-specializer examples

These examples execute the Lean semantics.  They do not claim Solana, token
adapter, persistence, or atomic rollback evidence.
-/

namespace DClutch.ClaimsRepresentation.Examples

open DClutch
open DClutch.Economic
open DClutch.ClaimsRepresentation
open DClutch.ExecutionRelease

def limit : Nat := 18446744073709551616

def binding (program artifact semantic : Nat) : Binding := {
  program
  artifactRelease := artifact
  semanticRelease := semantic
}

def release : ReleaseSet := {
  releaseSetId := 1
  core := binding 2 3 4
  claims := binding 5 6 7
  trading := binding 8 9 10
  resolution := binding 11 12 13
  custody := binding 14 15 16
}

def marketRegistryProgram : Identity := 17

def admission : ExecutionRelease.Admission := {
  marketRegistryProgram
  marketReleaseSetId := release.releaseSetId
  selected := release
  receipt := {
    registryProgram := marketRegistryProgram
    releaseSetId := release.releaseSetId
    role := .claims
    observed := release.claims
    activationCacheAuthenticated := true
    currentDeploymentReauthenticated := true
  }
}

theorem registry_ownership_and_release_selection_are_exact :
    marketRegistryProgram ≠ release.core.program /\
    admission.marketRegistryProgram = marketRegistryProgram /\
    admission.receipt.registryProgram = marketRegistryProgram /\
    admission.marketReleaseSetId = release.releaseSetId /\
    ExecutionRelease.admits admission .claims = true := by
  native_decide

def parties : Parties := {
  claimant := .seller
  wrapper := .buyer
  hoard := .venue
}

def fractionalDescriptor : Descriptor := {
  descriptorId := 11
  marketId := 12
  productId := 13
  resultDomainId := 14
  adapterAssetId := 15
  outcomeCount := 3
  claimAtomsPerLot := [1, 2, 1]
  receiptUnitsPerLot := 10
  releaseSetId := release.releaseSetId
}

def bearerDescriptor : Descriptor := {
  fractionalDescriptor with
  descriptorId := 21
  adapterAssetId := 22
  claimAtomsPerLot := [0, 1, 0]
  receiptUnitsPerLot := 1
}

def structuredDescriptor : Descriptor := {
  fractionalDescriptor with
  descriptorId := 31
  adapterAssetId := 32
  claimAtomsPerLot := [2, 0, 3]
  receiptUnitsPerLot := 1
}

theorem presentation_shapes_are_data_not_dispatch :
    bearerDescriptor.isBearerPresentation = true ∧
    structuredDescriptor.isStructuredPresentation = true ∧
    fractionalDescriptor.isFractionalPresentation = true ∧
    fractionalDescriptor.expectedClaims 3 = [3, 6, 3] ∧
    fractionalDescriptor.expectedReceiptUnits 3 = 30 := by
  native_decide

def issueEconomicPre : Economic.State := {
  phase := .open
  hoard := 10
  supply := [10, 10, 10]
  nativeSupply := [10, 10, 10]
  materializedSupply := [0, 0, 0]
  sourceNative := [10, 10, 10]
  sourceMaterialized := [0, 0, 0]
  destinationNative := [0, 0, 0]
  destinationMaterialized := [0, 0, 0]
}

def emptyFractional : State := {
  descriptorId := fractionalDescriptor.descriptorId
  nextNonce := 0
  issuedLots := 0
  retired := false
}

def adapterFor
    (descriptor : Descriptor) (lots : Nat) (authenticated : Bool := true) :
    AdapterProjection := {
  adapterAuthenticated := authenticated
  descriptorId := descriptor.descriptorId
  adapterAssetId := descriptor.adapterAssetId
  observedReceiptUnits := descriptor.expectedReceiptUnits lots
}

def issueFractional : Frame := {
  scalarLimit := limit
  descriptor := fractionalDescriptor
  admission
  adapterPre := adapterFor fractionalDescriptor 0
  parties
  wrapperPre := emptyFractional
  economicPre := issueEconomicPre
  command := .issue 3 0
}

def emptyFor (descriptor : Descriptor) : State := {
  descriptorId := descriptor.descriptorId
  nextNonce := 0
  issuedLots := 0
  retired := false
}

def issueBearer : Frame := {
  issueFractional with
  descriptor := bearerDescriptor
  adapterPre := adapterFor bearerDescriptor 0
  wrapperPre := emptyFor bearerDescriptor
  command := .issue 4 0
}

def issueStructured : Frame := {
  issueFractional with
  descriptor := structuredDescriptor
  adapterPre := adapterFor structuredDescriptor 0
  wrapperPre := emptyFor structuredDescriptor
  command := .issue 2 0
}

theorem bearer_structured_and_fractional_share_one_executor :
    succeeded issueBearer = true ∧
    emittedAdapterMutation? issueBearer = some (.mint .seller 4) ∧
    (runEconomicState issueBearer).materializedSupply = [0, 4, 0] ∧
    succeeded issueStructured = true ∧
    emittedAdapterMutation? issueStructured = some (.mint .seller 2) ∧
    (runEconomicState issueStructured).materializedSupply = [4, 0, 6] ∧
    succeeded issueFractional = true := by
  native_decide

theorem fractional_issue_executes_exact_basket :
    accepts issueFractional = true ∧
    succeeded issueFractional = true ∧
    (runWrapperState issueFractional).issuedLots = 3 ∧
    emittedAdapterMutation? issueFractional = some (.mint .seller 30) ∧
    (runEconomicState issueFractional).supply = [10, 10, 10] ∧
    (runEconomicState issueFractional).hoard = 10 ∧
    (runEconomicState issueFractional).nativeSupply = [7, 4, 7] ∧
    (runEconomicState issueFractional).materializedSupply = [3, 6, 3] ∧
    (runEconomicState issueFractional).destinationMaterialized = [3, 6, 3] := by
  native_decide

def issuedFractional : State := {
  emptyFractional with
  nextNonce := 1
  issuedLots := 3
}

def redeemEconomicPre : Economic.State := {
  phase := .open
  hoard := 10
  supply := [10, 10, 10]
  nativeSupply := [7, 4, 7]
  materializedSupply := [3, 6, 3]
  sourceNative := [0, 0, 0]
  sourceMaterialized := [3, 6, 3]
  destinationNative := [7, 4, 7]
  destinationMaterialized := [0, 0, 0]
}

def redeemFractional : Frame := {
  issueFractional with
  adapterPre := adapterFor fractionalDescriptor 3
  wrapperPre := issuedFractional
  economicPre := redeemEconomicPre
  command := .redeem 2 1
}

theorem fractional_redeem_is_exact_and_rounding_free :
    accepts redeemFractional = true ∧
    (runWrapperState redeemFractional).issuedLots = 1 ∧
    emittedAdapterMutation? redeemFractional = some (.burn .seller 20) ∧
    (runEconomicState redeemFractional).supply = redeemEconomicPre.supply ∧
    (runEconomicState redeemFractional).hoard = redeemEconomicPre.hoard ∧
    (runEconomicState redeemFractional).nativeSupply = [9, 8, 9] ∧
    (runEconomicState redeemFractional).materializedSupply = [1, 2, 1] ∧
    (runEconomicState redeemFractional).sourceMaterialized = [1, 2, 1] := by
  native_decide

def terminalEconomicPre : Economic.State := {
  phase := .retiring 1
  hoard := 6
  supply := [3, 6, 3]
  nativeSupply := [0, 0, 0]
  materializedSupply := [3, 6, 3]
  sourceNative := [0, 0, 0]
  sourceMaterialized := [3, 6, 3]
  destinationNative := [0, 0, 0]
  destinationMaterialized := [0, 0, 0]
}

def redeemTerminalFractional : Frame := {
  redeemFractional with
  economicPre := terminalEconomicPre
  command := .redeemTerminal 3 1
}

theorem terminal_redemption_burns_receipts_and_exact_claim_basket :
    accepts redeemTerminalFractional = true ∧
    (runWrapperState redeemTerminalFractional).issuedLots = 0 ∧
    emittedAdapterMutation? redeemTerminalFractional = some (.burn .seller 30) ∧
    (runEconomicState redeemTerminalFractional).hoard = 0 ∧
    (runEconomicState redeemTerminalFractional).supply = [0, 0, 0] ∧
    (runEconomicState redeemTerminalFractional).materializedSupply = [0, 0, 0] ∧
    (runEconomicState redeemTerminalFractional).sourceMaterialized = [0, 0, 0] := by
  native_decide

def terminalEmpty : Economic.State := {
  terminalEconomicPre with
  hoard := 0
  supply := [0, 0, 0]
  nativeSupply := [0, 0, 0]
  materializedSupply := [0, 0, 0]
  sourceMaterialized := [0, 0, 0]
}

def retireFractional : Frame := {
  redeemTerminalFractional with
  adapterPre := adapterFor fractionalDescriptor 0
  wrapperPre := { issuedFractional with
    nextNonce := 2
    issuedLots := 0
  }
  economicPre := terminalEmpty
  command := .retire 2
}

theorem empty_terminal_wrapper_retires_without_retiring_market_authority :
    accepts retireFractional = true ∧
    (runWrapperState retireFractional).retired = true ∧
    emittedAdapterMutation? retireFractional = some .retire ∧
    runEconomicState retireFractional = terminalEmpty := by
  native_decide

def hostileReleaseSubstitution : Frame := {
  issueFractional with
  admission := { admission with marketReleaseSetId := 99 }
}

def hostileAdapterSupply : Frame := {
  redeemFractional with
  adapterPre := {
    adapterFor fractionalDescriptor 3 with observedReceiptUnits := 29
  }
}

def hostileProjection : Frame := {
  redeemFractional with
  economicPre := { redeemEconomicPre with sourceMaterialized := [3, 5, 3] }
}

def hostileReplay : Frame := {
  redeemFractional with command := .redeem 2 0
}

def hostileOverdraw : Frame := {
  redeemFractional with command := .redeem 4 1
}

def hostileEarlyRetire : Frame := {
  issueFractional with command := .retire 0
}

theorem hostile_identity_conservation_and_lifecycle_cases_refuse :
    accepts hostileReleaseSubstitution = false ∧
    runWrapperState hostileReleaseSubstitution = hostileReleaseSubstitution.wrapperPre ∧
    accepts hostileAdapterSupply = false ∧
    runWrapperState hostileAdapterSupply = hostileAdapterSupply.wrapperPre ∧
    accepts hostileProjection = false ∧
    runWrapperState hostileProjection = hostileProjection.wrapperPre ∧
    accepts hostileReplay = false ∧
    runWrapperState hostileReplay = hostileReplay.wrapperPre ∧
    accepts hostileOverdraw = false ∧
    runWrapperState hostileOverdraw = hostileOverdraw.wrapperPre ∧
    accepts hostileEarlyRetire = false ∧
    runWrapperState hostileEarlyRetire = hostileEarlyRetire.wrapperPre := by
  native_decide

end DClutch.ClaimsRepresentation.Examples
