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

namespace Scalar

def phase := 0
def slot := 1
def sellerFrom := 2
def sellerThrough := 3
def buyerFrom := 4
def buyerThrough := 5
def sellerSide := 6
def buyerSide := 7
def sellerGeneration := 8
def buyerGeneration := 9
def sellerOutcome := 10
def buyerOutcome := 11
def outcomeCount := 12
def sellerLifecycle := 13
def sellerMaximum := 14
def buyerLifecycle := 15
def buyerMaximum := 16
def sellerNonce := 17
def buyerNonce := 18
def sellerNextNonce := 19
def buyerNextNonce := 20
def sellerLimit := 21
def executionPrice := 22
def buyerLimit := 23
def priceScale := 24
def sellerFeeBps := 25
def buyerFeeBps := 26
def policyFeeBps := 27
def fill := 28
def sellerClaims := 29
def buyerClaims := 30
def buyerCollateral := 31
def sellerCollateral := 32
def venueCollateral := 33
def grossOutput := 34
def feeOutput := 35
def zero := 36
def one := 37
def feeDenominator := 38
def sellerNonceOutput := 39
def buyerNonceOutput := 40
def count := 41

end Scalar

namespace Identity

def sellerMarket := 0
def buyerMarket := 1
def sellerMaker := 2
def buyerMaker := 3
def count := 4

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
  simp [state, registerState, Scalar.count, Identity.count]

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
