import DClutchSemantics.GeneralClearing

/-!
# Executable General-clearing witnesses

These examples exercise the whole successor semantic lifecycle with concrete
data. They are proofs about the Lean model, not claims about the SBF adapter.
-/

namespace DClutch.General.Examples

def localProfile : PhysicalProfile := {
  outcomeCount := {
    value := 16
    authority := .measuredProfile
    profileId := 1
    liftingPlanId := 0
  }
  executionsPerPage := {
    value := 32
    authority := .chainDerived
    profileId := 2
    liftingPlanId := 0
  }
  pagesPerCandidate := {
    value := 64
    authority := .provisional
    profileId := 3
    liftingPlanId := 9001
  }
  scalarLimit := {
    value := 18446744073709551615
    authority := .mathematical
    profileId := 4
    liftingPlanId := 0
  }
}

example : localProfile.valid = true := by native_decide

example :
    ({
      value := 64
      authority := .provisional
      profileId := 3
      liftingPlanId := 0
    } : CapacityBound).valid = false := by native_decide

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

def defaultSelectionPolicy : SelectionPolicy := {
  policyId := 501
  criteria := [.maximizeFilledLots, .minimizeQuoteSurplus, .minimizeCandidateId]
}

example : defaultSelectionPolicy.valid = true := by native_decide

def emptySelection : Selection := {
  policy := defaultSelectionPolicy
  closed := false
  best := none
}

def selectedMintCandidate : Selection := {
  policy := defaultSelectionPolicy
  closed := false
  best := some mintCandidate
}

example :
    selectedMintCandidate.consider malformedCandidate = selectedMintCandidate := by native_decide

example :
    emptySelection.run (.consider mintCandidate) = selectedMintCandidate := by native_decide

example :
    emptySelection.run .freeze = emptySelection := by native_decide

example :
    selectedMintCandidate.run .freeze =
      { selectedMintCandidate with closed := true } := by native_decide

/-! Fragmentation does not create another rounding boundary. Each order is
split across two executions, but the aggregate half-atom receipts round once:
one quote atom per order, not one atom per fragment. -/

def receiveTwoOutcomeZero : Order := {
  receiveOutcomeZero with orderId := 103, maxLots := 2
}

def receiveTwoOutcomeOne : Order := {
  receiveOutcomeOne with orderId := 104, maxLots := 2
}

def fragmentedCandidate : Candidate := {
  mintCandidate with
  candidateId := 203
  pages := [
    { executions := [
      { order := receiveTwoOutcomeZero, lots := 1, quoteDebit := 1, quoteCredit := 0 },
      { order := receiveTwoOutcomeOne, lots := 1, quoteDebit := 1, quoteCredit := 0 }
    ] },
    { executions := [
      { order := receiveTwoOutcomeZero, lots := 1, quoteDebit := 0, quoteCredit := 0 },
      { order := receiveTwoOutcomeOne, lots := 1, quoteDebit := 0, quoteCredit := 0 }
    ] }
  ]
}

example : fragmentedCandidate.valid = true := by native_decide
example : fragmentedCandidate.completeSetMove = .mint 2 := by native_decide
example : quoteInputs fragmentedCandidate = 2 := by native_decide
example : fragmentedCandidate.quoteDebitFor 103 = 1 := by native_decide
example : fragmentedCandidate.quoteDebitFor 104 = 1 := by native_decide

end DClutch.General.Examples
