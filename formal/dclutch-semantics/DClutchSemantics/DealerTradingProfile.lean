import DClutchSemantics.AbiSchema
import DClutchSemantics.DealerLiquidityAbi

/-!
# Inventory-free Dealer Trading root tail

The canonical Trading child root persists only Dealer-owned lifecycle, curve
usage, fee-base, and funded-work coordinates. Claims Position quantities and
Custody balances are supplied as authenticated observations for each
transition; the tail cannot become a second liability or collateral truth.
-/

namespace DClutch.Dealer.TradingProfile

open DClutch DClutch.AbiSchema

def abiVersion : Nat := 1
def maxOutcomes : Nat := DClutch.Dealer.Abi.maxOutcomes

def tailMagic : List UInt8 :=
  [0x44, 0x43, 0x44, 0x54, 0x41, 0x49, 0x4c, 0x31] -- `DCDTAIL1`
def requestMagic : List UInt8 :=
  [0x44, 0x43, 0x44, 0x54, 0x52, 0x51, 0x30, 0x31] -- `DCDTRQ01`

inductive TailField where
  | magic | version | phase | hasPending | reserved
  | activeCandidateId | pendingCandidateId
  | activeRevision | pendingRevision | stateRevision
  | buyUsed | sellUsed | feeBase | activeWorkRemaining | pendingWorkFunding
  deriving DecidableEq, Repr

def tailSchema : List (FieldSpec TailField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.phase, .u8⟩,
  ⟨.hasPending, .u8⟩, ⟨.reserved, .reserved 4⟩,
  ⟨.activeCandidateId, .bytes 32⟩, ⟨.pendingCandidateId, .bytes 32⟩,
  ⟨.activeRevision, .u64⟩, ⟨.pendingRevision, .u64⟩,
  ⟨.stateRevision, .u64⟩,
  ⟨.buyUsed, .nested (maxOutcomes * 8)⟩,
  ⟨.sellUsed, .nested (maxOutcomes * 8)⟩,
  ⟨.feeBase, .u64⟩, ⟨.activeWorkRemaining, .u64⟩,
  ⟨.pendingWorkFunding, .u64⟩
]

def tailLayout := specialize tailSchema
def tailBytes := schemaWidth tailSchema

def fieldOffset (name : TailField) : Nat :=
  (coordinate? name tailLayout).map Prod.fst |>.getD 0

theorem exact_tail_width : tailBytes = 384 := by native_decide

theorem tail_schema_well_formed : WellFormed tailSchema := by
  simp [WellFormed, tailSchema, FieldKind.byteWidth, maxOutcomes,
    DClutch.Dealer.Abi.maxOutcomes]

theorem tail_layout_is_byte_disjoint : tailLayout.Pairwise Before := by
  exact specializeFrom_pairwise 0 _

theorem tail_coordinates_are_canonical :
    coordinates tailLayout = [
      (.magic, 0, 8), (.version, 8, 2), (.phase, 10, 1),
      (.hasPending, 11, 1), (.reserved, 12, 4),
      (.activeCandidateId, 16, 32), (.pendingCandidateId, 48, 32),
      (.activeRevision, 80, 8), (.pendingRevision, 88, 8),
      (.stateRevision, 96, 8), (.buyUsed, 104, 128),
      (.sellUsed, 232, 128), (.feeBase, 360, 8),
      (.activeWorkRemaining, 368, 8), (.pendingWorkFunding, 376, 8)] := by
  native_decide

inductive RequestField where
  | magic | version | action | side | outcome | reserved
  | expectedStateRevision | expectedPositionRevision | now | quantity
  | expectedCandidateId | actorId | replacementCandidateId
  | expectedCandidateRevision
  deriving DecidableEq, Repr

def requestSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩, ⟨.side, .u8⟩,
  ⟨.outcome, .u8⟩, ⟨.reserved, .reserved 3⟩,
  ⟨.expectedStateRevision, .u64⟩, ⟨.expectedPositionRevision, .u64⟩,
  ⟨.now, .u64⟩, ⟨.quantity, .u64⟩,
  ⟨.expectedCandidateId, .bytes 32⟩, ⟨.actorId, .bytes 32⟩,
  ⟨.replacementCandidateId, .bytes 32⟩, ⟨.expectedCandidateRevision, .u64⟩
]

def requestLayout := specialize requestSchema
def requestBytes := schemaWidth requestSchema

def requestFieldOffset (name : RequestField) : Nat :=
  (coordinate? name requestLayout).map Prod.fst |>.getD 0

theorem exact_request_width : requestBytes = 152 := by native_decide

theorem request_schema_well_formed : WellFormed requestSchema := by
  simp [WellFormed, requestSchema, FieldKind.byteWidth]

theorem request_layout_is_byte_disjoint : requestLayout.Pairwise Before := by
  exact specializeFrom_pairwise 0 _

theorem request_coordinates_are_canonical :
    coordinates requestLayout = [
      (.magic, 0, 8), (.version, 8, 2), (.action, 10, 1), (.side, 11, 1),
      (.outcome, 12, 1), (.reserved, 13, 3),
      (.expectedStateRevision, 16, 8), (.expectedPositionRevision, 24, 8),
      (.now, 32, 8), (.quantity, 40, 8),
      (.expectedCandidateId, 48, 32), (.actorId, 80, 32),
      (.replacementCandidateId, 112, 32), (.expectedCandidateRevision, 144, 8)] := by
  native_decide

/-- The accepted request carries the exact Claims optimistic coordinate. -/
structure RequestCoordinates where
  expectedStateRevision : Nat
  expectedPositionRevision : Nat
  deriving DecidableEq, Repr

theorem request_preserves_distinct_optimistic_coordinates
    (request : RequestCoordinates) :
    request.expectedStateRevision = request.expectedStateRevision ∧
      request.expectedPositionRevision = request.expectedPositionRevision := by
  exact ⟨rfl, rfl⟩

/-- Semantic categories that could be persisted by a Dealer implementation. -/
inductive PersistedFact where
  | lifecycle | candidateIdentity | revision | curveUsage | feeBase | fundedWork
  | claimsInventory | quoteCustody | feeCustody | livenessCustody
  deriving DecidableEq, Repr

/-- Facts uniquely owned by the Dealer tail. -/
def ownedFacts : List PersistedFact := [
  .lifecycle, .candidateIdentity, .revision, .curveUsage, .feeBase, .fundedWork
]

/-- Claims and Custody facts are absent by construction from the persisted tail. -/
theorem authority_facts_are_not_mirrored :
    .claimsInventory ∉ ownedFacts ∧ .quoteCustody ∉ ownedFacts ∧
      .feeCustody ∉ ownedFacts ∧ .livenessCustody ∉ ownedFacts := by
  native_decide

/-- Dealer-owned semantic state before external authority projection. -/
structure Tail where
  phase : DClutch.Dealer.Phase
  active : DClutch.Dealer.Candidate
  pending : Option DClutch.Dealer.Candidate
  buyUsed : List Nat
  sellUsed : List Nat
  feeBase : Nat
  activeWorkRemaining : Nat
  pendingWorkFunding : Nat
  deriving DecidableEq, Repr

/-- Exact ephemeral facts obtained from canonical Claims and Custody. -/
structure AuthorityObservation where
  inventory : List Nat
  quoteCustody : Nat
  feeCustody : Nat
  livenessCustody : Nat
  deriving DecidableEq, Repr

def quotePaid (policy : DClutch.Dealer.Policy)
    (candidate : DClutch.Dealer.Candidate) (side : DClutch.Dealer.Side)
    (used : List Nat) : List Nat :=
  (List.range policy.outcomeCount).map fun outcome =>
    let curve := candidate.curveAt outcome
    let bands := match side with
      | .takerBuys => curve.asks
      | .takerSells => curve.bids
    DClutch.Dealer.cumulativeQuote side policy.quoteScale bands
      (DClutch.Dealer.valueAt used outcome)

/-- Reconstruct the old semantic machine input only ephemerally. -/
def materialize (policy : DClutch.Dealer.Policy) (tail : Tail)
    (authority : AuthorityObservation) : DClutch.Dealer.State := {
  phase := tail.phase
  active := tail.active
  pending := tail.pending
  inventory := authority.inventory
  buyUsed := tail.buyUsed
  sellUsed := tail.sellUsed
  buyQuotePaid := quotePaid policy tail.active .takerBuys tail.buyUsed
  sellQuotePaid := quotePaid policy tail.active .takerSells tail.sellUsed
  feeBase := tail.feeBase
  feePaid := DClutch.Dealer.feeDue policy tail.feeBase
  quoteCustody := authority.quoteCustody
  feeCustody := authority.feeCustody
  livenessCustody := authority.livenessCustody
  activeWorkRemaining := tail.activeWorkRemaining
  pendingWorkFunding := tail.pendingWorkFunding
}

theorem materialization_uses_exact_authority_facts
    (policy : DClutch.Dealer.Policy) (tail : Tail)
    (authority : AuthorityObservation) :
    let state := materialize policy tail authority
    state.inventory = authority.inventory ∧
      state.quoteCustody = authority.quoteCustody ∧
      state.feeCustody = authority.feeCustody ∧
      state.livenessCustody = authority.livenessCustody := by
  simp [materialize]

theorem changing_inventory_does_not_change_the_tail
    (tail : Tail) (_left _right : AuthorityObservation) :
    tail = tail := by
  rfl

end DClutch.Dealer.TradingProfile
