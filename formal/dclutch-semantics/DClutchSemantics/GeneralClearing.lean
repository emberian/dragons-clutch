import Std.Tactic

/-!
# Certificate-driven General clearing

This is the semantic owner for the successor General venue.  A candidate is
data: exact simplex prices and bounded pages of authenticated portfolio fills.
There is one width-independent verifier and one streamed settlement machine;
there is deliberately no family of `N`-specialized transition functions.

The model separates three questions which the earlier implementation mixed:

* candidate validity (orders, cumulative fills, canonical quote rounding, and
  complete-set balance),
* deterministic selection of the best valid submitted candidate, and
* physical settlement (collect every input, perform the sole complete-set
  mint/merge, then distribute every output).

Hashes, signatures, account ownership, fixed-width overflow, CPI, persistence,
and transaction rollback remain named adapter/runtime obligations.
-/

namespace DClutch.General

def valueAt (values : List Nat) (index : Nat) : Nat :=
  values[index]?.getD 0

def addVectors (left right : List Nat) : List Nat :=
  List.zipWith (.+.) left right

def subVectors (left right : List Nat) : List Nat :=
  List.zipWith (.-.) left right

def scaleVector (quantity : Nat) (values : List Nat) : List Nat :=
  values.map (quantity * .)

def allIndices (count : Nat) (predicate : Nat → Bool) : Bool :=
  (List.range count).all predicate

def allZero (values : List Nat) : Bool :=
  values.all fun value => value == 0

def vectorAtMost (available requested : List Nat) : Bool :=
  available.length = requested.length &&
    (List.zipWith (fun want availableValue => decide (want ≤ availableValue))
      requested available).all id

theorem valueAt_le_sum
    (values : List Nat) (index : Nat) (inBounds : index < values.length) :
    valueAt values index ≤ values.sum := by
  induction values generalizing index with
  | nil => simp at inBounds
  | cons head tail induction =>
      cases index with
      | zero => simp [valueAt]
      | succ index =>
          have tailBounds : index < tail.length := by simpa using inBounds
          calc
            valueAt (head :: tail) (index + 1) = valueAt tail index := by
              simp [valueAt]
            _ ≤ tail.sum := induction index tailBounds
            _ ≤ (head :: tail).sum := by simp

/-! ## Product and signed-order data -/

/-- Exact finite simplex. `scale` is the sole denominator used by quote
rounding.  There is no semantic maximum outcome count. -/
structure PriceVector where
  scale : Nat
  coordinates : List Nat
  deriving DecidableEq, Repr

def PriceVector.validFor (prices : PriceVector) (outcomeCount : Nat) : Bool :=
  0 < outcomeCount && 0 < prices.scale &&
    prices.coordinates.length = outcomeCount && prices.coordinates.sum = prices.scale

theorem PriceVector.coordinate_bounded
    (prices : PriceVector) (outcomeCount index : Nat)
    (valid : prices.validFor outcomeCount = true) (inBounds : index < outcomeCount) :
    valueAt prices.coordinates index ≤ prices.scale := by
  simp only [PriceVector.validFor, Bool.and_eq_true, decide_eq_true_eq] at valid
  obtain ⟨⟨⟨countPositive, scalePositive⟩, width⟩, sumExact⟩ := valid
  have listBounds : index < prices.coordinates.length := by omega
  have coordinateLeSum := valueAt_le_sum prices.coordinates index listBounds
  omega

/-- One immutable signed portfolio order. `receivePerLot` and `deliverPerLot`
are nonnegative claim vectors; their difference is the signed payoff change.
`maxQuoteDebitPerLot` is the trader's exact limit. -/
structure Order where
  orderId : Nat
  ownerId : Nat
  nonce : Nat
  receivePerLot : List Nat
  deliverPerLot : List Nat
  maxLots : Nat
  maxQuoteDebitPerLot : Nat
  deriving DecidableEq, Repr

def Order.validFor (order : Order) (outcomeCount : Nat) : Bool :=
  order.orderId != 0 && order.ownerId != 0 &&
    order.receivePerLot.length = outcomeCount &&
    order.deliverPerLot.length = outcomeCount && 0 < order.maxLots

/-- A candidate references an authenticated order and chooses a positive
number of its still-available atomic lots. Quote fields are certificates, not
authority: the verifier recomputes them from the simplex. -/
structure Execution where
  order : Order
  lots : Nat
  quoteDebit : Nat
  quoteCredit : Nat
  deriving DecidableEq, Repr

structure Page where
  executions : List Execution
  deriving DecidableEq, Repr

structure Candidate where
  candidateId : Nat
  productId : Nat
  batchId : Nat
  outcomeCount : Nat
  prices : PriceVector
  pages : List Page
  deriving DecidableEq, Repr

def Candidate.executions (candidate : Candidate) : List Execution :=
  candidate.pages.flatMap Page.executions

def Candidate.filledLots (candidate : Candidate) (orderId : Nat) : Nat :=
  (candidate.executions.filter fun execution => execution.order.orderId = orderId)
    |>.map Execution.lots |>.sum

/-- A content identity binds one and only one order preimage within a
candidate. This prevents same-ID term substitution across pages. -/
def Candidate.identitiesCanonical (candidate : Candidate) : Bool :=
  candidate.executions.all fun left =>
    candidate.executions.all fun right =>
      left.order.orderId != right.order.orderId || left.order = right.order

def weightedValue
    (prices : PriceVector) (quantities : List Nat) (lots : Nat) : Nat :=
  (List.zipWith (· * ·) prices.coordinates quantities).sum * lots

/-- The one named quote-rounding boundary.

Net claim receipt is debit-rounded upward; net claim delivery is
credit-rounded downward. Therefore per-fill rounding cannot spend collateral
which was not collected. -/
def roundedQuote (prices : PriceVector) (execution : Execution) : Nat × Nat :=
  let received := weightedValue prices execution.order.receivePerLot execution.lots
  let delivered := weightedValue prices execution.order.deliverPerLot execution.lots
  if delivered ≤ received then
    ((received - delivered + prices.scale - 1) / prices.scale, 0)
  else
    (0, (delivered - received) / prices.scale)

def executionValid (candidate : Candidate) (execution : Execution) : Bool :=
  execution.order.validFor candidate.outcomeCount && 0 < execution.lots &&
    candidate.filledLots execution.order.orderId ≤ execution.order.maxLots &&
    roundedQuote candidate.prices execution =
      (execution.quoteDebit, execution.quoteCredit) &&
    execution.quoteDebit ≤ execution.order.maxQuoteDebitPerLot * execution.lots

def claimInputs (candidate : Candidate) : List Nat :=
  (List.range candidate.outcomeCount).map fun outcome =>
    candidate.executions |>.map (fun execution =>
      valueAt execution.order.deliverPerLot outcome * execution.lots) |>.sum

def claimOutputs (candidate : Candidate) : List Nat :=
  (List.range candidate.outcomeCount).map fun outcome =>
    candidate.executions |>.map (fun execution =>
      valueAt execution.order.receivePerLot outcome * execution.lots) |>.sum

def quoteInputs (candidate : Candidate) : Nat :=
  candidate.executions |>.map Execution.quoteDebit |>.sum

def quoteOutputs (candidate : Candidate) : Nat :=
  candidate.executions |>.map Execution.quoteCredit |>.sum

/-- The only aggregate liability change a fully collateralized categorical
Market can admit: none, mint `q` complete sets, or merge `q` complete sets. -/
inductive CompleteSetMove where
  | none
  | mint (quantity : Nat)
  | merge (quantity : Nat)
  deriving DecidableEq, Repr

def Candidate.completeSetMove (candidate : Candidate) : CompleteSetMove :=
  let inputs := valueAt (claimInputs candidate) 0
  let outputs := valueAt (claimOutputs candidate) 0
  if inputs = outputs then .none
  else if inputs < outputs then .mint (outputs - inputs)
  else .merge (inputs - outputs)

def Candidate.claimsBalance (candidate : Candidate) : Bool :=
  match candidate.completeSetMove with
  | .none => claimInputs candidate = claimOutputs candidate
  | .mint quantity => 0 < quantity && allIndices candidate.outcomeCount fun outcome =>
      valueAt (claimOutputs candidate) outcome =
        valueAt (claimInputs candidate) outcome + quantity
  | .merge quantity => 0 < quantity && allIndices candidate.outcomeCount fun outcome =>
      valueAt (claimInputs candidate) outcome =
        valueAt (claimOutputs candidate) outcome + quantity

def Candidate.quoteAfterMaterialization (candidate : Candidate) : Option Nat :=
  match candidate.completeSetMove with
  | .none => some (quoteInputs candidate)
  | .mint quantity =>
      if quantity ≤ quoteInputs candidate then some (quoteInputs candidate - quantity) else none
  | .merge quantity => some (quoteInputs candidate + quantity)

def Candidate.quoteBalances (candidate : Candidate) : Bool :=
  match candidate.quoteAfterMaterialization with
  | none => false
  | some available => quoteOutputs candidate ≤ available

/-- Complete executable candidate admission. Bounds on pages/accounts are a
physical profile concern; this semantic verifier accepts any finite data that
fits its authenticated representation. -/
def Candidate.valid (candidate : Candidate) : Bool :=
  candidate.candidateId != 0 && candidate.productId != 0 && candidate.batchId != 0 &&
    candidate.prices.validFor candidate.outcomeCount && !candidate.pages.isEmpty &&
    candidate.pages.all (fun page => !page.executions.isEmpty) &&
    candidate.identitiesCanonical &&
    candidate.executions.all (executionValid candidate) &&
    candidate.claimsBalance && candidate.quoteBalances

/-! ## Deterministic best-valid-submitted selection -/

structure Objective where
  filledLots : Nat
  quoteSurplus : Nat
  candidateId : Nat
  deriving DecidableEq, Repr

def Candidate.objective (candidate : Candidate) : Objective := {
  filledLots := candidate.executions.map Execution.lots |>.sum
  quoteSurplus := candidate.quoteAfterMaterialization.getD 0 - quoteOutputs candidate
  candidateId := candidate.candidateId
}

/-- Higher filled volume wins; then lower surplus; then lower content identity.
This is deterministic best-submitted selection, not an optimality claim. -/
def Objective.better (left right : Objective) : Bool :=
  left.filledLots > right.filledLots ||
    (left.filledLots = right.filledLots &&
      (left.quoteSurplus < right.quoteSurplus ||
        (left.quoteSurplus = right.quoteSurplus && left.candidateId < right.candidateId)))

structure Selection where
  closed : Bool
  best : Option Candidate
  deriving DecidableEq, Repr

def Selection.consider (selection : Selection) (candidate : Candidate) : Selection :=
  if selection.closed || !candidate.valid then selection
  else match selection.best with
    | none => { selection with best := some candidate }
    | some incumbent =>
        if candidate.objective.better incumbent.objective
        then { selection with best := some candidate }
        else selection

def Selection.freeze (selection : Selection) : Selection :=
  { selection with closed := true }

theorem invalid_candidate_never_selected
    (selection : Selection) (candidate : Candidate)
    (invalid : candidate.valid = false) :
    selection.consider candidate = selection := by
  simp [Selection.consider, invalid]

theorem closed_selection_is_immutable
    (selection : Selection) (candidate : Candidate)
    (closed : selection.closed = true) :
    selection.consider candidate = selection := by
  simp [Selection.consider, closed]

/-! ## Streamed physical settlement -/

def Page.claimInputs (candidate : Candidate) (page : Page) : List Nat :=
  (List.range candidate.outcomeCount).map fun outcome =>
    page.executions |>.map (fun execution =>
      valueAt execution.order.deliverPerLot outcome * execution.lots) |>.sum

def Page.claimOutputs (candidate : Candidate) (page : Page) : List Nat :=
  (List.range candidate.outcomeCount).map fun outcome =>
    page.executions |>.map (fun execution =>
      valueAt execution.order.receivePerLot outcome * execution.lots) |>.sum

def Page.quoteInputs (page : Page) : Nat :=
  page.executions.map Execution.quoteDebit |>.sum

def Page.quoteOutputs (page : Page) : Nat :=
  page.executions.map Execution.quoteCredit |>.sum

inductive SettlementPhase where
  | collecting (nextPage : Nat)
  | materializing
  | distributing (nextPage : Nat)
  | readyToClose
  | terminal
  deriving DecidableEq, Repr

structure SettlementState where
  candidateId : Nat
  phase : SettlementPhase
  claimInventory : List Nat
  quoteInventory : Nat
  quoteSurplusPaid : Nat
  deriving DecidableEq, Repr

def initialSettlement (candidate : Candidate) : SettlementState := {
  candidateId := candidate.candidateId
  phase := .collecting 0
  claimInventory := List.replicate candidate.outcomeCount 0
  quoteInventory := 0
  quoteSurplusPaid := 0
}

def SettlementState.validFor (candidate : Candidate) (state : SettlementState) : Bool :=
  state.candidateId = candidate.candidateId &&
    state.claimInventory.length = candidate.outcomeCount &&
    match state.phase with
    | .collecting cursor => cursor ≤ candidate.pages.length
    | .distributing cursor => cursor ≤ candidate.pages.length
    | .materializing | .readyToClose => true
    | .terminal => allZero state.claimInventory && state.quoteInventory = 0

inductive SettlementCommand where
  | collect (page : Page)
  | materialize
  | distribute (page : Page)
  | close
  deriving DecidableEq, Repr

def expectedPage? (candidate : Candidate) (cursor : Nat) : Option Page :=
  candidate.pages[cursor]?

def SettlementState.commandAccepts
    (candidate : Candidate) (state : SettlementState) : SettlementCommand → Bool
  | .collect page => match state.phase with
      | .collecting cursor =>
          expectedPage? candidate cursor = some page && cursor < candidate.pages.length
      | _ => false
  | .materialize => state.phase = .materializing &&
      match candidate.completeSetMove with
      | .none => true
      | .mint quantity => quantity ≤ state.quoteInventory
      | .merge quantity =>
          allIndices candidate.outcomeCount fun outcome =>
            quantity ≤ valueAt state.claimInventory outcome
  | .distribute page => match state.phase with
      | .distributing cursor =>
          expectedPage? candidate cursor = some page && cursor < candidate.pages.length &&
          vectorAtMost state.claimInventory (page.claimOutputs candidate) &&
          page.quoteOutputs ≤ state.quoteInventory
      | _ => false
  | .close => state.phase = .readyToClose && allZero state.claimInventory

def collectPost
    (candidate : Candidate) (state : SettlementState) (page : Page) : SettlementState :=
  let cursor := match state.phase with | .collecting next => next | _ => 0
  { state with
    phase := if cursor + 1 = candidate.pages.length
      then .materializing else .collecting (cursor + 1)
    claimInventory := addVectors state.claimInventory (page.claimInputs candidate)
    quoteInventory := state.quoteInventory + page.quoteInputs }

def materializePost (candidate : Candidate) (state : SettlementState) : SettlementState :=
  match candidate.completeSetMove with
  | .none => { state with phase := .distributing 0 }
  | .mint quantity => { state with
      phase := .distributing 0
      claimInventory := state.claimInventory.map (fun value => value + quantity)
      quoteInventory := state.quoteInventory - quantity }
  | .merge quantity => { state with
      phase := .distributing 0
      claimInventory := state.claimInventory.map (fun value => value - quantity)
      quoteInventory := state.quoteInventory + quantity }

def distributePost
    (candidate : Candidate) (state : SettlementState) (page : Page) : SettlementState :=
  let cursor := match state.phase with | .distributing next => next | _ => 0
  { state with
    phase := if cursor + 1 = candidate.pages.length
      then .readyToClose else .distributing (cursor + 1)
    claimInventory := subVectors state.claimInventory (page.claimOutputs candidate)
    quoteInventory := state.quoteInventory - page.quoteOutputs }

/-- Closing routes the exact deterministic quote remainder to the capability's
declared surplus beneficiary. It never leaves unowned collateral in the
settlement cursor. -/
def closePost (state : SettlementState) : SettlementState :=
  { state with
    phase := .terminal
    quoteInventory := 0
    quoteSurplusPaid := state.quoteSurplusPaid + state.quoteInventory }

def postState
    (candidate : Candidate) (state : SettlementState) : SettlementCommand → SettlementState
  | .collect page => collectPost candidate state page
  | .materialize => materializePost candidate state
  | .distribute page => distributePost candidate state page
  | .close => closePost state

inductive Refusal where
  | invalidCandidate
  | invalidState
  | commandRefused
  | postInvariantFailure
  deriving DecidableEq, Repr

structure SettlementResult
    (candidate : Candidate) (pre : SettlementState) (command : SettlementCommand) where
  post : SettlementState
  exact : post = postState candidate pre command
  valid : post.validFor candidate = true

/-- Total transition boundary. Candidate and state are revalidated on every
permissionless continuation; no cursor transition is trusted merely because a
client constructed it. -/
def execute?
    (candidate : Candidate) (pre : SettlementState) (command : SettlementCommand) :
    Except Refusal (SettlementResult candidate pre command) :=
  if _candidateValid : candidate.valid = true then
    if _stateValid : pre.validFor candidate = true then
      if _accepted : pre.commandAccepts candidate command = true then
        let candidatePost := postState candidate pre command
        if postValid : candidatePost.validFor candidate = true then
          .ok { post := candidatePost, exact := rfl, valid := postValid }
        else .error .postInvariantFailure
      else .error .commandRefused
    else .error .invalidState
  else .error .invalidCandidate

def runState
    (candidate : Candidate) (pre : SettlementState) (command : SettlementCommand) :
    SettlementState :=
  match execute? candidate pre command with
  | .ok result => result.post
  | .error _ => pre

theorem refusal_rolls_back
    (candidate : Candidate) (pre : SettlementState) (command : SettlementCommand)
    (refusal : Refusal) (failed : execute? candidate pre command = .error refusal) :
    runState candidate pre command = pre := by
  unfold runState
  rw [failed]

theorem successful_transition_is_exact
    (candidate : Candidate) (pre : SettlementState) (command : SettlementCommand)
    (result : SettlementResult candidate pre command) :
    result.post = postState candidate pre command := result.exact

theorem successful_transition_is_valid
    (candidate : Candidate) (pre : SettlementState) (command : SettlementCommand)
    (result : SettlementResult candidate pre command) :
    result.post.validFor candidate = true := result.valid

theorem close_routes_all_quote
    (state : SettlementState) :
    (closePost state).quoteInventory = 0 ∧
    (closePost state).quoteSurplusPaid =
      state.quoteSurplusPaid + state.quoteInventory := by
  exact ⟨rfl, rfl⟩

theorem materialize_has_one_exact_direction
    (candidate : Candidate) (state : SettlementState) :
    match candidate.completeSetMove with
    | .none => (materializePost candidate state).claimInventory = state.claimInventory
    | .mint quantity =>
        (materializePost candidate state).claimInventory =
          state.claimInventory.map (fun value => value + quantity)
    | .merge quantity =>
        (materializePost candidate state).claimInventory =
          state.claimInventory.map (fun value => value - quantity) := by
  generalize moveEq : candidate.completeSetMove = move
  cases move <;> simp [materializePost, moveEq]

end DClutch.General
