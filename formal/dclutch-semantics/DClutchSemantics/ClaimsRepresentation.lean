import DClutchSemantics.EconomicKernel
import DClutchSemantics.ExecutionRelease

/-!
# Data-specialized claim representations

Bearer claims, structured baskets, and fractional receipts are instances of one
immutable descriptor.  The descriptor says how many claim atoms of each
Product outcome make one exact lot, and how many adapter receipt units represent
that lot.  No lifecycle branch depends on a presentation label:

* a one-hot unit lot is the familiar bearer presentation;
* a multi-outcome lot is a structured presentation; and
* more than one receipt unit per lot is a fractional presentation.

The wrapper state owns only replay state, the number of issued lots, and its own
retirement bit.  `Economic.State` remains the sole owner of Market supply,
native/materialized partition, and Hoard principal.  A registry-authenticated
release admission and an authenticated adapter projection are explicit inputs.
Token-2022, CPI, mint/account parsing, and transaction rollback are deliberately
outside this semantic layer.

Lean lists are the mathematical width-polymorphic representation.  A physical
kernel must refine them to caller-owned fixed-layout slices; this module does
not authorize allocation in first-party onchain code.
-/

namespace DClutch.ClaimsRepresentation

open DClutch
open DClutch.Economic
open DClutch.ExecutionRelease

/-! ## Immutable descriptor -/

/-- Immutable representation data.  The Product-owned outcome ordering is
identified, never redefined here. -/
structure Descriptor where
  descriptorId : Nat
  marketId : Nat
  productId : Nat
  resultDomainId : Nat
  adapterAssetId : Nat
  outcomeCount : Nat
  claimAtomsPerLot : List Nat
  receiptUnitsPerLot : Nat
  releaseSetId : ExecutionRelease.Identity
  deriving DecidableEq, Repr

def anyPositive (values : List Nat) : Bool :=
  values.any fun value => 0 < value

def Descriptor.valid (scalarLimit : Nat) (descriptor : Descriptor) : Bool :=
  0 < scalarLimit && descriptor.descriptorId != 0 &&
  descriptor.marketId != 0 && descriptor.productId != 0 &&
  descriptor.resultDomainId != 0 && descriptor.adapterAssetId != 0 &&
  0 < descriptor.outcomeCount &&
  descriptor.claimAtomsPerLot.length = descriptor.outcomeCount &&
  anyPositive descriptor.claimAtomsPerLot &&
  0 < descriptor.receiptUnitsPerLot &&
  descriptor.receiptUnitsPerLot < scalarLimit &&
  descriptor.claimAtomsPerLot.all fun atoms => atoms < scalarLimit &&
  identityValid descriptor.releaseSetId

def Descriptor.expectedClaims (descriptor : Descriptor) (lots : Nat) : List Nat :=
  descriptor.claimAtomsPerLot.map fun atoms => atoms * lots

def Descriptor.expectedReceiptUnits (descriptor : Descriptor) (lots : Nat) : Nat :=
  descriptor.receiptUnitsPerLot * lots

def vectorAdd (left right : List Nat) : List Nat :=
  List.zipWith (.+.) left right

/-- A presentation classification is derived documentation, not economic
authority and not an executable dispatch tag. -/
def Descriptor.isBearerPresentation (descriptor : Descriptor) : Bool :=
  descriptor.receiptUnitsPerLot = 1 &&
  descriptor.claimAtomsPerLot.countP (fun atoms => atoms != 0) = 1 &&
  descriptor.claimAtomsPerLot.all fun atoms => atoms = 0 || atoms = 1

def Descriptor.isStructuredPresentation (descriptor : Descriptor) : Bool :=
  descriptor.receiptUnitsPerLot = 1 && !descriptor.isBearerPresentation

def Descriptor.isFractionalPresentation (descriptor : Descriptor) : Bool :=
  1 < descriptor.receiptUnitsPerLot

/-! ## Explicit trust boundaries -/

/-- Authenticated projection supplied by a token/mint adapter.  It is generic
because Token-2022 is one possible representation adapter, not protocol
semantics. -/
structure AdapterProjection where
  adapterAuthenticated : Bool
  descriptorId : Nat
  adapterAssetId : Nat
  observedReceiptUnits : Nat
  deriving DecidableEq, Repr

def AdapterProjection.accepts
    (descriptor : Descriptor) (issuedLots : Nat)
    (projection : AdapterProjection) : Bool :=
  projection.adapterAuthenticated &&
  projection.descriptorId = descriptor.descriptorId &&
  projection.adapterAssetId = descriptor.adapterAssetId &&
  projection.observedReceiptUnits = descriptor.expectedReceiptUnits issuedLots

/-- Obligations the physical adapter must establish.  This structure is
descriptive: none of these runtime facts is proved by the pure model. -/
structure AdapterBoundary where
  authenticatesRegistry : Bool
  authenticatesProgramData : Bool
  authenticatesDescriptorAccount : Bool
  authenticatesAssetAndSupply : Bool
  authenticatesHolderAndAmount : Bool
  authenticatesMintAndBurnAuthority : Bool
  appliesEconomicAndAdapterEffectsAtomically : Bool
  deriving DecidableEq, Repr

def AdapterBoundary.complete (boundary : AdapterBoundary) : Bool :=
  boundary.authenticatesRegistry && boundary.authenticatesProgramData &&
  boundary.authenticatesDescriptorAccount &&
  boundary.authenticatesAssetAndSupply &&
  boundary.authenticatesHolderAndAmount &&
  boundary.authenticatesMintAndBurnAuthority &&
  boundary.appliesEconomicAndAdapterEffectsAtomically

/-! ## Wrapper state and commands -/

/-- Capability-owned state.  Market lifecycle is intentionally absent: the
economic state is its one semantic owner. -/
structure State where
  descriptorId : Nat
  nextNonce : Nat
  issuedLots : Nat
  retired : Bool
  deriving DecidableEq, Repr

inductive Command where
  | issue (lots nonce : Nat)
  | redeem (lots nonce : Nat)
  | redeemTerminal (lots nonce : Nat)
  | retire (nonce : Nat)
  deriving DecidableEq, Repr

def Command.nonce : Command → Nat
  | .issue _ nonce | .redeem _ nonce | .redeemTerminal _ nonce | .retire nonce => nonce

def Command.lots : Command → Nat
  | .issue lots _ | .redeem lots _ | .redeemTerminal lots _ => lots
  | .retire _ => 0

/-- Abstract authenticated parties.  The adapter binds these to physical
accounts. -/
structure Parties where
  claimant : Party
  wrapper : Party
  hoard : Party
  deriving DecidableEq, Repr

def Parties.distinct (parties : Parties) : Bool :=
  parties.claimant != parties.wrapper && parties.claimant != parties.hoard &&
  parties.wrapper != parties.hoard

structure Frame where
  scalarLimit : Nat
  descriptor : Descriptor
  admission : ExecutionRelease.Admission
  adapterPre : AdapterProjection
  parties : Parties
  wrapperPre : State
  economicPre : Economic.State
  command : Command
  deriving DecidableEq, Repr

def terminalLike : Economic.Phase → Bool
  | .terminal _ | .retiring _ | .retired => true
  | .open => false

def expectedProjection
    (descriptor : Descriptor) (state : State) : List Nat :=
  descriptor.expectedClaims state.issuedLots

/-- The operation selects which local Economic projection denotes this
wrapper.  This is an account-role view, not another aggregate supply truth. -/
def projectionLinked
    (descriptor : Descriptor) (state : State) (command : Command)
    (economic : Economic.State) : Bool :=
  match command with
  | .issue .. => economic.destinationMaterialized = expectedProjection descriptor state
  | .redeem .. | .redeemTerminal .. | .retire .. =>
      economic.sourceMaterialized = expectedProjection descriptor state

def productsFit (frame : Frame) (lots : Nat) : Bool :=
  frame.descriptor.expectedReceiptUnits lots < frame.scalarLimit &&
  (frame.descriptor.expectedClaims lots).all fun quantity => quantity < frame.scalarLimit

def postProductsFit (frame : Frame) (lots : Nat) : Bool :=
  frame.descriptor.expectedReceiptUnits (frame.wrapperPre.issuedLots + lots) <
      frame.scalarLimit &&
  (frame.descriptor.expectedClaims (frame.wrapperPre.issuedLots + lots)).all
    fun quantity => quantity < frame.scalarLimit

def staticAccepts (frame : Frame) : Bool :=
  frame.descriptor.valid frame.scalarLimit &&
  frame.descriptor.releaseSetId = frame.admission.marketReleaseSetId &&
  ExecutionRelease.admits frame.admission .claims &&
  frame.adapterPre.accepts frame.descriptor frame.wrapperPre.issuedLots &&
  frame.parties.distinct &&
  frame.wrapperPre.descriptorId = frame.descriptor.descriptorId &&
  frame.wrapperPre.nextNonce < frame.scalarLimit &&
  frame.wrapperPre.issuedLots < frame.scalarLimit &&
  productsFit frame frame.wrapperPre.issuedLots &&
  frame.command.nonce = frame.wrapperPre.nextNonce &&
  projectionLinked frame.descriptor frame.wrapperPre frame.command frame.economicPre &&
  Economic.valid frame.descriptor.outcomeCount frame.scalarLimit frame.economicPre

def commandAccepts (frame : Frame) : Bool :=
  !frame.wrapperPre.retired && frame.wrapperPre.nextNonce + 1 < frame.scalarLimit &&
  match frame.command with
  | .issue lots _ =>
      frame.economicPre.phase = .open && 0 < lots && productsFit frame lots &&
      postProductsFit frame lots
  | .redeem lots _ =>
      0 < lots && lots ≤ frame.wrapperPre.issuedLots && productsFit frame lots
  | .redeemTerminal lots _ =>
      terminalLike frame.economicPre.phase && 0 < lots &&
      lots ≤ frame.wrapperPre.issuedLots && productsFit frame lots
  | .retire _ =>
      terminalLike frame.economicPre.phase && frame.wrapperPre.issuedLots = 0 &&
      frame.adapterPre.observedReceiptUnits = 0

def accepts (frame : Frame) : Bool :=
  staticAccepts frame && commandAccepts frame

def indexedQuantities (descriptor : Descriptor) (lots : Nat) : List (Nat × Nat) :=
  (List.range descriptor.outcomeCount).zip (descriptor.expectedClaims lots)

def nonzeroQuantities (descriptor : Descriptor) (lots : Nat) : List (Nat × Nat) :=
  (indexedQuantities descriptor lots).filter fun entry => entry.2 != 0

def issueCommands (descriptor : Descriptor) (lots : Nat) : List Economic.Command :=
  (nonzeroQuantities descriptor lots).map fun entry =>
    .materializeClaim entry.1 entry.2

def redeemCommands (descriptor : Descriptor) (lots : Nat) : List Economic.Command :=
  (nonzeroQuantities descriptor lots).map fun entry =>
    .dematerializeClaim entry.1 entry.2

def terminalCommands (descriptor : Descriptor) (lots : Nat) : List Economic.Command :=
  redeemCommands descriptor lots ++
  (nonzeroQuantities descriptor lots).map fun entry =>
    .redeemTerminal .destination .native entry.1 entry.2

def economicCommands (frame : Frame) : List Economic.Command :=
  match frame.command with
  | .issue lots _ => issueCommands frame.descriptor lots
  | .redeem lots _ => redeemCommands frame.descriptor lots
  | .redeemTerminal lots _ => terminalCommands frame.descriptor lots
  | .retire _ => []

def economicBindings (frame : Frame) : Economic.Bindings :=
  match frame.command with
  | .issue .. => {
      source := frame.parties.claimant
      destination := frame.parties.wrapper
      hoard := frame.parties.hoard
    }
  | .redeem .. | .redeemTerminal .. | .retire .. => {
      source := frame.parties.wrapper
      destination := frame.parties.claimant
      hoard := frame.parties.hoard
    }

def economicFrame
    (frame : Frame) (pre : Economic.State) (command : Economic.Command) :
    Economic.Frame := {
  outcomeCount := frame.descriptor.outcomeCount
  scalarLimit := frame.scalarLimit
  bindings := economicBindings frame
  pre
  command
}

inductive Refusal where
  | notAdmissible
  | economic (cause : Economic.Refusal)
  | candidateInvariantFailure
  deriving DecidableEq, Repr

/-- Execute the descriptor-produced Economic program without width-specific
dispatch.  Every step reuses the total Economic kernel boundary. -/
def executeEconomic? (frame : Frame) :
    List Economic.Command → Economic.State → Except Refusal Economic.State
  | [], state => .ok state
  | command :: rest, state =>
      match Economic.execute? (economicFrame frame state command) with
      | .error cause => .error (.economic cause)
      | .ok settlement => executeEconomic? frame rest settlement.post

def wrapperPost (frame : Frame) : State :=
  match frame.command with
  | .issue lots _ => {
      frame.wrapperPre with
      nextNonce := frame.wrapperPre.nextNonce + 1
      issuedLots := frame.wrapperPre.issuedLots + lots
    }
  | .redeem lots _ | .redeemTerminal lots _ => {
      frame.wrapperPre with
      nextNonce := frame.wrapperPre.nextNonce + 1
      issuedLots := frame.wrapperPre.issuedLots - lots
    }
  | .retire _ => {
      frame.wrapperPre with
      nextNonce := frame.wrapperPre.nextNonce + 1
      retired := true
    }

inductive AdapterMutation where
  | mint (recipient : Party) (receiptUnits : Nat)
  | burn (owner : Party) (receiptUnits : Nat)
  | retire
  deriving DecidableEq, Repr

def adapterMutation (frame : Frame) : AdapterMutation :=
  match frame.command with
  | .issue lots _ =>
      .mint frame.parties.claimant (frame.descriptor.expectedReceiptUnits lots)
  | .redeem lots _ | .redeemTerminal lots _ =>
      .burn frame.parties.claimant (frame.descriptor.expectedReceiptUnits lots)
  | .retire _ => .retire

structure Settlement (frame : Frame) where
  wrapperPost : State
  economicPost : Economic.State
  adapter : AdapterMutation
  accepted : accepts frame = true
  wrapperExact : wrapperPost = ClaimsRepresentation.wrapperPost frame
  adapterExact : adapter = adapterMutation frame
  economicLinked :
    projectionLinked frame.descriptor wrapperPost frame.command economicPost = true

/-- Total specialization boundary.  Adapter mutation is exposed only after the
whole Economic command sequence and the candidate representation invariant
succeed. -/
def execute? (frame : Frame) : Except Refusal (Settlement frame) :=
  if accepted : accepts frame = true then
    match executeEconomic? frame (economicCommands frame) frame.economicPre with
    | .error cause => .error cause
    | .ok economicPost =>
        let candidate := wrapperPost frame
        if linked : projectionLinked frame.descriptor candidate frame.command economicPost = true then
          .ok {
            wrapperPost := candidate
            economicPost
            adapter := adapterMutation frame
            accepted
            wrapperExact := rfl
            adapterExact := rfl
            economicLinked := linked
          }
        else .error .candidateInvariantFailure
  else .error .notAdmissible

/-- Observable capability state rolls back on every refusal.  Atomic physical
rollback across Economic state and adapter state remains an adapter obligation. -/
def runWrapperState (frame : Frame) : State :=
  match execute? frame with
  | .ok settlement => settlement.wrapperPost
  | .error _ => frame.wrapperPre

def runEconomicState (frame : Frame) : Economic.State :=
  match execute? frame with
  | .ok settlement => settlement.economicPost
  | .error _ => frame.economicPre

def emittedAdapterMutation? (frame : Frame) : Option AdapterMutation :=
  match execute? frame with
  | .ok settlement => some settlement.adapter
  | .error _ => none

def succeeded (frame : Frame) : Bool :=
  match execute? frame with
  | .ok _ => true
  | .error _ => false

theorem refusal_rolls_back_wrapper
    (frame : Frame) (refusal : Refusal)
    (failed : execute? frame = .error refusal) :
    runWrapperState frame = frame.wrapperPre := by
  unfold runWrapperState
  rw [failed]

theorem refusal_rolls_back_economic_projection
    (frame : Frame) (refusal : Refusal)
    (failed : execute? frame = .error refusal) :
    runEconomicState frame = frame.economicPre := by
  unfold runEconomicState
  rw [failed]

theorem successful_wrapper_post_is_exact
    (frame : Frame) (settlement : Settlement frame) :
    settlement.wrapperPost = wrapperPost frame :=
  settlement.wrapperExact

theorem successful_adapter_mutation_is_exact
    (frame : Frame) (settlement : Settlement frame) :
    settlement.adapter = adapterMutation frame :=
  settlement.adapterExact

theorem successful_projection_is_linked
    (frame : Frame) (settlement : Settlement frame) :
    projectionLinked frame.descriptor settlement.wrapperPost frame.command
      settlement.economicPost = true :=
  settlement.economicLinked

/-! ## Exact lot arithmetic -/

theorem expected_receipt_units_add
    (descriptor : Descriptor) (left right : Nat) :
    descriptor.expectedReceiptUnits (left + right) =
      descriptor.expectedReceiptUnits left + descriptor.expectedReceiptUnits right := by
  simp [Descriptor.expectedReceiptUnits, Nat.mul_add]

theorem expected_claims_add
    (descriptor : Descriptor) (left right : Nat) :
    descriptor.expectedClaims (left + right) =
      vectorAdd (descriptor.expectedClaims left) (descriptor.expectedClaims right) := by
  simp [Descriptor.expectedClaims, vectorAdd, List.zipWith_map_left,
    List.zipWith_map_right, Nat.mul_add]

theorem issue_lot_accounting_is_exact
    (frame : Frame) (lots nonce : Nat) (command : frame.command = .issue lots nonce) :
    (wrapperPost frame).issuedLots = frame.wrapperPre.issuedLots + lots ∧
    frame.descriptor.expectedReceiptUnits (wrapperPost frame).issuedLots =
      frame.descriptor.expectedReceiptUnits frame.wrapperPre.issuedLots +
        frame.descriptor.expectedReceiptUnits lots := by
  simp [wrapperPost, command, expected_receipt_units_add]

theorem redeem_lot_accounting_is_exact
    (frame : Frame) (lots nonce : Nat) (command : frame.command = .redeem lots nonce)
    (available : lots ≤ frame.wrapperPre.issuedLots) :
    (wrapperPost frame).issuedLots + lots = frame.wrapperPre.issuedLots ∧
    frame.descriptor.expectedReceiptUnits (wrapperPost frame).issuedLots +
        frame.descriptor.expectedReceiptUnits lots =
      frame.descriptor.expectedReceiptUnits frame.wrapperPre.issuedLots := by
  simp [wrapperPost, command, Descriptor.expectedReceiptUnits]
  constructor
  · omega
  · rw [← Nat.mul_add, Nat.sub_add_cancel available]

theorem descriptor_program_has_no_zero_quantity_commands
    (descriptor : Descriptor) (lots : Nat)
    (command : Economic.Command) (member : command ∈ issueCommands descriptor lots) :
    match command with
    | .materializeClaim _ quantity => 0 < quantity
    | _ => False := by
  simp only [issueCommands, List.mem_map] at member
  obtain ⟨entry, entryMember, rfl⟩ := member
  have nonzero : entry.2 != 0 := (List.mem_filter.mp entryMember).2
  have nonzeroProp : entry.2 ≠ 0 := by
    intro equal
    simp [equal] at nonzero
  exact Nat.pos_of_ne_zero nonzeroProp

end DClutch.ClaimsRepresentation
