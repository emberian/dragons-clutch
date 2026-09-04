import DClutchSemantics.Physical
import Std.Tactic

/-!
# Reusable fully-collateralized economic microkernel

This is the common semantic program layer for Direct, General, Dealer, Bearer,
and Structured consumers.  A consumer binds the three abstract parties to its
authenticated accounts; it does not redefine complete-set or terminal
economics.

The state contains aggregate Market supply, its native/materialized partition,
and a two-party claim projection.  Claims outside that projection are the
nonnegative residual required by `valid`.  Hoard principal is claimant backing
only.  Physical account authenticity, token CPI, persistence, and transaction
rollback remain adapter obligations.
-/

namespace DClutch.Economic

open DClutch
open DClutch.Direct

/-- Lifecycle needed by the universal fully-collateralized operations. -/
inductive Phase where
  | open
  | terminal (winner : Nat)
  | retiring (winner : Nat)
  | retired
  deriving DecidableEq, Repr

/-- A claim's current representation.  Both representations contribute to the
one conservative Market supply. -/
inductive Representation where
  | native
  | materialized
  deriving DecidableEq, Repr

/-- Which of the two authenticated local claim projections is selected. -/
inductive Holder where
  | source
  | destination
  deriving DecidableEq, Repr

/-- Exact economic state projected for one bounded transition. -/
structure State where
  phase : Phase
  hoard : Nat
  supply : List Nat
  nativeSupply : List Nat
  materializedSupply : List Nat
  sourceNative : List Nat
  sourceMaterialized : List Nat
  destinationNative : List Nat
  destinationMaterialized : List Nat
  deriving DecidableEq, Repr

/-- Abstract roles compiled into the existing typed effect/physical IR. -/
structure Bindings where
  source : Party
  destination : Party
  hoard : Party
  deriving DecidableEq, Repr

/-- Shared first-order economic commands. -/
inductive Command where
  | splitCompleteSet (holder : Holder) (representation : Representation)
      (quantity : Nat)
  | mergeCompleteSet (holder : Holder) (representation : Representation)
      (quantity : Nat)
  | transferClaim (representation : Representation) (outcome quantity : Nat)
  | materializeClaim (outcome quantity : Nat)
  | dematerializeClaim (outcome quantity : Nat)
  | redeemTerminal (holder : Holder) (representation : Representation)
      (outcome quantity : Nat)
  | retireTerminal
  deriving DecidableEq, Repr

/-- One untrusted invocation of the shared program.  `scalarLimit` is supplied
by the physical profile (for example, `2^64`), rather than hidden in the
universal economics. -/
structure Frame where
  outcomeCount : Nat
  scalarLimit : Nat
  bindings : Bindings
  pre : State
  command : Command
  deriving DecidableEq, Repr

def valueAt (values : List Nat) (index : Nat) : Nat :=
  values[index]?.getD 0

def setAt (values : List Nat) (index value : Nat) : List Nat :=
  values.set index value

def addEvery (values : List Nat) (quantity : Nat) : List Nat :=
  values.map fun value => value + quantity

def subEvery (values : List Nat) (quantity : Nat) : List Nat :=
  values.map fun value => value - quantity

def addAt (values : List Nat) (outcome quantity : Nat) : List Nat :=
  setAt values outcome (valueAt values outcome + quantity)

def subAt (values : List Nat) (outcome quantity : Nat) : List Nat :=
  setAt values outcome (valueAt values outcome - quantity)

def State.representationSupply (state : State) : Representation → List Nat
  | .native => state.nativeSupply
  | .materialized => state.materializedSupply

def State.holderClaims (state : State) : Holder → Representation → List Nat
  | .source, .native => state.sourceNative
  | .source, .materialized => state.sourceMaterialized
  | .destination, .native => state.destinationNative
  | .destination, .materialized => state.destinationMaterialized

def State.withRepresentationSupply
    (state : State) (representation : Representation) (values : List Nat) : State :=
  match representation with
  | .native => { state with nativeSupply := values }
  | .materialized => { state with materializedSupply := values }

def State.withHolderClaims
    (state : State) (holder : Holder) (representation : Representation)
    (values : List Nat) : State :=
  match holder, representation with
  | .source, .native => { state with sourceNative := values }
  | .source, .materialized => { state with sourceMaterialized := values }
  | .destination, .native => { state with destinationNative := values }
  | .destination, .materialized => { state with destinationMaterialized := values }

def partyOf (bindings : Bindings) : Holder → Party
  | .source => bindings.source
  | .destination => bindings.destination

def allIndices (count : Nat) (predicate : Nat → Bool) : Bool :=
  (List.range count).all predicate

def vectorValid (count limit : Nat) (values : List Nat) : Bool :=
  values.length = count && values.all fun value => value < limit

def allAddFits (limit quantity : Nat) (values : List Nat) : Bool :=
  values.all fun value => value + quantity < limit

def allHas (quantity : Nat) (values : List Nat) : Bool :=
  values.all fun value => quantity ≤ value

def allZero (values : List Nat) : Bool :=
  values.all fun value => value = 0

def phaseValid (count : Nat) (state : State) : Bool :=
  match state.phase with
  | .open => allIndices count fun outcome => valueAt state.supply outcome ≤ state.hoard
  | .terminal winner | .retiring winner =>
      winner < count && valueAt state.supply winner ≤ state.hoard
  | .retired => state.hoard = 0 && allZero state.supply

/-- Executable exhaustive invariant check.  The native/materialized partition
is exact, the two named holders never exceed it, and the phase-specific
liability is fully collateralized. -/
def valid (count limit : Nat) (state : State) : Bool :=
  0 < count && 0 < limit &&
  vectorValid count limit state.supply &&
  vectorValid count limit state.nativeSupply &&
  vectorValid count limit state.materializedSupply &&
  vectorValid count limit state.sourceNative &&
  vectorValid count limit state.sourceMaterialized &&
  vectorValid count limit state.destinationNative &&
  vectorValid count limit state.destinationMaterialized &&
  allIndices count (fun outcome =>
    valueAt state.supply outcome =
      valueAt state.nativeSupply outcome + valueAt state.materializedSupply outcome) &&
  allIndices count (fun outcome =>
    valueAt state.sourceNative outcome + valueAt state.destinationNative outcome ≤
      valueAt state.nativeSupply outcome) &&
  allIndices count (fun outcome =>
    valueAt state.sourceMaterialized outcome +
        valueAt state.destinationMaterialized outcome ≤
      valueAt state.materializedSupply outcome) &&
  phaseValid count state

def livePhase : Phase → Bool
  | .open | .terminal _ | .retiring _ => true
  | .retired => false

def terminalWinner? : Phase → Option Nat
  | .terminal winner | .retiring winner => some winner
  | _ => none

def bindingAccepts (frame : Frame) : Bool :=
  match frame.command with
  | .splitCompleteSet holder _ _ | .mergeCompleteSet holder _ _ |
      .redeemTerminal holder _ _ _ =>
      partyOf frame.bindings holder != frame.bindings.hoard
  | .transferClaim .. | .materializeClaim .. | .dematerializeClaim .. =>
      frame.bindings.source != frame.bindings.destination
  | .retireTerminal => true

/-- Complete executable admission predicate.  Every subtraction and addition
needed by the candidate is checked before candidate construction. -/
def commandAccepts (frame : Frame) : Bool :=
  match frame.command with
  | .splitCompleteSet holder representation quantity =>
      frame.pre.phase = .open && 0 < quantity &&
      frame.pre.hoard + quantity < frame.scalarLimit &&
      allAddFits frame.scalarLimit quantity frame.pre.supply &&
      allAddFits frame.scalarLimit quantity
        (frame.pre.representationSupply representation) &&
      allAddFits frame.scalarLimit quantity
        (frame.pre.holderClaims holder representation)
  | .mergeCompleteSet holder representation quantity =>
      frame.pre.phase = .open && 0 < quantity && quantity ≤ frame.pre.hoard &&
      allHas quantity frame.pre.supply &&
      allHas quantity (frame.pre.representationSupply representation) &&
      allHas quantity (frame.pre.holderClaims holder representation)
  | .transferClaim representation outcome quantity =>
      livePhase frame.pre.phase && 0 < quantity && outcome < frame.outcomeCount &&
      quantity ≤ valueAt (frame.pre.holderClaims .source representation) outcome &&
      valueAt (frame.pre.holderClaims .destination representation) outcome + quantity <
        frame.scalarLimit
  | .materializeClaim outcome quantity =>
      frame.pre.phase = .open && 0 < quantity && outcome < frame.outcomeCount &&
      quantity ≤ valueAt frame.pre.nativeSupply outcome &&
      quantity ≤ valueAt frame.pre.sourceNative outcome &&
      valueAt frame.pre.materializedSupply outcome + quantity < frame.scalarLimit &&
      valueAt frame.pre.destinationMaterialized outcome + quantity < frame.scalarLimit
  | .dematerializeClaim outcome quantity =>
      livePhase frame.pre.phase && 0 < quantity && outcome < frame.outcomeCount &&
      quantity ≤ valueAt frame.pre.materializedSupply outcome &&
      quantity ≤ valueAt frame.pre.sourceMaterialized outcome &&
      valueAt frame.pre.nativeSupply outcome + quantity < frame.scalarLimit &&
      valueAt frame.pre.destinationNative outcome + quantity < frame.scalarLimit
  | .redeemTerminal holder representation outcome quantity =>
      (terminalWinner? frame.pre.phase).isSome && 0 < quantity &&
      outcome < frame.outcomeCount &&
      quantity ≤ valueAt frame.pre.supply outcome &&
      quantity ≤ valueAt (frame.pre.representationSupply representation) outcome &&
      quantity ≤ valueAt (frame.pre.holderClaims holder representation) outcome &&
      (if terminalWinner? frame.pre.phase = some outcome
        then quantity ≤ frame.pre.hoard else true)
  | .retireTerminal =>
      (match frame.pre.phase with | .retiring _ => true | _ => false) &&
      frame.pre.hoard = 0 && allZero frame.pre.supply &&
      allZero frame.pre.materializedSupply

def accepts (frame : Frame) : Bool :=
  valid frame.outcomeCount frame.scalarLimit frame.pre &&
    bindingAccepts frame && commandAccepts frame

def splitPost
    (pre : State) (holder : Holder) (representation : Representation)
    (quantity : Nat) : State :=
  let representationAfter := addEvery (pre.representationSupply representation) quantity
  let holderAfter := addEvery (pre.holderClaims holder representation) quantity
  let candidate :=
    (pre.withRepresentationSupply representation representationAfter).withHolderClaims
      holder representation holderAfter
  { candidate with
    hoard := pre.hoard + quantity
    supply := addEvery pre.supply quantity }

def mergePost
    (pre : State) (holder : Holder) (representation : Representation)
    (quantity : Nat) : State :=
  let representationAfter := subEvery (pre.representationSupply representation) quantity
  let holderAfter := subEvery (pre.holderClaims holder representation) quantity
  let candidate :=
    (pre.withRepresentationSupply representation representationAfter).withHolderClaims
      holder representation holderAfter
  { candidate with
    hoard := pre.hoard - quantity
    supply := subEvery pre.supply quantity }

def transferPost
    (pre : State) (representation : Representation) (outcome quantity : Nat) : State :=
  let sourceAfter := subAt (pre.holderClaims .source representation) outcome quantity
  let destinationAfter := addAt (pre.holderClaims .destination representation) outcome quantity
  (pre.withHolderClaims .source representation sourceAfter).withHolderClaims
    .destination representation destinationAfter

def materializePost (pre : State) (outcome quantity : Nat) : State :=
  { pre with
    nativeSupply := subAt pre.nativeSupply outcome quantity
    materializedSupply := addAt pre.materializedSupply outcome quantity
    sourceNative := subAt pre.sourceNative outcome quantity
    destinationMaterialized := addAt pre.destinationMaterialized outcome quantity }

def dematerializePost (pre : State) (outcome quantity : Nat) : State :=
  { pre with
    nativeSupply := addAt pre.nativeSupply outcome quantity
    materializedSupply := subAt pre.materializedSupply outcome quantity
    sourceMaterialized := subAt pre.sourceMaterialized outcome quantity
    destinationNative := addAt pre.destinationNative outcome quantity }

def redemptionPayout (phase : Phase) (outcome quantity : Nat) : Nat :=
  if terminalWinner? phase = some outcome then quantity else 0

def redeemPost
    (pre : State) (holder : Holder) (representation : Representation)
    (outcome quantity : Nat) : State :=
  let representationAfter := subAt (pre.representationSupply representation) outcome quantity
  let holderAfter := subAt (pre.holderClaims holder representation) outcome quantity
  let candidate :=
    (pre.withRepresentationSupply representation representationAfter).withHolderClaims
      holder representation holderAfter
  { candidate with
    hoard := pre.hoard - redemptionPayout pre.phase outcome quantity
    supply := subAt pre.supply outcome quantity }

def postState (frame : Frame) : State :=
  match frame.command with
  | .splitCompleteSet holder representation quantity =>
      splitPost frame.pre holder representation quantity
  | .mergeCompleteSet holder representation quantity =>
      mergePost frame.pre holder representation quantity
  | .transferClaim representation outcome quantity =>
      transferPost frame.pre representation outcome quantity
  | .materializeClaim outcome quantity =>
      materializePost frame.pre outcome quantity
  | .dematerializeClaim outcome quantity =>
      dematerializePost frame.pre outcome quantity
  | .redeemTerminal holder representation outcome quantity =>
      redeemPost frame.pre holder representation outcome quantity
  | .retireTerminal => { frame.pre with phase := .retired }

def claimCell (party : Party) (outcome : Nat) : Cell :=
  { party, resource := .outcomeClaim outcome }

def completeSetEffects
    (credit : Bool) (party : Party) (count quantity : Nat) : List Effect :=
  (List.range count).map fun outcome =>
    if credit then .credit (claimCell party outcome) quantity
    else .debit (claimCell party outcome) quantity

def claimMoveEffects
    (bindings : Bindings) (outcome quantity : Nat) : List Effect := [
  .debit (claimCell bindings.source outcome) quantity,
  .credit (claimCell bindings.destination outcome) quantity
]

def custodyMove (source destination : Party) (amount : Nat) :
    Direct.Physical.CustodyTransfer :=
  { source, destination, amount }

/-- Compile one semantic command into the existing first-order claim and exact
custody plan types.  Consumers may materialize this plan only after `execute?`
returns success. -/
def compile (frame : Frame) : Direct.Physical.PhysicalPlan :=
  match frame.command with
  | .splitCompleteSet holder _ quantity =>
      let holderParty := partyOf frame.bindings holder
      {
        claimEffects := EffectPlan.mk
          (completeSetEffects true holderParty frame.outcomeCount quantity)
        custodyTransfers := [custodyMove holderParty frame.bindings.hoard quantity]
      }
  | .mergeCompleteSet holder _ quantity =>
      let holderParty := partyOf frame.bindings holder
      {
        claimEffects := EffectPlan.mk
          (completeSetEffects false holderParty frame.outcomeCount quantity)
        custodyTransfers := [custodyMove frame.bindings.hoard holderParty quantity]
      }
  | .transferClaim _ outcome quantity |
      .materializeClaim outcome quantity |
      .dematerializeClaim outcome quantity => {
      claimEffects := EffectPlan.mk (claimMoveEffects frame.bindings outcome quantity)
      custodyTransfers := []
    }
  | .redeemTerminal holder _ outcome quantity =>
      let payout := redemptionPayout frame.pre.phase outcome quantity
      {
        claimEffects := EffectPlan.mk [
          .debit (claimCell (partyOf frame.bindings holder) outcome) quantity
        ]
        custodyTransfers := if payout = 0 then [] else [
          custodyMove frame.bindings.hoard (partyOf frame.bindings holder) payout
        ]
      }
  | .retireTerminal => { claimEffects := EffectPlan.mk [], custodyTransfers := [] }

inductive Refusal where
  | notAdmissible
  | candidateInvariantFailure
  deriving DecidableEq, Repr

/-- Successful, proof-carrying output.  The candidate is both the exact
semantic post-state and revalidated before a physical plan is exposed. -/
structure Settlement (frame : Frame) where
  post : State
  program : Direct.Physical.PhysicalPlan
  accepted : accepts frame = true
  postExact : post = postState frame
  programExact : program = compile frame
  postValid : valid frame.outcomeCount frame.scalarLimit post = true

/-- Total executable boundary.  A missed candidate invariant is an explicit
refusal, never a proof-only precondition. -/
def execute? (frame : Frame) : Except Refusal (Settlement frame) :=
  if accepted : accepts frame = true then
    let candidate := postState frame
    if candidateValid : valid frame.outcomeCount frame.scalarLimit candidate = true then
      .ok {
        post := candidate
        program := compile frame
        accepted
        postExact := rfl
        programExact := rfl
        postValid := candidateValid
      }
    else .error .candidateInvariantFailure
  else .error .notAdmissible

/-- Observable state semantics: every refusal exposes the complete pre-state. -/
def runState (frame : Frame) : State :=
  match execute? frame with
  | .ok settlement => settlement.post
  | .error _ => frame.pre

theorem successful_execution_is_valid
    (frame : Frame) (settlement : Settlement frame) :
    valid frame.outcomeCount frame.scalarLimit settlement.post = true :=
  settlement.postValid

theorem successful_execution_is_exact
    (frame : Frame) (settlement : Settlement frame) :
    settlement.post = postState frame :=
  settlement.postExact

theorem successful_execution_emits_compiled_program
    (frame : Frame) (settlement : Settlement frame) :
    settlement.program = compile frame := by
  exact settlement.programExact

theorem refusal_rolls_back
    (frame : Frame) (refusal : Refusal)
    (failed : execute? frame = .error refusal) :
    runState frame = frame.pre := by
  unfold runState
  rw [failed]

theorem rejected_execute_refuses
    (frame : Frame) (rejected : accepts frame = false) :
    execute? frame = .error .notAdmissible := by
  simp [execute?, rejected]

theorem valueAt_setAt_eq
    (values : List Nat) (index value : Nat) (inBounds : index < values.length) :
    valueAt (setAt values index value) index = value := by
  simp [valueAt, setAt, inBounds]

theorem valueAt_addAt_eq
    (values : List Nat) (index quantity : Nat) (inBounds : index < values.length) :
    valueAt (addAt values index quantity) index = valueAt values index + quantity := by
  simp [addAt, valueAt_setAt_eq values index _ inBounds]

theorem valueAt_subAt_eq
    (values : List Nat) (index quantity : Nat) (inBounds : index < values.length) :
    valueAt (subAt values index quantity) index = valueAt values index - quantity := by
  simp [subAt, valueAt_setAt_eq values index _ inBounds]

theorem valueAt_addEvery_eq
    (values : List Nat) (index quantity : Nat) (inBounds : index < values.length) :
    valueAt (addEvery values quantity) index = valueAt values index + quantity := by
  simp [valueAt, addEvery, inBounds]

theorem valueAt_subEvery_eq
    (values : List Nat) (index quantity : Nat) (inBounds : index < values.length) :
    valueAt (subEvery values quantity) index = valueAt values index - quantity := by
  simp [valueAt, subEvery, inBounds]

/-- A complete-set split locks exactly one collateral atom for each newly
issued unit of every outcome. -/
theorem split_complete_set_exact
    (pre : State) (holder : Holder) (representation : Representation)
    (quantity : Nat) :
    (splitPost pre holder representation quantity).hoard = pre.hoard + quantity ∧
    (splitPost pre holder representation quantity).supply = addEvery pre.supply quantity ∧
    (splitPost pre holder representation quantity).representationSupply representation =
      addEvery (pre.representationSupply representation) quantity ∧
    (splitPost pre holder representation quantity).holderClaims holder representation =
      addEvery (pre.holderClaims holder representation) quantity := by
  cases holder <;> cases representation <;>
    simp [splitPost, State.withRepresentationSupply, State.withHolderClaims,
      State.representationSupply, State.holderClaims]

/-- A complete-set merge burns the same quantity in every outcome and releases
exactly that quantity of Hoard backing. -/
theorem merge_complete_set_exact
    (pre : State) (holder : Holder) (representation : Representation)
    (quantity : Nat) :
    (mergePost pre holder representation quantity).hoard = pre.hoard - quantity ∧
    (mergePost pre holder representation quantity).supply = subEvery pre.supply quantity ∧
    (mergePost pre holder representation quantity).representationSupply representation =
      subEvery (pre.representationSupply representation) quantity ∧
    (mergePost pre holder representation quantity).holderClaims holder representation =
      subEvery (pre.holderClaims holder representation) quantity := by
  cases holder <;> cases representation <;>
    simp [mergePost, State.withRepresentationSupply, State.withHolderClaims,
      State.representationSupply, State.holderClaims]

theorem merge_complete_set_conserves_backing
    (pre : State) (holder : Holder) (representation : Representation)
    (outcome quantity : Nat)
    (supplyLength : outcome < pre.supply.length)
    (supplyAvailable : quantity ≤ valueAt pre.supply outcome)
    (hoardAvailable : quantity ≤ pre.hoard) :
    (mergePost pre holder representation quantity).hoard + quantity = pre.hoard ∧
    valueAt (mergePost pre holder representation quantity).supply outcome + quantity =
      valueAt pre.supply outcome := by
  have selected := valueAt_subEvery_eq pre.supply outcome quantity supplyLength
  cases holder <;> cases representation <;>
    simp [mergePost, State.withRepresentationSupply, State.withHolderClaims,
      selected]
  all_goals omega

/-- A claim transfer changes neither aggregate Market supply nor Hoard, and its
selected local source/destination sum is exact. -/
theorem claim_transfer_conserves
    (pre : State) (representation : Representation) (outcome quantity : Nat)
    (sourceLength : outcome < (pre.holderClaims .source representation).length)
    (destinationLength : outcome < (pre.holderClaims .destination representation).length)
    (available : quantity ≤ valueAt (pre.holderClaims .source representation) outcome) :
    (transferPost pre representation outcome quantity).supply = pre.supply ∧
    (transferPost pre representation outcome quantity).hoard = pre.hoard ∧
    valueAt ((transferPost pre representation outcome quantity).holderClaims
      .source representation) outcome +
        valueAt ((transferPost pre representation outcome quantity).holderClaims
          .destination representation) outcome =
      valueAt (pre.holderClaims .source representation) outcome +
        valueAt (pre.holderClaims .destination representation) outcome := by
  cases representation with
  | native =>
      have sourceLength' : outcome < pre.sourceNative.length := by
        simpa [State.holderClaims] using sourceLength
      have destinationLength' : outcome < pre.destinationNative.length := by
        simpa [State.holderClaims] using destinationLength
      have available' : quantity ≤ valueAt pre.sourceNative outcome := by
        simpa [State.holderClaims] using available
      refine ⟨rfl, rfl, ?_⟩
      change valueAt (subAt pre.sourceNative outcome quantity) outcome +
          valueAt (addAt pre.destinationNative outcome quantity) outcome =
        valueAt pre.sourceNative outcome + valueAt pre.destinationNative outcome
      rw [valueAt_subAt_eq _ _ _ sourceLength',
        valueAt_addAt_eq _ _ _ destinationLength']
      omega
  | materialized =>
      have sourceLength' : outcome < pre.sourceMaterialized.length := by
        simpa [State.holderClaims] using sourceLength
      have destinationLength' : outcome < pre.destinationMaterialized.length := by
        simpa [State.holderClaims] using destinationLength
      have available' : quantity ≤ valueAt pre.sourceMaterialized outcome := by
        simpa [State.holderClaims] using available
      refine ⟨rfl, rfl, ?_⟩
      change valueAt (subAt pre.sourceMaterialized outcome quantity) outcome +
          valueAt (addAt pre.destinationMaterialized outcome quantity) outcome =
        valueAt pre.sourceMaterialized outcome +
          valueAt pre.destinationMaterialized outcome
      rw [valueAt_subAt_eq _ _ _ sourceLength',
        valueAt_addAt_eq _ _ _ destinationLength']
      omega

/-- Materialization changes representation, never conservative Market supply
or Hoard principal. -/
theorem materialization_conserves
    (pre : State) (outcome quantity : Nat)
    (nativeLength : outcome < pre.nativeSupply.length)
    (materializedLength : outcome < pre.materializedSupply.length)
    (sourceLength : outcome < pre.sourceNative.length)
    (destinationLength : outcome < pre.destinationMaterialized.length)
    (aggregateAvailable : quantity ≤ valueAt pre.nativeSupply outcome)
    (holderAvailable : quantity ≤ valueAt pre.sourceNative outcome) :
    (materializePost pre outcome quantity).supply = pre.supply ∧
    (materializePost pre outcome quantity).hoard = pre.hoard ∧
    valueAt (materializePost pre outcome quantity).nativeSupply outcome +
        valueAt (materializePost pre outcome quantity).materializedSupply outcome =
      valueAt pre.nativeSupply outcome + valueAt pre.materializedSupply outcome ∧
    valueAt (materializePost pre outcome quantity).sourceNative outcome +
        valueAt (materializePost pre outcome quantity).destinationMaterialized outcome =
      valueAt pre.sourceNative outcome + valueAt pre.destinationMaterialized outcome := by
  simp [materializePost, valueAt_subAt_eq, valueAt_addAt_eq, nativeLength,
    materializedLength, sourceLength, destinationLength]
  constructor <;> omega

theorem dematerialization_conserves
    (pre : State) (outcome quantity : Nat)
    (nativeLength : outcome < pre.nativeSupply.length)
    (materializedLength : outcome < pre.materializedSupply.length)
    (sourceLength : outcome < pre.sourceMaterialized.length)
    (destinationLength : outcome < pre.destinationNative.length)
    (aggregateAvailable : quantity ≤ valueAt pre.materializedSupply outcome)
    (holderAvailable : quantity ≤ valueAt pre.sourceMaterialized outcome) :
    (dematerializePost pre outcome quantity).supply = pre.supply ∧
    (dematerializePost pre outcome quantity).hoard = pre.hoard ∧
    valueAt (dematerializePost pre outcome quantity).nativeSupply outcome +
        valueAt (dematerializePost pre outcome quantity).materializedSupply outcome =
      valueAt pre.nativeSupply outcome + valueAt pre.materializedSupply outcome ∧
    valueAt (dematerializePost pre outcome quantity).sourceMaterialized outcome +
        valueAt (dematerializePost pre outcome quantity).destinationNative outcome =
      valueAt pre.sourceMaterialized outcome + valueAt pre.destinationNative outcome := by
  simp [dematerializePost, valueAt_subAt_eq, valueAt_addAt_eq, nativeLength,
    materializedLength, sourceLength, destinationLength]
  constructor <;> omega

/-- Terminal redemption burns exactly the selected claim quantity and removes
exactly the categorical payout (quantity for the winner, zero otherwise). -/
theorem terminal_redemption_conserves
    (pre : State) (holder : Holder) (representation : Representation)
    (outcome quantity : Nat)
    (supplyLength : outcome < pre.supply.length)
    (supplyAvailable : quantity ≤ valueAt pre.supply outcome)
    (payoutAvailable : redemptionPayout pre.phase outcome quantity ≤ pre.hoard) :
    (redeemPost pre holder representation outcome quantity).hoard +
        redemptionPayout pre.phase outcome quantity = pre.hoard ∧
    valueAt (redeemPost pre holder representation outcome quantity).supply outcome +
        quantity = valueAt pre.supply outcome := by
  cases holder <;> cases representation <;>
    simp [redeemPost, valueAt_subAt_eq, supplyLength]
  all_goals omega

/-- Retirement is liability-neutral and only marks an already empty retiring
state as terminally retired. -/
theorem terminal_retirement_is_economically_neutral (pre : State) :
    ({ pre with phase := Phase.retired }).hoard = pre.hoard ∧
    ({ pre with phase := Phase.retired }).supply = pre.supply ∧
    ({ pre with phase := Phase.retired }).nativeSupply = pre.nativeSupply ∧
    ({ pre with phase := Phase.retired }).materializedSupply = pre.materializedSupply := by
  simp

theorem complete_set_effect_count
    (credit : Bool) (party : Party) (count quantity : Nat) :
    (completeSetEffects credit party count quantity).length = count := by
  simp [completeSetEffects]

/-! ## The failure escrow, and what an outage refunds

A runtime Product's claim vector is `ordinaryCount` ordinary regions followed by
exactly one explicit failure coordinate, so `failureSelector` is `ordinaryCount`
and the width is `ordinaryCount + 1`.  Founding mints one equal complete set,
and the escrow ruling changes only *where the failure coordinate lands*: the
ordinary coordinates go to the founder and the failure coordinate to an
identity the founding derives.  The aggregate is untouched --
`escrowed_founding_is_a_complete_set_split_in_the_aggregate` -- so every law
proved above about supply, backing and the native/materialized partition still
governs the escrowed founding, and the escrow is one more Position in the
supply-vector census rather than a hole in it.

An outage then refunds rather than paying whoever minted the failure claims.
Supply is uniform across every coordinate for as long as a Market is open --
the complete-set actions are the only ones that move it and both refuse a
non-uniform vector -- so with `supply` sets outstanding the escrow stands
against `ordinaryCount * supply` ordinary claims and the pro-rata share of one
ordinary claim is a CONSTANT the Market header alone determines.  Pro rata
needs no holder census, and the founder is paid for the ordinary claims they
still hold and for nothing else.

The remainder is not routed anywhere; it is made impossible.  A floored atom
would land in no declared compartment, so `foundingRefundExact` refuses at
founding, at the founder's own basis scale, rather than at a stranger's
redemption -- and under an admitted founding
`an_admitted_failure_walk_leaves_no_remainder` holds for EVERY partition of the
ordinary claims with no divisibility hypothesis left in it.
-/

def failureSelector (ordinaryCount : Nat) : Nat := ordinaryCount

def outcomeWidth (ordinaryCount : Nat) : Nat := ordinaryCount + 1

def addBelow : Nat → List Nat → Nat → List Nat
  | 0, values, _ => values
  | _, [], _ => []
  | bound + 1, value :: rest, quantity =>
      (value + quantity) :: addBelow bound rest quantity

theorem length_addBelow (bound : Nat) (values : List Nat) (quantity : Nat) :
    (addBelow bound values quantity).length = values.length := by
  induction bound generalizing values with
  | zero => simp [addBelow]
  | succ bound ih =>
      cases values with
      | nil => simp [addBelow]
      | cons value rest => simp [addBelow, ih]

theorem valueAt_addBelow_ordinary
    (bound : Nat) (values : List Nat) (quantity index : Nat)
    (ordinary : index < bound) (present : index < values.length) :
    valueAt (addBelow bound values quantity) index
      = valueAt values index + quantity := by
  induction bound generalizing values index with
  | zero => omega
  | succ bound ih =>
      cases values with
      | nil => simp at present
      | cons value rest =>
          cases index with
          | zero => simp [addBelow, valueAt]
          | succ index =>
              have inner : index < bound := by omega
              have shorter : index < rest.length := by simpa using present
              simpa [addBelow, valueAt] using ih rest index inner shorter

theorem valueAt_addBelow_failure
    (bound : Nat) (values : List Nat) (quantity index : Nat)
    (failure : bound ≤ index) :
    valueAt (addBelow bound values quantity) index = valueAt values index := by
  induction bound generalizing values index with
  | zero => simp [addBelow]
  | succ bound ih =>
      cases values with
      | nil => simp [addBelow]
      | cons value rest =>
          cases index with
          | zero => omega
          | succ index =>
              have inner : bound ≤ index := by omega
              simpa [addBelow, valueAt] using ih rest index inner

theorem valueAt_replicate_zero (count index : Nat) :
    valueAt (List.replicate count 0) index = 0 := by
  unfold valueAt
  cases h : (List.replicate count 0)[index]? with
  | none => simp
  | some value =>
      obtain ⟨_, hv⟩ := List.getElem?_eq_some_iff.mp h
      simp [hv.symm]


theorem valueAt_addAt_ne
    (values : List Nat) (index other quantity : Nat) (distinct : other ≠ index) :
    valueAt (addAt values index quantity) other = valueAt values other := by
  simp [addAt, setAt, valueAt, Ne.symm distinct]

/-- Founding under the escrow ruling: one complete set, the ordinary
coordinates credited to the founder (`destination`) and the failure coordinate
credited to the market-derived escrow (`source`). -/
def escrowedFoundingPost (ordinaryCount : Nat) (pre : State) (quantity : Nat) : State :=
  { pre with
    hoard := pre.hoard + quantity
    supply := addEvery pre.supply quantity
    nativeSupply := addEvery pre.nativeSupply quantity
    destinationNative := addBelow ordinaryCount pre.destinationNative quantity
    sourceNative := addAt pre.sourceNative (failureSelector ordinaryCount) quantity }

/-- The vacant pre-state founding runs against. -/
def vacantFounding (ordinaryCount : Nat) (pre : State) : Prop :=
  pre.sourceNative = List.replicate (outcomeWidth ordinaryCount) 0 ∧
  pre.destinationNative = List.replicate (outcomeWidth ordinaryCount) 0

theorem escrowed_founding_is_a_complete_set_split_in_the_aggregate
    (ordinaryCount : Nat) (pre : State) (quantity : Nat) :
    (escrowedFoundingPost ordinaryCount pre quantity).hoard =
      (splitPost pre .destination .native quantity).hoard ∧
    (escrowedFoundingPost ordinaryCount pre quantity).supply =
      (splitPost pre .destination .native quantity).supply ∧
    (escrowedFoundingPost ordinaryCount pre quantity).nativeSupply =
      (splitPost pre .destination .native quantity).nativeSupply ∧
    (escrowedFoundingPost ordinaryCount pre quantity).materializedSupply =
      (splitPost pre .destination .native quantity).materializedSupply := by
  simp [escrowedFoundingPost, splitPost, State.withRepresentationSupply,
    State.withHolderClaims, State.representationSupply, State.holderClaims]

theorem escrowed_founding_mints_ordinary_claims_to_the_founder
    (ordinaryCount : Nat) (pre : State) (quantity outcome : Nat)
    (vacant : vacantFounding ordinaryCount pre)
    (ordinary : outcome < ordinaryCount) :
    valueAt (escrowedFoundingPost ordinaryCount pre quantity).destinationNative outcome
      = quantity := by
  obtain ⟨_, founder⟩ := vacant
  have present : outcome < pre.destinationNative.length := by
    rw [founder]; simp [outcomeWidth]; omega
  simp only [escrowedFoundingPost]
  rw [valueAt_addBelow_ordinary ordinaryCount pre.destinationNative quantity outcome
      ordinary present, founder, valueAt_replicate_zero]
  simp

theorem escrowed_founding_seats_the_failure_coordinate_in_the_escrow
    (ordinaryCount : Nat) (pre : State) (quantity : Nat)
    (vacant : vacantFounding ordinaryCount pre) :
    valueAt (escrowedFoundingPost ordinaryCount pre quantity).destinationNative
        (failureSelector ordinaryCount) = 0 ∧
    valueAt (escrowedFoundingPost ordinaryCount pre quantity).sourceNative
        (failureSelector ordinaryCount) = quantity := by
  obtain ⟨escrow, founder⟩ := vacant
  have present : failureSelector ordinaryCount < pre.sourceNative.length := by
    rw [escrow]; simp [outcomeWidth, failureSelector]
  simp only [escrowedFoundingPost]
  constructor
  · rw [valueAt_addBelow_failure ordinaryCount pre.destinationNative quantity
      (failureSelector ordinaryCount) (Nat.le_refl _), founder, valueAt_replicate_zero]
  · rw [valueAt_addAt_eq _ _ _ present, escrow, valueAt_replicate_zero]
    simp

theorem escrowed_founding_gives_the_escrow_no_ordinary_claims
    (ordinaryCount : Nat) (pre : State) (quantity outcome : Nat)
    (vacant : vacantFounding ordinaryCount pre)
    (ordinary : outcome < ordinaryCount) :
    valueAt (escrowedFoundingPost ordinaryCount pre quantity).sourceNative outcome = 0 := by
  obtain ⟨escrow, _⟩ := vacant
  have distinct : outcome ≠ failureSelector ordinaryCount := by
    simp [failureSelector]; omega
  simp only [escrowedFoundingPost]
  rw [valueAt_addAt_ne _ _ _ _ distinct, escrow, valueAt_replicate_zero]

/-- Collateral one holder draws from the escrow for `quantity` ordinary claims
when the failure selector resolves the Market.  Supply is uniform across every
coordinate for as long as the Market is open, so with `supply` complete sets
outstanding the escrow holds `supply * multiplier` collateral against
`ordinaryCount * supply` ordinary claims and the rate is `multiplier` per
`ordinaryCount` claims -- a constant the Market header alone determines, with
no holder census anywhere in it. -/
def failureRefund (ordinaryCount quantity multiplier : Nat) : Nat :=
  quantity * multiplier / ordinaryCount

/-- The founding-time admission that makes every failure refund exact.  A
floored atom would land in no declared compartment -- the census names nine and
none of them is an upkeep vault, and creating one is an economic ruling this
lane does not own -- so the remainder is made IMPOSSIBLE rather than housed,
and the refusal sits at the founder's own basis scale rather than at a
stranger's redemption. -/
def foundingRefundExact (ordinaryCount multiplier : Nat) : Bool :=
  0 < ordinaryCount && multiplier % ordinaryCount == 0

/-- Why the founding constraint exists.  Without it a holder can present only
the largest divisible part of their claims, and what they cannot present is
worth strictly less than one collateral atom -- small, but with nowhere
declared to go. -/
theorem the_unredeemable_residue_is_smaller_than_one_atom
    (ordinaryCount quantity multiplier : Nat) (positive : 0 < ordinaryCount) :
    quantity * multiplier
        - ordinaryCount * failureRefund ordinaryCount quantity multiplier
      < ordinaryCount := by
  have split := Nat.div_add_mod (quantity * multiplier) ordinaryCount
  have bound := Nat.mod_lt (quantity * multiplier) positive
  unfold failureRefund
  omega

def refundTotal (ordinaryCount multiplier : Nat) (holdings : List Nat) : Nat :=
  (holdings.map fun quantity => failureRefund ordinaryCount quantity multiplier).sum

/-- Every ordinary claim outstanding redeems for exactly the escrow's whole
collateral: `supply * multiplier` out, and nothing left behind. -/
theorem failure_refund_exhausts_the_escrow
    (ordinaryCount supply multiplier : Nat) (positive : 0 < ordinaryCount) :
    failureRefund ordinaryCount (ordinaryCount * supply) multiplier
      = supply * multiplier := by
  unfold failureRefund
  rw [Nat.mul_assoc]
  exact Nat.mul_div_cancel_left _ positive

theorem ordinary_shares_divide_their_sum
    (ordinaryCount multiplier : Nat) (holdings : List Nat)
    (exact : ∀ quantity ∈ holdings, ordinaryCount ∣ quantity * multiplier) :
    ordinaryCount ∣ holdings.sum * multiplier := by
  induction holdings with
  | nil => simp
  | cons quantity rest ih =>
      have head := exact quantity (by simp)
      have tail : ∀ q ∈ rest, ordinaryCount ∣ q * multiplier :=
        fun q hq => exact q (by simp [hq])
      have rest := ih tail
      simpa [List.sum_cons, Nat.add_mul] using Nat.dvd_add head rest

/-- Pro rata is holder-count independent: splitting the same ordinary claims
across more holders pays out exactly the same total.  This is why the founder's
own ordinary claims pay like anyone else's -- the refund reads a claim balance
and nothing else about who holds it. -/
theorem failure_refund_is_additive_over_holders
    (ordinaryCount multiplier : Nat) (positive : 0 < ordinaryCount)
    (holdings : List Nat)
    (exact : ∀ quantity ∈ holdings, ordinaryCount ∣ quantity * multiplier) :
    refundTotal ordinaryCount multiplier holdings
      = failureRefund ordinaryCount holdings.sum multiplier := by
  induction holdings with
  | nil => simp [refundTotal, failureRefund]
  | cons quantity rest ih =>
      have head := exact quantity (by simp)
      have tail : ∀ q ∈ rest, ordinaryCount ∣ q * multiplier :=
        fun q hq => exact q (by simp [hq])
      obtain ⟨a, ha⟩ := head
      obtain ⟨b, hb⟩ := ordinary_shares_divide_their_sum ordinaryCount multiplier rest tail
      have sum : (quantity + rest.sum) * multiplier = ordinaryCount * (a + b) := by
        rw [Nat.add_mul, ha, hb, Nat.mul_add]
      simp only [refundTotal, List.map_cons, List.sum_cons] at *
      rw [ih tail]
      unfold failureRefund
      rw [ha, hb, sum, Nat.mul_div_cancel_left _ positive,
        Nat.mul_div_cancel_left _ positive, Nat.mul_div_cancel_left _ positive]

/-- The headline conservation.  When the ordinary claims outstanding are held
in any partition whatever, and every holder's share is exact, the escrow's
collateral minus the sum of the shares is ZERO. -/
theorem failure_refund_leaves_no_remainder
    (ordinaryCount supply multiplier : Nat) (positive : 0 < ordinaryCount)
    (holdings : List Nat)
    (exact : ∀ quantity ∈ holdings, ordinaryCount ∣ quantity * multiplier)
    (partition : holdings.sum = ordinaryCount * supply) :
    refundTotal ordinaryCount multiplier holdings = supply * multiplier ∧
    supply * multiplier - refundTotal ordinaryCount multiplier holdings = 0 := by
  have paid : refundTotal ordinaryCount multiplier holdings = supply * multiplier := by
    rw [failure_refund_is_additive_over_holders ordinaryCount multiplier positive
      holdings exact, partition,
      failure_refund_exhausts_the_escrow ordinaryCount supply multiplier positive]
  exact ⟨paid, by omega⟩

theorem failure_refund_never_exceeds_the_escrow
    (ordinaryCount supply multiplier quantity : Nat) (positive : 0 < ordinaryCount)
    (held : quantity ≤ ordinaryCount * supply) :
    failureRefund ordinaryCount quantity multiplier ≤ supply * multiplier := by
  have scaled : quantity * multiplier ≤ ordinaryCount * supply * multiplier :=
    Nat.mul_le_mul_right multiplier held
  have divided : quantity * multiplier / ordinaryCount
      ≤ ordinaryCount * supply * multiplier / ordinaryCount :=
    Nat.div_le_div_right scaled
  have collapse : ordinaryCount * supply * multiplier / ordinaryCount = supply * multiplier := by
    rw [Nat.mul_assoc]; exact Nat.mul_div_cancel_left _ positive
  unfold failureRefund
  omega

/-- A founder who holds no ordinary claims is paid nothing by an outage. -/
theorem a_founder_holding_no_ordinary_claims_is_paid_nothing
    (ordinaryCount multiplier : Nat) :
    failureRefund ordinaryCount 0 multiplier = 0 := by
  simp [failureRefund]

/-- A stranger holding half the ordinary claims draws half the escrow. -/
theorem half_the_ordinary_claims_draw_half_the_escrow
    (ordinaryCount supply multiplier : Nat) (positive : 0 < ordinaryCount) :
    2 * failureRefund ordinaryCount (ordinaryCount * supply) multiplier
      = failureRefund ordinaryCount (ordinaryCount * (2 * supply)) multiplier := by
  rw [failure_refund_exhausts_the_escrow ordinaryCount supply multiplier positive,
    failure_refund_exhausts_the_escrow ordinaryCount (2 * supply) multiplier positive]
  exact (Nat.mul_assoc 2 supply multiplier).symm

/-- Aggregate supply is uniform across every coordinate for as long as a Market
is open: the complete-set actions are the only ones that move it and both
refuse a non-uniform vector. -/
def uniformSupply (ordinaryCount : Nat) (state : State) (supply : Nat) : Prop :=
  ∀ outcome, outcome < outcomeWidth ordinaryCount → valueAt state.supply outcome = supply

/-- The failure coordinate's supply, valued in collateral, is exactly the sum
of the ordinary claims' pro-rata shares at resolution. -/
theorem the_failure_supply_equals_the_sum_of_ordinary_shares
    (ordinaryCount supply multiplier : Nat) (state : State)
    (positive : 0 < ordinaryCount)
    (uniform : uniformSupply ordinaryCount state supply)
    (holdings : List Nat)
    (exact : ∀ quantity ∈ holdings, ordinaryCount ∣ quantity * multiplier)
    (partition : holdings.sum = ordinaryCount * supply) :
    valueAt state.supply (failureSelector ordinaryCount) * multiplier
      = refundTotal ordinaryCount multiplier holdings := by
  rw [uniform (failureSelector ordinaryCount) (by simp [failureSelector, outcomeWidth]),
    (failure_refund_leaves_no_remainder ordinaryCount supply multiplier positive
      holdings exact partition).left]

/-- Terminal payout under the escrowed founding.  Off the failure walk this is
the kernel's own categorical payout scaled by the basis multiplier.  On the
failure walk the ordinary claims draw the escrow pro rata and the failure
claims -- which only the escrow holds -- draw nothing, so the Hoard is paid out
exactly once. -/
def escrowedRedemptionPayout
    (ordinaryCount : Nat) (phase : Phase) (outcome quantity multiplier : Nat) : Nat :=
  match terminalWinner? phase with
  | none => 0
  | some winner =>
      if winner = failureSelector ordinaryCount then
        if outcome < ordinaryCount then failureRefund ordinaryCount quantity multiplier
        else 0
      else if winner = outcome then quantity * multiplier
      else 0

/-- The honest walk is untouched: on any terminal that is not the failure
selector the escrowed payout is the kernel's existing categorical payout. -/
theorem escrowed_payout_agrees_with_the_kernel_off_the_failure_walk
    (ordinaryCount : Nat) (phase : Phase) (outcome quantity : Nat)
    (honest : ∀ winner, terminalWinner? phase = some winner →
      winner ≠ failureSelector ordinaryCount) :
    escrowedRedemptionPayout ordinaryCount phase outcome quantity 1
      = redemptionPayout phase outcome quantity := by
  unfold escrowedRedemptionPayout redemptionPayout
  cases h : terminalWinner? phase with
  | none => simp
  | some winner =>
      simp only [honest winner h, if_false, Option.some.injEq]
      by_cases hw : winner = outcome <;> simp [hw]

/-- The escrow's own failure claims pay nobody, so the escrow's collateral
leaves through the ordinary claims exactly once. -/
theorem the_escrow_pays_nobody_for_the_failure_coordinate
    (ordinaryCount quantity multiplier : Nat) :
    escrowedRedemptionPayout ordinaryCount
        (Phase.terminal (failureSelector ordinaryCount))
        (failureSelector ordinaryCount) quantity multiplier = 0 := by
  simp [escrowedRedemptionPayout, terminalWinner?, failureSelector]

/-- The failure walk pays out the escrow exactly: no more (solvency) and no
less (no remainder). -/
theorem the_failure_walk_pays_out_the_escrow_exactly
    (ordinaryCount supply multiplier outcome : Nat) (positive : 0 < ordinaryCount)
    (ordinary : outcome < ordinaryCount)
    (holdings : List Nat)
    (exact : ∀ quantity ∈ holdings, ordinaryCount ∣ quantity * multiplier)
    (partition : holdings.sum = ordinaryCount * supply) :
    (holdings.map fun quantity =>
        escrowedRedemptionPayout ordinaryCount
          (Phase.terminal (failureSelector ordinaryCount)) outcome quantity multiplier).sum
      = supply * multiplier := by
  have collapse : ∀ quantity : Nat,
      escrowedRedemptionPayout ordinaryCount
          (Phase.terminal (failureSelector ordinaryCount)) outcome quantity multiplier
        = failureRefund ordinaryCount quantity multiplier := by
    intro quantity
    simp [escrowedRedemptionPayout, terminalWinner?, ordinary]
  simp only [collapse]
  exact (failure_refund_leaves_no_remainder ordinaryCount supply multiplier positive
    holdings exact partition).left

/-- Under the founding admission every holder's share is exact, whatever they
hold: one ordinary claim redeems for exactly `unit` collateral atoms. -/
theorem an_admitted_founding_makes_every_refund_exact
    (ordinaryCount unit quantity : Nat) (positive : 0 < ordinaryCount) :
    failureRefund ordinaryCount quantity (ordinaryCount * unit) = quantity * unit := by
  unfold failureRefund
  rw [Nat.mul_comm quantity (ordinaryCount * unit), Nat.mul_assoc,
    Nat.mul_div_cancel_left _ positive, Nat.mul_comm]

/-- The unconditional conservation.  Under an admitted founding the failure
walk pays the escrow out to the last atom, for EVERY partition of the ordinary
claims among holders -- no divisibility hypothesis survives, so no holder can
be left with an unredeemable residue and no atom is left in the Hoard. -/
theorem an_admitted_failure_walk_leaves_no_remainder
    (ordinaryCount unit supply : Nat) (positive : 0 < ordinaryCount)
    (holdings : List Nat) (partition : holdings.sum = ordinaryCount * supply) :
    refundTotal ordinaryCount (ordinaryCount * unit) holdings
      = supply * (ordinaryCount * unit) ∧
    supply * (ordinaryCount * unit)
        - refundTotal ordinaryCount (ordinaryCount * unit) holdings = 0 := by
  have exact : ∀ quantity ∈ holdings, ordinaryCount ∣ quantity * (ordinaryCount * unit) :=
    fun quantity _ => ⟨quantity * unit, by
      rw [Nat.mul_comm quantity (ordinaryCount * unit), Nat.mul_assoc,
        Nat.mul_comm unit quantity]⟩
  exact failure_refund_leaves_no_remainder ordinaryCount supply (ordinaryCount * unit)
    positive holdings exact partition

/-- The failure terminal's payout vector: one collateral `unit` at every
ordinary column and nothing at the failure column.  The escrow's own claims pay
nobody, which is what lets the ordinary columns draw the whole Hoard exactly
once. -/
def failurePayoutVector (ordinaryCount unit : Nat) : List Nat :=
  List.replicate ordinaryCount unit ++ [0]

/-- The success terminal's payout vector for the same Market: the whole scale
at the winner and nothing anywhere else. -/
def successPayoutVector (ordinaryCount unit winner : Nat) : List Nat :=
  setAt (List.replicate (outcomeWidth ordinaryCount) 0) winner (ordinaryCount * unit)

theorem sum_set_replicate_zero (count index value : Nat) (present : index < count) :
    ((List.replicate count 0).set index value).sum = value := by
  induction count generalizing index with
  | zero => omega
  | succ count ih =>
      cases index with
      | zero => simp [List.replicate_succ]
      | succ index =>
          have inner : index < count := by omega
          simp [List.replicate_succ, ih index inner]

theorem failure_payout_vector_has_the_runtime_width (ordinaryCount unit : Nat) :
    (failurePayoutVector ordinaryCount unit).length = outcomeWidth ordinaryCount := by
  simp [failurePayoutVector, outcomeWidth]

theorem success_payout_vector_has_the_runtime_width
    (ordinaryCount unit winner : Nat) :
    (successPayoutVector ordinaryCount unit winner).length = outcomeWidth ordinaryCount := by
  simp [successPayoutVector, setAt]

/-- Both terminal arms partition the SAME payout scale, so the conservation
gate the terminal route already runs on every payout vector -- the vector must
sum to the scale -- admits the failure arm with nothing added to it. -/
theorem both_terminal_arms_partition_the_same_scale
    (ordinaryCount unit winner : Nat) (inRange : winner < outcomeWidth ordinaryCount) :
    (failurePayoutVector ordinaryCount unit).sum = ordinaryCount * unit ∧
    (successPayoutVector ordinaryCount unit winner).sum = ordinaryCount * unit := by
  constructor
  · simp [failurePayoutVector]
  · simp only [successPayoutVector, setAt]
    exact sum_set_replicate_zero (outcomeWidth ordinaryCount) winner
      (ordinaryCount * unit) inRange

end DClutch.Economic
