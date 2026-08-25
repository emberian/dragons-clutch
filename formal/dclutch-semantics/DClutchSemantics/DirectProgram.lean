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

/-- Physical-u64 register projection. Gross and fee are deliberately absent as
caller authority: their output slots start at zero and are derived by code. -/
def state (frame : FillFrame) : State := {
  scalars := [
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
    0, 0, 0, 0, 0, 0, 0
  ]
  identities := [
    frame.sellerIntent.market,
    frame.buyerIntent.market,
    frame.sellerIntent.maker,
    frame.buyerIntent.maker
  ]
}

/-- Width-independent Direct ordinary admission and derivation program. -/
def program : List Op := [
  .loadConst Scalar.zero 0,
  .loadConst Scalar.one 1,
  .loadConst Scalar.feeDenominator feeDenominator,
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
  .scalarLt Scalar.sellerOutcome Scalar.outcomeCount,
  .lifecycleAccepts Scalar.sellerLifecycle Scalar.sellerMaximum Scalar.fill,
  .lifecycleAccepts Scalar.buyerLifecycle Scalar.buyerMaximum Scalar.fill,
  .scalarEq Scalar.sellerNonce Scalar.sellerNextNonce,
  .scalarEq Scalar.buyerNonce Scalar.buyerNextNonce,
  .incrementInto Scalar.sellerNextNonce Scalar.sellerNonceOutput,
  .incrementInto Scalar.buyerNextNonce Scalar.buyerNonceOutput,
  .scalarLe Scalar.sellerLimit Scalar.executionPrice,
  .scalarLe Scalar.executionPrice Scalar.buyerLimit,
  .scalarLe Scalar.executionPrice Scalar.priceScale,
  .scalarEq Scalar.sellerFeeBps Scalar.policyFeeBps,
  .scalarEq Scalar.buyerFeeBps Scalar.policyFeeBps,
  .scalarLe Scalar.policyFeeBps Scalar.feeDenominator,
  .mulDivExact Scalar.fill Scalar.executionPrice Scalar.priceScale Scalar.grossOutput,
  .mulDivFloor Scalar.grossOutput Scalar.policyFeeBps Scalar.feeDenominator Scalar.feeOutput,
  .scalarLe Scalar.fill Scalar.sellerClaims,
  .addLe Scalar.grossOutput Scalar.feeOutput Scalar.buyerCollateral,
  .addFitsU64 Scalar.buyerClaims Scalar.fill,
  .addFitsU64 Scalar.sellerCollateral Scalar.grossOutput,
  .addFitsU64 Scalar.venueCollateral Scalar.feeOutput
]

theorem register_shape (frame : FillFrame) :
    (state frame).scalars.length = Scalar.count ∧
    (state frame).identities.length = Identity.count := by
  simp [state, Scalar.count, Identity.count]

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

theorem encoded_program_length :
    (TransitionVM.Codec.encodeProgram program).length = 568 := by
  native_decide

end DClutch.DirectProgram
