//! Staged General action 24: materialize one rent-owned V5 settlement slice.
//!
//! The 64-byte selector names only Epoch and counted SettlementRoot.  Every
//! page, owner, order, receipt, Reservation, Position, fee, and rent fact is
//! rederived from the program-owned root, retained Feed, and complete V5 page
//! traversal.  The generic route deliberately refuses Portfolio orders; the
//! exhaustive all-sibling Portfolio producer has a separate bounded composer.

use core::cell::Ref;

use clutch_batch_policy_identity::revenue_policy_v1::{
    decode_revenue_policy, RevenuePolicyV1, REVENUE_POLICY_BYTES,
};
use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    FreezeEntitlementPayloadV1, Id32, OwnerSettlementSeedTupleV5, SettlementRootChildStateV1,
};
use clutch_general_v2_runtime::{
    derive_candidate_entitlement_projection_v4, prepare_materialize_entitlement_slice_v5,
    CandidateEntitlementProjectionV4, EntitlementEndpointInputV5,
    MaterializationReservationInputV9, MaterializeEntitlementSliceInputV5,
    MaterializeEntitlementSlicePlanV5, OwnerRowMaterializationDispositionV5,
    OwnerRowMaterializationInputV5, PositionAccountInputV3, RentOwnedSettlementCreateFundingV5,
    SettlementLegV1, SettlementRouteV1,
};
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
    prepare_owner_fee_action24_v5, OwnerFeeAccountInputV5, OwnerFeeSnapshotAccountFrameV5,
    PreparedOwnerFeeAction24V5,
};
use super::general_v2_settlement_producer_v5::{create_from_payer, encode_account, rent_owner};
use super::general_v2_settlement_traversal_v5::{
    authenticate_settlement_traversal_v5, authenticate_writable_root_settlement_traversal_v5,
    AuthenticatedSettlementTraversalV5, SettlementTraversalAccountFrameV5,
};

/// Shared immutable traversal roles before the final PageV5 suffix.
pub const ACTION24_TRAVERSAL_PREFIX_ACCOUNTS: usize = 12;
/// Receipt, common rent payer, System program, and Rent sysvar.
pub const ACTION24_CREATION_HEADER_ACCOUNTS: usize = 4;
/// Owner row, ReservationV9, and PositionV3 for one real endpoint.
pub const ACTION24_ENDPOINT_ACCOUNTS: usize = 3;
/// Candidate-wide fee accounts shared by every newly created owner row.
pub const ACTION24_FEE_COMMON_ACCOUNTS: usize = 4;
/// Owner carry and payer snapshot for one newly created fee-bearing row.
pub const ACTION24_FEE_OWNER_ACCOUNTS: usize = 2;

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

fn id(key: &Pubkey) -> Id32 {
    Id32::from_bytes(key.to_bytes())
}

fn borrow_data<'a, 'info>(account: &'a AccountInfo<'info>) -> Outcome<Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(Ref::map(data, |bytes| &**bytes))
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
    entitlement: &CandidateEntitlementProjectionV4,
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

fn endpoint_base(ordinal: usize) -> Outcome<usize> {
    IX_FIRST_ENDPOINT
        .checked_add(
            ordinal
                .checked_mul(ACTION24_ENDPOINT_ACCOUNTS)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        )
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))
}

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

fn select_traversal(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Outcome<(AuthenticatedSettlementTraversalV5, usize)> {
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
    entitlement: &CandidateEntitlementProjectionV4,
    order_indices: [u8; 2],
    endpoint_count: usize,
    fresh: [bool; 2],
    fee_at: usize,
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
                    .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
                out[ordinal] = Some(prepare_owner_fee_action24_v5(
                    program_id,
                    root,
                    entitlement,
                    Id32::new(membership.owner)?,
                    &accounts[endpoint_base(ordinal)?],
                    OwnerFeeAccountInputV5::NoFeeRecord,
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
    let revenue_preimage = &accounts[fee_at + 3];
    require(
        !revenue_preimage.is_writable
            && !revenue_preimage.is_signer
            && revenue_preimage.executable
            && revenue_preimage.data_len() == REVENUE_POLICY_BYTES,
        ClutchError::MismatchedState,
    )?;
    let revenue: RevenuePolicyV1 = decode_revenue_policy(&borrow_data(revenue_preimage)?)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut owner_fee_at = common_end;
    let mut ordinal = 0usize;
    while ordinal < endpoint_count {
        if fresh[ordinal] {
            let owner_fee_end = owner_fee_at
                .checked_add(ACTION24_FEE_OWNER_ACCOUNTS)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
            require(
                owner_fee_end <= accounts.len(),
                ClutchError::WrongAccountCount,
            )?;
            let membership = entitlement
                .settlement_membership(order_indices[ordinal])
                .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
            out[ordinal] = Some(prepare_owner_fee_action24_v5(
                program_id,
                root,
                entitlement,
                Id32::new(membership.owner)?,
                &accounts[endpoint_base(ordinal)?],
                OwnerFeeAccountInputV5::CandidateFee {
                    frame: OwnerFeeSnapshotAccountFrameV5 {
                        owner_row: &accounts[endpoint_base(ordinal)?],
                        selected_fee_record: &accounts[fee_at],
                        owner_fee_carry: &accounts[owner_fee_at],
                        payer_allocation: &accounts[owner_fee_at + 1],
                        batch_policy: &accounts[fee_at + 1],
                        revenue_policy_record: &accounts[fee_at + 2],
                    },
                    revenue_policy: &revenue,
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
    fee: Option<PreparedOwnerFeeAction24V5>,
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
    root_traversal: &super::general_v2_settlement_traversal_v5::AuthenticatedRootSettlementTraversalV5<'_>,
    page_count: usize,
    rent: &RentParameters,
) -> Outcome<Box<MaterializeEntitlementSlicePlanV5>> {
    let root = root_traversal.root();
    let entitlement = Box::new(derive_candidate_entitlement_projection_v4(
        root.account(),
        root.root(),
        root_traversal.traversal().traversal(),
    )?);
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
        &entitlement,
        order_indices,
        endpoint_count,
        fresh,
        fee_at,
    )?;
    let fresh_count = usize::from(fresh[0]) + usize::from(fresh[1]);
    require(
        pages_at
            == non_page_account_count(
                endpoint_count,
                fresh_count,
                root.root().fee_record_state() == SettlementRootChildStateV1::Live,
            )?,
        ClutchError::WrongAccountCount,
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
        fee[0],
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
            fee[1],
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
    })?;
    Ok(Box::new(plan))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_partition_is_exact_and_presence_explicit() {
        assert_eq!(non_page_account_count(1, 0, false).unwrap(), 19);
        assert_eq!(non_page_account_count(2, 0, false).unwrap(), 22);
        assert_eq!(non_page_account_count(1, 1, true).unwrap(), 25);
        assert_eq!(non_page_account_count(2, 2, true).unwrap(), 30);
    }

    #[test]
    fn account_partition_refuses_impossible_fresh_rows() {
        assert!(non_page_account_count(0, 0, false).is_err());
        assert!(non_page_account_count(3, 0, false).is_err());
        assert!(non_page_account_count(1, 2, true).is_err());
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

fn apply_plan<'a>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'a>],
    rent: &RentParameters,
    plan: &MaterializeEntitlementSlicePlanV5,
) -> Outcome<()> {
    let payer = &accounts[IX_RENT_PAYER];
    let system = &accounts[IX_SYSTEM];
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

    let mut ordinal = 0u8;
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
        plan.settlement_root_poststate().encode(out)
    })
}

/// Prepare and atomically apply one non-Portfolio action-24 slice.
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
    let plan = prepare_generic_plan(program_id, accounts, &root_traversal, page_count, &rent)?;
    authenticate_plan_accounts(program_id, accounts, &plan)?;
    apply_plan(program_id, accounts, &rent, &plan)
}
