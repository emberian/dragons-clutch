//! Typed SBF retirement of counted General settlement children.
//!
//! Receipt, Reservation, owner-row, fee-finalization, cash-pot, and FinalPot
//! closure each authenticates the exact terminal child and advances exactly
//! one named root counter/state in the same rollback domain. The separate
//! phase gate cannot advance until every per-item liability is discharged.

use core::cell::{Ref, RefMut};

use clutch_fee_runtime_contract::{Id as FeeId, OwnerFeeFinalizationOutcomeV2};
use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    decode_settlement_retirement_payload_v1, CountedSettlementRootSelectorV1,
    DeletableRentOwnerV1, Id32, MarketBindingV5, SettlementChildRetirementPayloadV1,
    SettlementRetirementPayloadKindV1,
};
use clutch_solana_layout::registry::GeneralV2Action;
use clutch_solana_layout::reservation::RESERVATION_STATE_CONSUMED;
use clutch_solana_layout::MAX_OUTCOMES;
use clutch_solana_layout::reservation_v9::{
    DeletableRentOwnerV1 as LayoutRentV1, ReservationAccountV9,
    RESERVATION_ACCOUNT_BYTES_V9,
};
use clutch_solana_layout::settlement_receipt_v5::{
    SettlementReceiptAccountV5, SettlementReceiptTransitionCommitmentV5,
    SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{require, require_count, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;
use crate::seeds;

use super::general_v2_settlement_root::{
    authenticate_writable_general_settlement_root_epoch_v1,
    AuthenticatedGeneralSettlementRootV1,
};
use super::general_market_current_v5::{
    authenticate_general_market_current_prefix_v5, CURRENT_V5_IX_MARKET_BINDING,
    GENERAL_MARKET_CURRENT_ACCOUNT_COUNT_V5,
};

/// Root, child, MarketBinding, principal payer, and neutral sink.
pub const SETTLEMENT_CHILD_CLOSE_ACCOUNT_COUNT_V1: usize = 5;
/// Root, already-closed selected fee-record address, and MarketBinding.
pub const FEE_RECORD_RETIREMENT_ACCOUNT_COUNT_V1: usize = 3;
/// Writable root and immutable MarketBinding.
pub const BEGIN_RETIRING_ACCOUNT_COUNT_V1: usize = 2;
/// Exact actions48/49 frame: hostile current prefix plus root, child, payer, sink.
pub const CURRENT_SETTLEMENT_CHILD_CLOSE_ACCOUNT_COUNT_V5: usize =
    GENERAL_MARKET_CURRENT_ACCOUNT_COUNT_V5 + 4;
/// Exact action51 frame: hostile current prefix plus the writable indexed root.
pub const CURRENT_BEGIN_RETIRING_ACCOUNT_COUNT_V5: usize =
    GENERAL_MARKET_CURRENT_ACCOUNT_COUNT_V5 + 1;

const IX_ROOT: usize = 0;
const IX_CHILD: usize = 1;
const IX_BINDING: usize = 2;
const IX_PAYER: usize = 3;
const IX_SINK: usize = 4;

fn id(key: &Pubkey) -> Id32 {
    Id32::from_bytes(key.to_bytes())
}

fn borrow_data<'a, 'info>(account: &'a AccountInfo<'info>) -> Outcome<Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(Ref::map(data, |bytes| &**bytes))
}

fn borrow_mut_data<'a, 'info>(
    account: &'a AccountInfo<'info>,
) -> Outcome<RefMut<'a, [u8]>> {
    let data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(RefMut::map(data, |bytes| &mut **bytes))
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
        if writable { ClutchError::NotWritable } else { ClutchError::UnexpectedWritable },
    )?;
    if let Some(len) = exact_len {
        require(account.data_len() == len, ClutchError::WrongDataLength)?;
    }
    Ok(())
}

fn require_destination(account: &AccountInfo<'_>) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)
}

fn decode_binding(program_id: &Pubkey, account: &AccountInfo<'_>) -> Outcome<MarketBindingV5> {
    require_program_state(
        program_id,
        account,
        false,
        Some(contract::MARKET_BINDING_ACCOUNT_BYTES_V5),
    )?;
    let binding = MarketBindingV5::decode(&borrow_data(account)?)?;
    let canonical = seeds::general_v2_market_binding_pda(
        program_id,
        &binding.base().base().market_instance_v2_id.bytes(),
    );
    require(
        *account.key == canonical.0 && binding.base().base().stored_bump == canonical.1,
        ClutchError::WrongPda,
    )?;
    Ok(binding)
}

fn authenticate_root(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    selector: CountedSettlementRootSelectorV1,
) -> Outcome<AuthenticatedGeneralSettlementRootV1> {
    require(selector.settlement_root == id(account.key), ClutchError::MismatchedState)?;
    let root = authenticate_writable_general_settlement_root_epoch_v1(
        program_id,
        core::slice::from_ref(account),
        selector.epoch,
    )?;
    require(root.is_indexed(), ClutchError::MismatchedState)?;
    Ok(root)
}

fn require_root_binding(
    root: &AuthenticatedGeneralSettlementRootV1,
    binding_account: &AccountInfo<'_>,
    binding: &MarketBindingV5,
) -> Outcome<()> {
    let value = root.root();
    require(
        value.market_binding() == id(binding_account.key)
            && value.market() == binding.base().base().market
            && value.market_instance_v2_id() == binding.base().base().market_instance_v2_id
            && value.batch_policy_id() == binding.base().batch_policy_id(),
        ClutchError::MismatchedState,
    )
}

fn checked_close_balances(
    source: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    sink: &AccountInfo<'_>,
    rent: DeletableRentOwnerV1,
) -> Outcome<(u64, u64)> {
    rent.validate()?;
    require_destination(payer)?;
    require_destination(sink)?;
    require(
        source.key != payer.key
            && source.key != sink.key
            && rent.payer == id(payer.key),
        ClutchError::AccountAlias,
    )?;
    let required = rent
        .refundable_principal
        .checked_add(rent.donation_floor)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(source.lamports() >= required, ClutchError::MismatchedState)?;
    let donation = source
        .lamports()
        .checked_sub(rent.refundable_principal)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let (payer_after, sink_after) = if payer.key == sink.key {
        let after = payer
            .lamports()
            .checked_add(rent.refundable_principal)
            .and_then(|value| value.checked_add(donation))
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        (after, after)
    } else {
        (
            payer
                .lamports()
                .checked_add(rent.refundable_principal)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
            sink
                .lamports()
                .checked_add(donation)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        )
    };
    Ok((payer_after, sink_after))
}

fn contract_rent(rent: LayoutRentV1) -> Outcome<DeletableRentOwnerV1> {
    let value = DeletableRentOwnerV1 {
        payer: Id32::new(rent.payer.bytes())?,
        refundable_principal: rent.refundable_principal,
        donation_floor: rent.donation_floor,
    };
    value.validate()?;
    Ok(value)
}

fn set_lamports(account: &AccountInfo<'_>, value: u64) -> Outcome<()> {
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = value;
    Ok(())
}

fn close_program_account(account: &AccountInfo<'_>) -> Outcome<()> {
    set_lamports(account, 0)?;
    account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    account.assign(&SYSTEM_PROGRAM_ID);
    require(
        account.data_len() == 0
            && account.lamports() == 0
            && *account.owner == SYSTEM_PROGRAM_ID,
        ClutchError::MismatchedState,
    )
}

fn apply_child_close(
    root_account: &AccountInfo<'_>,
    child: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    sink: &AccountInfo<'_>,
    root_output: &[u8],
    payer_after: u64,
    sink_after: u64,
) -> Outcome<()> {
    for account in [root_account, child, payer, sink] {
        let data = account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        drop(data);
        let lamports = account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        drop(lamports);
    }
    borrow_mut_data(root_account)?.copy_from_slice(root_output);
    close_program_account(child)?;
    set_lamports(payer, payer_after)?;
    if payer.key == sink.key {
        require(payer_after == sink_after, ClutchError::MismatchedState)?;
    } else {
        set_lamports(sink, sink_after)?;
    }
    require(
        &*borrow_data(root_account)? == root_output
            && child.data_len() == 0
            && child.lamports() == 0
            && *child.owner == SYSTEM_PROGRAM_ID
            && payer.lamports() == payer_after
            && sink.lamports() == sink_after,
        ClutchError::MismatchedState,
    )
}

fn prepare_child_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selector: SettlementChildRetirementPayloadV1,
    exact_child_len: usize,
) -> Outcome<(AuthenticatedGeneralSettlementRootV1, MarketBindingV5)> {
    require_count(accounts, SETTLEMENT_CHILD_CLOSE_ACCOUNT_COUNT_V1)?;
    require(selector.child == id(accounts[IX_CHILD].key), ClutchError::MismatchedState)?;
    require_program_state(program_id, &accounts[IX_CHILD], true, Some(exact_child_len))?;
    require(
        accounts[IX_ROOT].key != accounts[IX_CHILD].key
            && accounts[IX_ROOT].key != accounts[IX_BINDING].key
            && accounts[IX_CHILD].key != accounts[IX_BINDING].key
            && accounts[IX_ROOT].key != accounts[IX_PAYER].key
            && accounts[IX_ROOT].key != accounts[IX_SINK].key,
        ClutchError::AccountAlias,
    )?;
    let root = authenticate_root(
        program_id,
        &accounts[IX_ROOT],
        CountedSettlementRootSelectorV1 {
            epoch: selector.epoch,
            settlement_root: selector.settlement_root,
        },
    )?;
    let binding = decode_binding(program_id, &accounts[IX_BINDING])?;
    require_root_binding(&root, &accounts[IX_BINDING], &binding)?;
    require(
        binding.base().base().neutral_sink == id(accounts[IX_SINK].key),
        ClutchError::MismatchedState,
    )?;
    Ok((root, binding))
}

#[inline(never)]
fn close_receipt(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selector: SettlementChildRetirementPayloadV1,
) -> Outcome<()> {
    let (root, _) = prepare_child_frame(
        program_id,
        accounts,
        selector,
        SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5,
    )?;
    let body = borrow_data(&accounts[IX_CHILD])?;
    let receipt = SettlementReceiptAccountV5::decode(&body)?;
    let semantic = receipt.semantic();
    let canonical = seeds::general_v2_receipt_v5_pda(
        program_id,
        &root.root().epoch().bytes(),
        &root.root().settlement_candidate_id().bytes(),
        semantic.slice_index,
    );
    require(
        *accounts[IX_CHILD].key == canonical.0
            && semantic.stored_bump == canonical.1
            && semantic.market.0 == root.root().market().bytes()
            && semantic.epoch.0 == root.root().epoch().bytes()
            && semantic.candidate.0 == root.root().settlement_candidate_id().bytes()
            && semantic.payment_complete()
            && !matches!(
                receipt.transition(),
                SettlementReceiptTransitionCommitmentV5::PortfolioPairPending
            ),
        ClutchError::MismatchedState,
    )?;
    let rent = contract_rent(receipt.rent())?;
    let (payer_after, sink_after) = checked_close_balances(
        &accounts[IX_CHILD],
        &accounts[IX_PAYER],
        &accounts[IX_SINK],
        rent,
    )?;
    let mut root_output = std::vec![0u8; root.account_bytes()];
    root.encode_receipt_retirement_successor(&mut root_output)?;
    drop(body);
    apply_child_close(
        &accounts[IX_ROOT], &accounts[IX_CHILD], &accounts[IX_PAYER],
        &accounts[IX_SINK], &root_output, payer_after, sink_after,
    )
}

#[inline(never)]
fn close_reservation(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selector: SettlementChildRetirementPayloadV1,
) -> Outcome<()> {
    let (root, _) = prepare_child_frame(
        program_id,
        accounts,
        selector,
        RESERVATION_ACCOUNT_BYTES_V9,
    )?;
    let bytes = borrow_data(&accounts[IX_CHILD])?;
    let reservation = ReservationAccountV9::decode(&bytes)?;
    let semantic = reservation.body();
    let canonical = seeds::general_v2_reservation_v9_pda(
        program_id,
        &semantic.reservation.bytes(),
    );
    require(
        *accounts[IX_CHILD].key == canonical.0
            && semantic.stored_bump == canonical.1
            && semantic.market.bytes() == root.root().market().bytes()
            && semantic.epoch.bytes() == root.root().epoch().bytes()
            && semantic.state == RESERVATION_STATE_CONSUMED
            && semantic.entitled_units > 0
            && semantic.consumed_units == semantic.entitled_units
            && semantic.paid_units == semantic.entitled_units
            && semantic.remaining_cash_atoms == 0
            && semantic.remaining_internal == [0; MAX_OUTCOMES],
        ClutchError::MismatchedState,
    )?;
    let rent = contract_rent(reservation.rent())?;
    let (payer_after, sink_after) = checked_close_balances(
        &accounts[IX_CHILD], &accounts[IX_PAYER], &accounts[IX_SINK], rent,
    )?;
    let mut root_output = std::vec![0u8; root.account_bytes()];
    root.encode_reservation_retirement_successor(&mut root_output)?;
    drop(bytes);
    apply_child_close(
        &accounts[IX_ROOT], &accounts[IX_CHILD], &accounts[IX_PAYER],
        &accounts[IX_SINK], &root_output, payer_after, sink_after,
    )
}

#[inline(never)]
fn close_owner_row(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selector: SettlementChildRetirementPayloadV1,
) -> Outcome<()> {
    let (root, _) = prepare_child_frame(
        program_id,
        accounts,
        selector,
        contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5,
    )?;
    let bytes = borrow_data(&accounts[IX_CHILD])?;
    let row = contract::OwnerSettlementV5AccountV1::decode(&bytes)?;
    let terminal = row.terminal_projection()?;
    let expectation = terminal.semantic().expectation();
    let canonical = seeds::general_v2_owner_settlement_v5_pda(
        program_id,
        &root.root().epoch().bytes(),
        &root.root().settlement_candidate_id().bytes(),
        &expectation.owner(),
    );
    require(
        *accounts[IX_CHILD].key == canonical.0
            && row.stored_bump == canonical.1
            && expectation.market() == root.root().market().bytes()
            && expectation.epoch() == root.root().epoch().bytes()
            && expectation.candidate() == root.root().settlement_candidate_id().bytes()
            && expectation.owner_order_set_digest()
                == root.root().owner_order_set_digest().bytes(),
        ClutchError::MismatchedState,
    )?;
    let (payer_after, sink_after) = checked_close_balances(
        &accounts[IX_CHILD], &accounts[IX_PAYER], &accounts[IX_SINK], row.rent,
    )?;
    let mut root_output = std::vec![0u8; root.account_bytes()];
    root.encode_owner_row_retirement_successor(&mut root_output)?;
    drop(bytes);
    apply_child_close(
        &accounts[IX_ROOT], &accounts[IX_CHILD], &accounts[IX_PAYER],
        &accounts[IX_SINK], &root_output, payer_after, sink_after,
    )
}

fn cash_pot_terminal(
    pot: &contract::SettlementCashPotV1AccountV1,
    root: &contract::SettlementRootV1AccountV1,
) -> Outcome<()> {
    let semantic = pot.semantic;
    let expected = root.cash_pot_expectation()?;
    let expected_state = match root.virtual_cash_direction() {
        contract::VirtualCashDirectionV1::Split => 2,
        contract::VirtualCashDirectionV1::None | contract::VirtualCashDirectionV1::Merge => 1,
    };
    require(
        semantic.expectation == expected
            && semantic.finalized_owner_count == expected.owner_count
            && semantic.state == expected_state,
        ClutchError::MismatchedState,
    )
}

#[inline(never)]
fn close_pot(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selector: SettlementChildRetirementPayloadV1,
) -> Outcome<()> {
    require_count(accounts, SETTLEMENT_CHILD_CLOSE_ACCOUNT_COUNT_V1)?;
    let root = authenticate_root(
        program_id,
        &accounts[IX_ROOT],
        CountedSettlementRootSelectorV1 {
            epoch: selector.epoch,
            settlement_root: selector.settlement_root,
        },
    )?;
    let binding = decode_binding(program_id, &accounts[IX_BINDING])?;
    require_root_binding(&root, &accounts[IX_BINDING], &binding)?;
    require(selector.child == id(accounts[IX_CHILD].key), ClutchError::MismatchedState)?;
    require_program_state(program_id, &accounts[IX_CHILD], true, None)?;
    require(
        accounts[IX_ROOT].key != accounts[IX_CHILD].key
            && accounts[IX_ROOT].key != accounts[IX_BINDING].key
            && accounts[IX_ROOT].key != accounts[IX_PAYER].key
            && accounts[IX_ROOT].key != accounts[IX_SINK].key
            && accounts[IX_CHILD].key != accounts[IX_BINDING].key,
        ClutchError::AccountAlias,
    )?;
    let bytes = borrow_data(&accounts[IX_CHILD])?;
    let (rent, final_pot) = if selector.child == root.root().settlement_cash_pot() {
        require(
            bytes.len() == contract::SETTLEMENT_CASH_POT_ACCOUNT_BYTES,
            ClutchError::WrongDataLength,
        )?;
        let pot = contract::SettlementCashPotV1AccountV1::decode(&bytes)?;
        let canonical = seeds::general_v2_settlement_cash_pot_pda(
            program_id,
            &root.root().epoch().bytes(),
            &root.root().settlement_candidate_id().bytes(),
        );
        require(
            *accounts[IX_CHILD].key == canonical.0 && pot.stored_bump == canonical.1,
            ClutchError::WrongPda,
        )?;
        cash_pot_terminal(&pot, root.root())?;
        (root.root().cash_pot_rent(), false)
    } else if selector.child == root.root().final_pot() {
        require(bytes.len() == contract::FINAL_POT_ACCOUNT_BYTES, ClutchError::WrongDataLength)?;
        let seed = contract::FinalPotSeedTupleV1::new(
            root.root().epoch(),
            root.root().settlement_candidate_id(),
        )?;
        let canonical = seeds::find(
            program_id,
            &[seed.domain(), seed.epoch(), seed.settlement_candidate()],
        );
        require(*accounts[IX_CHILD].key == canonical.0, ClutchError::WrongPda)?;
        contract::FinalPotV1AccountV1::decode_counted_root_retirement(
            &bytes,
            selector.child,
            canonical.1,
            root.root(),
        )?;
        (
            root.root()
                .final_pot_rent()?
                .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?,
            true,
        )
    } else {
        return Err(Refusal::Adapter(ClutchError::MismatchedState));
    };
    require(
        binding.base().base().neutral_sink == id(accounts[IX_SINK].key),
        ClutchError::MismatchedState,
    )?;
    let (payer_after, sink_after) = checked_close_balances(
        &accounts[IX_CHILD], &accounts[IX_PAYER], &accounts[IX_SINK], rent,
    )?;
    let mut root_output = std::vec![0u8; root.account_bytes()];
    if final_pot {
        root.encode_final_pot_retirement_successor(&mut root_output)?;
    } else {
        root.encode_cash_pot_retirement_successor(&mut root_output)?;
    }
    drop(bytes);
    apply_child_close(
        &accounts[IX_ROOT], &accounts[IX_CHILD], &accounts[IX_PAYER],
        &accounts[IX_SINK], &root_output, payer_after, sink_after,
    )
}

#[inline(never)]
fn close_fee_finalization(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selector: SettlementChildRetirementPayloadV1,
) -> Outcome<()> {
    let (root, _) = prepare_child_frame(
        program_id,
        accounts,
        selector,
        contract::OWNER_FEE_FINALIZATION_ACCOUNT_BYTES_V4,
    )?;
    let bytes = borrow_data(&accounts[IX_CHILD])?;
    let finalization = contract::OwnerFeeFinalizationV4AccountV1::decode(&bytes)?;
    let terminal = finalization.terminal_projection(FeeId(selector.child.bytes()))?;
    let canonical = seeds::general_v2_owner_fee_carry_pda(
        program_id,
        &root.root().fee_record().bytes(),
        &terminal.owner.0,
    );
    require(
        *accounts[IX_CHILD].key == canonical.0
            && finalization.stored_bump == canonical.1
            && terminal.outcome == OwnerFeeFinalizationOutcomeV2::Settled
            && terminal.fee_record.0 == root.root().fee_record().bytes()
            && terminal.settlement_candidate.0
                == root.root().settlement_candidate_id().bytes()
            && terminal.settlement_cash_pot.0
                == root.root().settlement_cash_pot().bytes(),
        ClutchError::MismatchedState,
    )?;
    let (payer_after, sink_after) = checked_close_balances(
        &accounts[IX_CHILD], &accounts[IX_PAYER], &accounts[IX_SINK], finalization.rent,
    )?;
    let mut root_output = std::vec![0u8; root.account_bytes()];
    root.encode_fee_finalization_retirement_successor(&mut root_output)?;
    drop(bytes);
    apply_child_close(
        &accounts[IX_ROOT], &accounts[IX_CHILD], &accounts[IX_PAYER],
        &accounts[IX_SINK], &root_output, payer_after, sink_after,
    )
}

#[inline(never)]
fn begin_retiring(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selector: CountedSettlementRootSelectorV1,
) -> Outcome<()> {
    require_count(accounts, BEGIN_RETIRING_ACCOUNT_COUNT_V1)?;
    require(accounts[IX_ROOT].key != accounts[IX_CHILD].key, ClutchError::AccountAlias)?;
    let root = authenticate_root(program_id, &accounts[IX_ROOT], selector)?;
    let binding = decode_binding(program_id, &accounts[IX_CHILD])?;
    require_root_binding(&root, &accounts[IX_CHILD], &binding)?;
    let mut root_output = std::vec![0u8; root.account_bytes()];
    root.encode_begin_retiring_successor(&mut root_output)?;
    borrow_mut_data(&accounts[IX_ROOT])?.copy_from_slice(&root_output);
    require(
        &*borrow_data(&accounts[IX_ROOT])? == root_output.as_slice(),
        ClutchError::MismatchedState,
    )
}

/// Dispatch-compatible entrypoint for current actions 48, 49, and 51.
#[inline(never)]
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        matches!(
            action,
            GeneralV2Action::CloseOwnerSettlementRow
                | GeneralV2Action::CloseOwnerFeeFinalization
                | GeneralV2Action::BeginSettlementRetirement
        ) && capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    let current = authenticate_general_market_current_prefix_v5(program_id, accounts)?;
    require(
        current.binding_account() == *accounts[CURRENT_V5_IX_MARKET_BINDING].key,
        ClutchError::MismatchedState,
    )?;
    let suffix = GENERAL_MARKET_CURRENT_ACCOUNT_COUNT_V5;
    match action {
        GeneralV2Action::CloseOwnerSettlementRow
        | GeneralV2Action::CloseOwnerFeeFinalization => {
            require_count(accounts, CURRENT_SETTLEMENT_CHILD_CLOSE_ACCOUNT_COUNT_V5)?;
            let local = [
                accounts[suffix].clone(),
                accounts[suffix + 1].clone(),
                accounts[CURRENT_V5_IX_MARKET_BINDING].clone(),
                accounts[suffix + 2].clone(),
                accounts[suffix + 3].clone(),
            ];
            process_local(program_id, &local, sequence, action, payload)
        }
        GeneralV2Action::BeginSettlementRetirement => {
            require_count(accounts, CURRENT_BEGIN_RETIRING_ACCOUNT_COUNT_V5)?;
            let local = [
                accounts[suffix].clone(),
                accounts[CURRENT_V5_IX_MARKET_BINDING].clone(),
            ];
            process_local(program_id, &local, sequence, action, payload)
        }
        _ => Err(Refusal::Adapter(ClutchError::UnsupportedInstruction)),
    }
}

#[inline(never)]
fn process_local(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    match decode_settlement_retirement_payload_v1(action.tag(), payload)? {
        SettlementRetirementPayloadKindV1::CloseReceipt(value) => {
            require(action == GeneralV2Action::CloseReceipt, ClutchError::UnsupportedInstruction)?;
            close_receipt(program_id, accounts, value)
        }
        SettlementRetirementPayloadKindV1::CloseReservation(value) => {
            require(action == GeneralV2Action::CloseReservation, ClutchError::UnsupportedInstruction)?;
            close_reservation(program_id, accounts, value)
        }
        SettlementRetirementPayloadKindV1::ClosePot(value) => {
            require(action == GeneralV2Action::ClosePot, ClutchError::UnsupportedInstruction)?;
            close_pot(program_id, accounts, value)
        }
        SettlementRetirementPayloadKindV1::CloseOwnerRow(value) => {
            require(
                action == GeneralV2Action::CloseOwnerSettlementRow,
                ClutchError::UnsupportedInstruction,
            )?;
            close_owner_row(program_id, accounts, value)
        }
        SettlementRetirementPayloadKindV1::CloseFeeFinalization(value) => {
            require(
                action == GeneralV2Action::CloseOwnerFeeFinalization,
                ClutchError::UnsupportedInstruction,
            )?;
            close_fee_finalization(program_id, accounts, value)
        }
        SettlementRetirementPayloadKindV1::BeginRetiring(value) => {
            require(
                action == GeneralV2Action::BeginSettlementRetirement,
                ClutchError::UnsupportedInstruction,
            )?;
            begin_retiring(program_id, accounts, value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_frames_are_frozen_by_transition_kind() {
        assert_eq!(SETTLEMENT_CHILD_CLOSE_ACCOUNT_COUNT_V1, 5);
        assert_eq!(FEE_RECORD_RETIREMENT_ACCOUNT_COUNT_V1, 3);
        assert_eq!(BEGIN_RETIRING_ACCOUNT_COUNT_V1, 2);
        assert_eq!(CURRENT_SETTLEMENT_CHILD_CLOSE_ACCOUNT_COUNT_V5, 29);
        assert_eq!(CURRENT_BEGIN_RETIRING_ACCOUNT_COUNT_V5, 26);
    }
}
