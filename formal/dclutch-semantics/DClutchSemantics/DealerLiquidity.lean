import DClutchSemantics.EconomicKernel
import DClutchSemantics.ExecutionRelease
import Std.Tactic

/-!
# Dealer liquidity as immutable data interpreted by one total machine

This is the semantic owner for the successor Dealer capability.  Quote curves,
inventory limits, replacement timing, fee policy, and prepaid work funding are
data.  One width-independent interpreter handles every finite Product width;
there is no family of outcome-count-specialized routes.

The module links every claim fill and terminal redemption to an admitted
`Economic.Frame`.  Terminal entry consumes the identity and result projected
from the canonical Core Market; it never installs a parallel resolver signer.
It separately emits exact collateral transfers because SPL custody is an
adapter boundary, not an integer hidden inside a claim program. Signature/CPI
authentication, canonical Core-account decoding, fixed-width arithmetic,
account persistence, atomic cross-program rollback, and executable release
identity remain explicit adapter obligations.
-/

namespace DClutch.Dealer

open DClutch

inductive Side where
  | takerBuys
  | takerSells
  deriving DecidableEq, Repr

/-- One constant-price interval in an immutable quote curve. -/
structure Band where
  capacity : Nat
  priceNumerator : Nat
  deriving DecidableEq, Repr

/-- Per-outcome bid and ask curves.  Prices use the Policy's sole scale. -/
structure OutcomeCurve where
  bids : List Band
  asks : List Band
  deriving DecidableEq, Repr

def nondecreasingPrices : List Band → Bool
  | [] | [_] => true
  | left :: right :: rest =>
      left.priceNumerator ≤ right.priceNumerator &&
        nondecreasingPrices (right :: rest)

def nonincreasingPrices : List Band → Bool
  | [] | [_] => true
  | left :: right :: rest =>
      right.priceNumerator ≤ left.priceNumerator &&
        nonincreasingPrices (right :: rest)

def bandsValid (scale : Nat) (bands : List Band) : Bool :=
  !bands.isEmpty && bands.all fun band =>
    0 < band.capacity && 0 < band.priceNumerator && band.priceNumerator ≤ scale

def spreadValid (curve : OutcomeCurve) : Bool :=
  curve.bids.all fun bid =>
    curve.asks.all fun ask => bid.priceNumerator ≤ ask.priceNumerator

def OutcomeCurve.valid (scale : Nat) (curve : OutcomeCurve) : Bool :=
  bandsValid scale curve.bids && bandsValid scale curve.asks &&
    nonincreasingPrices curve.bids && nondecreasingPrices curve.asks &&
    spreadValid curve

def bandsFor (curve : OutcomeCurve) : Side → List Band
  | .takerBuys => curve.asks
  | .takerSells => curve.bids

/-- Exact unrounded numerator for the first `quantity` units of a finite
curve.  Quantity past the curve has no implicit extrapolation. -/
def prefixNumerator : List Band → Nat → Nat
  | [], _ => 0
  | _, 0 => 0
  | band :: rest, quantity =>
      if quantity ≤ band.capacity then quantity * band.priceNumerator
      else band.capacity * band.priceNumerator +
        prefixNumerator rest (quantity - band.capacity)

def curveCapacity (bands : List Band) : Nat :=
  bands.map Band.capacity |>.sum

def ceilDiv (numerator denominator : Nat) : Nat :=
  (numerator + denominator - 1) / denominator

/-- Cumulative amount owed at a curve coordinate.  Ask debits round upward;
bid proceeds round downward.  Incremental fills are differences between these
cumulative values, so splitting a fill cannot repeatedly cross a rounding
boundary. -/
def cumulativeQuote (side : Side) (scale : Nat) (bands : List Band)
    (quantity : Nat) : Nat :=
  match side with
  | .takerBuys => ceilDiv (prefixNumerator bands quantity) scale
  | .takerSells => prefixNumerator bands quantity / scale

def incrementalQuote (side : Side) (scale : Nat) (bands : List Band)
    (used paid quantity : Nat) : Nat :=
  cumulativeQuote side scale bands (used + quantity) - paid

/-- Fragmentation independence is a consequence of paying a cumulative
liability, not a per-instruction fee.  The hypotheses are the executable
state invariant (`paid = due`) and monotonicity checked before subtraction. -/
theorem cumulative_quote_fragmentation_independent
    (side : Side) (scale : Nat) (bands : List Band)
    (start first second paid : Nat)
    (paidExact : paid = cumulativeQuote side scale bands start)
    (firstMonotone : paid ≤ cumulativeQuote side scale bands (start + first))
    (secondMonotone : cumulativeQuote side scale bands (start + first) ≤
      cumulativeQuote side scale bands (start + first + second)) :
    incrementalQuote side scale bands start paid first +
      incrementalQuote side scale bands (start + first)
        (cumulativeQuote side scale bands (start + first)) second =
      incrementalQuote side scale bands start paid (first + second) := by
  simp only [incrementalQuote]
  have endCoordinate : start + (first + second) = start + first + second := by omega
  rw [endCoordinate]
  omega

/-- Immutable policy selected by the Market capability.  Liveness and fees
have distinct owners and custody accounts. -/
structure Policy where
  marketId : Nat
  releaseSetId : Nat
  dealerId : Nat
  feeRecipientId : Nat
  unwindRecipientId : Nat
  outcomeCount : Nat
  quoteScale : Nat
  feeNumerator : Nat
  feeDenominator : Nat
  minimumWorkFunding : Nat
  replacementDelay : Nat
  scalarLimit : Nat
  deriving DecidableEq, Repr

def Policy.valid (policy : Policy) : Bool :=
  policy.marketId != 0 && policy.releaseSetId != 0 && policy.dealerId != 0 &&
    policy.feeRecipientId != 0 && policy.unwindRecipientId != 0 &&
    0 < policy.outcomeCount &&
    0 < policy.quoteScale && policy.quoteScale < policy.scalarLimit &&
    0 < policy.feeDenominator && policy.feeDenominator < policy.scalarLimit &&
    policy.feeNumerator ≤ policy.feeDenominator &&
    0 < policy.minimumWorkFunding && 0 < policy.replacementDelay &&
    0 < policy.scalarLimit

/-- A signed immutable liquidity release candidate.  `workFunding` is present
capital, never anticipated fee revenue. -/
structure Candidate where
  candidateId : Nat
  revision : Nat
  validFrom : Nat
  expiresAt : Nat
  curves : List OutcomeCurve
  minimumInventory : List Nat
  maximumInventory : List Nat
  quoteReserveFloor : Nat
  workFunding : Nat
  workReward : Nat
  deriving DecidableEq, Repr

def allIndices (count : Nat) (predicate : Nat → Bool) : Bool :=
  (List.range count).all predicate

def valueAt (values : List Nat) (index : Nat) : Nat :=
  values[index]?.getD 0

def setAt (values : List Nat) (index value : Nat) : List Nat :=
  values.set index value

def Candidate.validFor (policy : Policy) (candidate : Candidate) : Bool :=
  candidate.candidateId != 0 && candidate.revision != 0 &&
    candidate.validFrom < candidate.expiresAt &&
    candidate.curves.length = policy.outcomeCount &&
    candidate.minimumInventory.length = policy.outcomeCount &&
    candidate.maximumInventory.length = policy.outcomeCount &&
    candidate.curves.all (OutcomeCurve.valid policy.quoteScale) &&
    (allIndices policy.outcomeCount fun outcome =>
      valueAt candidate.minimumInventory outcome ≤
        valueAt candidate.maximumInventory outcome &&
      valueAt candidate.maximumInventory outcome < policy.scalarLimit) &&
    policy.minimumWorkFunding ≤ candidate.workFunding &&
    0 < candidate.workReward && candidate.workReward ≤ candidate.workFunding &&
    candidate.workFunding < policy.scalarLimit &&
    candidate.quoteReserveFloor < policy.scalarLimit

def Candidate.curveAt (candidate : Candidate) (outcome : Nat) : OutcomeCurve :=
  candidate.curves[outcome]?.getD { bids := [], asks := [] }

def feeDue (policy : Policy) (base : Nat) : Nat :=
  ceilDiv (base * policy.feeNumerator) policy.feeDenominator

def incrementalFee (policy : Policy) (base paid increment : Nat) : Nat :=
  feeDue policy (base + increment) - paid

theorem cumulative_fee_fragmentation_independent
    (policy : Policy) (start first second paid : Nat)
    (paidExact : paid = feeDue policy start)
    (firstMonotone : paid ≤ feeDue policy (start + first))
    (secondMonotone : feeDue policy (start + first) ≤
      feeDue policy (start + first + second)) :
    incrementalFee policy start paid first +
      incrementalFee policy (start + first) (feeDue policy (start + first)) second =
      incrementalFee policy start paid (first + second) := by
  simp only [incrementalFee]
  have endCoordinate : start + (first + second) = start + first + second := by omega
  rw [endCoordinate]
  omega

inductive Phase where
  | open
  | terminal (winner : Nat)
  | retired
  deriving DecidableEq, Repr

/-- Persistent Dealer projection.  Claim accounts remain authoritative; the
inventory vector is an atomically checked risk projection linked to each
EconomicKernel transition. -/
structure State where
  phase : Phase
  active : Candidate
  pending : Option Candidate
  inventory : List Nat
  buyUsed : List Nat
  sellUsed : List Nat
  buyQuotePaid : List Nat
  sellQuotePaid : List Nat
  feeBase : Nat
  feePaid : Nat
  quoteCustody : Nat
  feeCustody : Nat
  livenessCustody : Nat
  activeWorkRemaining : Nat
  pendingWorkFunding : Nat
  deriving DecidableEq, Repr

def vectorWidth (count : Nat) (values : List Nat) : Bool :=
  values.length = count

def vectorBounded (limit : Nat) (values : List Nat) : Bool :=
  values.all fun value => value < limit

def inventoryWithin (candidate : Candidate) (inventory : List Nat) : Bool :=
  inventory.length = candidate.minimumInventory.length &&
    inventory.length = candidate.maximumInventory.length &&
    allIndices inventory.length fun outcome =>
      valueAt candidate.minimumInventory outcome ≤ valueAt inventory outcome &&
      valueAt inventory outcome ≤ valueAt candidate.maximumInventory outcome

def quotePaidExact (policy : Policy) (state : State) : Bool :=
  allIndices policy.outcomeCount fun outcome =>
    let curve := state.active.curveAt outcome
    valueAt state.buyQuotePaid outcome =
      cumulativeQuote .takerBuys policy.quoteScale curve.asks
        (valueAt state.buyUsed outcome) &&
    valueAt state.sellQuotePaid outcome =
      cumulativeQuote .takerSells policy.quoteScale curve.bids
        (valueAt state.sellUsed outcome)

def useWithinCurves (policy : Policy) (state : State) : Bool :=
  allIndices policy.outcomeCount fun outcome =>
    let curve := state.active.curveAt outcome
    valueAt state.buyUsed outcome ≤ curveCapacity curve.asks &&
    valueAt state.sellUsed outcome ≤ curveCapacity curve.bids

def pendingValid (policy : Policy) (state : State) : Bool :=
  match state.pending with
  | none => state.pendingWorkFunding = 0
  | some pending =>
      pending.validFor policy && state.active.revision < pending.revision &&
      state.pendingWorkFunding = pending.workFunding

def phaseValid (policy : Policy) (state : State) : Bool :=
  match state.phase with
  | .open =>
      inventoryWithin state.active state.inventory &&
      state.active.quoteReserveFloor ≤ state.quoteCustody
  | .terminal winner => winner < policy.outcomeCount
  | .retired =>
      state.pending.isNone && state.inventory.all (fun quantity => quantity = 0) &&
      state.quoteCustody = 0 && state.feeCustody = 0 &&
      state.livenessCustody = 0 && state.activeWorkRemaining = 0 &&
      state.pendingWorkFunding = 0

def feeCustodyValid (state : State) : Bool :=
  match state.phase with
  | .retired => state.feeCustody = 0
  | _ => state.feeCustody = state.feePaid

/-- Exhaustive executable state invariant. -/
def valid (policy : Policy) (state : State) : Bool :=
  policy.valid && state.active.validFor policy &&
    vectorWidth policy.outcomeCount state.inventory &&
    vectorWidth policy.outcomeCount state.buyUsed &&
    vectorWidth policy.outcomeCount state.sellUsed &&
    vectorWidth policy.outcomeCount state.buyQuotePaid &&
    vectorWidth policy.outcomeCount state.sellQuotePaid &&
    vectorBounded policy.scalarLimit state.inventory &&
    vectorBounded policy.scalarLimit state.buyUsed &&
    vectorBounded policy.scalarLimit state.sellUsed &&
    vectorBounded policy.scalarLimit state.buyQuotePaid &&
    vectorBounded policy.scalarLimit state.sellQuotePaid &&
    useWithinCurves policy state && quotePaidExact policy state &&
    state.feeBase < policy.scalarLimit && state.feePaid < policy.scalarLimit &&
    state.quoteCustody < policy.scalarLimit && state.feeCustody < policy.scalarLimit &&
    state.livenessCustody < policy.scalarLimit &&
    state.activeWorkRemaining < policy.scalarLimit &&
    state.pendingWorkFunding < policy.scalarLimit &&
    state.feePaid = feeDue policy state.feeBase &&
    feeCustodyValid state &&
    state.activeWorkRemaining ≤ state.active.workFunding &&
    pendingValid policy state &&
    state.livenessCustody = state.activeWorkRemaining + state.pendingWorkFunding &&
    phaseValid policy state

/-! ## Authenticated commands and physical plans -/

structure Replacement where
  authenticatedDealerId : Nat
  now : Nat
  candidate : Candidate
  fundingDeposit : Nat
  deriving DecidableEq, Repr

structure Activation where
  now : Nat
  deriving DecidableEq, Repr

structure Fill where
  now : Nat
  expectedCandidateId : Nat
  expectedRevision : Nat
  side : Side
  outcome : Nat
  quantity : Nat
  economic : DClutch.Economic.Frame
  deriving DecidableEq, Repr

structure Resolution where
  coreMarketId : Nat
  releaseSetId : Nat
  winner : Nat
  deriving DecidableEq, Repr

structure Unwind where
  outcome : Nat
  quantity : Nat
  economic : DClutch.Economic.Frame
  deriving DecidableEq, Repr

/-- Owner-authorized capital adjustment. `outcome = none` selects quote
principal; a present outcome selects one native-claim coordinate. -/
structure LiquidityChange where
  authenticatedDealerId : Nat
  outcome : Option Nat
  quantity : Nat
  economic : Option DClutch.Economic.Frame
  deriving DecidableEq, Repr

inductive Command where
  | scheduleReplacement (replacement : Replacement)
  | activateReplacement (activation : Activation)
  | fill (fill : Fill)
  | enterTerminal (resolution : Resolution)
  | unwind (unwind : Unwind)
  | retire
  | addLiquidity (change : LiquidityChange)
  | removeLiquidity (change : LiquidityChange)
  deriving DecidableEq, Repr

/-- One current Registry/Core observation around a semantic command.  The
Dealer machine consumes the shared capability-neutral release interface; it
does not own or restate Loader authority rules. -/
structure Invocation where
  release : DClutch.ExecutionRelease.Admission
  command : Command
  deriving DecidableEq, Repr

inductive CustodyRole where
  | dealerQuote
  | takerQuote
  | feeVault
  | livenessVault
  | executor
  | dealerOwner
  | feeRecipient
  | unwindRecipient
  deriving DecidableEq, Repr

structure CustodyTransfer where
  source : CustodyRole
  destination : CustodyRole
  amount : Nat
  deriving DecidableEq, Repr

def custodyMove (source destination : CustodyRole) (amount : Nat) : CustodyTransfer :=
  { source, destination, amount }

structure PhysicalPlan where
  economic : Option DClutch.Direct.Physical.PhysicalPlan
  custody : List CustodyTransfer
  deriving DecidableEq, Repr

def economicBindings (side : Side) : DClutch.Economic.Bindings :=
  match side with
  | .takerBuys => { source := .seller, destination := .buyer, hoard := .venue }
  | .takerSells => { source := .buyer, destination := .seller, hoard := .venue }

def dealerHolder (side : Side) : DClutch.Economic.Holder :=
  match side with
  | .takerBuys => .source
  | .takerSells => .destination

def dealerClaims (side : Side) (economic : DClutch.Economic.State) : List Nat :=
  economic.holderClaims (dealerHolder side) .native

def inventoryAfter (side : Side) (inventory : List Nat)
    (outcome quantity : Nat) : List Nat :=
  match side with
  | .takerBuys => setAt inventory outcome (valueAt inventory outcome - quantity)
  | .takerSells => setAt inventory outcome (valueAt inventory outcome + quantity)

def usedVector (state : State) : Side → List Nat
  | .takerBuys => state.buyUsed
  | .takerSells => state.sellUsed

def paidVector (state : State) : Side → List Nat
  | .takerBuys => state.buyQuotePaid
  | .takerSells => state.sellQuotePaid

def grossQuote (policy : Policy) (state : State) (fill : Fill) : Nat :=
  let curve := bandsFor (state.active.curveAt fill.outcome) fill.side
  incrementalQuote fill.side policy.quoteScale curve
    (valueAt (usedVector state fill.side) fill.outcome)
    (valueAt (paidVector state fill.side) fill.outcome) fill.quantity

def fillFee (policy : Policy) (state : State) (fill : Fill) : Nat :=
  incrementalFee policy state.feeBase state.feePaid (grossQuote policy state fill)

def economicFillAccepts (policy : Policy) (state : State) (fill : Fill) : Bool :=
  fill.economic.outcomeCount = policy.outcomeCount &&
    fill.economic.scalarLimit = policy.scalarLimit &&
    fill.economic.bindings = economicBindings fill.side &&
    fill.economic.command =
      DClutch.Economic.Command.transferClaim .native fill.outcome fill.quantity &&
    dealerClaims fill.side fill.economic.pre = state.inventory &&
    DClutch.Economic.accepts fill.economic &&
    dealerClaims fill.side (DClutch.Economic.postState fill.economic) =
      inventoryAfter fill.side state.inventory fill.outcome fill.quantity

def fillAccepts (policy : Policy) (state : State) (fill : Fill) : Bool :=
  state.phase = .open && fill.now < state.active.expiresAt &&
    fill.expectedCandidateId = state.active.candidateId &&
    fill.expectedRevision = state.active.revision &&
    0 < fill.quantity && fill.outcome < policy.outcomeCount &&
    let curve := bandsFor (state.active.curveAt fill.outcome) fill.side
    let used := valueAt (usedVector state fill.side) fill.outcome
    let paid := valueAt (paidVector state fill.side) fill.outcome
    used + fill.quantity ≤ curveCapacity curve &&
    paid ≤ cumulativeQuote fill.side policy.quoteScale curve (used + fill.quantity) &&
    0 < grossQuote policy state fill &&
    state.feePaid ≤ feeDue policy (state.feeBase + grossQuote policy state fill) &&
    state.activeWorkRemaining ≥ state.active.workReward &&
    inventoryWithin state.active
      (inventoryAfter fill.side state.inventory fill.outcome fill.quantity) &&
    (match fill.side with
      | .takerBuys =>
          state.quoteCustody + grossQuote policy state fill < policy.scalarLimit
      | .takerSells =>
          fillFee policy state fill ≤ grossQuote policy state fill &&
          grossQuote policy state fill ≤ state.quoteCustody &&
          state.active.quoteReserveFloor ≤
            state.quoteCustody - grossQuote policy state fill) &&
    economicFillAccepts policy state fill

def replacementAccepts (policy : Policy) (state : State)
    (replacement : Replacement) : Bool :=
  state.phase = .open && replacement.authenticatedDealerId = policy.dealerId &&
    replacement.candidate.validFor policy &&
    state.active.revision < replacement.candidate.revision &&
    replacement.now + policy.replacementDelay ≤ replacement.candidate.validFrom &&
    replacement.fundingDeposit = replacement.candidate.workFunding &&
    (match state.pending with
      | none => true
      | some pending => pending.revision < replacement.candidate.revision) &&
    state.livenessCustody - state.pendingWorkFunding +
      replacement.fundingDeposit < policy.scalarLimit

def activationAccepts (policy : Policy) (state : State)
    (activation : Activation) : Bool :=
  state.phase = .open && match state.pending with
  | none => false
  | some pending =>
      activation.now ≥ pending.validFrom && activation.now < pending.expiresAt &&
      pending.validFor policy && inventoryWithin pending state.inventory &&
      pending.quoteReserveFloor ≤ state.quoteCustody

def resolutionAccepts (policy : Policy) (state : State)
    (resolution : Resolution) : Bool :=
  state.phase = .open &&
    resolution.coreMarketId = policy.marketId &&
    resolution.releaseSetId = policy.releaseSetId &&
    resolution.winner < policy.outcomeCount

def unwindEconomicAccepts (policy : Policy) (state : State)
    (unwind : Unwind) : Bool :=
  match state.phase with
  | .terminal winner =>
      unwind.economic.outcomeCount = policy.outcomeCount &&
      unwind.economic.scalarLimit = policy.scalarLimit &&
      unwind.economic.bindings =
        { source := .seller, destination := .buyer, hoard := .venue } &&
      unwind.economic.pre.phase = .retiring winner &&
      unwind.economic.command =
        DClutch.Economic.Command.redeemTerminal .source .native
          unwind.outcome unwind.quantity &&
      unwind.economic.pre.sourceNative = state.inventory &&
      DClutch.Economic.accepts unwind.economic &&
      (DClutch.Economic.postState unwind.economic).sourceNative =
        setAt state.inventory unwind.outcome
          (valueAt state.inventory unwind.outcome - unwind.quantity)
  | _ => false

def unwindAccepts (policy : Policy) (state : State) (unwind : Unwind) : Bool :=
  0 < unwind.quantity && unwind.outcome < policy.outcomeCount &&
    unwind.quantity ≤ valueAt state.inventory unwind.outcome &&
    state.activeWorkRemaining ≥ state.active.workReward &&
    state.quoteCustody +
      DClutch.Economic.redemptionPayout unwind.economic.pre.phase
        unwind.outcome unwind.quantity < policy.scalarLimit &&
    unwindEconomicAccepts policy state unwind

def liquiditySide (add : Bool) : Side :=
  if add then .takerSells else .takerBuys

def liquidityInventoryAfter (add : Bool) (state : State)
    (outcome quantity : Nat) : List Nat :=
  inventoryAfter (liquiditySide add) state.inventory outcome quantity

def liquidityEconomicAccepts (policy : Policy) (state : State) (add : Bool)
    (outcome quantity : Nat) : Option DClutch.Economic.Frame → Bool
  | none => false
  | some economic =>
      economic.outcomeCount = policy.outcomeCount &&
        economic.scalarLimit = policy.scalarLimit &&
        economic.bindings = economicBindings (liquiditySide add) &&
        economic.command =
          DClutch.Economic.Command.transferClaim .native outcome quantity &&
        dealerClaims (liquiditySide add) economic.pre = state.inventory &&
        DClutch.Economic.accepts economic &&
        dealerClaims (liquiditySide add) (DClutch.Economic.postState economic) =
          liquidityInventoryAfter add state outcome quantity

def liquidityChangeAccepts (policy : Policy) (state : State) (add : Bool)
    (change : LiquidityChange) : Bool :=
  state.phase = .open && change.authenticatedDealerId = policy.dealerId &&
    0 < change.quantity && match change.outcome with
    | none =>
        change.economic.isNone && if add then
          state.quoteCustody + change.quantity < policy.scalarLimit
        else
          change.quantity ≤ state.quoteCustody &&
            state.active.quoteReserveFloor ≤ state.quoteCustody - change.quantity
    | some outcome =>
        outcome < policy.outcomeCount &&
          inventoryWithin state.active
            (liquidityInventoryAfter add state outcome change.quantity) &&
          liquidityEconomicAccepts policy state add outcome change.quantity change.economic

def commandAccepts (policy : Policy) (state : State) : Command → Bool
  | .scheduleReplacement replacement => replacementAccepts policy state replacement
  | .activateReplacement activation => activationAccepts policy state activation
  | .fill fill => fillAccepts policy state fill
  | .enterTerminal resolution => resolutionAccepts policy state resolution
  | .unwind unwind => unwindAccepts policy state unwind
  | .retire =>
      (match state.phase with | .terminal _ => true | _ => false) &&
      state.pending.isNone && state.inventory.all (fun quantity => quantity = 0)
  | .addLiquidity change => liquidityChangeAccepts policy state true change
  | .removeLiquidity change => liquidityChangeAccepts policy state false change

def releaseAccepts (policy : Policy) (invocation : Invocation) : Bool :=
  invocation.release.marketReleaseSetId = policy.releaseSetId &&
    DClutch.ExecutionRelease.admits invocation.release .trading

def accepts (policy : Policy) (state : State) (invocation : Invocation) : Bool :=
  valid policy state && releaseAccepts policy invocation &&
    commandAccepts policy state invocation.command

def zeroVector (count : Nat) : List Nat := List.replicate count 0

def schedulePost (state : State) (replacement : Replacement) : State :=
  let oldPendingFunding := state.pendingWorkFunding
  { state with
    pending := some replacement.candidate
    pendingWorkFunding := replacement.fundingDeposit
    livenessCustody := state.livenessCustody - oldPendingFunding +
      replacement.fundingDeposit }

def activatePost (policy : Policy) (state : State) : State :=
  match state.pending with
  | none => state
  | some pending => {
      state with
      active := pending
      pending := none
      buyUsed := zeroVector policy.outcomeCount
      sellUsed := zeroVector policy.outcomeCount
      buyQuotePaid := zeroVector policy.outcomeCount
      sellQuotePaid := zeroVector policy.outcomeCount
      activeWorkRemaining := state.pendingWorkFunding
      pendingWorkFunding := 0
      livenessCustody := state.pendingWorkFunding }

def fillPost (policy : Policy) (state : State) (fill : Fill) : State :=
  let gross := grossQuote policy state fill
  let fee := fillFee policy state fill
  let usedAfter := setAt (usedVector state fill.side) fill.outcome
    (valueAt (usedVector state fill.side) fill.outcome + fill.quantity)
  let paidAfter := setAt (paidVector state fill.side) fill.outcome
    (valueAt (paidVector state fill.side) fill.outcome + gross)
  let base := state.feeBase + gross
  let common := {
    state with
    inventory := inventoryAfter fill.side state.inventory fill.outcome fill.quantity
    feeBase := base
    feePaid := state.feePaid + fee
    feeCustody := state.feeCustody + fee
    livenessCustody := state.livenessCustody - state.active.workReward
    activeWorkRemaining := state.activeWorkRemaining - state.active.workReward }
  match fill.side with
  | .takerBuys => {
      common with
      buyUsed := usedAfter
      buyQuotePaid := paidAfter
      quoteCustody := state.quoteCustody + gross }
  | .takerSells => {
      common with
      sellUsed := usedAfter
      sellQuotePaid := paidAfter
      quoteCustody := state.quoteCustody - gross }

def terminalPost (state : State) (winner : Nat) : State :=
  { state with
    phase := .terminal winner
    pending := none
    livenessCustody := state.activeWorkRemaining
    pendingWorkFunding := 0 }

def unwindPost (state : State) (unwind : Unwind) : State :=
  let payout := DClutch.Economic.redemptionPayout unwind.economic.pre.phase
    unwind.outcome unwind.quantity
  { state with
    inventory := setAt state.inventory unwind.outcome
      (valueAt state.inventory unwind.outcome - unwind.quantity)
    quoteCustody := state.quoteCustody + payout
    livenessCustody := state.livenessCustody - state.active.workReward
    activeWorkRemaining := state.activeWorkRemaining - state.active.workReward }

def retirePost (state : State) : State :=
  { state with
    phase := .retired
    quoteCustody := 0
    feeCustody := 0
    livenessCustody := 0
    activeWorkRemaining := 0 }

def liquidityPost (state : State) (add : Bool) (change : LiquidityChange) : State :=
  match change.outcome with
  | none => if add then { state with quoteCustody := state.quoteCustody + change.quantity }
      else { state with quoteCustody := state.quoteCustody - change.quantity }
  | some outcome =>
      { state with inventory := liquidityInventoryAfter add state outcome change.quantity }

def postState (policy : Policy) (state : State) : Command → State
  | .scheduleReplacement replacement => schedulePost state replacement
  | .activateReplacement _ => activatePost policy state
  | .fill fill => fillPost policy state fill
  | .enterTerminal resolution => terminalPost state resolution.winner
  | .unwind unwind => unwindPost state unwind
  | .retire => retirePost state
  | .addLiquidity change => liquidityPost state true change
  | .removeLiquidity change => liquidityPost state false change

def rawCustodyPlan (policy : Policy) (state : State) : Command → List CustodyTransfer
  | .scheduleReplacement replacement =>
      let refund : List CustodyTransfer := match state.pending with
        | none => []
        | some _ => [custodyMove .livenessVault .dealerOwner state.pendingWorkFunding]
      refund ++ [custodyMove .dealerOwner .livenessVault replacement.fundingDeposit]
  | .activateReplacement _ =>
      if state.activeWorkRemaining = 0 then [] else
        [custodyMove .livenessVault .dealerOwner state.activeWorkRemaining]
  | .fill fill =>
      let gross := grossQuote policy state fill
      let fee := fillFee policy state fill
      let trade := match fill.side with
        | .takerBuys => [
            custodyMove .takerQuote .dealerQuote gross,
            custodyMove .takerQuote .feeVault fee]
        | .takerSells => [
            custodyMove .dealerQuote .takerQuote (gross - fee),
            custodyMove .dealerQuote .feeVault fee]
      trade ++ [custodyMove .livenessVault .executor state.active.workReward]
  | .enterTerminal _ =>
      if state.pendingWorkFunding = 0 then [] else
        [custodyMove .livenessVault .dealerOwner state.pendingWorkFunding]
  | .unwind _ => [custodyMove .livenessVault .executor state.active.workReward]
  | .retire => [
      custodyMove .dealerQuote .unwindRecipient state.quoteCustody,
      custodyMove .feeVault .feeRecipient state.feeCustody,
      custodyMove .livenessVault .dealerOwner state.activeWorkRemaining]
  | .addLiquidity change => match change.outcome with
      | none => [custodyMove .dealerOwner .dealerQuote change.quantity]
      | some _ => []
  | .removeLiquidity change => match change.outcome with
      | none => [custodyMove .dealerQuote .dealerOwner change.quantity]
      | some _ => []

/-- Zero-value token CPIs are not part of the semantic plan. -/
def custodyPlan (policy : Policy) (state : State) (command : Command) :
    List CustodyTransfer :=
  (rawCustodyPlan policy state command).filter fun transfer => transfer.amount != 0

def physicalPlan (policy : Policy) (state : State) (command : Command) : PhysicalPlan := {
  economic := match command with
    | .fill fill => some (DClutch.Economic.compile fill.economic)
    | .unwind unwind => some (DClutch.Economic.compile unwind.economic)
    | .addLiquidity change | .removeLiquidity change =>
        change.economic.map DClutch.Economic.compile
    | _ => none
  custody := custodyPlan policy state command
}

inductive Refusal where
  | notAdmissible
  | postInvariantFailure
  deriving DecidableEq, Repr

structure Settlement (policy : Policy) (pre : State) (invocation : Invocation) where
  post : State
  plan : PhysicalPlan
  admitted : accepts policy pre invocation = true
  postExact : post = postState policy pre invocation.command
  planExact : plan = physicalPlan policy pre invocation.command
  postValid : valid policy post = true

/-- The single total transition boundary for all finite Product widths. -/
def execute? (policy : Policy) (pre : State) (invocation : Invocation) :
    Except Refusal (Settlement policy pre invocation) :=
  if admitted : accepts policy pre invocation = true then
    let post := postState policy pre invocation.command
    if postValid : valid policy post = true then
      .ok {
        post
        plan := physicalPlan policy pre invocation.command
        admitted
        postExact := rfl
        planExact := rfl
        postValid
      }
    else .error .postInvariantFailure
  else .error .notAdmissible

def run (policy : Policy) (pre : State) (invocation : Invocation) : State :=
  match execute? policy pre invocation with
  | .ok settlement => settlement.post
  | .error _ => pre

theorem refusal_rolls_back
    (policy : Policy) (pre : State) (invocation : Invocation) (refusal : Refusal)
    (failed : execute? policy pre invocation = .error refusal) :
    run policy pre invocation = pre := by
  unfold run
  rw [failed]

theorem successful_transition_revalidates
    (policy : Policy) (pre : State) (invocation : Invocation)
    (settlement : Settlement policy pre invocation) :
    valid policy settlement.post = true := settlement.postValid

theorem successful_transition_plan_is_exact
    (policy : Policy) (pre : State) (invocation : Invocation)
    (settlement : Settlement policy pre invocation) :
    settlement.plan = physicalPlan policy pre invocation.command := settlement.planExact

theorem admitted_invocation_uses_selected_trading_release
    (policy : Policy) (state : State) (invocation : Invocation)
    (accepted : accepts policy state invocation = true) :
    invocation.release.marketReleaseSetId = policy.releaseSetId ∧
      DClutch.ExecutionRelease.admits invocation.release .trading = true := by
  simp only [accepts, releaseAccepts, Bool.and_eq_true, decide_eq_true_eq] at accepted
  exact accepted.1.2

theorem fill_uses_shared_economic_kernel
    (policy : Policy) (state : State) (fill : Fill) :
    (physicalPlan policy state (.fill fill)).economic =
      some (DClutch.Economic.compile fill.economic) := rfl

theorem terminal_unwind_uses_shared_economic_kernel
    (policy : Policy) (state : State) (unwind : Unwind) :
    (physicalPlan policy state (.unwind unwind)).economic =
      some (DClutch.Economic.compile unwind.economic) := rfl

/-- Dealer terminalization has no independently selected resolver authority:
every admitted terminal observation is bound to the immutable logical Market
and execution release selected by the Dealer policy. The adapter obligation is
to derive this observation from the authenticated canonical Core account. -/
theorem admitted_terminal_is_core_market_bound
    (policy : Policy) (state : State) (resolution : Resolution)
    (accepted : resolutionAccepts policy state resolution = true) :
    resolution.coreMarketId = policy.marketId ∧
      resolution.releaseSetId = policy.releaseSetId := by
  simp only [resolutionAccepts, Bool.and_eq_true, decide_eq_true_eq] at accepted
  exact ⟨accepted.1.1.2, accepted.1.2⟩

theorem successful_replacement_resets_curve_coordinates
    (policy : Policy) (state : State) (activation : Activation)
    (pending : Candidate) (hasPending : state.pending = some pending) :
    (postState policy state (.activateReplacement activation)).active = pending ∧
    (postState policy state (.activateReplacement activation)).buyUsed =
      zeroVector policy.outcomeCount ∧
    (postState policy state (.activateReplacement activation)).sellUsed =
      zeroVector policy.outcomeCount := by
  simp [postState, activatePost, hasPending]

theorem retirement_empties_distinct_custodies
    (policy : Policy) (state : State) :
    let post := postState policy state .retire
    post.quoteCustody = 0 ∧ post.feeCustody = 0 ∧
      post.livenessCustody = 0 := by
  simp [postState, retirePost]

/-- Realized fees are not reclassified as liquidity principal by either
capital-adjustment route. -/
theorem liquidity_change_preserves_realized_fees
    (state : State) (add : Bool) (change : LiquidityChange) :
    (liquidityPost state add change).feeCustody = state.feeCustody ∧
      (liquidityPost state add change).feeBase = state.feeBase := by
  rcases change with ⟨authenticatedDealerId, outcome, quantity, economic⟩
  cases outcome <;> cases add <;> simp [liquidityPost]

end DClutch.Dealer
