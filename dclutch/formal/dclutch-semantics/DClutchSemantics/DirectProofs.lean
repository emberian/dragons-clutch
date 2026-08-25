import DClutchSemantics.Direct

/-!
# Direct-fill proofs and executable effect interpreter

The interpreter is deliberately tiny. It is the semantic target for a future
`no_std`, `no_alloc` SBF microkernel and for bytecode-level refinement.
-/

namespace DClutch.Direct

open DClutch

/-- Checked `u64`-range credit in the abstract machine. -/
def checkedCredit (current delta : Nat) : Option Nat :=
  if current + delta < u64Limit then some (current + delta) else none

/-- Checked debit in the abstract machine. -/
def checkedDebit (current delta : Nat) : Option Nat :=
  if delta ≤ current then some (current - delta) else none

/-- Apply one typed effect to the Direct state projection. -/
def applyEffect (outcome : Nat) (state : Ledger) (effect : Effect) : Option Ledger :=
  match effect with
  | .set cell value =>
      if value < u64Limit then
        match cell.party, cell.resource with
        | .seller, .replayNonce => some { state with sellerNextNonce := value }
        | .buyer, .replayNonce => some { state with buyerNextNonce := value }
        | _, _ => none
      else none
  | .debit cell amount =>
      match cell.party, cell.resource with
      | .seller, .outcomeClaim selected =>
          if selected = outcome then
            (checkedDebit state.sellerClaims amount).map fun value =>
              { state with sellerClaims := value }
          else none
      | .buyer, .collateral =>
          (checkedDebit state.buyerCollateral amount).map fun value =>
            { state with buyerCollateral := value }
      | _, _ => none
  | .credit cell amount =>
      match cell.party, cell.resource with
      | .buyer, .outcomeClaim selected =>
          if selected = outcome then
            (checkedCredit state.buyerClaims amount).map fun value =>
              { state with buyerClaims := value }
          else none
      | .seller, .collateral =>
          (checkedCredit state.sellerCollateral amount).map fun value =>
            { state with sellerCollateral := value }
      | .venue, .collateral =>
          (checkedCredit state.venueCollateral amount).map fun value =>
            { state with venueCollateral := value }
      | _, _ => none

/-- Total bounded-plan interpreter. `none` means the complete plan refuses. -/
def runEffects (outcome : Nat) : List Effect → Ledger → Option Ledger
  | [], state => some state
  | effect :: rest, state =>
      (applyEffect outcome state effect).bind (runEffects outcome rest)

/-- Execute an admitted fill and construct its erased proofs. -/
def execute (frame : FillFrame) (admitted : Admissible frame) : Settlement frame := {
  post := postState frame
  plan := effectPlan frame
  postU64 := by
    simp only [Ledger.U64Valid, postState]
    constructor
    · exact admitted.sellerNonceCanAdvance
    constructor
    · exact admitted.buyerNonceCanAdvance
    constructor
    · exact Nat.lt_of_le_of_lt (Nat.sub_le _ _) admitted.preU64.2.2.1
    constructor
    · exact admitted.buyerClaimCreditFits
    constructor
    · exact Nat.lt_of_le_of_lt (Nat.sub_le _ _) admitted.preU64.2.2.2.2.1
    constructor
    · exact admitted.sellerCollateralCreditFits
    · exact admitted.venueCreditFits
  claimConservation := by
    simp only [postState]
    have sellerHasClaims := admitted.sellerHasClaims
    omega
  collateralConservation := by
    simp only [postState]
    have buyerHasCollateral := admitted.buyerHasCollateral
    omega
  sellerReplayAdvanced := by rfl
  buyerReplayAdvanced := by rfl
  quoteIsExact := admitted.exactQuote
  feeUsesNamedFloor := admitted.exactFloorFee
}

/-- Decide admission and execute without accepting proof-only preconditions. -/
def execute? (frame : FillFrame) : Except Refusal (Settlement frame) :=
  if accepted : accepts frame = true then
    .ok (execute frame ((accepts_iff frame).mp accepted))
  else
    .error .notAdmissible

/-- Observable state semantics: any refusal leaves the entire pre-state intact. -/
def runLedger (frame : FillFrame) : Ledger :=
  if accepts frame then postState frame else frame.pre

theorem effectPlan_refines_transition (frame : FillFrame) (admitted : Admissible frame) :
    runEffects frame.sellerIntent.outcome (effectPlan frame).effects frame.pre =
      some (postState frame) := by
  simp only [effectPlan, runEffects, applyEffect, sellerReplayCell, buyerReplayCell,
    sellerClaimCell, buyerClaimCell, buyerCollateralCell, sellerCollateralCell,
    venueCollateralCell, checkedCredit, checkedDebit]
  simp [admitted.sellerNonceCanAdvance, admitted.buyerNonceCanAdvance,
    admitted.sellerHasClaims, admitted.buyerHasCollateral,
    admitted.buyerClaimCreditFits, admitted.sellerCollateralCreditFits,
    admitted.venueCreditFits, postState]

theorem admitted_plan_is_bounded (frame : FillFrame) (admitted : Admissible frame) :
    (execute frame admitted).plan.effects.length ≤ inlineOrdinaryFrame.maxEffects := by
  simp [execute, effectPlan, inlineOrdinaryFrame]

theorem admitted_claims_conserved (frame : FillFrame) (admitted : Admissible frame) :
    (execute frame admitted).post.sellerClaims + (execute frame admitted).post.buyerClaims =
      frame.pre.sellerClaims + frame.pre.buyerClaims :=
  (execute frame admitted).claimConservation

theorem admitted_collateral_conserved (frame : FillFrame) (admitted : Admissible frame) :
    (execute frame admitted).post.buyerCollateral +
        (execute frame admitted).post.sellerCollateral +
        (execute frame admitted).post.venueCollateral =
      frame.pre.buyerCollateral + frame.pre.sellerCollateral + frame.pre.venueCollateral :=
  (execute frame admitted).collateralConservation

theorem rejected_execute_refuses (frame : FillFrame) (rejected : accepts frame = false) :
    execute? frame = .error .notAdmissible := by
  simp [execute?, rejected]

theorem rejection_rolls_back (frame : FillFrame) (rejected : accepts frame = false) :
    runLedger frame = frame.pre := by
  simp [runLedger, rejected]

/-- Cumulative floor-fee deltas telescope. This is the algebraic reason matcher
fragmentation cannot change a registered order's final fee. -/
theorem cumulative_fee_telescopes
    (feeAt : Nat → Nat) (prior first second : Nat)
    (h₁ : feeAt prior ≤ feeAt (prior + first))
    (h₂ : feeAt (prior + first) ≤ feeAt (prior + first + second)) :
    (feeAt (prior + first) - feeAt prior) +
        (feeAt (prior + first + second) - feeAt (prior + first)) =
      feeAt (prior + first + second) - feeAt prior := by
  omega

/-- Concrete cumulative floor-fee function used by Direct resting orders. -/
def cumulativeFee (basisPoints gross : Nat) : Nat :=
  gross * basisPoints / feeDenominator

theorem cumulativeFee_monotone (basisPoints : Nat) {first second : Nat}
    (ordered : first ≤ second) :
    cumulativeFee basisPoints first ≤ cumulativeFee basisPoints second := by
  apply Nat.div_le_div_right
  exact Nat.mul_le_mul_right basisPoints ordered

/-- Splitting one gross increment into two matcher-selected pieces cannot change
the final cumulative floor fee. -/
theorem cumulative_floor_fee_fragmentation_independent
    (basisPoints prior first second : Nat) :
    (cumulativeFee basisPoints (prior + first) - cumulativeFee basisPoints prior) +
        (cumulativeFee basisPoints (prior + first + second) -
          cumulativeFee basisPoints (prior + first)) =
      cumulativeFee basisPoints (prior + first + second) -
        cumulativeFee basisPoints prior := by
  apply cumulative_fee_telescopes (cumulativeFee basisPoints)
  · apply cumulativeFee_monotone
    omega
  · apply cumulativeFee_monotone
    omega

end DClutch.Direct
