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

end DClutch.Economic
