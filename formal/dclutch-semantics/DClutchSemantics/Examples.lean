import DClutchSemantics.DirectProofs
import DClutchSemantics.Codec
import DClutchSemantics.Physical

/-!
# Executable examples

These are regression fixtures for the formal semantics, not deployment evidence.
-/

namespace DClutch.Direct.Examples

open DClutch

def binaryProduct : ProductIR := {
  outcomeCount := 2
  outcomeCountPositive := by decide
  priceScale := 1000000
  priceScalePositive := by decide
}

def feePolicy : FeePolicy := {
  basisPoints := 25
  basisPointsBounded := by decide
}

def sellerIntent : Intent := {
  market := 101
  generation := 3
  maker := 11
  nonce := 0
  validFromSlot := 90
  validThroughSlot := 110
  side := .sell
  lifecycle := .fillOrKill
  outcome := 1
  maxFill := 2000
  limitPrice := 400000
  feePolicyId := 77
}

def buyerIntent : Intent := {
  market := 101
  generation := 3
  maker := 12
  nonce := 0
  validFromSlot := 95
  validThroughSlot := 120
  side := .buy
  lifecycle := .fillOrKill
  outcome := 1
  maxFill := 2000
  limitPrice := 600000
  feePolicyId := 77
}

def frame : FillFrame := {
  product := binaryProduct
  feePolicy := feePolicy
  feePolicyId := 77
  phase := .open
  slot := 100
  sellerIntent := sellerIntent
  buyerIntent := buyerIntent
  pre := {
    sellerNextNonce := 0
    buyerNextNonce := 0
    sellerClaims := 5000
    buyerClaims := 200
    buyerCollateral := 2000
    sellerCollateral := 100
    venueCollateral := 20
  }
  fill := 2000
  executionPrice := 500000
  gross := 1000
  fee := 2
}

theorem frame_admissible : Admissible frame := by
  apply (accepts_iff frame).mp
  native_decide

theorem frame_effects_execute :
    runEffects frame.sellerIntent.outcome (effectPlan frame).effects frame.pre =
      some (postState frame) :=
  effectPlan_refines_transition frame frame_admissible

theorem frame_encoded_plan_length :
    (Codec.encodePlan (effectPlan frame)).length = 120 := by
  native_decide

theorem frame_encoded_plan_hex :
    Codec.hex (Codec.encodePlan (effectPlan frame)) =
      "444345460107000000000000000000000100000000000000000100000000000001000000000000000100010001000000d0070000000000000201010001000000d0070000000000000101020000000000ea030000000000000200020000000000e80300000000000002020200000000000200000000000000" := by
  native_decide

theorem frame_encoded_claim_plan_hex :
    Codec.hex (Codec.encodePlan (Physical.physicalPlan frame).claimEffects) =
      "444345460104000000000000000000000100000000000000000100000000000001000000000000000100010001000000d0070000000000000201010001000000d007000000000000" := by
  native_decide

theorem frame_encoded_custody_plan_hex :
    Codec.hex (Physical.Codec.encodeCustodyPlan
      (Physical.physicalPlan frame).custodyTransfers) =
      "44434350010200000100000000000000e80300000000000001020000000000000200000000000000" := by
  native_decide

theorem frame_post_state :
    (execute frame frame_admissible).post = {
      sellerNextNonce := 1
      buyerNextNonce := 1
      sellerClaims := 3000
      buyerClaims := 2200
      buyerCollateral := 998
      sellerCollateral := 1100
      venueCollateral := 22
    } := by
  native_decide

def hostileZeroFill : FillFrame := { frame with fill := 0, gross := 0, fee := 0 }

theorem hostile_zero_fill_rejected : accepts hostileZeroFill = false := by
  native_decide

theorem hostile_zero_fill_refuses : execute? hostileZeroFill = .error .notAdmissible :=
  rejected_execute_refuses hostileZeroFill hostile_zero_fill_rejected

theorem hostile_zero_fill_rolls_back : runLedger hostileZeroFill = hostileZeroFill.pre :=
  rejection_rolls_back hostileZeroFill hostile_zero_fill_rejected

end DClutch.Direct.Examples
