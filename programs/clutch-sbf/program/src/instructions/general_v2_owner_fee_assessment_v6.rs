//! Bounded page-streamed owner-fee assessment for General action 24.
//!
//! The deployed runtime admits at most 64 accounts. This continuation uses at
//! most 41 instruction AccountInfos: one exact retained Feed/page authority,
//! the compact root-bound locator pair, current fee policies, one transient
//! work account, and at most the 16 ReservationV9 envelopes owned by the
//! selected owner on that page. It never advances the counted SettlementRoot
//! or creates a fee liability.

use core::cell::Ref;
use std::boxed::Box;

use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    DeletableRentOwnerV1, Id32, IndexedSettlementRootV1AccountV1,
    OwnerFeeAssessmentAuthorityV1, OwnerFeeAssessmentEnvelopeV1,
    OwnerFeeAssessmentWorkV1AccountV1, SelectedFeeRecordV2AccountV1,
    SettlementRootChildStateV1, SettlementRootPhaseV1,
    INDEXED_SETTLEMENT_ROOT_BYTES_V1, OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_BYTES_V1,
};
use clutch_general_v2_runtime::{
    authenticate_counted_exact_index_read_v1, derive_owner_fee_envelope_from_page_v2,
    frozen_order_location_from_sealed_accounts_v1, AuthenticateCountedExactIndexReadInputV1,
    ExactIndexReadAccountInputV1, SettlementLegV1,
};
use clutch_solana_layout::reservation_v9::{ReservationAccountV9, RESERVATION_ACCOUNT_BYTES_V9};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, require_signer, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{read_rent, require_creatable, require_system_program, SYSTEM_PROGRAM_ID};
use crate::seeds;

use super::general_v2_fee_v5::{
    authenticate_owner_fee_common_core_v5, OwnerFeeCommonAccountFrameV5,
};
use super::general_v2_settlement_producer_v5::{create_from_payer, encode_account, rent_owner};
use super::general_v2_settlement_root::authenticate_readonly_general_settlement_root_v1;
use super::general_v2_settlement_traversal_v5::{
    authenticate_settlement_page_continuation_v5,
    AuthenticatedSettlementPageContinuationV5, SettlementPageContinuationFrameV5,
};

pub(crate) const ACTION24_ASSESSMENT_FIXED_ACCOUNTS_V1: usize = 25;
pub(crate) const ACTION24_ASSESSMENT_MAX_ACCOUNTS_V1: usize =
    ACTION24_ASSESSMENT_FIXED_ACCOUNTS_V1 + 16;

const IX_ROOT: usize = 0;
const IX_LOCATOR: usize = 1;
const IX_ADJACENCY: usize = 2;
const IX_FEED: usize = 3;
const IX_BINDING: usize = 4;
const IX_RUNTIME: usize = 5;
const IX_DOMAIN: usize = 6;
const IX_GRID: usize = 7;
const IX_REALM: usize = 8;
const IX_PROFILE: usize = 9;
const IX_POLICY: usize = 10;
const IX_TOKEN: usize = 11;
const IX_MARKET_INSTANCE: usize = 12;
const IX_GENESIS: usize = 13;
const IX_PAGE: usize = 14;
const IX_OWNER_ROW: usize = 15;
const IX_WORK: usize = 16;
const IX_SELECTED_FEE: usize = 17;
const IX_BATCH_POLICY: usize = 18;
const IX_REVENUE_RECORD: usize = 19;
const IX_REVENUE_PREIMAGE: usize = 20;
const IX_WORK_RENT_PAYER: usize = 21;
const IX_NEUTRAL_SINK: usize = 22;
const IX_SYSTEM: usize = 23;
const IX_RENT: usize = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Action24AssessmentDispatchV1 {
    NotAssessmentContinuation,
    Applied,
}

#[derive(Clone, Copy, Debug)]
struct RuntimeSha256;

impl contract::Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

fn id(key: &Pubkey) -> Id32 { Id32::from_bytes(key.to_bytes()) }

fn borrow_data<'a, 'info>(account: &'a AccountInfo<'info>) -> Outcome<Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(Ref::map(data, |bytes| &**bytes))
}

#[derive(Debug)]
struct ExactIndexAccountBorrowsV1<'a> {
    root: Ref<'a, [u8]>,
    locator: Ref<'a, [u8]>,
    adjacency: Ref<'a, [u8]>,
    feed: Ref<'a, [u8]>,
}

fn borrow_index_accounts_v1<'a>(
    accounts: &'a [AccountInfo<'_>],
) -> Outcome<ExactIndexAccountBorrowsV1<'a>> {
    Ok(ExactIndexAccountBorrowsV1 {
        root: borrow_data(&accounts[IX_ROOT])?,
        locator: borrow_data(&accounts[IX_LOCATOR])?,
        adjacency: borrow_data(&accounts[IX_ADJACENCY])?,
        feed: borrow_data(&accounts[IX_FEED])?,
    })
}

pub(crate) fn boxed_work_scratch_v6() -> Outcome<Box<OwnerFeeAssessmentWorkV1AccountV1>> {
    let layout = core::alloc::Layout::new::<OwnerFeeAssessmentWorkV1AccountV1>();
    unsafe {
        let pointer = std::alloc::alloc_zeroed(layout) as *mut OwnerFeeAssessmentWorkV1AccountV1;
        if pointer.is_null() {
            return Err(Refusal::Adapter(ClutchError::AccountCreationFailed));
        }
        // Every Rust field admits the all-zero representation; the strict
        // contract decoder/begin_into overwrites and validates it before any
        // semantic getter is used.
        Ok(Box::from_raw(pointer))
    }
}

fn require_pairwise_distinct(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            require(accounts[left].key != accounts[right].key, ClutchError::AccountAlias)?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn require_readonly_program_account(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    exact_len: Option<usize>,
) -> Outcome<()> {
    require(
        account.owner == program_id
            && !account.is_signer
            && !account.is_writable
            && !account.executable,
        ClutchError::MismatchedState,
    )?;
    if let Some(len) = exact_len {
        require(account.data_len() == len, ClutchError::WrongDataLength)?;
    }
    Ok(())
}

fn current_endpoint_indices(
    page: &AuthenticatedSettlementPageContinuationV5<'_>,
    root: &contract::SettlementRootV1AccountV1,
) -> Outcome<([u8; 2], usize)> {
    let slice = page.settlement_slice(root.counts().admitted_receipts)?;
    match (slice.buy(), slice.sell()) {
        (SettlementLegV1::Order(buy), SettlementLegV1::Order(sell)) => Ok(([buy, sell], 2)),
        (SettlementLegV1::Order(buy), SettlementLegV1::Split) => Ok(([buy, 0], 1)),
        (SettlementLegV1::Merge, SettlementLegV1::Order(sell)) => Ok(([sell, 0], 1)),
        _ => Err(Refusal::Adapter(ClutchError::MismatchedState)),
    }
}

fn discover_current_owner(
    program_id: &Pubkey,
    page: &AuthenticatedSettlementPageContinuationV5<'_>,
    sealed: &clutch_general_v2_runtime::SealedExactIndexPairInputV1<'_>,
    root: &contract::SettlementRootV1AccountV1,
    owner_row: &AccountInfo<'_>,
) -> Outcome<Id32> {
    let (indices, len) = current_endpoint_indices(page, root)?;
    let mut found = None;
    let mut at = 0usize;
    while at < len {
        let order_index = indices[at];
        let location = frozen_order_location_from_sealed_accounts_v1(sealed, order_index)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        if location.page_index() == u16::from(page.page_index()) {
            let row = page.order_at_physical_slot(location.page_slot())?;
            let owner = row.membership().owner();
            let seed = contract::OwnerSettlementSeedTupleV5::new(
                root.epoch(),
                root.settlement_candidate_id(),
                owner,
            )?;
            let expected = seeds::find(
                program_id,
                &[seed.domain(), seed.epoch(), seed.settlement_candidate(), seed.owner()],
            );
            if *owner_row.key == expected.0 {
                require(found.is_none(), ClutchError::MismatchedState)?;
                found = Some(owner);
            }
        }
        at += 1;
    }
    found.ok_or(Refusal::Adapter(ClutchError::MismatchedState))
}

#[allow(clippy::too_many_arguments)]
fn authenticate_index_pair<'a>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &'a ExactIndexAccountBorrowsV1<'_>,
    page: &AuthenticatedSettlementPageContinuationV5<'_>,
    root_bump: u8,
) -> Outcome<clutch_general_v2_runtime::SealedExactIndexPairInputV1<'a>> {
    for index in [IX_ROOT, IX_LOCATOR, IX_ADJACENCY, IX_FEED] {
        require_readonly_program_account(program_id, &accounts[index], None)?;
    }
    let locator_pda = seeds::general_v2_frozen_order_locator_pda(
        program_id,
        &accounts[IX_ROOT].key.to_bytes(),
    );
    let adjacency_pda = seeds::general_v2_candidate_slice_index_pda(
        program_id,
        &accounts[IX_ROOT].key.to_bytes(),
    );
    authenticate_counted_exact_index_read_v1(AuthenticateCountedExactIndexReadInputV1 {
        program_id: id(program_id),
        root: ExactIndexReadAccountInputV1 {
            account: id(accounts[IX_ROOT].key), body: &data.root, owner: id(accounts[IX_ROOT].owner),
            canonical_account: id(accounts[IX_ROOT].key), canonical_bump: root_bump,
            writable: false, executable: accounts[IX_ROOT].executable,
        },
        locator: ExactIndexReadAccountInputV1 {
            account: id(accounts[IX_LOCATOR].key), body: &data.locator,
            owner: id(accounts[IX_LOCATOR].owner), canonical_account: id(&locator_pda.0),
            canonical_bump: locator_pda.1, writable: false,
            executable: accounts[IX_LOCATOR].executable,
        },
        adjacency: ExactIndexReadAccountInputV1 {
            account: id(accounts[IX_ADJACENCY].key), body: &data.adjacency,
            owner: id(accounts[IX_ADJACENCY].owner), canonical_account: id(&adjacency_pda.0),
            canonical_bump: adjacency_pda.1, writable: false,
            executable: accounts[IX_ADJACENCY].executable,
        },
        feed: ExactIndexReadAccountInputV1 {
            account: page.feed_account(), body: &data.feed, owner: id(accounts[IX_FEED].owner),
            canonical_account: page.feed_account(), canonical_bump: page.feed().stored_bump,
            writable: false, executable: accounts[IX_FEED].executable,
        },
    })
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

fn selected_fee_data_id(
    selected_account: &AccountInfo<'_>,
    selected: &clutch_fee_runtime_contract::selected::SelectedCompositeFeeV2,
) -> Outcome<Id32> {
    let outer = SelectedFeeRecordV2AccountV1::decode_persisted(&borrow_data(selected_account)?)?;
    require(outer.semantic == *selected, ClutchError::MismatchedState)?;
    outer.data_id(&RuntimeSha256, id(selected_account.key)).map_err(Into::into)
}

fn work_rent_from_account(
    work: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    rent: &crate::instructions::genesis::RentParameters,
) -> Outcome<DeletableRentOwnerV1> {
    rent_owner(
        payer,
        work,
        rent,
        OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_BYTES_V1,
    )
}

fn apply_work_page(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    page: &AuthenticatedSettlementPageContinuationV5<'_>,
    sealed: &clutch_general_v2_runtime::SealedExactIndexPairInputV1<'_>,
    work: &mut OwnerFeeAssessmentWorkV1AccountV1,
    process_page: bool,
) -> Outcome<()> {
    if !process_page {
        require(
            accounts.len() == ACTION24_ASSESSMENT_FIXED_ACCOUNTS_V1,
            ClutchError::WrongAccountCount,
        )?;
        return Ok(());
    }
    require(
        work.semantic.next_page() == page.page_index(),
        ClutchError::Replay,
    )?;
    let owner = work.semantic.authority().owner;
    let feed = page.feed();
    let base = page.market().base().base();
    let mut reservation_at = ACTION24_ASSESSMENT_FIXED_ACCOUNTS_V1;
    let mut order_index = 0u8;
    while order_index < feed.order_count {
        let location = frozen_order_location_from_sealed_accounts_v1(sealed, order_index)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        if location.page_index() == u16::from(page.page_index()) {
            let row = page.order_at_physical_slot(location.page_slot())?;
            require(
                row.page_index() == location.page_index()
                    && row.page_slot() == location.page_slot(),
                ClutchError::MismatchedState,
            )?;
            let fill = page.selected_fill(order_index)?;
            if row.membership().owner() == owner && fill != 0 {
                let reservation_account = accounts
                    .get(reservation_at)
                    .ok_or(Refusal::Adapter(ClutchError::WrongAccountCount))?;
                reservation_at = reservation_at
                    .checked_add(1)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
                require_readonly_program_account(
                    program_id,
                    reservation_account,
                    Some(RESERVATION_ACCOUNT_BYTES_V9),
                )?;
                let reservation = ReservationAccountV9::decode(&borrow_data(reservation_account)?)?;
                let rent = reservation.rent();
                require(
                    reservation_account.lamports()
                        >= rent.refundable_principal.checked_add(rent.donation_floor)
                            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
                    ClutchError::MismatchedState,
                )?;
                let envelope = derive_owner_fee_envelope_from_page_v2(
                    feed,
                    page.price_grid_id(),
                    base.series_funding_terms_v2_id,
                    base.settlement_policy_id,
                    owner,
                    order_index,
                    fill,
                    row,
                    reservation,
                )
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
                let expected_pda = seeds::general_v2_reservation_v9_pda(
                    program_id,
                    &envelope.intent.0,
                );
                require(
                    *reservation_account.key == expected_pda.0
                        && reservation.body().stored_bump == expected_pda.1,
                    ClutchError::WrongPda,
                )?;
                work.semantic.record_page_envelope(
                    page.page_index(),
                    OwnerFeeAssessmentEnvelopeV1::new(owner, order_index, envelope)?,
                )?;
            }
        }
        order_index = order_index
            .checked_add(1)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }
    require(reservation_at == accounts.len(), ClutchError::WrongAccountCount)?;
    work.semantic.finish_page(page.page_index())?;
    Ok(())
}

fn route_is_assessment_continuation(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> bool {
    if accounts.len() < ACTION24_ASSESSMENT_FIXED_ACCOUNTS_V1
        || accounts.len() > ACTION24_ASSESSMENT_MAX_ACCOUNTS_V1
        || accounts[IX_ROOT].owner != program_id
        || accounts[IX_ROOT].data_len() != INDEXED_SETTLEMENT_ROOT_BYTES_V1
    {
        return false;
    }
    let Ok(data) = accounts[IX_ROOT].try_borrow_data() else { return false };
    let Ok(root) = IndexedSettlementRootV1AccountV1::decode(&data) else { return false };
    root.locator_account().bytes() == accounts[IX_LOCATOR].key.to_bytes()
}

/// Attempt the strict assessment-continuation account contract before the
/// ordinary full-traversal action-24 frame is parsed.
#[inline(never)]
pub(crate) fn try_process_action24_owner_fee_assessment_v6(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    expected_epoch: Id32,
    expected_root: Id32,
) -> Outcome<Action24AssessmentDispatchV1> {
    if !route_is_assessment_continuation(program_id, accounts) {
        return Ok(Action24AssessmentDispatchV1::NotAssessmentContinuation);
    }
    require_pairwise_distinct(accounts)?;
    let page = authenticate_settlement_page_continuation_v5(
        program_id,
        SettlementPageContinuationFrameV5 {
            retained_feed: &accounts[IX_FEED], market_binding: &accounts[IX_BINDING],
            market_runtime: &accounts[IX_RUNTIME], economic_domain: &accounts[IX_DOMAIN],
            price_grid: &accounts[IX_GRID], realm: &accounts[IX_REALM],
            profile: &accounts[IX_PROFILE], collateral_policy: &accounts[IX_POLICY],
            token_program: &accounts[IX_TOKEN], market_instance: &accounts[IX_MARKET_INSTANCE],
            market_genesis: &accounts[IX_GENESIS], page: &accounts[IX_PAGE],
        },
    )?;
    let root = authenticate_readonly_general_settlement_root_v1(
        program_id,
        core::slice::from_ref(&accounts[IX_ROOT]),
        page.feed().epoch,
        page.feed().settlement_candidate_id,
    )?;
    require(
        root.account() == expected_root
            && root.root().epoch() == expected_epoch
            && root.root().phase() == SettlementRootPhaseV1::Materializing
            && root.root().fee_record_state() == SettlementRootChildStateV1::Live
            && root.root().retained_feed() == page.feed_account()
            && root.root().order_set() == page.feed().order_set,
        ClutchError::MismatchedState,
    )?;
    let index_data = borrow_index_accounts_v1(accounts)?;
    let sealed = authenticate_index_pair(
        program_id,
        accounts,
        &index_data,
        &page,
        root.root().stored_bump(),
    )?;
    require(
        accounts[IX_OWNER_ROW].is_writable
            && !accounts[IX_OWNER_ROW].is_signer
            && !accounts[IX_OWNER_ROW].executable,
        ClutchError::MismatchedState,
    )?;
    let work_fresh = accounts[IX_WORK].owner == &SYSTEM_PROGRAM_ID
        && accounts[IX_WORK].data_len() == 0;
    let mut work = boxed_work_scratch_v6()?;
    let owner = if work_fresh {
        discover_current_owner(
            program_id,
            &page,
            &sealed,
            root.root(),
            &accounts[IX_OWNER_ROW],
        )?
    } else {
        require(
            accounts[IX_WORK].owner == program_id
                && accounts[IX_WORK].is_writable
                && !accounts[IX_WORK].is_signer
                && !accounts[IX_WORK].executable
                && accounts[IX_WORK].data_len() == OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_BYTES_V1,
            ClutchError::MismatchedState,
        )?;
        OwnerFeeAssessmentWorkV1AccountV1::decode_into_and_data_id(
            &borrow_data(&accounts[IX_WORK])?,
            &mut work,
            &RuntimeSha256,
            id(accounts[IX_WORK].key),
        )?;
        work.semantic.authority().owner
    };
    let owner_row_pda = seeds::find(
        program_id,
        &[
            contract::OWNER_SETTLEMENT_SEED_DOMAIN_V5,
            &root.root().epoch().bytes(),
            &root.root().settlement_candidate_id().bytes(),
            &owner.bytes(),
        ],
    );
    expect_pda(accounts[IX_OWNER_ROW].key, owner_row_pda, None)?;
    let selected = authenticate_owner_fee_common_core_v5(
        program_id,
        root.account(),
        root.root(),
        page.realm(),
        id(accounts[IX_OWNER_ROW].key),
        page.market().authority(),
        OwnerFeeCommonAccountFrameV5 {
            owner_row: &accounts[IX_OWNER_ROW], selected_fee_record: &accounts[IX_SELECTED_FEE],
            batch_policy: &accounts[IX_BATCH_POLICY],
            revenue_policy_record: &accounts[IX_REVENUE_RECORD], realm: &accounts[IX_REALM],
            revenue_policy_preimage: &accounts[IX_REVENUE_PREIMAGE],
        },
    )?;
    let selected_data_id = selected_fee_data_id(&accounts[IX_SELECTED_FEE], &selected)?;
    let work_pda = seeds::general_v2_owner_fee_assessment_work_pda(
        program_id,
        &accounts[IX_SELECTED_FEE].key.to_bytes(),
        &owner.bytes(),
    );
    expect_pda(accounts[IX_WORK].key, work_pda, None)?;
    require_system_program(&accounts[IX_SYSTEM])?;
    let rent = read_rent(&accounts[IX_RENT])?;
    if work_fresh {
        require_creatable(&accounts[IX_WORK])?;
        require_signer(&accounts[IX_WORK_RENT_PAYER])?;
        require(accounts[IX_WORK_RENT_PAYER].is_writable, ClutchError::NotWritable)?;
        let rent_owner = work_rent_from_account(
            &accounts[IX_WORK],
            &accounts[IX_WORK_RENT_PAYER],
            &rent,
        )?;
        let current = page.market().authority();
        contract::OwnerFeeAssessmentWorkV1::begin_into(
            &mut work.semantic,
            OwnerFeeAssessmentAuthorityV1 {
                settlement_root_account: root.account(),
                selected_fee_record_account: id(accounts[IX_SELECTED_FEE].key),
                selected_fee_record_data_id: selected_data_id,
                realm: page.realm(),
                revenue_policy_record_account: current.revenue_policy_record_account(),
                revenue_policy_record_v2_id: current.revenue_policy_record_v2_id(),
                revenue_policy_v2_digest: current.revenue_policy_v2_digest(),
                retained_feed_account: page.feed_account(),
                retained_feed_data_id: page.feed_data_id(),
                owner,
                owner_row_account: id(accounts[IX_OWNER_ROW].key),
                market: page.feed().market,
                epoch: page.feed().epoch,
                order_set: page.feed().order_set,
                owner_order_set_digest: root.root().owner_order_set_digest(),
            },
            page.page_count(),
            page.feed().order_count,
            page.market().base().base().neutral_sink,
        )?;
        work.rent = rent_owner;
        work.stored_bump = work_pda.1;
    } else {
        let authority = work.semantic.authority();
        let current = page.market().authority();
        require(
            authority.settlement_root_account == root.account()
                && authority.selected_fee_record_account == id(accounts[IX_SELECTED_FEE].key)
                && authority.selected_fee_record_data_id == selected_data_id
                && authority.realm == page.realm()
                && authority.revenue_policy_record_account
                    == current.revenue_policy_record_account()
                && authority.revenue_policy_record_v2_id
                    == current.revenue_policy_record_v2_id()
                && authority.revenue_policy_v2_digest
                    == current.revenue_policy_v2_digest()
                && authority.retained_feed_account == page.feed_account()
                && authority.retained_feed_data_id == page.feed_data_id()
                && authority.owner == owner
                && authority.owner_row_account == id(accounts[IX_OWNER_ROW].key)
                && authority.market == page.feed().market
                && authority.epoch == page.feed().epoch
                && authority.order_set == page.feed().order_set
                && authority.owner_order_set_digest == root.root().owner_order_set_digest()
                && work.semantic.page_count() == page.page_count()
                && work.semantic.order_count() == page.feed().order_count
                && work.rent.payer == id(accounts[IX_WORK_RENT_PAYER].key)
                && work.semantic.neutral_sink() == id(accounts[IX_NEUTRAL_SINK].key)
                && work.stored_bump == work_pda.1
                && accounts[IX_WORK].lamports()
                    >= work.rent.refundable_principal.checked_add(work.rent.donation_floor)
                        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
            ClutchError::MismatchedState,
        )?;
    }
    require(
        id(accounts[IX_NEUTRAL_SINK].key) == work.semantic.neutral_sink(),
        ClutchError::MismatchedState,
    )?;
    let process_page = if work_fresh {
        page.page_index() == 0
    } else {
        require(
            work.semantic.next_page() < work.semantic.page_count()
                && page.page_index() == work.semantic.next_page(),
            ClutchError::Replay,
        )?;
        true
    };
    apply_work_page(program_id, accounts, &page, &sealed, &mut work, process_page)?;
    if work_fresh {
        let bump = [work.stored_bump];
        let fee_record = accounts[IX_SELECTED_FEE].key.to_bytes();
        let owner_bytes = owner.bytes();
        let seeds: [&[u8]; 4] = [
            contract::OWNER_FEE_ASSESSMENT_WORK_SEED_DOMAIN_V1,
            &fee_record,
            &owner_bytes,
            &bump,
        ];
        create_from_payer(
            program_id,
            &accounts[IX_WORK_RENT_PAYER],
            &accounts[IX_WORK],
            &accounts[IX_SYSTEM],
            &rent,
            OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_BYTES_V1,
            work.rent,
            &seeds,
        )?;
    }
    encode_account(&accounts[IX_WORK], |output| work.encode(output))?;
    let mut post = boxed_work_scratch_v6()?;
    let post_data_id = OwnerFeeAssessmentWorkV1AccountV1::decode_into_and_data_id(
        &borrow_data(&accounts[IX_WORK])?,
        &mut post,
        &RuntimeSha256,
        id(accounts[IX_WORK].key),
    )?;
    require(
        *post == *work && !post_data_id.is_zero(),
        ClutchError::MismatchedState,
    )?;
    Ok(Action24AssessmentDispatchV1::Applied)
}

const _: () = assert!(ACTION24_ASSESSMENT_MAX_ACCOUNTS_V1 <= 63);
