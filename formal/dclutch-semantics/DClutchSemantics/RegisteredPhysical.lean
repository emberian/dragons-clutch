import DClutchSemantics.DirectLifecycleProgram

/-!
# Registered Direct physical claim projection

Registered fills replace the ordinary Direct replay roots with the two
registration accounts themselves.  Their local sequences and residual
quantities are the only replay/fill authority.  Positions remain the sole
owners of native claim balances; collateral remains outside this projection.

The child instruction carries only the selected positive fill.  The claim
owner recomputes both residual transitions from the Lean-owned lifecycle
program before it changes either registration or either Position.
-/

namespace DClutch.Direct.RegisteredPhysical

open DClutch DClutch.Direct DClutch.DirectLifecycle DClutch.TransitionVM

/-- The exact mutable projection owned by the claim/replay child. -/
structure ClaimState where
  seller : DirectLifecycle.State
  buyer : DirectLifecycle.State
  sellerClaims : Nat
  buyerClaims : Nat
  deriving DecidableEq, Repr

/-- Program outputs committed atomically by the claim/replay child. -/
structure ClaimPlan where
  sellerRemaining : Nat
  sellerSequence : Nat
  sellerPhase : Nat
  buyerRemaining : Nat
  buyerSequence : Nat
  buyerPhase : Nat
  sellerClaims : Nat
  buyerClaims : Nat
  deriving DecidableEq, Repr

def preState (frame : DirectLifecycle.FillFrame) : ClaimState := {
  seller := frame.seller
  buyer := frame.buyer
  sellerClaims := frame.pre.sellerClaims
  buyerClaims := frame.pre.buyerClaims
}

def plan (frame : DirectLifecycle.FillFrame) : ClaimPlan := {
  sellerRemaining := (fillResult frame).seller.remaining
  sellerSequence := (fillResult frame).seller.sequence
  sellerPhase := DirectLifecycleProgram.phaseTag (fillResult frame).seller.phase
  buyerRemaining := (fillResult frame).buyer.remaining
  buyerSequence := (fillResult frame).buyer.sequence
  buyerPhase := DirectLifecycleProgram.phaseTag (fillResult frame).buyer.phase
  sellerClaims := (fillResult frame).ledger.sellerClaims
  buyerClaims := (fillResult frame).ledger.buyerClaims
}

/-- Execute the same generic residual program independently for both
authenticated registrations, then join those outputs with the conserved claim
movement. -/
def compile? (frame : DirectLifecycle.FillFrame) : Option ClaimPlan := do
  let seller <-
    (run DirectLifecycleProgram.program
      (DirectLifecycleProgram.state frame.seller frame.fill)).bind
        DirectLifecycleProgram.outputs
  let buyer <-
    (run DirectLifecycleProgram.program
      (DirectLifecycleProgram.state frame.buyer frame.fill)).bind
        DirectLifecycleProgram.outputs
  if frame.fill ≤ frame.pre.sellerClaims then
    let sellerClaims := frame.pre.sellerClaims - frame.fill
    let buyerClaims := frame.pre.buyerClaims + frame.fill
    if buyerClaims < u64Limit then
      some {
        sellerRemaining := seller.1
        sellerSequence := seller.2.1
        sellerPhase := seller.2.2
        buyerRemaining := buyer.1
        buyerSequence := buyer.2.1
        buyerPhase := buyer.2.2
        sellerClaims
        buyerClaims
      }
    else none
  else none

theorem admitted_compiles_exact_plan
    (frame : DirectLifecycle.FillFrame)
    (admitted : DirectLifecycle.FillAdmissible frame) :
    compile? frame = some (plan frame) := by
  unfold compile?
  rw [DirectLifecycleProgram.admitted_program_refines frame.seller frame.fill]
  · rw [DirectLifecycleProgram.admitted_program_refines frame.buyer frame.fill]
    · have direct := DirectLifecycle.direct_admitted frame admitted
      have sellerEnough : frame.fill ≤ frame.pre.sellerClaims := by
        simpa [DirectLifecycle.executionFrame, DirectLifecycle.executionLedger] using
          direct.sellerHasClaims
      have buyerFits : frame.pre.buyerClaims + frame.fill < u64Limit := by
        simpa [DirectLifecycle.executionFrame, DirectLifecycle.executionLedger] using
          direct.buyerClaimCreditFits
      simp [plan, DirectLifecycle.fillResult, DirectLifecycle.ledgerAfterFill,
        DirectLifecycle.executionFrame, DirectLifecycle.executionLedger,
        Direct.postState, DirectLifecycle.stateAfterFill,
        DirectLifecycleProgram.phaseTag, sellerEnough, buyerFits]
    · exact (DirectLifecycle.direct_admitted frame admitted).positiveFill
    · exact (DirectLifecycle.direct_admitted frame admitted).buyerLifecycle
    · simpa [DirectLifecycle.executionFrame, DirectLifecycle.executionLedger] using
        (DirectLifecycle.direct_admitted frame admitted).buyerNonceCanAdvance
  · exact (DirectLifecycle.direct_admitted frame admitted).positiveFill
  · exact (DirectLifecycle.direct_admitted frame admitted).sellerLifecycle
  · simpa [DirectLifecycle.executionFrame, DirectLifecycle.executionLedger] using
      (DirectLifecycle.direct_admitted frame admitted).sellerNonceCanAdvance

theorem admitted_claims_conserved
    (frame : DirectLifecycle.FillFrame)
    (admitted : DirectLifecycle.FillAdmissible frame) :
    (plan frame).sellerClaims + (plan frame).buyerClaims =
      frame.pre.sellerClaims + frame.pre.buyerClaims := by
  simpa [plan] using DirectLifecycle.claim_conservation frame admitted

/-! ## Exact child instruction -/

def instructionMagic : List UInt8 :=
  [0x44, 0x43, 0x52, 0x46, 1, 0, 0, 0] -- `DCRF`, V1

def instructionBytes : Nat := 16

def encodeInstruction (fill : Nat) : List UInt8 :=
  instructionMagic ++ DClutch.Codec.encodeLE 8 fill

def decodeInstruction (bytes : List UInt8) : Option Nat := do
  if bytes.length != instructionBytes then none else
  if bytes.take 8 != instructionMagic then none else
  let fill := DClutch.Codec.decodeLE (bytes.drop 8)
  if fill = 0 then none else some fill

theorem encode_instruction_length (fill : Nat) :
    (encodeInstruction fill).length = instructionBytes := by
  simp [encodeInstruction, instructionMagic, instructionBytes,
    DClutch.Codec.encodeLE_length]

theorem decode_encode_instruction (fill : Nat)
    (positive : 0 < fill) (fits : fill < 256 ^ 8) :
    decodeInstruction (encodeInstruction fill) = some fill := by
  simp [decodeInstruction, encodeInstruction, instructionMagic,
    instructionBytes, DClutch.Codec.encodeLE_length,
    DClutch.Codec.decodeLE_encodeLE, Nat.ne_of_gt positive, fits]

/-! ## Single-registration terminal routes -/

inductive TerminalAction where
  | cancel | expire
  deriving DecidableEq, Repr

structure TerminalPlan where
  phase : Nat
  sequence : Nat
  deriving DecidableEq, Repr

def cancelPlan? (frame : DirectLifecycle.CancelFrame) : Option TerminalPlan :=
  (DirectLifecycle.cancel frame).map fun state => {
    phase := DirectLifecycleProgram.phaseTag state.phase
    sequence := state.sequence
  }

def expirePlan? (frame : DirectLifecycle.ExpireFrame) : Option TerminalPlan :=
  (DirectLifecycle.expire frame).map fun state => {
    phase := DirectLifecycleProgram.phaseTag state.phase
    sequence := state.sequence
  }

theorem admitted_cancel_plan
    (frame : DirectLifecycle.CancelFrame)
    (admitted : DirectLifecycle.CancelAdmissible frame) :
    cancelPlan? frame = some { phase := 2, sequence := frame.state.sequence + 1 } := by
  simp [cancelPlan?, DirectLifecycle.cancel, admitted,
    DirectLifecycleProgram.phaseTag]

theorem admitted_expire_plan
    (frame : DirectLifecycle.ExpireFrame)
    (admitted : DirectLifecycle.ExpireAdmissible frame) :
    expirePlan? frame = some { phase := 3, sequence := frame.state.sequence + 1 } := by
  simp [expirePlan?, DirectLifecycle.expire, admitted,
    DirectLifecycleProgram.phaseTag]

def terminalInstructionMagic : TerminalAction → List UInt8
  | .cancel => [0x44, 0x43, 0x52, 0x43, 1, 0, 0, 0] -- `DCRC`, V1
  | .expire => [0x44, 0x43, 0x52, 0x45, 1, 0, 0, 0] -- `DCRE`, V1

def terminalInstructionBytes : Nat := 16

def encodeTerminalInstruction (action : TerminalAction) (expectedSequence : Nat) :
    List UInt8 :=
  terminalInstructionMagic action ++ DClutch.Codec.encodeLE 8 expectedSequence

def decodeTerminalInstruction (bytes : List UInt8) : Option (TerminalAction × Nat) := do
  if bytes.length != terminalInstructionBytes then none else
  let action <-
    if bytes.take 8 = terminalInstructionMagic .cancel then some TerminalAction.cancel
    else if bytes.take 8 = terminalInstructionMagic .expire then some TerminalAction.expire
    else none
  some (action, DClutch.Codec.decodeLE (bytes.drop 8))

theorem terminal_instruction_lengths (action : TerminalAction) (expectedSequence : Nat) :
    (encodeTerminalInstruction action expectedSequence).length = terminalInstructionBytes := by
  cases action <;>
    simp [encodeTerminalInstruction, terminalInstructionMagic,
      terminalInstructionBytes, DClutch.Codec.encodeLE_length]

theorem terminal_instruction_examples_round_trip :
    decodeTerminalInstruction (encodeTerminalInstruction .cancel 7) = some (.cancel, 7) ∧
    decodeTerminalInstruction (encodeTerminalInstruction .expire 9) = some (.expire, 9) := by
  native_decide

end DClutch.Direct.RegisteredPhysical
