//! Live preparation and atomic creation for General V2 settlement producers.
//!
//! Action 39 authenticates the winning Window/AdmissionNode/retained-Feed
//! chain, the complete canonical V5 page set, the immutable Market/Realm/
//! collateral joins, and either the complete-book fee certificate or the
//! canonical absent selected-fee PDA. It then creates the counted Settlement
//! Root and its direction-dependent singleton children while atomically
//! finalizing Epoch and Window. No child expectation is read from payload.

use core::cell::Ref;

use clutch_batch_policy_identity::revenue_policy_v1::{
    decode_revenue_policy, RevenuePolicyV1, REVENUE_POLICY_BYTES,
};
use clutch_collateral_adapter_v2::{
    refine_market_collateral_v2, Id as CollateralId, MarketCollateralBindingV2,
};
use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    decode_settlement_root_payload_v1, AdmissionNodeV4AccountV1,
    CandidateWindowV5AccountV1, DeletableRentOwnerV1, GeneralEpochV6AccountV1, Id32,
    InitializeSettlementRootV1, MarketBindingV2, OptionalSettlementRentV1,
    SettlementCashPotV1AccountV1, SettlementRootPayloadV1,
};
use clutch_general_v2_runtime::{
    derive_settlement_root_expectation_v1, derive_settlement_traversal_projection_v4,
    project_owner_blind_book_costed_v1, CandidateFeeAggregateProjectionV1,
    GeneralOrderPageInputV5, SettlementRootExpectationProjectionV1,
};
use clutch_product_series::{ContentId, MarketGenesisProfileV2, MarketInstancePreimageV2};
use clutch_solana_layout::order_page_v5::{verify_page_v5, ORDER_PAGE_V5_BYTES};
use clutch_solana_layout::registry::GeneralV2Action;
use clutch_solana_layout::{account_len, PriceGridAccount, MAX_ORDER_PAGES};
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

use super::collateral_position_v3::authenticate_general_market_v2;
use super::general_v2_fee_v5::{
    compose_candidate_fee_collection_action39_v5, CandidateFeeCollectionAccountFrameV5,
    CandidateFeeCollectionExpectationV5,
};
use super::product_artifact::authenticate_product_artifact_v1;

/// Fixed action-39 roles before the optional fee suffix.
pub const ACTION39_COMMON_PREFIX_ACCOUNTS: usize = 15;
/// Four accounts present only for a nonzero selected fee record.
pub const ACTION39_FEE_SUFFIX_ACCOUNTS: usize = 4;
/// Fixed creation/sysvar roles after the optional fee suffix.
pub const ACTION39_CREATION_SUFFIX_ACCOUNTS: usize = 7;

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

fn id(key: &Pubkey) -> Id32 {
    Id32::from_bytes(key.to_bytes())
}

fn borrow_data<'a, 'b>(account: &'a AccountInfo<'b>) -> Outcome<Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(Ref::map(data, |bytes| &**bytes))
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

fn encode_account(
    account: &AccountInfo<'_>,
    encode: impl FnOnce(&mut [u8]) -> Result<(), contract::CodecError>,
) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    encode(&mut data)?;
    Ok(())
}

fn rent_owner(
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
fn create_from_payer<'a>(
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

fn authenticate_market_context(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Outcome<(
    MarketBindingV2,
    clutch_collateral_adapter_v2::BoundCollateralProfileV2,
    MarketGenesisProfileV2,
)> {
    let realm = crate::collateral_release::authenticate_realm_collateral_v2(
        program_id,
        &accounts[IX_REALM],
        &accounts[IX_PROFILE],
        &accounts[IX_COLLATERAL_POLICY],
        &accounts[IX_TOKEN_PROGRAM],
    )?;
    let (market, runtime) = authenticate_general_market_v2(
        program_id,
        &accounts[IX_MARKET_BINDING],
        &accounts[IX_MARKET_RUNTIME],
    )?;
    let base = market.base();
    let instance = *authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        &accounts[IX_MARKET_INSTANCE],
        ContentId::from_bytes(base.market_instance_v2_id.bytes()),
    )?
    .value();
    let genesis = *authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        &accounts[IX_MARKET_GENESIS],
        ContentId::from_bytes(base.market_genesis_profile_v2_id.bytes()),
    )?
    .value();
    require(
        instance
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes()
            == base.market_instance_v2_id.bytes()
            && runtime.market_instance_v2_id == base.market_instance_v2_id
            && instance.market_genesis_profile_id.content_id().bytes()
                == base.market_genesis_profile_v2_id.bytes()
            && genesis.realm_id.bytes() == realm.realm().realm.bytes()
            && genesis.profile_id.bytes() == realm.realm().profile.bytes()
            && genesis.price_measure_policy_id.content_id().bytes()
                == base.price_measure_policy_v1_id.bytes()
            && genesis.relation_policy_id.bytes() == base.relation_policy_id.bytes()
            && genesis.score_policy_id.bytes() == base.score_policy_id.bytes()
            && genesis.capability_profile_id.bytes() == capabilities::PROFILE_ID,
        ClutchError::MismatchedState,
    )?;
    let market_bytes = base.market_instance_v2_id.bytes();
    let bound = refine_market_collateral_v2(
        realm,
        MarketCollateralBindingV2 {
            market: CollateralId::from_bytes(market_bytes),
            realm: CollateralId::from_bytes(realm.realm().realm.bytes()),
            profile: CollateralId::from_bytes(realm.realm().profile.bytes()),
            collateral_cap_atoms: instance.collateral_cap,
            hoard_authority: CollateralId::from_bytes(
                seeds::hoard_authority_v2_pda(program_id, &market_bytes)
                    .0
                    .to_bytes(),
            ),
            hoard_token_account: CollateralId::from_bytes(
                seeds::hoard_token_v2_pda(program_id, &market_bytes)
                    .0
                    .to_bytes(),
            ),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    Ok((market, bound, genesis))
}

fn authenticate_pages<'a, 'info>(
    program_id: &Pubkey,
    epoch: Id32,
    market: Id32,
    accounts: &'a [AccountInfo<'info>],
    first_page: usize,
) -> Outcome<([Option<Ref<'a, [u8]>>; MAX_ORDER_PAGES], usize)> {
    let page_count = accounts
        .len()
        .checked_sub(first_page)
        .ok_or(Refusal::Adapter(ClutchError::WrongAccountCount))?;
    require(
        (1..=MAX_ORDER_PAGES).contains(&page_count),
        ClutchError::WrongAccountCount,
    )?;
    let mut refs: [Option<Ref<'a, [u8]>>; MAX_ORDER_PAGES] = [None, None, None, None];
    let mut index = 0usize;
    while index < page_count {
        let account = &accounts[first_page + index];
        require_program_state(program_id, account, false, Some(ORDER_PAGE_V5_BYTES))?;
        let page_index = u16::try_from(index).map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
        let expected = seeds::general_v2_order_page_v5_pda(program_id, &epoch.bytes(), page_index);
        require(*account.key == expected.0, ClutchError::WrongPda)?;
        let data = borrow_data(account)?;
        let page = verify_page_v5(&data)?;
        require(
            page.page.page_index == page_index
                && page.page.epoch.bytes() == epoch.bytes()
                && page.page.market.bytes() == market.bytes()
                && page.page.stored_bump == expected.1,
            ClutchError::MismatchedState,
        )?;
        refs[index] = Some(data);
        index += 1;
    }
    Ok((refs, page_count))
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
    request: contract::InitializeSettlementRootPayloadV1,
) -> Outcome<()> {
    require(
        accounts.len()
            >= ACTION39_COMMON_PREFIX_ACCOUNTS + ACTION39_CREATION_SUFFIX_ACCOUNTS + 1,
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
    require_program_state(program_id, &accounts[IX_FEED], false, None)?;
    require_program_state(
        program_id,
        &accounts[IX_ECONOMIC_DOMAIN],
        false,
        Some(contract::ECONOMIC_DOMAIN_ACCOUNT_BYTES),
    )?;
    require_program_state(
        program_id,
        &accounts[IX_PRICE_GRID],
        false,
        Some(account_len::PRICE_GRID),
    )?;

    let epoch = GeneralEpochV6AccountV1::decode(&borrow_data(&accounts[IX_EPOCH])?)?;
    let window = CandidateWindowV5AccountV1::decode(&borrow_data(&accounts[IX_WINDOW])?)?;
    let node = AdmissionNodeV4AccountV1::decode(&borrow_data(&accounts[IX_NODE])?)?;
    let feed_data = borrow_data(&accounts[IX_FEED])?;
    let (feed, _) = contract::complete_candidate_feed_v2(&feed_data, true)?;
    let domain = contract::EconomicDomainV2AccountV1::decode(&borrow_data(
        &accounts[IX_ECONOMIC_DOMAIN],
    )?)?;
    let grid = PriceGridAccount::decode(&borrow_data(&accounts[IX_PRICE_GRID])?)?;
    let (market, collateral, genesis) = authenticate_market_context(program_id, accounts)?;
    let base = market.base();

    let epoch_pda = seeds::general_v2_epoch_pda(
        program_id,
        &accounts[IX_MARKET_BINDING].key.to_bytes(),
        epoch.epoch_index,
    );
    expect_pda(accounts[IX_EPOCH].key, epoch_pda, Some(epoch.stored_bump))?;
    expect_pda(
        accounts[IX_WINDOW].key,
        seeds::general_v2_window_pda(program_id, &request.epoch.bytes()),
        None,
    )?;
    expect_pda(
        accounts[IX_ECONOMIC_DOMAIN].key,
        seeds::general_v2_economic_domain_pda(program_id, &request.epoch.bytes()),
        Some(domain.stored_bump),
    )?;
    expect_pda(
        accounts[IX_NODE].key,
        seeds::general_v2_node_pda(program_id, &request.epoch.bytes(), node.base().ordinal),
        Some(node.base().stored_bump),
    )?;
    expect_pda(
        accounts[IX_FEED].key,
        seeds::general_v2_feed_pda(program_id, &accounts[IX_NODE].key.to_bytes()),
        Some(feed.stored_bump),
    )?;
    let grid_pda = seeds::grid_pda(program_id, &grid.realm.bytes(), &grid.grid.bytes());
    expect_pda(accounts[IX_PRICE_GRID].key, grid_pda, Some(grid.stored_bump))?;
    require(
        request.selected_node == id(accounts[IX_NODE].key)
            && epoch.window == id(accounts[IX_WINDOW].key)
            && epoch.economic_domain == id(accounts[IX_ECONOMIC_DOMAIN].key)
            && epoch.market_binding == id(accounts[IX_MARKET_BINDING].key)
            && epoch.market_runtime == id(accounts[IX_MARKET_RUNTIME].key)
            && domain.epoch == request.epoch
            && feed.node == id(accounts[IX_NODE].key)
            && feed.order_set == epoch.order_set
            && grid.realm.bytes() == genesis.realm_id.bytes()
            && grid.grid.bytes() == genesis.price_grid_id.bytes(),
        ClutchError::MismatchedState,
    )?;

    let selected_fee_pda = seeds::general_v2_selected_fee_record_pda(
        program_id,
        &node.base().settlement_candidate_id.bytes(),
    );
    require(
        *accounts[IX_SELECTED_FEE_RECORD].key == selected_fee_pda.0
            && !accounts[IX_SELECTED_FEE_RECORD].is_writable
            && !accounts[IX_SELECTED_FEE_RECORD].is_signer
            && !accounts[IX_SELECTED_FEE_RECORD].executable,
        ClutchError::MismatchedState,
    )?;
    let fee_present = accounts[IX_SELECTED_FEE_RECORD].owner == program_id;
    let fee_suffix = if fee_present {
        ACTION39_FEE_SUFFIX_ACCOUNTS
    } else {
        require(
            *accounts[IX_SELECTED_FEE_RECORD].owner == SYSTEM_PROGRAM_ID
                && accounts[IX_SELECTED_FEE_RECORD].data_len() == 0,
            ClutchError::MismatchedState,
        )?;
        0
    };
    let creation_at = ACTION39_COMMON_PREFIX_ACCOUNTS + fee_suffix;
    let ix_root = creation_at;
    let ix_cash_pot = creation_at + 1;
    let ix_final_pot = creation_at + 2;
    let ix_payer = creation_at + 3;
    let ix_system = creation_at + 4;
    let ix_rent = creation_at + 5;
    let ix_clock = creation_at + 6;
    let first_page = creation_at + ACTION39_CREATION_SUFFIX_ACCOUNTS;
    require(
        accounts.len() > first_page && accounts.len() <= first_page + MAX_ORDER_PAGES,
        ClutchError::WrongAccountCount,
    )?;
    require_signer(&accounts[ix_payer])?;
    require(accounts[ix_payer].is_writable, ClutchError::NotWritable)?;
    require_system_program(&accounts[ix_system])?;
    let rent = read_rent(&accounts[ix_rent])?;
    let current_slot = read_clock_slot(&accounts[ix_clock])?;

    let (page_refs, page_count) = authenticate_pages(
        program_id,
        request.epoch,
        base.market,
        accounts,
        first_page,
    )?;
    let mut page_inputs = [GeneralOrderPageInputV5 {
        account: Id32::ZERO,
        body: &[],
    }; MAX_ORDER_PAGES];
    let mut page_index = 0usize;
    while page_index < page_count {
        page_inputs[page_index] = GeneralOrderPageInputV5 {
            account: id(accounts[first_page + page_index].key),
            body: page_refs[page_index]
                .as_ref()
                .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?,
        };
        page_index += 1;
    }
    let order_projection = std::boxed::Box::new(project_owner_blind_book_costed_v1(
        &page_inputs[..page_count],
        epoch.order_set,
        &domain,
        &market,
        &grid,
    )?);
    let traversal = std::boxed::Box::new(derive_settlement_traversal_projection_v4(
        id(accounts[IX_FEED].key),
        &feed_data,
        &order_projection,
        base.series_funding_terms_v2_id,
        base.settlement_policy_id,
        collateral,
    )?);

    let expectation: SettlementRootExpectationProjectionV1 = if fee_present {
        let revenue_preimage = &accounts[ACTION39_COMMON_PREFIX_ACCOUNTS + 2];
        require(
            !revenue_preimage.is_writable
                && !revenue_preimage.is_signer
                && !revenue_preimage.executable
                && revenue_preimage.data_len() == REVENUE_POLICY_BYTES,
            ClutchError::MismatchedState,
        )?;
        let revenue: RevenuePolicyV1 = decode_revenue_policy(&borrow_data(revenue_preimage)?)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let expected = CandidateFeeCollectionExpectationV5::new(
            Id32::from_bytes(genesis.realm_id.bytes()),
            base.market,
            request.epoch,
            node.base().settlement_candidate_id,
            id(accounts[IX_SELECTED_FEE_RECORD].key),
            market.batch_policy_id(),
            traversal.owner_order_set_digest(),
            traversal.owner_basis().owner_count(),
            base.price_scale,
            base.outcome_count,
        )?;
        compose_candidate_fee_collection_action39_v5(
            program_id,
            expected,
            CandidateFeeCollectionAccountFrameV5 {
                selected_fee_record: &accounts[IX_SELECTED_FEE_RECORD],
                certified_recipient_allocation: &accounts[ACTION39_COMMON_PREFIX_ACCOUNTS],
                batch_policy: &accounts[ACTION39_COMMON_PREFIX_ACCOUNTS + 1],
                revenue_policy_record: &accounts[ACTION39_COMMON_PREFIX_ACCOUNTS + 3],
            },
            &revenue,
            &traversal,
        )?
        .root_expectation()
    } else {
        derive_settlement_root_expectation_v1(
            &traversal,
            CandidateFeeAggregateProjectionV1::NoFeeRecord,
        )?
    };

    let root_pda = seeds::general_v2_settlement_root_pda(
        program_id,
        &request.epoch.bytes(),
        &node.base().settlement_candidate_id.bytes(),
    );
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
            && *accounts[ix_final_pot].key == final_pda.0,
        ClutchError::WrongPda,
    )?;
    for target in [&accounts[ix_root], &accounts[ix_cash_pot], &accounts[ix_final_pot]] {
        require_creatable(target)?;
        require(target.is_writable && !target.is_signer, ClutchError::NotWritable)?;
    }
    let root_rent = rent_owner(
        &accounts[ix_payer],
        &accounts[ix_root],
        &rent,
        contract::SETTLEMENT_ROOT_ACCOUNT_BYTES,
    )?;
    let cash_rent = rent_owner(
        &accounts[ix_payer],
        &accounts[ix_cash_pot],
        &rent,
        contract::SETTLEMENT_CASH_POT_ACCOUNT_BYTES,
    )?;
    let has_final_pot = expectation.cash().virtual_cash_direction
        != clutch_owner_settlement::VirtualCashDirectionV1::None;
    let final_rent = rent_owner(
        &accounts[ix_payer],
        &accounts[ix_final_pot],
        &rent,
        contract::FINAL_POT_ACCOUNT_BYTES,
    )?;
    let plan = contract::initialize_settlement_root_v1(InitializeSettlementRootV1 {
        root_account: id(accounts[ix_root].key),
        root_bump: root_pda.1,
        epoch_account: request.epoch,
        market_binding_account: id(accounts[IX_MARKET_BINDING].key),
        window_account: id(accounts[IX_WINDOW].key),
        retained_feed_account: id(accounts[IX_FEED].key),
        epoch_generation: epoch.generation,
        market_instance_v2_id: base.market_instance_v2_id,
        epoch: &epoch,
        market: &market,
        window: &window,
        node: &node,
        feed: &feed,
        current_slot,
        owner_order_set_digest: traversal.owner_order_set_digest(),
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

    let root_epoch = request.epoch.bytes();
    let candidate = node.base().settlement_candidate_id.bytes();
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
        contract::SETTLEMENT_ROOT_ACCOUNT_BYTES,
        root_rent,
        &root_seeds,
    )?;
    if let Some(cash) = plan.cash_pot() {
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
    if let Some(final_pot) = plan.final_pot() {
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
    encode_account(&accounts[ix_root], |out| plan.root().encode(out))?;
    encode_account(&accounts[IX_EPOCH], |out| plan.epoch().encode(out))?;
    encode_account(&accounts[IX_WINDOW], |out| plan.window().encode(out))
}
