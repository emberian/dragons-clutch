import DClutchSemantics.FractionalClaimV1
import Std.Tactic

/-!
# Structured V2: receipts backed by exact claim shards

Structured V1 could only admit a portfolio recipe whose least realization lot
equalled the Product denominator, because its backing had to land on whole
native categorical claims.  Structured V2 removes that restriction by moving
one level down the representation graph: a Structured receipt atom is backed by
exact *claim shard* atoms, and the shard layer already owns the exact
`F_i = D * C_i` denomination invariant.

For normalized coefficient `c_i / D`, receipt supply `S`, and Structured shard
custody `K_i`, the exact backing invariant is

```text
K_i = S * c_i      for every representation coordinate i
```

so one receipt atom denotes exactly `c_i / D` native claims of coordinate `i`
without any residual credit, remainder ledger, or rounding.

Three properties are structural rather than proof obligations:

* **Custody is `required + surplus`.**  A Structured shard custody account is an
  ordinary Token account, so anyone may donate into it.  This model therefore
  represents observed custody as required backing plus a named `surplus`, and no
  transition reads or moves `surplus`.  Donated atoms are consequently never
  backing, never redeemable, and never distributed.
* **Structured owns no rounding boundary.**  Terminal settlement reuses
  `DClutch.FractionalClaimV1.divideClaimShardsV1`, the protocol's sole
  quotient/remainder boundary, and `structured_reuses_the_fractional_boundary`
  proves the identity.  Sub-denominator remainders stay as ordinary transferable
  shard atoms of the same Mint.
* **The graph is a rank-decreasing DAG.**  Every backing edge strictly decreases
  rank, so no receipt can be backed by a receipt and no node can reach itself.

Market lifecycle, payout evaluation, native claim custody, shard Mint supply,
and Token holder balances each keep their existing semantic owner; this module
observes them and never mints a second authority.  Token-2022, CPI, account
parsing, rent, and transaction rollback are deliberately outside the model.
-/

namespace DClutch.StructuredV2

/-! ## The finite representation graph -/

/-- Every node kind admitted by the runtime representation profile. -/
inductive NodeKind where
  /-- Market-owned terminal liability; the unique sink. -/
  | marketLiability
  /-- Canonical Claims-owned native categorical Position. -/
  | nativePosition
  /-- Token-owned exact claim shard Mint. -/
  | claimShard
  /-- Token-owned Structured receipt Mint. -/
  | structuredReceipt
  deriving DecidableEq, Repr

/-- Strictly decreasing rank along every backing edge. -/
def NodeKind.rank : NodeKind → Nat
  | .marketLiability => 0
  | .nativePosition => 1
  | .claimShard => 2
  | .structuredReceipt => 3

/-- One backing edge: the parent's supply is backed by the child's. -/
structure BackingEdge where
  /-- Node whose supply is backed. -/
  parent : NodeKind
  /-- Node providing the backing. -/
  child : NodeKind
  deriving DecidableEq, Repr

/-- An edge is admissible exactly when it strictly decreases rank. -/
def BackingEdge.admits (edge : BackingEdge) : Bool :=
  edge.child.rank < edge.parent.rank

/-- The deliberately finite depth-two runtime graph. -/
def canonicalGraph : List BackingEdge :=
  [⟨.structuredReceipt, .claimShard⟩,
   ⟨.claimShard, .nativePosition⟩,
   ⟨.nativePosition, .marketLiability⟩]

/-- Every edge decreases rank and every node has at most one backing edge. -/
def AdmissibleGraph (edges : List BackingEdge) : Prop :=
  (∀ edge ∈ edges, edge.admits = true) ∧
    ∀ left ∈ edges, ∀ right ∈ edges, left.parent = right.parent → left.child = right.child

/-- Transitive backing reachability. -/
inductive Reaches (edges : List BackingEdge) : NodeKind → NodeKind → Prop where
  /-- One admitted backing edge. -/
  | edge {parent child : NodeKind} :
      BackingEdge.mk parent child ∈ edges → Reaches edges parent child
  /-- Composition of two backing paths. -/
  | tail {parent middle child : NodeKind} :
      Reaches edges parent middle → Reaches edges middle child → Reaches edges parent child

theorem reaches_decreases_rank
    (edges : List BackingEdge) (admissible : AdmissibleGraph edges)
    (parent child : NodeKind) (path : Reaches edges parent child) :
    child.rank < parent.rank := by
  induction path with
  | edge member =>
      have decreasing := admissible.1 _ member
      simpa [BackingEdge.admits, decide_eq_true_eq] using decreasing
  | tail _ _ left right => exact Nat.lt_trans right left

/-- A rank-decreasing backing graph cannot contain a cycle. -/
theorem admissible_graph_is_acyclic
    (edges : List BackingEdge) (admissible : AdmissibleGraph edges) (node : NodeKind) :
    ¬ Reaches edges node node := by
  intro path
  exact Nat.lt_irrefl node.rank (reaches_decreases_rank edges admissible node node path)

theorem canonical_graph_is_admissible : AdmissibleGraph canonicalGraph := by
  constructor
  · intro edge member
    simp only [canonicalGraph, List.mem_cons, List.not_mem_nil, or_false] at member
    rcases member with rfl | rfl | rfl <;> rfl
  · intro left leftMember right rightMember same
    simp only [canonicalGraph, List.mem_cons, List.not_mem_nil, or_false] at leftMember rightMember
    rcases leftMember with rfl | rfl | rfl <;> rcases rightMember with rfl | rfl | rfl <;>
      first
        | rfl
        | (exact absurd same (by decide))

theorem canonical_graph_is_acyclic (node : NodeKind) : ¬ Reaches canonicalGraph node node :=
  admissible_graph_is_acyclic canonicalGraph canonical_graph_is_admissible node

/-- Wrapper-on-wrapper live custody is refused by the rank rule alone. -/
theorem receipt_backed_by_receipt_is_refused :
    ¬ AdmissibleGraph (BackingEdge.mk .structuredReceipt .structuredReceipt :: canonicalGraph) := by
  intro admissible
  have decreasing := admissible.1 ⟨.structuredReceipt, .structuredReceipt⟩ (by simp)
  exact absurd decreasing (by decide)

/-- A shard may not be backed by a receipt: rank must strictly decrease. -/
theorem shard_backed_by_receipt_is_refused :
    ¬ AdmissibleGraph (BackingEdge.mk .claimShard .structuredReceipt :: canonicalGraph) := by
  intro admissible
  have decreasing := admissible.1 ⟨.claimShard, .structuredReceipt⟩ (by simp)
  exact absurd decreasing (by decide)

/-- A second backing edge for one supply owner is refused. -/
theorem two_backing_edges_are_refused :
    ¬ AdmissibleGraph (BackingEdge.mk .structuredReceipt .nativePosition :: canonicalGraph) := by
  intro admissible
  have collision :=
    admissible.2 ⟨.structuredReceipt, .nativePosition⟩ (by simp)
      ⟨.structuredReceipt, .claimShard⟩ (by simp [canonicalGraph]) rfl
  exact absurd collision (by decide)

/-! ## Immutable Structured basis -/

/-- Immutable finalized Structured V2 terms.  Coefficients are numerators over
the shard layer's denominator; the denominator itself is owned by the shard
terms and only restated here so every join is self-authenticating. -/
structure Basis where
  /-- Finalized content identity of the exact terms bytes. -/
  termsId : Nat
  /-- Logical Core Market. -/
  marketId : Nat
  /-- Finalized Product root digest. -/
  productRecordId : Nat
  /-- Product-owned result-domain identity and ordering. -/
  resultDomainId : Nat
  /-- Immutable release set. -/
  releaseSetId : Nat
  /-- Finalized exact claim-shard terms owning the `K` shard Mints. -/
  shardTermsId : Nat
  /-- Finalized Product-N to Claims-K exposure identity. -/
  shardExposureId : Nat
  /-- Token-owned Structured receipt Mint. -/
  receiptMintId : Nat
  /-- Stable representation-composition graph identity. -/
  graphId : Nat
  /-- Claims/shard representation width `K`. -/
  representationWidth : Nat
  /-- Exact shard atoms per whole native claim, owned by the shard terms. -/
  denominator : Nat
  /-- Exact shard atoms backing one receipt atom, in coordinate order. -/
  coefficients : List Nat
  deriving DecidableEq, Repr

/-- Every identity that must be nonzero and independently authenticated. -/
def Basis.identities (basis : Basis) : List Nat :=
  [basis.termsId, basis.marketId, basis.productRecordId, basis.resultDomainId,
   basis.releaseSetId, basis.shardTermsId, basis.shardExposureId,
   basis.receiptMintId, basis.graphId]

/-- Structural admission of one immutable basis. -/
def Basis.valid (scalarLimit : Nat) (basis : Basis) : Bool :=
  0 < scalarLimit &&
  basis.identities.all (fun identity => identity != 0) &&
  0 < basis.representationWidth &&
  basis.coefficients.length = basis.representationWidth &&
  basis.coefficients.any (fun coefficient => 0 < coefficient) &&
  basis.coefficients.all (fun coefficient => coefficient < scalarLimit) &&
  1 < basis.denominator && basis.denominator < scalarLimit

/-! ## Exact backing arithmetic -/

/-- Scale one coefficient vector by a receipt quantity. -/
def scaleCoefficients (quantity : Nat) (coefficients : List Nat) : List Nat :=
  coefficients.map fun coefficient => quantity * coefficient

/-- The exact shard custody required to back `receiptSupply` receipt atoms. -/
def Basis.requiredCustody (basis : Basis) (receiptSupply : Nat) : List Nat :=
  scaleCoefficients receiptSupply basis.coefficients

/-- **The exact backing invariant**: `K_i = S * c_i` for every coordinate. -/
def ExactlyBacked (basis : Basis) (receiptSupply : Nat) (shardCustody : List Nat) : Prop :=
  shardCustody = basis.requiredCustody receiptSupply

theorem scaleCoefficients_add (left right : Nat) (coefficients : List Nat) :
    scaleCoefficients (left + right) coefficients =
      List.zipWith (· + ·) (scaleCoefficients left coefficients)
        (scaleCoefficients right coefficients) := by
  induction coefficients with
  | nil => rfl
  | cons head tail induction =>
      simp [scaleCoefficients, Nat.add_mul]

theorem scaleCoefficients_sub (left right : Nat) (coefficients : List Nat) :
    scaleCoefficients (left - right) coefficients =
      List.zipWith (· - ·) (scaleCoefficients left coefficients)
        (scaleCoefficients right coefficients) := by
  induction coefficients with
  | nil => rfl
  | cons head tail induction =>
      simp [scaleCoefficients, Nat.sub_mul]

theorem scaleCoefficients_length (quantity : Nat) (coefficients : List Nat) :
    (scaleCoefficients quantity coefficients).length = coefficients.length := by
  simp [scaleCoefficients]

theorem scaleCoefficients_zero (coefficients : List Nat) :
    scaleCoefficients 0 coefficients = List.replicate coefficients.length 0 := by
  induction coefficients with
  | nil => rfl
  | cons head tail induction =>
      simp [scaleCoefficients, List.replicate] at induction ⊢
      exact induction

theorem scaleCoefficients_sum (quantity : Nat) (coefficients : List Nat) :
    (scaleCoefficients quantity coefficients).sum = quantity * coefficients.sum := by
  induction coefficients with
  | nil => simp [scaleCoefficients]
  | cons head tail induction =>
      simp [scaleCoefficients, Nat.mul_add] at induction ⊢
      omega

/-- Issuing `quantity` receipts and locking exactly `quantity * c_i` shard atoms
preserves the exact backing invariant. -/
theorem issue_preserves_exact_backing
    (basis : Basis) (receiptSupply quantity : Nat) (shardCustody : List Nat)
    (backed : ExactlyBacked basis receiptSupply shardCustody) :
    ExactlyBacked basis (receiptSupply + quantity)
      (List.zipWith (· + ·) shardCustody (basis.requiredCustody quantity)) := by
  unfold ExactlyBacked Basis.requiredCustody at backed ⊢
  rw [backed, scaleCoefficients_add]

/-- Burning `quantity` receipts and releasing exactly `quantity * c_i` shard
atoms preserves the exact backing invariant.  Unwrap and terminal redemption
share this arithmetic; only their admitted phases differ. -/
theorem release_preserves_exact_backing
    (basis : Basis) (receiptSupply quantity : Nat) (shardCustody : List Nat)
    (backed : ExactlyBacked basis receiptSupply shardCustody) :
    ExactlyBacked basis (receiptSupply - quantity)
      (List.zipWith (· - ·) shardCustody (basis.requiredCustody quantity)) := by
  unfold ExactlyBacked Basis.requiredCustody at backed ⊢
  rw [backed, scaleCoefficients_sub]

/-- Conservation: the released basket plus the remaining required backing is
exactly the previous required backing.  No shard atom is created or destroyed by
a Structured transition. -/
theorem release_basket_conserves_custody
    (basis : Basis) (receiptSupply quantity : Nat) (available : quantity ≤ receiptSupply) :
    List.zipWith (· + ·) (basis.requiredCustody (receiptSupply - quantity))
        (basis.requiredCustody quantity) =
      basis.requiredCustody receiptSupply := by
  unfold Basis.requiredCustody
  rw [← scaleCoefficients_add, Nat.sub_add_cancel available]

/-- Conservation in aggregate: the basket for `quantity` receipts totals exactly
`quantity * Σ c_i` shard atoms. -/
theorem basket_total_is_exact (basis : Basis) (quantity : Nat) :
    (basis.requiredCustody quantity).sum = quantity * basis.coefficients.sum :=
  scaleCoefficients_sum quantity basis.coefficients

/-- Zero receipt supply requires exactly zero shard custody, which is what makes
retirement closable. -/
theorem zero_supply_requires_zero_custody (basis : Basis) :
    basis.requiredCustody 0 = List.replicate basis.coefficients.length 0 :=
  scaleCoefficients_zero basis.coefficients

/-! ## Observed custody, surplus, and donation -/

/-- Adapter-observed Structured shard custody: required backing plus a named,
unowned surplus.  No transition in this module reads or moves `surplus`. -/
structure Custody where
  /-- Token-owned Structured receipt Mint supply `S`. -/
  receiptSupply : Nat
  /-- Donated or otherwise unowned shard atoms above the exact backing. -/
  surplus : List Nat
  deriving DecidableEq, Repr

/-- Exact chain-observed Structured shard custody balances. -/
def Custody.observed (basis : Basis) (custody : Custody) : List Nat :=
  List.zipWith (· + ·) (basis.requiredCustody custody.receiptSupply) custody.surplus

theorem observed_custody_is_solvent (basis : Basis) (custody : Custody) :
    Custody.observed basis custody =
      List.zipWith (· + ·) (basis.requiredCustody custody.receiptSupply) custody.surplus :=
  rfl

theorem no_surplus_is_exact_backing
    (basis : Basis) (custody : Custody)
    (clean : custody.surplus = List.replicate basis.coefficients.length 0) :
    ExactlyBacked basis custody.receiptSupply (Custody.observed basis custody) := by
  unfold ExactlyBacked Custody.observed Basis.requiredCustody at *
  rw [clean, ← scaleCoefficients_length custody.receiptSupply basis.coefficients]
  generalize scaleCoefficients custody.receiptSupply basis.coefficients = scaled
  induction scaled with
  | nil => rfl
  | cons head tail induction =>
      simp [List.replicate] at induction ⊢
      exact induction

/-! ## Authenticated lifecycle -/

/-- Authenticated Market lifecycle projected onto the Structured basis.  The
terminal payout vector is produced by the authenticated Product evaluator and
translated by the finalized exposure; Structured never re-derives it. -/
inductive Phase where
  /-- Market unresolved; receipts may be issued and unwrapped. -/
  | open
  /-- Terminal, with exact collateral atoms per whole native claim per coordinate. -/
  | terminal (payoutPerClaim : List Nat)
  /-- Zero supply and zero custody; the Structured node is closed. -/
  | retired
  deriving DecidableEq, Repr

/-- Structured-owned persisted state.  Supply, balances, payouts, and Market
lifecycle each keep their own semantic owner and are absent here. -/
structure Root where
  /-- Finalized basis identity. -/
  basisId : Nat
  /-- Logical Core Market. -/
  marketId : Nat
  /-- Permanent RentCredit beneficiary. -/
  rentBeneficiaryId : Nat
  /-- Replay revision. -/
  revision : Nat
  deriving DecidableEq, Repr

/-- Adapter-owned runtime projection; never persisted by Structured. -/
structure Projection where
  /-- Authenticated Market lifecycle. -/
  phase : Phase
  /-- Finalized basis identity. -/
  basisId : Nat
  /-- Logical Core Market. -/
  marketId : Nat
  /-- Finalized shard terms identity observed on the shard layer. -/
  shardTermsId : Nat
  /-- Exact shard atoms per whole native claim observed on the shard layer. -/
  shardDenominator : Nat
  /-- Observed representation width. -/
  representationWidth : Nat
  /-- Observed receipt supply and named surplus custody. -/
  custody : Custody
  /-- Observed root replay revision. -/
  revision : Nat
  deriving DecidableEq, Repr

/-- Exactly the four Structured V2 lifecycle actions. -/
inductive Command where
  /-- Lock the exact shard basket and mint `quantity` receipt atoms. -/
  | issue (quantity expectedRevision : Nat)
  /-- Burn `quantity` receipt atoms and release the exact shard basket. -/
  | unwrap (quantity expectedRevision : Nat)
  /-- Burn `quantity` receipt atoms after terminal resolution and settle exactly. -/
  | terminalRedeem (quantity expectedRevision : Nat)
  /-- Close a zero-supply, zero-custody Structured node and recover rent. -/
  | retire (expectedRevision : Nat)
  deriving DecidableEq, Repr

/-- Optimistic replay coordinate carried by every command. -/
def Command.expectedRevision : Command → Nat
  | .issue _ revision | .unwrap _ revision | .terminalRedeem _ revision | .retire revision =>
      revision

/-- Receipt atoms moved by the command; retirement moves none. -/
def Command.quantity : Command → Nat
  | .issue quantity _ | .unwrap quantity _ | .terminalRedeem quantity _ => quantity
  | .retire _ => 0

/-- One complete admission frame. -/
structure Frame where
  /-- Executable checked-arithmetic ceiling; a profile bound, not an ontology bound. -/
  scalarLimit : Nat
  /-- Immutable finalized basis. -/
  basis : Basis
  /-- Structured-owned persisted root. -/
  root : Root
  /-- Adapter-owned runtime projection. -/
  projection : Projection
  /-- Exact observed actor receipt balance. -/
  holderReceipts : Nat
  /-- Selected action. -/
  command : Command
  deriving DecidableEq, Repr

/-- Terminal payout vectors must match the authenticated representation width. -/
def Phase.payoutWellFormed (representationWidth : Nat) : Phase → Bool
  | .open | .retired => true
  | .terminal payoutPerClaim => payoutPerClaim.length = representationWidth

/-- Every identity, width, denominator, and revision join.  A substituted basis,
substituted shard layer, or stale revision refuses here. -/
def staticAccepts (frame : Frame) : Bool :=
  frame.basis.valid frame.scalarLimit &&
  frame.root.basisId = frame.basis.termsId &&
  frame.root.marketId = frame.basis.marketId &&
  frame.root.rentBeneficiaryId != 0 &&
  frame.projection.basisId = frame.basis.termsId &&
  frame.projection.marketId = frame.basis.marketId &&
  frame.projection.shardTermsId = frame.basis.shardTermsId &&
  frame.projection.shardDenominator = frame.basis.denominator &&
  frame.projection.representationWidth = frame.basis.representationWidth &&
  frame.projection.custody.surplus.length = frame.basis.representationWidth &&
  frame.projection.phase.payoutWellFormed frame.basis.representationWidth &&
  frame.projection.revision = frame.root.revision &&
  frame.root.revision + 1 < frame.scalarLimit &&
  frame.projection.custody.receiptSupply < frame.scalarLimit &&
  frame.command.expectedRevision = frame.root.revision

/-- Every product that the executable profile must hold without overflow. -/
def productsFit (frame : Frame) (receiptSupply : Nat) : Bool :=
  receiptSupply < frame.scalarLimit &&
  (frame.basis.requiredCustody receiptSupply).all
    fun shardAtoms => shardAtoms < frame.scalarLimit

/-- Phase-dependent and balance-dependent command admission. -/
def commandAccepts (frame : Frame) : Bool :=
  let custody := frame.projection.custody
  match frame.command with
  | .issue quantity _ =>
      frame.projection.phase = .open && 0 < quantity &&
      productsFit frame quantity &&
      productsFit frame (custody.receiptSupply + quantity)
  | .unwrap quantity _ =>
      frame.projection.phase = .open && 0 < quantity &&
      quantity ≤ custody.receiptSupply && quantity ≤ frame.holderReceipts &&
      productsFit frame quantity && productsFit frame custody.receiptSupply
  | .terminalRedeem quantity _ =>
      (match frame.projection.phase with
       | .terminal _ => true
       | _ => false) && 0 < quantity &&
      quantity ≤ custody.receiptSupply && quantity ≤ frame.holderReceipts &&
      productsFit frame quantity && productsFit frame custody.receiptSupply
  | .retire _ =>
      (match frame.projection.phase with
       | .terminal _ => true
       | _ => false) &&
      custody.receiptSupply = 0 &&
      custody.surplus.all fun donated => donated = 0

/-- Total admission boundary. -/
def accepts (frame : Frame) : Bool :=
  staticAccepts frame && commandAccepts frame

/-! ## Exact terminal settlement -/

/-- Exact terminal settlement of one representation coordinate.  Every field is
derived; none is accepted from a caller. -/
structure CoordinateSettlement where
  /-- Claims representation coordinate in `[0,K)`. -/
  representationCoordinate : Nat
  /-- Exact shard atoms released from Structured custody: `quantity * c_i`. -/
  releasedShards : Nat
  /-- Whole native claims represented by the released shards. -/
  wholeClaims : Nat
  /-- Exact whole-denominator multiple redeemed at the shard layer. -/
  burnedShards : Nat
  /-- Explicit same-Mint change that stays transferable and aggregable. -/
  changeShards : Nat
  /-- Authenticated collateral atoms per whole native claim. -/
  payoutPerClaim : Nat
  /-- Exact collateral atoms; zero is a valid, honest result. -/
  collateralAtoms : Nat
  deriving DecidableEq, Repr

/-- Settle one coordinate through the protocol's sole quotient/remainder
boundary.  Structured introduces no rounding of its own. -/
def settleCoordinate
    (denominator representationCoordinate releasedShards payoutPerClaim : Nat) :
    Option CoordinateSettlement :=
  match FractionalClaimV1.divideClaimShardsV1 denominator releasedShards with
  | some division =>
      some {
        representationCoordinate
        releasedShards := division.inputShards
        wholeClaims := division.wholeNativeClaims
        burnedShards := division.consumedShards
        changeShards := division.changeShards
        payoutPerClaim
        collateralAtoms := division.wholeNativeClaims * payoutPerClaim
      }
  | none => none

/-- The full terminal settlement of `quantity` receipts, in coordinate order. -/
def terminalSettlement
    (basis : Basis) (payoutPerClaim : List Nat) (quantity : Nat) :
    Option (List CoordinateSettlement) :=
  ((List.range basis.representationWidth).zip
      (basis.requiredCustody quantity |>.zip payoutPerClaim)).foldr
    (fun entry accumulated =>
      match accumulated with
      | none => none
      | some rows =>
          match settleCoordinate basis.denominator entry.1 entry.2.1 entry.2.2 with
          | none => none
          | some row => some (row :: rows))
    (some [])

/-- Total collateral atoms paid by one terminal settlement. -/
def totalCollateral (rows : List CoordinateSettlement) : Nat :=
  (rows.map fun row => row.collateralAtoms).sum

/-- Structured does not own a second quotient/remainder boundary: its settlement
is exactly the shard layer's division. -/
theorem structured_reuses_the_fractional_boundary
    (denominator representationCoordinate releasedShards payoutPerClaim : Nat)
    (settlement : CoordinateSettlement)
    (built : settleCoordinate denominator representationCoordinate releasedShards
      payoutPerClaim = some settlement) :
    FractionalClaimV1.divideClaimShardsV1 denominator releasedShards = some {
      inputShards := settlement.releasedShards
      wholeNativeClaims := settlement.wholeClaims
      consumedShards := settlement.burnedShards
      changeShards := settlement.changeShards
    } := by
  unfold settleCoordinate at built
  cases division : FractionalClaimV1.divideClaimShardsV1 denominator releasedShards with
  | none => rw [division] at built; exact absurd built (by simp)
  | some value =>
      rw [division] at built
      simp only [Option.some.injEq] at built
      subst built
      rfl

/-- Exact settlement with no hidden rounding: the released basket decomposes
into a whole-denominator multiple plus explicit sub-denominator change, and the
payout is exactly whole claims times the authenticated per-claim payout. -/
theorem settlement_is_exact
    (denominator representationCoordinate releasedShards payoutPerClaim : Nat)
    (positive : 0 < denominator) (settlement : CoordinateSettlement)
    (built : settleCoordinate denominator representationCoordinate releasedShards
      payoutPerClaim = some settlement) :
    settlement.releasedShards = settlement.burnedShards + settlement.changeShards ∧
    settlement.burnedShards = denominator * settlement.wholeClaims ∧
    settlement.changeShards < denominator ∧
    settlement.collateralAtoms = settlement.wholeClaims * settlement.payoutPerClaim := by
  unfold settleCoordinate FractionalClaimV1.divideClaimShardsV1 at built
  rw [if_pos positive] at built
  simp only [Option.some.injEq] at built
  subst built
  refine ⟨?_, rfl, Nat.mod_lt _ positive, rfl⟩
  exact (Nat.div_add_mod releasedShards denominator).symm

/-- Terminal-zero honesty: an authenticated zero payout settles for exactly zero
collateral atoms.  Losing coordinates cannot pay. -/
theorem zero_payout_settles_zero
    (denominator representationCoordinate releasedShards : Nat)
    (settlement : CoordinateSettlement)
    (built : settleCoordinate denominator representationCoordinate releasedShards 0 =
      some settlement) :
    settlement.collateralAtoms = 0 ∧ settlement.payoutPerClaim = 0 := by
  unfold settleCoordinate at built
  cases division : FractionalClaimV1.divideClaimShardsV1 denominator releasedShards with
  | none => rw [division] at built; exact absurd built (by simp)
  | some value =>
      rw [division] at built
      simp only [Option.some.injEq] at built
      subst built
      exact ⟨Nat.mul_zero _, rfl⟩

/-- A zero-payout row contributes exactly nothing to the total settlement. -/
theorem zero_payout_row_does_not_change_total
    (rows : List CoordinateSettlement) (row : CoordinateSettlement)
    (zero : row.collateralAtoms = 0) :
    totalCollateral (row :: rows) = totalCollateral rows := by
  simp [totalCollateral, zero]

/-- A settlement made only of authenticated zero-payout coordinates pays exactly
zero collateral atoms. -/
theorem all_losing_settlement_pays_zero
    (rows : List CoordinateSettlement)
    (losing : ∀ row ∈ rows, row.payoutPerClaim = 0)
    (derived : ∀ row ∈ rows, row.collateralAtoms = row.wholeClaims * row.payoutPerClaim) :
    totalCollateral rows = 0 := by
  induction rows with
  | nil => rfl
  | cons head tail induction =>
      have headZero : head.collateralAtoms = 0 := by
        rw [derived head (by simp), losing head (by simp), Nat.mul_zero]
      have tailTotal : totalCollateral tail = 0 :=
        induction (fun row member => losing row (by simp [member]))
          (fun row member => derived row (by simp [member]))
      rw [zero_payout_row_does_not_change_total tail head headZero, tailTotal]

/-- Explicit change is aggregable: combining two remainders yields a further
whole claim exactly when their sum reaches the denominator.  This is why a
sub-denominator remainder is a transferable instrument, not a rounding loss. -/
theorem change_shards_are_aggregable
    (denominator left right : Nat) (positive : 0 < denominator) :
    (left + right) / denominator =
      left / denominator + right / denominator +
        (left % denominator + right % denominator) / denominator := by
  have leftSplit : denominator * (left / denominator) + left % denominator = left :=
    Nat.div_add_mod left denominator
  have rightSplit : denominator * (right / denominator) + right % denominator = right :=
    Nat.div_add_mod right denominator
  have expand :
      left + right =
        denominator * (left / denominator) + denominator * (right / denominator) +
          (left % denominator + right % denominator) := by
    omega
  rw [expand, ← Nat.mul_add, Nat.mul_add_div positive]

/-! ## Transition, replay, and refusal -/

/-- Structured-owned post state.  Supply and custody stay with their owners. -/
def rootPost (frame : Frame) : Root :=
  { frame.root with revision := frame.root.revision + 1 }

/-- Receipt supply after the accepted command. -/
def receiptSupplyPost (frame : Frame) : Nat :=
  match frame.command with
  | .issue quantity _ => frame.projection.custody.receiptSupply + quantity
  | .unwrap quantity _ | .terminalRedeem quantity _ =>
      frame.projection.custody.receiptSupply - quantity
  | .retire _ => frame.projection.custody.receiptSupply

/-- Exact typed effect emitted only after full admission. -/
inductive Effect where
  /-- Lock the exact basket and mint receipts to the actor. -/
  | lockAndMint (quantity : Nat) (basket : List Nat)
  /-- Burn receipts and release the exact basket to the actor. -/
  | burnAndRelease (quantity : Nat) (basket : List Nat)
  /-- Burn receipts, release the exact basket, and settle every coordinate. -/
  | burnAndSettle (quantity : Nat) (basket : List Nat) (rows : List CoordinateSettlement)
  /-- Close the zero-supply Structured node to its permanent beneficiary. -/
  | closeToBeneficiary (rentBeneficiaryId : Nat)
  deriving DecidableEq, Repr

/-- Derive the exact effect for one frame, or refuse. -/
def effect (frame : Frame) : Option Effect :=
  match frame.command with
  | .issue quantity _ =>
      some (.lockAndMint quantity (frame.basis.requiredCustody quantity))
  | .unwrap quantity _ =>
      some (.burnAndRelease quantity (frame.basis.requiredCustody quantity))
  | .terminalRedeem quantity _ =>
      match frame.projection.phase with
      | .terminal payoutPerClaim =>
          match terminalSettlement frame.basis payoutPerClaim quantity with
          | some rows =>
              some (.burnAndSettle quantity (frame.basis.requiredCustody quantity) rows)
          | none => none
      | _ => none
  | .retire _ => some (.closeToBeneficiary frame.root.rentBeneficiaryId)

/-- Stable refusal cause. -/
inductive Refusal where
  /-- Identity, width, denominator, phase, balance, or replay admission failed. -/
  | notAdmissible
  /-- The exact terminal settlement could not be derived. -/
  | settlementUnavailable
  deriving DecidableEq, Repr

/-- Accepted transition result. -/
structure Settlement (frame : Frame) where
  /-- Structured-owned post root. -/
  rootPost : Root
  /-- Receipt supply after the command. -/
  receiptSupplyPost : Nat
  /-- Exact typed effect. -/
  effect : Effect
  /-- Full admission held. -/
  accepted : accepts frame = true
  /-- The post root is exactly the derived one. -/
  rootExact : rootPost = StructuredV2.rootPost frame
  /-- The post supply is exactly the derived one. -/
  supplyExact : receiptSupplyPost = StructuredV2.receiptSupplyPost frame
  /-- The effect is exactly the derived one. -/
  effectExact : some effect = StructuredV2.effect frame

/-- Total transition boundary. -/
def execute? (frame : Frame) : Except Refusal (Settlement frame) :=
  if accepted : accepts frame = true then
    match derived : effect frame with
    | some value =>
        .ok {
          rootPost := rootPost frame
          receiptSupplyPost := receiptSupplyPost frame
          effect := value
          accepted
          rootExact := rfl
          supplyExact := rfl
          effectExact := derived.symm
        }
    | none => .error .settlementUnavailable
  else .error .notAdmissible

/-- Observable Structured state rolls back on every refusal.  Physical atomic
rollback across Token, Claims, and Custody remains an adapter obligation. -/
def runRoot (frame : Frame) : Root :=
  match execute? frame with
  | .ok settlement => settlement.rootPost
  | .error _ => frame.root

/-- Emitted effect, absent on refusal. -/
def emittedEffect? (frame : Frame) : Option Effect :=
  match execute? frame with
  | .ok settlement => some settlement.effect
  | .error _ => none

theorem refusal_rolls_back_root
    (frame : Frame) (cause : Refusal) (failed : execute? frame = .error cause) :
    runRoot frame = frame.root := by
  unfold runRoot
  rw [failed]

theorem refusal_emits_no_effect
    (frame : Frame) (cause : Refusal) (failed : execute? frame = .error cause) :
    emittedEffect? frame = none := by
  unfold emittedEffect?
  rw [failed]

theorem successful_root_is_exact (frame : Frame) (settlement : Settlement frame) :
    settlement.rootPost = rootPost frame :=
  settlement.rootExact

theorem successful_effect_is_exact (frame : Frame) (settlement : Settlement frame) :
    some settlement.effect = effect frame :=
  settlement.effectExact

/-- Every accepted command advances the replay revision by exactly one. -/
theorem accepted_command_advances_revision (frame : Frame) :
    (rootPost frame).revision = frame.root.revision + 1 :=
  rfl

/-- Admission binds the command's optimistic revision to the persisted root. -/
theorem accepts_binds_revision (frame : Frame) (accepted : accepts frame = true) :
    frame.command.expectedRevision = frame.root.revision := by
  unfold accepts staticAccepts at accepted
  simp only [Bool.and_eq_true, decide_eq_true_eq] at accepted
  exact accepted.1.2

/-- Replay protection: the exact frame that was accepted refuses against its own
post state, because the command's expected revision no longer matches. -/
theorem replayed_command_is_refused
    (frame : Frame) (accepted : accepts frame = true) :
    accepts { frame with
      root := rootPost frame
      projection := { frame.projection with revision := (rootPost frame).revision } } = false := by
  have bound := accepts_binds_revision frame accepted
  apply Bool.eq_false_iff.mpr
  intro replayed
  have replayedBound := accepts_binds_revision _ replayed
  simp only [rootPost] at replayedBound
  omega

/-- Nonzero receipt supply refuses retirement. -/
theorem nonzero_supply_refuses_retirement
    (frame : Frame) (expectedRevision : Nat)
    (retiring : frame.command = .retire expectedRevision)
    (outstanding : frame.projection.custody.receiptSupply ≠ 0) :
    accepts frame = false := by
  unfold accepts commandAccepts
  rw [retiring]
  simp [outstanding]

/-- Donated surplus refuses retirement: a Structured node closes only when its
observed custody is exactly empty. -/
theorem surplus_refuses_retirement
    (frame : Frame) (expectedRevision donated : Nat)
    (retiring : frame.command = .retire expectedRevision)
    (member : donated ∈ frame.projection.custody.surplus)
    (positive : donated ≠ 0) :
    accepts frame = false := by
  unfold accepts commandAccepts
  rw [retiring]
  have notAll : (frame.projection.custody.surplus.all fun value => value = 0) = false := by
    apply Bool.eq_false_iff.mpr
    intro allZero
    simp only [List.all_eq_true, decide_eq_true_eq] at allZero
    exact positive (allZero donated member)
  simp [notAll]

/-- A substituted shard layer refuses: the observed shard terms must equal the
immutable basis selection. -/
theorem substituted_shard_terms_refused
    (frame : Frame) (different : frame.projection.shardTermsId ≠ frame.basis.shardTermsId) :
    accepts frame = false := by
  unfold accepts staticAccepts
  simp [different]

/-- A substituted shard denominator refuses. -/
theorem substituted_denominator_refused
    (frame : Frame) (different : frame.projection.shardDenominator ≠ frame.basis.denominator) :
    accepts frame = false := by
  unfold accepts staticAccepts
  simp [different]

/-- A basis backed by nothing is inadmissible: at least one coefficient must be
positive, so a receipt atom always denotes real exposure. -/
theorem all_zero_coefficients_refused
    (scalarLimit : Nat) (basis : Basis)
    (empty : ∀ coefficient ∈ basis.coefficients, coefficient = 0) :
    basis.valid scalarLimit = false := by
  unfold Basis.valid
  have none : (basis.coefficients.any fun coefficient => 0 < coefficient) = false := by
    simp only [Bool.eq_false_iff, ne_eq, List.any_eq_true, decide_eq_true_eq, not_exists]
    intro coefficient
    simp only [not_and, Nat.not_lt, Nat.le_zero_eq]
    intro member
    exact empty coefficient member
  simp [none]

/-- A degenerate denominator refuses: exact fractional denomination requires a
shard denominator greater than one. -/
theorem degenerate_denominator_refused
    (scalarLimit : Nat) (basis : Basis) (degenerate : basis.denominator ≤ 1) :
    basis.valid scalarLimit = false := by
  apply Bool.eq_false_iff.mpr
  intro valid
  unfold Basis.valid at valid
  simp only [Bool.and_eq_true, decide_eq_true_eq] at valid
  exact absurd valid.1.2 (Nat.not_lt.mpr degenerate)

/-- A coordinate carrying a zero coefficient is admissible but inert: it locks
no shard atom and settles for exactly zero. -/
theorem zero_coefficient_row_is_inert
    (basis : Basis) (quantity coordinate : Nat)
    (zero : basis.coefficients.getD coordinate 0 = 0) :
    (basis.requiredCustody quantity).getD coordinate 0 = 0 := by
  unfold Basis.requiredCustody scaleCoefficients
  rw [List.getD_eq_getElem?_getD, List.getElem?_map]
  rw [List.getD_eq_getElem?_getD] at zero
  cases lookup : basis.coefficients[coordinate]? with
  | none => simp
  | some value =>
      rw [lookup] at zero
      simp only [Option.getD] at zero ⊢
      simp [zero]

/-- Overflow refusal: a product beyond the executable checked ceiling refuses
rather than wrapping. -/
theorem overflow_refuses_issue
    (frame : Frame) (quantity expectedRevision shardAtoms : Nat)
    (issuing : frame.command = .issue quantity expectedRevision)
    (member : shardAtoms ∈ frame.basis.requiredCustody quantity)
    (overflowing : frame.scalarLimit ≤ shardAtoms) :
    accepts frame = false := by
  unfold accepts commandAccepts productsFit
  rw [issuing]
  have notAll :
      ((frame.basis.requiredCustody quantity).all
        fun atoms => atoms < frame.scalarLimit) = false := by
    apply Bool.eq_false_iff.mpr
    intro allFit
    simp only [List.all_eq_true, decide_eq_true_eq] at allFit
    exact absurd (allFit shardAtoms member) (Nat.not_lt.mpr overflowing)
  simp [notAll]

end DClutch.StructuredV2
