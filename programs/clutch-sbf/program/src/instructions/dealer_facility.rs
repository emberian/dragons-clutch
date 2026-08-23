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
    advance_collect_v2, advance_deliver_v2, bind_dealer_fee_terminal_v1,
    prepare_abort_lease_pot_v3, prepare_covered_dealer_row_progress_v1,
    dealer_runtime_liveness_policy_id_v1,
    prepare_finalize_lease_pot_v3,
    prepare_begin_covered_lease_pot_v4, prepare_bind_epoch_v3,
    prepare_dealer_sponsor_funding_transfer_v1,
    prepare_activate_dealer_v3, prepare_cancel_stale_funding_v3,
    prepare_dealer_lp_share_transfer_v1, prepare_dealer_sponsor_refund_transfer_v1,
    prepare_facility_initialization_v3, prepare_first_lp_page_v2,
    prepare_lapse_epoch_v3, project_covered_dealer_position_v1,
    prepare_lp_contribution_v2, prepare_lp_withdrawal_v2, prepare_next_lp_page_v2,
    prepare_refund_cancelled_sponsor_v3, CoveredDealerSelectionContextV1,
    CoveredDealerRowAssetTransitionV1, CoveredDealerSelectionV1, CoveredDealerTerminalV2,
    DealerActionReceiptV1,
    DealerChildCountsV2,
    DealerEpochCloseCreditsV2, DealerEpochCloseRentV2,
    DealerEpochBindingV2, DealerFacilityGenesisV1, DealerFacilityReplayV1,
    DealerFundedBudgetDependenciesV1, DealerFundedDependenciesV2, DealerGeneralEpochEvidenceV3,
    DealerLivenessCompartmentV1, DealerLivenessScheduleV1, DealerPhaseV2,
    DealerPositionMarketJoinV1, DealerPositionObservationV3, DealerReplayAccountBindingV1,
    DealerRuntimeActionV1, DealerRuntimeLivenessBindingV1, DealerSelectedFeeRecordBindingV1,
    DealerStateV2, DealerTransferPositionV3, DealerLeasePotCloseRentV3, DealerLeaseV2,
    DeletableRentOwnerV1,
    FacilityPositionBindingV2, FixedCodec, Id, LpPageV2, RootRentOwnerV1,
    SettlementPotPhaseV1, SettlementPotV2, SponsorCapitalDispositionV1,
};
use clutch_general_v2_contract::{
    fee_runtime_semantic_release_id_v1,
    project_general_position_replay_prestate_v1, project_general_replay_transition_v1,
    CandidateWindowV4AccountV1, EconomicDomainV2AccountV1, GeneralEpochV6AccountV1,
    GeneralPositionReplayPrestateV1, GeneralReplayTransitionKindV1, MarketBindingV2,
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
    ContentId, MarketGenesisProfileV2, MarketInstancePreimageV2, NativeClaimBasisV1,
    PriceMeasurePolicyV1, ProductTemplateV4, QuantizedEdgePolicyV1,
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
    PositionV3Sha256Backend, RentSplitV2, POSITION_TOMBSTONE_V3_BYTES, POSITION_V3_BYTES,
    ReplayV3Envelope,
};
use clutch_owner_settlement::{AuthenticatedPositionV3, PositionSettlementPoststateV3};
use clutch_solana_layout::registry::{
    DealerFacilityAction, DEALER_ACTION_RECEIPT_ACCOUNT_BYTES, DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
    DEALER_ACTION_RECEIPT_ACCOUNT_VERSION, DEALER_EPOCH_BINDING_V2_ACCOUNT_BYTES,
    DEALER_EPOCH_BINDING_V2_ACCOUNT_TAG, DEALER_EPOCH_BINDING_V2_ACCOUNT_VERSION,
    DEALER_COVERED_SELECTION_ACCOUNT_BYTES, DEALER_COVERED_SELECTION_ACCOUNT_TAG,
    DEALER_COVERED_SELECTION_ACCOUNT_VERSION, DEALER_COVERED_TERMINAL_ACCOUNT_VERSION,
    DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES, DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG,
    DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION, DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES,
    DEALER_LIVENESS_SCHEDULE_ACCOUNT_TAG, DEALER_LIVENESS_SCHEDULE_ACCOUNT_VERSION,
    DEALER_LP_PAGE_V2_ACCOUNT_BYTES, DEALER_LP_PAGE_V2_ACCOUNT_TAG,
    DEALER_LP_PAGE_V2_ACCOUNT_VERSION, DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_BYTES,
    DEALER_LEASE_V2_ACCOUNT_BYTES, DEALER_LEASE_V2_ACCOUNT_TAG,
    DEALER_LEASE_V2_ACCOUNT_VERSION, DEALER_SETTLEMENT_POT_V2_ACCOUNT_BYTES,
    DEALER_SETTLEMENT_POT_V2_ACCOUNT_TAG, DEALER_SETTLEMENT_POT_V2_ACCOUNT_VERSION,
    DEALER_STATE_V2_ACCOUNT_BYTES,
    DEALER_STATE_V2_ACCOUNT_TAG, DEALER_STATE_V2_ACCOUNT_VERSION,
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
use solana_pubkey::Pubkey;

use super::artifact::read_clock_slot;
use super::collateral_position_v3::RuntimeSha256;
use super::dealer_policy::{
    authenticate_catalog_policy, create_exact_payer_debit_pda, create_full_principal_pda,
    dealer_fault,
};
use super::product_artifact::{
    authenticate_product_artifact_v1, AuthenticatedProductArtifactV1,
};
use crate::instructions_sysvar::{InstructionsSysvarV1, SYSVAR_OWNER_ID};
use super::dealer_runtime::{
    decode_dealer_account_body_v1, encode_dealer_account_body_v1, DealerRuntimePayloadV1,
};
use super::genesis::{
    read_rent, require_creatable, require_system_program, SYSTEM_PROGRAM_ID,
};

const INITIALIZE_ACCOUNT_COUNT: usize = 22;
const CREATE_FIRST_LP_PAGE_ACCOUNT_COUNT: usize = 20;
const CREATE_NEXT_LP_PAGE_ACCOUNT_COUNT: usize = 21;
const LP_TRANSFER_ACCOUNT_COUNT: usize = 7;
const ACTIVATE_ACCOUNT_COUNT: usize = 21;
const CANCEL_FUNDING_ACCOUNT_COUNT: usize = 20;
const REFUND_CANCELLED_SPONSOR_ACCOUNT_COUNT: usize = 20;
const BIND_EPOCH_ACCOUNT_COUNT: usize = 24;
const LAPSE_EPOCH_ACCOUNT_COUNT: usize = 25;
const SELECT_LEASE_BEGIN_FIXED_ACCOUNT_COUNT: usize = 40;
const COLLECT_DELIVER_FIXED_ACCOUNT_COUNT: usize = 33;
const FINALIZE_ABORT_ACCOUNT_COUNT: usize = 29;

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

fn authenticate_lp_page(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<LpPageV2> {
    let (bump, page) = dealer_body::<LpPageV2>(
        program_id,
        account,
        true,
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
        require(
            account.is_writable == (index == writable_index),
            if index == writable_index {
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

#[inline(never)]
fn authenticate_general_position(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    policy: &clutch_dealer_runtime_contract::DealerPolicyV1,
) -> Outcome<(
    PositionAccountV3,
    clutch_retirement::GeneralPositionProjectionV3,
)> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(account.is_writable, ClutchError::NotWritable)?;
    require(
        !account.executable && !account.is_signer,
        ClutchError::MismatchedState,
    )?;
    require(
        account.data_len() == POSITION_V3_BYTES,
        ClutchError::WrongDataLength,
    )?;
    let position = PositionAccountV3::decode(&account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let pda = position.pda_seeds();
    expect_pda(
        account.key,
        seeds::position_v3_pda(
            program_id,
            &pda.market_instance_id().bytes(),
            &pda.owner().bytes(),
            pda.purpose(),
            &pda.purpose_binding_id().bytes(),
        ),
        Some(pda.stored_bump()),
    )?;
    require(
        position.purpose() == PositionPurposeV3::General
            && position.lifecycle() == PositionLifecycleV3::Open
            && position.market_instance_id().bytes() == policy.market_instance_v2_id.bytes()
            && position.realm_id().bytes() == policy.realm_id.bytes()
            && position.outcome_count() == policy.outcome_count,
        ClutchError::MismatchedState,
    )?;
    let projection = project_general_position_v3(
        position,
        AdapterPositionMarketBindingV3 {
            market_instance_id: position.market_instance_id(),
            outcome_count: position.outcome_count(),
            realm_id: position.realm_id(),
            collateral_policy_id: position.collateral_policy_id(),
            collateral_release_id: position.collateral_release_id(),
        },
        AdapterPositionPurposeBindingV3 {
            owner: position.owner(),
            controller: position.controller(),
            purpose_binding_id: position.purpose_binding_id(),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok((position, projection))
}

fn authenticate_general_position_replay_for_dealer(
    program_id: &Pubkey,
    root: &SettlementRootV1AccountV1,
    market: DealerPositionMarketJoinV1,
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

fn authenticate_controlled_general_position(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    actor: Id,
    policy: &clutch_dealer_runtime_contract::DealerPolicyV1,
) -> Outcome<(
    PositionAccountV3,
    clutch_retirement::GeneralPositionProjectionV3,
)> {
    let (position, projection) = authenticate_general_position(program_id, account, policy)?;
    require(
        position.controller().bytes() == actor.bytes(),
        ClutchError::MismatchedState,
    )?;
    Ok((position, projection))
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
    credit_lamports(epoch_payer, credits.epoch_refund_lamports)?;
    credit_lamports(
        bind_receipt_payer,
        credits.bind_receipt_refund_lamports,
    )?;
    let sink_credit = credits
        .epoch_sink_lamports
        .checked_add(credits.bind_receipt_sink_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    credit_lamports(neutral_sink, sink_credit)
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
    let (sponsor_position, sponsor_projection) = authenticate_controlled_general_position(
        program_id,
        &accounts[3],
        id(accounts[0].key),
        &policy,
    )?;
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
        collateral_policy_id: Id::from_bytes(sponsor_position.collateral_policy_id().bytes()),
        collateral_release_id: Id::from_bytes(sponsor_position.collateral_release_id().bytes()),
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
    for account in [
        &accounts[4],
        &accounts[5],
        &accounts[6],
        &accounts[7],
        &accounts[17],
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
    let market = DealerPositionMarketJoinV1 {
        market_instance_v2_id: policy.market_instance_v2_id,
        realm_id: policy.realm_id,
        collateral_policy_id: binding.collateral_policy_id,
        collateral_release_id: binding.collateral_release_id,
        outcome_count: policy.outcome_count,
    };
    let transfer = prepare_dealer_sponsor_funding_transfer_v1(
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
                .destination_post()
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
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
        &accounts[4],
        DEALER_STATE_V2_ACCOUNT_TAG,
        DEALER_STATE_V2_ACCOUNT_VERSION,
        state_bump,
        &prepared.state,
    )
}

#[inline(never)]
fn bind_epoch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    require_count(accounts, BIND_EPOCH_ACCOUNT_COUNT)?;
    let payload = DealerRuntimePayloadV1::decode(DealerFacilityAction::BindEpoch, payload_bytes)
        .map_err(dealer_fault)?;
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

    let generation_bytes = state.generation.to_le_bytes();
    create_full_principal_pda(
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
    create_full_principal_pda(
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
fn lapse_epoch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    require_count(accounts, LAPSE_EPOCH_ACCOUNT_COUNT)?;
    let payload = DealerRuntimePayloadV1::decode(DealerFacilityAction::LapseEpoch, payload_bytes)
        .map_err(dealer_fault)?;
    require(
        sequence == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    require_lapse_aliases(accounts)?;

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let state = authenticate_state(program_id, &accounts[2])?;
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
    require(
        prepared.close_credits.epoch_neutral_sink == policy.neutral_sink
            && prepared.close_credits.bind_receipt_neutral_sink == policy.neutral_sink,
        ClutchError::MismatchedState,
    )?;

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
        .map_err(dealer_fault)?;
    apply_epoch_close(
        &accounts[16],
        &accounts[17],
        &accounts[19],
        &accounts[20],
        &accounts[21],
        prepared.close_credits,
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
    let state = authenticate_state(program_id, &accounts[2])?;
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
    let total_child_principal = select_begin_rent_principal(
        receipt_principal,
        selection_principal,
        lease_principal,
        pot_principal,
        payload.keeper_payment_lamports,
    )?;

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

    let market = DealerPositionMarketJoinV1 {
        market_instance_v2_id: policy.market_instance_v2_id,
        realm_id: policy.realm_id,
        collateral_policy_id: position_binding.collateral_policy_id,
        collateral_release_id: position_binding.collateral_release_id,
        outcome_count: policy.outcome_count,
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
            let state_bump = accounts[2].data.borrow()[2];
            write_dealer_body(
                &accounts[2],
                DEALER_STATE_V2_ACCOUNT_TAG,
                DEALER_STATE_V2_ACCOUNT_VERSION,
                state_bump,
                &prepared.dealer.state_after,
            )?;
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
    let (lp_position, lp_projection) = authenticate_controlled_general_position(
        program_id,
        &accounts[5],
        id(accounts[0].key),
        &policy,
    )?;
    let page = authenticate_lp_page(program_id, &accounts[6])?;
    require(
        page.page_ordinal == payload.page_ordinal,
        ClutchError::MismatchedState,
    )?;
    let market = DealerPositionMarketJoinV1 {
        market_instance_v2_id: policy.market_instance_v2_id,
        realm_id: policy.realm_id,
        collateral_policy_id: binding.collateral_policy_id,
        collateral_release_id: binding.collateral_release_id,
        outcome_count: policy.outcome_count,
    };
    let lp_owner = Id::from_bytes(lp_position.owner().bytes());
    let transfer = prepare_dealer_lp_share_transfer_v1(
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
                prepared.transfer.destination_post(),
                prepared.transfer.source_post(),
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
                prepared.transfer.source_post(),
                prepared.transfer.destination_post(),
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
    let (refund_position, refund_projection) =
        authenticate_general_position(program_id, &accounts[4], &policy)?;
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
    let market = DealerPositionMarketJoinV1 {
        market_instance_v2_id: policy.market_instance_v2_id,
        realm_id: policy.realm_id,
        collateral_policy_id: binding.collateral_policy_id,
        collateral_release_id: binding.collateral_release_id,
        outcome_count: policy.outcome_count,
    };
    let transfer = prepare_dealer_sponsor_refund_transfer_v1(
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
                .destination_post()
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
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
    let state = authenticate_state_with_access(program_id, &accounts[2], false)?;
    require(
        state.policy_id.bytes() == policy_id && state.generation == payload.expected_generation,
        ClutchError::MismatchedState,
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
    let market = DealerPositionMarketJoinV1 {
        market_instance_v2_id: policy.market_instance_v2_id,
        realm_id: policy.realm_id,
        collateral_policy_id: position_binding.collateral_policy_id,
        collateral_release_id: position_binding.collateral_release_id,
        outcome_count: policy.outcome_count,
    };
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
                clutch_general_v2_contract::Id32::new(
                    authorization.receipt_semantic_id.bytes(),
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
            let row_transition = CoveredDealerRowAssetTransitionV1::new(
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
    let state = authenticate_state(program_id, &accounts[2])?;
    require(
        state.policy_id.bytes() == policy_id && state.generation == payload.expected_generation,
        ClutchError::MismatchedState,
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
            &selection.epoch_id.bytes(),
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
    let market = DealerPositionMarketJoinV1 {
        market_instance_v2_id: policy.market_instance_v2_id,
        realm_id: policy.realm_id,
        collateral_policy_id: position_binding.collateral_policy_id,
        collateral_release_id: position_binding.collateral_release_id,
        outcome_count: policy.outcome_count,
    };
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

    create_full_principal_pda(
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
    let state_bump = accounts[2].data.borrow()[2];
    write_dealer_body(
        &accounts[2],
        DEALER_STATE_V2_ACCOUNT_TAG,
        DEALER_STATE_V2_ACCOUNT_VERSION,
        state_bump,
        &prepared.state_after(),
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
    release_dealer_account(&accounts[19])?;
    release_dealer_account(&accounts[20])?;
    let refunds = close.refund_lamports();
    credit_lamports(&accounts[23], refunds[0])?;
    credit_lamports(&accounts[24], refunds[1])?;
    credit_lamports(&accounts[25], close.neutral_sink_lamports())
}

/// Execute one facility action admitted by the non-production profile.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: DealerFacilityAction,
    payload: &[u8],
) -> Outcome<()> {
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
        _ => super::dealer_runtime::process_reserved_disabled(action),
    }
}

#[cfg(test)]
mod select_begin_adversarial_tests {
    use super::select_begin_rent_principal;
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
    fn complete_begin_handler_remains_outside_every_current_capability_profile() {
        assert!(!crate::capabilities::extension_intent_action_enabled(
            DEALER_FAMILY_TAG,
            DEALER_FAMILY_VERSION,
            DealerFacilityAction::SelectLeaseAndBegin.tag(),
        ));
    }
}

#[cfg(test)]
mod collect_deliver_adversarial_tests {
    use super::DealerRuntimePayloadV1;
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
        assert_eq!(finalize.len(), 29);
        assert_eq!(abort.len(), 29);
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
