import DClutchSemantics.ExecutionRelease
import DClutchSemantics.SourceResolution
import Std.Tactic

/-!
# Universal Market Core and funded founding lifecycle

This module is the small lifecycle shared by every dClutch Market.  It owns an
immutable Realm, canonical Product-record/stable-Product identities, one immutable
execution-release set, exact prepaid creation, universal lifecycle,
terminal admission, redemption routing, and retirement.  The Claims child is
the sole owner of mutable claim supply and Hoard principal; Core stores only
immutable coordinates and lifecycle. Source/Resolution is the sole
owner of action-specific funding allocations, work, balances, and closure.
Trading venues,
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

/-- Canonical runtime-width Product projection selected before Found. The
Product record digest is the authority root; all child coordinates are
authenticated derivations rather than parallel persisted Core facts. -/
structure Product where
  productRecordId : Identity
  productId : Identity
  resultDomainId : Identity
  portfolioId : Identity
  coordinateDomainId : Identity
  resultUnitId : Identity
  claimBasisId : Identity
  liabilityBasisId : Identity
  representationReleaseId : Identity
  mappingReleaseId : Identity
  outcomeCount : Nat
  deriving DecidableEq, Repr

def Product.valid (product : Product) : Bool :=
  product.productRecordId != 0 && product.productId != 0 &&
  product.resultDomainId != 0 && product.portfolioId != 0 &&
  product.coordinateDomainId != 0 && product.resultUnitId != 0 &&
  product.claimBasisId != 0 && product.liabilityBasisId != 0 &&
  product.representationReleaseId != 0 && product.mappingReleaseId != 0 &&
  1 < product.outcomeCount

/-- Exact immutable Market identity.  `marketId` is the adapter-checked
canonical content/address identity for these coordinates. -/
structure MarketIdentity where
  marketId : Identity
  realmId : Identity
  productRecordId : Identity
  productId : Identity
  resolutionPolicyId : Identity
  capabilityManifestId : Identity
  executionReleaseSetId : Identity
  registryProgramId : Identity
  generation : Nat
  deriving DecidableEq, Repr

def MarketIdentity.valid (identity : MarketIdentity) : Bool :=
  identity.marketId != 0 && identity.realmId != 0 &&
  identity.productRecordId != 0 && identity.productId != 0 &&
  identity.resolutionPolicyId != 0 && identity.capabilityManifestId != 0 &&
  identity.executionReleaseSetId != 0 && identity.registryProgramId != 0

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

/-- Exact prepaid Core-owned Market account requirement. Capability funding,
Hoard initialization, readiness, and custody are child-owned effects. -/
structure FoundingQuote where
  marketRent : Nat
  deriving DecidableEq, Repr

def FoundingQuote.valid (quote : FoundingQuote) : Bool :=
  0 < quote.marketRent

structure FoundingAccounts where
  payerLamports : Nat
  rentCreditId : Identity
  market : VacantAccount
  deriving DecidableEq, Repr

structure FoundingFrame where
  realm : Realm
  product : Product
  identity : MarketIdentity
  coreAdmission : ExecutionRelease.Admission
  quote : FoundingQuote
  accounts : FoundingAccounts
  deriving DecidableEq, Repr

structure FoundingCreationPlan where
  market : AccountCreation
  payerDebit : Nat
  payerAfter : Nat
  deriving DecidableEq, Repr

def foundingCreationPlan (frame : FoundingFrame) : FoundingCreationPlan :=
  let market := planAccountCreation frame.accounts.market frame.quote.marketRent 0
  {
    market
    payerDebit := market.rentTopUp
    payerAfter := frame.accounts.payerLamports - market.rentTopUp
  }

def foundingAccepts (frame : FoundingFrame) : Bool :=
  let plan := foundingCreationPlan frame
  frame.realm.valid && frame.product.valid && frame.identity.valid &&
  frame.identity.realmId == frame.realm.realmId &&
  frame.identity.productRecordId == frame.product.productRecordId &&
  frame.identity.productId == frame.product.productId &&
  frame.identity.executionReleaseSetId == frame.coreAdmission.selected.releaseSetId &&
  frame.identity.registryProgramId == frame.coreAdmission.marketRegistryProgram &&
  frame.coreAdmission.marketReleaseSetId == frame.identity.executionReleaseSetId &&
  ExecutionRelease.admits frame.coreAdmission .core &&
  frame.quote.valid &&
  frame.accounts.rentCreditId != 0 && frame.accounts.market.valid &&
  frame.accounts.market.address == frame.identity.marketId &&
  frame.accounts.market.address != frame.accounts.rentCreditId &&
  plan.payerDebit <= frame.accounts.payerLamports

/-! ## Recurring-Series one-shot founding capability -/

/-- Immutable semantic body of the Core-owned permit consumed by Claims V5.
The two Claims digests are stored in the body rather than the PDA seeds, which
keeps the address acyclic while binding the exact future request. -/
structure SeriesFoundingPermit where
  releaseSetId : Identity
  marketId : Identity
  productRecordId : Identity
  sourceId : Identity
  founderId : Identity
  ticketContextId : Identity
  parentRootId : Identity
  projectedReplayId : Identity
  fundingSourceId : Identity
  hoardId : Identity
  projectedRequestDigest : Identity
  projectedReceiptDigest : Identity
  claimsIntentDigest : Identity
  claimsRequestDigest : Identity
  tradingProgramId : Identity
  claimsProgramId : Identity
  rentCreditId : Identity
  generation : Nat
  quantity : Nat
  basisScale : Nat
  expirySlot : Nat
  projectedResultingRevision : Nat
  normalReplayRevision : Nat
  deriving DecidableEq, Repr

def SeriesFoundingPermit.valid (permit : SeriesFoundingPermit) : Bool :=
  [permit.releaseSetId, permit.marketId, permit.productRecordId, permit.sourceId,
    permit.founderId, permit.ticketContextId, permit.parentRootId,
    permit.projectedReplayId, permit.fundingSourceId, permit.hoardId,
    permit.projectedRequestDigest, permit.projectedReceiptDigest,
    permit.claimsIntentDigest, permit.claimsRequestDigest,
    permit.tradingProgramId, permit.claimsProgramId, permit.rentCreditId].all (· != 0) &&
  permit.marketId != permit.fundingSourceId && permit.marketId != permit.hoardId &&
  permit.marketId != permit.projectedReplayId &&
  permit.fundingSourceId != permit.hoardId && permit.projectedReplayId != permit.hoardId &&
  permit.tradingProgramId != permit.claimsProgramId &&
  permit.claimsIntentDigest != permit.claimsRequestDigest &&
  0 < permit.generation && 0 < permit.quantity && 0 < permit.basisScale &&
  0 < permit.expirySlot && 0 < permit.projectedResultingRevision &&
  permit.normalReplayRevision = 1

/-- Chain observations for the exact permit PDA prepaid during Trading Prepare.
The current Rent minimum is debited from one selected `capability_native`
FundingState. Pre-existing lamports remain an explicit donation and never
become semantic capability funding. -/
structure SeriesPermitFundingObservation where
  account : VacantAccount
  canonicalPdaAuthenticated : Bool
  typedCapabilityNativeEntryAuthenticated : Bool
  tradingPreparedCommitAuthenticated : Bool
  preexistingLamports : Nat
  fundingDebit : Nat
  currentRentMinimum : Nat
  currentSlot : Nat
  deriving DecidableEq, Repr

def seriesPermitFundingAccepts
    (permit : SeriesFoundingPermit) (funding : SeriesPermitFundingObservation) : Bool :=
  permit.valid && funding.account.valid && funding.canonicalPdaAuthenticated &&
  funding.typedCapabilityNativeEntryAuthenticated &&
  funding.tradingPreparedCommitAuthenticated &&
  0 < funding.currentRentMinimum && funding.fundingDebit = funding.currentRentMinimum &&
  funding.account.lamports = funding.preexistingLamports + funding.fundingDebit &&
  funding.currentRentMinimum ≤ funding.account.lamports &&
  funding.currentSlot ≤ permit.expirySlot

/-- Allocation/assignment never debits Core or Trading during Consume: Prepare
has already prefunded the canonical still-System-owned zero-data PDA. -/
def seriesPermitCreationPlan
    (funding : SeriesPermitFundingObservation) : AccountCreation :=
  planAccountCreation funding.account funding.currentRentMinimum 0

theorem accepted_series_permit_needs_no_consume_topup
    (permit : SeriesFoundingPermit) (funding : SeriesPermitFundingObservation)
    (_accepted : seriesPermitFundingAccepts permit funding = true)
    (prefunded : funding.currentRentMinimum ≤ funding.account.lamports) :
    (seriesPermitCreationPlan funding).rentTopUp = 0 := by
  simp [seriesPermitCreationPlan, planAccountCreation, rentTopUp,
    Nat.sub_eq_zero_of_le prefunded]

/-- Exact Found-time join. Core allocates and writes the permit only after the
ordinary Market Found plan and the immediate projected Lock receipt both
authenticate. -/
structure SeriesPermitIssueFrame where
  found : FoundingFrame
  permit : SeriesFoundingPermit
  funding : SeriesPermitFundingObservation
  lockReceiptAuthenticated : Bool
  intentDigestAuthenticated : Bool
  claimsRequestDigestAuthenticated : Bool
  deriving DecidableEq, Repr

def seriesPermitIssueAccepts (frame : SeriesPermitIssueFrame) : Bool :=
  foundingAccepts frame.found && seriesPermitFundingAccepts frame.permit frame.funding &&
  frame.permit.releaseSetId == frame.found.identity.executionReleaseSetId &&
  frame.permit.marketId == frame.found.identity.marketId &&
  frame.permit.productRecordId == frame.found.identity.productRecordId &&
  frame.permit.sourceId == frame.found.identity.resolutionPolicyId &&
  frame.permit.generation == frame.found.identity.generation &&
  frame.permit.rentCreditId == frame.found.accounts.rentCreditId &&
  frame.permit.tradingProgramId == frame.found.coreAdmission.selected.trading.program &&
  frame.permit.claimsProgramId == frame.found.coreAdmission.selected.claims.program &&
  frame.funding.account.address != frame.found.identity.marketId &&
  frame.funding.account.address != frame.found.accounts.rentCreditId &&
  frame.lockReceiptAuthenticated && frame.intentDigestAuthenticated &&
  frame.claimsRequestDigestAuthenticated

theorem substituted_series_claims_program_refuses
    (frame : SeriesPermitIssueFrame)
    (substituted : frame.permit.claimsProgramId ≠ frame.found.coreAdmission.selected.claims.program) :
    seriesPermitIssueAccepts frame = false := by
  simp [seriesPermitIssueAccepts, substituted]

/-- Final Open evidence consumes the live Claims state and permit exactly once,
then returns every permit lamport (including donation) to immutable RentCredit. -/
structure SeriesPermitConsumeObservation where
  ownerCoreAuthenticated : Bool
  exactBodyAuthenticated : Bool
  claimsIntentAuthenticated : Bool
  claimsRequestAuthenticated : Bool
  claimsReceiptAuthenticated : Bool
  claimsPoststateAuthenticated : Bool
  normalReplayAuthenticated : Bool
  permitLamports : Nat
  rentCreditAuthenticated : Bool
  currentSlot : Nat
  deriving DecidableEq, Repr

def seriesPermitConsumeAccepts
    (permit : SeriesFoundingPermit) (observation : SeriesPermitConsumeObservation) : Bool :=
  permit.valid && observation.ownerCoreAuthenticated && observation.exactBodyAuthenticated &&
  observation.claimsIntentAuthenticated && observation.claimsRequestAuthenticated &&
  observation.claimsReceiptAuthenticated && observation.claimsPoststateAuthenticated &&
  observation.normalReplayAuthenticated && observation.rentCreditAuthenticated &&
  0 < observation.permitLamports && observation.currentSlot ≤ permit.expirySlot

def seriesPermitCloseRefund (observation : SeriesPermitConsumeObservation) : Nat :=
  observation.permitLamports

/-- Permissionless expiry cleans up only the still-unallocated canonical PDA
after an authenticated expired Ticket candidate. -/
structure SeriesPermitExpiryObservation where
  account : VacantAccount
  canonicalPdaAuthenticated : Bool
  expiredTicketCandidateAuthenticated : Bool
  rentCreditAuthenticated : Bool
  currentSlot : Nat
  deriving DecidableEq, Repr

def seriesPermitExpiryAccepts
    (permit : SeriesFoundingPermit) (observation : SeriesPermitExpiryObservation) : Bool :=
  permit.valid && observation.account.valid && observation.canonicalPdaAuthenticated &&
  observation.expiredTicketCandidateAuthenticated && observation.rentCreditAuthenticated &&
  permit.expirySlot < observation.currentSlot

def seriesPermitExpiryRefund (observation : SeriesPermitExpiryObservation) : Nat :=
  observation.account.lamports

theorem unexpired_series_permit_refund_refuses
    (permit : SeriesFoundingPermit) (observation : SeriesPermitExpiryObservation)
    (unexpired : observation.currentSlot ≤ permit.expirySlot) :
    seriesPermitExpiryAccepts permit observation = false := by
  simp [seriesPermitExpiryAccepts, Nat.not_lt_of_ge unexpired]

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

structure State where
  identity : MarketIdentity
  rentBeneficiaryId : Identity
  phase : Phase
  readiness : Readiness
  outstandingCapabilities : Nat
  terminalReceiptId : Identity
  deriving DecidableEq, Repr

def lifecyclePhaseValid (state : State) : Bool :=
  match state.phase with
  | .founding => state.readiness != .consumed
  | .open | .terminal _ | .retiring _ | .retired => state.readiness = .consumed

def terminalFieldsValid (state : State) : Bool :=
  match state.phase with
  | .founding | .open => state.terminalReceiptId = 0
  | .terminal _ | .retiring _ => state.terminalReceiptId != 0
  | .retired => state.terminalReceiptId != 0

def childCountPhaseValid (state : State) : Bool :=
  match state.phase with
  | .retired => state.outstandingCapabilities = 0
  | _ => true

/-- Complete executable invariant. -/
def State.valid (state : State) : Bool :=
  state.identity.valid && state.rentBeneficiaryId != 0 &&
  lifecyclePhaseValid state && terminalFieldsValid state && childCountPhaseValid state

def initialState (frame : FoundingFrame) : State :=
  {
    identity := frame.identity
    rentBeneficiaryId := frame.accounts.rentCreditId
    phase := .founding
    readiness := .prepaid
    outstandingCapabilities := 0
    terminalReceiptId := 0
  }

structure FoundingResult (frame : FoundingFrame) where
  post : State
  creation : FoundingCreationPlan
  accepted : foundingAccepts frame = true
  postExact : post = initialState frame
  postValid : post.valid = true

inductive Refusal where
  | notAdmissible
  | invalidState
  | wrongRelease
  | wrongPhase
  | wrongRealmCollateral
  | invalidTerminalReceipt
  | invalidAccount
  | childEffectRefusal
  | candidateInvariantFailure
  deriving DecidableEq, Repr

/-- Total Found boundary. -/
def found? (frame : FoundingFrame) : Except Refusal (FoundingResult frame) :=
  if accepted : foundingAccepts frame = true then
    let candidate := initialState frame
    if candidateValid : candidate.valid = true then
      .ok {
        post := candidate
        creation := foundingCreationPlan frame
        accepted
        postExact := rfl
        postValid := candidateValid
      }
    else .error .candidateInvariantFailure
  else .error .notAdmissible

/-! ## Readiness, Open, terminal, redemption, and retirement -/

def admissionMatches
    (state : State) (admission : ExecutionRelease.Admission)
    (role : ExecutionRelease.Role) : Bool :=
  admission.marketReleaseSetId == state.identity.executionReleaseSetId &&
  admission.marketRegistryProgram == state.identity.registryProgramId &&
  admission.selected.releaseSetId == state.identity.executionReleaseSetId &&
  ExecutionRelease.admits admission role

/-! Child programs own their request/receipt schemas. Core consumes only a
derived trust-boundary observation; callers cannot author these booleans at the
physical boundary because the adapter derives them from same-call CPI return
data and authenticated post-accounts. -/
structure ChildEffectObservation where
  exactRequestAuthenticated : Bool
  exactReceiptAuthenticated : Bool
  postResourceAuthenticated : Bool
  deriving DecidableEq, Repr

def ChildEffectObservation.complete (observation : ChildEffectObservation) : Bool :=
  observation.exactRequestAuthenticated && observation.exactReceiptAuthenticated &&
  observation.postResourceAuthenticated

structure ReadinessVerification where
  coreAdmission : ExecutionRelease.Admission
  manifestReadinessAuthenticated : Bool
  readinessEffect : ChildEffectObservation
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

structure ClaimsEffectObservation extends ChildEffectObservation where
  payout : Nat
  aggregateEmpty : Bool
  deriving DecidableEq, Repr

/-- Exact generic optional-child effect. Manifest/Funding ownership remains in
the capability child; Core stores only the outstanding-child replay count. -/
structure CapabilityChildFrame where
  childAdmission : ExecutionRelease.Admission
  targetRole : ExecutionRelease.Role
  manifestEntryAuthenticated : Bool
  fundingStateAuthenticated : Bool
  effect : ChildEffectObservation
  deriving DecidableEq, Repr

structure OpenFrame where
  custodyAdmission : ExecutionRelease.Admission
  realm : Realm
  realmRecordAuthenticated : Bool
  custodyDerivationAuthenticated : Bool
  collateral : CollateralObservation
  custody : VacantAccount
  custodyRentMinimum : Nat
  custodyRentAuthenticated : Bool
  custodyEffect : ChildEffectObservation
  deriving DecidableEq, Repr

/-- Exact ephemeral Custody creation plan. The physical adapter derives the
rent minimum from trusted Rent data and authenticates the same child request;
Core does not persist a mirror of the resulting account balance. -/
def openCreationPlan (frame : OpenFrame) : AccountCreation :=
  planAccountCreation frame.custody frame.custodyRentMinimum 0

/-- Atomic recurring-Series opening consumes a Core Founding/Prepaid Market,
an already-funded projected Hoard that Custody has realized into the normal
replay, an exact Claims founding receipt, and the Trading-owned Prepared to
Consumed replay candidate. The adapter derives every Boolean observation from
producer-bound receipts and independently authenticated post-accounts. -/
structure SeriesOpenFrame where
  claimsAdmission : ExecutionRelease.Admission
  custodyAdmission : ExecutionRelease.Admission
  quantity : Nat
  basisScale : Nat
  sourceDebit : Nat
  hoardCredit : Nat
  hoardFundingAuthenticated : Bool
  foundStateBoundByCustody : Bool
  claimsCustodyJoinAuthenticated : Bool
  ticketPreparedAuthenticated : Bool
  ticketConsumedCandidateAuthenticated : Bool
  claimsEffect : ChildEffectObservation
  custodyEffect : ChildEffectObservation
  deriving DecidableEq, Repr

structure TerminalFrame where
  resolutionAdmission : ExecutionRelease.Admission
  product : Product
  productRecordAuthenticated : Bool
  certificate : SourceResolution.Certificate
  deriving DecidableEq, Repr

structure SplitFrame where
  claimsAdmission : ExecutionRelease.Admission
  custodyAdmission : ExecutionRelease.Admission
  quantity : Nat
  claims : ClaimsEffectObservation
  custody : ChildEffectObservation
  deriving DecidableEq, Repr

structure RedemptionFrame where
  claimsAdmission : ExecutionRelease.Admission
  custodyAdmission : ExecutionRelease.Admission
  product : Product
  productRecordAuthenticated : Bool
  outcome : Nat
  quantity : Nat
  claims : ClaimsEffectObservation
  custody : Option ChildEffectObservation
  deriving DecidableEq, Repr

/-! Retirement uses one coherent release-set observation.  Physical adapters
may authenticate its four required roles serially, but they cannot substitute
role-specific copies of the selected ReleaseSet. -/
structure RetirementAdmissions where
  coreAdmission : ExecutionRelease.Admission
  claimsAdmission : ExecutionRelease.Admission
  resolutionAdmission : ExecutionRelease.Admission
  custodyAdmission : ExecutionRelease.Admission
  deriving DecidableEq, Repr

def RetirementAdmissions.valid (state : State) (admissions : RetirementAdmissions) : Bool :=
  admissionMatches state admissions.coreAdmission .core &&
  admissionMatches state admissions.claimsAdmission .claims &&
  admissionMatches state admissions.resolutionAdmission .resolution &&
  admissionMatches state admissions.custodyAdmission .custody &&
  admissions.coreAdmission.selected == admissions.claimsAdmission.selected &&
  admissions.coreAdmission.selected == admissions.resolutionAdmission.selected &&
  admissions.coreAdmission.selected == admissions.custodyAdmission.selected

structure RetirementFrame where
  admissions : RetirementAdmissions
  claims : ClaimsEffectObservation
  source : ChildEffectObservation
  custody : ChildEffectObservation
  coreAccountLamports : Nat
  coreAccountAuthenticated : Bool
  rentCreditAuthenticated : Bool
  deriving DecidableEq, Repr

inductive Command where
  | verifyReadiness (frame : ReadinessVerification)
  | openMarket (frame : OpenFrame)
  | openSeriesMarket (frame : SeriesOpenFrame)
  | split (frame : SplitFrame)
  | redeem (frame : RedemptionFrame)
  | admitTerminal (frame : TerminalFrame)
  | beginRetiring (coreAdmission : ExecutionRelease.Admission)
  | activateCapability (frame : CapabilityChildFrame)
  | closeCapability (frame : CapabilityChildFrame)
  | retire (frame : RetirementFrame)
  deriving DecidableEq, Repr

structure Candidate (pre : State) where
  post : State
  payout : Nat
  identityPreserved : post.identity = pre.identity
  rentBeneficiaryPreserved : post.rentBeneficiaryId = pre.rentBeneficiaryId

def verifyReadinessCandidate
    (state : State) (frame : ReadinessVerification) : Except Refusal (Candidate state) := do
  if state.phase != .founding || state.readiness != .prepaid then throw .wrongPhase
  if !admissionMatches state frame.coreAdmission .core then throw .wrongRelease
  if !frame.manifestReadinessAuthenticated || !frame.readinessEffect.complete then
    throw .childEffectRefusal
  pure {
    post := { state with readiness := .ready }
    payout := 0
    identityPreserved := rfl
    rentBeneficiaryPreserved := rfl
  }

def openCandidate (state : State) (frame : OpenFrame) : Except Refusal (Candidate state) := do
  if state.phase != .founding || state.readiness != .ready then throw .wrongPhase
  if !admissionMatches state frame.custodyAdmission .custody then throw .wrongRelease
  if !frame.realmRecordAuthenticated || !frame.realm.valid ||
      frame.realm.realmId != state.identity.realmId ||
      !collateralMatches frame.realm frame.collateral then throw .wrongRealmCollateral
  if !frame.custodyDerivationAuthenticated then throw .invalidAccount
  if !frame.custodyEffect.complete || !frame.custodyRentAuthenticated ||
      frame.custodyRentMinimum = 0 then throw .childEffectRefusal
  if !frame.custody.valid || frame.custody.address == state.identity.marketId ||
      frame.custody.address == state.rentBeneficiaryId then
    throw .invalidAccount
  pure {
    post := { state with phase := .open, readiness := .consumed }
    payout := 0
    identityPreserved := rfl
    rentBeneficiaryPreserved := rfl
  }

def seriesOpenCandidate
    (state : State) (frame : SeriesOpenFrame) : Except Refusal (Candidate state) := do
  if state.phase != .founding || state.readiness != .prepaid then throw .wrongPhase
  if !admissionMatches state frame.claimsAdmission .claims ||
      !admissionMatches state frame.custodyAdmission .custody then throw .wrongRelease
  let collateral := frame.quantity * frame.basisScale
  if frame.quantity = 0 || frame.basisScale = 0 || collateral = 0 ||
      frame.sourceDebit != collateral || frame.hoardCredit != collateral ||
      !frame.hoardFundingAuthenticated || !frame.foundStateBoundByCustody ||
      !frame.claimsCustodyJoinAuthenticated || !frame.ticketPreparedAuthenticated ||
      !frame.ticketConsumedCandidateAuthenticated || !frame.claimsEffect.complete ||
      !frame.custodyEffect.complete then throw .childEffectRefusal
  pure {
    post := { state with phase := .open, readiness := .consumed }
    payout := 0
    identityPreserved := rfl
    rentBeneficiaryPreserved := rfl
  }

def splitCandidate
    (state : State) (frame : SplitFrame) : Except Refusal (Candidate state) := do
  if state.phase != .open then throw .wrongPhase
  if !admissionMatches state frame.claimsAdmission .claims ||
      !admissionMatches state frame.custodyAdmission .custody then throw .wrongRelease
  if frame.quantity = 0 || !frame.claims.toChildEffectObservation.complete ||
      !frame.custody.complete || frame.claims.payout != 0 || frame.claims.aggregateEmpty then
    throw .childEffectRefusal
  pure {
    post := state
    payout := 0
    identityPreserved := rfl
    rentBeneficiaryPreserved := rfl
  }

def terminalWinner? (state : State) : Option Nat :=
  match state.phase with
  | .terminal winner | .retiring winner => some winner
  | _ => none

def redemptionCandidate
    (state : State) (frame : RedemptionFrame) : Except Refusal (Candidate state) := do
  let some winner := terminalWinner? state | throw .wrongPhase
  if !admissionMatches state frame.claimsAdmission .claims ||
      !admissionMatches state frame.custodyAdmission .custody then throw .wrongRelease
  if !frame.productRecordAuthenticated || !frame.product.valid ||
      frame.product.productRecordId != state.identity.productRecordId ||
      frame.product.productId != state.identity.productId ||
      frame.quantity = 0 || frame.product.outcomeCount ≤ frame.outcome ||
      !frame.claims.toChildEffectObservation.complete then
    throw .childEffectRefusal
  let payout := if frame.outcome = winner then frame.quantity else 0
  let custodyMatches := if payout = 0 then frame.custody.isNone
    else frame.custody.any ChildEffectObservation.complete
  if frame.claims.payout != payout || !custodyMatches then throw .childEffectRefusal
  pure {
    post := state
    payout
    identityPreserved := rfl
    rentBeneficiaryPreserved := rfl
  }

def certificateValid (certificate : SourceResolution.Certificate) : Bool :=
  certificate.marketId != 0 && certificate.sourceMaterialId != 0 &&
  certificate.productId != 0 && certificate.receiptAccountId != 0 &&
  certificate.generation != 0 &&
  match certificate.kind with
  | .resolutionSuccess =>
      certificate.routeId != 0 && certificate.providerEvidenceId != 0 &&
      0 < certificate.result.denominator
  | .recoveryAdvanced | .exhausted | .resolutionFailure =>
      certificate.providerEvidenceId = 0 && certificate.result = ⟨0, 0⟩

def terminalCertificateMatches
    (state : State) (product : Product) (productRecordAuthenticated : Bool)
    (certificate : SourceResolution.Certificate) : Bool :=
  productRecordAuthenticated && product.valid &&
  product.productRecordId == state.identity.productRecordId &&
  product.productId == state.identity.productId &&
  certificateValid certificate &&
  (certificate.kind == .resolutionSuccess || certificate.kind == .resolutionFailure) &&
  certificate.marketId == state.identity.marketId &&
  certificate.sourceMaterialId == state.identity.resolutionPolicyId &&
  certificate.productId == product.productId &&
  certificate.generation == state.identity.generation &&
  certificate.selector < product.outcomeCount

def terminalCandidate
    (state : State) (frame : TerminalFrame) : Except Refusal (Candidate state) := do
  if state.phase != .open then throw .wrongPhase
  if !admissionMatches state frame.resolutionAdmission .resolution then throw .wrongRelease
  if !terminalCertificateMatches state frame.product frame.productRecordAuthenticated
      frame.certificate then throw .invalidTerminalReceipt
  let winner := frame.certificate.selector
  pure {
    post := { state with
      phase := .terminal winner
      terminalReceiptId := frame.certificate.receiptAccountId
    }
    payout := 0
    identityPreserved := rfl
    rentBeneficiaryPreserved := rfl
  }

def beginRetiringCandidate
    (state : State) (admission : ExecutionRelease.Admission) :
    Except Refusal (Candidate state) := do
  if !admissionMatches state admission .core then throw .wrongRelease
  match state.phase with
  | .terminal winner => pure {
      post := { state with phase := .retiring winner }
      payout := 0
      identityPreserved := rfl
      rentBeneficiaryPreserved := rfl
    }
  | _ => throw .wrongPhase

def capabilityFrameValid (state : State) (frame : CapabilityChildFrame) : Bool :=
  frame.targetRole != .core && admissionMatches state frame.childAdmission frame.targetRole &&
  frame.manifestEntryAuthenticated && frame.fundingStateAuthenticated && frame.effect.complete

def activateCapabilityCandidate
    (state : State) (frame : CapabilityChildFrame) : Except Refusal (Candidate state) := do
  if state.phase != .open then throw .wrongPhase
  if !capabilityFrameValid state frame then throw .childEffectRefusal
  pure {
    post := { state with outstandingCapabilities := state.outstandingCapabilities + 1 }
    payout := 0
    identityPreserved := rfl
    rentBeneficiaryPreserved := rfl
  }

def closeCapabilityCandidate
    (state : State) (frame : CapabilityChildFrame) : Except Refusal (Candidate state) := do
  match state.phase with
  | .open | .terminal _ | .retiring _ => pure ()
  | _ => throw .wrongPhase
  if state.outstandingCapabilities = 0 || !capabilityFrameValid state frame then
    throw .childEffectRefusal
  pure {
    post := { state with outstandingCapabilities := state.outstandingCapabilities - 1 }
    payout := 0
    identityPreserved := rfl
    rentBeneficiaryPreserved := rfl
  }

def retireCandidate
    (state : State) (frame : RetirementFrame) : Except Refusal (Candidate state) := do
  if !frame.admissions.valid state then throw .wrongRelease
  match state.phase with
  | .retiring _ => pure ()
  | _ => throw .wrongPhase
  if state.outstandingCapabilities != 0 then throw .childEffectRefusal
  if !frame.claims.toChildEffectObservation.complete || !frame.claims.aggregateEmpty ||
      frame.claims.payout != 0 || !frame.source.complete || !frame.custody.complete then
    throw .childEffectRefusal
  if !frame.coreAccountAuthenticated || !frame.rentCreditAuthenticated then
    throw .invalidAccount
  pure {
    post := { state with phase := .retired }
    payout := frame.coreAccountLamports
    identityPreserved := rfl
    rentBeneficiaryPreserved := rfl
  }

def candidateFor (state : State) : Command → Except Refusal (Candidate state)
  | .verifyReadiness frame => verifyReadinessCandidate state frame
  | .openMarket frame => openCandidate state frame
  | .openSeriesMarket frame => seriesOpenCandidate state frame
  | .split frame => splitCandidate state frame
  | .redeem frame => redemptionCandidate state frame
  | .admitTerminal frame => terminalCandidate state frame
  | .beginRetiring admission => beginRetiringCandidate state admission
  | .activateCapability frame => activateCapabilityCandidate state frame
  | .closeCapability frame => closeCapabilityCandidate state frame
  | .retire frame => retireCandidate state frame

structure Settlement (pre : State) (command : Command) where
  candidate : Candidate pre
  preValid : pre.valid = true
  postValid : candidate.post.valid = true
  exactCandidate : candidateFor pre command = .ok candidate

def Settlement.post {pre : State} {command : Command}
    (settlement : Settlement pre command) : State := settlement.candidate.post

def Settlement.payout {pre : State} {command : Command}
    (settlement : Settlement pre command) : Nat := settlement.candidate.payout

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

theorem found_payer_debit_is_exact
    (frame : FoundingFrame)
    (funded : (foundingCreationPlan frame).payerDebit ≤ frame.accounts.payerLamports) :
    (foundingCreationPlan frame).payerAfter +
        (foundingCreationPlan frame).payerDebit = frame.accounts.payerLamports := by
  change frame.accounts.payerLamports - (foundingCreationPlan frame).payerDebit +
    (foundingCreationPlan frame).payerDebit = frame.accounts.payerLamports
  exact Nat.sub_add_cancel funded

theorem successful_step_preserves_immutable_identity
    (state : State) (command : Command) (settlement : Settlement state command) :
    settlement.post.identity = state.identity ∧
    settlement.post.rentBeneficiaryId = state.rentBeneficiaryId := by
  exact ⟨settlement.candidate.identityPreserved,
    settlement.candidate.rentBeneficiaryPreserved⟩

end DClutch.MarketCore
