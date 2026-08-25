import DClutchSemantics.GeneralClearing

/-!
# Executable General-clearing witnesses

These examples exercise the whole successor semantic lifecycle with concrete
data. They are proofs about the Lean model, not claims about the SBF adapter.
-/

namespace DClutch.General.Examples

def receiveOutcomeZero : Order := {
  orderId := 101
  ownerId := 11
  nonce := 1
  receivePerLot := [1, 0]
  deliverPerLot := [0, 0]
  maxLots := 1
  maxQuoteDebitPerLot := 1
}

def receiveOutcomeOne : Order := {
  orderId := 102
  ownerId := 12
  nonce := 1
  receivePerLot := [0, 1]
  deliverPerLot := [0, 0]
  maxLots := 1
  maxQuoteDebitPerLot := 1
}

def mintPage : Page := {
  executions := [
    { order := receiveOutcomeZero, lots := 1, quoteDebit := 1, quoteCredit := 0 },
    { order := receiveOutcomeOne, lots := 1, quoteDebit := 1, quoteCredit := 0 }
  ]
}

/-- Two buyers jointly demand one complete set at the exact `[1, 1] / 2`
simplex. Each half-atom debit rounds upward to one atom. Settlement spends one
atom to mint the complete set and routes the remaining atom as the explicit
quote surplus. -/
def mintCandidate : Candidate := {
  candidateId := 201
  productId := 301
  batchId := 401
  outcomeCount := 2
  prices := { scale := 2, coordinates := [1, 1] }
  pages := [mintPage]
}

example : mintCandidate.valid = true := by native_decide

example : mintCandidate.completeSetMove = .mint 1 := by native_decide

example : quoteInputs mintCandidate = 2 := by native_decide

example : mintCandidate.quoteAfterMaterialization = some 1 := by native_decide

def state0 : SettlementState := initialSettlement mintCandidate

def state1 : SettlementState :=
  runState mintCandidate state0 (.collect mintPage)

def state2 : SettlementState :=
  runState mintCandidate state1 .materialize

def state3 : SettlementState :=
  runState mintCandidate state2 (.distribute mintPage)

def state4 : SettlementState :=
  runState mintCandidate state3 .close

example : state1.phase = .materializing := by native_decide
example : state1.claimInventory = [0, 0] ∧ state1.quoteInventory = 2 := by native_decide

example : state2.phase = .distributing 0 := by native_decide
example : state2.claimInventory = [1, 1] ∧ state2.quoteInventory = 1 := by native_decide

example : state3.phase = .readyToClose := by native_decide
example : state3.claimInventory = [0, 0] ∧ state3.quoteInventory = 1 := by native_decide

example : state4.phase = .terminal := by native_decide
example : state4.claimInventory = [0, 0] ∧ state4.quoteInventory = 0 ∧
    state4.quoteSurplusPaid = 1 := by native_decide

/-- A forged page cannot move the collection cursor or inventory. -/
def forgedPage : Page := { executions := [] }

example : runState mintCandidate state0 (.collect forgedPage) = state0 := by native_decide

/-- Distribution before the sole materialization boundary is a checked
refusal with complete semantic rollback. -/
example : runState mintCandidate state1 (.distribute mintPage) = state1 := by native_decide

/-- Candidate selection never admits an invalid higher-volume impostor. -/
def malformedCandidate : Candidate := {
  mintCandidate with
  candidateId := 202
  prices := { scale := 2, coordinates := [2, 2] }
}

example : malformedCandidate.valid = false := by native_decide

example :
    ({ closed := false, best := some mintCandidate } : Selection).consider malformedCandidate =
      { closed := false, best := some mintCandidate } := by native_decide

example :
    ({ closed := false, best := none } : Selection).run (.consider mintCandidate) =
      { closed := false, best := some mintCandidate } := by native_decide

example :
    ({ closed := false, best := none } : Selection).run .freeze =
      { closed := false, best := none } := by native_decide

example :
    ({ closed := false, best := some mintCandidate } : Selection).run .freeze =
      { closed := true, best := some mintCandidate } := by native_decide

end DClutch.General.Examples
