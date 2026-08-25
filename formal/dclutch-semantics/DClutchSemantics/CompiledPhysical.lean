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

/-- Canonical claim projection selected from successful compiled outputs. -/
def claimPlanFromOutputs
    (sellerNextNonce buyerNextNonce outcome fill : Nat) : EffectPlan := {
  effects := [
    .set sellerReplayCell sellerNextNonce,
    .set buyerReplayCell buyerNextNonce,
    .debit (sellerClaimCell outcome) fill,
    .credit (buyerClaimCell outcome) fill
  ]
}

/-- Canonical custody projection selected from successful compiled outputs. -/
def custodyPlanFromOutputs (gross fee : Nat) : List Physical.CustodyTransfer := [
  { source := .buyer, destination := .seller, amount := gross },
  { source := .buyer, destination := .venue, amount := fee }
]

/-- Construct the two child plans from transition-program output registers.
The fill and selected outcome remain admission-checked input registers;
successor nonces, gross quote, and fee come only from successful program
outputs. -/
def planFromOutputs
    (frame : FillFrame) : Nat × Nat × Nat × Nat → Physical.PhysicalPlan
  | (sellerNextNonce, buyerNextNonce, gross, fee) => {
      claimEffects := claimPlanFromOutputs sellerNextNonce buyerNextNonce
        frame.sellerIntent.outcome frame.fill
      custodyTransfers := custodyPlanFromOutputs gross fee
    }

/-! ## Generated physical-plan ABI

The Rust controller starts from these Lean-encoded zero templates and patches
only the disjoint, named dynamic spans below. This removes its parallel copy of
headers, opcodes, parties, resources, and record geometry.
-/

def claimPlanTemplate : List UInt8 :=
  DClutch.Codec.encodePlan (claimPlanFromOutputs 0 0 0 0)

inductive ClaimPatch where
  | sellerNonce | buyerNonce | sellerOutcome | sellerFill | buyerOutcome | buyerFill
  deriving DecidableEq, Repr

namespace ClaimPatch

def all : List ClaimPatch := [
  .sellerNonce, .buyerNonce, .sellerOutcome, .sellerFill, .buyerOutcome, .buyerFill
]

def offset : ClaimPatch → Nat
  | .sellerNonce => DClutch.Codec.headerBytes + 8
  | .buyerNonce => DClutch.Codec.headerBytes + DClutch.Codec.effectBytes + 8
  | .sellerOutcome => DClutch.Codec.headerBytes + 2 * DClutch.Codec.effectBytes + 4
  | .sellerFill => DClutch.Codec.headerBytes + 2 * DClutch.Codec.effectBytes + 8
  | .buyerOutcome => DClutch.Codec.headerBytes + 3 * DClutch.Codec.effectBytes + 4
  | .buyerFill => DClutch.Codec.headerBytes + 3 * DClutch.Codec.effectBytes + 8

def width : ClaimPatch → Nat
  | .sellerOutcome | .buyerOutcome => 4
  | _ => 8

def rustName : ClaimPatch → String
  | .sellerNonce => "CLAIM_SELLER_NONCE_OFFSET"
  | .buyerNonce => "CLAIM_BUYER_NONCE_OFFSET"
  | .sellerOutcome => "CLAIM_SELLER_OUTCOME_OFFSET"
  | .sellerFill => "CLAIM_SELLER_FILL_OFFSET"
  | .buyerOutcome => "CLAIM_BUYER_OUTCOME_OFFSET"
  | .buyerFill => "CLAIM_BUYER_FILL_OFFSET"

theorem spans_are_bounded :
    ∀ patch ∈ all, offset patch + width patch ≤ claimPlanTemplate.length := by
  native_decide

theorem spans_are_disjoint :
    (all.flatMap fun patch => List.range' (offset patch) (width patch)).Nodup := by
  native_decide

theorem rust_names_are_unique : (all.map rustName).Nodup := by
  native_decide

end ClaimPatch

def custodyPlanTemplate : List UInt8 :=
  Physical.Codec.encodeCustodyPlan (custodyPlanFromOutputs 0 0)

inductive CustodyPatch where
  | gross | fee
  deriving DecidableEq, Repr

namespace CustodyPatch

def all : List CustodyPatch := [.gross, .fee]

def offset : CustodyPatch → Nat
  | .gross => Physical.Codec.custodyHeaderBytes + 8
  | .fee => Physical.Codec.custodyHeaderBytes +
      Physical.Codec.custodyTransferBytes + 8

def width (_ : CustodyPatch) : Nat := 8

def rustName : CustodyPatch → String
  | .gross => "CUSTODY_GROSS_OFFSET"
  | .fee => "CUSTODY_FEE_OFFSET"

theorem spans_are_bounded :
    ∀ patch ∈ all, offset patch + width patch ≤ custodyPlanTemplate.length := by
  native_decide

theorem spans_are_disjoint :
    (all.flatMap fun patch => List.range' (offset patch) (width patch)).Nodup := by
  native_decide

theorem rust_names_are_unique : (all.map rustName).Nodup := by
  native_decide

end CustodyPatch

/-- Pure model of one generated Rust little-endian template patch. -/
def patchLE
    (bytes : List UInt8) (offset width value : Nat) : List UInt8 :=
  bytes.take offset ++ DClutch.Codec.encodeLE width value ++
    bytes.drop (offset + width)

/-- The exact patch sequence emitted for the claim child. -/
def materializeClaimPlan
    (sellerNextNonce buyerNextNonce outcome fill : Nat) : List UInt8 :=
  let sellerNonce := patchLE claimPlanTemplate
    (ClaimPatch.offset .sellerNonce) (ClaimPatch.width .sellerNonce)
    sellerNextNonce
  let buyerNonce := patchLE sellerNonce
    (ClaimPatch.offset .buyerNonce) (ClaimPatch.width .buyerNonce)
    buyerNextNonce
  let sellerOutcome := patchLE buyerNonce
    (ClaimPatch.offset .sellerOutcome) (ClaimPatch.width .sellerOutcome) outcome
  let buyerOutcome := patchLE sellerOutcome
    (ClaimPatch.offset .buyerOutcome) (ClaimPatch.width .buyerOutcome) outcome
  let sellerFill := patchLE buyerOutcome
    (ClaimPatch.offset .sellerFill) (ClaimPatch.width .sellerFill) fill
  patchLE sellerFill
    (ClaimPatch.offset .buyerFill) (ClaimPatch.width .buyerFill) fill

/-- The exact patch sequence emitted for the custody child. -/
def materializeCustodyPlan (gross fee : Nat) : List UInt8 :=
  let grossBytes := patchLE custodyPlanTemplate
    (CustodyPatch.offset .gross) (CustodyPatch.width .gross) gross
  patchLE grossBytes
    (CustodyPatch.offset .fee) (CustodyPatch.width .fee) fee

theorem example_materialization_matches_encoding :
    materializeClaimPlan 1 1 1 2000 =
        DClutch.Codec.encodePlan (claimPlanFromOutputs 1 1 1 2000) ∧
      materializeCustodyPlan 1000 2 =
        Physical.Codec.encodeCustodyPlan (custodyPlanFromOutputs 1000 2) := by
  native_decide

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
