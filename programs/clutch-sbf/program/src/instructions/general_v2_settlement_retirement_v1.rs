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
use clutch_batch_policy_identity::revenue_policy_v2::{
    encode_revenue_policy_v2, REVENUE_POLICY_V2_BYTES,
};
use clutch_fee_runtime_contract::projection::SelectedOwnerFeeBookHashV1;
use clutch_fee_runtime_contract::codec::CertifiedRecipientAllocationAccessV3;
use clutch_fee_runtime_contract::integration::CandidateFeeSettlementV1;
use clutch_fee_runtime_contract::intent::{
    FeeRecordAccountIdV1, RecipientAllocationAccountIdV1, RecipientAllocationIntentV1,
    TreasuryLedgerAccountIdV1,
};
use clutch_fee_runtime_contract::retirement::FeeRetirementHashV1;
use clutch_fee_runtime_contract::terminal::{
    build_settled_fee_terminal_from_accumulator_v2, CandidateFeeAccountClosuresV1,
    CandidateFeeAccountRoleV1, ExternalFeeAccountClosureV1, FeeTerminalOutcomeV1,
};
use clutch_fee_runtime_contract::{Id as FeeId, OwnerFeeFinalizationOutcomeV2};
use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    decode_fee_retirement_payload_v1, decode_settlement_retirement_payload_v1,
    CountedSettlementRootSelectorV1, FeeFinalizeGlobalsPayloadV1,
    FeeMakerDistributionPayloadV1,
    FeeRetirementPayloadV1,
    fee_runtime_semantic_release_id_v2, DeletableRentOwnerV1,
    FeeClosureManifestV2AccountV1, FeeRecordTerminalV3AccountV1,
    FeeRetirementAccumulatorV1AccountV1, GeneralEpochPhaseV1,
    GeneralEpochV6AccountV1, Id32, MarketBindingV4, SelectedFeeRecordV2AccountV1,
    SettlementCashPotV1AccountV1,
    SettlementChildRetirementPayloadV1,
    SettlementRetirementPayloadKindV1, TreasuryLedgerV2AccountV1,
};
use clutch_product_series::{ContentId, MarketGenesisProfileV2, MarketInstancePreimageV2};
use clutch_retirement::{PositionAccountV3, PositionV3Sha256Backend, ReplayV3HashBackend};
use clutch_solana_layout::registry::GeneralV2Action;
use clutch_solana_layout::Hash32;
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

use crate::accounts::{require, require_count, require_signer, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;
use crate::seeds;

use super::genesis::{read_rent, require_system_program};
use super::general_v2_settlement_producer_v5::{create_from_payer, rent_owner};
use super::general_v2_fee_terminal_pair_v1::{
    authenticate_fee_terminal_pair_v1, FeeTerminalPairExpectationV1,
};
use super::revenue_policy_v2::{
    accept_treasury_service_transition_v1, authenticate_revenue_policy_record_v2,
    authenticate_treasury_service_ledger_v1, derive_revenue_market_treasury_v1,
    prepare_treasury_service_settlement_v1, AuthenticatedTreasuryServiceAdmissionV1,
    AuthenticatedTreasuryServiceSettlementV1, RevenueMarketTreasuryDerivationV1,
};

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
/// Action-50 final treasury credit, service settlement, four temporary closes,
/// and creation of the durable b9/v2+v3 pair.
pub const FEE_FINALIZE_GLOBALS_ACCOUNT_COUNT_V1: usize = 26;

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

const FINAL_IX_EPOCH: usize = 14;
const FINAL_IX_SELECTED: usize = 15;
const FINAL_IX_TREASURY: usize = 16;
const FINAL_IX_SERVICE: usize = 17;
const FINAL_IX_REVENUE_RECORD: usize = 18;
const FINAL_IX_MANIFEST: usize = 19;
const FINAL_IX_TERMINAL: usize = 20;
const FINAL_IX_CREATION_PAYER: usize = 21;
const FINAL_IX_REFUND_PAYER: usize = 22;
const FINAL_IX_NEUTRAL_SINK: usize = 23;
const FINAL_IX_SYSTEM_PROGRAM: usize = 24;
const FINAL_IX_RENT_SYSVAR: usize = 25;

const OWNER_FEE_CLOSE_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-owner-fee-close/v1\0";
const GLOBAL_FEE_CLOSE_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-global-fee-close/v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TreasuryServiceSettlementEvidenceV1 {
    realm: Hash32,
    market_instance_v2_id: Hash32,
    revenue_policy_record_account: Pubkey,
    revenue_policy_record_v2_id: Hash32,
    revenue_policy_v2_digest: Hash32,
    treasury_owner: Hash32,
    treasury_position_account: Pubkey,
    treasury_service_ledger_account: Pubkey,
    epoch_semantic_id: Hash32,
    admitted_epoch_count_before: u64,
    settled_epoch_count_before: u64,
}

impl AuthenticatedTreasuryServiceAdmissionV1 for TreasuryServiceSettlementEvidenceV1 {
    fn realm(&self) -> Option<Hash32> { Some(self.realm) }
    fn market_instance_v2_id(&self) -> Option<Hash32> { Some(self.market_instance_v2_id) }
    fn revenue_policy_record_account(&self) -> Option<Pubkey> {
        Some(self.revenue_policy_record_account)
    }
    fn revenue_policy_record_v2_id(&self) -> Option<Hash32> {
        Some(self.revenue_policy_record_v2_id)
    }
    fn revenue_policy_v2_digest(&self) -> Option<Hash32> {
        Some(self.revenue_policy_v2_digest)
    }
    fn treasury_owner(&self) -> Option<Hash32> { Some(self.treasury_owner) }
    fn treasury_position_account(&self) -> Option<Pubkey> {
        Some(self.treasury_position_account)
    }
    fn treasury_service_ledger_account(&self) -> Option<Pubkey> {
        Some(self.treasury_service_ledger_account)
    }
    fn epoch_semantic_id(&self) -> Option<Hash32> { Some(self.epoch_semantic_id) }
    fn admitted_epoch_count_before(&self) -> Option<u64> {
        Some(self.admitted_epoch_count_before)
    }
    fn settled_epoch_count_before(&self) -> Option<u64> {
        Some(self.settled_epoch_count_before)
    }
}

impl AuthenticatedTreasuryServiceSettlementV1 for TreasuryServiceSettlementEvidenceV1 {
    fn service_is_terminal(&self) -> Option<bool> { Some(true) }
}

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

fn decode_binding(program_id: &Pubkey, account: &AccountInfo<'_>) -> Outcome<Box<MarketBindingV4>> {
    require_program_state(
        program_id,
        account,
        false,
        Some(contract::MARKET_BINDING_ACCOUNT_BYTES_V4),
    )?;
    let binding = Box::new(MarketBindingV4::decode(&borrow_data(account)?)?);
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
            && binding.base().batch_policy_id() == root.root().batch_policy_id()
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
    let (payer_after, sink_after) = close_destination_balances(
        payer.lamports(),
        sink.lamports(),
        rent.refundable_principal,
        donation,
        payer.key == sink.key,
    )
    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok((payer_after, sink_after))
}

fn close_destination_balances(
    payer_before: u64,
    sink_before: u64,
    principal: u64,
    donation: u64,
    coalesced: bool,
) -> Option<(u64, u64)> {
    if coalesced {
        if payer_before != sink_before {
            return None;
        }
        let after = payer_before.checked_add(principal)?.checked_add(donation)?;
        Some((after, after))
    } else {
        Some((
            payer_before.checked_add(principal)?,
            sink_before.checked_add(donation)?,
        ))
    }
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

fn admit_global_fee_close(
    program_id: &Pubkey,
    runtime_release: Id32,
    fee_record: Id32,
    role: CandidateFeeAccountRoleV1,
    account: &AccountInfo<'_>,
    account_bytes: &[u8],
    rent: DeletableRentOwnerV1,
    payer: &AccountInfo<'_>,
    sink: &AccountInfo<'_>,
) -> Outcome<ExternalFeeAccountClosureV1> {
    rent.validate()?;
    require(
        rent.payer == id(payer.key)
            && account.key != payer.key
            && account.key != sink.key,
        ClutchError::AccountAlias,
    )?;
    let required = rent
        .refundable_principal
        .checked_add(rent.donation_floor)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(account.lamports() >= required, ClutchError::MismatchedState)?;
    let donation = account
        .lamports()
        .checked_sub(rent.refundable_principal)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let close_receipt = FeeId(
        solana_sha256_hasher::hashv(&[
            GLOBAL_FEE_CLOSE_RECEIPT_DOMAIN_V1,
            &[role as u8],
            &account.key.to_bytes(),
            account_bytes,
            &account.lamports().to_le_bytes(),
            &payer.key.to_bytes(),
            &sink.key.to_bytes(),
        ])
        .to_bytes(),
    );
    ExternalFeeAccountClosureV1::admit(
        role,
        FeeTerminalOutcomeV1::Settled,
        FeeId(program_id.to_bytes()),
        FeeId(runtime_release.bytes()),
        FeeId(fee_record.bytes()),
        FeeId(account.key.to_bytes()),
        FeeId([0; 32]),
        close_receipt,
        FeeId(payer.key.to_bytes()),
        FeeId(sink.key.to_bytes()),
        account.lamports(),
        rent.refundable_principal,
        donation,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

fn aggregate_global_close_balances(
    closures: &[ExternalFeeAccountClosureV1; 4],
    payer: &AccountInfo<'_>,
    sink: &AccountInfo<'_>,
) -> Outcome<(u64, u64)> {
    require_destination(payer)?;
    require_destination(sink)?;
    let mut principal = 0u64;
    let mut donation = 0u64;
    for closure in closures {
        require(
            closure.rent_payer().0 == payer.key.to_bytes()
                && closure.neutral_sink().0 == sink.key.to_bytes(),
            ClutchError::MismatchedState,
        )?;
        principal = principal
            .checked_add(closure.rent_refund_lamports())
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        donation = donation
            .checked_add(closure.neutral_credit_lamports())
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }
    close_destination_balances(
        payer.lamports(),
        sink.lamports(),
        principal,
        donation,
        payer.key == sink.key,
    )
    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))
}

fn require_final_fee_account_aliases(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let destination = |index: usize| {
                matches!(
                    index,
                    FINAL_IX_CREATION_PAYER | FINAL_IX_REFUND_PAYER | FINAL_IX_NEUTRAL_SINK
                )
            };
            let permitted_destination_alias = destination(left) && destination(right);
            require(
                permitted_destination_alias || accounts[left].key != accounts[right].key,
                ClutchError::AccountAlias,
            )?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
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
        require(payer_after == sink_after, ClutchError::MismatchedState)
    } else {
        set_lamports(sink, sink_after)
    }
}

fn prepare_child_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selector: SettlementChildRetirementPayloadV1,
    exact_child_len: usize,
) -> Outcome<(AuthenticatedGeneralSettlementRootV1, Box<MarketBindingV4>)> {
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
    require_root_binding(&root, &accounts[IX_BINDING], binding.as_ref())?;
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
    require(
        accounts[IX_ROOT].key != accounts[IX_CHILD].key
            && accounts[IX_ROOT].key != accounts[IX_BINDING].key
            && accounts[IX_ROOT].key != accounts[IX_FEE_ACCUMULATOR].key
            && accounts[IX_CHILD].key != accounts[IX_BINDING].key
            && accounts[IX_CHILD].key != accounts[IX_FEE_ACCUMULATOR].key
            && accounts[IX_BINDING].key != accounts[IX_FEE_ACCUMULATOR].key
            && accounts[IX_ROOT].key != accounts[IX_PAYER].key
            && accounts[IX_ROOT].key != accounts[IX_SINK].key
            && accounts[IX_FEE_ACCUMULATOR].key != accounts[IX_PAYER].key
            && accounts[IX_FEE_ACCUMULATOR].key != accounts[IX_SINK].key,
        ClutchError::AccountAlias,
    )?;
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
    let runtime_release = fee_runtime_semantic_release_id_v2(&RuntimeSha256)?;
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
    if accounts[IX_PAYER].key == accounts[IX_SINK].key {
        require(payer_after == sink_after, ClutchError::MismatchedState)
    } else {
        set_lamports(&accounts[IX_SINK], sink_after)
    }
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
        Some(contract::RECIPIENT_ALLOCATION_ACCOUNT_BYTES_V3),
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
    let runtime_release = fee_runtime_semantic_release_id_v2(&RuntimeSha256)?;
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
    let recipient = contract::decode_borrowed_recipient_allocation_v3_account(
        &recipient_data,
    )?;
    recipient.rent().validate()?;
    let recipient_data_id = contract::recipient_allocation_account_data_id_v3(
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
            && recipient.stored_bump() == recipient_pda.1
            && recipient.semantic().owner_order_set_digest()
                == accumulator.semantic.owner_order_set_digest()
            && recipient.semantic().fee_record().0 == selector.fee_record.bytes()
            && maker_index < usize::from(recipient.semantic().row_count())
            && recipient
                .semantic()
                .row(selector.maker_ordinal)?
                .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?
                .position()
                .0 == selector.maker_position.bytes()
            && recipient_data_id.bytes()
                == accumulator.semantic.recipient_allocation_data_id().0
            && accounts[FEE_IX_RECIPIENT].lamports()
                >= recipient
                    .rent()
                    .refundable_principal
                    .checked_add(recipient.rent().donation_floor)
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
    let credited_atoms = recipient
        .semantic()
        .row(selector.maker_ordinal)?
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?
        .rebate_atoms();
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
        .fold_maker_distribution(&recipient.semantic(), plan.semantic(), &RuntimeSha256)
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
fn finalize_treasury_and_fee_globals(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selector: FeeFinalizeGlobalsPayloadV1,
) -> Outcome<()> {
    require_count(accounts, FEE_FINALIZE_GLOBALS_ACCOUNT_COUNT_V1)?;
    require_final_fee_account_aliases(accounts)?;
    require_signer(&accounts[FINAL_IX_CREATION_PAYER])?;
    require_destination(&accounts[FINAL_IX_CREATION_PAYER])?;
    require_system_program(&accounts[FINAL_IX_SYSTEM_PROGRAM])?;
    let rent_parameters = read_rent(&accounts[FINAL_IX_RENT_SYSVAR])?;

    let root = authenticate_root(
        program_id,
        &accounts[FEE_IX_ROOT],
        CountedSettlementRootSelectorV1 {
            epoch: selector.epoch,
            settlement_root: selector.settlement_root,
        },
    )?;
    require(
        root.is_indexed()
            && root.root().phase() == contract::SettlementRootPhaseV1::Retiring
            && root.root().fee_record_state() == contract::SettlementRootChildStateV1::Live
            && root.root().cash_pot_state() == contract::SettlementRootChildStateV1::Live
            && selector.fee_record == root.root().fee_record()
            && selector.accumulator == id(accounts[FEE_IX_ACCUMULATOR].key)
            && selector.recipient_allocation == id(accounts[FEE_IX_RECIPIENT].key)
            && selector.treasury_ledger == id(accounts[FINAL_IX_TREASURY].key)
            && selector.settlement_cash_pot == id(accounts[FEE_IX_CASH_POT].key)
            && selector.terminal_receipt == id(accounts[FINAL_IX_TERMINAL].key)
            && selector.closure_manifest == id(accounts[FINAL_IX_MANIFEST].key),
        ClutchError::MismatchedState,
    )?;
    let binding = decode_binding(program_id, &accounts[FEE_IX_BINDING])?;
    require_root_binding(&root, &accounts[FEE_IX_BINDING], &binding)?;
    require(
        binding.base().base().neutral_sink == id(accounts[FINAL_IX_NEUTRAL_SINK].key),
        ClutchError::MismatchedState,
    )?;
    let bound = authenticate_fee_distribution_collateral(program_id, &root, accounts)?;

    require_program_state(
        program_id,
        &accounts[FINAL_IX_EPOCH],
        false,
        Some(contract::GENERAL_EPOCH_ACCOUNT_BYTES),
    )?;
    let epoch = GeneralEpochV6AccountV1::decode(&borrow_data(&accounts[FINAL_IX_EPOCH])?)?;
    let epoch_pda = seeds::general_v2_epoch_pda(
        program_id,
        &accounts[FEE_IX_BINDING].key.to_bytes(),
        epoch.epoch_index,
    );
    require(
        selector.epoch == id(accounts[FINAL_IX_EPOCH].key)
            && *accounts[FINAL_IX_EPOCH].key == epoch_pda.0
            && epoch.stored_bump == epoch_pda.1
            && epoch.phase == GeneralEpochPhaseV1::Finalized
            && epoch.market_binding == id(accounts[FEE_IX_BINDING].key)
            && epoch.market_runtime == id(accounts[FEE_IX_RUNTIME].key)
            && epoch.market_instance_v2_id == root.root().market_instance_v2_id(),
        ClutchError::MismatchedState,
    )?;
    let epoch_semantic_id = epoch.semantics_digest(&RuntimeSha256)?;

    require_program_state(
        program_id,
        &accounts[FINAL_IX_SELECTED],
        true,
        Some(contract::SELECTED_FEE_RECORD_ACCOUNT_BYTES_V2),
    )?;
    require_program_state(
        program_id,
        &accounts[FEE_IX_RECIPIENT],
        true,
        Some(contract::RECIPIENT_ALLOCATION_ACCOUNT_BYTES_V3),
    )?;
    require_program_state(
        program_id,
        &accounts[FINAL_IX_TREASURY],
        true,
        Some(contract::TREASURY_LEDGER_ACCOUNT_BYTES_V2),
    )?;
    require_program_state(
        program_id,
        &accounts[FEE_IX_ACCUMULATOR],
        true,
        Some(contract::FEE_RETIREMENT_ACCOUNT_BYTES_V1),
    )?;
    require_program_state(
        program_id,
        &accounts[FEE_IX_CASH_POT],
        true,
        Some(contract::SETTLEMENT_CASH_POT_ACCOUNT_BYTES),
    )?;

    let selected_pda = seeds::general_v2_selected_fee_record_pda(
        program_id,
        &root.root().settlement_candidate_id().bytes(),
    );
    let recipient_pda = seeds::general_v2_recipient_allocation_pda(
        program_id,
        &selector.fee_record.bytes(),
    );
    let treasury_pda = seeds::general_v2_treasury_ledger_pda(
        program_id,
        &selector.fee_record.bytes(),
    );
    let accumulator_pda = seeds::general_v2_fee_retirement_accumulator_pda(
        program_id,
        &selector.fee_record.bytes(),
    );
    require(
        id(accounts[FINAL_IX_SELECTED].key) == selector.fee_record
            && *accounts[FINAL_IX_SELECTED].key == selected_pda.0
            && *accounts[FEE_IX_RECIPIENT].key == recipient_pda.0
            && *accounts[FINAL_IX_TREASURY].key == treasury_pda.0
            && *accounts[FEE_IX_ACCUMULATOR].key == accumulator_pda.0,
        ClutchError::WrongPda,
    )?;

    let selected_data = borrow_data(&accounts[FINAL_IX_SELECTED])?;
    let selected = Box::new(SelectedFeeRecordV2AccountV1::decode_persisted(&selected_data)?);
    let recipient_data = borrow_data(&accounts[FEE_IX_RECIPIENT])?;
    let recipient = contract::decode_borrowed_recipient_allocation_v3_account(&recipient_data)?;
    let recipient_data_id = contract::recipient_allocation_account_data_id_v3(
        &recipient_data,
        &RuntimeSha256,
    )?;
    let treasury_data = borrow_data(&accounts[FINAL_IX_TREASURY])?;
    let treasury = Box::new(TreasuryLedgerV2AccountV1::decode(
        &treasury_data,
        &selected.semantic,
    )?);
    let accumulator_data = borrow_data(&accounts[FEE_IX_ACCUMULATOR])?;
    let accumulator = Box::new(FeeRetirementAccumulatorV1AccountV1::decode(&accumulator_data)?);
    let pot_data = borrow_data(&accounts[FEE_IX_CASH_POT])?;
    let pot = SettlementCashPotV1AccountV1::decode(&pot_data)?;
    for rent in [selected.rent, recipient.rent(), treasury.rent, accumulator.rent] {
        rent.validate()?;
        require(
            rent.payer == id(accounts[FINAL_IX_REFUND_PAYER].key),
            ClutchError::MismatchedState,
        )?;
    }
    let runtime_release = fee_runtime_semantic_release_id_v2(&RuntimeSha256)?;
    let current = binding.authority();
    require(
        selected.stored_bump == selected_pda.1
            && recipient.stored_bump() == recipient_pda.1
            && treasury.stored_bump == treasury_pda.1
            && accumulator.stored_bump == accumulator_pda.1
            && selected.semantic.fee_record().0 == selector.fee_record.bytes()
            && selected.semantic.realm().0 == accounts[FEE_IX_REALM].key.to_bytes()
            && selected.semantic.market().0 == root.root().market().bytes()
            && selected.semantic.epoch().0 == selector.epoch.bytes()
            && selected.semantic.selected_candidate().0
                == root.root().settlement_candidate_id().bytes()
            && selected.semantic.batch_policy().0 == binding.base().batch_policy_id().bytes()
            && selected.semantic.revenue_policy().0 == current.revenue_policy_v2_digest().bytes()
            && selected.semantic.treasury_owner().0 == current.treasury_owner().bytes()
            && selected.semantic.treasury_position().0
                == current.treasury_position_account().bytes()
            && selected.semantic.price_scale() == binding.base().base().price_scale
            && selected.semantic.outcome_count() == binding.base().base().outcome_count
            && recipient.semantic().fee_record().0 == selector.fee_record.bytes()
            && recipient_data_id.bytes()
                == accumulator.semantic.recipient_allocation_data_id().0
            && treasury.semantic.fee_record() == selected.semantic.fee_record()
            && accumulator.semantic.runtime_program().0 == program_id.to_bytes()
            && accumulator.semantic.runtime_release().0 == runtime_release.bytes()
            && accumulator.semantic.settlement_root().0 == root.account().bytes()
            && accumulator.semantic.selected_feed_data_id().0
                == root.selected_feed_data_id()?.bytes()
            && accumulator.semantic.fee_record().0 == selector.fee_record.bytes()
            && accumulator.semantic.settlement_candidate().0
                == root.root().settlement_candidate_id().bytes()
            && accumulator.semantic.recipient_allocation().0
                == accounts[FEE_IX_RECIPIENT].key.to_bytes()
            && accumulator.semantic.treasury_ledger().0
                == accounts[FINAL_IX_TREASURY].key.to_bytes()
            && accumulator.semantic.settlement_cash_pot().0
                == accounts[FEE_IX_CASH_POT].key.to_bytes()
            && accumulator.semantic.processed_owner_count()
                == accumulator.semantic.expected_owner_count()
            && accumulator.semantic.processed_maker_count()
                == accumulator.semantic.expected_maker_count()
            && recipient.semantic().nonzero_weight_row_count()
                == accumulator.semantic.expected_owner_count()
            && recipient.semantic().row_count()
                == accumulator.semantic.expected_maker_count()
            && recipient.semantic().executor_atoms() == 0
            && pot.semantic.expectation == root.root().cash_pot_expectation()?
            && pot.semantic.collected_fee_atoms == recipient.semantic().treasury_atoms(),
        ClutchError::MismatchedState,
    )?;
    selected
        .semantic
        .binds_revenue_policy(&selector.revenue_policy)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let mut revenue_policy_bytes = [0u8; REVENUE_POLICY_V2_BYTES];
    encode_revenue_policy_v2(&selector.revenue_policy, &mut revenue_policy_bytes)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let revenue_authority = authenticate_revenue_policy_record_v2(
        program_id,
        &accounts[FEE_IX_REALM],
        &accounts[FINAL_IX_REVENUE_RECORD],
        &revenue_policy_bytes,
    )?;
    require(
        current.revenue_policy_record_account().bytes()
                == revenue_authority.record_account().to_bytes()
            && current.revenue_policy_record_v2_id().bytes()
                == revenue_authority.record_semantic_id().bytes()
            && current.revenue_policy_v2_digest().bytes()
                == revenue_authority.policy_digest().bytes()
            && current.treasury_owner().bytes() == revenue_authority.treasury_owner().bytes()
            && current.treasury_position_derivation_policy_v2_id().bytes()
                == revenue_authority.treasury_position_derivation_policy_id().bytes()
            && current.treasury_service_ledger_account().bytes()
                == accounts[FINAL_IX_SERVICE].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let treasury_derivation: RevenueMarketTreasuryDerivationV1 =
        derive_revenue_market_treasury_v1(
            program_id,
            revenue_authority,
            Hash32::from_bytes(root.root().market_instance_v2_id().bytes()),
            *accounts[FEE_IX_RUNTIME].key,
        )?;
    require(
        treasury_derivation.treasury_position_account()
                == *accounts[FEE_IX_POSITION].key
            && treasury_derivation.treasury_position_account().to_bytes()
                == selected.semantic.treasury_position().0
            && treasury_derivation.treasury_replay_account()
                == *accounts[FEE_IX_REPLAY].key
            && treasury_derivation.treasury_service_ledger_account()
                == *accounts[FINAL_IX_SERVICE].key,
        ClutchError::MismatchedState,
    )?;

    let authenticated_service = authenticate_treasury_service_ledger_v1(
        program_id,
        &accounts[FINAL_IX_SERVICE],
        treasury_derivation,
        true,
    )?;
    let service_body = authenticated_service.body();
    let service_evidence = TreasuryServiceSettlementEvidenceV1 {
        realm: revenue_authority.realm(),
        market_instance_v2_id: treasury_derivation.market_instance_v2_id(),
        revenue_policy_record_account: revenue_authority.record_account(),
        revenue_policy_record_v2_id: revenue_authority.record_semantic_id(),
        revenue_policy_v2_digest: revenue_authority.policy_digest(),
        treasury_owner: revenue_authority.treasury_owner(),
        treasury_position_account: treasury_derivation.treasury_position_account(),
        treasury_service_ledger_account: authenticated_service.account(),
        epoch_semantic_id: Hash32::from_bytes(epoch_semantic_id.bytes()),
        admitted_epoch_count_before: service_body.admitted_epoch_count,
        settled_epoch_count_before: service_body.settled_epoch_count,
    };
    let service_transition = prepare_treasury_service_settlement_v1(
        authenticated_service,
        treasury_derivation,
        &service_evidence,
    )?;

    let position_data = borrow_data(&accounts[FEE_IX_POSITION])?;
    let position = PositionAccountV3::decode(&position_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        position.owner().bytes() == selected.semantic.treasury_owner().0,
        ClutchError::MismatchedState,
    )?;
    drop(position_data);
    let position_replay = authenticate_current_general_position_replay_v2(
        program_id,
        bound,
        &accounts[FEE_IX_BINDING],
        &accounts[FEE_IX_RUNTIME],
        &accounts[FEE_IX_POSITION],
        &accounts[FEE_IX_REPLAY],
        selected.semantic.treasury_owner().0,
    )?;
    let treasury_credit = recipient.semantic().treasury_atoms();
    let credit_plan = contract::prepare_fee_position_credit_v1(
        selector.fee_record,
        recipient_data_id,
        id(accounts[FEE_IX_CASH_POT].key),
        2,
        recipient.semantic().row_count(),
        treasury_credit,
        position_replay.replay,
        pot.semantic,
        &RuntimeSha256,
    )?;
    require(
        credit_plan.cash_pot().collected_fee_atoms == 0,
        ClutchError::MismatchedState,
    )?;
    let fee_record_id = FeeRecordAccountIdV1::admit(FeeId(selector.fee_record.bytes()))
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let recipient_id = RecipientAllocationAccountIdV1::admit(FeeId(
        accounts[FEE_IX_RECIPIENT].key.to_bytes(),
    ))
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let treasury_id = TreasuryLedgerAccountIdV1::admit(FeeId(
        accounts[FINAL_IX_TREASURY].key.to_bytes(),
    ))
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let recipient_intent = RecipientAllocationIntentV1::bind(
        &selected.semantic,
        fee_record_id,
        recipient_id,
        treasury_id,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let (accumulator_after_value, treasury_authorization) = accumulator
        .semantic
        .fold_treasury_distribution(
            &recipient.semantic(),
            credit_plan.semantic(),
            &RuntimeSha256,
            &selected.semantic,
            &recipient_intent,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let TreasuryLedgerV2AccountV1 {
        semantic: treasury_semantic,
        rent: treasury_rent,
        ..
    } = *treasury;
    let treasury_closed = treasury_semantic
        .credit_and_settle_retirement(treasury_authorization)
        .and_then(|value| value.withdraw(selected.semantic.treasury_owner(), treasury_credit))
        .and_then(|value| value.close())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let selected_close = admit_global_fee_close(
        program_id,
        runtime_release,
        selector.fee_record,
        CandidateFeeAccountRoleV1::SelectedFeeRecord,
        &accounts[FINAL_IX_SELECTED],
        &selected_data,
        selected.rent,
        &accounts[FINAL_IX_REFUND_PAYER],
        &accounts[FINAL_IX_NEUTRAL_SINK],
    )?;
    let recipient_close = admit_global_fee_close(
        program_id,
        runtime_release,
        selector.fee_record,
        CandidateFeeAccountRoleV1::RecipientAllocation,
        &accounts[FEE_IX_RECIPIENT],
        &recipient_data,
        recipient.rent(),
        &accounts[FINAL_IX_REFUND_PAYER],
        &accounts[FINAL_IX_NEUTRAL_SINK],
    )?;
    let treasury_close = admit_global_fee_close(
        program_id,
        runtime_release,
        selector.fee_record,
        CandidateFeeAccountRoleV1::TreasuryLedger,
        &accounts[FINAL_IX_TREASURY],
        &treasury_data,
        treasury_rent,
        &accounts[FINAL_IX_REFUND_PAYER],
        &accounts[FINAL_IX_NEUTRAL_SINK],
    )?;
    let accumulator_close = admit_global_fee_close(
        program_id,
        runtime_release,
        selector.fee_record,
        CandidateFeeAccountRoleV1::RetirementAccumulator,
        &accounts[FEE_IX_ACCUMULATOR],
        &accumulator_data,
        accumulator.rent,
        &accounts[FINAL_IX_REFUND_PAYER],
        &accounts[FINAL_IX_NEUTRAL_SINK],
    )?;
    let closures = [selected_close, recipient_close, treasury_close, accumulator_close];
    let (payer_after_closes, sink_after_closes) = aggregate_global_close_balances(
        &closures,
        &accounts[FINAL_IX_REFUND_PAYER],
        &accounts[FINAL_IX_NEUTRAL_SINK],
    )?;
    let completed = accumulator_after_value
        .complete(
            accumulator_close,
            CandidateFeeAccountClosuresV1 {
                selected_record: selected_close,
                recipient_allocation: recipient_close,
                treasury_ledger: treasury_close,
            },
            &recipient.semantic(),
            &RuntimeSha256,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let settlement = CandidateFeeSettlementV1 {
        fee_record: selected.semantic.fee_record(),
        hoard_collateral_before: 0,
        hoard_collateral_after: 0,
        selected_fee_debit_atoms: u128::from(recipient.semantic().collected_fee_atoms()),
        maker_rebate_atoms: recipient.semantic().maker_rebate_total(),
        executor_atoms: recipient.semantic().executor_atoms(),
        treasury_credit_atoms: treasury_credit,
    };
    let terminal_bundle = build_settled_fee_terminal_from_accumulator_v2(
        FeeId(accounts[FINAL_IX_TERMINAL].key.to_bytes()),
        FeeId(accounts[FINAL_IX_MANIFEST].key.to_bytes()),
        &selected.semantic,
        &recipient.semantic(),
        &settlement,
        &recipient_intent,
        &treasury_closed,
        completed,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let manifest_pda = seeds::general_v2_fee_closure_manifest_pda(
        program_id,
        &selector.fee_record.bytes(),
    );
    let terminal_pda = seeds::general_v2_fee_terminal_receipt_pda(
        program_id,
        &selector.fee_record.bytes(),
    );
    require(
        *accounts[FINAL_IX_MANIFEST].key == manifest_pda.0
            && *accounts[FINAL_IX_TERMINAL].key == terminal_pda.0,
        ClutchError::WrongPda,
    )?;
    let manifest_rent = rent_owner(
        &accounts[FINAL_IX_CREATION_PAYER],
        &accounts[FINAL_IX_MANIFEST],
        &rent_parameters,
        contract::FEE_RETIREMENT_ACCOUNT_BYTES_V2,
    )?;
    let terminal_rent = rent_owner(
        &accounts[FINAL_IX_CREATION_PAYER],
        &accounts[FINAL_IX_TERMINAL],
        &rent_parameters,
        contract::FEE_RETIREMENT_ACCOUNT_BYTES_V3,
    )?;
    let mut manifest_output = [0u8; contract::FEE_RETIREMENT_ACCOUNT_BYTES_V2];
    FeeClosureManifestV2AccountV1 {
        semantic: terminal_bundle.closure_manifest(),
        rent: manifest_rent,
        stored_bump: manifest_pda.1,
    }
    .encode(&mut manifest_output)?;
    let mut terminal_output = [0u8; contract::FEE_RETIREMENT_ACCOUNT_BYTES_V3];
    FeeRecordTerminalV3AccountV1 {
        semantic: terminal_bundle.terminal(),
        rent: terminal_rent,
        stored_bump: terminal_pda.1,
    }
    .encode(&mut terminal_output)?;
    let mut root_output = std::vec![0u8; root.account_bytes()];
    root.encode_fee_record_retirement_successor(&mut root_output)?;
    let mut pot_output = [0u8; contract::SETTLEMENT_CASH_POT_ACCOUNT_BYTES];
    SettlementCashPotV1AccountV1 {
        semantic: credit_plan.cash_pot(),
        stored_bump: pot.stored_bump,
        flags: 0,
    }
    .encode(&mut pot_output)?;
    let position_output = credit_plan
        .position()
        .semantic
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    drop(selected_data);
    drop(recipient_data);
    drop(treasury_data);
    drop(accumulator_data);
    drop(pot_data);
    for index in [
        FINAL_IX_SELECTED,
        FEE_IX_RECIPIENT,
        FINAL_IX_TREASURY,
        FEE_IX_ACCUMULATOR,
    ] {
        close_program_account(&accounts[index])?;
    }
    set_lamports(&accounts[FINAL_IX_REFUND_PAYER], payer_after_closes)?;
    if accounts[FINAL_IX_REFUND_PAYER].key == accounts[FINAL_IX_NEUTRAL_SINK].key {
        require(payer_after_closes == sink_after_closes, ClutchError::MismatchedState)?;
    } else {
        set_lamports(&accounts[FINAL_IX_NEUTRAL_SINK], sink_after_closes)?;
    }

    let fee_record_bytes = selector.fee_record.bytes();
    let manifest_bump = [manifest_pda.1];
    create_from_payer(
        program_id,
        &accounts[FINAL_IX_CREATION_PAYER],
        &accounts[FINAL_IX_MANIFEST],
        &accounts[FINAL_IX_SYSTEM_PROGRAM],
        &rent_parameters,
        contract::FEE_RETIREMENT_ACCOUNT_BYTES_V2,
        manifest_rent,
        &[
            contract::FEE_CLOSURE_MANIFEST_SEED_DOMAIN_V1,
            &fee_record_bytes,
            &manifest_bump,
        ],
    )?;
    let terminal_bump = [terminal_pda.1];
    create_from_payer(
        program_id,
        &accounts[FINAL_IX_CREATION_PAYER],
        &accounts[FINAL_IX_TERMINAL],
        &accounts[FINAL_IX_SYSTEM_PROGRAM],
        &rent_parameters,
        contract::FEE_RETIREMENT_ACCOUNT_BYTES_V3,
        terminal_rent,
        &[
            contract::FEE_TERMINAL_RECEIPT_SEED_DOMAIN_V1,
            &fee_record_bytes,
            &terminal_bump,
        ],
    )?;
    borrow_mut_data(&accounts[FINAL_IX_MANIFEST])?.copy_from_slice(&manifest_output);
    borrow_mut_data(&accounts[FINAL_IX_TERMINAL])?.copy_from_slice(&terminal_output);
    let _persisted_terminal_pair = authenticate_fee_terminal_pair_v1(
        program_id,
        &accounts[FINAL_IX_MANIFEST],
        &accounts[FINAL_IX_TERMINAL],
        FeeTerminalPairExpectationV1 {
            fee_record: selector.fee_record,
            settlement_root: root.account(),
            selected_feed_data_id: root.selected_feed_data_id()?,
            market: root.root().market(),
            epoch: selector.epoch,
            settlement_candidate: root.root().settlement_candidate_id(),
        },
        true,
    )?;
    borrow_mut_data(&accounts[FEE_IX_CASH_POT])?.copy_from_slice(&pot_output);
    if let Some(replay) = credit_plan.replay() {
        borrow_mut_data(&accounts[FEE_IX_POSITION])?.copy_from_slice(&position_output);
        borrow_mut_data(&accounts[FEE_IX_REPLAY])?
            .copy_from_slice(replay.replay_poststate_body());
    }
    accept_treasury_service_transition_v1(&accounts[FINAL_IX_SERVICE], service_transition)?;
    borrow_mut_data(&accounts[FEE_IX_ROOT])?.copy_from_slice(&root_output);
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
            FeeRetirementPayloadV1::FinalizeTreasuryAndGlobals(value) => {
                finalize_treasury_and_fee_globals(program_id, accounts, value)
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
        assert_eq!(FEE_FINALIZE_GLOBALS_ACCOUNT_COUNT_V1, 26);
    }

    #[test]
    fn close_credits_coalesce_without_overwrite_or_double_count() {
        assert_eq!(
            close_destination_balances(11, 11, 5, 7, true),
            Some((23, 23)),
        );
        assert_eq!(
            close_destination_balances(11, 13, 5, 7, false),
            Some((16, 20)),
        );
        assert_eq!(close_destination_balances(11, 13, 5, 7, true), None);
        assert_eq!(
            close_destination_balances(u64::MAX, u64::MAX, 1, 0, true),
            None,
        );
    }
}
