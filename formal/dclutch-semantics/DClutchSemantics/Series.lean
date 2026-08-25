import DClutchSemantics.ExecutionRelease
import DClutchSemantics.EconomicKernel
import Std.Tactic

/-!
# Recurring Series semantic specialization

Series is immutable schedule data plus one replay-owned cursor.  Each occurrence
is represented by an independently prepaid ticket that commits its eventual
Market identity, founder, refund owner, seed collateral, account rent, and
permissionless founding work.  Consuming a ticket atomically advances the
Series, drains only those named compartments, initializes one Market economic
state through the shared economic kernel, and marks the ticket consumed.

Registry admission, the ticket/Series/Market account projection, CPI, PDA
derivation, rent values, signatures, persistence, and transaction rollback are
adapter boundaries.  A failed physical attempt is modeled as an observable
refusal with complete semantic rollback, leaving the same ticket retryable
until its immutable deadline.  There is no mock authority path.
-/

namespace DClutch.Series

open DClutch

abbrev Identity := ExecutionRelease.Identity

/-! ## Immutable schedule and exact prepaid custody -/

/-- One content-addressed recurring-market template.  Every bound is data;
there are no width-specialized transition functions. -/
structure Template where
  templateId : Identity
  realmId : Identity
  productId : Identity
  releaseSetId : Identity
  outcomeCount : Nat
  firstOccurrenceSlot : Nat
  periodSlots : Nat
  occurrenceCount : Nat
  retryWindowSlots : Nat
  seedQuantity : Nat
  marketRentLamports : Nat
  capabilityRentLamports : Nat
  foundingWorkLamports : Nat
  seriesCloseRentLamports : Nat
  seriesRefundOwner : Identity
  deriving DecidableEq, Repr

/-- The four custody compartments held by one fresh ticket.  Hoard principal is
separate from rent and work funding by construction. -/
structure TicketFunds where
  hoardPrincipal : Nat
  marketRent : Nat
  capabilityRent : Nat
  foundingWork : Nat
  deriving DecidableEq, Repr

def TicketFunds.total (funds : TicketFunds) : Nat :=
  funds.hoardPrincipal + funds.marketRent +
    funds.capabilityRent + funds.foundingWork

def requiredFunds (template : Template) : TicketFunds := {
  hoardPrincipal := template.seedQuantity
  marketRent := template.marketRentLamports
  capabilityRent := template.capabilityRentLamports
  foundingWork := template.foundingWorkLamports
}

def fundsZero (funds : TicketFunds) : Bool :=
  funds.hoardPrincipal = 0 && funds.marketRent = 0 &&
    funds.capabilityRent = 0 && funds.foundingWork = 0

def scheduledSlot (template : Template) (occurrence : Nat) : Nat :=
  template.firstOccurrenceSlot + occurrence * template.periodSlots

def retryThroughSlot (template : Template) (occurrence : Nat) : Nat :=
  scheduledSlot template occurrence + template.retryWindowSlots

/-- Executable template validation.  `slotLimit` and `lamportLimit` are
physical-profile bounds, not mathematical restrictions on Series. -/
def templateValid
    (slotLimit lamportLimit : Nat) (template : Template) : Bool :=
  ExecutionRelease.identityValid template.templateId &&
  ExecutionRelease.identityValid template.realmId &&
  ExecutionRelease.identityValid template.productId &&
  ExecutionRelease.identityValid template.releaseSetId &&
  ExecutionRelease.identityValid template.seriesRefundOwner &&
  0 < template.outcomeCount &&
  0 < template.periodSlots &&
  0 < template.occurrenceCount &&
  0 < template.seedQuantity &&
  template.seedQuantity < lamportLimit &&
  (requiredFunds template).total < lamportLimit &&
  template.seriesCloseRentLamports < lamportLimit &&
  retryThroughSlot template (template.occurrenceCount - 1) < slotLimit

inductive TicketPhase where
  | ready
  | consumed
  | expired
  deriving DecidableEq, Repr

/-- A funded ticket precommits every identity used by consumption. -/
structure Ticket where
  ticketId : Identity
  templateId : Identity
  occurrence : Nat
  founder : Identity
  refundOwner : Identity
  committedMarketId : Identity
  revision : Nat
  phase : TicketPhase
  funds : TicketFunds
  deriving DecidableEq, Repr

inductive Phase where
  | active
  | terminal
  | closed
  deriving DecidableEq, Repr

/-- The Series account owns only its immutable template selection, occurrence
cursor, replay revision, phase, and separately funded close rent. -/
structure State where
  seriesId : Identity
  templateId : Identity
  phase : Phase
  nextOccurrence : Nat
  revision : Nat
  closeRentLamports : Nat
  deriving DecidableEq, Repr

/-- Exact account projection for one bounded transition. -/
structure Snapshot where
  template : Template
  series : State
  ticket : Ticket
  deriving DecidableEq, Repr

def Ticket.final : Ticket -> Bool
  | { phase := .ready, .. } => false
  | { phase := .consumed, .. } | { phase := .expired, .. } => true

def ticketValid (scalarLimit : Nat) (template : Template) (ticket : Ticket) : Bool :=
  ExecutionRelease.identityValid ticket.ticketId &&
  ExecutionRelease.identityValid ticket.founder &&
  ExecutionRelease.identityValid ticket.refundOwner &&
  ExecutionRelease.identityValid ticket.committedMarketId &&
  ticket.templateId = template.templateId &&
  ticket.occurrence < template.occurrenceCount &&
  ticket.revision + 1 < scalarLimit &&
  match ticket.phase with
  | .ready => ticket.funds = requiredFunds template
  | .consumed | .expired => fundsZero ticket.funds

def activeProjectionValid (snapshot : Snapshot) : Bool :=
  snapshot.series.nextOccurrence < snapshot.template.occurrenceCount &&
  snapshot.series.closeRentLamports = snapshot.template.seriesCloseRentLamports &&
  match snapshot.ticket.phase with
  | .ready => snapshot.ticket.occurrence = snapshot.series.nextOccurrence
  | .consumed | .expired => snapshot.ticket.occurrence < snapshot.series.nextOccurrence

def terminalProjectionValid (snapshot : Snapshot) : Bool :=
  snapshot.series.nextOccurrence = snapshot.template.occurrenceCount &&
  snapshot.series.closeRentLamports = snapshot.template.seriesCloseRentLamports &&
  snapshot.ticket.final

def closedProjectionValid (snapshot : Snapshot) : Bool :=
  snapshot.series.nextOccurrence = snapshot.template.occurrenceCount &&
  snapshot.series.closeRentLamports = 0 &&
  snapshot.ticket.final

/-- Complete executable projected-state invariant. -/
def valid
    (slotLimit lamportLimit scalarLimit : Nat) (snapshot : Snapshot) : Bool :=
  templateValid slotLimit lamportLimit snapshot.template &&
  ExecutionRelease.identityValid snapshot.series.seriesId &&
  snapshot.series.templateId = snapshot.template.templateId &&
  snapshot.series.revision + 1 < scalarLimit &&
  ticketValid scalarLimit snapshot.template snapshot.ticket &&
  match snapshot.series.phase with
  | .active => activeProjectionValid snapshot
  | .terminal => terminalProjectionValid snapshot
  | .closed => closedProjectionValid snapshot

/-! ## Shared complete-set founding semantics -/

def emptyEconomicState (outcomeCount : Nat) : Economic.State := {
  phase := .open
  hoard := 0
  supply := List.replicate outcomeCount 0
  nativeSupply := List.replicate outcomeCount 0
  materializedSupply := List.replicate outcomeCount 0
  sourceNative := List.replicate outcomeCount 0
  sourceMaterialized := List.replicate outcomeCount 0
  destinationNative := List.replicate outcomeCount 0
  destinationMaterialized := List.replicate outcomeCount 0
}

def foundingBindings : Economic.Bindings := {
  source := .seller
  destination := .buyer
  hoard := .venue
}

/-- The ticket founder receives one initial native complete set; its exact
collateral comes from the ticket's hoard-principal compartment. -/
def foundingEconomicFrame
    (lamportLimit : Nat) (template : Template) : Economic.Frame := {
  outcomeCount := template.outcomeCount
  scalarLimit := lamportLimit
  bindings := foundingBindings
  pre := emptyEconomicState template.outcomeCount
  command := .splitCompleteSet .destination .native template.seedQuantity
}

/-! ## Transition data and total execution -/

inductive Account where
  | ticketEscrow (ticketId : Identity)
  | seriesEscrow (seriesId : Identity)
  | marketHoard (marketId : Identity)
  | marketAccount (marketId : Identity)
  | capabilityAccounts (marketId : Identity)
  | beneficiary (owner : Identity)
  deriving DecidableEq, Repr

structure CustodyTransfer where
  source : Account
  destination : Account
  amount : Nat
  deriving DecidableEq, Repr

inductive Command where
  | consume (workRecipient : Identity)
  | expire
  | close
  deriving DecidableEq, Repr

/-- `physicalSucceeded` is the normalized atomic adapter outcome.  It is not an
authority bit and cannot make an inadmissible command succeed. -/
structure Frame where
  slotLimit : Nat
  lamportLimit : Nat
  scalarLimit : Nat
  nowSlot : Nat
  expectedSeriesRevision : Nat
  expectedTicketRevision : Nat
  pre : Snapshot
  releaseAdmission : ExecutionRelease.Admission
  physicalSucceeded : Bool
  command : Command
  deriving DecidableEq, Repr

def commonAccepts (frame : Frame) : Bool :=
  valid frame.slotLimit frame.lamportLimit frame.scalarLimit frame.pre &&
  frame.nowSlot < frame.slotLimit &&
  frame.expectedSeriesRevision = frame.pre.series.revision &&
  frame.expectedTicketRevision = frame.pre.ticket.revision &&
  frame.releaseAdmission.marketReleaseSetId = frame.pre.template.releaseSetId &&
  ExecutionRelease.admits frame.releaseAdmission .core

def consumeAccepts (frame : Frame) (workRecipient : Identity) : Bool :=
  frame.pre.series.phase = .active &&
  frame.pre.ticket.phase = .ready &&
  frame.pre.ticket.occurrence = frame.pre.series.nextOccurrence &&
  ExecutionRelease.identityValid workRecipient &&
  scheduledSlot frame.pre.template frame.pre.ticket.occurrence <= frame.nowSlot &&
  frame.nowSlot <= retryThroughSlot frame.pre.template frame.pre.ticket.occurrence &&
  Economic.accepts
    (foundingEconomicFrame frame.lamportLimit frame.pre.template)

def expireAccepts (frame : Frame) : Bool :=
  frame.pre.series.phase = .active &&
  frame.pre.ticket.phase = .ready &&
  frame.pre.ticket.occurrence = frame.pre.series.nextOccurrence &&
  retryThroughSlot frame.pre.template frame.pre.ticket.occurrence < frame.nowSlot

def closeAccepts (frame : Frame) : Bool :=
  frame.pre.series.phase = .terminal &&
  frame.pre.series.nextOccurrence = frame.pre.template.occurrenceCount &&
  frame.pre.ticket.final

def semanticAccepts (frame : Frame) : Bool :=
  commonAccepts frame &&
  match frame.command with
  | .consume workRecipient => consumeAccepts frame workRecipient
  | .expire => expireAccepts frame
  | .close => closeAccepts frame

def zeroFunds : TicketFunds := {
  hoardPrincipal := 0
  marketRent := 0
  capabilityRent := 0
  foundingWork := 0
}

def advanceSeries (template : Template) (series : State) : State :=
  let next := series.nextOccurrence + 1
  { series with
    phase := if next = template.occurrenceCount then .terminal else .active
    nextOccurrence := next
    revision := series.revision + 1 }

def consumePost (pre : Snapshot) : Snapshot := {
  pre with
  series := advanceSeries pre.template pre.series
  ticket := { pre.ticket with
    revision := pre.ticket.revision + 1
    phase := .consumed
    funds := zeroFunds }
}

def expirePost (pre : Snapshot) : Snapshot := {
  pre with
  series := advanceSeries pre.template pre.series
  ticket := { pre.ticket with
    revision := pre.ticket.revision + 1
    phase := .expired
    funds := zeroFunds }
}

def closePost (pre : Snapshot) : Snapshot := {
  pre with
  series := { pre.series with
    phase := .closed
    revision := pre.series.revision + 1
    closeRentLamports := 0 }
}

def postState (frame : Frame) : Snapshot :=
  match frame.command with
  | .consume _ => consumePost frame.pre
  | .expire => expirePost frame.pre
  | .close => closePost frame.pre

def transferIfPositive
    (source destination : Account) (amount : Nat) : List CustodyTransfer :=
  if amount = 0 then [] else [{ source, destination, amount }]

def consumeCustody (pre : Snapshot) (workRecipient : Identity) : List CustodyTransfer :=
  let source := Account.ticketEscrow pre.ticket.ticketId
  transferIfPositive source (.marketHoard pre.ticket.committedMarketId)
      pre.ticket.funds.hoardPrincipal ++
  transferIfPositive source (.marketAccount pre.ticket.committedMarketId)
      pre.ticket.funds.marketRent ++
  transferIfPositive source (.capabilityAccounts pre.ticket.committedMarketId)
      pre.ticket.funds.capabilityRent ++
  transferIfPositive source (.beneficiary workRecipient)
      pre.ticket.funds.foundingWork

def refundCustody (pre : Snapshot) : List CustodyTransfer :=
  let source := Account.ticketEscrow pre.ticket.ticketId
  let destination := Account.beneficiary pre.ticket.refundOwner
  transferIfPositive source destination pre.ticket.funds.hoardPrincipal ++
  transferIfPositive source destination pre.ticket.funds.marketRent ++
  transferIfPositive source destination pre.ticket.funds.capabilityRent ++
  transferIfPositive source destination pre.ticket.funds.foundingWork

def closeCustody (pre : Snapshot) : List CustodyTransfer :=
  transferIfPositive (.seriesEscrow pre.series.seriesId)
    (.beneficiary pre.template.seriesRefundOwner)
    pre.series.closeRentLamports

/-- Exact Market founding commitment emitted by ticket consumption. -/
structure MarketFounding where
  marketId : Identity
  templateId : Identity
  realmId : Identity
  productId : Identity
  releaseSetId : Identity
  occurrence : Nat
  scheduledSlot : Nat
  founder : Identity
  economicState : Economic.State
  claimEffects : EffectPlan
  deriving DecidableEq, Repr

def marketFounding (frame : Frame) : MarketFounding :=
  let economicFrame := foundingEconomicFrame frame.lamportLimit frame.pre.template
  {
    marketId := frame.pre.ticket.committedMarketId
    templateId := frame.pre.template.templateId
    realmId := frame.pre.template.realmId
    productId := frame.pre.template.productId
    releaseSetId := frame.pre.template.releaseSetId
    occurrence := frame.pre.ticket.occurrence
    scheduledSlot := scheduledSlot frame.pre.template frame.pre.ticket.occurrence
    founder := frame.pre.ticket.founder
    economicState := Economic.runState economicFrame
    claimEffects := (Economic.compile economicFrame).claimEffects
  }

structure Plan where
  custodyTransfers : List CustodyTransfer
  market : Option MarketFounding
  deriving DecidableEq, Repr

def compile (frame : Frame) : Plan :=
  match frame.command with
  | .consume workRecipient => {
      custodyTransfers := consumeCustody frame.pre workRecipient
      market := some (marketFounding frame)
    }
  | .expire => {
      custodyTransfers := refundCustody frame.pre
      market := none
    }
  | .close => {
      custodyTransfers := closeCustody frame.pre
      market := none
    }

inductive Refusal where
  | notAdmissible
  | physicalFailure
  | candidateInvariantFailure
  deriving DecidableEq, Repr

structure Settlement (frame : Frame) where
  post : Snapshot
  plan : Plan
  admitted : semanticAccepts frame = true
  physical : frame.physicalSucceeded = true
  postExact : post = postState frame
  planExact : plan = compile frame
  postValid : valid frame.slotLimit frame.lamportLimit frame.scalarLimit post = true

/-- Total atomic boundary.  No candidate or custody plan is exposed on either
semantic or physical refusal. -/
def execute? (frame : Frame) : Except Refusal (Settlement frame) :=
  if admitted : semanticAccepts frame = true then
    if physical : frame.physicalSucceeded = true then
      let candidate := postState frame
      if candidateValid :
          valid frame.slotLimit frame.lamportLimit frame.scalarLimit candidate = true then
        .ok {
          post := candidate
          plan := compile frame
          admitted
          physical
          postExact := rfl
          planExact := rfl
          postValid := candidateValid
        }
      else .error .candidateInvariantFailure
    else .error .physicalFailure
  else .error .notAdmissible

def runState (frame : Frame) : Snapshot :=
  match execute? frame with
  | .ok settlement => settlement.post
  | .error _ => frame.pre

/-- Executable refusal projection that erases proof-carrying success data. -/
def refusal? (frame : Frame) : Option Refusal :=
  match execute? frame with
  | .ok _ => none
  | .error refusal => some refusal

theorem refusal_rolls_back
    (frame : Frame) (refusal : Refusal)
    (failed : execute? frame = .error refusal) :
    runState frame = frame.pre := by
  unfold runState
  rw [failed]

/-- A physical failure consumes no ticket or cursor.  Since semantic admission
does not inspect `physicalSucceeded`, flipping only that observation preserves
admission for a retry. -/
theorem physical_failure_is_retryable
    (frame : Frame) (admitted : semanticAccepts frame = true) :
    execute? { frame with physicalSucceeded := false } = .error .physicalFailure /\
    runState { frame with physicalSucceeded := false } = frame.pre /\
    semanticAccepts { frame with physicalSucceeded := true } = true := by
  have admittedFalse :
      semanticAccepts { frame with physicalSucceeded := false } = true := by
    change semanticAccepts frame = true
    exact admitted
  have failed :
      execute? { frame with physicalSucceeded := false } = .error .physicalFailure := by
    unfold execute?
    split
    · split
      · rename_i impossible
        simp at impossible
      · rfl
    · rename_i rejected
      exact False.elim (rejected admittedFalse)
  refine ⟨failed, ?_, ?_⟩
  · exact refusal_rolls_back _ _ failed
  · change semanticAccepts frame = true
    exact admitted

/-- Every successful transition preserves the one immutable template value. -/
theorem successful_transition_preserves_template
    (frame : Frame) (settlement : Settlement frame) :
    settlement.post.template = frame.pre.template := by
  rw [settlement.postExact]
  cases commandEq : frame.command <;>
    simp [postState, commandEq, consumePost, expirePost, closePost]

/-- Consumption advances one occurrence and makes the ticket non-replayable in
the exact same atomic candidate. -/
theorem consumption_advances_and_consumes (pre : Snapshot) :
    (consumePost pre).series.nextOccurrence = pre.series.nextOccurrence + 1 /\
    (consumePost pre).series.revision = pre.series.revision + 1 /\
    (consumePost pre).ticket.revision = pre.ticket.revision + 1 /\
    (consumePost pre).ticket.phase = .consumed /\
    (consumePost pre).ticket.funds = zeroFunds := by
  simp [consumePost, advanceSeries]

/-- A consumed Market is seeded by the shared complete-set economic kernel,
not a Series-specific liability implementation. -/
theorem market_founding_uses_economic_kernel (frame : Frame) :
    (marketFounding frame).economicState =
      Economic.runState
        (foundingEconomicFrame frame.lamportLimit frame.pre.template) /\
    (marketFounding frame).claimEffects =
      (Economic.compile
        (foundingEconomicFrame frame.lamportLimit frame.pre.template)).claimEffects := by
  simp [marketFounding]

/-- Close rent is a separately named Series compartment and terminal close
cannot touch ticket funds or Market Hoard principal. -/
theorem terminal_close_preserves_ticket (pre : Snapshot) :
    (closePost pre).ticket = pre.ticket /\
    (closePost pre).series.closeRentLamports = 0 /\
    (closePost pre).series.phase = .closed := by
  simp [closePost]

end DClutch.Series
