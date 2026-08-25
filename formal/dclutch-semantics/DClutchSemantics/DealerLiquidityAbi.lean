import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import DClutchSemantics.DealerLiquidity

/-!
# Fixed-layout Dealer liquidity ABI

This is a measured physical profile for the width-independent Dealer
semantics.  Runtime quote curves are fixed-capacity data, not monomorphized
handlers.  Inactive outcomes and bands are canonically zero.  The limit of 16
outcomes and eight bid/eight ask bands per outcome is provisional and liftable
by regenerating the profile; it is not a semantic limit.
-/

namespace DClutch.Dealer.Abi

open DClutch DClutch.AbiSchema

def abiVersion : Nat := 1
def maxOutcomes : Nat := 16
def maxBandsPerSide : Nat := 8
def bandBytes : Nat := 16
def outcomeCurveBytes : Nat := 8 + 2 * maxBandsPerSide * bandBytes
def maxCustodyTransfers : Nat := 3

def policyMagic : List UInt8 :=
  [0x44, 0x43, 0x44, 0x50, 0x4f, 0x4c, 0x59, 0x31] -- `DCDPOLY1`
def candidateMagic : List UInt8 :=
  [0x44, 0x43, 0x44, 0x43, 0x41, 0x4e, 0x44, 0x31] -- `DCDCAND1`
def stateMagic : List UInt8 :=
  [0x44, 0x43, 0x44, 0x53, 0x54, 0x41, 0x54, 0x31] -- `DCDSTAT1`
def receiptMagic : List UInt8 :=
  [0x44, 0x43, 0x44, 0x52, 0x43, 0x50, 0x54, 0x31] -- `DCDRCPT1`
def requestMagic : List UInt8 :=
  [0x44, 0x43, 0x44, 0x52, 0x45, 0x51, 0x30, 0x31] -- `DCDREQ01`

inductive PolicyField where
  | magic | version | outcomeCount | reserved | marketId | releaseSetId
  | dealerId | resolutionAuthorityId | feeRecipientId | unwindRecipientId
  | quoteScale | feeNumerator | feeDenominator | minimumWorkFunding
  | replacementDelay
  deriving DecidableEq, Repr

def policySchema : List (FieldSpec PolicyField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.outcomeCount, .u8⟩,
  ⟨.reserved, .reserved 5⟩, ⟨.marketId, .bytes 32⟩,
  ⟨.releaseSetId, .bytes 32⟩, ⟨.dealerId, .bytes 32⟩,
  ⟨.resolutionAuthorityId, .bytes 32⟩, ⟨.feeRecipientId, .bytes 32⟩,
  ⟨.unwindRecipientId, .bytes 32⟩, ⟨.quoteScale, .u64⟩,
  ⟨.feeNumerator, .u64⟩, ⟨.feeDenominator, .u64⟩,
  ⟨.minimumWorkFunding, .u64⟩, ⟨.replacementDelay, .u64⟩
]

inductive CandidateField where
  | magic | version | outcomeCount | reserved | candidateId | revision
  | validFrom | expiresAt | quoteReserveFloor | workFunding | workReward
  | minimumInventory | maximumInventory | curves
  deriving DecidableEq, Repr

def candidateSchema : List (FieldSpec CandidateField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.outcomeCount, .u8⟩,
  ⟨.reserved, .reserved 5⟩, ⟨.candidateId, .bytes 32⟩,
  ⟨.revision, .u64⟩, ⟨.validFrom, .u64⟩, ⟨.expiresAt, .u64⟩,
  ⟨.quoteReserveFloor, .u64⟩, ⟨.workFunding, .u64⟩,
  ⟨.workReward, .u64⟩, ⟨.minimumInventory, .nested (maxOutcomes * 8)⟩,
  ⟨.maximumInventory, .nested (maxOutcomes * 8)⟩,
  ⟨.curves, .nested (maxOutcomes * outcomeCurveBytes)⟩
]

inductive StateField where
  | magic | version | phase | outcomeCount | hasPending | winner | reservedA
  | activeCandidateId | pendingCandidateId | releaseSetId | activeRevision
  | pendingRevision | stateRevision | reservedB | inventory | buyUsed | sellUsed
  | buyQuotePaid | sellQuotePaid | feeBase | feePaid | quoteCustody
  | feeCustody | livenessCustody | activeWorkRemaining | pendingWorkFunding
  deriving DecidableEq, Repr

def stateSchema : List (FieldSpec StateField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.phase, .u8⟩,
  ⟨.outcomeCount, .u8⟩, ⟨.hasPending, .u8⟩, ⟨.winner, .u8⟩,
  ⟨.reservedA, .reserved 2⟩, ⟨.activeCandidateId, .bytes 32⟩,
  ⟨.pendingCandidateId, .bytes 32⟩, ⟨.releaseSetId, .bytes 32⟩,
  ⟨.activeRevision, .u64⟩, ⟨.pendingRevision, .u64⟩,
  ⟨.stateRevision, .u64⟩, ⟨.reservedB, .reserved 8⟩,
  ⟨.inventory, .nested (maxOutcomes * 8)⟩,
  ⟨.buyUsed, .nested (maxOutcomes * 8)⟩,
  ⟨.sellUsed, .nested (maxOutcomes * 8)⟩,
  ⟨.buyQuotePaid, .nested (maxOutcomes * 8)⟩,
  ⟨.sellQuotePaid, .nested (maxOutcomes * 8)⟩,
  ⟨.feeBase, .u64⟩, ⟨.feePaid, .u64⟩,
  ⟨.quoteCustody, .u64⟩, ⟨.feeCustody, .u64⟩,
  ⟨.livenessCustody, .u64⟩, ⟨.activeWorkRemaining, .u64⟩,
  ⟨.pendingWorkFunding, .u64⟩
]

inductive ReceiptField where
  | magic | version | role | flags | reserved | registryProgram | releaseSetId
  | program | artifactRelease | semanticRelease
  deriving DecidableEq, Repr

def receiptSchema : List (FieldSpec ReceiptField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.role, .u8⟩,
  ⟨.flags, .u8⟩, ⟨.reserved, .reserved 4⟩,
  ⟨.registryProgram, .bytes 32⟩, ⟨.releaseSetId, .bytes 32⟩,
  ⟨.program, .bytes 32⟩, ⟨.artifactRelease, .bytes 32⟩,
  ⟨.semanticRelease, .bytes 32⟩
]

inductive RequestField where
  | magic | version | action | side | outcome | reserved | expectedStateRevision
  | now | quantity | expectedCandidateId | actorId | replacementCandidateId
  | expectedCandidateRevision
  deriving DecidableEq, Repr

def requestSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩,
  ⟨.side, .u8⟩, ⟨.outcome, .u8⟩, ⟨.reserved, .reserved 3⟩,
  ⟨.expectedStateRevision, .u64⟩, ⟨.now, .u64⟩,
  ⟨.quantity, .u64⟩, ⟨.expectedCandidateId, .bytes 32⟩,
  ⟨.actorId, .bytes 32⟩, ⟨.replacementCandidateId, .bytes 32⟩,
  ⟨.expectedCandidateRevision, .u64⟩
]

def policyLayout := specialize policySchema
def candidateLayout := specialize candidateSchema
def stateLayout := specialize stateSchema
def receiptLayout := specialize receiptSchema
def requestLayout := specialize requestSchema

def policyBytes := schemaWidth policySchema
def candidateBytes := schemaWidth candidateSchema
def stateBytes := schemaWidth stateSchema
def receiptBytes := schemaWidth receiptSchema
def requestBytes := schemaWidth requestSchema

theorem exact_physical_widths :
    bandBytes = 16 ∧ outcomeCurveBytes = 264 ∧ policyBytes = 248 ∧
    candidateBytes = 4576 ∧ stateBytes = 840 ∧ receiptBytes = 176 ∧
    requestBytes = 144 := by native_decide

theorem schemas_well_formed :
    WellFormed policySchema ∧ WellFormed candidateSchema ∧
    WellFormed stateSchema ∧ WellFormed receiptSchema ∧
    WellFormed requestSchema := by
  simp [WellFormed, policySchema, candidateSchema, stateSchema, receiptSchema,
    requestSchema, FieldKind.byteWidth, maxOutcomes, maxBandsPerSide, bandBytes,
    outcomeCurveBytes]

theorem layouts_are_byte_disjoint :
    policyLayout.Pairwise Before ∧ candidateLayout.Pairwise Before ∧
    stateLayout.Pairwise Before ∧ receiptLayout.Pairwise Before ∧
    requestLayout.Pairwise Before := by
  exact ⟨specializeFrom_pairwise 0 _, specializeFrom_pairwise 0 _,
    specializeFrom_pairwise 0 _, specializeFrom_pairwise 0 _,
    specializeFrom_pairwise 0 _⟩

theorem header_coordinates_are_canonical :
    coordinates policyLayout = [
      (.magic, 0, 8), (.version, 8, 2), (.outcomeCount, 10, 1),
      (.reserved, 11, 5), (.marketId, 16, 32), (.releaseSetId, 48, 32),
      (.dealerId, 80, 32), (.resolutionAuthorityId, 112, 32),
      (.feeRecipientId, 144, 32), (.unwindRecipientId, 176, 32),
      (.quoteScale, 208, 8), (.feeNumerator, 216, 8),
      (.feeDenominator, 224, 8), (.minimumWorkFunding, 232, 8),
      (.replacementDelay, 240, 8)] ∧
    coordinates candidateLayout = [
      (.magic, 0, 8), (.version, 8, 2), (.outcomeCount, 10, 1),
      (.reserved, 11, 5), (.candidateId, 16, 32), (.revision, 48, 8),
      (.validFrom, 56, 8), (.expiresAt, 64, 8),
      (.quoteReserveFloor, 72, 8), (.workFunding, 80, 8),
      (.workReward, 88, 8), (.minimumInventory, 96, 128),
      (.maximumInventory, 224, 128), (.curves, 352, 4224)] := by native_decide

theorem runtime_coordinates_are_canonical :
    coordinates stateLayout = [
      (.magic, 0, 8), (.version, 8, 2), (.phase, 10, 1),
      (.outcomeCount, 11, 1), (.hasPending, 12, 1), (.winner, 13, 1),
      (.reservedA, 14, 2), (.activeCandidateId, 16, 32),
      (.pendingCandidateId, 48, 32), (.releaseSetId, 80, 32),
      (.activeRevision, 112, 8), (.pendingRevision, 120, 8),
      (.stateRevision, 128, 8), (.reservedB, 136, 8),
      (.inventory, 144, 128), (.buyUsed, 272, 128),
      (.sellUsed, 400, 128), (.buyQuotePaid, 528, 128),
      (.sellQuotePaid, 656, 128), (.feeBase, 784, 8),
      (.feePaid, 792, 8), (.quoteCustody, 800, 8),
      (.feeCustody, 808, 8), (.livenessCustody, 816, 8),
      (.activeWorkRemaining, 824, 8), (.pendingWorkFunding, 832, 8)] ∧
    coordinates receiptLayout = [
      (.magic, 0, 8), (.version, 8, 2), (.role, 10, 1), (.flags, 11, 1),
      (.reserved, 12, 4), (.registryProgram, 16, 32), (.releaseSetId, 48, 32),
      (.program, 80, 32), (.artifactRelease, 112, 32),
      (.semanticRelease, 144, 32)] ∧
    coordinates requestLayout = [
      (.magic, 0, 8), (.version, 8, 2), (.action, 10, 1), (.side, 11, 1),
      (.outcome, 12, 1), (.reserved, 13, 3), (.expectedStateRevision, 16, 8),
      (.now, 24, 8), (.quantity, 32, 8), (.expectedCandidateId, 40, 32),
      (.actorId, 72, 32), (.replacementCandidateId, 104, 32),
      (.expectedCandidateRevision, 136, 8)] := by native_decide

def fieldOffset [DecidableEq α] (layout : List (PlacedField α)) (name : α) : Nat :=
  (coordinate? name layout).map Prod.fst |>.getD 0

inductive Action where
  | scheduleReplacement | activateReplacement | fill | enterTerminal | unwind | retire
  deriving DecidableEq, Repr

def Action.tag : Action → UInt8
  | .scheduleReplacement => 0 | .activateReplacement => 1 | .fill => 2
  | .enterTerminal => 3 | .unwind => 4 | .retire => 5

def phaseOpen : UInt8 := 0
def phaseTerminal : UInt8 := 1
def phaseRetired : UInt8 := 2
def sideBuy : UInt8 := 0
def sideSell : UInt8 := 1
def tradingRole : UInt8 := 2
def receiptRequiredFlags : UInt8 := 3

/-- A canonical generated fill-request fixture. -/
def exampleRequest : List UInt8 :=
  requestMagic ++ Codec.encodeLE 2 abiVersion ++
  [Action.fill.tag, sideBuy, 0] ++ List.replicate 3 0 ++
  Codec.encodeLE 8 7 ++ Codec.encodeLE 8 11 ++ Codec.encodeLE 8 13 ++
  Codec.encodeLE 32 0x22 ++ Codec.encodeLE 32 0 ++
  Codec.encodeLE 32 0 ++ Codec.encodeLE 8 9

theorem example_request_has_exact_width : exampleRequest.length = requestBytes := by
  native_decide

end DClutch.Dealer.Abi
