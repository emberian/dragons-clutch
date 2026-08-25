import DClutchSemantics.MarketCore

/-!
# Executable sparse Market Core examples and hostile substitutions

These closed examples exercise Found, external readiness, Open, generic
capability activation/close, terminal admission, redemption, and retirement.
They pin the one-owner boundary: Core stores lifecycle authority, while Claims,
Custody, Resolution, capability funding, and canonical reference records remain
external authenticated effects.
-/

namespace DClutch.MarketCore.Examples

open DClutch DClutch.MarketCore

def binding (program artifact semantic : Nat) : ExecutionRelease.Binding := {
  program
  artifactRelease := artifact
  semanticRelease := semantic
}

def releases : ExecutionRelease.ReleaseSet := {
  releaseSetId := 99
  core := binding 10 110 210
  claims := binding 11 111 211
  trading := binding 12 112 212
  resolution := binding 13 113 213
  custody := binding 14 114 214
}

def registryProgram : Nat := 9

def admission (role : ExecutionRelease.Role) : ExecutionRelease.Admission := {
  marketRegistryProgram := registryProgram
  marketReleaseSetId := releases.releaseSetId
  selected := releases
  receipt := {
    registryProgram
    releaseSetId := releases.releaseSetId
    role
    observed := releases.binding role
    activationCacheAuthenticated := true
    currentDeploymentReauthenticated := true
  }
}

def realm : Realm := {
  realmId := 100
  collateralMintId := 101
  tokenProgramId := 102
  collateralReleaseId := 103
}

def product : Product := {
  productId := 200
  resultDomainId := 201
  claimBasisId := 202
  capacityProfileId := 203
  compilerReleaseId := 204
  outcomeCount := 17
}

def identity : MarketIdentity := {
  marketId := 1000
  realmId := realm.realmId
  productId := product.productId
  resultDomainId := product.resultDomainId
  resolutionPolicyId := 555
  capabilityManifestId := 556
  executionReleaseSetId := releases.releaseSetId
  registryProgramId := registryProgram
  generation := 1
}

def vacant (address lamports : Nat) : VacantAccount := {
  address
  lamports
  systemOwned := true
  dataEmpty := true
  executable := false
}

def founding : FoundingFrame := {
  realm
  product
  identity
  coreAdmission := admission .core
  quote := { marketRent := 100 }
  accounts := {
    payerLamports := 5000
    rentCreditId := 600
    market := vacant identity.marketId 7
  }
}

def succeeded {ε α : Type} : Except ε α → Bool
  | .ok _ => true
  | .error _ => false

def refusedWith {α : Type} (expected : Refusal) : Except Refusal α → Bool
  | .error actual => actual == expected
  | .ok _ => false

def advance (state : State) (command : Command) : State :=
  match step? state command with
  | .ok settlement => settlement.post
  | .error _ => state

def child : ChildEffectObservation := {
  exactRequestAuthenticated := true
  exactReceiptAuthenticated := true
  postResourceAuthenticated := true
}

def emptyClaims : ClaimsEffectObservation := {
  exactRequestAuthenticated := true
  exactReceiptAuthenticated := true
  postResourceAuthenticated := true
  payout := 0
  aggregateEmpty := true
}

def initial : State := initialState founding

def ready : State := advance initial (.verifyReadiness {
  coreAdmission := admission .core
  manifestReadinessAuthenticated := true
  readinessEffect := child
})

def collateral : CollateralObservation := {
  adapterAuthenticated := true
  realmId := realm.realmId
  collateralMintId := realm.collateralMintId
  tokenProgramId := realm.tokenProgramId
  collateralReleaseId := realm.collateralReleaseId
}

def openFrame : OpenFrame := {
  custodyAdmission := admission .custody
  realm
  realmRecordAuthenticated := true
  custodyDerivationAuthenticated := true
  collateral
  custody := vacant 1004 75
  custodyRentMinimum := 70
  custodyRentAuthenticated := true
  custodyEffect := child
}

def opened : State := advance ready (.openMarket openFrame)

def optionalTrading : CapabilityChildFrame := {
  childAdmission := admission .trading
  targetRole := .trading
  manifestEntryAuthenticated := true
  fundingStateAuthenticated := true
  effect := child
}

def withCapability : State := advance opened (.activateCapability optionalTrading)
def capabilityClosed : State := advance withCapability (.closeCapability optionalTrading)

def terminalCertificate : SourceResolution.Certificate := {
  kind := .resolutionSuccess
  marketId := identity.marketId
  routeId := 77
  sourceMaterialId := identity.resolutionPolicyId
  productId := product.productId
  providerEvidenceId := 88
  fundingAllocationId := 0
  receiptAccountId := 900
  generation := identity.generation
  attemptIndex := 0
  scheduleIndex := 0
  selector := 16
  workPaid := 0
  fundingRemaining := 0
  result := ⟨16, 1⟩
  observedAt := 10000
}

def terminal : State := advance capabilityClosed (.admitTerminal {
  resolutionAdmission := admission .resolution
  product
  productRecordAuthenticated := true
  certificate := terminalCertificate
})

def retiring : State := advance terminal (.beginRetiring (admission .core))

def retired : State := advance retiring (.retire {
  coreAdmission := admission .core
  claimsAdmission := admission .claims
  resolutionAdmission := admission .resolution
  custodyAdmission := admission .custody
  claims := emptyClaims
  source := child
  custody := child
  coreAccountLamports := 123
  coreAccountAuthenticated := true
  rentCreditAuthenticated := true
})

example : foundingAccepts founding = true := by native_decide
example : succeeded (found? founding) = true := by native_decide
example : initial.valid = true := by native_decide
example : (foundingCreationPlan founding).market.rentTopUp = 93 := by native_decide
example : (foundingCreationPlan founding).market.donation = 0 := by native_decide
example : (foundingCreationPlan founding).payerDebit = 93 := by native_decide
example : (foundingCreationPlan founding).payerAfter = 4907 := by native_decide
example : ready.readiness = .ready := by native_decide
example : (openCreationPlan openFrame).rentTopUp = 0 := by native_decide
example : (openCreationPlan openFrame).donation = 5 := by native_decide
example : opened.phase = .open := by native_decide
example : withCapability.outstandingCapabilities = 1 := by native_decide
example : capabilityClosed.outstandingCapabilities = 0 := by native_decide
example : terminal.phase = .terminal 16 := by native_decide
example : retiring.phase = .retiring 16 := by native_decide
example : retired.phase = .retired := by native_decide
example : retired.valid = true := by native_decide

def wrongRealmFounding : FoundingFrame := {
  founding with identity := { founding.identity with realmId := 999 }
}

example : foundingAccepts wrongRealmFounding = false := by native_decide
example : refusedWith .notAdmissible (found? wrongRealmFounding) = true := by native_decide

def incompleteChild : ChildEffectObservation := {
  child with postResourceAuthenticated := false
}

example : refusedWith .childEffectRefusal (step? initial (.verifyReadiness {
  coreAdmission := admission .core
  manifestReadinessAuthenticated := true
  readinessEffect := incompleteChild
})) = true := by native_decide

def wrongCollateral : CollateralObservation := {
  collateral with collateralMintId := 999
}

example : refusedWith .wrongRealmCollateral (step? ready (.openMarket {
  openFrame with collateral := wrongCollateral
})) = true := by native_decide

def wrongCapability : CapabilityChildFrame := {
  optionalTrading with manifestEntryAuthenticated := false
}

example : refusedWith .childEffectRefusal
    (step? opened (.activateCapability wrongCapability)) = true := by native_decide

example : refusedWith .childEffectRefusal
    (step? opened (.closeCapability optionalTrading)) = true := by native_decide

def substitutedTerminal : SourceResolution.Certificate := {
  terminalCertificate with sourceMaterialId := 557
}

example : refusedWith .invalidTerminalReceipt (step? capabilityClosed (.admitTerminal {
  resolutionAdmission := admission .resolution
  product
  productRecordAuthenticated := true
  certificate := substitutedTerminal
})) = true := by native_decide

example : refusedWith .invalidTerminalReceipt (step? capabilityClosed (.admitTerminal {
  resolutionAdmission := admission .resolution
  product
  productRecordAuthenticated := false
  certificate := terminalCertificate
})) = true := by native_decide

/-- Retirement cannot skip closure of a manifest-selected optional child. -/
def terminalWithCapability : State := advance withCapability (.admitTerminal {
  resolutionAdmission := admission .resolution
  product
  productRecordAuthenticated := true
  certificate := terminalCertificate
})

def retiringWithCapability : State :=
  advance terminalWithCapability (.beginRetiring (admission .core))

example : refusedWith .childEffectRefusal (step? retiringWithCapability (.retire {
  coreAdmission := admission .core
  claimsAdmission := admission .claims
  resolutionAdmission := admission .resolution
  custodyAdmission := admission .custody
  claims := emptyClaims
  source := child
  custody := child
  coreAccountLamports := 123
  coreAccountAuthenticated := true
  rentCreditAuthenticated := true
})) = true := by native_decide

/-- Outcome width remains Product data. Core has no width-specialized command or
semantic N=16 ceiling. -/
example : product.outcomeCount = 17 := by native_decide
example : product.valid = true := by native_decide

end DClutch.MarketCore.Examples
