//! Live preparation and atomic creation for General V2 settlement producers.
//!
//! Action 39 authenticates the winning Window/AdmissionNode/retained-Feed
//! chain, the complete canonical V5 page set, the immutable Market/Realm/
//! collateral joins, both physical policy artifacts, and the counted treasury
//! service authority. It then creates the current selected fee globals, the
//! indexed counted Settlement Root, its compact exact-index pair, and any
//! direction-dependent singleton pots while atomically finalizing Epoch and
//! Window. No child expectation or fee total is read from payload.

use core::cell::Ref;
use std::boxed::Box;

use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    decode_settlement_root_payload_v1, AdmissionNodeV4AccountV1,
    CandidateWindowV5AccountV1, DeletableRentOwnerV1, GeneralEpochV6AccountV1, Id32,
    InitializeSettlementRootV1, OptionalSettlementRentV1, SelectedFeeRecordV2AccountV1,
    SettlementCashPotV1AccountV1, SettlementRootPayloadV1, Sha256BackendV1,
    TreasuryLedgerV2AccountV1,
};
use clutch_general_v2_runtime::{
    adjacency_data_len_v1, locator_data_len_v1, stream_counted_exact_index_root_v1,
    ConstructExactIndexStreamingInputV1, ExactIndexCreateAccountInputV1, ExactIndexPlaneErrorV1,
    SettlementRootExpectationProjectionV1,
};
use clutch_solana_layout::registry::{
    GeneralV2Action, REVENUE_POLICY_RECORD_V2_ACCOUNT_BYTES,
    TREASURY_SERVICE_LEDGER_V1_ACCOUNT_BYTES,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV3, SeriesMarketLinkAccountV3,
};
use clutch_solana_layout::MAX_ORDER_PAGES;
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, require_signer, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::read_clock_slot;
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_creatable, require_system_program,
    transfer_data, RentParameters, MAX_PERMITTED_DATA_INCREASE, SYSTEM_PROGRAM_ID,
};
use crate::seeds;

use super::general_v2_fee_creation_v6::{
    accept_candidate_fee_service_admission_v6, complete_candidate_fee_creation_v6,
    derive_root_expectation_v6, encode_candidate_recipient_v6,
    prepare_candidate_fee_creation_v6, treasury_ledger_v6, CandidateFeeAuthorityFrameV6,
};
use super::general_v2_settlement_traversal_v5::{
    authenticate_settlement_traversal_from_current_v5, AuthenticatedSettlementTraversalV5,
    SettlementTraversalAccountFrameV5,
};
use super::general_market_current_v5::{
    authenticate_general_market_current_v5, AuthenticatedGeneralMarketCurrentV5,
    GeneralMarketCurrentAccountFrameV5,
};

/// Fixed action-39 roles before the mandatory current fee suffix.
pub const ACTION39_COMMON_PREFIX_ACCOUNTS: usize = 15;
/// Current fee suffix: recipient, batch, service ledger, Revenue Record,
/// treasury ledger, and retirement accumulator.
pub const ACTION39_FEE_SUFFIX_ACCOUNTS: usize = 6;
/// Product RootV3/LinkV3/FundingV5 and the remaining current V7 authority
/// accounts not already present in the common/fee prefixes.
pub const ACTION39_CURRENT_SUFFIX_ACCOUNTS: usize = 19;
/// Fixed creation/sysvar roles after the mandatory fee and current-authority
/// suffixes.
pub const ACTION39_CREATION_SUFFIX_ACCOUNTS: usize = 9;

const IX_EPOCH: usize = 0;
const IX_WINDOW: usize = 1;
const IX_NODE: usize = 2;
const IX_FEED: usize = 3;
const IX_MARKET_BINDING: usize = 4;
const IX_MARKET_RUNTIME: usize = 5;
const IX_ECONOMIC_DOMAIN: usize = 6;
const IX_PRICE_GRID: usize = 7;
const IX_REALM: usize = 8;
const IX_PROFILE: usize = 9;
const IX_COLLATERAL_POLICY: usize = 10;
const IX_TOKEN_PROGRAM: usize = 11;
const IX_MARKET_INSTANCE: usize = 12;
const IX_MARKET_GENESIS: usize = 13;
const IX_SELECTED_FEE_RECORD: usize = 14;
const IX_RECIPIENT_ALLOCATION: usize = 15;
const IX_BATCH_POLICY: usize = 16;
const IX_TREASURY_SERVICE_LEDGER: usize = 17;
const IX_REVENUE_POLICY_RECORD: usize = 18;
const IX_TREASURY_LEDGER: usize = 19;
const IX_FEE_ACCUMULATOR: usize = 20;
const IX_CURRENT_PRODUCT_ROOT: usize = 21;
const IX_CURRENT_SERIES_LINK: usize = 22;
const IX_CURRENT_SERIES_FUNDING: usize = 23;
const IX_CURRENT_SERIES_REGISTRY: usize = 24;
const IX_CURRENT_REGISTRY_PROGRAM: usize = 25;
const IX_CURRENT_REGISTRY_PROGRAMDATA: usize = 26;
const IX_CURRENT_REGISTRY_RELEASE: usize = 27;
const IX_CURRENT_CAPABILITY_PROFILE: usize = 28;
const IX_CURRENT_SOURCE_RELEASE: usize = 29;
const IX_CURRENT_COMPILER_BUNDLE: usize = 30;
const IX_CURRENT_REVENUE_PREIMAGE: usize = 31;
const IX_CURRENT_SERIES_PLAN: usize = 32;
const IX_CURRENT_FUNDING_TERMS: usize = 33;
const IX_CURRENT_TEMPLATE: usize = 34;
const IX_CURRENT_NATIVE_BASIS: usize = 35;
const IX_CURRENT_RECOVERY: usize = 36;
const IX_CURRENT_PRICE_POLICY: usize = 37;
const IX_CURRENT_QUOTE: usize = 38;
const IX_CURRENT_ATTACHMENT: usize = 39;

fn id(key: &Pubkey) -> Id32 {
    Id32::from_bytes(key.to_bytes())
}

#[derive(Clone, Copy, Debug)]
struct RuntimeSha256;

impl Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; contract::ID_BYTES] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

fn exact_index_outcome<T>(value: Result<T, ExactIndexPlaneErrorV1>) -> Outcome<T> {
    value.map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

fn action39_total_principal(
    selected_fee_record: u64,
    recipient_allocation: u64,
    treasury_ledger: u64,
    fee_accumulator: u64,
    indexed_root: u64,
    locator: u64,
    adjacency: u64,
    cash_pot: Option<u64>,
    final_pot: Option<u64>,
) -> Option<u64> {
    let mut total = selected_fee_record
        .checked_add(recipient_allocation)?
        .checked_add(treasury_ledger)?
        .checked_add(fee_accumulator)?
        .checked_add(indexed_root)?
        .checked_add(locator)?
        .checked_add(adjacency)?;
    if let Some(principal) = cash_pot {
        total = total.checked_add(principal)?;
    }
    if let Some(principal) = final_pot {
        total = total.checked_add(principal)?;
    }
    Some(total)
}

fn borrow_data<'a, 'info>(account: &'a AccountInfo<'info>) -> Outcome<Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(Ref::map(data, |bytes| &**bytes))
}

#[inline(never)]
fn decode_epoch_boxed(account: &AccountInfo<'_>) -> Outcome<Box<GeneralEpochV6AccountV1>> {
    Ok(Box::new(GeneralEpochV6AccountV1::decode(&borrow_data(account)?)?))
}

#[inline(never)]
fn decode_window_boxed(account: &AccountInfo<'_>) -> Outcome<Box<CandidateWindowV5AccountV1>> {
    Ok(Box::new(CandidateWindowV5AccountV1::decode(&borrow_data(account)?)?))
}

#[inline(never)]
fn decode_node_boxed(account: &AccountInfo<'_>) -> Outcome<Box<AdmissionNodeV4AccountV1>> {
    Ok(Box::new(AdmissionNodeV4AccountV1::decode(&borrow_data(account)?)?))
}

#[inline(never)]
fn authenticate_traversal_boxed<'a, 'info>(
    program_id: &Pubkey,
    frame: SettlementTraversalAccountFrameV5<'a, 'info>,
    current: &AuthenticatedGeneralMarketCurrentV5,
) -> Outcome<Box<AuthenticatedSettlementTraversalV5<'info>>> {
    Ok(Box::new(authenticate_settlement_traversal_from_current_v5(
        program_id, frame, current,
    )?))
}

#[inline(never)]
fn authenticate_action39_current_market_v5(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Outcome<AuthenticatedGeneralMarketCurrentV5> {
    let artifacts = vec![
        accounts[IX_CURRENT_SERIES_PLAN].clone(),
        accounts[IX_CURRENT_FUNDING_TERMS].clone(),
        accounts[IX_CURRENT_TEMPLATE].clone(),
        accounts[IX_CURRENT_NATIVE_BASIS].clone(),
        accounts[IX_CURRENT_RECOVERY].clone(),
        accounts[IX_CURRENT_PRICE_POLICY].clone(),
        accounts[IX_MARKET_GENESIS].clone(),
        accounts[IX_CURRENT_QUOTE].clone(),
        accounts[IX_CURRENT_ATTACHMENT].clone(),
    ];
    let frame = GeneralMarketCurrentAccountFrameV5 {
        market_binding: &accounts[IX_MARKET_BINDING],
        market_runtime: &accounts[IX_MARKET_RUNTIME],
        product_root: &accounts[IX_CURRENT_PRODUCT_ROOT],
        series_link: &accounts[IX_CURRENT_SERIES_LINK],
        series_funding: &accounts[IX_CURRENT_SERIES_FUNDING],
        series_registry: &accounts[IX_CURRENT_SERIES_REGISTRY],
        registry_program: &accounts[IX_CURRENT_REGISTRY_PROGRAM],
        registry_programdata: &accounts[IX_CURRENT_REGISTRY_PROGRAMDATA],
        registry_release_artifact: &accounts[IX_CURRENT_REGISTRY_RELEASE],
        capability_profile_artifact: &accounts[IX_CURRENT_CAPABILITY_PROFILE],
        source_release: &accounts[IX_CURRENT_SOURCE_RELEASE],
        compiler_bundle: &accounts[IX_CURRENT_COMPILER_BUNDLE],
        market_instance: &accounts[IX_MARKET_INSTANCE],
        realm: &accounts[IX_REALM],
        revenue_record: &accounts[IX_REVENUE_POLICY_RECORD],
        revenue_policy_preimage: &accounts[IX_CURRENT_REVENUE_PREIMAGE],
        artifacts: &artifacts,
    };
    let mut root = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let mut link = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    authenticate_general_market_current_v5(program_id, &frame, &mut root, &mut link)
}

#[inline(never)]
fn prepare_root_plan_boxed<'a>(
    request: InitializeSettlementRootV1<'a>,
) -> Outcome<Box<contract::StreamingSettlementRootInitializationV1<'a>>> {
    Ok(Box::new(contract::prepare_streaming_settlement_root_v1(request)?))
}

fn require_program_state(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
    exact_len: Option<usize>,
) -> Outcome<()> {
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
    if let Some(len) = exact_len {
        require(account.data_len() == len, ClutchError::WrongDataLength)?;
    }
    Ok(())
}

fn require_all_distinct(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            require(
                accounts[left].key != accounts[right].key,
                ClutchError::AccountAlias,
            )?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

pub(crate) fn encode_account(
    account: &AccountInfo<'_>,
    encode: impl FnOnce(&mut [u8]) -> Result<(), contract::CodecError>,
) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    encode(&mut data)?;
    Ok(())
}

pub(crate) fn rent_owner(
    payer: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    rent: &RentParameters,
    space: usize,
) -> Outcome<DeletableRentOwnerV1> {
    Ok(DeletableRentOwnerV1 {
        payer: id(payer.key),
        refundable_principal: rent.minimum_balance(space)?,
        donation_floor: target.lamports(),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_from_payer<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    space: usize,
    owner: DeletableRentOwnerV1,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_creatable(target)?;
    require(
        target.is_writable && !target.is_signer && space <= MAX_PERMITTED_DATA_INCREASE,
        ClutchError::AccountCreationFailed,
    )?;
    let principal = rent.minimum_balance(space)?;
    require(
        owner.payer == id(payer.key)
            && owner.refundable_principal == principal
            && owner.donation_floor == target.lamports(),
        ClutchError::MismatchedState,
    )?;
    let expected = target
        .lamports()
        .checked_add(principal)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(principal),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*target.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[payer.clone(), target.clone(), system_program.clone()],
        &[],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(target.lamports() == expected, ClutchError::AccountCreationFailed)?;

    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(space),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.data_len() == space
            && *target.owner == SYSTEM_PROGRAM_ID
            && target.lamports() == expected,
        ClutchError::AccountCreationFailed,
    )?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.owner == program_id && target.lamports() == expected,
        ClutchError::AccountCreationFailed,
    )
}

/// Enter the exact action-39 producer route.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        action == GeneralV2Action::InitializeSettlementRoot
            && capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    let SettlementRootPayloadV1::InitializeSettlementRoot(request) =
        decode_settlement_root_payload_v1(action.tag(), payload)?
    else {
        return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction));
    };
    initialize_settlement_root(program_id, accounts, request)
}

#[inline(never)]
fn initialize_settlement_root(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: contract::InitializeSettlementRootPayloadV2,
) -> Outcome<()> {
    require(
        accounts.len()
            >= ACTION39_COMMON_PREFIX_ACCOUNTS
                + ACTION39_FEE_SUFFIX_ACCOUNTS
                + ACTION39_CURRENT_SUFFIX_ACCOUNTS
                + ACTION39_CREATION_SUFFIX_ACCOUNTS
                + 1,
        ClutchError::WrongAccountCount,
    )?;
    require_all_distinct(accounts)?;
    require(request.epoch == id(accounts[IX_EPOCH].key), ClutchError::MismatchedState)?;

    require_program_state(
        program_id,
        &accounts[IX_EPOCH],
        true,
        Some(contract::GENERAL_EPOCH_ACCOUNT_BYTES),
    )?;
    require_program_state(
        program_id,
        &accounts[IX_WINDOW],
        true,
        Some(contract::WINDOW_ACCOUNT_BYTES),
    )?;
    require_program_state(
        program_id,
        &accounts[IX_NODE],
        false,
        Some(contract::ADMISSION_NODE_ACCOUNT_BYTES_V2),
    )?;

    let epoch = decode_epoch_boxed(&accounts[IX_EPOCH])?;
    let window = decode_window_boxed(&accounts[IX_WINDOW])?;
    let node = decode_node_boxed(&accounts[IX_NODE])?;

    let selected_fee_pda = seeds::general_v2_selected_fee_record_pda(
        program_id,
        &node.base().settlement_candidate_id.bytes(),
    );
    require(
        *accounts[IX_SELECTED_FEE_RECORD].key == selected_fee_pda.0
            && !accounts[IX_SELECTED_FEE_RECORD].is_signer
            && !accounts[IX_SELECTED_FEE_RECORD].executable
            && accounts[IX_SELECTED_FEE_RECORD].is_writable,
        ClutchError::MismatchedState,
    )?;
    require_creatable(&accounts[IX_SELECTED_FEE_RECORD])?;
    let recipient_pda = seeds::general_v2_recipient_allocation_pda(
        program_id,
        &accounts[IX_SELECTED_FEE_RECORD].key.to_bytes(),
    );
    let treasury_pda = seeds::general_v2_treasury_ledger_pda(
        program_id,
        &accounts[IX_SELECTED_FEE_RECORD].key.to_bytes(),
    );
    let accumulator_pda = seeds::general_v2_fee_retirement_accumulator_pda(
        program_id,
        &accounts[IX_SELECTED_FEE_RECORD].key.to_bytes(),
    );
    require(
        *accounts[IX_RECIPIENT_ALLOCATION].key == recipient_pda.0
            && *accounts[IX_TREASURY_LEDGER].key == treasury_pda.0
            && *accounts[IX_FEE_ACCUMULATOR].key == accumulator_pda.0,
        ClutchError::WrongPda,
    )?;
    for target in [
        &accounts[IX_RECIPIENT_ALLOCATION],
        &accounts[IX_TREASURY_LEDGER],
        &accounts[IX_FEE_ACCUMULATOR],
    ] {
        require_creatable(target)?;
        require(!target.is_signer, ClutchError::MismatchedState)?;
    }
    require_program_state(
        program_id,
        &accounts[IX_REVENUE_POLICY_RECORD],
        false,
        Some(REVENUE_POLICY_RECORD_V2_ACCOUNT_BYTES),
    )?;
    require_program_state(
        program_id,
        &accounts[IX_TREASURY_SERVICE_LEDGER],
        true,
        Some(TREASURY_SERVICE_LEDGER_V1_ACCOUNT_BYTES),
    )?;

    let creation_at = ACTION39_COMMON_PREFIX_ACCOUNTS
        + ACTION39_FEE_SUFFIX_ACCOUNTS
        + ACTION39_CURRENT_SUFFIX_ACCOUNTS;
    let ix_root = creation_at;
    let ix_cash_pot = creation_at + 1;
    let ix_final_pot = creation_at + 2;
    let ix_locator = creation_at + 3;
    let ix_adjacency = creation_at + 4;
    let ix_payer = creation_at + 5;
    let ix_system = creation_at + 6;
    let ix_rent = creation_at + 7;
    let ix_clock = creation_at + 8;
    let first_page = creation_at + ACTION39_CREATION_SUFFIX_ACCOUNTS;
    require(
        accounts.len() > first_page && accounts.len() <= first_page + MAX_ORDER_PAGES,
        ClutchError::WrongAccountCount,
    )?;
    require_signer(&accounts[ix_payer])?;
    require(accounts[ix_payer].is_writable, ClutchError::NotWritable)?;
    let payer_lamports_before = accounts[ix_payer].lamports();
    require_system_program(&accounts[ix_system])?;
    let rent = read_rent(&accounts[ix_rent])?;
    let current_slot = read_clock_slot(&accounts[ix_clock])?;
    let current = authenticate_action39_current_market_v5(program_id, accounts)?;
    let authenticated = authenticate_traversal_boxed(
        program_id,
        SettlementTraversalAccountFrameV5 {
            retained_feed: &accounts[IX_FEED],
            market_binding: &accounts[IX_MARKET_BINDING],
            market_runtime: &accounts[IX_MARKET_RUNTIME],
            economic_domain: &accounts[IX_ECONOMIC_DOMAIN],
            price_grid: &accounts[IX_PRICE_GRID],
            realm: &accounts[IX_REALM],
            profile: &accounts[IX_PROFILE],
            collateral_policy: &accounts[IX_COLLATERAL_POLICY],
            token_program: &accounts[IX_TOKEN_PROGRAM],
            market_instance: &accounts[IX_MARKET_INSTANCE],
            market_genesis: &accounts[IX_MARKET_GENESIS],
            pages: &accounts[first_page..],
        },
        &current,
    )?;
    let feed = authenticated.feed();
    let market = authenticated.market();
    let traversal = authenticated.traversal();
    let base = market.base().base();

    let epoch_pda = seeds::general_v2_epoch_pda(
        program_id,
        &accounts[IX_MARKET_BINDING].key.to_bytes(),
        epoch.epoch_index,
    );
    expect_pda(accounts[IX_EPOCH].key, epoch_pda, Some(epoch.stored_bump))?;
    expect_pda(
        accounts[IX_WINDOW].key,
        seeds::general_v2_window_pda(program_id, &request.epoch.bytes()),
        Some(window.base().stored_bump),
    )?;
    expect_pda(
        accounts[IX_NODE].key,
        seeds::general_v2_node_pda(program_id, &request.epoch.bytes(), node.base().ordinal),
        Some(node.base().stored_bump),
    )?;
    require(
        request.selected_node == id(accounts[IX_NODE].key)
            && epoch.window == id(accounts[IX_WINDOW].key)
            && epoch.economic_domain == id(accounts[IX_ECONOMIC_DOMAIN].key)
            && epoch.market_binding == id(accounts[IX_MARKET_BINDING].key)
            && epoch.market_runtime == id(accounts[IX_MARKET_RUNTIME].key)
            && feed.node == id(accounts[IX_NODE].key)
            && feed.order_set == epoch.order_set,
        ClutchError::MismatchedState,
    )?;

    let epoch_semantic_id = epoch.semantics_digest(&RuntimeSha256)?;
    require(
        request.revenue_policy == current.revenue().policy(),
        ClutchError::MismatchedState,
    )?;
    let prepared_fee = prepare_candidate_fee_creation_v6(
        program_id,
        &authenticated,
        &current,
        CandidateFeeAuthorityFrameV6 {
            realm: &accounts[IX_REALM],
            batch_policy: &accounts[IX_BATCH_POLICY],
            revenue_policy_record: &accounts[IX_REVENUE_POLICY_RECORD],
            treasury_service_ledger: &accounts[IX_TREASURY_SERVICE_LEDGER],
        },
        current.revenue().policy(),
        request.epoch,
        epoch_semantic_id,
        node.base().settlement_candidate_id,
        id(accounts[IX_SELECTED_FEE_RECORD].key),
    )?;

    let root_pda = seeds::general_v2_settlement_root_pda(
        program_id,
        &request.epoch.bytes(),
        &node.base().settlement_candidate_id.bytes(),
    );
    let locator_pda =
        seeds::general_v2_frozen_order_locator_pda(program_id, &root_pda.0.to_bytes());
    let adjacency_pda =
        seeds::general_v2_candidate_slice_index_pda(program_id, &root_pda.0.to_bytes());
    let cash_pda = seeds::general_v2_settlement_cash_pot_pda(
        program_id,
        &request.epoch.bytes(),
        &node.base().settlement_candidate_id.bytes(),
    );
    let final_seed = contract::FinalPotSeedTupleV1::new(
        request.epoch,
        node.base().settlement_candidate_id,
    )?;
    let final_pda = seeds::find(
        program_id,
        &[
            final_seed.domain(),
            final_seed.epoch(),
            final_seed.settlement_candidate(),
        ],
    );
    require(
        *accounts[ix_root].key == root_pda.0
            && *accounts[ix_cash_pot].key == cash_pda.0
            && *accounts[ix_final_pot].key == final_pda.0
            && *accounts[ix_locator].key == locator_pda.0
            && *accounts[ix_adjacency].key == adjacency_pda.0,
        ClutchError::WrongPda,
    )?;
    for target in [
        &accounts[ix_root],
        &accounts[ix_cash_pot],
        &accounts[ix_final_pot],
        &accounts[ix_locator],
        &accounts[ix_adjacency],
    ] {
        require_creatable(target)?;
        require(target.is_writable && !target.is_signer, ClutchError::NotWritable)?;
    }
    let reference_count = traversal.projection().exact_slice_reference_count();
    let locator_len = exact_index_outcome(locator_data_len_v1(feed.order_count))?;
    let adjacency_len = exact_index_outcome(adjacency_data_len_v1(
        feed.order_count,
        reference_count,
    ))?;
    let selected_fee_rent = rent_owner(
        &accounts[ix_payer],
        &accounts[IX_SELECTED_FEE_RECORD],
        &rent,
        contract::SELECTED_FEE_RECORD_ACCOUNT_BYTES_V2,
    )?;
    let recipient_rent = rent_owner(
        &accounts[ix_payer],
        &accounts[IX_RECIPIENT_ALLOCATION],
        &rent,
        contract::RECIPIENT_ALLOCATION_ACCOUNT_BYTES_V3,
    )?;
    let treasury_rent = rent_owner(
        &accounts[ix_payer],
        &accounts[IX_TREASURY_LEDGER],
        &rent,
        contract::TREASURY_LEDGER_ACCOUNT_BYTES_V2,
    )?;
    let accumulator_rent = rent_owner(
        &accounts[ix_payer],
        &accounts[IX_FEE_ACCUMULATOR],
        &rent,
        contract::FEE_RETIREMENT_ACCOUNT_BYTES_V1,
    )?;
    let root_rent = rent_owner(
        &accounts[ix_payer],
        &accounts[ix_root],
        &rent,
        contract::INDEXED_SETTLEMENT_ROOT_BYTES_V1,
    )?;
    let locator_rent = rent_owner(
        &accounts[ix_payer],
        &accounts[ix_locator],
        &rent,
        locator_len,
    )?;
    let adjacency_rent = rent_owner(
        &accounts[ix_payer],
        &accounts[ix_adjacency],
        &rent,
        adjacency_len,
    )?;
    let cash_rent = rent_owner(
        &accounts[ix_payer],
        &accounts[ix_cash_pot],
        &rent,
        contract::SETTLEMENT_CASH_POT_ACCOUNT_BYTES,
    )?;
    let has_final_pot = traversal.projection().virtual_cash_direction()
        != clutch_owner_settlement::VirtualCashDirectionV1::None;
    let creates_cash_pot = traversal.projection().virtual_cash_direction()
        != clutch_owner_settlement::VirtualCashDirectionV1::Merge;
    let final_rent = rent_owner(
        &accounts[ix_payer],
        &accounts[ix_final_pot],
        &rent,
        contract::FINAL_POT_ACCOUNT_BYTES,
    )?;
    let locator_create = ExactIndexCreateAccountInputV1 {
        account: id(accounts[ix_locator].key),
        program_id: id(program_id),
        payer: id(accounts[ix_payer].key),
        payer_lamports: payer_lamports_before,
        target_lamports: accounts[ix_locator].lamports(),
        target_owner: Id32::ZERO,
        target_data_len: accounts[ix_locator].data_len(),
        target_writable: accounts[ix_locator].is_writable,
        target_executable: accounts[ix_locator].executable,
        rent_exempt_minimum: locator_rent.refundable_principal,
        stored_bump: locator_pda.1,
    };
    let adjacency_create = ExactIndexCreateAccountInputV1 {
        account: id(accounts[ix_adjacency].key),
        program_id: id(program_id),
        payer: id(accounts[ix_payer].key),
        payer_lamports: payer_lamports_before,
        target_lamports: accounts[ix_adjacency].lamports(),
        target_owner: Id32::ZERO,
        target_data_len: accounts[ix_adjacency].data_len(),
        target_writable: accounts[ix_adjacency].is_writable,
        target_executable: accounts[ix_adjacency].executable,
        rent_exempt_minimum: adjacency_rent.refundable_principal,
        stored_bump: adjacency_pda.1,
    };
    let total_principal = action39_total_principal(
        selected_fee_rent.refundable_principal,
        recipient_rent.refundable_principal,
        treasury_rent.refundable_principal,
        accumulator_rent.refundable_principal,
        root_rent.refundable_principal,
        locator_rent.refundable_principal,
        adjacency_rent.refundable_principal,
        creates_cash_pot.then_some(cash_rent.refundable_principal),
        has_final_pot.then_some(final_rent.refundable_principal),
    )
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        payer_lamports_before >= total_principal,
        ClutchError::AccountCreationFailed,
    )?;

    let root_epoch = request.epoch.bytes();
    let candidate = node.base().settlement_candidate_id.bytes();

    let selected_fee_bump = [selected_fee_pda.1];
    let selected_fee_seeds: [&[u8]; 3] = [
        seeds::SEED_GENERAL_V2_SELECTED_FEE_RECORD,
        &candidate,
        &selected_fee_bump,
    ];
    let selected_fee_account = SelectedFeeRecordV2AccountV1 {
        semantic: prepared_fee.selected,
        rent: selected_fee_rent,
        stored_bump: selected_fee_pda.1,
    };
    create_from_payer(
        program_id,
        &accounts[ix_payer],
        &accounts[IX_SELECTED_FEE_RECORD],
        &accounts[ix_system],
        &rent,
        contract::SELECTED_FEE_RECORD_ACCOUNT_BYTES_V2,
        selected_fee_rent,
        &selected_fee_seeds,
    )?;
    encode_account(&accounts[IX_SELECTED_FEE_RECORD], |out| {
        selected_fee_account.encode(out)
    })?;

    let selected_fee_key = accounts[IX_SELECTED_FEE_RECORD].key.to_bytes();
    let recipient_bump = [recipient_pda.1];
    let recipient_seeds: [&[u8]; 3] = [
        seeds::SEED_GENERAL_V2_RECIPIENT_ALLOCATION,
        &selected_fee_key,
        &recipient_bump,
    ];
    create_from_payer(
        program_id,
        &accounts[ix_payer],
        &accounts[IX_RECIPIENT_ALLOCATION],
        &accounts[ix_system],
        &rent,
        contract::RECIPIENT_ALLOCATION_ACCOUNT_BYTES_V3,
        recipient_rent,
        &recipient_seeds,
    )?;
    {
        let mut recipient_output = accounts[IX_RECIPIENT_ALLOCATION]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        encode_candidate_recipient_v6(
            prepared_fee.as_ref(),
            &mut recipient_output,
            recipient_rent,
            recipient_pda.1,
        )?;
    }
    let expectation: SettlementRootExpectationProjectionV1 = {
        let recipient_data = borrow_data(&accounts[IX_RECIPIENT_ALLOCATION])?;
        let recipient = contract::decode_borrowed_recipient_allocation_v3_account(
            &recipient_data,
        )?;
        derive_root_expectation_v6(
            traversal,
            &prepared_fee.selected,
            recipient.semantic().summary(),
        )?
    };

    let treasury_bump = [treasury_pda.1];
    let treasury_seeds: [&[u8]; 3] = [
        seeds::SEED_GENERAL_V2_TREASURY_LEDGER,
        &selected_fee_key,
        &treasury_bump,
    ];
    let treasury_account = TreasuryLedgerV2AccountV1 {
        semantic: treasury_ledger_v6(&prepared_fee.selected)?,
        rent: treasury_rent,
        stored_bump: treasury_pda.1,
    };
    create_from_payer(
        program_id,
        &accounts[ix_payer],
        &accounts[IX_TREASURY_LEDGER],
        &accounts[ix_system],
        &rent,
        contract::TREASURY_LEDGER_ACCOUNT_BYTES_V2,
        treasury_rent,
        &treasury_seeds,
    )?;
    encode_account(&accounts[IX_TREASURY_LEDGER], |out| treasury_account.encode(out))?;

    let (accumulator_account, service_transition) = {
        let recipient_data = borrow_data(&accounts[IX_RECIPIENT_ALLOCATION])?;
        let recipient = contract::decode_borrowed_recipient_allocation_v3_account(
            &recipient_data,
        )?;
        let recipient_data_id = contract::recipient_allocation_account_data_id_v3(
            &recipient_data,
            &RuntimeSha256,
        )?;
        complete_candidate_fee_creation_v6(
            program_id,
            *prepared_fee,
            id(accounts[ix_root].key),
            traversal.projection().feed_data_id(),
            id(accounts[IX_RECIPIENT_ALLOCATION].key),
            recipient_data_id,
            id(accounts[IX_TREASURY_LEDGER].key),
            id(accounts[ix_cash_pot].key),
            accumulator_rent,
            accumulator_pda.1,
            &recipient.semantic(),
        )?
    };
    let accumulator_bump = [accumulator_pda.1];
    let accumulator_seeds: [&[u8]; 3] = [
        contract::FEE_RETIREMENT_ACCUMULATOR_SEED_DOMAIN_V1,
        &selected_fee_key,
        &accumulator_bump,
    ];
    create_from_payer(
        program_id,
        &accounts[ix_payer],
        &accounts[IX_FEE_ACCUMULATOR],
        &accounts[ix_system],
        &rent,
        contract::FEE_RETIREMENT_ACCOUNT_BYTES_V1,
        accumulator_rent,
        &accumulator_seeds,
    )?;
    encode_account(&accounts[IX_FEE_ACCUMULATOR], |out| {
        accumulator_account.encode(out)
    })?;

    let plan = prepare_root_plan_boxed(InitializeSettlementRootV1 {
        root_account: id(accounts[ix_root].key),
        root_bump: root_pda.1,
        epoch_account: request.epoch,
        market_binding_account: id(accounts[IX_MARKET_BINDING].key),
        window_account: id(accounts[IX_WINDOW].key),
        retained_feed_account: id(accounts[IX_FEED].key),
        epoch_generation: epoch.generation,
        market_instance_v2_id: base.market_instance_v2_id,
        epoch: &epoch,
        market,
        window: &window,
        node: &node,
        feed: &feed,
        current_slot,
        owner_order_set_digest: traversal.projection().owner_order_set_digest(),
        cash_expectation: expectation.cash(),
        expected_reservations: expectation.expected_reservations(),
        expected_filled_reservations: expectation.expected_filled_reservations(),
        expected_merge_payments: expectation.expected_merge_payments(),
        settlement_cash_pot: id(accounts[ix_cash_pot].key),
        cash_pot_bump: cash_pda.1,
        final_pot: if has_final_pot {
            id(accounts[ix_final_pot].key)
        } else {
            Id32::ZERO
        },
        final_pot_bump: if has_final_pot { final_pda.1 } else { 0 },
        root_rent,
        cash_pot_rent: cash_rent,
        final_pot_rent: if has_final_pot {
            OptionalSettlementRentV1::present(final_rent)?
        } else {
            OptionalSettlementRentV1::ABSENT
        },
    })?;
    require(
        plan.creates_cash_pot() == creates_cash_pot
            && plan.creates_final_pot() == has_final_pot,
        ClutchError::MismatchedState,
    )?;
    let root_rent_preparation = contract::prepare_fresh_indexed_settlement_root_rent_v1(
        plan.root(),
        id(accounts[ix_root].key),
        accounts[ix_root].lamports(),
        root_rent.refundable_principal,
        payer_lamports_before,
        traversal.projection().neutral_sink(),
        &RuntimeSha256,
    )?;
    let root_rent_authority =
        root_rent_preparation.authenticate_source(plan.root(), &RuntimeSha256)?;

    let root_bump = [root_pda.1];
    let root_seeds: [&[u8]; 4] = [
        seeds::SEED_GENERAL_V2_SETTLEMENT_ROOT,
        &root_epoch,
        &candidate,
        &root_bump,
    ];
    create_from_payer(
        program_id,
        &accounts[ix_payer],
        &accounts[ix_root],
        &accounts[ix_system],
        &rent,
        contract::INDEXED_SETTLEMENT_ROOT_BYTES_V1,
        root_rent,
        &root_seeds,
    )?;
    let root_key = root_pda.0.to_bytes();
    let locator_bump = [locator_pda.1];
    let locator_seeds: [&[u8]; 3] = [
        seeds::SEED_GENERAL_V2_FROZEN_ORDER_LOCATOR,
        &root_key,
        &locator_bump,
    ];
    create_from_payer(
        program_id,
        &accounts[ix_payer],
        &accounts[ix_locator],
        &accounts[ix_system],
        &rent,
        locator_len,
        locator_rent,
        &locator_seeds,
    )?;
    let adjacency_bump = [adjacency_pda.1];
    let adjacency_seeds: [&[u8]; 3] = [
        seeds::SEED_GENERAL_V2_CANDIDATE_SLICE_INDEX,
        &root_key,
        &adjacency_bump,
    ];
    create_from_payer(
        program_id,
        &accounts[ix_payer],
        &accounts[ix_adjacency],
        &accounts[ix_system],
        &rent,
        adjacency_len,
        adjacency_rent,
        &adjacency_seeds,
    )?;
    if let Some(cash) = plan.cash_pot()? {
        let cash_bump = [cash_pda.1];
        let cash_seeds: [&[u8]; 4] = [
            seeds::SEED_GENERAL_V2_SETTLEMENT_CASH_POT,
            &root_epoch,
            &candidate,
            &cash_bump,
        ];
        create_from_payer(
            program_id,
            &accounts[ix_payer],
            &accounts[ix_cash_pot],
            &accounts[ix_system],
            &rent,
            contract::SETTLEMENT_CASH_POT_ACCOUNT_BYTES,
            cash_rent,
            &cash_seeds,
        )?;
        encode_account(&accounts[ix_cash_pot], |out| {
            SettlementCashPotV1AccountV1 {
                semantic: cash,
                stored_bump: cash_pda.1,
                flags: 0,
            }
            .encode(out)
        })?;
    }
    if let Some(final_pot) = plan.final_pot()? {
        let final_bump = [final_pda.1];
        let final_seeds: [&[u8]; 4] = [
            final_seed.domain(),
            final_seed.epoch(),
            final_seed.settlement_candidate(),
            &final_bump,
        ];
        create_from_payer(
            program_id,
            &accounts[ix_payer],
            &accounts[ix_final_pot],
            &accounts[ix_system],
            &rent,
            contract::FINAL_POT_ACCOUNT_BYTES,
            final_rent,
            &final_seeds,
        )?;
        encode_account(&accounts[ix_final_pot], |out| final_pot.encode(out))?;
    }
    {
        let mut root_output = accounts[ix_root]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut locator_output = accounts[ix_locator]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut adjacency_output = accounts[ix_adjacency]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        exact_index_outcome(stream_counted_exact_index_root_v1(
            root_rent_authority,
            &ConstructExactIndexStreamingInputV1 {
                traversal,
                settlement_root_account: id(accounts[ix_root].key),
                settlement_root: plan.root(),
                capability_profile_id: Id32::from_bytes(capabilities::PROFILE_ID),
                locator_create,
                adjacency_create,
            },
            &mut root_output,
            &mut locator_output,
            &mut adjacency_output,
        ))?;
    }
    encode_account(&accounts[IX_EPOCH], |out| plan.encode_epoch_successor(out))?;
    encode_account(&accounts[IX_WINDOW], |out| plan.encode_window_successor(out))?;
    accept_candidate_fee_service_admission_v6(
        &accounts[IX_TREASURY_SERVICE_LEDGER],
        service_transition,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_index_roles_fit_action39_and_all_principals_are_aggregated() {
        assert_eq!(ACTION39_CREATION_SUFFIX_ACCOUNTS, 9);
        assert_eq!(
            ACTION39_COMMON_PREFIX_ACCOUNTS
                + ACTION39_FEE_SUFFIX_ACCOUNTS
                + ACTION39_CURRENT_SUFFIX_ACCOUNTS
                + ACTION39_CREATION_SUFFIX_ACCOUNTS
                + MAX_ORDER_PAGES,
            53,
        );
        assert!(
            ACTION39_COMMON_PREFIX_ACCOUNTS
                + ACTION39_FEE_SUFFIX_ACCOUNTS
                + ACTION39_CURRENT_SUFFIX_ACCOUNTS
                + ACTION39_CREATION_SUFFIX_ACCOUNTS
                + MAX_ORDER_PAGES
                < 64
        );
        let source = include_str!("general_v2_settlement_producer_v5.rs");
        assert!(source.contains("authenticate_action39_current_market_v5"));
        assert!(source.contains("authenticate_settlement_traversal_from_current_v5"));
        assert!(!source.contains("authenticate_settlement_traversal_v5(program_id, frame"));
        assert_eq!(
            action39_total_principal(2, 3, 5, 7, 11, 13, 17, None, None),
            Some(58),
        );
        assert_eq!(
            action39_total_principal(2, 3, 5, 7, 11, 13, 17, Some(19), Some(23)),
            Some(100),
        );
        assert_eq!(
            action39_total_principal(u64::MAX, 1, 1, 1, 1, 1, 1, None, None),
            None,
        );
        assert!(contract::INDEXED_SETTLEMENT_ROOT_BYTES_V1 <= MAX_PERMITTED_DATA_INCREASE);
        assert!(clutch_general_v2_runtime::FROZEN_ORDER_LOCATOR_MAX_BYTES_V1
            <= MAX_PERMITTED_DATA_INCREASE);
        assert!(clutch_general_v2_runtime::CANDIDATE_ORDER_SLICE_INDEX_MAX_BYTES_V1
            <= MAX_PERMITTED_DATA_INCREASE);
    }
}
