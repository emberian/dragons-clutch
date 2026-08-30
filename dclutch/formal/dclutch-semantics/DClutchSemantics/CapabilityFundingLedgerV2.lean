import DClutchSemantics.CapabilityManifestV1Abi
import Std.Tactic

/-!
# FundingLedgerV2 semantic projection

This is the compact ledger's arithmetic model. The runtime adapter remains an
unverified boundary: it must authenticate the finalized manifest, decode the
account, derive the canonical PDA, observe physical custody, and apply exactly
the transition modeled here.

The mutable wire stores only seven remaining amounts per selected manifest slot.
Asset classes and quotes live in the immutable manifest. Released principal is
therefore derived, pointwise, as `quote - remaining`; it is never a second
persisted fact. Each physical ledger selects a nonempty, controller-homogeneous
subset. Its rows map selected manifest indices in ascending order; founding
requires all presented subsets to be pairwise disjoint and cover the exact
required union.
-/

namespace DClutch.CapabilityFundingLedgerV2

/-- Seven compartments in the manifest's canonical order. -/
abbrev Amounts := Fin 7 → Nat

inductive Status where
  | pending
  | active
  | closed
  deriving DecidableEq, Repr

structure Slot where
  status : Status
  activationSlot : Nat
  remaining : Amounts

structure Quote where
  amounts : Amounts

/-- The only released-principal projection admitted by V2. -/
def released (quote : Quote) (slot : Slot) : Amounts :=
  fun compartment => quote.amounts compartment - slot.remaining compartment

/-- Pointwise conservation. Aggregate equality alone is deliberately weaker. -/
def Conserved (quote : Quote) (slot : Slot) : Prop :=
  ∀ compartment, slot.remaining compartment ≤ quote.amounts compartment

theorem remaining_add_released_eq_quote
    {quote : Quote} {slot : Slot} (valid : Conserved quote slot)
    (compartment : Fin 7) :
    slot.remaining compartment + released quote slot compartment =
      quote.amounts compartment := by
  exact Nat.add_sub_of_le (valid compartment)

/-- The redundant V1 representation, used only to state compact equivalence. -/
structure LegacySlot where
  remaining : Amounts
  released : Amounts

def LegacyConserved (quote : Quote) (slot : LegacySlot) : Prop :=
  ∀ compartment,
    slot.remaining compartment + slot.released compartment = quote.amounts compartment

/-- Compacting a conserved legacy slot loses no released-principal meaning. -/
theorem compact_projection_recovers_legacy_released
    {quote : Quote} {legacy : LegacySlot}
    (valid : LegacyConserved quote legacy) (compartment : Fin 7) :
    released quote {
      status := .active
      activationSlot := 0
      remaining := legacy.remaining
    } compartment = legacy.released compartment := by
  unfold LegacyConserved at valid
  unfold released
  change quote.amounts compartment - legacy.remaining compartment =
    legacy.released compartment
  have := valid compartment
  have remainingLe : legacy.remaining compartment ≤ quote.amounts compartment := by
    omega
  omega

/-- Rent and Creation are coordinates zero and one. -/
def activationRemaining (remaining : Amounts) : Amounts :=
  fun compartment => if compartment.val < 2 then 0 else remaining compartment

def activate (slot : Slot) (currentSlot : Nat) : Option Slot :=
  if slot.status = .pending then
    some {
      status := .active
      activationSlot := currentSlot
      remaining := activationRemaining slot.remaining
    }
  else
    none

theorem activation_replay_refuses
    {slot activated : Slot} {currentSlot : Nat}
    (result : activate slot currentSlot = some activated) :
    activate activated (currentSlot + 1) = none := by
  unfold activate at result ⊢
  split at result
  · cases result
    simp
  · simp_all

theorem activation_preserves_noncreation_compartments
    {slot activated : Slot} {currentSlot : Nat}
    (result : activate slot currentSlot = some activated)
    (compartment : Fin 7) (notCreation : 2 ≤ compartment.val) :
    activated.remaining compartment = slot.remaining compartment := by
  unfold activate at result
  split at result
  · cases result
    simp [activationRemaining, Nat.not_lt.mpr notCreation]
  · simp_all

theorem activation_zeros_rent_and_creation
    {slot activated : Slot} {currentSlot : Nat}
    (result : activate slot currentSlot = some activated)
    (compartment : Fin 7) (creation : compartment.val < 2) :
    activated.remaining compartment = 0 := by
  unfold activate at result
  split at result
  · cases result
    simp [activationRemaining, creation]
  · simp_all

/-- One physical subset ledger. `rowToEntry` is the canonical ascending map
from physical rows to selected manifest indices. -/
structure Ledger (manifestCount rowCount : Nat) where
  manifestId : Nat
  selected : Fin manifestCount → Bool
  rowToEntry : Fin rowCount → Fin manifestCount
  rows : Fin rowCount → Slot
  rowSelected : ∀ row, selected (rowToEntry row) = true
  rowInjective : Function.Injective rowToEntry
  rowAscending : ∀ left right, left.val < right.val →
    (rowToEntry left).val < (rowToEntry right).val

/-! Replace exactly one authenticated physical row. -/
def replaceSlot {manifestCount rowCount : Nat}
    (ledger : Ledger manifestCount rowCount)
    (selected : Fin rowCount) (next : Slot) :
    Ledger manifestCount rowCount := {
  ledger with
  rows := fun index => if index = selected then next else ledger.rows index
}

theorem replace_slot_updates_selected_only
    {manifestCount rowCount : Nat} (ledger : Ledger manifestCount rowCount)
    (selected : Fin rowCount) (next : Slot) :
    (replaceSlot ledger selected next).rows selected = next := by
  simp [replaceSlot]

theorem replace_slot_preserves_every_unselected_slot
    {manifestCount rowCount : Nat} (ledger : Ledger manifestCount rowCount)
    (selected index : Fin rowCount) (next : Slot)
    (unselected : index ≠ selected) :
    (replaceSlot ledger selected next).rows index = ledger.rows index := by
  simp [replaceSlot, unselected]

/-- Two physical ledgers cannot own the same manifest entry. -/
def Disjoint {manifestCount leftCount rightCount : Nat}
    (left : Ledger manifestCount leftCount)
    (right : Ledger manifestCount rightCount) : Prop :=
  ∀ entry, left.selected entry = true → right.selected entry = false

theorem disjoint_subsets_have_no_shared_entry
    {manifestCount leftCount rightCount : Nat}
    {left : Ledger manifestCount leftCount}
    {right : Ledger manifestCount rightCount}
    (disjoint : Disjoint left right) (entry : Fin manifestCount) :
    ¬(left.selected entry = true ∧ right.selected entry = true) := by
  intro shared
  have rightFalse := disjoint entry shared.1
  rw [shared.2] at rightFalse
  contradiction

/-- Closing is a logical tombstone. Physical ledger rent remains indivisible
until every selected row is closed. -/
def close (slot : Slot) : Option Slot :=
  if slot.status = .active then
    some {
      status := .closed
      activationSlot := slot.activationSlot
      remaining := fun _ => 0
    }
  else
    none

/-- Shared-ledger surplus is physical lamports above exact Rent plus all
remaining native principal. It is neither principal nor protocol revenue. -/
def nativeSurplus (observed exactRent aggregateRemaining : Nat) : Nat :=
  observed - exactRent - aggregateRemaining

/-- The final logical close returns the last row's principal, one physical Rent
deposit, and the separately classified surplus to the immutable RentCredit. -/
def finalNativeRefund (lastRemaining exactRent surplus : Nat) : Nat :=
  lastRemaining + exactRent + surplus

theorem final_native_close_classifies_every_observed_lamport
    {observed exactRent lastRemaining : Nat}
    (funded : exactRent + lastRemaining ≤ observed) :
    finalNativeRefund lastRemaining exactRent
      (nativeSurplus observed exactRent lastRemaining) = observed := by
  unfold finalNativeRefund nativeSurplus
  omega

/-- A nonfinal row close refunds only that row's native principal; the one Rent
deposit and every surplus lamport remain in the physical subset ledger. -/
theorem nonfinal_close_preserves_rent_and_surplus
    {observed exactRent aggregateRemaining closingRemaining : Nat}
    (principal : closingRemaining ≤ aggregateRemaining)
    (funded : exactRent + aggregateRemaining ≤ observed) :
    nativeSurplus (observed - closingRemaining) exactRent
        (aggregateRemaining - closingRemaining) =
      nativeSurplus observed exactRent aggregateRemaining := by
  unfold nativeSurplus
  omega

def AllClosed {manifestCount rowCount : Nat}
    (ledger : Ledger manifestCount rowCount) : Prop :=
  ∀ index, (ledger.rows index).status = .closed

theorem closing_one_slot_does_not_close_an_unselected_slot
    {manifestCount rowCount : Nat} (ledger : Ledger manifestCount rowCount)
    (selected index : Fin rowCount) (closed : Slot)
    (_result : close (ledger.rows selected) = some closed)
    (unselected : index ≠ selected) :
    ((replaceSlot ledger selected closed).rows index).status =
      (ledger.rows index).status := by
  rw [replace_slot_preserves_every_unselected_slot ledger selected index closed unselected]

end DClutch.CapabilityFundingLedgerV2
