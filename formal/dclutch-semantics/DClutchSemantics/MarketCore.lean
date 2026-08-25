import DClutchSemantics.EconomicKernel
import DClutchSemantics.ExecutionRelease
import DClutchSemantics.SourceResolution
import Std.Tactic

/-!
# Universal Market Core and funded founding lifecycle

This module is the small lifecycle shared by every dClutch Market.  It owns an
immutable Realm, canonical Product/result-domain identities, one immutable
execution-release set, exact rent/funding commitments, the universal economic
state, terminal admission, redemption, and retirement.  Trading venues,
liquidity, wrappers, bearer representations, and recovery depth are absent:
they are capability children rather than optional Core slots.

Account observations and release receipts are normalized adapter inputs.
Solana address derivation, loader inspection, account ownership, CPI, and
transaction rollback remain outside this theorem boundary.
-/

namespace DClutch.MarketCore

open DClutch

abbrev Identity := ExecutionRelease.Identity

/-! ## Immutable ontology -/

/-- One immutable collateral Realm. -/
structure Realm where
  realmId : Identity
  collateralMintId : Identity
  tokenProgramId : Identity
  collateralReleaseId : Identity
  deriving DecidableEq, Repr

def Realm.valid (realm : Realm) : Bool :=
  realm.realmId != 0 && realm.collateralMintId != 0 &&
  realm.tokenProgramId != 0 && realm.collateralReleaseId != 0

/-- Canonical Product and result-domain coordinates selected before Found. -/
structure Product where
  productId : Identity
  resultDomainId : Identity
  claimBasisId : Identity
  capacityProfileId : Identity
  compilerReleaseId : Identity
  outcomeCount : Nat
  scalarLimit : Nat
  deriving DecidableEq, Repr

def Product.valid (product : Product) : Bool :=
  product.productId != 0 && product.resultDomainId != 0 &&
  product.claimBasisId != 0 && product.capacityProfileId != 0 &&
  product.compilerReleaseId != 0 && 1 < product.outcomeCount &&
  0 < product.scalarLimit

/-- Exact immutable Market identity.  `marketId` is the adapter-checked
canonical content/address identity for these coordinates. -/
structure MarketIdentity where
  marketId : Identity
  realmId : Identity
  productId : Identity
  resultDomainId : Identity
  resolutionPolicyId : Identity
  executionReleaseSetId : Identity
  generation : Nat
  deriving DecidableEq, Repr

def MarketIdentity.valid (identity : MarketIdentity) : Bool :=
  identity.marketId != 0 && identity.realmId != 0 &&
  identity.productId != 0 && identity.resultDomainId != 0 &&
  identity.resolutionPolicyId != 0 && identity.executionReleaseSetId != 0

/-! ## Dust-tolerant exact account creation -/

/-- Hostile observation of an address expected to be an unallocated System
account.  Its lamports may be nonzero. -/
structure VacantAccount where
  address : Identity
  lamports : Nat
  systemOwned : Bool
  dataEmpty : Bool
  executable : Bool
  deriving DecidableEq, Repr

def VacantAccount.valid (account : VacantAccount) : Bool :=
  account.address != 0 && account.systemOwned && account.dataEmpty && !account.executable

/-- Only the rent shortfall is filled from the payer.  Lamports above rent are
retained as explicitly unclassified donation, never reclassified as semantic
principal. -/
def rentTopUp (before rentMinimum : Nat) : Nat := rentMinimum - before

def donationAboveRent (before rentMinimum : Nat) : Nat := before - rentMinimum

/-- Exact creation of one account with separately named semantic principal. -/
structure AccountCreation where
  address : Identity
  before : Nat
  rentMinimum : Nat
  rentTopUp : Nat
  semanticPrincipal : Nat
  donation : Nat
  after : Nat
  deriving DecidableEq, Repr

def planAccountCreation
    (account : VacantAccount) (rentMinimum semanticPrincipal : Nat) : AccountCreation := {
  address := account.address
  before := account.lamports
  rentMinimum
  rentTopUp := rentTopUp account.lamports rentMinimum
  semanticPrincipal
  donation := donationAboveRent account.lamports rentMinimum
  after := account.lamports + rentTopUp account.lamports rentMinimum + semanticPrincipal
}

/-- Dust never silently becomes Market principal. -/
theorem account_creation_decomposes
    (account : VacantAccount) (rentMinimum semanticPrincipal : Nat) :
    (planAccountCreation account rentMinimum semanticPrincipal).after =
      rentMinimum + semanticPrincipal +
        (planAccountCreation account rentMinimum semanticPrincipal).donation := by
  simp [planAccountCreation, rentTopUp, donationAboveRent]
  omega

/-- Deferred rent is partitioned exactly between the vacancy top-up and the
unused reserve returned to RentCredit. -/
theorem rent_reserve_partition (before reserved : Nat) :
    rentTopUp before reserved + (reserved - rentTopUp before reserved) = reserved := by
  unfold rentTopUp
  omega

/-- Exact prepaid Found requirements.  Work capital and deferred custody rent
are present lamports.  Future fees and Hoard collateral do not appear. -/
structure FoundingQuote where
  marketRent : Nat
  hoardRent : Nat
  fundRent : Nat
  readinessRent : Nat
  custodyRent : Nat
  sourceFundingAllocationId : Identity
  sourceWorkCapital : Nat
  deriving DecidableEq, Repr

def FoundingQuote.valid (quote : FoundingQuote) : Bool :=
  0 < quote.marketRent && 0 < quote.hoardRent && 0 < quote.fundRent &&
  0 < quote.readinessRent && 0 < quote.custodyRent &&
  quote.sourceFundingAllocationId != 0 && 0 < quote.sourceWorkCapital

structure FoundingAccounts where
  payerLamports : Nat
  rentCreditId : Identity
  rentCreditLamports : Nat
  market : VacantAccount
  hoard : VacantAccount
  fund : VacantAccount
  readiness : VacantAccount
  deriving DecidableEq, Repr

/-- Adapter-authenticated canonical fixed Core children.  This is a closed
fixed set; venue and wrapper addresses are deliberately not Core fields. -/
structure CoreAccountCoordinates where
  derivationAuthenticated : Bool
  marketId : Identity
  hoardId : Identity
  fundId : Identity
  readinessId : Identity
  custodyId : Identity
  rentCreditId : Identity
  deriving DecidableEq, Repr

def CoreAccountCoordinates.addresses (coordinates : CoreAccountCoordinates) : List Identity := [
  coordinates.marketId,
  coordinates.hoardId,
  coordinates.fundId,
  coordinates.readinessId,
  coordinates.custodyId,
  coordinates.rentCreditId
]

def CoreAccountCoordinates.valid (coordinates : CoreAccountCoordinates) : Bool :=
  coordinates.derivationAuthenticated &&
  coordinates.addresses.all (fun address => address != 0) &&
  decide (coordinates.addresses.Pairwise fun left right => left ≠ right)

def FoundingAccounts.addresses (accounts : FoundingAccounts) : List Identity := [
  accounts.rentCreditId,
  accounts.market.address,
  accounts.hoard.address,
  accounts.fund.address,
  accounts.readiness.address
]

def FoundingAccounts.distinct (accounts : FoundingAccounts) : Bool :=
  decide (accounts.addresses.Pairwise fun left right => left ≠ right)

structure FoundingFrame where
  realm : Realm
  product : Product
  identity : MarketIdentity
  coreAdmission : ExecutionRelease.Admission
  coordinates : CoreAccountCoordinates
  quote : FoundingQuote
  accounts : FoundingAccounts
  deriving DecidableEq, Repr

structure FoundingCreationPlan where
  market : AccountCreation
  hoard : AccountCreation
  fund : AccountCreation
  readiness : AccountCreation
  payerDebit : Nat
  payerAfter : Nat
  deriving DecidableEq, Repr

def foundingCreationPlan (frame : FoundingFrame) : FoundingCreationPlan :=
  let market := planAccountCreation frame.accounts.market frame.quote.marketRent 0
  let hoard := planAccountCreation frame.accounts.hoard frame.quote.hoardRent 0
  let fundPrincipal := frame.quote.sourceWorkCapital + frame.quote.custodyRent
  let fund := planAccountCreation frame.accounts.fund frame.quote.fundRent fundPrincipal
  let readiness :=
    planAccountCreation frame.accounts.readiness frame.quote.readinessRent 0
  let debit := market.rentTopUp + hoard.rentTopUp +
    fund.rentTopUp + fund.semanticPrincipal + readiness.rentTopUp
  {
    market
    hoard
    fund
    readiness
    payerDebit := debit
    payerAfter := frame.accounts.payerLamports - debit
  }

def foundingAccepts (frame : FoundingFrame) : Bool :=
  let plan := foundingCreationPlan frame
  frame.realm.valid && frame.product.valid && frame.identity.valid &&
  frame.identity.realmId == frame.realm.realmId &&
  frame.identity.productId == frame.product.productId &&
  frame.identity.resultDomainId == frame.product.resultDomainId &&
  frame.identity.executionReleaseSetId == frame.coreAdmission.selected.releaseSetId &&
  frame.coreAdmission.marketReleaseSetId == frame.identity.executionReleaseSetId &&
  ExecutionRelease.admits frame.coreAdmission .core &&
  frame.coordinates.valid && frame.coordinates.marketId == frame.identity.marketId &&
  frame.accounts.rentCreditId == frame.coordinates.rentCreditId &&
  frame.accounts.market.address == frame.coordinates.marketId &&
  frame.accounts.hoard.address == frame.coordinates.hoardId &&
  frame.accounts.fund.address == frame.coordinates.fundId &&
  frame.accounts.readiness.address == frame.coordinates.readinessId &&
  frame.quote.valid &&
  frame.accounts.market.valid && frame.accounts.hoard.valid &&
  frame.accounts.fund.valid && frame.accounts.readiness.valid &&
  frame.accounts.distinct && plan.payerDebit <= frame.accounts.payerLamports

/-! ## Persistent Core state -/

inductive Phase where
  | founding
  | open
  | terminal (winner : Nat)
  | retiring (winner : Nat)
  | retired
  deriving DecidableEq, Repr

inductive Readiness where
  | prepaid
  | ready
  | consumed
  deriving DecidableEq, Repr

/-- Semantic capital classes.  Claimant Hoard principal has its own field and
is tied exactly to the Economic kernel. -/
structure Capital where
  marketRent : Nat
  marketDonation : Nat
  hoardRent : Nat
  hoardDonation : Nat
  fundRent : Nat
  fundDonation : Nat
  readinessRent : Nat
  readinessDonation : Nat
  custodyRent : Nat
  custodyDonation : Nat
  deferredCustodyRent : Nat
  rentCredit : Nat
  deriving DecidableEq, Repr

structure FundingCommitment where
  allocationId : Identity
  initialWorkCapital : Nat
  deriving DecidableEq, Repr

structure State where
  realm : Realm
  product : Product
  identity : MarketIdentity
  executionReleases : ExecutionRelease.ReleaseSet
  coordinates : CoreAccountCoordinates
  phase : Phase
  readiness : Readiness
  capital : Capital
  funding : FundingCommitment
  terminalReceiptId : Identity
  terminalFundingRemaining : Nat
  economic : Economic.State
  deriving DecidableEq, Repr

def zeroVector (count : Nat) : List Nat := List.replicate count 0

def initialEconomic (product : Product) : Economic.State := {
  phase := .open
  hoard := 0
  supply := zeroVector product.outcomeCount
  nativeSupply := zeroVector product.outcomeCount
  materializedSupply := zeroVector product.outcomeCount
  sourceNative := zeroVector product.outcomeCount
  sourceMaterialized := zeroVector product.outcomeCount
  destinationNative := zeroVector product.outcomeCount
  destinationMaterialized := zeroVector product.outcomeCount
}

def phaseMatches (state : State) : Bool :=
  match state.phase, state.economic.phase with
  | .founding, .open => state.readiness != .consumed
  | .open, .open => state.readiness = .consumed
  | .terminal left, .terminal right => left = right && state.readiness = .consumed
  | .retiring left, .retiring right => left = right && state.readiness = .consumed
  | .retired, .retired => state.readiness = .consumed
  | _, _ => false

def terminalFieldsValid (state : State) : Bool :=
  match state.phase with
  | .founding | .open => state.terminalReceiptId = 0 && state.terminalFundingRemaining = 0
  | .terminal _ | .retiring _ => state.terminalReceiptId != 0
  | .retired => state.terminalReceiptId != 0 && state.terminalFundingRemaining = 0

def capitalPhaseValid (state : State) : Bool :=
  match state.phase with
  | .founding =>
      0 < state.capital.marketRent && 0 < state.capital.hoardRent &&
      0 < state.capital.fundRent && 0 < state.capital.readinessRent &&
      0 < state.capital.deferredCustodyRent && state.capital.custodyRent = 0
  | .open | .terminal _ | .retiring _ =>
      0 < state.capital.marketRent && 0 < state.capital.hoardRent &&
      0 < state.capital.fundRent && state.capital.readinessRent = 0 &&
      state.capital.deferredCustodyRent = 0 && 0 < state.capital.custodyRent
  | .retired =>
      state.capital.marketRent = 0 && state.capital.marketDonation = 0 &&
      state.capital.hoardRent = 0 && state.capital.hoardDonation = 0 &&
      state.capital.fundRent = 0 && state.capital.fundDonation = 0 &&
      state.capital.readinessRent = 0 && state.capital.readinessDonation = 0 &&
      state.capital.custodyRent = 0 && state.capital.custodyDonation = 0 &&
      state.capital.deferredCustodyRent = 0

/-- Complete executable invariant. -/
def State.valid (state : State) : Bool :=
  state.realm.valid && state.product.valid && state.identity.valid &&
  state.identity.realmId == state.realm.realmId &&
  state.identity.productId == state.product.productId &&
  state.identity.resultDomainId == state.product.resultDomainId &&
  state.identity.executionReleaseSetId == state.executionReleases.releaseSetId &&
  ExecutionRelease.releaseSetValid state.executionReleases &&
  state.coordinates.valid && state.coordinates.marketId == state.identity.marketId &&
  state.funding.allocationId != 0 && 0 < state.funding.initialWorkCapital &&
  phaseMatches state && terminalFieldsValid state && capitalPhaseValid state &&
  Economic.valid state.product.outcomeCount state.product.scalarLimit state.economic

def initialState (frame : FoundingFrame) : State :=
  let plan := foundingCreationPlan frame
  {
    realm := frame.realm
    product := frame.product
    identity := frame.identity
    executionReleases := frame.coreAdmission.selected
    coordinates := frame.coordinates
    phase := .founding
    readiness := .prepaid
    capital := {
      marketRent := frame.quote.marketRent
      marketDonation := plan.market.donation
      hoardRent := frame.quote.hoardRent
      hoardDonation := plan.hoard.donation
      fundRent := frame.quote.fundRent
      fundDonation := plan.fund.donation
      readinessRent := frame.quote.readinessRent
      readinessDonation := plan.readiness.donation
      custodyRent := 0
      custodyDonation := 0
      deferredCustodyRent := frame.quote.custodyRent
      rentCredit := frame.accounts.rentCreditLamports
    }
    funding := {
      allocationId := frame.quote.sourceFundingAllocationId
      initialWorkCapital := frame.quote.sourceWorkCapital
    }
    terminalReceiptId := 0
    terminalFundingRemaining := 0
    economic := initialEconomic frame.product
  }

structure FoundingResult (frame : FoundingFrame) where
  post : State
  sourceFunding : SourceResolution.FundingState
  creation : FoundingCreationPlan
  accepted : foundingAccepts frame = true
  postExact : post = initialState frame
  postValid : post.valid = true
  fundingExact : sourceFunding = {
    allocationId := frame.quote.sourceFundingAllocationId
    initialCapital := frame.quote.sourceWorkCapital
    remainingCapital := frame.quote.sourceWorkCapital
    paidCapital := 0
    callCount := 0
  }

inductive Refusal where
  | notAdmissible
  | invalidState
  | wrongRelease
  | wrongPhase
  | wrongRealmCollateral
  | wrongFunding
  | invalidTerminalReceipt
  | invalidAccount
  | economicRefusal
  | candidateInvariantFailure
  deriving DecidableEq, Repr

/-- Total Found boundary. -/
def found? (frame : FoundingFrame) : Except Refusal (FoundingResult frame) :=
  if accepted : foundingAccepts frame = true then
    let candidate := initialState frame
    if candidateValid : candidate.valid = true then
      let funding : SourceResolution.FundingState := {
        allocationId := frame.quote.sourceFundingAllocationId
        initialCapital := frame.quote.sourceWorkCapital
        remainingCapital := frame.quote.sourceWorkCapital
        paidCapital := 0
        callCount := 0
      }
      .ok {
        post := candidate
        sourceFunding := funding
        creation := foundingCreationPlan frame
        accepted
        postExact := rfl
        postValid := candidateValid
        fundingExact := rfl
      }
    else .error .candidateInvariantFailure
  else .error .notAdmissible

/-! ## Readiness, Open, terminal, redemption, and retirement -/

def admissionMatches
    (state : State) (admission : ExecutionRelease.Admission)
    (role : ExecutionRelease.Role) : Bool :=
  admission.marketReleaseSetId == state.identity.executionReleaseSetId &&
  admission.selected == state.executionReleases &&
  ExecutionRelease.admits admission role

structure FundActivation where
  resolutionAdmission : ExecutionRelease.Admission
  funding : SourceResolution.FundingState
  deriving DecidableEq, Repr

structure CollateralObservation where
  adapterAuthenticated : Bool
  realmId : Identity
  collateralMintId : Identity
  tokenProgramId : Identity
  collateralReleaseId : Identity
  deriving DecidableEq, Repr

def collateralMatches (realm : Realm) (observation : CollateralObservation) : Bool :=
  observation.adapterAuthenticated && observation.realmId == realm.realmId &&
  observation.collateralMintId == realm.collateralMintId &&
  observation.tokenProgramId == realm.tokenProgramId &&
  observation.collateralReleaseId == realm.collateralReleaseId

structure OpenFrame where
  custodyAdmission : ExecutionRelease.Admission
  collateral : CollateralObservation
  custody : VacantAccount
  deriving DecidableEq, Repr

structure TerminalFrame where
  resolutionAdmission : ExecutionRelease.Admission
  certificate : SourceResolution.Certificate
  deriving DecidableEq, Repr

structure EconomicFrame where
  claimsAdmission : ExecutionRelease.Admission
  custodyAdmission : ExecutionRelease.Admission
  bindings : Economic.Bindings
  command : Economic.Command
  deriving DecidableEq, Repr

structure RetirementFrame where
  coreAdmission : ExecutionRelease.Admission
  custodyAdmission : ExecutionRelease.Admission
  funding : SourceResolution.FundingState
  deriving DecidableEq, Repr

inductive Command where
  | activateFund (frame : FundActivation)
  | openMarket (frame : OpenFrame)
  | economic (frame : EconomicFrame)
  | admitTerminal (frame : TerminalFrame)
  | beginRetiring (coreAdmission : ExecutionRelease.Admission)
  | retire (frame : RetirementFrame)
  deriving DecidableEq, Repr

def emptyProgram : Direct.Physical.PhysicalPlan := {
  claimEffects := EffectPlan.mk []
  custodyTransfers := []
}

structure Candidate (pre : State) where
  post : State
  program : Direct.Physical.PhysicalPlan
  realmPreserved : post.realm = pre.realm
  productPreserved : post.product = pre.product
  identityPreserved : post.identity = pre.identity
  releasesPreserved : post.executionReleases = pre.executionReleases
  coordinatesPreserved : post.coordinates = pre.coordinates

def fundingConserved (funding : SourceResolution.FundingState) : Bool :=
  funding.initialCapital = funding.remainingCapital + funding.paidCapital

def activateFundCandidate
    (state : State) (frame : FundActivation) : Except Refusal (Candidate state) := do
  if state.phase != .founding || state.readiness != .prepaid then throw .wrongPhase
  if !admissionMatches state frame.resolutionAdmission .resolution then throw .wrongRelease
  if frame.funding.allocationId != state.funding.allocationId ||
      frame.funding.initialCapital != state.funding.initialWorkCapital ||
      frame.funding.remainingCapital != state.funding.initialWorkCapital ||
      frame.funding.paidCapital != 0 || frame.funding.callCount != 0 ||
      !fundingConserved frame.funding then throw .wrongFunding
  pure {
    post := { state with readiness := .ready }
    program := emptyProgram
    realmPreserved := rfl
    productPreserved := rfl
    identityPreserved := rfl
    releasesPreserved := rfl
    coordinatesPreserved := rfl
  }

def openCandidate (state : State) (frame : OpenFrame) : Except Refusal (Candidate state) := do
  if state.phase != .founding || state.readiness != .ready then throw .wrongPhase
  if !admissionMatches state frame.custodyAdmission .custody then throw .wrongRelease
  if !collateralMatches state.realm frame.collateral then throw .wrongRealmCollateral
  if !frame.custody.valid || frame.custody.address != state.coordinates.custodyId then
    throw .invalidAccount
  let reserved := state.capital.deferredCustodyRent
  let topUp := rentTopUp frame.custody.lamports reserved
  let unusedReserve := reserved - topUp
  let readinessRefund := state.capital.readinessRent + state.capital.readinessDonation
  let postCapital := {
    state.capital with
    readinessRent := 0
    readinessDonation := 0
    custodyRent := reserved
    custodyDonation := donationAboveRent frame.custody.lamports reserved
    deferredCustodyRent := 0
    rentCredit := state.capital.rentCredit + unusedReserve + readinessRefund
  }
  pure {
    post := { state with phase := .open, readiness := .consumed, capital := postCapital }
    program := emptyProgram
    realmPreserved := rfl
    productPreserved := rfl
    identityPreserved := rfl
    releasesPreserved := rfl
    coordinatesPreserved := rfl
  }

def economicCandidate
    (state : State) (frame : EconomicFrame) : Except Refusal (Candidate state) := do
  if !admissionMatches state frame.claimsAdmission .claims ||
      !admissionMatches state frame.custodyAdmission .custody then throw .wrongRelease
  if frame.command = .retireTerminal then throw .wrongPhase
  let economicFrame : Economic.Frame := {
    outcomeCount := state.product.outcomeCount
    scalarLimit := state.product.scalarLimit
    bindings := frame.bindings
    pre := state.economic
    command := frame.command
  }
  match Economic.execute? economicFrame with
  | .error _ => throw .economicRefusal
  | .ok settlement => pure {
      post := { state with economic := settlement.post }
      program := settlement.program
      realmPreserved := rfl
      productPreserved := rfl
      identityPreserved := rfl
      releasesPreserved := rfl
      coordinatesPreserved := rfl
    }

def certificateValid (certificate : SourceResolution.Certificate) : Bool :=
  certificate.marketId != 0 && certificate.sourceMaterialId != 0 &&
  certificate.productId != 0 && certificate.fundingAllocationId != 0 &&
  certificate.receiptAccountId != 0 && certificate.generation != 0 &&
  0 < certificate.workPaid &&
  match certificate.kind with
  | .resolutionSuccess =>
      certificate.routeId != 0 && certificate.providerEvidenceId != 0 &&
      0 < certificate.result.denominator
  | .recoveryAdvanced | .exhausted | .resolutionFailure =>
      certificate.providerEvidenceId = 0 && certificate.result = ⟨0, 0⟩

def terminalCertificateMatches (state : State) (certificate : SourceResolution.Certificate) : Bool :=
  certificateValid certificate &&
  (certificate.kind == .resolutionSuccess || certificate.kind == .resolutionFailure) &&
  certificate.marketId == state.identity.marketId &&
  certificate.sourceMaterialId == state.identity.resolutionPolicyId &&
  certificate.productId == state.product.productId &&
  certificate.generation == state.identity.generation &&
  certificate.fundingAllocationId == state.funding.allocationId &&
  certificate.fundingRemaining <= state.funding.initialWorkCapital &&
  certificate.selector < state.product.outcomeCount

def terminalCandidate
    (state : State) (frame : TerminalFrame) : Except Refusal (Candidate state) := do
  if state.phase != .open then throw .wrongPhase
  if !admissionMatches state frame.resolutionAdmission .resolution then throw .wrongRelease
  if !terminalCertificateMatches state frame.certificate then throw .invalidTerminalReceipt
  let winner := frame.certificate.selector
  pure {
    post := { state with
      phase := .terminal winner
      terminalReceiptId := frame.certificate.receiptAccountId
      terminalFundingRemaining := frame.certificate.fundingRemaining
      economic := { state.economic with phase := .terminal winner }
    }
    program := emptyProgram
    realmPreserved := rfl
    productPreserved := rfl
    identityPreserved := rfl
    releasesPreserved := rfl
    coordinatesPreserved := rfl
  }

def beginRetiringCandidate
    (state : State) (admission : ExecutionRelease.Admission) :
    Except Refusal (Candidate state) := do
  if !admissionMatches state admission .core then throw .wrongRelease
  match state.phase with
  | .terminal winner => pure {
      post := { state with
        phase := .retiring winner
        economic := { state.economic with phase := .retiring winner }
      }
      program := emptyProgram
      realmPreserved := rfl
      productPreserved := rfl
      identityPreserved := rfl
      releasesPreserved := rfl
      coordinatesPreserved := rfl
    }
  | _ => throw .wrongPhase

def retirementRefund (state : State) (funding : SourceResolution.FundingState) : Nat :=
  state.capital.marketRent + state.capital.marketDonation +
  state.capital.hoardRent + state.capital.hoardDonation +
  state.capital.fundRent + state.capital.fundDonation + funding.remainingCapital +
  state.capital.custodyRent + state.capital.custodyDonation

def clearedCapital (state : State) (refund : Nat) : Capital := {
  marketRent := 0
  marketDonation := 0
  hoardRent := 0
  hoardDonation := 0
  fundRent := 0
  fundDonation := 0
  readinessRent := 0
  readinessDonation := 0
  custodyRent := 0
  custodyDonation := 0
  deferredCustodyRent := 0
  rentCredit := state.capital.rentCredit + refund
}

def retireCandidate
    (state : State) (frame : RetirementFrame) : Except Refusal (Candidate state) := do
  if !admissionMatches state frame.coreAdmission .core ||
      !admissionMatches state frame.custodyAdmission .custody then throw .wrongRelease
  match state.phase with
  | .retiring _ => pure ()
  | _ => throw .wrongPhase
  if frame.funding.allocationId != state.funding.allocationId ||
      frame.funding.initialCapital != state.funding.initialWorkCapital ||
      frame.funding.remainingCapital != state.terminalFundingRemaining ||
      !fundingConserved frame.funding then throw .wrongFunding
  let economicFrame : Economic.Frame := {
    outcomeCount := state.product.outcomeCount
    scalarLimit := state.product.scalarLimit
    bindings := { source := .seller, destination := .buyer, hoard := .venue }
    pre := state.economic
    command := .retireTerminal
  }
  match Economic.execute? economicFrame with
  | .error _ => throw .economicRefusal
  | .ok settlement =>
      let refund := retirementRefund state frame.funding
      pure {
        post := { state with
          phase := .retired
          capital := clearedCapital state refund
          terminalFundingRemaining := 0
          economic := settlement.post
        }
        program := settlement.program
        realmPreserved := rfl
        productPreserved := rfl
        identityPreserved := rfl
        releasesPreserved := rfl
        coordinatesPreserved := rfl
      }

def candidateFor (state : State) : Command → Except Refusal (Candidate state)
  | .activateFund frame => activateFundCandidate state frame
  | .openMarket frame => openCandidate state frame
  | .economic frame => economicCandidate state frame
  | .admitTerminal frame => terminalCandidate state frame
  | .beginRetiring admission => beginRetiringCandidate state admission
  | .retire frame => retireCandidate state frame

structure Settlement (pre : State) (command : Command) where
  candidate : Candidate pre
  preValid : pre.valid = true
  postValid : candidate.post.valid = true
  exactCandidate : candidateFor pre command = .ok candidate

def Settlement.post {pre : State} {command : Command}
    (settlement : Settlement pre command) : State := settlement.candidate.post

def Settlement.program {pre : State} {command : Command}
    (settlement : Settlement pre command) : Direct.Physical.PhysicalPlan :=
  settlement.candidate.program

/-- Total transition boundary: every hostile input is either a complete valid
post-state/program or a checked refusal. -/
def step? (state : State) (command : Command) : Except Refusal (Settlement state command) :=
  if preValid : state.valid = true then
    match candidate : candidateFor state command with
    | .error refusal => .error refusal
    | .ok value =>
        if postValid : value.post.valid = true then
          .ok {
            candidate := value
            preValid
            postValid
            exactCandidate := candidate
          }
        else .error .candidateInvariantFailure
  else .error .invalidState

def runState (state : State) (command : Command) : State :=
  match step? state command with
  | .ok settlement => settlement.post
  | .error _ => state

theorem successful_step_is_valid
    (state : State) (command : Command) (settlement : Settlement state command) :
    settlement.post.valid = true := settlement.postValid

theorem refusal_rolls_back
    (state : State) (command : Command) (refusal : Refusal)
    (failed : step? state command = .error refusal) :
    runState state command = state := by
  unfold runState
  rw [failed]

theorem found_hoard_principal_is_zero (frame : FoundingFrame) :
    (initialState frame).economic.hoard = 0 := rfl

theorem found_hoard_account_has_no_semantic_principal (frame : FoundingFrame) :
    (foundingCreationPlan frame).hoard.semanticPrincipal = 0 := rfl

theorem found_work_capital_is_exact (frame : FoundingFrame) :
    (initialState frame).funding.initialWorkCapital = frame.quote.sourceWorkCapital := rfl

theorem found_fund_principal_is_exact (frame : FoundingFrame) :
    (foundingCreationPlan frame).fund.semanticPrincipal =
      frame.quote.sourceWorkCapital + frame.quote.custodyRent := rfl

theorem found_payer_debit_is_exact
    (frame : FoundingFrame)
    (funded : (foundingCreationPlan frame).payerDebit ≤ frame.accounts.payerLamports) :
    (foundingCreationPlan frame).payerAfter +
        (foundingCreationPlan frame).payerDebit = frame.accounts.payerLamports := by
  change frame.accounts.payerLamports - (foundingCreationPlan frame).payerDebit +
    (foundingCreationPlan frame).payerDebit = frame.accounts.payerLamports
  exact Nat.sub_add_cancel funded

theorem successful_step_preserves_immutable_coordinates
    (state : State) (command : Command) (settlement : Settlement state command) :
    settlement.post.realm = state.realm ∧ settlement.post.product = state.product ∧
    settlement.post.identity = state.identity ∧
    settlement.post.executionReleases = state.executionReleases ∧
    settlement.post.coordinates = state.coordinates := by
  exact ⟨settlement.candidate.realmPreserved, settlement.candidate.productPreserved,
    settlement.candidate.identityPreserved, settlement.candidate.releasesPreserved,
    settlement.candidate.coordinatesPreserved⟩

end DClutch.MarketCore
