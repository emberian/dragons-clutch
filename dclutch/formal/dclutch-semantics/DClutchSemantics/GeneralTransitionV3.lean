import DClutchSemantics.GeneralControllerAbi
import DClutchSemantics.TransitionVMV3
import Std.Tactic

/-!
# General V3 action-selected TransitionVM programs

All fourteen General TransitionVM programs, authored here and emitted.
Before this module the General family had **no Lean counterpart at all** for its
transition artifacts: `crates/dclutch-general-adapter-contract/src/
transition_artifacts_v3.rs` built `InstructionV3` values imperatively and
carried its own instruction counts, which is exactly the gap
`DirectOrdinaryV3.lean` closed for Direct at `73f0793`. The collection and
candidate actions were subsequently authored against the same typed register
schema; the byte-identity gate says every Rust transcription is faithful to
this module.

The register schema is typed here and the wire indices are the Rust constants in
`hot_candidate_v3.rs`. That module remains the name authority for now, and
`the_lean_register_schema_is_the_one_the_rust_bank_declares` in
`transition_artifacts_v3.rs` is what stops the two drifting: it compares every
index in `ScalarSlot.all` / `ItemScalarSlot.all` / `IdentitySlot.all` against the
constant of the same name.

Protocol constants below are stated once, as decoded words, with the Rust
authority named. A wrong one does not produce a subtly wrong program: it
produces different bytes, and the byte gate refuses.
-/

namespace DClutch.General.TransitionV3

open DClutch
open DClutch.General.ControllerAbi
open DClutch.TransitionVMV3

/-- `GeneralLocalStateLayoutV3::magic_u64()` -- little-endian `DCGLST03`. -/
def localStateMagicWord : Nat := 3688540811555193668
/-- `GENERAL_LOCAL_STATE_VERSION_V3`. -/
def localStateVersion : Nat := 3
/-- `GeneralLocalStateKindV3::Selection.tag()`. -/
def kindSelection : Nat := 1
/-- `GeneralLocalStateKindV3::Settlement.tag()`. -/
def kindSettlement : Nat := 2
/-- `RuntimeSelectionLayoutV2::magic_u64()` -- little-endian `DCGSEL02`. -/
def selectionMagicWord : Nat := 3616474361412141892
/-- `RuntimeSelectionLayoutV2::version_value()`. -/
def selectionVersion : Nat := 2
/-- `RuntimeSelectionPhaseV2::Open.tag()`. -/
def selectionPhaseOpen : Nat := 1
/-- `RuntimeSelectionPhaseV2::Frozen.tag()`. -/
def selectionPhaseFrozen : Nat := 2
/-- `SettlementCursorLayoutV2::magic_u64()` -- little-endian `DCGSET02`. -/
def cursorMagicWord : Nat := 3616483157505164100
/-- `SettlementCursorLayoutV2::version_value()`. -/
def cursorVersion : Nat := 2
/-- `SettlementPhaseV2::Collecting.tag()`. -/
def cursorPhaseCollecting : Nat := 4
/-- `SettlementPhaseV2::Terminal.tag()`. -/
def cursorPhaseTerminal : Nat := 8
/-- `GeneralLifecycleV2::Active.tag()`. -/
def lifecycleActive : Nat := 1
/-- `OperationV1::Transfer` as the Custody operation tag. -/
def custodyOperationTransfer : Nat := 2
/-- `GeneralLocalStateKindV3::Batch.tag()`. -/
def kindBatch : Nat := 3
/-- `GeneralLocalStateKindV3::Order.tag()`. -/
def kindOrder : Nat := 4
/-- `GeneralLocalStateKindV3::Candidate.tag()`. -/
def kindCandidate : Nat := 5
/-- `GeneralLocalStateKindV3::Verifier.tag()`. -/
def kindVerifier : Nat := 6
/-- `GeneralBatchV1::record_magic_u64()` -- little-endian `DCGBAT01`. -/
def batchRecordMagicWord : Nat := 3544425546002154308
/-- `GeneralBatchV1::record_version_value()`; also the constant one the
transition loads into the `one` register, and they are deliberately the same
value so one `loadConst` serves both. -/
def batchRecordVersion : Nat := 1
/-- `GeneralBatchV1::record_phase_value()`. -/
def batchRecordPhase : Nat := 20
/-- `BatchStatusV1::Collecting.tag()`. -/
def batchStatusCollecting : Nat := 1
/-- `BatchStatusV1::Closed.tag()`. -/
def batchStatusClosed : Nat := 2
/-- `GeneralCandidateLayoutV1::MAGIC` as one little-endian scalar. -/
def candidateRecordMagicWord : Nat := 3544405840977412932
/-- `GeneralCandidateLayoutV1::PHASE`. -/
def candidateRecordPhase : Nat := 22
/-- `GeneralCandidateStatusV1::Submitted.tag()`. -/
def candidateStatusSubmitted : Nat := 1
/-- `GeneralCandidateStatusV1::Considered.tag()`. -/
def candidateStatusConsidered : Nat := 3
/-- `GeneralOrderPhaseV1::Placed.tag()`. -/
def orderPhasePlaced : Nat := 1
/-- `GeneralOrderLayoutV1::magic_u64()` -- little-endian `DCGORD01`. -/
def orderRecordMagicWord : Nat := 3544408027048657732
/-- `GeneralOrderLayoutV1::phase_value()`. -/
def orderRecordPhase : Nat := 21
/-- `GeneralOrderPhaseV1::Cancelled.tag()`. -/
def orderPhaseCancelled : Nat := 2
/-- `GeneralOrderPhaseV1::Released.tag()`. -/
def orderPhaseReleased : Nat := 3
/-- `DeltaDirectionV2::Neutral.tag()` -- the sole direction for magnitude zero. -/
def claimsDeltaNeutral : Nat := 0
/-- `DeltaDirectionV2::Credit.tag()`. -/
def claimsDeltaCredit : Nat := 1
/-- `DeltaDirectionV2::Debit.tag()`. -/
def claimsDeltaDebit : Nat := 2

inductive ScalarSlot where
  | action | completeSetMove | claimsAffineActive | custodyActive
  | terminal | orderCoordinate | settlementRevision | orderNonce
  | quoteQuantity | completeSetQuantity | outcomeCount | terminalCoordinate
  | generation | pageIndex | executionIndex | transferIndex
  | custodyExpectedRevision | custodyResultingRevision | custodyRentLamports | claimsMarketRevision
  | ownerPositionRevision | settlementPositionRevision | claimsPositionCount | claimsRowCount
  | claimsAdmitActive | claimsCloseActive | custodyOperation | custodySourceCompartment
  | custodyDestinationCompartment | claimsSourcePresent | claimsDestinationPresent | claimsSourcePositionIndex
  | claimsDestinationPositionIndex | claimsAggregateDirection | claimsSourceDirection | claimsDestinationDirection
  | observedPositionLamports | observedAdmissionLamports | positionRentPrincipal | admissionRentPrincipal
  | settlementPositionPresent | positionZeroRevision | positionOneRevision | positionTableCount
  | claimsPostMarketRevision | settlementPostPositionRevision | custodyAmount | custodyReplayRentLamports
  | custodyVaultRentLamports | custodyCloseVaultExpectedRevision | custodyCloseVaultResultingRevision | custodyCloseReplayResultingRevision
  | zero | selectionPhase | selectionRevision | selectionSubmittedCount
  | selectionBestCandidateCoordinate | selectionBestVerifiedRevision | selectionPriceScale | selectionMagic
  | runtimeWidthVersion | cursorPhase | cursorOrderCount | cursorNextOrder
  | cursorResultingRevision | cursorQuoteInventory | cursorCompleteSetQuantity | cursorMagic
  | cursorTerminalCoordinate | stateBump | terminalRecordBump | primaryBumpObservation
  | primaryPrincipalObservation | primaryCreated | primaryCanonicalBump | primaryRentPrincipal
  | terminalBumpObservation | terminalPrincipalObservation | terminalCreated | terminalCanonicalBump
  | terminalRentPrincipal | localStateMagic | localStateVersion | localStateKind
  | selectionBestFilledLots | selectionBestQuoteSurplus | inputScratchPageCount | manifestOrderIndex
  | rootLifecycleObservation | rootLifecycleActive
  -- The GEN-SEVEN widening. Everything below serves the seven collection and
  -- candidate actions; the settlement seven never address these coordinates,
  -- so widening the bank changes their header bytes and none of their conjuncts.
  | currentSlot | one | scratchA | scratchB
  | rootExpectedRevision | rootRevisionObservation | rootPostRevision | rootNextBatchSequenceObservation
  | rootPostBatchSequence | rootOpenBatchesObservation | rootPostOpenBatches | configCollectionSlots
  | configSelectionSlots | configSettlementSlots | configMaxOrders | batchStatusObservation
  | batchPostStatus | batchOrderCountObservation | batchPostOrderCount | batchCancelledCountObservation
  | batchPostCancelledCount | batchQuoteReserveObservation | batchPostQuoteReserve | batchCollectionCloseSlot
  | batchSettlementCloseSlot | orderMaxLots | orderMaxQuoteDebitPerLot | orderQuoteReserve
  | orderValidUntilSlot | orderPhaseObservation | orderPostPhase | orderAdmittedSlotObservation
  | orderPostReleasedSlot | escrowBalanceObservation | candidatePageCount | candidatePageRevision
  | candidateRowCount | candidateRewardRate | candidateStatusObservation | candidatePostStatus
  | candidateVerificationRemainingObservation | candidatePostVerificationRemaining | candidateCleanupRemainingObservation | candidatePostCleanupRemaining
  | candidateSubmittedSlot | verifyTerminal | verifyRevisionObservation | verifyPostRevision
  | verifyPageObservation | verifyPostPage | verifyRowObservation | verifyPostRow
  | verifyOrderCountObservation | verifyPostOrderCount | verifyManifestOrderCount | resultStateBump
  | resultBumpObservation | resultPrincipalObservation | resultCreated | resultCanonicalBump
  | resultRentPrincipal
  deriving DecidableEq, Repr

namespace ScalarSlot

/-- Constructor order IS the wire index; `index` below is the check. -/
def all : List ScalarSlot := [
  .action, .completeSetMove, .claimsAffineActive, .custodyActive,
  .terminal, .orderCoordinate, .settlementRevision, .orderNonce,
  .quoteQuantity, .completeSetQuantity, .outcomeCount, .terminalCoordinate,
  .generation, .pageIndex, .executionIndex, .transferIndex,
  .custodyExpectedRevision, .custodyResultingRevision, .custodyRentLamports, .claimsMarketRevision,
  .ownerPositionRevision, .settlementPositionRevision, .claimsPositionCount, .claimsRowCount,
  .claimsAdmitActive, .claimsCloseActive, .custodyOperation, .custodySourceCompartment,
  .custodyDestinationCompartment, .claimsSourcePresent, .claimsDestinationPresent, .claimsSourcePositionIndex,
  .claimsDestinationPositionIndex, .claimsAggregateDirection, .claimsSourceDirection, .claimsDestinationDirection,
  .observedPositionLamports, .observedAdmissionLamports, .positionRentPrincipal, .admissionRentPrincipal,
  .settlementPositionPresent, .positionZeroRevision, .positionOneRevision, .positionTableCount,
  .claimsPostMarketRevision, .settlementPostPositionRevision, .custodyAmount, .custodyReplayRentLamports,
  .custodyVaultRentLamports, .custodyCloseVaultExpectedRevision, .custodyCloseVaultResultingRevision, .custodyCloseReplayResultingRevision,
  .zero, .selectionPhase, .selectionRevision, .selectionSubmittedCount,
  .selectionBestCandidateCoordinate, .selectionBestVerifiedRevision, .selectionPriceScale, .selectionMagic,
  .runtimeWidthVersion, .cursorPhase, .cursorOrderCount, .cursorNextOrder,
  .cursorResultingRevision, .cursorQuoteInventory, .cursorCompleteSetQuantity, .cursorMagic,
  .cursorTerminalCoordinate, .stateBump, .terminalRecordBump, .primaryBumpObservation,
  .primaryPrincipalObservation, .primaryCreated, .primaryCanonicalBump, .primaryRentPrincipal,
  .terminalBumpObservation, .terminalPrincipalObservation, .terminalCreated, .terminalCanonicalBump,
  .terminalRentPrincipal, .localStateMagic, .localStateVersion, .localStateKind,
  .selectionBestFilledLots, .selectionBestQuoteSurplus, .inputScratchPageCount, .manifestOrderIndex,
  .rootLifecycleObservation, .rootLifecycleActive,
  .currentSlot, .one, .scratchA, .scratchB,
  .rootExpectedRevision, .rootRevisionObservation, .rootPostRevision, .rootNextBatchSequenceObservation,
  .rootPostBatchSequence, .rootOpenBatchesObservation, .rootPostOpenBatches, .configCollectionSlots,
  .configSelectionSlots, .configSettlementSlots, .configMaxOrders, .batchStatusObservation,
  .batchPostStatus, .batchOrderCountObservation, .batchPostOrderCount, .batchCancelledCountObservation,
  .batchPostCancelledCount, .batchQuoteReserveObservation, .batchPostQuoteReserve, .batchCollectionCloseSlot,
  .batchSettlementCloseSlot, .orderMaxLots, .orderMaxQuoteDebitPerLot, .orderQuoteReserve,
  .orderValidUntilSlot, .orderPhaseObservation, .orderPostPhase, .orderAdmittedSlotObservation,
  .orderPostReleasedSlot, .escrowBalanceObservation, .candidatePageCount, .candidatePageRevision,
  .candidateRowCount, .candidateRewardRate, .candidateStatusObservation, .candidatePostStatus,
  .candidateVerificationRemainingObservation, .candidatePostVerificationRemaining, .candidateCleanupRemainingObservation, .candidatePostCleanupRemaining,
  .candidateSubmittedSlot, .verifyTerminal, .verifyRevisionObservation, .verifyPostRevision,
  .verifyPageObservation, .verifyPostPage, .verifyRowObservation, .verifyPostRow,
  .verifyOrderCountObservation, .verifyPostOrderCount, .verifyManifestOrderCount, .resultStateBump,
  .resultBumpObservation, .resultPrincipalObservation, .resultCreated, .resultCanonicalBump,
  .resultRentPrincipal
]

@[simp] def index : ScalarSlot → Nat
  | .action => 0
  | .completeSetMove => 1
  | .claimsAffineActive => 2
  | .custodyActive => 3
  | .terminal => 4
  | .orderCoordinate => 5
  | .settlementRevision => 6
  | .orderNonce => 7
  | .quoteQuantity => 8
  | .completeSetQuantity => 9
  | .outcomeCount => 10
  | .terminalCoordinate => 11
  | .generation => 12
  | .pageIndex => 13
  | .executionIndex => 14
  | .transferIndex => 15
  | .custodyExpectedRevision => 16
  | .custodyResultingRevision => 17
  | .custodyRentLamports => 18
  | .claimsMarketRevision => 19
  | .ownerPositionRevision => 20
  | .settlementPositionRevision => 21
  | .claimsPositionCount => 22
  | .claimsRowCount => 23
  | .claimsAdmitActive => 24
  | .claimsCloseActive => 25
  | .custodyOperation => 26
  | .custodySourceCompartment => 27
  | .custodyDestinationCompartment => 28
  | .claimsSourcePresent => 29
  | .claimsDestinationPresent => 30
  | .claimsSourcePositionIndex => 31
  | .claimsDestinationPositionIndex => 32
  | .claimsAggregateDirection => 33
  | .claimsSourceDirection => 34
  | .claimsDestinationDirection => 35
  | .observedPositionLamports => 36
  | .observedAdmissionLamports => 37
  | .positionRentPrincipal => 38
  | .admissionRentPrincipal => 39
  | .settlementPositionPresent => 40
  | .positionZeroRevision => 41
  | .positionOneRevision => 42
  | .positionTableCount => 43
  | .claimsPostMarketRevision => 44
  | .settlementPostPositionRevision => 45
  | .custodyAmount => 46
  | .custodyReplayRentLamports => 47
  | .custodyVaultRentLamports => 48
  | .custodyCloseVaultExpectedRevision => 49
  | .custodyCloseVaultResultingRevision => 50
  | .custodyCloseReplayResultingRevision => 51
  | .zero => 52
  | .selectionPhase => 53
  | .selectionRevision => 54
  | .selectionSubmittedCount => 55
  | .selectionBestCandidateCoordinate => 56
  | .selectionBestVerifiedRevision => 57
  | .selectionPriceScale => 58
  | .selectionMagic => 59
  | .runtimeWidthVersion => 60
  | .cursorPhase => 61
  | .cursorOrderCount => 62
  | .cursorNextOrder => 63
  | .cursorResultingRevision => 64
  | .cursorQuoteInventory => 65
  | .cursorCompleteSetQuantity => 66
  | .cursorMagic => 67
  | .cursorTerminalCoordinate => 68
  | .stateBump => 69
  | .terminalRecordBump => 70
  | .primaryBumpObservation => 71
  | .primaryPrincipalObservation => 72
  | .primaryCreated => 73
  | .primaryCanonicalBump => 74
  | .primaryRentPrincipal => 75
  | .terminalBumpObservation => 76
  | .terminalPrincipalObservation => 77
  | .terminalCreated => 78
  | .terminalCanonicalBump => 79
  | .terminalRentPrincipal => 80
  | .localStateMagic => 81
  | .localStateVersion => 82
  | .localStateKind => 83
  | .selectionBestFilledLots => 84
  | .selectionBestQuoteSurplus => 85
  | .inputScratchPageCount => 86
  | .manifestOrderIndex => 87
  | .rootLifecycleObservation => 88
  | .rootLifecycleActive => 89
  | .currentSlot => 90
  | .one => 91
  | .scratchA => 92
  | .scratchB => 93
  | .rootExpectedRevision => 94
  | .rootRevisionObservation => 95
  | .rootPostRevision => 96
  | .rootNextBatchSequenceObservation => 97
  | .rootPostBatchSequence => 98
  | .rootOpenBatchesObservation => 99
  | .rootPostOpenBatches => 100
  | .configCollectionSlots => 101
  | .configSelectionSlots => 102
  | .configSettlementSlots => 103
  | .configMaxOrders => 104
  | .batchStatusObservation => 105
  | .batchPostStatus => 106
  | .batchOrderCountObservation => 107
  | .batchPostOrderCount => 108
  | .batchCancelledCountObservation => 109
  | .batchPostCancelledCount => 110
  | .batchQuoteReserveObservation => 111
  | .batchPostQuoteReserve => 112
  | .batchCollectionCloseSlot => 113
  | .batchSettlementCloseSlot => 114
  | .orderMaxLots => 115
  | .orderMaxQuoteDebitPerLot => 116
  | .orderQuoteReserve => 117
  | .orderValidUntilSlot => 118
  | .orderPhaseObservation => 119
  | .orderPostPhase => 120
  | .orderAdmittedSlotObservation => 121
  | .orderPostReleasedSlot => 122
  | .escrowBalanceObservation => 123
  | .candidatePageCount => 124
  | .candidatePageRevision => 125
  | .candidateRowCount => 126
  | .candidateRewardRate => 127
  | .candidateStatusObservation => 128
  | .candidatePostStatus => 129
  | .candidateVerificationRemainingObservation => 130
  | .candidatePostVerificationRemaining => 131
  | .candidateCleanupRemainingObservation => 132
  | .candidatePostCleanupRemaining => 133
  | .candidateSubmittedSlot => 134
  | .verifyTerminal => 135
  | .verifyRevisionObservation => 136
  | .verifyPostRevision => 137
  | .verifyPageObservation => 138
  | .verifyPostPage => 139
  | .verifyRowObservation => 140
  | .verifyPostRow => 141
  | .verifyOrderCountObservation => 142
  | .verifyPostOrderCount => 143
  | .verifyManifestOrderCount => 144
  | .resultStateBump => 145
  | .resultBumpObservation => 146
  | .resultPrincipalObservation => 147
  | .resultCreated => 148
  | .resultCanonicalBump => 149
  | .resultRentPrincipal => 150

end ScalarSlot

inductive ItemScalarSlot where
  | outcome | quantity | claimsAggregateMagnitude | claimsSourceMagnitude
  | claimsDestinationMagnitude | cursorInventory
  deriving DecidableEq, Repr

namespace ItemScalarSlot

/-- Constructor order IS the wire index; `index` below is the check. -/
def all : List ItemScalarSlot := [
  .outcome, .quantity, .claimsAggregateMagnitude, .claimsSourceMagnitude,
  .claimsDestinationMagnitude, .cursorInventory
]

@[simp] def index : ItemScalarSlot → Nat
  | .outcome => 0
  | .quantity => 1
  | .claimsAggregateMagnitude => 2
  | .claimsSourceMagnitude => 3
  | .claimsDestinationMagnitude => 4
  | .cursorInventory => 5

end ItemScalarSlot

inductive IdentitySlot where
  | parentRequestDigest | candidate | owner | order
  | beneficiary | releaseSet | market | productRecordDigest
  | semanticBasisId | linkedBasisRecordDigest | realm | tradingProgram
  | custodySource | custodyDestination | sourceVaultContext | destinationVaultContext
  | mint | tokenProgram | payer | rentRefund
  | settlementPositionOwner | rentCredit | rentProgram | custodySourceOwner
  | custodyDestinationOwner | positionZeroOwner | positionOneOwner | generalRoot
  | selectionProduct | selectionBatch | selectionPolicy | bestVerifiedDigest
  | primaryBeneficiaryObservation | primaryBeneficiary | primaryState | primaryOwner
  | terminalBeneficiaryObservation | terminalBeneficiary | terminalState | terminalOwner
  | generalConfigId | resultBeneficiaryObservation | resultBeneficiary | resultState
  | resultOwner
  deriving DecidableEq, Repr

namespace IdentitySlot

/-- Constructor order IS the wire index; `index` below is the check. -/
def all : List IdentitySlot := [
  .parentRequestDigest, .candidate, .owner, .order,
  .beneficiary, .releaseSet, .market, .productRecordDigest,
  .semanticBasisId, .linkedBasisRecordDigest, .realm, .tradingProgram,
  .custodySource, .custodyDestination, .sourceVaultContext, .destinationVaultContext,
  .mint, .tokenProgram, .payer, .rentRefund,
  .settlementPositionOwner, .rentCredit, .rentProgram, .custodySourceOwner,
  .custodyDestinationOwner, .positionZeroOwner, .positionOneOwner, .generalRoot,
  .selectionProduct, .selectionBatch, .selectionPolicy, .bestVerifiedDigest,
  .primaryBeneficiaryObservation, .primaryBeneficiary, .primaryState, .primaryOwner,
  .terminalBeneficiaryObservation, .terminalBeneficiary, .terminalState, .terminalOwner,
  .generalConfigId, .resultBeneficiaryObservation, .resultBeneficiary, .resultState,
  .resultOwner
]

@[simp] def index : IdentitySlot → Nat
  | .parentRequestDigest => 0
  | .candidate => 1
  | .owner => 2
  | .order => 3
  | .beneficiary => 4
  | .releaseSet => 5
  | .market => 6
  | .productRecordDigest => 7
  | .semanticBasisId => 8
  | .linkedBasisRecordDigest => 9
  | .realm => 10
  | .tradingProgram => 11
  | .custodySource => 12
  | .custodyDestination => 13
  | .sourceVaultContext => 14
  | .destinationVaultContext => 15
  | .mint => 16
  | .tokenProgram => 17
  | .payer => 18
  | .rentRefund => 19
  | .settlementPositionOwner => 20
  | .rentCredit => 21
  | .rentProgram => 22
  | .custodySourceOwner => 23
  | .custodyDestinationOwner => 24
  | .positionZeroOwner => 25
  | .positionOneOwner => 26
  | .generalRoot => 27
  | .selectionProduct => 28
  | .selectionBatch => 29
  | .selectionPolicy => 30
  | .bestVerifiedDigest => 31
  | .primaryBeneficiaryObservation => 32
  | .primaryBeneficiary => 33
  | .primaryState => 34
  | .primaryOwner => 35
  | .terminalBeneficiaryObservation => 36
  | .terminalBeneficiary => 37
  | .terminalState => 38
  | .terminalOwner => 39
  | .generalConfigId => 40
  | .resultBeneficiaryObservation => 41
  | .resultBeneficiary => 42
  | .resultState => 43
  | .resultOwner => 44

end IdentitySlot

/-- A common scalar coordinate. -/
def s (register : ScalarSlot) : Reg := common register.index
/-- A per-outcome scalar coordinate inside the Product-owned tail. -/
def t (register : ItemScalarSlot) : Reg := item register.index
/-- A common identity coordinate. -/
def d (register : IdentitySlot) : Reg := common register.index

def commonScalars : Nat := ScalarSlot.all.length
/-- The width of the per-outcome slot ENUM, which is not what every action
declares. See `actionItemScalarStride`. -/
def itemScalarStride : Nat := ItemScalarSlot.all.length
def commonIdentities : Nat := IdentitySlot.all.length
/-- General has no per-outcome identity tail. -/
def itemIdentityStride : Nat := 0

/-- The per-outcome scalar stride ONE action declares.

`openBatch` and `closeBatch` declare zero, because the batch record has no
per-outcome tail: their effect already declares no item operations, and the only
item instruction they ever emitted was the shared bound check `outcome <
outcomeCount` on a register whose sole legal value is the coordinate it occupies.
Neither action ever READ the tail it declared, and the declaration was the only
thing making the register bank grow with the Product width.

The cost it removes was measured on the real-ELF `OpenBatch` campaign on
2026-09-02: the Trading heap peak was `59,376 + 528*(N - 2)` bytes of 65,536, an
identity that reproduced both measured peaks and predicted the abort -- N = 13
peaked at 65,184 and committed, N = 14 needed 65,712 and the allocator died. Of
that 528 bytes per outcome only 48 was declared width; the rest was the same
width copied through eleven full-width banks that a no-op `dealloc` never
reclaims. At stride zero the peak is flat in N and the scratch-page span stops
growing with it, because the page count is derived from the bank width. -/
def actionItemScalarStride (action : Action) : Nat :=
  match action with
  | .openBatch | .closeBatch => 0
  | _ => itemScalarStride

/-- The local-state kind each action's PRIMARY state envelope carries.

The batch four share the Batch envelope because their primary state is the
batch window itself -- `PlaceOrder` and `CancelOrder` authenticate it and touch
their order as a SECONDARY state. The candidate pair's primary is the
submission record, and `ReleaseOrder`'s is the order it refunds. -/
def stateKind (action : Action) : Nat :=
  match action with
  | .consider | .freeze => kindSelection
  -- The register feeds the envelope an action CREATES (or, for a
  -- non-creating action, nothing at all): the batch pair creates or flips the
  -- Batch envelope, and PlaceOrder creates the ORDER envelope even though its
  -- primary derived state is the batch window it is admitted into.
  | .openBatch | .cancelOrder | .closeBatch => kindBatch
  | .submitCandidate | .verifyCandidateRow | .closeCandidate => kindCandidate
  | .placeOrder | .releaseOrder => kindOrder
  | _ => kindSettlement

/-- Every action's shared prelude.

The capability-root conjunct is the pair `loadConst ROOT_LIFECYCLE_ACTIVE` then
`scalarEq ROOT_LIFECYCLE_OBSERVATION`: the composite root's immutable header is
byte-identical for a live and a retired capability, so this is the only thing on
the runtime-width path that can tell them apart. An artifact whose AccountProfile
never projects the observation leaves the register at zero, which is not
`Active`, so the omission refuses instead of passing. -/
def commonOps (action : Action) : List Op := [
  .loadConst (s .action) action.tag.toNat,
  .loadConst (s .rootLifecycleActive) lifecycleActive,
  .scalarEq (s .rootLifecycleObservation) (s .rootLifecycleActive),
  .loadConst (s .localStateMagic) localStateMagicWord,
  .loadConst (s .localStateVersion) localStateVersion,
  .loadConst (s .localStateKind) (stateKind action),
  .nonzero (s .outcomeCount),
  .scalarEq (s .zero) (s .outcomeCount),
  .scalarEq (s .stateBump) (s .primaryCanonicalBump),
  .identityEq (d .primaryOwner) (d .tradingProgram),
  .nonzero (s .primaryRentPrincipal)
]

/-- The shared body of one streamed settlement row. -/
def rowOps : List Op := [
  .nonzero (s .settlementPositionPresent),
  .nonzero (s .orderCoordinate),
  .incrementInto (s .settlementRevision) (s .cursorResultingRevision),
  .incrementInto (s .claimsMarketRevision) (s .claimsPostMarketRevision),
  .incrementInto (s .settlementPositionRevision) (s .settlementPostPositionRevision),
  .incrementInto (s .custodyExpectedRevision) (s .custodyResultingRevision),
  .loadConst (s .custodyOperation) custodyOperationTransfer,
  .scalarEq (s .claimsRowCount) (s .outcomeCount)
]

/-- Require the Custody vault a transfer touches to be the one the row names.

Decision 0010 §2 rests "a maker can never be paid more than they escrowed" on the
vault being keyed by the order's own identity, and nothing required the vault
presented in the frame to BE that one: the context arrives from the
AccountProfile's projection of caller-supplied Custody accounts and the order
identity from the authenticated manifest row.

Expressible only where the direction is fixed at authoring time. `Materialize`
patches its compartments at runtime from the authenticated complete-set move, so
which side is the Hoard is not a constant of its artifact and it carries no
binding here. -/
def vaultContextOps (action : Action) : List Op :=
  match action with
  | .collect => [
      .identityEq (d .sourceVaultContext) (d .order),
      .identityEq (d .destinationVaultContext) (d .candidate)
    ]
  | .distribute => [
      .identityEq (d .sourceVaultContext) (d .candidate),
      .identityEq (d .custodyDestinationOwner) (d .owner)
    ]
  | .close => [
      .identityEq (d .sourceVaultContext) (d .candidate),
      .identityEq (d .custodyDestinationOwner) (d .beneficiary)
    ]
  | _ => []

/-- The action-selected half of each prelude, exhaustive by name. -/
def actionOps (action : Action) : List Op :=
  match action with
  | .consider => [
      .loadConst (s .selectionMagic) selectionMagicWord,
      .loadConst (s .runtimeWidthVersion) selectionVersion,
      .loadConst (s .selectionPhase) selectionPhaseOpen,
      .nonzero (s .selectionRevision)
    ]
  | .freeze => [
      .loadConst (s .selectionMagic) selectionMagicWord,
      .loadConst (s .runtimeWidthVersion) selectionVersion,
      .loadConst (s .selectionPhase) selectionPhaseFrozen,
      .nonzero (s .selectionRevision),
      .nonzero (s .selectionBestCandidateCoordinate),
      .nonzero (s .selectionBestVerifiedRevision)
    ]
  | .initializeSettlement => [
      .loadConst (s .zero) 0,
      .loadConst (s .cursorMagic) cursorMagicWord,
      .loadConst (s .runtimeWidthVersion) cursorVersion,
      .loadConst (s .cursorPhase) cursorPhaseCollecting,
      .loadConst (s .cursorNextOrder) 0,
      .loadConst (s .cursorResultingRevision) 1,
      .loadConst (s .cursorQuoteInventory) 0,
      .loadConst (s .cursorTerminalCoordinate) 0,
      .incrementInto (s .custodyExpectedRevision) (s .custodyResultingRevision),
      .incrementInto (s .claimsMarketRevision) (s .claimsPostMarketRevision)
    ]
  | .collect | .distribute => rowOps ++ vaultContextOps action
  | .materialize => [
      .nonzero (s .settlementPositionPresent),
      .incrementInto (s .settlementRevision) (s .cursorResultingRevision),
      .loadConst (s .custodyOperation) custodyOperationTransfer,
      .incrementInto (s .claimsMarketRevision) (s .claimsPostMarketRevision),
      .incrementInto (s .settlementPositionRevision) (s .settlementPostPositionRevision)
    ]
  | .close => [
      .nonzero (s .settlementPositionPresent),
      .incrementInto (s .settlementRevision) (s .cursorResultingRevision),
      .loadConst (s .terminal) 1,
      .loadConst (s .cursorPhase) cursorPhaseTerminal,
      .scalarEq (s .terminalRecordBump) (s .terminalCanonicalBump),
      .identityEq (d .terminalOwner) (d .tradingProgram),
      .nonzero (s .terminalRentPrincipal),
      .loadConst (s .zero) 0,
      .scalarEq (s .positionTableCount) (s .zero),
      .incrementInto (s .custodyExpectedRevision) (s .custodyCloseVaultExpectedRevision),
      .incrementInto (s .custodyCloseVaultExpectedRevision) (s .custodyCloseVaultResultingRevision),
      .incrementInto (s .custodyCloseVaultResultingRevision) (s .custodyCloseReplayResultingRevision),
      .nonzero (s .cursorTerminalCoordinate),
      .nonzero (s .terminalCoordinate)
    ] ++ vaultContextOps .close
  | .openBatch => [
      -- The caller's optimistic root revision must be the observed one, and
      -- the three root advances are exact: this is `GeneralRootV2::open_batch`
      -- stated as conjuncts. The EffectProgram writes the successor root tail
      -- from the three POST registers and the new batch record from the
      -- observation registers, so a program that skipped an increment would
      -- write a root that replays.
      .scalarEq (s .rootExpectedRevision) (s .rootRevisionObservation),
      .incrementInto (s .rootRevisionObservation) (s .rootPostRevision),
      .incrementInto (s .rootNextBatchSequenceObservation) (s .rootPostBatchSequence),
      .incrementInto (s .rootOpenBatchesObservation) (s .rootPostOpenBatches),
      -- A zero window or a zero admission bound is a batch that can admit
      -- nothing and never close by fullness; the config refuses it at
      -- construction and this refuses a projection that lost it.
      .nonzero (s .configCollectionSlots),
      .nonzero (s .configSelectionSlots),
      .nonzero (s .configSettlementSlots),
      .nonzero (s .configMaxOrders),
      -- The windows are config-derived, never caller-chosen:
      -- collection close = now + collection, settlement close = collection
      -- close + selection + settlement. `checkedAddInto` refuses overflow, so
      -- the close slots always lie strictly after the current slot.
      .checkedAddInto (s .currentSlot) (s .configCollectionSlots) (s .batchCollectionCloseSlot),
      .checkedAddInto (s .batchCollectionCloseSlot) (s .configSelectionSlots) (s .scratchA),
      .checkedAddInto (s .scratchA) (s .configSettlementSlots) (s .batchSettlementCloseSlot),
      -- The record constants the EffectProgram writes into the vacant account:
      -- status Collecting, and the batch record's own magic/version/phase.
      -- `one` doubles as the record version, which is 1.
      .loadConst (s .batchPostStatus) batchStatusCollecting,
      .loadConst (s .one) batchRecordVersion,
      .loadConst (s .scratchA) batchRecordMagicWord,
      .loadConst (s .scratchB) batchRecordPhase
    ]
  | .closeBatch => [
      -- `GeneralRootV2::close_batch` as conjuncts: revision advances by one
      -- and the open-batch count decrements by exactly one -- `subInto`
      -- refuses at zero, which is the root refusing a close it never opened.
      .scalarEq (s .rootExpectedRevision) (s .rootRevisionObservation),
      .incrementInto (s .rootRevisionObservation) (s .rootPostRevision),
      .loadConst (s .one) 1,
      .subInto (s .rootOpenBatchesObservation) (s .one) (s .rootPostOpenBatches),
      -- Only a collecting batch closes, and it closes to Closed.
      .loadConst (s .scratchA) batchStatusCollecting,
      .scalarEq (s .batchStatusObservation) (s .scratchA),
      .loadConst (s .batchPostStatus) batchStatusClosed,
      -- `close_is_permissionless`: the window is over, OR the batch is full.
      -- A disjunction over an all-conjunct vocabulary, built exactly:
      --   d1 := close - min(now, close)        (zero iff the window is over)
      --   d2 := bound - min(count, bound)      (zero iff the batch is full)
      --   min(d1,1) * min(d2,1) = 0            (the disjunction, clamped so
      --                                         the product cannot overflow)
      -- Anything else is an early close that truncates a maker's window.
      .minInto (s .currentSlot) (s .batchCollectionCloseSlot) (s .scratchA),
      .subInto (s .batchCollectionCloseSlot) (s .scratchA) (s .scratchA),
      .minInto (s .scratchA) (s .one) (s .scratchA),
      .minInto (s .batchOrderCountObservation) (s .configMaxOrders) (s .scratchB),
      .subInto (s .configMaxOrders) (s .scratchB) (s .scratchB),
      .minInto (s .scratchB) (s .one) (s .scratchB),
      .checkedMulInto (s .scratchA) (s .scratchB) (s .scratchA),
      .loadConst (s .scratchB) 0,
      .scalarEq (s .scratchA) (s .scratchB)
    ]
  | .submitCandidate => [
      .loadConst (s .one) 1,
      .nonzero (s .primaryCreated),
      .identityEq (d .primaryBeneficiary) (d .owner),
      -- The solver who PAYS is the solver the candidate NAMES. The
      -- AccountProfile projects the creation payer's key into `payer`; this
      -- is where the two are joined, because an account-profile guard reads
      -- the INPUT identity bank and could never see `owner`, which the same
      -- profile pass projects out of the record.
      .identityEq (d .payer) (d .owner),
      .identityEq (d .candidate) (d .bestVerifiedDigest),
      .identityEq (d .selectionPolicy) (d .selectionBatch),
      .identityEq (d .order) (d .selectionProduct),
      .scalarEq (s .selectionPriceScale) (s .orderMaxLots),
      .scalarEq (s .zero) (s .outcomeCount),
      .scalarEq (s .batchPostOrderCount) (s .outcomeCount),
      .identityEq (d .resultBeneficiaryObservation) (d .candidate),
      .identityEq (d .beneficiary) (d .selectionBatch),
      .scalarEq (s .verifyPostOrderCount) (s .outcomeCount),
      .scalarEq (s .verifyPostPage) (s .candidatePageCount),
      .loadConst (s .candidatePostStatus) candidateStatusSubmitted,
      .scalarEq (s .candidateStatusObservation) (s .candidatePostStatus),
      .nonzero (s .selectionBestCandidateCoordinate),
      .nonzero (s .candidatePageRevision),
      .nonzero (s .candidateRowCount),
      .nonzero (s .candidateRewardRate),
      .scalarLe (s .candidatePageCount) (s .candidateRowCount),
      .loadConst (s .scratchA) batchStatusClosed,
      .scalarEq (s .batchStatusObservation) (s .scratchA),
      .scalarLe (s .batchCollectionCloseSlot) (s .currentSlot),
      .scalarLt (s .currentSlot) (s .batchSettlementCloseSlot),
      .scalarEq (s .candidateSubmittedSlot) (s .currentSlot),
      .incrementInto (s .candidateRowCount) (s .candidatePostVerificationRemaining),
      .checkedMulInto (s .candidatePostVerificationRemaining) (s .candidateRewardRate)
        (s .candidatePostVerificationRemaining),
      .copyScalar (s .candidateRewardRate) (s .candidatePostCleanupRemaining),
      .scalarEq (s .candidateVerificationRemainingObservation)
        (s .candidatePostVerificationRemaining),
      .scalarEq (s .candidateCleanupRemainingObservation)
        (s .candidatePostCleanupRemaining),
      .checkedAddInto (s .candidatePostVerificationRemaining)
        (s .candidatePostCleanupRemaining) (s .scratchA),
      .checkedAddInto (s .scratchA) (s .primaryRentPrincipal) (s .scratchB),
      .loadConst (s .verifyRevisionObservation) candidateRecordMagicWord,
      .loadConst (s .verifyPostRevision) candidateRecordPhase
    ]
  | .verifyCandidateRow => [
      -- The Candidate envelope is authenticated by the common prelude and
      -- Lifecycle V5. Verification may advance only the Submitted phase.
      .loadConst (s .one) 1,
      .loadConst (s .scratchA) candidateStatusSubmitted,
      .scalarEq (s .candidateStatusObservation) (s .scratchA),
      -- Request subject and optimistic coordinates must name the exact
      -- authenticated Candidate and persisted verifier cursor. The request
      -- profile deliberately puts expected_revision at 94: scalar zero is
      -- ACTION and is overwritten before this program executes.
      .identityEq (d .parentRequestDigest) (d .candidate),
      .scalarEq (s .rootExpectedRevision) (s .verifyRevisionObservation),
      .incrementInto (s .verifyRevisionObservation) (s .verifyPostRevision),
      .scalarEq (s .completeSetMove) (s .verifyPageObservation),
      .scalarEq (s .claimsAffineActive) (s .verifyRowObservation),
      -- One row either continues the current globally grouped order or starts
      -- exactly one new one; it cannot delete or skip an order coordinate.
      .scalarLe (s .verifyOrderCountObservation) (s .verifyPostOrderCount),
      .incrementInto (s .verifyOrderCountObservation) (s .scratchA),
      .scalarLe (s .verifyPostOrderCount) (s .scratchA),
      -- Lifecycle V5 uses VERIFY_TERMINAL == 1 as the sole raw-certificate
      -- creation guard. Pinning this projection to {0,1} makes every other
      -- value refuse instead of accidentally behaving as nonterminal.
      .scalarLe (s .verifyTerminal) (s .one)
    ]
  | .closeCandidate => [
      .loadConst (s .one) 1,
      -- The request, Candidate body, Batch evidence and lifecycle beneficiary
      -- all name one immutable submission and its solver.
      .identityEq (d .parentRequestDigest) (d .candidate),
      .identityEq (d .primaryBeneficiary) (d .owner),
      .identityEq (d .rentCredit) (d .owner),
      -- Physical capitalization, not a record-only promise.
      .nonzero (s .candidateRewardRate),
      .scalarEq (s .candidateCleanupRemainingObservation) (s .candidateRewardRate),
      .checkedAddInto (s .candidateVerificationRemainingObservation)
        (s .candidateCleanupRemainingObservation) (s .scratchA),
      .checkedAddInto (s .scratchA) (s .primaryRentPrincipal) (s .scratchB),
      .scalarEq (s .observedPositionLamports) (s .scratchB),
      -- Permissionless close is legal only after consideration OR after the
      -- joined Batch's settlement window. Each distance is clamped to a bit;
      -- their product must be zero, which is the exact disjunction.
      .loadConst (s .scratchA) candidateStatusConsidered,
      .nonzero (s .candidateStatusObservation),
      .scalarLe (s .candidateStatusObservation) (s .scratchA),
      .minInto (s .candidateStatusObservation) (s .scratchA) (s .scratchB),
      .subInto (s .scratchA) (s .scratchB) (s .scratchB),
      .minInto (s .scratchB) (s .one) (s .scratchB),
      .minInto (s .currentSlot) (s .batchSettlementCloseSlot) (s .scratchA),
      .subInto (s .batchSettlementCloseSlot) (s .scratchA) (s .scratchA),
      .minInto (s .scratchA) (s .one) (s .scratchA),
      .checkedMulInto (s .scratchA) (s .scratchB) (s .scratchA),
      .loadConst (s .scratchB) 0,
      .scalarEq (s .scratchA) (s .scratchB),
      .loadConst (s .scratchA) batchStatusClosed,
      .scalarEq (s .batchStatusObservation) (s .scratchA)
    ]
  | .placeOrder => [
      -- The second derived state -- the order record this admission CREATES
      -- -- anchored exactly as a created secondary always is.
      .scalarEq (s .terminalRecordBump) (s .terminalCanonicalBump),
      .identityEq (d .terminalOwner) (d .tradingProgram),
      .nonzero (s .terminalRentPrincipal),
      -- The signed terms' width must be the Product width the prelude
      -- already equated with the batch's.
      .scalarEq (s .scratchA) (s .outcomeCount),
      -- Admission: a COLLECTING batch, inside its window, under its bound.
      .loadConst (s .one) 1,
      .scalarEq (s .batchStatusObservation) (s .one),
      .scalarLt (s .currentSlot) (s .batchCollectionCloseSlot),
      .scalarLt (s .batchOrderCountObservation) (s .configMaxOrders),
      .incrementInto (s .batchOrderCountObservation) (s .batchPostOrderCount),
      -- THE EXPIRY PIN (recorded choice 6): the signed valid_until_slot IS
      -- the batch's settlement close, exactly. Stricter than the pure admit,
      -- which accepts any later slot -- a later valid_until buys nothing (the
      -- batch cannot settle past its window) and is the one coordinate that
      -- could strand escrow past every window, failing E5's guaranteed
      -- self-cure. It is also what lets ReleaseOrder gate on the order alone.
      .scalarEq (s .orderValidUntilSlot) (s .batchSettlementCloseSlot),
      -- The escrow the admission MOVES: the exact worst case, into the
      -- order's own vault, and the batch commits exactly that.
      .nonzero (s .orderMaxLots),
      .checkedMulInto (s .orderMaxLots) (s .orderMaxQuoteDebitPerLot) (s .orderQuoteReserve),
      .copyScalar (s .orderQuoteReserve) (s .custodyAmount),
      .checkedAddInto (s .batchQuoteReserveObservation) (s .orderQuoteReserve)
        (s .batchPostQuoteReserve),
      -- Record constants the EffectProgram writes into the vacant account.
      -- `scratchA` carried the signed terms' width until the equality above
      -- consumed it; from here it carries the record magic.
      .loadConst (s .orderPostPhase) orderPhasePlaced,
      .loadConst (s .scratchB) orderRecordPhase,
      .loadConst (s .scratchA) orderRecordMagicWord,
      -- The quote-deposit route's guard is a PROVEN consequence of the signed
      -- terms: active exactly when the reserve is nonzero. A runtime bank can
      -- therefore never skip a deposit the batch just committed, and a
      -- pure-claims order (zero reserve) does not attempt a zero transfer.
      .minInto (s .orderQuoteReserve) (s .one) (s .custodyActive),
      -- The claims escrow-in: maker source (index zero) to the freshly
      -- admitted escrow Position (index one), nothing minted. A Position
      -- admit does not advance the Claims market (its evidence records
      -- before == after), so the escrow transfer expects the same observation
      -- the admit did and leaves its successor; a freshly admitted Position's
      -- revision is ZERO, and pinning it here means only a Position the admit
      -- just created can receive the escrow.
      .incrementInto (s .claimsMarketRevision) (s .claimsPostMarketRevision),
      .loadConst (s .positionOneRevision) 0,
      .loadConst (s .claimsSourcePresent) 1,
      .loadConst (s .claimsDestinationPresent) 1,
      .loadConst (s .claimsSourcePositionIndex) 0,
      .loadConst (s .claimsDestinationPositionIndex) 1,
      .loadConst (s .claimsAggregateDirection) claimsDeltaNeutral,
      .loadConst (s .claimsSourceDirection) claimsDeltaDebit,
      .loadConst (s .claimsDestinationDirection) claimsDeltaCredit,
      -- The 0010 SS2a addressing discipline, deposit direction: atoms leave
      -- the MAKER's external account and claims leave the MAKER's Position,
      -- and both arrive at addresses keyed by the order's own identity.
      .identityEq (d .destinationVaultContext) (d .order),
      .identityEq (d .custodySourceOwner) (d .owner),
      .identityEq (d .positionZeroOwner) (d .owner),
      .identityEq (d .positionOneOwner) (d .order),
      .identityEq (d .settlementPositionOwner) (d .order),
      .identityEq (d .rentCredit) (d .owner),
      -- The maker who PAYS is the maker the signed terms NAME. See
      -- `submitCandidate` above for why this join cannot live in the
      -- AccountProfile as a `RequireKey`.
      .identityEq (d .payer) (d .owner),
      .loadConst (s .custodyOperation) custodyOperationTransfer
    ]
  | .cancelOrder => [
      -- The second derived state: the order the maker is cancelling. Its bump
      -- witness, Trading ownership, and live rent principal are anchored the
      -- way settlement Close anchors its terminal record.
      .scalarEq (s .terminalRecordBump) (s .terminalCanonicalBump),
      .identityEq (d .terminalOwner) (d .tradingProgram),
      .nonzero (s .terminalRentPrincipal),
      -- The order record's width must be the Product width the prelude
      -- already equated with the batch's (`zero`).
      .scalarEq (s .scratchA) (s .outcomeCount),
      -- Only while the batch is COLLECTING, and only a PLACED order: after
      -- the close the order set is final and a candidate may already be built
      -- against this escrow. `Collecting.tag()` and `Placed.tag()` are both
      -- one by construction, so one constant serves both conjuncts.
      .loadConst (s .scratchB) batchStatusCollecting,
      .scalarEq (s .batchStatusObservation) (s .scratchB),
      .scalarEq (s .orderPhaseObservation) (s .scratchB),
      .scalarLt (s .currentSlot) (s .batchCollectionCloseSlot),
      .loadConst (s .orderPostPhase) orderPhaseCancelled,
      .copyScalar (s .currentSlot) (s .orderPostReleasedSlot),
      .scalarLe (s .orderAdmittedSlotObservation) (s .currentSlot),
      -- The refund is the WHOLE reserve, exactly: the batch is still
      -- collecting, so no Collect can have drawn on this vault, and a partial
      -- refund would strand the difference. The batch counter surrenders
      -- exactly what admission committed -- `subInto` refuses a batch that
      -- never held it.
      .checkedMulInto (s .orderMaxLots) (s .orderMaxQuoteDebitPerLot) (s .orderQuoteReserve),
      .copyScalar (s .orderQuoteReserve) (s .custodyAmount),
      .subInto (s .batchQuoteReserveObservation) (s .orderQuoteReserve)
        (s .batchPostQuoteReserve),
      -- One more cancellation, and never more cancellations than admissions.
      .scalarLt (s .batchCancelledCountObservation) (s .batchOrderCountObservation),
      .incrementInto (s .batchCancelledCountObservation) (s .batchPostCancelledCount),
      -- The four-route close suite and the claims refund advance exactly as
      -- ReleaseOrder's do: one revision per custody operation, one Claims
      -- market advance, and the escrow Position close expects its post-affine
      -- successor.
      .incrementInto (s .custodyExpectedRevision) (s .custodyResultingRevision),
      .incrementInto (s .custodyResultingRevision) (s .custodyCloseVaultExpectedRevision),
      .incrementInto (s .custodyCloseVaultExpectedRevision) (s .custodyCloseVaultResultingRevision),
      .incrementInto
        (s .custodyCloseVaultResultingRevision) (s .custodyCloseReplayResultingRevision),
      .loadConst (s .custodyOperation) custodyOperationTransfer,
      .incrementInto (s .claimsMarketRevision) (s .claimsPostMarketRevision),
      .incrementInto (s .positionZeroRevision) (s .settlementPositionRevision),
      .incrementInto (s .settlementPositionRevision) (s .settlementPostPositionRevision),
      .loadConst (s .claimsSourcePresent) 1,
      .loadConst (s .claimsDestinationPresent) 1,
      .loadConst (s .claimsSourcePositionIndex) 0,
      .loadConst (s .claimsDestinationPositionIndex) 1,
      .loadConst (s .claimsAggregateDirection) claimsDeltaNeutral,
      .loadConst (s .claimsSourceDirection) claimsDeltaDebit,
      .loadConst (s .claimsDestinationDirection) claimsDeltaCredit,
      -- The 0010 SS2a addressing discipline, identical to ReleaseOrder's:
      -- the order's own vault, the recorded maker, the order's Position, and
      -- the maker's rent on every close.
      .identityEq (d .sourceVaultContext) (d .order),
      .identityEq (d .custodyDestinationOwner) (d .owner),
      .identityEq (d .positionZeroOwner) (d .order),
      .identityEq (d .positionOneOwner) (d .owner),
      .identityEq (d .settlementPositionOwner) (d .order),
      .identityEq (d .rentCredit) (d .owner),
      .identityEq (d .rentRefund) (d .owner),
      -- The maker who PAYS is the maker the order record NAMES. See
      -- `submitCandidate` above for why this join cannot live in the
      -- AccountProfile as a `RequireKey`.
      .identityEq (d .payer) (d .owner)
    ]
  | .releaseOrder => [
      -- Only a placed order releases, and it releases to Released. A vacant
      -- state account projects phase zero, which is not Placed, so a release
      -- aimed at an address nothing occupies refuses here.
      .loadConst (s .scratchA) orderPhasePlaced,
      .scalarEq (s .orderPhaseObservation) (s .scratchA),
      .loadConst (s .orderPostPhase) orderPhaseReleased,
      -- The window gate, from the order alone. PlaceOrder pins the signed
      -- valid_until_slot to the batch's settlement close EXACTLY (the recorded
      -- choice that makes this action batch-free), so strictly-after-valid
      -- is the batch window's inclusive release boundary, and no batch account
      -- enters the frame. A maker can therefore ALWAYS reach this refund: the
      -- gate is a constant of the record they signed.
      .scalarLe (s .orderValidUntilSlot) (s .currentSlot),
      -- The successor's released slot is now, and now is not before admission.
      .copyScalar (s .currentSlot) (s .orderPostReleasedSlot),
      .scalarLe (s .orderAdmittedSlotObservation) (s .currentSlot),
      -- The residual is the observed vault balance -- never computed, exactly
      -- as decision 0010 states -- and it can never exceed the exact worst
      -- case admission escrowed.
      .checkedMulInto (s .orderMaxLots) (s .orderMaxQuoteDebitPerLot) (s .orderQuoteReserve),
      .scalarLe (s .escrowBalanceObservation) (s .orderQuoteReserve),
      .copyScalar (s .escrowBalanceObservation) (s .custodyAmount),
      -- The four-route close suite advances one revision per Custody
      -- operation: transfer, then vault close, then replay close -- the same
      -- chain settlement Close carries.
      .incrementInto (s .custodyExpectedRevision) (s .custodyResultingRevision),
      .incrementInto (s .custodyResultingRevision) (s .custodyCloseVaultExpectedRevision),
      .incrementInto (s .custodyCloseVaultExpectedRevision) (s .custodyCloseVaultResultingRevision),
      .incrementInto
        (s .custodyCloseVaultResultingRevision) (s .custodyCloseReplayResultingRevision),
      .loadConst (s .custodyOperation) custodyOperationTransfer,
      -- The claims residual advances the Claims market once, and the escrow
      -- Position close that follows it expects exactly that successor; the
      -- Position's own close-time revision is its post-affine successor.
      .incrementInto (s .claimsMarketRevision) (s .claimsPostMarketRevision),
      .incrementInto (s .positionZeroRevision) (s .settlementPositionRevision),
      .incrementInto (s .settlementPositionRevision) (s .settlementPostPositionRevision),
      -- The residual claims row plumbing is constant at authoring time: the
      -- escrow Position is the sole source (index zero), the maker's the sole
      -- destination (index one), and a transfer mints nothing, so the
      -- aggregate is neutral. The ROW COUNT is deliberately not pinned: an
      -- empty residual carries zero rows, and a runtime that omits a row a
      -- balance still requires leaves the Position nonzero, which the Claims
      -- Position close refuses -- conservation fails closed.
      .loadConst (s .claimsSourcePresent) 1,
      .loadConst (s .claimsDestinationPresent) 1,
      .loadConst (s .claimsSourcePositionIndex) 0,
      .loadConst (s .claimsDestinationPositionIndex) 1,
      .loadConst (s .claimsAggregateDirection) claimsDeltaNeutral,
      .loadConst (s .claimsSourceDirection) claimsDeltaDebit,
      .loadConst (s .claimsDestinationDirection) claimsDeltaCredit,
      -- The 0010 §2a addressing discipline, stated for every leg: the vault
      -- drawn on is the ORDER's own, the refunded owner is the record's maker,
      -- the closed Position is the order's, and every rent credit is the
      -- maker's -- an order's collateral and rent are reachable by exactly
      -- the identities the record names.
      .identityEq (d .sourceVaultContext) (d .order),
      .identityEq (d .custodyDestinationOwner) (d .owner),
      .identityEq (d .positionZeroOwner) (d .order),
      .identityEq (d .positionOneOwner) (d .owner),
      .identityEq (d .settlementPositionOwner) (d .order),
      .identityEq (d .rentCredit) (d .owner),
      .identityEq (d .rentRefund) (d .owner)
    ]
/-- The Product-owned item body, folded once per authenticated outcome. -/
def itemOps (action : Action) : List Op :=
  -- An action with no tail emits NO item section, not even the bound check:
  -- there is no `outcome` register for it to read. `Instruction.wellFormed`
  -- bounds every item operand by the declared stride, so a zero stride and a
  -- non-empty body is not a program at all.
  match action with
  | .openBatch | .closeBatch => []
  | _ =>
  .scalarLt (t .outcome) (s .outcomeCount) ::
    match action with
    | .consider | .freeze | .materialize => []
    | .initializeSettlement => [.loadConst (t .cursorInventory) 0]
    -- `cancelOrder` and `releaseOrder` move their refund claims through the
    -- same two-Position transfer shape as the settlement rows: nothing
    -- minted, source and destination magnitudes exactly the row quantity.
    | .collect | .distribute | .cancelOrder | .releaseOrder => [
        .loadConst (t .claimsAggregateMagnitude) 0,
        .scalarEq (t .claimsSourceMagnitude) (t .quantity),
        .scalarEq (t .claimsDestinationMagnitude) (t .quantity)
      ]
    -- `placeOrder` derives its escrow row from the signed terms: the claim
    -- reserve at each outcome is deliver-per-lot times the order's maximum
    -- fill, moved whole from the maker to the escrow.
    | .placeOrder => [
        .loadConst (t .claimsAggregateMagnitude) 0,
        .checkedMulInto (t .quantity) (s .orderMaxLots) (t .claimsSourceMagnitude),
        .copyScalar (t .claimsSourceMagnitude) (t .claimsDestinationMagnitude)
      ]
    | .close => [
        .loadConst (t .quantity) 0,
        .loadConst (t .claimsAggregateMagnitude) 0,
        .loadConst (t .claimsSourceMagnitude) 0,
        .loadConst (t .claimsDestinationMagnitude) 0,
        .loadConst (t .cursorInventory) 0
      ]
    | _ => []

/-- One action's complete authored program. -/
def program (action : Action) : Program := {
  commonScalars := commonScalars
  itemScalarStride := actionItemScalarStride action
  commonIdentities := commonIdentities
  itemIdentityStride := itemIdentityStride
  «prelude» := commonOps action ++ actionOps action
  itemBody := itemOps action
  epilogue := []
}

/-- All fifteen actions, in tag order. -/
def authoredActions : List Action := [
  .consider, .freeze, .initializeSettlement, .collect, .materialize, .distribute, .close,
  .openBatch, .placeOrder, .cancelOrder, .closeBatch, .submitCandidate, .verifyCandidateRow,
  .releaseOrder, .closeCandidate
]

def programs : List Program := authoredActions.map program

-- ---------------------------------------------------------------------------
-- Geometry, decided
-- ---------------------------------------------------------------------------

theorem the_register_schema_is_the_declared_bank :
    commonScalars = 151 ∧ itemScalarStride = 6 ∧ commonIdentities = 45 ∧
      itemIdentityStride = 0 := by native_decide

/-- The SLOT SCHEMA above is not what every program declares.

`itemScalarStride` is the width of the item slot enum and is unchanged; what
moved is the stride each PROGRAM carries, which is now the action's. Stated as
the exact list rather than by re-deriving `actionItemScalarStride`, so a program
that stopped consulting it is a failing theorem rather than a tautology. -/
theorem every_authored_program_declares_its_actions_stride :
    authoredActions.map (fun action => (program action).itemScalarStride) =
      [6, 6, 6, 6, 6, 6, 6, 0, 6, 6, 0, 6, 6, 6, 6] := by native_decide

/-- Zero stride and an empty item body are the same statement, both ways.

An item operand is bounded by the declared stride, so a zero-stride program with
a non-empty body addresses a register file that does not exist; and a program
with a tail it never emits an instruction over is declaring width nothing reads.
`Iff`, not implication, because both directions are defects. -/
theorem a_zero_stride_action_emits_no_item_body :
    authoredActions.all
        (fun action =>
          ((program action).itemScalarStride == 0) ==
            (program action).itemBody.isEmpty) = true := by
  native_decide

/-- Every constructor list is its own index sequence, so a reordered enum is a
failing theorem rather than a silently renumbered bank. -/
theorem every_slot_list_is_its_own_index_sequence :
    ScalarSlot.all.map ScalarSlot.index = List.range commonScalars ∧
      ItemScalarSlot.all.map ItemScalarSlot.index = List.range itemScalarStride ∧
      IdentitySlot.all.map IdentitySlot.index = List.range commonIdentities := by
  native_decide

theorem every_authored_program_is_well_formed :
    authoredActions.all (fun action => (program action).wellFormed) = true := by
  native_decide

/-- The counts `general_transition_instruction_count_v3` carries as a Rust match. -/
theorem authored_section_counts :
    authoredActions.map
        (fun action =>
          ((program action).prelude.length, (program action).itemBody.length,
            (program action).epilogue.length)) =
      [(15, 1, 0), (17, 1, 0), (21, 2, 0), (21, 4, 0), (16, 1, 0), (21, 4, 0), (27, 6, 0),
        (26, 0, 0), (46, 4, 0), (50, 4, 0), (27, 0, 0), (46, 1, 0), (23, 1, 0),
        (42, 4, 0), (34, 1, 0)] := by
  native_decide

/-- No two authored actions emit the same program. A shared prelude plus an
empty action half would produce two identical artifacts with two identities, and
the digest is what the descriptor and the capability seal name. -/
theorem authored_programs_are_pairwise_distinct :
    programs.Nodup := by native_decide

/-- The item space is addressable only from the item body -- `wellFormed` above
requires it, and this states the consequence that matters: every prelude
conjunct is Product-width-independent, which is what lets one artifact serve
N = 1 and N = 258. -/
theorem no_prelude_conjunct_is_product_width :
    authoredActions.all
        (fun action => ((program action).prelude.all (fun op => !op.usesItemSpace))) = true := by
  native_decide

/-- The three actions whose Custody direction is fixed at authoring time are
exactly the three that bind their vault context, and `Materialize` is not one of
them. -/
theorem exactly_the_fixed_direction_actions_bind_their_vault_context :
    authoredActions.filter (fun action => vaultContextOps action ≠ []) =
      [.collect, .distribute, .close] := by
  native_decide

-- ---------------------------------------------------------------------------
-- Emitted width
-- ---------------------------------------------------------------------------

def encodedWidth (action : Action) : Nat := (Codec.encodeProgram (program action)).length

theorem authored_encoded_widths :
    authoredActions.map encodedWidth =
      -- openBatch 680 -> 656 and closeBatch 704 -> 680: one 24-byte instruction
      -- each, the bound check that had no register to bound. The 32-byte header
      -- is unchanged; only the two zero-stride actions move.
      [416, 464, 584, 632, 440, 632, 824, 656, 1232, 1328, 680, 1160, 608, 1136,
        872] := by
  native_decide

-- ---------------------------------------------------------------------------
-- The root-writing pair, executed
-- ---------------------------------------------------------------------------

/-- One register bank at Product width 1: every named coordinate set, the rest
zero. Identities are the abstract `Nat` identities of the VM model. -/
private def bankOf (action : Action) (values : List (Nat × Nat))
    (identities : List (Nat × Nat)) : State :=
  ⟨values.foldl (fun bank (coordinate, value) => bank.setIfInBounds coordinate value)
      ((List.replicate (commonScalars + actionItemScalarStride action) 0).toArray),
    identities.foldl (fun bank (coordinate, value) => bank.setIfInBounds coordinate value)
      ((List.replicate commonIdentities 0).toArray)⟩

/-- The shared prelude's demands at width one, plus a live root. -/
private def commonBank : List (Nat × Nat) := [
  (ScalarSlot.index .rootLifecycleObservation, lifecycleActive),
  (ScalarSlot.index .outcomeCount, 1), (ScalarSlot.index .zero, 1),
  (ScalarSlot.index .stateBump, 7), (ScalarSlot.index .primaryCanonicalBump, 7),
  (ScalarSlot.index .primaryRentPrincipal, 1)
]

private def commonIdentityBank : List (Nat × Nat) := [
  (IdentitySlot.index .primaryOwner, 9), (IdentitySlot.index .tradingProgram, 9)
]

-- ---------------------------------------------------------------------------
-- VerifyCandidateRow, executed
-- ---------------------------------------------------------------------------

/-- One authenticated VerifyCandidateRow bank. The arguments expose every
request/status join plus the terminal and projected order successor, so each
hostile theorem below changes exactly one fact. -/
private def verifyCandidateRowBank
    (status subject expectedRevision expectedPage expectedRow terminal postOrderCount : Nat) : State :=
  bankOf .verifyCandidateRow (commonBank ++ [
      (ScalarSlot.index .candidateStatusObservation, status),
      (ScalarSlot.index .rootExpectedRevision, expectedRevision),
      (ScalarSlot.index .verifyRevisionObservation, 5),
      (ScalarSlot.index .verifyPostRevision, 6),
      (ScalarSlot.index .completeSetMove, expectedPage),
      (ScalarSlot.index .verifyPageObservation, 2),
      (ScalarSlot.index .claimsAffineActive, expectedRow),
      (ScalarSlot.index .verifyRowObservation, 3),
      (ScalarSlot.index .verifyOrderCountObservation, 4),
      (ScalarSlot.index .verifyPostOrderCount, postOrderCount),
      (ScalarSlot.index .verifyTerminal, terminal)
    ]) (commonIdentityBank ++ [
      (IdentitySlot.index .parentRequestDigest, subject),
      (IdentitySlot.index .candidate, 7)
    ])

/-- Both nonterminal and terminal rows accept, derive revision +1, and permit
the distinct-order count to stay put or advance by exactly one. Lifecycle V5,
not this transition, conditionally creates the raw certificate on the latter. -/
theorem verify_candidate_row_accepts_exact_current_coordinates :
    ((program .verifyCandidateRow).execute 1
        (verifyCandidateRowBank candidateStatusSubmitted 7 5 2 3 0 4)).isSome = true ∧
      ((program .verifyCandidateRow).execute 1
        (verifyCandidateRowBank candidateStatusSubmitted 7 5 2 3 1 5)).map (fun state =>
          (state.scalars[ScalarSlot.index .verifyPostRevision]!,
            state.scalars[ScalarSlot.index .verifyPostOrderCount]!,
            state.scalars[ScalarSlot.index .verifyTerminal]!)) = some (6, 5, 1) := by
  native_decide

/-- A substituted optimistic revision cannot consume the cursor. -/
theorem verify_candidate_row_refuses_a_substituted_revision :
    (program .verifyCandidateRow).execute 1
      (verifyCandidateRowBank candidateStatusSubmitted 7 4 2 3 0 4) = none := by
  native_decide

/-- A substituted page coordinate cannot select another immutable Page. -/
theorem verify_candidate_row_refuses_a_substituted_page :
    (program .verifyCandidateRow).execute 1
      (verifyCandidateRowBank candidateStatusSubmitted 7 5 1 3 0 4) = none := by
  native_decide

/-- A substituted row coordinate cannot select another execution row. -/
theorem verify_candidate_row_refuses_a_substituted_row :
    (program .verifyCandidateRow).execute 1
      (verifyCandidateRowBank candidateStatusSubmitted 7 5 2 2 0 4) = none := by
  native_decide

/-- The lifecycle guard's source is canonical boolean state, never an arbitrary
nonzero terminal marker. -/
theorem verify_candidate_row_refuses_a_substituted_terminal :
    (program .verifyCandidateRow).execute 1
      (verifyCandidateRowBank candidateStatusSubmitted 7 5 2 3 2 5) = none := by
  native_decide

/-- One row cannot erase an observed order or skip an order coordinate. -/
theorem verify_candidate_row_refuses_a_nonlocal_order_successor :
    (program .verifyCandidateRow).execute 1
        (verifyCandidateRowBank candidateStatusSubmitted 7 5 2 3 0 3) = none ∧
      (program .verifyCandidateRow).execute 1
        (verifyCandidateRowBank candidateStatusSubmitted 7 5 2 3 0 6) = none := by
  native_decide

/-- Only the authenticated Submitted Candidate named by the request advances. -/
theorem verify_candidate_row_refuses_wrong_status_or_subject :
    (program .verifyCandidateRow).execute 1
        (verifyCandidateRowBank 0 7 5 2 3 0 4) = none ∧
      (program .verifyCandidateRow).execute 1
        (verifyCandidateRowBank candidateStatusSubmitted 8 5 2 3 0 4) = none := by
  native_decide

/-- An OpenBatch bank whose optimistic revision matches the observed root. -/
private def openBatchBank (expectedRevision : Nat) : State :=
  bankOf .openBatch (commonBank ++ [
      (ScalarSlot.index .currentSlot, 1000),
      (ScalarSlot.index .rootExpectedRevision, expectedRevision),
      (ScalarSlot.index .rootRevisionObservation, 5),
      (ScalarSlot.index .rootNextBatchSequenceObservation, 3),
      (ScalarSlot.index .rootOpenBatchesObservation, 0),
      (ScalarSlot.index .configCollectionSlots, 10),
      (ScalarSlot.index .configSelectionSlots, 10),
      (ScalarSlot.index .configSettlementSlots, 10),
      (ScalarSlot.index .configMaxOrders, 8)
    ]) commonIdentityBank

/-- OpenBatch accepts the exact bank and computes the three root advances and
the two config-derived windows. -/
theorem open_batch_accepts_and_advances_the_root :
    ((program .openBatch).execute 1 (openBatchBank 5)).map (fun state =>
        (state.scalars[ScalarSlot.index .rootPostRevision]!,
          state.scalars[ScalarSlot.index .rootPostBatchSequence]!,
          state.scalars[ScalarSlot.index .rootPostOpenBatches]!,
          state.scalars[ScalarSlot.index .batchCollectionCloseSlot]!,
          state.scalars[ScalarSlot.index .batchSettlementCloseSlot]!)) =
      some (6, 4, 1, 1010, 1030) := by native_decide

/-- The replay guard is real: a stale optimistic revision refuses. -/
theorem open_batch_refuses_a_stale_root_revision :
    (program .openBatch).execute 1 (openBatchBank 4) = none := by native_decide

/-- A CloseBatch bank: window state and batch counters are the parameters. -/
private def closeBatchBank (currentSlot orderCount openBatches : Nat) : State :=
  bankOf .closeBatch (commonBank ++ [
      (ScalarSlot.index .currentSlot, currentSlot),
      (ScalarSlot.index .rootExpectedRevision, 6),
      (ScalarSlot.index .rootRevisionObservation, 6),
      (ScalarSlot.index .rootOpenBatchesObservation, openBatches),
      (ScalarSlot.index .batchStatusObservation, batchStatusCollecting),
      (ScalarSlot.index .batchCollectionCloseSlot, 500),
      (ScalarSlot.index .batchOrderCountObservation, orderCount),
      (ScalarSlot.index .configMaxOrders, 8)
    ]) commonIdentityBank

/-- After the window, a close accepts and the decrement is exact. -/
theorem close_batch_accepts_after_the_window :
    ((program .closeBatch).execute 1 (closeBatchBank 1000 0 1)).map (fun state =>
        (state.scalars[ScalarSlot.index .rootPostRevision]!,
          state.scalars[ScalarSlot.index .rootPostOpenBatches]!,
          state.scalars[ScalarSlot.index .batchPostStatus]!)) =
      some (7, 0, batchStatusClosed) := by native_decide

/-- A full batch may close early: nobody's window is truncated. -/
theorem close_batch_accepts_a_full_batch_early :
    ((program .closeBatch).execute 1 (closeBatchBank 100 8 1)).isSome = true := by
  native_decide

/-- A live, unfull batch refuses an early close: this is the griefing arm of
`close_is_permissionless`, and it is the disjunction's only refusing branch. -/
theorem close_batch_refuses_an_early_close_of_a_live_batch :
    (program .closeBatch).execute 1 (closeBatchBank 100 7 1) = none := by native_decide

/-- A close the root never opened refuses: the decrement has no minuend. -/
theorem close_batch_refuses_when_no_batch_is_open :
    (program .closeBatch).execute 1 (closeBatchBank 1000 0 0) = none := by native_decide

-- ---------------------------------------------------------------------------
-- PlaceOrder, executed
-- ---------------------------------------------------------------------------

/-- A PlaceOrder bank: the batch window, its counters, and the signed terms
are the parameters; the identity bindings hold unless a test breaks one. -/
private def placeOrderBank
    (currentSlot orderCount maxOrders validUntil : Nat) : State :=
  bankOf .placeOrder (commonBank ++ [
      (ScalarSlot.index .currentSlot, currentSlot),
      (ScalarSlot.index .terminalRecordBump, 9), (ScalarSlot.index .terminalCanonicalBump, 9),
      (ScalarSlot.index .terminalRentPrincipal, 1),
      (ScalarSlot.index .scratchA, 1),
      (ScalarSlot.index .batchStatusObservation, batchStatusCollecting),
      (ScalarSlot.index .batchCollectionCloseSlot, 1000),
      (ScalarSlot.index .batchSettlementCloseSlot, 3000),
      (ScalarSlot.index .configMaxOrders, maxOrders),
      (ScalarSlot.index .batchOrderCountObservation, orderCount),
      (ScalarSlot.index .batchQuoteReserveObservation, 58),
      (ScalarSlot.index .orderMaxLots, 6),
      (ScalarSlot.index .orderMaxQuoteDebitPerLot, 7),
      (ScalarSlot.index .orderValidUntilSlot, validUntil),
      (ScalarSlot.index .claimsMarketRevision, 11)
    ]) (commonIdentityBank ++ [
      (IdentitySlot.index .order, 3), (IdentitySlot.index .owner, 4),
      (IdentitySlot.index .terminalOwner, 9),
      (IdentitySlot.index .destinationVaultContext, 3),
      (IdentitySlot.index .custodySourceOwner, 4),
      (IdentitySlot.index .positionZeroOwner, 4), (IdentitySlot.index .positionOneOwner, 3),
      (IdentitySlot.index .settlementPositionOwner, 3),
      (IdentitySlot.index .rentCredit, 4), (IdentitySlot.index .payer, 4)
    ])

/-- Inside the window, under the bound, with the expiry pinned to the batch's
settlement close, an admission accepts: the batch commits exactly the worst
case, the count advances by one, and the deposit amount IS that worst case. -/
theorem place_order_accepts_and_commits_the_exact_worst_case :
    ((program .placeOrder).execute 1 (placeOrderBank 100 3 8 3000)).map
        (fun state =>
          [state.scalars[ScalarSlot.index .orderPostPhase]!,
            state.scalars[ScalarSlot.index .orderQuoteReserve]!,
            state.scalars[ScalarSlot.index .custodyAmount]!,
            state.scalars[ScalarSlot.index .batchPostQuoteReserve]!,
            state.scalars[ScalarSlot.index .batchPostOrderCount]!,
            state.scalars[ScalarSlot.index .claimsPostMarketRevision]!,
            state.scalars[ScalarSlot.index .positionOneRevision]!,
            state.scalars[ScalarSlot.index .custodyActive]!]) =
      some [orderPhasePlaced, 42, 42, 100, 4, 12, 0, 1] := by native_decide

/-- At or after the collection close, no admission. -/
theorem place_order_refuses_outside_the_window :
    (program .placeOrder).execute 1 (placeOrderBank 1000 3 8 3000) = none := by native_decide

/-- A full batch admits nothing further. -/
theorem place_order_refuses_a_full_batch :
    (program .placeOrder).execute 1 (placeOrderBank 100 8 8 3000) = none := by native_decide

/-- The expiry pin is exact in both directions: an order that would outlive
the window is refused exactly as one that would die inside it. Recorded
choice 6 -- this is what makes the batch-free ReleaseOrder gate sound and
every escrow self-curable. -/
theorem place_order_refuses_an_unpinned_expiry :
    (program .placeOrder).execute 1 (placeOrderBank 100 3 8 3001) = none ∧
      (program .placeOrder).execute 1 (placeOrderBank 100 3 8 2999) = none := by native_decide

/-- A deposit whose destination vault is keyed by anything but the order's
own identity refuses: the escrow an admission funds must be the one a
cancellation or release can reach. -/
theorem place_order_refuses_a_substituted_escrow_destination :
    ((program .placeOrder).execute 1
        (⟨(placeOrderBank 100 3 8 3000).scalars,
          ((placeOrderBank 100 3 8 3000).identities.setIfInBounds
            (IdentitySlot.index .destinationVaultContext) 8)⟩ : State)) = none := by
  native_decide

-- ---------------------------------------------------------------------------
-- CancelOrder, executed
-- ---------------------------------------------------------------------------

/-- A CancelOrder bank: the batch window, the counters, and the order phase
are the parameters; the identity bindings hold unless a test breaks one. -/
private def cancelOrderBank
    (currentSlot phase orderCount cancelledCount batchReserve : Nat) : State :=
  bankOf .cancelOrder (commonBank ++ [
      (ScalarSlot.index .currentSlot, currentSlot),
      (ScalarSlot.index .terminalRecordBump, 9), (ScalarSlot.index .terminalCanonicalBump, 9),
      (ScalarSlot.index .terminalRentPrincipal, 1),
      (ScalarSlot.index .scratchA, 1),
      (ScalarSlot.index .batchStatusObservation, batchStatusCollecting),
      (ScalarSlot.index .batchCollectionCloseSlot, 1000),
      (ScalarSlot.index .orderPhaseObservation, phase),
      (ScalarSlot.index .orderAdmittedSlotObservation, 10),
      (ScalarSlot.index .orderMaxLots, 6),
      (ScalarSlot.index .orderMaxQuoteDebitPerLot, 7),
      (ScalarSlot.index .batchQuoteReserveObservation, batchReserve),
      (ScalarSlot.index .batchOrderCountObservation, orderCount),
      (ScalarSlot.index .batchCancelledCountObservation, cancelledCount),
      (ScalarSlot.index .custodyExpectedRevision, 5),
      (ScalarSlot.index .claimsMarketRevision, 11),
      (ScalarSlot.index .positionZeroRevision, 3)
    ]) (commonIdentityBank ++ [
      (IdentitySlot.index .order, 3), (IdentitySlot.index .owner, 4),
      (IdentitySlot.index .terminalOwner, 9),
      (IdentitySlot.index .sourceVaultContext, 3),
      (IdentitySlot.index .custodyDestinationOwner, 4),
      (IdentitySlot.index .positionZeroOwner, 3), (IdentitySlot.index .positionOneOwner, 4),
      (IdentitySlot.index .settlementPositionOwner, 3),
      (IdentitySlot.index .rentCredit, 4), (IdentitySlot.index .rentRefund, 4),
      (IdentitySlot.index .payer, 4)
    ])

/-- While the batch collects, a placed order cancels: the phase flips to
Cancelled, the refund is the EXACT whole reserve, the batch surrenders
exactly what admission committed, and the cancellation counter advances by
one. -/
theorem cancel_order_accepts_and_refunds_the_exact_reserve :
    ((program .cancelOrder).execute 1 (cancelOrderBank 100 orderPhasePlaced 3 1 100)).map
        (fun state =>
          [state.scalars[ScalarSlot.index .orderPostPhase]!,
            state.scalars[ScalarSlot.index .orderPostReleasedSlot]!,
            state.scalars[ScalarSlot.index .custodyAmount]!,
            state.scalars[ScalarSlot.index .batchPostQuoteReserve]!,
            state.scalars[ScalarSlot.index .batchPostCancelledCount]!,
            state.scalars[ScalarSlot.index .custodyCloseReplayResultingRevision]!]) =
      some [orderPhaseCancelled, 100, 42, 58, 2, 9] := by native_decide

/-- After the collection window a cancellation refuses: the order set is
final and a candidate may already be built against this escrow. Release is
the verb that remains. -/
theorem cancel_order_refuses_after_the_window :
    (program .cancelOrder).execute 1 (cancelOrderBank 1000 orderPhasePlaced 3 1 100) = none := by
  native_decide

/-- A cancelled order does not cancel again, and a vacant state was never an
order. -/
theorem cancel_order_refuses_an_order_that_is_not_placed :
    (program .cancelOrder).execute 1 (cancelOrderBank 100 orderPhaseCancelled 3 1 100) = none ∧
      (program .cancelOrder).execute 1 (cancelOrderBank 100 0 3 1 100) = none := by
  native_decide

/-- A batch whose committed reserve does not hold this order's worst case is
not the batch that admitted it: the subtraction has no minuend. -/
theorem cancel_order_refuses_a_batch_that_never_held_the_reserve :
    (program .cancelOrder).execute 1 (cancelOrderBank 100 orderPhasePlaced 3 1 41) = none := by
  native_decide

/-- Cancellations can never outnumber admissions. -/
theorem cancel_order_refuses_when_every_admission_is_already_cancelled :
    (program .cancelOrder).execute 1 (cancelOrderBank 100 orderPhasePlaced 3 3 100) = none := by
  native_decide

-- ---------------------------------------------------------------------------
-- ReleaseOrder, executed
-- ---------------------------------------------------------------------------

/-- A ReleaseOrder bank: the phase, the clock, and the observed residual are
the parameters; the identity bindings hold unless a test breaks one. -/
private def releaseOrderBank
    (phase currentSlot escrowBalance sourceVaultContext : Nat) : State :=
  bankOf .releaseOrder (commonBank ++ [
      (ScalarSlot.index .currentSlot, currentSlot),
      (ScalarSlot.index .orderPhaseObservation, phase),
      (ScalarSlot.index .orderValidUntilSlot, 500),
      (ScalarSlot.index .orderAdmittedSlotObservation, 10),
      (ScalarSlot.index .orderMaxLots, 6),
      (ScalarSlot.index .orderMaxQuoteDebitPerLot, 7),
      (ScalarSlot.index .escrowBalanceObservation, escrowBalance),
      (ScalarSlot.index .custodyExpectedRevision, 5),
      (ScalarSlot.index .claimsMarketRevision, 11),
      (ScalarSlot.index .positionZeroRevision, 3)
    ]) (commonIdentityBank ++ [
      (IdentitySlot.index .order, 3), (IdentitySlot.index .owner, 4),
      (IdentitySlot.index .sourceVaultContext, sourceVaultContext),
      (IdentitySlot.index .custodyDestinationOwner, 4),
      (IdentitySlot.index .positionZeroOwner, 3), (IdentitySlot.index .positionOneOwner, 4),
      (IdentitySlot.index .settlementPositionOwner, 3),
      (IdentitySlot.index .rentCredit, 4), (IdentitySlot.index .rentRefund, 4)
    ])

/-- After the signed expiry, a placed order releases: the phase flips to
Released, the released slot is now, the amount moved is the OBSERVED balance
(never the computed reserve), and the custody close chain advances one
revision per operation. -/
theorem release_order_accepts_and_returns_the_observed_residual :
    ((program .releaseOrder).execute 1 (releaseOrderBank orderPhasePlaced 1000 40 3)).map
        (fun state =>
          [state.scalars[ScalarSlot.index .orderPostPhase]!,
            state.scalars[ScalarSlot.index .orderPostReleasedSlot]!,
            state.scalars[ScalarSlot.index .custodyAmount]!,
            state.scalars[ScalarSlot.index .orderQuoteReserve]!,
            state.scalars[ScalarSlot.index .custodyCloseReplayResultingRevision]!,
            state.scalars[ScalarSlot.index .claimsPostMarketRevision]!,
            state.scalars[ScalarSlot.index .settlementPostPositionRevision]!]) =
      some [orderPhaseReleased, 1000, 40, 42, 9, 12, 5] := by native_decide

/-- The signed expiry is the inclusive release boundary, exactly as the Batch
semantic owner defines it. -/
theorem release_order_accepts_at_its_signed_expiry :
    (program .releaseOrder).execute 1 (releaseOrderBank orderPhasePlaced 500 40 3) |>.isSome := by
  native_decide

/-- One slot before the signed expiry remains inside the maker's window. -/
theorem release_order_refuses_before_its_signed_expiry :
    (program .releaseOrder).execute 1 (releaseOrderBank orderPhasePlaced 499 40 3) = none := by
  native_decide

/-- A released order does not release again, and a vacant state (phase zero)
was never an order: both refuse on the same conjunct. -/
theorem release_order_refuses_an_order_that_is_not_placed :
    (program .releaseOrder).execute 1 (releaseOrderBank orderPhaseReleased 1000 40 3) = none ∧
      (program .releaseOrder).execute 1 (releaseOrderBank 0 1000 40 3) = none := by
  native_decide

/-- A residual above the exact admission reserve is not this order's money:
the bound the per-order address was said to give for free, stated as a
conjunct. -/
theorem release_order_refuses_a_residual_above_the_reserve :
    (program .releaseOrder).execute 1 (releaseOrderBank orderPhasePlaced 1000 43 3) = none := by
  native_decide

/-- A vault keyed by anything but the order's own identity refuses: one maker
can never be refunded out of another maker's escrow. -/
theorem release_order_refuses_a_substituted_vault_context :
    (program .releaseOrder).execute 1 (releaseOrderBank orderPhasePlaced 1000 40 8) = none := by
  native_decide

end DClutch.General.TransitionV3
