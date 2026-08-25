import DClutchSemantics.MarketCore

/-!
# Executable Market Core examples and hostile substitutions

These closed examples exercise the complete Found → Fund-ready → Open →
terminal → retiring → redemption → retired path.  They also pin refusal of
release, Realm, terminal-receipt, funding, and lifecycle substitutions.
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

def admission (role : ExecutionRelease.Role) : ExecutionRelease.Admission := {
  marketReleaseSetId := releases.releaseSetId
  selected := releases
  receipt := {
    registryProgram := releases.core.program
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
  outcomeCount := 4
  scalarLimit := 1000
}

def identity : MarketIdentity := {
  marketId := 1000
  realmId := realm.realmId
  productId := product.productId
  resultDomainId := product.resultDomainId
  resolutionPolicyId := 555
  executionReleaseSetId := releases.releaseSetId
  generation := 1
}

def vacant (address lamports : Nat) : VacantAccount := {
  address
  lamports
  systemOwned := true
  dataEmpty := true
  executable := false
}

def quote : FoundingQuote := {
  marketRent := 100
  hoardRent := 50
  fundRent := 80
  readinessRent := 30
  custodyRent := 70
  sourceFundingAllocationId := 700
  sourceWorkCapital := 500
}

def founding : FoundingFrame := {
  realm
  product
  identity
  coreAdmission := admission .core
  coordinates := {
    derivationAuthenticated := true
    marketId := identity.marketId
    hoardId := 1001
    fundId := 1002
    readinessId := 1003
    custodyId := 1004
    rentCreditId := 600
  }
  quote
  accounts := {
    payerLamports := 5000
    rentCreditId := 600
    rentCreditLamports := 100
    market := vacant identity.marketId 7
    hoard := vacant 1001 55
    fund := vacant 1002 10
    readiness := vacant 1003 35
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

def initial : State := initialState founding

def initialFunding : SourceResolution.FundingState := {
  allocationId := quote.sourceFundingAllocationId
  initialCapital := quote.sourceWorkCapital
  remainingCapital := quote.sourceWorkCapital
  paidCapital := 0
  callCount := 0
}

def ready : State := advance initial (.activateFund {
  resolutionAdmission := admission .resolution
  funding := initialFunding
})

def collateral : CollateralObservation := {
  adapterAuthenticated := true
  realmId := realm.realmId
  collateralMintId := realm.collateralMintId
  tokenProgramId := realm.tokenProgramId
  collateralReleaseId := realm.collateralReleaseId
}

def opened : State := advance ready (.openMarket {
  custodyAdmission := admission .custody
  collateral
  custody := vacant 1004 20
})

def economicCommand (command : Economic.Command) : Command := .economic {
  claimsAdmission := admission .claims
  custodyAdmission := admission .custody
  bindings := { source := .seller, destination := .buyer, hoard := .venue }
  command
}

def issued : State :=
  advance opened (economicCommand (.splitCompleteSet .source .native 10))

def terminalCertificate : SourceResolution.Certificate := {
  kind := .resolutionSuccess
  marketId := identity.marketId
  routeId := 77
  sourceMaterialId := identity.resolutionPolicyId
  productId := product.productId
  providerEvidenceId := 88
  fundingAllocationId := quote.sourceFundingAllocationId
  receiptAccountId := 900
  generation := identity.generation
  attemptIndex := 0
  scheduleIndex := 0
  selector := 1
  workPaid := 100
  fundingRemaining := 400
  result := ⟨1, 1⟩
  observedAt := 10000
}

def terminal : State := advance issued (.admitTerminal {
  resolutionAdmission := admission .resolution
  certificate := terminalCertificate
})

def retiring : State := advance terminal (.beginRetiring (admission .core))

def redeemedWinner : State :=
  advance retiring (economicCommand (.redeemTerminal .source .native 1 10))

def redeemedZero : State :=
  advance redeemedWinner (economicCommand (.redeemTerminal .source .native 0 10))

def redeemedTwo : State :=
  advance redeemedZero (economicCommand (.redeemTerminal .source .native 2 10))

def fullyRedeemed : State :=
  advance redeemedTwo (economicCommand (.redeemTerminal .source .native 3 10))

def terminalFunding : SourceResolution.FundingState := {
  allocationId := quote.sourceFundingAllocationId
  initialCapital := quote.sourceWorkCapital
  remainingCapital := terminalCertificate.fundingRemaining
  paidCapital := terminalCertificate.workPaid
  callCount := 1
}

def retired : State := advance fullyRedeemed (.retire {
  coreAdmission := admission .core
  custodyAdmission := admission .custody
  funding := terminalFunding
})

example : foundingAccepts founding = true := by native_decide
example : succeeded (found? founding) = true := by native_decide
example : initial.valid = true := by native_decide

/-- Existing lamports reduce only rent top-ups.  Work and deferred custody
principal remain exact new funding. -/
example : (foundingCreationPlan founding).payerDebit = 733 := by native_decide
example : (foundingCreationPlan founding).payerAfter = 4267 := by native_decide
example : (foundingCreationPlan founding).hoard.semanticPrincipal = 0 := by native_decide
example : (foundingCreationPlan founding).hoard.donation = 5 := by native_decide
example : (foundingCreationPlan founding).fund.semanticPrincipal = 570 := by native_decide

example : ready.readiness = .ready := by native_decide
example : opened.phase = .open := by native_decide
example : opened.capital.custodyRent = quote.custodyRent := by native_decide
example : opened.capital.rentCredit = 155 := by native_decide
example : issued.economic.hoard = 10 := by native_decide
example : issued.economic.supply = [10, 10, 10, 10] := by native_decide
example : terminal.phase = .terminal 1 := by native_decide
example : retiring.phase = .retiring 1 := by native_decide
example : redeemedWinner.economic.hoard = 0 := by native_decide
example : fullyRedeemed.economic.supply = [0, 0, 0, 0] := by native_decide
example : retired.phase = .retired := by native_decide
example : retired.capital.rentCredit = 860 := by native_decide
example : retired.valid = true := by native_decide

def wrongRealmFounding : FoundingFrame := {
  founding with identity := { founding.identity with realmId := 999 }
}

example : foundingAccepts wrongRealmFounding = false := by native_decide
example : refusedWith .notAdmissible (found? wrongRealmFounding) = true := by native_decide

def substitutedCoreAdmission : ExecutionRelease.Admission := {
  admission .core with marketReleaseSetId := 998
}

def substitutedFounding : FoundingFrame := {
  founding with coreAdmission := substitutedCoreAdmission
}

example : foundingAccepts substitutedFounding = false := by native_decide

/-- A current authenticated custody release is necessary but not sufficient:
Open also requires the exact immutable Realm collateral coordinates. -/
def wrongCollateral : CollateralObservation := {
  collateral with collateralMintId := 999
}

example : refusedWith .wrongRealmCollateral (step? ready (.openMarket {
    custodyAdmission := admission .custody
    collateral := wrongCollateral
    custody := vacant 1004 20
  })) = true := by native_decide

example : refusedWith .wrongPhase (step? initial (.openMarket {
    custodyAdmission := admission .custody
    collateral
    custody := vacant 1004 20
  })) = true := by native_decide

def substitutedTerminal : SourceResolution.Certificate := {
  terminalCertificate with sourceMaterialId := 556
}

example : refusedWith .invalidTerminalReceipt (step? issued (.admitTerminal {
    resolutionAdmission := admission .resolution
    certificate := substitutedTerminal
  })) = true := by native_decide

def substitutedTerminalFunding : SourceResolution.Certificate := {
  terminalCertificate with fundingAllocationId := 701
}

example : refusedWith .invalidTerminalReceipt (step? issued (.admitTerminal {
    resolutionAdmission := admission .resolution
    certificate := substitutedTerminalFunding
  })) = true := by native_decide

/-- Retirement cannot skip redemption merely because a terminal result exists. -/
example : refusedWith .economicRefusal (step? retiring (.retire {
    coreAdmission := admission .core
    custodyAdmission := admission .custody
    funding := terminalFunding
  })) = true := by native_decide

/-- Outcome width is Product data.  Core introduces no N-specific function or
semantic N=16 ceiling. -/
def widerProduct : Product := { product with outcomeCount := 17 }
def widerIdentity : MarketIdentity := { identity with productId := widerProduct.productId }
def widerFounding : FoundingFrame := { founding with
  product := widerProduct
  identity := widerIdentity
}

example : widerProduct.valid = true := by native_decide
example : foundingAccepts widerFounding = true := by native_decide

end DClutch.MarketCore.Examples
