import Std.Tactic

/-!
# Dealer terminal-scenario solvency and complete-set netting

This module owns the pure integer model for the runtime-width Dealer scenario
kernel. Capital contains only present eligible collateral. There is deliberately
no anticipated-fee, future-order-flow, or liquidation-proceeds coordinate.

Canonical Claims inventory and terminal obligations are borrowed projections;
neither becomes a second Dealer ledger. A complete-set split subtracts one
collateral atom and adds one claim in every scenario. A merge performs the
inverse. The least split needed for delivery and any bounded equal merge
therefore preserve exact terminal assets scenario by scenario.
-/

namespace DClutch.DealerScenarioSolvency

/-- Present authenticated capital. Unfunded expectations are unrepresentable. -/
structure CapitalObservation where
  eligibleCollateral : Nat
  deriving DecidableEq, Repr

/-- One terminal-scenario coordinate around an atomic Claims basket transfer. -/
structure Scenario where
  inventory : Nat
  acquired : Nat
  delivered : Nat
  obligationsBefore : Nat
  obligationsAfter : Nat
  deriving DecidableEq, Repr

/-- Exact signed terminal equity. -/
def terminalEquity (capital inventory obligation : Nat) : Int :=
  (capital : Int) + (inventory : Int) - (obligation : Int)

/-- Scenario equity meets the immutable locked capital floor. -/
def floorAdmissible (floor capital inventory obligation : Nat) : Prop :=
  (floor : Int) ≤ terminalEquity capital inventory obligation

/-- Claims missing from one coordinate before an equal complete-set split. -/
def shortfall (scenario : Scenario) : Nat :=
  scenario.delivered - (scenario.inventory + scenario.acquired)

/-- Least equal complete-set split covering every terminal delivery. -/
def minimumSplit : List Scenario → Nat
  | [] => 0
  | scenario :: tail => max (shortfall scenario) (minimumSplit tail)

/-- Inventory after acquisition, the equal split, and delivery. -/
def fundedInventory (scenario : Scenario) (split : Nat) : Nat :=
  scenario.inventory + scenario.acquired + split - scenario.delivered

/-- Inventory after an admitted equal residual complete-set merge. -/
def netInventory (scenario : Scenario) (split merge : Nat) : Nat :=
  fundedInventory scenario split - merge

theorem shortfall_le_minimumSplit_of_mem
    (scenario : Scenario) (book : List Scenario) (member : scenario ∈ book) :
    shortfall scenario ≤ minimumSplit book := by
  induction book with
  | nil => simp at member
  | cons head tail inductionHypothesis =>
      simp only [minimumSplit]
      rcases List.mem_cons.mp member with rfl | inTail
      · exact Nat.le_max_left _ _
      · exact Nat.le_trans (inductionHypothesis inTail) (Nat.le_max_right _ _)

/-- The derived split makes every nonnegative delivery executable. -/
theorem minimumSplit_covers_every_scenario
    (scenario : Scenario) (book : List Scenario) (member : scenario ∈ book) :
    scenario.delivered ≤
      scenario.inventory + scenario.acquired + minimumSplit book := by
  have covered := shortfall_le_minimumSplit_of_mem scenario book member
  unfold shortfall at covered
  omega

/-- Any equal split covering the complete book is at least the derived split. -/
theorem minimumSplit_is_least
    (book : List Scenario) (split : Nat)
    (covers : ∀ scenario ∈ book,
      scenario.delivered ≤ scenario.inventory + scenario.acquired + split) :
    minimumSplit book ≤ split := by
  induction book with
  | nil => simp [minimumSplit]
  | cons head tail inductionHypothesis =>
      simp only [minimumSplit]
      apply Nat.max_le.mpr
      constructor
      · have headCovered := covers head (by simp)
        unfold shortfall
        omega
      · apply inductionHypothesis
        intro scenario member
        exact covers scenario (by simp [member])

/-- Split, delivery, and a bounded equal merge preserve terminal assets. -/
theorem split_merge_preserves_terminal_assets
    (scenario : Scenario) (capital split merge : Nat)
    (splitFunded : split ≤ capital)
    (deliveryCovered :
      scenario.delivered ≤ scenario.inventory + scenario.acquired + split)
    (mergeBound : merge ≤ fundedInventory scenario split) :
    (capital - split + merge) + netInventory scenario split merge =
      capital + scenario.inventory + scenario.acquired - scenario.delivered := by
  simp only [netInventory, fundedInventory]
  simp only [fundedInventory] at mergeBound
  omega

/-- Netting preserves exact signed candidate equity for fixed obligations. -/
theorem split_merge_preserves_terminal_equity
    (scenario : Scenario) (capital split merge : Nat)
    (splitFunded : split ≤ capital)
    (deliveryCovered :
      scenario.delivered ≤ scenario.inventory + scenario.acquired + split)
    (mergeBound : merge ≤ fundedInventory scenario split) :
    terminalEquity
        (capital - split + merge)
        (netInventory scenario split merge)
        scenario.obligationsAfter =
      ((capital + scenario.inventory + scenario.acquired - scenario.delivered : Nat) : Int) -
        (scenario.obligationsAfter : Int) := by
  have assets := split_merge_preserves_terminal_assets scenario capital split merge
    splitFunded deliveryCovered mergeBound
  unfold terminalEquity
  omega

def nettedBook : List Scenario := [
  { inventory := 2, acquired := 3, delivered := 10,
    obligationsBefore := 2, obligationsAfter := 0 },
  { inventory := 10, acquired := 0, delivered := 1,
    obligationsBefore := 10, obligationsAfter := 9 },
  { inventory := 0, acquired := 4, delivered := 6,
    obligationsBefore := 0, obligationsAfter := 3 }
]

/-- Incoming Claims reduce the exact new split from ten to five. -/
theorem netted_book_derives_least_five_atom_split :
    minimumSplit nettedBook = 5 ∧
      (∀ scenario ∈ nettedBook,
        scenario.delivered ≤ scenario.inventory + scenario.acquired + 5) := by
  native_decide

/-- Expected revenue has no semantic coordinate capable of repairing equity. -/
theorem present_zero_capital_remains_below_zero_floor_with_one_obligation :
    ¬ floorAdmissible 0 0 0 1 := by
  simp [floorAdmissible, terminalEquity]

end DClutch.DealerScenarioSolvency
