//! Staged General action 24: materialize one rent-owned V5 settlement slice.
//!
//! The 64-byte selector names only Epoch and counted SettlementRoot.  Every
//! page, owner, order, receipt, Reservation, Position, fee, and rent fact is
//! rederived from the program-owned root, retained Feed, and complete V5 page
//! traversal.  The generic route deliberately refuses Portfolio orders; the
//! exhaustive all-sibling Portfolio producer has a separate bounded composer.

use core::cell::Ref;

use clutch_batch::portfolio_execution_v2::{
    AuthenticatedPortfolioReceiptSiblingSetV2, PORTFOLIO_PAIR_MAX_RECEIPTS_V2,
};
use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    CurrentMarketAuthorityV4, FreezeEntitlementPayloadV1, Id32, OwnerSettlementSeedTupleV5,
    SettlementRootChildStateV1,
};
use clutch_general_v2_runtime::{
    derive_candidate_entitlement_projection_v5, prepare_materialize_entitlement_slice_v5,
    prepare_materialize_portfolio_pair_v5, CandidateEntitlementProjectionV5,
    EntitlementEndpointInputV5, MaterializationReservationInputV9,
    MaterializeEntitlementSliceInputV5, MaterializeEntitlementSlicePlanV5,
    MaterializePortfolioPairInputV5, MaterializePortfolioPairPlanV5,
    OwnerRowMaterializationDispositionV5, OwnerRowMaterializationInputV5,
    PortfolioPairReceiptCreateInputV5, PositionAccountInputV3, RentOwnedSettlementCreateFundingV5,
    SettlementLegV1, SettlementRouteV1,
};
use clutch_owner_settlement::OrderKindV1;
use clutch_retirement::{PositionPurposeV3, POSITION_V3_BYTES};
use clutch_solana_layout::registry::GeneralV2Action;
use clutch_solana_layout::reservation_v9::{ReservationAccountV9, RESERVATION_ACCOUNT_BYTES_V9};
use clutch_solana_layout::MAX_ORDER_PAGES;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, require_signer, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    read_rent, require_creatable, require_system_program, RentParameters, SYSTEM_PROGRAM_ID,
};
use crate::seeds;

use super::general_v2_fee_v5::{
    prepare_owner_fee_action24_v5, OwnerFeeAction24InputV5,
    OwnerFeeCreationAccountFrameV5, OwnerFeeSnapshotAccountFrameV5,
    PreparedOwnerFeeAction24V5,
};
use super::general_v2_settlement_producer_v5::{create_from_payer, encode_account, rent_owner};
use super::general_v2_owner_fee_assessment_v6::{
    try_process_action24_owner_fee_assessment_v6, Action24AssessmentDispatchV1,
};
use super::general_v2_settlement_root::AuthenticatedGeneralSettlementRootV1;
use super::general_v2_settlement_traversal_v5::{
    authenticate_portfolio_materialization_sibling_set_v5, authenticate_settlement_traversal_v5,
    authenticate_writable_root_settlement_traversal_v5, AuthenticatedSettlementTraversalV5,
    SettlementTraversalAccountFrameV5,
};

/// Shared immutable traversal roles before the final PageV5 suffix.
pub const ACTION24_TRAVERSAL_PREFIX_ACCOUNTS: usize = 12;
/// Receipt, common rent payer, System program, and Rent sysvar.
pub const ACTION24_CREATION_HEADER_ACCOUNTS: usize = 4;
/// Owner row, ReservationV9, and PositionV3 for one real endpoint.
pub const ACTION24_ENDPOINT_ACCOUNTS: usize = 3;
/// Candidate-wide fee roles: selected, batch, Revenue record/preimage, sink.
pub const ACTION24_FEE_COMMON_ACCOUNTS: usize = 5;
/// Fixed fresh-owner roles: carry, payer snapshot, completed assessment work.
pub const ACTION24_FEE_OWNER_ACCOUNTS: usize = 3;
/// Receipt targets after the first fixed receipt role for a Portfolio pair.
pub const ACTION24_PORTFOLIO_EXTRA_RECEIPTS_MAX: usize = PORTFOLIO_PAIR_MAX_RECEIPTS_V2 - 1;
/// Deployed instruction ceiling leaves one of Solana's 64 keys for the program.
pub const ACTION24_MAX_ACCOUNT_INFOS_V5: usize = 63;

const IX_ROOT: usize = 0;
const IX_FEED: usize = 1;
const IX_BINDING: usize = 2;
const IX_RUNTIME: usize = 3;
const IX_DOMAIN: usize = 4;
const IX_GRID: usize = 5;
const IX_REALM: usize = 6;
const IX_PROFILE: usize = 7;
const IX_POLICY: usize = 8;
const IX_TOKEN: usize = 9;
const IX_MARKET_INSTANCE: usize = 10;
const IX_GENESIS: usize = 11;
const IX_RECEIPT: usize = 12;
const IX_RENT_PAYER: usize = 13;
const IX_SYSTEM: usize = 14;
const IX_RENT: usize = 15;
const IX_FIRST_ENDPOINT: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeSha256;

impl contract::Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

fn id(key: &Pubkey) -> Id32 {
    Id32::from_bytes(key.to_bytes())
}

fn borrow_data<'a, 'info>(account: &'a AccountInfo<'info>) -> Outcome<Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(Ref::map(data, |bytes| &**bytes))
}

fn require_reservation_rent_balance(
    account: &AccountInfo<'_>,
    reservation: ReservationAccountV9,
) -> Outcome<()> {
    let rent = reservation.rent();
    let accounted = rent
        .refundable_principal
        .checked_add(rent.donation_floor)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(account.lamports() >= accounted, ClutchError::MismatchedState)
}

fn require_all_distinct(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        if left != IX_RENT_PAYER {
            require(!accounts[left].is_signer, ClutchError::MismatchedState)?;
        }
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

fn endpoint_order_indices(
    entitlement: &CandidateEntitlementProjectionV5<'_>,
) -> Outcome<([u8; 2], usize)> {
    let slice = entitlement.current_slice();
    match (slice.buy(), slice.sell(), slice.route()) {
        (SettlementLegV1::Order(buy), SettlementLegV1::Order(sell), SettlementRouteV1::Direct) => {
            Ok(([buy, sell], 2))
        }
        (SettlementLegV1::Order(buy), SettlementLegV1::Split, SettlementRouteV1::SplitToBuy) => {
            Ok(([buy, 0], 1))
        }
        (SettlementLegV1::Merge, SettlementLegV1::Order(sell), SettlementRouteV1::SellToMerge) => {
            Ok(([sell, 0], 1))
        }
        _ => Err(Refusal::Adapter(ClutchError::MismatchedState)),
    }
}

fn current_slice_is_portfolio_pair(
    root_traversal: &super::general_v2_settlement_traversal_v5::AuthenticatedRootSettlementTraversalV5<'_, '_>,
) -> Outcome<bool> {
    let entitlement = derive_candidate_entitlement_projection_v5(
        root_traversal.root().account(),
        root_traversal.root().root(),
        root_traversal.traversal().traversal(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let (order_indices, endpoint_count) = endpoint_order_indices(&entitlement)?;
    let first = entitlement
        .settlement_membership(order_indices[0])
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    if endpoint_count == 1 {
        require(
            first.order_kind == OrderKindV1::Single,
            ClutchError::MismatchedState,
        )?;
        return Ok(false);
    }
    let second = entitlement
        .settlement_membership(order_indices[1])
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    match (first.order_kind, second.order_kind) {
        (
            OrderKindV1::Single,
            OrderKindV1::Single,
        ) => Ok(false),
        (
            OrderKindV1::Portfolio,
            OrderKindV1::Portfolio,
        ) => Ok(true),
        _ => Err(Refusal::Adapter(ClutchError::MismatchedState)),
    }
}

fn endpoint_base(ordinal: usize) -> Outcome<usize> {
    IX_FIRST_ENDPOINT
        .checked_add(
            ordinal
                .checked_mul(ACTION24_ENDPOINT_ACCOUNTS)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        )
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))
}

#[cfg(test)]
fn non_page_account_count(
    endpoint_count: usize,
    fresh_owner_count: usize,
    fee_present: bool,
) -> Outcome<usize> {
    require(
        (1..=2).contains(&endpoint_count),
        ClutchError::WrongAccountCount,
    )?;
    require(
        fresh_owner_count <= endpoint_count,
        ClutchError::WrongAccountCount,
    )?;
    let endpoint_accounts = endpoint_count
        .checked_mul(ACTION24_ENDPOINT_ACCOUNTS)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let fee_accounts = if fee_present {
        ACTION24_FEE_COMMON_ACCOUNTS
            .checked_add(
                fresh_owner_count
                    .checked_mul(ACTION24_FEE_OWNER_ACCOUNTS)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
            )
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
    } else {
        0
    };
    ACTION24_TRAVERSAL_PREFIX_ACCOUNTS
        .checked_add(ACTION24_CREATION_HEADER_ACCOUNTS)
        .and_then(|value| value.checked_add(endpoint_accounts))
        .and_then(|value| value.checked_add(fee_accounts))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))
}

#[cfg(test)]
fn portfolio_non_page_account_count(
    receipt_count: u8,
    fee_present: bool,
) -> Outcome<usize> {
    let receipt_count = usize::from(receipt_count);
    require(
        (1..=PORTFOLIO_PAIR_MAX_RECEIPTS_V2).contains(&receipt_count),
        ClutchError::WrongAccountCount,
    )?;
    let fee_accounts = if fee_present {
        ACTION24_FEE_COMMON_ACCOUNTS
            .checked_add(
                2usize
                    .checked_mul(ACTION24_FEE_OWNER_ACCOUNTS)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
            )
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
    } else {
        0
    };
    ACTION24_TRAVERSAL_PREFIX_ACCOUNTS
        .checked_add(ACTION24_CREATION_HEADER_ACCOUNTS)
        .and_then(|value| value.checked_add(2 * ACTION24_ENDPOINT_ACCOUNTS))
        .and_then(|value| value.checked_add(fee_accounts))
        .and_then(|value| value.checked_add(receipt_count - 1))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))
}

fn portfolio_receipt_account_index(receipt_index: usize, fee_end: usize) -> Outcome<usize> {
    require(
        receipt_index <= ACTION24_PORTFOLIO_EXTRA_RECEIPTS_MAX,
        ClutchError::WrongAccountCount,
    )?;
    if receipt_index == 0 {
        Ok(IX_RECEIPT)
    } else {
        fee_end
            .checked_add(receipt_index - 1)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))
    }
}

fn is_fresh_target(account: &AccountInfo<'_>) -> bool {
    account.owner == &SYSTEM_PROGRAM_ID && account.data_len() == 0
}

fn require_endpoint_accounts(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    ordinal: usize,
) -> Outcome<bool> {
    let at = endpoint_base(ordinal)?;
    let row = &accounts[at];
    let reservation = &accounts[at + 1];
    let position = &accounts[at + 2];
    require(
        row.is_writable && !row.is_signer && !row.executable,
        ClutchError::NotWritable,
    )?;
    let fresh = is_fresh_target(row);
    if fresh {
        require_creatable(row)?;
    } else {
        require(row.owner == program_id, ClutchError::WrongProgramOwner)?;
        require(
            row.data_len() == contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5,
            ClutchError::WrongDataLength,
        )?;
    }
    require(
        reservation.owner == program_id
            && reservation.is_writable
            && !reservation.is_signer
            && !reservation.executable
            && reservation.data_len() == RESERVATION_ACCOUNT_BYTES_V9,
        ClutchError::MismatchedState,
    )?;
    require(
        position.owner == program_id
            && !position.is_writable
            && !position.is_signer
            && !position.executable
            && position.data_len() == POSITION_V3_BYTES,
        ClutchError::MismatchedState,
    )?;
    Ok(fresh)
}

fn creation_funding(
    program_id: &Pubkey,
    payer: &AccountInfo<'_>,
    target: &AccountInfo<'_>,
    rent: &RentParameters,
    space: usize,
) -> Outcome<RentOwnedSettlementCreateFundingV5> {
    let data_len =
        u32::try_from(target.data_len()).map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(RentOwnedSettlementCreateFundingV5 {
        program_id: id(program_id),
        payer: id(payer.key),
        system_program_id: Id32::from_bytes(SYSTEM_PROGRAM_ID.to_bytes()),
        payer_lamports: payer.lamports(),
        target_lamports_before: target.lamports(),
        target_owner_before: id(target.owner),
        target_data_len_before: data_len,
        target_writable: target.is_writable,
        target_executable: target.executable,
        rent_minimum: rent.minimum_balance(space)?,
    })
}

fn select_traversal<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
) -> Outcome<(AuthenticatedSettlementTraversalV5<'info>, usize)> {
    let mut selected = None;
    let mut selected_pages = 0usize;
    let mut page_count = 1usize;
    while page_count <= MAX_ORDER_PAGES {
        if accounts.len() >= ACTION24_TRAVERSAL_PREFIX_ACCOUNTS + page_count {
            let first_page = accounts.len() - page_count;
            let attempt = authenticate_settlement_traversal_v5(
                program_id,
                SettlementTraversalAccountFrameV5 {
                    retained_feed: &accounts[IX_FEED],
                    market_binding: &accounts[IX_BINDING],
                    market_runtime: &accounts[IX_RUNTIME],
                    economic_domain: &accounts[IX_DOMAIN],
                    price_grid: &accounts[IX_GRID],
                    realm: &accounts[IX_REALM],
                    profile: &accounts[IX_PROFILE],
                    collateral_policy: &accounts[IX_POLICY],
                    token_program: &accounts[IX_TOKEN],
                    market_instance: &accounts[IX_MARKET_INSTANCE],
                    market_genesis: &accounts[IX_GENESIS],
                    pages: &accounts[first_page..],
                },
            );
            if let Ok(value) = attempt {
                require(selected.is_none(), ClutchError::MismatchedState)?;
                selected = Some(value);
                selected_pages = page_count;
            }
        }
        page_count += 1;
    }
    selected
        .map(|value| (value, selected_pages))
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))
}

fn prepare_owner_fee_evidence(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    root: &super::general_v2_settlement_root::AuthenticatedGeneralSettlementRootV1,
    current_market_authority: CurrentMarketAuthorityV4,
    entitlement: &CandidateEntitlementProjectionV5<'_>,
    order_indices: [u8; 2],
    endpoint_count: usize,
    fresh: [bool; 2],
    fee_at: usize,
    rent: &RentParameters,
) -> Outcome<([Option<PreparedOwnerFeeAction24V5>; 2], usize)> {
    let fee_present = root.root().fee_record_state() == SettlementRootChildStateV1::Live;
    let mut out = [None, None];
    if !fee_present {
        require(
            root.root().fee_record().is_zero(),
            ClutchError::MismatchedState,
        )?;
        let mut ordinal = 0usize;
        while ordinal < endpoint_count {
            if fresh[ordinal] {
                let membership = entitlement
                    .settlement_membership(order_indices[ordinal])
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
                out[ordinal] = Some(prepare_owner_fee_action24_v5(
                    program_id,
                    root,
                    current_market_authority,
                    entitlement,
                    Id32::new(membership.owner)?,
                    &accounts[endpoint_base(ordinal)?],
                    OwnerFeeAction24InputV5::NoFeeRecord,
                )?);
            }
            ordinal += 1;
        }
        return Ok((out, fee_at));
    }
    require(
        !root.root().fee_record().is_zero(),
        ClutchError::MismatchedState,
    )?;
    let common_end = fee_at
        .checked_add(ACTION24_FEE_COMMON_ACCOUNTS)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(common_end <= accounts.len(), ClutchError::WrongAccountCount)?;
    let mut owner_fee_at = common_end;
    let mut ordinal = 0usize;
    while ordinal < endpoint_count {
        if fresh[ordinal] {
            let membership = entitlement
                .settlement_membership(order_indices[ordinal])
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
            let owner = Id32::new(membership.owner)?;
            let owner_fee_end = owner_fee_at
                .checked_add(ACTION24_FEE_OWNER_ACCOUNTS)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
            require(owner_fee_end <= accounts.len(), ClutchError::WrongAccountCount)?;
            let carry_rent = rent_owner(
                &accounts[IX_RENT_PAYER],
                &accounts[owner_fee_at],
                rent,
                contract::OWNER_FEE_CARRY_ACCOUNT_BYTES_V3,
            )?;
            let payer_rent = rent_owner(
                &accounts[IX_RENT_PAYER],
                &accounts[owner_fee_at + 1],
                rent,
                contract::PAYER_ALLOCATION_ACCOUNT_BYTES_V2,
            )?;
            out[ordinal] = Some(prepare_owner_fee_action24_v5(
                program_id,
                root,
                current_market_authority,
                entitlement,
                owner,
                &accounts[endpoint_base(ordinal)?],
                OwnerFeeAction24InputV5::CandidateFee {
                    frame: OwnerFeeCreationAccountFrameV5 {
                        snapshot: OwnerFeeSnapshotAccountFrameV5 {
                            owner_row: &accounts[endpoint_base(ordinal)?],
                            selected_fee_record: &accounts[fee_at],
                            owner_fee_carry: &accounts[owner_fee_at],
                            payer_allocation: &accounts[owner_fee_at + 1],
                            batch_policy: &accounts[fee_at + 1],
                            revenue_policy_record: &accounts[fee_at + 2],
                            realm: &accounts[IX_REALM],
                            revenue_policy_preimage: &accounts[fee_at + 3],
                        },
                        assessment_work: &accounts[owner_fee_at + 2],
                        work_rent_payer: &accounts[IX_RENT_PAYER],
                        neutral_sink: &accounts[fee_at + 4],
                        carry_rent,
                        payer_rent,
                    },
                },
            )?);
            owner_fee_at = owner_fee_end;
        }
        ordinal += 1;
    }
    Ok((out, owner_fee_at))
}

fn make_endpoint_input<'a>(
    program_id: &Pubkey,
    root: &super::general_v2_settlement_root::AuthenticatedGeneralSettlementRootV1,
    accounts: &'a [AccountInfo<'_>],
    ordinal: usize,
    order_index: u8,
    fresh: bool,
    fee: Option<&PreparedOwnerFeeAction24V5>,
    payer: &AccountInfo<'_>,
    rent: &RentParameters,
    row_body: Option<&'a [u8]>,
    reservation_body: &'a [u8],
    position_body: &'a [u8],
) -> Outcome<EntitlementEndpointInputV5<'a>> {
    let at = endpoint_base(ordinal)?;
    let owner_row = if fresh {
        let fee = fee.ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        OwnerRowMaterializationInputV5::Create {
            account: id(accounts[at].key),
            bump: fee.owner_row_bump(),
            funding: creation_funding(
                program_id,
                payer,
                &accounts[at],
                rent,
                contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5,
            )?,
            fee_evidence: fee.owner_row_fee_evidence(),
        }
    } else {
        let membership_owner = fee;
        require(membership_owner.is_none(), ClutchError::MismatchedState)?;
        let row = contract::OwnerSettlementV5AccountV1::decode(
            row_body.ok_or(Refusal::Adapter(ClutchError::MismatchedState))?,
        )?;
        let expectation = row.semantic.expectation();
        let seed = OwnerSettlementSeedTupleV5::new(
            root.root().epoch(),
            root.root().settlement_candidate_id(),
            Id32::new(expectation.owner())?,
        )?;
        let pda = seeds::general_v2_owner_settlement_v5_pda(
            program_id,
            seed.epoch(),
            seed.settlement_candidate(),
            seed.owner(),
        );
        expect_pda(accounts[at].key, pda, Some(row.stored_bump))?;
        OwnerRowMaterializationInputV5::Existing {
            view: clutch_general_v2_runtime::OwnerSettlementAccountViewV5 {
                account: id(accounts[at].key),
                program_owner: id(program_id),
                exact_body: row_body.ok_or(Refusal::Adapter(ClutchError::MismatchedState))?,
                lamports: accounts[at].lamports(),
                rent_minimum: rent.minimum_balance(contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5)?,
                canonical_bump: pda.1,
                writable: true,
            },
        }
    };
    Ok(EntitlementEndpointInputV5 {
        order_index,
        reservation: MaterializationReservationInputV9 {
            account: id(accounts[at + 1].key),
            encoded_body: reservation_body,
        },
        position: PositionAccountInputV3 {
            account: id(accounts[at + 2].key),
            encoded_body: position_body,
        },
        owner_row,
    })
}

#[inline(never)]
fn prepare_generic_plan(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    root_traversal: &super::general_v2_settlement_traversal_v5::AuthenticatedRootSettlementTraversalV5<'_, '_>,
    page_count: usize,
    rent: &RentParameters,
) -> Outcome<(
    Box<MaterializeEntitlementSlicePlanV5>,
    [Option<PreparedOwnerFeeAction24V5>; 2],
)> {
    let root = root_traversal.root();
    let entitlement = Box::new(derive_candidate_entitlement_projection_v5(
        root.account(),
        root.root(),
        root_traversal.traversal().traversal(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?);
    let (order_indices, endpoint_count) = endpoint_order_indices(&entitlement)?;
    let mut fresh = [false; 2];
    let mut ordinal = 0usize;
    while ordinal < endpoint_count {
        fresh[ordinal] = require_endpoint_accounts(program_id, accounts, ordinal)?;
        ordinal += 1;
    }
    let fee_at = IX_FIRST_ENDPOINT
        .checked_add(
            endpoint_count
                .checked_mul(ACTION24_ENDPOINT_ACCOUNTS)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        )
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let (fee, pages_at) = prepare_owner_fee_evidence(
        program_id,
        accounts,
        root,
        root_traversal.traversal().market().authority(),
        &entitlement,
        order_indices,
        endpoint_count,
        fresh,
        fee_at,
        rent,
    )?;
    require(
        pages_at
            .checked_add(page_count)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
            == accounts.len(),
        ClutchError::WrongAccountCount,
    )?;

    let row0 = if fresh[0] {
        None
    } else {
        Some(borrow_data(&accounts[endpoint_base(0)?])?)
    };
    let reservation0 = borrow_data(&accounts[endpoint_base(0)? + 1])?;
    let position0 = borrow_data(&accounts[endpoint_base(0)? + 2])?;
    let row1 = if endpoint_count == 2 && !fresh[1] {
        Some(borrow_data(&accounts[endpoint_base(1)?])?)
    } else {
        None
    };
    let reservation1 = if endpoint_count == 2 {
        Some(borrow_data(&accounts[endpoint_base(1)? + 1])?)
    } else {
        None
    };
    let position1 = if endpoint_count == 2 {
        Some(borrow_data(&accounts[endpoint_base(1)? + 2])?)
    } else {
        None
    };
    let endpoint0 = make_endpoint_input(
        program_id,
        root,
        accounts,
        0,
        order_indices[0],
        fresh[0],
        fee[0].as_ref(),
        &accounts[IX_RENT_PAYER],
        rent,
        row0.as_deref(),
        &reservation0,
        &position0,
    )?;
    let endpoint1 = if endpoint_count == 2 {
        Some(make_endpoint_input(
            program_id,
            root,
            accounts,
            1,
            order_indices[1],
            fresh[1],
            fee[1].as_ref(),
            &accounts[IX_RENT_PAYER],
            rent,
            row1.as_deref(),
            reservation1
                .as_deref()
                .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?,
            position1
                .as_deref()
                .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?,
        )?)
    } else {
        None
    };

    let cursor = root.root().counts().admitted_receipts;
    let receipt_pda = seeds::general_v2_receipt_v5_pda(
        program_id,
        &root.root().epoch().bytes(),
        &root.root().settlement_candidate_id().bytes(),
        cursor,
    );
    expect_pda(accounts[IX_RECEIPT].key, receipt_pda, None)?;
    require_creatable(&accounts[IX_RECEIPT])?;
    let plan = prepare_materialize_entitlement_slice_v5(MaterializeEntitlementSliceInputV5 {
        entitlement: &entitlement,
        receipt_account: id(accounts[IX_RECEIPT].key),
        receipt_bump: receipt_pda.1,
        receipt_funding: creation_funding(
            program_id,
            &accounts[IX_RENT_PAYER],
            &accounts[IX_RECEIPT],
            rent,
            contract::SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5,
        )?,
        endpoints: [Some(endpoint0), endpoint1],
    })
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok((Box::new(plan), fee))
}

#[inline(never)]
fn prepare_portfolio_plan(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    root_traversal: &super::general_v2_settlement_traversal_v5::AuthenticatedRootSettlementTraversalV5<'_, '_>,
    page_count: usize,
    rent: &RentParameters,
    sibling_set: AuthenticatedPortfolioReceiptSiblingSetV2,
) -> Outcome<(
    Box<MaterializePortfolioPairPlanV5>,
    usize,
    [Option<PreparedOwnerFeeAction24V5>; 2],
)> {
    let root = root_traversal.root();
    let entitlement = Box::new(derive_candidate_entitlement_projection_v5(
        root.account(),
        root.root(),
        root_traversal.traversal().traversal(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?);
    let pair = sibling_set.pair();
    let order_indices = [
        pair.buyer().record().order_index,
        pair.seller().record().order_index,
    ];
    let fresh = [
        require_endpoint_accounts(program_id, accounts, 0)?,
        require_endpoint_accounts(program_id, accounts, 1)?,
    ];
    require(fresh == [true, true], ClutchError::MismatchedState)?;
    let fee_at = IX_FIRST_ENDPOINT
        .checked_add(
            2usize
                .checked_mul(ACTION24_ENDPOINT_ACCOUNTS)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        )
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let (fee, fee_end) = prepare_owner_fee_evidence(
        program_id,
        accounts,
        root,
        root_traversal.traversal().market().authority(),
        &entitlement,
        order_indices,
        2,
        fresh,
        fee_at,
        rent,
    )?;
    let receipt_count = sibling_set.sibling_count();
    let extra_receipts = usize::from(receipt_count)
        .checked_sub(1)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        fee_end
            .checked_add(extra_receipts)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
            .checked_add(page_count)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
            == accounts.len(),
        ClutchError::WrongAccountCount,
    )?;

    let reservation0 = borrow_data(&accounts[endpoint_base(0)? + 1])?;
    let position0 = borrow_data(&accounts[endpoint_base(0)? + 2])?;
    let reservation1 = borrow_data(&accounts[endpoint_base(1)? + 1])?;
    let position1 = borrow_data(&accounts[endpoint_base(1)? + 2])?;
    let endpoint0 = make_endpoint_input(
        program_id,
        root,
        accounts,
        0,
        order_indices[0],
        true,
        fee[0].as_ref(),
        &accounts[IX_RENT_PAYER],
        rent,
        None,
        &reservation0,
        &position0,
    )?;
    let endpoint1 = make_endpoint_input(
        program_id,
        root,
        accounts,
        1,
        order_indices[1],
        true,
        fee[1].as_ref(),
        &accounts[IX_RENT_PAYER],
        rent,
        None,
        &reservation1,
        &position1,
    )?;

    let mut receipts = [None; PORTFOLIO_PAIR_MAX_RECEIPTS_V2];
    let mut receipt_index = 0usize;
    while receipt_index < usize::from(receipt_count) {
        let sibling = sibling_set
            .sibling(
                u8::try_from(receipt_index)
                    .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?,
            )
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        let account_index = portfolio_receipt_account_index(receipt_index, fee_end)?;
        let receipt_pda = seeds::general_v2_receipt_v5_pda(
            program_id,
            &root.root().epoch().bytes(),
            &root.root().settlement_candidate_id().bytes(),
            sibling.slice_index,
        );
        expect_pda(accounts[account_index].key, receipt_pda, None)?;
        require_creatable(&accounts[account_index])?;
        receipts[receipt_index] = Some(PortfolioPairReceiptCreateInputV5 {
            account: id(accounts[account_index].key),
            bump: receipt_pda.1,
            funding: creation_funding(
                program_id,
                &accounts[IX_RENT_PAYER],
                &accounts[account_index],
                rent,
                contract::SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5,
            )?,
        });
        receipt_index += 1;
    }
    let plan = prepare_materialize_portfolio_pair_v5(MaterializePortfolioPairInputV5 {
        entitlement: &entitlement,
        sibling_set,
        receipts,
        endpoints: [endpoint0, endpoint1],
    })
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok((Box::new(plan), fee_end, fee))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_partition_is_exact_and_presence_explicit() {
        assert_eq!(non_page_account_count(1, 0, false).unwrap(), 19);
        assert_eq!(non_page_account_count(2, 0, false).unwrap(), 22);
        assert_eq!(non_page_account_count(1, 1, true).unwrap(), 27);
        assert_eq!(non_page_account_count(2, 2, true).unwrap(), 33);
    }

    #[test]
    fn account_partition_refuses_impossible_fresh_rows() {
        assert!(non_page_account_count(0, 0, false).is_err());
        assert!(non_page_account_count(3, 0, false).is_err());
        assert!(non_page_account_count(1, 2, true).is_err());
    }

    #[test]
    fn portfolio_partition_is_capability_counted_and_bounded() {
        assert_eq!(portfolio_non_page_account_count(1, false).unwrap(), 22);
        assert_eq!(portfolio_non_page_account_count(16, false).unwrap(), 37);
        assert_eq!(portfolio_non_page_account_count(1, true).unwrap(), 33);
        assert_eq!(portfolio_non_page_account_count(16, true).unwrap(), 48);
        assert!(portfolio_non_page_account_count(0, false).is_err());
        assert!(portfolio_non_page_account_count(17, true).is_err());
    }

    #[test]
    fn completed_assessment_work_keeps_every_final_frame_under_64() {
        let generic = non_page_account_count(2, 2, true).unwrap();
        let portfolio = portfolio_non_page_account_count(16, true).unwrap();
        assert_eq!(generic + MAX_ORDER_PAGES, 37);
        assert_eq!(portfolio + MAX_ORDER_PAGES, 52);
        assert!(generic + MAX_ORDER_PAGES <= 63);
        assert!(portfolio + MAX_ORDER_PAGES <= 63);
    }

    #[test]
    fn portfolio_receipt_roles_keep_the_first_receipt_fixed() {
        assert_eq!(portfolio_receipt_account_index(0, 30).unwrap(), IX_RECEIPT);
        assert_eq!(portfolio_receipt_account_index(1, 30).unwrap(), 30);
        assert_eq!(portfolio_receipt_account_index(15, 30).unwrap(), 44);
        assert!(portfolio_receipt_account_index(16, 30).is_err());
    }
}

fn authenticate_plan_accounts(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    plan: &MaterializeEntitlementSlicePlanV5,
) -> Outcome<()> {
    let receipt = plan.receipt();
    let receipt_seed = receipt.seed();
    expect_pda(
        accounts[IX_RECEIPT].key,
        seeds::find(
            program_id,
            &[
                receipt_seed.domain(),
                receipt_seed.epoch(),
                receipt_seed.settlement_candidate(),
                receipt_seed.slice_index_le(),
            ],
        ),
        Some(receipt.bump()),
    )?;
    let mut ordinal = 0u8;
    while ordinal < plan.endpoint_count() {
        let endpoint = plan
            .endpoint(ordinal)
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        let at = endpoint_base(usize::from(ordinal))?;
        let reservation = endpoint.reservation();
        let reservation_body = ReservationAccountV9::decode(&borrow_data(&accounts[at + 1])?)?;
        require_reservation_rent_balance(&accounts[at + 1], reservation_body)?;
        expect_pda(
            accounts[at + 1].key,
            seeds::general_v2_reservation_v9_pda(program_id, &reservation.semantic_id().bytes()),
            Some(reservation_body.body().stored_bump),
        )?;
        let position = endpoint.position().position();
        let membership = endpoint.membership();
        expect_pda(
            accounts[at + 2].key,
            seeds::position_v3_pda(
                program_id,
                &plan
                    .settlement_root_poststate()
                    .market_instance_v2_id()
                    .bytes(),
                &membership.owner,
                PositionPurposeV3::General,
                &accounts[IX_RUNTIME].key.to_bytes(),
            ),
            Some(position.stored_bump()),
        )?;
        require(
            id(accounts[at].key) == endpoint.owner_row().account(),
            ClutchError::MismatchedState,
        )?;
        ordinal += 1;
    }
    Ok(())
}

fn authenticate_portfolio_plan_accounts(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    fee_end: usize,
    plan: &MaterializePortfolioPairPlanV5,
) -> Outcome<()> {
    let mut receipt_index = 0u8;
    while receipt_index < plan.receipt_count() {
        let receipt = plan
            .receipt(receipt_index)
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        let account_index = portfolio_receipt_account_index(usize::from(receipt_index), fee_end)?;
        let seed = receipt.seed();
        expect_pda(
            accounts[account_index].key,
            seeds::find(
                program_id,
                &[
                    seed.domain(),
                    seed.epoch(),
                    seed.settlement_candidate(),
                    seed.slice_index_le(),
                ],
            ),
            Some(receipt.bump()),
        )?;
        require(
            id(accounts[account_index].key) == receipt.account(),
            ClutchError::MismatchedState,
        )?;
        receipt_index = receipt_index
            .checked_add(1)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }
    let mut ordinal = 0u8;
    while ordinal < 2 {
        let endpoint = plan
            .endpoint(ordinal)
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        let at = endpoint_base(usize::from(ordinal))?;
        let reservation = endpoint.reservation();
        let reservation_body = ReservationAccountV9::decode(&borrow_data(&accounts[at + 1])?)?;
        require_reservation_rent_balance(&accounts[at + 1], reservation_body)?;
        expect_pda(
            accounts[at + 1].key,
            seeds::general_v2_reservation_v9_pda(program_id, &reservation.semantic_id().bytes()),
            Some(reservation_body.body().stored_bump),
        )?;
        let position = endpoint.position().position();
        let membership = endpoint.membership();
        expect_pda(
            accounts[at + 2].key,
            seeds::position_v3_pda(
                program_id,
                &plan
                    .settlement_root_poststate()
                    .market_instance_v2_id()
                    .bytes(),
                &membership.owner,
                PositionPurposeV3::General,
                &accounts[IX_RUNTIME].key.to_bytes(),
            ),
            Some(position.stored_bump()),
        )?;
        require(
            id(accounts[at].key) == endpoint.owner_row().account(),
            ClutchError::MismatchedState,
        )?;
        ordinal += 1;
    }
    Ok(())
}

fn require_aggregate_creation_principal_v5(
    payer: &AccountInfo<'_>,
    rent: &RentParameters,
    receipt_count: u8,
    owner_row_count: u8,
    fees: &[Option<PreparedOwnerFeeAction24V5>; 2],
) -> Outcome<()> {
    let mut required = rent
        .minimum_balance(contract::SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5)?
        .checked_mul(u64::from(receipt_count))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    required = required
        .checked_add(
            rent.minimum_balance(contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5)?
                .checked_mul(u64::from(owner_row_count))
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        )
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    for fee in fees {
        if let Some(creation) = fee.as_ref().and_then(PreparedOwnerFeeAction24V5::creation) {
            required = required
                .checked_add(creation.carry_rent().refundable_principal)
                .and_then(|value| {
                    value.checked_add(creation.payer_rent().refundable_principal)
                })
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        }
    }
    require(
        payer.lamports() >= required,
        ClutchError::AccountCreationFailed,
    )
}

fn fee_creation_account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    expected: Id32,
) -> Outcome<&'a AccountInfo<'info>> {
    let mut found = None;
    for account in accounts {
        if account.key.to_bytes() == expected.bytes() {
            require(found.is_none(), ClutchError::AccountAlias)?;
            found = Some(account);
        }
    }
    found.ok_or(Refusal::Adapter(ClutchError::MismatchedState))
}

fn set_lamports_v5(account: &AccountInfo<'_>, value: u64) -> Outcome<()> {
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = value;
    Ok(())
}

fn close_owner_fee_assessment_work_v5(account: &AccountInfo<'_>) -> Outcome<()> {
    set_lamports_v5(account, 0)?;
    account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    account.assign(&SYSTEM_PROGRAM_ID);
    require(
        account.lamports() == 0
            && account.data_len() == 0
            && account.owner == &SYSTEM_PROGRAM_ID,
        ClutchError::MismatchedState,
    )
}

#[inline(never)]
fn apply_owner_fee_creations_v5<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    rent: &RentParameters,
    fees: &[Option<PreparedOwnerFeeAction24V5>; 2],
) -> Outcome<()> {
    let payer = &accounts[IX_RENT_PAYER];
    let system = &accounts[IX_SYSTEM];
    let payer_before = payer.lamports();
    let mut creation_principal = 0u64;
    let mut work_refund = 0u64;
    let mut work_donation = 0u64;
    let mut neutral_sink = None;
    for fee in fees {
        let Some(creation) = fee.as_ref().and_then(PreparedOwnerFeeAction24V5::creation) else {
            continue;
        };
        creation_principal = creation_principal
            .checked_add(creation.carry_rent().refundable_principal)
            .and_then(|value| {
                value.checked_add(creation.payer_rent().refundable_principal)
            })
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        work_refund = work_refund
            .checked_add(creation.assessment_work_rent().refundable_principal)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        work_donation = work_donation
            .checked_add(creation.assessment_work_donation_lamports())
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let work_account = fee_creation_account(accounts, creation.assessment_work_account())?;
        let sink_account = fee_creation_account(accounts, creation.neutral_sink())?;
        require(
            work_account.owner == program_id
                && work_account.is_writable
                && !work_account.is_signer
                && !work_account.executable
                && work_account.data_len()
                    == contract::OWNER_FEE_ASSESSMENT_WORK_ACCOUNT_BYTES_V1
                && creation.assessment_work_rent().payer == id(payer.key)
                && work_account.lamports()
                    == creation
                        .assessment_work_rent()
                        .refundable_principal
                        .checked_add(creation.assessment_work_donation_lamports())
                        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
                && creation.assessment_work_donation_lamports()
                    >= creation.assessment_work_rent().donation_floor
                && sink_account.is_writable
                && !sink_account.is_signer
                && !sink_account.executable,
            ClutchError::MismatchedState,
        )?;
        let mut decoded = super::general_v2_owner_fee_assessment_v6::boxed_work_scratch_v6()?;
        let work_data_id = contract::OwnerFeeAssessmentWorkV1AccountV1::decode_into_and_data_id(
            &borrow_data(work_account)?,
            &mut decoded,
            &RuntimeSha256,
            id(work_account.key),
        )?;
        require(
            !decoded.semantic.is_ready()
                && decoded.semantic.next_page() == decoded.semantic.page_count()
                && decoded.rent == creation.assessment_work_rent()
                && work_data_id == creation.assessment_work_data_id(),
            ClutchError::MismatchedState,
        )?;
        if let Some(expected) = neutral_sink {
            require(expected == id(sink_account.key), ClutchError::MismatchedState)?;
        } else {
            neutral_sink = Some(id(sink_account.key));
        }
        for account in [work_account, payer, sink_account] {
            let data = account
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            drop(data);
            let lamports = account
                .try_borrow_mut_lamports()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            drop(lamports);
        }
    }
    let payer_after_creates = payer_before
        .checked_sub(creation_principal)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let payer_after_close = payer_after_creates
        .checked_add(work_refund)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let sink_after = match neutral_sink {
        Some(sink) => fee_creation_account(accounts, sink)?
            .lamports()
            .checked_add(work_donation)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        None => 0,
    };
    for fee in fees {
        let Some(creation) = fee.as_ref().and_then(PreparedOwnerFeeAction24V5::creation) else {
            continue;
        };
        let carry_account = fee_creation_account(accounts, creation.carry_account())?;
        let payer_account =
            fee_creation_account(accounts, creation.payer_allocation_account())?;
        let fee_record = creation.fee_record().bytes();
        let owner = creation.owner().bytes();
        let carry_bump = [creation.carry_bump()];
        let carry_seeds: [&[u8]; 4] = [
            seeds::SEED_GENERAL_V2_OWNER_FEE_CARRY,
            &fee_record,
            &owner,
            &carry_bump,
        ];
        create_from_payer(
            program_id,
            payer,
            carry_account,
            system,
            rent,
            contract::OWNER_FEE_CARRY_ACCOUNT_BYTES_V3,
            creation.carry_rent(),
            &carry_seeds,
        )?;
        encode_account(carry_account, |out| {
            out.copy_from_slice(creation.carry_body());
            Ok(())
        })?;
        let payer_bump = [creation.payer_bump()];
        let payer_seeds: [&[u8]; 4] = [
            seeds::SEED_GENERAL_V2_PAYER_ALLOCATION,
            &fee_record,
            &owner,
            &payer_bump,
        ];
        create_from_payer(
            program_id,
            payer,
            payer_account,
            system,
            rent,
            contract::PAYER_ALLOCATION_ACCOUNT_BYTES_V2,
            creation.payer_rent(),
            &payer_seeds,
        )?;
        encode_account(payer_account, |out| {
            out.copy_from_slice(creation.payer_body());
            Ok(())
        })?;

        let carry_data = borrow_data(carry_account)?;
        let carry = contract::OwnerFeeCarryV3AccountV1::decode(
            &carry_data,
            &creation.selected(),
        )?;
        require(
            carry.rent == creation.carry_rent()
                && carry.stored_bump == creation.carry_bump()
                && &*carry_data == creation.carry_body()
                && carry_account.lamports()
                    == creation
                        .carry_rent()
                        .refundable_principal
                        .checked_add(creation.carry_rent().donation_floor)
                        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
            ClutchError::MismatchedState,
        )?;
        drop(carry_data);
        let payer_data = borrow_data(payer_account)?;
        require(
            &*payer_data == creation.payer_body()
                && contract::payer_allocation_account_data_id_v2(
                    &payer_data,
                    &RuntimeSha256,
                )? == creation.payer_data_id()
                && payer_account.lamports()
                    == creation
                        .payer_rent()
                        .refundable_principal
                        .checked_add(creation.payer_rent().donation_floor)
                        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
            ClutchError::MismatchedState,
        )?;
    }
    require(payer.lamports() == payer_after_creates, ClutchError::MismatchedState)?;
    for fee in fees {
        let Some(creation) = fee.as_ref().and_then(PreparedOwnerFeeAction24V5::creation) else {
            continue;
        };
        let work_account = fee_creation_account(accounts, creation.assessment_work_account())?;
        close_owner_fee_assessment_work_v5(work_account)?;
    }
    if let Some(sink) = neutral_sink {
        let sink_account = fee_creation_account(accounts, sink)?;
        set_lamports_v5(payer, payer_after_close)?;
        set_lamports_v5(sink_account, sink_after)?;
    } else {
        require(
            creation_principal == 0 && work_refund == 0 && work_donation == 0,
            ClutchError::MismatchedState,
        )?;
    }
    Ok(())
}

fn apply_plan<'a>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'a>],
    rent: &RentParameters,
    authenticated_root: &AuthenticatedGeneralSettlementRootV1,
    plan: &MaterializeEntitlementSlicePlanV5,
    fees: &[Option<PreparedOwnerFeeAction24V5>; 2],
) -> Outcome<()> {
    let payer = &accounts[IX_RENT_PAYER];
    let system = &accounts[IX_SYSTEM];
    let mut owner_row_count = 0u8;
    let mut ordinal = 0u8;
    while ordinal < plan.endpoint_count() {
        let endpoint = plan
            .endpoint(ordinal)
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        if matches!(
            endpoint.owner_row(),
            OwnerRowMaterializationDispositionV5::Create { .. }
        ) {
            owner_row_count = owner_row_count
                .checked_add(1)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        }
        ordinal += 1;
    }
    require_aggregate_creation_principal_v5(payer, rent, 1, owner_row_count, fees)?;
    apply_owner_fee_creations_v5(program_id, accounts, rent, fees)?;
    let receipt = plan.receipt();
    let receipt_seed = receipt.seed();
    let receipt_bump = [receipt.bump()];
    let receipt_seeds: [&[u8]; 5] = [
        receipt_seed.domain(),
        receipt_seed.epoch(),
        receipt_seed.settlement_candidate(),
        receipt_seed.slice_index_le(),
        &receipt_bump,
    ];
    let receipt_rent = rent_owner(
        payer,
        &accounts[IX_RECEIPT],
        rent,
        contract::SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5,
    )?;
    create_from_payer(
        program_id,
        payer,
        &accounts[IX_RECEIPT],
        system,
        rent,
        contract::SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5,
        receipt_rent,
        &receipt_seeds,
    )?;
    encode_account(&accounts[IX_RECEIPT], |out| receipt.receipt().encode(out))?;

    ordinal = 0u8;
    while ordinal < plan.endpoint_count() {
        let endpoint = plan
            .endpoint(ordinal)
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        let at = endpoint_base(usize::from(ordinal))?;
        if let OwnerRowMaterializationDispositionV5::Create { plan: row, .. } = endpoint.owner_row()
        {
            let seed = row.seed();
            let bump = [row.bump()];
            let signer_seeds: [&[u8]; 5] = [
                seed.domain(),
                seed.epoch(),
                seed.settlement_candidate(),
                seed.owner(),
                &bump,
            ];
            let owner = rent_owner(
                payer,
                &accounts[at],
                rent,
                contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5,
            )?;
            create_from_payer(
                program_id,
                payer,
                &accounts[at],
                system,
                rent,
                contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5,
                owner,
                &signer_seeds,
            )?;
            encode_account(&accounts[at], |out| {
                out.copy_from_slice(row.exact_body());
                Ok(())
            })?;
        }
        encode_account(&accounts[at + 1], |out| {
            out.copy_from_slice(endpoint.reservation().poststate_body());
            Ok(())
        })?;
        ordinal += 1;
    }
    encode_account(&accounts[IX_ROOT], |out| {
        authenticated_root.encode_materialization_successor(
            plan.settlement_root_poststate(),
            out,
        )
    })
}

fn apply_portfolio_plan<'a>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'a>],
    rent: &RentParameters,
    fee_end: usize,
    authenticated_root: &AuthenticatedGeneralSettlementRootV1,
    plan: &MaterializePortfolioPairPlanV5,
    fees: &[Option<PreparedOwnerFeeAction24V5>; 2],
) -> Outcome<()> {
    let payer = &accounts[IX_RENT_PAYER];
    let system = &accounts[IX_SYSTEM];
    require_aggregate_creation_principal_v5(payer, rent, plan.receipt_count(), 2, fees)?;
    apply_owner_fee_creations_v5(program_id, accounts, rent, fees)?;
    let mut receipt_index = 0u8;
    while receipt_index < plan.receipt_count() {
        let receipt = plan
            .receipt(receipt_index)
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        let account_index = portfolio_receipt_account_index(usize::from(receipt_index), fee_end)?;
        let seed = receipt.seed();
        let bump = [receipt.bump()];
        let signer_seeds: [&[u8]; 5] = [
            seed.domain(),
            seed.epoch(),
            seed.settlement_candidate(),
            seed.slice_index_le(),
            &bump,
        ];
        let receipt_rent = rent_owner(
            payer,
            &accounts[account_index],
            rent,
            contract::SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5,
        )?;
        create_from_payer(
            program_id,
            payer,
            &accounts[account_index],
            system,
            rent,
            contract::SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5,
            receipt_rent,
            &signer_seeds,
        )?;
        encode_account(&accounts[account_index], |out| {
            receipt.receipt().encode(out)
        })?;
        receipt_index = receipt_index
            .checked_add(1)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }

    let mut ordinal = 0u8;
    while ordinal < 2 {
        let endpoint = plan
            .endpoint(ordinal)
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        let at = endpoint_base(usize::from(ordinal))?;
        let OwnerRowMaterializationDispositionV5::Create { plan: row, .. } = endpoint.owner_row()
        else {
            return Err(Refusal::Adapter(ClutchError::MismatchedState));
        };
        let seed = row.seed();
        let bump = [row.bump()];
        let signer_seeds: [&[u8]; 5] = [
            seed.domain(),
            seed.epoch(),
            seed.settlement_candidate(),
            seed.owner(),
            &bump,
        ];
        let owner = rent_owner(
            payer,
            &accounts[at],
            rent,
            contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5,
        )?;
        create_from_payer(
            program_id,
            payer,
            &accounts[at],
            system,
            rent,
            contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5,
            owner,
            &signer_seeds,
        )?;
        encode_account(&accounts[at], |out| {
            out.copy_from_slice(row.exact_body());
            Ok(())
        })?;
        encode_account(&accounts[at + 1], |out| {
            out.copy_from_slice(endpoint.reservation().poststate_body());
            Ok(())
        })?;
        ordinal += 1;
    }
    encode_account(&accounts[IX_ROOT], |out| {
        authenticated_root.encode_materialization_successor(
            plan.settlement_root_poststate(),
            out,
        )
    })
}

/// Prepare and atomically apply one canonical action-24 materialization unit.
///
/// Scalar orders create one receipt. An exclusive Portfolio pair instead
/// creates the complete capability-derived ReceiptV5 sibling set in one
/// rollback domain; no packet count or anchor-only path selects that branch.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        action == GeneralV2Action::FreezeEntitlement
            && capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    let request = FreezeEntitlementPayloadV1::decode(payload)?;
    if try_process_action24_owner_fee_assessment_v6(
        program_id,
        accounts,
        request.epoch,
        request.settlement_root,
    )? == Action24AssessmentDispatchV1::Applied
    {
        return Ok(());
    }
    require(
        accounts.len() <= ACTION24_MAX_ACCOUNT_INFOS_V5,
        ClutchError::WrongAccountCount,
    )?;
    require(
        accounts.len()
            >= ACTION24_TRAVERSAL_PREFIX_ACCOUNTS
                + ACTION24_CREATION_HEADER_ACCOUNTS
                + ACTION24_ENDPOINT_ACCOUNTS
                + 1,
        ClutchError::WrongAccountCount,
    )?;
    require_all_distinct(accounts)?;
    require_signer(&accounts[IX_RENT_PAYER])?;
    require(
        accounts[IX_RENT_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;
    require_system_program(&accounts[IX_SYSTEM])?;
    let rent = read_rent(&accounts[IX_RENT])?;
    let (traversal, page_count) = select_traversal(program_id, accounts)?;
    let root_traversal = authenticate_writable_root_settlement_traversal_v5(
        program_id,
        &accounts[IX_ROOT],
        &traversal,
    )?;
    require(
        request.epoch == root_traversal.root().root().epoch()
            && request.settlement_root == root_traversal.root().account(),
        ClutchError::MismatchedState,
    )?;
    if current_slice_is_portfolio_pair(&root_traversal)? {
        require(
            accounts.len()
                >= ACTION24_TRAVERSAL_PREFIX_ACCOUNTS
                    + ACTION24_CREATION_HEADER_ACCOUNTS
                    + 2 * ACTION24_ENDPOINT_ACCOUNTS
                    + 1,
            ClutchError::WrongAccountCount,
        )?;
        let sibling_set = authenticate_portfolio_materialization_sibling_set_v5(
            program_id,
            &root_traversal,
            &accounts[endpoint_base(0)? + 1],
            &accounts[endpoint_base(0)? + 2],
            &accounts[endpoint_base(1)? + 1],
            &accounts[endpoint_base(1)? + 2],
        )?;
        let (plan, fee_end, fees) = prepare_portfolio_plan(
            program_id,
            accounts,
            &root_traversal,
            page_count,
            &rent,
            sibling_set,
        )?;
        authenticate_portfolio_plan_accounts(program_id, accounts, fee_end, &plan)?;
        apply_portfolio_plan(
            program_id,
            accounts,
            &rent,
            fee_end,
            root_traversal.root(),
            &plan,
            &fees,
        )
    } else {
        let (plan, fees) =
            prepare_generic_plan(program_id, accounts, &root_traversal, page_count, &rent)?;
        authenticate_plan_accounts(program_id, accounts, &plan)?;
        apply_plan(
            program_id,
            accounts,
            &rent,
            root_traversal.root(),
            &plan,
            &fees,
        )
    }
}
