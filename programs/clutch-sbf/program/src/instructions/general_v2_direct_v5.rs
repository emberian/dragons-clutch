//! Live SBF composition for General V2 action 26 direct Egg delivery.
//!
//! Account order is fixed and exhaustive:
//! 0 SettlementRoot, 1 retained Feed, 2 ReceiptV5, 3 MarketBindingV2,
//! 4 MarketRuntimeV3, 5 Realm, 6 ProfileV2, 7 collateral policy,
//! 8 Token-2022 program, 9 MarketInstanceV2 artifact,
//! 10 MarketGenesisProfileV2 artifact, 11 Rent sysvar,
//! then buyer and seller groups of five accounts each: read-only OwnerRowV5,
//! read-only OrderPageV5, writable ReservationV9, writable PositionV3, and
//! writable GEN1 ReplayV3.
//!
//! This action performs no CPI, lamport movement, account creation, or close.
//! It authenticates every prestate first, derives the indivisible pure plan,
//! and writes only its Receipt/Reservation/Position/Replay successors.

use core::cell::Ref;

use clutch_collateral_adapter_v2::{
    refine_market_collateral_v2, Id as CollateralId, MarketCollateralBindingV2,
};
use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    decode_direct_settlement_payload_v1, DirectSettlementPayloadV1, GeneralOrderPageSeedTupleV5,
    GeneralReservationSeedTupleV9, Id32, MarketBindingV2, OwnerSettlementSeedTupleV5,
    OwnerSettlementV5AccountV1,
};
use clutch_general_v2_runtime::{
    prepare_consume_direct_receipt_eggs_v5, project_owner_settlement_account_v5_readonly,
    ConsumeDirectReceiptEggsInputV5, DirectEggDeliveryEndpointInputV5,
    OwnerSettlementAccountProjectionV5, OwnerSettlementAccountViewV5, PositionAccountInputV3,
};
use clutch_product_series::{ContentId, MarketGenesisProfileV2, MarketInstancePreimageV2};
use clutch_retirement::{PositionPurposeV3, POSITION_V3_BYTES};
use clutch_solana_layout::order_page_v5::{verify_page_v5, ORDER_PAGE_V5_BYTES};
use clutch_solana_layout::registry::GeneralV2Action;
use clutch_solana_layout::reservation_v9::{ReservationAccountV9, RESERVATION_ACCOUNT_BYTES_V9};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, require_count, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::read_rent;
use crate::seeds;

use super::collateral_position_v3::{
    authenticate_general_market_v2, GeneralPositionReplayAuthorityV2,
};
use super::general_v2_position_replay::authenticate_current_general_position_replay_v2;
use super::general_v2_receipt_v5::{
    authenticate_general_receipt_v5_readonly_root, AuthenticatedGeneralReceiptV5,
    RECEIPT_V5_AUTH_ACCOUNT_COUNT,
};
use super::product_artifact::authenticate_product_artifact_v1;

/// Exact action-26 account count.
pub const ACCOUNT_COUNT: usize = 22;
pub const IX_ROOT: usize = 0;
pub const IX_FEED: usize = 1;
pub const IX_RECEIPT: usize = 2;
pub const IX_MARKET_BINDING: usize = 3;
pub const IX_MARKET_RUNTIME: usize = 4;
pub const IX_REALM: usize = 5;
pub const IX_PROFILE: usize = 6;
pub const IX_COLLATERAL_POLICY: usize = 7;
pub const IX_TOKEN_PROGRAM: usize = 8;
pub const IX_MARKET_INSTANCE: usize = 9;
pub const IX_MARKET_GENESIS: usize = 10;
pub const IX_RENT_SYSVAR: usize = 11;
pub const IX_BUYER_ROW: usize = 12;
pub const IX_BUYER_PAGE: usize = 13;
pub const IX_BUYER_RESERVATION: usize = 14;
pub const IX_BUYER_POSITION: usize = 15;
pub const IX_BUYER_REPLAY: usize = 16;
pub const IX_SELLER_ROW: usize = 17;
pub const IX_SELLER_PAGE: usize = 18;
pub const IX_SELLER_RESERVATION: usize = 19;
pub const IX_SELLER_POSITION: usize = 20;
pub const IX_SELLER_REPLAY: usize = 21;

#[derive(Debug)]
struct EndpointData<'a> {
    owner_row: Ref<'a, [u8]>,
    page: Ref<'a, [u8]>,
    reservation: Ref<'a, [u8]>,
    position: Ref<'a, [u8]>,
    replay: Ref<'a, [u8]>,
}

#[derive(Clone, Copy, Debug)]
struct AuthenticatedEndpointV5 {
    owner: Id32,
    owner_row: OwnerSettlementAccountProjectionV5,
    replay: GeneralPositionReplayAuthorityV2,
    replay_bump: u8,
}

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
    exact_len: usize,
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
    require(
        account.data_len() == exact_len,
        ClutchError::WrongDataLength,
    )
}

fn require_canonical_alias_partition(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        require(!accounts[left].is_signer, ClutchError::MismatchedState)?;
        let mut right = left + 1;
        while right < accounts.len() {
            let shared_page = left == IX_BUYER_PAGE && right == IX_SELLER_PAGE;
            require(
                accounts[left].key != accounts[right].key || shared_page,
                ClutchError::AccountAlias,
            )?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

pub(crate) fn authenticate_market_collateral_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    receipt: &AuthenticatedGeneralReceiptV5,
) -> Outcome<(
    clutch_collateral_adapter_v2::BoundCollateralProfileV2,
    MarketBindingV2,
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
    let root = receipt.root();
    require(
        root.market_binding().bytes() == accounts[IX_MARKET_BINDING].key.to_bytes()
            && root.market().bytes() == accounts[IX_MARKET_RUNTIME].key.to_bytes()
            && root.market_instance_v2_id() == base.market_instance_v2_id
            && runtime.market_instance_v2_id == base.market_instance_v2_id,
        ClutchError::MismatchedState,
    )?;
    let artifact = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        &accounts[IX_MARKET_INSTANCE],
        ContentId::from_bytes(root.market_instance_v2_id().bytes()),
    )?;
    let instance = *artifact.value();
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        &accounts[IX_MARKET_GENESIS],
        ContentId::from_bytes(base.market_genesis_profile_v2_id.bytes()),
    )?;
    let genesis = *genesis.value();
    require(
        instance
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes()
            == root.market_instance_v2_id().bytes()
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
    let market_id = CollateralId::from_bytes(root.market_instance_v2_id().bytes());
    let market_bytes = root.market_instance_v2_id().bytes();
    let bound = refine_market_collateral_v2(
        realm,
        MarketCollateralBindingV2 {
            market: market_id,
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
    Ok((bound, market))
}

#[allow(clippy::too_many_arguments)]
fn authenticate_endpoint_v5(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    receipt: &AuthenticatedGeneralReceiptV5,
    bound: clutch_collateral_adapter_v2::BoundCollateralProfileV2,
    rent_minimum: u64,
    row_index: usize,
    page_index: usize,
    reservation_index: usize,
    position_index: usize,
    replay_index: usize,
    data: &EndpointData<'_>,
) -> Outcome<AuthenticatedEndpointV5> {
    require_program_state(
        program_id,
        &accounts[row_index],
        false,
        contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5,
    )?;
    let row = OwnerSettlementV5AccountV1::decode(&data.owner_row)?;
    let expectation = row.semantic.expectation();
    let owner = Id32::new(expectation.owner())?;
    let root = receipt.root();
    let row_seed =
        OwnerSettlementSeedTupleV5::new(root.epoch(), root.settlement_candidate_id(), owner)?;
    let row_pda = seeds::general_v2_owner_settlement_v5_pda(
        program_id,
        row_seed.epoch(),
        row_seed.settlement_candidate(),
        row_seed.owner(),
    );
    expect_pda(accounts[row_index].key, row_pda, Some(row.stored_bump))?;
    let owner_row = project_owner_settlement_account_v5_readonly(
        OwnerSettlementAccountViewV5 {
            account: id(accounts[row_index].key),
            program_owner: id(accounts[row_index].owner),
            exact_body: &data.owner_row,
            lamports: accounts[row_index].lamports(),
            rent_minimum,
            canonical_bump: row_pda.1,
            writable: accounts[row_index].is_writable,
        },
        id(program_id),
        row_seed,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    require_program_state(
        program_id,
        &accounts[page_index],
        false,
        ORDER_PAGE_V5_BYTES,
    )?;
    let page =
        verify_page_v5(&data.page).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let page_seed = GeneralOrderPageSeedTupleV5::new(root.epoch(), page.page_index)?;
    let page_pda = seeds::general_v2_order_page_v5_pda(
        program_id,
        page_seed.epoch(),
        u16::from_le_bytes(*page_seed.page_index_le()),
    );
    expect_pda(accounts[page_index].key, page_pda, Some(page.stored_bump))?;
    require(
        page.frozen == 1
            && page.market.0 == root.market().bytes()
            && page.epoch.0 == root.epoch().bytes()
            && page.order_set.0 == root.order_set().bytes(),
        ClutchError::MismatchedState,
    )?;

    require_program_state(
        program_id,
        &accounts[reservation_index],
        true,
        RESERVATION_ACCOUNT_BYTES_V9,
    )?;
    let reservation = ReservationAccountV9::decode(&data.reservation)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let reservation_id = Id32::new(reservation.body().reservation.bytes())?;
    let reservation_seed = GeneralReservationSeedTupleV9::new(reservation_id)?;
    let reservation_pda =
        seeds::general_v2_reservation_v9_pda(program_id, reservation_seed.reservation_id());
    expect_pda(
        accounts[reservation_index].key,
        reservation_pda,
        Some(reservation.body().stored_bump),
    )?;

    let replay = authenticate_current_general_position_replay_v2(
        program_id,
        bound,
        &accounts[IX_MARKET_BINDING],
        &accounts[IX_MARKET_RUNTIME],
        &accounts[position_index],
        &accounts[replay_index],
        owner.bytes(),
    )?;
    let replay_pda = seeds::purpose_replay_v3_pda(
        program_id,
        &accounts[position_index].key.to_bytes(),
        PositionPurposeV3::General,
        &accounts[IX_MARKET_RUNTIME].key.to_bytes(),
    );
    Ok(AuthenticatedEndpointV5 {
        owner,
        owner_row,
        replay,
        replay_bump: replay_pda.1,
    })
}

fn endpoint_input<'a, 'b>(
    accounts: &[AccountInfo<'_>],
    auth: AuthenticatedEndpointV5,
    data: &'a EndpointData<'b>,
    page_index: usize,
    reservation_index: usize,
    position_index: usize,
    replay_index: usize,
) -> DirectEggDeliveryEndpointInputV5<'a> {
    DirectEggDeliveryEndpointInputV5 {
        owner_row: auth.owner_row,
        order_page_account: id(accounts[page_index].key),
        order_page_body: &data.page,
        reservation_account: id(accounts[reservation_index].key),
        reservation_body: &data.reservation,
        position: PositionAccountInputV3 {
            account: id(accounts[position_index].key),
            encoded_body: &data.position,
        },
        replay_account: id(accounts[replay_index].key),
        replay_bump: auth.replay_bump,
        replay_next_sequence: auth.replay.replay.next_sequence(),
        replay_body: &data.replay,
    }
}

fn write_exact(account: &AccountInfo<'_>, expected: Id32, body: &[u8]) -> Outcome<()> {
    require(id(account.key) == expected, ClutchError::MismatchedState)?;
    require(account.is_writable, ClutchError::NotWritable)?;
    require(
        account.data_len() == body.len(),
        ClutchError::WrongDataLength,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.copy_from_slice(body);
    Ok(())
}

/// Decode and execute exactly one current action-26 request.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        action == GeneralV2Action::ConsumeDirectReceiptEggs
            && capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    require_count(accounts, ACCOUNT_COUNT)?;
    require_canonical_alias_partition(accounts)?;
    let request = match decode_direct_settlement_payload_v1(action.tag(), payload)? {
        DirectSettlementPayloadV1::ConsumeDirectReceiptEggs(request) => request,
    };
    let receipt = authenticate_general_receipt_v5_readonly_root(
        program_id,
        &accounts[..RECEIPT_V5_AUTH_ACCOUNT_COUNT],
    )?;
    require(
        request.epoch == receipt.root().epoch() && request.receipt == receipt.receipt_account(),
        ClutchError::MismatchedState,
    )?;
    let (bound, market_v2) = authenticate_market_collateral_v2(program_id, accounts, &receipt)?;
    let rent = read_rent(&accounts[IX_RENT_SYSVAR])?;
    let owner_row_rent = rent.minimum_balance(contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5)?;

    let buyer_data = EndpointData {
        owner_row: borrow_data(&accounts[IX_BUYER_ROW])?,
        page: borrow_data(&accounts[IX_BUYER_PAGE])?,
        reservation: borrow_data(&accounts[IX_BUYER_RESERVATION])?,
        position: borrow_data(&accounts[IX_BUYER_POSITION])?,
        replay: borrow_data(&accounts[IX_BUYER_REPLAY])?,
    };
    let seller_data = EndpointData {
        owner_row: borrow_data(&accounts[IX_SELLER_ROW])?,
        page: borrow_data(&accounts[IX_SELLER_PAGE])?,
        reservation: borrow_data(&accounts[IX_SELLER_RESERVATION])?,
        position: borrow_data(&accounts[IX_SELLER_POSITION])?,
        replay: borrow_data(&accounts[IX_SELLER_REPLAY])?,
    };
    let buyer_auth = authenticate_endpoint_v5(
        program_id,
        accounts,
        &receipt,
        bound,
        owner_row_rent,
        IX_BUYER_ROW,
        IX_BUYER_PAGE,
        IX_BUYER_RESERVATION,
        IX_BUYER_POSITION,
        IX_BUYER_REPLAY,
        &buyer_data,
    )?;
    let seller_auth = authenticate_endpoint_v5(
        program_id,
        accounts,
        &receipt,
        bound,
        owner_row_rent,
        IX_SELLER_ROW,
        IX_SELLER_PAGE,
        IX_SELLER_RESERVATION,
        IX_SELLER_POSITION,
        IX_SELLER_REPLAY,
        &seller_data,
    )?;
    require(
        buyer_auth.owner != seller_auth.owner,
        ClutchError::AccountAlias,
    )?;
    let feed_data = borrow_data(&accounts[IX_FEED])?;
    let relation_market = market_v2.relation_projection();
    let plan = prepare_consume_direct_receipt_eggs_v5(ConsumeDirectReceiptEggsInputV5 {
        payload: request,
        settlement_root_account: receipt.settlement_root_account(),
        settlement_root: receipt.root(),
        retained_feed_account: receipt.retained_feed_account(),
        retained_feed_body: &feed_data,
        receipt: receipt.receipt(),
        receipt_evidence: receipt.evidence(),
        market_binding_account: id(accounts[IX_MARKET_BINDING].key),
        market_binding: &relation_market,
        collateral: bound,
        buyer: endpoint_input(
            accounts,
            buyer_auth,
            &buyer_data,
            IX_BUYER_PAGE,
            IX_BUYER_RESERVATION,
            IX_BUYER_POSITION,
            IX_BUYER_REPLAY,
        ),
        seller: endpoint_input(
            accounts,
            seller_auth,
            &seller_data,
            IX_SELLER_PAGE,
            IX_SELLER_RESERVATION,
            IX_SELLER_POSITION,
            IX_SELLER_REPLAY,
        ),
    })
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    require(
        plan.settlement_root_account() == receipt.settlement_root_account()
            && plan.retained_feed_account() == receipt.retained_feed_account()
            && plan.receipt_account() == receipt.receipt_account()
            && plan.buyer().owner_settlement_account() == id(accounts[IX_BUYER_ROW].key)
            && plan.buyer().order_page_account() == id(accounts[IX_BUYER_PAGE].key)
            && plan.buyer().reservation_account() == id(accounts[IX_BUYER_RESERVATION].key)
            && plan.buyer().position_account() == id(accounts[IX_BUYER_POSITION].key)
            && plan.buyer().replay().replay_account() == id(accounts[IX_BUYER_REPLAY].key)
            && plan.seller().owner_settlement_account() == id(accounts[IX_SELLER_ROW].key)
            && plan.seller().order_page_account() == id(accounts[IX_SELLER_PAGE].key)
            && plan.seller().reservation_account() == id(accounts[IX_SELLER_RESERVATION].key)
            && plan.seller().position_account() == id(accounts[IX_SELLER_POSITION].key)
            && plan.seller().replay().replay_account() == id(accounts[IX_SELLER_REPLAY].key),
        ClutchError::MismatchedState,
    )?;

    drop(feed_data);
    drop(buyer_data);
    drop(seller_data);
    write_exact(
        &accounts[IX_RECEIPT],
        plan.receipt_account(),
        plan.receipt_poststate_body(),
    )?;
    write_exact(
        &accounts[IX_BUYER_RESERVATION],
        plan.buyer().reservation_account(),
        plan.buyer().reservation_poststate_body(),
    )?;
    write_exact(
        &accounts[IX_BUYER_POSITION],
        plan.buyer().position_account(),
        plan.buyer().position_poststate_body(),
    )?;
    write_exact(
        &accounts[IX_BUYER_REPLAY],
        plan.buyer().replay().replay_account(),
        plan.buyer().replay().replay_poststate_body(),
    )?;
    write_exact(
        &accounts[IX_SELLER_RESERVATION],
        plan.seller().reservation_account(),
        plan.seller().reservation_poststate_body(),
    )?;
    write_exact(
        &accounts[IX_SELLER_POSITION],
        plan.seller().position_account(),
        plan.seller().position_poststate_body(),
    )?;
    write_exact(
        &accounts[IX_SELLER_REPLAY],
        plan.seller().replay().replay_account(),
        plan.seller().replay().replay_poststate_body(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_v5_account_roles_are_exhaustive() {
        let roles = [
            IX_ROOT,
            IX_FEED,
            IX_RECEIPT,
            IX_MARKET_BINDING,
            IX_MARKET_RUNTIME,
            IX_REALM,
            IX_PROFILE,
            IX_COLLATERAL_POLICY,
            IX_TOKEN_PROGRAM,
            IX_MARKET_INSTANCE,
            IX_MARKET_GENESIS,
            IX_RENT_SYSVAR,
            IX_BUYER_ROW,
            IX_BUYER_PAGE,
            IX_BUYER_RESERVATION,
            IX_BUYER_POSITION,
            IX_BUYER_REPLAY,
            IX_SELLER_ROW,
            IX_SELLER_PAGE,
            IX_SELLER_RESERVATION,
            IX_SELLER_POSITION,
            IX_SELLER_REPLAY,
        ];
        assert_eq!(roles.len(), ACCOUNT_COUNT);
        for (expected, observed) in roles.into_iter().enumerate() {
            assert_eq!(observed, expected);
        }
        assert_eq!(RECEIPT_V5_AUTH_ACCOUNT_COUNT, 3);
    }
}
