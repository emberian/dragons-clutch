import DClutchSemantics.TransitionVM
import DClutchSemantics.Examples

/-!
# Direct compiled transition program

This is the first semantic specialization target.  Account parsing and native
signature evidence remain adapter obligations; after those facts populate the
register frame, this one program derives nonce successors, exact gross quote,
and the named floor fee while refusing every failed admission relation.
-/

namespace DClutch.DirectProgram

open DClutch
open DClutch.Direct
open DClutch.TransitionVM

/-- Ordered scalar-register schema. Constructor order is the wire index; Rust
constant names are emitted from this same typed data rather than maintained as
parallel numeric literals. -/
inductive ScalarSlot where
  | phase | slot | sellerFrom | sellerThrough | buyerFrom | buyerThrough
  | sellerSide | buyerSide | sellerGeneration | buyerGeneration
  | sellerOutcome | buyerOutcome | outcomeCount
  | sellerLifecycle | sellerMaximum | buyerLifecycle | buyerMaximum
  | sellerNonce | buyerNonce | sellerNextNonce | buyerNextNonce
  | sellerLimit | executionPrice | buyerLimit | priceScale
  | sellerFeeBps | buyerFeeBps | policyFeeBps | fill
  | sellerClaims | buyerClaims | buyerCollateral | sellerCollateral
  | venueCollateral | grossOutput | feeOutput | zero | one | feeDenominator
  | sellerNonceOutput | buyerNonceOutput
  deriving DecidableEq, Repr

namespace ScalarSlot

def all : List ScalarSlot := [
  .phase, .slot, .sellerFrom, .sellerThrough, .buyerFrom, .buyerThrough,
  .sellerSide, .buyerSide, .sellerGeneration, .buyerGeneration,
  .sellerOutcome, .buyerOutcome, .outcomeCount,
  .sellerLifecycle, .sellerMaximum, .buyerLifecycle, .buyerMaximum,
  .sellerNonce, .buyerNonce, .sellerNextNonce, .buyerNextNonce,
  .sellerLimit, .executionPrice, .buyerLimit, .priceScale,
  .sellerFeeBps, .buyerFeeBps, .policyFeeBps, .fill,
  .sellerClaims, .buyerClaims, .buyerCollateral, .sellerCollateral,
  .venueCollateral, .grossOutput, .feeOutput, .zero, .one, .feeDenominator,
  .sellerNonceOutput, .buyerNonceOutput
]

@[simp] def index : ScalarSlot → Nat
  | .phase => 0
  | .slot => 1
  | .sellerFrom => 2
  | .sellerThrough => 3
  | .buyerFrom => 4
  | .buyerThrough => 5
  | .sellerSide => 6
  | .buyerSide => 7
  | .sellerGeneration => 8
  | .buyerGeneration => 9
  | .sellerOutcome => 10
  | .buyerOutcome => 11
  | .outcomeCount => 12
  | .sellerLifecycle => 13
  | .sellerMaximum => 14
  | .buyerLifecycle => 15
  | .buyerMaximum => 16
  | .sellerNonce => 17
  | .buyerNonce => 18
  | .sellerNextNonce => 19
  | .buyerNextNonce => 20
  | .sellerLimit => 21
  | .executionPrice => 22
  | .buyerLimit => 23
  | .priceScale => 24
  | .sellerFeeBps => 25
  | .buyerFeeBps => 26
  | .policyFeeBps => 27
  | .fill => 28
  | .sellerClaims => 29
  | .buyerClaims => 30
  | .buyerCollateral => 31
  | .sellerCollateral => 32
  | .venueCollateral => 33
  | .grossOutput => 34
  | .feeOutput => 35
  | .zero => 36
  | .one => 37
  | .feeDenominator => 38
  | .sellerNonceOutput => 39
  | .buyerNonceOutput => 40

def rustName : ScalarSlot → String
  | .phase => "SCALAR_PHASE"
  | .slot => "SCALAR_SLOT"
  | .sellerFrom => "SCALAR_SELLER_FROM"
  | .sellerThrough => "SCALAR_SELLER_THROUGH"
  | .buyerFrom => "SCALAR_BUYER_FROM"
  | .buyerThrough => "SCALAR_BUYER_THROUGH"
  | .sellerSide => "SCALAR_SELLER_SIDE"
  | .buyerSide => "SCALAR_BUYER_SIDE"
  | .sellerGeneration => "SCALAR_SELLER_GENERATION"
  | .buyerGeneration => "SCALAR_BUYER_GENERATION"
  | .sellerOutcome => "SCALAR_SELLER_OUTCOME"
  | .buyerOutcome => "SCALAR_BUYER_OUTCOME"
  | .outcomeCount => "SCALAR_OUTCOME_COUNT"
  | .sellerLifecycle => "SCALAR_SELLER_LIFECYCLE"
  | .sellerMaximum => "SCALAR_SELLER_MAXIMUM"
  | .buyerLifecycle => "SCALAR_BUYER_LIFECYCLE"
  | .buyerMaximum => "SCALAR_BUYER_MAXIMUM"
  | .sellerNonce => "SCALAR_SELLER_NONCE"
  | .buyerNonce => "SCALAR_BUYER_NONCE"
  | .sellerNextNonce => "SCALAR_SELLER_NEXT_NONCE"
  | .buyerNextNonce => "SCALAR_BUYER_NEXT_NONCE"
  | .sellerLimit => "SCALAR_SELLER_LIMIT"
  | .executionPrice => "SCALAR_EXECUTION_PRICE"
  | .buyerLimit => "SCALAR_BUYER_LIMIT"
  | .priceScale => "SCALAR_PRICE_SCALE"
  | .sellerFeeBps => "SCALAR_SELLER_FEE_BPS"
  | .buyerFeeBps => "SCALAR_BUYER_FEE_BPS"
  | .policyFeeBps => "SCALAR_POLICY_FEE_BPS"
  | .fill => "SCALAR_FILL"
  | .sellerClaims => "SCALAR_SELLER_CLAIMS"
  | .buyerClaims => "SCALAR_BUYER_CLAIMS"
  | .buyerCollateral => "SCALAR_BUYER_COLLATERAL"
  | .sellerCollateral => "SCALAR_SELLER_COLLATERAL"
  | .venueCollateral => "SCALAR_VENUE_COLLATERAL"
  | .grossOutput => "SCALAR_GROSS_OUTPUT"
  | .feeOutput => "SCALAR_FEE_OUTPUT"
  | .zero => "SCALAR_ZERO"
  | .one => "SCALAR_ONE"
  | .feeDenominator => "SCALAR_FEE_DENOMINATOR"
  | .sellerNonceOutput => "SCALAR_SELLER_NONCE_OUTPUT"
  | .buyerNonceOutput => "SCALAR_BUYER_NONCE_OUTPUT"

/-- Controller-populated scalar prefix. Remaining slots are program-owned
constants, intermediates, or outputs. -/
def inputs : List ScalarSlot := all.take (index .grossOutput)

def rustFieldName (register : ScalarSlot) : String :=
  ((rustName register).drop 7).copy.toLower

theorem index_matches_constructor (register : ScalarSlot) :
    index register = register.ctorIdx := by
  cases register <;> rfl

theorem indices_are_canonical :
    all.map index = List.range all.length := by
  native_decide

theorem rust_names_are_unique : (all.map rustName).Nodup := by
  native_decide

theorem input_indices_are_canonical :
    inputs.map index = List.range inputs.length := by
  native_decide

theorem rust_input_field_names_are_unique :
    (inputs.map rustFieldName).Nodup := by
  native_decide

end ScalarSlot

namespace Scalar

def phase := ScalarSlot.index .phase
def slot := ScalarSlot.index .slot
def sellerFrom := ScalarSlot.index .sellerFrom
def sellerThrough := ScalarSlot.index .sellerThrough
def buyerFrom := ScalarSlot.index .buyerFrom
def buyerThrough := ScalarSlot.index .buyerThrough
def sellerSide := ScalarSlot.index .sellerSide
def buyerSide := ScalarSlot.index .buyerSide
def sellerGeneration := ScalarSlot.index .sellerGeneration
def buyerGeneration := ScalarSlot.index .buyerGeneration
def sellerOutcome := ScalarSlot.index .sellerOutcome
def buyerOutcome := ScalarSlot.index .buyerOutcome
def outcomeCount := ScalarSlot.index .outcomeCount
def sellerLifecycle := ScalarSlot.index .sellerLifecycle
def sellerMaximum := ScalarSlot.index .sellerMaximum
def buyerLifecycle := ScalarSlot.index .buyerLifecycle
def buyerMaximum := ScalarSlot.index .buyerMaximum
def sellerNonce := ScalarSlot.index .sellerNonce
def buyerNonce := ScalarSlot.index .buyerNonce
def sellerNextNonce := ScalarSlot.index .sellerNextNonce
def buyerNextNonce := ScalarSlot.index .buyerNextNonce
def sellerLimit := ScalarSlot.index .sellerLimit
def executionPrice := ScalarSlot.index .executionPrice
def buyerLimit := ScalarSlot.index .buyerLimit
def priceScale := ScalarSlot.index .priceScale
def sellerFeeBps := ScalarSlot.index .sellerFeeBps
def buyerFeeBps := ScalarSlot.index .buyerFeeBps
def policyFeeBps := ScalarSlot.index .policyFeeBps
def fill := ScalarSlot.index .fill
def sellerClaims := ScalarSlot.index .sellerClaims
def buyerClaims := ScalarSlot.index .buyerClaims
def buyerCollateral := ScalarSlot.index .buyerCollateral
def sellerCollateral := ScalarSlot.index .sellerCollateral
def venueCollateral := ScalarSlot.index .venueCollateral
def grossOutput := ScalarSlot.index .grossOutput
def feeOutput := ScalarSlot.index .feeOutput
def zero := ScalarSlot.index .zero
def one := ScalarSlot.index .one
def feeDenominator := ScalarSlot.index .feeDenominator
def sellerNonceOutput := ScalarSlot.index .sellerNonceOutput
def buyerNonceOutput := ScalarSlot.index .buyerNonceOutput
def count := ScalarSlot.all.length

end Scalar

/-- Ordered identity-register schema. -/
inductive IdentitySlot where
  | sellerMarket | buyerMarket | sellerMaker | buyerMaker
  deriving DecidableEq, Repr

namespace IdentitySlot

def all : List IdentitySlot := [
  .sellerMarket, .buyerMarket, .sellerMaker, .buyerMaker
]

@[simp] def index : IdentitySlot → Nat
  | .sellerMarket => 0
  | .buyerMarket => 1
  | .sellerMaker => 2
  | .buyerMaker => 3

def rustName : IdentitySlot → String
  | .sellerMarket => "IDENTITY_SELLER_MARKET"
  | .buyerMarket => "IDENTITY_BUYER_MARKET"
  | .sellerMaker => "IDENTITY_SELLER_MAKER"
  | .buyerMaker => "IDENTITY_BUYER_MAKER"

def rustFieldName (register : IdentitySlot) : String :=
  ((rustName register).drop 9).copy.toLower

theorem index_matches_constructor (register : IdentitySlot) :
    index register = register.ctorIdx := by
  cases register <;> rfl

theorem indices_are_canonical :
    all.map index = List.range all.length := by
  native_decide

theorem rust_names_are_unique : (all.map rustName).Nodup := by
  native_decide

theorem rust_field_names_are_unique : (all.map rustFieldName).Nodup := by
  native_decide

end IdentitySlot

namespace Identity

def sellerMarket := IdentitySlot.index .sellerMarket
def buyerMarket := IdentitySlot.index .buyerMarket
def sellerMaker := IdentitySlot.index .sellerMaker
def buyerMaker := IdentitySlot.index .buyerMaker
def count := IdentitySlot.all.length

end Identity

def phaseTag : Phase → Nat
  | .founding => 0
  | .open => 1
  | .resolved => 2
  | .retiring => 3
  | .retired => 4

def sideTag : Side → Nat
  | .sell => 0
  | .buy => 1

def lifecycleTag : Lifecycle → Nat
  | .fillOrKill => 0
  | .immediateOrCancel => 1
  | .goodTillCancelled => 2

private def registerState
    (frame : FillFrame) (one denominator gross fee sellerNonce buyerNonce : Nat) : State := {
  scalars := #[
    phaseTag frame.phase,
    frame.slot,
    frame.sellerIntent.validFromSlot,
    frame.sellerIntent.validThroughSlot,
    frame.buyerIntent.validFromSlot,
    frame.buyerIntent.validThroughSlot,
    sideTag frame.sellerIntent.side,
    sideTag frame.buyerIntent.side,
    frame.sellerIntent.generation,
    frame.buyerIntent.generation,
    frame.sellerIntent.outcome,
    frame.buyerIntent.outcome,
    frame.product.outcomeCount,
    lifecycleTag frame.sellerIntent.lifecycle,
    frame.sellerIntent.maxFill,
    lifecycleTag frame.buyerIntent.lifecycle,
    frame.buyerIntent.maxFill,
    frame.sellerIntent.nonce,
    frame.buyerIntent.nonce,
    frame.pre.sellerNextNonce,
    frame.pre.buyerNextNonce,
    frame.sellerIntent.limitPrice,
    frame.executionPrice,
    frame.buyerIntent.limitPrice,
    frame.product.priceScale,
    frame.sellerIntent.feeBasisPoints,
    frame.buyerIntent.feeBasisPoints,
    frame.feePolicy.basisPoints,
    frame.fill,
    frame.pre.sellerClaims,
    frame.pre.buyerClaims,
    frame.pre.buyerCollateral,
    frame.pre.sellerCollateral,
    frame.pre.venueCollateral,
    gross, fee, 0, one, denominator, sellerNonce, buyerNonce
  ]
  identities := #[
    frame.sellerIntent.market,
    frame.buyerIntent.market,
    frame.sellerIntent.maker,
    frame.buyerIntent.maker
  ]
}

/-- Physical-u64 register projection. Gross and fee are deliberately absent as
caller authority: their output slots start at zero and are derived by code. -/
def state (frame : FillFrame) : State :=
  registerState frame 0 0 0 0 0 0

private def setupState (frame : FillFrame) : State :=
  registerState frame 1 feeDenominator 0 0 0 0

private def replayState (frame : FillFrame) : State :=
  registerState frame 1 feeDenominator 0 0
    (frame.pre.sellerNextNonce + 1) (frame.pre.buyerNextNonce + 1)

private def derivedState (frame : FillFrame) : State :=
  registerState frame 1 feeDenominator frame.gross frame.fee
    (frame.pre.sellerNextNonce + 1) (frame.pre.buyerNextNonce + 1)

private def setupProgram : List Op := [
  .loadConst Scalar.zero 0,
  .loadConst Scalar.one 1,
  .loadConst Scalar.feeDenominator feeDenominator
]

private def admissionProgram : List Op := [
  .scalarEq Scalar.phase Scalar.one,
  .nonzero Scalar.fill,
  .scalarLe Scalar.sellerFrom Scalar.slot,
  .scalarLe Scalar.slot Scalar.sellerThrough,
  .scalarLe Scalar.buyerFrom Scalar.slot,
  .scalarLe Scalar.slot Scalar.buyerThrough,
  .scalarEq Scalar.sellerSide Scalar.zero,
  .scalarEq Scalar.buyerSide Scalar.one,
  .identityEq Identity.sellerMarket Identity.buyerMarket,
  .scalarEq Scalar.sellerGeneration Scalar.buyerGeneration,
  .scalarEq Scalar.sellerOutcome Scalar.buyerOutcome,
  .identityNe Identity.sellerMaker Identity.buyerMaker,
  .scalarLt Scalar.sellerOutcome Scalar.outcomeCount
]

private def replayProgram : List Op := [
  .lifecycleAccepts Scalar.sellerLifecycle Scalar.sellerMaximum Scalar.fill,
  .lifecycleAccepts Scalar.buyerLifecycle Scalar.buyerMaximum Scalar.fill,
  .scalarEq Scalar.sellerNonce Scalar.sellerNextNonce,
  .scalarEq Scalar.buyerNonce Scalar.buyerNextNonce,
  .incrementInto Scalar.sellerNextNonce Scalar.sellerNonceOutput,
  .incrementInto Scalar.buyerNextNonce Scalar.buyerNonceOutput
]

private def pricingProgram : List Op := [
  .scalarLe Scalar.sellerLimit Scalar.executionPrice,
  .scalarLe Scalar.executionPrice Scalar.buyerLimit,
  .scalarLe Scalar.executionPrice Scalar.priceScale,
  .scalarEq Scalar.sellerFeeBps Scalar.policyFeeBps,
  .scalarEq Scalar.buyerFeeBps Scalar.policyFeeBps,
  .scalarLe Scalar.policyFeeBps Scalar.feeDenominator,
  .mulDivExact Scalar.fill Scalar.executionPrice Scalar.priceScale Scalar.grossOutput,
  .mulDivFloor Scalar.grossOutput Scalar.policyFeeBps Scalar.feeDenominator Scalar.feeOutput
]

private def balanceProgram : List Op := [
  .scalarLe Scalar.fill Scalar.sellerClaims,
  .addLe Scalar.grossOutput Scalar.feeOutput Scalar.buyerCollateral,
  .addFitsU64 Scalar.buyerClaims Scalar.fill,
  .addFitsU64 Scalar.sellerCollateral Scalar.grossOutput,
  .addFitsU64 Scalar.venueCollateral Scalar.feeOutput
]

/-- Width-independent Direct ordinary admission and derivation program. -/
def program : List Op :=
  setupProgram ++
    (admissionProgram ++ (replayProgram ++ (pricingProgram ++ balanceProgram)))

private def runStages (initial : State) : Option State := do
  let setup ← run setupProgram initial
  let admitted ← run admissionProgram setup
  let replayed ← run replayProgram admitted
  let priced ← run pricingProgram replayed
  run balanceProgram priced

private theorem run_program_stages (initial : State) :
    run program initial = runStages initial := by
  simp only [program, runStages, run_append, run_append_fn]
  rfl

  theorem register_shape (frame : FillFrame) :
    (state frame).scalars.size = Scalar.count ∧
    (state frame).identities.size = Identity.count := by
  simp [state, registerState, Scalar.count, Identity.count,
    ScalarSlot.all, IdentitySlot.all]

theorem program_length : program.length = 35 := by
  native_decide

def outputs (state : State) : Option (Nat × Nat × Nat × Nat) := do
  some (← scalar state Scalar.sellerNonceOutput,
    ← scalar state Scalar.buyerNonceOutput,
    ← scalar state Scalar.grossOutput,
    ← scalar state Scalar.feeOutput)

theorem example_runs :
    (run program (state Examples.frame)).bind outputs = some (1, 1, 1000, 2) := by
  native_decide

theorem hostile_zero_fill_refuses :
    run program (state Examples.hostileZeroFill) = none := by
  native_decide

set_option maxHeartbeats 20000 in
private theorem setup_runs (frame : FillFrame) :
    run setupProgram (state frame) = some (setupState frame) := by
  simp [setupProgram, state, setupState, registerState, run, step, setScalar,
    Scalar.zero, Scalar.one, Scalar.feeDenominator]

set_option maxHeartbeats 20000 in
private theorem admission_runs (frame : FillFrame) (admitted : Admissible frame) :
    run admissionProgram (setupState frame) = some (setupState frame) := by
  have positiveFill : frame.fill ≠ 0 := Nat.ne_of_gt admitted.positiveFill
  have buyerOutcomeInDomain :
      frame.buyerIntent.outcome < frame.product.outcomeCount := by
    rw [← admitted.sameOutcome]
    exact admitted.outcomeInDomain
  simp [admissionProgram, setupState, registerState, run, step, scalar, identity,
    require, Scalar.phase, Scalar.one, Scalar.fill, Scalar.sellerFrom, Scalar.slot,
    Scalar.sellerThrough, Scalar.buyerFrom, Scalar.buyerThrough, Scalar.sellerSide,
    Scalar.zero, Scalar.buyerSide, Scalar.sellerGeneration, Scalar.buyerGeneration,
    Scalar.sellerOutcome, Scalar.buyerOutcome, Scalar.outcomeCount,
    Identity.sellerMarket, Identity.buyerMarket, Identity.sellerMaker,
    Identity.buyerMaker, phaseTag, sideTag, admitted.phaseOpen, positiveFill,
    admitted.slotAfterStart, admitted.slotBeforeSellerEnd,
    admitted.buyerSlotAfterStart, admitted.buyerSlotBeforeEnd,
    admitted.sellerSide, admitted.buyerSide, admitted.sameMarket,
    admitted.sameGeneration, admitted.sameOutcome, admitted.distinctMakers,
    buyerOutcomeInDomain]

set_option maxHeartbeats 100000 in
private theorem replay_runs (frame : FillFrame) (admitted : Admissible frame) :
    run replayProgram (setupState frame) = some (replayState frame) := by
  rcases admitted with ⟨_, _, _, _, _, _, _, _, _, _, _, _, _, _, sellerLifecycle,
    buyerLifecycle, sellerNonce, buyerNonce, sellerNonceCanAdvance,
    buyerNonceCanAdvance, _⟩
  have sellerDecision :
      decide (frame.sellerIntent.lifecycle.accepts
        frame.sellerIntent.maxFill frame.fill) = true := by
    simpa using sellerLifecycle
  have buyerDecision :
      decide (frame.buyerIntent.lifecycle.accepts
        frame.buyerIntent.maxFill frame.fill) = true := by
    simpa using buyerLifecycle
  cases sellerTag : frame.sellerIntent.lifecycle <;>
    cases buyerTag : frame.buyerIntent.lifecycle <;>
    simp only [sellerTag, buyerTag, Lifecycle.accepts] at sellerDecision buyerDecision
  all_goals
    have sellerAccepted := of_decide_eq_true sellerDecision
    have buyerAccepted := of_decide_eq_true buyerDecision
    simp [replayProgram, setupState, replayState, registerState, run, step, scalar,
      setScalar, require, Scalar.sellerLifecycle, Scalar.sellerMaximum,
      Scalar.fill, Scalar.buyerLifecycle, Scalar.buyerMaximum, Scalar.sellerNonce,
      Scalar.sellerNextNonce, Scalar.buyerNonce, Scalar.buyerNextNonce,
      Scalar.sellerNonceOutput, Scalar.buyerNonceOutput, lifecycleTag,
      sellerTag, buyerTag, if_pos sellerAccepted, if_pos buyerAccepted,
      sellerNonce, buyerNonce,
      sellerNonceCanAdvance, buyerNonceCanAdvance]

set_option maxHeartbeats 20000 in
private theorem pricing_runs (frame : FillFrame) (admitted : Admissible frame) :
    run pricingProgram (replayState frame) = some (derivedState frame) := by
  have priceScaleNonzero : frame.product.priceScale ≠ 0 :=
    Nat.ne_of_gt frame.product.priceScalePositive
  have exactRemainder :
    frame.fill * frame.executionPrice % frame.product.priceScale = 0 := by
    rw [admitted.exactQuote]
    simp
  have exactGross :
      frame.fill * frame.executionPrice / frame.product.priceScale = frame.gross := by
    rw [admitted.exactQuote]
    simpa [Nat.mul_comm] using
      Nat.mul_div_right frame.gross frame.product.priceScalePositive
  have feeQuotientFits :
      frame.gross * frame.feePolicy.basisPoints / feeDenominator < u64Limit := by
    rw [← admitted.exactFloorFee]
    exact admitted.feeU64
  have feePolicyBounded : frame.feePolicy.basisPoints ≤ 10000 := by
    simpa [DClutch.Direct.feeDenominator] using
      frame.feePolicy.basisPointsBounded
  have feeQuotientFits10000 :
      frame.gross * frame.feePolicy.basisPoints / 10000 < u64Limit := by
    simpa [DClutch.Direct.feeDenominator] using feeQuotientFits
  simp [pricingProgram, replayState, derivedState, registerState, run, step, scalar,
    setScalar, require, Scalar.sellerLimit, Scalar.executionPrice,
    Scalar.buyerLimit, Scalar.priceScale, Scalar.sellerFeeBps,
    Scalar.policyFeeBps, Scalar.buyerFeeBps, Scalar.feeDenominator, Scalar.fill,
    Scalar.grossOutput, Scalar.feeOutput, admitted.sellerPrice,
    admitted.buyerPrice, admitted.priceInScale, admitted.sellerFeeRate,
    admitted.buyerFeeRate, feePolicyBounded, exactRemainder,
    exactGross, admitted.exactFloorFee, admitted.grossU64, priceScaleNonzero,
    feeQuotientFits10000, DClutch.Direct.feeDenominator]

set_option maxHeartbeats 20000 in
private theorem balance_runs (frame : FillFrame) (admitted : Admissible frame) :
    run balanceProgram (derivedState frame) = some (derivedState frame) := by
  simp [balanceProgram, derivedState, registerState, run, step, scalar, require,
    Scalar.fill, Scalar.sellerClaims, Scalar.grossOutput, Scalar.feeOutput,
    Scalar.buyerCollateral, Scalar.buyerClaims, Scalar.sellerCollateral,
    Scalar.venueCollateral, admitted.sellerHasClaims, admitted.buyerHasCollateral,
    admitted.buyerClaimCreditFits, admitted.sellerCollateralCreditFits,
    admitted.venueCreditFits]

private theorem bind_of_success
    {α β : Type} {value : Option α} {result : α} {next : α → Option β}
    (success : value = some result) : value.bind next = next result := by
  rw [success]
  exact Option.bind_some result next

private theorem run_stages_admitted (frame : FillFrame) (admitted : Admissible frame) :
    runStages (state frame) = some (derivedState frame) := by
  unfold runStages
  change (run setupProgram (state frame)).bind (fun setup =>
    (run admissionProgram setup).bind (fun admittedState =>
      (run replayProgram admittedState).bind (fun replayed =>
        (run pricingProgram replayed).bind (run balanceProgram)))) =
          some (derivedState frame)
  rw [bind_of_success (setup_runs frame)]
  rw [bind_of_success (admission_runs frame admitted)]
  rw [bind_of_success (replay_runs frame admitted)]
  rw [bind_of_success (pricing_runs frame admitted)]
  exact balance_runs frame admitted

/-- Every semantically admitted frame is accepted by the generated transition
program, which derives exactly the semantic successor nonces, gross quote, and
named floor fee. Physical register decoding remains an adapter obligation. -/
theorem admitted_program_refines
    (frame : FillFrame) (admitted : Admissible frame) :
    (run program (state frame)).bind outputs =
      some (frame.pre.sellerNextNonce + 1,
        frame.pre.buyerNextNonce + 1, frame.gross, frame.fee) := by
  rw [run_program_stages, run_stages_admitted frame admitted]
  rfl

theorem encoded_program_length :
    (TransitionVM.Codec.encodeProgram program).length = 568 := by
  native_decide

end DClutch.DirectProgram
