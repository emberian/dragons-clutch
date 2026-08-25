import Std.Tactic

/-!
# Dealer V2 finite-scenario collateral

This is the smallest semantic slice needed to generalize a covered quote-bin
Dealer without adding signed native Positions or a second liability ledger.
Every coordinate names one canonical Product outcome. `inventory` and
`acquired` are real native Claims held by the Dealer; `delivered` is the
nonnegative native Claims vector sent to the counterparty in the same atomic
transaction.

If one or more coordinates are short, the Dealer deposits present collateral
into the Market Hoard and Claims mints that many equal complete sets into the
Dealer Position. The required deposit is the maximum terminal shortfall. Any
equal residual complete sets may be merged only after all nonnegative transfers,
releasing exactly the same Hoard principal. Claims supply and Hoard remain the
sole liability and collateral truth.
-/

namespace DClutch.DealerScenarioCollateral

/-- One exact terminal scenario coordinate. -/
structure Scenario where
  inventory : Nat
  acquired : Nat
  delivered : Nat
  deriving DecidableEq, Repr

/-- Ephemeral projection of the one canonical Dealer Claims Position. -/
structure ClaimsPositionObservation where
  marketId : Nat
  dealerId : Nat
  revision : Nat
  inventory : List Nat
  deriving DecidableEq, Repr

/-- Exact immutable and optimistic coordinates supplied by Dealer policy/request. -/
structure PositionExpectation where
  marketId : Nat
  dealerId : Nat
  revision : Nat
  deriving DecidableEq, Repr

/-- Refuse stale or transplanted Claims inventory before pricing or effects. -/
def positionAdmissible
    (expected : PositionExpectation) (observed : ClaimsPositionObservation) : Bool :=
  expected.marketId != 0 && expected.dealerId != 0 &&
    observed.marketId = expected.marketId && observed.dealerId = expected.dealerId &&
    observed.revision = expected.revision && observed.inventory != []

theorem admitted_position_is_exactly_bound
    (expected : PositionExpectation) (observed : ClaimsPositionObservation)
  (admitted : positionAdmissible expected observed = true) :
    observed.marketId = expected.marketId ∧
      observed.dealerId = expected.dealerId ∧
      observed.revision = expected.revision := by
  unfold positionAdmissible at admitted
  have fifth := Bool.and_eq_true_iff.mp admitted
  have fourth := Bool.and_eq_true_iff.mp fifth.1
  have third := Bool.and_eq_true_iff.mp fourth.1
  have second := Bool.and_eq_true_iff.mp third.1
  exact ⟨of_decide_eq_true second.2, of_decide_eq_true third.2,
    of_decide_eq_true fourth.2⟩

/-- Missing claims in one scenario before complete-set minting. -/
def shortfall (scenario : Scenario) : Nat :=
  scenario.delivered - (scenario.inventory + scenario.acquired)

/-- Present collateral required to mint enough equal complete sets. -/
def reserveRequired : List Scenario → Nat
  | [] => 0
  | scenario :: tail => max (shortfall scenario) (reserveRequired tail)

/-- Gross complete-set reserve if same-transaction incoming claims were ignored. -/
def grossDeliveryReserve : List Scenario → Nat
  | [] => 0
  | scenario :: tail => max scenario.delivered (grossDeliveryReserve tail)

/-- Dealer claims remaining after acquisition, mint, and delivery. -/
def fundedInventory (scenario : Scenario) (reserve : Nat) : Nat :=
  scenario.inventory + scenario.acquired + reserve - scenario.delivered

/-- Claims remaining after merging an equal complete-set quantity. -/
def postInventory (scenario : Scenario) (reserve release : Nat) : Nat :=
  fundedInventory scenario reserve - release

/-- Every scenario can release the same complete-set quantity. -/
def releaseAdmissible (book : List Scenario) (reserve release : Nat) : Bool :=
  book.all fun scenario => release ≤ fundedInventory scenario reserve

/-- The reserve is funded by present Dealer capital, never expected revenue. -/
def fundingAdmissible (book : List Scenario) (presentFunding : Nat) : Bool :=
  reserveRequired book ≤ presentFunding

theorem shortfall_le_reserve_of_mem
    (scenario : Scenario) (book : List Scenario) (member : scenario ∈ book) :
    shortfall scenario ≤ reserveRequired book := by
  induction book with
  | nil => simp at member
  | cons head tail inductionHypothesis =>
      simp only [reserveRequired]
      rcases List.mem_cons.mp member with rfl | inTail
      · exact Nat.le_max_left _ _
      · exact Nat.le_trans (inductionHypothesis inTail) (Nat.le_max_right _ _)

/-- Maximum terminal shortfall is sufficient at every Product outcome. -/
theorem reserve_covers_every_scenario
    (scenario : Scenario) (book : List Scenario) (member : scenario ∈ book) :
    scenario.delivered ≤
      scenario.inventory + scenario.acquired + reserveRequired book := by
  have covered := shortfall_le_reserve_of_mem scenario book member
  unfold shortfall at covered
  omega

/-- Minting complete sets and delivering claims preserves each coordinate. -/
theorem funded_inventory_conserves
    (scenario : Scenario) (book : List Scenario) (member : scenario ∈ book) :
    fundedInventory scenario (reserveRequired book) + scenario.delivered =
      scenario.inventory + scenario.acquired + reserveRequired book := by
  have covered := reserve_covers_every_scenario scenario book member
  unfold fundedInventory
  omega

/-- Same-transaction portfolio netting never needs more than gross delivery. -/
theorem scenario_netting_never_requires_more_than_gross_delivery
    (book : List Scenario) :
    reserveRequired book ≤ grossDeliveryReserve book := by
  induction book with
  | nil => simp [reserveRequired, grossDeliveryReserve]
  | cons scenario tail inductionHypothesis =>
      simp only [reserveRequired, grossDeliveryReserve]
      exact Nat.max_le.mpr ⟨
        Nat.le_trans (Nat.sub_le _ _) (Nat.le_max_left _ _),
        Nat.le_trans inductionHypothesis (Nat.le_max_right _ _)⟩

/-- Equal complete-set merge preserves the same coordinate conservation law. -/
theorem merge_preserves_conservation
    (scenario : Scenario) (book : List Scenario) (member : scenario ∈ book)
    (release : Nat)
    (admitted : release ≤ fundedInventory scenario (reserveRequired book)) :
    postInventory scenario (reserveRequired book) release +
        scenario.delivered + release =
      scenario.inventory + scenario.acquired + reserveRequired book := by
  have conserved := funded_inventory_conserves scenario book member
  unfold postInventory
  omega

def nettedExample : List Scenario := [
  { inventory := 2, acquired := 3, delivered := 10 },
  { inventory := 10, acquired := 0, delivered := 1 },
  { inventory := 0, acquired := 4, delivered := 6 }
]

/-- Incoming portfolio legs cut required new collateral from ten to five. -/
theorem netted_example_uses_exact_maximum_shortfall :
    reserveRequired nettedExample = 5 ∧
      grossDeliveryReserve nettedExample = 10 ∧
      fundingAdmissible nettedExample 5 = true := by
  native_decide

/-- Future fees or other unfunded expectations cannot satisfy admission. -/
theorem hostile_underfunded_reserve_refuses :
    fundingAdmissible nettedExample 4 = false := by
  native_decide

def mergeExample : List Scenario := [
  { inventory := 9, acquired := 3, delivered := 2 },
  { inventory := 8, acquired := 4, delivered := 1 },
  { inventory := 10, acquired := 0, delivered := 0 }
]

/-- Only an equal residual complete set may release Hoard principal. -/
theorem hostile_overmerge_refuses_but_exact_merge_is_admitted :
    reserveRequired mergeExample = 0 ∧
      releaseAdmissible mergeExample 0 10 = true ∧
      releaseAdmissible mergeExample 0 11 = false := by
  native_decide

def expectedPosition : PositionExpectation := {
  marketId := 41
  dealerId := 42
  revision := 7
}

def observedPosition : ClaimsPositionObservation := {
  marketId := 41
  dealerId := 42
  revision := 7
  inventory := [2, 10, 0]
}

/-- Market, holder, revision, and nonempty width are all non-substitutable. -/
theorem hostile_position_substitution_and_staleness_refuse :
    positionAdmissible expectedPosition observedPosition = true ∧
      positionAdmissible expectedPosition { observedPosition with marketId := 99 } = false ∧
      positionAdmissible expectedPosition { observedPosition with dealerId := 99 } = false ∧
      positionAdmissible expectedPosition { observedPosition with revision := 8 } = false ∧
      positionAdmissible expectedPosition { observedPosition with inventory := [] } = false := by
  native_decide

end DClutch.DealerScenarioCollateral
