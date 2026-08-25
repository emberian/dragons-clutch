import DClutchSemantics.DirectProgram
import DClutchSemantics.Physical

/-!
# Compiled Direct output to physical plans

The controller does not accept claim or custody plans from its caller. It runs
the generated transition program, then constructs both child plans from the
successful output registers plus the frame's fill and outcome. Account and
signature authenticity remain adapter obligations. This file connects that
compilation step to the existing physical-plan refinement.
-/

namespace DClutch.Direct.CompiledPhysical

open DClutch
open DClutch.Direct
open DClutch.TransitionVM

/-- Construct the two child plans from transition-program output registers.
The fill and selected outcome remain admission-checked input registers;
successor nonces, gross quote, and fee come only from successful program
outputs. -/
def planFromOutputs
    (frame : FillFrame) : Nat × Nat × Nat × Nat → Physical.PhysicalPlan
  | (sellerNextNonce, buyerNextNonce, gross, fee) => {
      claimEffects := {
        effects := [
          .set sellerReplayCell sellerNextNonce,
          .set buyerReplayCell buyerNextNonce,
          .debit (sellerClaimCell frame.sellerIntent.outcome) frame.fill,
          .credit (buyerClaimCell frame.sellerIntent.outcome) frame.fill
        ]
      }
      custodyTransfers := [
        { source := .buyer, destination := .seller, amount := gross },
        { source := .buyer, destination := .venue, amount := fee }
      ]
    }

/-- Run the generated program and materialize child plans only after success. -/
def compilePhysicalPlan (frame : FillFrame) : Option Physical.PhysicalPlan :=
  ((run DirectProgram.program (DirectProgram.state frame)).bind
    DirectProgram.outputs).map (planFromOutputs frame)

theorem admitted_compiles_canonical_plan
    (frame : FillFrame) (admitted : Admissible frame) :
    compilePhysicalPlan frame = some (Physical.physicalPlan frame) := by
  unfold compilePhysicalPlan
  rw [DirectProgram.admitted_program_refines frame admitted]
  rfl

/-- End-to-end abstract composition: admitted semantic input selects the exact
canonical child plans, both children produce their named projections, and the
atomic join is the one high-level Direct post-state. This theorem does not model
account decoding, CPI, or the Solana rollback implementation. -/
theorem admitted_compilation_refines_physical_transition
    (frame : FillFrame) (admitted : Admissible frame) :
    compilePhysicalPlan frame = some (Physical.physicalPlan frame) ∧
      Physical.runClaimEffects frame.sellerIntent.outcome
          (Physical.physicalPlan frame).claimEffects.effects
          (Physical.claimPre frame) = some (Physical.claimPost frame) ∧
      Physical.runCustodyTransfers
          (Physical.physicalPlan frame).custodyTransfers
          (Physical.custodyPre frame) = some (Physical.custodyPost frame) ∧
      Physical.atomicCommit frame.pre
          (Physical.runClaimEffects frame.sellerIntent.outcome
            (Physical.physicalPlan frame).claimEffects.effects
            (Physical.claimPre frame))
          (Physical.runCustodyTransfers
            (Physical.physicalPlan frame).custodyTransfers
            (Physical.custodyPre frame)) = postState frame := by
  have claimRuns := Physical.claim_plan_refines frame admitted
  have custodyRuns := Physical.custody_plan_refines frame admitted
  refine ⟨admitted_compiles_canonical_plan frame admitted, claimRuns,
    custodyRuns, ?_⟩
  rw [claimRuns, custodyRuns]
  exact Physical.successful_atomic_commit frame

/-- Canonical physical plan bytes are a round-tripping projection of the typed
plans selected by compilation. The outcome-coordinate premise is the physical
V1 `u32` profile boundary; it is not a semantic Product-width restriction. -/
theorem admitted_physical_wire_round_trip
    (frame : FillFrame) (admitted : Admissible frame)
    (outcomeFits : frame.sellerIntent.outcome < 256 ^ 4) :
    DClutch.Codec.decodePlan
        (DClutch.Codec.encodePlan
          (Physical.physicalPlan frame).claimEffects) =
          some (Physical.physicalPlan frame).claimEffects ∧
      Physical.Codec.decodeCustodyPlan
        (Physical.Codec.encodeCustodyPlan
          (Physical.physicalPlan frame).custodyTransfers) =
          some (Physical.physicalPlan frame).custodyTransfers := by
  have sellerNonceFits : frame.pre.sellerNextNonce + 1 < 256 ^ 8 := by
    simpa [u64Limit] using admitted.sellerNonceCanAdvance
  have buyerNonceFits : frame.pre.buyerNextNonce + 1 < 256 ^ 8 := by
    simpa [u64Limit] using admitted.buyerNonceCanAdvance
  have fillFits : frame.fill < 256 ^ 8 := by
    simpa [u64Limit] using admitted.fillU64
  have grossFits : frame.gross < 256 ^ 8 := by
    simpa [u64Limit] using admitted.grossU64
  have feeFits : frame.fee < 256 ^ 8 := by
    simpa [u64Limit] using admitted.feeU64
  constructor
  · apply DClutch.Codec.decodePlan_encodePlan
    · simp [Physical.physicalPlan, DClutch.Codec.maxEffects]
    · simp [Physical.physicalPlan, DClutch.Codec.EffectEncodable,
        DClutch.Codec.effectCell, DClutch.Codec.outcomeCoordinate,
        DClutch.Codec.effectAmount, sellerReplayCell, buyerReplayCell,
        sellerClaimCell, buyerClaimCell, sellerNonceFits, buyerNonceFits,
        outcomeFits, fillFits]
  · apply Physical.Codec.decodeCustodyPlan_encode
    · simp [Physical.physicalPlan, Physical.Codec.maxCustodyTransfers]
    · simp [Physical.physicalPlan, Physical.Codec.CustodyTransferEncodable,
        grossFits, feeFits]

end DClutch.Direct.CompiledPhysical
