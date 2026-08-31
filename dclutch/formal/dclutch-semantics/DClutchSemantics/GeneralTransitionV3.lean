import DClutchSemantics.GeneralControllerAbi
import DClutchSemantics.TransitionVMV3
import Std.Tactic

/-!
# General V3 action-selected TransitionVM programs

The seven settlement-half TransitionVM programs, authored here and emitted.
Before this module the General family had **no Lean counterpart at all** for its
transition artifacts: `crates/dclutch-general-adapter-contract/src/
transition_artifacts_v3.rs` built `InstructionV3` values imperatively and
carried its own instruction counts, which is exactly the gap
`DirectOrdinaryV3.lean` closed for Direct at `73f0793`. The seven collection and
candidate actions have no program in either place; authoring theirs is what this
module exists to make possible, and the seven below are the byte-identity gate
that says the transcription is faithful before anything new is added.

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
def itemScalarStride : Nat := ItemScalarSlot.all.length
def commonIdentities : Nat := IdentitySlot.all.length
/-- General has no per-outcome identity tail. -/
def itemIdentityStride : Nat := 0

/-- The local-state kind each action's PRIMARY state envelope carries.

The batch four share the Batch envelope because their primary state is the
batch window itself -- `PlaceOrder` and `CancelOrder` authenticate it and touch
their order as a SECONDARY state. The candidate pair's primary is the
submission record, and `ReleaseOrder`'s is the order it refunds. -/
def stateKind (action : Action) : Nat :=
  match action with
  | .consider | .freeze => kindSelection
  | .openBatch | .placeOrder | .cancelOrder | .closeBatch => kindBatch
  | .submitCandidate | .verifyCandidateRow => kindCandidate
  | .releaseOrder => kindOrder
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

/-- The action-selected half of each prelude.

Exhaustive by name, and the five still-unauthored actions answer with the
empty list: they have no authored program, and `authoredActions` below is
what says so rather than a silently inherited default. -/
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
  | _ => []

/-- The Product-owned item body, folded once per authenticated outcome. -/
def itemOps (action : Action) : List Op :=
  .scalarLt (t .outcome) (s .outcomeCount) ::
    match action with
    | .consider | .freeze | .materialize => []
    | .initializeSettlement => [.loadConst (t .cursorInventory) 0]
    | .collect | .distribute => [
        .loadConst (t .claimsAggregateMagnitude) 0,
        .scalarEq (t .claimsSourceMagnitude) (t .quantity),
        .scalarEq (t .claimsDestinationMagnitude) (t .quantity)
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
  itemScalarStride := itemScalarStride
  commonIdentities := commonIdentities
  itemIdentityStride := itemIdentityStride
  «prelude» := commonOps action ++ actionOps action
  itemBody := itemOps action
  epilogue := []
}

/-- The actions whose transition program is authored here, in tag order.

`openBatch` and `closeBatch` are the root-writing pair of GEN-SEVEN: the two
that advance the `GeneralRootV2` tail and route to no Claims or Custody child.
The remaining five keep their place in `unauthoredActions` until each one's
complete quadruple lands. -/
def authoredActions : List Action := [
  .consider, .freeze, .initializeSettlement, .collect, .materialize, .distribute, .close,
  .openBatch, .closeBatch
]

/-- The five that are not, and whose quadruple is the open work. -/
def unauthoredActions : List Action := [
  .placeOrder, .cancelOrder,
  .submitCandidate, .verifyCandidateRow, .releaseOrder
]

def programs : List Program := authoredActions.map program

-- ---------------------------------------------------------------------------
-- Geometry, decided
-- ---------------------------------------------------------------------------

theorem the_register_schema_is_the_declared_bank :
    commonScalars = 151 ∧ itemScalarStride = 6 ∧ commonIdentities = 45 ∧
      itemIdentityStride = 0 := by native_decide

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
        (26, 1, 0), (27, 1, 0)] := by
  native_decide

/-- No two authored actions emit the same program. A shared prelude plus an
empty action half would produce two identical artifacts with two identities, and
the digest is what the descriptor and the capability seal name. -/
theorem authored_programs_are_pairwise_distinct :
    programs.Nodup := by native_decide

/-- The unauthored actions carry no program, and this is what says so.

`program` is total, so it answers for all fourteen; what it must never do is
answer with something an emitter could mistake for an artifact. An unauthored
action gets the shared prelude and the item bound alone -- no action conjunct at
all -- and the emitter below never asks for one. -/
theorem no_unauthored_action_carries_an_action_conjunct :
    unauthoredActions.all (fun action => actionOps action == []) = true ∧
      unauthoredActions.all (fun action => (itemOps action).length == 1) = true := by
  native_decide

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
    authoredActions.map encodedWidth = [416, 464, 584, 632, 440, 632, 824, 680, 704] := by
  native_decide

-- ---------------------------------------------------------------------------
-- The root-writing pair, executed
-- ---------------------------------------------------------------------------

/-- One register bank at Product width 1: every named coordinate set, the rest
zero. Identities are the abstract `Nat` identities of the VM model. -/
private def bankOf (values : List (Nat × Nat)) (identities : List (Nat × Nat)) : State :=
  ⟨values.foldl (fun bank (coordinate, value) => bank.setIfInBounds coordinate value)
      ((List.replicate (commonScalars + itemScalarStride) 0).toArray),
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

/-- An OpenBatch bank whose optimistic revision matches the observed root. -/
private def openBatchBank (expectedRevision : Nat) : State :=
  bankOf (commonBank ++ [
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
  bankOf (commonBank ++ [
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

end DClutch.General.TransitionV3
