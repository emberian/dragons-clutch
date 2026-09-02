//! Complete General Hot38 candidate-register ABI.
//!
//! The AccountProfile first projects independently authenticated readonly
//! observations into one exact register bank.  General then overwrites only
//! the semantic coordinates it owns, preserving every other byte until the
//! complete runtime-width plan accepts.  The resulting bank contains every
//! identity, revision, Position-table coordinate, and Custody replay fact
//! needed to project canonical `AffineBatchV2`, `ProtocolPositionV2`, and
//! `CustodyRequestV1` packets.  Trading remains the sole writer and CPI
//! authority.

use dclutch_claims_svm::affine_batch_v2::DeltaDirectionV2;
use dclutch_custody_contract::OperationV1;
use dclutch_execution_strategy_contract::v2::{ExecutionCandidateV2, register_bank_bytes_v2};
use dclutch_general_codec::Action;
use dclutch_general_config_contract::{
    root::{GeneralLifecycleV2, GeneralRootV2},
    v3::GeneralConfigV3,
};

use crate::{
    candidate_v1::{
        CandidateVerifyRowBuffersV1, CandidateVerifyRowSummaryV1, CandidateVerifyRowViewV1,
        GeneralCandidateErrorV1, GeneralCandidateLayoutV1, GeneralCandidateV1,
        candidate_certificate_len_v1, verify_candidate_row_v1, verify_candidate_row_workspace_v1,
    },
    collection_v1::{
        BatchStatusV1, EscrowDirectionV1, GeneralBatchLayoutV1, GeneralBatchOpeningV1,
        GeneralBatchV1, GeneralOrderLayoutV1, GeneralOrderPhaseV1, GeneralOrderV1,
        GeneralSignedOrderTermsV1, authenticate_order_residual_release_v1,
    },
    escrow_v1::{WorkEscrowClosePlanV1, WorkEscrowObservationV1},
    gen_seven_v1::{
        GeneralSevenPlanErrorV1, authenticate_general_seven_request_v1,
        plan_candidate_work_escrow_close_v1,
    },
    local_state_v3::{GeneralLocalStateKindV3, GeneralLocalStateLayoutV3},
    runtime_selection::{
        RuntimeSelectionCursorV2, RuntimeSelectionLayoutV2, RuntimeSelectionPhaseV2,
    },
    runtime_settlement::{RuntimeSettlementActionV2, RuntimeSettlementEffectPlanV2},
    runtime_verify::{RuntimeCandidateVerifierV2, RuntimeCompleteSetMoveV2},
    runtime_width::{CandidateV2, SettlementCursorLayoutV2, SettlementCursorV2},
};

#[cfg(test)]
use crate::collection_v1::{
    GeneralOrderHeaderV1, GeneralOrderStateV1, MakerFundingV1, general_order_len_v1,
    general_signed_order_terms_len_v1,
};

/// Exact common scalar-register count in the General Hot38 ABI.
///
/// Coordinates 0..=89 are the settlement bank; 90..=150 are the GEN-SEVEN
/// widening for the collection and candidate actions. Widening changed the
/// two-byte width field in every artifact header — every General artifact
/// digest moved — and no settlement conjunct, because no settlement program
/// addresses a widened coordinate. General has no published on-chain substrate,
/// so the re-digest strands nothing; the next cohort cut publishes the new
/// records.
pub const GENERAL_HOT_COMMON_SCALARS_V3: u32 = 151;
/// Outcome index, quantity, three claim magnitudes, and cursor inventory.
///
/// The width of the item slot ENUM, which is not what every action declares.
/// Ask [`general_hot_item_scalar_stride_v3`].
pub const GENERAL_HOT_ITEM_SCALAR_STRIDE_V3: u32 = 6;

/// The per-outcome scalar stride ONE action declares.
///
/// Lean is the author: `DClutchSemantics.GeneralTransitionV3.actionItemScalarStride`
/// and `GeneralRequestProfilesV1.actionItemScalarStride` decide this, and both
/// emitted artifacts carry the result --
/// `GENERAL_OPEN_BATCH_ITEM_INSTRUCTIONS_V3` is 0 and
/// `GENERAL_OPEN_BATCH_REQUEST_PROFILE_V1` carries `0x00` where the other
/// thirteen carry `0x06`. This function is the Rust half of that decision, and
/// `artifacts_v3` joins all four artifacts to it.
///
/// WHY THE TWO BATCH ACTIONS DECLARE NOTHING. The batch record has no
/// per-outcome tail. Their effect already declared zero item operations, and
/// the only item instruction their transition ever emitted was the shared bound
/// check `OUTCOME < OUTCOME_COUNT` -- on a register whose sole legal value is
/// the coordinate it occupies, which `hot_candidate_v3` refuses anything else
/// for. Neither action read the tail it declared; the declaration was the only
/// thing making the bank grow with the Product width.
///
/// WHAT IT COST, measured on the real-ELF `OpenBatch` campaign 2026-09-02
/// before this landed: the Trading heap peak was `59,376 + 528*(N - 2)` bytes
/// of 65,536 -- an identity that reproduced both measured peaks and predicted
/// the abort, N = 13 peaking at 65,184 and committing while N = 14 needed
/// 65,712 and the allocator died. Only 48 of those 528 bytes was declared
/// width; the rest was the same width copied through eleven full-width banks a
/// no-op `dealloc` never reclaims. At stride zero the peak is flat in N, and
/// the scratch-page span stops growing with it because the page count is
/// derived from the bank width.
#[must_use]
pub const fn general_hot_item_scalar_stride_v3(action: Action) -> u32 {
    match action {
        Action::OpenBatch | Action::CloseBatch => 0,
        _ => GENERAL_HOT_ITEM_SCALAR_STRIDE_V3,
    }
}
/// Exact common identity-register count in the General Hot38 ABI.
pub const GENERAL_HOT_COMMON_IDENTITIES_V3: u32 = 45;
/// General has no per-outcome identity tail.
pub const GENERAL_HOT_ITEM_IDENTITY_STRIDE_V3: u32 = 0;

/// Scalar coordinates consumed by exact child-packet projection.
pub mod scalar {
    /// Settlement action tag.
    pub const ACTION: u32 = 0;
    /// Complete-set movement tag.
    pub const COMPLETE_SET_MOVE: u32 = 1;
    /// Whether the affine Claims route is active.
    pub const CLAIMS_AFFINE_ACTIVE: u32 = 2;
    /// Whether the Custody route is active.
    pub const CUSTODY_ACTIVE: u32 = 3;
    /// Whether the General state becomes terminal.
    pub const TERMINAL: u32 = 4;
    /// One-based order coordinate.
    pub const ORDER_COORDINATE: u32 = 5;
    /// Consumed General settlement revision.
    pub const SETTLEMENT_REVISION: u32 = 6;
    /// Signed-order replay nonce.
    pub const ORDER_NONCE: u32 = 7;
    /// Exact collateral quantity and Custody transfer amount.
    pub const QUOTE_QUANTITY: u32 = 8;
    /// Uniform complete-set quantity.
    pub const COMPLETE_SET_QUANTITY: u32 = 9;
    /// Product-authenticated outcome count echoed by AccountProfile.
    pub const OUTCOME_COUNT: u32 = 10;
    /// Nonzero General terminal coordinate.
    pub const TERMINAL_COORDINATE: u32 = 11;
    /// Immutable Market generation.
    pub const GENERATION: u32 = 12;
    /// Immutable page coordinate.
    pub const PAGE_INDEX: u32 = 13;
    /// Immutable execution coordinate.
    pub const EXECUTION_INDEX: u32 = 14;
    /// Ordered Custody transfer coordinate.
    pub const TRANSFER_INDEX: u32 = 15;
    /// Custody replay pre-revision.
    pub const CUSTODY_EXPECTED_REVISION: u32 = 16;
    /// Custody replay required post-revision.
    pub const CUSTODY_RESULTING_REVISION: u32 = 17;
    /// Exact Custody rent movement; zero for settlement transfers.
    pub const CUSTODY_RENT_LAMPORTS: u32 = 18;
    /// Claims aggregate pre-revision.
    pub const CLAIMS_MARKET_REVISION: u32 = 19;
    /// Current row-owner Position pre-revision.
    pub const OWNER_POSITION_REVISION: u32 = 20;
    /// Current settlement Position pre-revision.
    pub const SETTLEMENT_POSITION_REVISION: u32 = 21;
    /// Exact AffineBatch position-table count.
    pub const CLAIMS_POSITION_COUNT: u32 = 22;
    /// Exact AffineBatch row count, equal to Product N when active.
    pub const CLAIMS_ROW_COUNT: u32 = 23;
    /// Reserved zero: successor rows never admit a Position.
    pub const CLAIMS_ADMIT_ACTIVE: u32 = 24;
    /// Reserved zero: successor rows never close a Position.
    pub const CLAIMS_CLOSE_ACTIVE: u32 = 25;
    /// Canonical Custody operation tag (`Transfer`).
    pub const CUSTODY_OPERATION: u32 = 26;
    /// Canonical Custody source-compartment tag.
    pub const CUSTODY_SOURCE_COMPARTMENT: u32 = 27;
    /// Canonical Custody destination-compartment tag.
    pub const CUSTODY_DESTINATION_COMPARTMENT: u32 = 28;
    /// Affine row source-presence bit.
    pub const CLAIMS_SOURCE_PRESENT: u32 = 29;
    /// Affine row destination-presence bit.
    pub const CLAIMS_DESTINATION_PRESENT: u32 = 30;
    /// Canonical sorted Position-table source index.
    pub const CLAIMS_SOURCE_POSITION_INDEX: u32 = 31;
    /// Canonical sorted Position-table destination index.
    pub const CLAIMS_DESTINATION_POSITION_INDEX: u32 = 32;
    /// Affine aggregate signed-magnitude direction.
    pub const CLAIMS_AGGREGATE_DIRECTION: u32 = 33;
    /// Affine source signed-magnitude direction.
    pub const CLAIMS_SOURCE_DIRECTION: u32 = 34;
    /// Affine destination signed-magnitude direction.
    pub const CLAIMS_DESTINATION_DIRECTION: u32 = 35;
    /// Observed settlement Position lamports for lifecycle admission/close.
    pub const OBSERVED_POSITION_LAMPORTS: u32 = 36;
    /// Observed admission-record lamports.
    pub const OBSERVED_ADMISSION_LAMPORTS: u32 = 37;
    /// Current Position rent principal.
    pub const POSITION_RENT_PRINCIPAL: u32 = 38;
    /// Current admission-record rent principal.
    pub const ADMISSION_RENT_PRINCIPAL: u32 = 39;
    /// Whether the settlement Position exists before this action.
    pub const SETTLEMENT_POSITION_PRESENT: u32 = 40;
    /// Canonical sorted Position-table coordinate zero pre-revision.
    pub const POSITION_ZERO_REVISION: u32 = 41;
    /// Canonical sorted Position-table coordinate one pre-revision.
    pub const POSITION_ONE_REVISION: u32 = 42;
    /// Exact total number of active position coordinates (zero, one, or two).
    pub const POSITION_TABLE_COUNT: u32 = 43;
    /// Claims aggregate revision after the one affine mutation.
    pub const CLAIMS_POST_MARKET_REVISION: u32 = 44;
    /// Settlement Position revision after the one affine mutation.
    pub const SETTLEMENT_POST_POSITION_REVISION: u32 = 45;
    /// Exact amount for the selected Custody request.
    pub const CUSTODY_AMOUNT: u32 = 46;
    /// Exact replay-account rent principal used for initialize/terminal close.
    pub const CUSTODY_REPLAY_RENT_LAMPORTS: u32 = 47;
    /// Exact vault-account rent principal used for open/terminal close.
    pub const CUSTODY_VAULT_RENT_LAMPORTS: u32 = 48;
    /// CloseVault pre-revision after the optional terminal surplus transfer.
    pub const CUSTODY_CLOSE_VAULT_EXPECTED_REVISION: u32 = 49;
    /// CloseVault post-revision and CloseReplay pre-revision.
    pub const CUSTODY_CLOSE_VAULT_RESULTING_REVISION: u32 = 50;
    /// Terminal CloseReplay post-revision.
    pub const CUSTODY_CLOSE_REPLAY_RESULTING_REVISION: u32 = 51;
    /// Canonical zero used only by initialization child templates.
    pub const ZERO: u32 = 52;
    /// Selection open/frozen phase.
    pub const SELECTION_PHASE: u32 = 53;
    /// Selection successor revision.
    pub const SELECTION_REVISION: u32 = 54;
    /// Number of distinct submitted candidates considered.
    pub const SELECTION_SUBMITTED_COUNT: u32 = 55;
    /// Best valid submitted Candidate coordinate.
    pub const SELECTION_BEST_CANDIDATE_COORDINATE: u32 = 56;
    /// Verification revision of the best submitted certificate.
    pub const SELECTION_BEST_VERIFIED_REVISION: u32 = 57;
    /// Selection comparison-domain price scale.
    pub const SELECTION_PRICE_SCALE: u32 = 58;
    /// Canonical Selection Cursor magic encoded as a scalar.
    pub const SELECTION_MAGIC: u32 = 59;
    /// Canonical runtime-width record ABI version.
    pub const RUNTIME_WIDTH_VERSION: u32 = 60;
    /// Settlement Cursor successor phase.
    pub const CURSOR_PHASE: u32 = 61;
    /// Settlement Cursor immutable verifier-emitted order count.
    pub const CURSOR_ORDER_COUNT: u32 = 62;
    /// Settlement Cursor next order coordinate.
    pub const CURSOR_NEXT_ORDER: u32 = 63;
    /// Settlement Cursor successor revision.
    pub const CURSOR_RESULTING_REVISION: u32 = 64;
    /// Settlement Cursor successor quote inventory.
    pub const CURSOR_QUOTE_INVENTORY: u32 = 65;
    /// Settlement Cursor immutable complete-set quantity.
    pub const CURSOR_COMPLETE_SET_QUANTITY: u32 = 66;
    /// Settlement Cursor canonical magic encoded as a scalar.
    pub const CURSOR_MAGIC: u32 = 67;
    /// Terminal coordinate persisted in the Settlement Cursor.
    pub const CURSOR_TERMINAL_COORDINATE: u32 = 68;
    /// Untrusted primary-state PDA bump witness projected from the request.
    pub const STATE_BUMP: u32 = 69;
    /// Untrusted Close terminal-record PDA bump witness projected from the request.
    pub const TERMINAL_RECORD_BUMP: u32 = 70;
    /// AccountProfile-owned persisted primary-state bump observation.
    pub const PRIMARY_BUMP_OBSERVATION: u32 = 71;
    /// AccountProfile-owned primary-state historical Rent principal observation.
    pub const PRIMARY_PRINCIPAL_OBSERVATION: u32 = 72;
    /// Lifecycle-owned primary-state created/authenticated branch.
    pub const PRIMARY_CREATED: u32 = 73;
    /// Lifecycle-owned primary-state canonical bump.
    pub const PRIMARY_CANONICAL_BUMP: u32 = 74;
    /// Lifecycle-owned primary-state historical Rent principal.
    pub const PRIMARY_RENT_PRINCIPAL: u32 = 75;
    /// AccountProfile-owned persisted terminal-record bump observation.
    pub const TERMINAL_BUMP_OBSERVATION: u32 = 76;
    /// AccountProfile-owned terminal-record historical Rent principal observation.
    pub const TERMINAL_PRINCIPAL_OBSERVATION: u32 = 77;
    /// Lifecycle-owned terminal-record created/authenticated branch.
    pub const TERMINAL_CREATED: u32 = 78;
    /// Lifecycle-owned terminal-record canonical bump.
    pub const TERMINAL_CANONICAL_BUMP: u32 = 79;
    /// Lifecycle-owned terminal-record historical Rent principal.
    pub const TERMINAL_RENT_PRINCIPAL: u32 = 80;
    /// General local-state envelope magic as one little-endian scalar.
    pub const LOCAL_STATE_MAGIC: u32 = 81;
    /// General local-state envelope ABI version.
    pub const LOCAL_STATE_VERSION: u32 = 82;
    /// General local-state selection/settlement kind.
    pub const LOCAL_STATE_KIND: u32 = 83;
    /// Filled-lots component of the best submitted candidate comparison key.
    pub const SELECTION_BEST_FILLED_LOTS: u32 = 84;
    /// Quote-surplus component of the best submitted candidate comparison key.
    pub const SELECTION_BEST_QUOTE_SURPLUS: u32 = 85;
    /// Trusted canonical input scratch-page count derived from bank geometry.
    pub const INPUT_SCRATCH_PAGE_COUNT: u32 = 86;
    /// Verifier-emitted settlement-manifest row ordinal selected by the request.
    pub const MANIFEST_ORDER_INDEX: u32 = 87;
    /// AccountProfile-projected capability-root lifecycle byte.
    ///
    /// The composite root is `CapabilityRootHeaderV1 || GeneralRootV2`. The
    /// header proves identity and says nothing about whether the capability
    /// still accepts work, so the runtime-width path projects the tail's
    /// lifecycle byte itself. Nothing else writes this coordinate: no request
    /// profile projects it (proved in
    /// `DClutchSemantics.GeneralRequestProfilesV1`), so an AccountProfile that
    /// omits the projection leaves it zero and every action refuses.
    pub const ROOT_LIFECYCLE_OBSERVATION: u32 = 88;
    /// Transition-owned `GeneralLifecycleV2::Active` constant.
    pub const ROOT_LIFECYCLE_ACTIVE: u32 = 89;

    // ------------------------------------------------------------------
    // The GEN-SEVEN widening: coordinates for the collection and candidate
    // actions. The settlement seven never address anything below.
    // ------------------------------------------------------------------

    /// Trusted current-slot projection for window-gated actions.
    pub const CURRENT_SLOT: u32 = 90;
    /// Transition-owned constant one, for exact decrements and clamps.
    pub const ONE: u32 = 91;
    /// Transition-owned scratch for compound-window arithmetic.
    pub const SCRATCH_A: u32 = 92;
    /// Transition-owned second scratch for compound-window arithmetic.
    pub const SCRATCH_B: u32 = 93;
    /// Request-projected optimistic root revision for root-writing actions.
    pub const ROOT_EXPECTED_REVISION: u32 = 94;
    /// AccountProfile-projected `GeneralRootV2::revision`.
    pub const ROOT_REVISION_OBSERVATION: u32 = 95;
    /// Transition-owned successor root revision (`observation + 1`).
    pub const ROOT_POST_REVISION: u32 = 96;
    /// AccountProfile-projected `GeneralRootV2::next_batch_sequence`.
    pub const ROOT_NEXT_BATCH_SEQUENCE_OBSERVATION: u32 = 97;
    /// Transition-owned successor next-batch sequence.
    pub const ROOT_POST_BATCH_SEQUENCE: u32 = 98;
    /// AccountProfile-projected `GeneralRootV2::open_batches`.
    pub const ROOT_OPEN_BATCHES_OBSERVATION: u32 = 99;
    /// Transition-owned successor open-batch count (+1 open, -1 close).
    pub const ROOT_POST_OPEN_BATCHES: u32 = 100;
    /// `GeneralConfigV3::collection_slots`, projected from the config record.
    pub const CONFIG_COLLECTION_SLOTS: u32 = 101;
    /// `GeneralConfigV3::selection_slots`, projected from the config record.
    pub const CONFIG_SELECTION_SLOTS: u32 = 102;
    /// `GeneralConfigV3::settlement_slots`, projected from the config record.
    pub const CONFIG_SETTLEMENT_SLOTS: u32 = 103;
    /// `GeneralConfigV3::max_orders_per_candidate` as the per-batch bound.
    pub const CONFIG_MAX_ORDERS: u32 = 104;
    /// AccountProfile-projected batch status byte.
    pub const BATCH_STATUS_OBSERVATION: u32 = 105;
    /// Transition-owned successor batch status.
    pub const BATCH_POST_STATUS: u32 = 106;
    /// AccountProfile-projected admitted-order count.
    pub const BATCH_ORDER_COUNT_OBSERVATION: u32 = 107;
    /// Transition-owned successor admitted-order count.
    pub const BATCH_POST_ORDER_COUNT: u32 = 108;
    /// AccountProfile-projected cancelled-order count.
    pub const BATCH_CANCELLED_COUNT_OBSERVATION: u32 = 109;
    /// Transition-owned successor cancelled-order count.
    pub const BATCH_POST_CANCELLED_COUNT: u32 = 110;
    /// AccountProfile-projected committed quote reserve.
    pub const BATCH_QUOTE_RESERVE_OBSERVATION: u32 = 111;
    /// Transition-owned successor committed quote reserve.
    pub const BATCH_POST_QUOTE_RESERVE: u32 = 112;
    /// Batch collection-close slot: computed at open, projected afterwards.
    pub const BATCH_COLLECTION_CLOSE_SLOT: u32 = 113;
    /// Batch settlement-close slot: computed at open, projected afterwards.
    pub const BATCH_SETTLEMENT_CLOSE_SLOT: u32 = 114;
    /// Signed-order candidate-wide maximum fill.
    pub const ORDER_MAX_LOTS: u32 = 115;
    /// Signed-order maximum quote debit per filled lot.
    pub const ORDER_MAX_QUOTE_DEBIT_PER_LOT: u32 = 116;
    /// Transition-checked exact worst-case quote obligation.
    pub const ORDER_QUOTE_RESERVE: u32 = 117;
    /// Signed-order settlement validity horizon.
    pub const ORDER_VALID_UNTIL_SLOT: u32 = 118;
    /// AccountProfile-projected order escrow phase.
    pub const ORDER_PHASE_OBSERVATION: u32 = 119;
    /// Transition-owned successor order escrow phase.
    pub const ORDER_POST_PHASE: u32 = 120;
    /// AccountProfile-projected order admission slot.
    pub const ORDER_ADMITTED_SLOT_OBSERVATION: u32 = 121;
    /// Transition-owned successor released-at slot.
    pub const ORDER_POST_RELEASED_SLOT: u32 = 122;
    /// Observed order-escrow vault balance for the residual release.
    pub const ESCROW_BALANCE_OBSERVATION: u32 = 123;
    /// Candidate submission's declared immutable page count.
    pub const CANDIDATE_PAGE_COUNT: u32 = 124;
    /// Candidate submission's pinned page revision.
    pub const CANDIDATE_PAGE_REVISION: u32 = 125;
    /// Candidate submission's declared execution-row count.
    pub const CANDIDATE_ROW_COUNT: u32 = 126;
    /// Exact lamports one crank of this candidate's work pays.
    pub const CANDIDATE_REWARD_RATE: u32 = 127;
    /// AccountProfile-projected candidate submission status.
    pub const CANDIDATE_STATUS_OBSERVATION: u32 = 128;
    /// Transition-owned successor candidate submission status.
    pub const CANDIDATE_POST_STATUS: u32 = 129;
    /// AccountProfile-projected verification-compartment lamports.
    pub const CANDIDATE_VERIFICATION_REMAINING_OBSERVATION: u32 = 130;
    /// Transition-owned successor verification-compartment lamports.
    pub const CANDIDATE_POST_VERIFICATION_REMAINING: u32 = 131;
    /// AccountProfile-projected cleanup-compartment lamports.
    pub const CANDIDATE_CLEANUP_REMAINING_OBSERVATION: u32 = 132;
    /// Transition-owned successor cleanup-compartment lamports.
    pub const CANDIDATE_POST_CLEANUP_REMAINING: u32 = 133;
    /// Candidate submission slot stamp.
    pub const CANDIDATE_SUBMITTED_SLOT: u32 = 134;
    /// Evaluator-asserted terminal-row indicator for one verification step.
    pub const VERIFY_TERMINAL: u32 = 135;
    /// AccountProfile-projected verifier-cursor revision.
    pub const VERIFY_REVISION_OBSERVATION: u32 = 136;
    /// Transition-owned successor verifier-cursor revision.
    pub const VERIFY_POST_REVISION: u32 = 137;
    /// AccountProfile-projected verifier page cursor.
    pub const VERIFY_PAGE_OBSERVATION: u32 = 138;
    /// Transition-owned successor verifier page cursor.
    pub const VERIFY_POST_PAGE: u32 = 139;
    /// AccountProfile-projected verifier row cursor.
    pub const VERIFY_ROW_OBSERVATION: u32 = 140;
    /// Transition-owned successor verifier row cursor.
    pub const VERIFY_POST_ROW: u32 = 141;
    /// AccountProfile-projected distinct grouped-order count.
    pub const VERIFY_ORDER_COUNT_OBSERVATION: u32 = 142;
    /// Transition-owned successor distinct grouped-order count.
    pub const VERIFY_POST_ORDER_COUNT: u32 = 143;
    /// Evaluator-asserted manifest rows emitted by one verification step.
    pub const VERIFY_MANIFEST_ORDER_COUNT: u32 = 144;
    /// Request-projected conditional result-state bump witness.
    pub const RESULT_STATE_BUMP: u32 = 145;
    /// AccountProfile-owned result-record bump observation.
    pub const RESULT_BUMP_OBSERVATION: u32 = 146;
    /// AccountProfile-owned result-record rent-principal observation.
    pub const RESULT_PRINCIPAL_OBSERVATION: u32 = 147;
    /// Lifecycle-owned result-record creation indicator.
    pub const RESULT_CREATED: u32 = 148;
    /// Lifecycle-owned result-record canonical bump.
    pub const RESULT_CANONICAL_BUMP: u32 = 149;
    /// Lifecycle-owned result-record historical Rent principal.
    pub const RESULT_RENT_PRINCIPAL: u32 = 150;
}

/// Scalar coordinates within each Product-outcome item bank.
pub mod item_scalar {
    /// Canonical Product outcome index.
    pub const OUTCOME: u32 = 0;
    /// Exact semantic quantity for this outcome.
    pub const QUANTITY: u32 = 1;
    /// Exact aggregate signed-magnitude magnitude.
    pub const CLAIMS_AGGREGATE_MAGNITUDE: u32 = 2;
    /// Exact source-Position signed-magnitude magnitude.
    pub const CLAIMS_SOURCE_MAGNITUDE: u32 = 3;
    /// Exact destination-Position signed-magnitude magnitude.
    pub const CLAIMS_DESTINATION_MAGNITUDE: u32 = 4;
    /// Settlement Cursor successor inventory for this Product outcome.
    pub const CURSOR_INVENTORY: u32 = 5;
}

/// Identity coordinates consumed by exact child-packet projection.
pub mod identity {
    /// Digest of the authenticated Hot parent request.
    pub const PARENT_REQUEST_DIGEST: u32 = 0;
    /// Best-valid-submitted Candidate identity.
    pub const CANDIDATE: u32 = 1;
    /// Current row owner.
    pub const OWNER: u32 = 2;
    /// Current signed-order identity.
    pub const ORDER: u32 = 3;
    /// Immutable terminal surplus beneficiary.
    pub const BENEFICIARY: u32 = 4;
    /// Selected release-set identity.
    pub const RELEASE_SET: u32 = 5;
    /// Canonical logical Market identity.
    pub const MARKET: u32 = 6;
    /// Exact Product record digest.
    pub const PRODUCT_RECORD_DIGEST: u32 = 7;
    /// Semantic LiabilityBasis identity.
    pub const SEMANTIC_BASIS_ID: u32 = 8;
    /// Linked-basis finalized-record digest.
    pub const LINKED_BASIS_RECORD_DIGEST: u32 = 9;
    /// Immutable Realm identity.
    pub const REALM: u32 = 10;
    /// Registry-selected Trading program.
    pub const TRADING_PROGRAM: u32 = 11;
    /// Exact Custody source token account.
    pub const CUSTODY_SOURCE: u32 = 12;
    /// Exact Custody destination token account.
    pub const CUSTODY_DESTINATION: u32 = 13;
    /// Custody source-vault context, zero only for External.
    pub const SOURCE_VAULT_CONTEXT: u32 = 14;
    /// Custody destination-vault context, zero only for External.
    pub const DESTINATION_VAULT_CONTEXT: u32 = 15;
    /// Realm-selected collateral Mint.
    pub const MINT: u32 = 16;
    /// Realm-selected Token or Token-2022 program.
    pub const TOKEN_PROGRAM: u32 = 17;
    /// Custody rent payer; zero for Transfer.
    pub const PAYER: u32 = 18;
    /// Custody rent beneficiary; zero for Transfer.
    pub const RENT_REFUND: u32 = 19;
    /// General settlement Position owner.
    pub const SETTLEMENT_POSITION_OWNER: u32 = 20;
    /// Permanent RentCredit identity for Claims lifecycle.
    pub const RENT_CREDIT: u32 = 21;
    /// Program owning the RentCredit.
    pub const RENT_PROGRAM: u32 = 22;
    /// External Custody source owner, zero for a vault.
    pub const CUSTODY_SOURCE_OWNER: u32 = 23;
    /// External Custody destination owner, zero for a vault.
    pub const CUSTODY_DESTINATION_OWNER: u32 = 24;
    /// Canonical sorted Position-table owner zero.
    pub const POSITION_ZERO_OWNER: u32 = 25;
    /// Canonical sorted Position-table owner one; zero for one-position plans.
    pub const POSITION_ONE_OWNER: u32 = 26;
    /// Canonical General root identity and Custody replay namespace.
    pub const GENERAL_ROOT: u32 = 27;
    /// Product content identity in the selection comparison domain.
    pub const SELECTION_PRODUCT: u32 = 28;
    /// Batch content identity in the selection comparison domain.
    pub const SELECTION_BATCH: u32 = 29;
    /// Immutable interpreted selection-policy content identity.
    pub const SELECTION_POLICY: u32 = 30;
    /// Digest of the exact best submitted VerifiedCandidate record.
    pub const BEST_VERIFIED_DIGEST: u32 = 31;
    /// AccountProfile-owned primary-state RentCredit beneficiary observation.
    pub const PRIMARY_BENEFICIARY_OBSERVATION: u32 = 32;
    /// Lifecycle-owned primary-state RentCredit beneficiary.
    pub const PRIMARY_BENEFICIARY: u32 = 33;
    /// Lifecycle-owned exact primary-state PDA.
    pub const PRIMARY_STATE: u32 = 34;
    /// Lifecycle-owned current Trading program owner.
    pub const PRIMARY_OWNER: u32 = 35;
    /// AccountProfile-owned terminal-record RentCredit beneficiary observation.
    pub const TERMINAL_BENEFICIARY_OBSERVATION: u32 = 36;
    /// Lifecycle-owned terminal-record RentCredit beneficiary.
    pub const TERMINAL_BENEFICIARY: u32 = 37;
    /// Lifecycle-owned exact terminal-record PDA.
    pub const TERMINAL_STATE: u32 = 38;
    /// Lifecycle-owned terminal-record Trading program owner.
    pub const TERMINAL_OWNER: u32 = 39;

    // ------------------------------------------------------------------
    // The GEN-SEVEN widening.
    // ------------------------------------------------------------------

    /// Immutable `GeneralConfigV3` content identity, projected from the root.
    pub const GENERAL_CONFIG_ID: u32 = 40;
    /// AccountProfile-owned result-record RentCredit beneficiary observation.
    pub const RESULT_BENEFICIARY_OBSERVATION: u32 = 41;
    /// Lifecycle-owned result-record RentCredit beneficiary.
    pub const RESULT_BENEFICIARY: u32 = 42;
    /// Lifecycle-owned exact result-record PDA.
    pub const RESULT_STATE: u32 = 43;
    /// Lifecycle-owned result-record Trading program owner.
    pub const RESULT_OWNER: u32 = 44;
}

/// Independently authenticated environment needed by exact Claims/Custody packets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralHotEnvironmentV3 {
    /// Canonical General root identity and Custody replay namespace.
    pub general_root: [u8; 32],
    /// Digest of the exact authenticated Hot request.
    pub parent_request_digest: [u8; 32],
    /// Selected execution release set.
    pub release_set: [u8; 32],
    /// Canonical logical Market.
    pub market: [u8; 32],
    /// Exact Product-record digest.
    pub product_record_digest: [u8; 32],
    /// Immutable `GeneralConfigV3` content identity projected from the root.
    pub general_config_id: [u8; 32],
    /// Exact semantic LiabilityBasis identity.
    pub semantic_basis_id: [u8; 32],
    /// Exact linked-basis finalized-record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Realm collateral authority.
    pub realm: [u8; 32],
    /// Selected Trading program.
    pub trading_program: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact candidate-page coordinate for row actions, zero otherwise.
    pub page_index: u32,
    /// Exact execution coordinate inside the page, zero otherwise.
    pub execution_index: u32,
    /// Claims Market optimistic revision.
    pub claims_market_revision: u64,
    /// Current row-owner Position revision.
    pub owner_position_revision: u64,
    /// Current settlement Position revision.
    pub settlement_position_revision: u64,
    /// Whether the settlement Position exists before this action.
    pub settlement_position_present: bool,
    /// Whether the post-affine settlement Position is proven empty and closeable.
    pub close_settlement_position: bool,
    /// Exact settlement Position owner.
    pub settlement_position_owner: [u8; 32],
    /// Claims Position RentCredit.
    pub rent_credit: [u8; 32],
    /// Program owning the RentCredit.
    pub rent_program: [u8; 32],
    /// Observed Position lamports.
    pub observed_position_lamports: u64,
    /// Observed admission-record lamports.
    pub observed_admission_lamports: u64,
    /// Position rent principal.
    pub position_rent_principal: u64,
    /// Admission-record rent principal.
    pub admission_rent_principal: u64,
    /// Exact Custody source token account.
    pub custody_source: [u8; 32],
    /// Exact Custody destination token account.
    pub custody_destination: [u8; 32],
    /// Exact external source owner or zero.
    pub custody_source_owner: [u8; 32],
    /// Exact external destination owner or zero.
    pub custody_destination_owner: [u8; 32],
    /// Source-vault context or zero for External.
    pub source_vault_context: [u8; 32],
    /// Destination-vault context or zero for External.
    pub destination_vault_context: [u8; 32],
    /// Realm-selected collateral Mint.
    pub mint: [u8; 32],
    /// Realm-selected Token or Token-2022 program.
    pub token_program: [u8; 32],
    /// Custody payer; Transfer requires zero.
    pub payer: [u8; 32],
    /// Custody rent refund; Transfer requires zero.
    pub rent_refund: [u8; 32],
    /// Custody replay pre-revision.
    pub custody_expected_revision: u64,
    /// Ordered transfer index.
    pub transfer_index: u16,
    /// Exact replay-account rent principal.
    pub custody_replay_rent_principal: u64,
    /// Exact vault-account rent principal.
    pub custody_vault_rent_principal: u64,
}

/// Decode the complete independently authenticated environment from the
/// Account/RequestProfile register bank supplied by common Trading.
///
/// The accelerator must not reconstruct these facts from child account order
/// or caller-provided side channels. Every value below is projected once by
/// the selected generic profile and remains read-only at the family boundary.
pub fn general_hot_environment_from_bank_v3(
    action: Action,
    bank: &[u8],
    outcome_count: u32,
) -> Result<GeneralHotEnvironmentV3> {
    // THE WIDTH IS CHECKED AGAINST THE ACTION, AND FIRST. Everything below
    // reads through `scalar_count`, which is where the identity bank starts,
    // so a bank sized for a different stride must refuse here rather than be
    // read at a rebased offset. See `BankStrideMismatch`.
    if bank.len() != general_hot_candidate_bank_len_v3(action, outcome_count)? {
        return Err(GeneralHotCandidateErrorV3::BankStrideMismatch);
    }
    if read_scalar(bank, scalar::OUTCOME_COUNT)? != u64::from(outcome_count) {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    let scalar_count = general_hot_scalar_count_v3(action, outcome_count)?;
    let scalar_u32 = |coordinate| {
        u32::try_from(read_scalar(bank, coordinate)?)
            .map_err(|_| GeneralHotCandidateErrorV3::InvalidCoordinate)
    };
    let scalar_u16 = |coordinate| {
        u16::try_from(read_scalar(bank, coordinate)?)
            .map_err(|_| GeneralHotCandidateErrorV3::InvalidCoordinate)
    };
    let present = read_scalar(bank, scalar::SETTLEMENT_POSITION_PRESENT)?;
    if present > 1 {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    Ok(GeneralHotEnvironmentV3 {
        general_root: read_identity(bank, scalar_count, identity::GENERAL_ROOT)?,
        parent_request_digest: read_identity(bank, scalar_count, identity::PARENT_REQUEST_DIGEST)?,
        release_set: read_identity(bank, scalar_count, identity::RELEASE_SET)?,
        market: read_identity(bank, scalar_count, identity::MARKET)?,
        product_record_digest: read_identity(bank, scalar_count, identity::PRODUCT_RECORD_DIGEST)?,
        general_config_id: read_identity(bank, scalar_count, identity::GENERAL_CONFIG_ID)?,
        semantic_basis_id: read_identity(bank, scalar_count, identity::SEMANTIC_BASIS_ID)?,
        linked_basis_record_digest: read_identity(
            bank,
            scalar_count,
            identity::LINKED_BASIS_RECORD_DIGEST,
        )?,
        realm: read_identity(bank, scalar_count, identity::REALM)?,
        trading_program: read_identity(bank, scalar_count, identity::TRADING_PROGRAM)?,
        generation: read_scalar(bank, scalar::GENERATION)?,
        page_index: scalar_u32(scalar::PAGE_INDEX)?,
        execution_index: scalar_u32(scalar::EXECUTION_INDEX)?,
        claims_market_revision: read_scalar(bank, scalar::CLAIMS_MARKET_REVISION)?,
        owner_position_revision: read_scalar(bank, scalar::OWNER_POSITION_REVISION)?,
        settlement_position_revision: read_scalar(bank, scalar::SETTLEMENT_POSITION_REVISION)?,
        settlement_position_present: present == 1,
        close_settlement_position: read_scalar(bank, scalar::POSITION_TABLE_COUNT)? == 0,
        settlement_position_owner: read_identity(
            bank,
            scalar_count,
            identity::SETTLEMENT_POSITION_OWNER,
        )?,
        rent_credit: read_identity(bank, scalar_count, identity::RENT_CREDIT)?,
        rent_program: read_identity(bank, scalar_count, identity::RENT_PROGRAM)?,
        observed_position_lamports: read_scalar(bank, scalar::OBSERVED_POSITION_LAMPORTS)?,
        observed_admission_lamports: read_scalar(bank, scalar::OBSERVED_ADMISSION_LAMPORTS)?,
        position_rent_principal: read_scalar(bank, scalar::POSITION_RENT_PRINCIPAL)?,
        admission_rent_principal: read_scalar(bank, scalar::ADMISSION_RENT_PRINCIPAL)?,
        custody_source: read_identity(bank, scalar_count, identity::CUSTODY_SOURCE)?,
        custody_destination: read_identity(bank, scalar_count, identity::CUSTODY_DESTINATION)?,
        custody_source_owner: read_identity(bank, scalar_count, identity::CUSTODY_SOURCE_OWNER)?,
        custody_destination_owner: read_identity(
            bank,
            scalar_count,
            identity::CUSTODY_DESTINATION_OWNER,
        )?,
        source_vault_context: read_identity(bank, scalar_count, identity::SOURCE_VAULT_CONTEXT)?,
        destination_vault_context: read_identity(
            bank,
            scalar_count,
            identity::DESTINATION_VAULT_CONTEXT,
        )?,
        mint: read_identity(bank, scalar_count, identity::MINT)?,
        token_program: read_identity(bank, scalar_count, identity::TOKEN_PROGRAM)?,
        payer: read_identity(bank, scalar_count, identity::PAYER)?,
        rent_refund: read_identity(bank, scalar_count, identity::RENT_REFUND)?,
        custody_expected_revision: read_scalar(bank, scalar::CUSTODY_EXPECTED_REVISION)?,
        transfer_index: scalar_u16(scalar::TRANSFER_INDEX)?,
        custody_replay_rent_principal: read_scalar(bank, scalar::CUSTODY_REPLAY_RENT_LAMPORTS)?,
        custody_vault_rent_principal: read_scalar(bank, scalar::CUSTODY_VAULT_RENT_LAMPORTS)?,
    })
}

/// Execute and project one canonical batch opening into a complete candidate bank.
///
/// `root_tail` is the hostile-decoded mutable tail of the real composite root,
/// not a model of it.  The exact config and Product coordinates are joined to
/// the independently projected register observations before
/// [`GeneralBatchV1::open`] is allowed to advance the root.  Only the resulting
/// root and batch facts are written; Trading remains the sole account writer.
pub fn project_general_open_batch_candidate_in_place_v3(
    root_tail: &[u8],
    config: GeneralConfigV3,
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    expected_revision: u64,
    requested_batch_id: Option<[u8; 32]>,
    candidate: &mut [u8],
) -> Result<()> {
    if candidate.len() != general_hot_candidate_bank_len_v3(Action::OpenBatch, outcome_count)? {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    let mut root =
        GeneralRootV2::decode(root_tail).map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let scalar_count = general_hot_scalar_count_v3(Action::OpenBatch, outcome_count)?;
    let current_slot = read_scalar(candidate, scalar::CURRENT_SLOT)?;
    let product_id = read_identity(candidate, scalar_count, identity::SELECTION_PRODUCT)?;
    if product_id == [0; 32]
        || environment.general_config_id == [0; 32]
        || root.lifecycle() != GeneralLifecycleV2::Active
        || root.market() != environment.market
        || root.config_id() != environment.general_config_id
        || root.generation() != environment.generation
        || root.revision() != read_scalar(candidate, scalar::ROOT_REVISION_OBSERVATION)?
        || root.next_batch_sequence()
            != read_scalar(candidate, scalar::ROOT_NEXT_BATCH_SEQUENCE_OBSERVATION)?
        || root.open_batches() != read_scalar(candidate, scalar::ROOT_OPEN_BATCHES_OBSERVATION)?
        || expected_revision != read_scalar(candidate, scalar::ROOT_EXPECTED_REVISION)?
        || read_scalar(candidate, scalar::OUTCOME_COUNT)? != u64::from(outcome_count)
        || read_scalar(candidate, scalar::ZERO)? != u64::from(outcome_count)
        || read_scalar(candidate, scalar::ROOT_LIFECYCLE_OBSERVATION)?
            != u64::from(GeneralLifecycleV2::Active.tag())
        || read_scalar(candidate, scalar::CONFIG_COLLECTION_SLOTS)? != config.collection_slots()
        || read_scalar(candidate, scalar::CONFIG_SELECTION_SLOTS)? != config.selection_slots()
        || read_scalar(candidate, scalar::CONFIG_SETTLEMENT_SLOTS)? != config.settlement_slots()
        || read_scalar(candidate, scalar::CONFIG_MAX_ORDERS)?
            != u64::from(config.max_orders_per_candidate())
        || read_scalar(candidate, scalar::SELECTION_PRICE_SCALE)? != config.price_scale()
        || read_scalar(candidate, scalar::GENERATION)? != config.generation()
        || read_identity(candidate, scalar_count, identity::MARKET)? != root.market()
        || read_identity(candidate, scalar_count, identity::GENERAL_CONFIG_ID)? != root.config_id()
        || read_scalar(candidate, scalar::STATE_BUMP)?
            != read_scalar(candidate, scalar::PRIMARY_CANONICAL_BUMP)?
        || read_identity(candidate, scalar_count, identity::PRIMARY_OWNER)?
            != read_identity(candidate, scalar_count, identity::TRADING_PROGRAM)?
        || read_scalar(candidate, scalar::PRIMARY_RENT_PRINCIPAL)? == 0
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    let collection_close_slot = current_slot
        .checked_add(config.collection_slots())
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    let settlement_close_slot = collection_close_slot
        .checked_add(config.selection_slots())
        .and_then(|slot| slot.checked_add(config.settlement_slots()))
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    let sequence = root.next_batch_sequence();
    let batch = GeneralBatchV1::open(
        &mut root,
        GeneralBatchOpeningV1 {
            outcome_count,
            sequence,
            generation: environment.generation,
            market: environment.market,
            product_id,
            config_id: environment.general_config_id,
            price_scale: config.price_scale(),
            collection_close_slot,
            settlement_close_slot,
            max_orders: config.max_orders_per_candidate(),
        },
        expected_revision,
        current_slot,
    )
    .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let opening = batch.opening();
    let state = batch.state();
    if requested_batch_id != Some(batch.batch_id()) || state.status != BatchStatusV1::Collecting {
        return Err(GeneralHotCandidateErrorV3::InvalidPlan);
    }
    write_local_state_constants(candidate, GeneralLocalStateKindV3::Batch)?;
    for (coordinate, value) in [
        (scalar::ACTION, u64::from(Action::OpenBatch as u8)),
        (
            scalar::ROOT_LIFECYCLE_ACTIVE,
            u64::from(GeneralLifecycleV2::Active.tag()),
        ),
        (scalar::ROOT_POST_REVISION, root.revision()),
        (scalar::ROOT_POST_BATCH_SEQUENCE, root.next_batch_sequence()),
        (scalar::ROOT_POST_OPEN_BATCHES, root.open_batches()),
        (
            scalar::BATCH_COLLECTION_CLOSE_SLOT,
            opening.collection_close_slot,
        ),
        (
            scalar::BATCH_SETTLEMENT_CLOSE_SLOT,
            opening.settlement_close_slot,
        ),
        (scalar::BATCH_POST_STATUS, u64::from(state.status.tag())),
        (
            scalar::ONE,
            u64::from(GeneralBatchLayoutV1::version_value()),
        ),
        (scalar::SCRATCH_A, GeneralBatchLayoutV1::magic_u64()),
        (
            scalar::SCRATCH_B,
            u64::from(GeneralBatchLayoutV1::phase_value()),
        ),
    ] {
        write_scalar(candidate, coordinate, value)?;
    }
    Ok(())
}

/// Execute and project one canonical permissionless batch close.
///
/// Both persisted inputs are hostile-decoded through their semantic owners.
/// The batch is joined back to the live root, config, Product and request
/// subject before [`GeneralBatchV1::close`] may consume the root revision.
/// Closing early is admitted only when the persisted admission count proves
/// the batch full; otherwise the config-derived collection window must have
/// elapsed. Trading remains the sole writer of the root and batch accounts.
pub fn project_general_close_batch_candidate_in_place_v3(
    root_tail: &[u8],
    batch_body: &[u8],
    config: GeneralConfigV3,
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    expected_revision: u64,
    requested_batch_id: Option<[u8; 32]>,
    candidate: &mut [u8],
) -> Result<()> {
    if candidate.len() != general_hot_candidate_bank_len_v3(Action::CloseBatch, outcome_count)? {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    let mut root =
        GeneralRootV2::decode(root_tail).map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let mut batch =
        GeneralBatchV1::decode(batch_body).map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let opening = batch.opening();
    let state = batch.state();
    let scalar_count = general_hot_scalar_count_v3(Action::CloseBatch, outcome_count)?;
    let current_slot = read_scalar(candidate, scalar::CURRENT_SLOT)?;
    let product_id = read_identity(candidate, scalar_count, identity::SELECTION_PRODUCT)?;
    if requested_batch_id != Some(batch.batch_id())
        || product_id == [0; 32]
        || environment.general_config_id == [0; 32]
        || root.lifecycle() != GeneralLifecycleV2::Active
        || root.market() != environment.market
        || root.config_id() != environment.general_config_id
        || root.generation() != environment.generation
        || root.revision() != read_scalar(candidate, scalar::ROOT_REVISION_OBSERVATION)?
        || root.open_batches() != read_scalar(candidate, scalar::ROOT_OPEN_BATCHES_OBSERVATION)?
        || expected_revision != root.revision()
        || expected_revision != read_scalar(candidate, scalar::ROOT_EXPECTED_REVISION)?
        || read_scalar(candidate, scalar::OUTCOME_COUNT)? != u64::from(outcome_count)
        || read_scalar(candidate, scalar::ZERO)? != u64::from(outcome_count)
        || read_scalar(candidate, scalar::ROOT_LIFECYCLE_OBSERVATION)?
            != u64::from(GeneralLifecycleV2::Active.tag())
        || read_scalar(candidate, scalar::BATCH_STATUS_OBSERVATION)?
            != u64::from(state.status.tag())
        || read_scalar(candidate, scalar::BATCH_ORDER_COUNT_OBSERVATION)?
            != u64::from(state.order_count)
        || read_scalar(candidate, scalar::BATCH_COLLECTION_CLOSE_SLOT)?
            != opening.collection_close_slot
        || read_scalar(candidate, scalar::CONFIG_MAX_ORDERS)? != u64::from(opening.max_orders)
        || opening.outcome_count != outcome_count
        || opening.market != root.market()
        || opening.product_id != product_id
        || opening.config_id != root.config_id()
        || opening.generation != root.generation()
        || opening.price_scale != config.price_scale()
        || opening.max_orders != config.max_orders_per_candidate()
        || read_identity(candidate, scalar_count, identity::MARKET)? != root.market()
        || read_identity(candidate, scalar_count, identity::GENERAL_CONFIG_ID)? != root.config_id()
        || read_scalar(candidate, scalar::STATE_BUMP)?
            != read_scalar(candidate, scalar::PRIMARY_CANONICAL_BUMP)?
        || read_identity(candidate, scalar_count, identity::PRIMARY_OWNER)?
            != read_identity(candidate, scalar_count, identity::TRADING_PROGRAM)?
        || read_scalar(candidate, scalar::PRIMARY_RENT_PRINCIPAL)? == 0
        || !batch.close_is_permissionless(current_slot)
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    batch
        .close(&mut root, expected_revision)
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let state = batch.state();
    write_local_state_constants(candidate, GeneralLocalStateKindV3::Batch)?;
    for (coordinate, value) in [
        (scalar::ACTION, u64::from(Action::CloseBatch as u8)),
        (
            scalar::ROOT_LIFECYCLE_ACTIVE,
            u64::from(GeneralLifecycleV2::Active.tag()),
        ),
        (scalar::ROOT_POST_REVISION, root.revision()),
        (scalar::ROOT_POST_OPEN_BATCHES, root.open_batches()),
        (scalar::ONE, 1),
        (scalar::SCRATCH_A, 0),
        (scalar::SCRATCH_B, 0),
        (scalar::BATCH_POST_STATUS, u64::from(state.status.tag())),
    ] {
        write_scalar(candidate, coordinate, value)?;
    }
    Ok(())
}

/// Authenticate and project one permissionless candidate submission.
///
/// `batch_body`, `candidate_body`, and `submission_body` are the exact hostile
/// records selected by the authored AccountProfile. The batch and candidate
/// semantic owners authenticate their content domains; [`GeneralCandidateV1::submit`]
/// is then the sole owner of window, solver, row, reward, and compartment
/// economics. The supplied submission must be byte-canonical and exactly equal
/// to that semantic result.
///
/// The work amount is derived from the semantic owner's exact compartment
/// capacities, never accepted from an observed or caller-stated balance. State
/// Rent remains the lifecycle policy's separately authenticated principal. The
/// Effect requires the final state balance to equal their checked sum, so
/// neither an underfunded nor overfunded submission is representable.
///
/// All decoding, joins, and arithmetic complete before the first write, so any
/// refusal preserves the caller-owned register bank byte-for-byte.
#[allow(clippy::too_many_arguments)]
pub fn project_general_submit_candidate_in_place_v3(
    root_tail: &[u8],
    batch_body: &[u8],
    config: GeneralConfigV3,
    candidate_body: &[u8],
    submission_body: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    requested_candidate_id: Option<[u8; 32]>,
    candidate: &mut [u8],
) -> Result<()> {
    if candidate.len() != general_hot_candidate_bank_len_v3(Action::SubmitCandidate, outcome_count)?
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    let root =
        GeneralRootV2::decode(root_tail).map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let batch =
        GeneralBatchV1::decode(batch_body).map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let candidate_record =
        CandidateV2::decode(candidate_body).map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let submitted = GeneralCandidateV1::decode(submission_body)
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let batch_opening = batch.opening();
    let batch_state = batch.state();
    let candidate_header = candidate_record.header();
    let submitted_opening = submitted.opening();
    let submitted_state = submitted.state();
    let current_slot = read_scalar(candidate, scalar::CURRENT_SLOT)?;
    let scalar_count = general_hot_scalar_count_v3(Action::SubmitCandidate, outcome_count)?;
    let settlement_duration = config
        .selection_slots()
        .checked_add(config.settlement_slots())
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    let expected_settlement_close = batch_opening
        .collection_close_slot
        .checked_add(settlement_duration)
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    let work_capacity = submitted_opening
        .work_capacity()
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    let expected = GeneralCandidateV1::submit(
        batch,
        candidate_record,
        submitted_opening.page_revision,
        submitted_opening.row_count,
        submitted_opening.reward_rate_lamports,
        submitted_opening.solver_id,
        work_capacity,
        current_slot,
    )
    .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let state_lamports = read_scalar(candidate, scalar::PRIMARY_RENT_PRINCIPAL)?
        .checked_add(work_capacity)
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    for item in 0..outcome_count {
        let base = GENERAL_HOT_COMMON_SCALARS_V3
            .checked_add(
                item.checked_mul(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            )
            .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
        if read_scalar(
            candidate,
            base.checked_add(item_scalar::OUTCOME)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
        )? != u64::from(item)
        {
            return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
        }
    }

    if expected.to_bytes().as_slice() != submission_body
        || requested_candidate_id != Some(candidate_header.candidate_id)
        || outcome_count != candidate_header.outcome_count
        || environment.general_root == [0; 32]
        || environment.trading_program == [0; 32]
        || root.lifecycle() != GeneralLifecycleV2::Active
        || root.market() != environment.market
        || root.config_id() != environment.general_config_id
        || root.generation() != environment.generation
        || config.generation() != environment.generation
        || batch_state.status != BatchStatusV1::Closed
        || batch_opening.market != root.market()
        || batch_opening.config_id != root.config_id()
        || batch_opening.generation != root.generation()
        || batch_opening.product_id != environment.product_record_digest
        || batch_opening.outcome_count != outcome_count
        || batch_opening.price_scale != config.price_scale()
        || batch_opening.max_orders != config.max_orders_per_candidate()
        || batch_opening.settlement_close_slot != expected_settlement_close
        || read_scalar(candidate, scalar::OUTCOME_COUNT)? != u64::from(outcome_count)
        || read_scalar(candidate, scalar::ZERO)? != u64::from(candidate_header.outcome_count)
        || read_scalar(candidate, scalar::ROOT_LIFECYCLE_OBSERVATION)?
            != u64::from(GeneralLifecycleV2::Active.tag())
        || read_scalar(candidate, scalar::BATCH_STATUS_OBSERVATION)?
            != u64::from(batch_state.status.tag())
        || read_scalar(candidate, scalar::BATCH_POST_ORDER_COUNT)?
            != u64::from(batch_opening.outcome_count)
        || read_scalar(candidate, scalar::BATCH_COLLECTION_CLOSE_SLOT)?
            != batch_opening.collection_close_slot
        || read_scalar(candidate, scalar::BATCH_SETTLEMENT_CLOSE_SLOT)?
            != batch_opening.settlement_close_slot
        || read_scalar(candidate, scalar::ORDER_MAX_LOTS)? != batch_opening.price_scale
        || read_scalar(candidate, scalar::CANDIDATE_PAGE_COUNT)?
            != u64::from(candidate_header.page_count)
        || read_scalar(candidate, scalar::SELECTION_BEST_CANDIDATE_COORDINATE)?
            != u64::from(candidate_header.candidate_coordinate)
        || read_scalar(candidate, scalar::SELECTION_PRICE_SCALE)? != candidate_header.price_scale
        || read_scalar(candidate, scalar::VERIFY_POST_ORDER_COUNT)?
            != u64::from(submitted_opening.outcome_count)
        || read_scalar(candidate, scalar::VERIFY_POST_PAGE)?
            != u64::from(submitted_opening.page_count)
        || read_scalar(candidate, scalar::CANDIDATE_STATUS_OBSERVATION)?
            != u64::from(submitted_state.status.tag())
        || read_scalar(candidate, scalar::CANDIDATE_PAGE_REVISION)?
            != submitted_opening.page_revision
        || read_scalar(candidate, scalar::CANDIDATE_SUBMITTED_SLOT)?
            != submitted_opening.submitted_slot
        || read_scalar(candidate, scalar::CANDIDATE_ROW_COUNT)?
            != u64::from(submitted_opening.row_count)
        || read_scalar(candidate, scalar::CANDIDATE_REWARD_RATE)?
            != submitted_opening.reward_rate_lamports
        || read_scalar(
            candidate,
            scalar::CANDIDATE_VERIFICATION_REMAINING_OBSERVATION,
        )? != submitted_state.verification_remaining
        || read_scalar(candidate, scalar::CANDIDATE_CLEANUP_REMAINING_OBSERVATION)?
            != submitted_state.cleanup_remaining
        || read_scalar(candidate, scalar::PRIMARY_BUMP_OBSERVATION)? != 0
        || read_scalar(candidate, scalar::PRIMARY_PRINCIPAL_OBSERVATION)? != 0
        || read_scalar(candidate, scalar::PRIMARY_CREATED)? != 1
        || read_scalar(candidate, scalar::STATE_BUMP)?
            != read_scalar(candidate, scalar::PRIMARY_CANONICAL_BUMP)?
        || read_scalar(candidate, scalar::PRIMARY_RENT_PRINCIPAL)? == 0
        || read_identity(
            candidate,
            scalar_count,
            identity::PRIMARY_BENEFICIARY_OBSERVATION,
        )? != [0; 32]
        || read_identity(candidate, scalar_count, identity::PRIMARY_BENEFICIARY)?
            != submitted_opening.solver_id
        || read_identity(candidate, scalar_count, identity::PRIMARY_OWNER)?
            != environment.trading_program
        || read_identity(candidate, scalar_count, identity::TRADING_PROGRAM)?
            != environment.trading_program
        || read_identity(candidate, scalar_count, identity::GENERAL_ROOT)?
            != environment.general_root
        || read_identity(candidate, scalar_count, identity::CANDIDATE)?
            != candidate_header.candidate_id
        || read_identity(candidate, scalar_count, identity::BEST_VERIFIED_DIGEST)?
            != candidate_header.candidate_id
        || read_identity(candidate, scalar_count, identity::ORDER)? != candidate_header.product_id
        || read_identity(candidate, scalar_count, identity::SELECTION_POLICY)?
            != candidate_header.batch_id
        || read_identity(candidate, scalar_count, identity::SELECTION_PRODUCT)?
            != batch_opening.product_id
        || read_identity(
            candidate,
            scalar_count,
            identity::RESULT_BENEFICIARY_OBSERVATION,
        )? != submitted_opening.candidate_id
        || read_identity(candidate, scalar_count, identity::BENEFICIARY)?
            != submitted_opening.batch_id
        || read_identity(candidate, scalar_count, identity::OWNER)? != submitted_opening.solver_id
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }

    write_local_state_constants(candidate, GeneralLocalStateKindV3::Candidate)?;
    for (coordinate, value) in [
        (scalar::ACTION, u64::from(Action::SubmitCandidate as u8)),
        (
            scalar::ROOT_LIFECYCLE_ACTIVE,
            u64::from(GeneralLifecycleV2::Active.tag()),
        ),
        (scalar::ONE, u64::from(GeneralCandidateLayoutV1::VERSION)),
        (
            scalar::CANDIDATE_POST_STATUS,
            u64::from(submitted_state.status.tag()),
        ),
        (
            scalar::CANDIDATE_POST_VERIFICATION_REMAINING,
            submitted_state.verification_remaining,
        ),
        (
            scalar::CANDIDATE_POST_CLEANUP_REMAINING,
            submitted_state.cleanup_remaining,
        ),
        (scalar::SCRATCH_A, work_capacity),
        (scalar::SCRATCH_B, state_lamports),
        (
            scalar::VERIFY_REVISION_OBSERVATION,
            u64::from_le_bytes(GeneralCandidateLayoutV1::MAGIC),
        ),
        (
            scalar::VERIFY_POST_REVISION,
            u64::from(GeneralCandidateLayoutV1::PHASE),
        ),
    ] {
        write_scalar(candidate, coordinate, value)?;
    }
    write_identity(
        candidate,
        scalar_count,
        identity::SELECTION_BATCH,
        batch.batch_id(),
    )?;
    Ok(())
}

/// Execute and project one maker-authorized order admission.
///
/// The AccountProfile projects every immutable signed-term byte, including
/// the interleaved runtime-width rows, into `candidate`. This function borrows
/// and hostile-decodes the exact signed immutable preimage, then compares every
/// projected header and row coordinate against it without allocating a
/// runtime-width order copy. Claims and Custody remain the physical balance
/// authorities: they prove the maker owns every exact reserve and atomically
/// move those atoms into order-keyed escrow.
///
/// The root and batch are independently decoded and joined back to the config,
/// Product, signer, request subject, and authenticated child addressing before
/// any candidate register is changed. Trading alone creates the order account,
/// advances the batch counters, and invokes the escrow routes.
#[allow(clippy::too_many_arguments)]
pub fn project_general_place_order_candidate_in_place_v3(
    root_tail: &[u8],
    batch_body: &[u8],
    config: GeneralConfigV3,
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    requested_order_id: Option<[u8; 32]>,
    signed_order_terms: &[u8],
    candidate: &mut [u8],
) -> Result<()> {
    if candidate.len() != general_hot_candidate_bank_len_v3(Action::PlaceOrder, outcome_count)? {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    let root =
        GeneralRootV2::decode(root_tail).map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let mut batch =
        GeneralBatchV1::decode(batch_body).map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let opening = batch.opening();
    let state = batch.state();
    let scalar_count = general_hot_scalar_count_v3(Action::PlaceOrder, outcome_count)?;
    let current_slot = read_scalar(candidate, scalar::CURRENT_SLOT)?;
    let owner = read_identity(candidate, scalar_count, identity::OWNER)?;
    let order_id = read_identity(candidate, scalar_count, identity::ORDER)?;
    let batch_id = read_identity(candidate, scalar_count, identity::SELECTION_BATCH)?;
    let product_id = read_identity(candidate, scalar_count, identity::SELECTION_PRODUCT)?;
    let max_lots = read_scalar(candidate, scalar::ORDER_MAX_LOTS)?;
    let max_quote_debit_per_lot = read_scalar(candidate, scalar::ORDER_MAX_QUOTE_DEBIT_PER_LOT)?;
    let valid_until_slot = read_scalar(candidate, scalar::ORDER_VALID_UNTIL_SLOT)?;
    let terms = GeneralSignedOrderTermsV1::decode(signed_order_terms)
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let header = terms.header();
    if requested_order_id != Some(terms.order_id())
        || order_id != terms.order_id()
        || header.outcome_count != outcome_count
        || header.nonce != read_scalar(candidate, scalar::ORDER_NONCE)?
        || header.owner_id != owner
        || header.market != root.market()
        || header.batch_id != batch_id
        || header.generation != read_scalar(candidate, scalar::GENERATION)?
        || header.max_lots != max_lots
        || header.max_quote_debit_per_lot != max_quote_debit_per_lot
        || header.valid_until_slot != valid_until_slot
        || owner == [0; 32]
        || product_id == [0; 32]
        || batch_id != batch.batch_id()
        || read_identity(candidate, scalar_count, identity::CANDIDATE)? != batch_id
        || environment.general_config_id == [0; 32]
        || root.lifecycle() != GeneralLifecycleV2::Active
        || root.market() != environment.market
        || root.config_id() != environment.general_config_id
        || root.generation() != environment.generation
        || read_scalar(candidate, scalar::OUTCOME_COUNT)? != u64::from(outcome_count)
        || read_scalar(candidate, scalar::ZERO)? != u64::from(outcome_count)
        || read_scalar(candidate, scalar::SCRATCH_A)? != u64::from(outcome_count)
        || read_scalar(candidate, scalar::ROOT_LIFECYCLE_OBSERVATION)?
            != u64::from(GeneralLifecycleV2::Active.tag())
        || read_scalar(candidate, scalar::BATCH_STATUS_OBSERVATION)?
            != u64::from(state.status.tag())
        || read_scalar(candidate, scalar::BATCH_ORDER_COUNT_OBSERVATION)?
            != u64::from(state.order_count)
        || read_scalar(candidate, scalar::BATCH_QUOTE_RESERVE_OBSERVATION)?
            != state.committed_quote_reserve
        || read_scalar(candidate, scalar::BATCH_COLLECTION_CLOSE_SLOT)?
            != opening.collection_close_slot
        || read_scalar(candidate, scalar::BATCH_SETTLEMENT_CLOSE_SLOT)?
            != opening.settlement_close_slot
        || read_scalar(candidate, scalar::CONFIG_MAX_ORDERS)? != u64::from(opening.max_orders)
        || valid_until_slot != opening.settlement_close_slot
        || opening.outcome_count != outcome_count
        || opening.market != root.market()
        || opening.product_id != product_id
        || opening.config_id != root.config_id()
        || opening.generation != root.generation()
        || opening.price_scale != config.price_scale()
        || opening.max_orders != config.max_orders_per_candidate()
        || read_identity(candidate, scalar_count, identity::MARKET)? != root.market()
        || read_identity(candidate, scalar_count, identity::GENERAL_CONFIG_ID)? != root.config_id()
        || read_identity(
            candidate,
            scalar_count,
            identity::TERMINAL_BENEFICIARY_OBSERVATION,
        )? != owner
        || read_scalar(candidate, scalar::STATE_BUMP)?
            != read_scalar(candidate, scalar::PRIMARY_CANONICAL_BUMP)?
        || read_identity(candidate, scalar_count, identity::PRIMARY_OWNER)?
            != read_identity(candidate, scalar_count, identity::TRADING_PROGRAM)?
        || read_scalar(candidate, scalar::PRIMARY_RENT_PRINCIPAL)? == 0
        || read_scalar(candidate, scalar::TERMINAL_RECORD_BUMP)?
            != read_scalar(candidate, scalar::TERMINAL_CANONICAL_BUMP)?
        || read_identity(candidate, scalar_count, identity::TERMINAL_OWNER)?
            != read_identity(candidate, scalar_count, identity::TRADING_PROGRAM)?
        || read_scalar(candidate, scalar::TERMINAL_RENT_PRINCIPAL)? == 0
        || environment.destination_vault_context != terms.order_id()
        || environment.custody_source_owner != owner
        || read_identity(candidate, scalar_count, identity::POSITION_ZERO_OWNER)? != owner
        || read_identity(candidate, scalar_count, identity::POSITION_ONE_OWNER)? != terms.order_id()
        || environment.settlement_position_owner != terms.order_id()
        || environment.rent_credit != owner
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    for item in 0..outcome_count {
        let base = GENERAL_HOT_COMMON_SCALARS_V3
            .checked_add(
                item.checked_mul(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            )
            .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
        if read_scalar(
            candidate,
            base.checked_add(item_scalar::CURSOR_INVENTORY)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
        )? != terms
            .receive_per_lot(item)
            .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?
            || read_scalar(
                candidate,
                base.checked_add(item_scalar::QUANTITY)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            )? != terms
                .deliver_per_lot(item)
                .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?
        {
            return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
        }
    }
    let quote_reserve = terms
        .quote_reserve()
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let escrow = batch
        .admit_signed_for_atomic_physical_escrow(terms, current_slot)
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let state = batch.state();
    if escrow.order_id != terms.order_id()
        || escrow.owner_id != owner
        || escrow.outcome_count != outcome_count
        || escrow.quote_atoms != quote_reserve
        || escrow.direction != EscrowDirectionV1::Deposit
    {
        return Err(GeneralHotCandidateErrorV3::InvalidPlan);
    }
    write_local_state_constants(candidate, GeneralLocalStateKindV3::Order)?;
    for (coordinate, value) in [
        (scalar::ACTION, u64::from(Action::PlaceOrder as u8)),
        (
            scalar::ROOT_LIFECYCLE_ACTIVE,
            u64::from(GeneralLifecycleV2::Active.tag()),
        ),
        (scalar::ONE, 1),
        (scalar::BATCH_POST_ORDER_COUNT, u64::from(state.order_count)),
        (scalar::ORDER_QUOTE_RESERVE, quote_reserve),
        (scalar::CUSTODY_AMOUNT, quote_reserve),
        (
            scalar::BATCH_POST_QUOTE_RESERVE,
            state.committed_quote_reserve,
        ),
        (
            scalar::ORDER_POST_PHASE,
            u64::from(GeneralOrderPhaseV1::Placed.tag()),
        ),
        (
            scalar::SCRATCH_B,
            u64::from(GeneralOrderLayoutV1::phase_value()),
        ),
        (scalar::SCRATCH_A, GeneralOrderLayoutV1::magic_u64()),
        (scalar::CUSTODY_ACTIVE, u64::from(quote_reserve != 0)),
        (
            scalar::CLAIMS_POST_MARKET_REVISION,
            environment
                .claims_market_revision
                .checked_add(1)
                .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?,
        ),
        (scalar::POSITION_ONE_REVISION, 0),
        (scalar::CLAIMS_SOURCE_PRESENT, 1),
        (scalar::CLAIMS_DESTINATION_PRESENT, 1),
        (scalar::CLAIMS_SOURCE_POSITION_INDEX, 0),
        (scalar::CLAIMS_DESTINATION_POSITION_INDEX, 1),
        (
            scalar::CLAIMS_AGGREGATE_DIRECTION,
            DeltaDirectionV2::Neutral as u64,
        ),
        (
            scalar::CLAIMS_SOURCE_DIRECTION,
            DeltaDirectionV2::Debit as u64,
        ),
        (
            scalar::CLAIMS_DESTINATION_DIRECTION,
            DeltaDirectionV2::Credit as u64,
        ),
        (scalar::CUSTODY_OPERATION, OperationV1::Transfer as u64),
    ] {
        write_scalar(candidate, coordinate, value)?;
    }
    for item in 0..outcome_count {
        let base = GENERAL_HOT_COMMON_SCALARS_V3
            .checked_add(
                item.checked_mul(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            )
            .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
        let quantity = terms
            .claim_reserve(item)
            .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
        for (coordinate, value) in [
            (item_scalar::OUTCOME, u64::from(item)),
            (item_scalar::CLAIMS_AGGREGATE_MAGNITUDE, 0),
            (item_scalar::CLAIMS_SOURCE_MAGNITUDE, quantity),
            (item_scalar::CLAIMS_DESTINATION_MAGNITUDE, quantity),
        ] {
            write_scalar(
                candidate,
                base.checked_add(coordinate)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
                value,
            )?;
        }
    }
    Ok(())
}

/// Execute and project one maker-authorized order cancellation and full escrow refund.
///
/// The persisted order is the semantic owner of its immutable reserve and
/// mutable phase. The batch transition removes exactly that reserve and the
/// projected Claims rows are recomputed from the order, rather than accepted
/// from the caller. Claims/Custody remain the physical balance authorities and
/// their close routes make an understated or overstated refund impossible.
#[allow(clippy::too_many_arguments)]
pub fn project_general_cancel_order_candidate_in_place_v3(
    root_tail: &[u8],
    batch_body: &[u8],
    order_body: &[u8],
    config: GeneralConfigV3,
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    requested_order_id: Option<[u8; 32]>,
    candidate: &mut [u8],
) -> Result<()> {
    if candidate.len() != general_hot_candidate_bank_len_v3(Action::CancelOrder, outcome_count)? {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    let root =
        GeneralRootV2::decode(root_tail).map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let mut batch =
        GeneralBatchV1::decode(batch_body).map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let order =
        GeneralOrderV1::decode(order_body).map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let opening = batch.opening();
    let before = batch.state();
    let header = order.header();
    let order_state = order.state();
    let scalar_count = general_hot_scalar_count_v3(Action::CancelOrder, outcome_count)?;
    let current_slot = read_scalar(candidate, scalar::CURRENT_SLOT)?;
    let owner = read_identity(candidate, scalar_count, identity::OWNER)?;
    if requested_order_id != Some(order.order_id())
        || read_identity(candidate, scalar_count, identity::ORDER)? != order.order_id()
        || owner != header.owner_id
        || owner == [0; 32]
        || read_identity(candidate, scalar_count, identity::SELECTION_BATCH)? != header.batch_id
        || read_identity(candidate, scalar_count, identity::CANDIDATE)? != header.batch_id
        || environment.general_config_id == [0; 32]
        || root.lifecycle() != GeneralLifecycleV2::Active
        || root.market() != environment.market
        || root.config_id() != environment.general_config_id
        || root.generation() != environment.generation
        || read_scalar(candidate, scalar::OUTCOME_COUNT)? != u64::from(outcome_count)
        || read_scalar(candidate, scalar::ZERO)? != u64::from(outcome_count)
        || read_scalar(candidate, scalar::SCRATCH_A)? != u64::from(outcome_count)
        || read_scalar(candidate, scalar::ROOT_LIFECYCLE_OBSERVATION)?
            != u64::from(GeneralLifecycleV2::Active.tag())
        || read_scalar(candidate, scalar::BATCH_STATUS_OBSERVATION)?
            != u64::from(before.status.tag())
        || read_scalar(candidate, scalar::BATCH_ORDER_COUNT_OBSERVATION)?
            != u64::from(before.order_count)
        || read_scalar(candidate, scalar::BATCH_CANCELLED_COUNT_OBSERVATION)?
            != u64::from(before.cancelled_count)
        || read_scalar(candidate, scalar::BATCH_QUOTE_RESERVE_OBSERVATION)?
            != before.committed_quote_reserve
        || read_scalar(candidate, scalar::BATCH_COLLECTION_CLOSE_SLOT)?
            != opening.collection_close_slot
        || read_scalar(candidate, scalar::ORDER_PHASE_OBSERVATION)?
            != u64::from(order_state.phase.tag())
        || read_scalar(candidate, scalar::ORDER_ADMITTED_SLOT_OBSERVATION)?
            != order_state.admitted_slot
        || read_scalar(candidate, scalar::ORDER_MAX_LOTS)? != header.max_lots
        || read_scalar(candidate, scalar::ORDER_MAX_QUOTE_DEBIT_PER_LOT)?
            != header.max_quote_debit_per_lot
        || read_scalar(candidate, scalar::ORDER_NONCE)? != header.nonce
        || opening.outcome_count != outcome_count
        || opening.market != root.market()
        || opening.product_id
            != read_identity(candidate, scalar_count, identity::SELECTION_PRODUCT)?
        || opening.config_id != root.config_id()
        || opening.generation != root.generation()
        || opening.price_scale != config.price_scale()
        || opening.max_orders != config.max_orders_per_candidate()
        || header.outcome_count != outcome_count
        || header.market != root.market()
        || header.generation != root.generation()
        || read_identity(candidate, scalar_count, identity::MARKET)? != root.market()
        || read_identity(candidate, scalar_count, identity::GENERAL_CONFIG_ID)? != root.config_id()
        || read_scalar(candidate, scalar::STATE_BUMP)?
            != read_scalar(candidate, scalar::PRIMARY_CANONICAL_BUMP)?
        || read_identity(candidate, scalar_count, identity::PRIMARY_OWNER)?
            != read_identity(candidate, scalar_count, identity::TRADING_PROGRAM)?
        || read_scalar(candidate, scalar::PRIMARY_RENT_PRINCIPAL)? == 0
        || read_scalar(candidate, scalar::TERMINAL_RECORD_BUMP)?
            != read_scalar(candidate, scalar::TERMINAL_CANONICAL_BUMP)?
        || read_identity(candidate, scalar_count, identity::TERMINAL_OWNER)?
            != read_identity(candidate, scalar_count, identity::TRADING_PROGRAM)?
        || read_scalar(candidate, scalar::TERMINAL_RENT_PRINCIPAL)? == 0
        || environment.source_vault_context != order.order_id()
        || environment.custody_destination_owner != owner
        || read_identity(candidate, scalar_count, identity::POSITION_ZERO_OWNER)?
            != order.order_id()
        || read_identity(candidate, scalar_count, identity::POSITION_ONE_OWNER)? != owner
        || environment.settlement_position_owner != order.order_id()
        || environment.rent_credit != owner
        || environment.rent_refund != owner
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    let escrow = batch
        .cancel(order, owner, current_slot)
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let after = batch.state();
    if escrow.order_id != order.order_id()
        || escrow.owner_id != owner
        || escrow.outcome_count != outcome_count
        || escrow.direction != EscrowDirectionV1::Refund
    {
        return Err(GeneralHotCandidateErrorV3::InvalidPlan);
    }
    let custody_resulting = environment
        .custody_expected_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let close_vault_expected = custody_resulting
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let close_vault_resulting = close_vault_expected
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let close_replay_resulting = close_vault_resulting
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let position_zero_revision = read_scalar(candidate, scalar::POSITION_ZERO_REVISION)?;
    let settlement_position_revision = position_zero_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let settlement_post_position_revision = settlement_position_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let mut claims_active = false;
    for item in 0..outcome_count {
        claims_active |= order
            .claim_reserve(item)
            .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?
            != 0;
    }
    write_local_state_constants(candidate, GeneralLocalStateKindV3::Batch)?;
    for (coordinate, value) in [
        (scalar::ACTION, u64::from(Action::CancelOrder as u8)),
        (
            scalar::ROOT_LIFECYCLE_ACTIVE,
            u64::from(GeneralLifecycleV2::Active.tag()),
        ),
        (
            scalar::SCRATCH_B,
            u64::from(BatchStatusV1::Collecting.tag()),
        ),
        (
            scalar::ORDER_POST_PHASE,
            u64::from(GeneralOrderPhaseV1::Cancelled.tag()),
        ),
        (scalar::ORDER_POST_RELEASED_SLOT, current_slot),
        (scalar::ORDER_QUOTE_RESERVE, escrow.quote_atoms),
        (scalar::CUSTODY_AMOUNT, escrow.quote_atoms),
        (
            scalar::BATCH_POST_QUOTE_RESERVE,
            after.committed_quote_reserve,
        ),
        (
            scalar::BATCH_POST_CANCELLED_COUNT,
            u64::from(after.cancelled_count),
        ),
        (scalar::CUSTODY_RESULTING_REVISION, custody_resulting),
        (
            scalar::CUSTODY_CLOSE_VAULT_EXPECTED_REVISION,
            close_vault_expected,
        ),
        (
            scalar::CUSTODY_CLOSE_VAULT_RESULTING_REVISION,
            close_vault_resulting,
        ),
        (
            scalar::CUSTODY_CLOSE_REPLAY_RESULTING_REVISION,
            close_replay_resulting,
        ),
        (scalar::CUSTODY_OPERATION, OperationV1::Transfer as u64),
        (
            scalar::CLAIMS_POST_MARKET_REVISION,
            environment
                .claims_market_revision
                .checked_add(1)
                .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?,
        ),
        (
            scalar::SETTLEMENT_POSITION_REVISION,
            settlement_position_revision,
        ),
        (
            scalar::SETTLEMENT_POST_POSITION_REVISION,
            settlement_post_position_revision,
        ),
        (scalar::CLAIMS_SOURCE_PRESENT, 1),
        (scalar::CLAIMS_DESTINATION_PRESENT, 1),
        (scalar::CLAIMS_SOURCE_POSITION_INDEX, 0),
        (scalar::CLAIMS_DESTINATION_POSITION_INDEX, 1),
        (
            scalar::CLAIMS_AGGREGATE_DIRECTION,
            DeltaDirectionV2::Neutral as u64,
        ),
        (
            scalar::CLAIMS_SOURCE_DIRECTION,
            DeltaDirectionV2::Debit as u64,
        ),
        (
            scalar::CLAIMS_DESTINATION_DIRECTION,
            DeltaDirectionV2::Credit as u64,
        ),
        (scalar::CLAIMS_AFFINE_ACTIVE, u64::from(claims_active)),
        (scalar::CUSTODY_ACTIVE, u64::from(escrow.quote_atoms != 0)),
    ] {
        write_scalar(candidate, coordinate, value)?;
    }
    for item in 0..outcome_count {
        let base = GENERAL_HOT_COMMON_SCALARS_V3
            .checked_add(
                item.checked_mul(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            )
            .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
        let quantity = order
            .claim_reserve(item)
            .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
        for (coordinate, value) in [
            (item_scalar::OUTCOME, u64::from(item)),
            (item_scalar::QUANTITY, quantity),
            (item_scalar::CLAIMS_AGGREGATE_MAGNITUDE, 0),
            (item_scalar::CLAIMS_SOURCE_MAGNITUDE, quantity),
            (item_scalar::CLAIMS_DESTINATION_MAGNITUDE, quantity),
        ] {
            write_scalar(
                candidate,
                base.checked_add(coordinate)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
                value,
            )?;
        }
    }
    Ok(())
}

/// Execute and project one permissionless post-window residual order release.
///
/// The immutable order validity horizon is the physical window authority; the
/// PlaceOrder transition pins it exactly to the batch settlement close. Quote
/// and claim residuals remain observations of the order-keyed physical escrow,
/// never recomputed promises: transfer overstatements fail at the balance
/// owner, understatements leave a nonempty account and fail its ordered close.
#[allow(clippy::too_many_arguments)]
pub fn project_general_release_order_candidate_in_place_v3(
    root_tail: &[u8],
    order_body: &[u8],
    config: GeneralConfigV3,
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    requested_order_id: Option<[u8; 32]>,
    candidate: &mut [u8],
) -> Result<()> {
    if candidate.len() != general_hot_candidate_bank_len_v3(Action::ReleaseOrder, outcome_count)? {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    let root =
        GeneralRootV2::decode(root_tail).map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let order =
        GeneralOrderV1::decode(order_body).map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let header = order.header();
    let state = order.state();
    let scalar_count = general_hot_scalar_count_v3(Action::ReleaseOrder, outcome_count)?;
    let current_slot = read_scalar(candidate, scalar::CURRENT_SLOT)?;
    let owner = read_identity(candidate, scalar_count, identity::OWNER)?;
    let escrow = authenticate_order_residual_release_v1(order, current_slot)
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let quote_reserve = order
        .quote_reserve()
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let observed_quote = read_scalar(candidate, scalar::ESCROW_BALANCE_OBSERVATION)?;
    if requested_order_id != Some(order.order_id())
        || read_identity(candidate, scalar_count, identity::ORDER)? != order.order_id()
        || owner != header.owner_id
        || owner == [0; 32]
        || read_identity(candidate, scalar_count, identity::CANDIDATE)? != header.batch_id
        || environment.general_config_id == [0; 32]
        || root.lifecycle() != GeneralLifecycleV2::Active
        || root.market() != environment.market
        || root.config_id() != environment.general_config_id
        || root.generation() != environment.generation
        || read_scalar(candidate, scalar::OUTCOME_COUNT)? != u64::from(outcome_count)
        || read_scalar(candidate, scalar::ZERO)? != u64::from(outcome_count)
        || read_scalar(candidate, scalar::ROOT_LIFECYCLE_OBSERVATION)?
            != u64::from(GeneralLifecycleV2::Active.tag())
        || read_scalar(candidate, scalar::ORDER_PHASE_OBSERVATION)? != u64::from(state.phase.tag())
        || read_scalar(candidate, scalar::ORDER_ADMITTED_SLOT_OBSERVATION)? != state.admitted_slot
        || read_scalar(candidate, scalar::ORDER_VALID_UNTIL_SLOT)? != header.valid_until_slot
        || read_scalar(candidate, scalar::ORDER_MAX_LOTS)? != header.max_lots
        || read_scalar(candidate, scalar::ORDER_MAX_QUOTE_DEBIT_PER_LOT)?
            != header.max_quote_debit_per_lot
        || read_scalar(candidate, scalar::ORDER_NONCE)? != header.nonce
        || observed_quote > quote_reserve
        || header.outcome_count != outcome_count
        || header.market != root.market()
        || header.generation != root.generation()
        || config.generation() != root.generation()
        || read_identity(candidate, scalar_count, identity::MARKET)? != root.market()
        || read_scalar(candidate, scalar::GENERATION)? != root.generation()
        || read_scalar(candidate, scalar::STATE_BUMP)?
            != read_scalar(candidate, scalar::PRIMARY_CANONICAL_BUMP)?
        || read_identity(candidate, scalar_count, identity::PRIMARY_OWNER)?
            != read_identity(candidate, scalar_count, identity::TRADING_PROGRAM)?
        || read_scalar(candidate, scalar::PRIMARY_RENT_PRINCIPAL)? == 0
        || environment.source_vault_context != order.order_id()
        || environment.custody_destination_owner != owner
        || read_identity(candidate, scalar_count, identity::POSITION_ZERO_OWNER)?
            != order.order_id()
        || read_identity(candidate, scalar_count, identity::POSITION_ONE_OWNER)? != owner
        || environment.settlement_position_owner != order.order_id()
        || environment.rent_credit != owner
        || environment.rent_refund != owner
        || escrow.order_id != order.order_id()
        || escrow.owner_id != owner
        || escrow.outcome_count != outcome_count
        || escrow.quote_atoms != 0
        || escrow.direction != EscrowDirectionV1::Residual
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    let custody_resulting = environment
        .custody_expected_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let close_vault_expected = custody_resulting
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let close_vault_resulting = close_vault_expected
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let close_replay_resulting = close_vault_resulting
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let position_zero_revision = read_scalar(candidate, scalar::POSITION_ZERO_REVISION)?;
    let settlement_position_revision = position_zero_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let settlement_post_position_revision = settlement_position_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let mut claims_active = false;
    for item in 0..outcome_count {
        let base = GENERAL_HOT_COMMON_SCALARS_V3
            .checked_add(
                item.checked_mul(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            )
            .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
        claims_active |= read_scalar(
            candidate,
            base.checked_add(item_scalar::QUANTITY)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
        )? != 0;
    }
    write_local_state_constants(candidate, GeneralLocalStateKindV3::Order)?;
    for (coordinate, value) in [
        (scalar::ACTION, u64::from(Action::ReleaseOrder as u8)),
        (
            scalar::ROOT_LIFECYCLE_ACTIVE,
            u64::from(GeneralLifecycleV2::Active.tag()),
        ),
        (
            scalar::SCRATCH_A,
            u64::from(GeneralOrderPhaseV1::Placed.tag()),
        ),
        (
            scalar::ORDER_POST_PHASE,
            u64::from(GeneralOrderPhaseV1::Released.tag()),
        ),
        (scalar::ORDER_POST_RELEASED_SLOT, current_slot),
        (scalar::ORDER_QUOTE_RESERVE, quote_reserve),
        (scalar::CUSTODY_AMOUNT, observed_quote),
        (scalar::CUSTODY_RESULTING_REVISION, custody_resulting),
        (
            scalar::CUSTODY_CLOSE_VAULT_EXPECTED_REVISION,
            close_vault_expected,
        ),
        (
            scalar::CUSTODY_CLOSE_VAULT_RESULTING_REVISION,
            close_vault_resulting,
        ),
        (
            scalar::CUSTODY_CLOSE_REPLAY_RESULTING_REVISION,
            close_replay_resulting,
        ),
        (scalar::CUSTODY_OPERATION, OperationV1::Transfer as u64),
        (
            scalar::CLAIMS_POST_MARKET_REVISION,
            environment
                .claims_market_revision
                .checked_add(1)
                .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?,
        ),
        (
            scalar::SETTLEMENT_POSITION_REVISION,
            settlement_position_revision,
        ),
        (
            scalar::SETTLEMENT_POST_POSITION_REVISION,
            settlement_post_position_revision,
        ),
        (scalar::CLAIMS_SOURCE_PRESENT, 1),
        (scalar::CLAIMS_DESTINATION_PRESENT, 1),
        (scalar::CLAIMS_SOURCE_POSITION_INDEX, 0),
        (scalar::CLAIMS_DESTINATION_POSITION_INDEX, 1),
        (
            scalar::CLAIMS_AGGREGATE_DIRECTION,
            DeltaDirectionV2::Neutral as u64,
        ),
        (
            scalar::CLAIMS_SOURCE_DIRECTION,
            DeltaDirectionV2::Debit as u64,
        ),
        (
            scalar::CLAIMS_DESTINATION_DIRECTION,
            DeltaDirectionV2::Credit as u64,
        ),
        (scalar::CLAIMS_AFFINE_ACTIVE, u64::from(claims_active)),
        (scalar::CUSTODY_ACTIVE, u64::from(observed_quote != 0)),
    ] {
        write_scalar(candidate, coordinate, value)?;
    }
    for item in 0..outcome_count {
        let base = GENERAL_HOT_COMMON_SCALARS_V3
            .checked_add(
                item.checked_mul(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            )
            .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
        let quantity = read_scalar(
            candidate,
            base.checked_add(item_scalar::QUANTITY)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
        )?;
        for (coordinate, value) in [
            (item_scalar::OUTCOME, u64::from(item)),
            (item_scalar::CLAIMS_AGGREGATE_MAGNITUDE, 0),
            (item_scalar::CLAIMS_SOURCE_MAGNITUDE, quantity),
            (item_scalar::CLAIMS_DESTINATION_MAGNITUDE, quantity),
        ] {
            write_scalar(
                candidate,
                base.checked_add(coordinate)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
                value,
            )?;
        }
    }
    Ok(())
}

/// Stable refusal from General Hot candidate projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralHotCandidateErrorV3 {
    /// Settlement effect bytes refused.
    InvalidPlan,
    /// Product width differed from the exact effect width.
    TailCountMismatch,
    /// Caller-owned banks were not one exact complete candidate width.
    InvalidCapacity,
    /// The bank is not the width THIS ACTION declares a bank to be.
    ///
    /// Split out of `InvalidCapacity` because the two accusations have
    /// different authors and different remedies. `InvalidCapacity` says a
    /// caller supplied a buffer of the wrong size; this says the buffer is a
    /// perfectly good bank for a DIFFERENT action, and the action it was
    /// handed with declares another stride.
    ///
    /// It is worth its own name because of where it is raised.
    /// `general_hot_environment_from_bank_v3` runs for every action before the
    /// accelerator dispatches on one, and the `scalar_count` it computes is the
    /// OFFSET at which the identity bank begins. A stride disagreement that
    /// reached the reads below would not fail -- it would read identities from
    /// the wrong offset and return a well-formed environment assembled from the
    /// wrong bytes. Fail-closed and named, not silently rebased.
    BankStrideMismatch,
    /// An authenticated child coordinate was zero, aliased, or noncanonical.
    InvalidCoordinate,
    /// Position or Custody optimistic revision could not advance.
    RevisionOverflow,
    /// Checked register or byte arithmetic overflowed.
    ArithmeticOverflow,
    /// The sole authenticated candidate-row verifier refused the supplied
    /// batch, candidate, page, order, cursor, or result prestate.
    Verify(GeneralCandidateErrorV1),
    /// The exact CloseCandidate request, censorship guard, candidate state, or
    /// conserved work-escrow movement refused.
    Close(GeneralSevenPlanErrorV1),
}

/// Result alias for General Hot candidate projection.
pub type Result<T> = core::result::Result<T, GeneralHotCandidateErrorV3>;

/// Authenticate one first-class CloseCandidate execution against its physical
/// Candidate account, joined closed Batch evidence, trusted slot, and exact
/// two-beneficiary lamport poststate.
///
/// This function does not write either a model record or an advisory amount.
/// [`plan_candidate_work_escrow_close_v1`] remains the sole semantic owner of
/// the censorship guard and the three-way conservation plan. The generic
/// Effect pays the cleanup compartment to the permissionless caller and the
/// verification remainder to the solver; Lifecycle V5 then returns only the
/// historical rent principal to that same solver and vacates the Candidate.
/// Every value supplied to that machinery is rejoined here to the hostile-
/// decoded Candidate and Batch before the admitted accelerator may accept.
pub fn authenticate_general_close_candidate_v3(
    family_request: &[u8],
    batch: GeneralBatchV1,
    submission: GeneralCandidateV1,
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    candidate: &[u8],
) -> Result<WorkEscrowClosePlanV1> {
    if candidate.len() != general_hot_candidate_bank_len_v3(Action::CloseCandidate, outcome_count)?
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    let action_plan = authenticate_general_seven_request_v1(family_request)
        .map_err(GeneralHotCandidateErrorV3::Close)?;
    let scalar_count = general_hot_scalar_count_v3(Action::CloseCandidate, outcome_count)?;
    let opening = submission.opening();
    let state = submission.state();
    let batch_opening = batch.opening();
    let batch_state = batch.state();
    let rent_principal = read_scalar(candidate, scalar::PRIMARY_RENT_PRINCIPAL)?;
    let verification = state.verification_remaining;
    let cleanup = state.cleanup_remaining;
    let solver_credit = verification
        .checked_add(rent_principal)
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;

    if outcome_count == 0
        || opening.outcome_count != outcome_count
        || batch_opening.outcome_count != outcome_count
        || opening.batch_id != batch.batch_id()
        || environment.general_root == [0; 32]
        || environment.trading_program == [0; 32]
        || batch_opening.market != environment.market
        || batch_opening.product_id != environment.product_record_digest
        || batch_opening.config_id != environment.general_config_id
        || batch_opening.generation != environment.generation
        || batch_state.status != BatchStatusV1::Closed
        || action_plan.subject_id() != opening.candidate_id
        || read_scalar(candidate, scalar::OUTCOME_COUNT)? != u64::from(outcome_count)
        || read_scalar(candidate, scalar::ROOT_LIFECYCLE_OBSERVATION)?
            != u64::from(GeneralLifecycleV2::Active.tag())
        || read_scalar(candidate, scalar::CANDIDATE_STATUS_OBSERVATION)?
            != u64::from(state.status.tag())
        || read_scalar(
            candidate,
            scalar::CANDIDATE_VERIFICATION_REMAINING_OBSERVATION,
        )? != verification
        || read_scalar(candidate, scalar::CANDIDATE_CLEANUP_REMAINING_OBSERVATION)? != cleanup
        || read_scalar(candidate, scalar::CANDIDATE_REWARD_RATE)? != opening.reward_rate_lamports
        || read_scalar(candidate, scalar::BATCH_SETTLEMENT_CLOSE_SLOT)?
            != batch_opening.settlement_close_slot
        || read_scalar(candidate, scalar::BATCH_STATUS_OBSERVATION)?
            != u64::from(batch_state.status.tag())
        || read_scalar(candidate, scalar::PRIMARY_PRINCIPAL_OBSERVATION)? != rent_principal
        || rent_principal == 0
        || read_identity(candidate, scalar_count, identity::PARENT_REQUEST_DIGEST)?
            != opening.candidate_id
        || read_identity(candidate, scalar_count, identity::CANDIDATE)? != opening.candidate_id
        || read_identity(candidate, scalar_count, identity::SELECTION_BATCH)? != opening.batch_id
        || read_identity(candidate, scalar_count, identity::OWNER)? != opening.solver_id
        || read_identity(candidate, scalar_count, identity::RENT_CREDIT)? != opening.solver_id
        || read_identity(
            candidate,
            scalar_count,
            identity::PRIMARY_BENEFICIARY_OBSERVATION,
        )? != opening.solver_id
        || read_identity(candidate, scalar_count, identity::PRIMARY_BENEFICIARY)?
            != opening.solver_id
        || read_identity(candidate, scalar_count, identity::PAYER)? == [0; 32]
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }

    let plan = plan_candidate_work_escrow_close_v1(
        action_plan,
        batch,
        submission,
        read_scalar(candidate, scalar::CURRENT_SLOT)?,
        WorkEscrowObservationV1 {
            escrow_lamports: read_scalar(candidate, scalar::OBSERVED_POSITION_LAMPORTS)?,
            rent_floor: rent_principal,
            beneficiary_lamports: read_scalar(candidate, scalar::OBSERVED_ADMISSION_LAMPORTS)?,
        },
        read_scalar(candidate, scalar::ESCROW_BALANCE_OBSERVATION)?,
    )
    .map_err(GeneralHotCandidateErrorV3::Close)?;
    if plan.escrow_before() != read_scalar(candidate, scalar::OBSERVED_POSITION_LAMPORTS)?
        || plan.cranker_before() != read_scalar(candidate, scalar::OBSERVED_ADMISSION_LAMPORTS)?
        || plan.solver_before() != read_scalar(candidate, scalar::ESCROW_BALANCE_OBSERVATION)?
        || plan.cleanup_reward() != cleanup
        || plan.solver_credit() != solver_credit
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    Ok(plan)
}

/// Exact chain-state result of one permissionless candidate-verification row.
///
/// This is the verifier's accepted summary, not a second calculation of row
/// progress or work escrow. The accelerator uses the three semantic output
/// buffers passed to [`project_general_verify_candidate_v3`] as its state-last
/// writes: Candidate always advances, Verifier always advances, and the raw
/// verified-candidate result is nonzero only for a terminal row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralVerifyCandidateProjectionV3 {
    /// Canonical verifier result returned by `candidate_v1`.
    pub summary: CandidateVerifyRowSummaryV1,
}

impl GeneralVerifyCandidateProjectionV3 {
    /// Whether the conditional raw verified-candidate state must be created
    /// and written after the generic transition/lifecycle join succeeds.
    #[must_use]
    pub const fn creates_verified_result(self) -> bool {
        self.summary.complete
    }
}

/// Run the only authenticated candidate-row verifier and project its exact
/// successor facts into one complete Hot candidate bank.
///
/// `candidate_v1` authenticates the closed batch, submitted Candidate,
/// immutable Candidate/Page, and escrowed order before it calls `runtime_verify`.
/// Its cursor, certificate, and manifest outputs are failure-atomic; this
/// wrapper leaves its bank output unchanged on every refusal. On success the
/// state-last physical writes are `summary.submission.to_bytes()`,
/// `cursor_output`, and `verified_output` iff `summary.complete`; the manifest
/// is readonly settlement evidence, never a caller-selected order list.
pub fn project_general_verify_candidate_v3<'a>(
    view: CandidateVerifyRowViewV1<'_>,
    verifier_buffers: CandidateVerifyRowBuffersV1<'_>,
    outcome_count: u32,
    authenticated_input: &[u8],
    scratch: &mut [u8],
    output: &'a mut [u8],
) -> Result<(ExecutionCandidateV2<'a>, GeneralVerifyCandidateProjectionV3)> {
    exact_candidate_capacities(
        Action::VerifyCandidateRow,
        outcome_count,
        authenticated_input,
        scratch,
        output,
    )?;
    let projection = project_general_verify_candidate_to_scratch_v3(
        view,
        verifier_buffers,
        outcome_count,
        authenticated_input,
        scratch,
    )?;
    output.copy_from_slice(scratch);
    Ok((ExecutionCandidateV2::Accepted(output), projection))
}

/// Run one authenticated candidate-verification row with one complete scratch
/// bank and commit it into the supplied candidate only after every semantic
/// check accepts.
///
/// This is the bounded-memory physical form used by the SBF accelerator. The
/// candidate remains byte-for-byte unchanged on every refusal; scratch and the
/// verifier output buffers are explicitly non-authoritative workspaces.
pub fn project_general_verify_candidate_in_place_v3(
    view: CandidateVerifyRowViewV1<'_>,
    verifier_buffers: CandidateVerifyRowBuffersV1<'_>,
    outcome_count: u32,
    candidate: &mut [u8],
    scratch: &mut [u8],
) -> Result<GeneralVerifyCandidateProjectionV3> {
    let required = general_hot_candidate_bank_len_v3(Action::VerifyCandidateRow, outcome_count)?;
    if candidate.len() != required || scratch.len() != required {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    let projection = project_general_verify_candidate_to_scratch_v3(
        view,
        verifier_buffers,
        outcome_count,
        candidate,
        scratch,
    )?;
    candidate.copy_from_slice(scratch);
    Ok(projection)
}

/// Run one authenticated candidate-verification row directly in the
/// accelerator's non-authoritative candidate workspace.
///
/// Unlike [`project_general_verify_candidate_in_place_v3`], this form does
/// not require a second complete bank. It is only suitable where the caller
/// discards the entire workspace on refusal and publishes no effect before
/// success, as the admitted-AOT accelerator does. The authenticated input join
/// and verifier are the same semantic owners used by the failure-atomic
/// wrappers above.
pub fn project_general_verify_candidate_workspace_v3(
    view: CandidateVerifyRowViewV1<'_>,
    outcome_count: u32,
    workspace: &mut [u8],
    cursor_workspace: &mut [u8],
    expected_manifest: &[u8],
) -> Result<GeneralVerifyCandidateProjectionV3> {
    if workspace.len()
        != general_hot_candidate_bank_len_v3(Action::VerifyCandidateRow, outcome_count)?
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    let authenticated =
        authenticate_general_verify_candidate_bank_v3(&view, outcome_count, workspace)?;
    let certificate_len = candidate_certificate_len_v1(view.submission)
        .map_err(GeneralHotCandidateErrorV3::Verify)?;
    let work_len = certificate_len
        .checked_add(expected_manifest.len())
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    let (summary, selection_magic, selection_phase) = {
        let work = workspace
            .get_mut(..work_len)
            .ok_or(GeneralHotCandidateErrorV3::InvalidCapacity)?;
        let (verified_workspace, manifest_workspace) = work.split_at_mut(certificate_len);
        let summary = verify_candidate_row_workspace_v1(
            view,
            cursor_workspace,
            verified_workspace,
            manifest_workspace,
        )
        .map_err(GeneralHotCandidateErrorV3::Verify)?;
        if manifest_workspace != expected_manifest {
            return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
        }
        let selection_magic = if summary.complete {
            read_data_u64(verified_workspace, 0)?
        } else {
            0
        };
        let selection_phase = if summary.complete {
            u64::from(read_data_u8(verified_workspace, 10)?)
        } else {
            0
        };
        (summary, selection_magic, selection_phase)
    };
    project_general_verify_candidate_summary_into_bank_v3(
        summary,
        cursor_workspace,
        selection_magic,
        selection_phase,
        outcome_count,
        authenticated,
        workspace,
    )
}

fn project_general_verify_candidate_to_scratch_v3(
    view: CandidateVerifyRowViewV1<'_>,
    verifier_buffers: CandidateVerifyRowBuffersV1<'_>,
    outcome_count: u32,
    authenticated_input: &[u8],
    scratch: &mut [u8],
) -> Result<GeneralVerifyCandidateProjectionV3> {
    let authenticated =
        authenticate_general_verify_candidate_bank_v3(&view, outcome_count, authenticated_input)?;
    scratch.copy_from_slice(authenticated_input);
    project_general_verify_candidate_into_bank_v3(
        view,
        verifier_buffers,
        outcome_count,
        authenticated,
        scratch,
    )
}

#[derive(Clone, Copy)]
struct GeneralVerifyCandidateBankV3 {
    candidate_id: [u8; 32],
    batch_id: [u8; 32],
    reward_rate_lamports: u64,
    principal: u64,
    payer: [u8; 32],
    trading_program: [u8; 32],
    scalar_count: u32,
}

fn authenticate_general_verify_candidate_bank_v3(
    view: &CandidateVerifyRowViewV1<'_>,
    outcome_count: u32,
    authenticated_input: &[u8],
) -> Result<GeneralVerifyCandidateBankV3> {
    let opening = view.submission.opening();
    let before = view.submission.state();
    let expected_page_index = view.expected_page_index;
    let expected_row_index = view.expected_row_index;
    let expected_revision = view.expected_revision;
    let scalar_count = general_hot_scalar_count_v3(Action::VerifyCandidateRow, outcome_count)?;
    let requested_candidate = read_identity(
        authenticated_input,
        scalar_count,
        identity::PARENT_REQUEST_DIGEST,
    )?;
    let payer = read_identity(authenticated_input, scalar_count, identity::PAYER)?;
    let trading_program =
        read_identity(authenticated_input, scalar_count, identity::TRADING_PROGRAM)?;
    let principal = read_scalar(authenticated_input, scalar::PRIMARY_PRINCIPAL_OBSERVATION)?;
    let expected_pre_lamports = principal
        .checked_add(before.verification_remaining)
        .and_then(|value| value.checked_add(before.cleanup_remaining))
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    if opening.outcome_count != outcome_count {
        return Err(GeneralHotCandidateErrorV3::TailCountMismatch);
    }
    if requested_candidate != opening.candidate_id
        || read_scalar(authenticated_input, scalar::ROOT_EXPECTED_REVISION)? != expected_revision
        || read_scalar(authenticated_input, scalar::COMPLETE_SET_MOVE)?
            != u64::from(expected_page_index)
        || read_scalar(authenticated_input, scalar::CLAIMS_AFFINE_ACTIVE)?
            != u64::from(expected_row_index)
        || read_scalar(authenticated_input, scalar::OUTCOME_COUNT)? != u64::from(outcome_count)
        || read_scalar(authenticated_input, scalar::ROOT_LIFECYCLE_OBSERVATION)?
            != u64::from(GeneralLifecycleV2::Active.tag())
        || read_scalar(authenticated_input, scalar::OBSERVED_POSITION_LAMPORTS)?
            != expected_pre_lamports
        || principal == 0
        || payer == [0; 32]
        || trading_program == [0; 32]
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    Ok(GeneralVerifyCandidateBankV3 {
        candidate_id: opening.candidate_id,
        batch_id: opening.batch_id,
        reward_rate_lamports: opening.reward_rate_lamports,
        principal,
        payer,
        trading_program,
        scalar_count,
    })
}

fn project_general_verify_candidate_into_bank_v3(
    view: CandidateVerifyRowViewV1<'_>,
    verifier_buffers: CandidateVerifyRowBuffersV1<'_>,
    outcome_count: u32,
    authenticated: GeneralVerifyCandidateBankV3,
    candidate: &mut [u8],
) -> Result<GeneralVerifyCandidateProjectionV3> {
    let CandidateVerifyRowBuffersV1 {
        cursor_scratch,
        cursor_output,
        verified_scratch,
        verified_output,
        manifest_scratch,
        manifest_output,
    } = verifier_buffers;
    let summary = verify_candidate_row_v1(
        view,
        CandidateVerifyRowBuffersV1 {
            cursor_scratch,
            cursor_output: &mut *cursor_output,
            verified_scratch,
            verified_output: &mut *verified_output,
            manifest_scratch,
            manifest_output,
        },
    )
    .map_err(GeneralHotCandidateErrorV3::Verify)?;
    let selection_magic = if summary.complete {
        read_data_u64(verified_output, 0)?
    } else {
        0
    };
    let selection_phase = if summary.complete {
        u64::from(read_data_u8(verified_output, 10)?)
    } else {
        0
    };
    project_general_verify_candidate_summary_into_bank_v3(
        summary,
        cursor_output,
        selection_magic,
        selection_phase,
        outcome_count,
        authenticated,
        candidate,
    )
}

fn project_general_verify_candidate_summary_into_bank_v3(
    summary: CandidateVerifyRowSummaryV1,
    cursor_output: &[u8],
    selection_magic: u64,
    selection_phase: u64,
    outcome_count: u32,
    authenticated: GeneralVerifyCandidateBankV3,
    candidate: &mut [u8],
) -> Result<GeneralVerifyCandidateProjectionV3> {
    let cursor = RuntimeCandidateVerifierV2::decode(cursor_output).map_err(|error| {
        GeneralHotCandidateErrorV3::Verify(GeneralCandidateErrorV1::Verify(error))
    })?;
    let cursor_header = cursor.header();
    let current = cursor.current_order().map_err(|error| {
        GeneralHotCandidateErrorV3::Verify(GeneralCandidateErrorV1::Verify(error))
    })?;
    let (
        current_order,
        current_owner,
        current_nonce,
        current_max_lots,
        current_quote_limit,
        current_lots,
        current_source_page,
        current_source_row,
    ) = current.map_or(([0; 32], [0; 32], 0, 0, 0, 0, 0, 0), |value| {
        (
            value.order_id,
            value.owner_id,
            value.nonce,
            value.max_lots,
            value.max_quote_debit_per_lot,
            value.lots,
            value.source_page_index,
            value.source_execution_index,
        )
    });
    let after = summary.submission.state();
    let expected_post_lamports = authenticated
        .principal
        .checked_add(after.verification_remaining)
        .and_then(|value| value.checked_add(after.cleanup_remaining))
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    if summary.reward.lamports != authenticated.reward_rate_lamports
        || cursor_header.outcome_count != outcome_count
        || cursor_header.candidate_id != authenticated.candidate_id
        || cursor_header.batch_id != authenticated.batch_id
        || cursor_header.revision != summary.revision
        || cursor_header.order_count != summary.order_count
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    write_local_state_constants(candidate, GeneralLocalStateKindV3::Verifier)?;
    for (coordinate, value) in [
        (scalar::ACTION, u64::from(Action::VerifyCandidateRow as u8)),
        (scalar::OUTCOME_COUNT, u64::from(outcome_count)),
        (scalar::VERIFY_TERMINAL, u64::from(summary.complete)),
        (scalar::CURSOR_MAGIC, read_data_u64(cursor_output, 0)?),
        (
            scalar::RUNTIME_WIDTH_VERSION,
            u64::from(read_data_u16(cursor_output, 8)?),
        ),
        (
            scalar::CUSTODY_ACTIVE,
            u64::from(cursor_header.has_current_order),
        ),
        (
            scalar::CANDIDATE_PAGE_COUNT,
            u64::from(cursor_header.page_count),
        ),
        (
            scalar::SELECTION_BEST_CANDIDATE_COORDINATE,
            u64::from(cursor_header.candidate_coordinate),
        ),
        (
            scalar::VERIFY_POST_PAGE,
            u64::from(cursor_header.next_page_index),
        ),
        (
            scalar::VERIFY_POST_ROW,
            u64::from(cursor_header.next_row_index),
        ),
        (
            scalar::VERIFY_POST_ORDER_COUNT,
            u64::from(summary.order_count),
        ),
        (scalar::VERIFY_POST_REVISION, summary.revision),
        (scalar::SELECTION_PRICE_SCALE, cursor_header.price_scale),
        (
            scalar::SELECTION_BEST_FILLED_LOTS,
            cursor_header.filled_lots,
        ),
        (scalar::ORDER_QUOTE_RESERVE, cursor_header.quote_debit),
        (scalar::QUOTE_QUANTITY, cursor_header.quote_credit),
        (scalar::ORDER_NONCE, current_nonce),
        (scalar::ORDER_MAX_LOTS, current_max_lots),
        (scalar::ORDER_MAX_QUOTE_DEBIT_PER_LOT, current_quote_limit),
        (scalar::ORDER_VALID_UNTIL_SLOT, current_lots),
        (scalar::PAGE_INDEX, u64::from(current_source_page)),
        (scalar::EXECUTION_INDEX, u64::from(current_source_row)),
        (
            scalar::VERIFY_MANIFEST_ORDER_COUNT,
            u64::from(summary.manifest_order_count),
        ),
        (scalar::CANDIDATE_POST_STATUS, u64::from(after.status.tag())),
        (
            scalar::CANDIDATE_POST_VERIFICATION_REMAINING,
            after.verification_remaining,
        ),
        (
            scalar::CANDIDATE_POST_CLEANUP_REMAINING,
            after.cleanup_remaining,
        ),
        (
            scalar::SELECTION_BEST_VERIFIED_REVISION,
            after.verified_revision,
        ),
        (scalar::SCRATCH_A, summary.reward.lamports),
        (scalar::SCRATCH_B, expected_post_lamports),
        (scalar::SELECTION_MAGIC, selection_magic),
        (scalar::SELECTION_PHASE, selection_phase),
    ] {
        write_scalar(candidate, coordinate, value)?;
    }
    for (coordinate, value) in [
        (identity::CANDIDATE, authenticated.candidate_id),
        (identity::SELECTION_PRODUCT, cursor_header.product_id),
        (identity::SELECTION_BATCH, cursor_header.batch_id),
        (identity::ORDER, current_order),
        (identity::OWNER, current_owner),
        (identity::BEST_VERIFIED_DIGEST, after.verified_digest),
        (
            identity::RESULT_BENEFICIARY_OBSERVATION,
            authenticated.payer,
        ),
        (identity::RESULT_BENEFICIARY, authenticated.payer),
        (identity::RESULT_OWNER, authenticated.trading_program),
    ] {
        write_identity(candidate, authenticated.scalar_count, coordinate, value)?;
    }
    for item in 0..outcome_count {
        let base = GENERAL_HOT_COMMON_SCALARS_V3
            .checked_add(
                item.checked_mul(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            )
            .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
        for (coordinate, value) in [
            (item_scalar::OUTCOME, u64::from(item)),
            (
                item_scalar::QUANTITY,
                cursor.price(item).map_err(|error| {
                    GeneralHotCandidateErrorV3::Verify(GeneralCandidateErrorV1::Verify(error))
                })?,
            ),
            (
                item_scalar::CURSOR_INVENTORY,
                cursor.current_receive_per_lot(item).map_err(|error| {
                    GeneralHotCandidateErrorV3::Verify(GeneralCandidateErrorV1::Verify(error))
                })?,
            ),
            (
                item_scalar::CLAIMS_AGGREGATE_MAGNITUDE,
                cursor.current_deliver_per_lot(item).map_err(|error| {
                    GeneralHotCandidateErrorV3::Verify(GeneralCandidateErrorV1::Verify(error))
                })?,
            ),
            (
                item_scalar::CLAIMS_SOURCE_MAGNITUDE,
                cursor.claim_input(item).map_err(|error| {
                    GeneralHotCandidateErrorV3::Verify(GeneralCandidateErrorV1::Verify(error))
                })?,
            ),
            (
                item_scalar::CLAIMS_DESTINATION_MAGNITUDE,
                cursor.claim_output(item).map_err(|error| {
                    GeneralHotCandidateErrorV3::Verify(GeneralCandidateErrorV1::Verify(error))
                })?,
            ),
        ] {
            write_scalar(candidate, base + coordinate, value)?;
        }
    }
    Ok(GeneralVerifyCandidateProjectionV3 { summary })
}

/// Return the exact scalar count for one action at Product width `outcome_count`.
///
/// The action is a parameter because the stride is. Every producer and consumer
/// of a General register bank reaches this one function, so an action that
/// declares no tail has a bank that does not grow -- everywhere at once.
pub fn general_hot_scalar_count_v3(action: Action, outcome_count: u32) -> Result<u32> {
    GENERAL_HOT_COMMON_SCALARS_V3
        .checked_add(
            outcome_count
                .checked_mul(general_hot_item_scalar_stride_v3(action))
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
        )
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)
}

/// Return the exact scalar-then-identity bank width.
pub fn general_hot_candidate_bank_len_v3(action: Action, outcome_count: u32) -> Result<usize> {
    if outcome_count == 0 {
        return Err(GeneralHotCandidateErrorV3::TailCountMismatch);
    }
    let bytes = register_bank_bytes_v2(
        general_hot_scalar_count_v3(action, outcome_count)?,
        GENERAL_HOT_COMMON_IDENTITIES_V3,
    )
    .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    usize::try_from(bytes).map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)
}

/// Authenticated input-bank transport for a read-only General accelerator.
///
/// Implementations may read one contiguous inline request or copy from the
/// exact ordered Trading-owned scratch pages. The destination is explicitly a
/// non-authoritative workspace: callers may observe partial bytes on refusal,
/// but no acknowledgement or state effect may be emitted until projection
/// returns `Ok(())` for the complete Product-derived width.
pub trait GeneralCandidateBankSourceV3 {
    /// Copy the complete authenticated bank into one exact-width workspace.
    fn copy_complete_bank_v3(&self, output: &mut [u8]) -> Result<()>;
}

/// Borrowed contiguous implementation used by inline accelerator requests and
/// host-side differential tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContiguousGeneralBankV3<'a> {
    bytes: &'a [u8],
}

impl<'a> ContiguousGeneralBankV3<'a> {
    /// Construct one exact borrowed bank source.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
}

impl GeneralCandidateBankSourceV3 for ContiguousGeneralBankV3<'_> {
    fn copy_complete_bank_v3(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != self.bytes.len() {
            return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
        }
        output.copy_from_slice(self.bytes);
        Ok(())
    }
}

/// Project one exact Consider or Freeze selection successor into a candidate bank.
///
/// The selection evaluator produced `selection_after` failure-atomically. This
/// projection preserves all independently authenticated registers and writes
/// only the canonical persisted state fields later consumed by the common
/// EffectProgram. It owns no account and performs no CPI.
pub fn project_general_selection_candidate_v3<'a>(
    action: Action,
    selection_after: &[u8],
    outcome_count: u32,
    authenticated_input: &[u8],
    scratch: &mut [u8],
    output: &'a mut [u8],
) -> Result<ExecutionCandidateV2<'a>> {
    if !matches!(action, Action::Consider | Action::Freeze) {
        return Err(GeneralHotCandidateErrorV3::InvalidPlan);
    }
    exact_candidate_capacities(action, outcome_count, authenticated_input, scratch, output)?;
    project_general_selection_candidate_scratch_v3(
        action,
        selection_after,
        outcome_count,
        &ContiguousGeneralBankV3::new(authenticated_input),
        scratch,
    )?;
    output.copy_from_slice(scratch);
    Ok(ExecutionCandidateV2::Accepted(output))
}

/// Project Consider/Freeze into one non-authoritative complete workspace.
///
/// The source may be inline or scratch-page backed. The returned bytes remain
/// non-authoritative until the accelerator has committed the whole-bank digest
/// and emitted the exact request-selected acknowledgement chunk.
pub fn project_general_selection_candidate_scratch_v3(
    action: Action,
    selection_after: &[u8],
    outcome_count: u32,
    source: &impl GeneralCandidateBankSourceV3,
    candidate_scratch: &mut [u8],
) -> Result<()> {
    let required = general_hot_candidate_bank_len_v3(action, outcome_count)?;
    if candidate_scratch.len() != required {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    source.copy_complete_bank_v3(candidate_scratch)?;
    apply_general_selection_candidate_v3(action, selection_after, outcome_count, candidate_scratch)
}

/// Apply Consider/Freeze directly to one complete non-authoritative bank.
///
/// This is the bounded-heap SBF accelerator seam: authenticated scratch pages
/// are assembled into `candidate_scratch` once, then the semantic projection
/// mutates that workspace in place. Callers must not emit an acknowledgement
/// unless this function succeeds for the complete Product-derived width.
pub fn project_general_selection_candidate_in_place_v3(
    action: Action,
    selection_after: &[u8],
    outcome_count: u32,
    candidate_scratch: &mut [u8],
) -> Result<()> {
    if candidate_scratch.len() != general_hot_candidate_bank_len_v3(action, outcome_count)? {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    apply_general_selection_candidate_v3(action, selection_after, outcome_count, candidate_scratch)
}

fn apply_general_selection_candidate_v3(
    action: Action,
    selection_after: &[u8],
    outcome_count: u32,
    candidate: &mut [u8],
) -> Result<()> {
    if !matches!(action, Action::Consider | Action::Freeze) {
        return Err(GeneralHotCandidateErrorV3::InvalidPlan);
    }
    let selection = RuntimeSelectionCursorV2::decode(selection_after)
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let header = selection.header();
    let expected_phase = if action == Action::Consider {
        RuntimeSelectionPhaseV2::Open
    } else {
        RuntimeSelectionPhaseV2::Frozen
    };
    if header.outcome_count != outcome_count || header.phase != expected_phase {
        return Err(GeneralHotCandidateErrorV3::TailCountMismatch);
    }
    write_local_state_constants(candidate, GeneralLocalStateKindV3::Selection)?;
    for (coordinate, value) in [
        (scalar::ACTION, u64::from(action as u8)),
        (scalar::OUTCOME_COUNT, u64::from(outcome_count)),
        (scalar::SELECTION_PHASE, selection_phase_tag(header.phase)),
        (scalar::SELECTION_REVISION, header.revision),
        (
            scalar::SELECTION_SUBMITTED_COUNT,
            u64::from(header.submitted_count),
        ),
        (
            scalar::SELECTION_BEST_CANDIDATE_COORDINATE,
            u64::from(header.best_candidate_coordinate),
        ),
        (
            scalar::SELECTION_BEST_VERIFIED_REVISION,
            header.best_verified_revision,
        ),
        (scalar::SELECTION_PRICE_SCALE, header.price_scale),
        (scalar::SELECTION_BEST_FILLED_LOTS, header.best_filled_lots),
        (
            scalar::SELECTION_BEST_QUOTE_SURPLUS,
            header.best_quote_surplus,
        ),
        (
            scalar::SELECTION_MAGIC,
            RuntimeSelectionLayoutV2::magic_u64(),
        ),
        (
            scalar::RUNTIME_WIDTH_VERSION,
            u64::from(RuntimeSelectionLayoutV2::version_value()),
        ),
    ] {
        write_scalar(candidate, coordinate, value)?;
    }
    let scalar_count = general_hot_scalar_count_v3(action, outcome_count)?;
    for (coordinate, value) in [
        (identity::CANDIDATE, header.best_candidate_id),
        (identity::SELECTION_PRODUCT, header.product_id),
        (identity::SELECTION_BATCH, header.batch_id),
        (identity::SELECTION_POLICY, header.policy_id),
        (identity::BEST_VERIFIED_DIGEST, header.best_verified_digest),
    ] {
        write_identity(candidate, scalar_count, coordinate, value)?;
    }
    Ok(())
}

/// Project settlement initialization and exact Custody replay/vault creation
/// facts into one complete candidate bank.
pub fn project_general_initialize_candidate_v3<'a>(
    cursor_after: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    authenticated_input: &[u8],
    scratch: &mut [u8],
    output: &'a mut [u8],
) -> Result<ExecutionCandidateV2<'a>> {
    exact_candidate_capacities(
        Action::InitializeSettlement,
        outcome_count,
        authenticated_input,
        scratch,
        output,
    )?;
    project_general_initialize_candidate_scratch_v3(
        cursor_after,
        outcome_count,
        environment,
        &ContiguousGeneralBankV3::new(authenticated_input),
        scratch,
    )?;
    output.copy_from_slice(scratch);
    Ok(ExecutionCandidateV2::Accepted(output))
}

/// Project settlement initialization into one non-authoritative complete bank.
pub fn project_general_initialize_candidate_scratch_v3(
    cursor_after: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    source: &impl GeneralCandidateBankSourceV3,
    candidate_scratch: &mut [u8],
) -> Result<()> {
    let required = general_hot_candidate_bank_len_v3(Action::InitializeSettlement, outcome_count)?;
    if candidate_scratch.len() != required {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    source.copy_complete_bank_v3(candidate_scratch)?;
    apply_general_initialize_candidate_v3(
        cursor_after,
        outcome_count,
        environment,
        candidate_scratch,
    )
}

/// Apply settlement initialization directly to one complete non-authoritative bank.
///
/// This avoids retaining a second 14KiB register bank at hostile widths while
/// preserving Trading's authority boundary: the workspace remains candidate
/// data until its whole-bank digest is acknowledged and accepted by Hot.
pub fn project_general_initialize_candidate_in_place_v3(
    cursor_after: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    candidate_scratch: &mut [u8],
) -> Result<()> {
    if candidate_scratch.len()
        != general_hot_candidate_bank_len_v3(Action::InitializeSettlement, outcome_count)?
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    apply_general_initialize_candidate_v3(
        cursor_after,
        outcome_count,
        environment,
        candidate_scratch,
    )
}

fn apply_general_initialize_candidate_v3(
    cursor_after: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    candidate: &mut [u8],
) -> Result<()> {
    let cursor = SettlementCursorV2::decode(cursor_after)
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let header = cursor.header();
    if header.outcome_count != outcome_count
        || header.phase != crate::runtime_width::SettlementPhaseV2::Collecting
        || header.revision != 1
        || environment.generation == 0
        || environment.page_index != 0
        || environment.execution_index != 0
        || environment.custody_expected_revision != 0
        || environment.custody_replay_rent_principal == 0
        || environment.custody_vault_rent_principal == 0
        || environment.claims_market_revision == u64::MAX
        || environment.settlement_position_present
        || environment.settlement_position_revision != 0
        || environment.settlement_position_owner == [0; 32]
        || environment.rent_credit == [0; 32]
        || environment.rent_program == [0; 32]
        || environment.position_rent_principal == 0
        || environment.admission_rent_principal == 0
        || environment.observed_position_lamports < environment.position_rent_principal
        || environment.observed_admission_lamports < environment.admission_rent_principal
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    let scalar_count = general_hot_scalar_count_v3(Action::InitializeSettlement, outcome_count)?;
    if read_scalar(candidate, scalar::OUTCOME_COUNT)? != u64::from(outcome_count)
        || read_identity(candidate, scalar_count, identity::PARENT_REQUEST_DIGEST)?
            != environment.parent_request_digest
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    for value in [
        environment.general_root,
        environment.parent_request_digest,
        environment.release_set,
        environment.market,
        environment.realm,
        environment.trading_program,
        environment.custody_destination,
        environment.mint,
        environment.token_program,
        environment.payer,
        environment.rent_refund,
    ] {
        if value.iter().all(|byte| *byte == 0) {
            return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
        }
    }
    write_local_state_constants(candidate, GeneralLocalStateKindV3::Settlement)?;
    for (coordinate, value) in [
        (
            scalar::ACTION,
            u64::from(Action::InitializeSettlement as u8),
        ),
        (scalar::OUTCOME_COUNT, u64::from(outcome_count)),
        (scalar::GENERATION, environment.generation),
        (
            scalar::CLAIMS_MARKET_REVISION,
            environment.claims_market_revision,
        ),
        (
            scalar::SETTLEMENT_POSITION_REVISION,
            environment.settlement_position_revision,
        ),
        (
            scalar::OBSERVED_POSITION_LAMPORTS,
            environment.observed_position_lamports,
        ),
        (
            scalar::OBSERVED_ADMISSION_LAMPORTS,
            environment.observed_admission_lamports,
        ),
        (
            scalar::POSITION_RENT_PRINCIPAL,
            environment.position_rent_principal,
        ),
        (
            scalar::ADMISSION_RENT_PRINCIPAL,
            environment.admission_rent_principal,
        ),
        (
            scalar::CUSTODY_REPLAY_RENT_LAMPORTS,
            environment.custody_replay_rent_principal,
        ),
        (
            scalar::CUSTODY_VAULT_RENT_LAMPORTS,
            environment.custody_vault_rent_principal,
        ),
        (scalar::ZERO, 0),
        (scalar::CURSOR_PHASE, settlement_phase_tag(header.phase)),
        (scalar::CURSOR_ORDER_COUNT, u64::from(header.order_count)),
        (scalar::CURSOR_NEXT_ORDER, u64::from(header.next_order)),
        (scalar::CURSOR_RESULTING_REVISION, header.revision),
        (scalar::CURSOR_QUOTE_INVENTORY, header.quote_inventory),
        (
            scalar::CURSOR_COMPLETE_SET_QUANTITY,
            header.complete_set_quantity,
        ),
        (scalar::CURSOR_MAGIC, SettlementCursorLayoutV2::magic_u64()),
        (
            scalar::RUNTIME_WIDTH_VERSION,
            u64::from(SettlementCursorLayoutV2::version_value()),
        ),
        (
            scalar::CURSOR_TERMINAL_COORDINATE,
            header.terminal_coordinate,
        ),
    ] {
        write_scalar(candidate, coordinate, value)?;
    }
    for item in 0..outcome_count {
        let base = GENERAL_HOT_COMMON_SCALARS_V3
            .checked_add(
                item.checked_mul(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            )
            .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
        write_scalar(
            candidate,
            base.checked_add(item_scalar::OUTCOME)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            u64::from(item),
        )?;
        write_scalar(
            candidate,
            base.checked_add(item_scalar::CURSOR_INVENTORY)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            cursor
                .inventory(item)
                .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?,
        )?;
    }
    for (coordinate, value) in [
        (
            identity::PARENT_REQUEST_DIGEST,
            environment.parent_request_digest,
        ),
        (identity::CANDIDATE, header.candidate_id),
        (identity::RELEASE_SET, environment.release_set),
        (identity::MARKET, environment.market),
        (identity::REALM, environment.realm),
        (identity::TRADING_PROGRAM, environment.trading_program),
        (
            identity::SETTLEMENT_POSITION_OWNER,
            environment.settlement_position_owner,
        ),
        (identity::RENT_CREDIT, environment.rent_credit),
        (identity::RENT_PROGRAM, environment.rent_program),
        (
            identity::CUSTODY_DESTINATION,
            environment.custody_destination,
        ),
        (
            identity::DESTINATION_VAULT_CONTEXT,
            environment.general_root,
        ),
        (identity::MINT, environment.mint),
        (identity::TOKEN_PROGRAM, environment.token_program),
        (identity::PAYER, environment.payer),
        (identity::RENT_REFUND, environment.rent_refund),
        (identity::GENERAL_ROOT, environment.general_root),
    ] {
        write_identity(candidate, scalar_count, coordinate, value)?;
    }
    Ok(())
}

/// Project one complete General plan without discarding authenticated inputs.
///
/// `authenticated_input`, `scratch`, and `output` must have the one exact
/// Product-derived capacity. The entire input is copied to scratch first;
/// output changes only after every semantic and child-ABI coordinate accepts.
pub fn project_general_hot_candidate_v3<'a>(
    action: Action,
    effect_plan: &[u8],
    cursor_after: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    authenticated_input: &[u8],
    scratch: &mut [u8],
    output: &'a mut [u8],
) -> Result<ExecutionCandidateV2<'a>> {
    exact_candidate_capacities(action, outcome_count, authenticated_input, scratch, output)?;
    project_general_hot_candidate_scratch_v3(
        action,
        effect_plan,
        cursor_after,
        outcome_count,
        environment,
        &ContiguousGeneralBankV3::new(authenticated_input),
        scratch,
    )?;
    output.copy_from_slice(scratch);
    Ok(ExecutionCandidateV2::Accepted(output))
}

/// Project Collect/Materialize/Distribute/Close into one complete scratch bank.
pub fn project_general_hot_candidate_scratch_v3(
    action: Action,
    effect_plan: &[u8],
    cursor_after: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    source: &impl GeneralCandidateBankSourceV3,
    candidate_scratch: &mut [u8],
) -> Result<()> {
    let required = general_hot_candidate_bank_len_v3(action, outcome_count)?;
    if candidate_scratch.len() != required {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    source.copy_complete_bank_v3(candidate_scratch)?;
    apply_general_hot_candidate_v3(
        action,
        effect_plan,
        cursor_after,
        outcome_count,
        environment,
        candidate_scratch,
    )
}

/// Apply one collect/materialize/distribute/close plan directly to a complete bank.
///
/// The buffer is an accelerator-owned scratch candidate, never persisted
/// state. Semantic refusal may leave scratch bytes changed, but no partial
/// bank can become authoritative because no acknowledgement is emitted.
pub fn project_general_hot_candidate_in_place_v3(
    action: Action,
    effect_plan: &[u8],
    cursor_after: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    candidate_scratch: &mut [u8],
) -> Result<()> {
    if candidate_scratch.len() != general_hot_candidate_bank_len_v3(action, outcome_count)? {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    apply_general_hot_candidate_v3(
        action,
        effect_plan,
        cursor_after,
        outcome_count,
        environment,
        candidate_scratch,
    )
}

fn apply_general_hot_candidate_v3(
    action: Action,
    effect_plan: &[u8],
    cursor_after: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    candidate: &mut [u8],
) -> Result<()> {
    let plan = RuntimeSettlementEffectPlanV2::decode(effect_plan)
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let cursor = SettlementCursorV2::decode(cursor_after)
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let cursor_header = cursor.header();
    if plan.header().outcome_count != outcome_count
        || cursor_header.outcome_count != outcome_count
        || cursor_header.candidate_id != plan.header().candidate_id
    {
        return Err(GeneralHotCandidateErrorV3::TailCountMismatch);
    }
    let input_scalar_count = general_hot_scalar_count_v3(action, outcome_count)?;
    if read_scalar(candidate, scalar::OUTCOME_COUNT)? != u64::from(outcome_count)
        || read_identity(
            candidate,
            input_scalar_count,
            identity::PARENT_REQUEST_DIGEST,
        )? != environment.parent_request_digest
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    validate_environment(
        plan.header().action,
        plan.header().custody_active,
        environment,
    )?;
    write_local_state_constants(candidate, GeneralLocalStateKindV3::Settlement)?;
    let header = plan.header();
    let position = position_geometry(header.action, header.claims_active, environment, header)?;
    let custody = custody_geometry(
        header.action,
        header.custody_active,
        header.complete_set_move,
    );
    let custody_resulting_revision = environment
        .custody_expected_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let custody_close_vault_expected_revision = environment
        .custody_expected_revision
        .checked_add(u64::from(header.custody_active))
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let custody_close_vault_resulting_revision = custody_close_vault_expected_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let custody_close_replay_resulting_revision = custody_close_vault_resulting_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let claims_post_market_revision = environment
        .claims_market_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let settlement_post_position_revision = environment
        .settlement_position_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let custody_amount = if header.action == RuntimeSettlementActionV2::Materialize {
        header.complete_set_quantity
    } else {
        header.quote_quantity
    };
    for (coordinate, value) in [
        (scalar::ACTION, action_tag(header.action)),
        (
            scalar::COMPLETE_SET_MOVE,
            move_tag(header.complete_set_move),
        ),
        (
            scalar::CLAIMS_AFFINE_ACTIVE,
            u64::from(header.claims_active),
        ),
        (scalar::CUSTODY_ACTIVE, u64::from(header.custody_active)),
        (scalar::TERMINAL, u64::from(header.terminal)),
        (scalar::ORDER_COORDINATE, u64::from(header.order_coordinate)),
        (scalar::SETTLEMENT_REVISION, header.revision),
        (scalar::ORDER_NONCE, header.nonce),
        (scalar::QUOTE_QUANTITY, header.quote_quantity),
        (scalar::COMPLETE_SET_QUANTITY, header.complete_set_quantity),
        (scalar::OUTCOME_COUNT, u64::from(outcome_count)),
        (scalar::TERMINAL_COORDINATE, header.terminal_coordinate),
        (scalar::GENERATION, environment.generation),
        (scalar::PAGE_INDEX, u64::from(environment.page_index)),
        (
            scalar::EXECUTION_INDEX,
            u64::from(environment.execution_index),
        ),
        (
            scalar::TRANSFER_INDEX,
            u64::from(environment.transfer_index),
        ),
        (
            scalar::CUSTODY_EXPECTED_REVISION,
            environment.custody_expected_revision,
        ),
        (
            scalar::CUSTODY_RESULTING_REVISION,
            custody_resulting_revision,
        ),
        (scalar::CUSTODY_RENT_LAMPORTS, 0),
        (
            scalar::CLAIMS_MARKET_REVISION,
            environment.claims_market_revision,
        ),
        (
            scalar::OWNER_POSITION_REVISION,
            environment.owner_position_revision,
        ),
        (
            scalar::SETTLEMENT_POSITION_REVISION,
            environment.settlement_position_revision,
        ),
        (scalar::CLAIMS_POSITION_COUNT, u64::from(position.count)),
        (
            scalar::CLAIMS_ROW_COUNT,
            if header.claims_active {
                u64::from(outcome_count)
            } else {
                0
            },
        ),
        (scalar::CLAIMS_ADMIT_ACTIVE, 0),
        (scalar::CLAIMS_CLOSE_ACTIVE, 0),
        (scalar::CUSTODY_OPERATION, 2),
        (scalar::CUSTODY_SOURCE_COMPARTMENT, custody.source),
        (scalar::CUSTODY_DESTINATION_COMPARTMENT, custody.destination),
        (
            scalar::CLAIMS_SOURCE_PRESENT,
            u64::from(position.source_present),
        ),
        (
            scalar::CLAIMS_DESTINATION_PRESENT,
            u64::from(position.destination_present),
        ),
        (
            scalar::CLAIMS_SOURCE_POSITION_INDEX,
            u64::from(position.source_index),
        ),
        (
            scalar::CLAIMS_DESTINATION_POSITION_INDEX,
            u64::from(position.destination_index),
        ),
        (
            scalar::CLAIMS_AGGREGATE_DIRECTION,
            position.aggregate_direction,
        ),
        (scalar::CLAIMS_SOURCE_DIRECTION, position.source_direction),
        (
            scalar::CLAIMS_DESTINATION_DIRECTION,
            position.destination_direction,
        ),
        (
            scalar::OBSERVED_POSITION_LAMPORTS,
            environment.observed_position_lamports,
        ),
        (
            scalar::OBSERVED_ADMISSION_LAMPORTS,
            environment.observed_admission_lamports,
        ),
        (
            scalar::POSITION_RENT_PRINCIPAL,
            environment.position_rent_principal,
        ),
        (
            scalar::ADMISSION_RENT_PRINCIPAL,
            environment.admission_rent_principal,
        ),
        (
            scalar::SETTLEMENT_POSITION_PRESENT,
            u64::from(environment.settlement_position_present),
        ),
        (scalar::POSITION_ZERO_REVISION, position.zero_revision),
        (scalar::POSITION_ONE_REVISION, position.one_revision),
        (scalar::POSITION_TABLE_COUNT, u64::from(position.count)),
        (
            scalar::CLAIMS_POST_MARKET_REVISION,
            claims_post_market_revision,
        ),
        (
            scalar::SETTLEMENT_POST_POSITION_REVISION,
            settlement_post_position_revision,
        ),
        (scalar::CUSTODY_AMOUNT, custody_amount),
        (
            scalar::CUSTODY_REPLAY_RENT_LAMPORTS,
            environment.custody_replay_rent_principal,
        ),
        (
            scalar::CUSTODY_VAULT_RENT_LAMPORTS,
            environment.custody_vault_rent_principal,
        ),
        (
            scalar::CUSTODY_CLOSE_VAULT_EXPECTED_REVISION,
            custody_close_vault_expected_revision,
        ),
        (
            scalar::CUSTODY_CLOSE_VAULT_RESULTING_REVISION,
            custody_close_vault_resulting_revision,
        ),
        (
            scalar::CUSTODY_CLOSE_REPLAY_RESULTING_REVISION,
            custody_close_replay_resulting_revision,
        ),
        (scalar::ZERO, 0),
        (
            scalar::CURSOR_PHASE,
            settlement_phase_tag(cursor_header.phase),
        ),
        (
            scalar::CURSOR_ORDER_COUNT,
            u64::from(cursor_header.order_count),
        ),
        (
            scalar::CURSOR_NEXT_ORDER,
            u64::from(cursor_header.next_order),
        ),
        (scalar::CURSOR_RESULTING_REVISION, cursor_header.revision),
        (
            scalar::CURSOR_QUOTE_INVENTORY,
            cursor_header.quote_inventory,
        ),
        (
            scalar::CURSOR_COMPLETE_SET_QUANTITY,
            cursor_header.complete_set_quantity,
        ),
        (scalar::CURSOR_MAGIC, SettlementCursorLayoutV2::magic_u64()),
        (
            scalar::RUNTIME_WIDTH_VERSION,
            u64::from(SettlementCursorLayoutV2::version_value()),
        ),
        (
            scalar::CURSOR_TERMINAL_COORDINATE,
            cursor_header.terminal_coordinate,
        ),
    ] {
        write_scalar(candidate, coordinate, value)?;
    }
    for item in 0..outcome_count {
        let base = GENERAL_HOT_COMMON_SCALARS_V3
            .checked_add(
                item.checked_mul(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            )
            .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
        write_scalar(
            candidate,
            base.checked_add(item_scalar::OUTCOME)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            u64::from(item),
        )?;
        let quantity = plan
            .quantity(item)
            .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
        write_scalar(
            candidate,
            base.checked_add(item_scalar::QUANTITY)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            quantity,
        )?;
        write_scalar(
            candidate,
            base.checked_add(item_scalar::CURSOR_INVENTORY)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            cursor
                .inventory(item)
                .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?,
        )?;
        for (coordinate, direction) in [
            (
                item_scalar::CLAIMS_AGGREGATE_MAGNITUDE,
                position.aggregate_direction,
            ),
            (
                item_scalar::CLAIMS_SOURCE_MAGNITUDE,
                position.source_direction,
            ),
            (
                item_scalar::CLAIMS_DESTINATION_MAGNITUDE,
                position.destination_direction,
            ),
        ] {
            write_scalar(
                candidate,
                base.checked_add(coordinate)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
                if direction == 0 { 0 } else { quantity },
            )?;
        }
    }
    let scalar_count = general_hot_scalar_count_v3(action, outcome_count)?;
    for (coordinate, value) in [
        (
            identity::PARENT_REQUEST_DIGEST,
            environment.parent_request_digest,
        ),
        (identity::CANDIDATE, header.candidate_id),
        (identity::OWNER, header.owner_id),
        (identity::ORDER, header.order_id),
        (identity::BENEFICIARY, header.beneficiary),
        (identity::RELEASE_SET, environment.release_set),
        (identity::MARKET, environment.market),
        (
            identity::PRODUCT_RECORD_DIGEST,
            environment.product_record_digest,
        ),
        (identity::SEMANTIC_BASIS_ID, environment.semantic_basis_id),
        (
            identity::LINKED_BASIS_RECORD_DIGEST,
            environment.linked_basis_record_digest,
        ),
        (identity::REALM, environment.realm),
        (identity::TRADING_PROGRAM, environment.trading_program),
        (identity::CUSTODY_SOURCE, environment.custody_source),
        (
            identity::CUSTODY_DESTINATION,
            environment.custody_destination,
        ),
        (
            identity::SOURCE_VAULT_CONTEXT,
            environment.source_vault_context,
        ),
        (
            identity::DESTINATION_VAULT_CONTEXT,
            environment.destination_vault_context,
        ),
        (identity::MINT, environment.mint),
        (identity::TOKEN_PROGRAM, environment.token_program),
        (identity::PAYER, environment.payer),
        (identity::RENT_REFUND, environment.rent_refund),
        (
            identity::SETTLEMENT_POSITION_OWNER,
            environment.settlement_position_owner,
        ),
        (identity::RENT_CREDIT, environment.rent_credit),
        (identity::RENT_PROGRAM, environment.rent_program),
        (
            identity::CUSTODY_SOURCE_OWNER,
            environment.custody_source_owner,
        ),
        (
            identity::CUSTODY_DESTINATION_OWNER,
            environment.custody_destination_owner,
        ),
        (identity::POSITION_ZERO_OWNER, position.zero_owner),
        (identity::POSITION_ONE_OWNER, position.one_owner),
        (identity::GENERAL_ROOT, environment.general_root),
    ] {
        write_identity(candidate, scalar_count, coordinate, value)?;
    }
    Ok(())
}

fn exact_candidate_capacities(
    action: Action,
    outcome_count: u32,
    authenticated_input: &[u8],
    scratch: &[u8],
    output: &[u8],
) -> Result<()> {
    let required = general_hot_candidate_bank_len_v3(action, outcome_count)?;
    if authenticated_input.len() != required
        || scratch.len() != required
        || output.len() != required
    {
        Err(GeneralHotCandidateErrorV3::InvalidCapacity)
    } else {
        Ok(())
    }
}

const fn selection_phase_tag(phase: RuntimeSelectionPhaseV2) -> u64 {
    match phase {
        RuntimeSelectionPhaseV2::Open => 1,
        RuntimeSelectionPhaseV2::Frozen => 2,
    }
}

const fn settlement_phase_tag(phase: crate::runtime_width::SettlementPhaseV2) -> u64 {
    match phase {
        crate::runtime_width::SettlementPhaseV2::Collecting => 4,
        crate::runtime_width::SettlementPhaseV2::Materializing => 5,
        crate::runtime_width::SettlementPhaseV2::Distributing => 6,
        crate::runtime_width::SettlementPhaseV2::ReadyToClose => 7,
        crate::runtime_width::SettlementPhaseV2::Terminal => 8,
    }
}

#[derive(Clone, Copy)]
struct PositionGeometryV3 {
    count: u32,
    source_present: bool,
    destination_present: bool,
    source_index: u32,
    destination_index: u32,
    aggregate_direction: u64,
    source_direction: u64,
    destination_direction: u64,
    zero_owner: [u8; 32],
    one_owner: [u8; 32],
    zero_revision: u64,
    one_revision: u64,
}

fn position_geometry(
    action: RuntimeSettlementActionV2,
    claims_active: bool,
    environment: GeneralHotEnvironmentV3,
    header: crate::runtime_settlement::RuntimeSettlementEffectHeaderV2,
) -> Result<PositionGeometryV3> {
    if !claims_active {
        return Ok(PositionGeometryV3 {
            count: 0,
            source_present: false,
            destination_present: false,
            source_index: 0,
            destination_index: 0,
            aggregate_direction: 0,
            source_direction: 0,
            destination_direction: 0,
            zero_owner: [0; 32],
            one_owner: [0; 32],
            zero_revision: 0,
            one_revision: 0,
        });
    }
    match action {
        RuntimeSettlementActionV2::Collect | RuntimeSettlementActionV2::Distribute => {
            if !environment.settlement_position_present
                || header.owner_id == environment.settlement_position_owner
            {
                return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
            }
            let collect = action == RuntimeSettlementActionV2::Collect;
            let (source_owner, source_revision, destination_owner, destination_revision) =
                if collect {
                    (
                        header.owner_id,
                        environment.owner_position_revision,
                        environment.settlement_position_owner,
                        environment.settlement_position_revision,
                    )
                } else {
                    (
                        environment.settlement_position_owner,
                        environment.settlement_position_revision,
                        header.owner_id,
                        environment.owner_position_revision,
                    )
                };
            let (zero_owner, zero_revision, one_owner, one_revision, source_index) =
                if source_owner < destination_owner {
                    (
                        source_owner,
                        source_revision,
                        destination_owner,
                        destination_revision,
                        0,
                    )
                } else {
                    (
                        destination_owner,
                        destination_revision,
                        source_owner,
                        source_revision,
                        1,
                    )
                };
            Ok(PositionGeometryV3 {
                count: 2,
                source_present: true,
                destination_present: true,
                source_index,
                destination_index: 1 - source_index,
                aggregate_direction: 0,
                source_direction: 2,
                destination_direction: 1,
                zero_owner,
                one_owner,
                zero_revision,
                one_revision,
            })
        }
        RuntimeSettlementActionV2::Materialize => {
            if !environment.settlement_position_present {
                return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
            }
            let (source_present, destination_present, aggregate, source, destination) =
                match header.complete_set_move {
                    RuntimeCompleteSetMoveV2::Mint => (false, true, 1, 0, 1),
                    RuntimeCompleteSetMoveV2::Merge => (true, false, 2, 2, 0),
                    RuntimeCompleteSetMoveV2::None => {
                        return Err(GeneralHotCandidateErrorV3::InvalidPlan);
                    }
                };
            Ok(PositionGeometryV3 {
                count: 1,
                source_present,
                destination_present,
                source_index: 0,
                destination_index: 0,
                aggregate_direction: aggregate,
                source_direction: source,
                destination_direction: destination,
                zero_owner: environment.settlement_position_owner,
                one_owner: [0; 32],
                zero_revision: environment.settlement_position_revision,
                one_revision: 0,
            })
        }
        RuntimeSettlementActionV2::Close => Err(GeneralHotCandidateErrorV3::InvalidPlan),
    }
}

#[derive(Clone, Copy)]
struct CustodyGeometryV3 {
    source: u64,
    destination: u64,
}

fn custody_geometry(
    action: RuntimeSettlementActionV2,
    active: bool,
    movement: RuntimeCompleteSetMoveV2,
) -> CustodyGeometryV3 {
    if !active {
        return CustodyGeometryV3 {
            source: 0,
            destination: 0,
        };
    }
    let (source, destination) = match action {
        RuntimeSettlementActionV2::Collect => (1, 2),
        RuntimeSettlementActionV2::Distribute | RuntimeSettlementActionV2::Close => (2, 1),
        RuntimeSettlementActionV2::Materialize => match movement {
            RuntimeCompleteSetMoveV2::Mint => (2, 3),
            RuntimeCompleteSetMoveV2::Merge => (3, 2),
            RuntimeCompleteSetMoveV2::None => (0, 0),
        },
    };
    CustodyGeometryV3 {
        source,
        destination,
    }
}

fn validate_environment(
    action: RuntimeSettlementActionV2,
    custody_active: bool,
    environment: GeneralHotEnvironmentV3,
) -> Result<()> {
    for identity in [
        environment.general_root,
        environment.parent_request_digest,
        environment.release_set,
        environment.market,
        environment.product_record_digest,
        environment.semantic_basis_id,
        environment.linked_basis_record_digest,
        environment.realm,
        environment.trading_program,
        environment.settlement_position_owner,
        environment.rent_credit,
        environment.rent_program,
    ] {
        if identity == [0; 32] {
            return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
        }
    }
    if environment.claims_market_revision == u64::MAX
        || environment.owner_position_revision == u64::MAX
        || environment.settlement_position_revision == u64::MAX
        || environment.position_rent_principal == 0
        || environment.admission_rent_principal == 0
        || environment.custody_replay_rent_principal == 0
        || environment.custody_vault_rent_principal == 0
        || environment.observed_position_lamports < environment.position_rent_principal
        || environment.observed_admission_lamports < environment.admission_rent_principal
        || environment.payer != [0; 32]
        || (action == RuntimeSettlementActionV2::Close && environment.rent_refund == [0; 32])
        || (action == RuntimeSettlementActionV2::Close
            && (!environment.settlement_position_present || !environment.close_settlement_position))
        || (action != RuntimeSettlementActionV2::Close && !environment.settlement_position_present)
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    if custody_active {
        for identity in [
            environment.custody_source,
            environment.custody_destination,
            environment.mint,
            environment.token_program,
        ] {
            if identity == [0; 32] {
                return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
            }
        }
        if environment.custody_source == environment.custody_destination
            || environment.custody_expected_revision == u64::MAX
        {
            return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
        }
        let source_external = action == RuntimeSettlementActionV2::Collect;
        let destination_external = matches!(
            action,
            RuntimeSettlementActionV2::Distribute | RuntimeSettlementActionV2::Close
        );
        let source_shape = if source_external {
            environment.source_vault_context == [0; 32]
                && environment.custody_source_owner != [0; 32]
        } else {
            environment.source_vault_context != [0; 32]
                && environment.custody_source_owner == [0; 32]
        };
        let destination_shape = if destination_external {
            environment.destination_vault_context == [0; 32]
                && environment.custody_destination_owner != [0; 32]
        } else {
            environment.destination_vault_context != [0; 32]
                && environment.custody_destination_owner == [0; 32]
        };
        if !source_shape || !destination_shape {
            return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
        }
    }
    Ok(())
}

fn action_tag(action: RuntimeSettlementActionV2) -> u64 {
    match action {
        RuntimeSettlementActionV2::Collect => 1,
        RuntimeSettlementActionV2::Materialize => 2,
        RuntimeSettlementActionV2::Distribute => 3,
        RuntimeSettlementActionV2::Close => 4,
    }
}

fn move_tag(value: RuntimeCompleteSetMoveV2) -> u64 {
    match value {
        RuntimeCompleteSetMoveV2::None => 0,
        RuntimeCompleteSetMoveV2::Mint => 1,
        RuntimeCompleteSetMoveV2::Merge => 2,
    }
}

fn write_local_state_constants(output: &mut [u8], kind: GeneralLocalStateKindV3) -> Result<()> {
    for (coordinate, value) in [
        (
            scalar::LOCAL_STATE_MAGIC,
            GeneralLocalStateLayoutV3::magic_u64(),
        ),
        (
            scalar::LOCAL_STATE_VERSION,
            u64::from(GeneralLocalStateLayoutV3::version_value()),
        ),
        (scalar::LOCAL_STATE_KIND, u64::from(kind.tag())),
    ] {
        write_scalar(output, coordinate, value)?;
    }
    Ok(())
}

fn write_scalar(output: &mut [u8], coordinate: u32, value: u64) -> Result<()> {
    let offset = usize::try_from(coordinate)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    put(output, offset, &value.to_le_bytes())
}

fn read_scalar(input: &[u8], coordinate: u32) -> Result<u64> {
    let offset = usize::try_from(coordinate)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    let bytes: [u8; 8] = input
        .get(
            offset
                ..offset
                    .checked_add(8)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
        )
        .ok_or(GeneralHotCandidateErrorV3::InvalidCapacity)?
        .try_into()
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidCapacity)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_data_u64(input: &[u8], offset: usize) -> Result<u64> {
    let bytes: [u8; 8] = input
        .get(
            offset
                ..offset
                    .checked_add(8)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
        )
        .ok_or(GeneralHotCandidateErrorV3::InvalidCapacity)?
        .try_into()
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidCapacity)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_data_u16(input: &[u8], offset: usize) -> Result<u16> {
    let bytes: [u8; 2] = input
        .get(
            offset
                ..offset
                    .checked_add(2)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
        )
        .ok_or(GeneralHotCandidateErrorV3::InvalidCapacity)?
        .try_into()
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidCapacity)?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_data_u8(input: &[u8], offset: usize) -> Result<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(GeneralHotCandidateErrorV3::InvalidCapacity)
}

fn write_identity(
    output: &mut [u8],
    scalar_count: u32,
    coordinate: u32,
    value: [u8; 32],
) -> Result<()> {
    let scalar_bytes = usize::try_from(scalar_count)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    let identity_offset = usize::try_from(coordinate)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(32)
        .and_then(|offset| scalar_bytes.checked_add(offset))
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    put(output, identity_offset, &value)
}

fn read_identity(input: &[u8], scalar_count: u32, coordinate: u32) -> Result<[u8; 32]> {
    let scalar_bytes = usize::try_from(scalar_count)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    let offset = usize::try_from(coordinate)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(32)
        .and_then(|value| scalar_bytes.checked_add(value))
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    input
        .get(
            offset
                ..offset
                    .checked_add(32)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
        )
        .ok_or(GeneralHotCandidateErrorV3::InvalidCapacity)?
        .try_into()
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidCapacity)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    output
        .get_mut(offset..end)
        .ok_or(GeneralHotCandidateErrorV3::InvalidCapacity)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{vec, vec::Vec};

    use super::*;
    use crate::{
        candidate_v1::{
            GeneralCandidateStatusV1, candidate_certificate_len_v1, candidate_verifier_len_v1,
            candidate_verify_manifest_orders_v1, general_candidate_identity_v1,
        },
        runtime_manifest::settlement_manifest_len_v2,
        runtime_settlement::RUNTIME_SETTLEMENT_EFFECT_HEADER_BYTES_V2,
        runtime_width::{
            CandidateHeaderV2, CandidateLayoutV2, ExecutionHeaderV2, ExecutionV2, PageHeaderV2,
            PageV2, SettlementCursorHeaderV2, SettlementPhaseV2, VerifiedCandidateV2,
            candidate_len, execution_len, page_len, settlement_cursor_len,
        },
        transition_artifacts_v3::general_transition_program_bytes_lean_v3,
    };
    use dclutch_general_codec::successor_request_v3::{ControllerActionV3, ControllerRequestV3};
    use dclutch_general_config_contract::v3::GeneralConfigV3Input;
    use dclutch_transition_vm::v3::{
        ProgramV3, RegisterInput, RegisterOutput, execute_fold_atomic,
    };

    fn put_test(output: &mut [u8], offset: usize, value: &[u8]) {
        output[offset..offset + value.len()].copy_from_slice(value);
    }

    fn materialize_plan(outcome_count: u32) -> Vec<u8> {
        let mut output = vec![
            0_u8;
            RUNTIME_SETTLEMENT_EFFECT_HEADER_BYTES_V2
                + usize::try_from(outcome_count).expect("test count") * 8
        ];
        put_test(&mut output, 0, b"DCGFXP02");
        put_test(&mut output, 8, &2_u16.to_le_bytes());
        output[10] = RuntimeSettlementActionV2::Materialize as u8;
        output[11] = 1;
        put_test(&mut output, 12, &outcome_count.to_le_bytes());
        output[16] = 0b11;
        put_test(&mut output, 24, &7_u64.to_le_bytes());
        put_test(&mut output, 40, &[0x31; 32]);
        put_test(&mut output, 168, &3_u64.to_le_bytes());
        put_test(&mut output, 176, &3_u64.to_le_bytes());
        for item in 0..outcome_count {
            let offset = RUNTIME_SETTLEMENT_EFFECT_HEADER_BYTES_V2
                + usize::try_from(item).expect("item") * 8;
            put_test(&mut output, offset, &3_u64.to_le_bytes());
        }
        RuntimeSettlementEffectPlanV2::decode(&output).expect("canonical plan");
        output
    }

    fn materialize_cursor(outcome_count: u32) -> Vec<u8> {
        let mut output = vec![0; settlement_cursor_len(outcome_count).expect("cursor width")];
        SettlementCursorV2::encode_into(
            SettlementCursorHeaderV2 {
                outcome_count,
                order_count: 1,
                next_order: 0,
                revision: 8,
                candidate_id: [0x31; 32],
                quote_inventory: 0,
                complete_set_quantity: 3,
                terminal_coordinate: 0,
                phase: SettlementPhaseV2::Distributing,
            },
            &vec![3; usize::try_from(outcome_count).expect("test width")],
            &mut output,
        )
        .expect("cursor encode");
        output
    }

    fn initialized_cursor(outcome_count: u32) -> Vec<u8> {
        let mut output = vec![0; settlement_cursor_len(outcome_count).expect("cursor width")];
        SettlementCursorV2::encode_into(
            SettlementCursorHeaderV2 {
                outcome_count,
                order_count: 1,
                next_order: 0,
                revision: 1,
                candidate_id: [0x31; 32],
                quote_inventory: 0,
                complete_set_quantity: 3,
                terminal_coordinate: 0,
                phase: SettlementPhaseV2::Collecting,
            },
            &vec![0; usize::try_from(outcome_count).expect("test width")],
            &mut output,
        )
        .expect("cursor encode");
        output
    }

    fn environment() -> GeneralHotEnvironmentV3 {
        GeneralHotEnvironmentV3 {
            general_root: [21; 32],
            parent_request_digest: [1; 32],
            release_set: [2; 32],
            market: [3; 32],
            product_record_digest: [4; 32],
            general_config_id: [22; 32],
            semantic_basis_id: [5; 32],
            linked_basis_record_digest: [6; 32],
            realm: [7; 32],
            trading_program: [8; 32],
            generation: 9,
            page_index: 0,
            execution_index: 0,
            claims_market_revision: 10,
            owner_position_revision: 11,
            settlement_position_revision: 0,
            settlement_position_present: false,
            close_settlement_position: false,
            settlement_position_owner: [12; 32],
            rent_credit: [13; 32],
            rent_program: [14; 32],
            observed_position_lamports: 20,
            observed_admission_lamports: 21,
            position_rent_principal: 18,
            admission_rent_principal: 19,
            custody_source: [15; 32],
            custody_destination: [16; 32],
            custody_source_owner: [0; 32],
            custody_destination_owner: [0; 32],
            source_vault_context: [17; 32],
            destination_vault_context: [18; 32],
            mint: [19; 32],
            token_program: [20; 32],
            payer: [0; 32],
            rent_refund: [0; 32],
            custody_expected_revision: 22,
            transfer_index: 1,
            custody_replay_rent_principal: 23,
            custody_vault_rent_principal: 24,
        }
    }

    /// A BANK IS REFUSED BY NAME WHEN IT IS NOT THE WIDTH ITS ACTION DECLARES.
    ///
    /// `general_hot_environment_from_bank_v3` runs for every action before the
    /// accelerator dispatches on one, and the scalar count it derives is the
    /// OFFSET at which the identity bank begins. Before the stride became
    /// per-action every action agreed on that offset, so nothing could
    /// disagree; now two can, and a disagreement that got past this check would
    /// not fail -- it would read identities from the wrong offset and hand back
    /// a well-formed environment assembled from the wrong bytes.
    ///
    /// Both directions, and each with its own positive control: the same bytes
    /// read as the action they were built for are accepted, so this is a test
    /// of the stride and not of a malformed buffer.
    #[test]
    fn a_bank_built_for_another_actions_stride_refuses_by_name() {
        for (built_for, read_as) in [
            (Action::OpenBatch, Action::Consider),
            (Action::Consider, Action::OpenBatch),
            (Action::CloseBatch, Action::Collect),
        ] {
            let bank = authenticated_input(built_for, 4, environment());
            // The control is stated against THIS refusal, not against success:
            // a bare fixture bank refuses further down for reasons of content,
            // and a control that demanded `is_ok` would be testing the fixture
            // rather than the stride.
            assert_ne!(
                general_hot_environment_from_bank_v3(built_for, &bank, 4).map(|_| ()),
                Err(GeneralHotCandidateErrorV3::BankStrideMismatch),
                "{built_for:?} must not call its own bank the wrong width"
            );
            assert_eq!(
                general_hot_environment_from_bank_v3(read_as, &bank, 4).map(|_| ()),
                Err(GeneralHotCandidateErrorV3::BankStrideMismatch),
                "a {built_for:?} bank read as {read_as:?} must refuse by name"
            );
        }
    }

    fn authenticated_input(
        action: Action,
        outcome_count: u32,
        environment: GeneralHotEnvironmentV3,
    ) -> Vec<u8> {
        let len = general_hot_candidate_bank_len_v3(action, outcome_count).expect("bank length");
        let mut input = vec![0x7a; len];
        write_scalar(&mut input, scalar::OUTCOME_COUNT, u64::from(outcome_count))
            .expect("tail witness");
        write_identity(
            &mut input,
            general_hot_scalar_count_v3(action, outcome_count).expect("scalar count"),
            identity::PARENT_REQUEST_DIGEST,
            environment.parent_request_digest,
        )
        .expect("parent witness");
        input
    }

    fn open_batch_config(environment: GeneralHotEnvironmentV3) -> GeneralConfigV3 {
        GeneralConfigV3::new(GeneralConfigV3Input {
            capacity_profile_id: [0x61; 32],
            claim_basis_id: environment.semantic_basis_id,
            program_set_id: [0x62; 32],
            generation: environment.generation,
            price_scale: 1_000,
            collection_slots: 10,
            selection_slots: 20,
            settlement_slots: 30,
            max_orders_per_candidate: 8,
            max_pages_per_candidate: 4,
            continuation_reward_lamports: 5,
            selection_policy_id: [0x63; 32],
            quote_surplus_beneficiary: [0x64; 32],
        })
        .expect("valid General config")
    }

    fn open_batch_input(
        outcome_count: u32,
        environment: GeneralHotEnvironmentV3,
        config: GeneralConfigV3,
        root: GeneralRootV2,
    ) -> Vec<u8> {
        let mut input = authenticated_input(Action::OpenBatch, outcome_count, environment);
        for (coordinate, value) in [
            (scalar::CURRENT_SLOT, 100),
            (scalar::ROOT_EXPECTED_REVISION, root.revision()),
            (scalar::ROOT_REVISION_OBSERVATION, root.revision()),
            (
                scalar::ROOT_NEXT_BATCH_SEQUENCE_OBSERVATION,
                root.next_batch_sequence(),
            ),
            (scalar::ROOT_OPEN_BATCHES_OBSERVATION, root.open_batches()),
            (
                scalar::ROOT_LIFECYCLE_OBSERVATION,
                u64::from(root.lifecycle().tag()),
            ),
            (scalar::ZERO, u64::from(outcome_count)),
            (scalar::CONFIG_COLLECTION_SLOTS, config.collection_slots()),
            (scalar::CONFIG_SELECTION_SLOTS, config.selection_slots()),
            (scalar::CONFIG_SETTLEMENT_SLOTS, config.settlement_slots()),
            (
                scalar::CONFIG_MAX_ORDERS,
                u64::from(config.max_orders_per_candidate()),
            ),
            (scalar::SELECTION_PRICE_SCALE, config.price_scale()),
            (scalar::GENERATION, config.generation()),
            (scalar::STATE_BUMP, 7),
            (scalar::PRIMARY_CANONICAL_BUMP, 7),
            (scalar::PRIMARY_RENT_PRINCIPAL, 1),
        ] {
            write_scalar(&mut input, coordinate, value).expect("opening scalar");
        }
        let scalar_count =
            general_hot_scalar_count_v3(Action::OpenBatch, outcome_count).expect("scalar count");
        for (coordinate, value) in [
            (identity::MARKET, root.market()),
            (identity::GENERAL_CONFIG_ID, root.config_id()),
            (identity::SELECTION_PRODUCT, [0x65; 32]),
            (identity::PRIMARY_OWNER, environment.trading_program),
            (identity::TRADING_PROGRAM, environment.trading_program),
        ] {
            write_identity(&mut input, scalar_count, coordinate, value).expect("opening identity");
        }
        input
    }

    fn opened_batch_id(
        outcome_count: u32,
        environment: GeneralHotEnvironmentV3,
        config: GeneralConfigV3,
        root: GeneralRootV2,
    ) -> [u8; 32] {
        let mut root = root;
        let revision = root.revision();
        let sequence = root.next_batch_sequence();
        GeneralBatchV1::open(
            &mut root,
            GeneralBatchOpeningV1 {
                outcome_count,
                sequence,
                generation: environment.generation,
                market: environment.market,
                product_id: [0x65; 32],
                config_id: environment.general_config_id,
                price_scale: config.price_scale(),
                collection_close_slot: 110,
                settlement_close_slot: 160,
                max_orders: config.max_orders_per_candidate(),
            },
            revision,
            100,
        )
        .expect("expected batch opening")
        .batch_id()
    }

    fn opened_batch(
        outcome_count: u32,
        environment: GeneralHotEnvironmentV3,
        config: GeneralConfigV3,
    ) -> (GeneralRootV2, GeneralBatchV1) {
        let mut root = GeneralRootV2::active(
            environment.market,
            environment.general_config_id,
            environment.generation,
        )
        .expect("active root");
        let revision = root.revision();
        let sequence = root.next_batch_sequence();
        let batch = GeneralBatchV1::open(
            &mut root,
            GeneralBatchOpeningV1 {
                outcome_count,
                sequence,
                generation: environment.generation,
                market: environment.market,
                product_id: [0x65; 32],
                config_id: environment.general_config_id,
                price_scale: config.price_scale(),
                collection_close_slot: 110,
                settlement_close_slot: 160,
                max_orders: config.max_orders_per_candidate(),
            },
            revision,
            100,
        )
        .expect("expected batch opening");
        (root, batch)
    }

    fn close_batch_input(
        outcome_count: u32,
        environment: GeneralHotEnvironmentV3,
        root: GeneralRootV2,
        batch: GeneralBatchV1,
        current_slot: u64,
    ) -> Vec<u8> {
        let mut input = authenticated_input(Action::CloseBatch, outcome_count, environment);
        let opening = batch.opening();
        let state = batch.state();
        for (coordinate, value) in [
            (scalar::CURRENT_SLOT, current_slot),
            (scalar::ROOT_EXPECTED_REVISION, root.revision()),
            (scalar::ROOT_REVISION_OBSERVATION, root.revision()),
            (scalar::ROOT_OPEN_BATCHES_OBSERVATION, root.open_batches()),
            (
                scalar::ROOT_LIFECYCLE_OBSERVATION,
                u64::from(root.lifecycle().tag()),
            ),
            (scalar::ZERO, u64::from(outcome_count)),
            (
                scalar::BATCH_STATUS_OBSERVATION,
                u64::from(state.status.tag()),
            ),
            (
                scalar::BATCH_ORDER_COUNT_OBSERVATION,
                u64::from(state.order_count),
            ),
            (
                scalar::BATCH_COLLECTION_CLOSE_SLOT,
                opening.collection_close_slot,
            ),
            (scalar::CONFIG_MAX_ORDERS, u64::from(opening.max_orders)),
            (scalar::STATE_BUMP, 7),
            (scalar::PRIMARY_CANONICAL_BUMP, 7),
            (scalar::PRIMARY_RENT_PRINCIPAL, 1),
        ] {
            write_scalar(&mut input, coordinate, value).expect("closing scalar");
        }
        let scalar_count =
            general_hot_scalar_count_v3(Action::CloseBatch, outcome_count).expect("scalar count");
        for (coordinate, value) in [
            (identity::MARKET, root.market()),
            (identity::GENERAL_CONFIG_ID, root.config_id()),
            (identity::SELECTION_PRODUCT, opening.product_id),
            (identity::PRIMARY_OWNER, environment.trading_program),
            (identity::TRADING_PROGRAM, environment.trading_program),
        ] {
            write_identity(&mut input, scalar_count, coordinate, value).expect("closing identity");
        }
        input
    }

    fn placed_order_bytes(
        outcome_count: u32,
        environment: GeneralHotEnvironmentV3,
        batch: GeneralBatchV1,
        current_slot: u64,
    ) -> Vec<u8> {
        let count = usize::try_from(outcome_count).expect("test count");
        let mut output = vec![0; general_order_len_v1(outcome_count).expect("order width")];
        GeneralOrderV1::encode_into(
            GeneralOrderHeaderV1 {
                outcome_count,
                nonce: 5,
                owner_id: [0x70; 32],
                market: environment.market,
                batch_id: batch.batch_id(),
                generation: environment.generation,
                max_lots: 2,
                max_quote_debit_per_lot: 3,
                valid_until_slot: batch.opening().settlement_close_slot,
            },
            &vec![1; count],
            &vec![2; count],
            GeneralOrderStateV1 {
                phase: GeneralOrderPhaseV1::Placed,
                admitted_slot: current_slot,
                released_slot: 0,
            },
            &mut output,
        )
        .expect("canonical order");
        output
    }

    fn place_order_input(
        outcome_count: u32,
        environment: GeneralHotEnvironmentV3,
        root: GeneralRootV2,
        batch: GeneralBatchV1,
        order: GeneralOrderV1<'_>,
        current_slot: u64,
    ) -> Vec<u8> {
        let mut input = authenticated_input(Action::PlaceOrder, outcome_count, environment);
        let opening = batch.opening();
        let state = batch.state();
        let header = order.header();
        for (coordinate, value) in [
            (scalar::CURRENT_SLOT, current_slot),
            (scalar::ZERO, u64::from(outcome_count)),
            (scalar::SCRATCH_A, u64::from(outcome_count)),
            (
                scalar::ROOT_LIFECYCLE_OBSERVATION,
                u64::from(root.lifecycle().tag()),
            ),
            (
                scalar::BATCH_STATUS_OBSERVATION,
                u64::from(state.status.tag()),
            ),
            (
                scalar::BATCH_ORDER_COUNT_OBSERVATION,
                u64::from(state.order_count),
            ),
            (
                scalar::BATCH_QUOTE_RESERVE_OBSERVATION,
                state.committed_quote_reserve,
            ),
            (
                scalar::BATCH_COLLECTION_CLOSE_SLOT,
                opening.collection_close_slot,
            ),
            (
                scalar::BATCH_SETTLEMENT_CLOSE_SLOT,
                opening.settlement_close_slot,
            ),
            (scalar::CONFIG_MAX_ORDERS, u64::from(opening.max_orders)),
            (scalar::ORDER_NONCE, header.nonce),
            (scalar::ORDER_MAX_LOTS, header.max_lots),
            (
                scalar::ORDER_MAX_QUOTE_DEBIT_PER_LOT,
                header.max_quote_debit_per_lot,
            ),
            (scalar::ORDER_VALID_UNTIL_SLOT, header.valid_until_slot),
            (scalar::GENERATION, header.generation),
            (scalar::STATE_BUMP, 7),
            (scalar::PRIMARY_CANONICAL_BUMP, 7),
            (scalar::PRIMARY_RENT_PRINCIPAL, 1),
            (scalar::TERMINAL_RECORD_BUMP, 8),
            (scalar::TERMINAL_CANONICAL_BUMP, 8),
            (scalar::TERMINAL_RENT_PRINCIPAL, 1),
            (
                scalar::CLAIMS_MARKET_REVISION,
                environment.claims_market_revision,
            ),
        ] {
            write_scalar(&mut input, coordinate, value).expect("place scalar");
        }
        for item in 0..outcome_count {
            let base = GENERAL_HOT_COMMON_SCALARS_V3 + item * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
            write_scalar(&mut input, base + item_scalar::OUTCOME, u64::from(item))
                .expect("outcome");
            write_scalar(
                &mut input,
                base + item_scalar::CURSOR_INVENTORY,
                order.receive_per_lot(item).expect("receive"),
            )
            .expect("receive row");
            write_scalar(
                &mut input,
                base + item_scalar::QUANTITY,
                order.deliver_per_lot(item).expect("deliver"),
            )
            .expect("deliver row");
        }
        let scalar_count =
            general_hot_scalar_count_v3(Action::PlaceOrder, outcome_count).expect("scalar count");
        for (coordinate, value) in [
            (identity::MARKET, root.market()),
            (identity::GENERAL_CONFIG_ID, root.config_id()),
            (identity::SELECTION_PRODUCT, opening.product_id),
            (identity::SELECTION_BATCH, batch.batch_id()),
            (identity::CANDIDATE, batch.batch_id()),
            (identity::OWNER, header.owner_id),
            (identity::ORDER, order.order_id()),
            (identity::PRIMARY_OWNER, environment.trading_program),
            (identity::TERMINAL_OWNER, environment.trading_program),
            (identity::TERMINAL_BENEFICIARY_OBSERVATION, header.owner_id),
            (
                identity::DESTINATION_VAULT_CONTEXT,
                environment.destination_vault_context,
            ),
            (
                identity::CUSTODY_SOURCE_OWNER,
                environment.custody_source_owner,
            ),
            (identity::POSITION_ZERO_OWNER, header.owner_id),
            (identity::POSITION_ONE_OWNER, order.order_id()),
            (
                identity::SETTLEMENT_POSITION_OWNER,
                environment.settlement_position_owner,
            ),
            (identity::RENT_CREDIT, environment.rent_credit),
            (identity::TRADING_PROGRAM, environment.trading_program),
        ] {
            write_identity(&mut input, scalar_count, coordinate, value).expect("place identity");
        }
        input
    }

    fn admitted_batch_and_order(
        outcome_count: u32,
        environment: GeneralHotEnvironmentV3,
        config: GeneralConfigV3,
    ) -> (GeneralRootV2, GeneralBatchV1, Vec<u8>) {
        let (root, mut batch) = opened_batch(outcome_count, environment, config);
        let order_bytes = placed_order_bytes(outcome_count, environment, batch, 101);
        let order = GeneralOrderV1::decode(&order_bytes).expect("order");
        let claims = vec![4; usize::try_from(outcome_count).expect("test count")];
        batch
            .admit(
                order,
                MakerFundingV1 {
                    owner_id: order.header().owner_id,
                    available_quote: 6,
                    available_claims: &claims,
                },
                101,
            )
            .expect("admit order");
        (root, batch, order_bytes)
    }

    fn cancel_order_input(
        outcome_count: u32,
        environment: GeneralHotEnvironmentV3,
        root: GeneralRootV2,
        batch: GeneralBatchV1,
        order: GeneralOrderV1<'_>,
        current_slot: u64,
    ) -> Vec<u8> {
        let mut input = authenticated_input(Action::CancelOrder, outcome_count, environment);
        let opening = batch.opening();
        let state = batch.state();
        let header = order.header();
        let order_state = order.state();
        for (coordinate, value) in [
            (scalar::CURRENT_SLOT, current_slot),
            (scalar::ZERO, u64::from(outcome_count)),
            (scalar::SCRATCH_A, u64::from(outcome_count)),
            (
                scalar::ROOT_LIFECYCLE_OBSERVATION,
                u64::from(root.lifecycle().tag()),
            ),
            (
                scalar::BATCH_STATUS_OBSERVATION,
                u64::from(state.status.tag()),
            ),
            (
                scalar::BATCH_ORDER_COUNT_OBSERVATION,
                u64::from(state.order_count),
            ),
            (
                scalar::BATCH_CANCELLED_COUNT_OBSERVATION,
                u64::from(state.cancelled_count),
            ),
            (
                scalar::BATCH_QUOTE_RESERVE_OBSERVATION,
                state.committed_quote_reserve,
            ),
            (
                scalar::BATCH_COLLECTION_CLOSE_SLOT,
                opening.collection_close_slot,
            ),
            (
                scalar::ORDER_PHASE_OBSERVATION,
                u64::from(order_state.phase.tag()),
            ),
            (
                scalar::ORDER_ADMITTED_SLOT_OBSERVATION,
                order_state.admitted_slot,
            ),
            (scalar::ORDER_MAX_LOTS, header.max_lots),
            (
                scalar::ORDER_MAX_QUOTE_DEBIT_PER_LOT,
                header.max_quote_debit_per_lot,
            ),
            (scalar::ORDER_NONCE, header.nonce),
            (scalar::STATE_BUMP, 7),
            (scalar::PRIMARY_CANONICAL_BUMP, 7),
            (scalar::PRIMARY_RENT_PRINCIPAL, 1),
            (scalar::TERMINAL_RECORD_BUMP, 8),
            (scalar::TERMINAL_CANONICAL_BUMP, 8),
            (scalar::TERMINAL_RENT_PRINCIPAL, 1),
            (scalar::POSITION_ZERO_REVISION, 4),
            (
                scalar::CLAIMS_MARKET_REVISION,
                environment.claims_market_revision,
            ),
            (
                scalar::CUSTODY_EXPECTED_REVISION,
                environment.custody_expected_revision,
            ),
        ] {
            write_scalar(&mut input, coordinate, value).expect("cancel scalar");
        }
        let scalar_count =
            general_hot_scalar_count_v3(Action::CancelOrder, outcome_count).expect("scalar count");
        for (coordinate, value) in [
            (identity::MARKET, root.market()),
            (identity::GENERAL_CONFIG_ID, root.config_id()),
            (identity::SELECTION_PRODUCT, opening.product_id),
            (identity::SELECTION_BATCH, batch.batch_id()),
            (identity::CANDIDATE, batch.batch_id()),
            (identity::OWNER, header.owner_id),
            (identity::ORDER, order.order_id()),
            (identity::PRIMARY_OWNER, environment.trading_program),
            (identity::TERMINAL_OWNER, environment.trading_program),
            (
                identity::SOURCE_VAULT_CONTEXT,
                environment.source_vault_context,
            ),
            (
                identity::CUSTODY_DESTINATION_OWNER,
                environment.custody_destination_owner,
            ),
            (identity::POSITION_ZERO_OWNER, order.order_id()),
            (identity::POSITION_ONE_OWNER, header.owner_id),
            (
                identity::SETTLEMENT_POSITION_OWNER,
                environment.settlement_position_owner,
            ),
            (identity::RENT_CREDIT, environment.rent_credit),
            (identity::RENT_REFUND, environment.rent_refund),
            (identity::TRADING_PROGRAM, environment.trading_program),
        ] {
            write_identity(&mut input, scalar_count, coordinate, value).expect("cancel identity");
        }
        input
    }

    fn release_order_input(
        outcome_count: u32,
        environment: GeneralHotEnvironmentV3,
        root: GeneralRootV2,
        order: GeneralOrderV1<'_>,
        current_slot: u64,
        observed_quote: u64,
    ) -> Vec<u8> {
        let mut input = authenticated_input(Action::ReleaseOrder, outcome_count, environment);
        let header = order.header();
        let state = order.state();
        for (coordinate, value) in [
            (scalar::CURRENT_SLOT, current_slot),
            (scalar::ZERO, u64::from(outcome_count)),
            (
                scalar::ROOT_LIFECYCLE_OBSERVATION,
                u64::from(root.lifecycle().tag()),
            ),
            (
                scalar::ORDER_PHASE_OBSERVATION,
                u64::from(state.phase.tag()),
            ),
            (scalar::ORDER_ADMITTED_SLOT_OBSERVATION, state.admitted_slot),
            (scalar::ORDER_VALID_UNTIL_SLOT, header.valid_until_slot),
            (scalar::ORDER_MAX_LOTS, header.max_lots),
            (
                scalar::ORDER_MAX_QUOTE_DEBIT_PER_LOT,
                header.max_quote_debit_per_lot,
            ),
            (scalar::ORDER_NONCE, header.nonce),
            (scalar::ESCROW_BALANCE_OBSERVATION, observed_quote),
            (scalar::GENERATION, header.generation),
            (scalar::STATE_BUMP, 7),
            (scalar::PRIMARY_CANONICAL_BUMP, 7),
            (scalar::PRIMARY_RENT_PRINCIPAL, 1),
            (scalar::POSITION_ZERO_REVISION, 4),
            (
                scalar::CLAIMS_MARKET_REVISION,
                environment.claims_market_revision,
            ),
            (
                scalar::CUSTODY_EXPECTED_REVISION,
                environment.custody_expected_revision,
            ),
        ] {
            write_scalar(&mut input, coordinate, value).expect("release scalar");
        }
        for item in 0..outcome_count {
            let base = GENERAL_HOT_COMMON_SCALARS_V3 + item * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
            write_scalar(&mut input, base + item_scalar::OUTCOME, u64::from(item))
                .expect("outcome");
            write_scalar(&mut input, base + item_scalar::QUANTITY, 1).expect("residual claim");
        }
        let scalar_count =
            general_hot_scalar_count_v3(Action::ReleaseOrder, outcome_count).expect("scalar count");
        for (coordinate, value) in [
            (identity::MARKET, root.market()),
            (identity::CANDIDATE, header.batch_id),
            (identity::OWNER, header.owner_id),
            (identity::ORDER, order.order_id()),
            (identity::PRIMARY_OWNER, environment.trading_program),
            (
                identity::SOURCE_VAULT_CONTEXT,
                environment.source_vault_context,
            ),
            (
                identity::CUSTODY_DESTINATION_OWNER,
                environment.custody_destination_owner,
            ),
            (identity::POSITION_ZERO_OWNER, order.order_id()),
            (identity::POSITION_ONE_OWNER, header.owner_id),
            (
                identity::SETTLEMENT_POSITION_OWNER,
                environment.settlement_position_owner,
            ),
            (identity::RENT_CREDIT, environment.rent_credit),
            (identity::RENT_REFUND, environment.rent_refund),
            (identity::TRADING_PROGRAM, environment.trading_program),
        ] {
            write_identity(&mut input, scalar_count, coordinate, value).expect("release identity");
        }
        input
    }

    #[derive(Clone)]
    struct SubmitCandidateFixture {
        root: GeneralRootV2,
        batch: GeneralBatchV1,
        config: GeneralConfigV3,
        environment: GeneralHotEnvironmentV3,
        candidate_body: Vec<u8>,
        submission_body: Vec<u8>,
        candidate_id: [u8; 32],
        bank: Vec<u8>,
    }

    fn submit_candidate_fixture(outcome_count: u32, current_slot: u64) -> SubmitCandidateFixture {
        let mut environment = environment();
        let config = open_batch_config(environment);
        let (mut root, mut batch) = opened_batch(outcome_count, environment, config);
        let root_revision = root.revision();
        batch
            .close(&mut root, root_revision)
            .expect("closed candidate batch");
        let opening = batch.opening();
        environment.product_record_digest = opening.product_id;

        let count = usize::try_from(outcome_count).expect("test outcome count");
        let mut prices = vec![0_u64; count];
        prices[0] = opening.price_scale;
        let mut candidate_body = vec![0_u8; candidate_len(outcome_count).expect("candidate width")];
        let mut header = CandidateHeaderV2 {
            outcome_count,
            page_count: 1,
            candidate_coordinate: 1,
            price_scale: opening.price_scale,
            candidate_id: [0x71; 32],
            product_id: opening.product_id,
            batch_id: batch.batch_id(),
        };
        CandidateV2::encode_into(header, &prices, &mut candidate_body).expect("candidate draft");
        header.candidate_id =
            general_candidate_identity_v1(&candidate_body).expect("masked candidate identity");
        CandidateV2::encode_into(header, &prices, &mut candidate_body)
            .expect("canonical candidate");
        let candidate_record = CandidateV2::decode(&candidate_body).expect("candidate");
        let solver = [0x72; 32];
        let row_count = outcome_count;
        let reward_rate = 7_u64;
        let work_capacity = u64::from(row_count)
            .checked_add(2)
            .and_then(|cranks| cranks.checked_mul(reward_rate))
            .expect("work capacity");
        let submission = GeneralCandidateV1::submit(
            batch,
            candidate_record,
            9,
            row_count,
            reward_rate,
            solver,
            work_capacity,
            current_slot,
        )
        .expect("canonical submission");
        let submitted_opening = submission.opening();
        let submitted_state = submission.state();
        let mut bank = authenticated_input(Action::SubmitCandidate, outcome_count, environment);
        for (coordinate, value) in [
            (scalar::CURRENT_SLOT, current_slot),
            (scalar::ZERO, u64::from(outcome_count)),
            (
                scalar::ROOT_LIFECYCLE_OBSERVATION,
                u64::from(root.lifecycle().tag()),
            ),
            (
                scalar::BATCH_STATUS_OBSERVATION,
                u64::from(batch.state().status.tag()),
            ),
            (scalar::BATCH_POST_ORDER_COUNT, u64::from(outcome_count)),
            (
                scalar::BATCH_COLLECTION_CLOSE_SLOT,
                opening.collection_close_slot,
            ),
            (
                scalar::BATCH_SETTLEMENT_CLOSE_SLOT,
                opening.settlement_close_slot,
            ),
            (scalar::ORDER_MAX_LOTS, opening.price_scale),
            (scalar::CANDIDATE_PAGE_COUNT, u64::from(header.page_count)),
            (
                scalar::SELECTION_BEST_CANDIDATE_COORDINATE,
                u64::from(header.candidate_coordinate),
            ),
            (scalar::SELECTION_PRICE_SCALE, header.price_scale),
            (
                scalar::VERIFY_POST_ORDER_COUNT,
                u64::from(submitted_opening.outcome_count),
            ),
            (
                scalar::VERIFY_POST_PAGE,
                u64::from(submitted_opening.page_count),
            ),
            (
                scalar::CANDIDATE_STATUS_OBSERVATION,
                u64::from(submitted_state.status.tag()),
            ),
            (
                scalar::CANDIDATE_PAGE_REVISION,
                submitted_opening.page_revision,
            ),
            (
                scalar::CANDIDATE_SUBMITTED_SLOT,
                submitted_opening.submitted_slot,
            ),
            (
                scalar::CANDIDATE_ROW_COUNT,
                u64::from(submitted_opening.row_count),
            ),
            (
                scalar::CANDIDATE_REWARD_RATE,
                submitted_opening.reward_rate_lamports,
            ),
            (
                scalar::CANDIDATE_VERIFICATION_REMAINING_OBSERVATION,
                submitted_state.verification_remaining,
            ),
            (
                scalar::CANDIDATE_CLEANUP_REMAINING_OBSERVATION,
                submitted_state.cleanup_remaining,
            ),
            (scalar::PRIMARY_BUMP_OBSERVATION, 0),
            (scalar::PRIMARY_PRINCIPAL_OBSERVATION, 0),
            (scalar::PRIMARY_CREATED, 1),
            (scalar::STATE_BUMP, 7),
            (scalar::PRIMARY_CANONICAL_BUMP, 7),
            (scalar::PRIMARY_RENT_PRINCIPAL, 1_000),
        ] {
            write_scalar(&mut bank, coordinate, value).expect("submission scalar");
        }
        for item in 0..outcome_count {
            let base = GENERAL_HOT_COMMON_SCALARS_V3 + item * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
            write_scalar(&mut bank, base + item_scalar::OUTCOME, u64::from(item))
                .expect("canonical outcome index");
        }
        let scalar_count = general_hot_scalar_count_v3(Action::SubmitCandidate, outcome_count)
            .expect("scalar count");
        for (coordinate, value) in [
            (identity::CANDIDATE, header.candidate_id),
            (identity::BEST_VERIFIED_DIGEST, header.candidate_id),
            (identity::ORDER, header.product_id),
            (identity::SELECTION_POLICY, header.batch_id),
            (identity::SELECTION_PRODUCT, opening.product_id),
            // AccountProfile initially projects the closed evidence account
            // key here. The semantic projector replaces it with the Batch
            // content identity after authenticating the body.
            (identity::SELECTION_BATCH, [0x73; 32]),
            (
                identity::RESULT_BENEFICIARY_OBSERVATION,
                submitted_opening.candidate_id,
            ),
            (identity::BENEFICIARY, submitted_opening.batch_id),
            (identity::OWNER, submitted_opening.solver_id),
            // The creation payer the AccountProfile projects, which the
            // authored transition now joins to the solver the candidate names.
            (identity::PAYER, submitted_opening.solver_id),
            (identity::PRIMARY_BENEFICIARY_OBSERVATION, [0; 32]),
            (identity::PRIMARY_BENEFICIARY, submitted_opening.solver_id),
            (identity::PRIMARY_OWNER, environment.trading_program),
            (identity::TRADING_PROGRAM, environment.trading_program),
            (identity::GENERAL_ROOT, environment.general_root),
        ] {
            write_identity(&mut bank, scalar_count, coordinate, value)
                .expect("submission identity");
        }
        SubmitCandidateFixture {
            root,
            batch,
            config,
            environment,
            candidate_body,
            submission_body: submission.to_bytes().to_vec(),
            candidate_id: header.candidate_id,
            bank,
        }
    }

    fn project_submit(fixture: &mut SubmitCandidateFixture) -> Result<()> {
        project_general_submit_candidate_in_place_v3(
            &fixture.root.to_bytes(),
            &fixture.batch.to_bytes(),
            fixture.config,
            &fixture.candidate_body,
            &fixture.submission_body,
            fixture.batch.opening().outcome_count,
            fixture.environment,
            Some(fixture.candidate_id),
            &mut fixture.bank,
        )
    }

    fn close_candidate_fixture(
        outcome_count: u32,
        current_slot: u64,
    ) -> (
        GeneralBatchV1,
        GeneralCandidateV1,
        GeneralHotEnvironmentV3,
        Vec<u8>,
        [u8; 64],
    ) {
        let fixture = submit_candidate_fixture(outcome_count, 120);
        let submission = GeneralCandidateV1::decode(&fixture.submission_body).expect("submission");
        let opening = submission.opening();
        let state = submission.state();
        let rent_principal = 1_000_u64;
        let mut bank =
            authenticated_input(Action::CloseCandidate, outcome_count, fixture.environment);
        for (coordinate, value) in [
            (scalar::CURRENT_SLOT, current_slot),
            (
                scalar::ROOT_LIFECYCLE_OBSERVATION,
                u64::from(fixture.root.lifecycle().tag()),
            ),
            (
                scalar::CANDIDATE_STATUS_OBSERVATION,
                u64::from(state.status.tag()),
            ),
            (
                scalar::CANDIDATE_VERIFICATION_REMAINING_OBSERVATION,
                state.verification_remaining,
            ),
            (
                scalar::CANDIDATE_CLEANUP_REMAINING_OBSERVATION,
                state.cleanup_remaining,
            ),
            (scalar::CANDIDATE_REWARD_RATE, opening.reward_rate_lamports),
            (
                scalar::BATCH_SETTLEMENT_CLOSE_SLOT,
                fixture.batch.opening().settlement_close_slot,
            ),
            (
                scalar::BATCH_STATUS_OBSERVATION,
                u64::from(fixture.batch.state().status.tag()),
            ),
            (scalar::PRIMARY_PRINCIPAL_OBSERVATION, rent_principal),
            (scalar::PRIMARY_RENT_PRINCIPAL, rent_principal),
            (
                scalar::OBSERVED_POSITION_LAMPORTS,
                rent_principal + state.verification_remaining + state.cleanup_remaining,
            ),
            (scalar::OBSERVED_ADMISSION_LAMPORTS, 200),
            (scalar::ESCROW_BALANCE_OBSERVATION, 300),
        ] {
            write_scalar(&mut bank, coordinate, value).expect("close scalar");
        }
        let scalar_count = general_hot_scalar_count_v3(Action::CloseCandidate, outcome_count)
            .expect("scalar count");
        for (coordinate, value) in [
            (identity::PARENT_REQUEST_DIGEST, opening.candidate_id),
            (identity::CANDIDATE, opening.candidate_id),
            (identity::SELECTION_BATCH, opening.batch_id),
            (identity::OWNER, opening.solver_id),
            (identity::RENT_CREDIT, opening.solver_id),
            (identity::PRIMARY_BENEFICIARY_OBSERVATION, opening.solver_id),
            (identity::PRIMARY_BENEFICIARY, opening.solver_id),
            (identity::PAYER, [0x81; 32]),
        ] {
            write_identity(&mut bank, scalar_count, coordinate, value).expect("close identity");
        }
        let request = ControllerRequestV3 {
            action: ControllerActionV3::CloseCandidate,
            expected_revision: 0,
            subject_id: Some(opening.candidate_id),
            page_index: 0,
            execution_index: 0,
            manifest_order_index: 0,
            primary_state_bump: 7,
            secondary_state_bump: 0,
            result_state_bump: 0,
        }
        .to_bytes()
        .expect("close request");
        (
            fixture.batch,
            submission,
            fixture.environment,
            bank,
            request,
        )
    }

    #[test]
    fn close_candidate_authenticates_exact_conservation_at_runtime_widths() {
        for outcome_count in [1_u32, 258] {
            let (batch, submission, environment, bank, request) =
                close_candidate_fixture(outcome_count, 160);
            let plan = authenticate_general_close_candidate_v3(
                &request,
                batch,
                submission,
                outcome_count,
                environment,
                &bank,
            )
            .expect("permissionless close after the settlement deadline");
            let state = submission.state();
            assert_eq!(plan.cleanup_reward(), state.cleanup_remaining);
            assert_eq!(plan.solver_credit(), state.verification_remaining + 1_000);
            assert_eq!(
                plan.escrow_before(),
                state.verification_remaining + state.cleanup_remaining + 1_000,
            );
            assert_eq!(plan.cranker_before(), 200);
            assert_eq!(plan.solver_before(), 300);
            assert_eq!(plan.cranker_after(), 200 + state.cleanup_remaining,);
            assert_eq!(
                plan.solver_after(),
                300 + state.verification_remaining + 1_000,
            );
        }
    }

    #[test]
    fn close_candidate_refuses_censorship_substitution_and_unconserved_balance() {
        let outcome_count = 1;
        let (batch, submission, environment, bank, request) =
            close_candidate_fixture(outcome_count, 159);
        assert!(matches!(
            authenticate_general_close_candidate_v3(
                &request,
                batch,
                submission,
                outcome_count,
                environment,
                &bank,
            ),
            Err(GeneralHotCandidateErrorV3::Close(
                GeneralSevenPlanErrorV1::CandidateStillLive
            ))
        ));

        let (_, _, _, mut substituted, _) = close_candidate_fixture(outcome_count, 160);
        let scalar_count = general_hot_scalar_count_v3(Action::CloseCandidate, outcome_count)
            .expect("scalar count");
        write_identity(
            &mut substituted,
            scalar_count,
            identity::SELECTION_BATCH,
            [0x99; 32],
        )
        .expect("substitute batch");
        assert_eq!(
            authenticate_general_close_candidate_v3(
                &request,
                batch,
                submission,
                outcome_count,
                environment,
                &substituted,
            ),
            Err(GeneralHotCandidateErrorV3::InvalidCoordinate),
        );

        let (_, _, _, mut unconserved, _) = close_candidate_fixture(outcome_count, 160);
        let observed = read_scalar(&unconserved, scalar::OBSERVED_POSITION_LAMPORTS)
            .expect("observed balance");
        write_scalar(
            &mut unconserved,
            scalar::OBSERVED_POSITION_LAMPORTS,
            observed + 1,
        )
        .expect("overcapitalized balance");
        assert!(matches!(
            authenticate_general_close_candidate_v3(
                &request,
                batch,
                submission,
                outcome_count,
                environment,
                &unconserved,
            ),
            Err(GeneralHotCandidateErrorV3::Close(
                GeneralSevenPlanErrorV1::Escrow(_)
            ))
        ));
    }

    fn execute_submit_transition(bank: &[u8], outcome_count: u32) {
        let scalar_count = general_hot_scalar_count_v3(Action::SubmitCandidate, outcome_count)
            .expect("scalar count");
        let scalars: Vec<u64> = (0..scalar_count)
            .map(|coordinate| read_scalar(bank, coordinate).expect("projected scalar"))
            .collect();
        let identities: Vec<[u8; 32]> = (0..GENERAL_HOT_COMMON_IDENTITIES_V3)
            .map(|coordinate| {
                read_identity(bank, scalar_count, coordinate).expect("projected identity")
            })
            .collect();
        let program = ProgramV3::decode(general_transition_program_bytes_lean_v3(
            Action::SubmitCandidate,
        ))
        .expect("authored SubmitCandidate transition");
        let mut scalar_scratch = vec![0_u64; scalars.len()];
        let mut scalar_output = vec![0_u64; scalars.len()];
        let mut identity_scratch = vec![[0_u8; 32]; identities.len()];
        let mut identity_output = vec![[0_u8; 32]; identities.len()];
        execute_fold_atomic(
            program,
            outcome_count,
            RegisterInput {
                scalars: &scalars,
                identities: &identities,
            },
            RegisterOutput {
                scalars: &mut scalar_scratch,
                identities: &mut identity_scratch,
            },
            RegisterOutput {
                scalars: &mut scalar_output,
                identities: &mut identity_output,
            },
        )
        .expect("projector output executes the authored transition");
    }

    #[test]
    fn submit_candidate_projects_exact_transition_and_effect_inputs_at_runtime_widths() {
        for outcome_count in [1_u32, 258] {
            let mut fixture = submit_candidate_fixture(outcome_count, 120);
            project_submit(&mut fixture).expect("semantic submission");
            execute_submit_transition(&fixture.bank, outcome_count);
            let expected_work = (u64::from(outcome_count) + 2) * 7;
            assert_eq!(
                read_scalar(&fixture.bank, scalar::ACTION),
                Ok(u64::from(Action::SubmitCandidate as u8)),
            );
            assert_eq!(
                read_scalar(&fixture.bank, scalar::SCRATCH_A),
                Ok(expected_work)
            );
            assert_eq!(
                read_scalar(&fixture.bank, scalar::SCRATCH_B),
                Ok(expected_work + 1_000),
            );
            assert_eq!(
                read_scalar(&fixture.bank, scalar::CANDIDATE_POST_VERIFICATION_REMAINING,),
                Ok((u64::from(outcome_count) + 1) * 7),
            );
            assert_eq!(
                read_scalar(&fixture.bank, scalar::CANDIDATE_POST_CLEANUP_REMAINING),
                Ok(7),
            );
            assert_eq!(
                read_scalar(&fixture.bank, scalar::LOCAL_STATE_KIND),
                Ok(u64::from(GeneralLocalStateKindV3::Candidate.tag())),
            );
            assert_eq!(
                read_identity(
                    &fixture.bank,
                    general_hot_scalar_count_v3(Action::SubmitCandidate, outcome_count)
                        .expect("scalar count"),
                    identity::SELECTION_BATCH,
                ),
                Ok(fixture.batch.batch_id()),
            );
        }
    }

    #[test]
    fn submit_candidate_refuses_noncanonical_content_and_economics_atomically() {
        for outcome_count in [1_u32, 258] {
            let fixture = submit_candidate_fixture(outcome_count, 120);

            let mut underfunded = fixture.clone();
            let offset = GeneralCandidateLayoutV1::VERIFICATION_REMAINING_OFFSET;
            let current = u64::from_le_bytes(
                underfunded.submission_body[offset..offset + 8]
                    .try_into()
                    .expect("verification bytes"),
            );
            put_test(
                &mut underfunded.submission_body,
                offset,
                &(current - 1).to_le_bytes(),
            );
            write_scalar(
                &mut underfunded.bank,
                scalar::CANDIDATE_VERIFICATION_REMAINING_OBSERVATION,
                current - 1,
            )
            .expect("matching hostile projection");
            let before = underfunded.bank.clone();
            assert!(project_submit(&mut underfunded).is_err());
            assert_eq!(underfunded.bank, before);

            let mut forged_identity = fixture.clone();
            put_test(
                &mut forged_identity.candidate_body,
                CandidateLayoutV2::CANDIDATE_ID,
                &[0xee; 32],
            );
            forged_identity.candidate_id = [0xee; 32];
            let scalar_count = general_hot_scalar_count_v3(Action::SubmitCandidate, outcome_count)
                .expect("scalar count");
            for coordinate in [identity::CANDIDATE, identity::BEST_VERIFIED_DIGEST] {
                write_identity(
                    &mut forged_identity.bank,
                    scalar_count,
                    coordinate,
                    [0xee; 32],
                )
                .expect("matching forged projection");
            }
            let before = forged_identity.bank.clone();
            assert!(project_submit(&mut forged_identity).is_err());
            assert_eq!(forged_identity.bank, before);

            let mut wrong_product = fixture.clone();
            wrong_product.environment.product_record_digest = [0xef; 32];
            let before = wrong_product.bank.clone();
            assert!(project_submit(&mut wrong_product).is_err());
            assert_eq!(wrong_product.bank, before);

            let mut wrong_item = fixture.clone();
            let last = GENERAL_HOT_COMMON_SCALARS_V3
                + (outcome_count - 1) * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
            write_scalar(
                &mut wrong_item.bank,
                last + item_scalar::OUTCOME,
                u64::from(outcome_count),
            )
            .expect("out-of-range outcome");
            let before = wrong_item.bank.clone();
            assert!(project_submit(&mut wrong_item).is_err());
            assert_eq!(wrong_item.bank, before);
        }
    }

    #[test]
    fn submit_candidate_window_and_lifecycle_authority_refuse_atomically() {
        for outcome_count in [1_u32, 258] {
            let mut at_collection_close = submit_candidate_fixture(outcome_count, 110);
            project_submit(&mut at_collection_close).expect("inclusive submission boundary");

            let fixture = submit_candidate_fixture(outcome_count, 159);
            for (coordinate, hostile) in [
                (scalar::CURRENT_SLOT, 160),
                (scalar::PRIMARY_CREATED, 0),
                (scalar::PRIMARY_CANONICAL_BUMP, 8),
            ] {
                let mut hostile_fixture = fixture.clone();
                write_scalar(&mut hostile_fixture.bank, coordinate, hostile)
                    .expect("hostile lifecycle scalar");
                let before = hostile_fixture.bank.clone();
                assert!(project_submit(&mut hostile_fixture).is_err());
                assert_eq!(hostile_fixture.bank, before);
            }
            for coordinate in [
                identity::PRIMARY_BENEFICIARY,
                identity::RESULT_BENEFICIARY_OBSERVATION,
            ] {
                let mut hostile_fixture = fixture.clone();
                write_identity(
                    &mut hostile_fixture.bank,
                    general_hot_scalar_count_v3(Action::SubmitCandidate, outcome_count)
                        .expect("scalar count"),
                    coordinate,
                    [0xed; 32],
                )
                .expect("hostile lifecycle identity");
                let before = hostile_fixture.bank.clone();
                assert!(project_submit(&mut hostile_fixture).is_err());
                assert_eq!(hostile_fixture.bank, before);
            }
        }
    }

    #[test]
    fn verify_candidate_projects_the_exact_terminal_cursor_result_and_reward() {
        let outcome_count = 1_u32;
        let current_slot = 120_u64;
        let mut environment = environment();
        let config = open_batch_config(environment);
        let (mut root, mut batch) = opened_batch(outcome_count, environment, config);
        let order_bytes = placed_order_bytes(outcome_count, environment, batch, 101);
        let order = GeneralOrderV1::decode(&order_bytes).expect("escrowed order");
        batch
            .admit(
                order,
                MakerFundingV1 {
                    owner_id: order.header().owner_id,
                    available_quote: 6,
                    available_claims: &[4],
                },
                101,
            )
            .expect("admitted order");
        let root_revision = root.revision();
        batch
            .close(&mut root, root_revision)
            .expect("closed verification batch");
        environment.product_record_digest = batch.opening().product_id;

        let mut candidate_bytes = vec![0_u8; candidate_len(1).expect("candidate width")];
        let mut candidate_header = CandidateHeaderV2 {
            outcome_count: 1,
            page_count: 1,
            candidate_coordinate: 1,
            price_scale: batch.opening().price_scale,
            candidate_id: [0xf1; 32],
            product_id: batch.opening().product_id,
            batch_id: batch.batch_id(),
        };
        CandidateV2::encode_into(
            candidate_header,
            &[batch.opening().price_scale],
            &mut candidate_bytes,
        )
        .expect("candidate draft");
        candidate_header.candidate_id =
            general_candidate_identity_v1(&candidate_bytes).expect("candidate identity");
        CandidateV2::encode_into(
            candidate_header,
            &[batch.opening().price_scale],
            &mut candidate_bytes,
        )
        .expect("canonical candidate");
        let candidate = CandidateV2::decode(&candidate_bytes).expect("candidate");

        let mut execution_bytes = vec![0_u8; execution_len(1).expect("execution width")];
        ExecutionV2::encode_into(
            ExecutionHeaderV2 {
                outcome_count: 1,
                page_coordinate: 1,
                execution_coordinate: 1,
                nonce: order.header().nonce,
                order_id: order.order_id(),
                owner_id: order.header().owner_id,
                max_lots: order.header().max_lots,
                lots: 1,
            },
            &[order.receive_per_lot(0).expect("receive")],
            &[order.deliver_per_lot(0).expect("deliver")],
            &mut execution_bytes,
        )
        .expect("execution row");
        let mut page_bytes = vec![0_u8; page_len(1, 1).expect("page width")];
        PageV2::encode_into(
            PageHeaderV2 {
                outcome_count: 1,
                page_coordinate: 1,
                page_count: 1,
                revision: 9,
                candidate_id: candidate_header.candidate_id,
            },
            &[execution_bytes.as_slice()],
            &mut page_bytes,
        )
        .expect("candidate page");

        let reward_rate = 7_u64;
        let submission = GeneralCandidateV1::submit(
            batch,
            candidate,
            9,
            1,
            reward_rate,
            [0xf2; 32],
            3 * reward_rate,
            current_slot,
        )
        .expect("submitted candidate");
        let view = CandidateVerifyRowViewV1 {
            batch,
            submission,
            candidate: &candidate_bytes,
            page: &page_bytes,
            order: &order_bytes,
            cursor_before: &vec![
                0_u8;
                candidate_verifier_len_v1(submission).expect("verifier width")
            ],
            verified_before: &vec![
                0_u8;
                candidate_certificate_len_v1(submission).expect("result width")
            ],
            expected_page_index: 0,
            expected_row_index: 0,
            expected_revision: 0,
        };
        // Give the borrowed empty states stable storage before using the view.
        let empty_cursor = view.cursor_before.to_vec();
        let empty_result = view.verified_before.to_vec();
        let view = CandidateVerifyRowViewV1 {
            cursor_before: &empty_cursor,
            verified_before: &empty_result,
            ..view
        };
        let manifest_orders =
            candidate_verify_manifest_orders_v1(&view).expect("manifest order count");
        let verifier_len = empty_cursor.len();
        let result_len = empty_result.len();
        let manifest_len =
            settlement_manifest_len_v2(outcome_count, manifest_orders).expect("manifest width");
        let mut cursor_scratch = vec![0_u8; verifier_len];
        let mut cursor_output = vec![0_u8; verifier_len];
        let mut result_scratch = vec![0_u8; result_len];
        let mut result_output = vec![0_u8; result_len];
        let mut manifest_scratch = vec![0_u8; manifest_len];
        let mut manifest_output = vec![0_u8; manifest_len];

        let principal = 1_000_u64;
        let mut input = authenticated_input(Action::VerifyCandidateRow, outcome_count, environment);
        for (coordinate, value) in [
            (scalar::ROOT_EXPECTED_REVISION, 0),
            (scalar::COMPLETE_SET_MOVE, 0),
            (scalar::CLAIMS_AFFINE_ACTIVE, 0),
            (
                scalar::ROOT_LIFECYCLE_OBSERVATION,
                u64::from(GeneralLifecycleV2::Active.tag()),
            ),
            (scalar::PRIMARY_PRINCIPAL_OBSERVATION, principal),
            (
                scalar::OBSERVED_POSITION_LAMPORTS,
                principal
                    + submission.state().verification_remaining
                    + submission.state().cleanup_remaining,
            ),
        ] {
            write_scalar(&mut input, coordinate, value).expect("verify input scalar");
        }
        let scalar_count = general_hot_scalar_count_v3(Action::VerifyCandidateRow, outcome_count)
            .expect("scalar count");
        for (coordinate, value) in [
            (
                identity::PARENT_REQUEST_DIGEST,
                candidate_header.candidate_id,
            ),
            (identity::PAYER, [0xf3; 32]),
            (identity::TRADING_PROGRAM, environment.trading_program),
        ] {
            write_identity(&mut input, scalar_count, coordinate, value)
                .expect("verify input identity");
        }
        let mut scratch = vec![0_u8; input.len()];
        let mut output = vec![0xa5_u8; input.len()];
        let (_, projection) = project_general_verify_candidate_v3(
            view,
            CandidateVerifyRowBuffersV1 {
                cursor_scratch: &mut cursor_scratch,
                cursor_output: &mut cursor_output,
                verified_scratch: &mut result_scratch,
                verified_output: &mut result_output,
                manifest_scratch: &mut manifest_scratch,
                manifest_output: &mut manifest_output,
            },
            outcome_count,
            &input,
            &mut scratch,
            &mut output,
        )
        .expect("terminal verify projection");

        assert!(projection.creates_verified_result());
        assert_eq!(
            projection.summary.submission.state().status,
            GeneralCandidateStatusV1::Verified
        );
        assert_eq!(read_scalar(&output, scalar::VERIFY_TERMINAL), Ok(1));
        assert_eq!(read_scalar(&output, scalar::SCRATCH_A), Ok(reward_rate));
        assert_eq!(
            read_scalar(&output, scalar::SCRATCH_B),
            Ok(principal + 2 * reward_rate)
        );
        assert_eq!(
            read_scalar(&output, scalar::LOCAL_STATE_KIND),
            Ok(u64::from(GeneralLocalStateKindV3::Verifier.tag()))
        );
        let cursor = RuntimeCandidateVerifierV2::decode(&cursor_output).expect("verifier output");
        assert_eq!(cursor.header().revision, 1);
        assert_eq!(cursor.header().candidate_id, candidate_header.candidate_id);
        let result = VerifiedCandidateV2::decode(&result_output).expect("verified result");
        assert_eq!(result.header().candidate_id, candidate_header.candidate_id);
        assert_eq!(
            read_identity(&output, scalar_count, identity::BEST_VERIFIED_DIGEST,),
            Ok(projection.summary.submission.state().verified_digest)
        );

        let mut in_place_cursor_scratch = vec![0_u8; verifier_len];
        let mut in_place_cursor_output = vec![0_u8; verifier_len];
        let mut in_place_result_scratch = vec![0_u8; result_len];
        let mut in_place_result_output = vec![0_u8; result_len];
        let mut in_place_manifest_scratch = vec![0_u8; manifest_len];
        let mut in_place_manifest_output = vec![0_u8; manifest_len];
        let mut in_place_candidate = input.clone();
        let mut in_place_scratch = vec![0_u8; input.len()];
        let in_place_projection = project_general_verify_candidate_in_place_v3(
            CandidateVerifyRowViewV1 {
                batch,
                submission,
                candidate: &candidate_bytes,
                page: &page_bytes,
                order: &order_bytes,
                cursor_before: &empty_cursor,
                verified_before: &empty_result,
                expected_page_index: 0,
                expected_row_index: 0,
                expected_revision: 0,
            },
            CandidateVerifyRowBuffersV1 {
                cursor_scratch: &mut in_place_cursor_scratch,
                cursor_output: &mut in_place_cursor_output,
                verified_scratch: &mut in_place_result_scratch,
                verified_output: &mut in_place_result_output,
                manifest_scratch: &mut in_place_manifest_scratch,
                manifest_output: &mut in_place_manifest_output,
            },
            outcome_count,
            &mut in_place_candidate,
            &mut in_place_scratch,
        )
        .expect("bounded-memory terminal verify projection");
        assert_eq!(in_place_projection, projection);
        assert_eq!(in_place_candidate, output);
        assert_eq!(in_place_cursor_output, cursor_output);
        assert_eq!(in_place_result_output, result_output);
        assert_eq!(in_place_manifest_output, manifest_output);

        let mut hostile_candidate = input.clone();
        write_identity(
            &mut hostile_candidate,
            scalar_count,
            identity::PAYER,
            [0; 32],
        )
        .expect("hostile payer");
        let hostile_before = hostile_candidate.clone();
        let mut hostile_scratch = vec![0_u8; input.len()];
        assert_eq!(
            project_general_verify_candidate_in_place_v3(
                CandidateVerifyRowViewV1 {
                    batch,
                    submission,
                    candidate: &candidate_bytes,
                    page: &page_bytes,
                    order: &order_bytes,
                    cursor_before: &empty_cursor,
                    verified_before: &empty_result,
                    expected_page_index: 0,
                    expected_row_index: 0,
                    expected_revision: 0,
                },
                CandidateVerifyRowBuffersV1 {
                    cursor_scratch: &mut in_place_cursor_scratch,
                    cursor_output: &mut in_place_cursor_output,
                    verified_scratch: &mut in_place_result_scratch,
                    verified_output: &mut in_place_result_output,
                    manifest_scratch: &mut in_place_manifest_scratch,
                    manifest_output: &mut in_place_manifest_output,
                },
                outcome_count,
                &mut hostile_candidate,
                &mut hostile_scratch,
            ),
            Err(GeneralHotCandidateErrorV3::InvalidCoordinate)
        );
        assert_eq!(hostile_candidate, hostile_before);
    }

    #[test]
    fn open_batch_executes_the_real_root_and_batch_transition_at_runtime_widths() {
        for outcome_count in [1_u32, 258] {
            let environment = environment();
            let config = open_batch_config(environment);
            let root = GeneralRootV2::active(
                environment.market,
                environment.general_config_id,
                environment.generation,
            )
            .expect("active root");
            let batch_id = opened_batch_id(outcome_count, environment, config, root);
            let mut candidate = open_batch_input(outcome_count, environment, config, root);
            project_general_open_batch_candidate_in_place_v3(
                &root.to_bytes(),
                config,
                outcome_count,
                environment,
                root.revision(),
                Some(batch_id),
                &mut candidate,
            )
            .expect("semantic opening");
            assert_eq!(read_scalar(&candidate, scalar::ROOT_POST_REVISION), Ok(2));
            assert_eq!(
                read_scalar(&candidate, scalar::ROOT_POST_BATCH_SEQUENCE),
                Ok(1)
            );
            assert_eq!(
                read_scalar(&candidate, scalar::ROOT_POST_OPEN_BATCHES),
                Ok(1)
            );
            assert_eq!(
                read_scalar(&candidate, scalar::BATCH_COLLECTION_CLOSE_SLOT),
                Ok(110)
            );
            assert_eq!(
                read_scalar(&candidate, scalar::BATCH_SETTLEMENT_CLOSE_SLOT),
                Ok(160)
            );
            assert_eq!(
                read_scalar(&candidate, scalar::BATCH_POST_STATUS),
                Ok(u64::from(BatchStatusV1::Collecting.tag()))
            );
            assert_eq!(
                read_scalar(&candidate, scalar::LOCAL_STATE_KIND),
                Ok(u64::from(GeneralLocalStateKindV3::Batch.tag()))
            );
            assert_eq!(
                read_scalar(&candidate, scalar::SCRATCH_A),
                Ok(GeneralBatchLayoutV1::magic_u64())
            );
        }
    }

    #[test]
    fn close_batch_executes_the_real_root_and_batch_transition_at_runtime_widths() {
        for outcome_count in [1_u32, 258] {
            let environment = environment();
            let config = open_batch_config(environment);
            let (root, batch) = opened_batch(outcome_count, environment, config);
            let mut candidate = close_batch_input(
                outcome_count,
                environment,
                root,
                batch,
                batch.opening().collection_close_slot,
            );
            project_general_close_batch_candidate_in_place_v3(
                &root.to_bytes(),
                &batch.to_bytes(),
                config,
                outcome_count,
                environment,
                root.revision(),
                Some(batch.batch_id()),
                &mut candidate,
            )
            .expect("semantic close");
            assert_eq!(read_scalar(&candidate, scalar::ROOT_POST_REVISION), Ok(3));
            assert_eq!(
                read_scalar(&candidate, scalar::ROOT_POST_OPEN_BATCHES),
                Ok(0)
            );
            assert_eq!(
                read_scalar(&candidate, scalar::BATCH_POST_STATUS),
                Ok(u64::from(BatchStatusV1::Closed.tag()))
            );
            assert_eq!(read_scalar(&candidate, scalar::SCRATCH_A), Ok(0));
            assert_eq!(read_scalar(&candidate, scalar::SCRATCH_B), Ok(0));
            assert_eq!(
                read_scalar(&candidate, scalar::LOCAL_STATE_KIND),
                Ok(u64::from(GeneralLocalStateKindV3::Batch.tag()))
            );
        }
    }

    #[test]
    fn close_batch_before_the_window_refuses_without_candidate_mutation() {
        let outcome_count = 1;
        let environment = environment();
        let config = open_batch_config(environment);
        let (root, batch) = opened_batch(outcome_count, environment, config);
        let mut candidate = close_batch_input(
            outcome_count,
            environment,
            root,
            batch,
            batch.opening().collection_close_slot - 1,
        );
        let before = candidate.clone();
        assert_eq!(
            project_general_close_batch_candidate_in_place_v3(
                &root.to_bytes(),
                &batch.to_bytes(),
                config,
                outcome_count,
                environment,
                root.revision(),
                Some(batch.batch_id()),
                &mut candidate,
            ),
            Err(GeneralHotCandidateErrorV3::InvalidCoordinate)
        );
        assert_eq!(candidate, before);
    }

    #[test]
    fn place_order_executes_batch_admission_and_projects_exact_escrow_at_runtime_widths() {
        for outcome_count in [1_u32, 258] {
            let mut environment = environment();
            let config = open_batch_config(environment);
            let (root, batch) = opened_batch(outcome_count, environment, config);
            let current_slot = 101;
            let order_bytes = placed_order_bytes(outcome_count, environment, batch, current_slot);
            let order = GeneralOrderV1::decode(&order_bytes).expect("order");
            environment.destination_vault_context = order.order_id();
            environment.custody_source_owner = order.header().owner_id;
            environment.settlement_position_owner = order.order_id();
            environment.rent_credit = order.header().owner_id;
            let mut candidate =
                place_order_input(outcome_count, environment, root, batch, order, current_slot);
            let mut signed_terms =
                vec![0; general_signed_order_terms_len_v1(outcome_count).expect("signed width")];
            order
                .encode_signed_terms_into(&mut signed_terms)
                .expect("signed immutable terms");
            project_general_place_order_candidate_in_place_v3(
                &root.to_bytes(),
                &batch.to_bytes(),
                config,
                outcome_count,
                environment,
                Some(order.order_id()),
                &signed_terms,
                &mut candidate,
            )
            .expect("semantic admission");
            assert_eq!(
                GeneralSignedOrderTermsV1::decode(&signed_terms)
                    .expect("signed terms")
                    .order_id(),
                order.order_id()
            );
            assert_eq!(
                read_scalar(&candidate, scalar::BATCH_POST_ORDER_COUNT),
                Ok(1)
            );
            assert_eq!(
                read_scalar(&candidate, scalar::BATCH_POST_QUOTE_RESERVE),
                Ok(6)
            );
            assert_eq!(read_scalar(&candidate, scalar::CUSTODY_AMOUNT), Ok(6));
            assert_eq!(read_scalar(&candidate, scalar::CUSTODY_ACTIVE), Ok(1));
            assert_eq!(
                read_scalar(&candidate, scalar::ORDER_POST_PHASE),
                Ok(u64::from(GeneralOrderPhaseV1::Placed.tag()))
            );
            let last = GENERAL_HOT_COMMON_SCALARS_V3
                + (outcome_count - 1) * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
            assert_eq!(
                read_scalar(&candidate, last + item_scalar::CLAIMS_SOURCE_MAGNITUDE),
                Ok(4)
            );
            assert_eq!(
                read_scalar(&candidate, last + item_scalar::CLAIMS_DESTINATION_MAGNITUDE),
                Ok(4)
            );
        }
    }

    #[test]
    fn place_order_row_substitution_refuses_before_candidate_mutation() {
        let outcome_count = 1;
        let mut environment = environment();
        let config = open_batch_config(environment);
        let (root, batch) = opened_batch(outcome_count, environment, config);
        let current_slot = 101;
        let order_bytes = placed_order_bytes(outcome_count, environment, batch, current_slot);
        let order = GeneralOrderV1::decode(&order_bytes).expect("order");
        environment.destination_vault_context = order.order_id();
        environment.custody_source_owner = order.header().owner_id;
        environment.settlement_position_owner = order.order_id();
        environment.rent_credit = order.header().owner_id;
        let mut candidate =
            place_order_input(outcome_count, environment, root, batch, order, current_slot);
        let base = GENERAL_HOT_COMMON_SCALARS_V3;
        write_scalar(&mut candidate, base + item_scalar::QUANTITY, 9).expect("hostile row");
        let before = candidate.clone();
        let mut signed_terms =
            vec![0; general_signed_order_terms_len_v1(outcome_count).expect("signed width")];
        order
            .encode_signed_terms_into(&mut signed_terms)
            .expect("signed immutable terms");
        assert_eq!(
            project_general_place_order_candidate_in_place_v3(
                &root.to_bytes(),
                &batch.to_bytes(),
                config,
                outcome_count,
                environment,
                Some(order.order_id()),
                &signed_terms,
                &mut candidate,
            ),
            Err(GeneralHotCandidateErrorV3::InvalidCoordinate)
        );
        assert_eq!(candidate, before);
    }

    #[test]
    fn cancel_order_executes_exact_batch_refund_semantics_at_runtime_widths() {
        for outcome_count in [1_u32, 258] {
            let mut environment = environment();
            let config = open_batch_config(environment);
            let (root, batch, order_bytes) =
                admitted_batch_and_order(outcome_count, environment, config);
            let order = GeneralOrderV1::decode(&order_bytes).expect("order");
            let owner = order.header().owner_id;
            environment.source_vault_context = order.order_id();
            environment.custody_destination_owner = owner;
            environment.settlement_position_owner = order.order_id();
            environment.rent_credit = owner;
            environment.rent_refund = owner;
            let mut candidate =
                cancel_order_input(outcome_count, environment, root, batch, order, 102);
            project_general_cancel_order_candidate_in_place_v3(
                &root.to_bytes(),
                &batch.to_bytes(),
                order.as_bytes(),
                config,
                outcome_count,
                environment,
                Some(order.order_id()),
                &mut candidate,
            )
            .expect("semantic cancellation");
            assert_eq!(
                read_scalar(&candidate, scalar::BATCH_POST_CANCELLED_COUNT),
                Ok(1)
            );
            assert_eq!(
                read_scalar(&candidate, scalar::BATCH_POST_QUOTE_RESERVE),
                Ok(0)
            );
            assert_eq!(read_scalar(&candidate, scalar::CUSTODY_AMOUNT), Ok(6));
            assert_eq!(
                read_scalar(&candidate, scalar::ORDER_POST_PHASE),
                Ok(u64::from(GeneralOrderPhaseV1::Cancelled.tag()))
            );
            assert_eq!(
                read_scalar(&candidate, scalar::ORDER_POST_RELEASED_SLOT),
                Ok(102)
            );
            assert_eq!(
                read_scalar(&candidate, scalar::CUSTODY_CLOSE_REPLAY_RESULTING_REVISION),
                Ok(environment.custody_expected_revision + 4)
            );
            let last = GENERAL_HOT_COMMON_SCALARS_V3
                + (outcome_count - 1) * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
            assert_eq!(read_scalar(&candidate, last + item_scalar::QUANTITY), Ok(4));
            assert_eq!(
                read_scalar(&candidate, last + item_scalar::CLAIMS_SOURCE_MAGNITUDE),
                Ok(4)
            );
        }
    }

    #[test]
    fn cancel_order_maker_substitution_refuses_without_candidate_mutation() {
        let outcome_count = 1;
        let mut environment = environment();
        let config = open_batch_config(environment);
        let (root, batch, order_bytes) =
            admitted_batch_and_order(outcome_count, environment, config);
        let order = GeneralOrderV1::decode(&order_bytes).expect("order");
        let owner = order.header().owner_id;
        environment.source_vault_context = order.order_id();
        environment.custody_destination_owner = owner;
        environment.settlement_position_owner = order.order_id();
        environment.rent_credit = owner;
        environment.rent_refund = owner;
        let mut candidate = cancel_order_input(outcome_count, environment, root, batch, order, 102);
        let scalar_count =
            general_hot_scalar_count_v3(Action::CancelOrder, outcome_count).expect("scalar count");
        write_identity(&mut candidate, scalar_count, identity::OWNER, [0x99; 32])
            .expect("hostile owner");
        let before = candidate.clone();
        assert_eq!(
            project_general_cancel_order_candidate_in_place_v3(
                &root.to_bytes(),
                &batch.to_bytes(),
                order.as_bytes(),
                config,
                outcome_count,
                environment,
                Some(order.order_id()),
                &mut candidate,
            ),
            Err(GeneralHotCandidateErrorV3::InvalidCoordinate)
        );
        assert_eq!(candidate, before);
    }

    #[test]
    fn release_order_projects_observed_residuals_at_runtime_widths() {
        for outcome_count in [1_u32, 258] {
            let mut environment = environment();
            let config = open_batch_config(environment);
            let (root, _batch, order_bytes) =
                admitted_batch_and_order(outcome_count, environment, config);
            let order = GeneralOrderV1::decode(&order_bytes).expect("order");
            let owner = order.header().owner_id;
            environment.source_vault_context = order.order_id();
            environment.custody_destination_owner = owner;
            environment.settlement_position_owner = order.order_id();
            environment.rent_credit = owner;
            environment.rent_refund = owner;
            let mut candidate = release_order_input(
                outcome_count,
                environment,
                root,
                order,
                order.header().valid_until_slot,
                2,
            );
            project_general_release_order_candidate_in_place_v3(
                &root.to_bytes(),
                order.as_bytes(),
                config,
                outcome_count,
                environment,
                Some(order.order_id()),
                &mut candidate,
            )
            .expect("semantic residual release");
            assert_eq!(read_scalar(&candidate, scalar::CUSTODY_AMOUNT), Ok(2));
            assert_eq!(read_scalar(&candidate, scalar::CUSTODY_ACTIVE), Ok(1));
            assert_eq!(
                read_scalar(&candidate, scalar::ORDER_POST_PHASE),
                Ok(u64::from(GeneralOrderPhaseV1::Released.tag()))
            );
            assert_eq!(
                read_scalar(&candidate, scalar::ORDER_POST_RELEASED_SLOT),
                Ok(order.header().valid_until_slot)
            );
            let last = GENERAL_HOT_COMMON_SCALARS_V3
                + (outcome_count - 1) * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
            assert_eq!(
                read_scalar(&candidate, last + item_scalar::CLAIMS_SOURCE_MAGNITUDE),
                Ok(1)
            );
            assert_eq!(
                read_scalar(&candidate, last + item_scalar::CLAIMS_DESTINATION_MAGNITUDE),
                Ok(1)
            );
        }
    }

    #[test]
    fn release_order_overstated_quote_refuses_without_candidate_mutation() {
        let outcome_count = 1;
        let mut environment = environment();
        let config = open_batch_config(environment);
        let (root, _batch, order_bytes) =
            admitted_batch_and_order(outcome_count, environment, config);
        let order = GeneralOrderV1::decode(&order_bytes).expect("order");
        let owner = order.header().owner_id;
        environment.source_vault_context = order.order_id();
        environment.custody_destination_owner = owner;
        environment.settlement_position_owner = order.order_id();
        environment.rent_credit = owner;
        environment.rent_refund = owner;
        let mut candidate = release_order_input(
            outcome_count,
            environment,
            root,
            order,
            order.header().valid_until_slot,
            7,
        );
        let before = candidate.clone();
        assert_eq!(
            project_general_release_order_candidate_in_place_v3(
                &root.to_bytes(),
                order.as_bytes(),
                config,
                outcome_count,
                environment,
                Some(order.order_id()),
                &mut candidate,
            ),
            Err(GeneralHotCandidateErrorV3::InvalidCoordinate)
        );
        assert_eq!(candidate, before);
    }

    #[test]
    fn open_batch_substitution_refuses_before_candidate_mutation() {
        let outcome_count = 1;
        let environment = environment();
        let config = open_batch_config(environment);
        let root = GeneralRootV2::active(
            environment.market,
            environment.general_config_id,
            environment.generation,
        )
        .expect("active root");
        let mut candidate = open_batch_input(outcome_count, environment, config, root);
        let batch_id = opened_batch_id(outcome_count, environment, config, root);
        write_identity(
            &mut candidate,
            general_hot_scalar_count_v3(Action::OpenBatch, outcome_count).expect("scalar count"),
            identity::GENERAL_CONFIG_ID,
            [0x99; 32],
        )
        .expect("hostile config identity");
        let before = candidate.clone();
        assert_eq!(
            project_general_open_batch_candidate_in_place_v3(
                &root.to_bytes(),
                config,
                outcome_count,
                environment,
                root.revision(),
                Some(batch_id),
                &mut candidate,
            ),
            Err(GeneralHotCandidateErrorV3::InvalidCoordinate)
        );
        assert_eq!(candidate, before);
    }

    #[test]
    fn runtime_width_one_and_two_fifty_eight_emit_complete_child_facts() {
        for outcome_count in [1_u32, 258] {
            let environment = GeneralHotEnvironmentV3 {
                settlement_position_revision: 12,
                settlement_position_present: true,
                ..environment()
            };
            let input = authenticated_input(Action::Materialize, outcome_count, environment);
            let mut scratch = vec![0_u8; input.len()];
            let mut output = vec![0x55_u8; input.len()];
            let accepted = project_general_hot_candidate_v3(
                Action::Materialize,
                &materialize_plan(outcome_count),
                &materialize_cursor(outcome_count),
                outcome_count,
                environment,
                &input,
                &mut scratch,
                &mut output,
            )
            .expect("complete candidate");
            assert!(matches!(accepted, ExecutionCandidateV2::Accepted(_)));
            let mut in_place = input.clone();
            project_general_hot_candidate_in_place_v3(
                Action::Materialize,
                &materialize_plan(outcome_count),
                &materialize_cursor(outcome_count),
                outcome_count,
                environment,
                &mut in_place,
            )
            .expect("one-workspace candidate");
            assert_eq!(in_place, output);
            assert_eq!(read_scalar(&output, scalar::CLAIMS_ADMIT_ACTIVE), Ok(0));
            assert_eq!(read_scalar(&output, scalar::CLAIMS_POSITION_COUNT), Ok(1));
            assert_eq!(
                read_scalar(&output, scalar::CLAIMS_ROW_COUNT),
                Ok(u64::from(outcome_count))
            );
            assert_eq!(
                read_scalar(&output, scalar::CLAIMS_POST_MARKET_REVISION),
                Ok(environment.claims_market_revision + 1)
            );
            assert_eq!(
                read_scalar(&output, scalar::SETTLEMENT_POST_POSITION_REVISION),
                Ok(environment.settlement_position_revision + 1)
            );
            assert_eq!(read_scalar(&output, scalar::CUSTODY_AMOUNT), Ok(3));
            let last = GENERAL_HOT_COMMON_SCALARS_V3
                + (outcome_count - 1) * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
            assert_eq!(
                read_scalar(&output, last + item_scalar::OUTCOME),
                Ok(u64::from(outcome_count - 1))
            );
            assert_eq!(read_scalar(&output, last + item_scalar::QUANTITY), Ok(3));
            assert_eq!(
                read_scalar(&output, last + item_scalar::CLAIMS_AGGREGATE_MAGNITUDE,),
                Ok(3)
            );
            assert_eq!(
                read_scalar(&output, last + item_scalar::CLAIMS_SOURCE_MAGNITUDE),
                Ok(0)
            );
            assert_eq!(
                read_scalar(&output, last + item_scalar::CLAIMS_DESTINATION_MAGNITUDE,),
                Ok(3)
            );
            assert_eq!(
                read_identity(
                    &output,
                    general_hot_scalar_count_v3(Action::Materialize, outcome_count)
                        .expect("scalars"),
                    identity::POSITION_ZERO_OWNER,
                ),
                Ok(environment.settlement_position_owner)
            );
        }
    }

    #[test]
    fn initialization_projects_exact_custody_and_cursor_facts_at_runtime_width() {
        for outcome_count in [1_u32, 258] {
            let mut environment = environment();
            environment.payer = [0x51; 32];
            environment.rent_refund = [0x52; 32];
            environment.custody_expected_revision = 0;
            let input =
                authenticated_input(Action::InitializeSettlement, outcome_count, environment);
            let mut scratch = vec![0_u8; input.len()];
            let mut output = vec![0x55_u8; input.len()];
            assert!(matches!(
                project_general_initialize_candidate_v3(
                    &initialized_cursor(outcome_count),
                    outcome_count,
                    environment,
                    &input,
                    &mut scratch,
                    &mut output,
                ),
                Ok(ExecutionCandidateV2::Accepted(_))
            ));
            let mut in_place = input.clone();
            project_general_initialize_candidate_in_place_v3(
                &initialized_cursor(outcome_count),
                outcome_count,
                environment,
                &mut in_place,
            )
            .expect("one-workspace initialize candidate");
            assert_eq!(in_place, output);
            assert_eq!(
                read_scalar(&output, scalar::CURSOR_RESULTING_REVISION),
                Ok(1)
            );
            assert_eq!(
                read_scalar(&output, scalar::CUSTODY_REPLAY_RENT_LAMPORTS),
                Ok(environment.custody_replay_rent_principal)
            );
            assert_eq!(
                read_identity(
                    &output,
                    general_hot_scalar_count_v3(Action::InitializeSettlement, outcome_count)
                        .expect("scalars"),
                    identity::PAYER,
                ),
                Ok(environment.payer)
            );
            let last = GENERAL_HOT_COMMON_SCALARS_V3
                + (outcome_count - 1) * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
            assert_eq!(
                read_scalar(&output, last + item_scalar::CURSOR_INVENTORY),
                Ok(0)
            );
        }
    }

    #[test]
    fn substituted_product_or_parent_witness_preserves_output() {
        let outcome_count = 258;
        let environment = environment();
        let plan = materialize_plan(outcome_count);
        let canonical = authenticated_input(Action::Materialize, outcome_count, environment);
        let mut hostile_tail = canonical.clone();
        write_scalar(&mut hostile_tail, scalar::OUTCOME_COUNT, 257).expect("hostile tail");
        let mut hostile_parent = canonical;
        write_identity(
            &mut hostile_parent,
            general_hot_scalar_count_v3(Action::Materialize, outcome_count).expect("scalars"),
            identity::PARENT_REQUEST_DIGEST,
            [0x99; 32],
        )
        .expect("hostile parent");
        for input in [hostile_tail, hostile_parent] {
            let mut scratch = vec![0_u8; input.len()];
            let mut output = vec![0x55_u8; input.len()];
            let before = output.clone();
            assert!(
                project_general_hot_candidate_v3(
                    Action::Materialize,
                    &plan,
                    &materialize_cursor(outcome_count),
                    outcome_count,
                    environment,
                    &input,
                    &mut scratch,
                    &mut output,
                )
                .is_err()
            );
            assert_eq!(output, before);
        }
    }
}
