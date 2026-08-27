import DClutchSemantics.IR

/-!
# Source and resolution semantic specialization

This module is a fresh Lean model of the provider-neutral Source successor.  It
ends at two explicit boundaries:

* a provider adapter supplies `NormalizedEvidence` only after authenticating
  CPI, accounts, Program/ProgramData, parser rules, and Clock; and
* a shared executor applies the emitted first-order effects atomically to the
  authenticated accounts named by its frame.

Neither boundary is proved here.  Inside the boundary, release substitution,
stale evidence, skipped recovery, early failure, underfunded work, and malformed
Product partitions are checked refusals.
-/

namespace DClutch.SourceResolution

/-! ## Exact Product-owned result domain -/

/-- Exact signed rational.  Denominator positivity is a checked input property,
not a proof-only constructor premise. -/
structure Rational where
  numerator : Int
  denominator : Nat
  deriving DecidableEq, Repr

def Rational.Valid (value : Rational) : Prop := 0 < value.denominator

/-- Mathematical exact comparison.  Lean's integers are unbounded; a physical
fixed-width evaluator must independently validate its overflow-free refinement. -/
def Rational.lt (left right : Rational) : Bool :=
  left.numerator * (right.denominator : Int) <
    right.numerator * (left.denominator : Int)

/-- Result kind is a sum.  Failure is therefore part of the Product result
domain, not a magic ordinary selector supplied by Source. -/
inductive Result where
  | observed (value : Rational)
  | failure
  deriving DecidableEq, Repr

/-- Canonical finite partition of the exact rational line.

For `cuts = [c₀, …, cₙ₋₁]`, region zero is below `c₀`, region `i`
is bounded by the adjacent cuts, and region `n` is at or above the final cut.
The derived failure selector is `n + 1`.  Product is the sole owner of this
ordering. -/
structure ProductDomain where
  productId : Nat
  coordinateDomainId : Nat
  resultUnitId : Nat
  releaseId : Nat
  cutDenominator : Nat
  cuts : List Int
  deriving DecidableEq, Repr

def ProductDomain.Valid (domain : ProductDomain) : Prop :=
  domain.productId ≠ 0 ∧ domain.coordinateDomainId ≠ 0 ∧
  domain.resultUnitId ≠ 0 ∧ domain.releaseId ≠ 0 ∧
  0 < domain.cutDenominator ∧ domain.cuts.Pairwise (.<.)

def strictlyIncreasing : List Int → Bool
  | [] | [_] => true
  | left :: right :: rest => left < right && strictlyIncreasing (right :: rest)

def ProductDomain.valid (domain : ProductDomain) : Bool :=
  domain.productId != 0 && domain.coordinateDomainId != 0 &&
  domain.resultUnitId != 0 && domain.releaseId != 0 &&
  domain.cutDenominator != 0 && strictlyIncreasing domain.cuts

def ProductDomain.regionCount (domain : ProductDomain) : Nat :=
  domain.cuts.length + 1

def ProductDomain.outcomeCount (domain : ProductDomain) : Nat :=
  domain.regionCount + 1

def ProductDomain.failureSelector (domain : ProductDomain) : Nat :=
  domain.regionCount

/-- Locate the first strict upper cut.  Equality advances to the region on the
right, fixing boundary orientation canonically. -/
def ProductDomain.ordinarySelectorFrom
    (value : Rational) (cutDenominator : Nat) : List Int → Nat
  | [] => 0
  | cut :: rest =>
      if value.lt { numerator := cut, denominator := cutDenominator } then 0
      else 1 + ordinarySelectorFrom value cutDenominator rest

def ProductDomain.ordinarySelector (domain : ProductDomain) (value : Rational) : Nat :=
  ordinarySelectorFrom value domain.cutDenominator domain.cuts

def ProductDomain.map (domain : ProductDomain) : Result → Nat
  | .observed value => domain.ordinarySelector value
  | .failure => domain.failureSelector

theorem ProductDomain.ordinarySelectorFrom_le_length
    (value : Rational) (denominator : Nat) (cuts : List Int) :
    ordinarySelectorFrom value denominator cuts ≤ cuts.length := by
  induction cuts with
  | nil => exact Nat.le_refl _
  | cons cut rest induction =>
      simp only [ordinarySelectorFrom, List.length_cons]
      split
      · omega
      · omega

/-- Equality with a cut maps to the region on its right.  This is the one
canonical edge orientation used by Product. -/
theorem ProductDomain.cut_boundary_maps_right
    (cut : Int) (denominator : Nat) (rest : List Int) :
    ordinarySelectorFrom ⟨cut, denominator⟩ denominator (cut :: rest) =
      1 + ordinarySelectorFrom ⟨cut, denominator⟩ denominator rest := by
  simp [ordinarySelectorFrom, Rational.lt]

/-- Every ordinary result lands in exactly one ordinary region. -/
theorem ProductDomain.ordinary_partition_exhaustive
    (domain : ProductDomain) (value : Rational) :
    domain.map (.observed value) < domain.regionCount := by
  simp only [ProductDomain.map, ProductDomain.ordinarySelector,
    ProductDomain.regionCount]
  have := ordinarySelectorFrom_le_length value domain.cutDenominator domain.cuts
  omega

/-- Explicit failure is the final Product outcome and cannot alias an ordinary
region. -/
theorem ProductDomain.ordinary_ne_failure
    (domain : ProductDomain) (value : Rational) :
    domain.map (.observed value) ≠ domain.map .failure := by
  have hlt := domain.ordinary_partition_exhaustive value
  simp only [ProductDomain.map, ProductDomain.failureSelector] at hlt ⊢
  omega

@[simp] theorem ProductDomain.map_failure (domain : ProductDomain) :
    domain.map .failure = domain.failureSelector := rfl

/-- Ordinary and failure selectors both lie inside the exhaustive outcome
width derived from the same Product artifact. -/
theorem ProductDomain.map_bounded (domain : ProductDomain) (result : Result) :
    domain.map result < domain.outcomeCount := by
  cases result with
  | observed value =>
      have := domain.ordinary_partition_exhaustive value
      unfold ProductDomain.outcomeCount
      omega
  | failure =>
      simp only [ProductDomain.map, ProductDomain.failureSelector,
        ProductDomain.outcomeCount]
      omega

/-- Mapping is functional, hence its regions are disjoint. -/
theorem ProductDomain.mapping_disjoint
    {domain : ProductDomain} {result : Result} {left right : Nat}
    (hleft : domain.map result = left) (hright : domain.map result = right) :
    left = right := by
  omega

/-! ## Release-bound provider admission -/

/-- Immutable release coordinates chosen by one Source leg. -/
structure ProviderRelease where
  sourceMaterialId : Nat
  sourceId : Nat
  providerFamilyId : Nat
  providerReleaseId : Nat
  adapterReleaseId : Nat
  decodingRulesId : Nat
  transportProfileId : Nat
  scheduleId : Nat
  deriving DecidableEq, Repr

def ProviderRelease.Valid (release : ProviderRelease) : Prop :=
  release.sourceMaterialId ≠ 0 ∧ release.sourceId ≠ 0 ∧
  release.providerFamilyId ≠ 0 ∧ release.providerReleaseId ≠ 0 ∧
  release.adapterReleaseId ≠ 0 ∧ release.decodingRulesId ≠ 0 ∧
  release.transportProfileId ≠ 0 ∧ release.scheduleId ≠ 0

def ProviderRelease.valid (release : ProviderRelease) : Bool :=
  release.sourceMaterialId != 0 && release.sourceId != 0 &&
  release.providerFamilyId != 0 && release.providerReleaseId != 0 &&
  release.adapterReleaseId != 0 && release.decodingRulesId != 0 &&
  release.transportProfileId != 0 && release.scheduleId != 0

/-- One canonical scheduled Source leg.

## The observation window has width, and why that is not optional

`windowStart` and `windowEnd` are the **closed observation window**: the period
of foreign time the Product actually sold.  They are two fields rather than one
instant because a real provider does not publish on demand at a second of our
choosing.  Pyth's devnet SOL/USD feed publishes on a p50 cadence of roughly five
minutes; a market whose window were one exact second would be resolvable only if
a publication happened to land on that second, which is to say essentially never.
A degenerate window (`windowStart = windowEnd`) remains legal and means exactly
what it says — an instant — but it is a choice a market makes, not a shape the
type forces on every market.

`acceptThrough` is a different clock and answers a different question.  The
window bounds *what the observation is about*; `acceptThrough` bounds *when this
cluster will still act on it*.  A fresh observation of the wrong period and a
stale observation of the right one must both refuse, so neither bound can be
derived from the other.  `maximumPublicationAge` bounds the third relationship,
between the provider's publication stamp and this cluster's clock.

## The selection rule

Widening the window admits more than one observation, so the rule that keeps
"exactly one answer" must be stated rather than inherited from arithmetic:

> The **first admissible** observation terminalizes the leg.  Admissible means
> `windowStart ≤ observationTime ≤ windowEnd` under a live clock
> (`windowStart ≤ now ≤ acceptThrough`) with a publication this cluster will
> still believe.  Every later observation refuses, admissible or not, because
> `Config.activeLeg?` is `none` at every terminal phase.  A window that closes
> with no admissible observation reaches the Product's own failure outcome
> through `.exhaust` and then `.commitFailure`, and by no other path.

Nothing about that rule is caller-chosen and nothing about it is a race: the
transition single-writes the phase, so "first" is decided by the ledger's own
ordering and is checked, not assumed.  See `Leg.admits`,
`checkEvidence_ok_is_admissible`, and
`two_admissible_observations_cannot_both_terminalize`. -/
structure Leg where
  release : ProviderRelease
  scheduleIndex : Nat
  windowStart : Nat
  windowEnd : Nat
  acceptThrough : Nat
  maximumPublicationAge : Nat
  fundingAllocationId : Nat
  workQuote : Nat
  deriving DecidableEq, Repr

def Leg.Valid (leg : Leg) : Prop :=
  leg.release.Valid ∧ leg.windowStart ≤ leg.windowEnd ∧
  leg.windowEnd ≤ leg.acceptThrough ∧
  0 < leg.maximumPublicationAge ∧ leg.fundingAllocationId ≠ 0 ∧
  0 < leg.workQuote

def Leg.valid (leg : Leg) : Bool :=
  leg.release.valid && leg.windowStart <= leg.windowEnd &&
  leg.windowEnd <= leg.acceptThrough &&
  leg.maximumPublicationAge != 0 && leg.fundingAllocationId != 0 &&
  leg.workQuote != 0

/-- Ordered finite repair leg.  The position in the list is the attempt index;
there is no separately stored, potentially divergent index. -/
structure RecoveryAttempt where
  leg : Leg
  entryFundingAllocationId : Nat
  entryWorkQuote : Nat
  deriving DecidableEq, Repr

def RecoveryAttempt.Valid (attempt : RecoveryAttempt) : Prop :=
  attempt.leg.Valid ∧ attempt.entryFundingAllocationId ≠ 0 ∧
  0 < attempt.entryWorkQuote

def RecoveryAttempt.valid (attempt : RecoveryAttempt) : Bool :=
  attempt.leg.valid && attempt.entryFundingAllocationId != 0 &&
  attempt.entryWorkQuote != 0

/-- Provider-neutral normalized evidence after the named adapter boundary.
`adapterAuthenticated` records the trust assumption explicitly; it is rechecked
alongside every release coordinate. -/
structure NormalizedEvidence where
  adapterAuthenticated : Bool
  sourceMaterialId : Nat
  sourceId : Nat
  providerFamilyId : Nat
  providerReleaseId : Nat
  adapterReleaseId : Nat
  decodingRulesId : Nat
  transportProfileId : Nat
  scheduleId : Nat
  scheduleIndex : Nat
  observationTime : Nat
  publicationTime : Nat
  evidenceId : Nat
  value : Rational
  deriving DecidableEq, Repr

/-- Provider and Solana runtime operations deliberately remain outside the pure
specialization.  This type documents exactly what the adapter must establish. -/
structure AdapterBoundary where
  authenticatesCpi : Bool
  authenticatesProgramData : Bool
  authenticatesAccounts : Bool
  authenticatesParser : Bool
  authenticatesClock : Bool
  deriving DecidableEq, Repr

def AdapterBoundary.Complete (boundary : AdapterBoundary) : Prop :=
  boundary.authenticatesCpi = true ∧
  boundary.authenticatesProgramData = true ∧
  boundary.authenticatesAccounts = true ∧
  boundary.authenticatesParser = true ∧
  boundary.authenticatesClock = true

/-! ## Funding, lifecycle, effects, and certificates -/

/-- Capability-owned prepaid work allocation.  Source consumes an authenticated
projection but does not persist a second amount. -/
structure FundingState where
  allocationId : Nat
  initialCapital : Nat
  remainingCapital : Nat
  paidCapital : Nat
  callCount : Nat
  deriving DecidableEq, Repr

def FundingState.Conserved (funding : FundingState) : Prop :=
  funding.initialCapital = funding.remainingCapital + funding.paidCapital

def FundingState.charge?
    (funding : FundingState) (allocationId quote : Nat) : Option FundingState :=
  if allocationId = 0 ∨ funding.allocationId ≠ allocationId ∨ quote = 0 ∨
      funding.remainingCapital < quote then
    none
  else
    some { funding with
      remainingCapital := funding.remainingCapital - quote
      paidCapital := funding.paidCapital + quote
      callCount := funding.callCount + 1
    }

theorem FundingState.charge_exact
    {funding charged : FundingState} {allocationId quote : Nat}
    (h : funding.charge? allocationId quote = some charged) :
    charged.remainingCapital + quote = funding.remainingCapital ∧
    charged.paidCapital = funding.paidCapital + quote ∧
    charged.callCount = funding.callCount + 1 ∧
    charged.initialCapital = funding.initialCapital := by
  unfold FundingState.charge? at h
  split at h
  · contradiction
  · simp only [Option.some.injEq] at h
    subst charged
    simp
    omega

theorem FundingState.charge_conserves
    {funding charged : FundingState} {allocationId quote : Nat}
    (hconserved : funding.Conserved)
    (h : funding.charge? allocationId quote = some charged) : charged.Conserved := by
  obtain ⟨hremaining, hpaid, _, hinitial⟩ := charge_exact h
  unfold FundingState.Conserved at hconserved ⊢
  omega

inductive Phase where
  | primary
  | recovery (index : Nat)
  | resolved (selector : Nat)
  | exhausted
  | failureCommitted
  | retired
  deriving DecidableEq, Repr

def Phase.tag : Phase → Nat
  | .primary => 1
  | .recovery _ => 2
  | .resolved _ => 3
  | .exhausted => 4
  | .failureCommitted => 5
  | .retired => 6

def Phase.coordinate : Phase → Nat
  | .recovery index => index
  | .resolved selector => selector
  | _ => 0

/-- Persisted semantic Source authority.  Funding amounts and provider account
bytes are deliberately absent. -/
structure State where
  marketId : Nat
  generation : Nat
  sourceMaterialId : Nat
  phase : Phase
  transitionSequence : Nat
  terminalEvidenceId : Nat
  deriving DecidableEq, Repr

def State.Valid (state : State) : Prop :=
  state.marketId ≠ 0 ∧ state.generation ≠ 0 ∧
  state.sourceMaterialId ≠ 0 ∧
  match state.phase with
  | .primary | .recovery _ | .exhausted => state.terminalEvidenceId = 0
  | .resolved _ | .failureCommitted | .retired => state.terminalEvidenceId ≠ 0

/-- Immutable liveness and ownership configuration. -/
structure Config where
  marketId : Nat
  generation : Nat
  sourceOwnerId : Nat
  sourceStateId : Nat
  productResolutionStateId : Nat
  receiptOwnerId : Nat
  productDomain : ProductDomain
  primary : Leg
  recoveries : List RecoveryAttempt
  exhaustFundingAllocationId : Nat
  exhaustWorkQuote : Nat
  failureFundingAllocationId : Nat
  failureWorkQuote : Nat
  deriving DecidableEq, Repr

def RecoveryDeadlinesOrdered : List RecoveryAttempt → Prop
  | [] | [_] => True
  | left :: right :: rest =>
      left.leg.acceptThrough < right.leg.acceptThrough ∧
        RecoveryDeadlinesOrdered (right :: rest)

def RecoveriesFollowPrimary (primary : Leg) : List RecoveryAttempt → Prop
  | [] => True
  | first :: rest =>
      primary.acceptThrough < first.leg.acceptThrough ∧
        RecoveryDeadlinesOrdered (first :: rest)

def recoveryDeadlinesOrdered : List RecoveryAttempt → Bool
  | [] | [_] => true
  | left :: right :: rest =>
      left.leg.acceptThrough < right.leg.acceptThrough &&
        recoveryDeadlinesOrdered (right :: rest)

def recoveriesFollowPrimary (primary : Leg) : List RecoveryAttempt → Bool
  | [] => true
  | first :: rest =>
      primary.acceptThrough < first.leg.acceptThrough &&
        recoveryDeadlinesOrdered (first :: rest)

def Config.Valid (config : Config) : Prop :=
  config.marketId ≠ 0 ∧ config.generation ≠ 0 ∧
  config.sourceOwnerId ≠ 0 ∧ config.sourceStateId ≠ 0 ∧
  config.productResolutionStateId ≠ 0 ∧ config.receiptOwnerId ≠ 0 ∧
  config.productDomain.Valid ∧ config.primary.Valid ∧
  (∀ attempt ∈ config.recoveries, attempt.Valid) ∧
  (∀ attempt ∈ config.recoveries,
    attempt.leg.release.sourceMaterialId = config.primary.release.sourceMaterialId) ∧
  RecoveriesFollowPrimary config.primary config.recoveries ∧
  config.exhaustFundingAllocationId ≠ 0 ∧ 0 < config.exhaustWorkQuote ∧
  config.failureFundingAllocationId ≠ 0 ∧ 0 < config.failureWorkQuote

/-- Executable validation of the finite configuration.  The list check avoids
turning an unbounded `∀ attempt` proposition into a runtime assumption. -/
def Config.valid (config : Config) : Bool :=
  config.marketId != 0 && config.generation != 0 &&
  config.sourceOwnerId != 0 && config.sourceStateId != 0 &&
  config.productResolutionStateId != 0 && config.receiptOwnerId != 0 &&
  config.productDomain.valid && config.primary.valid &&
  recoveriesFollowPrimary config.primary config.recoveries &&
  config.exhaustFundingAllocationId != 0 && config.exhaustWorkQuote != 0 &&
  config.failureFundingAllocationId != 0 && config.failureWorkQuote != 0 &&
  config.recoveries.all fun attempt =>
    attempt.valid &&
      attempt.leg.release.sourceMaterialId == config.primary.release.sourceMaterialId

def State.valid (state : State) : Bool :=
  decide (state.marketId ≠ 0 ∧ state.generation ≠ 0 ∧
    state.sourceMaterialId ≠ 0) &&
  match state.phase with
  | .primary | .recovery _ | .exhausted => decide (state.terminalEvidenceId = 0)
  | .resolved _ | .failureCommitted | .retired => decide (state.terminalEvidenceId ≠ 0)

/-- Closed mutation vocabulary.  Tags and fixed-width encoding live in
`SourceResolutionAbi`; this semantic type contains no account memory. -/
inductive AccountRole where
  | sourceState
  | fundingState
  | worker
  | productResolution
  | receipt
  deriving DecidableEq, Repr

inductive Resource where
  | phase
  | generation
  | workCapital
  | resolutionOutcome
  | terminalReceipt
  deriving DecidableEq, Repr

inductive Operation where
  | set
  | debit
  | credit
  deriving DecidableEq, Repr

structure Effect where
  operation : Operation
  role : AccountRole
  resource : Resource
  coordinate : Nat
  value : Nat
  deriving DecidableEq, Repr

structure EffectPlan where
  effects : List Effect
  deriving DecidableEq, Repr

inductive CertificateKind where
  | resolutionSuccess
  | recoveryAdvanced
  | exhausted
  | resolutionFailure
  deriving DecidableEq, Repr

def CertificateKind.tag : CertificateKind → Nat
  | .resolutionSuccess => 1
  | .recoveryAdvanced => 2
  | .exhausted => 3
  | .resolutionFailure => 4

/-- One compact canonical receipt shape for every Source transition.  Inactive
fields are canonical zero and checked by `Certificate.Valid`. -/
structure Certificate where
  kind : CertificateKind
  marketId : Nat
  routeId : Nat
  sourceMaterialId : Nat
  productId : Nat
  providerEvidenceId : Nat
  fundingAllocationId : Nat
  receiptAccountId : Nat
  generation : Nat
  attemptIndex : Nat
  scheduleIndex : Nat
  selector : Nat
  workPaid : Nat
  fundingRemaining : Nat
  result : Rational
  observedAt : Nat
  deriving DecidableEq, Repr

def Certificate.Valid (certificate : Certificate) : Prop :=
  certificate.marketId ≠ 0 ∧ certificate.sourceMaterialId ≠ 0 ∧
  certificate.productId ≠ 0 ∧ certificate.fundingAllocationId ≠ 0 ∧
  certificate.receiptAccountId ≠ 0 ∧ certificate.generation ≠ 0 ∧
  0 < certificate.workPaid ∧
  match certificate.kind with
  | .resolutionSuccess =>
      certificate.routeId ≠ 0 ∧ certificate.providerEvidenceId ≠ 0 ∧
        certificate.result.Valid
  | .recoveryAdvanced | .exhausted | .resolutionFailure =>
      certificate.providerEvidenceId = 0 ∧ certificate.result = ⟨0, 0⟩

structure Plan where
  sourcePost : State
  fundingPost : FundingState
  certificate : Certificate
  effectPlan : EffectPlan
  deriving DecidableEq, Repr

inductive Refusal where
  | invalidConfiguration
  | invalidState
  | wrongMarket
  | wrongGeneration
  | wrongPhase
  | adapterRejected
  | wrongRelease
  | wrongSchedule
  | wrongObservationTime
  | beforeObservationTime
  | legExpired
  | futurePublication
  | stalePublication
  | invalidResult
  | invalidDestination
  | wrongFundingAllocation
  | insufficientWorkCapital
  | recoveryUnavailable
  | recoveryNotLast
  | notExhausted
  deriving DecidableEq, Repr

inductive Command where
  | accept (evidence : NormalizedEvidence) (now worker receiptAccountId : Nat)
  | failNext (now worker receiptAccountId : Nat)
  | exhaust (now worker receiptAccountId : Nat)
  | commitFailure (worker receiptAccountId : Nat)
  deriving DecidableEq, Repr

/-! ## Pure helpers -/

/-- The active leg is selected only by persisted phase and the canonical
recovery list position. -/
def Config.activeLeg? (config : Config) (phase : Phase) : Option Leg :=
  match phase with
  | .primary => some config.primary
  | .recovery index => (config.recoveries[index]?).map RecoveryAttempt.leg
  | _ => none

def optionToExcept (error : Refusal) : Option α → Except Refusal α
  | some value => .ok value
  | none => .error error

theorem optionToExcept_ok {error : Refusal} {value : Option α} {result : α}
    (h : optionToExcept error value = .ok result) : value = some result := by
  cases value <;> simp_all [optionToExcept]

def releaseMatches (release : ProviderRelease) (evidence : NormalizedEvidence) : Bool :=
  evidence.sourceMaterialId == release.sourceMaterialId &&
  evidence.sourceId == release.sourceId &&
  evidence.providerFamilyId == release.providerFamilyId &&
  evidence.providerReleaseId == release.providerReleaseId &&
  evidence.adapterReleaseId == release.adapterReleaseId &&
  evidence.decodingRulesId == release.decodingRulesId &&
  evidence.transportProfileId == release.transportProfileId

/-- The admissibility predicate for one leg: exactly the observations this leg
is allowed to answer with.  `checkEvidence` is its refusal-labelled twin, and
`checkEvidence_ok_is_admissible` pins them together in both directions so the
selection rule cannot drift away from the checks that enforce it. -/
def Leg.admits (leg : Leg) (evidence : NormalizedEvidence) (now : Nat) : Bool :=
  evidence.adapterAuthenticated && evidence.evidenceId != 0 &&
  releaseMatches leg.release evidence &&
  evidence.scheduleId == leg.release.scheduleId &&
  evidence.scheduleIndex == leg.scheduleIndex &&
  leg.windowStart <= evidence.observationTime &&
  evidence.observationTime <= leg.windowEnd &&
  leg.windowStart <= now && now <= leg.acceptThrough &&
  evidence.publicationTime <= now &&
  now - evidence.publicationTime <= leg.maximumPublicationAge &&
  evidence.value.denominator != 0

/-- The guards behind `Leg.admits`, each carrying its own refusal.

The two observation bounds are separate refusals on purpose.  An observation
before `windowStart` is about a period the market had not started selling yet;
one after `windowEnd` is a **late** observation, the exact case a real provider
cadence produces when nobody submitted in time, and it must refuse rather than
resolve the market on a price from after the question closed. -/
def checkEvidence (leg : Leg) (evidence : NormalizedEvidence) (now : Nat) :
    Except Refusal Unit := do
  if !evidence.adapterAuthenticated || evidence.evidenceId = 0 then
    throw .adapterRejected
  let release := leg.release
  if !releaseMatches release evidence then throw .wrongRelease
  if evidence.scheduleId ≠ release.scheduleId ||
      evidence.scheduleIndex ≠ leg.scheduleIndex then
    throw .wrongSchedule
  if evidence.observationTime < leg.windowStart then throw .beforeObservationTime
  if leg.windowEnd < evidence.observationTime then throw .wrongObservationTime
  if now < leg.windowStart then throw .beforeObservationTime
  if leg.acceptThrough < now then throw .legExpired
  if now < evidence.publicationTime then throw .futurePublication
  if leg.maximumPublicationAge < now - evidence.publicationTime then
    throw .stalePublication
  if evidence.value.denominator = 0 then throw .invalidResult

/-- **Soundness of the rule**: nothing outside the window is ever accepted.  Any
evidence the guards let through satisfies every clause of `Leg.admits`, including
both closed observation bounds. -/
theorem checkEvidence_ok_implies_admissible {leg : Leg}
    {evidence : NormalizedEvidence} {now : Nat}
    (h : checkEvidence leg evidence now = .ok ()) :
    leg.admits evidence now = true := by
  unfold checkEvidence at h
  simp only [bind, Except.bind, pure, Except.pure] at h
  repeat' split at h
  all_goals try contradiction
  -- Exactly one branch survives: the one where every guard was passed.  Only
  -- the three boolean guards need naming; the seven time bounds are ordinary
  -- `Nat` facts that `omega` reads straight out of the context.
  rename_i hadapter hrelease hschedule _ _ _ _ _ _ _
  simp only [Bool.or_eq_true, Bool.not_eq_true', decide_eq_true_eq, not_or,
    Bool.not_eq_false, ne_eq, Decidable.not_not] at hadapter hrelease hschedule
  unfold Leg.admits
  simp only [Bool.and_eq_true, bne_iff_ne, beq_iff_eq, decide_eq_true_eq, ne_eq]
  repeat' apply And.intro
  all_goals first
    | exact hadapter.1
    | exact hadapter.2
    | exact hrelease
    | exact hschedule.1
    | exact hschedule.2
    | omega

/-- **Completeness of the rule**: the window is not secretly narrowed by some
other guard.  Every admissible observation passes `checkEvidence`, so the two
edges of `[windowStart, windowEnd]` are genuinely reachable rather than
unreachable in practice — which is exactly the defect a one-instant window
had. -/
theorem admissible_implies_checkEvidence_ok {leg : Leg}
    {evidence : NormalizedEvidence} {now : Nat}
    (h : leg.admits evidence now = true) :
    checkEvidence leg evidence now = .ok () := by
  unfold Leg.admits at h
  simp only [Bool.and_eq_true, bne_iff_ne, beq_iff_eq, decide_eq_true_eq,
    ne_eq] at h
  obtain ⟨⟨⟨⟨⟨⟨⟨⟨⟨⟨⟨hauth, hid⟩, hrel⟩, hsched⟩, hindex⟩, _⟩, _⟩,
    _⟩, _⟩, _⟩, _⟩, _⟩ := h
  unfold checkEvidence
  simp only [bind, Except.bind, pure, Except.pure]
  repeat' split
  -- One branch per guard, plus the acceptance.  Every refusal branch is
  -- refuted by the admissibility clause that guard tests: `omega` for the
  -- seven time bounds, the named boolean facts for the other three.
  all_goals first
    | rfl
    | (exfalso; omega)
    | (exfalso; rename_i c; simp [hauth, hid, hrel, hsched, hindex] at c <;> omega)

/-- The checks and the stated rule are the same set. -/
theorem checkEvidence_ok_is_admissible (leg : Leg) (evidence : NormalizedEvidence)
    (now : Nat) :
    checkEvidence leg evidence now = .ok () ↔ leg.admits evidence now = true :=
  ⟨checkEvidence_ok_implies_admissible, admissible_implies_checkEvidence_ok⟩

def fundingEffects (quote : Nat) : List Effect := [
  { operation := .debit, role := .fundingState, resource := .workCapital,
    coordinate := 0, value := quote },
  { operation := .credit, role := .worker, resource := .workCapital,
    coordinate := 0, value := quote }
]

def statePhaseEffect (phase : Phase) : Effect := {
  operation := .set
  role := .sourceState
  resource := .phase
  coordinate := phase.coordinate
  value := phase.tag
}

def generationEffect (generation : Nat) : Effect := {
  operation := .set
  role := .sourceState
  resource := .generation
  coordinate := 0
  value := generation
}

def resolutionEffect (selector : Nat) : Effect := {
  operation := .set
  role := .productResolution
  resource := .resolutionOutcome
  coordinate := selector
  value := 1
}

def receiptEffect (sequence : Nat) : Effect := {
  operation := .set
  role := .receipt
  resource := .terminalReceipt
  coordinate := 0
  value := sequence
}

def chargeFunding (funding : FundingState) (allocation quote : Nat) :
    Except Refusal FundingState :=
  if funding.allocationId ≠ allocation then .error .wrongFundingAllocation
  else match funding.charge? allocation quote with
    | some charged => .ok charged
    | none => .error .insufficientWorkCapital

theorem chargeFunding_conserves
    {funding charged : FundingState} {allocation quote : Nat}
    (hconserved : funding.Conserved)
    (h : chargeFunding funding allocation quote = .ok charged) :
    charged.Conserved := by
  unfold chargeFunding at h
  split at h
  · contradiction
  next hallocation =>
    split at h
    · next next hnext =>
        simp only [Except.ok.injEq] at h
        subst charged
        exact FundingState.charge_conserves hconserved hnext
    · contradiction

theorem chargeFunding_exact
    {funding charged : FundingState} {allocation quote : Nat}
    (h : chargeFunding funding allocation quote = .ok charged) :
    charged.remainingCapital + quote = funding.remainingCapital ∧
    charged.paidCapital = funding.paidCapital + quote ∧
    charged.callCount = funding.callCount + 1 := by
  unfold chargeFunding at h
  split at h
  · contradiction
  next hallocation =>
    split at h
    · next next hnext =>
        simp only [Except.ok.injEq] at h
        subst charged
        obtain ⟨hremaining, hpaid, hcalls, _⟩ := FundingState.charge_exact hnext
        exact ⟨hremaining, hpaid, hcalls⟩
    · contradiction

def validateContext (config : Config) (state : State) : Except Refusal Unit := do
  if !config.valid then throw .invalidConfiguration
  if !state.valid then throw .invalidState
  if state.marketId ≠ config.marketId ||
      state.sourceMaterialId ≠ config.primary.release.sourceMaterialId then
    throw .wrongMarket
  if state.generation ≠ config.generation then throw .wrongGeneration

/-- Deterministic Source specializer.  A successful result carries the whole
post-state, capability-owned funding post-state, canonical certificate, and
bounded effects.  Refusal carries none of them. -/
def specialize (config : Config) (state : State) (funding : FundingState)
    (command : Command) : Except Refusal Plan := do
  validateContext config state
  match command with
  | .accept evidence now worker receiptAccountId =>
      if worker = 0 || receiptAccountId = 0 then throw .invalidDestination
      let leg ← optionToExcept .wrongPhase (config.activeLeg? state.phase)
      checkEvidence leg evidence now
      let charged ← chargeFunding funding leg.fundingAllocationId leg.workQuote
      let selector := config.productDomain.map (.observed evidence.value)
      let sequence := state.transitionSequence + 1
      let post := { state with
        phase := .resolved selector
        transitionSequence := sequence
        terminalEvidenceId := evidence.evidenceId
      }
      let certificate : Certificate := {
        kind := .resolutionSuccess
        marketId := state.marketId
        routeId := leg.release.providerReleaseId
        sourceMaterialId := state.sourceMaterialId
        productId := config.productDomain.productId
        providerEvidenceId := evidence.evidenceId
        fundingAllocationId := charged.allocationId
        receiptAccountId
        generation := state.generation
        attemptIndex := match state.phase with | .recovery index => index + 1 | _ => 0
        scheduleIndex := leg.scheduleIndex
        selector
        workPaid := leg.workQuote
        fundingRemaining := charged.remainingCapital
        result := evidence.value
        observedAt := now
      }
      let effects := fundingEffects leg.workQuote ++ [
        statePhaseEffect post.phase,
        resolutionEffect selector,
        receiptEffect sequence
      ]
      pure {
        sourcePost := post
        fundingPost := charged
        certificate := certificate
        effectPlan := { effects := effects }
      }
  | .failNext now worker receiptAccountId =>
      if worker = 0 || receiptAccountId = 0 then throw .invalidDestination
      let current ← optionToExcept .wrongPhase (config.activeLeg? state.phase)
      if now ≤ current.acceptThrough then throw .legExpired
      let nextIndex := match state.phase with | .primary => 0 | .recovery index => index + 1 | _ => 0
      let attempt ← optionToExcept .recoveryUnavailable config.recoveries[nextIndex]?
      let charged ← chargeFunding funding attempt.entryFundingAllocationId
        attempt.entryWorkQuote
      let sequence := state.transitionSequence + 1
      let post := { state with phase := .recovery nextIndex, transitionSequence := sequence }
      let certificate : Certificate := {
        kind := .recoveryAdvanced
        marketId := state.marketId
        routeId := attempt.leg.release.providerReleaseId
        sourceMaterialId := state.sourceMaterialId
        productId := config.productDomain.productId
        providerEvidenceId := 0
        fundingAllocationId := charged.allocationId
        receiptAccountId
        generation := state.generation
        attemptIndex := nextIndex + 1
        scheduleIndex := attempt.leg.scheduleIndex
        selector := 0
        workPaid := attempt.entryWorkQuote
        fundingRemaining := charged.remainingCapital
        result := ⟨0, 0⟩
        observedAt := now
      }
      let effects := fundingEffects attempt.entryWorkQuote ++ [
        statePhaseEffect post.phase,
        generationEffect post.generation,
        receiptEffect sequence
      ]
      pure {
        sourcePost := post
        fundingPost := charged
        certificate := certificate
        effectPlan := { effects := effects }
      }
  | .exhaust now worker receiptAccountId =>
      if worker = 0 || receiptAccountId = 0 then throw .invalidDestination
      let current ← optionToExcept .wrongPhase (config.activeLeg? state.phase)
      if now ≤ current.acceptThrough then throw .legExpired
      let isLast := match state.phase with
        | .primary => config.recoveries.isEmpty
        | .recovery index => index + 1 = config.recoveries.length
        | _ => false
      if !isLast then throw .recoveryNotLast
      let charged ← chargeFunding funding config.exhaustFundingAllocationId
        config.exhaustWorkQuote
      let sequence := state.transitionSequence + 1
      let post := { state with phase := .exhausted, transitionSequence := sequence }
      let certificate : Certificate := {
        kind := .exhausted
        marketId := state.marketId
        routeId := current.release.providerReleaseId
        sourceMaterialId := state.sourceMaterialId
        productId := config.productDomain.productId
        providerEvidenceId := 0
        fundingAllocationId := charged.allocationId
        receiptAccountId
        generation := state.generation
        attemptIndex := config.recoveries.length
        scheduleIndex := current.scheduleIndex
        selector := 0
        workPaid := config.exhaustWorkQuote
        fundingRemaining := charged.remainingCapital
        result := ⟨0, 0⟩
        observedAt := now
      }
      let effects := fundingEffects config.exhaustWorkQuote ++ [
        statePhaseEffect post.phase,
        receiptEffect sequence
      ]
      pure {
        sourcePost := post
        fundingPost := charged
        certificate := certificate
        effectPlan := { effects := effects }
      }
  | .commitFailure worker receiptAccountId =>
      if state.phase = .exhausted then pure () else throw .notExhausted
      if worker = 0 || receiptAccountId = 0 then throw .invalidDestination
      let charged ← chargeFunding funding config.failureFundingAllocationId
        config.failureWorkQuote
      let selector := config.productDomain.map .failure
      let sequence := state.transitionSequence + 1
      let post := { state with
        phase := .failureCommitted
        transitionSequence := sequence
        terminalEvidenceId := receiptAccountId
      }
      let certificate : Certificate := {
        kind := .resolutionFailure
        marketId := state.marketId
        routeId := 0
        sourceMaterialId := state.sourceMaterialId
        productId := config.productDomain.productId
        providerEvidenceId := 0
        fundingAllocationId := charged.allocationId
        receiptAccountId
        generation := state.generation
        attemptIndex := config.recoveries.length
        scheduleIndex := 0
        selector
        workPaid := config.failureWorkQuote
        fundingRemaining := charged.remainingCapital
        result := ⟨0, 0⟩
        observedAt := 0
      }
      let effects := fundingEffects config.failureWorkQuote ++ [
        statePhaseEffect post.phase,
        resolutionEffect selector,
        receiptEffect sequence
      ]
      pure {
        sourcePost := post
        fundingPost := charged
        certificate := certificate
        effectPlan := { effects := effects }
      }

/-! ## Transition properties -/

theorem specialize_deterministic
    {config : Config} {state : State} {funding : FundingState} {command : Command}
    {left right : Plan}
    (hleft : specialize config state funding command = .ok left)
    (hright : specialize config state funding command = .ok right) : left = right := by
  rw [hleft] at hright
  exact Except.ok.inj hright

/-- Refusal exposes the old semantic and funding states with no effects. -/
def executeProjection (config : Config) (state : State) (funding : FundingState)
    (command : Command) : State × FundingState × List Effect :=
  match specialize config state funding command with
  | .ok plan => (plan.sourcePost, plan.fundingPost, plan.effectPlan.effects)
  | .error _ => (state, funding, [])

theorem refusal_is_atomic
    {config : Config} {state : State} {funding : FundingState}
    {command : Command} {error : Refusal}
    (h : specialize config state funding command = .error error) :
    executeProjection config state funding command = (state, funding, []) := by
  unfold executeProjection
  rw [h]

/-- Every successful Source action conserves the capability-owned funding
allocation exactly. -/
theorem specialize_conserves_funding
    {config : Config} {state : State} {funding : FundingState}
    {command : Command} {plan : Plan}
    (hconserved : funding.Conserved)
    (h : specialize config state funding command = .ok plan) :
    plan.fundingPost.Conserved := by
  unfold specialize at h
  simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals try contradiction
  all_goals
    simp only [pure, Except.pure, Except.ok.injEq] at h
    subst plan
    apply chargeFunding_conserves hconserved
    assumption

/-- Every emitted receipt reports the exact capability-owned allocation,
post-payment remainder, and charged work amount. -/
theorem specialize_receipt_matches_funding
    {config : Config} {state : State} {funding : FundingState}
    {command : Command} {plan : Plan}
    (h : specialize config state funding command = .ok plan) :
    plan.certificate.fundingAllocationId = plan.fundingPost.allocationId ∧
    plan.certificate.fundingRemaining = plan.fundingPost.remainingCapital ∧
    plan.fundingPost.remainingCapital + plan.certificate.workPaid =
      funding.remainingCapital := by
  unfold specialize at h
  simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals try contradiction
  all_goals
    simp only [pure, Except.pure, Except.ok.injEq] at h
    subst plan
    refine ⟨rfl, rfl, ?_⟩
    have hexact := chargeFunding_exact (by assumption)
    exact hexact.1

/-- Successful failure commitment proves prior explicit exhaustion. -/
theorem failure_commit_requires_exhaustion
    {config : Config} {state : State} {funding : FundingState}
    {worker receiptAccountId : Nat} {plan : Plan}
    (h : specialize config state funding (.commitFailure worker receiptAccountId) = .ok plan) :
    state.phase = .exhausted := by
  unfold specialize at h
  simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals try contradiction
  assumption

/-- Failure resolution is exactly Product's derived final selector. -/
theorem failure_commit_uses_product_selector
    {config : Config} {state : State} {funding : FundingState}
    {worker receiptAccountId : Nat} {plan : Plan}
    (h : specialize config state funding (.commitFailure worker receiptAccountId) = .ok plan) :
    plan.certificate.selector = config.productDomain.failureSelector ∧
    plan.sourcePost.phase = .failureCommitted := by
  unfold specialize at h
  simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals try contradiction
  next charged hcharge =>
    simp only [pure, Except.pure, Except.ok.injEq] at h
    subst plan
    simp [ProductDomain.map]

/-- A substituted provider release bind-refuses before normalized evidence can
affect state. -/
theorem wrong_provider_release_refuses
    {leg : Leg} {evidence : NormalizedEvidence} {now : Nat}
    (hauthenticated : evidence.adapterAuthenticated = true)
    (hevidence : evidence.evidenceId ≠ 0)
    (hwrong : evidence.providerReleaseId ≠ leg.release.providerReleaseId) :
    checkEvidence leg evidence now = .error .wrongRelease := by
  have hmismatch : releaseMatches leg.release evidence = false := by
    simp [releaseMatches, hwrong]
  simp [checkEvidence, hauthenticated, hevidence, hmismatch, bind, Except.bind,
    pure, Except.pure]

/-! ## Exactly one answer

The three theorems below are the whole content of the selection rule.  Widening
a terminal window from an instant to `[windowStart, windowEnd]` admits more than
one observation, so "exactly one answer" stops being a consequence of the
window's arithmetic and becomes a property of the transition that has to be
proved.  It is: the phase is single-written, `Config.activeLeg?` is `none` at
every terminal phase, and therefore no second observation can reach the
transition at all — not a later one, not a better one, not one from a caller who
picked it. -/

/-- No leg is active once the state is terminal.  This is the mechanism; the two
theorems after it are what it buys. -/
@[simp] theorem activeLeg_resolved (config : Config) (selector : Nat) :
    config.activeLeg? (.resolved selector) = none := rfl

/-- A successful acceptance leaves the state resolved at Product's own selector
for that evidence, and records that evidence as terminal. -/
theorem accept_post_is_resolved
    {config : Config} {state : State} {funding : FundingState}
    {evidence : NormalizedEvidence} {now worker receiptAccountId : Nat} {plan : Plan}
    (h : specialize config state funding
      (.accept evidence now worker receiptAccountId) = .ok plan) :
    plan.sourcePost.phase =
      .resolved (config.productDomain.map (.observed evidence.value)) := by
  unfold specialize at h
  simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals try contradiction
  all_goals
    simp only [pure, Except.pure, Except.ok.injEq] at h
    subst plan
    rfl

/-- **One answer.**  A resolved Source refuses every acceptance, whatever the
observation, whatever the clock, and whoever submits it.  Nothing here inspects
the second observation: it never gets that far. -/
theorem resolved_admits_no_second_answer
    {config : Config} {state : State} {funding : FundingState}
    {evidence : NormalizedEvidence} {now worker receiptAccountId selector : Nat}
    {plan : Plan}
    (hphase : state.phase = .resolved selector) :
    specialize config state funding
      (.accept evidence now worker receiptAccountId) ≠ .ok plan := by
  unfold specialize
  simp only [bind, Except.bind]
  repeat' split
  all_goals try simp
  all_goals rw [hphase] at *
  all_goals simp_all [Config.activeLeg?, optionToExcept]

/-- **The race shape, executed.**  Two admissible observations cannot both
terminalize: run any acceptance against the post-state of a successful one and it
refuses.  This is the statement a last-writer race would falsify. -/
theorem two_admissible_observations_cannot_both_terminalize
    {config : Config} {state : State} {funding : FundingState}
    {first second : NormalizedEvidence}
    {nowFirst workerFirst receiptFirst : Nat}
    {nowSecond workerSecond receiptSecond : Nat}
    {firstPlan secondPlan : Plan}
    (hfirst : specialize config state funding
      (.accept first nowFirst workerFirst receiptFirst) = .ok firstPlan) :
    specialize config firstPlan.sourcePost firstPlan.fundingPost
      (.accept second nowSecond workerSecond receiptSecond) ≠ .ok secondPlan :=
  resolved_admits_no_second_answer (accept_post_is_resolved hfirst)

/-- The observation-free window lands in the failure path and nowhere else, and
it may not do so early: `.exhaust` refuses while the active leg is still live, so
the last second on which an honest observation may resolve the market and the
first second on which it may be walked to failure are different seconds.  With
`failure_commit_requires_exhaustion`, that makes the Product's failure outcome
reachable only after the window it sold has actually closed. -/
theorem exhaust_requires_the_window_to_have_closed
    {config : Config} {state : State} {funding : FundingState}
    {now worker receiptAccountId : Nat} {plan : Plan}
    (h : specialize config state funding
      (.exhaust now worker receiptAccountId) = .ok plan) :
    ∃ leg, config.activeLeg? state.phase = some leg ∧ leg.acceptThrough < now := by
  unfold specialize at h
  simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals try contradiction
  all_goals exact ⟨_, optionToExcept_ok (by assumption), by omega⟩

/-- Recovery advancement selects only the immediate canonical list successor. -/
theorem failNext_from_recovery_advances_one
    {config : Config} {state : State} {funding : FundingState}
    {index now worker receiptAccountId : Nat} {plan : Plan}
    (hphase : state.phase = .recovery index)
    (h : specialize config state funding (.failNext now worker receiptAccountId) = .ok plan) :
    plan.sourcePost.phase = .recovery (index + 1) := by
  unfold specialize at h
  rw [hphase] at h
  simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals try contradiction
  next current hcurrent attempt hattempt charged hcharge =>
    simp only [pure, Except.pure, Except.ok.injEq] at h
    subst plan
    rfl

end DClutch.SourceResolution
