import DClutchSemantics.IR
import Std.Tactic

/-!
# Lean-owned Direct-fill semantics

This module defines the protocol meaning of one inline ordinary fill.  It does
not model Solana transport, Ed25519 parsing, CPI, or account memory.  Those are
separately named adapter obligations.
-/

namespace DClutch.Direct

open DClutch

/-- One more than the greatest value representable by an onchain `u64`. -/
def u64Limit : Nat := 18446744073709551616

/-- Basis-point denominator. -/
def feeDenominator : Nat := 10000

/-- Canonical Market phase relevant to the Direct slice. -/
inductive Phase where
  | founding
  | open
  | resolved
  | retiring
  | retired
  deriving DecidableEq, Repr

/-- Signed order direction. -/
inductive Side where
  | sell
  | buy
  deriving DecidableEq, Repr

/-- Inline execution promise. -/
inductive Lifecycle where
  | fillOrKill
  | immediateOrCancel
  deriving DecidableEq, Repr

/-- Immutable fee policy selected by the Market capability manifest. -/
structure FeePolicy where
  basisPoints : Nat
  basisPointsBounded : basisPoints ≤ feeDenominator

/-- Semantic facts bound by one maker signature. -/
structure Intent where
  market : Nat
  generation : Nat
  maker : Nat
  nonce : Nat
  validFromSlot : Nat
  validThroughSlot : Nat
  side : Side
  lifecycle : Lifecycle
  outcome : Nat
  maxFill : Nat
  limitPrice : Nat
  feePolicyId : Nat
  feeBasisPoints : Nat
  deriving DecidableEq, Repr

/-- The exact mutable state projection touched by an ordinary fill. -/
structure Ledger where
  sellerNextNonce : Nat
  buyerNextNonce : Nat
  sellerClaims : Nat
  buyerClaims : Nat
  buyerCollateral : Nat
  sellerCollateral : Nat
  venueCollateral : Nat
  deriving DecidableEq, Repr

/-- Every field that will inhabit a physical `u64` is in range. -/
def Ledger.U64Valid (state : Ledger) : Prop :=
  state.sellerNextNonce < u64Limit ∧
  state.buyerNextNonce < u64Limit ∧
  state.sellerClaims < u64Limit ∧
  state.buyerClaims < u64Limit ∧
  state.buyerCollateral < u64Limit ∧
  state.sellerCollateral < u64Limit ∧
  state.venueCollateral < u64Limit

instance (state : Ledger) : Decidable state.U64Valid := by
  unfold Ledger.U64Valid
  infer_instance

/-- Untrusted candidate frame. `gross` and `fee` are checked witnesses, not
authoritative caller assertions. -/
structure FillFrame where
  product : ProductIR
  feePolicy : FeePolicy
  feePolicyId : Nat
  phase : Phase
  slot : Nat
  sellerIntent : Intent
  buyerIntent : Intent
  pre : Ledger
  fill : Nat
  executionPrice : Nat
  gross : Nat
  fee : Nat

/-- FOK and IOC admission is stated once and shared by both sides. -/
def Lifecycle.accepts (lifecycle : Lifecycle) (maxFill fill : Nat) : Prop :=
  match lifecycle with
  | .fillOrKill => fill = maxFill
  | .immediateOrCancel => fill ≤ maxFill

instance (lifecycle : Lifecycle) (maxFill fill : Nat) :
    Decidable (lifecycle.accepts maxFill fill) := by
  cases lifecycle <;> simp [Lifecycle.accepts] <;> infer_instance

/-- Complete semantic admission predicate for one inline ordinary fill.

The Solana adapter must establish signature/account authenticity before asking
this semantic kernel to decide the economic transition.
-/
structure Admissible (frame : FillFrame) : Prop where
  preU64 : frame.pre.U64Valid
  phaseOpen : frame.phase = .open
  positiveFill : 0 < frame.fill
  slotAfterStart : frame.sellerIntent.validFromSlot ≤ frame.slot
  slotBeforeSellerEnd : frame.slot ≤ frame.sellerIntent.validThroughSlot
  buyerSlotAfterStart : frame.buyerIntent.validFromSlot ≤ frame.slot
  buyerSlotBeforeEnd : frame.slot ≤ frame.buyerIntent.validThroughSlot
  sellerSide : frame.sellerIntent.side = .sell
  buyerSide : frame.buyerIntent.side = .buy
  sameMarket : frame.sellerIntent.market = frame.buyerIntent.market
  sameGeneration : frame.sellerIntent.generation = frame.buyerIntent.generation
  sameOutcome : frame.sellerIntent.outcome = frame.buyerIntent.outcome
  distinctMakers : frame.sellerIntent.maker ≠ frame.buyerIntent.maker
  outcomeInDomain : frame.sellerIntent.outcome < frame.product.outcomeCount
  sellerLifecycle : frame.sellerIntent.lifecycle.accepts frame.sellerIntent.maxFill frame.fill
  buyerLifecycle : frame.buyerIntent.lifecycle.accepts frame.buyerIntent.maxFill frame.fill
  sellerNonce : frame.sellerIntent.nonce = frame.pre.sellerNextNonce
  buyerNonce : frame.buyerIntent.nonce = frame.pre.buyerNextNonce
  sellerNonceCanAdvance : frame.pre.sellerNextNonce + 1 < u64Limit
  buyerNonceCanAdvance : frame.pre.buyerNextNonce + 1 < u64Limit
  sellerPrice : frame.sellerIntent.limitPrice ≤ frame.executionPrice
  buyerPrice : frame.executionPrice ≤ frame.buyerIntent.limitPrice
  priceInScale : frame.executionPrice ≤ frame.product.priceScale
  sellerFeePolicy : frame.sellerIntent.feePolicyId = frame.feePolicyId
  buyerFeePolicy : frame.buyerIntent.feePolicyId = frame.feePolicyId
  sellerFeeRate : frame.sellerIntent.feeBasisPoints = frame.feePolicy.basisPoints
  buyerFeeRate : frame.buyerIntent.feeBasisPoints = frame.feePolicy.basisPoints
  exactQuote : frame.fill * frame.executionPrice = frame.gross * frame.product.priceScale
  exactFloorFee : frame.fee = frame.gross * frame.feePolicy.basisPoints / feeDenominator
  fillU64 : frame.fill < u64Limit
  priceU64 : frame.executionPrice < u64Limit
  grossU64 : frame.gross < u64Limit
  feeU64 : frame.fee < u64Limit
  sellerHasClaims : frame.fill ≤ frame.pre.sellerClaims
  buyerHasCollateral : frame.gross + frame.fee ≤ frame.pre.buyerCollateral
  buyerClaimCreditFits : frame.pre.buyerClaims + frame.fill < u64Limit
  sellerCollateralCreditFits : frame.pre.sellerCollateral + frame.gross < u64Limit
  venueCreditFits : frame.pre.venueCollateral + frame.fee < u64Limit

/-- Executable admission decision. Its equivalence to `Admissible` is checked
below, so the Boolean boundary cannot silently drift from the proof predicate. -/
def accepts (frame : FillFrame) : Bool :=
  decide frame.pre.U64Valid &&
  decide (frame.phase = .open) &&
  decide (0 < frame.fill) &&
  decide (frame.sellerIntent.validFromSlot ≤ frame.slot) &&
  decide (frame.slot ≤ frame.sellerIntent.validThroughSlot) &&
  decide (frame.buyerIntent.validFromSlot ≤ frame.slot) &&
  decide (frame.slot ≤ frame.buyerIntent.validThroughSlot) &&
  decide (frame.sellerIntent.side = .sell) &&
  decide (frame.buyerIntent.side = .buy) &&
  decide (frame.sellerIntent.market = frame.buyerIntent.market) &&
  decide (frame.sellerIntent.generation = frame.buyerIntent.generation) &&
  decide (frame.sellerIntent.outcome = frame.buyerIntent.outcome) &&
  decide (frame.sellerIntent.maker ≠ frame.buyerIntent.maker) &&
  decide (frame.sellerIntent.outcome < frame.product.outcomeCount) &&
  decide (frame.sellerIntent.lifecycle.accepts frame.sellerIntent.maxFill frame.fill) &&
  decide (frame.buyerIntent.lifecycle.accepts frame.buyerIntent.maxFill frame.fill) &&
  decide (frame.sellerIntent.nonce = frame.pre.sellerNextNonce) &&
  decide (frame.buyerIntent.nonce = frame.pre.buyerNextNonce) &&
  decide (frame.pre.sellerNextNonce + 1 < u64Limit) &&
  decide (frame.pre.buyerNextNonce + 1 < u64Limit) &&
  decide (frame.sellerIntent.limitPrice ≤ frame.executionPrice) &&
  decide (frame.executionPrice ≤ frame.buyerIntent.limitPrice) &&
  decide (frame.executionPrice ≤ frame.product.priceScale) &&
  decide (frame.sellerIntent.feePolicyId = frame.feePolicyId) &&
  decide (frame.buyerIntent.feePolicyId = frame.feePolicyId) &&
  decide (frame.sellerIntent.feeBasisPoints = frame.feePolicy.basisPoints) &&
  decide (frame.buyerIntent.feeBasisPoints = frame.feePolicy.basisPoints) &&
  decide (frame.fill * frame.executionPrice = frame.gross * frame.product.priceScale) &&
  decide (frame.fee = frame.gross * frame.feePolicy.basisPoints / feeDenominator) &&
  decide (frame.fill < u64Limit) &&
  decide (frame.executionPrice < u64Limit) &&
  decide (frame.gross < u64Limit) &&
  decide (frame.fee < u64Limit) &&
  decide (frame.fill ≤ frame.pre.sellerClaims) &&
  decide (frame.gross + frame.fee ≤ frame.pre.buyerCollateral) &&
  decide (frame.pre.buyerClaims + frame.fill < u64Limit) &&
  decide (frame.pre.sellerCollateral + frame.gross < u64Limit) &&
  decide (frame.pre.venueCollateral + frame.fee < u64Limit)

theorem accepts_iff (frame : FillFrame) : accepts frame = true ↔ Admissible frame := by
  simp only [accepts, Bool.and_eq_true, decide_eq_true_eq]
  constructor
  · intro evidence
    rcases evidence with ⟨evidence, venueCreditFits⟩
    rcases evidence with ⟨evidence, sellerCollateralCreditFits⟩
    rcases evidence with ⟨evidence, buyerClaimCreditFits⟩
    rcases evidence with ⟨evidence, buyerHasCollateral⟩
    rcases evidence with ⟨evidence, sellerHasClaims⟩
    rcases evidence with ⟨evidence, feeU64⟩
    rcases evidence with ⟨evidence, grossU64⟩
    rcases evidence with ⟨evidence, priceU64⟩
    rcases evidence with ⟨evidence, fillU64⟩
    rcases evidence with ⟨evidence, exactFloorFee⟩
    rcases evidence with ⟨evidence, exactQuote⟩
    rcases evidence with ⟨evidence, buyerFeeRate⟩
    rcases evidence with ⟨evidence, sellerFeeRate⟩
    rcases evidence with ⟨evidence, buyerFeePolicy⟩
    rcases evidence with ⟨evidence, sellerFeePolicy⟩
    rcases evidence with ⟨evidence, priceInScale⟩
    rcases evidence with ⟨evidence, buyerPrice⟩
    rcases evidence with ⟨evidence, sellerPrice⟩
    rcases evidence with ⟨evidence, buyerNonceCanAdvance⟩
    rcases evidence with ⟨evidence, sellerNonceCanAdvance⟩
    rcases evidence with ⟨evidence, buyerNonce⟩
    rcases evidence with ⟨evidence, sellerNonce⟩
    rcases evidence with ⟨evidence, buyerLifecycle⟩
    rcases evidence with ⟨evidence, sellerLifecycle⟩
    rcases evidence with ⟨evidence, outcomeInDomain⟩
    rcases evidence with ⟨evidence, distinctMakers⟩
    rcases evidence with ⟨evidence, sameOutcome⟩
    rcases evidence with ⟨evidence, sameGeneration⟩
    rcases evidence with ⟨evidence, sameMarket⟩
    rcases evidence with ⟨evidence, buyerSide⟩
    rcases evidence with ⟨evidence, sellerSide⟩
    rcases evidence with ⟨evidence, buyerSlotBeforeEnd⟩
    rcases evidence with ⟨evidence, buyerSlotAfterStart⟩
    rcases evidence with ⟨evidence, slotBeforeSellerEnd⟩
    rcases evidence with ⟨evidence, slotAfterStart⟩
    rcases evidence with ⟨evidence, positiveFill⟩
    rcases evidence with ⟨preU64, phaseOpen⟩
    exact {
      preU64, phaseOpen, positiveFill, slotAfterStart, slotBeforeSellerEnd,
      buyerSlotAfterStart, buyerSlotBeforeEnd, sellerSide, buyerSide, sameMarket,
      sameGeneration, sameOutcome, distinctMakers, outcomeInDomain, sellerLifecycle,
      buyerLifecycle, sellerNonce, buyerNonce, sellerNonceCanAdvance,
      buyerNonceCanAdvance, sellerPrice, buyerPrice, priceInScale, sellerFeePolicy,
      buyerFeePolicy, sellerFeeRate, buyerFeeRate, exactQuote, exactFloorFee,
      fillU64, priceU64, grossU64, feeU64,
      sellerHasClaims, buyerHasCollateral, buyerClaimCreditFits,
      sellerCollateralCreditFits, venueCreditFits
    }
  · intro admitted
    apply And.intro ?_ admitted.venueCreditFits
    apply And.intro ?_ admitted.sellerCollateralCreditFits
    apply And.intro ?_ admitted.buyerClaimCreditFits
    apply And.intro ?_ admitted.buyerHasCollateral
    apply And.intro ?_ admitted.sellerHasClaims
    apply And.intro ?_ admitted.feeU64
    apply And.intro ?_ admitted.grossU64
    apply And.intro ?_ admitted.priceU64
    apply And.intro ?_ admitted.fillU64
    apply And.intro ?_ admitted.exactFloorFee
    apply And.intro ?_ admitted.exactQuote
    apply And.intro ?_ admitted.buyerFeeRate
    apply And.intro ?_ admitted.sellerFeeRate
    apply And.intro ?_ admitted.buyerFeePolicy
    apply And.intro ?_ admitted.sellerFeePolicy
    apply And.intro ?_ admitted.priceInScale
    apply And.intro ?_ admitted.buyerPrice
    apply And.intro ?_ admitted.sellerPrice
    apply And.intro ?_ admitted.buyerNonceCanAdvance
    apply And.intro ?_ admitted.sellerNonceCanAdvance
    apply And.intro ?_ admitted.buyerNonce
    apply And.intro ?_ admitted.sellerNonce
    apply And.intro ?_ admitted.buyerLifecycle
    apply And.intro ?_ admitted.sellerLifecycle
    apply And.intro ?_ admitted.outcomeInDomain
    apply And.intro ?_ admitted.distinctMakers
    apply And.intro ?_ admitted.sameOutcome
    apply And.intro ?_ admitted.sameGeneration
    apply And.intro ?_ admitted.sameMarket
    apply And.intro ?_ admitted.buyerSide
    apply And.intro ?_ admitted.sellerSide
    apply And.intro ?_ admitted.buyerSlotBeforeEnd
    apply And.intro ?_ admitted.buyerSlotAfterStart
    apply And.intro ?_ admitted.slotBeforeSellerEnd
    apply And.intro ?_ admitted.slotAfterStart
    apply And.intro ?_ admitted.positiveFill
    exact ⟨admitted.preU64, admitted.phaseOpen⟩

/-- Exact post-state, defined independently of any Rust DTO. -/
def postState (frame : FillFrame) : Ledger := {
  sellerNextNonce := frame.pre.sellerNextNonce + 1
  buyerNextNonce := frame.pre.buyerNextNonce + 1
  sellerClaims := frame.pre.sellerClaims - frame.fill
  buyerClaims := frame.pre.buyerClaims + frame.fill
  buyerCollateral := frame.pre.buyerCollateral - (frame.gross + frame.fee)
  sellerCollateral := frame.pre.sellerCollateral + frame.gross
  venueCollateral := frame.pre.venueCollateral + frame.fee
}

def sellerReplayCell : Cell := { party := .seller, resource := .replayNonce }
def buyerReplayCell : Cell := { party := .buyer, resource := .replayNonce }
def sellerClaimCell (outcome : Nat) : Cell := { party := .seller, resource := .outcomeClaim outcome }
def buyerClaimCell (outcome : Nat) : Cell := { party := .buyer, resource := .outcomeClaim outcome }
def buyerCollateralCell : Cell := { party := .buyer, resource := .collateral }
def sellerCollateralCell : Cell := { party := .seller, resource := .collateral }
def venueCollateralCell : Cell := { party := .venue, resource := .collateral }

/-- Width-independent effect data for the fill. -/
def effectPlan (frame : FillFrame) : EffectPlan := {
  effects := [
    .set sellerReplayCell (frame.pre.sellerNextNonce + 1),
    .set buyerReplayCell (frame.pre.buyerNextNonce + 1),
    .debit (sellerClaimCell frame.sellerIntent.outcome) frame.fill,
    .credit (buyerClaimCell frame.sellerIntent.outcome) frame.fill,
    .debit buyerCollateralCell (frame.gross + frame.fee),
    .credit sellerCollateralCell frame.gross,
    .credit venueCollateralCell frame.fee
  ]
}

theorem effectPlan_length (frame : FillFrame) : (effectPlan frame).effects.length = 7 := by
  rfl

/-- Proof-carrying semantic result. Proof fields erase; the first-order plan does
not contain them. -/
structure Settlement (frame : FillFrame) where
  post : Ledger
  plan : EffectPlan
  postU64 : post.U64Valid
  claimConservation :
    post.sellerClaims + post.buyerClaims = frame.pre.sellerClaims + frame.pre.buyerClaims
  collateralConservation :
    post.buyerCollateral + post.sellerCollateral + post.venueCollateral =
      frame.pre.buyerCollateral + frame.pre.sellerCollateral + frame.pre.venueCollateral
  sellerReplayAdvanced : post.sellerNextNonce = frame.pre.sellerNextNonce + 1
  buyerReplayAdvanced : post.buyerNextNonce = frame.pre.buyerNextNonce + 1
  quoteIsExact : frame.fill * frame.executionPrice = frame.gross * frame.product.priceScale
  feeUsesNamedFloor : frame.fee = frame.gross * frame.feePolicy.basisPoints / feeDenominator

/-- A refusal from the semantic admission boundary. Future versions refine this
into stable machine-readable causes without changing `execute`. -/
inductive Refusal where
  | notAdmissible
  deriving DecidableEq, Repr

end DClutch.Direct
