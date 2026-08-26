import DClutchSemantics.ProductPayoffV2
import DClutchSemantics.Codec
import Std.Tactic

/-!
# Runtime-width nonnegative liability bases

This module separates the economic theorem for a finite nonnegative integer
partition of unity from any fixed physical width.  A basis evaluation returns
one nonnegative integer payout per elementary claim and the payouts sum to the
positive collateral scale `Q`.

`cappedRampComplementFloorBoundaryV2` is the sole apportionment boundary in the
two-claim ramp profile.  It is definitionally the Product V2 final interpolation
floor.  The second claim receives the exact integer complement, so no second
rounding decision or unclassified residue exists.

The physical Rust profile uses bounded integers and a fixed hostile-decodable
request.  Those physical bounds are not premises of the mathematical theorems
below.
-/

namespace DClutch.LiabilityBasisV2

/-- Runtime-width dot product. A physical caller must prove equal lengths. -/
def liability : List Nat → List Nat → Nat
  | supply :: supplies, payout :: payouts =>
      supply * payout + liability supplies payouts
  | _, _ => 0

/-- Add the same complete-set quantity to every elementary claim supply. -/
def splitSupply (quantity : Nat) (supplies : List Nat) : List Nat :=
  supplies.map (fun supply => supply + quantity)

/-- Semantic contract for one finite nonnegative integer partition of unity. -/
structure Basis (Result : Type) where
  width : Nat
  scale : Nat
  widthPositive : 0 < width
  scalePositive : 0 < scale
  evaluate : Result → List Nat
  exactWidth : ∀ result, (evaluate result).length = width
  payoutBounded : ∀ result payout, payout ∈ evaluate result → payout ≤ scale
  partitionUnity : ∀ result, (evaluate result).sum = scale

theorem liability_split
    (quantity : Nat) (supplies payouts : List Nat)
    (sameWidth : supplies.length = payouts.length) :
    liability (splitSupply quantity supplies) payouts =
      liability supplies payouts + quantity * payouts.sum := by
  induction supplies generalizing payouts with
  | nil =>
      cases payouts <;> simp_all [liability, splitSupply]
  | cons supply supplies induction =>
      cases payouts with
      | nil => simp at sameWidth
      | cons payout payouts =>
          have tailWidth : supplies.length = payouts.length := by
            simpa using sameWidth
          simp only [splitSupply, List.map_cons, liability, List.sum_cons]
          change (supply + quantity) * payout +
              liability (splitSupply quantity supplies) payouts = _
          rw [induction payouts tailWidth]
          simp only [Nat.add_mul, Nat.mul_add]
          omega

/-- A complete-set split increases liability at every result by exactly
`quantity * Q`; no maximization argument or categorical one-hot premise is
needed. -/
theorem Basis.liability_split
    (basis : Basis Result) (result : Result)
    (quantity : Nat) (supplies : List Nat)
    (sameWidth : supplies.length = basis.width) :
    liability (splitSupply quantity supplies) (basis.evaluate result) =
      liability supplies (basis.evaluate result) + quantity * basis.scale := by
  rw [DClutch.LiabilityBasisV2.liability_split
    quantity supplies (basis.evaluate result)]
  · rw [basis.partitionUnity result]
  · rw [basis.exactWidth result]
    exact sameWidth

/-- Pointwise collateralization at one result.  Global solvency quantifies this
predicate over the complete terminal-result domain. -/
def SolventAt (hoard : Nat) (supplies payouts : List Nat) : Prop :=
  liability supplies payouts ≤ hoard

/-- Solvency over the complete terminal-result domain, without assuming that
the domain is enumerable in this theorem. -/
def Basis.GloballySolvent
    (basis : Basis Result) (hoard : Nat) (supplies : List Nat) : Prop :=
  ∀ result, SolventAt hoard supplies (basis.evaluate result)

/-- Crediting `quantity * Q` collateral alongside a complete-set split
preserves pointwise solvency for every result. -/
theorem Basis.split_preserves_solvency
    (basis : Basis Result) (result : Result)
    (quantity hoard : Nat) (supplies : List Nat)
    (sameWidth : supplies.length = basis.width)
    (solvent : SolventAt hoard supplies (basis.evaluate result)) :
    SolventAt (hoard + quantity * basis.scale)
      (splitSupply quantity supplies) (basis.evaluate result) := by
  unfold SolventAt at solvent ⊢
  rw [basis.liability_split result quantity supplies sameWidth]
  exact Nat.add_le_add_right solvent _

/-- The same exact split/collateral delta preserves the liability bound at
every terminal result simultaneously. -/
theorem Basis.split_preserves_global_solvency
    (basis : Basis Result) (quantity hoard : Nat) (supplies : List Nat)
    (sameWidth : supplies.length = basis.width)
    (solvent : basis.GloballySolvent hoard supplies) :
    basis.GloballySolvent (hoard + quantity * basis.scale)
      (splitSupply quantity supplies) := by
  intro result
  exact basis.split_preserves_solvency result quantity hoard supplies
    sameWidth (solvent result)

/-! ## Pointwise supply algebra

Complete-set split, complete-set merge, claim transfer, and terminal
redemption are four uses of one runtime-width pointwise supply algebra.
Conservation is proved once here so that no preservation theorem below has to
repeat a per-operation induction.
-/

/-- Aggregate two holders' supplies coordinate by coordinate. -/
def pointwiseAdd : List Nat → List Nat → List Nat
  | left :: lefts, right :: rights => (left + right) :: pointwiseAdd lefts rights
  | _, _ => []

/-- Retire one supply vector from another coordinate by coordinate.
`Dominates` rules out truncating `Nat` subtraction, so no hidden residue can
appear at any coordinate. -/
def pointwiseSub : List Nat → List Nat → List Nat
  | left :: lefts, right :: rights => (left - right) :: pointwiseSub lefts rights
  | _, _ => []

/-- Equal runtime width, with every retired quantity backed by outstanding
supply at the same claim coordinate. -/
def Dominates : List Nat → List Nat → Prop
  | [], [] => True
  | left :: lefts, right :: rights => right ≤ left ∧ Dominates lefts rights
  | _, _ => False

/-- Total coordinate read. An out-of-range coordinate reads zero. -/
def entryAt (values : List Nat) (index : Nat) : Nat := values[index]?.getD 0

@[simp] theorem entryAt_nil (index : Nat) : entryAt [] index = 0 := rfl

@[simp] theorem entryAt_cons_zero (value : Nat) (values : List Nat) :
    entryAt (value :: values) 0 = value := rfl

@[simp] theorem entryAt_cons_succ (value : Nat) (values : List Nat) (index : Nat) :
    entryAt (value :: values) (index + 1) = entryAt values index := rfl

theorem liability_nil_payouts (supplies : List Nat) :
    liability supplies [] = 0 := by
  cases supplies <;> rfl

theorem liability_replicate_zero_supplies (width : Nat) (payouts : List Nat) :
    liability (List.replicate width 0) payouts = 0 := by
  induction width generalizing payouts with
  | zero => rfl
  | succ width induction =>
      cases payouts with
      | nil => rfl
      | cons payout payouts =>
          rw [List.replicate_succ]
          simp only [liability, Nat.zero_mul, Nat.zero_add]
          exact induction payouts

theorem liability_replicate_zero_payouts (supplies : List Nat) (width : Nat) :
    liability supplies (List.replicate width 0) = 0 := by
  induction supplies generalizing width with
  | nil => rfl
  | cons _ values induction =>
      cases width with
      | zero => rfl
      | succ width =>
          rw [List.replicate_succ]
          simp only [liability, Nat.mul_zero, Nat.zero_add]
          exact induction width

/-- Liability is additive across holders: aggregate exposure is the sum of the
holders' exposures at every result. -/
theorem liability_pointwiseAdd
    (left right payouts : List Nat)
    (sameWidth : left.length = right.length) :
    liability (pointwiseAdd left right) payouts =
      liability left payouts + liability right payouts := by
  induction left generalizing right payouts with
  | nil =>
      cases right with
      | nil => cases payouts <;> rfl
      | cons _ _ => simp at sameWidth
  | cons leftValue leftValues induction =>
      cases right with
      | nil => simp at sameWidth
      | cons rightValue rightValues =>
          have tailWidth : leftValues.length = rightValues.length := by
            simpa using sameWidth
          cases payouts with
          | nil => rfl
          | cons payout payouts =>
              simp only [pointwiseAdd, liability]
              rw [induction rightValues payouts tailWidth, Nat.add_mul]
              omega

/-- Exact retirement conservation: what a retired vector removes from the
liability is exactly the liability of the retired vector itself. -/
theorem liability_pointwiseSub
    (supplies retired payouts : List Nat)
    (backed : Dominates supplies retired) :
    liability (pointwiseSub supplies retired) payouts +
      liability retired payouts = liability supplies payouts := by
  induction supplies generalizing retired payouts with
  | nil =>
      cases retired with
      | nil => cases payouts <;> rfl
      | cons _ _ => exact absurd backed (by simp [Dominates])
  | cons supply supplies induction =>
      cases retired with
      | nil => exact absurd backed (by simp [Dominates])
      | cons retiredValue retiredValues =>
          obtain ⟨bound, tailBacked⟩ := backed
          cases payouts with
          | nil => rfl
          | cons payout payouts =>
              have tail := induction retiredValues payouts tailBacked
              have step : (supply - retiredValue) * payout + retiredValue * payout
                  = supply * payout := by
                rw [← Nat.add_mul, Nat.sub_add_cancel bound]
              simp only [pointwiseSub, liability]
              omega

theorem pointwiseSub_self (supplies : List Nat) :
    pointwiseSub supplies supplies = List.replicate supplies.length 0 := by
  induction supplies with
  | nil => rfl
  | cons value values induction =>
      simp only [pointwiseSub, List.length_cons, List.replicate_succ,
        Nat.sub_self, List.cons.injEq, true_and]
      exact induction

theorem Dominates.refl (supplies : List Nat) : Dominates supplies supplies := by
  induction supplies with
  | nil => trivial
  | cons _ _ induction => exact ⟨Nat.le_refl _, induction⟩

/-! ## Complete-set merge -/

/-- Retire the same complete-set quantity from every elementary claim supply. -/
def mergeSupply (quantity : Nat) (supplies : List Nat) : List Nat :=
  supplies.map (fun supply => supply - quantity)

/-- A complete-set merge is admissible only when every elementary supply backs
the retired quantity; otherwise a coordinate would truncate. -/
def MergeAdmissible (quantity : Nat) (supplies : List Nat) : Prop :=
  ∀ supply ∈ supplies, quantity ≤ supply

theorem mergeSupply_length (quantity : Nat) (supplies : List Nat) :
    (mergeSupply quantity supplies).length = supplies.length := by
  simp [mergeSupply]

theorem mergeSupply_splitSupply (quantity : Nat) (supplies : List Nat) :
    mergeSupply quantity (splitSupply quantity supplies) = supplies := by
  induction supplies with
  | nil => rfl
  | cons value values induction =>
      simp only [splitSupply, mergeSupply, List.map_cons, Nat.add_sub_cancel,
        List.cons.injEq, true_and]
      simpa [splitSupply, mergeSupply] using induction

theorem splitSupply_mergeSupply
    (quantity : Nat) (supplies : List Nat)
    (admissible : MergeAdmissible quantity supplies) :
    splitSupply quantity (mergeSupply quantity supplies) = supplies := by
  induction supplies with
  | nil => rfl
  | cons value values induction =>
      have bound : quantity ≤ value := admissible value (by simp)
      have tail : MergeAdmissible quantity values := by
        intro supply member
        exact admissible supply (by simp [member])
      simp only [splitSupply, mergeSupply, List.map_cons, List.cons.injEq]
      refine ⟨by omega, ?_⟩
      simpa [splitSupply, mergeSupply] using induction tail

/-- A complete-set merge lowers liability at every result by exactly
`quantity * Q`, with no truncation and no residue. -/
theorem liability_merge
    (quantity : Nat) (supplies payouts : List Nat)
    (sameWidth : supplies.length = payouts.length)
    (admissible : MergeAdmissible quantity supplies) :
    liability (mergeSupply quantity supplies) payouts + quantity * payouts.sum =
      liability supplies payouts := by
  induction supplies generalizing payouts with
  | nil =>
      cases payouts with
      | nil => rfl
      | cons _ _ => simp at sameWidth
  | cons supply supplies induction =>
      cases payouts with
      | nil => simp at sameWidth
      | cons payout payouts =>
          have tailWidth : supplies.length = payouts.length := by
            simpa using sameWidth
          have bound : quantity ≤ supply := admissible supply (by simp)
          have tailAdmissible : MergeAdmissible quantity supplies := by
            intro value member
            exact admissible value (by simp [member])
          have tail := induction payouts tailWidth tailAdmissible
          have step : (supply - quantity) * payout + quantity * payout
              = supply * payout := by
            rw [← Nat.add_mul, Nat.sub_add_cancel bound]
          simp only [mergeSupply, List.map_cons, liability, List.sum_cons,
            Nat.mul_add]
          simp only [mergeSupply] at tail
          omega

/-- The complete-set merge form of the generalized liability identity. -/
theorem Basis.liability_merge
    (basis : Basis Result) (result : Result)
    (quantity : Nat) (supplies : List Nat)
    (sameWidth : supplies.length = basis.width)
    (admissible : MergeAdmissible quantity supplies) :
    liability (mergeSupply quantity supplies) (basis.evaluate result) +
      quantity * basis.scale = liability supplies (basis.evaluate result) := by
  have width : supplies.length = (basis.evaluate result).length := by
    rw [basis.exactWidth result]; exact sameWidth
  rw [← basis.partitionUnity result]
  exact DClutch.LiabilityBasisV2.liability_merge quantity supplies
    (basis.evaluate result) width admissible

/-- Releasing exactly `quantity * Q` collateral alongside a complete-set merge
preserves pointwise solvency at every result. -/
theorem Basis.merge_preserves_solvency
    (basis : Basis Result) (result : Result)
    (quantity hoard : Nat) (supplies : List Nat)
    (sameWidth : supplies.length = basis.width)
    (admissible : MergeAdmissible quantity supplies)
    (solvent : SolventAt hoard supplies (basis.evaluate result)) :
    SolventAt (hoard - quantity * basis.scale)
      (mergeSupply quantity supplies) (basis.evaluate result) := by
  unfold SolventAt at solvent ⊢
  have identity := basis.liability_merge result quantity supplies sameWidth admissible
  omega

theorem Basis.merge_preserves_global_solvency
    (basis : Basis Result) (quantity hoard : Nat) (supplies : List Nat)
    (sameWidth : supplies.length = basis.width)
    (admissible : MergeAdmissible quantity supplies)
    (solvent : basis.GloballySolvent hoard supplies) :
    basis.GloballySolvent (hoard - quantity * basis.scale)
      (mergeSupply quantity supplies) := by
  intro result
  exact basis.merge_preserves_solvency result quantity hoard supplies
    sameWidth admissible (solvent result)

/-! ## Claim transfer -/

/-- Adjust exactly one claim coordinate; an out-of-range coordinate is inert. -/
def adjustAt (index : Nat) (change : Nat → Nat) : List Nat → List Nat
  | [] => []
  | value :: rest =>
      match index with
      | 0 => change value :: rest
      | next + 1 => value :: adjustAt next change rest

/-- Credit one claim coordinate. -/
def creditAt (index quantity : Nat) (values : List Nat) : List Nat :=
  adjustAt index (fun value => value + quantity) values

/-- Debit one claim coordinate. -/
def debitAt (index quantity : Nat) (values : List Nat) : List Nat :=
  adjustAt index (fun value => value - quantity) values

/-- A transfer or single-claim redemption is backed when the seller holds the
transferred quantity at that coordinate. -/
def TransferBacked (index quantity : Nat) (seller : List Nat) : Prop :=
  quantity ≤ entryAt seller index

/-- A backed transfer moves claims between holders without changing aggregate
outstanding supply at any coordinate. -/
theorem trade_preserves_aggregate
    (index quantity : Nat) (seller buyer : List Nat)
    (sameWidth : seller.length = buyer.length)
    (backed : TransferBacked index quantity seller) :
    pointwiseAdd (debitAt index quantity seller) (creditAt index quantity buyer) =
      pointwiseAdd seller buyer := by
  induction seller generalizing buyer index with
  | nil =>
      cases buyer with
      | nil => simp [pointwiseAdd, debitAt, creditAt, adjustAt]
      | cons _ _ => simp at sameWidth
  | cons sellerValue sellerValues induction =>
      cases buyer with
      | nil => simp at sameWidth
      | cons buyerValue buyerValues =>
          have tailWidth : sellerValues.length = buyerValues.length := by
            simpa using sameWidth
          cases index with
          | zero =>
              have bound : quantity ≤ sellerValue := by
                simpa [TransferBacked, entryAt] using backed
              simp only [debitAt, creditAt, adjustAt, pointwiseAdd,
                List.cons.injEq, and_true]
              omega
          | succ index =>
              have tailBacked : TransferBacked index quantity sellerValues := by
                simpa [TransferBacked, entryAt] using backed
              simp only [debitAt, creditAt, adjustAt, pointwiseAdd,
                List.cons.injEq, true_and]
              simpa [debitAt, creditAt] using
                induction _ _ tailWidth tailBacked

/-- A backed transfer changes no holder-independent liability, so global
solvency is preserved with the Hoard untouched. -/
theorem Basis.trade_preserves_global_solvency
    (basis : Basis Result) (hoard index quantity : Nat) (seller buyer : List Nat)
    (sameWidth : seller.length = buyer.length)
    (backed : TransferBacked index quantity seller)
    (solvent : basis.GloballySolvent hoard (pointwiseAdd seller buyer)) :
    basis.GloballySolvent hoard
      (pointwiseAdd (debitAt index quantity seller)
        (creditAt index quantity buyer)) := by
  rw [trade_preserves_aggregate index quantity seller buyer sameWidth backed]
  exact solvent

/-! ## Terminal payout and redemption -/

/-- Exact collateral released by redeeming `redeemed` at one admitted terminal
result. It is the liability of the redeemed vector itself. -/
def terminalPayout (redeemed payouts : List Nat) : Nat := liability redeemed payouts

/-- Terminal redemption conservation: residual liability plus the exact
terminal payout is the incoming liability. -/
theorem Basis.terminal_payout_conserves
    (basis : Basis Result) (result : Result) (supplies redeemed : List Nat)
    (backed : Dominates supplies redeemed) :
    liability (pointwiseSub supplies redeemed) (basis.evaluate result) +
      terminalPayout redeemed (basis.evaluate result) =
        liability supplies (basis.evaluate result) :=
  liability_pointwiseSub supplies redeemed (basis.evaluate result) backed

theorem Basis.terminal_payout_le_liability
    (basis : Basis Result) (result : Result) (supplies redeemed : List Nat)
    (backed : Dominates supplies redeemed) :
    terminalPayout redeemed (basis.evaluate result) ≤
      liability supplies (basis.evaluate result) := by
  have conserved := basis.terminal_payout_conserves result supplies redeemed backed
  omega

/-- Paying the exact terminal payout out of the Hoard preserves solvency at
the resolved result. -/
theorem Basis.redemption_preserves_solvency
    (basis : Basis Result) (result : Result) (hoard : Nat)
    (supplies redeemed : List Nat)
    (backed : Dominates supplies redeemed)
    (solvent : SolventAt hoard supplies (basis.evaluate result)) :
    SolventAt (hoard - terminalPayout redeemed (basis.evaluate result))
      (pointwiseSub supplies redeemed) (basis.evaluate result) := by
  unfold SolventAt at solvent ⊢
  have conserved := basis.terminal_payout_conserves result supplies redeemed backed
  omega

/-- Redeeming the whole outstanding supply pays exactly the terminal liability
and leaves zero residual liability. -/
theorem Basis.full_redemption_pays_exact_liability
    (basis : Basis Result) (result : Result) (supplies : List Nat) :
    terminalPayout supplies (basis.evaluate result) =
      liability supplies (basis.evaluate result) := rfl

theorem Basis.full_redemption_zeroes_liability
    (basis : Basis Result) (result : Result) (supplies : List Nat) :
    liability (pointwiseSub supplies supplies) (basis.evaluate result) = 0 := by
  rw [pointwiseSub_self]
  exact liability_replicate_zero_supplies _ _

/-- Single-claim terminal redemption: the exact statement the physical Claims
planner executes. -/
theorem liability_debitAt
    (index quantity : Nat) (supplies payouts : List Nat)
    (backed : TransferBacked index quantity supplies) :
    liability (debitAt index quantity supplies) payouts +
      quantity * entryAt payouts index = liability supplies payouts := by
  induction supplies generalizing payouts index with
  | nil =>
      have zero : quantity = 0 := by
        simpa [TransferBacked, entryAt] using backed
      subst zero
      cases payouts <;> cases index <;>
        simp [debitAt, adjustAt, liability, entryAt]
  | cons supply supplies induction =>
      cases payouts with
      | nil =>
          cases index <;>
            simp [debitAt, adjustAt, liability_nil_payouts, entryAt, liability]
      | cons payout payouts =>
          cases index with
          | zero =>
              have bound : quantity ≤ supply := by
                simpa [TransferBacked, entryAt] using backed
              have step : (supply - quantity) * payout + quantity * payout
                  = supply * payout := by
                rw [← Nat.add_mul, Nat.sub_add_cancel bound]
              simp only [debitAt, adjustAt, liability, entryAt,
                List.getElem?_cons_zero, Option.getD_some]
              omega
          | succ index =>
              have tailBacked : TransferBacked index quantity supplies := by
                simpa [TransferBacked, entryAt] using backed
              have tail := induction index payouts tailBacked
              simp only [debitAt, adjustAt, liability, entryAt,
                List.getElem?_cons_succ]
              simp only [debitAt, entryAt] at tail
              omega

/-! ## Generalized solvency without domain enumeration -/

/-- Peak outstanding supply across the runtime width. -/
def peakSupply : List Nat → Nat
  | [] => 0
  | value :: rest => Nat.max value (peakSupply rest)

theorem le_peakSupply (supplies : List Nat) (value : Nat)
    (member : value ∈ supplies) : value ≤ peakSupply supplies := by
  induction supplies with
  | nil => simp at member
  | cons head tail induction =>
      rcases List.mem_cons.1 member with rfl | member
      · exact Nat.le_max_left _ _
      · exact Nat.le_trans (induction member) (Nat.le_max_right _ _)

theorem peakSupply_mem (supplies : List Nat) (nonempty : supplies ≠ []) :
    peakSupply supplies ∈ supplies := by
  induction supplies with
  | nil => exact absurd rfl nonempty
  | cons head tail induction =>
      by_cases empty : tail = []
      · subst empty
        simp [peakSupply]
      · have member := induction empty
        rcases Nat.le_total (peakSupply tail) head with bound | bound
        · have : Nat.max head (peakSupply tail) = head := by
            simp only [Nat.max_def]; split <;> omega
          simp [peakSupply, this]
        · have : Nat.max head (peakSupply tail) = peakSupply tail := by
            simp only [Nat.max_def]; split <;> omega
          simp only [peakSupply, this]
          exact List.mem_cons_of_mem _ member

/-- Arithmetic boundedness: liability never exceeds peak supply times the
total payout of the result. -/
theorem liability_le_peak_mul_sum (supplies payouts : List Nat) :
    liability supplies payouts ≤ peakSupply supplies * payouts.sum := by
  induction supplies generalizing payouts with
  | nil => cases payouts <;> simp [liability, peakSupply]
  | cons supply supplies induction =>
      cases payouts with
      | nil => simp [liability_nil_payouts]
      | cons payout payouts =>
          have tail := induction payouts
          have headBound : supply * payout ≤ peakSupply (supply :: supplies) * payout :=
            Nat.mul_le_mul_right _ (Nat.le_max_left _ _)
          have tailBound :
              peakSupply supplies * payouts.sum
                ≤ peakSupply (supply :: supplies) * payouts.sum :=
            Nat.mul_le_mul_right _ (Nat.le_max_right _ _)
          simp only [liability, List.sum_cons, Nat.mul_add]
          omega

/-- Exact liability at any admitted result is bounded by `Q * peak(T)`. -/
theorem Basis.liability_le_peak_mul_scale
    (basis : Basis Result) (result : Result) (supplies : List Nat) :
    liability supplies (basis.evaluate result) ≤ peakSupply supplies * basis.scale := by
  have bound := liability_le_peak_mul_sum supplies (basis.evaluate result)
  rw [basis.partitionUnity result] at bound
  exact bound

/-- `Q * peak(T) <= H` certifies global solvency for every basis, without
enumerating the terminal-result domain. -/
theorem Basis.peak_bound_globally_solvent
    (basis : Basis Result) (hoard : Nat) (supplies : List Nat)
    (covered : peakSupply supplies * basis.scale ≤ hoard) :
    basis.GloballySolvent hoard supplies := by
  intro result
  exact Nat.le_trans (basis.liability_le_peak_mul_scale result supplies) covered


/-! ## Hostile partition admission -/

/-- Hostile partition checker mirroring the physical kernel exactly: a payout
vector is admitted only when it is nonempty, the scale is positive, every
payout is at most `Q`, and the payouts sum to exactly `Q`. -/
def validPartition (payouts : List Nat) (scale : Nat) : Bool :=
  !payouts.isEmpty && decide (0 < scale) &&
    payouts.all (fun payout => decide (payout ≤ scale)) &&
    decide (payouts.sum = scale)

theorem validPartition_sum
    (payouts : List Nat) (scale : Nat)
    (valid : validPartition payouts scale = true) : payouts.sum = scale := by
  unfold validPartition at valid
  simp only [Bool.and_eq_true, decide_eq_true_eq] at valid
  exact valid.2

theorem validPartition_bounded
    (payouts : List Nat) (scale payout : Nat)
    (valid : validPartition payouts scale = true)
    (member : payout ∈ payouts) : payout ≤ scale := by
  unfold validPartition at valid
  simp only [Bool.and_eq_true, decide_eq_true_eq, List.all_eq_true] at valid
  exact valid.1.2 payout member

theorem validPartition_scalePositive
    (payouts : List Nat) (scale : Nat)
    (valid : validPartition payouts scale = true) : 0 < scale := by
  unfold validPartition at valid
  simp only [Bool.and_eq_true, decide_eq_true_eq] at valid
  exact valid.1.1.2

/-- **A non-summing evaluator is refused.** No payout vector whose atoms fail
to sum to exactly `Q` can be admitted. -/
theorem not_validPartition_of_sum_ne
    (payouts : List Nat) (scale : Nat) (mismatch : payouts.sum ≠ scale) :
    validPartition payouts scale = false := by
  cases valid : validPartition payouts scale with
  | false => rfl
  | true => exact absurd (validPartition_sum payouts scale valid) mismatch

/-- A zero-scale basis is refused. -/
theorem not_validPartition_of_zero_scale (payouts : List Nat) :
    validPartition payouts 0 = false := by
  cases valid : validPartition payouts 0 with
  | false => rfl
  | true => exact absurd (validPartition_scalePositive payouts 0 valid) (by omega)

/-- A zero-width basis is refused. -/
theorem not_validPartition_of_zero_width (scale : Nat) :
    validPartition [] scale = false := by
  unfold validPartition
  simp

/-- Every basis evaluation is an admitted partition of its own positive
scale, so the checker never refuses an honest evaluator. -/
theorem Basis.validPartition_evaluate
    (basis : Basis Result) (result : Result) :
    validPartition (basis.evaluate result) basis.scale = true := by
  have nonempty : basis.evaluate result ≠ [] := by
    intro empty
    have width := basis.exactWidth result
    rw [empty, List.length_nil] at width
    have positive := basis.widthPositive
    omega
  unfold validPartition
  simp only [Bool.and_eq_true, decide_eq_true_eq, List.all_eq_true,
    Bool.not_eq_eq_eq_not, Bool.not_true, List.isEmpty_eq_false_iff]
  refine ⟨⟨⟨nonempty, basis.scalePositive⟩, ?_⟩, basis.partitionUnity result⟩
  intro payout member
  simpa using basis.payoutBounded result payout member

/-! ## Categorical embedding -/

/-- Runtime-width categorical one-hot payout. Out-of-range defensive indices
produce all zeros; `Fin width` construction rules them out below. -/
def categoricalPayoutAt : Nat → Nat → List Nat
  | 0, _ => []
  | width + 1, 0 => 1 :: List.replicate width 0
  | width + 1, winner + 1 => 0 :: categoricalPayoutAt width winner

def categoricalPayout (width : Nat) (winner : Fin width) : List Nat :=
  categoricalPayoutAt width winner.val

theorem categoricalPayoutAt_length (width winner : Nat) :
    (categoricalPayoutAt width winner).length = width := by
  induction width generalizing winner with
  | zero => simp [categoricalPayoutAt]
  | succ width induction =>
      cases winner with
      | zero => simp [categoricalPayoutAt]
      | succ winner => simp [categoricalPayoutAt, induction]

theorem categoricalPayoutAt_bounded
    (width winner payout : Nat)
    (member : payout ∈ categoricalPayoutAt width winner) : payout ≤ 1 := by
  induction width generalizing winner with
  | zero => simp [categoricalPayoutAt] at member
  | succ width induction =>
      cases winner with
      | zero =>
          simp only [categoricalPayoutAt, List.mem_cons,
            List.mem_replicate] at member
          rcases member with rfl | ⟨_, rfl⟩ <;> omega
      | succ winner =>
          simp only [categoricalPayoutAt, List.mem_cons] at member
          rcases member with rfl | member
          · omega
          · exact induction winner member

theorem categoricalPayoutAt_sum
    (width winner : Nat) (inRange : winner < width) :
    (categoricalPayoutAt width winner).sum = 1 := by
  induction width generalizing winner with
  | zero => omega
  | succ width induction =>
      cases winner with
      | zero => simp [categoricalPayoutAt]
      | succ winner =>
          simp only [categoricalPayoutAt, List.sum_cons, Nat.zero_add]
          exact induction winner (by omega)

theorem categoricalPayout_length (width : Nat) (winner : Fin width) :
    (categoricalPayout width winner).length = width := by
  exact categoricalPayoutAt_length width winner.val

theorem categoricalPayout_bounded
    (width : Nat) (winner : Fin width) (payout : Nat)
    (member : payout ∈ categoricalPayout width winner) : payout ≤ 1 := by
  exact categoricalPayoutAt_bounded width winner.val payout member

theorem categoricalPayout_sum (width : Nat) (winner : Fin width) :
    (categoricalPayout width winner).sum = 1 := by
  exact categoricalPayoutAt_sum width winner.val winner.isLt

/-- Categorical claims embed exactly as the `Q = 1` one-hot basis. -/
def categoricalBasis (width : Nat) (widthPositive : 0 < width) : Basis (Fin width) := {
  width
  scale := 1
  widthPositive
  scalePositive := by omega
  evaluate := categoricalPayout width
  exactWidth := categoricalPayout_length width
  payoutBounded := categoricalPayout_bounded width
  partitionUnity := categoricalPayout_sum width
}


/-- Categorical liability at one winner reads exactly that claim's supply. -/
theorem liability_categoricalPayoutAt (supplies : List Nat) (index : Nat) :
    liability supplies (categoricalPayoutAt supplies.length index) =
      entryAt supplies index := by
  induction supplies generalizing index with
  | nil => simp [liability]
  | cons supply supplies induction =>
      cases index with
      | zero =>
          simp only [categoricalPayoutAt, liability, entryAt_cons_zero]
          rw [liability_replicate_zero_payouts]
          omega
      | succ index =>
          simp only [categoricalPayoutAt, liability, entryAt_cons_succ]
          rw [induction index]
          omega

theorem categoricalBasis_liability
    (width : Nat) (widthPositive : 0 < width) (supplies : List Nat)
    (sameWidth : supplies.length = width) (winner : Fin width) :
    liability supplies ((categoricalBasis width widthPositive).evaluate winner) =
      entryAt supplies winner.val := by
  have evaluate : (categoricalBasis width widthPositive).evaluate winner
      = categoricalPayoutAt supplies.length winner.val := by
    rw [sameWidth]; rfl
  rw [evaluate, liability_categoricalPayoutAt]

/-- The `Q * peak(T)` envelope is attained by the categorical basis, so it is
the exact global solvency requirement and not a conservative bound. -/
theorem categoricalBasis_peak_attained
    (width : Nat) (widthPositive : 0 < width) (supplies : List Nat)
    (sameWidth : supplies.length = width) :
    ∃ winner : Fin width,
      liability supplies ((categoricalBasis width widthPositive).evaluate winner) =
        peakSupply supplies * (categoricalBasis width widthPositive).scale := by
  have nonempty : supplies ≠ [] := by
    intro empty
    rw [empty, List.length_nil] at sameWidth
    omega
  obtain ⟨index, bound, value⟩ :=
    List.mem_iff_getElem.1 (peakSupply_mem supplies nonempty)
  refine ⟨⟨index, by omega⟩, ?_⟩
  rw [categoricalBasis_liability width widthPositive supplies sameWidth ⟨index, by omega⟩]
  simp only [categoricalBasis, entryAt]
  rw [List.getElem?_eq_getElem bound, Option.getD_some, value]
  omega

/-- **Generalized solvency for categorical claims is exactly `H >= Q*peak(T)`.**
This is the mathematical content the physical Claims planner relies on when it
uses `Q * max(supply)` as the pre-resolution liability. -/
theorem categoricalBasis_globally_solvent_iff
    (width : Nat) (widthPositive : 0 < width) (hoard : Nat) (supplies : List Nat)
    (sameWidth : supplies.length = width) :
    (categoricalBasis width widthPositive).GloballySolvent hoard supplies ↔
      peakSupply supplies * (categoricalBasis width widthPositive).scale ≤ hoard := by
  constructor
  · intro solvent
    obtain ⟨winner, attained⟩ :=
      categoricalBasis_peak_attained width widthPositive supplies sameWidth
    have bound := solvent winner
    unfold SolventAt at bound
    omega
  · exact Basis.peak_bound_globally_solvent _ hoard supplies

/-! ## Two-claim capped ramp and exact complement -/

abbrev RationalCoordinate := DClutch.ProductV2.RationalCoordinate

/-- A two-claim capped-ramp profile. Knots are exact signed numerators over one
positive common denominator. -/
structure CappedRampComplement where
  scale : Nat
  knotDenominator : Nat
  leftNumerator : Int
  rightNumerator : Int
  scalePositive : 0 < scale
  knotDenominatorPositive : 0 < knotDenominator
  knotsOrdered : leftNumerator < rightNumerator

/-- **The sole capped-ramp apportionment boundary.** This is definitionally the
Product V2 final interpolation floor, merely given its liability-basis name. -/
def cappedRampComplementFloorBoundaryV2
    (scale : Nat) (elapsed width : Int) : Nat :=
  DClutch.ProductV2.interpolationFloor scale elapsed width

theorem cappedRampComplementFloorBoundaryV2_le
    (scale : Nat) (elapsed width : Int) :
    cappedRampComplementFloorBoundaryV2 scale elapsed width ≤ scale := by
  exact DClutch.ProductV2.interpolationFloor_le scale elapsed width


/-- Under the admitted interior premises the sole boundary is exactly the floor
of the positive rational interpolation; the defensive clamp is inert there. -/
theorem cappedRampComplementFloorBoundaryV2_interior
    (scale : Nat) (elapsed width : Int)
    (positiveElapsed : 0 < elapsed) (interior : elapsed < width) :
    cappedRampComplementFloorBoundaryV2 scale elapsed width
      = Int.toNat (((scale : Int) * elapsed) / width) := by
  have positiveWidth : (0 : Int) < width := by omega
  have quotientBound : Int.toNat (((scale : Int) * elapsed) / width) ≤ scale := by
    rw [Int.toNat_le]
    calc ((scale : Int) * elapsed) / width ≤ ((scale : Int) * width) / width :=
          Int.ediv_le_ediv positiveWidth
            (Int.mul_le_mul_of_nonneg_left (by omega) (by omega))
      _ = (scale : Int) := Int.mul_ediv_cancel _ (by omega)
  unfold cappedRampComplementFloorBoundaryV2 DClutch.ProductV2.interpolationFloor
  rw [if_neg (by simp only [Bool.or_eq_true, decide_eq_true_eq, not_or]; omega)]
  simp only [Nat.min_def]
  split <;> omega

/-- **Rounding direction, first half.** The primary claim is never apportioned
more than its exact rational share: the sole boundary rounds down. -/
theorem cappedRampComplementFloorBoundaryV2_never_rounds_up
    (scale : Nat) (elapsed width : Int)
    (positiveElapsed : 0 < elapsed) (interior : elapsed < width) :
    (cappedRampComplementFloorBoundaryV2 scale elapsed width : Int) * width
      ≤ (scale : Int) * elapsed := by
  have positiveWidth : (0 : Int) < width := by omega
  have productNonneg : (0 : Int) ≤ (scale : Int) * elapsed :=
    Int.mul_nonneg (by omega) (by omega)
  have nonneg : (0 : Int) ≤ ((scale : Int) * elapsed) / width :=
    Int.ediv_nonneg productNonneg (by omega)
  have expand := Int.mul_ediv_add_emod ((scale : Int) * elapsed) width
  have remainderNonneg : (0 : Int) ≤ ((scale : Int) * elapsed) % width :=
    Int.emod_nonneg _ (by omega)
  have commuted : (((scale : Int) * elapsed) / width) * width
      = width * (((scale : Int) * elapsed) / width) := Int.mul_comm _ _
  rw [cappedRampComplementFloorBoundaryV2_interior scale elapsed width
      positiveElapsed interior, Int.toNat_of_nonneg nonneg]
  omega

/-- **Rounding direction, second half.** The residue the exact complement
absorbs is strictly smaller than one apportioned atom, so the primary payout is
the exact integer floor and no second rounding decision exists. -/
theorem cappedRampComplementFloorBoundaryV2_residue_lt_one_atom
    (scale : Nat) (elapsed width : Int)
    (positiveElapsed : 0 < elapsed) (interior : elapsed < width) :
    (scale : Int) * elapsed <
      ((cappedRampComplementFloorBoundaryV2 scale elapsed width : Int) + 1) * width := by
  have positiveWidth : (0 : Int) < width := by omega
  have productNonneg : (0 : Int) ≤ (scale : Int) * elapsed :=
    Int.mul_nonneg (by omega) (by omega)
  have nonneg : (0 : Int) ≤ ((scale : Int) * elapsed) / width :=
    Int.ediv_nonneg productNonneg (by omega)
  have expand := Int.mul_ediv_add_emod ((scale : Int) * elapsed) width
  have remainderLt : ((scale : Int) * elapsed) % width < width :=
    Int.emod_lt_of_pos _ positiveWidth
  have expandStep : ((((scale : Int) * elapsed) / width) + 1) * width
      = width * (((scale : Int) * elapsed) / width) + width := by
    rw [Int.add_mul, Int.one_mul, Int.mul_comm]
  rw [cappedRampComplementFloorBoundaryV2_interior scale elapsed width
      positiveElapsed interior, Int.toNat_of_nonneg nonneg]
  omega

/-- The sole boundary is monotone in the elapsed coordinate, so a later
coordinate never apportions the primary claim fewer atoms. -/
theorem cappedRampComplementFloorBoundaryV2_monotone
    (scale : Nat) (first second width : Int)
    (positiveFirst : 0 < first) (ordered : first ≤ second) (interior : second < width) :
    cappedRampComplementFloorBoundaryV2 scale first width
      ≤ cappedRampComplementFloorBoundaryV2 scale second width := by
  have positiveWidth : (0 : Int) < width := by omega
  rw [cappedRampComplementFloorBoundaryV2_interior scale first width
      positiveFirst (by omega),
    cappedRampComplementFloorBoundaryV2_interior scale second width
      (by omega) interior]
  exact Int.toNat_le_toNat (Int.ediv_le_ediv positiveWidth
    (Int.mul_le_mul_of_nonneg_left ordered (by omega)))

/-- Capped ramp on already-scaled exact integer coordinates. The two knot
tails clamp explicitly, so every scaled coordinate has a payout. -/
def rampValue (scale : Nat) (observed left right : Int) : Nat :=
  if observed ≤ left then 0
  else if right ≤ observed then scale
  else cappedRampComplementFloorBoundaryV2 scale (observed - left) (right - left)

theorem rampValue_le (scale : Nat) (observed left right : Int) :
    rampValue scale observed left right ≤ scale := by
  unfold rampValue
  split
  · omega
  · split
    · omega
    · exact cappedRampComplementFloorBoundaryV2_le _ _ _

/-- Lower cap edge case, including the kink exactly at the left knot. -/
theorem rampValue_of_le_left
    (scale : Nat) (observed left right : Int) (atOrBelow : observed ≤ left) :
    rampValue scale observed left right = 0 := by
  unfold rampValue
  rw [if_pos atOrBelow]

/-- Upper cap edge case, including the kink exactly at the right knot. -/
theorem rampValue_of_right_le
    (scale : Nat) (observed left right : Int)
    (aboveLeft : left < observed) (atOrAbove : right ≤ observed) :
    rampValue scale observed left right = scale := by
  unfold rampValue
  rw [if_neg (by omega), if_pos atOrAbove]

theorem rampValue_interior
    (scale : Nat) (observed left right : Int)
    (aboveLeft : left < observed) (belowRight : observed < right) :
    rampValue scale observed left right =
      cappedRampComplementFloorBoundaryV2 scale (observed - left) (right - left) := by
  unfold rampValue
  rw [if_neg (by omega), if_neg (by omega)]

/-- Strictly inside the two knots the primary claim never reaches the cap, so
the exact complement always retains at least one atom. -/
theorem rampValue_lt_scale_of_interior
    (scale : Nat) (observed left right : Int)
    (positiveScale : 0 < scale)
    (aboveLeft : left < observed) (belowRight : observed < right) :
    rampValue scale observed left right < scale := by
  have never := cappedRampComplementFloorBoundaryV2_never_rounds_up scale
    (observed - left) (right - left) (by omega) (by omega)
  have strict : (scale : Int) * (observed - left) < (scale : Int) * (right - left) :=
    Int.mul_lt_mul_of_pos_left (by omega) (by omega)
  have product :
      (cappedRampComplementFloorBoundaryV2 scale (observed - left) (right - left) : Int)
          * (right - left) < (scale : Int) * (right - left) := by omega
  have lifted :
      (cappedRampComplementFloorBoundaryV2 scale (observed - left) (right - left) : Int)
        < (scale : Int) :=
    Int.lt_of_mul_lt_mul_right product (by omega)
  rw [rampValue_interior scale observed left right aboveLeft belowRight]
  omega

/-- The apportionment is monotone in the observed coordinate, so no coordinate
ordering can invert the two claims' payouts. -/
theorem rampValue_monotone
    (scale : Nat) (first second left right : Int) (ordered : first ≤ second) :
    rampValue scale first left right ≤ rampValue scale second left right := by
  by_cases lowFirst : first ≤ left
  · rw [rampValue_of_le_left scale first left right lowFirst]
    omega
  · by_cases highFirst : right ≤ first
    · rw [rampValue_of_right_le scale first left right (by omega) highFirst,
        rampValue_of_right_le scale second left right (by omega) (by omega)]
      omega
    · by_cases highSecond : right ≤ second
      · rw [rampValue_of_right_le scale second left right (by omega) highSecond]
        exact rampValue_le scale first left right
      · rw [rampValue_interior scale first left right (by omega) (by omega),
          rampValue_interior scale second left right (by omega) (by omega)]
        exact cappedRampComplementFloorBoundaryV2_monotone scale
          (first - left) (second - left) (right - left) (by omega) (by omega) (by omega)

def scaledCoordinate
    (profile : CappedRampComplement) (coordinate : RationalCoordinate) : Int :=
  coordinate.numerator * profile.knotDenominator

def scaledKnot
    (coordinate : RationalCoordinate) (numerator : Int) : Int :=
  numerator * coordinate.denominator

/-- Primary capped-ramp payout. Defensive zero-denominator input remains total;
physical admission rejects it before evaluation. -/
def CappedRampComplement.primary
    (profile : CappedRampComplement) (coordinate : RationalCoordinate) : Nat :=
  let observed := scaledCoordinate profile coordinate
  let left := scaledKnot coordinate profile.leftNumerator
  let right := scaledKnot coordinate profile.rightNumerator
  if observed ≤ left then 0
  else if right ≤ observed then profile.scale
  else cappedRampComplementFloorBoundaryV2 profile.scale
    (observed - left) (right - left)

theorem CappedRampComplement.primary_le
    (profile : CappedRampComplement) (coordinate : RationalCoordinate) :
    profile.primary coordinate ≤ profile.scale := by
  unfold CappedRampComplement.primary
  simp only
  split
  · omega
  · split
    · omega
    · exact cappedRampComplementFloorBoundaryV2_le _ _ _


/-- The profile evaluator is definitionally the scaled-coordinate capped ramp. -/
theorem CappedRampComplement.primary_eq_rampValue
    (profile : CappedRampComplement) (coordinate : RationalCoordinate) :
    profile.primary coordinate =
      rampValue profile.scale (scaledCoordinate profile coordinate)
        (scaledKnot coordinate profile.leftNumerator)
        (scaledKnot coordinate profile.rightNumerator) := rfl

/-- Lower cap, including the kink exactly at the left knot. -/
theorem CappedRampComplement.primary_of_le_left
    (profile : CappedRampComplement) (coordinate : RationalCoordinate)
    (atOrBelow : scaledCoordinate profile coordinate
      ≤ scaledKnot coordinate profile.leftNumerator) :
    profile.primary coordinate = 0 := by
  rw [profile.primary_eq_rampValue]
  exact rampValue_of_le_left _ _ _ _ atOrBelow

/-- Upper cap, including the kink exactly at the right knot. -/
theorem CappedRampComplement.primary_of_right_le
    (profile : CappedRampComplement) (coordinate : RationalCoordinate)
    (aboveLeft : scaledKnot coordinate profile.leftNumerator
      < scaledCoordinate profile coordinate)
    (atOrAbove : scaledKnot coordinate profile.rightNumerator
      ≤ scaledCoordinate profile coordinate) :
    profile.primary coordinate = profile.scale := by
  rw [profile.primary_eq_rampValue]
  exact rampValue_of_right_le _ _ _ _ aboveLeft atOrAbove

/-- Strictly between the knots the primary claim never reaches the cap, so the
exact complement always retains at least one collateral atom. -/
theorem CappedRampComplement.primary_lt_scale_of_interior
    (profile : CappedRampComplement) (coordinate : RationalCoordinate)
    (aboveLeft : scaledKnot coordinate profile.leftNumerator
      < scaledCoordinate profile coordinate)
    (belowRight : scaledCoordinate profile coordinate
      < scaledKnot coordinate profile.rightNumerator) :
    profile.primary coordinate < profile.scale := by
  rw [profile.primary_eq_rampValue]
  exact rampValue_lt_scale_of_interior _ _ _ _ profile.scalePositive
    aboveLeft belowRight

/-- Two coordinates over one common denominator never invert the two claims. -/
theorem CappedRampComplement.primary_monotone
    (profile : CappedRampComplement) (first second : RationalCoordinate)
    (sameDenominator : first.denominator = second.denominator)
    (ordered : scaledCoordinate profile first ≤ scaledCoordinate profile second) :
    profile.primary first ≤ profile.primary second := by
  have leftSame : scaledKnot first profile.leftNumerator
      = scaledKnot second profile.leftNumerator := by
    simp [scaledKnot, sameDenominator]
  have rightSame : scaledKnot first profile.rightNumerator
      = scaledKnot second profile.rightNumerator := by
    simp [scaledKnot, sameDenominator]
  rw [profile.primary_eq_rampValue, profile.primary_eq_rampValue,
    leftSame, rightSame]
  exact rampValue_monotone _ _ _ _ _ ordered

/-- Exact two-claim payout. The complement receives all integer atoms not
assigned by the single floor boundary. -/
def CappedRampComplement.evaluate
    (profile : CappedRampComplement) (coordinate : RationalCoordinate) : List Nat :=
  let primary := profile.primary coordinate
  [primary, profile.scale - primary]

theorem CappedRampComplement.evaluate_length
    (profile : CappedRampComplement) (coordinate : RationalCoordinate) :
    (profile.evaluate coordinate).length = 2 := by
  simp [CappedRampComplement.evaluate]

theorem CappedRampComplement.evaluate_bounded
    (profile : CappedRampComplement) (coordinate : RationalCoordinate)
    (payout : Nat) (member : payout ∈ profile.evaluate coordinate) :
    payout ≤ profile.scale := by
  simp only [CappedRampComplement.evaluate, List.mem_cons] at member
  rcases member with rfl | member
  · exact profile.primary_le coordinate
  · rcases member with rfl | impossible
    · exact Nat.sub_le _ _
    · contradiction

theorem CappedRampComplement.evaluate_partition
    (profile : CappedRampComplement) (coordinate : RationalCoordinate) :
    (profile.evaluate coordinate).sum = profile.scale := by
  simp only [CappedRampComplement.evaluate, List.sum_cons, List.sum_nil,
    Nat.add_zero]
  exact Nat.add_sub_of_le (profile.primary_le coordinate)

/-- The capped ramp and its exact complement form a width-two `Q`-scaled
nonnegative liability basis. -/
def CappedRampComplement.basis
    (profile : CappedRampComplement) : Basis RationalCoordinate := {
  width := 2
  scale := profile.scale
  widthPositive := by omega
  scalePositive := profile.scalePositive
  evaluate := profile.evaluate
  exactWidth := profile.evaluate_length
  payoutBounded := profile.evaluate_bounded
  partitionUnity := profile.evaluate_partition
}


/-! ### Attained caps and exact two-claim solvency -/

/-- The left knot itself, as an exact rational coordinate. -/
def CappedRampComplement.leftKnotCoordinate
    (profile : CappedRampComplement) : RationalCoordinate :=
  { numerator := profile.leftNumerator, denominator := profile.knotDenominator }

/-- The right knot itself, as an exact rational coordinate. -/
def CappedRampComplement.rightKnotCoordinate
    (profile : CappedRampComplement) : RationalCoordinate :=
  { numerator := profile.rightNumerator, denominator := profile.knotDenominator }

theorem CappedRampComplement.evaluate_at_left_knot
    (profile : CappedRampComplement) :
    profile.evaluate profile.leftKnotCoordinate = [0, profile.scale] := by
  have primary : profile.primary profile.leftKnotCoordinate = 0 := by
    refine profile.primary_of_le_left _ ?_
    simp [scaledCoordinate, scaledKnot, CappedRampComplement.leftKnotCoordinate]
  simp [CappedRampComplement.evaluate, primary]

theorem CappedRampComplement.evaluate_at_right_knot
    (profile : CappedRampComplement) :
    profile.evaluate profile.rightKnotCoordinate = [profile.scale, 0] := by
  have denominatorPositive : (0 : Int) < (profile.knotDenominator : Int) := by
    have positive := profile.knotDenominatorPositive
    omega
  have aboveLeft : scaledKnot profile.rightKnotCoordinate profile.leftNumerator
      < scaledCoordinate profile profile.rightKnotCoordinate := by
    simp only [scaledCoordinate, scaledKnot, CappedRampComplement.rightKnotCoordinate]
    exact Int.mul_lt_mul_of_pos_right profile.knotsOrdered denominatorPositive
  have primary : profile.primary profile.rightKnotCoordinate = profile.scale := by
    refine profile.primary_of_right_le _ aboveLeft ?_
    simp [scaledCoordinate, scaledKnot, CappedRampComplement.rightKnotCoordinate]
  simp [CappedRampComplement.evaluate, primary]

/-- Both caps are attained on the exact-rational coordinate domain. -/
theorem CappedRampComplement.peak_attained
    (profile : CappedRampComplement) (supplies : List Nat)
    (sameWidth : supplies.length = 2) :
    ∃ coordinate : RationalCoordinate,
      liability supplies (profile.evaluate coordinate) =
        peakSupply supplies * profile.scale := by
  cases supplies with
  | nil => simp at sameWidth
  | cons primarySupply rest =>
      cases rest with
      | nil => simp at sameWidth
      | cons complementSupply rest =>
          cases rest with
          | cons _ _ => simp at sameWidth
          | nil =>
              rcases Nat.le_total complementSupply primarySupply with bound | bound
              · refine ⟨profile.rightKnotCoordinate, ?_⟩
                have peak : peakSupply [primarySupply, complementSupply]
                    = primarySupply := by
                  simp only [peakSupply, Nat.max_def]
                  split <;> (try split) <;> omega
                rw [profile.evaluate_at_right_knot, peak]
                simp [liability]
              · refine ⟨profile.leftKnotCoordinate, ?_⟩
                have peak : peakSupply [primarySupply, complementSupply]
                    = complementSupply := by
                  simp only [peakSupply, Nat.max_def]
                  split <;> (try split) <;> omega
                rw [profile.evaluate_at_left_knot, peak]
                simp [liability]

/-- **Generalized solvency for the two-claim capped ramp is exactly
`H >= Q * peak(T)`.** Both caps are attained, so the `Q * max(supply)`
pre-resolution liability the physical planner uses is exact rather than a
conservative envelope. -/
theorem CappedRampComplement.globally_solvent_iff
    (profile : CappedRampComplement) (hoard : Nat) (supplies : List Nat)
    (sameWidth : supplies.length = 2) :
    profile.basis.GloballySolvent hoard supplies ↔
      peakSupply supplies * profile.scale ≤ hoard := by
  constructor
  · intro solvent
    obtain ⟨coordinate, attained⟩ := profile.peak_attained supplies sameWidth
    have bound := solvent coordinate
    unfold SolventAt at bound
    have evaluate : profile.basis.evaluate coordinate = profile.evaluate coordinate := rfl
    rw [evaluate] at bound
    omega
  · intro covered
    exact Basis.peak_bound_globally_solvent profile.basis hoard supplies covered

/-- Categorical claims are exactly the `Q = 1` member of the same family. -/
theorem categoricalBasis_scale (width : Nat) (widthPositive : 0 < width) :
    (categoricalBasis width widthPositive).scale = 1 := rfl

/-! ## Provisional exact physical profile

The 64-byte request is only the first measured differential profile.  Its
`i64` numerators and `u32` positive scales/denominators are provisional
representation bounds, not premises of `Basis` or its preservation theorems.
-/

namespace PhysicalAbi

def requestMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x4c, 0x42, 0x56, 0x32]
def requestBytes : Nat := 64
def schemaVersion : Nat := 2
def profile : Nat := 1

def magicOffset : Nat := 0
def versionOffset : Nat := 8
def profileOffset : Nat := 10
def scaleOffset : Nat := 12
def knotDenominatorOffset : Nat := 16
def leftNumeratorOffset : Nat := 20
def rightNumeratorOffset : Nat := 28
def coordinateNumeratorOffset : Nat := 36
def coordinateDenominatorOffset : Nat := 44
def reservedOffset : Nat := 48
def reservedBytes : Nat := 16

structure Request where
  scale : Nat
  knotDenominator : Nat
  leftNumerator : Int
  rightNumerator : Int
  coordinateNumerator : Int
  coordinateDenominator : Nat
  deriving DecidableEq, Repr

def i64Min : Int := -(2 ^ 63)
def i64Max : Int := 2 ^ 63 - 1
def u32Limit : Nat := 2 ^ 32

def Request.physicallyRepresentable (request : Request) : Bool :=
  request.scale < u32Limit && request.knotDenominator < u32Limit &&
    request.coordinateDenominator < u32Limit &&
    i64Min ≤ request.leftNumerator && request.leftNumerator ≤ i64Max &&
    i64Min ≤ request.rightNumerator && request.rightNumerator ≤ i64Max &&
    i64Min ≤ request.coordinateNumerator && request.coordinateNumerator ≤ i64Max

/-- Semantic evaluation after the exact positive/ordered physical premises are
checked. This owns the generated agreement corpus, not Rust execution. -/
def Request.evaluate? (request : Request) : Option (List Nat) :=
  if _represented : request.physicallyRepresentable then
    if scalePositive : 0 < request.scale then
      if denominatorPositive : 0 < request.knotDenominator then
        if _coordinateDenominatorPositive : 0 < request.coordinateDenominator then
          if knotsOrdered : request.leftNumerator < request.rightNumerator then
            let profile : CappedRampComplement := {
              scale := request.scale
              knotDenominator := request.knotDenominator
              leftNumerator := request.leftNumerator
              rightNumerator := request.rightNumerator
              scalePositive
              knotDenominatorPositive := denominatorPositive
              knotsOrdered
            }
            some (profile.evaluate {
              numerator := request.coordinateNumerator
              denominator := request.coordinateDenominator
            })
          else none
        else none
      else none
    else none
  else none

/-- Two's-complement low 64 bits. Physical corpus inputs separately fit `i64`. -/
def encodeI64 (value : Int) : List UInt8 :=
  let bits := if 0 ≤ value then value.toNat else ((2 ^ 64 : Int) + value).toNat
  DClutch.Codec.encodeLE 8 bits

def encodeRequest (request : Request) : List UInt8 :=
  requestMagic ++
    DClutch.Codec.encodeLE 2 schemaVersion ++
    DClutch.Codec.encodeLE 2 profile ++
    DClutch.Codec.encodeLE 4 request.scale ++
    DClutch.Codec.encodeLE 4 request.knotDenominator ++
    encodeI64 request.leftNumerator ++
    encodeI64 request.rightNumerator ++
    encodeI64 request.coordinateNumerator ++
    DClutch.Codec.encodeLE 4 request.coordinateDenominator ++
    List.replicate reservedBytes 0

/-- Decode one signed two's-complement `i64` field from exactly eight bytes. -/
def decodeI64 (bytes : List UInt8) : Int :=
  let bits := DClutch.Codec.decodeLE bytes
  if bits < 2 ^ 63 then Int.ofNat bits else Int.ofNat bits - 2 ^ 64

def field (bytes : List UInt8) (offset width : Nat) : List UInt8 :=
  (bytes.drop offset).take width

/-- The exact field projection the hostile decoder performs before it applies
any semantic guard. -/
def projectRequest (bytes : List UInt8) : Request := {
  scale := DClutch.Codec.decodeLE (field bytes scaleOffset 4)
  knotDenominator := DClutch.Codec.decodeLE (field bytes knotDenominatorOffset 4)
  leftNumerator := decodeI64 (field bytes leftNumeratorOffset 8)
  rightNumerator := decodeI64 (field bytes rightNumeratorOffset 8)
  coordinateNumerator := decodeI64 (field bytes coordinateNumeratorOffset 8)
  coordinateDenominator :=
    DClutch.Codec.decodeLE (field bytes coordinateDenominatorOffset 4)
}

/-- Hostile semantic decoder used to own the generated refusal corpus. Error
tags match the handwritten Rust kernel: length `0`, magic `1`, schema `2`,
profile `3`, reserved `4`, scale `5`, denominator `6`, knot order `7`. -/
def decodeRequest (bytes : List UInt8) : Except Nat Request :=
  if bytes.length != requestBytes then .error 0
  else if field bytes magicOffset requestMagic.length != requestMagic then .error 1
  else if DClutch.Codec.decodeLE (field bytes versionOffset 2) != schemaVersion then
    .error 2
  else if DClutch.Codec.decodeLE (field bytes profileOffset 2) != profile then .error 3
  else if !(field bytes reservedOffset reservedBytes).all (fun byte => byte == 0) then
    .error 4
  else if (projectRequest bytes).scale = 0 then .error 5
  else if (projectRequest bytes).knotDenominator = 0 ||
      (projectRequest bytes).coordinateDenominator = 0 then .error 6
  else if (projectRequest bytes).leftNumerator >= (projectRequest bytes).rightNumerator then
    .error 7
  else .ok (projectRequest bytes)


/-- Exact physical admission premises for one hostile-decoded request. -/
def Request.Admissible (request : Request) : Prop :=
  request.physicallyRepresentable = true ∧ 0 < request.scale ∧
    0 < request.knotDenominator ∧ 0 < request.coordinateDenominator ∧
    request.leftNumerator < request.rightNumerator

/-- **Evaluator totality.** Evaluation succeeds on exactly the admitted
requests: no admitted terminal result is left without a payout, and no
inadmissible request silently receives one. -/
theorem Request.evaluate?_isSome_iff (request : Request) :
    request.evaluate?.isSome = true ↔ request.Admissible := by
  unfold Request.evaluate? Request.Admissible
  by_cases represented : request.physicallyRepresentable = true
  · by_cases scalePositive : 0 < request.scale
    · by_cases denominatorPositive : 0 < request.knotDenominator
      · by_cases coordinatePositive : 0 < request.coordinateDenominator
        · by_cases ordered : request.leftNumerator < request.rightNumerator
          · simp [represented, scalePositive, denominatorPositive,
              coordinatePositive, ordered]
          · simp [represented, scalePositive, denominatorPositive,
              coordinatePositive, ordered]
        · simp [represented, scalePositive, denominatorPositive, coordinatePositive]
      · simp [represented, scalePositive, denominatorPositive]
    · simp [represented, scalePositive]
  · simp [represented]

/-- Every successful evaluation is the exact capped-ramp profile evaluation. -/
theorem Request.evaluate?_eq_profile
    (request : Request) (payouts : List Nat)
    (evaluated : request.evaluate? = some payouts) :
    ∃ profile : CappedRampComplement,
      profile.scale = request.scale ∧
        payouts = profile.evaluate
          { numerator := request.coordinateNumerator,
            denominator := request.coordinateDenominator } := by
  unfold Request.evaluate? at evaluated
  split at evaluated
  · split at evaluated
    · split at evaluated
      · split at evaluated
        · split at evaluated
          · exact ⟨_, rfl, by simpa using evaluated.symm⟩
          · simp at evaluated
        · simp at evaluated
      · simp at evaluated
    · simp at evaluated
  · simp at evaluated

theorem Request.evaluate?_length
    (request : Request) (payouts : List Nat)
    (evaluated : request.evaluate? = some payouts) : payouts.length = 2 := by
  obtain ⟨profile, _, rfl⟩ := request.evaluate?_eq_profile payouts evaluated
  exact profile.evaluate_length _

/-- **Exact partition sum at the physical boundary.** -/
theorem Request.evaluate?_partition
    (request : Request) (payouts : List Nat)
    (evaluated : request.evaluate? = some payouts) :
    payouts.sum = request.scale := by
  obtain ⟨profile, scale, rfl⟩ := request.evaluate?_eq_profile payouts evaluated
  rw [profile.evaluate_partition, scale]

theorem Request.evaluate?_bounded
    (request : Request) (payouts : List Nat) (payout : Nat)
    (evaluated : request.evaluate? = some payouts)
    (member : payout ∈ payouts) : payout ≤ request.scale := by
  obtain ⟨profile, scale, rfl⟩ := request.evaluate?_eq_profile payouts evaluated
  rw [← scale]
  exact profile.evaluate_bounded _ payout member

theorem Request.evaluate?_validPartition
    (request : Request) (payouts : List Nat)
    (evaluated : request.evaluate? = some payouts) :
    validPartition payouts request.scale = true := by
  obtain ⟨profile, scale, rfl⟩ := request.evaluate?_eq_profile payouts evaluated
  rw [← scale]
  exact profile.basis.validPartition_evaluate _

/-! ### Hostile decoder bounds -/

theorem decodeLE_lt (bytes : List UInt8) :
    DClutch.Codec.decodeLE bytes < 256 ^ bytes.length := by
  induction bytes with
  | nil => simp [DClutch.Codec.decodeLE]
  | cons byte bytes induction =>
      have byteBound : byte.toNat < 256 := byte.toNat_lt_size
      simp only [DClutch.Codec.decodeLE, List.length_cons, Nat.pow_succ]
      omega

theorem field_length_le (bytes : List UInt8) (offset width : Nat) :
    (field bytes offset width).length ≤ width := by
  simp only [field, List.length_take, List.length_drop]
  exact Nat.min_le_left _ _

/-- Every fixed-width decoded field is inside its physical envelope, so the
`u32` and `i64` representation bounds are decoder facts, not assumptions. -/
theorem decodeField_lt (bytes : List UInt8) (offset width : Nat) :
    DClutch.Codec.decodeLE (field bytes offset width) < 256 ^ width := by
  have bound := decodeLE_lt (field bytes offset width)
  have widthBound : (256 : Nat) ^ (field bytes offset width).length ≤ 256 ^ width :=
    Nat.pow_le_pow_right (by omega) (field_length_le bytes offset width)
  omega

theorem decodeField_lt_u32Limit (bytes : List UInt8) (offset : Nat) :
    DClutch.Codec.decodeLE (field bytes offset 4) < u32Limit := by
  have bound := decodeField_lt bytes offset 4
  have expand : (256 : Nat) ^ 4 = u32Limit := by decide
  omega

theorem decodeI64_range (bytes : List UInt8) (widthBound : bytes.length ≤ 8) :
    i64Min ≤ decodeI64 bytes ∧ decodeI64 bytes ≤ i64Max := by
  have raw := decodeLE_lt bytes
  have step : (256 : Nat) ^ bytes.length ≤ 256 ^ 8 :=
    Nat.pow_le_pow_right (by omega) widthBound
  have expand : (256 : Nat) ^ 8 = 2 ^ 64 := by decide
  have bound : DClutch.Codec.decodeLE bytes < 2 ^ 64 := by omega
  simp only [decodeI64, i64Min, i64Max, Int.ofNat_eq_natCast]
  split <;> omega

theorem decodeI64_field_range (bytes : List UInt8) (offset : Nat) :
    i64Min ≤ decodeI64 (field bytes offset 8) ∧
      decodeI64 (field bytes offset 8) ≤ i64Max :=
  decodeI64_range _ (field_length_le bytes offset 8)


/-- Every field projection lands inside the physical `u32`/`i64` envelope, so
representability is a decoder fact rather than an assumption. -/
theorem projectRequest_representable (bytes : List UInt8) :
    (projectRequest bytes).physicallyRepresentable = true := by
  have scaleBound := decodeField_lt_u32Limit bytes scaleOffset
  have knotBound := decodeField_lt_u32Limit bytes knotDenominatorOffset
  have coordinateBound := decodeField_lt_u32Limit bytes coordinateDenominatorOffset
  have leftRange := decodeI64_field_range bytes leftNumeratorOffset
  have rightRange := decodeI64_field_range bytes rightNumeratorOffset
  have numeratorRange := decodeI64_field_range bytes coordinateNumeratorOffset
  simp only [Request.physicallyRepresentable, projectRequest, Bool.and_eq_true,
    decide_eq_true_eq]
  omega

/-- **Hostile decode is total into evaluation.** Every accepted request meets
every admission premise, so an accepted request always has an exact payout. -/
theorem decodeRequest_admissible
    (bytes : List UInt8) (request : Request)
    (decoded : decodeRequest bytes = .ok request) : request.Admissible := by
  unfold decodeRequest at decoded
  split at decoded
  · simp at decoded
  · split at decoded
    · simp at decoded
    · split at decoded
      · simp at decoded
      · split at decoded
        · simp at decoded
        · split at decoded
          · simp at decoded
          · split at decoded
            · simp at decoded
            · split at decoded
              · simp at decoded
              · split at decoded
                · simp at decoded
                · rename_i scaleGuard denominatorGuard orderGuard
                  obtain rfl : request = projectRequest bytes := by
                    simpa using decoded.symm
                  simp only [Bool.or_eq_true, decide_eq_true_eq, not_or]
                    at denominatorGuard
                  exact ⟨projectRequest_representable bytes, by omega, by omega,
                    by omega, by omega⟩

theorem decodeRequest_evaluates
    (bytes : List UInt8) (request : Request)
    (decoded : decodeRequest bytes = .ok request) :
    request.evaluate?.isSome = true :=
  (Request.evaluate?_isSome_iff request).2 (decodeRequest_admissible bytes request decoded)

/-- Any input that is not exactly the canonical physical width is refused with
the stable length tag; no short or long record is ever partially decoded. -/
theorem decodeRequest_refuses_wrong_length
    (bytes : List UInt8) (wrongLength : bytes.length ≠ requestBytes) :
    decodeRequest bytes = .error 0 := by
  unfold decodeRequest
  rw [if_pos (by simpa using wrongLength)]

theorem encodeI64_length (value : Int) : (encodeI64 value).length = 8 := by
  simp [encodeI64, DClutch.Codec.encodeLE_length]

theorem encodeRequest_length (request : Request) :
    (encodeRequest request).length = requestBytes := by
  simp [encodeRequest, requestMagic, requestBytes, reservedBytes,
    DClutch.Codec.encodeLE_length, encodeI64_length]

end PhysicalAbi

/-! ## Provisional physical transition planner

The theorems above quantify over unbounded `Nat`.  The physical Claims planner
additionally works inside a `u64` envelope and refuses with stable tags.  This
model owns the generated transition corpus: it is the exact statement the
handwritten Rust planner must reproduce, and its admitted outcomes are proved
to agree with the pure liability semantics.

The `u64` envelope is a provisional physical representation bound, not a
premise of `Basis` or of any preservation theorem above.
-/

namespace PhysicalPlanner

/-- Physical `u64` envelope for supplies, payouts, scale, quantity, and Hoard. -/
def u64Limit : Nat := 2 ^ 64

/-- The three admitted pure Claims transitions. -/
inductive Operation where
  | split
  | merge
  | terminalRedeem
  deriving DecidableEq, Repr

/-- Stable generated-corpus operation tag. -/
def Operation.tag : Operation → Nat
  | .split => 0
  | .merge => 1
  | .terminalRedeem => 2

/-- One complete pure transition candidate over a runtime-width basis. -/
structure Transition where
  supplies : List Nat
  payouts : List Nat
  scale : Nat
  quantity : Nat
  claimIndex : Nat
  hoard : Nat
  operation : Operation
  deriving Repr

/-- The three exact economic facts an admitted plan commits. -/
structure Outcome where
  hoardAfter : Nat
  liabilityBefore : Nat
  liabilityAfter : Nat
  deriving DecidableEq, Repr

def Transition.width (transition : Transition) : Nat := transition.supplies.length

def Transition.liabilityBefore (transition : Transition) : Nat :=
  liability transition.supplies transition.payouts

/-- Exact collateral moved by the transition. -/
def Transition.collateralDelta (transition : Transition) : Nat :=
  match transition.operation with
  | .split | .merge => transition.quantity * transition.scale
  | .terminalRedeem =>
      transition.quantity * entryAt transition.payouts transition.claimIndex

def Transition.supplyAfter (transition : Transition) : List Nat :=
  match transition.operation with
  | .split => splitSupply transition.quantity transition.supplies
  | .merge => mergeSupply transition.quantity transition.supplies
  | .terminalRedeem =>
      debitAt transition.claimIndex transition.quantity transition.supplies

def Transition.liabilityAfter (transition : Transition) : Nat :=
  liability transition.supplyAfter transition.payouts

def Transition.hoardAfter (transition : Transition) : Nat :=
  match transition.operation with
  | .split => transition.hoard + transition.collateralDelta
  | .merge | .terminalRedeem => transition.hoard - transition.collateralDelta

def Transition.outcome (transition : Transition) : Outcome := {
  hoardAfter := transition.hoardAfter
  liabilityBefore := transition.liabilityBefore
  liabilityAfter := transition.liabilityAfter
}

/-- Ordered refusal checks. The first failing check names the refusal tag, so
the ordering is part of the translation contract. Tags: `5` zero scale,
`8` empty basis, `9` width mismatch, `10` non-partition, `11` arithmetic
overflow, `12` insolvent, `13` claim coordinate out of range, `14`
insufficient supply. -/
def Transition.checks (transition : Transition) : List (Nat × Bool) := [
  (8, !transition.supplies.isEmpty && !transition.payouts.isEmpty),
  (9, decide (transition.supplies.length = transition.payouts.length)),
  (5, decide (0 < transition.scale)),
  (10, validPartition transition.payouts transition.scale),
  (13, match transition.operation with
      | .terminalRedeem => decide (transition.claimIndex < transition.width)
      | _ => true),
  (14, match transition.operation with
      | .merge =>
          transition.supplies.all (fun supply => decide (transition.quantity ≤ supply))
      | .terminalRedeem =>
          decide (transition.quantity ≤ entryAt transition.supplies transition.claimIndex)
      | .split => true),
  (11, decide (transition.liabilityBefore < u64Limit)),
  (11, decide (transition.collateralDelta < u64Limit)),
  (12, decide (transition.liabilityBefore ≤ transition.hoard)),
  (11, match transition.operation with
      | .split => decide (transition.hoard + transition.collateralDelta < u64Limit)
      | _ => true),
  (12, match transition.operation with
      | .split => true
      | _ => decide (transition.collateralDelta ≤ transition.hoard)),
  (11, transition.supplyAfter.all (fun supply => decide (supply < u64Limit))),
  (11, decide (transition.liabilityAfter < u64Limit)),
  (12, decide (transition.liabilityAfter ≤ transition.hoardAfter))
]

/-- The tag of the first failing check, or `none` when the plan is admitted. -/
def Transition.refusal? (transition : Transition) : Option Nat :=
  (transition.checks.find? (fun check => !check.2)).map Prod.fst

/-- The complete physical planner. -/
def Transition.plan? (transition : Transition) : Except Nat Outcome :=
  match transition.refusal? with
  | some tag => .error tag
  | none => .ok transition.outcome

theorem refusal_none_check
    (transition : Transition) (check : Nat × Bool)
    (admitted : transition.refusal? = none)
    (member : check ∈ transition.checks) : check.2 = true := by
  unfold Transition.refusal? at admitted
  rw [Option.map_eq_none_iff, List.find?_eq_none] at admitted
  simpa using admitted check member

theorem refusal_none_getElem
    (transition : Transition) (index : Nat)
    (bound : index < transition.checks.length)
    (admitted : transition.refusal? = none) :
    (transition.checks[index]).2 = true :=
  refusal_none_check transition _ admitted (List.getElem_mem bound)

theorem plan_ok_admitted
    (transition : Transition) (outcome : Outcome)
    (planned : transition.plan? = .ok outcome) :
    transition.refusal? = none ∧ outcome = transition.outcome := by
  unfold Transition.plan? at planned
  split at planned
  · simp at planned
  · rename_i admitted
    exact ⟨admitted, by simpa using planned.symm⟩

theorem plan_ok_sameWidth
    (transition : Transition) (outcome : Outcome)
    (planned : transition.plan? = .ok outcome) :
    transition.supplies.length = transition.payouts.length := by
  have admitted := (plan_ok_admitted transition outcome planned).1
  have check := refusal_none_getElem transition 1 (by simp [Transition.checks]) admitted
  simpa [Transition.checks] using check

theorem plan_ok_partition
    (transition : Transition) (outcome : Outcome)
    (planned : transition.plan? = .ok outcome) :
    validPartition transition.payouts transition.scale = true := by
  have admitted := (plan_ok_admitted transition outcome planned).1
  have check := refusal_none_getElem transition 3 (by simp [Transition.checks]) admitted
  simpa [Transition.checks] using check

/-- **Every admitted plan is solvent at the evaluated result.** -/
theorem plan_ok_solvent
    (transition : Transition) (outcome : Outcome)
    (planned : transition.plan? = .ok outcome) :
    outcome.liabilityAfter ≤ outcome.hoardAfter := by
  have admitted := (plan_ok_admitted transition outcome planned).1
  have check := refusal_none_getElem transition 13 (by simp [Transition.checks]) admitted
  rw [(plan_ok_admitted transition outcome planned).2]
  simpa [Transition.checks] using check

theorem plan_ok_liabilityBefore_le_hoard
    (transition : Transition) (outcome : Outcome)
    (planned : transition.plan? = .ok outcome) :
    outcome.liabilityBefore ≤ transition.hoard := by
  have admitted := (plan_ok_admitted transition outcome planned).1
  have check := refusal_none_getElem transition 8 (by simp [Transition.checks]) admitted
  rw [(plan_ok_admitted transition outcome planned).2]
  simpa [Transition.checks] using check

/-- **Split agreement.** The admitted plan's liability and collateral both move
by exactly `quantity * Q`. -/
theorem split_plan_exact
    (transition : Transition) (outcome : Outcome)
    (isSplit : transition.operation = Operation.split)
    (planned : transition.plan? = .ok outcome) :
    outcome.liabilityAfter =
        outcome.liabilityBefore + transition.quantity * transition.scale ∧
      outcome.hoardAfter = transition.hoard + transition.quantity * transition.scale := by
  have sameWidth := plan_ok_sameWidth transition outcome planned
  have partition := plan_ok_partition transition outcome planned
  have sum := validPartition_sum transition.payouts transition.scale partition
  rw [(plan_ok_admitted transition outcome planned).2]
  refine ⟨?_, ?_⟩
  · show transition.liabilityAfter = transition.liabilityBefore + _
    unfold Transition.liabilityAfter Transition.liabilityBefore Transition.supplyAfter
    rw [isSplit]
    rw [liability_split transition.quantity transition.supplies transition.payouts sameWidth,
      sum]
  · show transition.hoardAfter = _
    unfold Transition.hoardAfter Transition.collateralDelta
    rw [isSplit]

/-- **Merge agreement.** The admitted plan's liability and collateral both fall
by exactly `quantity * Q`, with no truncation at any coordinate. -/
theorem merge_plan_exact
    (transition : Transition) (outcome : Outcome)
    (isMerge : transition.operation = Operation.merge)
    (planned : transition.plan? = .ok outcome) :
    outcome.liabilityAfter + transition.quantity * transition.scale =
        outcome.liabilityBefore ∧
      outcome.hoardAfter + transition.quantity * transition.scale = transition.hoard := by
  have admitted := (plan_ok_admitted transition outcome planned).1
  have sameWidth := plan_ok_sameWidth transition outcome planned
  have partition := plan_ok_partition transition outcome planned
  have sum := validPartition_sum transition.payouts transition.scale partition
  have backedCheck := refusal_none_getElem transition 5 (by simp [Transition.checks]) admitted
  have coveredCheck := refusal_none_getElem transition 10 (by simp [Transition.checks]) admitted
  have backed : MergeAdmissible transition.quantity transition.supplies := by
    simp only [Transition.checks, isMerge, List.getElem_cons_succ, List.getElem_cons_zero,
      List.all_eq_true, decide_eq_true_eq] at backedCheck
    intro supply member
    exact backedCheck supply member
  have covered : transition.collateralDelta ≤ transition.hoard := by
    simp only [Transition.checks, isMerge, List.getElem_cons_succ,
      List.getElem_cons_zero, decide_eq_true_eq] at coveredCheck
    exact coveredCheck
  have delta : transition.collateralDelta = transition.quantity * transition.scale := by
    unfold Transition.collateralDelta
    rw [isMerge]
  rw [(plan_ok_admitted transition outcome planned).2]
  refine ⟨?_, ?_⟩
  · show transition.liabilityAfter + _ = transition.liabilityBefore
    unfold Transition.liabilityAfter Transition.liabilityBefore Transition.supplyAfter
    rw [isMerge, ← sum]
    exact liability_merge transition.quantity transition.supplies transition.payouts
      sameWidth backed
  · show transition.hoardAfter + _ = transition.hoard
    have hoardValue :
        transition.hoardAfter = transition.hoard - transition.collateralDelta := by
      unfold Transition.hoardAfter
      rw [isMerge]
    rw [hoardValue, ← delta]
    omega

/-- **Terminal redemption agreement.** The admitted plan's liability and
collateral both fall by exactly `quantity * p_i(x)`. -/
theorem redeem_plan_exact
    (transition : Transition) (outcome : Outcome)
    (isRedeem : transition.operation = Operation.terminalRedeem)
    (planned : transition.plan? = .ok outcome) :
    outcome.liabilityAfter +
        transition.quantity * entryAt transition.payouts transition.claimIndex =
        outcome.liabilityBefore ∧
      outcome.hoardAfter +
        transition.quantity * entryAt transition.payouts transition.claimIndex =
        transition.hoard := by
  have admitted := (plan_ok_admitted transition outcome planned).1
  have backedCheck := refusal_none_getElem transition 5 (by simp [Transition.checks]) admitted
  have coveredCheck := refusal_none_getElem transition 10 (by simp [Transition.checks]) admitted
  have backed : TransferBacked transition.claimIndex transition.quantity transition.supplies := by
    simp only [Transition.checks, isRedeem, List.getElem_cons_succ, List.getElem_cons_zero,
      decide_eq_true_eq] at backedCheck
    exact backedCheck
  have covered : transition.collateralDelta ≤ transition.hoard := by
    simp only [Transition.checks, isRedeem, List.getElem_cons_succ,
      List.getElem_cons_zero, decide_eq_true_eq] at coveredCheck
    exact coveredCheck
  have delta : transition.collateralDelta =
      transition.quantity * entryAt transition.payouts transition.claimIndex := by
    unfold Transition.collateralDelta
    rw [isRedeem]
  rw [(plan_ok_admitted transition outcome planned).2]
  refine ⟨?_, ?_⟩
  · show transition.liabilityAfter + _ = transition.liabilityBefore
    unfold Transition.liabilityAfter Transition.liabilityBefore Transition.supplyAfter
    rw [isRedeem]
    exact liability_debitAt transition.claimIndex transition.quantity
      transition.supplies transition.payouts backed
  · show transition.hoardAfter + _ = transition.hoard
    have hoardValue :
        transition.hoardAfter = transition.hoard - transition.collateralDelta := by
      unfold Transition.hoardAfter
      rw [isRedeem]
    rw [hoardValue, ← delta]
    omega

end PhysicalPlanner

end DClutch.LiabilityBasisV2
