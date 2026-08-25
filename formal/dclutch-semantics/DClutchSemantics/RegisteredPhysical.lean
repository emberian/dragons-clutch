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

end DClutch.Direct.RegisteredPhysical
