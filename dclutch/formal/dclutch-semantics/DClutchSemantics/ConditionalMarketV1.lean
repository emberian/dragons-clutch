import DClutchSemantics.EconomicKernel
import DClutchSemantics.JointClearingV1

/-!
# Conditional and product markets: the combinatorial layer

The semantic owner of the CONDITIONAL mechanism
(`docs/design/MECHANISM_CONDITIONAL_MARKETS_2026_09_04.md`): a child Market
whose outcomes are built from two parents' outcomes and whose settlement is a
function of the parents' resolution certificates, with no observation of its
own.

Two shapes, one settlement rule:

* a **product market** `A × B` has one ordinary cell per pair of ordinary parent
  outcomes, laid out row-major with the first parent as the major axis, plus
  the child's own explicit failure coordinate;
* a **conditional market** `B | A = a` has one ordinary cell per ordinary
  outcome of `B` (the condition branch), one ordinary *off-condition* cell that
  pays when `A` resolves to any other ordinary outcome, plus the child's own
  failure coordinate.

Both settle to the child's failure coordinate — and so to decision 0025's
constant-per-claim refund walk — exactly when a parent the branch depends on
resolved to *its* failure coordinate. The conditional market is the product
market's row projection: on the condition branch the two selectors agree up to
the row offset (`conditional_is_the_row_projection`), off it the conditional
settles ordinarily where the product may still fail
(`off_condition_ignores_B`), which is the precise sense in which it is a
projection and not a synonym.

What the theorems say:

* `childPayoutVector_sum` — every child terminal, on either parent's
  certificate, partitions the child's own payout scale: full backing, the
  parents untouched (census laws L1/L8 restricted to the child);
* `outage_refund_is_constant_per_claim` — the refund arm is the escrow's walk,
  reused by name;
* `rows_partition_the_cells`, `columns_partition_the_cells`,
  `marginals_sum_to_the_scale` — marginals read off a product clearing's price
  vector are exact and sum to the scale less the failure price;
* `row_bundle_replicates_the_parent_claim_off_failure` — a row bundle of the
  child pays exactly `R_B` parent claims whenever neither parent fails, so a
  price gap between the two is a riskless trade off failure;
  `row_bundle_refunds_when_B_fails` is the one scenario where they differ;
* `closed_arbitrage_makes_the_marginal_the_parent_read` — with that gap closed
  the child's marginal and the parent's price name the same probability;
* `branch_claims_pay_nothing_off_condition` — the formal root of the
  decision-market pathology: off the condition every branch claim is worth
  nothing whatever it was priced at;
* `settleProduct_ok_is_the_selector`, `settleProduct_needs_A`,
  `admit_refuses_a_replaced_parent`, `admit_refuses_a_stranger` — the child's
  selector is a function of the parents' certificates and of nothing else, and
  the hostiles (a child settling before a parent, a parent replaced after
  founding, a certificate of another market) are refused by name.

Everything physical — that a certificate account is the Resolution-owned PDA
`authenticate_terminal_certificate` reads, that the child's Hoard is its own
Custody compartment — is the adapter's, as in `GeneralClearing`'s
`AdapterBoundary`. No existing module is edited.
-/

namespace DClutch.ConditionalMarket

open DClutch.JointClearing (sumRange sumRange_congr sumRange_add sumRange_zero
  sumRange_le sumRange_mul_left Clearing prices_total price_nonneg)

/-! ## Sums over rectangles -/

theorem sumRange_succ_last (n : Nat) (f : Nat → Int) :
    sumRange (n + 1) f = sumRange n f + f n := by
  induction n generalizing f with
  | zero => simp [sumRange]
  | succ n ih =>
    have step : sumRange (n + 1 + 1) f = f 0 + sumRange (n + 1) (fun i => f (i + 1)) := rfl
    have step' : sumRange (n + 1) f = f 0 + sumRange n (fun i => f (i + 1)) := rfl
    rw [step, ih (fun i => f (i + 1)), step']
    omega

theorem sumRange_split_at (p q : Nat) (f : Nat → Int) :
    sumRange (p + q) f = sumRange p f + sumRange q (fun i => f (p + i)) := by
  induction p generalizing f with
  | zero => simp [sumRange]
  | succ p ih =>
    have step : sumRange (p + 1 + q) f = f 0 + sumRange (p + q) (fun i => f (i + 1)) := by
      rw [show p + 1 + q = (p + q) + 1 by omega]
      rfl
    have step' : sumRange (p + 1) f = f 0 + sumRange p (fun i => f (i + 1)) := rfl
    rw [step, ih (fun i => f (i + 1)), step']
    have shift : sumRange q (fun i => f (p + i + 1)) = sumRange q (fun i => f (p + 1 + i)) :=
      sumRange_congr (fun i _ => by rw [show p + i + 1 = p + 1 + i by omega])
    rw [shift]
    omega

theorem sumRange_rect (m n : Nat) (f : Nat → Int) :
    sumRange (m * n) f = sumRange m (fun a => sumRange n (fun b => f (a * n + b))) := by
  induction m generalizing f with
  | zero => simp [sumRange]
  | succ m ih =>
    rw [Nat.succ_mul, sumRange_split_at, ih, sumRange_succ_last]

theorem sumRange_swap (m n : Nat) (g : Nat → Nat → Int) :
    sumRange m (fun a => sumRange n (fun b => g a b)) =
      sumRange n (fun b => sumRange m (fun a => g a b)) := by
  induction m generalizing g with
  | zero => simp [sumRange, sumRange_zero]
  | succ m ih =>
    have step : sumRange (m + 1) (fun a => sumRange n (fun b => g a b)) =
        sumRange n (fun b => g 0 b) + sumRange m (fun a => sumRange n (fun b => g (a + 1) b)) :=
      rfl
    rw [step, ih (fun a b => g (a + 1) b), ← sumRange_add]
    exact sumRange_congr (fun b _ => rfl)

theorem sumRange_const (n : Nat) (c : Int) : sumRange n (fun _ => c) = n * c := by
  induction n with
  | zero => simp [sumRange]
  | succ n ih =>
    have step : sumRange (n + 1) (fun _ => c) = c + sumRange n (fun _ => c) := rfl
    rw [step, ih]
    simp [Int.add_mul]
    omega

theorem term_le_sumRange (n : Nat) (f : Nat → Int) (nonneg : ∀ i, i < n → 0 ≤ f i)
    (j : Nat) (hj : j < n) : f j ≤ sumRange n f := by
  induction n generalizing f j with
  | zero => omega
  | succ n ih =>
    have step : sumRange (n + 1) f = f 0 + sumRange n (fun i => f (i + 1)) := rfl
    rw [step]
    cases j with
    | zero =>
      have rest : 0 ≤ sumRange n (fun i => f (i + 1)) := by
        have := sumRange_le (n := n) (f := fun _ => 0) (g := fun i => f (i + 1))
          (fun i hi => nonneg (i + 1) (by omega))
        rw [sumRange_zero] at this
        exact this
      omega
    | succ j =>
      have h0 := nonneg 0 (by omega)
      have tail : f (j + 1) ≤ sumRange n (fun i => f (i + 1)) :=
        ih (fun i => f (i + 1)) (fun i hi => nonneg (i + 1) (by omega)) j (by omega)
      omega

theorem sumRange_indicator (n j : Nat) (v : Int) (hj : j < n) :
    sumRange n (fun i => if i = j then v else 0) = v := by
  induction n generalizing j with
  | zero => omega
  | succ n ih =>
    have step : sumRange (n + 1) (fun i => if i = j then v else 0) =
        (if 0 = j then v else 0) + sumRange n (fun i => if i + 1 = j then v else 0) := rfl
    rw [step]
    cases j with
    | zero =>
      have rest : sumRange n (fun i => if i + 1 = 0 then v else 0) = 0 := by
        rw [sumRange_congr (g := fun _ => 0) (fun i _ => by simp)]
        exact sumRange_zero n
      rw [rest, if_pos rfl]
      omega
    | succ j =>
      have rest : sumRange n (fun i => if i + 1 = j + 1 then v else 0) = v := by
        rw [sumRange_congr (g := fun i => if i = j then v else 0) (fun i _ => by simp)]
        exact ih j (by omega)
      rw [rest, if_neg (by omega)]
      omega

/-! ## What a parent hands its children -/

/-- A parent's terminal as the child reads it: the ordinary count its Product
record binds (`outcomeCount − 1`) and the selector its certificate carries. A
selector equal to the ordinary count is the parent's failure coordinate. -/
structure ParentTerminal where
  ordinaryCount : Nat
  selector : Nat
  deriving DecidableEq, Repr

/-- The reference a child records at founding, immutable and content-addressed:
which market, at which generation, under which Product record, with how many
ordinary outcomes. A parent replaced after founding — a new generation at the
same address, or a re-founded Product — fails this reference at settlement. -/
structure ParentRef where
  marketId : Nat
  generation : Nat
  productRecordDigest : Nat
  ordinaryCount : Nat
  deriving DecidableEq, Repr

/-- The fields of a `ResolutionCertificateV2` the child reads. `terminal` is
whether the kind is `ResolutionSuccess` or `ResolutionFailure`; the two
liveness kinds (`RecoveryAdvanced`, `Exhausted`) are not terminals and a child
presented one has been presented nothing. -/
structure ParentCertificate where
  marketId : Nat
  generation : Nat
  productRecordDigest : Nat
  ordinaryCount : Nat
  selector : Nat
  terminal : Bool
  deriving DecidableEq, Repr

inductive Refusal where
  /-- A parent the branch depends on has no terminal certificate yet. -/
  | parentNotTerminal
  /-- The certificate names a market the reference does not. -/
  | wrongParent
  /-- The certificate's generation is not the referenced one. -/
  | parentGenerationMismatch
  /-- The certificate binds a Product record the reference does not. -/
  | parentRecordMismatch
  /-- The parent's ordinary count is not the referenced one. -/
  | parentWidthMismatch
  /-- The certificate's selector exceeds the parent's width. -/
  | selectorOutOfRange
  /-- The condition names an outcome past the parent's failure coordinate. -/
  | conditionOutOfRange
  /-- The condition names the parent's failure coordinate: a market that pays
  on an outage, which decision 0025 forbids. -/
  | conditionOnFailure
  /-- Both parents are one market. -/
  | sameParent
  /-- A parent with no ordinary outcome. -/
  | emptyParent
  /-- The child's width exceeds the bank cap. -/
  | widthOverflow
  deriving DecidableEq, Repr

/-- Admit one certificate against the reference it must satisfy. -/
def ParentRef.admit (ref : ParentRef) (cert : ParentCertificate) : Except Refusal ParentTerminal :=
  if !cert.terminal then .error .parentNotTerminal
  else if cert.marketId ≠ ref.marketId then .error .wrongParent
  else if cert.generation ≠ ref.generation then .error .parentGenerationMismatch
  else if cert.productRecordDigest ≠ ref.productRecordDigest then .error .parentRecordMismatch
  else if cert.ordinaryCount ≠ ref.ordinaryCount then .error .parentWidthMismatch
  else if ref.ordinaryCount < cert.selector then .error .selectorOutOfRange
  else .ok { ordinaryCount := ref.ordinaryCount, selector := cert.selector }

def admitParent (ref : ParentRef) : Option ParentCertificate → Except Refusal ParentTerminal
  | none => .error .parentNotTerminal
  | some cert => ref.admit cert

/-- The General bank cap: `151 + 6K ≤ 512 ⇒ K ≤ 60`
(`programs/dclutch-trading-sbf/src/hot_v3.rs:456`). Mathematical. -/
def maxOutcomeCount : Nat := 60

/-! ## The product market -/

/-- `A × B`, row-major with `A` as the major axis: cell `(a, b)` sits at
`a · R_B + b`, so every "conditional on `A = a`" bundle is one interval of the
joint clearing's admitted shape. -/
structure ProductShape where
  parentA : ParentRef
  parentB : ParentRef
  deriving DecidableEq, Repr

namespace ProductShape

def rows (s : ProductShape) : Nat := s.parentA.ordinaryCount
def columns (s : ProductShape) : Nat := s.parentB.ordinaryCount
def cells (s : ProductShape) : Nat := s.rows * s.columns
def width (s : ProductShape) : Nat := s.cells + 1
def failureSelector (s : ProductShape) : Nat := s.cells
def cell (s : ProductShape) (a b : Nat) : Nat := a * s.columns + b
def row (s : ProductShape) (i : Nat) : Nat := i / s.columns
def column (s : ProductShape) (i : Nat) : Nat := i % s.columns

/-- Founding admission: distinct parents, both with an ordinary outcome, and a
width the bank clears. -/
def found? (s : ProductShape) : Except Refusal ProductShape :=
  if s.parentA.marketId = s.parentB.marketId then .error .sameParent
  else if s.rows = 0 ∨ s.columns = 0 then .error .emptyParent
  else if maxOutcomeCount < s.width then .error .widthOverflow
  else .ok s

end ProductShape

theorem cell_lt_cells (s : ProductShape) (a b : Nat) (ha : a < s.rows) (hb : b < s.columns) :
    s.cell a b < s.cells := by
  unfold ProductShape.cell ProductShape.cells
  have h1 : (a + 1) * s.columns ≤ s.rows * s.columns := Nat.mul_le_mul_right _ ha
  have h2 : a * s.columns + b < (a + 1) * s.columns := by rw [Nat.succ_mul]; omega
  omega

theorem cell_row (s : ProductShape) (a b : Nat) (hb : b < s.columns) : s.row (s.cell a b) = a := by
  unfold ProductShape.row ProductShape.cell
  have pos : 0 < s.columns := by omega
  rw [Nat.mul_comm, Nat.mul_add_div pos, Nat.div_eq_of_lt hb]
  simp

theorem cell_column (s : ProductShape) (a b : Nat) (hb : b < s.columns) :
    s.column (s.cell a b) = b := by
  unfold ProductShape.column ProductShape.cell
  rw [Nat.mul_comm, Nat.mul_add_mod, Nat.mod_eq_of_lt hb]

/-- Two cells with the same index are the same pair: the layout is injective. -/
theorem cell_injective (s : ProductShape) (a b a' b' : Nat) (hb : b < s.columns)
    (hb' : b' < s.columns) (h : s.cell a b = s.cell a' b') : a = a' ∧ b = b' := by
  constructor
  · have := congrArg s.row h
    rwa [cell_row _ _ _ hb, cell_row _ _ _ hb'] at this
  · have := congrArg s.column h
    rwa [cell_column _ _ _ hb, cell_column _ _ _ hb'] at this

/-- The child's selector from the parents' terminals: the cell when both are
ordinary, the child's failure coordinate when either is not. -/
def productSelector (s : ProductShape) (ta tb : ParentTerminal) : Nat :=
  if ta.selector < s.rows ∧ tb.selector < s.columns then s.cell ta.selector tb.selector
  else s.failureSelector

theorem productSelector_lt_width (s : ProductShape) (ta tb : ParentTerminal) :
    productSelector s ta tb < s.width := by
  unfold productSelector ProductShape.width ProductShape.failureSelector
  split
  · rename_i h
    have := cell_lt_cells s _ _ h.1 h.2
    omega
  · omega

theorem productSelector_ordinary_iff (s : ProductShape) (ta tb : ParentTerminal) :
    productSelector s ta tb < s.cells ↔ (ta.selector < s.rows ∧ tb.selector < s.columns) := by
  unfold productSelector ProductShape.failureSelector
  split
  · rename_i h
    exact ⟨fun _ => h, fun _ => cell_lt_cells s _ _ h.1 h.2⟩
  · rename_i h
    exact ⟨fun contra => absurd contra (Nat.lt_irrefl _), fun hh => absurd hh h⟩

/-- A parent's outage is the child's outage: no cell pays on either parent's
failure coordinate, which is decision 0025 carried one level up. -/
theorem productSelector_failure_of_parent_failure (s : ProductShape) (ta tb : ParentTerminal)
    (h : s.rows ≤ ta.selector ∨ s.columns ≤ tb.selector) :
    productSelector s ta tb = s.failureSelector := by
  unfold productSelector
  split
  · rename_i hh
    omega
  · rfl

/-- The selector decodes: the row is the first parent's outcome and the column
the second's. This is marginalisation at the level of outcomes. -/
theorem productSelector_decodes (s : ProductShape) (ta tb : ParentTerminal)
    (ha : ta.selector < s.rows) (hb : tb.selector < s.columns) :
    s.row (productSelector s ta tb) = ta.selector ∧
      s.column (productSelector s ta tb) = tb.selector := by
  unfold productSelector
  simp only [ha, hb, and_self, if_true]
  exact ⟨cell_row s _ _ hb, cell_column s _ _ hb⟩

/-! ## The conditional market -/

/-- `B | A = condition`. Cells `0 … R_B − 1` are the condition branch, cell
`R_B` is the off-condition cell, `R_B + 1` is the child's failure coordinate. -/
structure ConditionalShape where
  parentA : ParentRef
  condition : Nat
  parentB : ParentRef
  deriving DecidableEq, Repr

namespace ConditionalShape

def branch (s : ConditionalShape) : Nat := s.parentB.ordinaryCount
def offConditionCell (s : ConditionalShape) : Nat := s.branch
def ordinaryCount (s : ConditionalShape) : Nat := s.branch + 1
def failureSelector (s : ConditionalShape) : Nat := s.branch + 1
def width (s : ConditionalShape) : Nat := s.branch + 2

/-- The product market the conditional projects. -/
def product (s : ConditionalShape) : ProductShape := { parentA := s.parentA, parentB := s.parentB }

def found? (s : ConditionalShape) : Except Refusal ConditionalShape :=
  if s.parentA.marketId = s.parentB.marketId then .error .sameParent
  else if s.parentA.ordinaryCount = 0 ∨ s.branch = 0 then .error .emptyParent
  else if s.condition = s.parentA.ordinaryCount then .error .conditionOnFailure
  else if s.parentA.ordinaryCount < s.condition then .error .conditionOutOfRange
  else if maxOutcomeCount < s.width then .error .widthOverflow
  else .ok s

end ConditionalShape

/-- The conditional settlement. `A` is always needed; `B` only on the
condition branch, which is why it is an `Option`: a child whose condition
failed settles the moment `A` is terminal, whatever `B` is doing. -/
def conditionalSelector (s : ConditionalShape) (ta : ParentTerminal)
    (tb : Option ParentTerminal) : Except Refusal Nat :=
  if s.parentA.ordinaryCount ≤ ta.selector then .ok s.failureSelector
  else if ta.selector ≠ s.condition then .ok s.offConditionCell
  else match tb with
    | none => .error .parentNotTerminal
    | some tb => if tb.selector < s.branch then .ok tb.selector else .ok s.failureSelector

theorem off_condition_settles_without_B (s : ConditionalShape) (ta : ParentTerminal)
    (ordinary : ta.selector < s.parentA.ordinaryCount) (off : ta.selector ≠ s.condition) :
    conditionalSelector s ta none = .ok s.offConditionCell := by
  unfold conditionalSelector
  simp [Nat.not_le.mpr ordinary, off]

theorem condition_branch_needs_B (s : ConditionalShape) (ta : ParentTerminal)
    (ordinary : ta.selector < s.parentA.ordinaryCount) (on : ta.selector = s.condition) :
    conditionalSelector s ta none = .error .parentNotTerminal := by
  unfold conditionalSelector
  rw [on] at ordinary
  simp [Nat.not_le.mpr ordinary, on]

/-- Off the condition, `B` is not read at all: the settlement is the same for
every `B`, terminal or not. This is the formal root of the decision-market
pathology — a branch the decision does not take pays its claims nothing and
refunds nobody's mispricing (`branch_claims_pay_nothing_off_condition`). -/
theorem off_condition_ignores_B (s : ConditionalShape) (ta : ParentTerminal)
    (tb tb' : Option ParentTerminal)
    (ordinary : ta.selector < s.parentA.ordinaryCount) (off : ta.selector ≠ s.condition) :
    conditionalSelector s ta tb = conditionalSelector s ta tb' := by
  unfold conditionalSelector
  simp [Nat.not_le.mpr ordinary, off]

theorem conditionalSelector_lt_width (s : ConditionalShape) (ta : ParentTerminal)
    (tb : Option ParentTerminal) (v : Nat) (h : conditionalSelector s ta tb = .ok v) :
    v < s.width := by
  unfold conditionalSelector at h
  unfold ConditionalShape.width
  split at h
  · simp only [Except.ok.injEq] at h
    unfold ConditionalShape.failureSelector at h
    omega
  · split at h
    · simp only [Except.ok.injEq] at h
      unfold ConditionalShape.offConditionCell at h
      omega
    · split at h
      · exact absurd h (by simp)
      · split at h
        · simp only [Except.ok.injEq] at h
          omega
        · simp only [Except.ok.injEq] at h
          unfold ConditionalShape.failureSelector at h
          omega

/-- **The conditional market is the product market's row projection.** On the
condition branch, with both parents ordinary, the product selector is the
conditional selector plus the row offset `condition · R_B`. -/
theorem conditional_is_the_row_projection (s : ConditionalShape) (ta tb : ParentTerminal)
    (on : ta.selector = s.condition) (ordinaryA : ta.selector < s.parentA.ordinaryCount)
    (ordinaryB : tb.selector < s.branch) :
    conditionalSelector s ta (some tb) = .ok tb.selector ∧
      productSelector s.product ta tb = s.condition * s.branch + tb.selector := by
  have ordinaryA' : s.condition < s.parentA.ordinaryCount := on ▸ ordinaryA
  have ordinaryB' : tb.selector < s.parentB.ordinaryCount := ordinaryB
  constructor
  · unfold conditionalSelector
    simp [Nat.not_le.mpr ordinaryA', on, ordinaryB]
  · unfold productSelector ProductShape.cell ProductShape.rows ProductShape.columns
      ConditionalShape.product
    rw [on]
    simp only [ordinaryA', ordinaryB', and_self, if_true]
    rfl

/-- Off the condition the product selector lies outside the condition row, and
the conditional settles to its off-condition cell. -/
theorem off_condition_is_the_complement (s : ConditionalShape) (ta tb : ParentTerminal)
    (off : ta.selector ≠ s.condition) (ordinaryA : ta.selector < s.parentA.ordinaryCount)
    (ordinaryB : tb.selector < s.branch) :
    conditionalSelector s ta (some tb) = .ok s.offConditionCell ∧
      s.product.row (productSelector s.product ta tb) ≠ s.condition := by
  constructor
  · unfold conditionalSelector
    simp [Nat.not_le.mpr ordinaryA, off]
  · have := (productSelector_decodes s.product ta tb ordinaryA ordinaryB).1
    rw [this]
    exact off

/-- On the condition branch the two shapes fail together: `B`'s outage is the
child's outage under either shape. -/
theorem failure_agrees_on_the_condition_branch (s : ConditionalShape) (ta tb : ParentTerminal)
    (on : ta.selector = s.condition) (ordinaryA : ta.selector < s.parentA.ordinaryCount)
    (failedB : s.branch ≤ tb.selector) :
    conditionalSelector s ta (some tb) = .ok s.failureSelector ∧
      productSelector s.product ta tb = s.product.failureSelector := by
  have ordinaryA' : s.condition < s.parentA.ordinaryCount := on ▸ ordinaryA
  constructor
  · unfold conditionalSelector
    simp [Nat.not_le.mpr ordinaryA', on, Nat.not_lt.mpr failedB]
  · exact productSelector_failure_of_parent_failure s.product ta tb (Or.inr failedB)

/-! ## (a) Full backing: every child terminal partitions the child's own scale -/

/-- The child's terminal payout vector at `selector`: the kernel's categorical
one-hot on an ordinary cell, the escrow's refund vector otherwise. `unit` is
the atoms one ordinary claim draws on the refund walk; the scale is
`ordinaryCount · unit`, the founding shape decision 0025 admits
(`foundingRefundExact`). -/
def childPayoutVector (ordinaryCount unit selector : Nat) : List Nat :=
  if selector < ordinaryCount then Economic.successPayoutVector ordinaryCount unit selector
  else Economic.failurePayoutVector ordinaryCount unit

theorem childPayoutVector_length (ordinaryCount unit selector : Nat) :
    (childPayoutVector ordinaryCount unit selector).length = ordinaryCount + 1 := by
  unfold childPayoutVector
  split
  · rw [Economic.success_payout_vector_has_the_runtime_width]; rfl
  · rw [Economic.failure_payout_vector_has_the_runtime_width]; rfl

/-- **Full backing.** Whatever the parents' certificates say, the child's
payout vector sums to the child's own scale: the child's Hoard is drawn exactly
once and the parents' Hoards do not appear. Census law L1 restricted to the
child's Custody compartment, and L8's zero delta for every parent class. -/
theorem childPayoutVector_sum (ordinaryCount unit selector : Nat) :
    (childPayoutVector ordinaryCount unit selector).sum = ordinaryCount * unit := by
  unfold childPayoutVector
  split
  · rename_i h
    exact (Economic.both_terminal_arms_partition_the_same_scale ordinaryCount unit selector
      (by unfold Economic.outcomeWidth; omega)).2
  · exact (Economic.both_terminal_arms_partition_the_same_scale ordinaryCount unit 0
      (by unfold Economic.outcomeWidth; omega)).1

theorem product_settlement_draws_exactly_the_child_scale (s : ProductShape) (unit : Nat)
    (ta tb : ParentTerminal) :
    (childPayoutVector s.cells unit (productSelector s ta tb)).sum = s.cells * unit :=
  childPayoutVector_sum _ _ _

theorem conditional_settlement_draws_exactly_the_child_scale (s : ConditionalShape) (unit : Nat)
    (ta : ParentTerminal) (tb : Option ParentTerminal) (v : Nat)
    (_ : conditionalSelector s ta tb = .ok v) :
    (childPayoutVector s.ordinaryCount unit v).sum = s.ordinaryCount * unit :=
  childPayoutVector_sum _ _ _

/-- The payout reads nothing of the parents but their selectors: two parent
pairs with one child selector are one payout. -/
theorem child_payout_reads_only_the_selector (s : ProductShape) (unit : Nat)
    (ta tb ta' tb' : ParentTerminal) (same : productSelector s ta tb = productSelector s ta' tb') :
    childPayoutVector s.cells unit (productSelector s ta tb) =
      childPayoutVector s.cells unit (productSelector s ta' tb') := by
  rw [same]

/-- **The refund arm is the escrow's walk.** On the child's failure coordinate
each ordinary claim draws `unit`, a constant the child's own header
determines, with no holder census: decision 0025's
`an_admitted_founding_makes_every_refund_exact`, reused by name. -/
theorem outage_refund_is_constant_per_claim (ordinaryCount unit quantity : Nat)
    (positive : 0 < ordinaryCount) :
    Economic.failureRefund ordinaryCount quantity (ordinaryCount * unit) = quantity * unit :=
  Economic.an_admitted_founding_makes_every_refund_exact ordinaryCount unit quantity positive

/-! ## Reading one coordinate of a payout vector -/

theorem valueAt_success (ordinaryCount unit winner index : Nat)
    (inRange : winner < ordinaryCount + 1) :
    Economic.valueAt (Economic.successPayoutVector ordinaryCount unit winner) index =
      if index = winner then ordinaryCount * unit else 0 := by
  unfold Economic.successPayoutVector Economic.setAt Economic.valueAt
  have len : (List.replicate (Economic.outcomeWidth ordinaryCount) 0).length = ordinaryCount + 1 := by
    simp [Economic.outcomeWidth]
  by_cases h : index = winner
  · subst h
    simp [len, inRange]
  · have distinct := Ne.symm h
    simp [distinct, h, List.getElem?_replicate]
    split <;> rfl

theorem valueAt_failure (ordinaryCount unit index : Nat) :
    Economic.valueAt (Economic.failurePayoutVector ordinaryCount unit) index =
      if index < ordinaryCount then unit else 0 := by
  unfold Economic.failurePayoutVector Economic.valueAt
  by_cases h : index < ordinaryCount
  · simp [List.getElem?_append, h]
  · simp only [List.getElem?_append, List.length_replicate, h, if_false]
    have : index - ordinaryCount = 0 ∨ 0 < index - ordinaryCount := by omega
    rcases this with hz | hp
    · simp [hz]
    · have : ([0] : List Nat)[index - ordinaryCount]? = none := by
        apply List.getElem?_eq_none
        simp; omega
      simp [this]

/-- Off the condition, every claim of the condition branch is worth nothing:
whatever it was priced at, the off-condition terminal pays it zero. -/
theorem branch_claims_pay_nothing_off_condition (s : ConditionalShape) (unit b : Nat)
    (hb : b < s.branch) :
    Economic.valueAt (childPayoutVector s.ordinaryCount unit s.offConditionCell) b = 0 := by
  unfold childPayoutVector
  have inRange : s.offConditionCell < s.ordinaryCount := by
    unfold ConditionalShape.offConditionCell ConditionalShape.ordinaryCount; omega
  simp only [inRange, if_true]
  rw [valueAt_success _ _ _ _ (by unfold ConditionalShape.ordinaryCount at inRange ⊢; omega)]
  have : b ≠ s.offConditionCell := by unfold ConditionalShape.offConditionCell; omega
  simp [this]

/-- And the off-condition cell itself pays the whole scale to whoever holds
it, which is what makes the hedge constructible from the child's own claims. -/
theorem off_condition_cell_pays_the_scale (s : ConditionalShape) (unit : Nat) :
    Economic.valueAt (childPayoutVector s.ordinaryCount unit s.offConditionCell)
      s.offConditionCell = s.ordinaryCount * unit := by
  unfold childPayoutVector
  have inRange : s.offConditionCell < s.ordinaryCount := by
    unfold ConditionalShape.offConditionCell ConditionalShape.ordinaryCount; omega
  simp only [inRange, if_true]
  rw [valueAt_success _ _ _ _ (by unfold ConditionalShape.ordinaryCount at inRange ⊢; omega)]
  simp

/-! ## (b) Marginals of a product clearing -/

/-- A product market's clearing is a joint clearing of the child's width. -/
def ProductShape.clearingFits (s : ProductShape) (c : Clearing) : Prop :=
  c.outcomeCount = s.width

/-- The price of the row bundle "`A = a`, whatever `B`": the child's implied
price of the first parent's outcome `a`. -/
def rowPrice (c : Clearing) (s : ProductShape) (a : Nat) : Int :=
  sumRange s.columns (fun b => c.price (s.cell a b))

/-- The price of the column bundle "`B = b`, whatever `A`". Not an interval
under the row-major layout: a trader expresses it as `R_A` single-cell orders. -/
def columnPrice (c : Clearing) (s : ProductShape) (b : Nat) : Int :=
  sumRange s.rows (fun a => c.price (s.cell a b))

theorem rows_partition_the_cells (c : Clearing) (s : ProductShape) :
    sumRange s.rows (rowPrice c s) = sumRange s.cells c.price := by
  unfold rowPrice ProductShape.cells ProductShape.cell
  rw [sumRange_rect]

theorem columns_partition_the_cells (c : Clearing) (s : ProductShape) :
    sumRange s.columns (columnPrice c s) = sumRange s.cells c.price := by
  unfold columnPrice ProductShape.cells ProductShape.cell
  rw [sumRange_rect, sumRange_swap]

/-- The marginals of either parent, read off the child's price vector, sum to
the child's scale less the price of the child's failure coordinate. With the
failure coordinate held by the escrow and priced at zero — `residual_worth_
nothing` in the joint clearing — the marginals are an exact simplex. -/
theorem marginals_sum_to_the_scale (c : Clearing) (s : ProductShape)
    (fits : s.clearingFits c) (valid : c.valid = true) :
    sumRange s.rows (rowPrice c s) + c.price s.failureSelector = (c.scale : Int) ∧
      sumRange s.columns (columnPrice c s) + c.price s.failureSelector = (c.scale : Int) := by
  have total := prices_total c valid
  unfold ProductShape.clearingFits ProductShape.width at fits
  rw [fits, sumRange_succ_last] at total
  unfold ProductShape.failureSelector
  rw [rows_partition_the_cells, columns_partition_the_cells]
  exact ⟨total, total⟩

/-- `P(B = b | A = a)` as an exact ratio of the child's own prices: the cell
over its row. No parent price enters the read. -/
def conditionalRead (c : Clearing) (s : ProductShape) (a b : Nat) : Int × Int :=
  (c.price (s.cell a b), rowPrice c s a)

theorem conditional_reads_partition_the_row (c : Clearing) (s : ProductShape) (a : Nat) :
    sumRange s.columns (fun b => (conditionalRead c s a b).1) = rowPrice c s a := rfl

theorem conditionalRead_le_row (c : Clearing) (s : ProductShape) (a b : Nat) (hb : b < s.columns) :
    (conditionalRead c s a b).1 ≤ (conditionalRead c s a b).2 := by
  unfold conditionalRead rowPrice
  exact term_le_sumRange _ _ (fun i _ => price_nonneg c _) b hb

/-! ## (b) Replication: a row bundle against the parent's own claim -/

/-- What one claim of parent `A`'s outcome `a` pays at `A`'s terminal. -/
def parentClaimPayout (rows unit selector a : Nat) : Int :=
  (Economic.valueAt (childPayoutVector rows unit selector) a : Int)

/-- What the row bundle — one claim of every cell `(a, ·)` of the child — pays
at the child's terminal. -/
def rowBundlePayout (s : ProductShape) (unit a : Nat) (ta tb : ParentTerminal) : Int :=
  sumRange s.columns (fun b =>
    (Economic.valueAt (childPayoutVector s.cells unit (productSelector s ta tb)) (s.cell a b) : Int))

/-- **Replication off failure.** Whenever neither parent fails, the row bundle
pays exactly `R_B` parent claims of `a` — on every joint outcome. A price gap
between the bundle and `R_B` parent claims is therefore a riskless trade in
every scenario but a parent's outage, which is what "consistency is closed by
arbitrage, not by a conjunct" means. -/
theorem row_bundle_replicates_the_parent_claim_off_failure (s : ProductShape) (unit a : Nat)
    (ta tb : ParentTerminal) (ha : ta.selector < s.rows) (hb : tb.selector < s.columns) :
    rowBundlePayout s unit a ta tb =
      (s.columns : Int) * parentClaimPayout s.rows unit ta.selector a := by
  have hsel : productSelector s ta tb = s.cell ta.selector tb.selector := by
    unfold productSelector; simp [ha, hb]
  have hk : s.cell ta.selector tb.selector < s.cells := cell_lt_cells s _ _ ha hb
  unfold rowBundlePayout parentClaimPayout childPayoutVector
  rw [hsel]
  simp only [hk, ha, if_true]
  by_cases hrow : a = ta.selector
  · subst hrow
    rw [sumRange_congr (g := fun b => if b = tb.selector then ((s.cells * unit : Nat) : Int) else 0)
      (fun b hb' => by
        rw [valueAt_success _ _ _ _ (by omega)]
        by_cases hbb : b = tb.selector
        · simp [hbb]
        · have distinct : s.cell ta.selector b ≠ s.cell ta.selector tb.selector := by
            intro heq
            exact hbb (cell_injective s ta.selector b ta.selector tb.selector hb' hb heq).2
          simp [distinct, hbb])]
    rw [sumRange_indicator _ _ _ hb, valueAt_success _ _ _ _ (by omega)]
    simp only [if_true]
    have arith : s.cells * unit = s.columns * (s.rows * unit) := by
      unfold ProductShape.cells
      rw [Nat.mul_comm s.rows s.columns, Nat.mul_assoc]
    rw [arith, Int.natCast_mul]
  · rw [sumRange_congr (g := fun _ => 0) (fun b hb' => by
        rw [valueAt_success _ _ _ _ (by omega)]
        have distinct : s.cell a b ≠ s.cell ta.selector tb.selector := by
          intro heq
          exact hrow (cell_injective s a b ta.selector tb.selector hb' hb heq).1
        simp [distinct])]
    rw [sumRange_zero, valueAt_success _ _ _ _ (by omega)]
    simp [hrow]

/-- **Where replication breaks.** With `A = a` ordinary and `B` on its failure
coordinate the child refunds: the row bundle pays `R_B · unit`, one refund per
claim, while `R_B` parent claims pay `R_B · R_A · unit`. The gap is the
failure premium a product price carries against its parent's. -/
theorem row_bundle_refunds_when_B_fails (s : ProductShape) (unit a : Nat)
    (ta tb : ParentTerminal) (ha : ta.selector < s.rows) (haa : a < s.rows)
    (failedB : s.columns ≤ tb.selector) :
    rowBundlePayout s unit a ta tb = (s.columns : Int) * unit ∧
      (s.columns : Int) * parentClaimPayout s.rows unit ta.selector ta.selector =
        (s.columns : Int) * ((s.rows * unit : Nat) : Int) := by
  have hsel : productSelector s ta tb = s.failureSelector :=
    productSelector_failure_of_parent_failure s ta tb (Or.inr failedB)
  constructor
  · unfold rowBundlePayout childPayoutVector
    rw [hsel]
    unfold ProductShape.failureSelector
    simp only [Nat.lt_irrefl, if_false]
    rw [sumRange_congr (g := fun _ => (unit : Int)) (fun b hb => by
      rw [valueAt_failure]
      simp [cell_lt_cells s a b haa hb])]
    rw [sumRange_const]
  · unfold parentClaimPayout childPayoutVector
    simp only [ha, if_true]
    rw [valueAt_success _ _ _ _ (by omega)]
    simp

/-- **Consistency under closed arbitrage.** If the row bundle's clearing cost
equals `R_B` parent claims' cost — the gap of the previous theorem closed — the
child's marginal and the parent's price name the same probability, at their
two scales. -/
theorem closed_arbitrage_makes_the_marginal_the_parent_read
    (rowPriceOfChild parentPrice : Int) (rows columns unit : Nat)
    (closed : rowPriceOfChild = (columns : Int) * parentPrice) :
    rowPriceOfChild * ((rows * unit : Nat) : Int) =
      parentPrice * ((rows * columns * unit : Nat) : Int) := by
  subst closed
  simp only [Int.natCast_mul]
  ac_rfl

/-! ## (d) Settlement is a function of the parents' certificates -/

def settleProduct (s : ProductShape) (certA certB : Option ParentCertificate) :
    Except Refusal Nat := do
  let ta ← admitParent s.parentA certA
  let tb ← admitParent s.parentB certB
  pure (productSelector s ta tb)

def settleConditional (s : ConditionalShape) (certA certB : Option ParentCertificate) :
    Except Refusal Nat := do
  let ta ← admitParent s.parentA certA
  let tb ← match certB with
    | none => pure none
    | some cert =>
        if cert.terminal then (s.parentB.admit cert).map some else pure none
  conditionalSelector s ta tb

/-- HOSTILE: a child settling before its first parent. -/
theorem settleProduct_needs_A (s : ProductShape) (certB : Option ParentCertificate) :
    settleProduct s none certB = .error .parentNotTerminal := rfl

/-- HOSTILE: a child settling before its second parent. -/
theorem settleProduct_needs_B (s : ProductShape) (certA : Option ParentCertificate)
    (ta : ParentTerminal) (admitted : admitParent s.parentA certA = .ok ta) :
    settleProduct s certA none = .error .parentNotTerminal := by
  unfold settleProduct
  simp only [bind, Except.bind]
  rw [admitted]
  rfl

/-- **Determinism.** A settled child selector is the product selector of the
two admitted terminals: a function of the certificates, of the reference, and
of nothing else — not of the caller, the slot, or the order the parents
resolved in. -/
theorem settleProduct_ok_is_the_selector (s : ProductShape)
    (certA certB : Option ParentCertificate) (v : Nat)
    (h : settleProduct s certA certB = .ok v) :
    ∃ ta tb, admitParent s.parentA certA = .ok ta ∧ admitParent s.parentB certB = .ok tb ∧
      v = productSelector s ta tb := by
  unfold settleProduct at h
  simp only [bind, Except.bind] at h
  split at h
  · exact absurd h (by simp)
  · rename_i ta hta
    split at h
    · exact absurd h (by simp)
    · rename_i tb htb
      simp only [pure, Except.pure, Except.ok.injEq] at h
      exact ⟨ta, tb, hta, htb, h.symm⟩

/-- HOSTILE: a live parent (no terminal certificate, or a liveness
certificate). -/
theorem admit_refuses_a_live_parent (ref : ParentRef) (cert : ParentCertificate)
    (live : cert.terminal = false) : ref.admit cert = .error .parentNotTerminal := by
  unfold ParentRef.admit
  simp [live]

/-- HOSTILE: a certificate of another market. -/
theorem admit_refuses_a_stranger (ref : ParentRef) (cert : ParentCertificate)
    (terminal : cert.terminal = true) (stranger : cert.marketId ≠ ref.marketId) :
    ref.admit cert = .error .wrongParent := by
  unfold ParentRef.admit
  simp [terminal, stranger]

/-- HOSTILE: a parent replaced after founding — the same market at another
generation. -/
theorem admit_refuses_a_replaced_parent (ref : ParentRef) (cert : ParentCertificate)
    (terminal : cert.terminal = true) (same : cert.marketId = ref.marketId)
    (moved : cert.generation ≠ ref.generation) :
    ref.admit cert = .error .parentGenerationMismatch := by
  unfold ParentRef.admit
  simp [terminal, same, moved]

/-- HOSTILE: a parent whose Product record moved under the reference. -/
theorem admit_refuses_a_moved_record (ref : ParentRef) (cert : ParentCertificate)
    (terminal : cert.terminal = true) (same : cert.marketId = ref.marketId)
    (generation : cert.generation = ref.generation)
    (moved : cert.productRecordDigest ≠ ref.productRecordDigest) :
    ref.admit cert = .error .parentRecordMismatch := by
  unfold ParentRef.admit
  simp [terminal, same, generation, moved]

/-- An admitted terminal binds every coordinate of the reference. -/
theorem admit_ok_binds_the_reference (ref : ParentRef) (cert : ParentCertificate)
    (t : ParentTerminal) (h : ref.admit cert = .ok t) :
    cert.terminal = true ∧ cert.marketId = ref.marketId ∧ cert.generation = ref.generation ∧
      cert.productRecordDigest = ref.productRecordDigest ∧
      cert.ordinaryCount = ref.ordinaryCount ∧ t.ordinaryCount = ref.ordinaryCount ∧
      t.selector = cert.selector ∧ t.selector ≤ ref.ordinaryCount := by
  unfold ParentRef.admit at h
  split at h
  · exact absurd h (by simp)
  · rename_i hterm
    split at h
    · exact absurd h (by simp)
    · split at h
      · exact absurd h (by simp)
      · split at h
        · exact absurd h (by simp)
        · split at h
          · exact absurd h (by simp)
          · split at h
            · exact absurd h (by simp)
            · simp only [Except.ok.injEq] at h
              subst h
              have terminal : cert.terminal = true := by
                revert hterm
                cases cert.terminal <;> simp
              refine ⟨terminal, ?_, ?_, ?_, ?_, rfl, rfl, ?_⟩
              · omega
              · omega
              · omega
              · omega
              · show cert.selector ≤ ref.ordinaryCount
                omega

/-- Founding admits only a width the bank clears. -/
theorem found_product_fits_the_bank (s s' : ProductShape) (h : s.found? = .ok s') :
    s' = s ∧ s'.width ≤ maxOutcomeCount ∧ 0 < s'.rows ∧ 0 < s'.columns := by
  unfold ProductShape.found? at h
  split at h
  · exact absurd h (by simp)
  · split at h
    · exact absurd h (by simp)
    · split at h
      · exact absurd h (by simp)
      · simp only [Except.ok.injEq] at h
        subst h
        rename_i notEmpty notWide
        refine ⟨rfl, by omega, ?_, ?_⟩ <;> omega

/-- Founding refuses a condition on the parent's failure coordinate and any
condition past it: the condition is always an ordinary outcome of `A`. -/
theorem found_conditional_condition_is_ordinary (s s' : ConditionalShape)
    (h : s.found? = .ok s') : s' = s ∧ s'.condition < s'.parentA.ordinaryCount := by
  unfold ConditionalShape.found? at h
  split at h
  · exact absurd h (by simp)
  · split at h
    · exact absurd h (by simp)
    · split at h
      · exact absurd h (by simp)
      · split at h
        · exact absurd h (by simp)
        · split at h
          · exact absurd h (by simp)
          · simp only [Except.ok.injEq] at h
            subst h
            rename_i notFailure notPast _
            exact ⟨rfl, by omega⟩

/-! ## Executable witnesses and hostiles

Two width-3 parents (two ordinary outcomes and a failure coordinate each, the
shape decision 0029 item 7 ships). Each `native_decide` is a proof about this
model, not a claim about an SBF adapter. -/

instance {ε α : Type} [DecidableEq ε] [DecidableEq α] : DecidableEq (Except ε α) :=
  fun left right =>
    match left, right with
    | .ok a, .ok b =>
        if h : a = b then isTrue (by rw [h]) else isFalse (by intro e; cases e; exact h rfl)
    | .error a, .error b =>
        if h : a = b then isTrue (by rw [h]) else isFalse (by intro e; cases e; exact h rfl)
    | .ok _, .error _ => isFalse (by intro e; cases e)
    | .error _, .ok _ => isFalse (by intro e; cases e)

namespace Examples

def parentA : ParentRef := { marketId := 0xA, generation := 1, productRecordDigest := 0xA1, ordinaryCount := 2 }
def parentB : ParentRef := { marketId := 0xB, generation := 1, productRecordDigest := 0xB1, ordinaryCount := 2 }

def certificate (ref : ParentRef) (selector : Nat) : ParentCertificate := {
  marketId := ref.marketId
  generation := ref.generation
  productRecordDigest := ref.productRecordDigest
  ordinaryCount := ref.ordinaryCount
  selector
  terminal := true
}

/-- `A × B`: four cells and a failure coordinate, width five. -/
def product : ProductShape := { parentA, parentB }

example : product.found? = .ok product := by native_decide
example : product.cells = 4 ∧ product.width = 5 ∧ product.failureSelector = 4 := by native_decide

/-- Both ordinary: `(1, 0)` is cell `2`. -/
example : settleProduct product (some (certificate parentA 1)) (some (certificate parentB 0)) =
    .ok 2 := by native_decide

/-- `A` on its failure coordinate: the child fails, whatever `B` said. -/
example : settleProduct product (some (certificate parentA 2)) (some (certificate parentB 0)) =
    .ok 4 := by native_decide

/-- The failure terminal refunds one unit to every cell and nothing to the
failure coordinate; every terminal partitions the scale `4 · unit`. -/
example : childPayoutVector 4 7 4 = [7, 7, 7, 7, 0] := by native_decide
example : childPayoutVector 4 7 2 = [0, 0, 28, 0, 0] := by native_decide
example : (List.range 5).all fun selector => (childPayoutVector 4 7 selector).sum = 28 := by
  native_decide

/-- `B | A = 0`: two branch cells, the off-condition cell `2`, failure `3`. -/
def conditional : ConditionalShape := { parentA, condition := 0, parentB }

example : conditional.found? = .ok conditional := by native_decide
example : conditional.width = 4 ∧ conditional.offConditionCell = 2 ∧
    conditional.failureSelector = 3 := by native_decide

/-- On the condition branch the conditional selector is the product cell less
the row offset `0 · 2`. -/
example : settleConditional conditional (some (certificate parentA 0)) (some (certificate parentB 1)) =
    .ok 1 := by native_decide
example : settleProduct product (some (certificate parentA 0)) (some (certificate parentB 1)) =
    .ok 1 := by native_decide

/-- Off the condition the conditional settles with `B` still live. -/
example : settleConditional conditional (some (certificate parentA 1)) none = .ok 2 := by
  native_decide

/-- On the condition branch it cannot. -/
example : settleConditional conditional (some (certificate parentA 0)) none =
    .error .parentNotTerminal := by native_decide

/-- Off the condition, `B`'s outage is not the child's. -/
example : settleConditional conditional (some (certificate parentA 1)) (some (certificate parentB 2)) =
    .ok 2 := by native_decide

/-- On it, it is. -/
example : settleConditional conditional (some (certificate parentA 0)) (some (certificate parentB 2)) =
    .ok 3 := by native_decide

/-- HOSTILE: a child settling before a parent. -/
example : settleProduct product none (some (certificate parentB 0)) = .error .parentNotTerminal := by
  native_decide
example : settleProduct product (some (certificate parentA 0)) none = .error .parentNotTerminal := by
  native_decide

/-- HOSTILE: a liveness certificate is not a terminal. -/
example : settleProduct product (some { certificate parentA 0 with terminal := false })
    (some (certificate parentB 0)) = .error .parentNotTerminal := by native_decide

/-- HOSTILE: a parent replaced after founding. -/
example : settleProduct product (some { certificate parentA 0 with generation := 2 })
    (some (certificate parentB 0)) = .error .parentGenerationMismatch := by native_decide

/-- HOSTILE: a certificate of a stranger. -/
example : settleProduct product (some { certificate parentA 0 with marketId := 0xC })
    (some (certificate parentB 0)) = .error .wrongParent := by native_decide

/-- HOSTILE: a parent whose Product record moved. -/
example : settleProduct product (some { certificate parentA 0 with productRecordDigest := 0xA2 })
    (some (certificate parentB 0)) = .error .parentRecordMismatch := by native_decide

/-- HOSTILE: a selector past the parent's width. -/
example : settleProduct product (some (certificate parentA 3)) (some (certificate parentB 0)) =
    .error .selectorOutOfRange := by native_decide

/-- HOSTILE: a refund arm keyed on the parent's failure coordinate. -/
example : ({ parentA, condition := 2, parentB } : ConditionalShape).found? =
    .error .conditionOnFailure := by native_decide
example : ({ parentA, condition := 3, parentB } : ConditionalShape).found? =
    .error .conditionOutOfRange := by native_decide

/-- HOSTILE: a market crossed with itself. -/
example : ({ parentA, parentB := parentA } : ProductShape).found? = .error .sameParent := by
  native_decide

/-- The bank cap: `7 × 8` cells fit at width 57; `8 × 8` do not at width 65. -/
def sevenA : ParentRef := { parentA with ordinaryCount := 7 }
def eightA : ParentRef := { parentA with ordinaryCount := 8 }
def eightB : ParentRef := { parentB with ordinaryCount := 8 }
def sevenByEight : ProductShape := { parentA := sevenA, parentB := eightB }
def eightByEight : ProductShape := { parentA := eightA, parentB := eightB }

example : sevenByEight.found? = .ok sevenByEight ∧ sevenByEight.width = 57 := by native_decide
example : eightByEight.found? = .error .widthOverflow ∧ eightByEight.width = 65 := by
  native_decide

/-- A product clearing at scale 100: one buyer per cell at 35/20/30/15 with
the failure coordinate at zero, jointly funding one complete set; the rows
read `55 / 45` for `A`, the columns `65 / 35` for `B`, and
`P(B = 1 | A = 0) = 20 / 55`. -/
def clearing : Clearing := {
  outcomeCount := 5
  scale := 100
  prices := [35, 20, 30, 15, 0]
  fills := [
    { order := JointClearing.Examples.buy 1 0 1 35 5, lots := 1 },
    { order := JointClearing.Examples.buy 2 1 1 20 5, lots := 1 },
    { order := JointClearing.Examples.buy 3 2 1 30 5, lots := 1 },
    { order := JointClearing.Examples.buy 4 3 1 15 5, lots := 1 }
  ]
  sets := 1
}

example : clearing.valid = true := by native_decide
example : JointClearing.collectedQuote clearing clearing.fills = 100 := by native_decide
example : rowPrice clearing product 0 = 55 ∧ rowPrice clearing product 1 = 45 := by native_decide
example : columnPrice clearing product 0 = 65 ∧ columnPrice clearing product 1 = 35 := by
  native_decide
example : conditionalRead clearing product 0 1 = (20, 55) := by native_decide
example : sumRange product.rows (rowPrice clearing product) + clearing.price 4 = 100 := by
  native_decide

/-- HOSTILE: a product price vector not summing to the scale is the joint
clearing's own refusal; nothing here relaxes it. -/
example : ({ clearing with prices := [30, 20, 25, 15, 5] } : Clearing).valid = false := by
  native_decide

end Examples

end DClutch.ConditionalMarket
