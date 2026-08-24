// SPDX-License-Identifier: AGPL-3.0-or-later

//! Executable non-production Dealer facility transitions.
//!
//! This module owns the Solana trust boundary for the small set of facility
//! actions admitted by the laboratory profile. It authenticates physical
//! account ownership, exact codecs, PDA seeds, rent, Clock, Replay, and the
//! separately owned General/Position/liveness state before applying one atomic
//! postimage. Pure Dealer economics remain in `clutch-dealer-runtime-contract`.

use crate::accounts::{expect_pda, require, require_count, require_signer, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_dealer_runtime_contract::{
    accept_dealer_asset_transfer_v2, advance_collect_v2, advance_deliver_v2,
    bind_dealer_general_position_transfer_v3,
    bind_dealer_fee_terminal_v1,
    prepare_abort_lease_pot_v3, prepare_covered_dealer_row_progress_v1,
    dealer_runtime_liveness_policy_id_v1,
    prepare_finalize_lease_pot_v3,
    prepare_begin_covered_lease_pot_v4, prepare_bind_epoch_v3,
    prepare_dealer_sponsor_funding_transfer_v2,
    prepare_activate_dealer_v3, prepare_cancel_stale_funding_v3,
    prepare_dealer_lp_share_transfer_v2, prepare_dealer_sponsor_refund_transfer_v2,
    prepare_dealer_terminal_claim_replay_v2, prepare_dealer_terminal_claim_v2,
    begin_terminal_resolution_v1,
    prepare_facility_initialization_v3, prepare_first_lp_page_v2,
    prepare_lapse_epoch_v3, project_covered_dealer_position_v1,
    prepare_lp_contribution_v2, prepare_lp_withdrawal_v2, prepare_next_lp_page_v2,
    prepare_refund_cancelled_sponsor_v3, prepare_sponsor_halt_dealer_v3,
    prepare_enter_unwind_by_queue_v3,
    prepare_increase_exit_ticket_v1, prepare_new_exit_ticket_v1,
    prepare_timed_close_dealer_v3,
    CoveredDealerSelectionContextV1,
    CoveredDealerRowAssetTransitionV2, CoveredDealerSelectionV1, CoveredDealerTerminalV2,
    DealerActionReceiptV1, DealerAssetEndpointKindV1, DealerAssetTransferBundleV2,
    DealerAssetTransferPostObservationV2,
    DealerChildCountsV2,
    DealerEpochCloseCreditsV2, DealerEpochCloseRentV2,
    DealerEpochBindingV2, DealerFacilityGenesisV1, DealerFacilityReplayV1,
    DealerFundedBudgetDependenciesV1, DealerFundedDependenciesV2, DealerGeneralEpochEvidenceV3,
    DealerFutureCreditFundingV1, DealerFutureCreditUnusedCloseV1,
    DealerActionLivenessAuthorizationV1, DealerTransitionIntentV1,
    DealerTransitionLivenessModeV1,
    DealerClaimWorkV1, DealerExitTicketV1, DealerLivenessCompartmentV1,
    DealerLivenessScheduleV1, DealerTerminalAllocationV1,
    DealerTerminalRoundingPolicyV1, DEALER_PAGE_BITMAP_BYTES_V1,
    DealerPhaseV2, DealerQueueExitLivenessV1, DealerTerminalStateReceiptV2,
    DealerPositionMarketJoinV2, DealerPositionObservationV3, DealerReplayAccountBindingV1,
    DealerRuntimeActionV1, DealerRuntimeLivenessBindingV1, DealerSelectedFeeRecordBindingV1,
    DealerSeriesObligationBindingV1, DealerSeriesObligationKeyV1,
    DealerSeriesObligationBindingV2, DealerSeriesObligationKeyV2,
    DealerSeriesObligationPhaseV1, DealerStateV2, DealerStateV3,
    DealerTransferPositionV3, DealerLeasePotCloseRentV3, DealerLeaseV2,
    DeletableRentOwnerV1,
    FacilityPositionBindingV2, FixedCodec, Id, LpPageV2, RootRentOwnerV1,
    SettlementPotPhaseV1, SettlementPotV2, SponsorCapitalDispositionV1,
};
use clutch_general_v2_contract::{
    fee_runtime_semantic_release_id_v1,
    project_general_position_replay_prestate_v1, project_general_replay_transition_v1,
    CandidateWindowV4AccountV1, EconomicDomainV2AccountV1, GeneralEpochV6AccountV1,
    GeneralPositionReplayPrestateV1, GeneralReplayTransitionKindV1,
    GeneralReplayTransitionPlanV1, MarketBindingV2,
    SelectedFeeRecordV1AccountV1, SettlementRootChildStateV1, SettlementRootPhaseV1,
    SettlementRootV1AccountV1,
    ECONOMIC_DOMAIN_ACCOUNT_BYTES, GENERAL_EPOCH_ACCOUNT_BYTES, MARKET_BINDING_ACCOUNT_BYTES_V2,
    SELECTED_FEE_RECORD_ACCOUNT_BYTES, SETTLEMENT_ROOT_ACCOUNT_BYTES, WINDOW_ACCOUNT_BYTES,
};
use clutch_fee_runtime_contract::terminal::{
    FeeTerminalReceiptBundleV1, FEE_CLOSURE_MANIFEST_V1_BYTES,
    FEE_TERMINAL_RECEIPT_V1_BYTES,
};
use clutch_batch::portfolio_book_v2::{
    authenticate_complete_portfolio_book_into_v2,
    authenticate_complete_portfolio_book_for_root_transition_into_v2,
    AuthenticatedCompletePortfolioBookRefV2, PortfolioBookAccountExpectationV2,
    PortfolioBookAccountRoleV2, PortfolioBookAdapterV2, PortfolioBookInPlaceAdapterV2,
    PortfolioBookPageSetRecordV2, PortfolioCompleteBookProjectionExpectationV2,
    PORTFOLIO_BOOK_AUTHORITY_VERSION_V2, PORTFOLIO_BOOK_MAX_PAGES_V2,
};
use clutch_batch::portfolio_execution_v2::{
    authenticate_selected_portfolio_order_v2, PortfolioAccountExpectationV2,
    PortfolioAccountRoleV2, PortfolioAdapterV2, PortfolioIdentityV2,
    PortfolioSelectionMembershipExpectationV2, PortfolioTransitionExpectationV2,
    PortfolioSettlementReceiptV5TransitionExpectationV2, SelectedPortfolioOrderRecordV2,
    PortfolioSourceOrderKindV2, PORTFOLIO_EXECUTION_VERSION_V2,
    PORTFOLIO_PAIR_MAX_RECEIPTS_V2,
};
use clutch_batch::dealer_leg_v2::DealerLegVerdictV2;
use clutch_batch::relation_v2::{EconomicBookV2, EconomicCandidateV2, EconomicDomainV2};
use clutch_batch::Side;
use clutch_batch_policy_identity::revenue_policy_v1::{
    decode_revenue_policy, revenue_policy_digest, REVENUE_POLICY_BYTES,
};
use clutch_batch_policy_identity::{
    batch_policy_digest, decode_batch_policy, BATCH_POLICY_BYTES,
};
use clutch_general_v2_runtime::{
    decode_sealed_candidate_feed_v1, project_owner_blind_slot,
    verify_smooth_covered_dealer_candidate_into_v1,
};
use clutch_product_series::{
    ContentId, MarketGenesisProfileV2, MarketInstancePreimageV2, MarketInstanceV2Id,
    MarketLifecyclePhaseV2, NativeClaimBasisV1, PriceMeasurePolicyV1, ProductTemplateV4,
    QuantizedEdgePolicyV1, SeriesLinkObligationStatusV2, SeriesLinkObligationV2,
    SeriesMarketLinkPhaseV2, SeriesPlanV5Id,
};
use clutch_liveness::runtime_adapter_v1::{
    plan_runtime_transition_v1, RuntimeAtomicTransitionV1, RuntimePersistedAccountViewV1,
    RuntimeReceiptObservationV1, RuntimeTransferRoleV1, RuntimeTransitionActionV1,
    RuntimeTransitionIntentV1,
};
use clutch_liveness::runtime_v1::{
    PresentFundingSourceV1, PresentFundingV1, RuntimeCompartmentAdmissionV1,
    RuntimeCompartmentIdentityV1, RuntimeCompartmentKindV1, RuntimeCompartmentV1,
    RuntimeLivenessBundleV1, RuntimeLivenessPolicyV1, RUNTIME_COMPARTMENT_COUNT_V1,
    RUNTIME_COMPARTMENT_ORDER_V1, RUNTIME_LIVENESS_ACCOUNT_BYTES_V1,
    RUNTIME_LIVENESS_POLICY_BYTES_V1,
};
use clutch_retirement::{
    project_dealer_position_v3, project_general_position_v3, AdapterPositionMarketBindingV3,
    AdapterPositionPurposeBindingV3, DeletableRentOwnerV1 as ReplayRentOwnerV1, Identity32V1,
    PositionAccountV3, PositionLifecycleV3, PositionPurposeV3, PositionV3Fields,
    PositionTombstoneV3, PositionV3Sha256Backend, RentSplitV2,
    POSITION_TOMBSTONE_V3_BYTES, POSITION_V3_BYTES, ReplayV3Envelope,
};
use clutch_retirement_adapter::{
    authenticate_and_prepare_position_replay_close_v4,
    authenticate_position_v3_exact, authenticate_purpose_replay_v3_exact, AccountAccessV2,
    AccountViewV2 as RetirementAccountViewV2, CanonicalPdaV1,
    PositionReplayCloseRuntimeRequestV4, PositionV3RetirementRealmV1,
    PreparedPositionReplayCloseV3, RetirementRecipientViewV1,
};
use clutch_fractional_redemption_runtime::{
    bind_dealer_facility_vector_prestate_v1, CreditCreationV1,
    DealerFacilityVectorRequestV1, BoundDealerFacilityVectorPrestateV1,
    FRACTIONAL_CREDIT_ACCOUNT_BYTES,
};
use clutch_owner_settlement::{AuthenticatedPositionV3, PositionSettlementPoststateV3};
use clutch_solana_layout::registry::{
    DealerFacilityAction, DEALER_ACTION_RECEIPT_ACCOUNT_BYTES, DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
    DEALER_ACTION_RECEIPT_ACCOUNT_VERSION, DEALER_EPOCH_BINDING_V2_ACCOUNT_BYTES,
    DEALER_EPOCH_BINDING_V2_ACCOUNT_TAG, DEALER_EPOCH_BINDING_V2_ACCOUNT_VERSION,
    DEALER_COVERED_SELECTION_ACCOUNT_BYTES, DEALER_COVERED_SELECTION_ACCOUNT_TAG,
    DEALER_COVERED_SELECTION_ACCOUNT_VERSION, DEALER_COVERED_TERMINAL_ACCOUNT_VERSION,
    DEALER_EXIT_TICKET_ACCOUNT_BYTES, DEALER_EXIT_TICKET_ACCOUNT_TAG,
    DEALER_EXIT_TICKET_ACCOUNT_VERSION,
    DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES, DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG,
    DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION, DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES,
    DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_BYTES, DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_TAG,
    DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_VERSION,
    DEALER_LIVENESS_SCHEDULE_ACCOUNT_TAG, DEALER_LIVENESS_SCHEDULE_ACCOUNT_VERSION,
    DEALER_LP_PAGE_V2_ACCOUNT_BYTES, DEALER_LP_PAGE_V2_ACCOUNT_TAG,
    DEALER_LP_PAGE_V2_ACCOUNT_VERSION, DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_BYTES,
    DEALER_CLAIM_WORK_ACCOUNT_BYTES, DEALER_CLAIM_WORK_ACCOUNT_TAG,
    DEALER_CLAIM_WORK_ACCOUNT_VERSION, DEALER_TERMINAL_ALLOCATION_ACCOUNT_BYTES,
    DEALER_TERMINAL_ALLOCATION_ACCOUNT_TAG, DEALER_TERMINAL_ALLOCATION_ACCOUNT_VERSION,
    DEALER_LEASE_V2_ACCOUNT_BYTES, DEALER_LEASE_V2_ACCOUNT_TAG,
    DEALER_LEASE_V2_ACCOUNT_VERSION, DEALER_SETTLEMENT_POT_V2_ACCOUNT_BYTES,
    DEALER_SETTLEMENT_POT_V2_ACCOUNT_TAG, DEALER_SETTLEMENT_POT_V2_ACCOUNT_VERSION,
    DEALER_STATE_V2_ACCOUNT_BYTES, DEALER_STATE_V2_ACCOUNT_TAG, DEALER_STATE_V2_ACCOUNT_VERSION,
    DEALER_SERIES_OBLIGATION_ACCOUNT_BYTES, DEALER_SERIES_OBLIGATION_ACCOUNT_TAG,
    DEALER_SERIES_OBLIGATION_ACCOUNT_VERSION, DEALER_STATE_V3_ACCOUNT_BYTES,
    DEALER_SERIES_OBLIGATION_ACCOUNT_BYTES_V2,
    DEALER_SERIES_OBLIGATION_ACCOUNT_VERSION_V2,
    DEALER_STATE_V3_ACCOUNT_TAG, DEALER_STATE_V3_ACCOUNT_VERSION,
    FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_BYTES,
    FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_BYTES,
};
use clutch_solana_layout::order_page_v5::{
    verify_page_set_v5_streaming, OrderPageHeaderV5, OrderSlotCursorV5, ORDER_PAGE_V5_BYTES,
};
use clutch_solana_layout::reservation::{
    ReservationPlan, ORDER_KIND_PORTFOLIO, RESERVATION_STATE_CONSUMED,
    RESERVATION_STATE_ENTITLED,
};
use clutch_solana_layout::reservation_v9::{
    ReservationAccountV9, RESERVATION_ACCOUNT_BYTES_V9,
};
use clutch_solana_layout::OrderSlot;
use clutch_solana_layout::revenue::{RevenuePolicyRecordV1, REVENUE_POLICY_RECORD_BYTES};
use clutch_solana_layout::{account_len, PriceGridAccount};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::artifact::read_clock_slot;
use super::collateral_position_v3::{
    authenticate_general_market_value_authority_v2, authenticate_general_position_replay_v2,
    GeneralMarketValueAuthorityV2, GeneralPositionReplayAuthorityV2, RuntimeSha256,
};
use super::dealer_policy::{
    authenticate_catalog_policy, create_exact_payer_debit_pda, create_full_principal_pda,
    dealer_fault, fund_and_resize_program_account,
};
use super::product_artifact::{
    authenticate_product_artifact_v1, authenticate_registry_capability_v3,
    authenticate_series_registry_capability_refs_v2, AuthenticatedProductArtifactV1,
};
use super::product_market::{
    admit_series_dealer_obligation_v1, authenticate_series_dealer_authorization_v1,
    authenticate_market_lifecycle_root_v1, authenticate_series_market_link_v1,
    AuthenticatedSeriesDealerAdmissionOwnerV1, AuthenticatedSeriesDealerAuthorizationV1,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, MarketLifecycleRootAccountV2,
    SeriesMarketLinkAccountV1, SeriesMarketLinkAccountV2,
};
use crate::instructions_sysvar::{InstructionsSysvarV1, SYSVAR_OWNER_ID};
use super::dealer_runtime::{
    authenticate_dealer_meta_contract_v1, authenticate_dealer_series_obligation_v1,
    authenticate_dealer_series_obligation_v2, authenticate_dealer_state_v3,
    decode_dealer_account_body_v1, encode_dealer_account_body_v1, DealerRuntimePayloadV1,
};
use super::product_series_current::{
    admit_series_dealer_obligation_v2, authenticate_market_lifecycle_root_v2,
    authenticate_live_series_dealer_obligation_v2, authenticate_registry_capability_v4,
    authenticate_series_market_link_v2,
    authenticate_series_registry_account_v3, AuthenticatedSeriesDealerAdmissionOwnerV2,
    AuthenticatedLiveSeriesDealerObligationV2,
    AuthenticatedMarketLifecycleRootV2, AuthenticatedRegistryCapabilityV4,
    AuthenticatedSeriesMarketLinkV2,
};
use super::genesis::{
    allocate_data, assign_data, read_rent, require_creatable, require_system_program,
    transfer_data, SYSTEM_PROGRAM_ID,
};
use super::fractional_redemption::{
    accept_dealer_facility_credit_funding_v1,
    apply_dealer_facility_credit_terminal_v1,
    apply_dealer_facility_vector_transition_v1,
    AcceptedDealerFacilityCreditFundingV1, AcceptedDealerFacilityVectorTransitionV1,
    AcceptedDealerFacilityCreditTerminalV1,
    AuthenticatedDealerFacilityCreditTerminalAuthorityV1,
    AuthenticatedDealerFacilityVectorAuthorityV1, DealerFacilityCreditTerminalAccountsV1,
    DealerFacilityCreditTerminalObservationV1, DealerFacilityCreditTerminalPrestateV1,
    DealerFacilityVectorAccountsV1,
};

const DEALER_COLLATERAL_AUTHORITY_ACCOUNT_COUNT: usize = 10;
const INITIALIZE_ACCOUNT_COUNT: usize = 24 + DEALER_COLLATERAL_AUTHORITY_ACCOUNT_COUNT;
const CREATE_FIRST_LP_PAGE_ACCOUNT_COUNT: usize = 20;
const CREATE_NEXT_LP_PAGE_ACCOUNT_COUNT: usize = 21;
const LP_TRANSFER_ACCOUNT_COUNT: usize = 8 + DEALER_COLLATERAL_AUTHORITY_ACCOUNT_COUNT;
const ACTIVATE_ACCOUNT_COUNT: usize = 21;
const CANCEL_FUNDING_ACCOUNT_COUNT: usize = 20;
const REFUND_CANCELLED_SPONSOR_ACCOUNT_COUNT: usize =
    21 + DEALER_COLLATERAL_AUTHORITY_ACCOUNT_COUNT;
const BIND_EPOCH_ACCOUNT_COUNT: usize = 24;
const LAPSE_EPOCH_ACCOUNT_COUNT: usize = 25;
const ENTER_UNWIND_ACCOUNT_COUNT: usize = 20;
const TIMED_CLOSE_ACCOUNT_COUNT: usize = 21;
const CLAIM_TERMINAL_ACCOUNT_COUNT: usize = 35;
const RESOLVE_FACILITY_VECTOR_ACCOUNT_COUNT: usize = 41;
const QUEUE_EXIT_CALLER_NEW_ACCOUNT_COUNT: usize = 10;
const QUEUE_EXIT_CALLER_EXISTING_ACCOUNT_COUNT: usize = 8;
const QUEUE_EXIT_EXTERNAL_ACCOUNT_COUNT: usize = 22;
const SELECT_LEASE_BEGIN_FIXED_ACCOUNT_COUNT: usize = 58;
const COLLECT_DELIVER_FIXED_ACCOUNT_COUNT: usize = 43;
const FINALIZE_ABORT_ACCOUNT_COUNT: usize = 30 + DEALER_COLLATERAL_AUTHORITY_ACCOUNT_COUNT;
const RETIRE_ACTIVE_FACILITY_CREDIT_ACCOUNT_COUNT: usize = 48;
const RETIRE_UNUSED_FUTURE_CREDIT_ACCOUNT_COUNT: usize = 45;
const DEALER_GENERAL_REPLAY_VALUE_EVIDENCE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/dealer-general-replay-value-evidence/v2\0";
const DEALER_SERIES_ADMISSION_PREWRITE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/dealer-series-admission-prewrite/v1\0";
const DEALER_SERIES_TERMINAL_PREWRITE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/dealer-series-terminal-prewrite/v2\0";
const DEALER_POSITION_REPLAY_CLOSE_POSTWRITE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/dealer-position-replay-close-postwrite/v3\0";
const DEALER_PRODUCT_RESOLUTION_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/dealer-product-resolution-authentication/v2\0";
const DEALER_FUTURE_CREDIT_POSTWRITE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/dealer-future-credit-postwrite/v1\0";
const DEALER_FUTURE_CREDIT_UNUSED_CLOSE_POSTWRITE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/dealer-future-credit-unused-close-postwrite/v1\0";
const _: () = assert!(
    DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_BYTES
        == 8 + clutch_dealer_runtime_contract::DEALER_FUTURE_CREDIT_FUNDING_BYTES_V1
);

/// Exact ordered current collateral deployment accounts for one Dealer
/// liability movement. Hoard and ClaimLedger are always read-only here:
/// Dealer reclassifies internal Position/Pot liabilities and never sources a
/// token CPI, fee, rent, or liveness payment from Hoard principal.
struct DealerCollateralAuthorityAccountsV2<'a, 'info> {
    realm: &'a AccountInfo<'info>,
    profile: &'a AccountInfo<'info>,
    policy: &'a AccountInfo<'info>,
    token_program: &'a AccountInfo<'info>,
    token_programdata: &'a AccountInfo<'info>,
    market_binding: &'a AccountInfo<'info>,
    market_runtime: &'a AccountInfo<'info>,
    market_instance: &'a AccountInfo<'info>,
    hoard: &'a AccountInfo<'info>,
    claim_ledger: &'a AccountInfo<'info>,
}

/// Non-detachable Dealer owner of the first Product obligation admission.
/// The plan exists only after the exact V2 State, facility/0xaf PDAs, current
/// Product authority, rent principal, hostile prefund, refund owner, and sink
/// have all been authenticated before any write or CPI.
struct AuthenticatedDealerSeriesAdmissionPrewriteV1 {
    authentication_id: ContentId,
    product_authorization_id: ContentId,
    state_account_id: Id,
    state_pre_content_id: Id,
    key: DealerSeriesObligationKeyV1,
    owner_admission_receipt_id: Id,
    rent: DeletableRentOwnerV1,
}

/// Current Product/Dealer first-lease prewrite. This non-Copy value owns only
/// Dealer facts; Product independently authenticates RootV2, LinkV2,
/// RegistryV3/ReleaseV2/ProfileV4, BundleV6, and AttachmentV5 before invoking
/// the exact callback below.
struct AuthenticatedDealerSeriesAdmissionPrewriteV2 {
    authentication_id: ContentId,
    state_account_id: Id,
    state_pre_content_id: Id,
    key: clutch_dealer_runtime_contract::DealerSeriesObligationKeyV2,
    owner_admission_receipt_id: Id,
    capability_profile_id: Id,
    rent: DeletableRentOwnerV1,
}

/// Compact non-authoritative projection retained only after hostile decoding
/// current Product RootV2, LinkV2, and Dealer-owned `0xaf/v2` in one call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedDealerProductResolutionV2 {
    root_account_id: Id,
    root_semantic_id: Id,
    link_account_id: Id,
    link_semantic_id: Id,
    resolution_semantic_id: Id,
    resolution_data_id: Id,
    authentication_id: Id,
}

/// One-shot physical authority that converts the Dealer-owned `0xbc/v1`
/// principal into the canonical facility-owned Fractional a6/v2 account.
/// The by-value Fractional trait consumes this value exactly once.
struct AuthenticatedDealerFacilityVectorAuthoritySbfV1<'a, 'info> {
    prestate: BoundDealerFacilityVectorPrestateV1,
    funding: DealerFutureCreditFundingV1,
    funding_account: &'a AccountInfo<'info>,
    refund_owner: &'a AccountInfo<'info>,
    neutral_sink: &'a AccountInfo<'info>,
    credit_account: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    current_generation: u64,
    live_credit_rent_lamports: u64,
    tombstone_rent_lamports: u64,
}

/// Dealer-owned half of the active-credit action25 terminal cut.
///
/// This non-Copy value is constructible only after the same outer has prepared
/// the terminal Dealer Replay and accepted Product's exact LinkV2 terminal
/// postwrite. Fractional consumes it by value before touching a5,
/// ClaimLedgerV3, or a6/v2.
struct AuthenticatedDealerFacilityCreditTerminalAuthoritySbfV1 {
    prestate: DealerFacilityCreditTerminalPrestateV1,
    fractional_ledger_account: Id,
    fractional_ledger_before_id: Id,
    product_root_account: Id,
    product_root_authentication_id: Id,
    resolution_semantic_id: Id,
    resolution_data_id: Id,
    stored_payer: Id,
    neutral_sink: Id,
}

/// Dealer's non-detachable input to Product's sole current LinkV2 terminal writer.
///
/// This retains the exact hostile-authenticated live Product receipt, current
/// StateV3/Replay/Position cut, and live `0xaf/v2`. Product must consume this
/// value before Dealer can persist the terminal `0xaf` successor or release
/// any value-bearing child.
pub(crate) struct AuthenticatedDealerSeriesTerminalPrewriteV2 {
    authentication_id: ContentId,
    live_product: AuthenticatedLiveSeriesDealerObligationV2,
    obligation_account: Pubkey,
    obligation_presemantic_id: ContentId,
    state_account: Pubkey,
    state_presemantic_id: ContentId,
    terminal_state_receipt: DealerTerminalStateReceiptV2,
    terminal_state_receipt_id: ContentId,
    replay_presemantic_id: ContentId,
    replay_pre_ordinal: u64,
    owner_terminal_receipt_id: ContentId,
    expected_link_transition_sequence: u64,
    rent_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
}

/// Exact prospective PositionV3 tombstone and Replay deletion retained until
/// the same outer has accepted Product's LinkV2 terminal postwrite.
#[derive(Clone, Copy)]
struct PreparedDealerPositionReplayCloseV3 {
    plan: PreparedPositionReplayCloseV3,
    terminal_replay: DealerFacilityReplayV1,
    close_receipt_id: ContentId,
}

impl PreparedDealerPositionReplayCloseV3 {
    const fn close_receipt_id(self) -> ContentId { self.close_receipt_id }
}

impl AuthenticatedDealerSeriesTerminalPrewriteV2 {
    pub(crate) const fn id(&self) -> ContentId { self.authentication_id }
    pub(crate) const fn dealer_obligation_account(&self) -> Pubkey {
        self.obligation_account
    }
    pub(crate) const fn dealer_obligation_presemantic_id(&self) -> ContentId {
        self.obligation_presemantic_id
    }
    pub(crate) const fn dealer_state_account(&self) -> Pubkey { self.state_account }
    pub(crate) const fn dealer_state_presemantic_id(&self) -> ContentId {
        self.state_presemantic_id
    }
    pub(crate) const fn terminal_state_receipt(&self) -> DealerTerminalStateReceiptV2 {
        self.terminal_state_receipt
    }
    pub(crate) const fn terminal_state_receipt_id(&self) -> ContentId {
        self.terminal_state_receipt_id
    }
    pub(crate) const fn replay_presemantic_id(&self) -> ContentId {
        self.replay_presemantic_id
    }
    pub(crate) const fn replay_pre_ordinal(&self) -> u64 { self.replay_pre_ordinal }
    pub(crate) const fn owner_terminal_receipt_id(&self) -> ContentId {
        self.owner_terminal_receipt_id
    }
    pub(crate) const fn expected_link_transition_sequence(&self) -> u64 {
        self.expected_link_transition_sequence
    }
    pub(crate) const fn rent_refund_owner(&self) -> Pubkey { self.rent_refund_owner }
    pub(crate) const fn neutral_lamport_sink(&self) -> Pubkey { self.neutral_lamport_sink }
}

/// Non-detachable physical receipt proving the unused `0xbc/v1` owner was
/// deleted only after the exact current Product obligation reached Terminal.
/// It retains the pure terminal plan plus every observed lamport poststate.
struct AuthenticatedDealerFutureCreditUnusedCloseV1 {
    plan: DealerFutureCreditUnusedCloseV1,
    postwrite_receipt_id: Id,
    refund_owner_lamports_after: u64,
    neutral_sink_lamports_after: u64,
}

impl AuthenticatedDealerFutureCreditUnusedCloseV1 {
    const fn receipt_id(&self) -> Id {
        self.postwrite_receipt_id
    }
}

/// Exact already-live Product obligation retained across later Dealer leases.
/// This is authenticated from the hostile StateV3 and `0xaf` bodies plus the
/// current Product root/link/Registry authority; no caller-shaped admission
/// tuple can select the existing path.
#[derive(Clone, Copy)]
struct AuthenticatedExistingDealerSeriesAdmissionV1 {
    state: DealerStateV3,
    obligation: DealerSeriesObligationBindingV1,
}

/// Hostile current Product/Dealer proof for a later lease after the once-only
/// first-lease admission. This receipt is read-only and cannot advance LinkV2.
struct AuthenticatedExistingDealerSeriesAdmissionV2 {
    state: DealerStateV3,
    obligation: DealerSeriesObligationBindingV2,
    product: AuthenticatedLiveSeriesDealerObligationV2,
}

impl AuthenticatedSeriesDealerAdmissionOwnerV1
    for AuthenticatedDealerSeriesAdmissionPrewriteV1
{
    fn authenticate_series_dealer_admission_owner_v1(
        &self,
        authorization: AuthenticatedSeriesDealerAuthorizationV1,
    ) -> Outcome<ContentId> {
        let expected_owner_receipt_id = self
            .key
            .admission_owner_receipt_id(
                Id::from_bytes(authorization.link_semantic_id().bytes()),
                authorization
                    .link_transition_sequence()
                    .checked_add(1)
                    .ok_or(ClutchError::Arithmetic)?,
            )
            .map_err(dealer_fault)?;
        require(
            self.authentication_id != ContentId::ZERO
                && self.product_authorization_id == authorization.id()
                && authorization.root_account()
                    == authorization.product_market_root_account()
                && authorization.root_authentication_id() != ContentId::ZERO
                && authorization.root_semantic_id() != ContentId::ZERO
                && authorization.registry_account() != authorization.root_account()
                && authorization.registry_programdata_account()
                    != authorization.registry_account()
                && authorization.registry_programdata_sha256() != ContentId::ZERO
                && authorization.registry_release_id() != ContentId::ZERO
                && self.key.dealer_state_account_id == self.state_account_id
                && self.key.product_market_root_account_id
                    == id(&authorization.root_account())
                && self.key.product_market_binding_id
                    == Id::from_bytes(authorization.product_market_binding_id().bytes())
                && self.key.market_instance_v2_id
                    == Id::from_bytes(authorization.market_instance_id().bytes())
                && self.key.series_plan_v5_id
                    == Id::from_bytes(authorization.series_plan_id().bytes())
                && self.key.series_market_link_account_id == id(&authorization.link_account())
                && self.key.attachment_plan_v4_id
                    == Id::from_bytes(authorization.attachment_plan_id().bytes())
                && self.key.product_generation == authorization.generation()
                && self.key.series_ordinal == authorization.ordinal()
                && self.rent.neutral_sink
                    == Id::from_bytes(authorization.neutral_lamport_sink().bytes())
                && self.rent.refundable_principal != 0
                && self.rent.payer != self.rent.neutral_sink
                && self.state_pre_content_id != Id::ZERO
                && self.owner_admission_receipt_id == expected_owner_receipt_id,
            ClutchError::AuthorizationUnavailable,
        )?;
        Ok(ContentId::from_bytes(
            self.owner_admission_receipt_id.bytes(),
        ))
    }
}

impl AuthenticatedSeriesDealerAdmissionOwnerV2
    for AuthenticatedDealerSeriesAdmissionPrewriteV2
{
    fn owner_admission_receipt_id(&self) -> Outcome<ContentId> {
        Ok(ContentId::from_bytes(self.owner_admission_receipt_id.bytes()))
    }

    fn dealer_obligation_account(&self) -> Outcome<Pubkey> {
        Ok(Pubkey::new_from_array(self.key.binding_account_id.bytes()))
    }

    fn dealer_state_account(&self) -> Outcome<Pubkey> {
        Ok(Pubkey::new_from_array(self.state_account_id.bytes()))
    }

    fn dealer_state_presemantic_id(&self) -> Outcome<ContentId> {
        Ok(ContentId::from_bytes(self.state_pre_content_id.bytes()))
    }

    fn dealer_facility_id(&self) -> Outcome<ContentId> {
        Ok(ContentId::from_bytes(self.key.facility_id.bytes()))
    }

    fn dealer_position_binding_id(&self) -> Outcome<ContentId> {
        Ok(ContentId::from_bytes(
            self.key.facility_position_binding_id.bytes(),
        ))
    }

    fn dealer_rent_principal_lamports(&self) -> Outcome<u64> {
        Ok(self.rent.refundable_principal)
    }

    fn dealer_prefund_donation_lamports(&self) -> Outcome<u64> {
        Ok(self.rent.donation_floor)
    }

    fn rent_refund_owner(&self) -> Outcome<ContentId> {
        Ok(ContentId::from_bytes(self.rent.payer.bytes()))
    }

    fn neutral_lamport_sink(&self) -> Outcome<ContentId> {
        Ok(ContentId::from_bytes(self.rent.neutral_sink.bytes()))
    }

    #[allow(clippy::too_many_arguments)]
    fn authenticate_series_dealer_admission_owner_v2(
        &self,
        authorization_id: ContentId,
        root_account: Pubkey,
        root_binding_id: ContentId,
        link_account: Pubkey,
        _link_binding_id: ContentId,
        series_plan_id: SeriesPlanV5Id,
        ordinal: u32,
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        _funding_quote_id: clutch_product_series::SeriesFundingQuoteV5Id,
        compiler_bundle_id: clutch_product_series::CompiledProductSeriesBundleV6Id,
        attachment_plan_id: clutch_product_series::SeriesAttachmentPlanV5Id,
        liquidity_facility_plan_id: ContentId,
        _registry_release_id: ContentId,
        capability_profile_id: ContentId,
        _dealer_obligation_configuration_id: ContentId,
        dealer_obligation_account: Pubkey,
        dealer_state_account: Pubkey,
        dealer_state_presemantic_id: ContentId,
        dealer_facility_id: ContentId,
        dealer_position_binding_id: ContentId,
        dealer_rent_principal_lamports: u64,
        dealer_prefund_donation_lamports: u64,
        rent_refund_owner: ContentId,
        neutral_lamport_sink: ContentId,
        owner_admission_receipt_id: ContentId,
    ) -> Outcome<()> {
        require(
            self.authentication_id != ContentId::ZERO
                && authorization_id != ContentId::ZERO
                && self.key.product_market_root_account_id == id(&root_account)
                && self.key.product_market_binding_id
                    == Id::from_bytes(root_binding_id.bytes())
                && self.key.series_market_link_account_id == id(&link_account)
                && self.key.series_plan_v5_id == Id::from_bytes(series_plan_id.bytes())
                && self.key.series_ordinal == ordinal
                && self.key.market_instance_v2_id == Id::from_bytes(market_instance_id.bytes())
                && self.key.product_generation == generation
                && self.key.compiler_bundle_v6_id
                    == Id::from_bytes(compiler_bundle_id.bytes())
                && self.key.attachment_plan_v5_id
                    == Id::from_bytes(attachment_plan_id.bytes())
                && self.key.policy_id == Id::from_bytes(liquidity_facility_plan_id.bytes())
                && self.key.binding_account_id == id(&dealer_obligation_account)
                && self.state_account_id == id(&dealer_state_account)
                && self.state_pre_content_id
                    == Id::from_bytes(dealer_state_presemantic_id.bytes())
                && self.key.facility_id == Id::from_bytes(dealer_facility_id.bytes())
                && self.key.facility_position_binding_id
                    == Id::from_bytes(dealer_position_binding_id.bytes())
                && self.rent.refundable_principal == dealer_rent_principal_lamports
                && self.rent.donation_floor == dealer_prefund_donation_lamports
                && self.rent.payer == Id::from_bytes(rent_refund_owner.bytes())
                && self.rent.neutral_sink == Id::from_bytes(neutral_lamport_sink.bytes())
                && self.owner_admission_receipt_id
                    == Id::from_bytes(owner_admission_receipt_id.bytes())
                && self.capability_profile_id
                    == Id::from_bytes(capability_profile_id.bytes()),
            ClutchError::MismatchedState,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn authenticate_dealer_series_admission_prewrite_v1(
    program_id: &Pubkey,
    authorization: AuthenticatedSeriesDealerAuthorizationV1,
    state_account: &AccountInfo<'_>,
    state: &DealerStateV2,
    obligation_account: &AccountInfo<'_>,
    key: DealerSeriesObligationKeyV1,
    stored_bump: u8,
    rent_payer: &AccountInfo<'_>,
    refundable_principal: u64,
    donation_floor: u64,
) -> Outcome<AuthenticatedDealerSeriesAdmissionPrewriteV1> {
    key.validate().map_err(dealer_fault)?;
    state.validate().map_err(dealer_fault)?;
    let state_pre_content_id = state.state_content_id().map_err(dealer_fault)?;
    let next_sequence = authorization
        .link_transition_sequence()
        .checked_add(1)
        .ok_or(ClutchError::Arithmetic)?;
    let owner_admission_receipt_id = key
        .admission_owner_receipt_id(
            Id::from_bytes(authorization.link_semantic_id().bytes()),
            next_sequence,
        )
        .map_err(dealer_fault)?;
    let (expected_obligation, expected_bump) =
        seeds::dealer_series_obligation_pda(program_id, &state.facility_id.bytes());
    require(
        authorization.requires_product_admission()
            && state.phase == DealerPhaseV2::Trading
            && state_account.owner == program_id
            && state_account.is_writable
            && state_account.data_len() == DEALER_STATE_V2_ACCOUNT_BYTES
            && key.dealer_state_account_id == id(state_account.key)
            && key.facility_id == state.facility_id
            && key.policy_id == state.policy_id
            && key.facility_position_binding_id == state.facility_position_binding_id
            && obligation_account.key == &expected_obligation
            && stored_bump == expected_bump
            && obligation_account.owner == &SYSTEM_PROGRAM_ID
            && obligation_account.is_writable
            && !obligation_account.is_signer
            && !obligation_account.executable
            && obligation_account.data_len() == 0
            && obligation_account.lamports() == donation_floor
            && refundable_principal != 0
            && rent_payer.key != obligation_account.key
            && rent_payer.key.to_bytes() != authorization.neutral_lamport_sink().bytes(),
        ClutchError::AuthorizationUnavailable,
    )?;
    let rent = DeletableRentOwnerV1 {
        payer: id(rent_payer.key),
        neutral_sink: Id::from_bytes(authorization.neutral_lamport_sink().bytes()),
        refundable_principal,
        donation_floor,
    };
    rent.validate().map_err(dealer_fault)?;
    let authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            DEALER_SERIES_ADMISSION_PREWRITE_DOMAIN_V1,
            program_id.as_ref(),
            &authorization.id().bytes(),
            state_account.key.as_ref(),
            &state_pre_content_id.bytes(),
            obligation_account.key.as_ref(),
            &key.binding_account_id.bytes(),
            &owner_admission_receipt_id.bytes(),
            rent_payer.key.as_ref(),
            &rent.neutral_sink.bytes(),
            &refundable_principal.to_le_bytes(),
            &donation_floor.to_le_bytes(),
            &[stored_bump],
        ])
        .to_bytes(),
    );
    require(authentication_id != ContentId::ZERO, ClutchError::AuthorizationUnavailable)?;
    Ok(AuthenticatedDealerSeriesAdmissionPrewriteV1 {
        authentication_id,
        product_authorization_id: authorization.id(),
        state_account_id: id(state_account.key),
        state_pre_content_id,
        key,
        owner_admission_receipt_id,
        rent,
    })
}

#[allow(clippy::too_many_arguments)]
fn authenticate_dealer_series_admission_prewrite_v2(
    program_id: &Pubkey,
    root: AuthenticatedMarketLifecycleRootV2<'_>,
    link: AuthenticatedSeriesMarketLinkV2<'_>,
    registry: &AuthenticatedRegistryCapabilityV4,
    state_account: &AccountInfo<'_>,
    state: &DealerStateV2,
    obligation_account: &AccountInfo<'_>,
    key: clutch_dealer_runtime_contract::DealerSeriesObligationKeyV2,
    stored_bump: u8,
    rent_payer: &AccountInfo<'_>,
    refundable_principal: u64,
    donation_floor: u64,
) -> Outcome<AuthenticatedDealerSeriesAdmissionPrewriteV2> {
    use clutch_product_series::{
        MarketLifecyclePhaseV2, SeriesLinkObligationStatusV2, SeriesLinkObligationV2,
        SeriesMarketLinkPhaseV2,
    };

    key.validate().map_err(dealer_fault)?;
    state.validate().map_err(dealer_fault)?;
    let state_pre_content_id = state.state_content_id().map_err(dealer_fault)?;
    let root_binding = root.state().binding();
    let link_binding = link.state().binding();
    let root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_semantic_id = link
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let next_sequence = link
        .state()
        .transition_sequence()
        .checked_add(1)
        .ok_or(ClutchError::Arithmetic)?;
    let owner_admission_receipt_id = key
        .admission_owner_receipt_id(
            Id::from_bytes(link_semantic_id.bytes()),
            next_sequence,
        )
        .map_err(dealer_fault)?;
    let (expected_obligation, expected_bump) =
        seeds::dealer_series_obligation_pda(program_id, &state.facility_id.bytes());
    require(
        !root.is_writable()
            && link.is_writable()
            && root.state().phase() == MarketLifecyclePhaseV2::Active
            && root.state().resolution_semantic_id() == ContentId::ZERO
            && root.state().resolution_data_id() == ContentId::ZERO
            && root.state().resolution_activation_receipt_id() == ContentId::ZERO
            && link.state().phase() == SeriesMarketLinkPhaseV2::Active
            && link
                .state()
                .obligation_status(SeriesLinkObligationV2::Dealer)
                == SeriesLinkObligationStatusV2::EnabledNeverFounded
            && registry.activation_consumed()
            && state.phase == DealerPhaseV2::Trading
            && state_account.owner == program_id
            && state_account.is_writable
            && state_account.data_len() == DEALER_STATE_V2_ACCOUNT_BYTES
            && key.dealer_state_account_id == id(state_account.key)
            && key.facility_id == state.facility_id
            && key.policy_id == state.policy_id
            && key.facility_position_binding_id == state.facility_position_binding_id
            && key.product_market_root_account_id == id(&root.account())
            && key.product_market_binding_id == Id::from_bytes(root_binding_id.bytes())
            && key.market_instance_v2_id == Id::from_bytes(root_binding.market_instance_id.bytes())
            && key.product_generation == root_binding.generation
            && key.series_market_link_account_id == id(&link.account())
            && key.series_plan_v5_id == Id::from_bytes(link_binding.series_plan_id.bytes())
            && key.series_ordinal == link_binding.ordinal
            && key.compiler_bundle_v6_id
                == Id::from_bytes(link_binding.compiler_bundle_id.bytes())
            && key.attachment_plan_v5_id
                == Id::from_bytes(link_binding.attachment_plan_id.bytes())
            && registry.series_plan_id() == link_binding.series_plan_id
            && registry.registry_release_id() == root_binding.registry_release_id
            && registry.capability_profile_id() == root_binding.capability_profile_id
            && obligation_account.key == &expected_obligation
            && stored_bump == expected_bump
            && obligation_account.owner == &SYSTEM_PROGRAM_ID
            && obligation_account.is_writable
            && !obligation_account.is_signer
            && !obligation_account.executable
            && obligation_account.data_len() == 0
            && obligation_account.lamports() == donation_floor
            && refundable_principal != 0
            && rent_payer.key != obligation_account.key
            && rent_payer.key.to_bytes() == link_binding.rent_refund_owner.bytes()
            && rent_payer.key.to_bytes() != link_binding.neutral_lamport_sink.bytes(),
        ClutchError::AuthorizationUnavailable,
    )?;
    let rent = DeletableRentOwnerV1 {
        payer: id(rent_payer.key),
        neutral_sink: Id::from_bytes(link_binding.neutral_lamport_sink.bytes()),
        refundable_principal,
        donation_floor,
    };
    rent.validate().map_err(dealer_fault)?;
    let authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            DEALER_SERIES_ADMISSION_PREWRITE_DOMAIN_V1,
            b"current-v2\0",
            program_id.as_ref(),
            &root.authentication_id().bytes(),
            &link.authentication_id().bytes(),
            &registry.id().bytes(),
            state_account.key.as_ref(),
            &state_pre_content_id.bytes(),
            obligation_account.key.as_ref(),
            &key.binding_account_id.bytes(),
            &owner_admission_receipt_id.bytes(),
            rent_payer.key.as_ref(),
            &rent.neutral_sink.bytes(),
            &refundable_principal.to_le_bytes(),
            &donation_floor.to_le_bytes(),
            &[stored_bump],
        ])
        .to_bytes(),
    );
    require(authentication_id != ContentId::ZERO, ClutchError::AuthorizationUnavailable)?;
    Ok(AuthenticatedDealerSeriesAdmissionPrewriteV2 {
        authentication_id,
        state_account_id: id(state_account.key),
        state_pre_content_id,
        key,
        owner_admission_receipt_id,
        capability_profile_id: Id::from_bytes(registry.capability_profile_id().bytes()),
        rent,
    })
}

fn authenticate_existing_dealer_series_admission_v1(
    authorization: AuthenticatedSeriesDealerAuthorizationV1,
    state_account: &AccountInfo<'_>,
    state: DealerStateV3,
    obligation_account: &AccountInfo<'_>,
    obligation: DealerSeriesObligationBindingV1,
) -> Outcome<AuthenticatedExistingDealerSeriesAdmissionV1> {
    state.validate().map_err(dealer_fault)?;
    obligation.validate().map_err(dealer_fault)?;
    let binding_id = obligation.binding_id().map_err(dealer_fault)?;
    require(
        !authorization.requires_product_admission()
            && authorization.dealer_status()
                == clutch_product_series::SeriesLinkObligationStatusV1::Live
            && authorization.dealer_admission_receipt_id().bytes()
                == obligation.admission_owner_receipt_id.bytes()
            && state.series_obligation_children == 1
            && state.series_obligation_binding_account_id == id(obligation_account.key)
            && state.series_obligation_binding_id == binding_id
            && obligation.phase == DealerSeriesObligationPhaseV1::Live
            && obligation.key.binding_account_id == id(obligation_account.key)
            && obligation.key.dealer_state_account_id == id(state_account.key)
            && obligation.key.policy_id == state.base.policy_id
            && obligation.key.facility_id == state.base.facility_id
            && obligation.key.facility_position_binding_id
                == state.base.facility_position_binding_id
            && obligation.key.market_instance_v2_id.bytes()
                == authorization.market_instance_id().bytes()
            && obligation.key.product_market_root_account_id
                == id(&authorization.product_market_root_account())
            && obligation.key.product_market_binding_id.bytes()
                == authorization.product_market_binding_id().bytes()
            && obligation.key.series_plan_v5_id.bytes()
                == authorization.series_plan_id().bytes()
            && obligation.key.series_market_link_account_id
                == id(&authorization.link_account())
            && obligation.key.attachment_plan_v4_id.bytes()
                == authorization.attachment_plan_id().bytes()
            && obligation.key.product_generation == authorization.generation()
            && obligation.key.series_ordinal == authorization.ordinal()
            && authorization.link_transition_sequence()
                >= obligation.admission_link_transition_sequence
            && obligation.rent.neutral_sink.bytes()
                == authorization.neutral_lamport_sink().bytes(),
        ClutchError::AuthorizationUnavailable,
    )?;
    Ok(AuthenticatedExistingDealerSeriesAdmissionV1 { state, obligation })
}

fn authenticate_existing_dealer_series_admission_v2(
    product: AuthenticatedLiveSeriesDealerObligationV2,
    state_account: &AccountInfo<'_>,
    state: DealerStateV3,
    obligation_account: &AccountInfo<'_>,
    obligation: DealerSeriesObligationBindingV2,
) -> Outcome<AuthenticatedExistingDealerSeriesAdmissionV2> {
    state.validate().map_err(dealer_fault)?;
    obligation.validate().map_err(dealer_fault)?;
    let binding_id = obligation.binding_id().map_err(dealer_fault)?;
    require(
        product.id() != ContentId::ZERO
            && product.root_authentication_id() != ContentId::ZERO
            && product.link_authentication_id() != ContentId::ZERO
            && product.registry_capability_id() != ContentId::ZERO
            && state.series_obligation_children == 1
            && state.series_obligation_binding_account_id == id(obligation_account.key)
            && state.series_obligation_binding_id == binding_id
            && obligation.phase == DealerSeriesObligationPhaseV1::Live
            && obligation.key.binding_account_id == id(obligation_account.key)
            && obligation.key.dealer_state_account_id == id(state_account.key)
            && obligation.key.policy_id == state.base.policy_id
            && obligation.key.facility_id == state.base.facility_id
            && obligation.key.facility_position_binding_id
                == state.base.facility_position_binding_id
            && obligation.key.market_instance_v2_id.bytes()
                == product.market_instance_id().bytes()
            && obligation.key.product_market_root_account_id == id(&product.root_account())
            && obligation.key.product_market_binding_id
                == Id::from_bytes(product.root_binding_id().bytes())
            && obligation.key.series_plan_v5_id.bytes() == product.series_plan_id().bytes()
            && obligation.key.series_market_link_account_id == id(&product.link_account())
            && obligation.key.compiler_bundle_v6_id.bytes()
                == product.compiler_bundle_id().bytes()
            && obligation.key.attachment_plan_v5_id.bytes()
                == product.attachment_plan_id().bytes()
            && obligation.key.policy_id.bytes()
                == product.liquidity_facility_plan_id().bytes()
            && obligation.key.product_generation == product.generation()
            && obligation.key.series_ordinal == product.ordinal()
            && product.dealer_admission_receipt_id().bytes()
                == obligation.admission_projection_id.bytes()
            && product.link_transition_sequence()
                >= obligation.admission_link_transition_sequence
            && obligation.rent.payer.bytes() == product.rent_refund_owner().bytes()
            && obligation.rent.neutral_sink.bytes()
                == product.neutral_lamport_sink().bytes(),
        ClutchError::AuthorizationUnavailable,
    )?;
    Ok(AuthenticatedExistingDealerSeriesAdmissionV2 {
        state,
        obligation,
        product,
    })
}

/// Freeze Dealer's exact current terminal cut for Product's sole LinkV2 writer.
///
/// No Product bytes are mutated here. The returned non-Copy value is useful
/// only to the Product-owned writer that will hostile-reopen the same RootV2,
/// LinkV2, RegistryCapabilityV4, BundleV6, and AttachmentV5 immediately before
/// changing Dealer's obligation status from Live to Terminal.
#[inline(never)]
fn authenticate_dealer_series_terminal_prewrite_v2(
    program_id: &Pubkey,
    existing: AuthenticatedExistingDealerSeriesAdmissionV2,
    state_account: &AccountInfo<'_>,
    obligation_account: &AccountInfo<'_>,
    replay: &DealerFacilityReplayV1,
    terminal_state_receipt: DealerTerminalStateReceiptV2,
) -> Outcome<AuthenticatedDealerSeriesTerminalPrewriteV2> {
    let state = existing.state;
    let obligation = existing.obligation;
    let product = existing.product;
    state.validate().map_err(dealer_fault)?;
    obligation.validate().map_err(dealer_fault)?;
    replay.validate().map_err(dealer_fault)?;
    terminal_state_receipt.validate().map_err(dealer_fault)?;

    let state_presemantic_id = state.state_id().map_err(dealer_fault)?;
    let state_base_presemantic_id = state.base.state_content_id().map_err(dealer_fault)?;
    let obligation_presemantic_id = obligation.binding_id().map_err(dealer_fault)?;
    let replay_presemantic_id = replay.replay_id().map_err(dealer_fault)?;
    let terminal_state_receipt_id = terminal_state_receipt
        .receipt_id()
        .map_err(dealer_fault)?;
    let expected_link_transition_sequence = product
        .link_transition_sequence()
        .checked_add(1)
        .ok_or(ClutchError::Arithmetic)?;
    let owner_terminal_receipt_id = obligation
        .terminal_owner_receipt_id(
            terminal_state_receipt_id,
            Id::from_bytes(product.link_semantic_id().bytes()),
            expected_link_transition_sequence,
        )
        .map_err(dealer_fault)?;
    let rent_floor = obligation
        .rent
        .refundable_principal
        .checked_add(obligation.rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;

    require(
        product.id() != ContentId::ZERO
            && product.root_authentication_id() != ContentId::ZERO
            && product.link_authentication_id() != ContentId::ZERO
            && product.registry_capability_id() != ContentId::ZERO
            && state_account.owner == program_id
            && state_account.is_writable
            && !state_account.is_signer
            && !state_account.executable
            && state_account.data_len() == DEALER_STATE_V3_ACCOUNT_BYTES
            && id(state_account.key) == terminal_state_receipt.dealer_state_account_id
            && id(state_account.key) == obligation.key.dealer_state_account_id
            && obligation_account.owner == program_id
            && obligation_account.is_writable
            && !obligation_account.is_signer
            && !obligation_account.executable
            && obligation_account.data_len() == DEALER_SERIES_OBLIGATION_ACCOUNT_BYTES_V2
            && id(obligation_account.key) == obligation.key.binding_account_id
            && obligation_account.lamports() >= rent_floor
            && state.base.phase == DealerPhaseV2::Retiring
            && state.series_obligation_children == 1
            && state.series_obligation_binding_account_id == obligation.key.binding_account_id
            && state.series_obligation_binding_id == obligation_presemantic_id
            && obligation.phase == DealerSeriesObligationPhaseV1::Live
            && obligation.admission_projection_id
                == Id::from_bytes(product.dealer_admission_receipt_id().bytes())
            && obligation.key.product_market_root_account_id == id(&product.root_account())
            && obligation.key.product_market_binding_id
                == Id::from_bytes(product.root_binding_id().bytes())
            && obligation.key.series_market_link_account_id == id(&product.link_account())
            && obligation.key.series_plan_v5_id.bytes() == product.series_plan_id().bytes()
            && obligation.key.series_ordinal == product.ordinal()
            && obligation.key.market_instance_v2_id.bytes()
                == product.market_instance_id().bytes()
            && obligation.key.product_generation == product.generation()
            && obligation.key.compiler_bundle_v6_id.bytes()
                == product.compiler_bundle_id().bytes()
            && obligation.key.attachment_plan_v5_id.bytes()
                == product.attachment_plan_id().bytes()
            && obligation.key.policy_id.bytes()
                == product.liquidity_facility_plan_id().bytes()
            && obligation.rent.payer.bytes() == product.rent_refund_owner().bytes()
            && obligation.rent.neutral_sink.bytes()
                == product.neutral_lamport_sink().bytes()
            && terminal_state_receipt.policy_id == state.base.policy_id
            && terminal_state_receipt.facility_id == state.base.facility_id
            && terminal_state_receipt.facility_position_binding_id
                == state.base.facility_position_binding_id
            && terminal_state_receipt.terminal_state_content_id == state_base_presemantic_id
            && terminal_state_receipt.terminal_position_semantic_id
                == state.base.facility_position_id
            && terminal_state_receipt.replay_account_id == state.base.facility_replay_account_id
            && terminal_state_receipt.terminal_generation == state.base.generation
            && terminal_state_receipt.terminal_child_sequence == state.base.child_sequence
            && replay.replay_account_id() == state.base.facility_replay_account_id
            && replay.position_generation() == state.base.generation
            && replay.lifecycle() == clutch_retirement::ReplayV3Lifecycle::Live
            && replay.next_transition_ordinal() != 0
            && expected_link_transition_sequence > obligation.admission_link_transition_sequence,
        ClutchError::AuthorizationUnavailable,
    )?;

    let authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            DEALER_SERIES_TERMINAL_PREWRITE_DOMAIN_V2,
            program_id.as_ref(),
            &product.id().bytes(),
            &product.root_authentication_id().bytes(),
            &product.link_authentication_id().bytes(),
            &product.link_semantic_id().bytes(),
            &product.link_transition_sequence().to_le_bytes(),
            state_account.key.as_ref(),
            &state_presemantic_id.bytes(),
            obligation_account.key.as_ref(),
            &obligation_presemantic_id.bytes(),
            &terminal_state_receipt_id.bytes(),
            &replay_presemantic_id.bytes(),
            &replay.next_transition_ordinal().to_le_bytes(),
            &owner_terminal_receipt_id.bytes(),
            &expected_link_transition_sequence.to_le_bytes(),
            &obligation.rent.refundable_principal.to_le_bytes(),
            &obligation.rent.donation_floor.to_le_bytes(),
            &obligation.rent.payer.bytes(),
            &obligation.rent.neutral_sink.bytes(),
        ])
        .to_bytes(),
    );
    require(authentication_id != ContentId::ZERO, ClutchError::AuthorizationUnavailable)?;
    Ok(AuthenticatedDealerSeriesTerminalPrewriteV2 {
        authentication_id,
        live_product: product,
        obligation_account: *obligation_account.key,
        obligation_presemantic_id: ContentId::from_bytes(obligation_presemantic_id.bytes()),
        state_account: *state_account.key,
        state_presemantic_id: ContentId::from_bytes(state_presemantic_id.bytes()),
        terminal_state_receipt,
        terminal_state_receipt_id: ContentId::from_bytes(terminal_state_receipt_id.bytes()),
        replay_presemantic_id: ContentId::from_bytes(replay_presemantic_id.bytes()),
        replay_pre_ordinal: replay.next_transition_ordinal(),
        owner_terminal_receipt_id: ContentId::from_bytes(owner_terminal_receipt_id.bytes()),
        expected_link_transition_sequence,
        rent_refund_owner: Pubkey::new_from_array(obligation.rent.payer.bytes()),
        neutral_lamport_sink: Pubkey::new_from_array(obligation.rent.neutral_sink.bytes()),
    })
}

#[inline(never)]
fn authenticate_dealer_collateral_value_v2(
    program_id: &Pubkey,
    policy: &clutch_dealer_runtime_contract::DealerPolicyV1,
    position_binding: Option<&FacilityPositionBindingV2>,
    accounts: DealerCollateralAuthorityAccountsV2<'_, '_>,
) -> Outcome<(GeneralMarketValueAuthorityV2, DealerPositionMarketJoinV2)> {
    let value = authenticate_general_market_value_authority_v2(
        program_id,
        accounts.realm,
        accounts.profile,
        accounts.policy,
        accounts.token_program,
        accounts.token_programdata,
        accounts.market_binding,
        accounts.market_runtime,
        accounts.market_instance,
        accounts.hoard,
        accounts.claim_ledger,
        false,
        false,
    )?;
    let liabilities = value.liabilities;
    let bound = liabilities.bound;
    let realm = bound.realm_bound().realm();
    let collateral_policy = bound.policy();
    let release = bound.release();
    let release_id = release
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let market = liabilities.market_binding.base();
    require(
        policy.market_instance_v2_id.bytes() == market.market_instance_v2_id.bytes()
            && policy.realm_id == Id::from_bytes(realm.realm.bytes())
            && policy.profile_id == Id::from_bytes(realm.profile.bytes())
            && policy.claim_basis_id.bytes() == market.native_claim_basis_id.bytes()
            && policy.collateral_mint == Id::from_bytes(collateral_policy.mint.bytes())
            && policy.token_program == Id::from_bytes(release.token_program.bytes())
            && policy.outcome_count == market.outcome_count
            && liabilities.hoard.outcome_count == policy.outcome_count
            && liabilities.claim_ledger.outcome_count == policy.outcome_count
            && liabilities.hoard_semantic_id
                == liabilities
                    .hoard
                    .semantic_id(&RuntimeSha256)
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && liabilities.claim_ledger_semantic_id
                == liabilities
                    .claim_ledger
                    .semantic_id(&RuntimeSha256)
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ClutchError::MismatchedState,
    )?;
    if let Some(binding) = position_binding {
        require(
            binding.market_instance_v2_id == policy.market_instance_v2_id
                && binding.collateral_policy_id == Id::from_bytes(bound.policy_id().bytes())
                && binding.collateral_release_id == Id::from_bytes(release_id.bytes()),
            ClutchError::MismatchedState,
        )?;
    }
    let join = DealerPositionMarketJoinV2 {
        market_instance_v2_id: policy.market_instance_v2_id,
        realm_id: policy.realm_id,
        collateral_policy_id: Id::from_bytes(bound.policy_id().bytes()),
        collateral_release_id: Id::from_bytes(release_id.bytes()),
        collateral_value_receipt_id: Id::from_bytes(value.receipt_id.bytes()),
        outcome_count: policy.outcome_count,
    };
    Ok((value, join))
}

fn dealer_general_replay_value_evidence_id_v2(
    collateral_value_receipt_id: Id,
    liveness_receipt_id: Id,
) -> Outcome<clutch_general_v2_contract::Id32> {
    collateral_value_receipt_id
        .validate_live()
        .map_err(dealer_fault)?;
    liveness_receipt_id.validate_live().map_err(dealer_fault)?;
    Ok(clutch_general_v2_contract::Id32::new(
        solana_sha256_hasher::hashv(&[
            DEALER_GENERAL_REPLAY_VALUE_EVIDENCE_DOMAIN_V2,
            &collateral_value_receipt_id.bytes(),
            &liveness_receipt_id.bytes(),
        ])
        .to_bytes(),
    )?)
}

fn current_general_replay_sequence_v1(account: &AccountInfo<'_>) -> Outcome<u64> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let replay = ReplayV3Envelope::decode(&data, &RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::Replay))?;
    Ok(replay.header().next_sequence())
}

fn prepare_dealer_general_replay_v2(
    authority: GeneralPositionReplayAuthorityV2,
    position_post: PositionAccountV3,
    kind: GeneralReplayTransitionKindV1,
    transfer_bundle_id: Id,
    collateral_value_receipt_id: Id,
    action_evidence_id: Id,
) -> Outcome<GeneralReplayTransitionPlanV1> {
    let position_poststate = PositionSettlementPoststateV3 {
        account: authority.position.account,
        general_market_runtime: authority.position.general_market_runtime,
        prestate_semantic_id: authority.position.semantic_id,
        semantic: position_post,
    };
    project_general_replay_transition_v1(
        authority.replay,
        position_poststate,
        kind,
        clutch_general_v2_contract::Id32::new(transfer_bundle_id.bytes())?,
        dealer_general_replay_value_evidence_id_v2(
            collateral_value_receipt_id,
            action_evidence_id,
        )?,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::Replay))
}

fn write_and_accept_general_replay_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    plan: &GeneralReplayTransitionPlanV1,
) -> Outcome<Id> {
    require(
        account.owner == program_id
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && account.key.to_bytes() == plan.replay_account().bytes(),
        ClutchError::MismatchedState,
    )?;
    account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(plan.replay_poststate_body());
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let observed = ReplayV3Envelope::decode(&data, &RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::Replay))?;
    let observed_id = observed
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::Replay))?;
    require(
        data.as_ref() == plan.replay_poststate_body()
            && observed_id.bytes() == plan.replay_poststate_semantic_id().bytes(),
        ClutchError::Replay,
    )?;
    Ok(Id::from_bytes(observed_id.bytes()))
}

/// Static source for the adapter's heap-resident complete RelationV2 book.
///
/// Copying this through `boxed_copy_of` avoids materializing the 12-KiB-plus
/// fixed book in any SBF call frame before the streaming page cursor fills it.
static EMPTY_DEALER_ECONOMIC_BOOK_V2: EconomicBookV2 = EconomicBookV2::empty();

/// Static source for heap-first hostile decoding of one signed Dealer quote.
static EMPTY_DEALER_QUOTE_ADMISSION_V1:
    clutch_dealer_runtime_contract::DealerQuoteAdmissionV1 =
    clutch_dealer_runtime_contract::DealerQuoteAdmissionV1::ZEROED;

/// Static source for the heap-owned checked Dealer verdict postimage.
static EMPTY_DEALER_LEG_VERDICT_V2: DealerLegVerdictV2 = DealerLegVerdictV2::ZEROED;

/// Static source for heap-owned creation of the 5,436-byte selection body.
static EMPTY_COVERED_DEALER_SELECTION_V1: CoveredDealerSelectionV1 =
    CoveredDealerSelectionV1::ZEROED;

fn id(key: &Pubkey) -> Id {
    Id::from_bytes(key.to_bytes())
}

const fn id_from_content(value: ContentId) -> Id {
    Id::from_bytes(value.bytes())
}

fn retirement_id(value: Id) -> Outcome<Identity32V1> {
    Identity32V1::new(value.bytes()).map_err(|_| ClutchError::MismatchedState.into())
}

fn liveness_id(value: Id) -> clutch_liveness::Id {
    clutch_liveness::Id::from_bytes(value.bytes())
}

fn position_semantic_id(position: PositionAccountV3) -> Outcome<Id> {
    Ok(Id::from_bytes(
        position
            .semantic_id(&RuntimeSha256)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes(),
    ))
}

fn select_begin_rent_principal(
    receipt: u64,
    selection: u64,
    lease: u64,
    pot: u64,
    keeper_payment: u64,
) -> Outcome<u64> {
    let total = receipt
        .checked_add(selection)
        .and_then(|value| value.checked_add(lease))
        .and_then(|value| value.checked_add(pot))
        .ok_or(ClutchError::Arithmetic)?;
    require(keeper_payment >= total, ClutchError::MismatchedState)?;
    Ok(total)
}

const fn runtime_kind_seed(kind: RuntimeCompartmentKindV1) -> u8 {
    match kind {
        RuntimeCompartmentKindV1::Source => 0,
        RuntimeCompartmentKindV1::Candidate => 1,
        RuntimeCompartmentKindV1::Clearing => 2,
        RuntimeCompartmentKindV1::Settlement => 3,
        RuntimeCompartmentKindV1::Resolution => 4,
        RuntimeCompartmentKindV1::Retirement => 5,
        RuntimeCompartmentKindV1::Recovery => 6,
    }
}

#[inline(never)]
fn prepare_runtime_compartment_admission(
    program_id: &Pubkey,
    facility_id: Id,
    state_account_id: Id,
    payer: &AccountInfo<'_>,
    account: &AccountInfo<'_>,
    policy: RuntimeLivenessPolicyV1,
    kind: RuntimeCompartmentKindV1,
    required_rent_principal: u64,
) -> Outcome<(RuntimeCompartmentV1, u8)> {
    require_creatable(account)?;
    require(account.is_writable, ClutchError::NotWritable)?;
    require(
        !account.is_signer && !account.executable,
        ClutchError::MismatchedState,
    )?;
    let compartment_policy = policy.compartment(kind);
    require(
        compartment_policy.account_rent_principal_lamports == required_rent_principal,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    let (address, bump) = seeds::dealer_runtime_liveness_account_pda(
        program_id,
        &facility_id.bytes(),
        runtime_kind_seed(kind),
    );
    expect_pda(account.key, (address, bump), None)?;
    let payer_debit = compartment_policy
        .total_payer_debit_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let balance_before = account.lamports();
    let balance_after = balance_before
        .checked_add(payer_debit)
        .ok_or(ClutchError::Arithmetic)?;
    let semantic_owner = if kind == RuntimeCompartmentKindV1::Source {
        compartment_policy.receipt_program_id
    } else {
        liveness_id(state_account_id)
    };
    let state = RuntimeCompartmentV1::admit(
        policy,
        RuntimeCompartmentAdmissionV1 {
            kind,
            identity: RuntimeCompartmentIdentityV1 {
                policy_id: policy.policy_id,
                lifecycle_id: liveness_id(facility_id),
                account_id: clutch_liveness::Id::from_bytes(account.key.to_bytes()),
                owner: semantic_owner,
                payer: clutch_liveness::Id::from_bytes(payer.key.to_bytes()),
                neutral_sink: policy.neutral_sink,
                generation: 0,
            },
            funding: PresentFundingV1 {
                payer: clutch_liveness::Id::from_bytes(payer.key.to_bytes()),
                source: PresentFundingSourceV1::ExternalSignerNativeLamports,
                payer_debit_lamports: payer_debit,
                account_balance_before: balance_before,
                account_balance_after: balance_after,
            },
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok((state, bump))
}

#[inline(never)]
fn dealer_body<T: FixedCodec>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
    tag: u8,
    version: u8,
    account_bytes: usize,
) -> Outcome<(u8, T)> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(
        account.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(
        account.data_len() == account_bytes,
        ClutchError::WrongDataLength,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (envelope, body) =
        decode_dealer_account_body_v1::<T>(&data, tag, version).map_err(dealer_fault)?;
    Ok((envelope.bump, body))
}

fn dealer_transfer_endpoint_semantic_id_v2(
    program_id: &Pubkey,
    kind: DealerAssetEndpointKindV1,
    account: &AccountInfo<'_>,
) -> Outcome<Id> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(
        account.is_writable && !account.is_signer && !account.executable,
        ClutchError::MismatchedState,
    )?;
    match kind {
        DealerAssetEndpointKindV1::GeneralPosition
        | DealerAssetEndpointKindV1::FacilityPosition => {
            require(
                account.data_len() == POSITION_V3_BYTES,
                ClutchError::WrongDataLength,
            )?;
            let position = PositionAccountV3::decode(&account.data.borrow())
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            position
                .semantic_id(&RuntimeSha256)
                .map(|value| Id::from_bytes(value.bytes()))
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
        }
        DealerAssetEndpointKindV1::SettlementPot => {
            let (_, pot) = dealer_body::<SettlementPotV2>(
                program_id,
                account,
                true,
                DEALER_SETTLEMENT_POT_V2_ACCOUNT_TAG,
                DEALER_SETTLEMENT_POT_V2_ACCOUNT_VERSION,
                DEALER_SETTLEMENT_POT_V2_ACCOUNT_BYTES,
            )?;
            pot.pot_content_id().map_err(dealer_fault)
        }
    }
}

/// Hostile-reload both semantic owners after an internal Dealer movement.
/// The returned identity is the exact V2 transfer bundle committed by Replay.
fn accept_dealer_asset_transfer_postwrite_v2(
    program_id: &Pubkey,
    bundle: DealerAssetTransferBundleV2,
    first: &AccountInfo<'_>,
    second: &AccountInfo<'_>,
) -> Outcome<Id> {
    require(first.key != second.key, ClutchError::AccountAlias)?;
    let first_id = id(first.key);
    let second_id = id(second.key);
    let (source, destination) = if first_id == bundle.source_account_id
        && second_id == bundle.destination_account_id
    {
        (first, second)
    } else if second_id == bundle.source_account_id && first_id == bundle.destination_account_id {
        (second, first)
    } else {
        return Err(ClutchError::MismatchedState.into());
    };
    let source_post_semantic_id =
        dealer_transfer_endpoint_semantic_id_v2(program_id, bundle.source_kind, source)?;
    let destination_post_semantic_id =
        dealer_transfer_endpoint_semantic_id_v2(program_id, bundle.destination_kind, destination)?;
    accept_dealer_asset_transfer_v2(
        bundle,
        DealerAssetTransferPostObservationV2 {
            source_account_id: id(source.key),
            destination_account_id: id(destination.key),
            source_post_semantic_id,
            destination_post_semantic_id,
        },
    )
    .map_err(dealer_fault)
}

fn authenticate_state_with_access(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
) -> Outcome<DealerStateV2> {
    let (bump, state) = dealer_body::<DealerStateV2>(
        program_id,
        account,
        writable,
        DEALER_STATE_V2_ACCOUNT_TAG,
        DEALER_STATE_V2_ACCOUNT_VERSION,
        DEALER_STATE_V2_ACCOUNT_BYTES,
    )?;
    expect_pda(
        account.key,
        seeds::dealer_state_v2_pda(program_id, &state.facility_id.bytes()),
        Some(bump),
    )?;
    let floor = state
        .rent
        .refundable_live_principal
        .checked_add(state.rent.permanent_tombstone_principal)
        .and_then(|value| value.checked_add(state.rent.donation_floor))
        .ok_or(ClutchError::Arithmetic)?;
    require(
        account.lamports() >= floor,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    Ok(state)
}

fn authenticate_state(program_id: &Pubkey, account: &AccountInfo<'_>) -> Outcome<DealerStateV2> {
    authenticate_state_with_access(program_id, account, true)
}

fn authenticate_live_series_obligation_for_state_v3(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    state_account: &AccountInfo<'_>,
    state: &DealerStateV3,
) -> Outcome<DealerSeriesObligationBindingV1> {
    let authenticated = authenticate_dealer_series_obligation_v1(program_id, account, false)?;
    let binding = *authenticated.binding();
    let binding_id = binding.binding_id().map_err(dealer_fault)?;
    require(
        state.series_obligation_children == 1
            && state.series_obligation_binding_account_id == authenticated.account_id()
            && state.series_obligation_binding_id == binding_id
            && binding.phase == DealerSeriesObligationPhaseV1::Live
            && binding.key.binding_account_id == authenticated.account_id()
            && binding.key.policy_id == state.base.policy_id
            && binding.key.facility_id == state.base.facility_id
            && binding.key.dealer_state_account_id == id(state_account.key)
            && binding.key.facility_position_binding_id
                == state.base.facility_position_binding_id
            && binding.rent.neutral_sink == state.base.rent.neutral_sink,
        ClutchError::MismatchedState,
    )?;
    Ok(binding)
}

/// Authenticate the current Product RootV2/LinkV2 owner of a live Dealer
/// obligation. The two large Product bodies are decoded in disjoint lexical
/// scopes and collapsed only to their canonical semantic/authentication IDs.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_current_product_resolution_v2(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    obligation: &DealerSeriesObligationBindingV2,
    state_account: &AccountInfo<'_>,
    state: &DealerStateV3,
    policy: &clutch_dealer_runtime_contract::DealerPolicyV1,
    value_authority: GeneralMarketValueAuthorityV2,
) -> Outcome<AuthenticatedDealerProductResolutionV2> {
    obligation.validate().map_err(dealer_fault)?;
    let key = obligation.key;
    let bound = value_authority.liabilities.bound;
    let realm_binding = bound.realm_bound().realm();
    let release_id = bound.release().id().map_err(|_| {
        Refusal::Adapter(ClutchError::MismatchedState)
    })?;
    require(
        obligation.phase == clutch_dealer_runtime_contract::DealerSeriesObligationPhaseV1::Live
            && state.series_obligation_children == 1
            && state.series_obligation_binding_account_id == key.binding_account_id
            && state.series_obligation_binding_id == obligation.binding_id().map_err(dealer_fault)?
            && key.policy_id == state.base.policy_id
            && key.facility_id == state.base.facility_id
            && key.dealer_state_account_id == id(state_account.key)
            && key.facility_position_binding_id == state.base.facility_position_binding_id
            && key.market_instance_v2_id == policy.market_instance_v2_id
            && key.product_market_root_account_id == id(root_account.key)
            && key.series_market_link_account_id == id(link_account.key),
        ClutchError::MismatchedState,
    )?;

    let (
        root_semantic_id,
        root_authentication_id,
        root_capability_profile_id,
        resolution_semantic_id,
        resolution_data_id,
    ) = {
        let mut root_body = MarketLifecycleRootAccountV2::decode_buffer();
        let root = authenticate_market_lifecycle_root_v2(
            program_id,
            root_account,
            MarketInstanceV2Id::from_bytes(key.market_instance_v2_id.bytes()),
            key.product_generation,
            false,
            &mut root_body,
        )?;
        let root_state = root.state();
        let binding = root_state.binding();
        let semantic_id = root_state
            .semantic_id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            root_state.phase() == MarketLifecyclePhaseV2::Active
                && binding.id().map_err(|_| {
                    Refusal::Adapter(ClutchError::MismatchedState)
                })?.bytes() == key.product_market_binding_id.bytes()
                && binding.market_instance_id.bytes() == key.market_instance_v2_id.bytes()
                && binding.generation == key.product_generation
                && binding.outcome_count == policy.outcome_count
                && binding.realm_id.bytes() == realm_binding.realm.bytes()
                && binding.collateral_profile_id.bytes() == realm_binding.profile.bytes()
                && binding.collateral_policy_id.bytes() == bound.policy_id().bytes()
                && binding.collateral_release_id.bytes() == release_id.bytes()
                && root_state.resolution_activation_receipt_id() != ContentId::ZERO,
            ClutchError::MismatchedState,
        )?;
        (
            id_from_content(semantic_id),
            id_from_content(root.authentication_id()),
            id_from_content(binding.capability_profile_id),
            id_from_content(root_state.resolution_semantic_id()),
            id_from_content(root_state.resolution_data_id()),
        )
    };

    let (link_semantic_id, link_authentication_id) = {
        let mut link_body = SeriesMarketLinkAccountV2::decode_buffer();
        let link = authenticate_series_market_link_v2(
            program_id,
            link_account,
            SeriesPlanV5Id::from_bytes(key.series_plan_v5_id.bytes()),
            key.series_ordinal,
            MarketInstanceV2Id::from_bytes(key.market_instance_v2_id.bytes()),
            key.product_generation,
            *root_account.key,
            false,
            &mut link_body,
        )?;
        let link_state = *link.state();
        let binding = link_state.binding();
        let semantic_id = link_state
            .semantic_id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            link_state.phase() == SeriesMarketLinkPhaseV2::Active
                && binding.market_binding_id.bytes() == key.product_market_binding_id.bytes()
                && binding.compiler_bundle_id.bytes() == key.compiler_bundle_v6_id.bytes()
                && binding.attachment_plan_id.bytes() == key.attachment_plan_v5_id.bytes()
                && binding.capability_profile_id.bytes()
                    == root_capability_profile_id.bytes()
                && binding.rent_refund_owner.bytes() == obligation.rent.payer.bytes()
                && binding.neutral_lamport_sink.bytes()
                    == obligation.rent.neutral_sink.bytes(),
            ClutchError::MismatchedState,
        )?;
        require(
            link_state.obligation_status(SeriesLinkObligationV2::Dealer)
                == SeriesLinkObligationStatusV2::Live
                && link_state
                    .obligation_admission_receipt_id(SeriesLinkObligationV2::Dealer)
                    .bytes()
                    == obligation.admission_projection_id.bytes()
                && link_state.transition_sequence()
                    >= obligation.admission_link_transition_sequence,
            ClutchError::MismatchedState,
        )?;
        (
            id_from_content(semantic_id.content_id()),
            id_from_content(link.authentication_id()),
        )
    };

    let authentication_id = Id::from_bytes(
        solana_sha256_hasher::hashv(&[
            DEALER_PRODUCT_RESOLUTION_AUTHENTICATION_DOMAIN_V2,
            root_account.key.as_ref(),
            link_account.key.as_ref(),
            &root_semantic_id.bytes(),
            &root_authentication_id.bytes(),
            &link_semantic_id.bytes(),
            &link_authentication_id.bytes(),
            &obligation.binding_id().map_err(dealer_fault)?.bytes(),
            &state.state_id().map_err(dealer_fault)?.bytes(),
            &value_authority.receipt_id.bytes(),
            &resolution_semantic_id.bytes(),
            &resolution_data_id.bytes(),
        ])
        .to_bytes(),
    );
    authentication_id.validate_live().map_err(dealer_fault)?;
    Ok(AuthenticatedDealerProductResolutionV2 {
        root_account_id: id(root_account.key),
        root_semantic_id,
        link_account_id: id(link_account.key),
        link_semantic_id,
        resolution_semantic_id,
        resolution_data_id,
        authentication_id,
    })
}

fn authenticate_dependency(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    facility_id: Id,
) -> Outcome<DealerFundedDependenciesV2> {
    let (bump, dependency) = dealer_body::<DealerFundedDependenciesV2>(
        program_id,
        account,
        false,
        DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG,
        DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION,
        DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES,
    )?;
    expect_pda(
        account.key,
        seeds::dealer_funded_v2_pda(program_id, &facility_id.bytes()),
        Some(bump),
    )?;
    let floor = dependency
        .rent
        .refundable_principal
        .checked_add(dependency.rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        account.lamports() >= floor,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    Ok(dependency)
}

fn authenticate_future_credit_funding(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
) -> Outcome<(u8, DealerFutureCreditFundingV1)> {
    let (bump, funding) = dealer_body::<DealerFutureCreditFundingV1>(
        program_id,
        account,
        writable,
        DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_TAG,
        DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_VERSION,
        DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_BYTES,
    )?;
    expect_pda(
        account.key,
        seeds::dealer_future_credit_funding_pda(program_id, &funding.facility_id.bytes()),
        Some(bump),
    )?;
    require(
        funding.funding_account_id == id(account.key)
            && account.lamports()
                >= funding
                    .minimum_balance_lamports()
                    .map_err(dealer_fault)?,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    Ok((bump, funding))
}

/// Delete the unused future-credit funding owner and partition its balance.
///
/// This helper is intentionally private and has no raw-plan caller. The final
/// action25 outer must first persist Product's LinkV2 terminal successor and
/// then pass the hostile-reopened terminal `0xaf/v2` and Retiring StateV3.
/// The alternative live-a6 path is owned by Fractional and cannot call this
/// function after action23 has deleted `0xbc/v1`.
#[inline(never)]
fn close_unused_future_credit_funding_v1(
    program_id: &Pubkey,
    state_account: &AccountInfo<'_>,
    terminal_obligation_account: &AccountInfo<'_>,
    funding_account: &AccountInfo<'_>,
    refund_owner: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
) -> Outcome<AuthenticatedDealerFutureCreditUnusedCloseV1> {
    let authenticated_state = authenticate_dealer_state_v3(program_id, state_account, true)?;
    let authenticated_obligation =
        authenticate_dealer_series_obligation_v2(program_id, terminal_obligation_account, true)?;
    let (_, funding) = authenticate_future_credit_funding(program_id, funding_account, true)?;
    require(
        refund_owner.is_writable
            && neutral_sink.is_writable
            && !refund_owner.executable
            && neutral_sink.owner == &SYSTEM_PROGRAM_ID
            && neutral_sink.data_is_empty()
            && !neutral_sink.is_signer
            && !neutral_sink.executable
            && funding_account.key != refund_owner.key
            && funding_account.key != neutral_sink.key
            && refund_owner.key != neutral_sink.key
            && funding.refund_owner == id(refund_owner.key)
            && funding.neutral_sink == id(neutral_sink.key),
        ClutchError::MismatchedState,
    )?;
    let plan = funding
        .prepare_unused_close(
            id(state_account.key),
            authenticated_state.state(),
            authenticated_obligation.binding(),
            funding_account.lamports(),
        )
        .map_err(dealer_fault)?;
    require(
        plan.funding_account_id == id(funding_account.key)
            && plan.refund_owner == id(refund_owner.key)
            && plan.neutral_sink == id(neutral_sink.key)
            && plan.terminal_obligation_binding_id
                == authenticated_obligation.binding().binding_id().map_err(dealer_fault)?,
        ClutchError::MismatchedState,
    )?;

    let refund_before = refund_owner.lamports();
    let neutral_before = neutral_sink.lamports();
    release_dealer_account(funding_account)?;
    credit_exact_dealer_terminal_lamports([
        (refund_owner, plan.refundable_principal_lamports),
        (neutral_sink, plan.neutral_sink_credit_lamports),
        (neutral_sink, 0),
        (neutral_sink, 0),
    ])?;
    require_released_dealer_account(funding_account)?;
    let refund_after = refund_before
        .checked_add(plan.refundable_principal_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let neutral_after = neutral_before
        .checked_add(plan.neutral_sink_credit_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        refund_owner.lamports() == refund_after && neutral_sink.lamports() == neutral_after,
        ClutchError::MismatchedState,
    )?;
    let postwrite_receipt_id = Id::from_bytes(
        solana_sha256_hasher::hashv(&[
            DEALER_FUTURE_CREDIT_UNUSED_CLOSE_POSTWRITE_DOMAIN_V1,
            &plan.terminal_receipt_id.bytes(),
            &plan.state_pre_semantic_id.bytes(),
            &plan.terminal_state_receipt_id.bytes(),
            &plan.terminal_obligation_binding_id.bytes(),
            &plan.terminal_product_projection_id.bytes(),
            &plan.terminal_link_post_semantic_id.bytes(),
            &plan.terminal_link_transition_sequence.to_le_bytes(),
            funding_account.key.as_ref(),
            refund_owner.key.as_ref(),
            neutral_sink.key.as_ref(),
            &plan.observed_balance_lamports.to_le_bytes(),
            &refund_before.to_le_bytes(),
            &refund_after.to_le_bytes(),
            &neutral_before.to_le_bytes(),
            &neutral_after.to_le_bytes(),
        ])
        .to_bytes(),
    );
    postwrite_receipt_id.validate_live().map_err(dealer_fault)?;
    Ok(AuthenticatedDealerFutureCreditUnusedCloseV1 {
        plan,
        postwrite_receipt_id,
        refund_owner_lamports_after: refund_after,
        neutral_sink_lamports_after: neutral_after,
    })
}

impl AuthenticatedDealerFacilityVectorAuthorityV1
    for AuthenticatedDealerFacilityVectorAuthoritySbfV1<'_, '_>
{
    fn fractional_vector_prestate_v1(&self) -> BoundDealerFacilityVectorPrestateV1 {
        self.prestate
    }

    #[inline(never)]
    fn consume_future_credit_prefund_v1(
        self,
        program_id: &Pubkey,
        fractional_policy_account: Identity32V1,
        credit_account: &AccountInfo<'_>,
        system_program: &AccountInfo<'_>,
    ) -> Outcome<AcceptedDealerFacilityCreditFundingV1> {
        require(
            credit_account.key == self.credit_account.key
                && system_program.key == self.system_program.key,
            ClutchError::MismatchedState,
        )?;
        let credit_account = self.credit_account;
        let system_program = self.system_program;
        require_system_program(system_program)?;
        require(
            self.funding_account.is_writable
                && self.refund_owner.is_writable
                && self.neutral_sink.is_writable
                && self.funding.funding_account_id == id(self.funding_account.key)
                && self.funding.refund_owner == id(self.refund_owner.key)
                && self.funding.neutral_sink == id(self.neutral_sink.key)
                && self.neutral_sink.owner == &SYSTEM_PROGRAM_ID
                && self.neutral_sink.data_is_empty()
                && !self.neutral_sink.is_signer
                && !self.neutral_sink.executable
                && self.refund_owner.key != self.neutral_sink.key
                && self.refund_owner.key != credit_account.key
                && self.neutral_sink.key != credit_account.key,
            ClutchError::MismatchedState,
        )?;
        let credit_account_id = id(credit_account.key);
        let consumption = self
            .funding
            .prepare_consumption(
                self.funding_account.lamports(),
                self.current_generation,
                Id::from_bytes(fractional_policy_account.bytes()),
                credit_account_id,
            )
            .map_err(dealer_fault)?;
        require(
            self.live_credit_rent_lamports
                == self
                    .funding
                    .credit_principal_lamports()
                    .map_err(dealer_fault)?
                && self.tombstone_rent_lamports
                    == self.funding.credit_tombstone_principal_lamports
                && self
                    .live_credit_rent_lamports
                    .checked_sub(self.tombstone_rent_lamports)
                    == Some(self.funding.credit_refundable_principal_lamports),
            ClutchError::DealerPolicyRentMismatch,
        )?;

        let expected_credit = seeds::fractional_credit_v2_pda(
            program_id,
            &fractional_policy_account.bytes(),
            &self.funding.facility_id.bytes(),
        );
        expect_pda(credit_account.key, expected_credit, None)?;
        require_creatable(credit_account)?;

        let credit_prefund = credit_account.lamports();
        let neutral_before_sweep = self.neutral_sink.lamports();
        if credit_prefund != 0 {
            let transfer = Instruction::new_with_bytes(
                SYSTEM_PROGRAM_ID,
                &transfer_data(credit_prefund),
                vec![
                    AccountMeta::new(*credit_account.key, true),
                    AccountMeta::new(*self.neutral_sink.key, false),
                ],
            );
            let policy_bytes = fractional_policy_account.bytes();
            let facility_bytes = self.funding.facility_id.bytes();
            let bump = [expected_credit.1];
            let signer = [
                seeds::SEED_FRACTIONAL_CREDIT_V2,
                &policy_bytes,
                &facility_bytes,
                &bump,
            ];
            invoke_signed(
                &transfer,
                &[
                    credit_account.clone(),
                    self.neutral_sink.clone(),
                    system_program.clone(),
                ],
                &[&signer],
            )
            .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
        }
        require(
            credit_account.lamports() == 0
                && self.neutral_sink.lamports()
                    == neutral_before_sweep
                        .checked_add(credit_prefund)
                        .ok_or(ClutchError::Arithmetic)?,
            ClutchError::MismatchedState,
        )?;

        let refund_before = self.refund_owner.lamports();
        let neutral_before_distribution = self.neutral_sink.lamports();
        release_dealer_account(self.funding_account)?;
        credit_exact_dealer_terminal_lamports([
            (
                self.refund_owner,
                consumption.funding_account_principal_lamports,
            ),
            (
                credit_account,
                consumption
                    .credit_refundable_principal_lamports
                    .checked_add(consumption.credit_tombstone_principal_lamports)
                    .ok_or(ClutchError::Arithmetic)?,
            ),
            (
                self.neutral_sink,
                consumption.neutral_sink_credit_lamports,
            ),
            (self.neutral_sink, 0),
        ])?;
        require_released_dealer_account(self.funding_account)?;
        require(
            self.refund_owner.lamports()
                == refund_before
                    .checked_add(consumption.funding_account_principal_lamports)
                    .ok_or(ClutchError::Arithmetic)?
                && self.neutral_sink.lamports()
                    == neutral_before_distribution
                        .checked_add(consumption.neutral_sink_credit_lamports)
                        .ok_or(ClutchError::Arithmetic)?
                && credit_account.lamports() == self.live_credit_rent_lamports,
            ClutchError::MismatchedState,
        )?;

        let allocate = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &allocate_data(FRACTIONAL_CREDIT_ACCOUNT_BYTES),
            vec![AccountMeta::new(*credit_account.key, true)],
        );
        let policy_bytes = fractional_policy_account.bytes();
        let facility_bytes = self.funding.facility_id.bytes();
        let bump = [expected_credit.1];
        let signer = [
            seeds::SEED_FRACTIONAL_CREDIT_V2,
            &policy_bytes,
            &facility_bytes,
            &bump,
        ];
        invoke_signed(
            &allocate,
            &[credit_account.clone(), system_program.clone()],
            &[&signer],
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
        let assign = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &assign_data(program_id),
            vec![AccountMeta::new(*credit_account.key, true)],
        );
        invoke_signed(
            &assign,
            &[credit_account.clone(), system_program.clone()],
            &[&signer],
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
        require(
            credit_account.owner == program_id
                && credit_account.data_len() == FRACTIONAL_CREDIT_ACCOUNT_BYTES
                && credit_account.lamports() == self.live_credit_rent_lamports,
            ClutchError::AccountCreationFailed,
        )?;

        let terminal_postwrite_id = Identity32V1::new(
            solana_sha256_hasher::hashv(&[
                DEALER_FUTURE_CREDIT_POSTWRITE_DOMAIN_V1,
                &consumption.terminal_receipt_id.bytes(),
                &self.prestate.dealer_state_pre_semantic_id().bytes(),
                self.funding_account.key.as_ref(),
                credit_account.key.as_ref(),
                self.refund_owner.key.as_ref(),
                self.neutral_sink.key.as_ref(),
                &credit_prefund.to_le_bytes(),
                &consumption.observed_balance_lamports.to_le_bytes(),
                &self.live_credit_rent_lamports.to_le_bytes(),
                &self.tombstone_rent_lamports.to_le_bytes(),
                &[expected_credit.1],
            ])
            .to_bytes(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let creation = CreditCreationV1::Fresh {
            claimant: retirement_id(self.funding.facility_id)?,
            stored_bump: expected_credit.1,
            rent: RentSplitV2 {
                payer: retirement_id(self.funding.refund_owner)?,
                refundable_live_principal: self.funding.credit_refundable_principal_lamports,
                permanent_tombstone_principal: self.funding.credit_tombstone_principal_lamports,
                donation_floor: 0,
            },
        };
        accept_dealer_facility_credit_funding_v1(
            program_id,
            self.prestate,
            fractional_policy_account,
            credit_account,
            creation,
            retirement_id(consumption.funding_receipt_id)?,
            terminal_postwrite_id,
            retirement_id(self.funding.neutral_sink)?,
        )
    }
}

impl AuthenticatedDealerFacilityCreditTerminalAuthorityV1
    for AuthenticatedDealerFacilityCreditTerminalAuthoritySbfV1
{
    fn fractional_credit_terminal_prestate_v1(&self) -> DealerFacilityCreditTerminalPrestateV1 {
        self.prestate
    }

    fn consume_dealer_facility_credit_terminal_authority_v1(
        self,
        observed: DealerFacilityCreditTerminalObservationV1,
    ) -> Outcome<()> {
        require(
            observed.authorization_id == self.prestate.authorization_id()
                && observed.facility_id == self.prestate.facility_id()
                && observed.market_instance == self.prestate.market_instance()
                && observed.domain_generation == self.prestate.domain_generation()
                && observed.facility_credit_account
                    == self.prestate.facility_credit_account()
                && Id::from_bytes(observed.fractional_ledger_account.bytes())
                    == self.fractional_ledger_account
                && Id::from_bytes(observed.fractional_ledger_before_id.bytes())
                    == self.fractional_ledger_before_id
                && Id::from_bytes(observed.market_root_account.bytes())
                    == self.product_root_account
                && Id::from_bytes(observed.market_root_authentication_id.bytes())
                    == self.product_root_authentication_id
                && Id::from_bytes(observed.resolution_semantic_id.bytes())
                    == self.resolution_semantic_id
                && Id::from_bytes(observed.resolution_data_id.bytes())
                    == self.resolution_data_id
                && Id::from_bytes(observed.stored_payer.bytes()) == self.stored_payer
                && Id::from_bytes(observed.neutral_sink.bytes()) == self.neutral_sink
                && observed.dealer_terminal_state_receipt_id
                    == self.prestate.dealer_terminal_state_receipt_id()
                && observed.product_terminal_receipt_id
                    == self.prestate.product_terminal_receipt_id(),
            ClutchError::MismatchedState,
        )
    }
}

fn authenticate_schedule(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<DealerLivenessScheduleV1> {
    let (bump, schedule) = dealer_body::<DealerLivenessScheduleV1>(
        program_id,
        account,
        false,
        DEALER_LIVENESS_SCHEDULE_ACCOUNT_TAG,
        DEALER_LIVENESS_SCHEDULE_ACCOUNT_VERSION,
        DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES,
    )?;
    let schedule_id = schedule.schedule_id().map_err(dealer_fault)?.untyped();
    expect_pda(
        account.key,
        seeds::dealer_liveness_schedule_pda(program_id, &schedule_id.bytes()),
        Some(bump),
    )?;
    Ok(schedule)
}

fn authenticate_lp_page_with_access(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
) -> Outcome<LpPageV2> {
    let (bump, page) = dealer_body::<LpPageV2>(
        program_id,
        account,
        writable,
        DEALER_LP_PAGE_V2_ACCOUNT_TAG,
        DEALER_LP_PAGE_V2_ACCOUNT_VERSION,
        DEALER_LP_PAGE_V2_ACCOUNT_BYTES,
    )?;
    expect_pda(
        account.key,
        seeds::dealer_lp_page_v2_pda(program_id, &page.facility_id.bytes(), page.page_ordinal),
        Some(bump),
    )?;
    let floor = page
        .rent
        .refundable_principal
        .checked_add(page.rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        account.lamports() >= floor,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    Ok(page)
}

fn authenticate_lp_page(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<LpPageV2> {
    authenticate_lp_page_with_access(program_id, account, true)
}

fn authenticate_terminal_allocation(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<(u8, DealerTerminalAllocationV1)> {
    let (bump, allocation) = dealer_body::<DealerTerminalAllocationV1>(
        program_id,
        account,
        true,
        DEALER_TERMINAL_ALLOCATION_ACCOUNT_TAG,
        DEALER_TERMINAL_ALLOCATION_ACCOUNT_VERSION,
        DEALER_TERMINAL_ALLOCATION_ACCOUNT_BYTES,
    )?;
    expect_pda(
        account.key,
        seeds::dealer_terminal_allocation_pda(
            program_id,
            &allocation.facility_id.bytes(),
            allocation.page_ordinal,
        ),
        Some(bump),
    )?;
    let floor = allocation
        .rent
        .refundable_principal
        .checked_add(allocation.rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        allocation.allocation_receipt_program_id == id(program_id)
            && account.lamports() >= floor,
        ClutchError::MismatchedState,
    )?;
    Ok((bump, allocation))
}

fn authenticate_claim_work_with_access(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
) -> Outcome<DealerClaimWorkV1> {
    let (bump, work) = dealer_body::<DealerClaimWorkV1>(
        program_id,
        account,
        writable,
        DEALER_CLAIM_WORK_ACCOUNT_TAG,
        DEALER_CLAIM_WORK_ACCOUNT_VERSION,
        DEALER_CLAIM_WORK_ACCOUNT_BYTES,
    )?;
    expect_pda(
        account.key,
        seeds::dealer_claim_work_pda(program_id, &work.facility_id.bytes()),
        Some(bump),
    )?;
    let floor = work
        .rent
        .refundable_principal
        .checked_add(work.rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        work.claim_work_account_id == id(account.key)
            && work.resolve_receipt_program_id == id(program_id)
            && account.lamports() >= floor,
        ClutchError::MismatchedState,
    )?;
    Ok(work)
}

fn authenticate_claim_work(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<DealerClaimWorkV1> {
    authenticate_claim_work_with_access(program_id, account, false)
}

fn authenticate_exit_ticket(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<(u8, DealerExitTicketV1)> {
    let (bump, ticket) = dealer_body::<DealerExitTicketV1>(
        program_id,
        account,
        true,
        DEALER_EXIT_TICKET_ACCOUNT_TAG,
        DEALER_EXIT_TICKET_ACCOUNT_VERSION,
        DEALER_EXIT_TICKET_ACCOUNT_BYTES,
    )?;
    expect_pda(
        account.key,
        seeds::dealer_exit_ticket_pda(
            program_id,
            &ticket.facility_id.bytes(),
            &ticket.owner.bytes(),
        ),
        Some(bump),
    )?;
    let floor = ticket
        .rent
        .refundable_principal
        .checked_add(ticket.rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        account.lamports() >= floor,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    Ok((bump, ticket))
}

fn authenticate_epoch_binding_with_access(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    facility_id: Id,
    writable: bool,
) -> Outcome<(u8, DealerEpochBindingV2)> {
    let (bump, epoch) = dealer_body::<DealerEpochBindingV2>(
        program_id,
        account,
        writable,
        DEALER_EPOCH_BINDING_V2_ACCOUNT_TAG,
        DEALER_EPOCH_BINDING_V2_ACCOUNT_VERSION,
        DEALER_EPOCH_BINDING_V2_ACCOUNT_BYTES,
    )?;
    expect_pda(
        account.key,
        seeds::dealer_epoch_v2_pda(program_id, &facility_id.bytes(), epoch.counted_generation),
        Some(bump),
    )?;
    let floor = epoch
        .rent
        .refundable_principal
        .checked_add(epoch.rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        account.lamports() >= floor,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    Ok((bump, epoch))
}

fn authenticate_epoch_binding(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    facility_id: Id,
) -> Outcome<(u8, DealerEpochBindingV2)> {
    authenticate_epoch_binding_with_access(program_id, account, facility_id, true)
}

fn authenticate_action_receipt(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<(u8, DealerActionReceiptV1)> {
    let (bump, receipt) = dealer_body::<DealerActionReceiptV1>(
        program_id,
        account,
        true,
        DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
        DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
        DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
    )?;
    let slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
    expect_pda(
        account.key,
        seeds::dealer_action_receipt_pda(program_id, &slot.bytes()),
        Some(bump),
    )?;
    let receipt_rent = receipt.rent();
    let floor = receipt_rent
        .refundable_principal
        .checked_add(receipt_rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        account.lamports() >= floor,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    Ok((bump, receipt))
}

fn authenticate_fee_terminal_for_dealer(
    program_id: &Pubkey,
    closure_manifest_account: &AccountInfo<'_>,
    terminal_account: &AccountInfo<'_>,
    policy: &clutch_dealer_runtime_contract::DealerPolicyV1,
    selection: &CoveredDealerSelectionV1,
    epoch: &DealerEpochBindingV2,
    lease: &DealerLeaseV2,
) -> Outcome<clutch_dealer_runtime_contract::DealerFeeTerminalJoinV1> {
    for (account, expected_len) in [
        (closure_manifest_account, FEE_CLOSURE_MANIFEST_V1_BYTES),
        (terminal_account, FEE_TERMINAL_RECEIPT_V1_BYTES),
    ] {
        require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
        require(
            !account.executable && !account.is_signer && !account.is_writable,
            ClutchError::MismatchedState,
        )?;
        require(account.data_len() == expected_len, ClutchError::WrongDataLength)?;
    }
    let manifest_data = closure_manifest_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let terminal_data = terminal_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let bundle = FeeTerminalReceiptBundleV1::decode(&manifest_data, &terminal_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let manifest = bundle.closure_manifest();
    let terminal = bundle.terminal();
    let runtime_release = fee_runtime_semantic_release_id_v1(&RuntimeSha256)?;
    require(
        manifest.receipt().0 == closure_manifest_account.key.to_bytes()
            && terminal.terminal_receipt().0 == terminal_account.key.to_bytes()
            && terminal.closure_manifest().0 == closure_manifest_account.key.to_bytes()
            && manifest.runtime_program().0 == program_id.to_bytes()
            && terminal.runtime_program().0 == program_id.to_bytes()
            && manifest.runtime_release().0 == runtime_release.bytes()
            && terminal.runtime_release().0 == runtime_release.bytes(),
        ClutchError::MismatchedState,
    )?;
    bind_dealer_fee_terminal_v1(terminal.project_dealer(), policy, selection, epoch, lease)
        .map_err(dealer_fault)
}

#[inline(never)]
fn authenticate_position_and_replay(
    program_id: &Pubkey,
    state_account: &AccountInfo<'_>,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    policy: &clutch_dealer_runtime_contract::DealerPolicyV1,
    state: &DealerStateV2,
    position_writable: bool,
) -> Outcome<(
    FacilityPositionBindingV2,
    DealerPositionObservationV3,
    DealerFacilityReplayV1,
    DealerReplayAccountBindingV1,
)> {
    require(
        position_account.owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require(
        replay_account.owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require(
        position_account.is_writable == position_writable,
        if position_writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(replay_account.is_writable, ClutchError::NotWritable)?;
    require(
        !position_account.executable
            && !replay_account.executable
            && !position_account.is_signer
            && !replay_account.is_signer,
        ClutchError::ExecutableAccount,
    )?;
    require(
        position_account.data_len() == POSITION_V3_BYTES
            && replay_account.data_len()
                == clutch_dealer_runtime_contract::DEALER_FACILITY_REPLAY_BYTES_V1,
        ClutchError::WrongDataLength,
    )?;
    let position = PositionAccountV3::decode(&position_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_rent = position.rent();
    let position_floor = position_rent
        .refundable_live_principal
        .checked_add(position_rent.permanent_tombstone_principal)
        .and_then(|value| value.checked_add(position_rent.donation_floor))
        .ok_or(ClutchError::Arithmetic)?;
    require(
        position_account.lamports() >= position_floor,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    let binding = FacilityPositionBindingV2 {
        facility_id: state.facility_id,
        policy_id: state.policy_id,
        market_instance_v2_id: policy.market_instance_v2_id,
        collateral_policy_id: Id::from_bytes(position.collateral_policy_id().bytes()),
        collateral_release_id: Id::from_bytes(position.collateral_release_id().bytes()),
        dealer_state_account_id: id(state_account.key),
        initial_position_generation: 1,
    };
    let binding_id = binding.binding_id().map_err(dealer_fault)?;
    require(
        binding_id == state.facility_position_binding_id,
        ClutchError::MismatchedState,
    )?;
    let position_seeds = position.pda_seeds();
    expect_pda(
        position_account.key,
        seeds::position_v3_pda(
            program_id,
            &position_seeds.market_instance_id().bytes(),
            &position_seeds.owner().bytes(),
            position_seeds.purpose(),
            &position_seeds.purpose_binding_id().bytes(),
        ),
        Some(position_seeds.stored_bump()),
    )?;
    let replay =
        DealerFacilityReplayV1::decode(&replay_account.data.borrow()).map_err(dealer_fault)?;
    let replay_rent = replay.rent();
    let replay_floor = replay_rent
        .refundable_principal()
        .checked_add(replay_rent.donation_floor())
        .ok_or(ClutchError::Arithmetic)?;
    require(
        replay_account.lamports() >= replay_floor,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    let replay_seeds = replay.pda_seeds();
    expect_pda(
        replay_account.key,
        seeds::purpose_replay_v3_pda(
            program_id,
            &replay_seeds.position_account().bytes(),
            replay_seeds.purpose(),
            &replay_seeds.purpose_binding_id().bytes(),
        ),
        Some(replay_seeds.stored_bump()),
    )?;
    require(
        position_account.key.to_bytes() == state.facility_position_account_id.bytes()
            && replay_account.key.to_bytes() == state.facility_replay_account_id.bytes()
            && position.replay_account().bytes() == replay_account.key.to_bytes()
            && position.purpose() == PositionPurposeV3::DealerFacility,
        ClutchError::MismatchedState,
    )?;
    let projection = project_dealer_position_v3(
        position,
        AdapterPositionMarketBindingV3 {
            market_instance_id: position.market_instance_id(),
            outcome_count: position.outcome_count(),
            realm_id: position.realm_id(),
            collateral_policy_id: position.collateral_policy_id(),
            collateral_release_id: position.collateral_release_id(),
        },
        AdapterPositionPurposeBindingV3 {
            owner: retirement_id(state.facility_id)?,
            controller: retirement_id(binding.dealer_state_account_id)?,
            purpose_binding_id: retirement_id(binding_id)?,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let semantic_id = Id::from_bytes(
        position
            .semantic_id(&RuntimeSha256)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes(),
    );
    let observation = DealerPositionObservationV3 {
        account_id: id(position_account.key),
        semantic_id,
        projection,
    };
    observation
        .validate_current(state, &binding, policy)
        .map_err(dealer_fault)?;
    Ok((
        binding,
        observation,
        replay,
        DealerReplayAccountBindingV1 {
            replay_account_id: id(replay_account.key),
            position_replay_account_id: Id::from_bytes(position.replay_account().bytes()),
        },
    ))
}

#[inline(never)]
fn authenticate_runtime_bundle(
    program_id: &Pubkey,
    dependency: &DealerFundedDependenciesV2,
    policy_account: &AccountInfo<'_>,
    compartments: &[AccountInfo<'_>],
    writable_index: usize,
) -> Outcome<(
    RuntimeLivenessPolicyV1,
    [RuntimeCompartmentV1; RUNTIME_COMPARTMENT_COUNT_V1],
    DealerRuntimeLivenessBindingV1,
)> {
    authenticate_runtime_bundle_with_access(
        program_id,
        dependency,
        policy_account,
        compartments,
        Some(writable_index),
    )
}

#[inline(never)]
fn authenticate_runtime_bundle_with_access(
    program_id: &Pubkey,
    dependency: &DealerFundedDependenciesV2,
    policy_account: &AccountInfo<'_>,
    compartments: &[AccountInfo<'_>],
    writable_index: Option<usize>,
) -> Outcome<(
    RuntimeLivenessPolicyV1,
    [RuntimeCompartmentV1; RUNTIME_COMPARTMENT_COUNT_V1],
    DealerRuntimeLivenessBindingV1,
)> {
    require(
        dependency.bindings.runtime_liveness_program_id.bytes() == program_id.to_bytes(),
        ClutchError::WrongProgramOwner,
    )?;
    require(
        policy_account.key.to_bytes()
            == dependency
                .bindings
                .runtime_liveness_policy_account_id
                .bytes(),
        ClutchError::MismatchedState,
    )?;
    require(
        policy_account.owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require(!policy_account.is_writable, ClutchError::UnexpectedWritable)?;
    require(
        policy_account.data_len() == RUNTIME_LIVENESS_POLICY_BYTES_V1,
        ClutchError::WrongDataLength,
    )?;
    let runtime_policy = RuntimeLivenessPolicyV1::decode(&policy_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let runtime_policy_id = dealer_runtime_liveness_policy_id_v1(runtime_policy)
        .map_err(dealer_fault)?;
    expect_pda(
        policy_account.key,
        seeds::dealer_runtime_liveness_policy_pda(program_id, &runtime_policy_id.bytes()),
        None,
    )?;
    require(
        runtime_policy.policy_id.bytes() == dependency.bindings.runtime_liveness_policy_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    require(
        compartments.len() == RUNTIME_COMPARTMENT_COUNT_V1,
        ClutchError::AccountCount,
    )?;
    let first = RuntimeCompartmentV1::decode(&compartments[0].data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut states = [first; RUNTIME_COMPARTMENT_COUNT_V1];
    let mut index = 0usize;
    while index < RUNTIME_COMPARTMENT_COUNT_V1 {
        let account = &compartments[index];
        require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
        require(
            !account.executable && !account.is_signer,
            ClutchError::MismatchedState,
        )?;
        let expected_writable = writable_index == Some(index);
        require(
            account.is_writable == expected_writable,
            if expected_writable {
                ClutchError::NotWritable
            } else {
                ClutchError::UnexpectedWritable
            },
        )?;
        require(
            account.data_len() == RUNTIME_LIVENESS_ACCOUNT_BYTES_V1,
            ClutchError::WrongDataLength,
        )?;
        let state = RuntimeCompartmentV1::decode(&account.data.borrow())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            state.kind.index() == index
                && state.identity.account_id.bytes() == account.key.to_bytes()
                && account.lamports()
                    >= state
                        .expected_account_balance_lamports()
                        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
            ClutchError::MismatchedState,
        )?;
        states[index] = state;
        index += 1;
    }
    let binding = DealerRuntimeLivenessBindingV1::from_canonical(&runtime_policy, &states)
        .map_err(dealer_fault)?;
    Ok((runtime_policy, states, binding))
}

fn validate_runtime_dependency_join(
    program_id: &Pubkey,
    state_account: &AccountInfo<'_>,
    policy: &clutch_dealer_runtime_contract::DealerPolicyV1,
    state: &DealerStateV2,
    binding: &FacilityPositionBindingV2,
    dependency: &DealerFundedDependenciesV2,
    schedule: &DealerLivenessScheduleV1,
    runtime_policy: RuntimeLivenessPolicyV1,
    runtime_binding: DealerRuntimeLivenessBindingV1,
) -> Outcome<()> {
    require(
        dependency.facility_position_binding_id == binding.binding_id().map_err(dealer_fault)?
            && dependency.bindings.runtime_liveness_binding_digest
                == runtime_binding.binding_digest().map_err(dealer_fault)?
            && dependency.bindings.policy_id == state.policy_id
            && dependency.bindings.facility_id == state.facility_id
            && dependency.bindings.liveness_schedule_id
                == schedule.schedule_id().map_err(dealer_fault)?.untyped()
            && dependency.bindings.liveness_schedule_id == policy.liveness_policy_id
            && dependency.bindings.runtime_liveness_policy_id
                == runtime_binding.runtime_policy_id()
            && runtime_binding.realm_id() == policy.realm_id
            && runtime_binding.lifecycle_id() == state.facility_id
            && runtime_binding.neutral_sink() == policy.neutral_sink
            && dependency.bindings.fee_policy_id == policy.fee_policy_id
            && dependency.bindings.collateral_mint == policy.collateral_mint
            && dependency.bindings.token_program == policy.token_program
            && dependency.bindings.asset_vault_authority_account_id == id(state_account.key)
            && dependency.bindings.neutral_sink == policy.neutral_sink
            && dependency.bindings.dealer_liveness_work_principal_lamports
                == schedule
                    .dealer_runtime_work_principal_lamports()
                    .map_err(dealer_fault)?
            && runtime_policy.policy_id.bytes()
                == dependency.bindings.runtime_liveness_policy_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let mut runtime_index = 1usize;
    while runtime_index < RUNTIME_COMPARTMENT_COUNT_V1 {
        let compartment = match runtime_index {
            1 => DealerLivenessCompartmentV1::Candidate,
            2 => DealerLivenessCompartmentV1::Clearing,
            3 => DealerLivenessCompartmentV1::Settlement,
            4 => DealerLivenessCompartmentV1::Resolution,
            5 => DealerLivenessCompartmentV1::Retirement,
            6 => DealerLivenessCompartmentV1::Recovery,
            _ => return Err(ClutchError::MismatchedState.into()),
        };
        require(
            runtime_binding.owner(compartment) == id(state_account.key)
                && runtime_binding.receipt_program_id(compartment) == id(program_id),
            ClutchError::MismatchedState,
        )?;
        runtime_index += 1;
    }
    Ok(())
}

#[inline(never)]
fn authenticate_general_epoch(
    program_id: &Pubkey,
    epoch_account: &AccountInfo<'_>,
    window_account: &AccountInfo<'_>,
    domain_account: &AccountInfo<'_>,
    policy: &clutch_dealer_runtime_contract::DealerPolicyV1,
) -> Outcome<DealerGeneralEpochEvidenceV3> {
    for (account, length) in [
        (epoch_account, GENERAL_EPOCH_ACCOUNT_BYTES),
        (window_account, WINDOW_ACCOUNT_BYTES),
        (domain_account, ECONOMIC_DOMAIN_ACCOUNT_BYTES),
    ] {
        require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
        require(!account.is_writable, ClutchError::UnexpectedWritable)?;
        require(
            !account.executable && !account.is_signer,
            ClutchError::MismatchedState,
        )?;
        require(account.data_len() == length, ClutchError::WrongDataLength)?;
    }
    let epoch = GeneralEpochV6AccountV1::decode(&epoch_account.data.borrow())?;
    let window = CandidateWindowV4AccountV1::decode(&window_account.data.borrow())?;
    let domain = EconomicDomainV2AccountV1::decode(&domain_account.data.borrow())?;
    expect_pda(
        epoch_account.key,
        seeds::general_v2_epoch_pda(program_id, &epoch.market_binding.bytes(), epoch.epoch_index),
        Some(epoch.stored_bump),
    )?;
    expect_pda(
        window_account.key,
        seeds::general_v2_window_pda(program_id, &epoch_account.key.to_bytes()),
        Some(window.stored_bump),
    )?;
    expect_pda(
        domain_account.key,
        seeds::general_v2_economic_domain_pda(program_id, &epoch_account.key.to_bytes()),
        Some(domain.stored_bump),
    )?;
    DealerGeneralEpochEvidenceV3::new(
        id(epoch_account.key),
        epoch,
        id(window_account.key),
        window,
        id(domain_account.key),
        domain,
        policy,
    )
    .map_err(dealer_fault)
}

struct DealerCompleteBookAdapterV2<'a> {
    owner_program_id: [u8; 32],
    root_account_id: [u8; 32],
    root_data_id: [u8; 32],
    root_generation: u64,
    root_writable: bool,
    feed_account_id: [u8; 32],
    feed_data_id: [u8; 32],
    page_account_ids: [[u8; 32]; PORTFOLIO_BOOK_MAX_PAGES_V2],
    page_data_ids: [[u8; 32]; PORTFOLIO_BOOK_MAX_PAGES_V2],
    page_count: u8,
    relation_domain: EconomicDomainV2,
    raw_pages: &'a [&'a [u8]],
}

impl PortfolioBookAdapterV2 for DealerCompleteBookAdapterV2<'_> {
    fn authenticate_book_account(&self, expected: &PortfolioBookAccountExpectationV2) -> bool {
        if expected.owner_program_id != self.owner_program_id {
            return false;
        }
        match expected.role {
            PortfolioBookAccountRoleV2::SettlementRoot => {
                expected.account_id == self.root_account_id
                    && expected.data_semantic_id == self.root_data_id
                    && expected.generation == Some(self.root_generation)
                    && expected.page_index.is_none()
                    && expected.writable == self.root_writable
            }
            PortfolioBookAccountRoleV2::RetainedFeed => {
                expected.account_id == self.feed_account_id
                    && expected.data_semantic_id == self.feed_data_id
                    && expected.generation.is_none()
                    && expected.page_index.is_none()
                    && !expected.writable
            }
            PortfolioBookAccountRoleV2::OrderPage => {
                let Some(page_index) = expected.page_index else {
                    return false;
                };
                let page = usize::from(page_index);
                page < usize::from(self.page_count)
                    && expected.account_id == self.page_account_ids[page]
                    && expected.data_semantic_id == self.page_data_ids[page]
                    && expected.generation.is_none()
                    && !expected.writable
            }
        }
    }

    fn project_complete_economic_book(
        &self,
        _expected: &PortfolioCompleteBookProjectionExpectationV2,
    ) -> Option<EconomicBookV2> {
        // This SBF-only adapter deliberately supports only the in-place
        // capability constructor. Returning an owning 12-KiB-plus book would
        // recreate the frame hazard this boundary exists to remove.
        None
    }
}

impl PortfolioBookInPlaceAdapterV2 for DealerCompleteBookAdapterV2<'_> {
    fn project_complete_economic_book_into(
        &self,
        expected: &PortfolioCompleteBookProjectionExpectationV2,
        output: &mut EconomicBookV2,
    ) -> bool {
        if expected.page_set.page_count != self.page_count
            || expected.page_set.page_account_ids != self.page_account_ids
            || expected.page_set.page_semantic_ids != self.page_data_ids
        {
            return false;
        }
        output.orders.fill(clutch_batch::relation_v2::EMPTY_ECONOMIC_ORDER_V2);
        output.len = 0;
        let mut page = 0usize;
        while page < self.raw_pages.len() {
            let header = match OrderPageHeaderV5::decode(self.raw_pages[page]) {
                Ok(value) => value,
                Err(_) => return false,
            };
            let mut cursor = match OrderSlotCursorV5::new(self.raw_pages[page]) {
                Ok(value) => value,
                Err(_) => return false,
            };
            let mut slot = 0usize;
            while slot < usize::from(header.order_count) {
                let verified = match cursor.next_slot() {
                    Some(Ok(value)) => value,
                    _ => return false,
                };
                let projection = match project_owner_blind_slot(
                    verified.slot,
                    &self.relation_domain,
                ) {
                    Ok(value) => value,
                    Err(_) => return false,
                };
                if let Some((order, _membership)) = projection {
                    let at = usize::from(output.len);
                    if at >= output.orders.len() {
                        return false;
                    }
                    output.orders[at] = order;
                    output.len = match output.len.checked_add(1) {
                        Some(value) => value,
                        None => return false,
                    };
                }
                slot += 1;
            }
            page += 1;
        }
        true
    }
}

struct DealerSelectedRowAdapterV1 {
    record: SelectedPortfolioOrderRecordV2,
    accounts: [PortfolioAccountExpectationV2; 4],
}

impl PortfolioAdapterV2 for DealerSelectedRowAdapterV1 {
    fn authenticate_account(&self, expected: &PortfolioAccountExpectationV2) -> bool {
        self.accounts.iter().any(|account| account == expected)
    }

    fn authenticate_selection_membership(
        &self,
        expected: &PortfolioSelectionMembershipExpectationV2,
        relation_order: &clutch_batch::relation_v2::EconomicOrderV2,
        candidate: &EconomicCandidateV2,
    ) -> bool {
        let at = usize::from(self.record.order_index);
        expected.record == self.record
            && relation_order.order_id == self.record.order_id
            && relation_order.side == self.record.side
            && candidate.fills.get(at).copied() == Some(self.record.selected_fill_units)
    }

    fn authenticate_transition(&self, _expected: &PortfolioTransitionExpectationV2) -> bool {
        false
    }

    fn derive_settlement_receipt_v5_post_data_ids(
        &self,
        _expected: &PortfolioSettlementReceiptV5TransitionExpectationV2,
    ) -> Option<[PortfolioIdentityV2; PORTFOLIO_PAIR_MAX_RECEIPTS_V2]> {
        None
    }
}

#[derive(Clone, Copy)]
struct DealerPageRowMembershipV1 {
    page_index: u16,
    slot_index: u8,
    page_semantic_id: [u8; 32],
    owner_id: [u8; 32],
    order_generation: u64,
    position_generation: u64,
    slot: OrderSlot,
}

fn find_dealer_page_membership_v1(
    pages: &[AccountInfo<'_>],
    order_id: [u8; 32],
) -> Outcome<DealerPageRowMembershipV1> {
    let mut found = None;
    let mut page = 0usize;
    while page < pages.len() {
        let data = pages[page]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let header = OrderPageHeaderV5::decode(&data)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let mut cursor = OrderSlotCursorV5::new(&data)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let mut slot = 0usize;
        while slot < usize::from(header.order_count) {
            let verified = cursor
                .next_slot()
                .ok_or(ClutchError::MismatchedState)?
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            if let OrderSlot::Portfolio(order) = verified.slot {
                if order.order_id.bytes() == order_id {
                    require(found.is_none(), ClutchError::MismatchedState)?;
                    found = Some(DealerPageRowMembershipV1 {
                        page_index: header.page_index,
                        slot_index: verified.slot_index,
                        page_semantic_id: header.page_digest.bytes(),
                        owner_id: order.owner.bytes(),
                        order_generation: order.generation,
                        position_generation: verified.position_generation,
                        slot: verified.slot,
                    });
                }
            }
            slot += 1;
        }
        page += 1;
    }
    found.ok_or(ClutchError::MismatchedState.into())
}

#[inline(never)]
fn with_authenticated_complete_dealer_book_v2<R>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root: &SettlementRootV1AccountV1,
    feed_account: &AccountInfo<'_>,
    domain: &EconomicDomainV2AccountV1,
    page_accounts: &[AccountInfo<'_>],
    root_writable: bool,
    consume: impl FnOnce(&AuthenticatedCompletePortfolioBookRefV2<'_>, &[u8]) -> Outcome<R>,
) -> Outcome<R> {
    require(root_account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(
        root_account.is_writable == root_writable,
        if root_writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(
        !root_account.is_signer && !root_account.executable,
        ClutchError::MismatchedState,
    )?;
    require(
        root_account.data_len() == SETTLEMENT_ROOT_ACCOUNT_BYTES,
        ClutchError::WrongDataLength,
    )?;
    expect_pda(
        root_account.key,
        seeds::general_v2_settlement_root_pda(
            program_id,
            &root.epoch().bytes(),
            &root.settlement_candidate_id().bytes(),
        ),
        Some(root.stored_bump()),
    )?;
    require(feed_account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!feed_account.is_writable, ClutchError::UnexpectedWritable)?;
    require(
        !feed_account.is_signer && !feed_account.executable,
        ClutchError::MismatchedState,
    )?;
    let feed_data = feed_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (feed_header, _) = decode_sealed_candidate_feed_v1(&feed_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let feed_floor = feed_header
        .rent
        .refundable_principal
        .checked_add(feed_header.rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    expect_pda(
        feed_account.key,
        seeds::general_v2_feed_pda(program_id, &feed_header.node.bytes()),
        Some(feed_header.stored_bump),
    )?;
    let feed_data_id = clutch_general_v2_contract::candidate_bundle_digest_v1(
        &RuntimeSha256,
        &feed_data,
        true,
    )?;
    require(
        feed_account.lamports() >= feed_floor
            && root.retained_feed().bytes() == feed_account.key.to_bytes()
            && root.candidate_bundle_digest() == feed_data_id
            && feed_header.epoch == root.epoch()
            && feed_header.market == root.market()
            && feed_header.order_set == root.order_set()
            && feed_header.relation_policy_id == domain.transcript.relation_policy_id
            && feed_header.economic_domain_digest
                == clutch_general_v2_contract::economic_domain_digest_v2(
                    &RuntimeSha256,
                    domain.transcript,
                )?
            && feed_header.native_claim_basis_id == domain.transcript.native_claim_basis_id
            && feed_header.price_measure_policy_v1_id
                == domain.transcript.price_measure_policy_v1_id
            && feed_header.settlement_candidate_id == root.settlement_candidate_id()
            && feed_header.settlement_witness_digest == root.settlement_witness_digest()
            && feed_header.epoch_generation == root.epoch_generation()
            && feed_header.outcome_count == root.outcome_count()
            && feed_header.order_count == root.order_count()
            && feed_header.price_scale == domain.transcript.price_scale
            && feed_header.candidate_kind
                == clutch_general_v2_contract::SettlementCandidateKindV1::CoveredDealer,
        ClutchError::MismatchedState,
    )?;
    require(
        !page_accounts.is_empty() && page_accounts.len() <= PORTFOLIO_BOOK_MAX_PAGES_V2,
        ClutchError::AccountCount,
    )?;

    // Never materialize even one 4,140-byte OrderPageV5, much less the
    // four-page maximum, in an SBF frame. The streaming layout verifier folds
    // the exact raw account bytes and the cursor exposes one authenticated
    // slot at a time.
    let mut page_data = Vec::with_capacity(page_accounts.len());
    let mut page = 0usize;
    while page < page_accounts.len() {
        page_data.push(
            page_accounts[page]
                .try_borrow_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
        );
        page += 1;
    }
    let mut raw_pages = Vec::with_capacity(page_data.len());
    page = 0;
    while page < page_data.len() {
        raw_pages.push(&page_data[page][..]);
        page += 1;
    }
    let observed_order_set = verify_page_set_v5_streaming(&raw_pages)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        observed_order_set.bytes() == root.order_set().bytes(),
        ClutchError::MismatchedState,
    )?;

    let mut page_account_ids = [[0u8; 32]; PORTFOLIO_BOOK_MAX_PAGES_V2];
    let mut page_data_ids = [[0u8; 32]; PORTFOLIO_BOOK_MAX_PAGES_V2];
    let mut authenticated_live_orders = 0u16;
    page = 0;
    while page < page_accounts.len() {
        let account = &page_accounts[page];
        require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
        require(!account.is_writable, ClutchError::UnexpectedWritable)?;
        require(
            !account.is_signer && !account.executable && account.data_len() == ORDER_PAGE_V5_BYTES,
            ClutchError::MismatchedState,
        )?;
        let header = OrderPageHeaderV5::decode(&raw_pages[page])
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let page_index = u16::try_from(page).map_err(|_| ClutchError::Arithmetic)?;
        expect_pda(
            account.key,
            seeds::general_v2_order_page_v5_pda(program_id, &root.epoch().bytes(), page_index),
            Some(header.stored_bump),
        )?;
        require(
            header.page_index == page_index
                && usize::from(header.page_count) == page_accounts.len()
                && header.epoch.bytes() == root.epoch().bytes()
                && header.order_set.bytes() == root.order_set().bytes()
                && header.frozen != 0,
            ClutchError::MismatchedState,
        )?;
        authenticated_live_orders = authenticated_live_orders
            .checked_add(u16::from(header.live_count()))
            .ok_or(ClutchError::Arithmetic)?;
        page_account_ids[page] = account.key.to_bytes();
        page_data_ids[page] = header.page_digest.bytes();
        page += 1;
    }
    require(
        authenticated_live_orders == u16::from(root.order_count()),
        ClutchError::MismatchedState,
    )?;

    let transcript = domain.transcript;
    let relation_domain = EconomicDomainV2 {
        relation_version: transcript.relation_version,
        market_semantics_digest: transcript.market_instance_v2_id.bytes(),
        epoch_semantics_digest: transcript.epoch_semantics_digest.bytes(),
        relation_policy_digest: transcript.relation_policy_id.bytes(),
        price_policy_digest: transcript.price_measure_policy_v1_id.bytes(),
        epoch_index: transcript.epoch_index,
        outcome_count: transcript.outcome_count,
        price_scale: transcript.price_scale,
    };
    let root_data_id = root.data_id(&RuntimeSha256, clutch_general_v2_contract::Id32::new(
        root_account.key.to_bytes(),
    )?)?;
    let page_set = PortfolioBookPageSetRecordV2 {
        version: PORTFOLIO_BOOK_AUTHORITY_VERSION_V2,
        outcome_count: root.outcome_count(),
        page_count: u8::try_from(page_accounts.len()).map_err(|_| ClutchError::Arithmetic)?,
        order_count: root.order_count(),
        traversal_index: u16::from(root.order_count()),
        settlement_root_epoch_generation: root.epoch_generation(),
        market_semantics_digest: relation_domain.market_semantics_digest,
        epoch_semantics_digest: relation_domain.epoch_semantics_digest,
        order_set_digest: root.order_set().bytes(),
        settlement_root_account_id: root_account.key.to_bytes(),
        settlement_root_pre_semantic_id: root_data_id.bytes(),
        retained_feed_account_id: feed_account.key.to_bytes(),
        retained_feed_semantic_id: feed_data_id.bytes(),
        settlement_candidate_id: root.settlement_candidate_id().bytes(),
        settlement_witness_id: root.settlement_witness_digest().bytes(),
        page_account_ids,
        page_semantic_ids: page_data_ids,
    };
    let adapter = DealerCompleteBookAdapterV2 {
        owner_program_id: program_id.to_bytes(),
        root_account_id: root_account.key.to_bytes(),
        root_data_id: root_data_id.bytes(),
        root_generation: root.epoch_generation(),
        root_writable,
        feed_account_id: feed_account.key.to_bytes(),
        feed_data_id: feed_data_id.bytes(),
        page_account_ids,
        page_data_ids,
        page_count: page_set.page_count,
        relation_domain,
        raw_pages: &raw_pages,
    };
    let mut authenticated_book =
        super::orders_batch::boxed_copy_of(&EMPTY_DEALER_ECONOMIC_BOOK_V2)?;
    let authenticated = if root_writable {
        authenticate_complete_portfolio_book_for_root_transition_into_v2(
            &adapter,
            program_id.to_bytes(),
            &relation_domain,
            page_set,
            &mut authenticated_book,
        )
    } else {
        authenticate_complete_portfolio_book_into_v2(
            &adapter,
            program_id.to_bytes(),
            &relation_domain,
            page_set,
            &mut authenticated_book,
        )
    }
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    // The borrowed capability retains only `authenticated_book`; the adapter
    // owns no second complete-book allocation.
    drop(adapter);
    consume(&authenticated, &feed_data)
}

#[inline(never)]
fn authenticate_signed_dealer_quote_v1(
    quote_account: &AccountInfo<'_>,
    instructions_account: &AccountInfo<'_>,
    policy: &clutch_dealer_runtime_contract::DealerPolicyV1,
) -> Outcome<Box<clutch_dealer_runtime_contract::DealerQuoteAdmissionV1>> {
    require(
        !quote_account.is_writable && !quote_account.is_signer && !quote_account.executable,
        ClutchError::MismatchedState,
    )?;
    require(
        quote_account.data_len()
            == clutch_dealer_runtime_contract::DEALER_QUOTE_ADMISSION_BYTES_V1,
        ClutchError::WrongDataLength,
    )?;
    let mut quote = super::orders_batch::boxed_copy_of(&EMPTY_DEALER_QUOTE_ADMISSION_V1)?;
    clutch_dealer_runtime_contract::DealerQuoteAdmissionV1::decode_into(
        &quote_account.data.borrow(),
        &mut quote,
    )
    .map_err(dealer_fault)?;
    let instructions_data = instructions_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let instructions = InstructionsSysvarV1::new(
        instructions_account.key.to_bytes(),
        instructions_account.owner.to_bytes(),
        instructions_account.is_writable,
        &instructions_data,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    instructions
        .preceding_ed25519_quote_v1(
            policy.quote_authority.bytes(),
            quote.admission_id().map_err(dealer_fault)?.bytes(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        instructions_account.owner.to_bytes() == SYSVAR_OWNER_ID,
        ClutchError::WrongProgramOwner,
    )?;
    Ok(quote)
}

struct AuthenticatedCoveredProductV1 {
    market_instance: AuthenticatedProductArtifactV1<MarketInstancePreimageV2>,
    product_template: AuthenticatedProductArtifactV1<ProductTemplateV4>,
    native_basis: AuthenticatedProductArtifactV1<NativeClaimBasisV1>,
    price_policy: AuthenticatedProductArtifactV1<PriceMeasurePolicyV1>,
    genesis: AuthenticatedProductArtifactV1<MarketGenesisProfileV2>,
}

struct AuthenticatedCoveredRootInputsV1 {
    root: Box<SettlementRootV1AccountV1>,
    domain: Box<EconomicDomainV2AccountV1>,
    binding: Box<MarketBindingV2>,
    grid: Box<PriceGridAccount>,
}

#[inline(never)]
fn authenticate_covered_root_inputs_v1(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    domain_account: &AccountInfo<'_>,
    binding_account: &AccountInfo<'_>,
    grid_account: &AccountInfo<'_>,
    policy: &clutch_dealer_runtime_contract::DealerPolicyV1,
    root_writable: bool,
) -> Outcome<AuthenticatedCoveredRootInputsV1> {
    for (account, writable, length) in [
        (root_account, root_writable, SETTLEMENT_ROOT_ACCOUNT_BYTES),
        (domain_account, false, ECONOMIC_DOMAIN_ACCOUNT_BYTES),
        (binding_account, false, MARKET_BINDING_ACCOUNT_BYTES_V2),
        (grid_account, false, account_len::PRICE_GRID),
    ] {
        require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
        require(
            account.is_writable == writable,
            if writable {
                ClutchError::NotWritable
            } else {
                ClutchError::UnexpectedWritable
            },
        )?;
        require(
            !account.is_signer && !account.executable && account.data_len() == length,
            ClutchError::MismatchedState,
        )?;
    }
    let root = Box::new(SettlementRootV1AccountV1::decode(&root_account.data.borrow())?);
    let domain = Box::new(EconomicDomainV2AccountV1::decode(&domain_account.data.borrow())?);
    let binding = Box::new(MarketBindingV2::decode(&binding_account.data.borrow())?);
    let grid = Box::new(PriceGridAccount::decode(&grid_account.data.borrow())?);
    expect_pda(
        root_account.key,
        seeds::general_v2_settlement_root_pda(
            program_id,
            &root.epoch().bytes(),
            &root.settlement_candidate_id().bytes(),
        ),
        Some(root.stored_bump()),
    )?;
    expect_pda(
        domain_account.key,
        seeds::general_v2_economic_domain_pda(program_id, &root.epoch().bytes()),
        Some(domain.stored_bump),
    )?;
    expect_pda(
        binding_account.key,
        seeds::general_v2_market_binding_pda(
            program_id,
            &root.market_instance_v2_id().bytes(),
        ),
        Some(binding.base().stored_bump),
    )?;
    expect_pda(
        grid_account.key,
        seeds::grid_pda(program_id, &grid.realm.bytes(), &grid.grid.bytes()),
        Some(grid.stored_bump),
    )?;
    let root_floor = root
        .root_rent()
        .refundable_principal
        .checked_add(root.root_rent().donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    let domain_floor = domain
        .rent
        .refundable_principal
        .checked_add(domain.rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        root_account.lamports() >= root_floor
            && domain_account.lamports() >= domain_floor
            && root.market_binding().bytes() == binding_account.key.to_bytes()
            && root.market().bytes() == binding.base().market.bytes()
            && root.market_instance_v2_id().bytes() == policy.market_instance_v2_id.bytes()
            && binding.base().market_instance_v2_id.bytes()
                == policy.market_instance_v2_id.bytes()
            && binding.base().relation_policy_id.bytes() == policy.relation_v2_id.bytes()
            && binding.base().price_measure_policy_v1_id.bytes()
                == policy.price_measure_policy_id.bytes()
            && binding.base().native_claim_basis_id.bytes() == policy.claim_basis_id.bytes()
            && binding.base().outcome_count == policy.outcome_count
            && binding.base().neutral_sink.bytes() == policy.neutral_sink.bytes()
            && binding.base().price_scale == grid.price_scale
            && domain.epoch.bytes() == root.epoch().bytes()
            && domain.transcript.market_instance_v2_id.bytes()
                == policy.market_instance_v2_id.bytes()
            && domain.transcript.relation_policy_id == binding.base().relation_policy_id
            && domain.transcript.price_measure_policy_v1_id
                == binding.base().price_measure_policy_v1_id
            && domain.transcript.native_claim_basis_id == binding.base().native_claim_basis_id
            && domain.transcript.outcome_count == binding.base().outcome_count
            && domain.transcript.price_scale == binding.base().price_scale
            && grid.realm.bytes() == policy.realm_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedCoveredRootInputsV1 {
        root,
        domain,
        binding,
        grid,
    })
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_covered_product_v1(
    program_id: &Pubkey,
    market_instance_account: &AccountInfo<'_>,
    product_template_account: &AccountInfo<'_>,
    native_basis_account: &AccountInfo<'_>,
    price_policy_account: &AccountInfo<'_>,
    genesis_account: &AccountInfo<'_>,
    root: &SettlementRootV1AccountV1,
    binding: &MarketBindingV2,
    grid: &PriceGridAccount,
    policy: &clutch_dealer_runtime_contract::DealerPolicyV1,
) -> Outcome<AuthenticatedCoveredProductV1> {
    let market_instance = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        market_instance_account,
        ContentId::from_bytes(root.market_instance_v2_id().bytes()),
    )?;
    let product_template = authenticate_product_artifact_v1::<ProductTemplateV4>(
        program_id,
        product_template_account,
        market_instance.value().product_template_id.content_id(),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        genesis_account,
        market_instance
            .value()
            .market_genesis_profile_id
            .content_id(),
    )?;
    let native_basis = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        native_basis_account,
        product_template.value().native_claim_basis_id.content_id(),
    )?;
    let price_policy = authenticate_product_artifact_v1::<PriceMeasurePolicyV1>(
        program_id,
        price_policy_account,
        genesis.value().price_measure_policy_id.content_id(),
    )?;
    market_instance
        .value()
        .validate_bindings(
            product_template.value(),
            native_basis.value(),
            price_policy.value(),
            genesis.value(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let base = binding.base();
    require(
        root.market_instance_v2_id().bytes() == policy.market_instance_v2_id.bytes()
            && base.market_instance_v2_id.bytes() == policy.market_instance_v2_id.bytes()
            && base.market_genesis_profile_v2_id.bytes() == genesis.semantic_id().bytes()
            && base.price_measure_policy_v1_id.bytes() == price_policy.semantic_id().bytes()
            && base.native_claim_basis_id.bytes() == native_basis.semantic_id().bytes()
            && base.relation_policy_id.bytes() == policy.relation_v2_id.bytes()
            && base.score_policy_id == root.score_policy_id()
            && binding.batch_policy_id() == root.batch_policy_id()
            && base.outcome_count == policy.outcome_count
            && base.outcome_count == native_basis.value().outcome_count
            && base.basis_degree == native_basis.value().basis_degree
            && base.price_scale == grid.price_scale
            && genesis.value().realm_id.bytes() == policy.realm_id.bytes()
            && genesis.value().profile_id.bytes() == policy.profile_id.bytes()
            && genesis.value().price_grid_id.bytes() == grid.grid.bytes()
            && genesis.value().relation_policy_id.bytes() == policy.relation_v2_id.bytes()
            && genesis.value().fee_policy_id.bytes() == policy.fee_policy_id.bytes()
            && genesis.value().price_measure_policy_id.bytes()
                == policy.price_measure_policy_id.bytes()
            && product_template.value().native_claim_basis_id.bytes()
                == policy.claim_basis_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedCoveredProductV1 {
        market_instance,
        product_template,
        native_basis,
        price_policy,
        genesis,
    })
}

#[inline(never)]
fn authenticate_selected_dealer_fee_v1(
    program_id: &Pubkey,
    root: &SettlementRootV1AccountV1,
    batch_account: &AccountInfo<'_>,
    revenue_preimage_account: &AccountInfo<'_>,
    revenue_record_account: &AccountInfo<'_>,
    selected_account: &AccountInfo<'_>,
) -> Outcome<DealerSelectedFeeRecordBindingV1> {
    for (account, length) in [
        (batch_account, BATCH_POLICY_BYTES),
        (revenue_record_account, REVENUE_POLICY_RECORD_BYTES),
        (selected_account, SELECTED_FEE_RECORD_ACCOUNT_BYTES),
    ] {
        require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
        require(!account.is_writable, ClutchError::UnexpectedWritable)?;
        require(
            !account.is_signer && !account.executable && account.data_len() == length,
            ClutchError::MismatchedState,
        )?;
    }
    require(
        !revenue_preimage_account.is_writable
            && !revenue_preimage_account.is_signer
            && !revenue_preimage_account.executable
            && revenue_preimage_account.data_len() == REVENUE_POLICY_BYTES,
        ClutchError::MismatchedState,
    )?;
    let batch = decode_batch_policy(&batch_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let batch_id = batch_policy_digest(&batch)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    expect_pda(
        batch_account.key,
        seeds::batch_policy_pda(program_id, &root.epoch().bytes(), &batch_id.0),
        None,
    )?;
    let revenue = decode_revenue_policy(&revenue_preimage_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let revenue_id = revenue_policy_digest(&revenue)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let revenue_record = RevenuePolicyRecordV1::decode(&revenue_record_account.data.borrow())?;
    expect_pda(
        revenue_record_account.key,
        seeds::revenue_policy_pda(program_id, &revenue_record.realm.bytes()),
        Some(revenue_record.stored_bump),
    )?;
    require(
        revenue_record.policy_digest.bytes() == revenue_id.0
            && revenue_record.treasury.bytes() == revenue.treasury,
        ClutchError::MismatchedState,
    )?;
    let selected_outer = SelectedFeeRecordV1AccountV1::decode(
        &selected_account.data.borrow(),
        &batch,
        &revenue,
    )?;
    expect_pda(
        selected_account.key,
        seeds::general_v2_selected_fee_record_pda(
            program_id,
            &root.settlement_candidate_id().bytes(),
        ),
        Some(selected_outer.stored_bump),
    )?;
    require(
        root.fee_record().bytes() == selected_account.key.to_bytes()
            && root.batch_policy_id().bytes() == batch_id.0
            && selected_outer.semantic.batch_policy().0 == batch_id.0
            && selected_outer.semantic.revenue_policy().0 == revenue_id.0
            && selected_outer.semantic.treasury_owner().0 == revenue_record.treasury.bytes(),
        ClutchError::MismatchedState,
    )?;
    let semantic_id = selected_outer.data_id(
        &RuntimeSha256,
        clutch_general_v2_contract::Id32::new(selected_account.key.to_bytes())?,
    )?;
    DealerSelectedFeeRecordBindingV1::from_canonical(
        id(program_id),
        id(selected_account.key),
        Id::from_bytes(semantic_id.bytes()),
        &selected_outer.semantic,
    )
    .map_err(dealer_fault)
}

fn require_aliases(accounts: &[AccountInfo<'_>], allowed: (usize, usize)) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            if (left, right) != allowed {
                require(
                    accounts[left].key != accounts[right].key,
                    ClutchError::AccountAlias,
                )?;
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn require_finalize_abort_aliases(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let recipient_alias = matches!(
                (left, right),
                (0, 17) | (0, 23) | (0, 24) | (17, 23) | (17, 24) | (23, 24)
            );
            if !recipient_alias {
                require(
                    accounts[left].key != accounts[right].key,
                    ClutchError::AccountAlias,
                )?;
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn require_initialize_aliases(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let identity_only_alias = (left, right) == (0, 18);
            if !identity_only_alias {
                require(
                    accounts[left].key != accounts[right].key,
                    ClutchError::AccountAlias,
                )?;
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn require_lapse_aliases(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    const RECIPIENTS: [usize; 4] = [0, 18, 19, 20];
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let recipient_alias = RECIPIENTS.contains(&left) && RECIPIENTS.contains(&right);
            if !recipient_alias {
                require(
                    accounts[left].key != accounts[right].key,
                    ClutchError::AccountAlias,
                )?;
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn authenticate_general_position_replay_for_dealer(
    program_id: &Pubkey,
    root: &SettlementRootV1AccountV1,
    market: DealerPositionMarketJoinV2,
    owner: Id,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
) -> Outcome<(AuthenticatedPositionV3, GeneralPositionReplayPrestateV1)> {
    require(
        position_account.owner == program_id && replay_account.owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require(
        position_account.is_writable && replay_account.is_writable,
        ClutchError::NotWritable,
    )?;
    require(
        !position_account.is_signer
            && !position_account.executable
            && !replay_account.is_signer
            && !replay_account.executable
            && position_account.data_len() == POSITION_V3_BYTES
            && replay_account.data_len()
                == clutch_general_v2_contract::GENERAL_REPLAY_ACCOUNT_V1_BYTES,
        ClutchError::MismatchedState,
    )?;
    let position = PositionAccountV3::decode(&position_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_rent = position.rent();
    let position_floor = position_rent
        .refundable_live_principal
        .checked_add(position_rent.permanent_tombstone_principal)
        .and_then(|value| value.checked_add(position_rent.donation_floor))
        .ok_or(ClutchError::Arithmetic)?;
    let purpose_binding = Identity32V1::new(root.market().bytes())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_pda = seeds::position_v3_pda(
        program_id,
        &root.market_instance_v2_id().bytes(),
        &owner.bytes(),
        PositionPurposeV3::General,
        &purpose_binding.bytes(),
    );
    let replay_pda = seeds::purpose_replay_v3_pda(
        program_id,
        &position_account.key.to_bytes(),
        PositionPurposeV3::General,
        &purpose_binding.bytes(),
    );
    let fields = position.fields();
    require(
        *position_account.key == position_pda.0
            && position.stored_bump() == position_pda.1
            && *replay_account.key == replay_pda.0
            && fields.purpose == PositionPurposeV3::General
            && fields.lifecycle == PositionLifecycleV3::Open
            && fields.market_instance_id.bytes() == root.market_instance_v2_id().bytes()
            && fields.owner.bytes() == owner.bytes()
            && fields.controller.bytes() == owner.bytes()
            && fields.purpose_binding_id == purpose_binding
            && fields.replay_account.bytes() == replay_account.key.to_bytes()
            && fields.outcome_count == root.outcome_count(),
        ClutchError::MismatchedState,
    )?;
    require(
        fields.market_instance_id.bytes() == market.market_instance_v2_id.bytes()
            && fields.realm_id.bytes() == market.realm_id.bytes()
            && fields.collateral_policy_id.bytes() == market.collateral_policy_id.bytes()
            && fields.collateral_release_id.bytes() == market.collateral_release_id.bytes()
            && fields.outcome_count == market.outcome_count
            && position_account.lamports() >= position_floor,
        ClutchError::MismatchedState,
    )?;
    let semantic_id = position
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .bytes();
    let authenticated = AuthenticatedPositionV3 {
        account: position_account.key.to_bytes(),
        general_market_runtime: root.market().bytes(),
        semantic: position,
        semantic_id,
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: true,
    };
    authenticated
        .validate_writable()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_data = replay_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let envelope = ReplayV3Envelope::decode(&replay_data, &RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::Replay))?;
    let replay_rent = envelope.header().rent();
    let replay_floor = replay_rent
        .refundable_principal()
        .checked_add(replay_rent.donation_floor())
        .ok_or(ClutchError::Arithmetic)?;
    require(
        replay_account.lamports() >= replay_floor,
        ClutchError::MismatchedState,
    )?;
    let replay = project_general_position_replay_prestate_v1(
        clutch_general_v2_contract::Id32::new(replay_account.key.to_bytes())?,
        replay_pda.1,
        envelope.header().next_sequence(),
        &replay_data,
        authenticated,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::Replay))?;
    Ok((authenticated, replay))
}

fn prepare_dealer_row_collect_post(
    reservation: ReservationAccountV9,
    position: AuthenticatedPositionV3,
    row: clutch_dealer_runtime_contract::CoveredDealerSettlementRowV1,
) -> Outcome<(ReservationAccountV9, PositionSettlementPoststateV3)> {
    let mut body = reservation.body();
    let fields = position.semantic.fields();
    require(
        body.order_id.bytes() == row.order_id().bytes()
            && body.owner.bytes() == row.owner_id().bytes()
            && body.position_generation == row.position_generation()
            && body.order_kind == ORDER_KIND_PORTFOLIO
            && body.state == RESERVATION_STATE_ENTITLED
            && body.entitled_units == row.fill_units()
            && body.consumed_units == 0
            && body.paid_units == 0
            && fields.generation == row.position_generation(),
        ClutchError::MismatchedState,
    )?;
    let mut next_fields = fields;
    match row.side() {
        Side::Buy => {
            require(body.side == 0, ClutchError::MismatchedState)?;
            next_fields.cash_atoms = next_fields
                .cash_atoms
                .checked_sub(row.collect_slice().cash_atoms)
                .ok_or(ClutchError::Arithmetic)?;
            next_fields.reserved_cash_atoms = next_fields
                .reserved_cash_atoms
                .checked_sub(body.remaining_cash_atoms)
                .ok_or(ClutchError::Arithmetic)?;
            body.remaining_cash_atoms = 0;
        }
        Side::Sell => {
            require(body.side == 1, ClutchError::MismatchedState)?;
            let eggs = row.collect_slice().eggs;
            let mut outcome = 0usize;
            while outcome < MAX_OUTCOMES {
                let remainder = body.remaining_internal[outcome]
                    .checked_sub(eggs[outcome])
                    .ok_or(ClutchError::Arithmetic)?;
                next_fields.native_eggs[outcome] = next_fields.native_eggs[outcome]
                    .checked_add(remainder)
                    .ok_or(ClutchError::Arithmetic)?;
                body.remaining_internal[outcome] = 0;
                outcome += 1;
            }
        }
    }
    body.remaining_cash_atoms = 0;
    body.remaining_internal = [0; MAX_OUTCOMES];
    let reservation_post = ReservationAccountV9::new(body, reservation.rent())?;
    let semantic = PositionAccountV3::new(next_fields)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok((
        reservation_post,
        PositionSettlementPoststateV3 {
            account: position.account,
            general_market_runtime: position.general_market_runtime,
            prestate_semantic_id: position.semantic_id,
            semantic,
        },
    ))
}

fn prepare_dealer_row_deliver_post(
    reservation: ReservationAccountV9,
    position: AuthenticatedPositionV3,
    row: clutch_dealer_runtime_contract::CoveredDealerSettlementRowV1,
) -> Outcome<(ReservationAccountV9, PositionSettlementPoststateV3)> {
    let mut body = reservation.body();
    let mut fields = position.semantic.fields();
    require(
        body.order_id.bytes() == row.order_id().bytes()
            && body.owner.bytes() == row.owner_id().bytes()
            && body.position_generation == row.position_generation()
            && body.order_kind == ORDER_KIND_PORTFOLIO
            && body.state == RESERVATION_STATE_ENTITLED
            && body.entitled_units == row.fill_units()
            && body.remaining_cash_atoms == 0
            && body.remaining_internal == [0; MAX_OUTCOMES]
            && body.consumed_units == 0
            && body.paid_units == 0
            && fields.generation == row.position_generation(),
        ClutchError::MismatchedState,
    )?;
    let delivered = row.deliver_slice();
    fields.cash_atoms = fields
        .cash_atoms
        .checked_add(delivered.cash_atoms)
        .ok_or(ClutchError::Arithmetic)?;
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        fields.native_eggs[outcome] = fields.native_eggs[outcome]
            .checked_add(delivered.eggs[outcome])
            .ok_or(ClutchError::Arithmetic)?;
        outcome += 1;
    }
    fields.outstanding_reservations = fields
        .outstanding_reservations
        .checked_sub(1)
        .ok_or(ClutchError::Arithmetic)?;
    body.consumed_units = body.entitled_units;
    body.paid_units = body.entitled_units;
    body.state = RESERVATION_STATE_CONSUMED;
    let reservation_post = ReservationAccountV9::new(body, reservation.rent())?;
    let semantic = PositionAccountV3::new(fields)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok((
        reservation_post,
        PositionSettlementPoststateV3 {
            account: position.account,
            general_market_runtime: position.general_market_runtime,
            prestate_semantic_id: position.semantic_id,
            semantic,
        },
    ))
}

fn write_dealer_body<T: FixedCodec>(
    account: &AccountInfo<'_>,
    tag: u8,
    version: u8,
    bump: u8,
    body: &T,
) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    encode_dealer_account_body_v1(&mut data, tag, version, bump, body).map_err(dealer_fault)
}

fn successor_facility_position(
    account_id: Id,
    current: &DealerPositionObservationV3,
    binding: &FacilityPositionBindingV2,
    policy: &clutch_dealer_runtime_contract::DealerPolicyV1,
) -> Outcome<(PositionAccountV3, DealerPositionObservationV3)> {
    let current_position = current.projection.position();
    let mut fields: PositionV3Fields = current_position.fields();
    fields.generation = fields
        .generation
        .checked_add(1)
        .ok_or(ClutchError::Arithmetic)?;
    let position = PositionAccountV3::new(fields)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let binding_id = binding.binding_id().map_err(dealer_fault)?;
    let projection = project_dealer_position_v3(
        position,
        AdapterPositionMarketBindingV3 {
            market_instance_id: retirement_id(policy.market_instance_v2_id)?,
            outcome_count: policy.outcome_count,
            realm_id: retirement_id(policy.realm_id)?,
            collateral_policy_id: retirement_id(binding.collateral_policy_id)?,
            collateral_release_id: retirement_id(binding.collateral_release_id)?,
        },
        AdapterPositionPurposeBindingV3 {
            owner: retirement_id(binding.facility_id)?,
            controller: retirement_id(binding.dealer_state_account_id)?,
            purpose_binding_id: retirement_id(binding_id)?,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let semantic_id = Id::from_bytes(
        position
            .semantic_id(&RuntimeSha256)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes(),
    );
    Ok((
        position,
        DealerPositionObservationV3 {
            account_id,
            semantic_id,
            projection,
        },
    ))
}

fn credit_lamports(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    let after = account
        .lamports()
        .checked_add(amount)
        .ok_or(ClutchError::Arithmetic)?;
    **account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = after;
    Ok(())
}

fn release_dealer_account(account: &AccountInfo<'_>) -> Outcome<()> {
    **account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = 0;
    account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    account.assign(&SYSTEM_PROGRAM_ID);
    Ok(())
}

fn require_released_dealer_account(account: &AccountInfo<'_>) -> Outcome<()> {
    require(
        account.owner == &SYSTEM_PROGRAM_ID
            && account.lamports() == 0
            && account.data_len() == 0
            && account.is_writable
            && !account.is_signer
            && !account.executable,
        ClutchError::MismatchedState,
    )
}

fn credit_exact_dealer_terminal_lamports(
    destinations: [(&AccountInfo<'_>, u64); 4],
) -> Outcome<()> {
    let before = [
        destinations[0].0.lamports(),
        destinations[1].0.lamports(),
        destinations[2].0.lamports(),
        destinations[3].0.lamports(),
    ];
    let mut credit = 0usize;
    while credit < destinations.len() {
        credit_lamports(destinations[credit].0, destinations[credit].1)?;
        credit += 1;
    }
    let mut observed = 0usize;
    while observed < destinations.len() {
        let mut expected_delta = 0u64;
        let mut contribution = 0usize;
        while contribution < destinations.len() {
            if destinations[contribution].0.key == destinations[observed].0.key {
                expected_delta = expected_delta
                    .checked_add(destinations[contribution].1)
                    .ok_or(ClutchError::Arithmetic)?;
            }
            contribution += 1;
        }
        require(
            before[observed].checked_add(expected_delta)
                == Some(destinations[observed].0.lamports()),
            ClutchError::MismatchedState,
        )?;
        observed += 1;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn prepare_dealer_position_replay_close_v3(
    program_id: &Pubkey,
    bound: clutch_collateral_adapter_v2::BoundCollateralProfileV2,
    outcome_count: u8,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    terminal_position: DealerPositionObservationV3,
    terminal_replay: DealerFacilityReplayV1,
    replay_pre_ordinal: u64,
    position_refund_owner: &AccountInfo<'_>,
    replay_refund_owner: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
) -> Outcome<PreparedDealerPositionReplayCloseV3> {
    let position = terminal_position.projection.position();
    let runtime_program = retirement_id(id(program_id))?;
    let neutral_sink_id = retirement_id(id(neutral_sink.key))?;
    let expected_terminal_ordinal = replay_pre_ordinal
        .checked_add(1)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        position_account.owner == program_id
            && replay_account.owner == program_id
            && position_account.is_writable
            && replay_account.is_writable
            && !position_account.is_signer
            && !replay_account.is_signer
            && !position_account.executable
            && !replay_account.executable
            && position_account.data_len() == POSITION_V3_BYTES
            && replay_account.data_len()
                == clutch_dealer_runtime_contract::DEALER_FACILITY_REPLAY_BYTES_V1
            && terminal_position.account_id == id(position_account.key)
            && terminal_position.semantic_id
                == Id::from_bytes(
                    position
                        .semantic_id(&RuntimeSha256)
                        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                        .bytes(),
                )
            && position.lifecycle() == PositionLifecycleV3::CloseRequested
            && position.cash_atoms() == 0
            && position.reserved_cash_atoms() == 0
            && position.native_eggs() == [0; clutch_retirement::MAX_OUTCOMES]
            && position.outstanding_reservations() == 0
            && terminal_replay.lifecycle() == clutch_retirement::ReplayV3Lifecycle::Terminal
            && terminal_replay.position_generation() == position.generation()
            && terminal_replay.replay_account_id() == id(replay_account.key)
            && terminal_replay.facility_position_account_id() == id(position_account.key)
            && terminal_replay.facility_position_binding_id()
                == Id::from_bytes(position.purpose_binding_id().bytes())
            && terminal_replay.next_transition_ordinal() == expected_terminal_ordinal
            && position.rent().payer.bytes() == position_refund_owner.key.to_bytes()
            && terminal_replay.rent().payer().bytes()
                == replay_refund_owner.key.to_bytes()
            && neutral_sink.key != position_account.key
            && neutral_sink.key != replay_account.key
            && neutral_sink.key != position_refund_owner.key
            && neutral_sink.key != replay_refund_owner.key,
        ClutchError::MismatchedState,
    )?;
    for recipient in [position_refund_owner, replay_refund_owner, neutral_sink] {
        require(
            recipient.owner == &SYSTEM_PROGRAM_ID
                && recipient.is_writable
                && !recipient.executable
                && recipient.data_is_empty(),
            ClutchError::MismatchedState,
        )?;
    }

    let position_pda = seeds::position_v3_pda(
        program_id,
        &position.market_instance_id().bytes(),
        &position.owner().bytes(),
        position.purpose(),
        &position.purpose_binding_id().bytes(),
    );
    let replay_pda = seeds::purpose_replay_v3_pda(
        program_id,
        &position_account.key.to_bytes(),
        position.purpose(),
        &position.purpose_binding_id().bytes(),
    );
    expect_pda(position_account.key, position_pda, Some(position.stored_bump()))?;
    expect_pda(
        replay_account.key,
        replay_pda,
        Some(terminal_replay.pda_seeds().stored_bump()),
    )?;

    let expected_position = position
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_data = position_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(position_data.as_ref() == &expected_position[..], ClutchError::MismatchedState)?;
    let position_authenticated = authenticate_position_v3_exact(
        RetirementAccountViewV2 {
            address: retirement_id(id(position_account.key))?,
            owner: runtime_program,
            data: &position_data,
            is_writable: true,
            is_executable: false,
        },
        runtime_program,
        CanonicalPdaV1::after_derivation(retirement_id(id(&position_pda.0))?, position_pda.1),
        AccountAccessV2::Writable,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let mut terminal_replay_bytes =
        [0u8; clutch_dealer_runtime_contract::DEALER_FACILITY_REPLAY_BYTES_V1];
    terminal_replay
        .encode_into(&mut terminal_replay_bytes)
        .map_err(dealer_fault)?;
    let replay_authenticated = authenticate_purpose_replay_v3_exact(
        RetirementAccountViewV2 {
            address: retirement_id(id(replay_account.key))?,
            owner: runtime_program,
            data: &terminal_replay_bytes,
            is_writable: true,
            is_executable: false,
        },
        runtime_program,
        CanonicalPdaV1::after_derivation(retirement_id(id(&replay_pda.0))?, replay_pda.1),
        AccountAccessV2::Writable,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let realm = PositionV3RetirementRealmV1::after_immutable_realm_authentication(
        bound,
        outcome_count,
        neutral_sink_id,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let position_payer = retirement_id(id(position_refund_owner.key))?;
    let replay_payer = retirement_id(id(replay_refund_owner.key))?;
    let mut recipients = [None; clutch_retirement::MAX_RETIREMENT_RECIPIENTS];
    recipients[0] = Some(RetirementRecipientViewV1 {
        address: position_payer,
        lamports: position_refund_owner.lamports(),
        is_writable: true,
        is_executable: false,
    });
    let sink_index = if replay_payer == position_payer {
        1usize
    } else {
        recipients[1] = Some(RetirementRecipientViewV1 {
            address: replay_payer,
            lamports: replay_refund_owner.lamports(),
            is_writable: true,
            is_executable: false,
        });
        2usize
    };
    recipients[sink_index] = Some(RetirementRecipientViewV1 {
        address: neutral_sink_id,
        lamports: neutral_sink.lamports(),
        is_writable: true,
        is_executable: false,
    });
    let plan = authenticate_and_prepare_position_replay_close_v4(
        PositionReplayCloseRuntimeRequestV4 {
            position: position_authenticated,
            replay: replay_authenticated,
            realm,
            signed_sequence: expected_terminal_ordinal,
            position_lamports: position_account.lamports(),
            replay_lamports: replay_account.lamports(),
            recipients,
        },
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_credit = plan
        .recipient_credits()
        .get(position_payer)
        .ok_or(ClutchError::MismatchedState)?;
    let replay_credit = plan
        .recipient_credits()
        .get(replay_payer)
        .ok_or(ClutchError::MismatchedState)?;
    let neutral_credit = plan
        .recipient_credits()
        .get(neutral_sink_id)
        .ok_or(ClutchError::MismatchedState)?;
    let close_receipt_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            DEALER_POSITION_REPLAY_CLOSE_POSTWRITE_DOMAIN_V3,
            program_id.as_ref(),
            position_account.key.as_ref(),
            replay_account.key.as_ref(),
            &terminal_position.semantic_id.bytes(),
            &plan.replay_terminal_semantic_id().bytes(),
            &replay_pre_ordinal.to_le_bytes(),
            &expected_terminal_ordinal.to_le_bytes(),
            position_refund_owner.key.as_ref(),
            replay_refund_owner.key.as_ref(),
            neutral_sink.key.as_ref(),
            &position_credit.credit_lamports.to_le_bytes(),
            &replay_credit.credit_lamports.to_le_bytes(),
            &neutral_credit.credit_lamports.to_le_bytes(),
            &plan.position_lamports_after().to_le_bytes(),
            &plan.replay_lamports_after().to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(close_receipt_id != ContentId::ZERO, ClutchError::MismatchedState)?;
    drop(position_data);
    Ok(PreparedDealerPositionReplayCloseV3 {
        plan,
        terminal_replay,
        close_receipt_id,
    })
}

fn apply_dealer_position_replay_close_v3(
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    position_refund_owner: &AccountInfo<'_>,
    replay_refund_owner: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    prepared: PreparedDealerPositionReplayCloseV3,
) -> Outcome<ContentId> {
    let position_payer = retirement_id(id(position_refund_owner.key))?;
    let replay_payer = retirement_id(id(replay_refund_owner.key))?;
    let neutral_sink_id = retirement_id(id(neutral_sink.key))?;
    let credits = prepared.plan.recipient_credits();
    for (recipient, account) in [
        (position_payer, position_refund_owner),
        (replay_payer, replay_refund_owner),
        (neutral_sink_id, neutral_sink),
    ] {
        let credit = credits
            .get(recipient)
            .ok_or(ClutchError::MismatchedState)?;
        require(
            account.lamports().checked_add(credit.credit_lamports)
                == Some(credit.balance_after),
            ClutchError::MismatchedState,
        )?;
    }

    let tombstone = prepared.plan.position_tombstone_bytes();
    **position_account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? =
        prepared.plan.position_lamports_after();
    position_account
        .resize(POSITION_TOMBSTONE_V3_BYTES)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    position_account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&tombstone);
    release_dealer_account(replay_account)?;
    for (recipient, account) in [
        (position_payer, position_refund_owner),
        (replay_payer, replay_refund_owner),
        (neutral_sink_id, neutral_sink),
    ] {
        let credit = credits
            .get(recipient)
            .ok_or(ClutchError::MismatchedState)?;
        **account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? =
            credit.balance_after;
    }

    let observed_tombstone = PositionTombstoneV3::decode(
        &position_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        observed_tombstone
            == PositionTombstoneV3::decode(&tombstone)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && position_account.data_len() == POSITION_TOMBSTONE_V3_BYTES
            && position_account.lamports() == prepared.plan.position_lamports_after()
            && replay_account.owner == &SYSTEM_PROGRAM_ID
            && replay_account.data_is_empty()
            && replay_account.lamports() == prepared.plan.replay_lamports_after(),
        ClutchError::MismatchedState,
    )?;
    for (recipient, account) in [
        (position_payer, position_refund_owner),
        (replay_payer, replay_refund_owner),
        (neutral_sink_id, neutral_sink),
    ] {
        require(
            credits
                .get(recipient)
                .map(|credit| credit.balance_after)
                == Some(account.lamports()),
            ClutchError::MismatchedState,
        )?;
    }
    Ok(prepared.close_receipt_id())
}

fn apply_epoch_close(
    epoch_account: &AccountInfo<'_>,
    bind_receipt_account: &AccountInfo<'_>,
    epoch_payer: &AccountInfo<'_>,
    bind_receipt_payer: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    credits: DealerEpochCloseCreditsV2,
) -> Outcome<()> {
    require(
        credits.epoch_refund_recipient.bytes() == epoch_payer.key.to_bytes()
            && credits.bind_receipt_refund_recipient.bytes()
                == bind_receipt_payer.key.to_bytes()
            && credits.epoch_neutral_sink.bytes() == neutral_sink.key.to_bytes()
            && credits.bind_receipt_neutral_sink.bytes() == neutral_sink.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    release_dealer_account(epoch_account)?;
    release_dealer_account(bind_receipt_account)?;
    let sink_credit = credits
        .epoch_sink_lamports
        .checked_add(credits.bind_receipt_sink_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    credit_exact_dealer_terminal_lamports([
        (epoch_payer, credits.epoch_refund_lamports),
        (bind_receipt_payer, credits.bind_receipt_refund_lamports),
        (neutral_sink, sink_credit),
        (neutral_sink, 0),
    ])?;
    require_released_dealer_account(epoch_account)?;
    require_released_dealer_account(bind_receipt_account)
}

fn apply_liveness_transition(
    compartment: &AccountInfo<'_>,
    actor: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    transition: &clutch_liveness::runtime_adapter_v1::RuntimeAtomicTransitionV1,
) -> Outcome<()> {
    require(compartment.is_writable, ClutchError::NotWritable)?;
    require(
        actor.is_writable && payer.is_writable,
        ClutchError::NotWritable,
    )?;
    require(
        transition.account_id.bytes() == compartment.key.to_bytes()
            && transition.account_balance_before == compartment.lamports()
            && transition.write_account_data
            && !transition.close_account,
        ClutchError::MismatchedState,
    )?;
    let mut actor_credit = 0u64;
    let mut payer_credit = 0u64;
    for movement in transition.transfers() {
        match movement.role {
            RuntimeTransferRoleV1::KeeperPayment => {
                require(
                    movement.destination.bytes() == actor.key.to_bytes(),
                    ClutchError::MismatchedState,
                )?;
                actor_credit = actor_credit
                    .checked_add(movement.lamports)
                    .ok_or(ClutchError::Arithmetic)?;
            }
            RuntimeTransferRoleV1::PayerWorkRefund => {
                require(
                    movement.destination.bytes() == payer.key.to_bytes(),
                    ClutchError::MismatchedState,
                )?;
                payer_credit = payer_credit
                    .checked_add(movement.lamports)
                    .ok_or(ClutchError::Arithmetic)?;
            }
            _ => return Err(ClutchError::MismatchedState.into()),
        }
    }
    **compartment
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? =
        transition.account_balance_after;
    if actor.key == payer.key {
        let credit = actor_credit
            .checked_add(payer_credit)
            .ok_or(ClutchError::Arithmetic)?;
        let after = actor
            .lamports()
            .checked_add(credit)
            .ok_or(ClutchError::Arithmetic)?;
        **actor
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = after;
    } else {
        let actor_after = actor
            .lamports()
            .checked_add(actor_credit)
            .ok_or(ClutchError::Arithmetic)?;
        let payer_after = payer
            .lamports()
            .checked_add(payer_credit)
            .ok_or(ClutchError::Arithmetic)?;
        **actor
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = actor_after;
        **payer
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = payer_after;
    }
    compartment
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&transition.post_account_data);
    Ok(())
}

/// Authenticate one funded call while treating any balance above the last
/// persisted observation as hostile donation, never as work principal.
///
/// The generic runtime deliberately makes donation observation an explicit
/// transition. The SBF boundary composes that transition with the funded call
/// before writing either postimage, so unsolicited lamports cannot stall the
/// facility and cannot alter the keeper ceiling or payer refund.
#[inline(never)]
fn plan_liveness_spend_absorbing_donation(
    program_id: &Pubkey,
    policy_account: &AccountInfo<'_>,
    compartment_account: &AccountInfo<'_>,
    compartment: RuntimeCompartmentV1,
    spend_intent: RuntimeTransitionIntentV1,
    receipt: RuntimeReceiptObservationV1,
) -> Outcome<RuntimeAtomicTransitionV1> {
    require(
        spend_intent.action == RuntimeTransitionActionV1::SpendWork
            && spend_intent.account_id.bytes() == compartment_account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let actual_balance = compartment_account.lamports();
    let expected_balance = compartment
        .expected_account_balance_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(actual_balance >= expected_balance, ClutchError::MismatchedState)?;
    let account_balance_after = actual_balance
        .checked_sub(spend_intent.call_ceiling_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let expected_runtime_program_id =
        clutch_liveness::Id::from_bytes(program_id.to_bytes());
    let expected_policy_account_id =
        clutch_liveness::Id::from_bytes(policy_account.key.to_bytes());

    if actual_balance == expected_balance {
        let policy_data = policy_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let compartment_data = compartment_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        return plan_runtime_transition_v1(
            expected_runtime_program_id,
            expected_policy_account_id,
            RuntimePersistedAccountViewV1 {
                account_id: expected_policy_account_id,
                owner_program_id: expected_runtime_program_id,
                lamports: policy_account.lamports(),
                data: &policy_data,
                writable: false,
            },
            RuntimePersistedAccountViewV1 {
                account_id: clutch_liveness::Id::from_bytes(
                    compartment_account.key.to_bytes(),
                ),
                owner_program_id: expected_runtime_program_id,
                lamports: actual_balance,
                data: &compartment_data,
                writable: true,
            },
            spend_intent,
            Some(receipt),
            account_balance_after,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState));
    }

    let donation_intent = RuntimeTransitionIntentV1 {
        action: RuntimeTransitionActionV1::ObserveDonation,
        kind: compartment.kind,
        policy_id: compartment.identity.policy_id,
        lifecycle_id: compartment.identity.lifecycle_id,
        account_id: compartment.identity.account_id,
        semantic_owner: compartment.identity.owner,
        quote_schedule_id: compartment.quote_schedule_id,
        receipt_id: clutch_liveness::Id::ZERO,
        keeper: clutch_liveness::Id::ZERO,
        generation: compartment.identity.generation,
        call_ordinal: 0,
        call_ceiling_lamports: 0,
        keeper_payment_lamports: 0,
        flags: 0,
    };
    let donation_transition = {
        let policy_data = policy_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let compartment_data = compartment_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        plan_runtime_transition_v1(
            expected_runtime_program_id,
            expected_policy_account_id,
            RuntimePersistedAccountViewV1 {
                account_id: expected_policy_account_id,
                owner_program_id: expected_runtime_program_id,
                lamports: policy_account.lamports(),
                data: &policy_data,
                writable: false,
            },
            RuntimePersistedAccountViewV1 {
                account_id: clutch_liveness::Id::from_bytes(
                    compartment_account.key.to_bytes(),
                ),
                owner_program_id: expected_runtime_program_id,
                lamports: actual_balance,
                data: &compartment_data,
                writable: true,
            },
            donation_intent,
            None,
            actual_balance,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
    };
    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    plan_runtime_transition_v1(
        expected_runtime_program_id,
        expected_policy_account_id,
        RuntimePersistedAccountViewV1 {
            account_id: expected_policy_account_id,
            owner_program_id: expected_runtime_program_id,
            lamports: policy_account.lamports(),
            data: &policy_data,
            writable: false,
        },
        RuntimePersistedAccountViewV1 {
            account_id: clutch_liveness::Id::from_bytes(compartment_account.key.to_bytes()),
            owner_program_id: expected_runtime_program_id,
            lamports: actual_balance,
            data: &donation_transition.post_account_data,
            writable: true,
        },
        spend_intent,
        Some(receipt),
        account_balance_after,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

#[inline(never)]
fn initialize_facility(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    require_count(accounts, INITIALIZE_ACCOUNT_COUNT)?;
    let payload = DealerRuntimePayloadV1::decode(DealerFacilityAction::Initialize, payload_bytes)
        .map_err(dealer_fault)?;
    require(sequence == 0, ClutchError::Replay)?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    require(!accounts[1].is_writable, ClutchError::UnexpectedWritable)?;
    require(
        !accounts[1].is_signer && !accounts[1].executable,
        ClutchError::MismatchedState,
    )?;
    require_initialize_aliases(accounts)?;

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[2])?;
    let (collateral_value, market) = authenticate_dealer_collateral_value_v2(
        program_id,
        &policy,
        None,
        DealerCollateralAuthorityAccountsV2 {
            realm: &accounts[22],
            profile: &accounts[23],
            policy: &accounts[24],
            token_program: &accounts[25],
            token_programdata: &accounts[26],
            market_binding: &accounts[27],
            market_runtime: &accounts[28],
            market_instance: &accounts[29],
            hoard: &accounts[30],
            claim_ledger: &accounts[31],
        },
    )?;
    let sponsor_replay_sequence = current_general_replay_sequence_v1(&accounts[32])?;
    let sponsor_authority = authenticate_general_position_replay_v2(
        program_id,
        collateral_value.liabilities.bound,
        &accounts[27],
        &accounts[28],
        &accounts[3],
        &accounts[32],
        accounts[0].key.to_bytes(),
        sponsor_replay_sequence,
    )?;
    let sponsor_position = sponsor_authority.position.semantic;
    let sponsor_projection = sponsor_authority.projection;
    let sponsor = Id::from_bytes(sponsor_position.owner().bytes());
    let genesis = DealerFacilityGenesisV1 {
        policy_id: Id::from_bytes(policy_id),
        sponsor,
        sponsor_refund_recipient: id(accounts[1].key),
        facility_nonce: payload.facility_nonce,
    };
    let facility_id = genesis
        .facility_id_for_policy(&policy)
        .map_err(dealer_fault)?
        .untyped();
    let (state_address, state_bump) = seeds::dealer_state_v2_pda(program_id, &facility_id.bytes());
    expect_pda(accounts[4].key, (state_address, state_bump), None)?;
    let binding = FacilityPositionBindingV2 {
        facility_id,
        policy_id: Id::from_bytes(policy_id),
        market_instance_v2_id: policy.market_instance_v2_id,
        collateral_policy_id: market.collateral_policy_id,
        collateral_release_id: market.collateral_release_id,
        dealer_state_account_id: id(accounts[4].key),
        initial_position_generation: 1,
    };
    let binding_id = binding
        .binding_id_for(&genesis, &policy)
        .map_err(dealer_fault)?;
    let (facility_position_address, facility_position_bump) = seeds::position_v3_pda(
        program_id,
        &policy.market_instance_v2_id.bytes(),
        &facility_id.bytes(),
        PositionPurposeV3::DealerFacility,
        &binding_id.bytes(),
    );
    expect_pda(
        accounts[5].key,
        (facility_position_address, facility_position_bump),
        None,
    )?;
    let (replay_address, replay_bump) = seeds::purpose_replay_v3_pda(
        program_id,
        &accounts[5].key.to_bytes(),
        PositionPurposeV3::DealerFacility,
        &binding_id.bytes(),
    );
    expect_pda(accounts[6].key, (replay_address, replay_bump), None)?;
    let (dependency_address, dependency_bump) =
        seeds::dealer_funded_v2_pda(program_id, &facility_id.bytes());
    expect_pda(accounts[7].key, (dependency_address, dependency_bump), None)?;
    let (future_credit_funding_address, future_credit_funding_bump) =
        seeds::dealer_future_credit_funding_pda(program_id, &facility_id.bytes());
    expect_pda(
        accounts[33].key,
        (future_credit_funding_address, future_credit_funding_bump),
        None,
    )?;
    for account in [
        &accounts[4],
        &accounts[5],
        &accounts[6],
        &accounts[7],
        &accounts[17],
        &accounts[33],
    ] {
        require_creatable(account)?;
    }

    let schedule = authenticate_schedule(program_id, &accounts[8])?;
    require(
        schedule.schedule_id().map_err(dealer_fault)?.untyped() == policy.liveness_policy_id,
        ClutchError::MismatchedState,
    )?;
    require(
        accounts[9].owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require(
        accounts[9].data_len() == RUNTIME_LIVENESS_POLICY_BYTES_V1,
        ClutchError::WrongDataLength,
    )?;
    require(
        !accounts[9].is_writable && !accounts[9].is_signer && !accounts[9].executable,
        ClutchError::MismatchedState,
    )?;
    require_signer(&accounts[18])?;
    require(accounts[18].is_writable, ClutchError::NotWritable)?;
    let rent = read_rent(&accounts[20])?;
    require_system_program(&accounts[21])?;
    let runtime_policy = RuntimeLivenessPolicyV1::decode(&accounts[9].data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let runtime_policy_id = dealer_runtime_liveness_policy_id_v1(runtime_policy)
        .map_err(dealer_fault)?;
    expect_pda(
        accounts[9].key,
        seeds::dealer_runtime_liveness_policy_pda(program_id, &runtime_policy_id.bytes()),
        None,
    )?;
    require(
        accounts[9].lamports() >= rent.minimum_balance(RUNTIME_LIVENESS_POLICY_BYTES_V1)?,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    let runtime_account_rent = rent.minimum_balance(RUNTIME_LIVENESS_ACCOUNT_BYTES_V1)?;
    let (first_runtime_state, first_runtime_bump) = prepare_runtime_compartment_admission(
        program_id,
        facility_id,
        id(accounts[4].key),
        &accounts[18],
        &accounts[10],
        runtime_policy,
        RUNTIME_COMPARTMENT_ORDER_V1[0],
        runtime_account_rent,
    )?;
    let mut runtime_states = [first_runtime_state; RUNTIME_COMPARTMENT_COUNT_V1];
    let mut runtime_bumps = [first_runtime_bump; RUNTIME_COMPARTMENT_COUNT_V1];
    let mut total_runtime_debit = runtime_policy
        .compartment(RUNTIME_COMPARTMENT_ORDER_V1[0])
        .total_payer_debit_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut runtime_index = 1usize;
    while runtime_index < RUNTIME_COMPARTMENT_COUNT_V1 {
        let kind = RUNTIME_COMPARTMENT_ORDER_V1[runtime_index];
        let (state, bump) = prepare_runtime_compartment_admission(
            program_id,
            facility_id,
            id(accounts[4].key),
            &accounts[18],
            &accounts[10 + runtime_index],
            runtime_policy,
            kind,
            runtime_account_rent,
        )?;
        runtime_states[runtime_index] = state;
        runtime_bumps[runtime_index] = bump;
        total_runtime_debit = total_runtime_debit
            .checked_add(
                runtime_policy
                    .compartment(kind)
                    .total_payer_debit_lamports()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
            )
            .ok_or(ClutchError::Arithmetic)?;
        runtime_index += 1;
    }
    let runtime_bundle = RuntimeLivenessBundleV1 {
        policy_id: runtime_policy.policy_id,
        lifecycle_id: liveness_id(facility_id),
        compartments: runtime_states,
    };
    runtime_bundle
        .validate(runtime_policy)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        total_runtime_debit
            == runtime_policy
                .total_payer_debit_lamports()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ClutchError::MismatchedState,
    )?;
    let runtime_binding = DealerRuntimeLivenessBindingV1::from_canonical(
        &runtime_policy,
        &runtime_states,
    )
    .map_err(dealer_fault)?;
    require(
        runtime_binding.realm_id() == policy.realm_id
            && runtime_binding.lifecycle_id() == facility_id
            && runtime_binding.neutral_sink() == policy.neutral_sink,
        ClutchError::MismatchedState,
    )?;
    runtime_index = 1;
    while runtime_index < RUNTIME_COMPARTMENT_COUNT_V1 {
        let compartment = match runtime_index {
            1 => DealerLivenessCompartmentV1::Candidate,
            2 => DealerLivenessCompartmentV1::Clearing,
            3 => DealerLivenessCompartmentV1::Settlement,
            4 => DealerLivenessCompartmentV1::Resolution,
            5 => DealerLivenessCompartmentV1::Retirement,
            6 => DealerLivenessCompartmentV1::Recovery,
            _ => return Err(ClutchError::MismatchedState.into()),
        };
        require(
            runtime_binding.owner(compartment) == id(accounts[4].key)
                && runtime_binding.receipt_program_id(compartment) == id(program_id),
            ClutchError::MismatchedState,
        )?;
        runtime_index += 1;
    }

    require(
        accounts[8].lamports() >= rent.minimum_balance(DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES)?,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    let state_principal = rent.minimum_balance(DEALER_STATE_V2_ACCOUNT_BYTES)?;
    let state_permanent = rent.minimum_balance(DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_BYTES)?;
    let state_refundable = state_principal
        .checked_sub(state_permanent)
        .ok_or(ClutchError::Arithmetic)?;
    let position_principal = rent.minimum_balance(POSITION_V3_BYTES)?;
    let position_permanent = rent.minimum_balance(POSITION_TOMBSTONE_V3_BYTES)?;
    let position_refundable = position_principal
        .checked_sub(position_permanent)
        .ok_or(ClutchError::Arithmetic)?;
    let replay_principal =
        rent.minimum_balance(clutch_dealer_runtime_contract::DEALER_FACILITY_REPLAY_BYTES_V1)?;
    let dependency_principal = rent.minimum_balance(DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES)?;
    let receipt_principal = rent.minimum_balance(DEALER_ACTION_RECEIPT_ACCOUNT_BYTES)?;
    let future_credit_funding_principal =
        rent.minimum_balance(DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_BYTES)?;
    let future_credit_live_principal =
        rent.minimum_balance(FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_BYTES)?;
    let future_credit_tombstone_principal =
        rent.minimum_balance(FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_BYTES)?;
    let future_credit_refundable_principal = future_credit_live_principal
        .checked_sub(future_credit_tombstone_principal)
        .ok_or(ClutchError::Arithmetic)?;

    let facility_position_pre = PositionAccountV3::new(PositionV3Fields {
        purpose: PositionPurposeV3::DealerFacility,
        lifecycle: PositionLifecycleV3::Open,
        outcome_count: policy.outcome_count,
        stored_bump: facility_position_bump,
        generation: 1,
        market_instance_id: retirement_id(policy.market_instance_v2_id)?,
        realm_id: retirement_id(policy.realm_id)?,
        collateral_policy_id: sponsor_position.collateral_policy_id(),
        collateral_release_id: sponsor_position.collateral_release_id(),
        owner: retirement_id(facility_id)?,
        controller: retirement_id(id(accounts[4].key))?,
        replay_account: retirement_id(id(accounts[6].key))?,
        purpose_binding_id: retirement_id(binding_id)?,
        cash_atoms: 0,
        reserved_cash_atoms: 0,
        native_eggs: [0; clutch_retirement::MAX_OUTCOMES],
        outstanding_reservations: 0,
        rent: RentSplitV2 {
            payer: retirement_id(id(accounts[0].key))?,
            refundable_live_principal: position_refundable,
            permanent_tombstone_principal: position_permanent,
            donation_floor: accounts[5].lamports(),
        },
    })
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let facility_projection_pre = project_dealer_position_v3(
        facility_position_pre,
        AdapterPositionMarketBindingV3 {
            market_instance_id: facility_position_pre.market_instance_id(),
            outcome_count: facility_position_pre.outcome_count(),
            realm_id: facility_position_pre.realm_id(),
            collateral_policy_id: facility_position_pre.collateral_policy_id(),
            collateral_release_id: facility_position_pre.collateral_release_id(),
        },
        AdapterPositionPurposeBindingV3 {
            owner: facility_position_pre.owner(),
            controller: facility_position_pre.controller(),
            purpose_binding_id: facility_position_pre.purpose_binding_id(),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let transfer = prepare_dealer_sponsor_funding_transfer_v2(
        market,
        sponsor,
        payload.sponsor_capital_atoms,
        DealerTransferPositionV3::General {
            account_id: id(accounts[3].key),
            position: sponsor_projection,
        },
        DealerTransferPositionV3::Facility {
            account_id: id(accounts[5].key),
            position: facility_projection_pre,
        },
    )
    .map_err(dealer_fault)?;
    let facility_position_post = transfer.destination_post();
    let facility_projection_post = project_dealer_position_v3(
        facility_position_post,
        AdapterPositionMarketBindingV3 {
            market_instance_id: facility_position_post.market_instance_id(),
            outcome_count: facility_position_post.outcome_count(),
            realm_id: facility_position_post.realm_id(),
            collateral_policy_id: facility_position_post.collateral_policy_id(),
            collateral_release_id: facility_position_post.collateral_release_id(),
        },
        AdapterPositionPurposeBindingV3 {
            owner: facility_position_post.owner(),
            controller: facility_position_post.controller(),
            purpose_binding_id: facility_position_post.purpose_binding_id(),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let facility_semantic_id = Id::from_bytes(
        facility_position_post
            .semantic_id(&RuntimeSha256)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes(),
    );
    let facility_observation = DealerPositionObservationV3 {
        account_id: id(accounts[5].key),
        semantic_id: facility_semantic_id,
        projection: facility_projection_post,
    };

    let clearing = runtime_states[DealerLivenessCompartmentV1::Clearing.index()];
    require(
        clearing.identity.payer.bytes() == accounts[18].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let receipt = DealerActionReceiptV1 {
        policy_id: Id::from_bytes(policy_id),
        facility_id,
        dealer_state_account_id: id(accounts[4].key),
        liveness_schedule_id: policy.liveness_policy_id,
        runtime_policy_id: runtime_binding.runtime_policy_id(),
        runtime_account_id: runtime_binding.account_id(DealerLivenessCompartmentV1::Clearing),
        runtime_owner: runtime_binding.owner(DealerLivenessCompartmentV1::Clearing),
        quote_schedule_id: runtime_binding.quote_schedule_id(DealerLivenessCompartmentV1::Clearing),
        receipt_account_id: id(accounts[17].key),
        receipt_program_id: id(program_id),
        keeper: id(accounts[0].key),
        replay_account_id: id(accounts[6].key),
        action: DealerRuntimeActionV1::Initialize,
        compartment: DealerLivenessCompartmentV1::Clearing,
        runtime_generation: runtime_binding.generation(DealerLivenessCompartmentV1::Clearing),
        facility_generation: 1,
        call_ordinal: payload.liveness_call_ordinal,
        call_ceiling_lamports: schedule.reward_lamports[DealerRuntimeActionV1::Initialize as usize],
        keeper_payment_lamports: payload.keeper_payment_lamports,
        expected_replay_ordinal: 0,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: receipt_principal,
            donation_floor: accounts[17].lamports(),
        },
    };
    let receipt_slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
    let (receipt_address, receipt_bump) =
        seeds::dealer_action_receipt_pda(program_id, &receipt_slot.bytes());
    expect_pda(accounts[17].key, (receipt_address, receipt_bump), None)?;

    let dependency = DealerFundedDependenciesV2 {
        bindings: DealerFundedBudgetDependenciesV1 {
            policy_id: Id::from_bytes(policy_id),
            facility_id,
            liveness_schedule_id: policy.liveness_policy_id,
            runtime_liveness_policy_id: runtime_binding.runtime_policy_id(),
            runtime_liveness_program_id: id(program_id),
            runtime_liveness_policy_account_id: id(accounts[9].key),
            runtime_liveness_binding_digest: runtime_binding
                .binding_digest()
                .map_err(dealer_fault)?,
            fee_policy_id: policy.fee_policy_id,
            collateral_mint: policy.collateral_mint,
            token_program: policy.token_program,
            asset_vault_authority_account_id: id(accounts[4].key),
            neutral_sink: policy.neutral_sink,
            counted_generation: 0,
            dealer_liveness_work_principal_lamports: schedule
                .dealer_runtime_work_principal_lamports()
                .map_err(dealer_fault)?,
        },
        facility_position_binding_id: binding_id,
        initialize_receipt_account_id: id(accounts[17].key),
        initialize_receipt_semantic_id: receipt.semantic_receipt_id().map_err(dealer_fault)?,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: dependency_principal,
            donation_floor: accounts[7].lamports(),
        },
    };
    let future_credit_funding = DealerFutureCreditFundingV1 {
        funding_account_id: id(accounts[33].key),
        policy_id: Id::from_bytes(policy_id),
        facility_id,
        market_instance_v2_id: policy.market_instance_v2_id,
        realm_id: policy.realm_id,
        collateral_policy_id: market.collateral_policy_id,
        collateral_release_id: market.collateral_release_id,
        collateral_value_receipt_id: market.collateral_value_receipt_id,
        dealer_state_account_id: id(accounts[4].key),
        facility_position_account_id: id(accounts[5].key),
        facility_position_binding_id: binding_id,
        dealer_replay_account_id: id(accounts[6].key),
        refund_owner: id(accounts[0].key),
        neutral_sink: policy.neutral_sink,
        founding_generation: 1,
        funding_account_principal_lamports: future_credit_funding_principal,
        credit_refundable_principal_lamports: future_credit_refundable_principal,
        credit_tombstone_principal_lamports: future_credit_tombstone_principal,
        donation_floor_lamports: accounts[33].lamports(),
    };
    future_credit_funding.validate().map_err(dealer_fault)?;
    let state = DealerStateV2 {
        policy_id: Id::from_bytes(policy_id),
        facility_id,
        facility_position_binding_id: binding_id,
        facility_position_id: facility_semantic_id,
        facility_position_account_id: id(accounts[5].key),
        facility_replay_account_id: id(accounts[6].key),
        sponsor,
        sponsor_refund_recipient: id(accounts[1].key),
        lp_page_head_id: Id::ZERO,
        lp_page_set_root: Id::ZERO,
        last_lp_owner: Id::ZERO,
        active_epoch_id: Id::ZERO,
        active_epoch_binding_account_id: Id::ZERO,
        active_lease_id: Id::ZERO,
        funded_dependencies_id: dependency.dependency_id().map_err(dealer_fault)?,
        funded_dependencies_account_id: id(accounts[7].key),
        terminal_position_tombstone_id: Id::ZERO,
        terminal_replay_semantic_id: Id::ZERO,
        terminal_replay_intent_id: Id::ZERO,
        terminal_state_receipt_id: Id::ZERO,
        phase: DealerPhaseV2::Funding,
        sponsor_capital_disposition: SponsorCapitalDispositionV1::Refundable,
        outcome_count: policy.outcome_count,
        generation: 1,
        child_sequence: 0,
        total_shares: 0,
        queued_shares: 0,
        terminal_claimed_shares: 0,
        sponsor_capital_atoms: payload.sponsor_capital_atoms,
        net_sold: [0; clutch_dealer_runtime_contract::MAX_OUTCOMES],
        children: DealerChildCountsV2 {
            facility_positions: 1,
            facility_replays: 1,
            funded_dependencies: 1,
            ..DealerChildCountsV2::default()
        },
        rent: RootRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_live_principal: state_refundable,
            permanent_tombstone_principal: state_permanent,
            donation_floor: accounts[4].lamports(),
        },
    };
    state
        .validate_against_policy(&policy)
        .map_err(dealer_fault)?;
    let replay = DealerFacilityReplayV1::founding(
        id(accounts[5].key),
        id(accounts[6].key),
        binding_id,
        1,
        replay_bump,
        ReplayRentOwnerV1::from_persisted(
            retirement_id(id(accounts[0].key))?,
            replay_principal,
            accounts[6].lamports(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
    )
    .map_err(dealer_fault)?;
    let authorization = receipt
        .authorization(&schedule, &runtime_binding, &clearing)
        .map_err(dealer_fault)?;
    let liveness_intent = receipt.runtime_transition_intent().map_err(dealer_fault)?;
    let liveness_observation = receipt
        .runtime_receipt_observation()
        .map_err(dealer_fault)?;
    let clearing_before_balance = clearing
        .expected_account_balance_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let clearing_after_balance = clearing_before_balance
        .checked_sub(receipt.call_ceiling_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let mut clearing_pre_data = [0u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1];
    clearing
        .encode(&mut clearing_pre_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let liveness_transition = plan_runtime_transition_v1(
        clutch_liveness::Id::from_bytes(program_id.to_bytes()),
        clutch_liveness::Id::from_bytes(accounts[9].key.to_bytes()),
        RuntimePersistedAccountViewV1 {
            account_id: clutch_liveness::Id::from_bytes(accounts[9].key.to_bytes()),
            owner_program_id: clutch_liveness::Id::from_bytes(program_id.to_bytes()),
            lamports: accounts[9].lamports(),
            data: &accounts[9].data.borrow(),
            writable: false,
        },
        RuntimePersistedAccountViewV1 {
            account_id: clutch_liveness::Id::from_bytes(accounts[12].key.to_bytes()),
            owner_program_id: clutch_liveness::Id::from_bytes(program_id.to_bytes()),
            lamports: clearing_before_balance,
            data: &clearing_pre_data,
            writable: true,
        },
        liveness_intent,
        Some(liveness_observation),
        clearing_after_balance,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        runtime_policy.policy_id.bytes() == dependency.bindings.runtime_liveness_policy_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let replay_binding = DealerReplayAccountBindingV1 {
        replay_account_id: id(accounts[6].key),
        position_replay_account_id: Id::from_bytes(facility_position_post.replay_account().bytes()),
    };
    let sponsor_general_replay = prepare_dealer_general_replay_v2(
        sponsor_authority,
        transfer.source_post(),
        GeneralReplayTransitionKindV1::DealerSponsorFunding,
        transfer.bundle().bundle_id().map_err(dealer_fault)?,
        market.collateral_value_receipt_id,
        authorization.receipt_semantic_id,
    )?;
    let transfer = bind_dealer_general_position_transfer_v3(transfer, &sponsor_general_replay)
        .map_err(dealer_fault)?;
    let prepared = prepare_facility_initialization_v3(
        &genesis,
        &binding,
        &policy,
        &schedule,
        &runtime_binding,
        id(accounts[4].key),
        id(accounts[7].key),
        &dependency,
        &authorization,
        &facility_observation,
        &state,
        transfer,
        &replay,
        replay_binding,
    )
    .map_err(dealer_fault)?;
    let current_slot = read_clock_slot(&accounts[19])?;
    require(
        current_slot < policy.funding_deadline_slot,
        ClutchError::MismatchedState,
    )?;

    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[4],
        &accounts[21],
        &rent,
        DEALER_STATE_V2_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_STATE_V2,
            &facility_id.bytes(),
            &[state_bump],
        ],
    )?;
    let future_credit_total_principal = future_credit_funding
        .funding_account_principal_lamports
        .checked_add(
            future_credit_funding
                .credit_principal_lamports()
                .map_err(dealer_fault)?,
        )
        .ok_or(ClutchError::Arithmetic)?;
    let observed_future_credit_donation = create_exact_payer_debit_pda(
        program_id,
        &accounts[0],
        &accounts[33],
        &accounts[21],
        future_credit_total_principal,
        DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_FUTURE_CREDIT_FUNDING,
            &facility_id.bytes(),
            &[future_credit_funding_bump],
        ],
    )?;
    require(
        observed_future_credit_donation == future_credit_funding.donation_floor_lamports,
        ClutchError::MismatchedState,
    )?;
    let purpose_seed = [u8::from(PositionPurposeV3::DealerFacility)];
    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[5],
        &accounts[21],
        &rent,
        POSITION_V3_BYTES,
        &[
            clutch_retirement::POSITION_V3_PDA_PREFIX,
            &policy.market_instance_v2_id.bytes(),
            &facility_id.bytes(),
            &purpose_seed,
            &binding_id.bytes(),
            &[facility_position_bump],
        ],
    )?;
    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[6],
        &accounts[21],
        &rent,
        clutch_dealer_runtime_contract::DEALER_FACILITY_REPLAY_BYTES_V1,
        &[
            clutch_retirement::PURPOSE_REPLAY_V3_PDA_PREFIX,
            &accounts[5].key.to_bytes(),
            &purpose_seed,
            &binding_id.bytes(),
            &[replay_bump],
        ],
    )?;
    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[7],
        &accounts[21],
        &rent,
        DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_FUNDED_V2,
            &facility_id.bytes(),
            &[dependency_bump],
        ],
    )?;
    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[17],
        &accounts[21],
        &rent,
        DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_ACTION_RECEIPT,
            &receipt_slot.bytes(),
            &[receipt_bump],
        ],
    )?;
    runtime_index = 0;
    while runtime_index < RUNTIME_COMPARTMENT_COUNT_V1 {
        let kind = RUNTIME_COMPARTMENT_ORDER_V1[runtime_index];
        let kind_seed = [runtime_kind_seed(kind)];
        let payer_debit = runtime_policy
            .compartment(kind)
            .total_payer_debit_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let donation = create_exact_payer_debit_pda(
            program_id,
            &accounts[18],
            &accounts[10 + runtime_index],
            &accounts[21],
            payer_debit,
            RUNTIME_LIVENESS_ACCOUNT_BYTES_V1,
            &[
                seeds::SEED_DEALER_RUNTIME_LIVENESS_ACCOUNT,
                &facility_id.bytes(),
                &kind_seed,
                &[runtime_bumps[runtime_index]],
            ],
        )?;
        require(
            donation == runtime_states[runtime_index].donation_received_lamports,
            ClutchError::MismatchedState,
        )?;
        let mut runtime_data = accounts[10 + runtime_index]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        runtime_states[runtime_index]
            .encode(&mut runtime_data[..])
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        runtime_index += 1;
    }
    apply_liveness_transition(
        &accounts[12],
        &accounts[0],
        &accounts[18],
        &liveness_transition,
    )?;
    accounts[3]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &prepared
                .transfer
                .transfer()
                .source_post()
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    accounts[5]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &prepared
                .transfer
                .transfer()
                .destination_post()
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    let _accepted_transfer = accept_dealer_asset_transfer_postwrite_v2(
        program_id,
        prepared.transfer.transfer().bundle(),
        &accounts[3],
        &accounts[5],
    )?;
    let observed_general_replay =
        write_and_accept_general_replay_v1(program_id, &accounts[32], &sponsor_general_replay)?;
    require(
        observed_general_replay == prepared.transfer.general_replay_post_semantic_id(),
        ClutchError::Replay,
    )?;
    prepared
        .replay
        .replay_post()
        .encode_into(&mut accounts[6].data.borrow_mut())
        .map_err(dealer_fault)?;
    write_dealer_body(
        &accounts[7],
        DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG,
        DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION,
        dependency_bump,
        &dependency,
    )?;
    write_dealer_body(
        &accounts[17],
        DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
        DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
        receipt_bump,
        &receipt,
    )?;
    write_dealer_body(
        &accounts[33],
        DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_TAG,
        DEALER_FUTURE_CREDIT_FUNDING_ACCOUNT_VERSION,
        future_credit_funding_bump,
        &future_credit_funding,
    )?;
    let (_, observed_future_credit_funding) =
        authenticate_future_credit_funding(program_id, &accounts[33], true)?;
    require(
        observed_future_credit_funding == future_credit_funding,
        ClutchError::MismatchedState,
    )?;
    write_dealer_body(
        &accounts[4],
        DEALER_STATE_V2_ACCOUNT_TAG,
        DEALER_STATE_V2_ACCOUNT_VERSION,
        state_bump,
        &prepared.state,
    )
}

/// Enter UnwindOnly under the exact immutable sponsor signature. Before the
/// first lease the authoritative root remains StateV2 and no Product Series
/// obligation exists; after first admission the same transition preserves the
/// exact StateV3/`0xaf` edge. This path is balance-neutral in both cases.
#[inline(never)]
fn sponsor_halt(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    let payload = DealerRuntimePayloadV1::decode(DealerFacilityAction::SponsorHalt, payload_bytes)
        .map_err(dealer_fault)?;
    require(
        sequence == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let (state, state_v3, state_bump, obligation) = if payload.existing_series_admission {
        let authenticated = authenticate_dealer_state_v3(program_id, &accounts[2], true)?;
        let state_v3 = *authenticated.state();
        let obligation = authenticate_live_series_obligation_for_state_v3(
            program_id,
            &accounts[15],
            &accounts[2],
            &state_v3,
        )?;
        (state_v3.base, Some(state_v3), authenticated.bump(), Some(obligation))
    } else {
        let state = authenticate_state(program_id, &accounts[2])?;
        let bump = accounts[2].data.borrow()[2];
        (state, None, bump, None)
    };
    require(
        state.policy_id.bytes() == policy_id && state.generation == payload.expected_generation,
        ClutchError::MismatchedState,
    )?;
    let (position_binding, position, replay, replay_binding) = authenticate_position_and_replay(
        program_id,
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &policy,
        &state,
        false,
    )?;
    require(
        replay.next_transition_ordinal() == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let dependency = authenticate_dependency(program_id, &accounts[5], state.facility_id)?;
    let schedule = authenticate_schedule(program_id, &accounts[6])?;
    let (runtime_policy, _runtime_states, runtime_binding) =
        authenticate_runtime_bundle_with_access(
            program_id,
            &dependency,
            &accounts[7],
            &accounts[8..15],
            None,
        )?;
    validate_runtime_dependency_join(
        program_id,
        &accounts[2],
        &policy,
        &state,
        &position_binding,
        &dependency,
        &schedule,
        runtime_policy,
        runtime_binding,
    )?;
    let prepared = prepare_sponsor_halt_dealer_v3(
        &policy,
        &position_binding,
        &state,
        id(accounts[2].key),
        &dependency,
        &schedule,
        &runtime_binding,
        id(accounts[0].key),
        &position,
        &replay,
        replay_binding,
    )
    .map_err(dealer_fault)?;
    let state_after_v3 = match state_v3 {
        Some(current) => Some(current.with_base(prepared.state_after).map_err(dealer_fault)?),
        None => None,
    };
    match state_after_v3 {
        Some(state_after) => write_dealer_body(
            &accounts[2],
            DEALER_STATE_V3_ACCOUNT_TAG,
            DEALER_STATE_V3_ACCOUNT_VERSION,
            state_bump,
            &state_after,
        )?,
        None => write_dealer_body(
            &accounts[2],
            DEALER_STATE_V2_ACCOUNT_TAG,
            DEALER_STATE_V2_ACCOUNT_VERSION,
            state_bump,
            &prepared.state_after,
        )?,
    };
    prepared
        .replay
        .replay_post()
        .encode_into(&mut accounts[4].data.borrow_mut())
        .map_err(dealer_fault)?;
    let observed_replay = DealerFacilityReplayV1::decode(&accounts[4].data.borrow())
        .map_err(dealer_fault)?;
    let state_matches = match state_after_v3 {
        Some(state_after) => {
            let observed_state = authenticate_dealer_state_v3(program_id, &accounts[2], true)?;
            let observed_obligation =
                authenticate_dealer_series_obligation_v1(program_id, &accounts[15], false)?;
            let obligation_matches = match obligation.as_ref() {
                Some(value) => observed_obligation.binding() == value,
                None => false,
            };
            observed_state.state() == &state_after && obligation_matches
        }
        None => authenticate_state(program_id, &accounts[2])? == prepared.state_after,
    };
    require(
        state_matches && observed_replay == prepared.replay.replay_post(),
        ClutchError::MismatchedState,
    )
}

/// Permissionlessly enter UnwindOnly after the exact queued-share quorum.
#[inline(never)]
fn enter_unwind(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    funded_unwind(
        program_id,
        accounts,
        sequence,
        DealerFacilityAction::EnterUnwind,
        payload_bytes,
    )
}

/// Permissionlessly enter UnwindOnly after the immutable close slot.
#[inline(never)]
fn timed_close(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    funded_unwind(
        program_id,
        accounts,
        sequence,
        DealerFacilityAction::TimedClose,
        payload_bytes,
    )
}

/// Shared exact Retirement-funded transition for the two permissionless
/// Trading→UnwindOnly causes. The facility Position and Realm Hoard remain
/// byte-for-byte balance-neutral and Product's `0xaf` survives unchanged.
#[inline(never)]
fn funded_unwind(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: DealerFacilityAction,
    payload_bytes: &[u8],
) -> Outcome<()> {
    let payload = DealerRuntimePayloadV1::decode(action, payload_bytes).map_err(dealer_fault)?;
    let admitted_count = match action {
        DealerFacilityAction::EnterUnwind => ENTER_UNWIND_ACCOUNT_COUNT,
        DealerFacilityAction::TimedClose => TIMED_CLOSE_ACCOUNT_COUNT,
        _ => return Err(ClutchError::UnsupportedInstruction.into()),
    };
    let expected_count = if payload.existing_series_admission {
        admitted_count
    } else {
        admitted_count
            .checked_sub(1)
            .ok_or(ClutchError::Arithmetic)?
    };
    require_count(accounts, expected_count)?;
    require(
        sequence == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    require_aliases(accounts, (0, 16))?;

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let (clock_index, rent_index, system_index, obligation_index) = match action {
        DealerFacilityAction::EnterUnwind => (None, 17usize, 18usize, 19usize),
        DealerFacilityAction::TimedClose => (Some(17usize), 18usize, 19usize, 20usize),
        _ => return Err(ClutchError::UnsupportedInstruction.into()),
    };
    let (state, state_v3, state_bump, obligation) = if payload.existing_series_admission {
        let authenticated = authenticate_dealer_state_v3(program_id, &accounts[2], true)?;
        let state_v3 = *authenticated.state();
        let obligation = authenticate_live_series_obligation_for_state_v3(
            program_id,
            &accounts[obligation_index],
            &accounts[2],
            &state_v3,
        )?;
        (state_v3.base, Some(state_v3), authenticated.bump(), Some(obligation))
    } else {
        let state = authenticate_state(program_id, &accounts[2])?;
        let bump = accounts[2].data.borrow()[2];
        (state, None, bump, None)
    };
    require(
        state.policy_id.bytes() == policy_id && state.generation == payload.expected_generation,
        ClutchError::MismatchedState,
    )?;
    let (position_binding, position, replay, replay_binding) =
        authenticate_position_and_replay(
            program_id,
            &accounts[2],
            &accounts[3],
            &accounts[4],
            &policy,
            &state,
            false,
        )?;
    require(
        replay.next_transition_ordinal() == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let dependency = authenticate_dependency(program_id, &accounts[5], state.facility_id)?;
    let schedule = authenticate_schedule(program_id, &accounts[6])?;
    let (runtime_policy, runtime_states, runtime_binding) = authenticate_runtime_bundle(
        program_id,
        &dependency,
        &accounts[7],
        &accounts[8..15],
        DealerLivenessCompartmentV1::Retirement.index(),
    )?;
    validate_runtime_dependency_join(
        program_id,
        &accounts[2],
        &policy,
        &state,
        &position_binding,
        &dependency,
        &schedule,
        runtime_policy,
        runtime_binding,
    )?;
    let retirement = runtime_states[DealerLivenessCompartmentV1::Retirement.index()];
    require(
        retirement.identity.payer.bytes() == accounts[16].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let current_slot = match clock_index {
        Some(index) => Some(read_clock_slot(&accounts[index])?),
        None => None,
    };
    let rent = read_rent(&accounts[rent_index])?;
    require_system_program(&accounts[system_index])?;
    require(
        accounts[6].lamports() >= rent.minimum_balance(DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES)?,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    require_creatable(&accounts[15])?;

    let receipt_principal = rent.minimum_balance(DEALER_ACTION_RECEIPT_ACCOUNT_BYTES)?;
    let runtime_action = match action {
        DealerFacilityAction::EnterUnwind => DealerRuntimeActionV1::EnterUnwind,
        DealerFacilityAction::TimedClose => DealerRuntimeActionV1::TimedClose,
        _ => return Err(ClutchError::UnsupportedInstruction.into()),
    };
    let action_index = DealerLivenessScheduleV1::action_index(runtime_action);
    let receipt = DealerActionReceiptV1 {
        policy_id: state.policy_id,
        facility_id: state.facility_id,
        dealer_state_account_id: id(accounts[2].key),
        liveness_schedule_id: schedule.schedule_id().map_err(dealer_fault)?.untyped(),
        runtime_policy_id: runtime_binding.runtime_policy_id(),
        runtime_account_id: runtime_binding.account_id(DealerLivenessCompartmentV1::Retirement),
        runtime_owner: runtime_binding.owner(DealerLivenessCompartmentV1::Retirement),
        quote_schedule_id: runtime_binding
            .quote_schedule_id(DealerLivenessCompartmentV1::Retirement),
        receipt_account_id: id(accounts[15].key),
        receipt_program_id: id(program_id),
        keeper: id(accounts[0].key),
        replay_account_id: id(accounts[4].key),
        action: runtime_action,
        compartment: DealerLivenessCompartmentV1::Retirement,
        runtime_generation: runtime_binding.generation(DealerLivenessCompartmentV1::Retirement),
        facility_generation: state.generation,
        call_ordinal: payload.liveness_call_ordinal,
        call_ceiling_lamports: schedule.reward_lamports[action_index],
        keeper_payment_lamports: payload.keeper_payment_lamports,
        expected_replay_ordinal: payload.expected_replay_ordinal,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: receipt_principal,
            donation_floor: accounts[15].lamports(),
        },
    };
    let receipt_slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
    let (receipt_address, receipt_bump) =
        seeds::dealer_action_receipt_pda(program_id, &receipt_slot.bytes());
    expect_pda(accounts[15].key, (receipt_address, receipt_bump), None)?;
    receipt
        .validate_against(&schedule, &runtime_binding)
        .map_err(dealer_fault)?;
    let authorization = receipt
        .authorization(&schedule, &runtime_binding, &retirement)
        .map_err(dealer_fault)?;
    let liveness_transition = plan_liveness_spend_absorbing_donation(
        program_id,
        &accounts[7],
        &accounts[13],
        retirement,
        receipt.runtime_transition_intent().map_err(dealer_fault)?,
        receipt
            .runtime_receipt_observation()
            .map_err(dealer_fault)?,
    )?;
    let prepared = match (runtime_action, current_slot) {
        (DealerRuntimeActionV1::EnterUnwind, None) => prepare_enter_unwind_by_queue_v3(
            &policy,
            &position_binding,
            &state,
            id(accounts[2].key),
            &dependency,
            &schedule,
            &runtime_binding,
            &authorization,
            &position,
            &replay,
            replay_binding,
        ),
        (DealerRuntimeActionV1::TimedClose, Some(current_slot)) => prepare_timed_close_dealer_v3(
            &policy,
            &position_binding,
            &state,
            id(accounts[2].key),
            &dependency,
            &schedule,
            &runtime_binding,
            &authorization,
            current_slot,
            &position,
            &replay,
            replay_binding,
        ),
        _ => return Err(ClutchError::MismatchedState.into()),
    }
    .map_err(dealer_fault)?;
    let state_after_v3 = match state_v3 {
        Some(current) => Some(current.with_base(prepared.state_after).map_err(dealer_fault)?),
        None => None,
    };

    let (observed_receipt_principal, observed_receipt_donation) = create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[15],
        &accounts[system_index],
        &rent,
        DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_ACTION_RECEIPT,
            &receipt_slot.bytes(),
            &[receipt_bump],
        ],
    )?;
    require(
        observed_receipt_principal == receipt.rent.refundable_principal
            && observed_receipt_donation == receipt.rent.donation_floor,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    apply_liveness_transition(
        &accounts[13],
        &accounts[0],
        &accounts[16],
        &liveness_transition,
    )?;
    write_dealer_body(
        &accounts[15],
        DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
        DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
        receipt_bump,
        &receipt,
    )?;
    match state_after_v3 {
        Some(state_after) => write_dealer_body(
            &accounts[2],
            DEALER_STATE_V3_ACCOUNT_TAG,
            DEALER_STATE_V3_ACCOUNT_VERSION,
            state_bump,
            &state_after,
        )?,
        None => write_dealer_body(
            &accounts[2],
            DEALER_STATE_V2_ACCOUNT_TAG,
            DEALER_STATE_V2_ACCOUNT_VERSION,
            state_bump,
            &prepared.state_after,
        )?,
    };
    prepared
        .replay
        .replay_post()
        .encode_into(&mut accounts[4].data.borrow_mut())
        .map_err(dealer_fault)?;

    let (_, observed_receipt) = authenticate_action_receipt(program_id, &accounts[15])?;
    let (_, observed_position, observed_replay, _) = authenticate_position_and_replay(
        program_id,
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &policy,
        &prepared.state_after,
        false,
    )?;
    let retirement_data = accounts[13]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let state_matches = match state_after_v3 {
        Some(state_after) => {
            let observed_state = authenticate_dealer_state_v3(program_id, &accounts[2], true)?;
            let observed_obligation = authenticate_dealer_series_obligation_v1(
                program_id,
                &accounts[obligation_index],
                false,
            )?;
            let obligation_matches = match obligation.as_ref() {
                Some(value) => observed_obligation.binding() == value,
                None => false,
            };
            observed_state.state() == &state_after && obligation_matches
        }
        None => authenticate_state(program_id, &accounts[2])? == prepared.state_after,
    };
    require(
        state_matches
            && observed_receipt == receipt
            && observed_position == position
            && observed_replay == prepared.replay.replay_post()
            && accounts[13].lamports() == liveness_transition.account_balance_after
            && retirement_data.as_ref() == liveness_transition.post_account_data.as_slice(),
        ClutchError::MismatchedState,
    )
}

/// Resolve the complete facility inventory through Fractional's private
/// divide-once vector owner, then advance Dealer State and its purpose Replay
/// exactly once. The one-shot `0xbc/v1` rent owner is consumed inside the
/// Fractional call; no Dealer State/Replay/liveness postwrite precedes it.
#[inline(never)]
fn resolve_facility_vector(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    require_count(accounts, RESOLVE_FACILITY_VECTOR_ACCOUNT_COUNT)?;
    let payload = DealerRuntimePayloadV1::decode(DealerFacilityAction::Resolve, payload_bytes)
        .map_err(dealer_fault)?;
    require(
        sequence == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let authenticated_state = authenticate_dealer_state_v3(program_id, &accounts[2], true)?;
    let state_v3 = *authenticated_state.state();
    let state = state_v3.base;
    require(
        state.policy_id.bytes() == policy_id
            && state.generation == payload.expected_generation
            && matches!(state.phase, DealerPhaseV2::Trading | DealerPhaseV2::UnwindOnly),
        ClutchError::MismatchedState,
    )?;
    let authenticated_obligation =
        authenticate_dealer_series_obligation_v2(program_id, &accounts[25], false)?;
    let obligation = *authenticated_obligation.binding();
    let obligation_id = obligation.binding_id().map_err(dealer_fault)?;
    require(
        obligation.phase == DealerSeriesObligationPhaseV1::Live
            && state_v3.series_obligation_children == 1
            && state_v3.series_obligation_binding_account_id == id(accounts[25].key)
            && state_v3.series_obligation_binding_id == obligation_id,
        ClutchError::MismatchedState,
    )?;

    let (position_binding, position_before, replay, replay_binding) =
        authenticate_position_and_replay(
            program_id,
            &accounts[2],
            &accounts[3],
            &accounts[4],
            &policy,
            &state,
            true,
        )?;
    require(
        replay.next_transition_ordinal() == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let dependency = authenticate_dependency(program_id, &accounts[5], state.facility_id)?;
    let schedule = authenticate_schedule(program_id, &accounts[6])?;
    let (runtime_policy, runtime_states, runtime_binding) = authenticate_runtime_bundle(
        program_id,
        &dependency,
        &accounts[7],
        &accounts[8..15],
        DealerLivenessCompartmentV1::Resolution.index(),
    )?;
    validate_runtime_dependency_join(
        program_id,
        &accounts[2],
        &policy,
        &state,
        &position_binding,
        &dependency,
        &schedule,
        runtime_policy,
        runtime_binding,
    )?;
    let resolution_liveness =
        runtime_states[DealerLivenessCompartmentV1::Resolution.index()];
    require(
        resolution_liveness.identity.payer.bytes() == accounts[16].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;

    let current_slot = read_clock_slot(&accounts[21])?;
    let rent = read_rent(&accounts[22])?;
    require_system_program(&accounts[23])?;
    require(
        accounts[6].lamports() >= rent.minimum_balance(DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES)?,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    require_creatable(&accounts[15])?;
    require_creatable(&accounts[17])?;

    let receipt_principal = rent.minimum_balance(DEALER_ACTION_RECEIPT_ACCOUNT_BYTES)?;
    let action_index = DealerLivenessScheduleV1::action_index(DealerRuntimeActionV1::Resolve);
    let receipt = DealerActionReceiptV1 {
        policy_id: state.policy_id,
        facility_id: state.facility_id,
        dealer_state_account_id: id(accounts[2].key),
        liveness_schedule_id: schedule.schedule_id().map_err(dealer_fault)?.untyped(),
        runtime_policy_id: runtime_binding.runtime_policy_id(),
        runtime_account_id: runtime_binding.account_id(DealerLivenessCompartmentV1::Resolution),
        runtime_owner: runtime_binding.owner(DealerLivenessCompartmentV1::Resolution),
        quote_schedule_id: runtime_binding
            .quote_schedule_id(DealerLivenessCompartmentV1::Resolution),
        receipt_account_id: id(accounts[15].key),
        receipt_program_id: id(program_id),
        keeper: id(accounts[0].key),
        replay_account_id: id(accounts[4].key),
        action: DealerRuntimeActionV1::Resolve,
        compartment: DealerLivenessCompartmentV1::Resolution,
        runtime_generation: runtime_binding.generation(DealerLivenessCompartmentV1::Resolution),
        facility_generation: state.generation,
        call_ordinal: payload.liveness_call_ordinal,
        call_ceiling_lamports: schedule.reward_lamports[action_index],
        keeper_payment_lamports: payload.keeper_payment_lamports,
        expected_replay_ordinal: payload.expected_replay_ordinal,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: receipt_principal,
            donation_floor: accounts[15].lamports(),
        },
    };
    let receipt_slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
    let (receipt_address, receipt_bump) =
        seeds::dealer_action_receipt_pda(program_id, &receipt_slot.bytes());
    expect_pda(accounts[15].key, (receipt_address, receipt_bump), None)?;
    receipt
        .validate_against(&schedule, &runtime_binding)
        .map_err(dealer_fault)?;
    let authorization = receipt
        .authorization(&schedule, &runtime_binding, &resolution_liveness)
        .map_err(dealer_fault)?;
    let liveness_transition = plan_liveness_spend_absorbing_donation(
        program_id,
        &accounts[7],
        &accounts[12],
        resolution_liveness,
        receipt.runtime_transition_intent().map_err(dealer_fault)?,
        receipt
            .runtime_receipt_observation()
            .map_err(dealer_fault)?,
    )?;

    let value_authority = authenticate_general_market_value_authority_v2(
        program_id,
        &accounts[27],
        &accounts[28],
        &accounts[29],
        &accounts[30],
        &accounts[31],
        &accounts[32],
        &accounts[33],
        &accounts[34],
        &accounts[35],
        &accounts[36],
        true,
        true,
    )?;
    let product_resolution = authenticate_current_product_resolution_v2(
        program_id,
        &accounts[24],
        &accounts[26],
        &obligation,
        &accounts[2],
        &state_v3,
        &policy,
        value_authority,
    )?;
    require(
        product_resolution.root_account_id == id(accounts[24].key)
            && product_resolution.link_account_id == id(accounts[26].key)
            && product_resolution.link_semantic_id != Id::ZERO,
        ClutchError::MismatchedState,
    )?;

    let (_funding_bump, funding) =
        authenticate_future_credit_funding(program_id, &accounts[18], true)?;
    let bound = value_authority.liabilities.bound;
    let realm = bound.realm_bound().realm();
    let release_id = bound
        .release()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        funding.policy_id == state.policy_id
            && funding.facility_id == state.facility_id
            && funding.market_instance_v2_id == policy.market_instance_v2_id
            && funding.realm_id.bytes() == realm.realm.bytes()
            && funding.collateral_policy_id.bytes() == bound.policy_id().bytes()
            && funding.collateral_release_id.bytes() == release_id.bytes()
            && funding.dealer_state_account_id == id(accounts[2].key)
            && funding.facility_position_account_id == id(accounts[3].key)
            && funding.facility_position_binding_id == state.facility_position_binding_id
            && funding.dealer_replay_account_id == id(accounts[4].key)
            && funding.refund_owner == id(accounts[19].key)
            && funding.neutral_sink == id(accounts[20].key)
            && funding.neutral_sink == policy.neutral_sink
            && funding.founding_generation <= state.generation,
        ClutchError::MismatchedState,
    )?;
    let live_credit_rent_lamports = rent.minimum_balance(FRACTIONAL_CREDIT_ACCOUNT_BYTES)?;
    let tombstone_rent_lamports =
        rent.minimum_balance(FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_BYTES)?;
    require(
        live_credit_rent_lamports
            == funding
                .credit_principal_lamports()
                .map_err(dealer_fault)?
            && tombstone_rent_lamports == funding.credit_tombstone_principal_lamports,
        ClutchError::DealerPolicyRentMismatch,
    )?;

    let state_pre_id = state_v3.state_id().map_err(dealer_fault)?;
    let replay_pre_id = replay.replay_id().map_err(dealer_fault)?;
    let funding_receipt_id = funding.funding_receipt_id().map_err(dealer_fault)?;
    let prestate = bind_dealer_facility_vector_prestate_v1(
        retirement_id(state.facility_id)?,
        retirement_id(id(accounts[2].key))?,
        retirement_id(state_pre_id)?,
        retirement_id(id(accounts[3].key))?,
        position_before.projection.position(),
        retirement_id(position_before.semantic_id)?,
        retirement_id(state.facility_position_binding_id)?,
        retirement_id(id(accounts[4].key))?,
        retirement_id(replay_pre_id)?,
        replay.next_transition_ordinal(),
        retirement_id(id(accounts[25].key))?,
        retirement_id(obligation_id)?,
        retirement_id(product_resolution.root_account_id)?,
        retirement_id(product_resolution.root_semantic_id)?,
        retirement_id(product_resolution.authentication_id)?,
        retirement_id(id(accounts[40].key))?,
        retirement_id(funding_receipt_id)?,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let request = DealerFacilityVectorRequestV1 {
        expected_ledger_sequence: payload.expected_fractional_ledger_sequence,
        expected_credit_sequence: payload.expected_fractional_credit_sequence,
        expected_position_generation: state.generation,
        expected_replay_ordinal: payload.expected_replay_ordinal,
        outcome_count: payload.resolution_outcome_count,
        quantities: payload.resolution_quantities,
    };
    request
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let vector = apply_dealer_facility_vector_transition_v1(
        program_id,
        DealerFacilityVectorAccountsV1 {
            realm: &accounts[27],
            profile: &accounts[28],
            collateral_policy: &accounts[29],
            collateral_token_program: &accounts[30],
            collateral_token_programdata: &accounts[31],
            market_binding: &accounts[32],
            market_runtime: &accounts[33],
            market_instance: &accounts[34],
            hoard: &accounts[35],
            claim_ledger: &accounts[36],
            resolution: &accounts[37],
            fractional_policy: &accounts[38],
            fractional_ledger: &accounts[39],
            facility_position: &accounts[3],
            facility_credit: &accounts[40],
            system_program: &accounts[23],
        },
        request,
        AuthenticatedDealerFacilityVectorAuthoritySbfV1 {
            prestate,
            funding,
            funding_account: &accounts[18],
            refund_owner: &accounts[19],
            neutral_sink: &accounts[20],
            credit_account: &accounts[40],
            system_program: &accounts[23],
            current_generation: state.generation,
            live_credit_rent_lamports,
            tombstone_rent_lamports,
        },
    )?;
    require(
        vector.facility_account().bytes() == accounts[3].key.to_bytes()
            && vector.facility_pre_semantic_id().bytes() == position_before.semantic_id.bytes()
            && vector.facility_post_generation()
                == state.generation.checked_add(1).ok_or(ClutchError::Arithmetic)?
            && vector.resolution_semantic_id().bytes()
                == product_resolution.resolution_semantic_id.bytes()
            && vector.resolution_data_id().bytes()
                == product_resolution.resolution_data_id.bytes(),
        ClutchError::MismatchedState,
    )?;

    let position_after_body = PositionAccountV3::decode(
        &accounts[3]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_after_projection = project_dealer_position_v3(
        position_after_body,
        AdapterPositionMarketBindingV3 {
            market_instance_id: position_after_body.market_instance_id(),
            outcome_count: position_after_body.outcome_count(),
            realm_id: position_after_body.realm_id(),
            collateral_policy_id: position_after_body.collateral_policy_id(),
            collateral_release_id: position_after_body.collateral_release_id(),
        },
        AdapterPositionPurposeBindingV3 {
            owner: retirement_id(state.facility_id)?,
            controller: retirement_id(id(accounts[2].key))?,
            purpose_binding_id: retirement_id(state.facility_position_binding_id)?,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_after = DealerPositionObservationV3 {
        account_id: id(accounts[3].key),
        semantic_id: Id::from_bytes(vector.facility_post_semantic_id().bytes()),
        projection: position_after_projection,
    };
    position_after
        .validate_against(&position_binding, state.facility_position_binding_id, &policy)
        .map_err(dealer_fault)?;
    require(
        position_after_body
            .semantic_id(&RuntimeSha256)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes()
            == vector.facility_post_semantic_id().bytes(),
        ClutchError::MismatchedState,
    )?;

    let claim_work_principal = rent.minimum_balance(DEALER_CLAIM_WORK_ACCOUNT_BYTES)?;
    let (claim_work_address, claim_work_bump) =
        seeds::dealer_claim_work_pda(program_id, &state.facility_id.bytes());
    expect_pda(accounts[17].key, (claim_work_address, claim_work_bump), None)?;
    let claim_work = DealerClaimWorkV1 {
        policy_id: state.policy_id,
        facility_id: state.facility_id,
        dealer_state_account_id: id(accounts[2].key),
        facility_position_binding_id: state.facility_position_binding_id,
        claim_work_account_id: id(accounts[17].key),
        market_instance_v2_id: policy.market_instance_v2_id,
        terminal_settlement_id: Id::from_bytes(vector.resolution_semantic_id().bytes()),
        payout_id: Id::from_bytes(vector.vector_transition_id().bytes()),
        funded_dependencies_id: state.funded_dependencies_id,
        runtime_liveness_policy_id: dependency.bindings.runtime_liveness_policy_id,
        runtime_liveness_binding_digest: dependency.bindings.runtime_liveness_binding_digest,
        dealer_liveness_schedule_id: schedule.schedule_id().map_err(dealer_fault)?.untyped(),
        resolve_receipt_account_id: id(accounts[15].key),
        resolve_receipt_semantic_id: authorization.receipt_semantic_id,
        resolve_receipt_program_id: id(program_id),
        rounding_policy: DealerTerminalRoundingPolicyV1::OwnerPrefixFloorV1,
        counted_generation: vector.facility_post_generation(),
        original_page_count: state.children.lp_pages,
        next_allocation_page_ordinal: 0,
        original_total_shares: state.total_shares,
        terminal_cash_atoms: position_after_body.cash_atoms(),
        allocated_share_prefix: 0,
        allocated_cash_atoms: 0,
        closed_pages: [0; DEALER_PAGE_BITMAP_BYTES_V1],
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: claim_work_principal,
            donation_floor: accounts[17].lamports(),
        },
    };
    claim_work.validate().map_err(dealer_fault)?;
    let state_after_base = begin_terminal_resolution_v1(
        &policy,
        &position_binding,
        &state,
        id(accounts[2].key),
        &claim_work,
        &schedule,
        &runtime_binding,
        &authorization,
        &position_before,
        &position_after,
        current_slot,
    )
    .map_err(dealer_fault)?;
    let state_after = state_v3.with_base(state_after_base).map_err(dealer_fault)?;
    let replay_plan = replay
        .prepare_transition(
            replay_binding,
            DealerTransitionIntentV1 {
                replay_account_id: id(accounts[4].key),
                replay_pre_id,
                state_pre_content_id: state_pre_id,
                state_post_content_id: state_after.state_id().map_err(dealer_fault)?,
                position_pre_semantic_id: position_before.semantic_id,
                position_post_semantic_id: position_after.semantic_id,
                liveness_receipt_semantic_id: authorization.receipt_semantic_id,
                fee_evidence_id: Id::ZERO,
                asset_transfer_bundle_id: Id::from_bytes(vector.execution_receipt_id().bytes()),
                position_generation_before: state.generation,
                position_generation_after: vector.facility_post_generation(),
                expected_ordinal: payload.expected_replay_ordinal,
                action: DealerRuntimeActionV1::Resolve,
                liveness_mode: DealerTransitionLivenessModeV1::ExternalReceipt,
            },
        )
        .map_err(dealer_fault)?;

    let (observed_receipt_principal, observed_receipt_donation) = create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[15],
        &accounts[23],
        &rent,
        DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_ACTION_RECEIPT,
            &receipt_slot.bytes(),
            &[receipt_bump],
        ],
    )?;
    let (observed_work_principal, observed_work_donation) = create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[17],
        &accounts[23],
        &rent,
        DEALER_CLAIM_WORK_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_CLAIM_WORK,
            &state.facility_id.bytes(),
            &[claim_work_bump],
        ],
    )?;
    require(
        observed_receipt_principal == receipt.rent.refundable_principal
            && observed_receipt_donation == receipt.rent.donation_floor
            && observed_work_principal == claim_work.rent.refundable_principal
            && observed_work_donation == claim_work.rent.donation_floor,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    apply_liveness_transition(
        &accounts[12],
        &accounts[0],
        &accounts[16],
        &liveness_transition,
    )?;
    write_dealer_body(
        &accounts[15],
        DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
        DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
        receipt_bump,
        &receipt,
    )?;
    write_dealer_body(
        &accounts[17],
        DEALER_CLAIM_WORK_ACCOUNT_TAG,
        DEALER_CLAIM_WORK_ACCOUNT_VERSION,
        claim_work_bump,
        &claim_work,
    )?;
    write_dealer_body(
        &accounts[2],
        DEALER_STATE_V3_ACCOUNT_TAG,
        DEALER_STATE_V3_ACCOUNT_VERSION,
        authenticated_state.bump(),
        &state_after,
    )?;
    replay_plan
        .replay_post()
        .encode_into(&mut accounts[4].data.borrow_mut())
        .map_err(dealer_fault)?;

    let (_, observed_receipt) = authenticate_action_receipt(program_id, &accounts[15])?;
    let observed_work = authenticate_claim_work_with_access(program_id, &accounts[17], true)?;
    let observed_state = authenticate_dealer_state_v3(program_id, &accounts[2], true)?;
    let observed_obligation =
        authenticate_dealer_series_obligation_v2(program_id, &accounts[25], false)?;
    let observed_replay = DealerFacilityReplayV1::decode(&accounts[4].data.borrow())
        .map_err(dealer_fault)?;
    let resolution_liveness_data = accounts[12]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        observed_receipt == receipt
            && observed_work == claim_work
            && observed_state.state() == &state_after
            && observed_obligation.binding() == &obligation
            && observed_replay == replay_plan.replay_post()
            && accounts[12].lamports() == liveness_transition.account_balance_after
            && resolution_liveness_data.as_ref()
                == liveness_transition.post_account_data.as_slice(),
        ClutchError::MismatchedState,
    )
}

/// Deliver one sealed terminal LP allocation through the canonical facility
/// and General Position/Replay owners. Claim moves only internal cash
/// liability: Hoard custody and both aggregate market-liability accounts are
/// authenticated read-only under the current Realm-selected deployment.
#[inline(never)]
fn claim_terminal_allocation(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    require_count(accounts, CLAIM_TERMINAL_ACCOUNT_COUNT)?;
    let payload = DealerRuntimePayloadV1::decode(DealerFacilityAction::Claim, payload_bytes)
        .map_err(dealer_fault)?;
    require(
        sequence == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    require_aliases(accounts, (0, 21))?;

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let authenticated_state = authenticate_dealer_state_v3(program_id, &accounts[2], true)?;
    let state_v3 = *authenticated_state.state();
    let state = state_v3.base;
    require(
        state.policy_id.bytes() == policy_id
            && state.generation == payload.expected_generation
            && state.phase == DealerPhaseV2::Resolved,
        ClutchError::MismatchedState,
    )?;
    let obligation = authenticate_live_series_obligation_for_state_v3(
        program_id,
        &accounts[34],
        &accounts[2],
        &state_v3,
    )?;
    let (position_binding, facility_position, facility_replay, replay_binding) =
        authenticate_position_and_replay(
            program_id,
            &accounts[2],
            &accounts[3],
            &accounts[4],
            &policy,
            &state,
            true,
        )?;
    require(
        facility_replay.next_transition_ordinal() == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let page = authenticate_lp_page_with_access(program_id, &accounts[7], false)?;
    require(
        page.page_ordinal == payload.page_ordinal,
        ClutchError::MismatchedState,
    )?;
    let (allocation_bump, allocation) =
        authenticate_terminal_allocation(program_id, &accounts[8])?;
    let work = authenticate_claim_work(program_id, &accounts[9])?;
    let dependency = authenticate_dependency(program_id, &accounts[10], state.facility_id)?;
    let schedule = authenticate_schedule(program_id, &accounts[11])?;
    let (runtime_policy, runtime_states, runtime_binding) = authenticate_runtime_bundle(
        program_id,
        &dependency,
        &accounts[12],
        &accounts[13..20],
        DealerLivenessCompartmentV1::Settlement.index(),
    )?;
    validate_runtime_dependency_join(
        program_id,
        &accounts[2],
        &policy,
        &state,
        &position_binding,
        &dependency,
        &schedule,
        runtime_policy,
        runtime_binding,
    )?;
    let settlement = runtime_states[DealerLivenessCompartmentV1::Settlement.index()];
    require(
        settlement.identity.payer.bytes() == accounts[21].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let rent = read_rent(&accounts[22])?;
    require_system_program(&accounts[23])?;
    require(
        accounts[11].lamports() >= rent.minimum_balance(DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES)?,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    require_creatable(&accounts[20])?;

    let (collateral_value, market) = authenticate_dealer_collateral_value_v2(
        program_id,
        &policy,
        Some(&position_binding),
        DealerCollateralAuthorityAccountsV2 {
            realm: &accounts[24],
            profile: &accounts[25],
            policy: &accounts[26],
            token_program: &accounts[27],
            token_programdata: &accounts[28],
            market_binding: &accounts[29],
            market_runtime: &accounts[30],
            market_instance: &accounts[31],
            hoard: &accounts[32],
            claim_ledger: &accounts[33],
        },
    )?;
    let entry = page.entries[usize::from(payload.entry_index)];
    let lp_authority = authenticate_general_position_replay_v2(
        program_id,
        collateral_value.liabilities.bound,
        &accounts[29],
        &accounts[30],
        &accounts[5],
        &accounts[6],
        entry.owner.bytes(),
        payload.expected_general_replay_sequence,
    )?;
    let facility_endpoint = DealerTransferPositionV3::Facility {
        account_id: id(accounts[3].key),
        position: facility_position.projection,
    };
    let lp_endpoint = DealerTransferPositionV3::General {
        account_id: id(accounts[5].key),
        position: lp_authority.projection,
    };
    let preview = prepare_dealer_terminal_claim_v2(
        &policy,
        &state,
        id(accounts[2].key),
        &work,
        id(accounts[7].key),
        &page,
        &allocation,
        payload.entry_index,
        market,
        facility_endpoint,
        lp_endpoint,
    )
    .map_err(dealer_fault)?;
    let transfer_bundle_id = preview.transfer_bundle().bundle_id().map_err(dealer_fault)?;

    let receipt_principal = rent.minimum_balance(DEALER_ACTION_RECEIPT_ACCOUNT_BYTES)?;
    require(
        payload.keeper_payment_lamports >= receipt_principal,
        ClutchError::MismatchedState,
    )?;
    let action_index = DealerLivenessScheduleV1::action_index(DealerRuntimeActionV1::Claim);
    let receipt = DealerActionReceiptV1 {
        policy_id: state.policy_id,
        facility_id: state.facility_id,
        dealer_state_account_id: id(accounts[2].key),
        liveness_schedule_id: schedule.schedule_id().map_err(dealer_fault)?.untyped(),
        runtime_policy_id: runtime_binding.runtime_policy_id(),
        runtime_account_id: runtime_binding.account_id(DealerLivenessCompartmentV1::Settlement),
        runtime_owner: runtime_binding.owner(DealerLivenessCompartmentV1::Settlement),
        quote_schedule_id: runtime_binding
            .quote_schedule_id(DealerLivenessCompartmentV1::Settlement),
        receipt_account_id: id(accounts[20].key),
        receipt_program_id: id(program_id),
        keeper: id(accounts[0].key),
        replay_account_id: id(accounts[4].key),
        action: DealerRuntimeActionV1::Claim,
        compartment: DealerLivenessCompartmentV1::Settlement,
        runtime_generation: runtime_binding.generation(DealerLivenessCompartmentV1::Settlement),
        facility_generation: state.generation,
        call_ordinal: payload.liveness_call_ordinal,
        call_ceiling_lamports: schedule.reward_lamports[action_index],
        keeper_payment_lamports: payload.keeper_payment_lamports,
        expected_replay_ordinal: payload.expected_replay_ordinal,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: receipt_principal,
            donation_floor: accounts[20].lamports(),
        },
    };
    let receipt_slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
    let (receipt_address, receipt_bump) =
        seeds::dealer_action_receipt_pda(program_id, &receipt_slot.bytes());
    expect_pda(accounts[20].key, (receipt_address, receipt_bump), None)?;
    receipt
        .validate_against(&schedule, &runtime_binding)
        .map_err(dealer_fault)?;
    let authorization = receipt
        .authorization(&schedule, &runtime_binding, &settlement)
        .map_err(dealer_fault)?;
    let liveness_transition = plan_liveness_spend_absorbing_donation(
        program_id,
        &accounts[12],
        &accounts[16],
        settlement,
        receipt.runtime_transition_intent().map_err(dealer_fault)?,
        receipt
            .runtime_receipt_observation()
            .map_err(dealer_fault)?,
    )?;
    let general_replay = prepare_dealer_general_replay_v2(
        lp_authority,
        preview.lp_position_post(),
        GeneralReplayTransitionKindV1::DealerLpClaim,
        transfer_bundle_id,
        market.collateral_value_receipt_id,
        authorization.receipt_semantic_id,
    )?;
    let prepared = prepare_dealer_terminal_claim_replay_v2(
        &policy,
        &state,
        id(accounts[2].key),
        &work,
        id(accounts[7].key),
        &page,
        &allocation,
        payload.entry_index,
        &schedule,
        &runtime_binding,
        &authorization,
        market,
        facility_endpoint,
        lp_endpoint,
        &general_replay,
        &facility_replay,
        replay_binding,
    )
    .map_err(dealer_fault)?;
    let claim = prepared.claim();
    require(
        claim.transfer_bundle() == preview.transfer_bundle()
            && claim.lp_owner() == entry.owner
            && prepared.general_transfer().transfer().bundle() == claim.transfer_bundle(),
        ClutchError::MismatchedState,
    )?;
    let state_after = state_v3
        .with_base(claim.state_after())
        .map_err(dealer_fault)?;

    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[20],
        &accounts[23],
        &rent,
        DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_ACTION_RECEIPT,
            &receipt_slot.bytes(),
            &[receipt_bump],
        ],
    )?;
    apply_liveness_transition(
        &accounts[16],
        &accounts[0],
        &accounts[21],
        &liveness_transition,
    )?;
    write_dealer_body(
        &accounts[20],
        DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
        DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
        receipt_bump,
        &receipt,
    )?;
    accounts[3]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &claim
                .facility_position_post()
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    accounts[5]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &claim
                .lp_position_post()
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    write_dealer_body(
        &accounts[8],
        DEALER_TERMINAL_ALLOCATION_ACCOUNT_TAG,
        DEALER_TERMINAL_ALLOCATION_ACCOUNT_VERSION,
        allocation_bump,
        &claim.allocation_after(),
    )?;
    write_dealer_body(
        &accounts[2],
        DEALER_STATE_V3_ACCOUNT_TAG,
        DEALER_STATE_V3_ACCOUNT_VERSION,
        authenticated_state.bump(),
        &state_after,
    )?;
    prepared
        .replay()
        .replay_post()
        .encode_into(&mut accounts[4].data.borrow_mut())
        .map_err(dealer_fault)?;
    let accepted_transfer = accept_dealer_asset_transfer_postwrite_v2(
        program_id,
        claim.transfer_bundle(),
        &accounts[3],
        &accounts[5],
    )?;
    let accepted_general_replay =
        write_and_accept_general_replay_v1(program_id, &accounts[6], &general_replay)?;

    let observed_state = authenticate_dealer_state_v3(program_id, &accounts[2], true)?;
    let observed_obligation =
        authenticate_dealer_series_obligation_v1(program_id, &accounts[34], false)?;
    let (_, observed_allocation) = authenticate_terminal_allocation(program_id, &accounts[8])?;
    let (_, observed_receipt) = authenticate_action_receipt(program_id, &accounts[20])?;
    let (_, observed_facility_position, observed_facility_replay, _) =
        authenticate_position_and_replay(
            program_id,
            &accounts[2],
            &accounts[3],
            &accounts[4],
            &policy,
            &claim.state_after(),
            true,
        )?;
    let settlement_data = accounts[16]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        observed_state.state() == &state_after
            && observed_obligation.binding() == &obligation
            && observed_allocation == claim.allocation_after()
            && observed_receipt == receipt
            && observed_facility_position.projection.position()
                == claim.facility_position_post()
            && observed_facility_replay == prepared.replay().replay_post()
            && accepted_transfer == transfer_bundle_id
            && accepted_general_replay
                == Id::from_bytes(general_replay.replay_poststate_semantic_id().bytes())
            && accounts[16].lamports() == liveness_transition.account_balance_after
            && settlement_data.as_ref() == liveness_transition.post_account_data.as_slice(),
        ClutchError::MismatchedState,
    )
}

/// Queue an immutable LP owner's exit without mutating its sealed page.
/// Before first lease admission this consumes the exact StateV2/Position/Replay
/// boundary; afterward it additionally preserves StateV3's live Product
/// obligation. New ticket rent is always supplied by the owner, including on
/// the optional Retirement-funded maintenance path. Hoard custody is absent.
#[inline(never)]
fn queue_exit(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    let payload = DealerRuntimePayloadV1::decode(DealerFacilityAction::QueueExit, payload_bytes)
        .map_err(dealer_fault)?;
    let admitted_count = if payload.external_liveness {
        QUEUE_EXIT_EXTERNAL_ACCOUNT_COUNT
    } else if payload.existing_ticket {
        QUEUE_EXIT_CALLER_EXISTING_ACCOUNT_COUNT
    } else {
        QUEUE_EXIT_CALLER_NEW_ACCOUNT_COUNT
    };
    let expected_count = if payload.existing_series_admission {
        admitted_count
    } else {
        admitted_count
            .checked_sub(1)
            .ok_or(ClutchError::Arithmetic)?
    };
    require_count(accounts, expected_count)?;
    require(
        sequence == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    require_signer(&accounts[0])?;
    if !payload.existing_ticket || payload.external_liveness {
        require(accounts[0].is_writable, ClutchError::NotWritable)?;
    }
    if payload.external_liveness {
        require_aliases(accounts, (0, 18))?;
    }

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let obligation_index = if payload.external_liveness { 21 } else { 7 };
    let (state, state_v3, state_bump, obligation) = if payload.existing_series_admission {
        let authenticated = authenticate_dealer_state_v3(program_id, &accounts[2], true)?;
        let state_v3 = *authenticated.state();
        let obligation = authenticate_live_series_obligation_for_state_v3(
            program_id,
            &accounts[obligation_index],
            &accounts[2],
            &state_v3,
        )?;
        (state_v3.base, Some(state_v3), authenticated.bump(), Some(obligation))
    } else {
        let state = authenticate_state(program_id, &accounts[2])?;
        let bump = accounts[2].data.borrow()[2];
        (state, None, bump, None)
    };
    require(
        state.policy_id.bytes() == policy_id && state.generation == payload.expected_generation,
        ClutchError::MismatchedState,
    )?;
    let (position_binding, position, replay, replay_binding) =
        authenticate_position_and_replay(
            program_id,
            &accounts[2],
            &accounts[3],
            &accounts[4],
            &policy,
            &state,
            false,
        )?;
    require(
        replay.next_transition_ordinal() == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let page = authenticate_lp_page_with_access(program_id, &accounts[5], false)?;
    require(
        page.page_ordinal == payload.page_ordinal
            && usize::from(payload.entry_index) < usize::from(page.entry_count)
            && page.entries[usize::from(payload.entry_index)].owner == id(accounts[0].key),
        ClutchError::MismatchedState,
    )?;
    let (expected_ticket, expected_ticket_bump) = seeds::dealer_exit_ticket_pda(
        program_id,
        &state.facility_id.bytes(),
        &accounts[0].key.to_bytes(),
    );
    expect_pda(accounts[6].key, (expected_ticket, expected_ticket_bump), None)?;

    let rent = if payload.external_liveness {
        Some(read_rent(&accounts[19])?)
    } else if !payload.existing_ticket {
        let rent_index = if payload.existing_series_admission {
            8usize
        } else {
            7usize
        };
        Some(read_rent(&accounts[rent_index])?)
    } else {
        None
    };
    let ticket_principal = if payload.existing_ticket {
        0
    } else {
        let rent = rent.as_ref().ok_or(ClutchError::MismatchedState)?;
        require_creatable(&accounts[6])?;
        let principal = rent.minimum_balance(DEALER_EXIT_TICKET_ACCOUNT_BYTES)?;
        require(principal != 0, ClutchError::DealerPolicyRentMismatch)?;
        principal
    };
    let ticket_donation = if payload.existing_ticket {
        0
    } else {
        accounts[6].lamports()
    };
    let existing_ticket = if payload.existing_ticket {
        let (stored_bump, ticket) = authenticate_exit_ticket(program_id, &accounts[6])?;
        require(
            stored_bump == expected_ticket_bump
                && ticket.owner == id(accounts[0].key)
                && ticket.page_ordinal == payload.page_ordinal
                && ticket.entry_index == payload.entry_index,
            ClutchError::MismatchedState,
        )?;
        Some(ticket)
    } else {
        None
    };

    let mut liveness_transition = None;
    let mut receipt_postwrite = None;
    let liveness = if payload.external_liveness {
        require_system_program(&accounts[20])?;
        let dependency = authenticate_dependency(program_id, &accounts[7], state.facility_id)?;
        let schedule = authenticate_schedule(program_id, &accounts[8])?;
        let (runtime_policy, runtime_states, runtime_binding) = authenticate_runtime_bundle(
            program_id,
            &dependency,
            &accounts[9],
            &accounts[10..17],
            DealerLivenessCompartmentV1::Retirement.index(),
        )?;
        validate_runtime_dependency_join(
            program_id,
            &accounts[2],
            &policy,
            &state,
            &position_binding,
            &dependency,
            &schedule,
            runtime_policy,
            runtime_binding,
        )?;
        let retirement = runtime_states[DealerLivenessCompartmentV1::Retirement.index()];
        require(
            retirement.identity.payer.bytes() == accounts[18].key.to_bytes()
                && accounts[8].lamports()
                    >= rent
                        .as_ref()
                        .ok_or(ClutchError::MismatchedState)?
                        .minimum_balance(DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES)?,
            ClutchError::MismatchedState,
        )?;
        require_creatable(&accounts[17])?;
        let receipt_principal = rent
            .as_ref()
            .ok_or(ClutchError::MismatchedState)?
            .minimum_balance(DEALER_ACTION_RECEIPT_ACCOUNT_BYTES)?;
        let action_index =
            DealerLivenessScheduleV1::action_index(DealerRuntimeActionV1::QueueExit);
        let receipt = DealerActionReceiptV1 {
            policy_id: state.policy_id,
            facility_id: state.facility_id,
            dealer_state_account_id: id(accounts[2].key),
            liveness_schedule_id: schedule.schedule_id().map_err(dealer_fault)?.untyped(),
            runtime_policy_id: runtime_binding.runtime_policy_id(),
            runtime_account_id: runtime_binding
                .account_id(DealerLivenessCompartmentV1::Retirement),
            runtime_owner: runtime_binding.owner(DealerLivenessCompartmentV1::Retirement),
            quote_schedule_id: runtime_binding
                .quote_schedule_id(DealerLivenessCompartmentV1::Retirement),
            receipt_account_id: id(accounts[17].key),
            receipt_program_id: id(program_id),
            keeper: id(accounts[0].key),
            replay_account_id: id(accounts[4].key),
            action: DealerRuntimeActionV1::QueueExit,
            compartment: DealerLivenessCompartmentV1::Retirement,
            runtime_generation: runtime_binding
                .generation(DealerLivenessCompartmentV1::Retirement),
            facility_generation: state.generation,
            call_ordinal: payload.liveness_call_ordinal,
            call_ceiling_lamports: schedule.reward_lamports[action_index],
            keeper_payment_lamports: payload.keeper_payment_lamports,
            expected_replay_ordinal: payload.expected_replay_ordinal,
            rent: DeletableRentOwnerV1 {
                payer: id(accounts[0].key),
                neutral_sink: policy.neutral_sink,
                refundable_principal: receipt_principal,
                donation_floor: accounts[17].lamports(),
            },
        };
        let receipt_slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
        let (receipt_address, receipt_bump) =
            seeds::dealer_action_receipt_pda(program_id, &receipt_slot.bytes());
        expect_pda(accounts[17].key, (receipt_address, receipt_bump), None)?;
        receipt
            .validate_against(&schedule, &runtime_binding)
            .map_err(dealer_fault)?;
        let authorization = receipt
            .authorization(&schedule, &runtime_binding, &retirement)
            .map_err(dealer_fault)?;
        let transition = plan_liveness_spend_absorbing_donation(
            program_id,
            &accounts[9],
            &accounts[15],
            retirement,
            receipt.runtime_transition_intent().map_err(dealer_fault)?,
            receipt
                .runtime_receipt_observation()
                .map_err(dealer_fault)?,
        )?;
        let checked = DealerQueueExitLivenessV1::external(
            &schedule,
            &runtime_binding,
            &authorization,
            &state,
            id(accounts[2].key),
        )
        .map_err(dealer_fault)?;
        liveness_transition = Some(transition);
        receipt_postwrite = Some((receipt, receipt_bump, receipt_principal));
        checked
    } else {
        DealerQueueExitLivenessV1::caller_funded()
    };

    let (ticket_after, state_after_base, replay_after) = if let Some(ticket) = existing_ticket {
        let prepared = prepare_increase_exit_ticket_v1(
            &policy,
            &state,
            id(accounts[2].key),
            id(accounts[5].key),
            &page,
            &ticket,
            payload.share_delta,
            liveness,
            &replay,
            replay_binding,
        )
        .map_err(dealer_fault)?;
        (
            prepared.ticket_after,
            prepared.state_after,
            prepared.replay,
        )
    } else {
        let prepared = prepare_new_exit_ticket_v1(
            &policy,
            &state,
            id(accounts[2].key),
            id(accounts[5].key),
            &page,
            payload.entry_index,
            id(accounts[0].key),
            payload.share_delta,
            DeletableRentOwnerV1 {
                payer: id(accounts[0].key),
                neutral_sink: policy.neutral_sink,
                refundable_principal: ticket_principal,
                donation_floor: ticket_donation,
            },
            liveness,
            &replay,
            replay_binding,
        )
        .map_err(dealer_fault)?;
        (prepared.ticket, prepared.state_after, prepared.replay)
    };
    let state_after_v3 = match state_v3 {
        Some(current) => Some(current.with_base(state_after_base).map_err(dealer_fault)?),
        None => None,
    };

    if !payload.existing_ticket {
        let rent = rent.as_ref().ok_or(ClutchError::MismatchedState)?;
        let system_index = if payload.external_liveness {
            20usize
        } else if payload.existing_series_admission {
            9usize
        } else {
            8usize
        };
        let (observed_principal, observed_donation) = create_full_principal_pda(
            program_id,
            &accounts[0],
            &accounts[6],
            &accounts[system_index],
            rent,
            DEALER_EXIT_TICKET_ACCOUNT_BYTES,
            &[
                seeds::SEED_DEALER_EXIT_TICKET,
                &state.facility_id.bytes(),
                &accounts[0].key.to_bytes(),
                &[expected_ticket_bump],
            ],
        )?;
        require(
            observed_principal == ticket_principal && observed_donation == ticket_donation,
            ClutchError::DealerPolicyRentMismatch,
        )?;
    }
    if let Some((receipt, receipt_bump, receipt_principal)) = receipt_postwrite {
        let rent = rent.as_ref().ok_or(ClutchError::MismatchedState)?;
        let (observed_principal, observed_donation) = create_full_principal_pda(
            program_id,
            &accounts[0],
            &accounts[17],
            &accounts[20],
            rent,
            DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
            &[
                seeds::SEED_DEALER_ACTION_RECEIPT,
                &receipt.receipt_slot_id().map_err(dealer_fault)?.bytes(),
                &[receipt_bump],
            ],
        )?;
        require(
            observed_principal == receipt_principal
                && observed_donation == receipt.rent().donation_floor,
            ClutchError::DealerPolicyRentMismatch,
        )?;
        apply_liveness_transition(
            &accounts[15],
            &accounts[0],
            &accounts[18],
            &liveness_transition.ok_or(ClutchError::MismatchedState)?,
        )?;
        write_dealer_body(
            &accounts[17],
            DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
            DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
            receipt_bump,
            &receipt,
        )?;
    }
    write_dealer_body(
        &accounts[6],
        DEALER_EXIT_TICKET_ACCOUNT_TAG,
        DEALER_EXIT_TICKET_ACCOUNT_VERSION,
        expected_ticket_bump,
        &ticket_after,
    )?;
    match state_after_v3 {
        Some(state_after) => write_dealer_body(
            &accounts[2],
            DEALER_STATE_V3_ACCOUNT_TAG,
            DEALER_STATE_V3_ACCOUNT_VERSION,
            state_bump,
            &state_after,
        )?,
        None => write_dealer_body(
            &accounts[2],
            DEALER_STATE_V2_ACCOUNT_TAG,
            DEALER_STATE_V2_ACCOUNT_VERSION,
            state_bump,
            &state_after_base,
        )?,
    };
    replay_after
        .replay_post()
        .encode_into(&mut accounts[4].data.borrow_mut())
        .map_err(dealer_fault)?;

    let (_, observed_ticket) = authenticate_exit_ticket(program_id, &accounts[6])?;
    let (_, observed_position, observed_replay, _) = authenticate_position_and_replay(
        program_id,
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &policy,
        &state_after_base,
        false,
    )?;
    let state_matches = match state_after_v3 {
        Some(state_after) => {
            let observed_state = authenticate_dealer_state_v3(program_id, &accounts[2], true)?;
            let observed_obligation = authenticate_dealer_series_obligation_v1(
                program_id,
                &accounts[obligation_index],
                false,
            )?;
            let obligation_matches = match obligation.as_ref() {
                Some(value) => observed_obligation.binding() == value,
                None => false,
            };
            observed_state.state() == &state_after && obligation_matches
        }
        None => authenticate_state(program_id, &accounts[2])? == state_after_base,
    };
    require(
        state_matches
            && observed_ticket == ticket_after
            && observed_position == position
            && observed_replay == replay_after.replay_post(),
        ClutchError::MismatchedState,
    )?;
    if let Some((receipt, _, _)) = receipt_postwrite {
        let (_, observed_receipt) = authenticate_action_receipt(program_id, &accounts[17])?;
        let transition = liveness_transition.ok_or(ClutchError::MismatchedState)?;
        let retirement_data = accounts[15]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(
            observed_receipt == receipt
                && accounts[15].lamports() == transition.account_balance_after
                && retirement_data.as_ref() == transition.post_account_data.as_slice(),
            ClutchError::MismatchedState,
        )?;
    }
    Ok(())
}

/// Bind the next General epoch to either the founding StateV2 root or the
/// admitted StateV3 root. Repeated epochs preserve the exact live Product
/// obligation and never lower the state back to V2.
#[inline(never)]
fn bind_epoch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    let payload = DealerRuntimePayloadV1::decode(DealerFacilityAction::BindEpoch, payload_bytes)
        .map_err(dealer_fault)?;
    let expected_count = BIND_EPOCH_ACCOUNT_COUNT
        .checked_add(usize::from(payload.existing_series_admission))
        .ok_or(ClutchError::Arithmetic)?;
    require_count(accounts, expected_count)?;
    require(
        sequence == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    // Actor and the immutable compartment payer may intentionally be the same
    // writable system account. Every semantic/state role remains disjoint.
    require_aliases(accounts, (0, 16))?;

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let (state, state_v3, state_bump, obligation) = if payload.existing_series_admission {
        let authenticated = authenticate_dealer_state_v3(program_id, &accounts[2], true)?;
        let state_v3 = *authenticated.state();
        let obligation = authenticate_live_series_obligation_for_state_v3(
            program_id,
            &accounts[24],
            &accounts[2],
            &state_v3,
        )?;
        (state_v3.base, Some(state_v3), authenticated.bump(), Some(obligation))
    } else {
        let state = authenticate_state(program_id, &accounts[2])?;
        let bump = accounts[2].data.borrow()[2];
        (state, None, bump, None)
    };
    require(
        state.policy_id.bytes() == policy_id && state.generation == payload.expected_generation,
        ClutchError::MismatchedState,
    )?;
    let (binding, _position, replay, replay_binding) = authenticate_position_and_replay(
        program_id,
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &policy,
        &state,
        false,
    )?;
    require(
        replay.next_transition_ordinal() == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let dependency = authenticate_dependency(program_id, &accounts[5], state.facility_id)?;
    let schedule = authenticate_schedule(program_id, &accounts[6])?;
    let (runtime_policy, runtime_states, runtime_binding) = authenticate_runtime_bundle(
        program_id,
        &dependency,
        &accounts[7],
        &accounts[8..15],
        DealerLivenessCompartmentV1::Candidate.index(),
    )?;
    validate_runtime_dependency_join(
        program_id,
        &accounts[2],
        &policy,
        &state,
        &binding,
        &dependency,
        &schedule,
        runtime_policy,
        runtime_binding,
    )?;
    let candidate = runtime_states[DealerLivenessCompartmentV1::Candidate.index()];
    require(
        candidate.identity.payer.bytes() == accounts[16].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let rent = read_rent(&accounts[22])?;
    require(
        accounts[6].lamports() >= rent.minimum_balance(DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES)?,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    require_system_program(&accounts[23])?;
    require_creatable(&accounts[15])?;
    require_creatable(&accounts[17])?;
    let receipt_principal = rent.minimum_balance(DEALER_ACTION_RECEIPT_ACCOUNT_BYTES)?;
    let receipt = DealerActionReceiptV1 {
        policy_id: state.policy_id,
        facility_id: state.facility_id,
        dealer_state_account_id: id(accounts[2].key),
        liveness_schedule_id: schedule.schedule_id().map_err(dealer_fault)?.untyped(),
        runtime_policy_id: runtime_binding.runtime_policy_id(),
        runtime_account_id: runtime_binding.account_id(DealerLivenessCompartmentV1::Candidate),
        runtime_owner: runtime_binding.owner(DealerLivenessCompartmentV1::Candidate),
        quote_schedule_id: runtime_binding
            .quote_schedule_id(DealerLivenessCompartmentV1::Candidate),
        receipt_account_id: id(accounts[15].key),
        receipt_program_id: id(program_id),
        keeper: id(accounts[0].key),
        replay_account_id: id(accounts[4].key),
        action: DealerRuntimeActionV1::BindEpoch,
        compartment: DealerLivenessCompartmentV1::Candidate,
        runtime_generation: runtime_binding.generation(DealerLivenessCompartmentV1::Candidate),
        facility_generation: state.generation,
        call_ordinal: payload.liveness_call_ordinal,
        call_ceiling_lamports: schedule.reward_lamports[DealerRuntimeActionV1::BindEpoch as usize],
        keeper_payment_lamports: payload.keeper_payment_lamports,
        expected_replay_ordinal: payload.expected_replay_ordinal,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: receipt_principal,
            donation_floor: accounts[15].lamports(),
        },
    };
    let receipt_slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
    let (receipt_address, receipt_bump) =
        seeds::dealer_action_receipt_pda(program_id, &receipt_slot.bytes());
    expect_pda(accounts[15].key, (receipt_address, receipt_bump), None)?;
    receipt
        .validate_against(&schedule, &runtime_binding)
        .map_err(dealer_fault)?;
    let authorization = receipt
        .authorization(&schedule, &runtime_binding, &candidate)
        .map_err(dealer_fault)?;
    let intent = receipt.runtime_transition_intent().map_err(dealer_fault)?;
    let observation = receipt
        .runtime_receipt_observation()
        .map_err(dealer_fault)?;
    let liveness_transition = plan_liveness_spend_absorbing_donation(
        program_id,
        &accounts[7],
        &accounts[9],
        candidate,
        intent,
        observation,
    )?;
    require(
        runtime_policy.policy_id.bytes() == dependency.bindings.runtime_liveness_policy_id.bytes(),
        ClutchError::MismatchedState,
    )?;

    let general = authenticate_general_epoch(
        program_id,
        &accounts[18],
        &accounts[19],
        &accounts[20],
        &policy,
    )?;
    let current_slot = read_clock_slot(&accounts[21])?;
    let (epoch_address, epoch_bump) =
        seeds::dealer_epoch_v2_pda(program_id, &state.facility_id.bytes(), state.generation);
    expect_pda(accounts[17].key, (epoch_address, epoch_bump), None)?;
    let epoch_principal = rent.minimum_balance(DEALER_EPOCH_BINDING_V2_ACCOUNT_BYTES)?;
    let epoch = DealerEpochBindingV2::new_bound(
        &policy,
        &state,
        id(accounts[2].key),
        &dependency,
        &schedule,
        &runtime_binding,
        &authorization,
        &general,
        id(accounts[17].key),
        current_slot,
        DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: epoch_principal,
            donation_floor: accounts[17].lamports(),
        },
    )
    .map_err(dealer_fault)?;
    let prepared = prepare_bind_epoch_v3(
        &policy,
        &state,
        id(accounts[2].key),
        &dependency,
        &schedule,
        &runtime_binding,
        &authorization,
        &epoch,
        &general,
        &replay,
        replay_binding,
    )
    .map_err(dealer_fault)?;
    let state_after_v3 = match state_v3 {
        Some(current) => Some(current.with_base(prepared.state_after).map_err(dealer_fault)?),
        None => None,
    };

    let generation_bytes = state.generation.to_le_bytes();
    let (observed_receipt_principal, observed_receipt_donation) = create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[15],
        &accounts[23],
        &rent,
        DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_ACTION_RECEIPT,
            &receipt_slot.bytes(),
            &[receipt_bump],
        ],
    )?;
    let (observed_epoch_principal, observed_epoch_donation) = create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[17],
        &accounts[23],
        &rent,
        DEALER_EPOCH_BINDING_V2_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_EPOCH_V2,
            &state.facility_id.bytes(),
            &generation_bytes,
            &[epoch_bump],
        ],
    )?;
    require(
        observed_receipt_principal == receipt.rent.refundable_principal
            && observed_receipt_donation == receipt.rent.donation_floor
            && observed_epoch_principal == epoch.rent.refundable_principal
            && observed_epoch_donation == epoch.rent.donation_floor,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    apply_liveness_transition(
        &accounts[9],
        &accounts[0],
        &accounts[16],
        &liveness_transition,
    )?;
    write_dealer_body(
        &accounts[15],
        DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
        DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
        receipt_bump,
        &receipt,
    )?;
    write_dealer_body(
        &accounts[17],
        DEALER_EPOCH_BINDING_V2_ACCOUNT_TAG,
        DEALER_EPOCH_BINDING_V2_ACCOUNT_VERSION,
        epoch_bump,
        &epoch,
    )?;
    match state_after_v3 {
        Some(state_after) => write_dealer_body(
            &accounts[2],
            DEALER_STATE_V3_ACCOUNT_TAG,
            DEALER_STATE_V3_ACCOUNT_VERSION,
            state_bump,
            &state_after,
        )?,
        None => write_dealer_body(
            &accounts[2],
            DEALER_STATE_V2_ACCOUNT_TAG,
            DEALER_STATE_V2_ACCOUNT_VERSION,
            state_bump,
            &prepared.state_after,
        )?,
    };
    prepared
        .replay
        .replay_post()
        .encode_into(&mut accounts[4].data.borrow_mut())
        .map_err(dealer_fault)?;

    let (_, observed_receipt) = authenticate_action_receipt(program_id, &accounts[15])?;
    let (_, observed_epoch) =
        authenticate_epoch_binding(program_id, &accounts[17], state.facility_id)?;
    let observed_replay = DealerFacilityReplayV1::decode(&accounts[4].data.borrow())
        .map_err(dealer_fault)?;
    let state_matches = match state_after_v3 {
        Some(state_after) => {
            let observed_state = authenticate_dealer_state_v3(program_id, &accounts[2], true)?;
            let observed_obligation =
                authenticate_dealer_series_obligation_v1(program_id, &accounts[24], false)?;
            let obligation_matches = match obligation.as_ref() {
                Some(value) => observed_obligation.binding() == value,
                None => false,
            };
            observed_state.state() == &state_after && obligation_matches
        }
        None => authenticate_state(program_id, &accounts[2])? == prepared.state_after,
    };
    let candidate_data = accounts[9]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        state_matches
            && observed_receipt == receipt
            && observed_epoch == epoch
            && observed_replay == prepared.replay.replay_post()
            && accounts[9].lamports() == liveness_transition.account_balance_after
            && candidate_data.as_ref() == liveness_transition.post_account_data.as_slice(),
        ClutchError::MismatchedState,
    )
}

/// Lapse one unused epoch and consume the Position generation. The transition
/// preserves a live Product obligation when the facility was already admitted,
/// and closes both epoch-owned rent accounts with exact principal/donation
/// postconditions.
#[inline(never)]
fn lapse_epoch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    let payload = DealerRuntimePayloadV1::decode(DealerFacilityAction::LapseEpoch, payload_bytes)
        .map_err(dealer_fault)?;
    let expected_count = LAPSE_EPOCH_ACCOUNT_COUNT
        .checked_add(usize::from(payload.existing_series_admission))
        .ok_or(ClutchError::Arithmetic)?;
    require_count(accounts, expected_count)?;
    require(
        sequence == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    require_lapse_aliases(accounts)?;

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let (state, state_v3, state_bump, obligation) = if payload.existing_series_admission {
        let authenticated = authenticate_dealer_state_v3(program_id, &accounts[2], true)?;
        let state_v3 = *authenticated.state();
        let obligation = authenticate_live_series_obligation_for_state_v3(
            program_id,
            &accounts[25],
            &accounts[2],
            &state_v3,
        )?;
        (state_v3.base, Some(state_v3), authenticated.bump(), Some(obligation))
    } else {
        let state = authenticate_state(program_id, &accounts[2])?;
        let bump = accounts[2].data.borrow()[2];
        (state, None, bump, None)
    };
    require(
        state.policy_id.bytes() == policy_id && state.generation == payload.expected_generation,
        ClutchError::MismatchedState,
    )?;
    let (binding, position_before, replay, replay_binding) =
        authenticate_position_and_replay(
            program_id,
            &accounts[2],
            &accounts[3],
            &accounts[4],
            &policy,
            &state,
            true,
        )?;
    require(
        replay.next_transition_ordinal() == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let dependency = authenticate_dependency(program_id, &accounts[5], state.facility_id)?;
    let schedule = authenticate_schedule(program_id, &accounts[6])?;
    let (runtime_policy, runtime_states, runtime_binding) = authenticate_runtime_bundle(
        program_id,
        &dependency,
        &accounts[7],
        &accounts[8..15],
        DealerLivenessCompartmentV1::Candidate.index(),
    )?;
    validate_runtime_dependency_join(
        program_id,
        &accounts[2],
        &policy,
        &state,
        &binding,
        &dependency,
        &schedule,
        runtime_policy,
        runtime_binding,
    )?;
    let candidate = runtime_states[DealerLivenessCompartmentV1::Candidate.index()];
    require(
        candidate.identity.payer.bytes() == accounts[18].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let (_epoch_bump, epoch) =
        authenticate_epoch_binding(program_id, &accounts[16], state.facility_id)?;
    let (_bind_receipt_bump, bind_receipt) =
        authenticate_action_receipt(program_id, &accounts[17])?;
    let current_slot = read_clock_slot(&accounts[22])?;
    let rent = read_rent(&accounts[23])?;
    require_system_program(&accounts[24])?;
    require(
        accounts[6].lamports() >= rent.minimum_balance(DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES)?,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    require_creatable(&accounts[15])?;

    let receipt_principal = rent.minimum_balance(DEALER_ACTION_RECEIPT_ACCOUNT_BYTES)?;
    let receipt = DealerActionReceiptV1 {
        policy_id: state.policy_id,
        facility_id: state.facility_id,
        dealer_state_account_id: id(accounts[2].key),
        liveness_schedule_id: schedule.schedule_id().map_err(dealer_fault)?.untyped(),
        runtime_policy_id: runtime_binding.runtime_policy_id(),
        runtime_account_id: runtime_binding.account_id(DealerLivenessCompartmentV1::Candidate),
        runtime_owner: runtime_binding.owner(DealerLivenessCompartmentV1::Candidate),
        quote_schedule_id: runtime_binding
            .quote_schedule_id(DealerLivenessCompartmentV1::Candidate),
        receipt_account_id: id(accounts[15].key),
        receipt_program_id: id(program_id),
        keeper: id(accounts[0].key),
        replay_account_id: id(accounts[4].key),
        action: DealerRuntimeActionV1::LapseEpoch,
        compartment: DealerLivenessCompartmentV1::Candidate,
        runtime_generation: runtime_binding.generation(DealerLivenessCompartmentV1::Candidate),
        facility_generation: state.generation,
        call_ordinal: payload.liveness_call_ordinal,
        call_ceiling_lamports: schedule.reward_lamports
            [DealerRuntimeActionV1::LapseEpoch as usize],
        keeper_payment_lamports: payload.keeper_payment_lamports,
        expected_replay_ordinal: payload.expected_replay_ordinal,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: receipt_principal,
            donation_floor: accounts[15].lamports(),
        },
    };
    let receipt_slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
    let (receipt_address, receipt_bump) =
        seeds::dealer_action_receipt_pda(program_id, &receipt_slot.bytes());
    expect_pda(accounts[15].key, (receipt_address, receipt_bump), None)?;
    receipt
        .validate_against(&schedule, &runtime_binding)
        .map_err(dealer_fault)?;
    let authorization = receipt
        .authorization(&schedule, &runtime_binding, &candidate)
        .map_err(dealer_fault)?;
    let liveness_transition = plan_liveness_spend_absorbing_donation(
        program_id,
        &accounts[7],
        &accounts[9],
        candidate,
        receipt.runtime_transition_intent().map_err(dealer_fault)?,
        receipt
            .runtime_receipt_observation()
            .map_err(dealer_fault)?,
    )?;
    let (position_after, position_after_observation) = successor_facility_position(
        id(accounts[3].key),
        &position_before,
        &binding,
        &policy,
    )?;
    let prepared = prepare_lapse_epoch_v3(
        &policy,
        &state,
        id(accounts[2].key),
        &binding,
        &epoch,
        &schedule,
        &runtime_binding,
        &bind_receipt,
        &authorization,
        &position_before,
        &position_after_observation,
        &replay,
        replay_binding,
        current_slot,
        DealerEpochCloseRentV2 {
            epoch_lamports_before: accounts[16].lamports(),
            epoch_lamports_after: 0,
            bind_receipt_lamports_before: accounts[17].lamports(),
            bind_receipt_lamports_after: 0,
        },
    )
    .map_err(dealer_fault)?;
    let state_after_v3 = match state_v3 {
        Some(current) => Some(current.with_base(prepared.state_after).map_err(dealer_fault)?),
        None => None,
    };
    require(
        prepared.close_credits.epoch_neutral_sink == policy.neutral_sink
            && prepared.close_credits.bind_receipt_neutral_sink == policy.neutral_sink,
        ClutchError::MismatchedState,
    )?;

    let (observed_lapse_receipt_principal, observed_lapse_receipt_donation) =
        create_full_principal_pda(
            program_id,
            &accounts[0],
            &accounts[15],
            &accounts[24],
            &rent,
            DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
            &[
                seeds::SEED_DEALER_ACTION_RECEIPT,
                &receipt_slot.bytes(),
                &[receipt_bump],
            ],
        )?;
    require(
        observed_lapse_receipt_principal == receipt.rent.refundable_principal
            && observed_lapse_receipt_donation == receipt.rent.donation_floor,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    apply_liveness_transition(
        &accounts[9],
        &accounts[0],
        &accounts[18],
        &liveness_transition,
    )?;
    write_dealer_body(
        &accounts[15],
        DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
        DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
        receipt_bump,
        &receipt,
    )?;
    accounts[3]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &position_after
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    match state_after_v3 {
        Some(state_after) => write_dealer_body(
            &accounts[2],
            DEALER_STATE_V3_ACCOUNT_TAG,
            DEALER_STATE_V3_ACCOUNT_VERSION,
            state_bump,
            &state_after,
        )?,
        None => write_dealer_body(
            &accounts[2],
            DEALER_STATE_V2_ACCOUNT_TAG,
            DEALER_STATE_V2_ACCOUNT_VERSION,
            state_bump,
            &prepared.state_after,
        )?,
    };
    prepared
        .replay
        .replay_post()
        .encode_into(&mut accounts[4].data.borrow_mut())
        .map_err(dealer_fault)?;
    apply_epoch_close(
        &accounts[16],
        &accounts[17],
        &accounts[19],
        &accounts[20],
        &accounts[21],
        prepared.close_credits,
    )?;

    let (_, observed_receipt) = authenticate_action_receipt(program_id, &accounts[15])?;
    let (_, observed_position, observed_replay, _) = authenticate_position_and_replay(
        program_id,
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &policy,
        &prepared.state_after,
        true,
    )?;
    let state_matches = match state_after_v3 {
        Some(state_after) => {
            let observed_state = authenticate_dealer_state_v3(program_id, &accounts[2], true)?;
            let observed_obligation =
                authenticate_dealer_series_obligation_v1(program_id, &accounts[25], false)?;
            let obligation_matches = match obligation.as_ref() {
                Some(value) => observed_obligation.binding() == value,
                None => false,
            };
            observed_state.state() == &state_after && obligation_matches
        }
        None => authenticate_state(program_id, &accounts[2])? == prepared.state_after,
    };
    let candidate_data = accounts[9]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        state_matches
            && observed_receipt == receipt
            && observed_position == position_after_observation
            && observed_replay == prepared.replay.replay_post()
            && accounts[9].lamports() == liveness_transition.account_balance_after
            && candidate_data.as_ref() == liveness_transition.post_account_data.as_slice(),
        ClutchError::MismatchedState,
    )
}

#[inline(never)]
fn select_lease_and_begin(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    let payload = DealerRuntimePayloadV1::decode(
        DealerFacilityAction::SelectLeaseAndBegin,
        payload_bytes,
    )
    .map_err(dealer_fault)?;
    let expected_count = SELECT_LEASE_BEGIN_FIXED_ACCOUNT_COUNT
        .checked_add(usize::from(payload.book_page_count))
        .ok_or(ClutchError::Arithmetic)?;
    require_count(accounts, expected_count)?;
    require(
        sequence == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    // The permissionless actor may also be the immutable Candidate payer.
    // No semantic owner, artifact, new PDA, or page may alias another role.
    require_aliases(accounts, (0, 17))?;

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let state_v3_before = if payload.existing_series_admission {
        Some(*authenticate_dealer_state_v3(program_id, &accounts[2], true)?.state())
    } else {
        None
    };
    let state = match state_v3_before {
        Some(state_v3) => state_v3.base,
        None => authenticate_state(program_id, &accounts[2])?,
    };
    require(
        state.policy_id.bytes() == policy_id && state.generation == payload.expected_generation,
        ClutchError::MismatchedState,
    )?;
    let (position_binding, facility_position, replay, replay_binding) =
        authenticate_position_and_replay(
            program_id,
            &accounts[2],
            &accounts[3],
            &accounts[4],
            &policy,
            &state,
            true,
        )?;
    require(
        replay.next_transition_ordinal() == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let dependency = authenticate_dependency(program_id, &accounts[5], state.facility_id)?;
    let (epoch_bump, epoch) =
        authenticate_epoch_binding(program_id, &accounts[6], state.facility_id)?;
    let schedule = authenticate_schedule(program_id, &accounts[7])?;
    let (runtime_policy, runtime_states, runtime_binding) = authenticate_runtime_bundle(
        program_id,
        &dependency,
        &accounts[8],
        &accounts[9..16],
        DealerLivenessCompartmentV1::Candidate.index(),
    )?;
    validate_runtime_dependency_join(
        program_id,
        &accounts[2],
        &policy,
        &state,
        &position_binding,
        &dependency,
        &schedule,
        runtime_policy,
        runtime_binding,
    )?;
    let candidate = runtime_states[DealerLivenessCompartmentV1::Candidate.index()];
    require(
        candidate.identity.payer.bytes() == accounts[17].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let rent = read_rent(&accounts[38])?;
    require_system_program(&accounts[39])?;
    require(
        accounts[7].lamports() >= rent.minimum_balance(DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES)?,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    for account in [&accounts[16], &accounts[34], &accounts[35], &accounts[36]] {
        require_creatable(account)?;
    }
    let current_slot = read_clock_slot(&accounts[37])?;

    let receipt_principal = rent.minimum_balance(DEALER_ACTION_RECEIPT_ACCOUNT_BYTES)?;
    let selection_principal = rent.minimum_balance(DEALER_COVERED_SELECTION_ACCOUNT_BYTES)?;
    let lease_principal = rent.minimum_balance(DEALER_LEASE_V2_ACCOUNT_BYTES)?;
    let pot_principal = rent.minimum_balance(DEALER_SETTLEMENT_POT_V2_ACCOUNT_BYTES)?;
    let (state_upgrade_principal, series_obligation_principal) =
        if payload.existing_series_admission {
            (0, 0)
        } else {
            let state_v2_principal = rent.minimum_balance(DEALER_STATE_V2_ACCOUNT_BYTES)?;
            let state_v3_principal = rent.minimum_balance(DEALER_STATE_V3_ACCOUNT_BYTES)?;
            (
                state_v3_principal
                    .checked_sub(state_v2_principal)
                    .ok_or(ClutchError::Arithmetic)?,
                rent.minimum_balance(DEALER_SERIES_OBLIGATION_ACCOUNT_BYTES_V2)?,
            )
        };
    let series_obligation_donation_floor = if payload.existing_series_admission {
        0
    } else {
        accounts[57].lamports()
    };
    let total_child_principal = select_begin_rent_principal(
        receipt_principal,
        selection_principal,
        lease_principal,
        pot_principal,
        payload.keeper_payment_lamports,
    )?
    .checked_add(state_upgrade_principal)
    .and_then(|value| value.checked_add(series_obligation_principal))
    .ok_or(ClutchError::Arithmetic)?;

    let action_index = DealerLivenessScheduleV1::action_index(
        DealerRuntimeActionV1::SelectLeaseAndBegin,
    );
    let receipt = DealerActionReceiptV1 {
        policy_id: state.policy_id,
        facility_id: state.facility_id,
        dealer_state_account_id: id(accounts[2].key),
        liveness_schedule_id: schedule.schedule_id().map_err(dealer_fault)?.untyped(),
        runtime_policy_id: runtime_binding.runtime_policy_id(),
        runtime_account_id: runtime_binding.account_id(DealerLivenessCompartmentV1::Candidate),
        runtime_owner: runtime_binding.owner(DealerLivenessCompartmentV1::Candidate),
        quote_schedule_id: runtime_binding
            .quote_schedule_id(DealerLivenessCompartmentV1::Candidate),
        receipt_account_id: id(accounts[16].key),
        receipt_program_id: id(program_id),
        keeper: id(accounts[0].key),
        replay_account_id: id(accounts[4].key),
        action: DealerRuntimeActionV1::SelectLeaseAndBegin,
        compartment: DealerLivenessCompartmentV1::Candidate,
        runtime_generation: runtime_binding.generation(DealerLivenessCompartmentV1::Candidate),
        facility_generation: state.generation,
        call_ordinal: payload.liveness_call_ordinal,
        call_ceiling_lamports: schedule.reward_lamports[action_index],
        keeper_payment_lamports: payload.keeper_payment_lamports,
        expected_replay_ordinal: payload.expected_replay_ordinal,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[17].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: receipt_principal,
            donation_floor: accounts[16].lamports(),
        },
    };
    let receipt_slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
    let (receipt_address, receipt_bump) =
        seeds::dealer_action_receipt_pda(program_id, &receipt_slot.bytes());
    expect_pda(accounts[16].key, (receipt_address, receipt_bump), None)?;
    receipt
        .validate_against(&schedule, &runtime_binding)
        .map_err(dealer_fault)?;
    let authorization = receipt
        .authorization(&schedule, &runtime_binding, &candidate)
        .map_err(dealer_fault)?;
    require(
        authorization.call_ceiling_lamports >= total_child_principal,
        ClutchError::MismatchedState,
    )?;
    let liveness_transition = plan_liveness_spend_absorbing_donation(
        program_id,
        &accounts[8],
        &accounts[10],
        candidate,
        receipt.runtime_transition_intent().map_err(dealer_fault)?,
        receipt
            .runtime_receipt_observation()
            .map_err(dealer_fault)?,
    )?;

    let root_inputs = authenticate_covered_root_inputs_v1(
        program_id,
        &accounts[18],
        &accounts[20],
        &accounts[21],
        &accounts[24],
        &policy,
        true,
    )?;
    let root = root_inputs.root.as_ref();
    require(
        root.epoch().bytes() == epoch.epoch_account_id.bytes()
            && root.epoch_generation() == epoch.general_epoch_generation,
        ClutchError::MismatchedState,
    )?;
    let selected_fee = authenticate_selected_dealer_fee_v1(
        program_id,
        root,
        &accounts[30],
        &accounts[31],
        &accounts[32],
        &accounts[33],
    )?;
    let quote = authenticate_signed_dealer_quote_v1(&accounts[22], &accounts[23], &policy)?;
    let product = authenticate_covered_product_v1(
        program_id,
        &accounts[25],
        &accounts[26],
        &accounts[27],
        &accounts[28],
        &accounts[29],
        root,
        root_inputs.binding.as_ref(),
        root_inputs.grid.as_ref(),
        &policy,
    )?;

    let generation_bytes = state.generation.to_le_bytes();
    let facility_bytes = state.facility_id.bytes();
    let (lease_address, lease_bump) =
        seeds::dealer_lease_v2_pda(program_id, &facility_bytes, state.generation);
    let (pot_address, pot_bump) =
        seeds::dealer_pot_v2_pda(program_id, &facility_bytes, state.generation);
    expect_pda(accounts[35].key, (lease_address, lease_bump), None)?;
    expect_pda(accounts[36].key, (pot_address, pot_bump), None)?;
    let root_epoch_bytes = root.epoch().bytes();
    let settlement_candidate_bytes = root.settlement_candidate_id().bytes();
    let (selection_address, selection_bump) = seeds::dealer_covered_selection_pda(
        program_id,
        &root_epoch_bytes,
        &settlement_candidate_bytes,
    );
    expect_pda(
        accounts[34].key,
        (selection_address, selection_bump),
        None,
    )?;

    let (collateral_value, market) = authenticate_dealer_collateral_value_v2(
        program_id,
        &policy,
        Some(&position_binding),
        DealerCollateralAuthorityAccountsV2 {
            realm: &accounts[40],
            profile: &accounts[41],
            policy: &accounts[42],
            token_program: &accounts[43],
            token_programdata: &accounts[44],
            market_binding: &accounts[21],
            market_runtime: &accounts[45],
            market_instance: &accounts[25],
            hoard: &accounts[46],
            claim_ledger: &accounts[47],
        },
    )?;
    let mut product_link_pre = Box::new(SeriesMarketLinkAccountV2::decode_buffer());
    {
        let data = accounts[54]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        SeriesMarketLinkAccountV2::decode_into(&data, &mut product_link_pre)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    }
    let untrusted_product_binding = product_link_pre.state.binding();
    let mut product_root_pre = Box::new(MarketLifecycleRootAccountV2::decode_buffer());
    let product_root = authenticate_market_lifecycle_root_v2(
        program_id,
        &accounts[48],
        untrusted_product_binding.market_instance_id,
        untrusted_product_binding.generation,
        false,
        &mut product_root_pre,
    )?;
    let product_registry = authenticate_series_registry_account_v3(
        program_id,
        &accounts[49],
        untrusted_product_binding.series_plan_id,
        false,
    )?;
    let registry_capability = authenticate_registry_capability_v4(
        program_id,
        product_registry,
        &accounts[50],
        &accounts[51],
        &accounts[52],
        &accounts[53],
    )?;
    let product_link = authenticate_series_market_link_v2(
        program_id,
        &accounts[54],
        untrusted_product_binding.series_plan_id,
        untrusted_product_binding.ordinal,
        untrusted_product_binding.market_instance_id,
        untrusted_product_binding.generation,
        Pubkey::new_from_array(untrusted_product_binding.market_root_account_id.bytes()),
        !payload.existing_series_admission,
        &mut product_link_pre,
    )?;
    let root_binding = product_root.state().binding();
    require(
        root_binding.market_instance_id.bytes() == policy.market_instance_v2_id.bytes()
            && root_binding.realm_id.bytes() == market.realm_id.bytes()
            && root_binding.collateral_profile_id.bytes()
                == collateral_value.liabilities.hoard.profile_id.bytes()
            && root_binding.collateral_policy_id.bytes()
                == market.collateral_policy_id.bytes()
            && root_binding.collateral_release_id.bytes()
                == market.collateral_release_id.bytes()
            && untrusted_product_binding.neutral_lamport_sink.bytes()
                == policy.neutral_sink.bytes()
            && registry_capability.capability_profile_id().bytes()
                == collateral_value.liabilities.hoard.profile_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let (series_obligation_address, series_obligation_bump) =
        seeds::dealer_series_obligation_pda(program_id, &state.facility_id.bytes());
    expect_pda(
        accounts[57].key,
        (series_obligation_address, series_obligation_bump),
        None,
    )?;
    let product_root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let series_obligation_key = DealerSeriesObligationKeyV2 {
        binding_account_id: id(accounts[57].key),
        policy_id: state.policy_id,
        facility_id: state.facility_id,
        dealer_state_account_id: id(accounts[2].key),
        facility_position_binding_id: state.facility_position_binding_id,
        market_instance_v2_id: policy.market_instance_v2_id,
        product_market_root_account_id: id(&product_root.account()),
        product_market_binding_id: Id::from_bytes(product_root_binding_id.bytes()),
        series_plan_v5_id: Id::from_bytes(untrusted_product_binding.series_plan_id.bytes()),
        series_market_link_account_id: id(accounts[54].key),
        compiler_bundle_v6_id: Id::from_bytes(untrusted_product_binding.compiler_bundle_id.bytes()),
        attachment_plan_v5_id: Id::from_bytes(untrusted_product_binding.attachment_plan_id.bytes()),
        product_generation: untrusted_product_binding.generation,
        series_ordinal: untrusted_product_binding.ordinal,
    };
    let existing_series_admission = if let Some(state_v3) = state_v3_before {
        let authenticated_obligation =
            authenticate_dealer_series_obligation_v2(program_id, &accounts[57], false)?;
        let product_live = authenticate_live_series_dealer_obligation_v2(
            program_id,
            product_root,
            product_link,
            &registry_capability,
            &accounts[55],
            &accounts[56],
        )?;
        let existing = authenticate_existing_dealer_series_admission_v2(
            product_live,
            &accounts[2],
            state_v3,
            &accounts[57],
            *authenticated_obligation.binding(),
        )?;
        require(
            existing.obligation.key == series_obligation_key,
            ClutchError::MismatchedState,
        )?;
        Some(existing)
    } else {
        None
    };
    let series_admission_prewrite = if existing_series_admission.is_none() {
        require_creatable(&accounts[57])?;
        Some(authenticate_dealer_series_admission_prewrite_v2(
            program_id,
            product_root,
            product_link,
            &registry_capability,
            &accounts[2],
            &state,
            &accounts[57],
            series_obligation_key,
            series_obligation_bump,
            &accounts[17],
            series_obligation_principal,
            series_obligation_donation_floor,
        )?)
    } else {
        None
    };
    let facility_endpoint = DealerTransferPositionV3::Facility {
        account_id: id(accounts[3].key),
        position: facility_position.projection,
    };

    with_authenticated_complete_dealer_book_v2(
        program_id,
        &accounts[18],
        root,
        &accounts[19],
        root_inputs.domain.as_ref(),
        &accounts[SELECT_LEASE_BEGIN_FIXED_ACCOUNT_COUNT..],
        true,
        |book, feed_data| {
            let (feed_header, _) = decode_sealed_candidate_feed_v1(feed_data)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            let mut verdict = super::orders_batch::boxed_copy_of(
                &EMPTY_DEALER_LEG_VERDICT_V2,
            )?;
            let verified = verify_smooth_covered_dealer_candidate_into_v1(
                clutch_general_v2_contract::Id32::new(accounts[19].key.to_bytes())?,
                feed_data,
                clutch_general_v2_contract::Id32::new(accounts[18].key.to_bytes())?,
                root,
                root_inputs.domain.as_ref(),
                root_inputs.binding.as_ref(),
                root_inputs.grid.as_ref(),
                product.product_template.value(),
                product.native_basis.value(),
                product.price_policy.value(),
                product.genesis.value(),
                product.market_instance.value(),
                QuantizedEdgePolicyV1::Clamp,
                book,
                &quote.dealer,
                &quote.quote,
                &mut verdict,
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

            let child_rent = |principal: u64, account: &AccountInfo<'_>| {
                DeletableRentOwnerV1 {
                    payer: id(accounts[17].key),
                    neutral_sink: policy.neutral_sink,
                    refundable_principal: principal,
                    donation_floor: account.lamports(),
                }
            };
            let mut selection =
                super::orders_batch::boxed_copy_of(&EMPTY_COVERED_DEALER_SELECTION_V1)?;
            CoveredDealerSelectionV1::from_verified_ref_into(
                CoveredDealerSelectionContextV1 {
                    selection_account_id: id(accounts[34].key),
                    settlement_root_account_id: id(accounts[18].key),
                    retained_feed_account_id: id(accounts[19].key),
                    upstream_economic_candidate_id: Id::from_bytes(
                        feed_header.base_relation_candidate_id.bytes(),
                    ),
                    candidate_bundle_digest: Id::from_bytes(
                        root.candidate_bundle_digest().bytes(),
                    ),
                    settlement_witness_digest: Id::from_bytes(
                        root.settlement_witness_digest().bytes(),
                    ),
                    lease_account_id: id(accounts[35].key),
                    settlement_pot_account_id: id(accounts[36].key),
                    current_slot,
                    stored_bump: selection_bump,
                    rent: child_rent(selection_principal, &accounts[34]),
                },
                &policy,
                &state,
                &epoch,
                root,
                &quote,
                verified.dealer_leg(),
                verified.price_measure(),
                &selected_fee,
                &mut selection,
            )
            .map_err(dealer_fault)?;

            let projected_position =
                project_covered_dealer_position_v1(&selection, market, facility_endpoint)
                    .map_err(dealer_fault)?;
            let leased_position_id = position_semantic_id(projected_position.leased())?;
            let terminal_position_id = position_semantic_id(projected_position.terminal())?;
            let post_generation = state
                .generation
                .checked_add(1)
                .ok_or(ClutchError::Arithmetic)?;
            let selected_fee_digest = selected_fee.binding_digest().map_err(dealer_fault)?;
            let dependency_id = dependency.dependency_id().map_err(dealer_fault)?;
            let schedule_id = schedule.schedule_id().map_err(dealer_fault)?.untyped();
            let runtime_binding_digest = runtime_binding.binding_digest().map_err(dealer_fault)?;
            let selection_id = selection.selection_id().map_err(dealer_fault)?;

            let lease = Box::new(DealerLeaseV2 {
                policy_id: state.policy_id,
                facility_id: state.facility_id,
                facility_position_binding_id: state.facility_position_binding_id,
                dealer_state_account_id: id(accounts[2].key),
                facility_position_pre_id: state.facility_position_id,
                facility_position_leased_id: leased_position_id,
                lease_account_id: id(accounts[35].key),
                market_instance_v2_id: policy.market_instance_v2_id,
                epoch_id: epoch.epoch_id,
                epoch_binding_account_id: id(accounts[6].key),
                settlement_candidate_id: selection.settlement_candidate_id,
                settlement_root_account_id: id(accounts[18].key),
                covered_dealer_selection_account_id: id(accounts[34].key),
                covered_dealer_selection_id: selection_id,
                upstream_economic_candidate_id: selection.upstream_economic_candidate_id,
                quote_id: selection.quote_semantics_id,
                dealer_leg_verdict_id: selection.settlement_candidate_id,
                curve_price_certificate_id: selection.curve_price_certificate_id,
                settlement_rows_root: selection.settlement_witness_digest,
                settlement_pot_id: id(accounts[36].key),
                funded_dependencies_id: dependency_id,
                runtime_liveness_policy_id: runtime_binding.runtime_policy_id(),
                runtime_liveness_binding_digest: runtime_binding_digest,
                dealer_liveness_schedule_id: schedule_id,
                select_begin_receipt_account_id: authorization.receipt_account_id,
                select_begin_receipt_semantic_id: authorization.receipt_semantic_id,
                select_begin_receipt_program_id: authorization.receipt_program_id,
                selected_fee_binding_digest: selected_fee_digest,
                selected_fee_record_account_id: selected_fee.fee_record_account_id,
                selected_fee_record_semantic_id: selected_fee.fee_record_semantic_id,
                fee_revenue_policy_id: selected_fee.revenue_policy_id,
                pre_generation: state.generation,
                post_generation,
                created_slot: current_slot,
                collect_deadline_slot: selection.collect_deadline_slot,
                deliver_deadline_slot: selection.deliver_deadline_slot,
                outcome_count: selection.outcome_count,
                row_count: u16::from(selection.allocation_count),
                rent: child_rent(lease_principal, &accounts[35]),
            });
            let lease_id = lease.lease_id().map_err(dealer_fault)?;
            let pot = Box::new(SettlementPotV2 {
                policy_id: state.policy_id,
                facility_id: state.facility_id,
                facility_position_binding_id: state.facility_position_binding_id,
                lease_id,
                epoch_id: epoch.epoch_id,
                settlement_candidate_id: selection.settlement_candidate_id,
                aggregate_verdict_id: selection.settlement_candidate_id,
                curve_price_certificate_id: selection.curve_price_certificate_id,
                facility_position_pre_id: state.facility_position_id,
                facility_position_leased_id: leased_position_id,
                facility_position_post_id: terminal_position_id,
                settlement_rows_root: selection.settlement_witness_digest,
                funded_dependencies_id: dependency_id,
                runtime_liveness_binding_digest: runtime_binding_digest,
                dealer_liveness_schedule_id: schedule_id,
                selected_fee_binding_digest: selected_fee_digest,
                selected_fee_record_account_id: selected_fee.fee_record_account_id,
                phase: SettlementPotPhaseV1::Collecting,
                outcome_count: selection.outcome_count,
                pre_generation: state.generation,
                post_generation,
                row_count: u16::from(selection.allocation_count),
                collect_cursor: 0,
                deliver_cursor: 0,
                user_cash_in_atoms: selection
                    .allocations
                    .iter()
                    .take(usize::from(selection.allocation_count))
                    .try_fold(0u64, |total, row| {
                        total.checked_add(row.user_cash_in_atoms)
                    })
                    .ok_or(ClutchError::Arithmetic)?,
                user_cash_out_atoms: selection
                    .allocations
                    .iter()
                    .take(usize::from(selection.allocation_count))
                    .try_fold(0u64, |total, row| {
                        total.checked_add(row.user_cash_out_atoms)
                    })
                    .ok_or(ClutchError::Arithmetic)?,
                dealer_net_cash_in_atoms: selection.receipt.dealer_net_cash_in_atoms,
                dealer_net_cash_out_atoms: selection.receipt.dealer_net_cash_out_atoms,
                facility_buy_eggs: selection.trade.buy_from_users,
                facility_sell_eggs: selection.trade.sell_to_users,
                collected_user_cash_atoms: 0,
                collected_user_eggs: [0; clutch_dealer_runtime_contract::MAX_OUTCOMES],
                delivered_user_cash_atoms: 0,
                delivered_user_eggs: [0; clutch_dealer_runtime_contract::MAX_OUTCOMES],
                rent: child_rent(pot_principal, &accounts[36]),
            });
            let prepared = prepare_begin_covered_lease_pot_v4(
                &policy,
                &position_binding,
                &state,
                id(accounts[2].key),
                &dependency,
                &epoch,
                root,
                &selection,
                receipt.rent(),
                &lease,
                id(accounts[36].key),
                &pot,
                &schedule,
                &runtime_binding,
                &authorization,
                &selected_fee,
                market,
                facility_endpoint,
                &replay,
                replay_binding,
                current_slot,
            )
            .map_err(dealer_fault)?;

            let receipt_slot_bytes = receipt_slot.bytes();
            create_full_principal_pda(
                program_id,
                &accounts[0],
                &accounts[16],
                &accounts[39],
                &rent,
                DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
                &[
                    seeds::SEED_DEALER_ACTION_RECEIPT,
                    &receipt_slot_bytes,
                    &[receipt_bump],
                ],
            )?;
            create_full_principal_pda(
                program_id,
                &accounts[0],
                &accounts[34],
                &accounts[39],
                &rent,
                DEALER_COVERED_SELECTION_ACCOUNT_BYTES,
                &[
                    seeds::SEED_DEALER_COVERED_SELECTION,
                    &root_epoch_bytes,
                    &settlement_candidate_bytes,
                    &[selection_bump],
                ],
            )?;
            create_full_principal_pda(
                program_id,
                &accounts[0],
                &accounts[35],
                &accounts[39],
                &rent,
                DEALER_LEASE_V2_ACCOUNT_BYTES,
                &[
                    seeds::SEED_DEALER_LEASE_V2,
                    &facility_bytes,
                    &generation_bytes,
                    &[lease_bump],
                ],
            )?;
            create_full_principal_pda(
                program_id,
                &accounts[0],
                &accounts[36],
                &accounts[39],
                &rent,
                DEALER_SETTLEMENT_POT_V2_ACCOUNT_BYTES,
                &[
                    seeds::SEED_DEALER_POT_V2,
                    &facility_bytes,
                    &generation_bytes,
                    &[pot_bump],
                ],
            )?;
            let mut product_root_rebound =
                Box::new(MarketLifecycleRootAccountV2::decode_buffer());
            let mut product_link_rebound =
                Box::new(SeriesMarketLinkAccountV2::decode_buffer());
            let (state_v3, series_obligation, product_admission) =
                if let Some(series_admission_prewrite) = series_admission_prewrite.as_ref() {
                    let (product_link_after, product_admission) =
                        admit_series_dealer_obligation_v2(
                            program_id,
                            &accounts[48],
                            product_root,
                            &mut product_root_rebound,
                            &accounts[54],
                            product_link,
                            &registry_capability,
                            &accounts[55],
                            &accounts[56],
                            series_admission_prewrite,
                            &mut product_link_rebound,
                        )?;
                    let (observed_obligation_principal, observed_obligation_donation) =
                        create_full_principal_pda(
                            program_id,
                            &accounts[0],
                            &accounts[57],
                            &accounts[39],
                            &rent,
                            DEALER_SERIES_OBLIGATION_ACCOUNT_BYTES_V2,
                            &[
                                seeds::SEED_DEALER_SERIES_OBLIGATION,
                                &state.facility_id.bytes(),
                                &[series_obligation_bump],
                            ],
                        )?;
                    require(
                        observed_obligation_principal == series_obligation_principal
                            && observed_obligation_donation == series_obligation_donation_floor,
                        ClutchError::DealerPolicyRentMismatch,
                    )?;
                    fund_and_resize_program_account(
                        program_id,
                        &accounts[0],
                        &accounts[2],
                        &accounts[39],
                        state_upgrade_principal,
                        DEALER_STATE_V2_ACCOUNT_BYTES,
                        DEALER_STATE_V3_ACCOUNT_BYTES,
                    )?;
                    let product_admission_projection_id = product_admission
                        .product_admission_projection()
                        .id()
                        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
                    let series_obligation = DealerSeriesObligationBindingV2::new_live(
                        series_obligation_key,
                        series_admission_prewrite.owner_admission_receipt_id,
                        Id::from_bytes(product_admission_projection_id.bytes()),
                        Id::from_bytes(product_admission.link_semantic_before().bytes()),
                        Id::from_bytes(product_admission.link_semantic_after().bytes()),
                        product_admission.link_transition_sequence_after(),
                        series_admission_prewrite.rent,
                    )
                    .map_err(dealer_fault)?;
                    let state_v3 = DealerStateV3::promote_current(
                        prepared.dealer.state_after,
                        &series_obligation,
                        DeletableRentOwnerV1 {
                            payer: id(accounts[17].key),
                            neutral_sink: policy.neutral_sink,
                            refundable_principal: state_upgrade_principal,
                            donation_floor: 0,
                        },
                    )
                    .map_err(dealer_fault)?;
                    require(
                        product_link_after.authentication_id()
                            == product_admission.link_authentication_after(),
                        ClutchError::MismatchedState,
                    )?;
                    (state_v3, series_obligation, Some(product_admission))
                } else {
                    let existing = existing_series_admission
                        .ok_or(ClutchError::AuthorizationUnavailable)?;
                    let state_v3 = existing
                        .state
                        .with_base(prepared.dealer.state_after)
                        .map_err(dealer_fault)?;
                    (state_v3, existing.obligation, None)
                };
            apply_liveness_transition(
                &accounts[10],
                &accounts[0],
                &accounts[17],
                &liveness_transition,
            )?;
            write_dealer_body(
                &accounts[16],
                DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
                DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
                receipt_bump,
                &receipt,
            )?;
            write_dealer_body(
                &accounts[34],
                DEALER_COVERED_SELECTION_ACCOUNT_TAG,
                DEALER_COVERED_SELECTION_ACCOUNT_VERSION,
                selection_bump,
                selection.as_ref(),
            )?;
            write_dealer_body(
                &accounts[35],
                DEALER_LEASE_V2_ACCOUNT_TAG,
                DEALER_LEASE_V2_ACCOUNT_VERSION,
                lease_bump,
                lease.as_ref(),
            )?;
            write_dealer_body(
                &accounts[36],
                DEALER_SETTLEMENT_POT_V2_ACCOUNT_TAG,
                DEALER_SETTLEMENT_POT_V2_ACCOUNT_VERSION,
                pot_bump,
                pot.as_ref(),
            )?;
            accounts[3]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
                .copy_from_slice(
                    &prepared
                        .dealer
                        .transfer
                        .position_post()
                        .encode()
                        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
                );
            let _accepted_transfer = accept_dealer_asset_transfer_postwrite_v2(
                program_id,
                prepared.dealer.transfer.bundle(),
                &accounts[3],
                &accounts[36],
            )?;
            let state_bump = accounts[2].data.borrow()[2];
            write_dealer_body(
                &accounts[2],
                DEALER_STATE_V3_ACCOUNT_TAG,
                DEALER_STATE_V3_ACCOUNT_VERSION,
                state_bump,
                &state_v3,
            )?;
            if product_admission.is_some() {
                write_dealer_body(
                    &accounts[57],
                    DEALER_SERIES_OBLIGATION_ACCOUNT_TAG,
                    DEALER_SERIES_OBLIGATION_ACCOUNT_VERSION_V2,
                    series_obligation_bump,
                    &series_obligation,
                )?;
            }
            let observed_state = authenticate_dealer_state_v3(program_id, &accounts[2], true)?;
            let observed_obligation = authenticate_dealer_series_obligation_v2(
                program_id,
                &accounts[57],
                product_admission.is_some(),
            )?;
            require(
                observed_state.state() == &state_v3
                    && observed_obligation.binding() == &series_obligation,
                ClutchError::MismatchedState,
            )?;
            if let Some(product_admission) = product_admission.as_ref() {
                require(
                    product_admission.link_account() == *accounts[54].key
                        && product_admission.root_account() == *accounts[48].key
                        && product_admission.dealer_obligation_account() == *accounts[57].key
                        && product_admission.dealer_state_account() == *accounts[2].key
                        && product_admission.owner_admission_receipt_id().bytes()
                            == series_obligation.admission_owner_receipt_id.bytes()
                        && product_admission
                            .product_admission_projection()
                            .id()
                            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                            .bytes()
                            == series_obligation.admission_projection_id.bytes()
                        && product_admission.link_semantic_before().bytes()
                            == series_obligation.admission_link_pre_semantic_id.bytes()
                        && product_admission.link_semantic_after().bytes()
                            == series_obligation.admission_link_post_semantic_id.bytes()
                        && product_admission.link_transition_sequence_after()
                            == series_obligation.admission_link_transition_sequence,
                    ClutchError::MismatchedState,
                )?;
            } else {
                let mut product_link_post =
                    Box::new(SeriesMarketLinkAccountV2::decode_buffer());
                let rebound = authenticate_series_market_link_v2(
                    program_id,
                    &accounts[54],
                    untrusted_product_binding.series_plan_id,
                    untrusted_product_binding.ordinal,
                    untrusted_product_binding.market_instance_id,
                    untrusted_product_binding.generation,
                    *accounts[48].key,
                    false,
                    &mut product_link_post,
                )?;
                require(
                    rebound.authentication_id() == product_link.authentication_id(),
                    ClutchError::MismatchedState,
                )?;
            }
            write_dealer_body(
                &accounts[6],
                DEALER_EPOCH_BINDING_V2_ACCOUNT_TAG,
                DEALER_EPOCH_BINDING_V2_ACCOUNT_VERSION,
                epoch_bump,
                &prepared.dealer.epoch_after,
            )?;
            prepared
                .dealer
                .replay
                .replay_post()
                .encode_into(&mut accounts[4].data.borrow_mut())
                .map_err(dealer_fault)?;
            prepared
                .settlement_root_after
                .encode(&mut accounts[18].data.borrow_mut())?;
            Ok(())
        },
    )
}

#[inline(never)]
fn create_lp_page(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    let payload = DealerRuntimePayloadV1::decode(
        DealerFacilityAction::CreateLpPage,
        payload_bytes,
    )
    .map_err(dealer_fault)?;
    let first_page = payload.page_ordinal == 0;
    let expected_count = if first_page {
        CREATE_FIRST_LP_PAGE_ACCOUNT_COUNT
    } else {
        CREATE_NEXT_LP_PAGE_ACCOUNT_COUNT
    };
    require_count(accounts, expected_count)?;
    require(
        sequence == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    let page_index = if first_page { 16 } else { 17 };
    let payer_index = if first_page { 17 } else { 18 };
    let rent_index = if first_page { 18 } else { 19 };
    let system_index = if first_page { 19 } else { 20 };
    require_aliases(accounts, (0, payer_index))?;

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let state = authenticate_state(program_id, &accounts[2])?;
    require(
        state.policy_id.bytes() == policy_id && state.generation == payload.expected_generation,
        ClutchError::MismatchedState,
    )?;
    let (binding, _position, replay, replay_binding) = authenticate_position_and_replay(
        program_id,
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &policy,
        &state,
        false,
    )?;
    require(
        replay.next_transition_ordinal() == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let dependency = authenticate_dependency(program_id, &accounts[5], state.facility_id)?;
    let schedule = authenticate_schedule(program_id, &accounts[6])?;
    let (runtime_policy, runtime_states, runtime_binding) = authenticate_runtime_bundle(
        program_id,
        &dependency,
        &accounts[7],
        &accounts[8..15],
        DealerLivenessCompartmentV1::Clearing.index(),
    )?;
    validate_runtime_dependency_join(
        program_id,
        &accounts[2],
        &policy,
        &state,
        &binding,
        &dependency,
        &schedule,
        runtime_policy,
        runtime_binding,
    )?;
    let clearing = runtime_states[DealerLivenessCompartmentV1::Clearing.index()];
    require(
        clearing.identity.payer.bytes() == accounts[payer_index].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;

    let rent = read_rent(&accounts[rent_index])?;
    require_system_program(&accounts[system_index])?;
    require(
        accounts[6].lamports() >= rent.minimum_balance(DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES)?,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    require_creatable(&accounts[15])?;
    require_creatable(&accounts[page_index])?;
    let receipt_principal = rent.minimum_balance(DEALER_ACTION_RECEIPT_ACCOUNT_BYTES)?;
    let receipt = DealerActionReceiptV1 {
        policy_id: state.policy_id,
        facility_id: state.facility_id,
        dealer_state_account_id: id(accounts[2].key),
        liveness_schedule_id: schedule.schedule_id().map_err(dealer_fault)?.untyped(),
        runtime_policy_id: runtime_binding.runtime_policy_id(),
        runtime_account_id: runtime_binding.account_id(DealerLivenessCompartmentV1::Clearing),
        runtime_owner: runtime_binding.owner(DealerLivenessCompartmentV1::Clearing),
        quote_schedule_id: runtime_binding
            .quote_schedule_id(DealerLivenessCompartmentV1::Clearing),
        receipt_account_id: id(accounts[15].key),
        receipt_program_id: id(program_id),
        keeper: id(accounts[0].key),
        replay_account_id: id(accounts[4].key),
        action: DealerRuntimeActionV1::CreateLpPage,
        compartment: DealerLivenessCompartmentV1::Clearing,
        runtime_generation: runtime_binding.generation(DealerLivenessCompartmentV1::Clearing),
        facility_generation: state.generation,
        call_ordinal: payload.liveness_call_ordinal,
        call_ceiling_lamports: schedule.reward_lamports
            [DealerRuntimeActionV1::CreateLpPage as usize],
        keeper_payment_lamports: payload.keeper_payment_lamports,
        expected_replay_ordinal: payload.expected_replay_ordinal,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: receipt_principal,
            donation_floor: accounts[15].lamports(),
        },
    };
    let receipt_slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
    let (receipt_address, receipt_bump) =
        seeds::dealer_action_receipt_pda(program_id, &receipt_slot.bytes());
    expect_pda(accounts[15].key, (receipt_address, receipt_bump), None)?;
    receipt
        .validate_against(&schedule, &runtime_binding)
        .map_err(dealer_fault)?;
    let authorization = receipt
        .authorization(&schedule, &runtime_binding, &clearing)
        .map_err(dealer_fault)?;
    let liveness_transition = plan_liveness_spend_absorbing_donation(
        program_id,
        &accounts[7],
        &accounts[10],
        clearing,
        receipt.runtime_transition_intent().map_err(dealer_fault)?,
        receipt
            .runtime_receipt_observation()
            .map_err(dealer_fault)?,
    )?;

    let (page_address, page_bump) = seeds::dealer_lp_page_v2_pda(
        program_id,
        &state.facility_id.bytes(),
        payload.page_ordinal,
    );
    expect_pda(accounts[page_index].key, (page_address, page_bump), None)?;
    let page_principal = rent.minimum_balance(DEALER_LP_PAGE_V2_ACCOUNT_BYTES)?;
    let page_rent = DeletableRentOwnerV1 {
        payer: id(accounts[0].key),
        neutral_sink: policy.neutral_sink,
        refundable_principal: page_principal,
        donation_floor: accounts[page_index].lamports(),
    };
    let (page, state_after, replay_after, previous_page_after) = if first_page {
        let prepared = prepare_first_lp_page_v2(
            &policy,
            &state,
            id(accounts[2].key),
            id(accounts[page_index].key),
            page_rent,
            &dependency,
            &schedule,
            &runtime_binding,
            &authorization,
            &replay,
            replay_binding,
        )
        .map_err(dealer_fault)?;
        (
            prepared.page,
            prepared.state_after,
            prepared.replay.replay_post(),
            None,
        )
    } else {
        let previous_page = authenticate_lp_page(program_id, &accounts[16])?;
        let prepared = prepare_next_lp_page_v2(
            &policy,
            &state,
            id(accounts[2].key),
            id(accounts[16].key),
            &previous_page,
            id(accounts[page_index].key),
            page_rent,
            &dependency,
            &schedule,
            &runtime_binding,
            &authorization,
            &replay,
            replay_binding,
        )
        .map_err(dealer_fault)?;
        (
            prepared.page,
            prepared.state_after,
            prepared.replay.replay_post(),
            Some(prepared.previous_page_after),
        )
    };
    require(page.page_ordinal == payload.page_ordinal, ClutchError::MismatchedState)?;

    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[15],
        &accounts[system_index],
        &rent,
        DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_ACTION_RECEIPT,
            &receipt_slot.bytes(),
            &[receipt_bump],
        ],
    )?;
    let page_ordinal_bytes = payload.page_ordinal.to_le_bytes();
    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[page_index],
        &accounts[system_index],
        &rent,
        DEALER_LP_PAGE_V2_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_LP_PAGE_V2,
            &state.facility_id.bytes(),
            &page_ordinal_bytes,
            &[page_bump],
        ],
    )?;
    apply_liveness_transition(
        &accounts[10],
        &accounts[0],
        &accounts[payer_index],
        &liveness_transition,
    )?;
    write_dealer_body(
        &accounts[15],
        DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
        DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
        receipt_bump,
        &receipt,
    )?;
    write_dealer_body(
        &accounts[page_index],
        DEALER_LP_PAGE_V2_ACCOUNT_TAG,
        DEALER_LP_PAGE_V2_ACCOUNT_VERSION,
        page_bump,
        &page,
    )?;
    if let Some(previous_page_after) = previous_page_after {
        let previous_bump = accounts[16].data.borrow()[2];
        write_dealer_body(
            &accounts[16],
            DEALER_LP_PAGE_V2_ACCOUNT_TAG,
            DEALER_LP_PAGE_V2_ACCOUNT_VERSION,
            previous_bump,
            &previous_page_after,
        )?;
    }
    let state_bump = accounts[2].data.borrow()[2];
    write_dealer_body(
        &accounts[2],
        DEALER_STATE_V2_ACCOUNT_TAG,
        DEALER_STATE_V2_ACCOUNT_VERSION,
        state_bump,
        &state_after,
    )?;
    replay_after
        .encode_into(&mut accounts[4].data.borrow_mut())
        .map_err(dealer_fault)
}

#[inline(never)]
fn transfer_lp_funding(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: DealerFacilityAction,
    payload_bytes: &[u8],
) -> Outcome<()> {
    require_count(accounts, LP_TRANSFER_ACCOUNT_COUNT)?;
    let payload = DealerRuntimePayloadV1::decode(action, payload_bytes).map_err(dealer_fault)?;
    require(
        sequence == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    require_signer(&accounts[0])?;
    require(!accounts[0].is_writable, ClutchError::UnexpectedWritable)?;
    require_aliases(accounts, (accounts.len(), accounts.len()))?;

    let runtime_action = match action {
        DealerFacilityAction::Contribute => DealerRuntimeActionV1::Contribute,
        DealerFacilityAction::WithdrawFunding => DealerRuntimeActionV1::WithdrawFunding,
        _ => return Err(ClutchError::UnsupportedInstruction.into()),
    };
    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let state = authenticate_state(program_id, &accounts[2])?;
    require(
        state.policy_id.bytes() == policy_id && state.generation == payload.expected_generation,
        ClutchError::MismatchedState,
    )?;
    let (binding, facility_observation, replay, replay_binding) =
        authenticate_position_and_replay(
            program_id,
            &accounts[2],
            &accounts[3],
            &accounts[4],
            &policy,
            &state,
            true,
        )?;
    require(
        replay.next_transition_ordinal() == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let page = authenticate_lp_page(program_id, &accounts[6])?;
    require(
        page.page_ordinal == payload.page_ordinal,
        ClutchError::MismatchedState,
    )?;
    let (collateral_value, market) = authenticate_dealer_collateral_value_v2(
        program_id,
        &policy,
        Some(&binding),
        DealerCollateralAuthorityAccountsV2 {
            realm: &accounts[7],
            profile: &accounts[8],
            policy: &accounts[9],
            token_program: &accounts[10],
            token_programdata: &accounts[11],
            market_binding: &accounts[12],
            market_runtime: &accounts[13],
            market_instance: &accounts[14],
            hoard: &accounts[15],
            claim_ledger: &accounts[16],
        },
    )?;
    let lp_replay_sequence = current_general_replay_sequence_v1(&accounts[17])?;
    let lp_authority = authenticate_general_position_replay_v2(
        program_id,
        collateral_value.liabilities.bound,
        &accounts[12],
        &accounts[13],
        &accounts[5],
        &accounts[17],
        accounts[0].key.to_bytes(),
        lp_replay_sequence,
    )?;
    let lp_position = lp_authority.position.semantic;
    let lp_projection = lp_authority.projection;
    let lp_owner = Id::from_bytes(lp_position.owner().bytes());
    let transfer = prepare_dealer_lp_share_transfer_v2(
        runtime_action,
        &policy,
        market,
        lp_owner,
        payload.share_delta,
        DealerTransferPositionV3::General {
            account_id: id(accounts[5].key),
            position: lp_projection,
        },
        DealerTransferPositionV3::Facility {
            account_id: id(accounts[3].key),
            position: facility_observation.projection,
        },
    )
    .map_err(dealer_fault)?;
    let transfer_bundle = transfer.bundle();
    let general_kind = match runtime_action {
        DealerRuntimeActionV1::Contribute => GeneralReplayTransitionKindV1::DealerLpContribute,
        DealerRuntimeActionV1::WithdrawFunding => {
            GeneralReplayTransitionKindV1::DealerLpWithdraw
        }
        _ => return Err(ClutchError::UnsupportedInstruction.into()),
    };
    let lp_general_post = match runtime_action {
        DealerRuntimeActionV1::Contribute => transfer.source_post(),
        DealerRuntimeActionV1::WithdrawFunding => transfer.destination_post(),
        _ => return Err(ClutchError::UnsupportedInstruction.into()),
    };
    let transfer_bundle_id = transfer_bundle.bundle_id().map_err(dealer_fault)?;
    let lp_general_replay = prepare_dealer_general_replay_v2(
        lp_authority,
        lp_general_post,
        general_kind,
        transfer_bundle_id,
        market.collateral_value_receipt_id,
        transfer_bundle_id,
    )?;
    let transfer = bind_dealer_general_position_transfer_v3(transfer, &lp_general_replay)
        .map_err(dealer_fault)?;
    let (page_after, state_after, replay_after, facility_post, lp_post) = match action {
        DealerFacilityAction::Contribute => {
            let prepared = prepare_lp_contribution_v2(
                &policy,
                &state,
                id(accounts[2].key),
                &page,
                lp_owner,
                payload.share_delta,
                transfer,
                &replay,
                replay_binding,
            )
            .map_err(dealer_fault)?;
            (
                prepared.page_after,
                prepared.state_after,
                prepared.replay.replay_post(),
                prepared.transfer.transfer().destination_post(),
                prepared.transfer.transfer().source_post(),
            )
        }
        DealerFacilityAction::WithdrawFunding => {
            let prepared = prepare_lp_withdrawal_v2(
                &policy,
                &state,
                id(accounts[2].key),
                &page,
                lp_owner,
                payload.share_delta,
                transfer,
                &replay,
                replay_binding,
            )
            .map_err(dealer_fault)?;
            (
                prepared.page_after,
                prepared.state_after,
                prepared.replay.replay_post(),
                prepared.transfer.transfer().source_post(),
                prepared.transfer.transfer().destination_post(),
            )
        }
        _ => return Err(ClutchError::UnsupportedInstruction.into()),
    };

    accounts[3]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &facility_post
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    accounts[5]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &lp_post
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    let _accepted_transfer = accept_dealer_asset_transfer_postwrite_v2(
        program_id,
        transfer_bundle,
        &accounts[3],
        &accounts[5],
    )?;
    let observed_general_replay =
        write_and_accept_general_replay_v1(program_id, &accounts[17], &lp_general_replay)?;
    require(
        observed_general_replay == transfer.general_replay_post_semantic_id(),
        ClutchError::Replay,
    )?;
    let page_bump = accounts[6].data.borrow()[2];
    write_dealer_body(
        &accounts[6],
        DEALER_LP_PAGE_V2_ACCOUNT_TAG,
        DEALER_LP_PAGE_V2_ACCOUNT_VERSION,
        page_bump,
        &page_after,
    )?;
    let state_bump = accounts[2].data.borrow()[2];
    write_dealer_body(
        &accounts[2],
        DEALER_STATE_V2_ACCOUNT_TAG,
        DEALER_STATE_V2_ACCOUNT_VERSION,
        state_bump,
        &state_after,
    )?;
    replay_after
        .encode_into(&mut accounts[4].data.borrow_mut())
        .map_err(dealer_fault)
}

#[inline(never)]
fn activate_facility(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    require_count(accounts, ACTIVATE_ACCOUNT_COUNT)?;
    let payload = DealerRuntimePayloadV1::decode(DealerFacilityAction::Activate, payload_bytes)
        .map_err(dealer_fault)?;
    require(
        sequence == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    require_aliases(accounts, (0, 17))?;

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let state = authenticate_state(program_id, &accounts[2])?;
    require(
        state.policy_id.bytes() == policy_id && state.generation == payload.expected_generation,
        ClutchError::MismatchedState,
    )?;
    let (binding, position, replay, replay_binding) = authenticate_position_and_replay(
        program_id,
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &policy,
        &state,
        false,
    )?;
    require(
        replay.next_transition_ordinal() == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let dependency = authenticate_dependency(program_id, &accounts[5], state.facility_id)?;
    let schedule = authenticate_schedule(program_id, &accounts[6])?;
    let (runtime_policy, runtime_states, runtime_binding) = authenticate_runtime_bundle(
        program_id,
        &dependency,
        &accounts[7],
        &accounts[8..15],
        DealerLivenessCompartmentV1::Clearing.index(),
    )?;
    validate_runtime_dependency_join(
        program_id,
        &accounts[2],
        &policy,
        &state,
        &binding,
        &dependency,
        &schedule,
        runtime_policy,
        runtime_binding,
    )?;
    let clearing = runtime_states[DealerLivenessCompartmentV1::Clearing.index()];
    require(
        clearing.identity.payer.bytes() == accounts[17].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let tail = authenticate_lp_page(program_id, &accounts[16])?;
    let current_slot = read_clock_slot(&accounts[18])?;
    let rent = read_rent(&accounts[19])?;
    require_system_program(&accounts[20])?;
    require(
        accounts[6].lamports() >= rent.minimum_balance(DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES)?,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    require_creatable(&accounts[15])?;
    let receipt_principal = rent.minimum_balance(DEALER_ACTION_RECEIPT_ACCOUNT_BYTES)?;
    let receipt = DealerActionReceiptV1 {
        policy_id: state.policy_id,
        facility_id: state.facility_id,
        dealer_state_account_id: id(accounts[2].key),
        liveness_schedule_id: schedule.schedule_id().map_err(dealer_fault)?.untyped(),
        runtime_policy_id: runtime_binding.runtime_policy_id(),
        runtime_account_id: runtime_binding.account_id(DealerLivenessCompartmentV1::Clearing),
        runtime_owner: runtime_binding.owner(DealerLivenessCompartmentV1::Clearing),
        quote_schedule_id: runtime_binding
            .quote_schedule_id(DealerLivenessCompartmentV1::Clearing),
        receipt_account_id: id(accounts[15].key),
        receipt_program_id: id(program_id),
        keeper: id(accounts[0].key),
        replay_account_id: id(accounts[4].key),
        action: DealerRuntimeActionV1::Activate,
        compartment: DealerLivenessCompartmentV1::Clearing,
        runtime_generation: runtime_binding.generation(DealerLivenessCompartmentV1::Clearing),
        facility_generation: state.generation,
        call_ordinal: payload.liveness_call_ordinal,
        call_ceiling_lamports: schedule.reward_lamports
            [DealerRuntimeActionV1::Activate as usize],
        keeper_payment_lamports: payload.keeper_payment_lamports,
        expected_replay_ordinal: payload.expected_replay_ordinal,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: receipt_principal,
            donation_floor: accounts[15].lamports(),
        },
    };
    let receipt_slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
    let (receipt_address, receipt_bump) =
        seeds::dealer_action_receipt_pda(program_id, &receipt_slot.bytes());
    expect_pda(accounts[15].key, (receipt_address, receipt_bump), None)?;
    receipt
        .validate_against(&schedule, &runtime_binding)
        .map_err(dealer_fault)?;
    let authorization = receipt
        .authorization(&schedule, &runtime_binding, &clearing)
        .map_err(dealer_fault)?;
    let liveness_transition = plan_liveness_spend_absorbing_donation(
        program_id,
        &accounts[7],
        &accounts[10],
        clearing,
        receipt.runtime_transition_intent().map_err(dealer_fault)?,
        receipt
            .runtime_receipt_observation()
            .map_err(dealer_fault)?,
    )?;
    let prepared = prepare_activate_dealer_v3(
        &policy,
        &binding,
        &state,
        id(accounts[2].key),
        &dependency,
        &schedule,
        &runtime_binding,
        &authorization,
        current_slot,
        &tail,
        &position,
        &replay,
        replay_binding,
    )
    .map_err(dealer_fault)?;

    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[15],
        &accounts[20],
        &rent,
        DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_ACTION_RECEIPT,
            &receipt_slot.bytes(),
            &[receipt_bump],
        ],
    )?;
    apply_liveness_transition(
        &accounts[10],
        &accounts[0],
        &accounts[17],
        &liveness_transition,
    )?;
    write_dealer_body(
        &accounts[15],
        DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
        DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
        receipt_bump,
        &receipt,
    )?;
    let tail_bump = accounts[16].data.borrow()[2];
    write_dealer_body(
        &accounts[16],
        DEALER_LP_PAGE_V2_ACCOUNT_TAG,
        DEALER_LP_PAGE_V2_ACCOUNT_VERSION,
        tail_bump,
        &prepared.tail_page_after,
    )?;
    let state_bump = accounts[2].data.borrow()[2];
    write_dealer_body(
        &accounts[2],
        DEALER_STATE_V2_ACCOUNT_TAG,
        DEALER_STATE_V2_ACCOUNT_VERSION,
        state_bump,
        &prepared.state_after,
    )?;
    prepared
        .replay
        .replay_post()
        .encode_into(&mut accounts[4].data.borrow_mut())
        .map_err(dealer_fault)
}

#[inline(never)]
fn cancel_stale_funding(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    require_count(accounts, CANCEL_FUNDING_ACCOUNT_COUNT)?;
    let payload =
        DealerRuntimePayloadV1::decode(DealerFacilityAction::CancelFunding, payload_bytes)
            .map_err(dealer_fault)?;
    require(
        sequence == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    require_aliases(accounts, (0, 16))?;

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let state = authenticate_state(program_id, &accounts[2])?;
    require(
        state.policy_id.bytes() == policy_id && state.generation == payload.expected_generation,
        ClutchError::MismatchedState,
    )?;
    let (binding, position, replay, replay_binding) = authenticate_position_and_replay(
        program_id,
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &policy,
        &state,
        false,
    )?;
    require(
        replay.next_transition_ordinal() == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let dependency = authenticate_dependency(program_id, &accounts[5], state.facility_id)?;
    let schedule = authenticate_schedule(program_id, &accounts[6])?;
    let (runtime_policy, runtime_states, runtime_binding) = authenticate_runtime_bundle(
        program_id,
        &dependency,
        &accounts[7],
        &accounts[8..15],
        DealerLivenessCompartmentV1::Recovery.index(),
    )?;
    validate_runtime_dependency_join(
        program_id,
        &accounts[2],
        &policy,
        &state,
        &binding,
        &dependency,
        &schedule,
        runtime_policy,
        runtime_binding,
    )?;
    let recovery = runtime_states[DealerLivenessCompartmentV1::Recovery.index()];
    require(
        recovery.identity.payer.bytes() == accounts[16].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let current_slot = read_clock_slot(&accounts[17])?;
    let rent = read_rent(&accounts[18])?;
    require_system_program(&accounts[19])?;
    require(
        accounts[6].lamports() >= rent.minimum_balance(DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES)?,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    require_creatable(&accounts[15])?;
    let receipt_principal = rent.minimum_balance(DEALER_ACTION_RECEIPT_ACCOUNT_BYTES)?;
    let receipt = DealerActionReceiptV1 {
        policy_id: state.policy_id,
        facility_id: state.facility_id,
        dealer_state_account_id: id(accounts[2].key),
        liveness_schedule_id: schedule.schedule_id().map_err(dealer_fault)?.untyped(),
        runtime_policy_id: runtime_binding.runtime_policy_id(),
        runtime_account_id: runtime_binding.account_id(DealerLivenessCompartmentV1::Recovery),
        runtime_owner: runtime_binding.owner(DealerLivenessCompartmentV1::Recovery),
        quote_schedule_id: runtime_binding
            .quote_schedule_id(DealerLivenessCompartmentV1::Recovery),
        receipt_account_id: id(accounts[15].key),
        receipt_program_id: id(program_id),
        keeper: id(accounts[0].key),
        replay_account_id: id(accounts[4].key),
        action: DealerRuntimeActionV1::CancelFunding,
        compartment: DealerLivenessCompartmentV1::Recovery,
        runtime_generation: runtime_binding.generation(DealerLivenessCompartmentV1::Recovery),
        facility_generation: state.generation,
        call_ordinal: payload.liveness_call_ordinal,
        call_ceiling_lamports: schedule.reward_lamports
            [DealerRuntimeActionV1::CancelFunding as usize],
        keeper_payment_lamports: payload.keeper_payment_lamports,
        expected_replay_ordinal: payload.expected_replay_ordinal,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: receipt_principal,
            donation_floor: accounts[15].lamports(),
        },
    };
    let receipt_slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
    let (receipt_address, receipt_bump) =
        seeds::dealer_action_receipt_pda(program_id, &receipt_slot.bytes());
    expect_pda(accounts[15].key, (receipt_address, receipt_bump), None)?;
    receipt
        .validate_against(&schedule, &runtime_binding)
        .map_err(dealer_fault)?;
    let authorization = receipt
        .authorization(&schedule, &runtime_binding, &recovery)
        .map_err(dealer_fault)?;
    let liveness_transition = plan_liveness_spend_absorbing_donation(
        program_id,
        &accounts[7],
        &accounts[14],
        recovery,
        receipt.runtime_transition_intent().map_err(dealer_fault)?,
        receipt
            .runtime_receipt_observation()
            .map_err(dealer_fault)?,
    )?;
    let prepared = prepare_cancel_stale_funding_v3(
        &policy,
        &binding,
        &state,
        id(accounts[2].key),
        &dependency,
        &schedule,
        &runtime_binding,
        &authorization,
        current_slot,
        &position,
        &replay,
        replay_binding,
    )
    .map_err(dealer_fault)?;

    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[15],
        &accounts[19],
        &rent,
        DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_ACTION_RECEIPT,
            &receipt_slot.bytes(),
            &[receipt_bump],
        ],
    )?;
    apply_liveness_transition(
        &accounts[14],
        &accounts[0],
        &accounts[16],
        &liveness_transition,
    )?;
    write_dealer_body(
        &accounts[15],
        DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
        DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
        receipt_bump,
        &receipt,
    )?;
    let state_bump = accounts[2].data.borrow()[2];
    write_dealer_body(
        &accounts[2],
        DEALER_STATE_V2_ACCOUNT_TAG,
        DEALER_STATE_V2_ACCOUNT_VERSION,
        state_bump,
        &prepared.state_after,
    )?;
    prepared
        .replay
        .replay_post()
        .encode_into(&mut accounts[4].data.borrow_mut())
        .map_err(dealer_fault)
}

#[inline(never)]
fn refund_cancelled_sponsor(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    require_count(accounts, REFUND_CANCELLED_SPONSOR_ACCOUNT_COUNT)?;
    let payload = DealerRuntimePayloadV1::decode(
        DealerFacilityAction::RefundCancelledSponsor,
        payload_bytes,
    )
    .map_err(dealer_fault)?;
    require(
        sequence == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    require_aliases(accounts, (0, 17))?;

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let state = authenticate_state(program_id, &accounts[2])?;
    require(
        state.policy_id.bytes() == policy_id && state.generation == payload.expected_generation,
        ClutchError::MismatchedState,
    )?;
    let (binding, position, replay, replay_binding) = authenticate_position_and_replay(
        program_id,
        &accounts[2],
        &accounts[3],
        &accounts[5],
        &policy,
        &state,
        true,
    )?;
    require(
        replay.next_transition_ordinal() == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let dependency = authenticate_dependency(program_id, &accounts[6], state.facility_id)?;
    let schedule = authenticate_schedule(program_id, &accounts[7])?;
    let (runtime_policy, runtime_states, runtime_binding) = authenticate_runtime_bundle(
        program_id,
        &dependency,
        &accounts[8],
        &accounts[9..16],
        DealerLivenessCompartmentV1::Recovery.index(),
    )?;
    validate_runtime_dependency_join(
        program_id,
        &accounts[2],
        &policy,
        &state,
        &binding,
        &dependency,
        &schedule,
        runtime_policy,
        runtime_binding,
    )?;
    let recovery = runtime_states[DealerLivenessCompartmentV1::Recovery.index()];
    require(
        recovery.identity.payer.bytes() == accounts[17].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let rent = read_rent(&accounts[18])?;
    require_system_program(&accounts[19])?;
    require(
        accounts[7].lamports() >= rent.minimum_balance(DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES)?,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    require_creatable(&accounts[16])?;
    let receipt_principal = rent.minimum_balance(DEALER_ACTION_RECEIPT_ACCOUNT_BYTES)?;
    let receipt = DealerActionReceiptV1 {
        policy_id: state.policy_id,
        facility_id: state.facility_id,
        dealer_state_account_id: id(accounts[2].key),
        liveness_schedule_id: schedule.schedule_id().map_err(dealer_fault)?.untyped(),
        runtime_policy_id: runtime_binding.runtime_policy_id(),
        runtime_account_id: runtime_binding.account_id(DealerLivenessCompartmentV1::Recovery),
        runtime_owner: runtime_binding.owner(DealerLivenessCompartmentV1::Recovery),
        quote_schedule_id: runtime_binding
            .quote_schedule_id(DealerLivenessCompartmentV1::Recovery),
        receipt_account_id: id(accounts[16].key),
        receipt_program_id: id(program_id),
        keeper: id(accounts[0].key),
        replay_account_id: id(accounts[5].key),
        action: DealerRuntimeActionV1::RefundCancelledSponsor,
        compartment: DealerLivenessCompartmentV1::Recovery,
        runtime_generation: runtime_binding.generation(DealerLivenessCompartmentV1::Recovery),
        facility_generation: state.generation,
        call_ordinal: payload.liveness_call_ordinal,
        call_ceiling_lamports: schedule.reward_lamports
            [DealerRuntimeActionV1::RefundCancelledSponsor as usize],
        keeper_payment_lamports: payload.keeper_payment_lamports,
        expected_replay_ordinal: payload.expected_replay_ordinal,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: receipt_principal,
            donation_floor: accounts[16].lamports(),
        },
    };
    let receipt_slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
    let (receipt_address, receipt_bump) =
        seeds::dealer_action_receipt_pda(program_id, &receipt_slot.bytes());
    expect_pda(accounts[16].key, (receipt_address, receipt_bump), None)?;
    receipt
        .validate_against(&schedule, &runtime_binding)
        .map_err(dealer_fault)?;
    let authorization = receipt
        .authorization(&schedule, &runtime_binding, &recovery)
        .map_err(dealer_fault)?;
    let liveness_transition = plan_liveness_spend_absorbing_donation(
        program_id,
        &accounts[8],
        &accounts[15],
        recovery,
        receipt.runtime_transition_intent().map_err(dealer_fault)?,
        receipt
            .runtime_receipt_observation()
            .map_err(dealer_fault)?,
    )?;
    let (collateral_value, market) = authenticate_dealer_collateral_value_v2(
        program_id,
        &policy,
        Some(&binding),
        DealerCollateralAuthorityAccountsV2 {
            realm: &accounts[20],
            profile: &accounts[21],
            policy: &accounts[22],
            token_program: &accounts[23],
            token_programdata: &accounts[24],
            market_binding: &accounts[25],
            market_runtime: &accounts[26],
            market_instance: &accounts[27],
            hoard: &accounts[28],
            claim_ledger: &accounts[29],
        },
    )?;
    let refund_replay_sequence = current_general_replay_sequence_v1(&accounts[30])?;
    let refund_authority = authenticate_general_position_replay_v2(
        program_id,
        collateral_value.liabilities.bound,
        &accounts[25],
        &accounts[26],
        &accounts[4],
        &accounts[30],
        state.sponsor_refund_recipient.bytes(),
        refund_replay_sequence,
    )?;
    let refund_position = refund_authority.position.semantic;
    let refund_projection = refund_authority.projection;
    let transfer = prepare_dealer_sponsor_refund_transfer_v2(
        market,
        state.sponsor_refund_recipient,
        state.sponsor_capital_atoms,
        DealerTransferPositionV3::Facility {
            account_id: id(accounts[3].key),
            position: position.projection,
        },
        DealerTransferPositionV3::General {
            account_id: id(accounts[4].key),
            position: refund_projection,
        },
    )
    .map_err(dealer_fault)?;
    require(
        Id::from_bytes(refund_position.owner().bytes()) == state.sponsor_refund_recipient,
        ClutchError::MismatchedState,
    )?;
    let transfer_bundle_id = transfer.bundle().bundle_id().map_err(dealer_fault)?;
    let refund_general_replay = prepare_dealer_general_replay_v2(
        refund_authority,
        transfer.destination_post(),
        GeneralReplayTransitionKindV1::DealerSponsorRefund,
        transfer_bundle_id,
        market.collateral_value_receipt_id,
        authorization.receipt_semantic_id,
    )?;
    let transfer = bind_dealer_general_position_transfer_v3(transfer, &refund_general_replay)
        .map_err(dealer_fault)?;
    let prepared = prepare_refund_cancelled_sponsor_v3(
        &policy,
        &binding,
        &state,
        id(accounts[2].key),
        &dependency,
        &schedule,
        &runtime_binding,
        &authorization,
        &position,
        transfer,
        &replay,
        replay_binding,
    )
    .map_err(dealer_fault)?;

    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[16],
        &accounts[19],
        &rent,
        DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_ACTION_RECEIPT,
            &receipt_slot.bytes(),
            &[receipt_bump],
        ],
    )?;
    apply_liveness_transition(
        &accounts[15],
        &accounts[0],
        &accounts[17],
        &liveness_transition,
    )?;
    accounts[3]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &prepared
                .transfer
                .transfer()
                .source_post()
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    accounts[4]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &prepared
                .transfer
                .transfer()
                .destination_post()
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    let _accepted_transfer = accept_dealer_asset_transfer_postwrite_v2(
        program_id,
        prepared.transfer.transfer().bundle(),
        &accounts[3],
        &accounts[4],
    )?;
    let observed_general_replay =
        write_and_accept_general_replay_v1(program_id, &accounts[30], &refund_general_replay)?;
    require(
        observed_general_replay == prepared.transfer.general_replay_post_semantic_id(),
        ClutchError::Replay,
    )?;
    write_dealer_body(
        &accounts[16],
        DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
        DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
        receipt_bump,
        &receipt,
    )?;
    let state_bump = accounts[2].data.borrow()[2];
    write_dealer_body(
        &accounts[2],
        DEALER_STATE_V2_ACCOUNT_TAG,
        DEALER_STATE_V2_ACCOUNT_VERSION,
        state_bump,
        &prepared.state_after,
    )?;
    prepared
        .replay
        .replay_post()
        .encode_into(&mut accounts[5].data.borrow_mut())
        .map_err(dealer_fault)
}

#[inline(never)]
fn collect_or_deliver_row(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: DealerFacilityAction,
    payload_bytes: &[u8],
) -> Outcome<()> {
    require(
        matches!(action, DealerFacilityAction::Collect | DealerFacilityAction::Deliver),
        ClutchError::UnsupportedInstruction,
    )?;
    let payload = DealerRuntimePayloadV1::decode(action, payload_bytes).map_err(dealer_fault)?;
    let expected_count = COLLECT_DELIVER_FIXED_ACCOUNT_COUNT
        .checked_add(usize::from(payload.book_page_count))
        .ok_or(ClutchError::Arithmetic)?;
    require_count(accounts, expected_count)?;
    require(
        sequence == payload.expected_replay_ordinal && payload.row_count == 1,
        ClutchError::Replay,
    )?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    require_aliases(accounts, (0, 17))?;

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let authenticated_state = authenticate_dealer_state_v3(program_id, &accounts[2], false)?;
    let state_v3 = *authenticated_state.state();
    let state = state_v3.base;
    require(
        state.policy_id.bytes() == policy_id && state.generation == payload.expected_generation,
        ClutchError::MismatchedState,
    )?;
    let _series_obligation = authenticate_live_series_obligation_for_state_v3(
        program_id,
        &accounts[33],
        &accounts[2],
        &state_v3,
    )?;
    let (position_binding, _facility_position, facility_replay, replay_binding) =
        authenticate_position_and_replay(
            program_id,
            &accounts[2],
            &accounts[3],
            &accounts[4],
            &policy,
            &state,
            false,
        )?;
    require(
        facility_replay.next_transition_ordinal() == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let dependency = authenticate_dependency(program_id, &accounts[5], state.facility_id)?;
    let (_epoch_bump, epoch) =
        authenticate_epoch_binding_with_access(program_id, &accounts[6], state.facility_id, false)?;
    let schedule = authenticate_schedule(program_id, &accounts[7])?;
    let (runtime_policy, runtime_states, runtime_binding) = authenticate_runtime_bundle(
        program_id,
        &dependency,
        &accounts[8],
        &accounts[9..16],
        DealerLivenessCompartmentV1::Settlement.index(),
    )?;
    validate_runtime_dependency_join(
        program_id,
        &accounts[2],
        &policy,
        &state,
        &position_binding,
        &dependency,
        &schedule,
        runtime_policy,
        runtime_binding,
    )?;
    let settlement_runtime =
        runtime_states[DealerLivenessCompartmentV1::Settlement.index()];
    require(
        settlement_runtime.identity.payer.bytes() == accounts[17].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;

    let (_selection_bump, selection) = dealer_body::<CoveredDealerSelectionV1>(
        program_id,
        &accounts[18],
        false,
        DEALER_COVERED_SELECTION_ACCOUNT_TAG,
        DEALER_COVERED_SELECTION_ACCOUNT_VERSION,
        DEALER_COVERED_SELECTION_ACCOUNT_BYTES,
    )?;
    let (lease_bump, lease) = dealer_body::<DealerLeaseV2>(
        program_id,
        &accounts[19],
        false,
        DEALER_LEASE_V2_ACCOUNT_TAG,
        DEALER_LEASE_V2_ACCOUNT_VERSION,
        DEALER_LEASE_V2_ACCOUNT_BYTES,
    )?;
    let (pot_bump, pot) = dealer_body::<SettlementPotV2>(
        program_id,
        &accounts[20],
        true,
        DEALER_SETTLEMENT_POT_V2_ACCOUNT_TAG,
        DEALER_SETTLEMENT_POT_V2_ACCOUNT_VERSION,
        DEALER_SETTLEMENT_POT_V2_ACCOUNT_BYTES,
    )?;
    expect_pda(
        accounts[19].key,
        seeds::dealer_lease_v2_pda(program_id, &state.facility_id.bytes(), state.generation),
        Some(lease_bump),
    )?;
    expect_pda(
        accounts[20].key,
        seeds::dealer_pot_v2_pda(program_id, &state.facility_id.bytes(), state.generation),
        Some(pot_bump),
    )?;
    selection
        .validate_lease_pot(&lease, &pot, &epoch, &policy)
        .map_err(dealer_fault)?;

    let root_inputs = authenticate_covered_root_inputs_v1(
        program_id,
        &accounts[21],
        &accounts[23],
        &accounts[30],
        &accounts[31],
        &policy,
        false,
    )?;
    let root = root_inputs.root.as_ref();
    let domain = root_inputs.domain.as_ref();
    let binding = root_inputs.binding.as_ref();
    let grid = root_inputs.grid.as_ref();
    let counts = root.counts();
    require(
        selection.settlement_root_account_id == id(accounts[21].key)
            && selection.retained_feed_account_id == id(accounts[22].key)
            && selection.economic_domain_id == id(accounts[23].key)
            && selection.settlement_candidate_id
                == Id::from_bytes(root.settlement_candidate_id().bytes())
            && selection.selection_account_id == id(accounts[18].key)
            && selection.lease_account_id == id(accounts[19].key)
            && selection.settlement_pot_account_id == id(accounts[20].key)
            && lease.lease_account_id == id(accounts[19].key)
            && lease.settlement_pot_id == id(accounts[20].key)
            && epoch.epoch_account_id == Id::from_bytes(root.epoch().bytes())
            && epoch.general_epoch_generation == root.epoch_generation()
            && matches!(
                root.phase(),
                SettlementRootPhaseV1::Materializing | SettlementRootPhaseV1::Settling
            )
            && root.retained_feed_state() == SettlementRootChildStateV1::Live
            && counts.expected_dealer_children == 1
            && counts.admitted_dealer_children == 1
            && counts.live_dealer_children == 1,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[18].key,
        seeds::dealer_covered_selection_pda(
            program_id,
            &root.epoch().bytes(),
            &root.settlement_candidate_id().bytes(),
        ),
        Some(selection.stored_bump),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        &accounts[32],
        ContentId::from_bytes(binding.base().market_genesis_profile_v2_id.bytes()),
    )?;
    require(
        genesis.value().realm_id.bytes() == policy.realm_id.bytes()
            && genesis.value().profile_id.bytes() == policy.profile_id.bytes()
            && genesis.value().price_grid_id.bytes() == grid.grid.bytes()
            && genesis.value().relation_policy_id.bytes() == policy.relation_v2_id.bytes()
            && genesis.value().fee_policy_id.bytes() == policy.fee_policy_id.bytes()
            && genesis.value().price_measure_policy_id.bytes()
                == policy.price_measure_policy_id.bytes()
            && genesis.value().score_policy_id.bytes() == root.score_policy_id().bytes()
            && genesis.value().coordinate_domain_min
                == domain.transcript.coordinate_domain_min
            && genesis.value().coordinate_domain_max
                == domain.transcript.coordinate_domain_max,
        ClutchError::MismatchedState,
    )?;

    for (account, rent_owner) in [
        (&accounts[18], selection.rent),
        (&accounts[19], lease.rent),
        (&accounts[20], pot.rent),
    ] {
        let floor = rent_owner
            .refundable_principal
            .checked_add(rent_owner.donation_floor)
            .ok_or(ClutchError::Arithmetic)?;
        require(
            account.lamports() >= floor && rent_owner.neutral_sink == policy.neutral_sink,
            ClutchError::DealerPolicyRentMismatch,
        )?;
    }

    require(
        accounts[24].owner == program_id
            && accounts[24].is_writable
            && accounts[24].data_len() == RESERVATION_ACCOUNT_BYTES_V9,
        ClutchError::MismatchedState,
    )?;
    let reservation = ReservationAccountV9::decode(&accounts[24].data.borrow())?;
    let reservation_body = reservation.body();
    let reservation_rent = reservation.rent();
    let reservation_floor = reservation_rent
        .refundable_principal
        .checked_add(reservation_rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        accounts[24].lamports() >= reservation_floor,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[24].key,
        seeds::general_v2_reservation_v9_pda(
            program_id,
            &reservation_body.reservation.bytes(),
        ),
        Some(reservation_body.stored_bump),
    )?;
    let owner = Id::from_bytes(reservation_body.owner.bytes());
    let (_collateral_value, market) = authenticate_dealer_collateral_value_v2(
        program_id,
        &policy,
        Some(&position_binding),
        DealerCollateralAuthorityAccountsV2 {
            realm: &accounts[34],
            profile: &accounts[35],
            policy: &accounts[36],
            token_program: &accounts[37],
            token_programdata: &accounts[38],
            market_binding: &accounts[30],
            market_runtime: &accounts[39],
            market_instance: &accounts[40],
            hoard: &accounts[41],
            claim_ledger: &accounts[42],
        },
    )?;
    let (user_position, user_replay) = authenticate_general_position_replay_for_dealer(
        program_id,
        root,
        market,
        owner,
        &accounts[25],
        &accounts[26],
    )?;
    let rent = read_rent(&accounts[28])?;
    require_system_program(&accounts[29])?;
    let receipt_principal = rent.minimum_balance(DEALER_ACTION_RECEIPT_ACCOUNT_BYTES)?;
    require(
        payload.keeper_payment_lamports >= receipt_principal,
        ClutchError::MismatchedState,
    )?;
    require_creatable(&accounts[16])?;
    let runtime_action = match action {
        DealerFacilityAction::Collect => DealerRuntimeActionV1::Collect,
        DealerFacilityAction::Deliver => DealerRuntimeActionV1::Deliver,
        _ => return Err(ClutchError::UnsupportedInstruction.into()),
    };
    let action_index = DealerLivenessScheduleV1::action_index(runtime_action);
    let receipt = DealerActionReceiptV1 {
        policy_id: state.policy_id,
        facility_id: state.facility_id,
        dealer_state_account_id: id(accounts[2].key),
        liveness_schedule_id: schedule.schedule_id().map_err(dealer_fault)?.untyped(),
        runtime_policy_id: runtime_binding.runtime_policy_id(),
        runtime_account_id: runtime_binding.account_id(DealerLivenessCompartmentV1::Settlement),
        runtime_owner: runtime_binding.owner(DealerLivenessCompartmentV1::Settlement),
        quote_schedule_id: runtime_binding
            .quote_schedule_id(DealerLivenessCompartmentV1::Settlement),
        receipt_account_id: id(accounts[16].key),
        receipt_program_id: id(program_id),
        keeper: id(accounts[0].key),
        replay_account_id: id(accounts[4].key),
        action: runtime_action,
        compartment: DealerLivenessCompartmentV1::Settlement,
        runtime_generation: runtime_binding.generation(DealerLivenessCompartmentV1::Settlement),
        facility_generation: state.generation,
        call_ordinal: payload.liveness_call_ordinal,
        call_ceiling_lamports: schedule.reward_lamports[action_index],
        keeper_payment_lamports: payload.keeper_payment_lamports,
        expected_replay_ordinal: payload.expected_replay_ordinal,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[17].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: receipt_principal,
            donation_floor: accounts[16].lamports(),
        },
    };
    let receipt_slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
    let (receipt_address, receipt_bump) =
        seeds::dealer_action_receipt_pda(program_id, &receipt_slot.bytes());
    expect_pda(accounts[16].key, (receipt_address, receipt_bump), None)?;
    receipt
        .validate_against(&schedule, &runtime_binding)
        .map_err(dealer_fault)?;
    let authorization = receipt
        .authorization(&schedule, &runtime_binding, &settlement_runtime)
        .map_err(dealer_fault)?;
    let liveness_transition = plan_liveness_spend_absorbing_donation(
        program_id,
        &accounts[8],
        &accounts[12],
        settlement_runtime,
        receipt.runtime_transition_intent().map_err(dealer_fault)?,
        receipt.runtime_receipt_observation().map_err(dealer_fault)?,
    )?;
    let current_slot = read_clock_slot(&accounts[27])?;

    with_authenticated_complete_dealer_book_v2(
        program_id,
        &accounts[21],
        &root,
        &accounts[22],
        domain,
        &accounts[COLLECT_DELIVER_FIXED_ACCOUNT_COUNT..],
        false,
        |book, feed_data| {
            let (feed_header, feed_economics) = decode_sealed_candidate_feed_v1(feed_data)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            let row_index = payload.row_start;
            require(
                row_index < u16::from(selection.allocation_count),
                ClutchError::MismatchedState,
            )?;
            let allocation = selection.allocations[usize::from(row_index)];
            let membership = find_dealer_page_membership_v1(
                &accounts[COLLECT_DELIVER_FIXED_ACCOUNT_COUNT..],
                allocation.order_id,
            )?;
            let mut dense = None;
            let mut index = 0usize;
            while index < usize::from(book.economic_book().len) {
                if book.economic_book().orders[index].order_id == allocation.order_id {
                    dense = Some(u8::try_from(index).map_err(|_| ClutchError::Arithmetic)?);
                    break;
                }
                index += 1;
            }
            let order_index = dense.ok_or(ClutchError::MismatchedState)?;
            let position_semantic_id = user_position.semantic_id;
            let root_data_id = root.data_id(
                &RuntimeSha256,
                clutch_general_v2_contract::Id32::new(accounts[21].key.to_bytes())?,
            )?;
            let feed_data_id = clutch_general_v2_contract::candidate_bundle_digest_v1(
                &RuntimeSha256,
                feed_data,
                true,
            )?;
            let page_account = &accounts[COLLECT_DELIVER_FIXED_ACCOUNT_COUNT
                + usize::from(membership.page_index)];
            let relation_domain = EconomicDomainV2 {
                relation_version: domain.transcript.relation_version,
                market_semantics_digest: domain.transcript.market_instance_v2_id.bytes(),
                epoch_semantics_digest: domain.transcript.epoch_semantics_digest.bytes(),
                relation_policy_digest: domain.transcript.relation_policy_id.bytes(),
                price_policy_digest: domain.transcript.price_measure_policy_v1_id.bytes(),
                epoch_index: domain.transcript.epoch_index,
                outcome_count: domain.transcript.outcome_count,
                price_scale: domain.transcript.price_scale,
            };
            let candidate = EconomicCandidateV2 {
                fills: feed_economics.fills,
                honored_aon_mask: feed_header.honored_aon_mask,
                virtual_split: feed_header.virtual_split,
                virtual_merge: feed_header.virtual_merge,
            };
            let relation_order = book.economic_book().orders[usize::from(order_index)];
            let record = SelectedPortfolioOrderRecordV2 {
                version: PORTFOLIO_EXECUTION_VERSION_V2,
                outcome_count: selection.outcome_count,
                source_kind: PortfolioSourceOrderKindV2::Portfolio,
                side: relation_order.side,
                order_index,
                page_slot: membership.slot_index,
                traversal_index: row_index,
                page_index: membership.page_index,
                settlement_root_epoch_generation: root.epoch_generation(),
                position_generation: membership.position_generation,
                selected_fill_units: allocation.dealer_fill_units,
                market_semantics_digest: relation_domain.market_semantics_digest,
                epoch_semantics_digest: relation_domain.epoch_semantics_digest,
                economic_candidate_digest: feed_header.base_relation_candidate_id.bytes(),
                order_set_digest: root.order_set().bytes(),
                settlement_root_account_id: accounts[21].key.to_bytes(),
                settlement_root_pre_semantic_id: root_data_id.bytes(),
                settlement_candidate_id: root.settlement_candidate_id().bytes(),
                retained_feed_account_id: accounts[22].key.to_bytes(),
                retained_feed_semantic_id: feed_data_id.bytes(),
                settlement_witness_id: root.settlement_witness_digest().bytes(),
                order_page_account_id: page_account.key.to_bytes(),
                order_page_semantic_id: membership.page_semantic_id,
                position_account_id: accounts[25].key.to_bytes(),
                position_pre_semantic_id: position_semantic_id,
                order_id: allocation.order_id,
                owner_id: membership.owner_id,
            };
            let owner_program = program_id.to_bytes();
            let adapter = DealerSelectedRowAdapterV1 {
                record,
                accounts: [
                    PortfolioAccountExpectationV2 {
                        role: PortfolioAccountRoleV2::SettlementRoot,
                        account_id: accounts[21].key.to_bytes(),
                        owner_program_id: owner_program,
                        data_semantic_id: root_data_id.bytes(),
                        generation: Some(root.epoch_generation()),
                        writable: false,
                        must_exist: true,
                    },
                    PortfolioAccountExpectationV2 {
                        role: PortfolioAccountRoleV2::RetainedFeed,
                        account_id: accounts[22].key.to_bytes(),
                        owner_program_id: owner_program,
                        data_semantic_id: feed_data_id.bytes(),
                        generation: None,
                        writable: false,
                        must_exist: true,
                    },
                    PortfolioAccountExpectationV2 {
                        role: PortfolioAccountRoleV2::OrderPage,
                        account_id: page_account.key.to_bytes(),
                        owner_program_id: owner_program,
                        data_semantic_id: membership.page_semantic_id,
                        generation: None,
                        writable: false,
                        must_exist: true,
                    },
                    PortfolioAccountExpectationV2 {
                        role: PortfolioAccountRoleV2::Position,
                        account_id: accounts[25].key.to_bytes(),
                        owner_program_id: owner_program,
                        data_semantic_id: position_semantic_id,
                        generation: Some(membership.position_generation),
                        writable: true,
                        must_exist: true,
                    },
                ],
            };
            let authenticated_order = authenticate_selected_portfolio_order_v2(
                &adapter,
                owner_program,
                &relation_domain,
                book.economic_book(),
                &candidate,
                feed_header.base_relation_candidate_id.bytes(),
                record,
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            let row = selection
                .authenticate_settlement_row(&authenticated_order, row_index)
                .map_err(dealer_fault)?;
            require(
                row.owner_id() == owner
                    && row.position_account_id() == id(accounts[25].key)
                    && reservation_body.order_generation == membership.order_generation
                    && reservation_body.page_index == membership.page_index
                    && reservation_body.market.bytes() == root.market().bytes()
                    && reservation_body.epoch.bytes() == root.epoch().bytes(),
                ClutchError::MismatchedState,
            )?;
            let expected_plan = ReservationPlan::for_order(
                &membership.slot,
                root.outcome_count(),
                grid.price_scale,
                reservation_body.max_fee_atoms,
            )?;
            require(
                reservation_body.price_grid.bytes() == grid.grid.bytes()
                    && reservation_body.terms.bytes()
                        == binding.base().series_funding_terms_v2_id.bytes()
                    && reservation_body.policy.bytes()
                        == binding.base().settlement_policy_id.bytes()
                    && reservation_body.outcome_count == root.outcome_count()
                    && reservation_body.initial_cash_atoms == expected_plan.cash_atoms
                    && reservation_body.max_fee_atoms == expected_plan.max_fee_atoms
                    && reservation_body.initial_internal == expected_plan.internal
                    && reservation_body.order_kind == expected_plan.order_kind
                    && reservation_body.side == expected_plan.side,
                ClutchError::MismatchedState,
            )?;

            let (reservation_post, position_post) = match runtime_action {
                DealerRuntimeActionV1::Collect => {
                    prepare_dealer_row_collect_post(reservation, user_position, row)?
                }
                DealerRuntimeActionV1::Deliver => {
                    prepare_dealer_row_deliver_post(reservation, user_position, row)?
                }
                _ => return Err(ClutchError::UnsupportedInstruction.into()),
            };
            let reservation_pre_id = reservation.data_id()?;
            let reservation_post_id = reservation_post.data_id()?;
            let replay_kind = match (runtime_action, row.side()) {
                (DealerRuntimeActionV1::Collect, Side::Buy) => {
                    GeneralReplayTransitionKindV1::DealerCollectBuyer
                }
                (DealerRuntimeActionV1::Collect, Side::Sell) => {
                    GeneralReplayTransitionKindV1::DealerCollectSeller
                }
                (DealerRuntimeActionV1::Deliver, Side::Buy) => {
                    GeneralReplayTransitionKindV1::DealerDeliverBuyer
                }
                (DealerRuntimeActionV1::Deliver, Side::Sell) => {
                    GeneralReplayTransitionKindV1::DealerDeliverSeller
                }
                _ => return Err(ClutchError::UnsupportedInstruction.into()),
            };
            let general_replay_post = project_general_replay_transition_v1(
                user_replay,
                position_post,
                replay_kind,
                clutch_general_v2_contract::Id32::new(reservation_post_id.bytes())?,
                dealer_general_replay_value_evidence_id_v2(
                    market.collateral_value_receipt_id,
                    authorization.receipt_semantic_id,
                )?,
                &RuntimeSha256,
            )?;
            let pot_after = match runtime_action {
                DealerRuntimeActionV1::Collect => advance_collect_v2(
                    &pot,
                    &lease,
                    &schedule,
                    &runtime_binding,
                    row.collect_slice(),
                    current_slot,
                    Some(&authorization),
                ),
                DealerRuntimeActionV1::Deliver => advance_deliver_v2(
                    &pot,
                    &lease,
                    &schedule,
                    &runtime_binding,
                    row.deliver_slice(),
                    current_slot,
                    Some(&authorization),
                ),
                _ => return Err(ClutchError::UnsupportedInstruction.into()),
            }
            .map_err(dealer_fault)?;
            let position_post_id = position_post
                .semantic
                .semantic_id(&RuntimeSha256)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            let row_transition = CoveredDealerRowAssetTransitionV2::new(
                runtime_action,
                row,
                id(accounts[24].key),
                Id::from_bytes(reservation_pre_id.bytes()),
                Id::from_bytes(reservation_post_id.bytes()),
                Id::from_bytes(user_position.semantic_id),
                Id::from_bytes(position_post_id.bytes()),
                Id::from_bytes(user_replay.replay_semantic_id().bytes()),
                Id::from_bytes(general_replay_post.replay_poststate_semantic_id().bytes()),
                pot.pot_content_id().map_err(dealer_fault)?,
                pot_after.pot_content_id().map_err(dealer_fault)?,
                market.collateral_value_receipt_id,
            )
            .map_err(dealer_fault)?;
            let prepared = prepare_covered_dealer_row_progress_v1(
                &policy,
                &state,
                id(accounts[2].key),
                &epoch,
                &selection,
                row,
                &lease,
                &pot,
                &schedule,
                &runtime_binding,
                &authorization,
                row_transition,
                &facility_replay,
                replay_binding,
                current_slot,
            )
            .map_err(dealer_fault)?;
            require(
                prepared.pot_after == pot_after,
                ClutchError::MismatchedState,
            )?;

            create_full_principal_pda(
                program_id,
                &accounts[0],
                &accounts[16],
                &accounts[29],
                &rent,
                DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
                &[
                    seeds::SEED_DEALER_ACTION_RECEIPT,
                    &receipt_slot.bytes(),
                    &[receipt_bump],
                ],
            )?;
            apply_liveness_transition(
                &accounts[12],
                &accounts[0],
                &accounts[17],
                &liveness_transition,
            )?;
            write_dealer_body(
                &accounts[16],
                DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
                DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
                receipt_bump,
                &receipt,
            )?;
            let mut reservation_bytes = [0u8; RESERVATION_ACCOUNT_BYTES_V9];
            reservation_post.encode(&mut reservation_bytes)?;
            accounts[24]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
                .copy_from_slice(&reservation_bytes);
            accounts[25]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
                .copy_from_slice(
                    &position_post
                        .semantic
                        .encode()
                        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
                );
            accounts[26]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
                .copy_from_slice(general_replay_post.replay_poststate_body());
            write_dealer_body(
                &accounts[20],
                DEALER_SETTLEMENT_POT_V2_ACCOUNT_TAG,
                DEALER_SETTLEMENT_POT_V2_ACCOUNT_VERSION,
                pot_bump,
                &prepared.pot_after,
            )?;
            let observed_reservation = ReservationAccountV9::decode(&accounts[24].data.borrow())?;
            let observed_position = PositionAccountV3::decode(&accounts[25].data.borrow())
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            let observed_replay_data = accounts[26]
                .try_borrow_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            let observed_replay = ReplayV3Envelope::decode(&observed_replay_data, &RuntimeSha256)
                .map_err(|_| Refusal::Adapter(ClutchError::Replay))?;
            let observed_replay_id = observed_replay
                .semantic_id(&RuntimeSha256)
                .map_err(|_| Refusal::Adapter(ClutchError::Replay))?;
            let (_, observed_pot) = dealer_body::<SettlementPotV2>(
                program_id,
                &accounts[20],
                true,
                DEALER_SETTLEMENT_POT_V2_ACCOUNT_TAG,
                DEALER_SETTLEMENT_POT_V2_ACCOUNT_VERSION,
                DEALER_SETTLEMENT_POT_V2_ACCOUNT_BYTES,
            )?;
            let observed_position_id = observed_position
                .semantic_id(&RuntimeSha256)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            let observed_transition = CoveredDealerRowAssetTransitionV2::new(
                runtime_action,
                row,
                id(accounts[24].key),
                Id::from_bytes(reservation_pre_id.bytes()),
                Id::from_bytes(observed_reservation.data_id()?.bytes()),
                Id::from_bytes(user_position.semantic_id),
                Id::from_bytes(observed_position_id.bytes()),
                Id::from_bytes(user_replay.replay_semantic_id().bytes()),
                Id::from_bytes(observed_replay_id.bytes()),
                pot.pot_content_id().map_err(dealer_fault)?,
                observed_pot.pot_content_id().map_err(dealer_fault)?,
                market.collateral_value_receipt_id,
            )
            .map_err(dealer_fault)?;
            require(
                observed_reservation == reservation_post
                    && observed_position == position_post.semantic
                    && observed_replay_data.as_ref()
                        == general_replay_post.replay_poststate_body()
                    && observed_pot == prepared.pot_after
                    && observed_transition.bundle_id() == row_transition.bundle_id(),
                ClutchError::MismatchedState,
            )?;
            drop(observed_replay_data);
            prepared
                .facility_replay
                .replay_post()
                .encode_into(&mut accounts[4].data.borrow_mut())
                .map_err(dealer_fault)
        },
    )
}

#[inline(never)]
fn finalize_or_abort_lease_pot(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: DealerFacilityAction,
    payload_bytes: &[u8],
) -> Outcome<()> {
    require(
        matches!(
            action,
            DealerFacilityAction::FinalizeSettlement
                | DealerFacilityAction::AbortBeforeCollection
        ),
        ClutchError::UnsupportedInstruction,
    )?;
    let payload = DealerRuntimePayloadV1::decode(action, payload_bytes).map_err(dealer_fault)?;
    require_count(accounts, FINALIZE_ABORT_ACCOUNT_COUNT)?;
    require(
        sequence == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    require_finalize_abort_aliases(accounts)?;

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let authenticated_state = authenticate_dealer_state_v3(program_id, &accounts[2], true)?;
    let state_v3 = *authenticated_state.state();
    let state = state_v3.base;
    require(
        state.policy_id.bytes() == policy_id && state.generation == payload.expected_generation,
        ClutchError::MismatchedState,
    )?;
    let _series_obligation = authenticate_live_series_obligation_for_state_v3(
        program_id,
        &accounts[29],
        &accounts[2],
        &state_v3,
    )?;
    let (position_binding, facility_position, facility_replay, replay_binding) =
        authenticate_position_and_replay(
            program_id,
            &accounts[2],
            &accounts[3],
            &accounts[4],
            &policy,
            &state,
            true,
        )?;
    require(
        facility_replay.next_transition_ordinal() == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let dependency = authenticate_dependency(program_id, &accounts[5], state.facility_id)?;
    let (_epoch_bump, epoch) =
        authenticate_epoch_binding_with_access(program_id, &accounts[6], state.facility_id, false)?;
    let schedule = authenticate_schedule(program_id, &accounts[7])?;
    let compartment = match action {
        DealerFacilityAction::FinalizeSettlement => DealerLivenessCompartmentV1::Settlement,
        DealerFacilityAction::AbortBeforeCollection => DealerLivenessCompartmentV1::Recovery,
        _ => return Err(ClutchError::UnsupportedInstruction.into()),
    };
    let (runtime_policy, runtime_states, runtime_binding) = authenticate_runtime_bundle(
        program_id,
        &dependency,
        &accounts[8],
        &accounts[9..16],
        compartment.index(),
    )?;
    validate_runtime_dependency_join(
        program_id,
        &accounts[2],
        &policy,
        &state,
        &position_binding,
        &dependency,
        &schedule,
        runtime_policy,
        runtime_binding,
    )?;
    let selected_runtime = runtime_states[compartment.index()];
    require(
        selected_runtime.identity.payer.bytes() == accounts[17].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;

    let (selection_bump, selection) = dealer_body::<CoveredDealerSelectionV1>(
        program_id,
        &accounts[18],
        true,
        DEALER_COVERED_SELECTION_ACCOUNT_TAG,
        DEALER_COVERED_SELECTION_ACCOUNT_VERSION,
        DEALER_COVERED_SELECTION_ACCOUNT_BYTES,
    )?;
    let (lease_bump, lease) = dealer_body::<DealerLeaseV2>(
        program_id,
        &accounts[19],
        true,
        DEALER_LEASE_V2_ACCOUNT_TAG,
        DEALER_LEASE_V2_ACCOUNT_VERSION,
        DEALER_LEASE_V2_ACCOUNT_BYTES,
    )?;
    let (pot_bump, pot) = dealer_body::<SettlementPotV2>(
        program_id,
        &accounts[20],
        true,
        DEALER_SETTLEMENT_POT_V2_ACCOUNT_TAG,
        DEALER_SETTLEMENT_POT_V2_ACCOUNT_VERSION,
        DEALER_SETTLEMENT_POT_V2_ACCOUNT_BYTES,
    )?;
    expect_pda(
        accounts[18].key,
        seeds::dealer_covered_selection_pda(
            program_id,
            &epoch.epoch_account_id.bytes(),
            &selection.settlement_candidate_id.bytes(),
        ),
        Some(selection_bump),
    )?;
    expect_pda(
        accounts[19].key,
        seeds::dealer_lease_v2_pda(program_id, &state.facility_id.bytes(), state.generation),
        Some(lease_bump),
    )?;
    expect_pda(
        accounts[20].key,
        seeds::dealer_pot_v2_pda(program_id, &state.facility_id.bytes(), state.generation),
        Some(pot_bump),
    )?;
    selection
        .validate_lease_pot(&lease, &pot, &epoch, &policy)
        .map_err(dealer_fault)?;
    require(
        selection.selection_account_id == id(accounts[18].key)
            && selection.stored_bump == selection_bump
            && selection.lease_account_id == id(accounts[19].key)
            && selection.settlement_pot_account_id == id(accounts[20].key)
            && selection.dealer_state_account_id == id(accounts[2].key)
            && selection.epoch_binding_account_id == id(accounts[6].key),
        ClutchError::MismatchedState,
    )?;
    for (account, owner) in [(&accounts[18], selection.rent), (&accounts[19], lease.rent), (&accounts[20], pot.rent)] {
        let floor = owner
            .refundable_principal
            .checked_add(owner.donation_floor)
            .ok_or(ClutchError::Arithmetic)?;
        require(
            account.lamports() >= floor && owner.neutral_sink == policy.neutral_sink,
            ClutchError::DealerPolicyRentMismatch,
        )?;
    }

    let fee_terminal = authenticate_fee_terminal_for_dealer(
        program_id,
        &accounts[21],
        &accounts[22],
        &policy,
        &selection,
        &epoch,
        &lease,
    )?;
    for account in [&accounts[23], &accounts[24], &accounts[25]] {
        require(
            account.is_writable && !account.executable,
            ClutchError::NotWritable,
        )?;
    }
    require(
        id(accounts[23].key) == lease.rent.payer
            && id(accounts[24].key) == pot.rent.payer
            && id(accounts[25].key) == policy.neutral_sink,
        ClutchError::MismatchedState,
    )?;
    let current_slot = read_clock_slot(&accounts[26])?;
    let rent = read_rent(&accounts[27])?;
    require_system_program(&accounts[28])?;
    let receipt_principal = rent.minimum_balance(DEALER_ACTION_RECEIPT_ACCOUNT_BYTES)?;
    require(
        payload.keeper_payment_lamports >= receipt_principal,
        ClutchError::MismatchedState,
    )?;
    require_creatable(&accounts[16])?;
    let runtime_action = match action {
        DealerFacilityAction::FinalizeSettlement => DealerRuntimeActionV1::FinalizeSettlement,
        DealerFacilityAction::AbortBeforeCollection => DealerRuntimeActionV1::AbortBeforeCollection,
        _ => return Err(ClutchError::UnsupportedInstruction.into()),
    };
    let action_index = DealerLivenessScheduleV1::action_index(runtime_action);
    let receipt = DealerActionReceiptV1 {
        policy_id: state.policy_id,
        facility_id: state.facility_id,
        dealer_state_account_id: id(accounts[2].key),
        liveness_schedule_id: schedule.schedule_id().map_err(dealer_fault)?.untyped(),
        runtime_policy_id: runtime_binding.runtime_policy_id(),
        runtime_account_id: runtime_binding.account_id(compartment),
        runtime_owner: runtime_binding.owner(compartment),
        quote_schedule_id: runtime_binding.quote_schedule_id(compartment),
        receipt_account_id: id(accounts[16].key),
        receipt_program_id: id(program_id),
        keeper: id(accounts[0].key),
        replay_account_id: id(accounts[4].key),
        action: runtime_action,
        compartment,
        runtime_generation: runtime_binding.generation(compartment),
        facility_generation: state.generation,
        call_ordinal: payload.liveness_call_ordinal,
        call_ceiling_lamports: schedule.reward_lamports[action_index],
        keeper_payment_lamports: payload.keeper_payment_lamports,
        expected_replay_ordinal: payload.expected_replay_ordinal,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[17].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: receipt_principal,
            donation_floor: accounts[16].lamports(),
        },
    };
    let receipt_slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
    let (receipt_address, receipt_bump) =
        seeds::dealer_action_receipt_pda(program_id, &receipt_slot.bytes());
    expect_pda(accounts[16].key, (receipt_address, receipt_bump), None)?;
    receipt
        .validate_against(&schedule, &runtime_binding)
        .map_err(dealer_fault)?;
    let authorization = receipt
        .authorization(&schedule, &runtime_binding, &selected_runtime)
        .map_err(dealer_fault)?;
    let liveness_transition = plan_liveness_spend_absorbing_donation(
        program_id,
        &accounts[8],
        &accounts[9 + compartment.index()],
        selected_runtime,
        receipt.runtime_transition_intent().map_err(dealer_fault)?,
        receipt.runtime_receipt_observation().map_err(dealer_fault)?,
    )?;
    let (_collateral_value, market) = authenticate_dealer_collateral_value_v2(
        program_id,
        &policy,
        Some(&position_binding),
        DealerCollateralAuthorityAccountsV2 {
            realm: &accounts[30],
            profile: &accounts[31],
            policy: &accounts[32],
            token_program: &accounts[33],
            token_programdata: &accounts[34],
            market_binding: &accounts[35],
            market_runtime: &accounts[36],
            market_instance: &accounts[37],
            hoard: &accounts[38],
            claim_ledger: &accounts[39],
        },
    )?;
    let close_rent = DealerLeasePotCloseRentV3 {
        lease_lamports_before: accounts[19].lamports(),
        pot_lamports_before: accounts[20].lamports(),
        lease_lamports_after: 0,
        pot_lamports_after: 0,
    };
    let prepared = match runtime_action {
        DealerRuntimeActionV1::FinalizeSettlement => prepare_finalize_lease_pot_v3(
            &policy,
            &position_binding,
            &state,
            id(accounts[2].key),
            &dependency,
            &lease,
            id(accounts[20].key),
            &pot,
            &schedule,
            &runtime_binding,
            &authorization,
            &fee_terminal,
            market,
            &facility_position,
            &facility_replay,
            replay_binding,
            close_rent,
        ),
        DealerRuntimeActionV1::AbortBeforeCollection => prepare_abort_lease_pot_v3(
            &policy,
            &position_binding,
            &state,
            id(accounts[2].key),
            &dependency,
            &lease,
            id(accounts[20].key),
            &pot,
            &schedule,
            &runtime_binding,
            &authorization,
            &fee_terminal,
            market,
            &facility_position,
            &facility_replay,
            replay_binding,
            current_slot,
            close_rent,
        ),
        _ => return Err(ClutchError::UnsupportedInstruction.into()),
    }
    .map_err(dealer_fault)?;
    let terminal = CoveredDealerTerminalV2::from_prepared(
        &selection,
        &epoch,
        &state,
        &lease,
        &pot,
        &facility_replay,
        &fee_terminal,
        &receipt,
        prepared,
        current_slot,
    )
    .map_err(dealer_fault)?;
    let close = prepared.close();
    require(
        close.refund_recipients() == [id(accounts[23].key), id(accounts[24].key)]
            && close.neutral_sink() == id(accounts[25].key)
            && terminal.selection_account_id() == id(accounts[18].key)
            && terminal.stored_bump() == selection_bump,
        ClutchError::MismatchedState,
    )?;

    let (created_receipt_principal, observed_receipt_donation) = create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[16],
        &accounts[28],
        &rent,
        DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_ACTION_RECEIPT,
            &receipt_slot.bytes(),
            &[receipt_bump],
        ],
    )?;
    require(
        created_receipt_principal == receipt.rent.refundable_principal
            && observed_receipt_donation == receipt.rent.donation_floor,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    apply_liveness_transition(
        &accounts[9 + compartment.index()],
        &accounts[0],
        &accounts[17],
        &liveness_transition,
    )?;
    write_dealer_body(
        &accounts[16],
        DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
        DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
        receipt_bump,
        &receipt,
    )?;
    accounts[3]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &prepared
                .transfer()
                .position_post()
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    let state_after = state_v3
        .with_base(prepared.state_after())
        .map_err(dealer_fault)?;
    write_dealer_body(
        &accounts[2],
        DEALER_STATE_V3_ACCOUNT_TAG,
        DEALER_STATE_V3_ACCOUNT_VERSION,
        authenticated_state.bump(),
        &state_after,
    )?;
    prepared
        .replay()
        .replay_post()
        .encode_into(&mut accounts[4].data.borrow_mut())
        .map_err(dealer_fault)?;
    write_dealer_body(
        &accounts[18],
        DEALER_COVERED_SELECTION_ACCOUNT_TAG,
        DEALER_COVERED_TERMINAL_ACCOUNT_VERSION,
        selection_bump,
        &terminal,
    )?;
    let receipt_rent = terminal.action_receipt_rent();
    let expected_receipt_lamports = receipt_rent
        .refundable_principal
        .checked_add(receipt_rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        receipt_rent.payer == id(accounts[17].key)
            && receipt_rent.neutral_sink == id(accounts[25].key)
            && accounts[16].lamports() == expected_receipt_lamports,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    release_dealer_account(&accounts[16])?;
    release_dealer_account(&accounts[19])?;
    release_dealer_account(&accounts[20])?;
    require_released_dealer_account(&accounts[16])?;
    require_released_dealer_account(&accounts[19])?;
    require_released_dealer_account(&accounts[20])?;
    let transfer_bundle = prepared.transfer().bundle();
    let observed_position_post = dealer_transfer_endpoint_semantic_id_v2(
        program_id,
        DealerAssetEndpointKindV1::FacilityPosition,
        &accounts[3],
    )?;
    let _accepted_transfer = accept_dealer_asset_transfer_v2(
        transfer_bundle,
        DealerAssetTransferPostObservationV2 {
            source_account_id: id(accounts[20].key),
            destination_account_id: id(accounts[3].key),
            source_post_semantic_id: close.transition_id(),
            destination_post_semantic_id: observed_position_post,
        },
    )
    .map_err(dealer_fault)?;
    let refunds = close.refund_lamports();
    let neutral_credit = close
        .neutral_sink_lamports()
        .checked_add(receipt_rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    credit_exact_dealer_terminal_lamports([
        (&accounts[17], receipt_rent.refundable_principal),
        (&accounts[23], refunds[0]),
        (&accounts[24], refunds[1]),
        (&accounts[25], neutral_credit),
    ])
}

/// Execute one facility action admitted by the non-production profile.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: DealerFacilityAction,
    payload: &[u8],
) -> Outcome<()> {
    let implemented = matches!(
        action,
        DealerFacilityAction::Initialize
            | DealerFacilityAction::CreateLpPage
            | DealerFacilityAction::Contribute
            | DealerFacilityAction::WithdrawFunding
            | DealerFacilityAction::Activate
            | DealerFacilityAction::CancelFunding
            | DealerFacilityAction::RefundCancelledSponsor
            | DealerFacilityAction::BindEpoch
            | DealerFacilityAction::LapseEpoch
            | DealerFacilityAction::SelectLeaseAndBegin
            | DealerFacilityAction::Collect
            | DealerFacilityAction::Deliver
            | DealerFacilityAction::FinalizeSettlement
            | DealerFacilityAction::AbortBeforeCollection
            | DealerFacilityAction::QueueExit
            | DealerFacilityAction::SponsorHalt
            | DealerFacilityAction::EnterUnwind
            | DealerFacilityAction::TimedClose
            | DealerFacilityAction::Claim
    );
    if !implemented {
        return super::dealer_runtime::process_reserved_disabled(action);
    }
    let account_contract_payload =
        DealerRuntimePayloadV1::decode(action, payload).map_err(dealer_fault)?;
    authenticate_dealer_meta_contract_v1(
        program_id,
        accounts,
        action,
        account_contract_payload,
    )?;
    match action {
        DealerFacilityAction::Initialize => {
            initialize_facility(program_id, accounts, sequence, payload)
        }
        DealerFacilityAction::CreateLpPage => {
            create_lp_page(program_id, accounts, sequence, payload)
        }
        DealerFacilityAction::Contribute | DealerFacilityAction::WithdrawFunding => {
            transfer_lp_funding(program_id, accounts, sequence, action, payload)
        }
        DealerFacilityAction::Activate => {
            activate_facility(program_id, accounts, sequence, payload)
        }
        DealerFacilityAction::CancelFunding => {
            cancel_stale_funding(program_id, accounts, sequence, payload)
        }
        DealerFacilityAction::RefundCancelledSponsor => {
            refund_cancelled_sponsor(program_id, accounts, sequence, payload)
        }
        DealerFacilityAction::BindEpoch => bind_epoch(program_id, accounts, sequence, payload),
        DealerFacilityAction::LapseEpoch => lapse_epoch(program_id, accounts, sequence, payload),
        DealerFacilityAction::SelectLeaseAndBegin => {
            select_lease_and_begin(program_id, accounts, sequence, payload)
        }
        DealerFacilityAction::Collect | DealerFacilityAction::Deliver => {
            collect_or_deliver_row(program_id, accounts, sequence, action, payload)
        }
        DealerFacilityAction::FinalizeSettlement
        | DealerFacilityAction::AbortBeforeCollection => {
            finalize_or_abort_lease_pot(program_id, accounts, sequence, action, payload)
        }
        DealerFacilityAction::QueueExit => {
            queue_exit(program_id, accounts, sequence, payload)
        }
        DealerFacilityAction::SponsorHalt => {
            sponsor_halt(program_id, accounts, sequence, payload)
        }
        DealerFacilityAction::EnterUnwind => {
            enter_unwind(program_id, accounts, sequence, payload)
        }
        DealerFacilityAction::TimedClose => {
            timed_close(program_id, accounts, sequence, payload)
        }
        DealerFacilityAction::Resolve => {
            resolve_facility_vector(program_id, accounts, sequence, payload)
        }
        DealerFacilityAction::Claim => {
            claim_terminal_allocation(program_id, accounts, sequence, payload)
        }
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

#[cfg(test)]
mod future_credit_funding_adversarial_tests {
    use super::INITIALIZE_ACCOUNT_COUNT;
    use crate::instructions::dealer_runtime::{meta_contract_v1, DealerMetaRoleV1, DealerRuntimePayloadV1};
    use clutch_solana_layout::registry::DealerFacilityAction;

    #[test]
    fn initialize_contract_requires_the_exact_writable_future_credit_owner() {
        let mut payload = [0u8; 48];
        payload[0..8].copy_from_slice(&1u64.to_le_bytes());
        payload[8..16].copy_from_slice(&0u64.to_le_bytes());
        payload[16..24].copy_from_slice(&3u64.to_le_bytes());
        payload[24..32].copy_from_slice(&4u64.to_le_bytes());
        payload[32..36].copy_from_slice(&5u32.to_le_bytes());
        payload[40..48].copy_from_slice(&6u64.to_le_bytes());
        let decoded = DealerRuntimePayloadV1::decode(DealerFacilityAction::Initialize, &payload)
            .expect("canonical initialize payload");
        let metas = meta_contract_v1(DealerFacilityAction::Initialize, decoded)
            .expect("frozen initialize contract");
        assert_eq!(metas.len(), INITIALIZE_ACCOUNT_COUNT);
        assert_eq!(metas[33].role, DealerMetaRoleV1::FutureCreditFunding);
        assert!(metas[33].writable);
        assert!(!metas[33].signer);
    }

    #[test]
    fn initialization_debits_both_principals_and_hostile_prefund_never_discounts_them() {
        let source = include_str!("dealer_facility.rs");
        let handler = source
            .split("fn initialize_facility")
            .nth(1)
            .and_then(|value| value.split("fn sponsor_halt").next())
            .expect("Initialize handler");
        for guard in [
            "future_credit_live_principal",
            "future_credit_tombstone_principal",
            "future_credit_total_principal",
            "create_exact_payer_debit_pda",
            "observed_future_credit_donation == future_credit_funding.donation_floor_lamports",
            "authenticate_future_credit_funding(program_id, &accounts[33], true)",
            "observed_future_credit_funding == future_credit_funding",
        ] {
            assert!(handler.contains(guard), "missing future-credit guard {guard}");
        }
    }
}

#[cfg(test)]
mod epoch_state_version_adversarial_tests {
    use super::{DealerRuntimePayloadV1, BIND_EPOCH_ACCOUNT_COUNT, LAPSE_EPOCH_ACCOUNT_COUNT};
    use crate::instructions::dealer_runtime::{meta_contract_v1, DealerMetaRoleV1};
    use clutch_solana_layout::registry::{
        DealerFacilityAction, DEALER_FAMILY_TAG, DEALER_FAMILY_VERSION,
    };

    fn payload(admitted: bool) -> [u8; 32] {
        let mut value = [0u8; 32];
        value[0..8].copy_from_slice(&1u64.to_le_bytes());
        value[8..16].copy_from_slice(&2u64.to_le_bytes());
        value[16..20].copy_from_slice(&3u32.to_le_bytes());
        value[20] = u8::from(admitted);
        value[24..32].copy_from_slice(&4u64.to_le_bytes());
        value
    }

    #[test]
    fn epoch_actions_select_exact_state_version_and_product_child_shape() {
        for (action, base_count) in [
            (DealerFacilityAction::BindEpoch, BIND_EPOCH_ACCOUNT_COUNT),
            (DealerFacilityAction::LapseEpoch, LAPSE_EPOCH_ACCOUNT_COUNT),
        ] {
            let founding = DealerRuntimePayloadV1::decode(action, &payload(false)).unwrap();
            let founding_metas = meta_contract_v1(action, founding).unwrap();
            assert_eq!(founding_metas.len(), base_count);
            assert!(!founding_metas
                .iter()
                .any(|meta| meta.role == DealerMetaRoleV1::SeriesObligation));

            let admitted = DealerRuntimePayloadV1::decode(action, &payload(true)).unwrap();
            let admitted_metas = meta_contract_v1(action, admitted).unwrap();
            assert_eq!(admitted_metas.len(), base_count + 1);
            assert_eq!(
                admitted_metas[base_count].role,
                DealerMetaRoleV1::SeriesObligation,
            );
            assert!(!admitted_metas[base_count].writable);
        }
    }

    #[test]
    fn epoch_actions_refuse_ambiguous_version_or_tail_bytes() {
        for action in [DealerFacilityAction::BindEpoch, DealerFacilityAction::LapseEpoch] {
            let mut invalid_version = payload(false);
            invalid_version[20] = 2;
            assert!(DealerRuntimePayloadV1::decode(action, &invalid_version).is_err());

            let mut noncanonical_tail = payload(false);
            noncanonical_tail[21] = 1;
            assert!(DealerRuntimePayloadV1::decode(action, &noncanonical_tail).is_err());
        }
    }

    #[test]
    fn epoch_handlers_preserve_admitted_product_obligation_and_hostile_poststates() {
        let source = include_str!("dealer_facility.rs");
        let bind = source
            .split("fn bind_epoch")
            .nth(1)
            .and_then(|value| value.split("fn lapse_epoch").next())
            .expect("BindEpoch handler");
        for guard in [
            "authenticate_dealer_state_v3",
            "authenticate_state(program_id, &accounts[2])",
            "authenticate_live_series_obligation_for_state_v3",
            "current.with_base(prepared.state_after)",
            "observed_obligation.binding() == value",
            "observed_receipt == receipt",
            "observed_epoch == epoch",
            "liveness_transition.post_account_data",
        ] {
            assert!(bind.contains(guard), "missing BindEpoch guard {guard}");
        }

        let lapse = source
            .split("fn lapse_epoch")
            .nth(1)
            .and_then(|value| value.split("fn select_lease_and_begin").next())
            .expect("LapseEpoch handler");
        for guard in [
            "authenticate_dealer_state_v3",
            "authenticate_state(program_id, &accounts[2])",
            "authenticate_live_series_obligation_for_state_v3",
            "current.with_base(prepared.state_after)",
            "observed_obligation.binding() == value",
            "observed_receipt == receipt",
            "observed_position == position_after_observation",
            "apply_epoch_close(",
            "liveness_transition.post_account_data",
        ] {
            assert!(lapse.contains(guard), "missing LapseEpoch guard {guard}");
        }
        let close = source
            .split("fn apply_epoch_close")
            .nth(1)
            .and_then(|value| value.split("fn apply_liveness_transition").next())
            .expect("Epoch close adapter");
        for guard in [
            "credit_exact_dealer_terminal_lamports",
            "require_released_dealer_account(epoch_account)",
            "require_released_dealer_account(bind_receipt_account)",
        ] {
            assert!(close.contains(guard), "missing Epoch close guard {guard}");
        }
    }

    #[test]
    fn epoch_actions_remain_disabled_until_the_complete_family_closes() {
        for action in [DealerFacilityAction::BindEpoch, DealerFacilityAction::LapseEpoch] {
            assert!(!crate::capabilities::extension_intent_action_enabled(
                DEALER_FAMILY_TAG,
                DEALER_FAMILY_VERSION,
                action.tag(),
            ));
        }
    }
}

#[cfg(test)]
mod select_begin_adversarial_tests {
    use super::{select_begin_rent_principal, DealerRuntimePayloadV1};
    use crate::instructions::dealer_runtime::{meta_contract_v1, DealerMetaRoleV1};
    use clutch_solana_layout::registry::{
        DealerFacilityAction, DEALER_FAMILY_TAG, DEALER_FAMILY_VERSION,
    };

    #[test]
    fn keeper_payment_must_cover_every_new_refundable_principal() {
        assert_eq!(
            select_begin_rent_principal(11, 13, 17, 19, 60).unwrap(),
            60
        );
        assert!(select_begin_rent_principal(11, 13, 17, 19, 59).is_err());
        assert!(select_begin_rent_principal(11, 13, 17, 19, 0).is_err());
    }

    #[test]
    fn principal_sum_overflow_refuses_before_any_account_creation() {
        assert!(select_begin_rent_principal(u64::MAX, 1, 1, 1, u64::MAX).is_err());
        assert!(select_begin_rent_principal(1, u64::MAX, 1, 1, u64::MAX).is_err());
        assert!(select_begin_rent_principal(1, 1, u64::MAX, 1, u64::MAX).is_err());
        assert!(select_begin_rent_principal(1, 1, 1, u64::MAX, u64::MAX).is_err());
    }

    #[test]
    fn first_lease_promotes_only_with_the_exact_product_dealer_latch() {
        let mut payload = [0u8; 32];
        payload[0..8].copy_from_slice(&1u64.to_le_bytes());
        payload[8..16].copy_from_slice(&2u64.to_le_bytes());
        payload[16] = 1;
        payload[20..24].copy_from_slice(&3u32.to_le_bytes());
        payload[24..32].copy_from_slice(&4u64.to_le_bytes());
        let decoded = DealerRuntimePayloadV1::decode(
            DealerFacilityAction::SelectLeaseAndBegin,
            &payload,
        )
        .unwrap();
        let metas = meta_contract_v1(DealerFacilityAction::SelectLeaseAndBegin, decoded).unwrap();
        assert_eq!(metas.len(), 59);
        assert_eq!(metas[48].role, DealerMetaRoleV1::ProductMarketRoot);
        assert!(!metas[48].writable);
        assert_eq!(metas[49].role, DealerMetaRoleV1::SeriesRegistry);
        assert!(!metas[49].writable);
        assert_eq!(metas[50].role, DealerMetaRoleV1::CurrentProgram);
        assert!(!metas[50].writable);
        assert_eq!(metas[51].role, DealerMetaRoleV1::CurrentProgramData);
        assert!(!metas[51].writable);
        assert_eq!(metas[52].role, DealerMetaRoleV1::RegistryRelease);
        assert!(!metas[52].writable);
        assert_eq!(metas[53].role, DealerMetaRoleV1::CapabilityProfile);
        assert!(!metas[53].writable);
        assert_eq!(metas[54].role, DealerMetaRoleV1::SeriesMarketLink);
        assert!(metas[54].writable);
        assert_eq!(metas[55].role, DealerMetaRoleV1::CompilerBundle);
        assert_eq!(metas[56].role, DealerMetaRoleV1::Attachment);
        assert_eq!(metas[57].role, DealerMetaRoleV1::SeriesObligation);
        assert!(metas[57].writable);
        assert_eq!(metas[58].role, DealerMetaRoleV1::OrderPage);
    }

    #[test]
    fn later_lease_requires_read_only_live_product_admission_and_obligation() {
        let mut payload = [0u8; 32];
        payload[0..8].copy_from_slice(&2u64.to_le_bytes());
        payload[8..16].copy_from_slice(&7u64.to_le_bytes());
        payload[16] = 1;
        payload[17] = 1;
        payload[20..24].copy_from_slice(&8u32.to_le_bytes());
        payload[24..32].copy_from_slice(&9u64.to_le_bytes());
        let decoded = DealerRuntimePayloadV1::decode(
            DealerFacilityAction::SelectLeaseAndBegin,
            &payload,
        )
        .unwrap();
        assert!(decoded.existing_series_admission);
        let metas = meta_contract_v1(DealerFacilityAction::SelectLeaseAndBegin, decoded).unwrap();
        assert_eq!(metas.len(), 59);
        assert_eq!(metas[54].role, DealerMetaRoleV1::SeriesMarketLink);
        assert!(!metas[54].writable);
        assert_eq!(metas[57].role, DealerMetaRoleV1::SeriesObligation);
        assert_eq!(
            metas[57].owner,
            crate::instructions::dealer_runtime::DealerMetaOwnerV1::SelfProgram
        );
        assert!(!metas[57].writable);
    }

    #[test]
    fn series_admission_selector_is_canonical_and_cannot_escalate_privileges() {
        let mut payload = [0u8; 32];
        payload[0..8].copy_from_slice(&2u64.to_le_bytes());
        payload[8..16].copy_from_slice(&7u64.to_le_bytes());
        payload[16] = 1;
        payload[20..24].copy_from_slice(&8u32.to_le_bytes());
        payload[24..32].copy_from_slice(&9u64.to_le_bytes());

        payload[17] = 2;
        assert!(DealerRuntimePayloadV1::decode(
            DealerFacilityAction::SelectLeaseAndBegin,
            &payload,
        )
        .is_err());

        payload[17] = 1;
        payload[18] = 1;
        assert!(DealerRuntimePayloadV1::decode(
            DealerFacilityAction::SelectLeaseAndBegin,
            &payload,
        )
        .is_err());
    }

    #[test]
    fn product_admission_state_upgrade_and_obligation_write_are_one_outer() {
        let source = include_str!("dealer_facility.rs");
        let handler = source
            .split("fn select_lease_and_begin")
            .nth(1)
            .and_then(|value| value.split("fn create_lp_page").next())
            .expect("SelectLeaseAndBegin handler");
        for guard in [
            "authenticate_series_market_link_v2",
            "authenticate_market_lifecycle_root_v2",
            "authenticate_registry_capability_v4",
            "authenticate_live_series_dealer_obligation_v2",
            "admission_owner_receipt_id",
            "authenticate_dealer_series_admission_prewrite_v2",
            "authenticate_existing_dealer_series_admission_v2",
            "create_full_principal_pda",
            "fund_and_resize_program_account",
            "admit_series_dealer_obligation_v2",
            "DealerSeriesObligationBindingV2::new_live",
            "DealerStateV3::promote_current",
            "DEALER_STATE_V3_ACCOUNT_VERSION",
            "authenticate_dealer_series_obligation_v2",
            ".with_base(prepared.dealer.state_after)",
        ] {
            assert!(handler.contains(guard), "missing atomic promotion guard {guard}");
        }
    }

    #[test]
    fn existing_admission_rejoins_every_persisted_product_coordinate() {
        let source = include_str!("dealer_facility.rs");
        let authenticator = source
            .split("fn authenticate_existing_dealer_series_admission_v2")
            .nth(1)
            .and_then(|value| {
                value
                    .split("fn authenticate_dealer_collateral_value_v2")
                    .next()
            })
            .expect("existing Series admission authenticator");
        for guard in [
            "product.dealer_admission_receipt_id()",
            "product.registry_capability_id()",
            "state.series_obligation_binding_account_id",
            "state.series_obligation_binding_id",
            "obligation.key.product_market_root_account_id",
            "product.root_account()",
            "obligation.key.product_market_binding_id",
            "obligation.key.series_plan_v5_id",
            "obligation.key.series_market_link_account_id",
            "obligation.key.compiler_bundle_v6_id",
            "obligation.key.attachment_plan_v5_id",
            "obligation.key.product_generation",
            "obligation.key.series_ordinal",
            "product.link_transition_sequence()",
            "obligation.rent.neutral_sink",
        ] {
            assert!(authenticator.contains(guard), "missing existing admission join {guard}");
        }
    }

    #[test]
    fn dealer_admission_prewriter_owns_exact_pda_rent_and_product_receipt() {
        let source = include_str!("dealer_facility.rs");
        let prewriter = source
            .split("fn authenticate_dealer_series_admission_prewrite_v2")
            .nth(1)
            .and_then(|value| value.split("fn authenticate_dealer_collateral_value_v2").next())
            .expect("Dealer admission prewriter");
        for guard in [
            "state.state_content_id()",
            "link.state().transition_sequence()",
            "admission_owner_receipt_id",
            "seeds::dealer_series_obligation_pda",
            "state_account.data_len() == DEALER_STATE_V2_ACCOUNT_BYTES",
            "obligation_account.owner == &SYSTEM_PROGRAM_ID",
            "obligation_account.data_len() == 0",
            "obligation_account.lamports() == donation_floor",
            "refundable_principal != 0",
            "rent_payer.key.to_bytes() == link_binding.rent_refund_owner.bytes()",
            "registry.activation_consumed()",
            "DEALER_SERIES_ADMISSION_PREWRITE_DOMAIN_V1",
        ] {
            assert!(prewriter.contains(guard), "missing admission prewrite guard {guard}");
        }

        let owner = source
            .split("impl AuthenticatedSeriesDealerAdmissionOwnerV2")
            .nth(1)
            .and_then(|value| {
                value.split("fn authenticate_dealer_series_admission_prewrite_v1")
                    .next()
            })
            .expect("Dealer admission owner implementation");
        for guard in [
            "self.key.product_market_root_account_id",
            "self.key.compiler_bundle_v6_id",
            "self.key.attachment_plan_v5_id",
            "self.key.policy_id == Id::from_bytes(liquidity_facility_plan_id.bytes())",
            "self.owner_admission_receipt_id",
            "self.capability_profile_id",
        ] {
            assert!(owner.contains(guard), "missing retained owner guard {guard}");
        }
    }

    #[test]
    fn complete_begin_handler_remains_outside_every_current_capability_profile() {
        assert!(!crate::capabilities::extension_intent_action_enabled(
            DEALER_FAMILY_TAG,
            DEALER_FAMILY_VERSION,
            DealerFacilityAction::SelectLeaseAndBegin.tag(),
        ));
    }
}

#[cfg(test)]
mod sponsor_halt_adversarial_tests {
    use super::DealerRuntimePayloadV1;
    use crate::instructions::dealer_runtime::{meta_contract_v1, DealerMetaRoleV1};
    use clutch_solana_layout::registry::DealerFacilityAction;

    #[test]
    fn sponsor_halt_requires_the_complete_immutable_runtime_and_product_child() {
        let mut payload = [0u8; 24];
        payload[0..8].copy_from_slice(&1u64.to_le_bytes());
        payload[8..16].copy_from_slice(&2u64.to_le_bytes());
        payload[16] = 1;
        let decoded = DealerRuntimePayloadV1::decode(DealerFacilityAction::SponsorHalt, &payload)
            .unwrap();
        let metas = meta_contract_v1(DealerFacilityAction::SponsorHalt, decoded).unwrap();
        assert_eq!(metas.len(), 16);
        assert_eq!(metas[7].role, DealerMetaRoleV1::LivenessPolicy);
        assert_eq!(metas[8].role, DealerMetaRoleV1::LivenessSource);
        assert_eq!(metas[14].role, DealerMetaRoleV1::LivenessRecovery);
        assert_eq!(metas[15].role, DealerMetaRoleV1::SeriesObligation);
        assert!(!metas[15].writable);

        payload[16] = 0;
        let decoded = DealerRuntimePayloadV1::decode(DealerFacilityAction::SponsorHalt, &payload)
            .unwrap();
        let metas = meta_contract_v1(DealerFacilityAction::SponsorHalt, decoded).unwrap();
        assert_eq!(metas.len(), 15);
        assert!(!metas.iter().any(|meta| meta.role == DealerMetaRoleV1::SeriesObligation));
    }

    #[test]
    fn sponsor_halt_refuses_ambiguous_admission_or_noncanonical_tail() {
        let mut payload = [0u8; 24];
        payload[0..8].copy_from_slice(&1u64.to_le_bytes());
        payload[8..16].copy_from_slice(&2u64.to_le_bytes());
        payload[16] = 2;
        assert!(DealerRuntimePayloadV1::decode(
            DealerFacilityAction::SponsorHalt,
            &payload,
        )
        .is_err());

        payload[16] = 0;
        payload[17] = 1;
        assert!(DealerRuntimePayloadV1::decode(
            DealerFacilityAction::SponsorHalt,
            &payload,
        )
        .is_err());
    }

    #[test]
    fn sponsor_halt_preserves_the_exact_state_v3_obligation_and_replay() {
        let source = include_str!("dealer_facility.rs");
        let handler = source
            .split("fn sponsor_halt")
            .nth(1)
            .and_then(|value| value.split("fn bind_epoch").next())
            .expect("SponsorHalt handler");
        for guard in [
            "authenticate_dealer_state_v3",
            "authenticate_state(program_id, &accounts[2])",
            "authenticate_live_series_obligation_for_state_v3",
            "authenticate_runtime_bundle_with_access",
            "validate_runtime_dependency_join",
            "prepare_sponsor_halt_dealer_v3",
            ".with_base(prepared.state_after)",
            "DEALER_STATE_V3_ACCOUNT_VERSION",
            "obligation_matches",
            "observed_replay == prepared.replay.replay_post()",
        ] {
            assert!(handler.contains(guard), "missing sponsor-halt guard {guard}");
        }
    }
}

#[cfg(test)]
mod timed_close_adversarial_tests {
    use super::{
        DealerRuntimePayloadV1, ENTER_UNWIND_ACCOUNT_COUNT, TIMED_CLOSE_ACCOUNT_COUNT,
    };
    use crate::instructions::dealer_runtime::{meta_contract_v1, DealerMetaRoleV1};
    use clutch_solana_layout::registry::{
        DealerFacilityAction, DEALER_FAMILY_TAG, DEALER_FAMILY_VERSION,
    };

    fn payload() -> [u8; 32] {
        let mut payload = [0u8; 32];
        payload[0..8].copy_from_slice(&1u64.to_le_bytes());
        payload[8..16].copy_from_slice(&2u64.to_le_bytes());
        payload[16..20].copy_from_slice(&3u32.to_le_bytes());
        payload[20] = 1;
        payload[24..32].copy_from_slice(&4u64.to_le_bytes());
        payload
    }

    #[test]
    fn timed_close_requires_complete_retirement_liveness_and_product_child() {
        let decoded = DealerRuntimePayloadV1::decode(
            DealerFacilityAction::TimedClose,
            &payload(),
        )
        .unwrap();
        let metas = meta_contract_v1(DealerFacilityAction::TimedClose, decoded).unwrap();
        assert_eq!(metas.len(), TIMED_CLOSE_ACCOUNT_COUNT);
        assert_eq!(metas[7].role, DealerMetaRoleV1::LivenessPolicy);
        assert_eq!(metas[8].role, DealerMetaRoleV1::LivenessSource);
        assert_eq!(metas[13].role, DealerMetaRoleV1::LivenessRetirement);
        assert!(metas[13].writable);
        assert_eq!(metas[15].role, DealerMetaRoleV1::LivenessReceipt);
        assert!(metas[15].writable);
        assert_eq!(metas[20].role, DealerMetaRoleV1::SeriesObligation);
        assert!(!metas[20].writable);
    }

    #[test]
    fn queued_unwind_uses_the_same_retirement_plane_without_a_clock() {
        let decoded = DealerRuntimePayloadV1::decode(
            DealerFacilityAction::EnterUnwind,
            &payload(),
        )
        .unwrap();
        let metas = meta_contract_v1(DealerFacilityAction::EnterUnwind, decoded).unwrap();
        assert_eq!(metas.len(), ENTER_UNWIND_ACCOUNT_COUNT);
        assert_eq!(metas[13].role, DealerMetaRoleV1::LivenessRetirement);
        assert!(metas[13].writable);
        assert_eq!(metas[15].role, DealerMetaRoleV1::LivenessReceipt);
        assert_eq!(metas[17].role, DealerMetaRoleV1::Rent);
        assert_eq!(metas[18].role, DealerMetaRoleV1::SystemProgram);
        assert_eq!(metas[19].role, DealerMetaRoleV1::SeriesObligation);
        assert!(!metas.iter().any(|meta| meta.role == DealerMetaRoleV1::Clock));
    }

    #[test]
    fn pre_admission_unwind_uses_state_v2_without_a_fictional_product_child() {
        let mut pre_admission = payload();
        pre_admission[20] = 0;
        for action in [DealerFacilityAction::EnterUnwind, DealerFacilityAction::TimedClose] {
            let decoded = DealerRuntimePayloadV1::decode(action, &pre_admission).unwrap();
            let metas = meta_contract_v1(action, decoded).unwrap();
            let admitted_count = if action == DealerFacilityAction::EnterUnwind {
                ENTER_UNWIND_ACCOUNT_COUNT
            } else {
                TIMED_CLOSE_ACCOUNT_COUNT
            };
            assert_eq!(metas.len(), admitted_count - 1);
            assert!(!metas.iter().any(|meta| meta.role == DealerMetaRoleV1::SeriesObligation));
        }
        pre_admission[20] = 2;
        assert!(DealerRuntimePayloadV1::decode(
            DealerFacilityAction::TimedClose,
            &pre_admission,
        )
        .is_err());
    }

    #[test]
    fn timed_close_refuses_missing_liveness_ordinal_and_padding_drift() {
        let mut missing_ordinal = payload();
        missing_ordinal[16..20].copy_from_slice(&0u32.to_le_bytes());
        assert!(DealerRuntimePayloadV1::decode(
            DealerFacilityAction::TimedClose,
            &missing_ordinal,
        )
        .is_err());

        let mut padding = payload();
        padding[21] = 1;
        assert!(DealerRuntimePayloadV1::decode(
            DealerFacilityAction::TimedClose,
            &padding,
        )
        .is_err());
    }

    #[test]
    fn timed_close_composes_receipt_runtime_state_replay_and_product_obligation() {
        let source = include_str!("dealer_facility.rs");
        let handler = source
            .split("fn timed_close")
            .nth(1)
            .and_then(|value| value.split("fn bind_epoch").next())
            .expect("TimedClose handler");
        for guard in [
            "authenticate_dealer_state_v3",
            "authenticate_live_series_obligation_for_state_v3",
            "authenticate_runtime_bundle",
            "DealerLivenessCompartmentV1::Retirement",
            "plan_liveness_spend_absorbing_donation",
            "prepare_timed_close_dealer_v3",
            "create_full_principal_pda",
            "apply_liveness_transition",
            "DEALER_STATE_V3_ACCOUNT_VERSION",
            "authenticate_action_receipt",
            "obligation_matches",
            "observed_position == position",
            "liveness_transition.post_account_data",
        ] {
            assert!(handler.contains(guard), "missing TimedClose guard {guard}");
        }
    }

    #[test]
    fn queued_unwind_cannot_bypass_the_state_owned_share_threshold() {
        let contract = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../crates/clutch-dealer-runtime-contract/src/transitions_v3.rs"
        ));
        let transition = contract
            .split("pub fn prepare_enter_unwind_by_queue_v3")
            .nth(1)
            .and_then(|value| value.split("pub fn prepare_timed_close_dealer_v3").next())
            .expect("queued unwind successor");
        for guard in [
            "validate_v3_plane",
            "authorization.validate_against",
            "DealerRuntimeActionV1::EnterUnwind",
            "authorization.owner != state_account_id",
            "authorization.lifecycle_id != state.facility_id",
            "authorization.facility_generation != state.generation",
            "policy.shutdown_queue_threshold_met(state.queued_shares, state.total_shares)",
            "DealerTransitionLivenessModeV1::ExternalReceipt",
        ] {
            assert!(transition.contains(guard), "missing queue-unwind guard {guard}");
        }
    }

    #[test]
    fn timed_close_remains_disabled_until_the_complete_dealer_family_closes() {
        assert!(!crate::capabilities::extension_intent_action_enabled(
            DEALER_FAMILY_TAG,
            DEALER_FAMILY_VERSION,
            DealerFacilityAction::TimedClose.tag(),
        ));
    }
}

#[cfg(test)]
mod current_terminal_cut_adversarial_tests {
    use super::{
        DealerRuntimePayloadV1, RETIRE_ACTIVE_FACILITY_CREDIT_ACCOUNT_COUNT,
        RETIRE_UNUSED_FUTURE_CREDIT_ACCOUNT_COUNT,
    };
    use crate::instructions::dealer_runtime::{
        meta_contract_v1, DealerMetaOwnerV1, DealerMetaRoleV1,
        DEALER_RETIRE_ACTIVE_FACILITY_CREDIT_V1,
        DEALER_RETIRE_UNUSED_FUTURE_CREDIT_V1,
    };
    use clutch_solana_layout::registry::{
        DealerFacilityAction, DEALER_FAMILY_TAG, DEALER_FAMILY_VERSION,
    };

    fn payload(target: u8) -> [u8; 24] {
        let mut payload = [0u8; 24];
        payload[0..8].copy_from_slice(&11u64.to_le_bytes());
        payload[8..16].copy_from_slice(&17u64.to_le_bytes());
        payload[16] = target;
        payload
    }

    #[test]
    fn active_and_unused_credit_terminal_cuts_are_disjoint_exact_contracts() {
        let active = DealerRuntimePayloadV1::decode(
            DealerFacilityAction::Retire,
            &payload(DEALER_RETIRE_ACTIVE_FACILITY_CREDIT_V1),
        )
        .unwrap();
        let active = meta_contract_v1(DealerFacilityAction::Retire, active).unwrap();
        assert_eq!(active.len(), RETIRE_ACTIVE_FACILITY_CREDIT_ACCOUNT_COUNT);
        assert_eq!(active[24].role, DealerMetaRoleV1::SeriesObligation);
        assert!(active[24].writable);
        assert_eq!(active[25].role, DealerMetaRoleV1::ProductMarketRoot);
        assert!(!active[25].writable);
        assert_eq!(active[31].role, DealerMetaRoleV1::SeriesMarketLink);
        assert!(active[31].writable);
        assert_eq!(active[38].role, DealerMetaRoleV1::CollateralTokenProgramData);
        assert!(!active[38].writable);
        assert_eq!(active[43].role, DealerMetaRoleV1::ClaimLedger);
        assert!(active[43].writable);
        assert_eq!(active[46].role, DealerMetaRoleV1::FractionalLedger);
        assert!(active[46].writable);
        assert_eq!(active[47].role, DealerMetaRoleV1::FacilityCredit);
        assert_eq!(active[47].owner, DealerMetaOwnerV1::SelfProgram);
        assert!(active[47].writable);
        assert!(!active
            .iter()
            .any(|meta| meta.role == DealerMetaRoleV1::FutureCreditFunding));

        let unused = DealerRuntimePayloadV1::decode(
            DealerFacilityAction::Retire,
            &payload(DEALER_RETIRE_UNUSED_FUTURE_CREDIT_V1),
        )
        .unwrap();
        let unused = meta_contract_v1(DealerFacilityAction::Retire, unused).unwrap();
        assert_eq!(unused.len(), RETIRE_UNUSED_FUTURE_CREDIT_ACCOUNT_COUNT);
        assert_eq!(unused[43].role, DealerMetaRoleV1::ClaimLedger);
        assert!(!unused[43].writable);
        assert_eq!(unused[44].role, DealerMetaRoleV1::FutureCreditFunding);
        assert_eq!(unused[44].owner, DealerMetaOwnerV1::SelfProgram);
        assert!(unused[44].writable);
        assert!(!unused
            .iter()
            .any(|meta| meta.role == DealerMetaRoleV1::FacilityCredit));
    }

    #[test]
    fn terminal_cut_payload_refuses_unknown_target_page_fields_and_padding() {
        let mut unknown = payload(DEALER_RETIRE_UNUSED_FUTURE_CREDIT_V1);
        unknown[16] = DEALER_RETIRE_UNUSED_FUTURE_CREDIT_V1 + 1;
        assert!(DealerRuntimePayloadV1::decode(DealerFacilityAction::Retire, &unknown).is_err());

        let mut last_page = payload(DEALER_RETIRE_ACTIVE_FACILITY_CREDIT_V1);
        last_page[17] = 1;
        assert!(DealerRuntimePayloadV1::decode(DealerFacilityAction::Retire, &last_page).is_err());

        let mut padding = payload(DEALER_RETIRE_ACTIVE_FACILITY_CREDIT_V1);
        padding[18] = 1;
        assert!(DealerRuntimePayloadV1::decode(DealerFacilityAction::Retire, &padding).is_err());
    }

    #[test]
    fn terminal_cut_remains_unavailable_until_product_and_fractional_join() {
        assert!(!crate::capabilities::extension_intent_action_enabled(
            DEALER_FAMILY_TAG,
            DEALER_FAMILY_VERSION,
            DealerFacilityAction::Retire.tag(),
        ));
        let source = include_str!("dealer_facility.rs");
        let dispatcher = source
            .split("let implemented = matches!(")
            .nth(1)
            .and_then(|value| value.split(");").next())
            .expect("Dealer implementation mask");
        assert!(!dispatcher.contains("DealerFacilityAction::Retire"));
    }

    #[test]
    fn product_terminal_prewrite_binds_the_exact_live_dealer_cut() {
        let source = include_str!("dealer_facility.rs");
        let prewrite = source
            .split("fn authenticate_dealer_series_terminal_prewrite_v2")
            .nth(1)
            .and_then(|value| value.split("fn authenticate_dealer_collateral_value_v2").next())
            .expect("current Dealer terminal prewrite");
        for guard in [
            "product.root_authentication_id()",
            "product.link_authentication_id()",
            "product.registry_capability_id()",
            "state.state_id()",
            "obligation.binding_id()",
            "replay.replay_id()",
            "terminal_state_receipt.receipt_id()",
            "terminal_owner_receipt_id",
            "product.link_transition_sequence()",
            "obligation.admission_projection_id",
            "product.dealer_admission_receipt_id()",
            "obligation.key.compiler_bundle_v6_id",
            "obligation.key.attachment_plan_v5_id",
            "obligation.rent.refundable_principal",
            "obligation.rent.donation_floor",
            "DEALER_SERIES_TERMINAL_PREWRITE_DOMAIN_V2",
        ] {
            assert!(prewrite.contains(guard), "missing terminal prewrite guard {guard}");
        }
    }

    #[test]
    fn position_replay_close_uses_the_realm_bound_adapter_and_hostile_postcheck() {
        let source = include_str!("dealer_facility.rs");
        let prepare = source
            .split("fn prepare_dealer_position_replay_close_v3")
            .nth(1)
            .and_then(|value| value.split("fn apply_dealer_position_replay_close_v3").next())
            .expect("Dealer Position/Replay close preflight");
        for guard in [
            "replay_pre_ordinal.checked_add(1)",
            "terminal_replay.next_transition_ordinal() == expected_terminal_ordinal",
            "authenticate_position_v3_exact",
            "authenticate_purpose_replay_v3_exact",
            "PositionV3RetirementRealmV1::after_immutable_realm_authentication",
            "authenticate_and_prepare_position_replay_close_v4",
            "position_refund_owner.key.to_bytes()",
            "replay_refund_owner.key.to_bytes()",
            "neutral_sink.key",
            "DEALER_POSITION_REPLAY_CLOSE_POSTWRITE_DOMAIN_V3",
        ] {
            assert!(prepare.contains(guard), "missing close preflight guard {guard}");
        }
        assert!(prepare.contains("if replay_payer == position_payer"));

        let apply = source
            .split("fn apply_dealer_position_replay_close_v3")
            .nth(1)
            .and_then(|value| value.split("fn apply_epoch_close").next())
            .expect("Dealer Position/Replay close postwrite");
        for guard in [
            "position_tombstone_bytes",
            "POSITION_TOMBSTONE_V3_BYTES",
            "release_dealer_account(replay_account)",
            "PositionTombstoneV3::decode",
            "replay_account.owner == &SYSTEM_PROGRAM_ID",
            "replay_account.data_is_empty()",
            "credit.balance_after",
        ] {
            assert!(apply.contains(guard), "missing close postwrite guard {guard}");
        }
    }
}

#[cfg(test)]
mod queue_exit_adversarial_tests {
    use super::{
        DealerRuntimePayloadV1, QUEUE_EXIT_CALLER_EXISTING_ACCOUNT_COUNT,
        QUEUE_EXIT_CALLER_NEW_ACCOUNT_COUNT, QUEUE_EXIT_EXTERNAL_ACCOUNT_COUNT,
    };
    use crate::instructions::dealer_runtime::{meta_contract_v1, DealerMetaOwnerV1, DealerMetaRoleV1};
    use clutch_solana_layout::registry::{
        DealerFacilityAction, DEALER_FAMILY_TAG, DEALER_FAMILY_VERSION,
    };

    fn payload(existing: bool, external: bool) -> [u8; 48] {
        let mut payload = [0u8; 48];
        payload[0..8].copy_from_slice(&1u64.to_le_bytes());
        payload[8..16].copy_from_slice(&2u64.to_le_bytes());
        payload[16..20].copy_from_slice(&3u32.to_le_bytes());
        payload[20] = 4;
        payload[21] = u8::from(existing);
        payload[22] = u8::from(external);
        payload[23] = 1;
        payload[24..32].copy_from_slice(&5u64.to_le_bytes());
        if external {
            payload[32..36].copy_from_slice(&6u32.to_le_bytes());
            payload[40..48].copy_from_slice(&7u64.to_le_bytes());
        }
        payload
    }

    #[test]
    fn queue_contracts_separate_new_existing_and_external_funding() {
        let cases = [
            (false, false, QUEUE_EXIT_CALLER_NEW_ACCOUNT_COUNT),
            (true, false, QUEUE_EXIT_CALLER_EXISTING_ACCOUNT_COUNT),
            (false, true, QUEUE_EXIT_EXTERNAL_ACCOUNT_COUNT),
            (true, true, QUEUE_EXIT_EXTERNAL_ACCOUNT_COUNT),
        ];
        for (existing, external, expected_count) in cases {
            let decoded = DealerRuntimePayloadV1::decode(
                DealerFacilityAction::QueueExit,
                &payload(existing, external),
            )
            .unwrap();
            let metas = meta_contract_v1(DealerFacilityAction::QueueExit, decoded).unwrap();
            assert_eq!(metas.len(), expected_count);
            assert_eq!(metas[6].role, DealerMetaRoleV1::ExitTicket);
            assert_eq!(
                metas[6].owner,
                if existing {
                    DealerMetaOwnerV1::SelfProgram
                } else {
                    DealerMetaOwnerV1::System
                },
            );
            assert!(metas[6].writable);
            let obligation = if external { 21 } else { 7 };
            assert_eq!(metas[obligation].role, DealerMetaRoleV1::SeriesObligation);
            assert!(!metas[obligation].writable);
            if external {
                assert_eq!(metas[9].role, DealerMetaRoleV1::LivenessPolicy);
                assert_eq!(metas[15].role, DealerMetaRoleV1::LivenessRetirement);
                assert!(metas[15].writable);
                assert_eq!(metas[17].owner, DealerMetaOwnerV1::System);
                assert!(metas[17].writable);
            }
        }
    }

    #[test]
    fn queue_before_first_lease_omits_only_the_product_obligation() {
        for (existing, external, admitted_count) in [
            (false, false, QUEUE_EXIT_CALLER_NEW_ACCOUNT_COUNT),
            (true, false, QUEUE_EXIT_CALLER_EXISTING_ACCOUNT_COUNT),
            (false, true, QUEUE_EXIT_EXTERNAL_ACCOUNT_COUNT),
            (true, true, QUEUE_EXIT_EXTERNAL_ACCOUNT_COUNT),
        ] {
            let mut frame = payload(existing, external);
            frame[23] = 0;
            let decoded = DealerRuntimePayloadV1::decode(DealerFacilityAction::QueueExit, &frame)
                .unwrap();
            let metas = meta_contract_v1(DealerFacilityAction::QueueExit, decoded).unwrap();
            assert_eq!(metas.len(), admitted_count - 1);
            assert!(!metas.iter().any(|meta| meta.role == DealerMetaRoleV1::SeriesObligation));
            if !external && !existing {
                assert_eq!(metas[7].role, DealerMetaRoleV1::Rent);
                assert_eq!(metas[8].role, DealerMetaRoleV1::SystemProgram);
            }
        }

        let mut invalid = payload(false, false);
        invalid[23] = 2;
        assert!(DealerRuntimePayloadV1::decode(DealerFacilityAction::QueueExit, &invalid).is_err());
    }

    #[test]
    fn queue_payload_refuses_detached_or_ambiguous_liveness_fields() {
        let mut missing = payload(false, true);
        missing[32..36].copy_from_slice(&0u32.to_le_bytes());
        assert!(DealerRuntimePayloadV1::decode(DealerFacilityAction::QueueExit, &missing).is_err());

        let mut caller_with_payment = payload(false, false);
        caller_with_payment[40..48].copy_from_slice(&1u64.to_le_bytes());
        assert!(DealerRuntimePayloadV1::decode(
            DealerFacilityAction::QueueExit,
            &caller_with_payment,
        )
        .is_err());

        let mut padding = payload(false, true);
        padding[36] = 1;
        assert!(DealerRuntimePayloadV1::decode(DealerFacilityAction::QueueExit, &padding).is_err());
    }

    #[test]
    fn queue_handler_retains_owner_ticket_rent_replay_and_product_child() {
        let source = include_str!("dealer_facility.rs");
        let handler = source
            .split("fn queue_exit")
            .nth(1)
            .and_then(|value| value.split("fn bind_epoch").next())
            .expect("QueueExit handler");
        for guard in [
            "authenticate_dealer_state_v3",
            "authenticate_live_series_obligation_for_state_v3",
            "page.entries[usize::from(payload.entry_index)].owner == id(accounts[0].key)",
            "seeds::dealer_exit_ticket_pda",
            "authenticate_exit_ticket",
            "DealerQueueExitLivenessV1::external",
            "DealerQueueExitLivenessV1::caller_funded",
            "prepare_new_exit_ticket_v1",
            "prepare_increase_exit_ticket_v1",
            "create_full_principal_pda",
            "DEALER_EXIT_TICKET_ACCOUNT_VERSION",
            "obligation_matches",
            "observed_position == position",
        ] {
            assert!(handler.contains(guard), "missing QueueExit guard {guard}");
        }
    }

    #[test]
    fn queue_exit_remains_disabled_until_the_complete_dealer_family_closes() {
        assert!(!crate::capabilities::extension_intent_action_enabled(
            DEALER_FAMILY_TAG,
            DEALER_FAMILY_VERSION,
            DealerFacilityAction::QueueExit.tag(),
        ));
    }
}

#[cfg(test)]
mod collect_deliver_adversarial_tests {
    use super::DealerRuntimePayloadV1;
    use crate::instructions::dealer_runtime::{meta_contract_v1, DealerMetaRoleV1};
    use clutch_solana_layout::registry::{
        DealerFacilityAction, DEALER_FAMILY_TAG, DEALER_FAMILY_VERSION,
    };

    fn payload() -> [u8; 40] {
        let mut value = [0u8; 40];
        value[0..8].copy_from_slice(&7u64.to_le_bytes());
        value[8..16].copy_from_slice(&9u64.to_le_bytes());
        value[16..18].copy_from_slice(&3u16.to_le_bytes());
        value[18..20].copy_from_slice(&1u16.to_le_bytes());
        value[20] = 4;
        value[24..28].copy_from_slice(&11u32.to_le_bytes());
        value[32..40].copy_from_slice(&13u64.to_le_bytes());
        value
    }

    #[test]
    fn row_frame_requires_one_row_authenticated_page_count_and_liveness() {
        let exact = payload();
        for action in [DealerFacilityAction::Collect, DealerFacilityAction::Deliver] {
            let decoded = DealerRuntimePayloadV1::decode(action, &exact).unwrap();
            assert_eq!(decoded.row_start, 3);
            assert_eq!(decoded.row_count, 1);
            assert_eq!(decoded.book_page_count, 4);
            assert_eq!(decoded.liveness_call_ordinal, 11);
            assert_eq!(decoded.keeper_payment_lamports, 13);
        }
        let mut two_rows = exact;
        two_rows[18..20].copy_from_slice(&2u16.to_le_bytes());
        assert!(DealerRuntimePayloadV1::decode(DealerFacilityAction::Collect, &two_rows).is_err());
        let mut no_pages = exact;
        no_pages[20] = 0;
        assert!(DealerRuntimePayloadV1::decode(DealerFacilityAction::Collect, &no_pages).is_err());
        let mut padding = exact;
        padding[21] = 1;
        assert!(DealerRuntimePayloadV1::decode(DealerFacilityAction::Deliver, &padding).is_err());
    }

    #[test]
    fn complete_handlers_remain_outside_every_current_capability_profile() {
        for action in [DealerFacilityAction::Collect, DealerFacilityAction::Deliver] {
            assert!(!crate::capabilities::extension_intent_action_enabled(
                DEALER_FAMILY_TAG,
                DEALER_FAMILY_VERSION,
                action.tag(),
            ));
        }
    }

    #[test]
    fn row_frame_authenticates_the_counted_facility_product_obligation_before_pages() {
        let decoded = DealerRuntimePayloadV1::decode(
            DealerFacilityAction::Collect,
            &payload(),
        )
        .unwrap();
        let metas = meta_contract_v1(DealerFacilityAction::Collect, decoded).unwrap();
        assert_eq!(metas.len(), 47);
        assert_eq!(metas[33].role, DealerMetaRoleV1::SeriesObligation);
        assert!(!metas[33].writable);
        assert_eq!(metas[37].role, DealerMetaRoleV1::CollateralTokenProgram);
        assert_eq!(metas[38].role, DealerMetaRoleV1::CollateralTokenProgramData);
        assert_eq!(metas[41].role, DealerMetaRoleV1::Hoard);
        assert_eq!(metas[42].role, DealerMetaRoleV1::ClaimLedger);
        assert!(metas[34..43].iter().all(|meta| !meta.writable && !meta.signer));
        assert!(metas[43..]
            .iter()
            .all(|meta| meta.role == DealerMetaRoleV1::OrderPage));
    }
}

#[cfg(test)]
mod finalize_abort_adversarial_tests {
    use super::DealerRuntimePayloadV1;
    use crate::instructions::dealer_runtime::{meta_contract_v1, DealerMetaRoleV1};
    use clutch_solana_layout::registry::{
        DealerFacilityAction, DEALER_FAMILY_TAG, DEALER_FAMILY_VERSION,
    };

    fn payload() -> [u8; 32] {
        let mut value = [0u8; 32];
        value[0..8].copy_from_slice(&7u64.to_le_bytes());
        value[8..16].copy_from_slice(&9u64.to_le_bytes());
        value[16..20].copy_from_slice(&11u32.to_le_bytes());
        value[24..32].copy_from_slice(&13u64.to_le_bytes());
        value
    }

    #[test]
    fn terminal_payload_rejects_padding_stale_width_and_zero_call_ordinal() {
        for action in [
            DealerFacilityAction::FinalizeSettlement,
            DealerFacilityAction::AbortBeforeCollection,
        ] {
            let exact = payload();
            let decoded = DealerRuntimePayloadV1::decode(action, &exact).unwrap();
            assert_eq!(decoded.expected_generation, 7);
            assert_eq!(decoded.expected_replay_ordinal, 9);
            assert_eq!(decoded.liveness_call_ordinal, 11);
            assert_eq!(decoded.keeper_payment_lamports, 13);
            assert!(DealerRuntimePayloadV1::decode(action, &exact[..31]).is_err());
            let mut padding = exact;
            padding[20] = 1;
            assert!(DealerRuntimePayloadV1::decode(action, &padding).is_err());
            let mut no_ordinal = exact;
            no_ordinal[16..20].fill(0);
            assert!(DealerRuntimePayloadV1::decode(action, &no_ordinal).is_err());
        }
    }

    #[test]
    fn only_the_action_owned_liveness_compartment_is_writable() {
        let decoded = DealerRuntimePayloadV1::decode(
            DealerFacilityAction::FinalizeSettlement,
            &payload(),
        )
        .unwrap();
        let finalize = meta_contract_v1(DealerFacilityAction::FinalizeSettlement, decoded).unwrap();
        let abort = meta_contract_v1(DealerFacilityAction::AbortBeforeCollection, decoded).unwrap();
        assert_eq!(finalize.len(), 40);
        assert_eq!(abort.len(), 40);
        assert_eq!(
            finalize[29].role,
            DealerMetaRoleV1::SeriesObligation
        );
        assert_eq!(abort[29].role, DealerMetaRoleV1::SeriesObligation);
        assert!(!finalize[29].writable);
        assert!(!abort[29].writable);
        assert_eq!(finalize[33].role, DealerMetaRoleV1::CollateralTokenProgram);
        assert_eq!(finalize[34].role, DealerMetaRoleV1::CollateralTokenProgramData);
        assert!(finalize[30..].iter().all(|meta| !meta.writable && !meta.signer));
        assert!(abort[30..].iter().all(|meta| !meta.writable && !meta.signer));
        let finalize_settlement = finalize
            .iter()
            .find(|meta| meta.role == DealerMetaRoleV1::LivenessSettlement)
            .unwrap();
        let finalize_recovery = finalize
            .iter()
            .find(|meta| meta.role == DealerMetaRoleV1::LivenessRecovery)
            .unwrap();
        let abort_settlement = abort
            .iter()
            .find(|meta| meta.role == DealerMetaRoleV1::LivenessSettlement)
            .unwrap();
        let abort_recovery = abort
            .iter()
            .find(|meta| meta.role == DealerMetaRoleV1::LivenessRecovery)
            .unwrap();
        assert!(finalize_settlement.writable);
        assert!(!finalize_recovery.writable);
        assert!(!abort_settlement.writable);
        assert!(abort_recovery.writable);
    }

    #[test]
    fn terminal_handlers_remain_outside_every_current_capability_profile() {
        for action in [
            DealerFacilityAction::FinalizeSettlement,
            DealerFacilityAction::AbortBeforeCollection,
        ] {
            assert!(!crate::capabilities::extension_intent_action_enabled(
                DEALER_FAMILY_TAG,
                DEALER_FAMILY_VERSION,
                action.tag(),
            ));
        }
    }
}

#[cfg(test)]
mod terminal_claim_adversarial_tests {
    use super::DealerRuntimePayloadV1;
    use crate::instructions::dealer_runtime::{
        meta_contract_v1, DealerMetaOwnerV1, DealerMetaRoleV1,
    };
    use clutch_solana_layout::registry::{
        DealerFacilityAction, DEALER_FAMILY_TAG, DEALER_FAMILY_VERSION,
    };

    fn payload() -> [u8; 48] {
        let mut value = [0u8; 48];
        value[0..8].copy_from_slice(&7u64.to_le_bytes());
        value[8..16].copy_from_slice(&9u64.to_le_bytes());
        value[16..20].copy_from_slice(&3u32.to_le_bytes());
        value[20] = 4;
        value[24..32].copy_from_slice(&11u64.to_le_bytes());
        value[32..36].copy_from_slice(&13u32.to_le_bytes());
        value[40..48].copy_from_slice(&17u64.to_le_bytes());
        value
    }

    #[test]
    fn claim_payload_refuses_stale_general_replay_and_padding() {
        let exact = payload();
        let decoded = DealerRuntimePayloadV1::decode(DealerFacilityAction::Claim, &exact).unwrap();
        assert_eq!(decoded.expected_generation, 7);
        assert_eq!(decoded.expected_replay_ordinal, 9);
        assert_eq!(decoded.page_ordinal, 3);
        assert_eq!(decoded.entry_index, 4);
        assert_eq!(decoded.expected_general_replay_sequence, 11);
        assert_eq!(decoded.liveness_call_ordinal, 13);
        assert_eq!(decoded.keeper_payment_lamports, 17);

        let mut stale = exact;
        stale[24..32].fill(0);
        assert!(DealerRuntimePayloadV1::decode(DealerFacilityAction::Claim, &stale).is_err());
        let mut no_liveness = exact;
        no_liveness[32..36].fill(0);
        assert!(DealerRuntimePayloadV1::decode(DealerFacilityAction::Claim, &no_liveness).is_err());
        let mut page_padding = exact;
        page_padding[21] = 1;
        assert!(DealerRuntimePayloadV1::decode(DealerFacilityAction::Claim, &page_padding).is_err());
        let mut liveness_padding = exact;
        liveness_padding[36] = 1;
        assert!(DealerRuntimePayloadV1::decode(DealerFacilityAction::Claim, &liveness_padding)
            .is_err());
        assert!(DealerRuntimePayloadV1::decode(DealerFacilityAction::Claim, &exact[..47]).is_err());
    }

    #[test]
    fn claim_contract_owns_both_replays_current_release_and_exact_liveness() {
        let decoded = DealerRuntimePayloadV1::decode(DealerFacilityAction::Claim, &payload())
            .unwrap();
        let contract = meta_contract_v1(DealerFacilityAction::Claim, decoded).unwrap();
        assert_eq!(contract.len(), 35);
        assert_eq!(contract[4].role, DealerMetaRoleV1::FacilityReplay);
        assert!(contract[4].writable);
        assert_eq!(contract[6].role, DealerMetaRoleV1::GeneralReplay);
        assert!(contract[6].writable);
        assert_eq!(contract[16].role, DealerMetaRoleV1::LivenessSettlement);
        assert!(contract[16].writable);
        assert_eq!(contract[20].role, DealerMetaRoleV1::LivenessReceipt);
        assert_eq!(contract[20].owner, DealerMetaOwnerV1::System);
        assert_eq!(contract[27].role, DealerMetaRoleV1::CollateralTokenProgram);
        assert_eq!(contract[27].owner, DealerMetaOwnerV1::ExternalExecutable);
        assert_eq!(contract[28].role, DealerMetaRoleV1::CollateralTokenProgramData);
        assert_eq!(contract[32].role, DealerMetaRoleV1::Hoard);
        assert_eq!(contract[33].role, DealerMetaRoleV1::ClaimLedger);
        assert_eq!(contract[34].role, DealerMetaRoleV1::SeriesObligation);
        assert!(contract[24..35].iter().all(|meta| !meta.signer && !meta.writable));
    }

    #[test]
    fn claim_handler_has_no_token_cpi_or_detached_position_authority() {
        let source = include_str!("dealer_facility.rs");
        let handler = source
            .split("fn claim_terminal_allocation")
            .nth(1)
            .and_then(|value| value.split("fn queue_exit").next())
            .expect("Claim handler");
        for guard in [
            "authenticate_dealer_state_v3",
            "authenticate_live_series_obligation_for_state_v3",
            "authenticate_dealer_collateral_value_v2",
            "authenticate_general_position_replay_v2",
            "prepare_dealer_terminal_claim_v2",
            "prepare_dealer_terminal_claim_replay_v2",
            "accept_dealer_asset_transfer_postwrite_v2",
            "write_and_accept_general_replay_v1",
            "apply_liveness_transition",
            "observed_obligation.binding() == &obligation",
        ] {
            assert!(handler.contains(guard), "missing Claim guard {guard}");
        }
        assert!(!handler.contains("invoke_signed"));
        assert!(!handler.contains("transfer_checked"));
    }

    #[test]
    fn claim_remains_disabled_until_vector_resolution_and_retirement_close() {
        assert!(!crate::capabilities::extension_intent_action_enabled(
            DEALER_FAMILY_TAG,
            DEALER_FAMILY_VERSION,
            DealerFacilityAction::Claim.tag(),
        ));
    }
}

#[cfg(test)]
mod resolve_vector_adversarial_tests {
    use super::DealerRuntimePayloadV1;
    use crate::instructions::dealer_runtime::{meta_contract_v1, DealerMetaRoleV1};
    use clutch_solana_layout::registry::{
        DealerFacilityAction, DEALER_FAMILY_TAG, DEALER_FAMILY_VERSION,
    };

    fn payload() -> [u8; 184] {
        let mut value = [0u8; 184];
        value[0..8].copy_from_slice(&7u64.to_le_bytes());
        value[8..16].copy_from_slice(&9u64.to_le_bytes());
        value[16..24].copy_from_slice(&11u64.to_le_bytes());
        value[24..32].copy_from_slice(&1u64.to_le_bytes());
        value[32] = 2;
        value[40..48].copy_from_slice(&13u64.to_le_bytes());
        value[48..56].copy_from_slice(&17u64.to_le_bytes());
        value[168..172].copy_from_slice(&19u32.to_le_bytes());
        value[176..184].copy_from_slice(&23u64.to_le_bytes());
        value
    }

    #[test]
    fn vector_payload_refuses_stale_credit_padding_and_inactive_inventory() {
        let exact = payload();
        let decoded = DealerRuntimePayloadV1::decode(DealerFacilityAction::Resolve, &exact)
            .unwrap();
        assert_eq!(decoded.expected_generation, 7);
        assert_eq!(decoded.expected_replay_ordinal, 9);
        assert_eq!(decoded.expected_fractional_ledger_sequence, 11);
        assert_eq!(decoded.expected_fractional_credit_sequence, 1);
        assert_eq!(decoded.resolution_quantities[0], 13);
        assert_eq!(decoded.resolution_quantities[1], 17);

        let mut stale_credit = exact;
        stale_credit[24..32].copy_from_slice(&2u64.to_le_bytes());
        assert!(DealerRuntimePayloadV1::decode(
            DealerFacilityAction::Resolve,
            &stale_credit,
        )
        .is_err());
        let mut header_padding = exact;
        header_padding[33] = 1;
        assert!(DealerRuntimePayloadV1::decode(
            DealerFacilityAction::Resolve,
            &header_padding,
        )
        .is_err());
        let mut inactive = exact;
        inactive[56..64].copy_from_slice(&1u64.to_le_bytes());
        assert!(DealerRuntimePayloadV1::decode(DealerFacilityAction::Resolve, &inactive)
            .is_err());
        let mut liveness_padding = exact;
        liveness_padding[172] = 1;
        assert!(DealerRuntimePayloadV1::decode(
            DealerFacilityAction::Resolve,
            &liveness_padding,
        )
        .is_err());
    }

    #[test]
    fn vector_contract_carries_the_complete_current_authority_and_one_shot_credit() {
        let decoded = DealerRuntimePayloadV1::decode(DealerFacilityAction::Resolve, &payload())
            .unwrap();
        let contract = meta_contract_v1(DealerFacilityAction::Resolve, decoded).unwrap();
        assert_eq!(contract.len(), 41);
        assert_eq!(contract[18].role, DealerMetaRoleV1::FutureCreditFunding);
        assert!(contract[18].writable);
        assert_eq!(contract[21].role, DealerMetaRoleV1::Clock);
        assert_eq!(contract[24].role, DealerMetaRoleV1::ProductMarketRoot);
        assert_eq!(contract[25].role, DealerMetaRoleV1::SeriesObligation);
        assert_eq!(contract[26].role, DealerMetaRoleV1::SeriesMarketLink);
        assert_eq!(contract[30].role, DealerMetaRoleV1::CollateralTokenProgram);
        assert_eq!(contract[31].role, DealerMetaRoleV1::CollateralTokenProgramData);
        assert_eq!(contract[35].role, DealerMetaRoleV1::Hoard);
        assert!(contract[35].writable);
        assert_eq!(contract[36].role, DealerMetaRoleV1::ClaimLedger);
        assert!(contract[36].writable);
        assert_eq!(contract[39].role, DealerMetaRoleV1::FractionalLedger);
        assert!(contract[39].writable);
        assert_eq!(contract[40].role, DealerMetaRoleV1::FacilityCredit);
        assert!(contract[40].writable);
    }

    #[test]
    fn vector_handler_stays_profile_disabled_until_product_admission_is_callable() {
        assert!(!crate::capabilities::extension_intent_action_enabled(
            DEALER_FAMILY_TAG,
            DEALER_FAMILY_VERSION,
            DealerFacilityAction::Resolve.tag(),
        ));
        let source = include_str!("dealer_facility.rs");
        let handler = source
            .split("fn resolve_facility_vector")
            .nth(1)
            .and_then(|value| value.split("fn claim_terminal_allocation").next())
            .expect("Resolve handler");
        for guard in [
            "authenticate_dealer_series_obligation_v2",
            "authenticate_current_product_resolution_v2",
            "authenticate_future_credit_funding",
            "apply_dealer_facility_vector_transition_v1",
            "begin_terminal_resolution_v1",
            "counted_generation: vector.facility_post_generation()",
            "prepare_transition",
            "facility_post_generation",
            "observed_obligation.binding() == &obligation",
        ] {
            assert!(handler.contains(guard), "missing Resolve guard {guard}");
        }
    }
}

#[cfg(test)]
mod future_credit_terminal_adversarial_tests {
    #[test]
    fn unused_close_is_terminal_product_bound_and_partitions_exact_lamports() {
        let source = include_str!("dealer_facility.rs");
        let helper = source
            .split("fn close_unused_future_credit_funding_v1")
            .nth(1)
            .and_then(|value| value.split("impl AuthenticatedDealerFacilityVectorAuthorityV1").next())
            .expect("unused future-credit close helper");
        for guard in [
            "authenticate_dealer_state_v3",
            "authenticate_dealer_series_obligation_v2",
            "authenticate_future_credit_funding",
            "prepare_unused_close",
            "plan.terminal_obligation_binding_id",
            "release_dealer_account",
            "plan.refundable_principal_lamports",
            "plan.neutral_sink_credit_lamports",
            "require_released_dealer_account",
            "DEALER_FUTURE_CREDIT_UNUSED_CLOSE_POSTWRITE_DOMAIN_V1",
        ] {
            assert!(helper.contains(guard), "missing unused-close guard {guard}");
        }
    }

    #[test]
    fn unused_close_cannot_alias_refund_sink_or_leave_a_detachable_raw_plan() {
        let source = include_str!("dealer_facility.rs");
        let helper = source
            .split("fn close_unused_future_credit_funding_v1")
            .nth(1)
            .and_then(|value| value.split("impl AuthenticatedDealerFacilityVectorAuthorityV1").next())
            .expect("unused future-credit close helper");
        for guard in [
            "funding_account.key != refund_owner.key",
            "funding_account.key != neutral_sink.key",
            "refund_owner.key != neutral_sink.key",
            "neutral_sink.owner == &SYSTEM_PROGRAM_ID",
            "neutral_sink.data_is_empty()",
        ] {
            assert!(helper.contains(guard), "missing alias/owner guard {guard}");
        }
        assert!(!source.contains("pub(crate) fn close_unused_future_credit_funding_v1"));
        assert!(!source.contains("pub fn close_unused_future_credit_funding_v1"));
    }
}
