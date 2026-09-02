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

/-! ## Runtime selection decision corpus

`crates/dclutch-general-adapter-contract` decides the best valid submitted
candidate in a COMPOSITION of three functions, and reading any one of them
alone gives a wrong verdict about this module:

* `runtime_verify::runtime_verified_balance_v2` derives the complete-set
  movement and the exact quote surplus from a certificate's claim banks and
  aggregate quote fields, which is `Candidate.completeSetMove` and
  `Candidate.quoteAfterMaterialization` here;
* `runtime_verify::runtime_candidate_key_better_v2` interprets the immutable
  policy over the resulting three-field key, which is `betterBy` here;
* `runtime_selection::consider_verified_candidate_v2` is the fold, and it is
  the only one of the three that decides anything -- replacement, and the
  keep-on-tie that `exact_objective_tie_keeps_the_submitted_incumbent` proves.
  It reads the incumbent's key from the PERSISTED CURSOR rather than
  recomputing it, so those bytes are a third author of the comparison.

Three facts about the runtime side this corpus declines to smooth over.

Rust refuses a gate EARLIER than this module in two places. Its balance
derivation refuses `ClaimImbalance` when any coordinate disagrees with the
movement read off coordinate zero, where `Candidate.completeSetMove` is total
and `Candidate.claimsBalance` is a separate conjunct; and it refuses
`QuoteImbalance` on a credit exceeding available quote, where
`Objective.quoteSurplus` truncates in `Nat`. Those two conjuncts are asserted
of every vector below, so the replay runs on the domain where the two agree
rather than on the domain where one of them is silently absorbing a defect.

Rust also refuses `DuplicateCandidate`, `Substitution` and `RevisionMismatch`,
which have no counterpart here at all: they are the optimistic-concurrency and
comparison-domain obligations of a permissionless account frame, not facts
about selection. Product, Batch, policy identity, price scale and outcome
width are held fixed across each vector so none of them can fire.

And candidate identity is a `Nat` here and a little-endian 32-byte content
identity there. The emitted bytes are that embedding; one vector separates two
identities in byte 31 alone, so a big-endian reading of the tie-break would
disagree here rather than nowhere.

What this does not bridge: candidate ADMISSION. A certificate is already
verified when selection reads it, and these certificates are deliberately not
`Candidate.valid` -- their quote fields are candidate-wide aggregates rather
than per-execution rounded quotes, so `quotesCanonical` fails on all of them.
The two admission conjuncts the runtime's balance derivation actually mirrors
are `claimsBalance` and `quoteBalances`, and those are the two asserted. -/

/-- One submitted certificate exactly as the runtime selection cursor reads
it: a content identity, one signed per-lot claim exchange scaled by the
candidate's filled lots, and the aggregate quote debit and credit. -/
structure SubmittedCertificate where
  candidateId : Nat
  lots : Nat
  deliverPerLot : List Nat
  receivePerLot : List Nat
  quoteDebit : Nat
  quoteCredit : Nat
  deriving Repr

/-- The `Candidate` a certificate stands for. Prices, order identity and page
geometry are held at benign values because `Candidate.objective` reads none of
them; every quantity the corpus emits is then computed by `GeneralClearing`'s
own definitions from this candidate rather than restated. -/
def certificateCandidate (certificate : SubmittedCertificate) : Candidate := {
  candidateId := certificate.candidateId
  productId := 1
  batchId := 1
  outcomeCount := certificate.deliverPerLot.length
  prices := { scale := 1, coordinates := [1] }
  pages := [{ executions := [{
    order := {
      orderId := 1
      ownerId := 1
      nonce := 0
      receivePerLot := certificate.receivePerLot
      deliverPerLot := certificate.deliverPerLot
      maxLots := certificate.lots
      maxQuoteDebitPerLot := certificate.quoteDebit }
    lots := certificate.lots
    quoteDebit := certificate.quoteDebit
    quoteCredit := certificate.quoteCredit }] }]
}

/-- Exactly what the runtime cursor persists about its incumbent: the
Candidate's one-based coordinate in its immutable Batch, and the objective the
policy interprets. -/
abbrev SubmissionKey := Nat × Objective

/-- One fold step of the runtime selection cursor. -/
def considerKey (policy : SelectionPolicy)
    (incumbent : Option SubmissionKey) (submitted : SubmissionKey) : Option SubmissionKey :=
  match incumbent with
  | none => some submitted
  | some current =>
      if betterBy policy.criteria submitted.2 current.2 then some submitted else some current

/-- The complete fold over one Batch's submissions, in submission order. -/
def selectBest (policy : SelectionPolicy) (submissions : List SubmissionKey) :
    Option SubmissionKey :=
  submissions.foldl (considerKey policy) none

/-- Certificates paired with their one-based Batch coordinates. -/
def certificateKeys : Nat → List SubmittedCertificate → List SubmissionKey
  | _, [] => []
  | coordinate, certificate :: rest =>
      (coordinate, (certificateCandidate certificate).objective)
        :: certificateKeys (coordinate + 1) rest

theorem consider_takes_the_first_valid_submission
    (selection : Selection) (candidate : Candidate)
    (openSelection : selection.closed = false)
    (candidateValid : candidate.valid = true)
    (vacant : selection.best = none) :
    (selection.consider candidate).best = some candidate := by
  simp [Selection.consider, openSelection, candidateValid, vacant]

theorem consider_replaces_a_strictly_better_submission
    (selection : Selection) (incumbent candidate : Candidate)
    (openSelection : selection.closed = false)
    (candidateValid : candidate.valid = true)
    (incumbentSelected : selection.best = some incumbent)
    (better :
      betterBy selection.policy.criteria candidate.objective incumbent.objective = true) :
    (selection.consider candidate).best = some candidate := by
  simp [Selection.consider, openSelection, candidateValid, incumbentSelected,
    SelectionPolicy.better, better]

theorem consider_keeps_an_incumbent_no_submission_betters
    (selection : Selection) (incumbent candidate : Candidate)
    (openSelection : selection.closed = false)
    (candidateValid : candidate.valid = true)
    (incumbentSelected : selection.best = some incumbent)
    (notBetter :
      betterBy selection.policy.criteria candidate.objective incumbent.objective = false) :
    (selection.consider candidate).best = some incumbent := by
  simp [Selection.consider, openSelection, candidateValid, incumbentSelected,
    SelectionPolicy.better, notBetter]

/-- The key-level fold IS `Selection.consider`, projected onto the objective
the runtime cursor persists. This is what lets a corpus of keys stand in for a
corpus of candidates: the coordinate rides along untouched, so keeping the
incumbent on a tie is what makes the EARLIEST submission win. -/
theorem consider_key_agrees_with_consider
    (selection : Selection) (incumbent candidate : Candidate)
    (coordinate incumbentCoordinate : Nat)
    (openSelection : selection.closed = false)
    (candidateValid : candidate.valid = true)
    (incumbentSelected : selection.best = some incumbent) :
    (considerKey selection.policy (some (incumbentCoordinate, incumbent.objective))
        (coordinate, candidate.objective)).map Prod.snd =
      (selection.consider candidate).best.map Candidate.objective := by
  by_cases better :
      betterBy selection.policy.criteria candidate.objective incumbent.objective = true
  · rw [consider_replaces_a_strictly_better_submission selection incumbent candidate
      openSelection candidateValid incumbentSelected better]
    simp [considerKey, better]
  · simp only [Bool.not_eq_true] at better
    rw [consider_keeps_an_incumbent_no_submission_betters selection incumbent candidate
      openSelection candidateValid incumbentSelected better]
    simp [considerKey, better]

/-! ### The vectors -/

def certificate (candidateId lots : Nat) (deliverPerLot receivePerLot : List Nat)
    (quoteDebit quoteCredit : Nat) : SubmittedCertificate :=
  { candidateId, lots, deliverPerLot, receivePerLot, quoteDebit, quoteCredit }

/-- The exact-tie submission. It is submitted twice, under two Batch
coordinates, which is the only way an exact objective tie can reach the
runtime fold at all: two byte-identical certificates would be refused as a
`DuplicateCandidate` before any comparison happened. -/
def tieCertificate : SubmittedCertificate := certificate 11 2 [4, 4] [4, 4] 12 3

/-- One selection decision case: submissions in cursor order. -/
structure SelectionVector where
  name : String
  submissions : List SubmittedCertificate

/-- The production V5 policy as a complete admissible policy record. -/
def canonicalPolicy : SelectionPolicy :=
  { policyId := 1, criteria := canonicalCriteria }

theorem canonical_policy_is_admissible : canonicalPolicy.valid = true := by
  native_decide

def selectionVectors : List SelectionVector := [
  { name := "the_first_submission_becomes_the_incumbent",
    submissions := [certificate 7 3 [2, 2] [2, 2] 10 4] },
  { name := "more_filled_lots_replaces_a_smaller_quote_surplus",
    submissions := [certificate 5 2 [1, 1] [1, 1] 9 9,
                    certificate 9 4 [1, 1] [1, 1] 9 1] },
  { name := "fewer_filled_lots_never_replaces_however_small_the_surplus",
    submissions := [certificate 9 5 [1, 1] [1, 1] 9 1,
                    certificate 2 4 [1, 1] [1, 1] 3 3] },
  { name := "equal_lots_are_decided_by_the_smaller_quote_surplus",
    submissions := [certificate 4 6 [3, 3] [3, 3] 20 5,
                    certificate 8 6 [3, 3] [3, 3] 20 18] },
  { name := "equal_lots_and_surplus_are_decided_in_the_most_significant_id_byte",
    submissions := [certificate 452312848583266388373324160190187140051835877600158453279131187530910662656
                      3 [1, 1] [1, 1] 7 2,
                    certificate 6 3 [1, 1] [1, 1] 7 2] },
  { name := "an_exact_objective_tie_keeps_the_submitted_incumbent",
    submissions := [tieCertificate, tieCertificate] },
  { name := "the_complete_set_mint_and_merge_reach_the_same_surplus_domain",
    submissions := [certificate 3 1 [2, 2] [5, 5] 10 1,
                    certificate 4 1 [6, 6] [1, 1] 10 12] },
  { name := "a_three_outcome_batch_keeps_a_winner_a_later_submission_ties_on_lots",
    submissions := [certificate 20 4 [1, 1, 1] [1, 1, 1] 8 0,
                    certificate 30 9 [2, 2, 2] [2, 2, 2] 5 5,
                    certificate 1 9 [1, 1, 1] [1, 1, 1] 6 5] }
]

/-- The decision one policy reaches on one vector: winning coordinate, filled
lots, quote surplus, candidate identity. -/
def decisionUnder (criteria : List SelectionCriterion) (vector : SelectionVector) :
    Nat × Nat × Nat × Nat :=
  match selectBest { policyId := 1, criteria := criteria }
      (certificateKeys 1 vector.submissions) with
  | none => (0, 0, 0, 0)
  | some (coordinate, objective) =>
      (coordinate, objective.filledLots, objective.quoteSurplus, objective.candidateId)

def SelectionVector.decision (vector : SelectionVector) : Nat × Nat × Nat × Nat :=
  decisionUnder canonicalCriteria vector

/-- Every certificate's claim banks agree with the movement this module reads
off coordinate zero. This is the domain on which the runtime's balance
derivation does not refuse `ClaimImbalance`, so a green replay means the two
agree rather than that one of them never ran. -/
theorem corpus_certificates_have_balanced_claims :
    selectionVectors.all (fun vector =>
      vector.submissions.all (fun submitted =>
        (certificateCandidate submitted).claimsBalance)) = true := by
  native_decide

/-- Every certificate's quote credit is covered after materialization: the
domain on which the runtime refuses no `QuoteImbalance` and on which
`Objective.quoteSurplus` does not truncate. -/
theorem corpus_certificates_cover_their_quote_credit :
    selectionVectors.all (fun vector =>
      vector.submissions.all (fun submitted =>
        (certificateCandidate submitted).quoteBalances)) = true := by
  native_decide

/-- The exact quote surplus of every submission, in order. The runtime derives
each of these from claim banks and aggregate quote fields; these are the
numbers it must reach. -/
theorem corpus_submission_quote_surpluses_are_exact :
    selectionVectors.map (fun vector =>
      vector.submissions.map (fun submitted =>
        (certificateCandidate submitted).objective.quoteSurplus)) =
      [[6], [0, 8], [8, 0], [15, 2], [5, 5], [9, 9], [6, 3], [8, 0, 1]] := by
  native_decide

/-- The exact decision of every vector, in order. -/
theorem corpus_selection_decisions_are_exact :
    selectionVectors.map SelectionVector.decision =
      [(1, 3, 6, 7), (2, 4, 8, 9), (1, 5, 8, 9), (2, 6, 2, 8),
       (2, 3, 5, 6), (1, 2, 9, 11), (2, 1, 3, 4), (2, 9, 0, 30)] := by
  native_decide

/-- The corpus is not one answer repeated, and it is not agreeable to any
policy: drop any single criterion from the canonical order and some vector's
decision changes. A vector list that agreed with everything could not satisfy
this. -/
theorem every_canonical_criterion_decides_some_vector :
    (selectionVectors.any fun vector =>
      decisionUnder [.minimizeQuoteSurplus, .minimizeCandidateId] vector !=
        vector.decision) = true ∧
    (selectionVectors.any fun vector =>
      decisionUnder [.maximizeFilledLots, .minimizeCandidateId] vector !=
        vector.decision) = true ∧
    (selectionVectors.any fun vector =>
      decisionUnder [.maximizeFilledLots, .minimizeQuoteSurplus] vector !=
        vector.decision) = true := by
  native_decide

/-- The tie vector really ties, and the earlier coordinate keeps the
selection. The first conjunct is `exact_objective_tie_never_replaces` at the
canonical policy on real corpus data rather than on a hypothesis. -/
theorem corpus_exact_tie_keeps_the_earlier_coordinate :
    canonicalBetter (certificateCandidate tieCertificate).objective
        (certificateCandidate tieCertificate).objective = false ∧
      (decisionUnder canonicalCriteria
        { name := "tie", submissions := [tieCertificate, tieCertificate] }).1 = 1 := by
  refine ⟨exact_objective_tie_never_replaces canonicalCriteria _ _ rfl, ?_⟩
  native_decide

end DClutch.General.V5Assurance
