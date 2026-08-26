import DClutchSemantics.DirectProofs
import Std.Tactic

/-!
# Registered Direct intent lifecycle

This module extends the inline Direct fill with one persistent, authenticated
intent state machine.  The maker nonce is consumed exactly once at
registration.  Thereafter the registration-local sequence owns replay, while
the sole persisted `remaining` quantity owns residual fill authority.

`immediateOrCancel` may fill once and cancels any remainder.  The separately
named `goodTillCancelled` policy is the only policy that leaves a nonzero
remainder reusable.  No Rust action body or account layout is defined here.
-/

namespace DClutch.DirectLifecycle

open DClutch DClutch.Direct

/-- Persistent phase of one registered intent. -/
inductive Phase where
  | open
  | filled
  | cancelled
  | expired
  deriving DecidableEq, Repr

/-- Semantic owner of registered execution authority. -/
structure State where
  terms : Intent
  phase : Phase
  remaining : Nat
  sequence : Nat
  deriving DecidableEq, Repr

/-- Canonical state invariant.  A terminal cancellation or expiry retains its
unfilled quantity as evidence, but cannot authorize another fill. -/
def State.Valid (state : State) : Prop :=
  state.terms.maxFill < u64Limit ∧
  state.remaining < u64Limit ∧
  state.sequence < u64Limit ∧
  state.remaining ≤ state.terms.maxFill ∧
  match state.phase with
  | .open => 0 < state.remaining
  | .filled => state.remaining = 0
  | .cancelled | .expired => True

instance (state : State) : Decidable state.Valid := by
  unfold State.Valid
  cases state.phase <;> infer_instance

/-- Untrusted registration request.  Signature and account authenticity remain
adapter obligations; `vacant` is the semantic observation of the derived
registration coordinate. -/
structure RegisterFrame where
  product : ProductIR
  feePolicy : FeePolicy
  marketPhase : Direct.Phase
  slot : Nat
  intent : Intent
  makerNextNonce : Nat
  vacant : Bool

def RegisterAdmissible (frame : RegisterFrame) : Prop :=
  frame.vacant = true ∧
  frame.marketPhase = .open ∧
  frame.intent.validFromSlot ≤ frame.intent.validThroughSlot ∧
  frame.slot ≤ frame.intent.validThroughSlot ∧
  0 < frame.intent.maxFill ∧
  frame.intent.maxFill < u64Limit ∧
  frame.intent.outcome < frame.product.outcomeCount ∧
  frame.intent.feeBasisPoints = frame.feePolicy.basisPoints ∧
  frame.intent.nonce = frame.makerNextNonce ∧
  frame.makerNextNonce + 1 < u64Limit

instance (frame : RegisterFrame) : Decidable (RegisterAdmissible frame) := by
  unfold RegisterAdmissible
  infer_instance

def registerAccepts (frame : RegisterFrame) : Bool :=
  decide (RegisterAdmissible frame)

theorem register_accepts_iff (frame : RegisterFrame) :
    registerAccepts frame = true ↔ RegisterAdmissible frame := by
  simp [registerAccepts]

def initialState (intent : Intent) : State := {
  terms := intent
  phase := .open
  remaining := intent.maxFill
  sequence := 0
}

/-- Registration consumes the maker nonce once and creates all residual
authority in one state value. -/
def register (frame : RegisterFrame) : Option (State × Nat) :=
  if RegisterAdmissible frame then
    some (initialState frame.intent, frame.makerNextNonce + 1)
  else none

theorem register_success
    (frame : RegisterFrame) (admitted : RegisterAdmissible frame) :
    register frame = some (initialState frame.intent, frame.makerNextNonce + 1) := by
  simp [register, admitted]

theorem initial_state_valid
    (frame : RegisterFrame) (admitted : RegisterAdmissible frame) :
    (initialState frame.intent).Valid := by
  rcases admitted with ⟨_, _, _, _, positive, bounded, _, _, _, _⟩
  refine ⟨bounded, bounded, ?_, Nat.le_refl _, positive⟩
  change 0 < u64Limit
  decide

/-- Claim/collateral projection reused from the ordinary Direct semantics.
Registration sequences are supplied separately by `executionLedger`. -/
structure Ledger where
  sellerClaims : Nat
  buyerClaims : Nat
  buyerCollateral : Nat
  sellerCollateral : Nat
  venueCollateral : Nat
  deriving DecidableEq, Repr

/-- Untrusted match over two authenticated registration states. -/
structure FillFrame where
  product : ProductIR
  feePolicy : FeePolicy
  marketPhase : Direct.Phase
  slot : Nat
  seller : State
  buyer : State
  pre : Ledger
  fill : Nat
  executionPrice : Nat
  gross : Nat
  fee : Nat

/-- State-authorized execution view.  It is not a new signed intent: the
registration account already authenticated `terms`; only its local replay
sequence and sole remaining quantity are projected into the ordinary kernel. -/
def executionIntent (state : State) : Intent := {
  state.terms with
  nonce := state.sequence
  maxFill := state.remaining
}

def executionLedger (frame : FillFrame) : Direct.Ledger := {
  sellerNextNonce := frame.seller.sequence
  buyerNextNonce := frame.buyer.sequence
  sellerClaims := frame.pre.sellerClaims
  buyerClaims := frame.pre.buyerClaims
  buyerCollateral := frame.pre.buyerCollateral
  sellerCollateral := frame.pre.sellerCollateral
  venueCollateral := frame.pre.venueCollateral
}

/-- Exact reuse of ordinary-fill economics after registered-state projection. -/
def executionFrame (frame : FillFrame) : Direct.FillFrame := {
  product := frame.product
  feePolicy := frame.feePolicy
  phase := frame.marketPhase
  slot := frame.slot
  sellerIntent := executionIntent frame.seller
  buyerIntent := executionIntent frame.buyer
  pre := executionLedger frame
  fill := frame.fill
  executionPrice := frame.executionPrice
  gross := frame.gross
  fee := frame.fee
}

def FillAdmissible (frame : FillFrame) : Prop :=
  frame.seller.Valid ∧
  frame.buyer.Valid ∧
  frame.seller.phase = .open ∧
  frame.buyer.phase = .open ∧
  Direct.accepts (executionFrame frame) = true

instance (frame : FillFrame) : Decidable (FillAdmissible frame) := by
  unfold FillAdmissible
  infer_instance

def fillAccepts (frame : FillFrame) : Bool := decide (FillAdmissible frame)

theorem fill_accepts_iff (frame : FillFrame) :
    fillAccepts frame = true ↔ FillAdmissible frame := by
  simp [fillAccepts]

theorem direct_admitted
    (frame : FillFrame) (admitted : FillAdmissible frame) :
    Direct.Admissible (executionFrame frame) :=
  (Direct.accepts_iff (executionFrame frame)).mp admitted.2.2.2.2

/-- Terminal/open phase derived only from the authenticated policy and computed
remainder. -/
def phaseAfterFill (state : State) (fill : Nat) : Phase :=
  if state.remaining = fill then .filled
  else match state.terms.lifecycle with
    | .goodTillCancelled => .open
    | .fillOrKill | .immediateOrCancel => .cancelled

def stateAfterFill (state : State) (fill : Nat) : State := {
  state with
  phase := phaseAfterFill state fill
  remaining := state.remaining - fill
  sequence := state.sequence + 1
}

def ledgerAfterFill (frame : FillFrame) : Ledger :=
  let post := Direct.postState (executionFrame frame)
  {
    sellerClaims := post.sellerClaims
    buyerClaims := post.buyerClaims
    buyerCollateral := post.buyerCollateral
    sellerCollateral := post.sellerCollateral
    venueCollateral := post.venueCollateral
  }

structure FillResult where
  seller : State
  buyer : State
  ledger : Ledger
  plan : EffectPlan
  deriving DecidableEq, Repr

def fillResult (frame : FillFrame) : FillResult := {
  seller := stateAfterFill frame.seller frame.fill
  buyer := stateAfterFill frame.buyer frame.fill
  ledger := ledgerAfterFill frame
  plan := Direct.effectPlan (executionFrame frame)
}

def fill (frame : FillFrame) : Option FillResult :=
  if FillAdmissible frame then some (fillResult frame) else none

theorem fill_success
    (frame : FillFrame) (admitted : FillAdmissible frame) :
    fill frame = some (fillResult frame) := by
  simp [fill, admitted]

private theorem lifecycle_fill_le
    (lifecycle : Lifecycle) (maximum fill : Nat)
    (accepted : lifecycle.accepts maximum fill) : fill ≤ maximum := by
  cases lifecycle <;> simp [Lifecycle.accepts] at accepted ⊢ <;> omega

theorem seller_fill_le_remaining
    (frame : FillFrame) (admitted : FillAdmissible frame) :
    frame.fill ≤ frame.seller.remaining := by
  exact lifecycle_fill_le _ _ _ (direct_admitted frame admitted).sellerLifecycle

theorem buyer_fill_le_remaining
    (frame : FillFrame) (admitted : FillAdmissible frame) :
    frame.fill ≤ frame.buyer.remaining := by
  exact lifecycle_fill_le _ _ _ (direct_admitted frame admitted).buyerLifecycle

theorem seller_remaining_conserved
    (frame : FillFrame) (admitted : FillAdmissible frame) :
    (fillResult frame).seller.remaining + frame.fill = frame.seller.remaining := by
  simp [fillResult, stateAfterFill]
  exact Nat.sub_add_cancel (seller_fill_le_remaining frame admitted)

theorem buyer_remaining_conserved
    (frame : FillFrame) (admitted : FillAdmissible frame) :
    (fillResult frame).buyer.remaining + frame.fill = frame.buyer.remaining := by
  simp [fillResult, stateAfterFill]
  exact Nat.sub_add_cancel (buyer_fill_le_remaining frame admitted)

theorem seller_sequence_advanced (frame : FillFrame) :
    (fillResult frame).seller.sequence = frame.seller.sequence + 1 := by
  rfl

theorem buyer_sequence_advanced (frame : FillFrame) :
    (fillResult frame).buyer.sequence = frame.buyer.sequence + 1 := by
  rfl

private theorem state_after_fill_valid
    (state : State) (fill : Nat)
    (valid : state.Valid)
    (accepted : state.terms.lifecycle.accepts state.remaining fill)
    (sequenceFits : state.sequence + 1 < u64Limit) :
    (stateAfterFill state fill).Valid := by
  have fillLe : fill ≤ state.remaining :=
    lifecycle_fill_le _ _ _ accepted
  have residualBounded : state.remaining - fill < u64Limit :=
    Nat.lt_of_le_of_lt (Nat.sub_le _ _) valid.2.1
  have residualWithinMaximum : state.remaining - fill ≤ state.terms.maxFill :=
    Nat.le_trans (Nat.sub_le _ _) valid.2.2.2.1
  refine ⟨valid.1, residualBounded, sequenceFits, residualWithinMaximum, ?_⟩
  unfold stateAfterFill phaseAfterFill
  dsimp
  by_cases complete : state.remaining = fill
  · simp [complete]
  · cases policy : state.terms.lifecycle with
    | fillOrKill =>
        simp [Lifecycle.accepts, policy] at accepted
        exact (complete accepted.symm).elim
    | immediateOrCancel => simp [complete]
    | goodTillCancelled =>
        have positiveResidual : 0 < state.remaining - fill := by omega
        simp [complete, positiveResidual]

theorem seller_state_after_fill_valid
    (frame : FillFrame) (admitted : FillAdmissible frame) :
    (fillResult frame).seller.Valid := by
  apply state_after_fill_valid
  · exact admitted.1
  · exact (direct_admitted frame admitted).sellerLifecycle
  · simpa [executionFrame, executionLedger] using
      (direct_admitted frame admitted).sellerNonceCanAdvance

theorem buyer_state_after_fill_valid
    (frame : FillFrame) (admitted : FillAdmissible frame) :
    (fillResult frame).buyer.Valid := by
  apply state_after_fill_valid
  · exact admitted.2.1
  · exact (direct_admitted frame admitted).buyerLifecycle
  · simpa [executionFrame, executionLedger] using
      (direct_admitted frame admitted).buyerNonceCanAdvance

theorem reusable_residual_iff_good_till_cancelled
    (state : State) (fill : Nat) (proper : fill < state.remaining) :
    (stateAfterFill state fill).phase = .open ↔
      state.terms.lifecycle = .goodTillCancelled := by
  simp [stateAfterFill, phaseAfterFill, Nat.ne_of_gt proper]
  cases state.terms.lifecycle <;> simp

theorem immediate_or_cancel_closes_residual
    (state : State) (fill : Nat)
    (policy : state.terms.lifecycle = .immediateOrCancel)
    (proper : fill < state.remaining) :
    (stateAfterFill state fill).phase = .cancelled := by
  simp [stateAfterFill, phaseAfterFill, policy, Nat.ne_of_gt proper]

theorem fill_or_kill_has_no_residual
    (frame : FillFrame) (admitted : FillAdmissible frame)
    (policy : frame.seller.terms.lifecycle = .fillOrKill) :
    (fillResult frame).seller.remaining = 0 := by
  have accepted := (direct_admitted frame admitted).sellerLifecycle
  simp [executionFrame, executionIntent, Lifecycle.accepts, policy] at accepted
  simp [fillResult, stateAfterFill, accepted]

theorem cumulative_fills_never_exceed_registration
    (maximum first second : Nat)
    (firstBound : first ≤ maximum)
    (secondBound : second ≤ maximum - first) :
    first + second ≤ maximum := by
  omega

theorem terminal_state_cannot_fill
    (frame : FillFrame)
    (terminal : frame.seller.phase = .filled ∨
      frame.seller.phase = .cancelled ∨ frame.seller.phase = .expired) :
    ¬ FillAdmissible frame := by
  intro admitted
  rcases terminal with filled | cancelled | expired
  · rw [admitted.2.2.1] at filled
    cases filled
  · rw [admitted.2.2.1] at cancelled
    cases cancelled
  · rw [admitted.2.2.1] at expired
    cases expired

theorem claim_conservation
    (frame : FillFrame) (admitted : FillAdmissible frame) :
    (fillResult frame).ledger.sellerClaims + (fillResult frame).ledger.buyerClaims =
      frame.pre.sellerClaims + frame.pre.buyerClaims := by
  have enough : frame.fill ≤ frame.pre.sellerClaims := by
    simpa [executionFrame, executionLedger] using
      (direct_admitted frame admitted).sellerHasClaims
  simp [fillResult, ledgerAfterFill, Direct.postState, executionFrame, executionLedger]
  omega

theorem collateral_conservation
    (frame : FillFrame) (admitted : FillAdmissible frame) :
    (fillResult frame).ledger.buyerCollateral +
      (fillResult frame).ledger.sellerCollateral +
      (fillResult frame).ledger.venueCollateral =
        frame.pre.buyerCollateral + frame.pre.sellerCollateral + frame.pre.venueCollateral := by
  have enough : frame.gross + frame.fee ≤ frame.pre.buyerCollateral := by
    simpa [executionFrame, executionLedger] using
      (direct_admitted frame admitted).buyerHasCollateral
  simp [fillResult, ledgerAfterFill, Direct.postState, executionFrame, executionLedger]
  omega

/-- Maker-authorized cancellation frame. -/
structure CancelFrame where
  state : State
  expectedSequence : Nat

def CancelAdmissible (frame : CancelFrame) : Prop :=
  frame.state.Valid ∧
  frame.state.phase = .open ∧
  frame.expectedSequence = frame.state.sequence ∧
  frame.state.sequence + 1 < u64Limit

instance (frame : CancelFrame) : Decidable (CancelAdmissible frame) := by
  unfold CancelAdmissible
  infer_instance

def cancel (frame : CancelFrame) : Option State :=
  if CancelAdmissible frame then
    some { frame.state with phase := .cancelled, sequence := frame.state.sequence + 1 }
  else none

/-- Permissionless expiry frame. -/
structure ExpireFrame where
  state : State
  slot : Nat
  expectedSequence : Nat

def ExpireAdmissible (frame : ExpireFrame) : Prop :=
  frame.state.Valid ∧
  frame.state.phase = .open ∧
  frame.state.terms.validThroughSlot < frame.slot ∧
  frame.expectedSequence = frame.state.sequence ∧
  frame.state.sequence + 1 < u64Limit

instance (frame : ExpireFrame) : Decidable (ExpireAdmissible frame) := by
  unfold ExpireAdmissible
  infer_instance

def expire (frame : ExpireFrame) : Option State :=
  if ExpireAdmissible frame then
    some { frame.state with phase := .expired, sequence := frame.state.sequence + 1 }
  else none

theorem cancel_is_terminal
    (frame : CancelFrame) (admitted : CancelAdmissible frame) :
    cancel frame = some { frame.state with
      phase := .cancelled, sequence := frame.state.sequence + 1 } := by
  simp [cancel, admitted]

theorem cancelled_state_valid
    (frame : CancelFrame) (admitted : CancelAdmissible frame) :
    ({ frame.state with
      phase := .cancelled
      sequence := frame.state.sequence + 1 } : State).Valid := by
  rcases admitted with ⟨valid, _, _, sequenceFits⟩
  exact ⟨valid.1, valid.2.1, sequenceFits, valid.2.2.2.1, trivial⟩

theorem expire_is_terminal
    (frame : ExpireFrame) (admitted : ExpireAdmissible frame) :
    expire frame = some { frame.state with
      phase := .expired, sequence := frame.state.sequence + 1 } := by
  simp [expire, admitted]

theorem expired_state_valid
    (frame : ExpireFrame) (admitted : ExpireAdmissible frame) :
    ({ frame.state with
      phase := .expired
      sequence := frame.state.sequence + 1 } : State).Valid := by
  rcases admitted with ⟨valid, _, _, _, sequenceFits⟩
  exact ⟨valid.1, valid.2.1, sequenceFits, valid.2.2.2.1, trivial⟩

theorem early_expiry_refuses
    (frame : ExpireFrame) (early : frame.slot ≤ frame.state.terms.validThroughSlot) :
    expire frame = none := by
  have refused : ¬ ExpireAdmissible frame := by
    intro admitted
    have late := admitted.2.2.1
    omega
  simp [expire, refused]

theorem cancelled_registration_cannot_cancel_again
    (frame : CancelFrame) (terminal : frame.state.phase = .cancelled) :
    cancel frame = none := by
  have refused : ¬ CancelAdmissible frame := by
    intro admitted
    rw [admitted.2.1] at terminal
    cases terminal
  simp [cancel, refused]

/-! ## Terminal registration retirement

Retirement destroys no economic balance.  It is admitted only after the sole
registered execution authority is terminal.  The physical adapter separately
binds the refund destination to the persisted maker and, for buyer intents,
revokes the maker's SPL delegation before the account owner returns rent.
-/

def RetireAdmissible (state : State) : Prop :=
  state.Valid ∧ state.phase ≠ .open

instance (state : State) : Decidable (RetireAdmissible state) := by
  unfold RetireAdmissible
  infer_instance

def retire (state : State) : Option Unit :=
  if RetireAdmissible state then some () else none

theorem terminal_state_retires
    (state : State) (valid : state.Valid) (terminal : state.phase ≠ .open) :
    retire state = some () := by
  simp [retire, RetireAdmissible, valid, terminal]

theorem open_state_cannot_retire
    (state : State) (openState : state.phase = .open) :
    retire state = none := by
  have refused : ¬ RetireAdmissible state := by
    intro admitted
    exact admitted.2 openState
  simp [retire, refused]

end DClutch.DirectLifecycle
