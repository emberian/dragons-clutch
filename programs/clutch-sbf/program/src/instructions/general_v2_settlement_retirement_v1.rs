//! Typed SBF retirement of counted General settlement children.
//!
//! Receipt, Reservation, owner-row, fee-finalization, cash-pot, and FinalPot
//! closure each authenticates the exact terminal child and advances exactly
//! one named root counter/state in the same rollback domain. The separate
//! phase gate cannot advance until every per-item liability is discharged.

use core::cell::{Ref, RefMut};

use clutch_collateral_adapter_v2::{
    refine_market_collateral_v2, BoundCollateralProfileV2, Id as CollateralId,
    MarketCollateralBindingV2,
};
use clutch_fee_runtime_contract::projection::SelectedOwnerFeeBookHashV1;
use clutch_fee_runtime_contract::retirement::FeeRetirementHashV1;
use clutch_fee_runtime_contract::terminal::{
    CandidateFeeAccountRoleV1, ExternalFeeAccountClosureV1, FeeTerminalOutcomeV1,
};
use clutch_fee_runtime_contract::{Id as FeeId, OwnerFeeFinalizationOutcomeV2};
use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    decode_fee_retirement_payload_v1, decode_settlement_retirement_payload_v1,
    CountedSettlementRootSelectorV1, FeeMakerDistributionPayloadV1,
    FeeRetirementPayloadV1, fee_runtime_semantic_release_id_v1, DeletableRentOwnerV1,
    FeeRetirementAccumulatorV1AccountV1, Id32, MarketBindingV4,
    RecipientAllocationV2AccountV1, SettlementCashPotV1AccountV1,
    SettlementChildRetirementPayloadV1,
    SettlementRetirementPayloadKindV1,
};
use clutch_product_series::{ContentId, MarketGenesisProfileV2, MarketInstancePreimageV2};
use clutch_retirement::{PositionAccountV3, PositionV3Sha256Backend, ReplayV3HashBackend};
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
    authenticate_readonly_general_settlement_root_epoch_v1,
    authenticate_writable_general_settlement_root_epoch_v1,
    AuthenticatedGeneralSettlementRootV1,
};
use super::collateral_position_v3::authenticate_general_market_v4;
use super::general_v2_position_replay::authenticate_current_general_position_replay_v2;
use super::product_artifact::authenticate_product_artifact_v1;

/// Root, child, MarketBinding, principal payer, and neutral sink.
pub const SETTLEMENT_CHILD_CLOSE_ACCOUNT_COUNT_V1: usize = 5;
/// Root, finalization, MarketBinding, refund owner, neutral sink, accumulator.
pub const FEE_FINALIZATION_CLOSE_ACCOUNT_COUNT_V1: usize = 6;
/// Root, already-closed selected fee-record address, and MarketBinding.
pub const FEE_RECORD_RETIREMENT_ACCOUNT_COUNT_V1: usize = 3;
/// Writable root and immutable MarketBinding.
pub const BEGIN_RETIRING_ACCOUNT_COUNT_V1: usize = 2;
/// Action-50 maker credit including the complete current collateral join.
pub const FEE_MAKER_DISTRIBUTION_ACCOUNT_COUNT_V1: usize = 14;

const IX_ROOT: usize = 0;
const IX_CHILD: usize = 1;
const IX_BINDING: usize = 2;
const IX_PAYER: usize = 3;
const IX_SINK: usize = 4;
const IX_FEE_ACCUMULATOR: usize = 5;

const FEE_IX_ROOT: usize = 0;
const FEE_IX_ACCUMULATOR: usize = 1;
const FEE_IX_RECIPIENT: usize = 2;
const FEE_IX_CASH_POT: usize = 3;
const FEE_IX_POSITION: usize = 4;
const FEE_IX_REPLAY: usize = 5;
const FEE_IX_BINDING: usize = 6;
const FEE_IX_RUNTIME: usize = 7;
const FEE_IX_REALM: usize = 8;
const FEE_IX_PROFILE: usize = 9;
const FEE_IX_POLICY: usize = 10;
const FEE_IX_TOKEN: usize = 11;
const FEE_IX_MARKET_INSTANCE: usize = 12;
const FEE_IX_MARKET_GENESIS: usize = 13;

const OWNER_FEE_CLOSE_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-owner-fee-close/v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeSha256;

impl contract::Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

impl SelectedOwnerFeeBookHashV1 for RuntimeSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

impl FeeRetirementHashV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

impl PositionV3Sha256Backend for RuntimeSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

impl ReplayV3HashBackend for RuntimeSha256 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
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

fn decode_binding(program_id: &Pubkey, account: &AccountInfo<'_>) -> Outcome<MarketBindingV4> {
    require_program_state(
        program_id,
        account,
        false,
        Some(contract::MARKET_BINDING_ACCOUNT_BYTES_V4),
    )?;
    let binding = MarketBindingV4::decode(&borrow_data(account)?)?;
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

fn authenticate_fee_distribution_collateral(
    program_id: &Pubkey,
    root: &AuthenticatedGeneralSettlementRootV1,
    accounts: &[AccountInfo<'_>],
) -> Outcome<BoundCollateralProfileV2> {
    let realm = crate::collateral_release::authenticate_realm_collateral_v2(
        program_id,
        &accounts[FEE_IX_REALM],
        &accounts[FEE_IX_PROFILE],
        &accounts[FEE_IX_POLICY],
        &accounts[FEE_IX_TOKEN],
    )?;
    let (binding, runtime) = authenticate_general_market_v4(
        program_id,
        &accounts[FEE_IX_BINDING],
        &accounts[FEE_IX_RUNTIME],
    )?;
    let base = binding.base().base();
    let instance = *authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        &accounts[FEE_IX_MARKET_INSTANCE],
        ContentId::from_bytes(base.market_instance_v2_id.bytes()),
    )?
    .value();
    let genesis = *authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        &accounts[FEE_IX_MARKET_GENESIS],
        ContentId::from_bytes(base.market_genesis_profile_v2_id.bytes()),
    )?
    .value();
    require(
        id(accounts[FEE_IX_BINDING].key) == root.root().market_binding()
            && base.market == root.root().market()
            && base.market_instance_v2_id == root.root().market_instance_v2_id()
            && runtime.market_instance_v2_id == base.market_instance_v2_id
            && instance
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
                == base.market_instance_v2_id.bytes()
            && instance.market_genesis_profile_id.content_id().bytes()
                == base.market_genesis_profile_v2_id.bytes()
            && genesis.realm_id.bytes() == realm.realm().realm.bytes()
            && genesis.profile_id.bytes() == realm.realm().profile.bytes()
            && genesis.capability_profile_id.bytes() == capabilities::PROFILE_ID,
        ClutchError::MismatchedState,
    )?;
    let market_bytes = base.market_instance_v2_id.bytes();
    refine_market_collateral_v2(
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
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))
}

fn authenticate_root(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    selector: CountedSettlementRootSelectorV1,
) -> Outcome<AuthenticatedGeneralSettlementRootV1> {
    require(selector.settlement_root == id(account.key), ClutchError::MismatchedState)?;
    authenticate_writable_general_settlement_root_epoch_v1(
        program_id,
        core::slice::from_ref(account),
        selector.epoch,
    )
}

fn authenticate_readonly_root(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    selector: CountedSettlementRootSelectorV1,
) -> Outcome<AuthenticatedGeneralSettlementRootV1> {
    require(selector.settlement_root == id(account.key), ClutchError::MismatchedState)?;
    authenticate_readonly_general_settlement_root_epoch_v1(
        program_id,
        core::slice::from_ref(account),
        selector.epoch,
    )
}

fn require_root_binding(
    root: &AuthenticatedGeneralSettlementRootV1,
    binding_account: &AccountInfo<'_>,
    binding: &MarketBindingV4,
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
            && payer.key != sink.key
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
    let payer_after = payer
        .lamports()
        .checked_add(rent.refundable_principal)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let sink_after = sink
        .lamports()
        .checked_add(donation)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
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
    set_lamports(sink, sink_after)
}

fn prepare_child_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selector: SettlementChildRetirementPayloadV1,
    exact_child_len: usize,
) -> Outcome<(AuthenticatedGeneralSettlementRootV1, MarketBindingV4)> {
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
            && semantic.state == expected_state
            && semantic.collected_fee_atoms == 0,
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
    require_count(accounts, FEE_FINALIZATION_CLOSE_ACCOUNT_COUNT_V1)?;
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            require(accounts[left].key != accounts[right].key, ClutchError::AccountAlias)?;
            right += 1;
        }
        left += 1;
    }
    require(selector.child == id(accounts[IX_CHILD].key), ClutchError::MismatchedState)?;
    require_program_state(
        program_id,
        &accounts[IX_CHILD],
        true,
        Some(contract::OWNER_FEE_FINALIZATION_ACCOUNT_BYTES_V4),
    )?;
    require_program_state(
        program_id,
        &accounts[IX_FEE_ACCUMULATOR],
        true,
        Some(contract::FEE_RETIREMENT_ACCOUNT_BYTES_V1),
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
    let accumulator_bytes = borrow_data(&accounts[IX_FEE_ACCUMULATOR])?;
    let accumulator = FeeRetirementAccumulatorV1AccountV1::decode(&accumulator_bytes)?;
    accumulator.rent.validate()?;
    let accumulator_pda = seeds::general_v2_fee_retirement_accumulator_pda(
        program_id,
        &root.root().fee_record().bytes(),
    );
    let runtime_release = fee_runtime_semantic_release_id_v1(&RuntimeSha256)?;
    require(
        *accounts[IX_FEE_ACCUMULATOR].key == accumulator_pda.0
            && accumulator.stored_bump == accumulator_pda.1
            && accumulator.semantic.runtime_program().0 == program_id.to_bytes()
            && accumulator.semantic.runtime_release().0 == runtime_release.bytes()
            && accumulator.semantic.settlement_root().0 == root.account().bytes()
            && accumulator.semantic.selected_feed_data_id().0
                == root.selected_feed_data_id()?.bytes()
            && accumulator.semantic.fee_record().0 == root.root().fee_record().bytes()
            && accumulator.semantic.settlement_candidate().0
                == root.root().settlement_candidate_id().bytes()
            && accumulator.semantic.owner_order_set_digest().0
                == root.root().owner_order_set_digest().bytes()
            && accumulator.semantic.settlement_cash_pot().0
                == root.root().settlement_cash_pot().bytes()
            && accumulator.rent.payer != id(accounts[IX_FEE_ACCUMULATOR].key)
            && accounts[IX_FEE_ACCUMULATOR].lamports()
                >= accumulator
                    .rent
                    .refundable_principal
                    .checked_add(accumulator.rent.donation_floor)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::MismatchedState,
    )?;
    let (payer_after, sink_after) = checked_close_balances(
        &accounts[IX_CHILD], &accounts[IX_PAYER], &accounts[IX_SINK], finalization.rent,
    )?;
    let close_receipt = FeeId(solana_sha256_hasher::hashv(&[
        OWNER_FEE_CLOSE_RECEIPT_DOMAIN_V1,
        &accounts[IX_FEE_ACCUMULATOR].key.to_bytes(),
        &accounts[IX_CHILD].key.to_bytes(),
        &*bytes,
        &accounts[IX_CHILD].lamports().to_le_bytes(),
        &accounts[IX_PAYER].key.to_bytes(),
        &accounts[IX_SINK].key.to_bytes(),
    ]).to_bytes());
    let closure = ExternalFeeAccountClosureV1::admit(
        CandidateFeeAccountRoleV1::OwnerFinalization,
        FeeTerminalOutcomeV1::Settled,
        FeeId(program_id.to_bytes()),
        FeeId(runtime_release.bytes()),
        terminal.fee_record,
        FeeId(accounts[IX_CHILD].key.to_bytes()),
        terminal.owner,
        close_receipt,
        FeeId(finalization.rent.payer.bytes()),
        FeeId(accounts[IX_SINK].key.to_bytes()),
        accounts[IX_CHILD].lamports(),
        finalization.rent.refundable_principal,
        accounts[IX_CHILD]
            .lamports()
            .checked_sub(finalization.rent.refundable_principal)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let successor = accumulator
        .semantic
        .fold_owner(
            &clutch_fee_runtime_contract::terminal::AuthenticatedOwnerFeeFinalizationV1 {
                carry_account: FeeId(accounts[IX_CHILD].key.to_bytes()),
                receipt: finalization.semantic,
            },
            &closure,
            &RuntimeSha256,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut root_output = std::vec![0u8; root.account_bytes()];
    root.encode_fee_finalization_retirement_successor(&mut root_output)?;
    let mut accumulator_output = [0u8; contract::FEE_RETIREMENT_ACCOUNT_BYTES_V1];
    FeeRetirementAccumulatorV1AccountV1 {
        semantic: successor,
        rent: accumulator.rent,
        stored_bump: accumulator.stored_bump,
    }
    .encode(&mut accumulator_output)?;
    drop(bytes);
    drop(accumulator_bytes);
    for account in [
        &accounts[IX_ROOT],
        &accounts[IX_CHILD],
        &accounts[IX_PAYER],
        &accounts[IX_SINK],
        &accounts[IX_FEE_ACCUMULATOR],
    ] {
        let data = account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        drop(data);
        let lamports = account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        drop(lamports);
    }
    borrow_mut_data(&accounts[IX_ROOT])?.copy_from_slice(&root_output);
    borrow_mut_data(&accounts[IX_FEE_ACCUMULATOR])?.copy_from_slice(&accumulator_output);
    close_program_account(&accounts[IX_CHILD])?;
    set_lamports(&accounts[IX_PAYER], payer_after)?;
    set_lamports(&accounts[IX_SINK], sink_after)
}

#[inline(never)]
fn distribute_maker_fee(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selector: FeeMakerDistributionPayloadV1,
) -> Outcome<()> {
    require_count(accounts, FEE_MAKER_DISTRIBUTION_ACCOUNT_COUNT_V1)?;
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            require(accounts[left].key != accounts[right].key, ClutchError::AccountAlias)?;
            right += 1;
        }
        left += 1;
    }
    let root = authenticate_readonly_root(
        program_id,
        &accounts[FEE_IX_ROOT],
        CountedSettlementRootSelectorV1 {
            epoch: selector.epoch,
            settlement_root: selector.settlement_root,
        },
    )?;
    require(
        root.root().phase() == contract::SettlementRootPhaseV1::Retiring
            && root.root().fee_record_state() == contract::SettlementRootChildStateV1::Live
            && selector.fee_record == root.root().fee_record()
            && selector.accumulator == id(accounts[FEE_IX_ACCUMULATOR].key)
            && selector.recipient_allocation == id(accounts[FEE_IX_RECIPIENT].key)
            && selector.maker_position == id(accounts[FEE_IX_POSITION].key),
        ClutchError::MismatchedState,
    )?;
    require_program_state(
        program_id,
        &accounts[FEE_IX_ACCUMULATOR],
        true,
        Some(contract::FEE_RETIREMENT_ACCOUNT_BYTES_V1),
    )?;
    require_program_state(
        program_id,
        &accounts[FEE_IX_RECIPIENT],
        false,
        Some(contract::RECIPIENT_ALLOCATION_ACCOUNT_BYTES_V2),
    )?;
    require_program_state(
        program_id,
        &accounts[FEE_IX_CASH_POT],
        true,
        Some(contract::SETTLEMENT_CASH_POT_ACCOUNT_BYTES),
    )?;
    let accumulator_data = borrow_data(&accounts[FEE_IX_ACCUMULATOR])?;
    let accumulator = FeeRetirementAccumulatorV1AccountV1::decode(&accumulator_data)?;
    accumulator.rent.validate()?;
    let accumulator_pda = seeds::general_v2_fee_retirement_accumulator_pda(
        program_id,
        &root.root().fee_record().bytes(),
    );
    let runtime_release = fee_runtime_semantic_release_id_v1(&RuntimeSha256)?;
    require(
        *accounts[FEE_IX_ACCUMULATOR].key == accumulator_pda.0
            && accumulator.stored_bump == accumulator_pda.1
            && accumulator.semantic.runtime_program().0 == program_id.to_bytes()
            && accumulator.semantic.runtime_release().0 == runtime_release.bytes()
            && accumulator.semantic.settlement_root().0 == root.account().bytes()
            && accumulator.semantic.selected_feed_data_id().0
                == root.selected_feed_data_id()?.bytes()
            && accumulator.semantic.fee_record().0 == selector.fee_record.bytes()
            && accumulator.semantic.recipient_allocation().0
                == selector.recipient_allocation.bytes()
            && accumulator.semantic.settlement_cash_pot().0
                == accounts[FEE_IX_CASH_POT].key.to_bytes()
            && accumulator.semantic.processed_owner_count()
                == accumulator.semantic.expected_owner_count()
            && accumulator.semantic.processed_maker_count() == selector.maker_ordinal,
        ClutchError::MismatchedState,
    )?;
    let recipient_data = borrow_data(&accounts[FEE_IX_RECIPIENT])?;
    let recipient = RecipientAllocationV2AccountV1::decode_persisted(&recipient_data)?;
    recipient.rent.validate()?;
    let recipient_data_id = contract::recipient_allocation_account_data_id_v2(
        &recipient_data,
        &RuntimeSha256,
    )?;
    let recipient_pda = seeds::general_v2_recipient_allocation_pda(
        program_id,
        &root.root().fee_record().bytes(),
    );
    let maker_index = usize::from(selector.maker_ordinal);
    require(
        *accounts[FEE_IX_RECIPIENT].key == recipient_pda.0
            && recipient.stored_bump == recipient_pda.1
            && recipient.semantic.owner_fee_book_data_id()
                == accumulator.semantic.owner_fee_book_data_id()
            && recipient.semantic.owner_order_set_digest()
                == accumulator.semantic.owner_order_set_digest()
            && recipient.semantic.allocation().fee_record().0 == selector.fee_record.bytes()
            && maker_index < usize::from(recipient.semantic.allocation().maker_len())
            && recipient.semantic.allocation().maker_positions()[maker_index].0
                == selector.maker_position.bytes()
            && recipient_data_id.bytes()
                == accumulator.semantic.recipient_allocation_data_id().0
            && accounts[FEE_IX_RECIPIENT].lamports()
                >= recipient
                    .rent
                    .refundable_principal
                    .checked_add(recipient.rent.donation_floor)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::MismatchedState,
    )?;
    let pot_data = borrow_data(&accounts[FEE_IX_CASH_POT])?;
    let pot = SettlementCashPotV1AccountV1::decode(&pot_data)?;
    let pot_pda = seeds::general_v2_settlement_cash_pot_pda(
        program_id,
        &root.root().epoch().bytes(),
        &root.root().settlement_candidate_id().bytes(),
    );
    require(
        *accounts[FEE_IX_CASH_POT].key == pot_pda.0
            && pot.stored_bump == pot_pda.1
            && pot.semantic.expectation == root.root().cash_pot_expectation()?,
        ClutchError::MismatchedState,
    )?;
    let bound = authenticate_fee_distribution_collateral(program_id, &root, accounts)?;
    let position_data = borrow_data(&accounts[FEE_IX_POSITION])?;
    let position = PositionAccountV3::decode(&position_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_owner = position.owner().bytes();
    drop(position_data);
    let position_replay = authenticate_current_general_position_replay_v2(
        program_id,
        bound,
        &accounts[FEE_IX_BINDING],
        &accounts[FEE_IX_RUNTIME],
        &accounts[FEE_IX_POSITION],
        &accounts[FEE_IX_REPLAY],
        position_owner,
    )?;
    let credited_atoms = recipient.semantic.allocation().maker_rebate_atoms()[maker_index];
    let plan = contract::prepare_fee_position_credit_v1(
        selector.fee_record,
        recipient_data_id,
        id(accounts[FEE_IX_CASH_POT].key),
        1,
        selector.maker_ordinal,
        credited_atoms,
        position_replay.replay,
        pot.semantic,
        &RuntimeSha256,
    )?;
    let accumulator_successor = accumulator
        .semantic
        .fold_maker_distribution(&recipient.semantic, plan.semantic(), &RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut accumulator_output = [0u8; contract::FEE_RETIREMENT_ACCOUNT_BYTES_V1];
    FeeRetirementAccumulatorV1AccountV1 {
        semantic: accumulator_successor,
        rent: accumulator.rent,
        stored_bump: accumulator.stored_bump,
    }
    .encode(&mut accumulator_output)?;
    let mut pot_output = [0u8; contract::SETTLEMENT_CASH_POT_ACCOUNT_BYTES];
    SettlementCashPotV1AccountV1 {
        semantic: plan.cash_pot(),
        stored_bump: pot.stored_bump,
        flags: 0,
    }
    .encode(&mut pot_output)?;
    let position_output = plan
        .position()
        .semantic
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(accumulator_data);
    drop(recipient_data);
    drop(pot_data);
    borrow_mut_data(&accounts[FEE_IX_ACCUMULATOR])?.copy_from_slice(&accumulator_output);
    borrow_mut_data(&accounts[FEE_IX_CASH_POT])?.copy_from_slice(&pot_output);
    if let Some(replay) = plan.replay() {
        borrow_mut_data(&accounts[FEE_IX_POSITION])?.copy_from_slice(&position_output);
        borrow_mut_data(&accounts[FEE_IX_REPLAY])?
            .copy_from_slice(replay.replay_poststate_body());
    }
    Ok(())
}

#[inline(never)]
fn retire_fee_record(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selector: SettlementChildRetirementPayloadV1,
) -> Outcome<()> {
    require_count(accounts, FEE_RECORD_RETIREMENT_ACCOUNT_COUNT_V1)?;
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
    let record = &accounts[IX_CHILD];
    let canonical = seeds::general_v2_selected_fee_record_pda(
        program_id,
        &root.root().settlement_candidate_id().bytes(),
    );
    require(
        selector.child == root.root().fee_record()
            && selector.child == id(record.key)
            && *record.key == canonical.0
            && *record.owner == SYSTEM_PROGRAM_ID
            && record.data_len() == 0
            && record.lamports() == 0
            && !record.is_writable
            && !record.is_signer
            && !record.executable,
        ClutchError::MismatchedState,
    )?;
    let mut root_output = std::vec![0u8; root.account_bytes()];
    root.encode_fee_record_retirement_successor(&mut root_output)?;
    borrow_mut_data(&accounts[IX_ROOT])?.copy_from_slice(&root_output);
    Ok(())
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
    Ok(())
}

/// Dispatch-compatible entrypoint for counted settlement child retirement.
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
        capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    if action == GeneralV2Action::AdvanceFeeRetirement {
        return match decode_fee_retirement_payload_v1(action.tag(), payload)? {
            FeeRetirementPayloadV1::MakerDistribution(value) => {
                distribute_maker_fee(program_id, accounts, value)
            }
            FeeRetirementPayloadV1::FinalizeTreasuryAndGlobals(_) => {
                Err(Refusal::Adapter(ClutchError::UnsupportedInstruction))
            }
        };
    }
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
        assert_eq!(FEE_FINALIZATION_CLOSE_ACCOUNT_COUNT_V1, 6);
        assert_eq!(FEE_RECORD_RETIREMENT_ACCOUNT_COUNT_V1, 3);
        assert_eq!(BEGIN_RETIRING_ACCOUNT_COUNT_V1, 2);
    }
}
