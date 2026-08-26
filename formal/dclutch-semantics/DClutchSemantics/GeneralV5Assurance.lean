import DClutchSemantics.GeneralClearing
import Std.Tactic

/-!
# General V5 best-valid-submitted and settlement assurance

This module proves the honest claims exposed by the current General semantic
kernel.  Selection is only over valid candidates actually submitted before
freeze; there is no global search and no optimal-clearing claim.  Settlement
has one complete collection pass, one complete-set materialization boundary,
and one distribution pass.  All quantities are exact scaled natural numbers.

Candidate/account bytes, signatures, fixed-width refinement, CPI effects, and
transaction rollback remain the explicit `AdapterBoundary` obligations from
`GeneralClearing`.  This file defines no executable or persisted wire format.
-/

namespace DClutch.General.V5Assurance

open DClutch.General

/-! ## Every selectable candidate passed the complete semantic verifier -/

def ExactCandidateChecks (candidate : Candidate) : Prop :=
  candidate.prices.validFor candidate.outcomeCount = true ∧
  candidate.identitiesCanonical = true ∧
  candidate.executions.all (executionValid candidate) = true ∧
  candidate.quotesCanonical = true ∧
  candidate.claimsBalance = true ∧
  candidate.quoteBalances = true

theorem candidate_valid_implies_exact_checks
    (candidate : Candidate) (valid : candidate.valid = true) :
    ExactCandidateChecks candidate := by
  simp only [Candidate.valid, Bool.and_eq_true] at valid
  rcases valid with
    ⟨⟨⟨⟨⟨⟨⟨⟨⟨⟨candidateId, productId⟩, batchId⟩,
      prices⟩, pages⟩, pageRows⟩, identities⟩, executions⟩,
      quotes⟩, claims⟩, quoteBalance⟩
  exact ⟨prices, identities, executions, quotes, claims, quoteBalance⟩

theorem selected_candidate_was_exactly_checked
    (selection : Selection) (candidate : Candidate)
    (selectionValid : selection.valid = true)
    (selected : selection.best = some candidate) :
    ExactCandidateChecks candidate := by
  simp only [Selection.valid, Bool.and_eq_true, selected] at selectionValid
  exact candidate_valid_implies_exact_checks candidate selectionValid.2

theorem kernel_freeze_contains_checked_submitted_candidate
    (selection : Selection) (candidate : Candidate)
    (selectionValid : selection.valid = true)
    (selected : selection.best = some candidate) :
    (selection.freeze).closed = true ∧
      (selection.freeze).best = some candidate ∧
      ExactCandidateChecks candidate := by
  exact ⟨rfl, selected,
    selected_candidate_was_exactly_checked selection candidate selectionValid selected⟩

theorem execute_freeze_expands (selection : Selection) :
    selection.execute? .freeze =
      if _stateValid : selection.valid = true then
        if selection.closed then .error .selectionClosed
        else if selection.best.isSome then .ok selection.freeze
        else .error .noValidSubmission
      else .error .invalidState := by
  rfl

theorem successful_execute_freeze_contains_checked_submitted_candidate
    (selection post : Selection)
    (success : selection.execute? .freeze = .ok post) :
    post.closed = true ∧
      ∃ candidate, post.best = some candidate ∧ ExactCandidateChecks candidate := by
  rw [execute_freeze_expands] at success
  split at success
  next selectionValid =>
    split at success
    next selectionClosed => simp at success
    next selectionOpen =>
      split at success
      next hasBest =>
        have postExact : post = selection.freeze := by simpa using success.symm
        subst post
        cases selected : selection.best with
        | none => simp [selected] at hasBest
        | some candidate =>
            refine ⟨rfl, candidate, selected, ?_⟩
            exact selected_candidate_was_exactly_checked
              selection candidate selectionValid selected
      next noBest => simp at success
  next selectionInvalid => simp at success

/-! ## Deterministic replacement and tie handling -/

/-- The production V5 policy order used by the semantic witness.  Capability
data may select another admitted list, but every admitted policy still ends in
candidate identity. -/
def canonicalCriteria : List SelectionCriterion :=
  [.maximizeFilledLots, .minimizeQuoteSurplus, .minimizeCandidateId]

def canonicalBetter (left right : Objective) : Bool :=
  betterBy canonicalCriteria left right

theorem exact_objective_tie_never_replaces
    (criteria : List SelectionCriterion) (left right : Objective)
    (tie : left = right) :
    betterBy criteria left right = false := by
  subst right
  induction criteria with
  | nil => rfl
  | cons criterion rest induction =>
      cases criterion <;> simp [betterBy, induction]

theorem exact_objective_tie_keeps_the_submitted_incumbent
    (selection : Selection) (incumbent candidate : Candidate)
    (openSelection : selection.closed = false)
    (incumbentSelected : selection.best = some incumbent)
    (candidateValid : candidate.valid = true)
    (tie : candidate.objective = incumbent.objective) :
    selection.consider candidate = selection := by
  simp [Selection.consider, openSelection, candidateValid, incumbentSelected,
    SelectionPolicy.better,
    exact_objective_tie_never_replaces selection.policy.criteria
      candidate.objective incumbent.objective tie]

theorem canonical_replacement_is_asymmetric
    (left right : Objective)
    (better : canonicalBetter left right = true) :
    canonicalBetter right left = false := by
  unfold canonicalBetter canonicalCriteria at better ⊢
  simp only [betterBy] at better ⊢
  by_cases filledTie : left.filledLots = right.filledLots
  · rw [if_pos filledTie] at better
    rw [if_pos filledTie.symm]
    by_cases surplusTie : left.quoteSurplus = right.quoteSurplus
    · rw [if_pos surplusTie] at better
      rw [if_pos surplusTie.symm]
      by_cases idTie : left.candidateId = right.candidateId
      · rw [if_pos idTie] at better
        simp at better
      · rw [if_neg idTie] at better
        rw [if_neg (Ne.symm idTie)]
        have idLess : left.candidateId < right.candidateId := by simpa using better
        simp
        omega
    · rw [if_neg surplusTie] at better
      rw [if_neg (Ne.symm surplusTie)]
      have surplusLess : left.quoteSurplus < right.quoteSurplus := by simpa using better
      simp
      omega
  · rw [if_neg filledTie] at better
    rw [if_neg (Ne.symm filledTie)]
    have filledGreater : right.filledLots < left.filledLots := by simpa using better
    simp
    omega

theorem canonical_distinct_objectives_have_one_direction
    (left right : Objective) (distinct : left ≠ right) :
    canonicalBetter left right = true ∨ canonicalBetter right left = true := by
  unfold canonicalBetter canonicalCriteria
  simp only [betterBy]
  by_cases filledTie : left.filledLots = right.filledLots
  · rw [if_pos filledTie, if_pos filledTie.symm]
    by_cases surplusTie : left.quoteSurplus = right.quoteSurplus
    · rw [if_pos surplusTie, if_pos surplusTie.symm]
      have idDiff : left.candidateId ≠ right.candidateId := by
        intro idTie
        apply distinct
        cases left
        cases right
        simp_all
      rw [if_neg idDiff, if_neg (Ne.symm idDiff)]
      simp
      omega
    · rw [if_neg surplusTie, if_neg (Ne.symm surplusTie)]
      simp
      omega
  · rw [if_neg filledTie, if_neg (Ne.symm filledTie)]
    simp
    omega

/-! ## Exact aggregate Collect → Materialize → Distribute accounting -/

/-- Claims held after every page has been collected and the one complete-set
movement has executed. -/
def materializedClaimInventory (candidate : Candidate) : List Nat :=
  match candidate.completeSetMove with
  | .none => claimInputs candidate
  | .mint quantity => (claimInputs candidate).map (fun value => value + quantity)
  | .merge quantity => (claimInputs candidate).map (fun value => value - quantity)

/-- Quote held after the one complete-set movement.  Invalid candidates map to
zero only to keep this projection total; candidate admission proves `some`. -/
def materializedQuoteInventory (candidate : Candidate) : Nat :=
  candidate.quoteAfterMaterialization.getD 0

def distributedClaimResidual (candidate : Candidate) : List Nat :=
  subVectors (materializedClaimInventory candidate) (claimOutputs candidate)

def distributedQuoteResidual (candidate : Candidate) : Nat :=
  materializedQuoteInventory candidate - quoteOutputs candidate

theorem all_indices_true
    (count : Nat) (predicate : Nat → Bool)
    (accepted : allIndices count predicate = true)
    (index : Nat) (inBounds : index < count) :
    predicate index = true := by
  unfold allIndices at accepted
  have membership : index ∈ List.range count := List.mem_range.mpr inBounds
  exact List.all_eq_true.mp accepted index membership

theorem claims_balance_materializes_exact_declared_outputs
    (candidate : Candidate) (balanced : candidate.claimsBalance = true) :
    materializedClaimInventory candidate = claimOutputs candidate := by
  generalize moveEq : candidate.completeSetMove = move
  cases move with
  | none =>
      simp [Candidate.claimsBalance, moveEq] at balanced
      simpa [materializedClaimInventory, moveEq] using balanced
  | mint quantity =>
      simp only [Candidate.claimsBalance, moveEq, Bool.and_eq_true] at balanced
      rcases balanced with ⟨positive, coordinates⟩
      simp only [materializedClaimInventory, moveEq, claimInputs, claimOutputs,
        List.map_map]
      apply List.map_congr_left
      intro outcome membership
      have inBounds : outcome < candidate.outcomeCount := List.mem_range.mp membership
      have exactCoordinate := all_indices_true candidate.outcomeCount
        (fun index =>
          valueAt (claimOutputs candidate) index =
            valueAt (claimInputs candidate) index + quantity)
        coordinates outcome inBounds
      have exactCoordinateProp :
          valueAt (claimOutputs candidate) outcome =
            valueAt (claimInputs candidate) outcome + quantity := by
        simpa only [decide_eq_true_eq] using exactCoordinate
      simpa [Function.comp_apply, claimInputs, claimOutputs, valueAt, inBounds]
        using exactCoordinateProp.symm
  | merge quantity =>
      simp only [Candidate.claimsBalance, moveEq, Bool.and_eq_true] at balanced
      rcases balanced with ⟨positive, coordinates⟩
      simp only [materializedClaimInventory, moveEq, claimInputs, claimOutputs,
        List.map_map]
      apply List.map_congr_left
      intro outcome membership
      have inBounds : outcome < candidate.outcomeCount := List.mem_range.mp membership
      have exactCoordinate := all_indices_true candidate.outcomeCount
        (fun index =>
          valueAt (claimInputs candidate) index =
            valueAt (claimOutputs candidate) index + quantity)
        coordinates outcome inBounds
      simp [claimInputs, claimOutputs, valueAt, inBounds] at exactCoordinate ⊢
      omega

theorem sub_vectors_self_are_zero (values : List Nat) :
    allZero (subVectors values values) = true := by
  simp [subVectors, allZero]

theorem valid_candidate_distribution_has_zero_claim_residual
    (candidate : Candidate) (valid : candidate.valid = true) :
    allZero (distributedClaimResidual candidate) = true := by
  have checks := candidate_valid_implies_exact_checks candidate valid
  rw [distributedClaimResidual,
    claims_balance_materializes_exact_declared_outputs candidate checks.2.2.2.2.1]
  exact sub_vectors_self_are_zero (claimOutputs candidate)

theorem quote_balance_exposes_exact_materialized_inventory
    (candidate : Candidate) (balanced : candidate.quoteBalances = true) :
    ∃ available,
      candidate.quoteAfterMaterialization = some available ∧
      quoteOutputs candidate ≤ available := by
  unfold Candidate.quoteBalances at balanced
  cases availableEq : candidate.quoteAfterMaterialization with
  | none => simp [availableEq] at balanced
  | some available =>
      refine ⟨available, rfl, ?_⟩
      simp only [availableEq, decide_eq_true_eq] at balanced
      exact balanced

theorem valid_candidate_distribution_conserves_quote_exactly
    (candidate : Candidate) (valid : candidate.valid = true) :
    distributedQuoteResidual candidate + quoteOutputs candidate =
      materializedQuoteInventory candidate := by
  have checks := candidate_valid_implies_exact_checks candidate valid
  obtain ⟨available, availableEq, outputBound⟩ :=
    quote_balance_exposes_exact_materialized_inventory candidate checks.2.2.2.2.2
  simp [distributedQuoteResidual, materializedQuoteInventory, availableEq]
  omega

theorem collect_post_is_exact_scaled_addition
    (candidate : Candidate) (pre : SettlementState) (page : Page) :
    (collectPost candidate pre page).claimInventory =
        addVectors pre.claimInventory (page.claimInputs candidate) ∧
      (collectPost candidate pre page).quoteInventory =
        pre.quoteInventory + page.quoteInputs := by
  exact ⟨rfl, rfl⟩

theorem materialize_post_is_exact_scaled_movement
    (candidate : Candidate) (pre : SettlementState) :
    match candidate.completeSetMove with
    | .none =>
        (materializePost candidate pre).claimInventory = pre.claimInventory ∧
        (materializePost candidate pre).quoteInventory = pre.quoteInventory
    | .mint quantity =>
        (materializePost candidate pre).claimInventory =
            pre.claimInventory.map (fun value => value + quantity) ∧
        (materializePost candidate pre).quoteInventory = pre.quoteInventory - quantity
    | .merge quantity =>
        (materializePost candidate pre).claimInventory =
            pre.claimInventory.map (fun value => value - quantity) ∧
        (materializePost candidate pre).quoteInventory = pre.quoteInventory + quantity := by
  generalize moveEq : candidate.completeSetMove = move
  cases move <;> simp [materializePost, moveEq]

theorem distribute_post_is_exact_scaled_subtraction
    (candidate : Candidate) (pre : SettlementState) (page : Page) :
    (distributePost candidate pre page).claimInventory =
        subVectors pre.claimInventory (page.claimOutputs candidate) ∧
      (distributePost candidate pre page).quoteInventory =
        pre.quoteInventory - page.quoteOutputs := by
  exact ⟨rfl, rfl⟩

/-! ## Phase ordering and terminal closure -/

theorem incomplete_collection_cannot_materialize
    (candidate : Candidate) (state : SettlementState) (nextPage : Nat)
    (phase : state.phase = .collecting nextPage) :
    state.commandAccepts candidate .materialize = false := by
  simp [SettlementState.commandAccepts, phase]

theorem distribution_cannot_precede_materialization
    (candidate : Candidate) (state : SettlementState) (page : Page)
    (phase : (∃ nextPage, state.phase = .collecting nextPage) ∨
      state.phase = .materializing) :
    state.commandAccepts candidate (.distribute page) = false := by
  rcases phase with ⟨nextPage, phase⟩ | phase <;>
    simp [SettlementState.commandAccepts, phase]

theorem final_collection_enters_materialization
    (candidate : Candidate) (state : SettlementState) (page : Page) (nextPage : Nat)
    (phase : state.phase = .collecting nextPage)
    (last : nextPage + 1 = candidate.pages.length) :
    (collectPost candidate state page).phase = .materializing := by
  simp [collectPost, phase, last]

theorem materialization_enters_distribution
    (candidate : Candidate) (state : SettlementState) :
    (materializePost candidate state).phase = .distributing 0 := by
  generalize moveEq : candidate.completeSetMove = move
  cases move <;> simp [materializePost, moveEq]

theorem accepted_close_requires_zero_residual_claim_liability
    (candidate : Candidate) (state : SettlementState)
    (accepted : state.commandAccepts candidate .close = true) :
    state.phase = .readyToClose ∧ allZero state.claimInventory = true := by
  simpa [SettlementState.commandAccepts, Bool.and_eq_true] using accepted

theorem accepted_close_routes_surplus_and_leaves_zero_residuals
    (candidate : Candidate) (state : SettlementState)
    (accepted : state.commandAccepts candidate .close = true) :
    (closePost state).phase = .terminal ∧
      allZero (closePost state).claimInventory = true ∧
      (closePost state).quoteInventory = 0 ∧
      (closePost state).quoteSurplusPaid =
        state.quoteSurplusPaid + state.quoteInventory := by
  have pre := accepted_close_requires_zero_residual_claim_liability candidate state accepted
  exact ⟨rfl, pre.2, rfl, rfl⟩

end DClutch.General.V5Assurance
