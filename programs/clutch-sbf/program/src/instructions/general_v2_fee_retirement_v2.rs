//! Sole current SBF owner of General action 50 fee retirement.
//!
//! Maker distribution advances the move-only `0xb9/v1` accumulator one
//! canonical Position at a time. The final frame credits treasury, settles the
//! counted Revenue service ledger, drains the fee cash pot, closes every
//! temporary fee global with exact principal/donation routing, and atomically
//! creates the hostile-authenticated durable `0xb9/v2` + `0xb9/v3` pair.

use core::cell::{Ref, RefMut};
use std::boxed::Box;

use clutch_collateral_adapter_v2::{
    refine_market_collateral_v2, BoundCollateralProfileV2, Id as CollateralId,
    MarketCollateralBindingV2,
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
    decode_fee_retirement_payload_v1,
    CountedSettlementRootSelectorV1, FeeFinalizeGlobalsPayloadV1,
    FeeMakerDistributionPayloadV1,
    FeeRetirementPayloadV1,
    fee_runtime_semantic_release_id_v2, DeletableRentOwnerV1,
    FeeClosureManifestV2AccountV1, FeeRecordTerminalV3AccountV1,
    FeeRetirementAccumulatorV1AccountV1, GeneralEpochPhaseV1,
    GeneralEpochV6AccountV1, Id32, MarketBindingV5, SelectedFeeRecordV2AccountV1,
    SettlementCashPotV1AccountV1,
    TreasuryLedgerV2AccountV1,
};
use clutch_retirement::{PositionAccountV3, PositionV3Sha256Backend, ReplayV3HashBackend};
use clutch_solana_layout::registry::GeneralV2Action;
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV3, SeriesMarketLinkAccountV3,
};
use clutch_solana_layout::Hash32;
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
    accept_treasury_service_transition_v1, authenticate_treasury_service_ledger_v1,
    prepare_treasury_service_settlement_v1, AuthenticatedTreasuryServiceAdmissionV1,
    AuthenticatedTreasuryServiceSettlementV1,
};

use super::general_v2_settlement_root::{
    authenticate_readonly_general_settlement_root_epoch_v1,
    authenticate_writable_general_settlement_root_epoch_v1,
    AuthenticatedGeneralSettlementRootV1,
};
use super::general_market_current_v5::{
    authenticate_general_market_current_v5, AuthenticatedGeneralMarketCurrentV5,
    GeneralMarketCurrentAccountFrameV5, GENERAL_MARKET_CURRENT_ACCOUNT_COUNT_V5,
};
use super::general_v2_position_replay::authenticate_current_general_position_replay_v5;

/// Action-50 maker credit including the complete current collateral join.
pub const FEE_MAKER_DISTRIBUTION_ACCOUNT_COUNT_V1: usize = 34;
/// Action-50 final treasury credit, service settlement, four temporary closes,
/// and creation of the durable b9/v2+v3 pair.
pub const FEE_FINALIZE_GLOBALS_ACCOUNT_COUNT_V1: usize = 45;

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
const CURRENT_IX_PRODUCT_ROOT: usize = 13;
const CURRENT_IX_SERIES_LINK: usize = 14;
const CURRENT_IX_SERIES_FUNDING: usize = 15;
const CURRENT_IX_SERIES_REGISTRY: usize = 16;
const CURRENT_IX_REGISTRY_PROGRAM: usize = 17;
const CURRENT_IX_REGISTRY_PROGRAMDATA: usize = 18;
const CURRENT_IX_REGISTRY_RELEASE: usize = 19;
const CURRENT_IX_CAPABILITY_PROFILE: usize = 20;
const CURRENT_IX_SOURCE_RELEASE: usize = 21;
const CURRENT_IX_COMPILER_BUNDLE: usize = 22;
const CURRENT_IX_REVENUE_RECORD: usize = 23;
const CURRENT_IX_REVENUE_PREIMAGE: usize = 24;
const CURRENT_IX_ARTIFACTS_START: usize = 25;
const CURRENT_IX_ARTIFACTS_END: usize = 34;

const FINAL_IX_EPOCH: usize = 34;
const FINAL_IX_SELECTED: usize = 35;
const FINAL_IX_TREASURY: usize = 36;
const FINAL_IX_SERVICE: usize = 37;
const FINAL_IX_MANIFEST: usize = 38;
const FINAL_IX_TERMINAL: usize = 39;
const FINAL_IX_CREATION_PAYER: usize = 40;
const FINAL_IX_REFUND_PAYER: usize = 41;
const FINAL_IX_NEUTRAL_SINK: usize = 42;
const FINAL_IX_SYSTEM_PROGRAM: usize = 43;
const FINAL_IX_RENT_SYSVAR: usize = 44;

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

/// Hostile-authenticate the complete current Product/General/Revenue graph
/// from the shared action-50 prefix.  Product RootV3 and LinkV3 use heap-owned
/// caller buffers; no full-width account value crosses this helper's frame.
#[inline(never)]
fn authenticate_action50_current_market_v5(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Outcome<AuthenticatedGeneralMarketCurrentV5> {
    require(
        accounts.len() >= CURRENT_IX_ARTIFACTS_END
            && CURRENT_IX_ARTIFACTS_END - CURRENT_IX_ARTIFACTS_START == 9
            && GENERAL_MARKET_CURRENT_ACCOUNT_COUNT_V5 == 25,
        ClutchError::AccountCount,
    )?;
    let frame = GeneralMarketCurrentAccountFrameV5 {
        market_binding: &accounts[FEE_IX_BINDING],
        market_runtime: &accounts[FEE_IX_RUNTIME],
        product_root: &accounts[CURRENT_IX_PRODUCT_ROOT],
        series_link: &accounts[CURRENT_IX_SERIES_LINK],
        series_funding: &accounts[CURRENT_IX_SERIES_FUNDING],
        series_registry: &accounts[CURRENT_IX_SERIES_REGISTRY],
        registry_program: &accounts[CURRENT_IX_REGISTRY_PROGRAM],
        registry_programdata: &accounts[CURRENT_IX_REGISTRY_PROGRAMDATA],
        registry_release_artifact: &accounts[CURRENT_IX_REGISTRY_RELEASE],
        capability_profile_artifact: &accounts[CURRENT_IX_CAPABILITY_PROFILE],
        source_release: &accounts[CURRENT_IX_SOURCE_RELEASE],
        compiler_bundle: &accounts[CURRENT_IX_COMPILER_BUNDLE],
        market_instance: &accounts[FEE_IX_MARKET_INSTANCE],
        realm: &accounts[FEE_IX_REALM],
        revenue_record: &accounts[CURRENT_IX_REVENUE_RECORD],
        revenue_policy_preimage: &accounts[CURRENT_IX_REVENUE_PREIMAGE],
        artifacts: &accounts[CURRENT_IX_ARTIFACTS_START..CURRENT_IX_ARTIFACTS_END],
    };
    let mut product_root = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let mut product_link = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    authenticate_general_market_current_v5(
        program_id,
        &frame,
        &mut product_root,
        &mut product_link,
    )
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

fn authenticate_fee_distribution_collateral(
    program_id: &Pubkey,
    current: &AuthenticatedGeneralMarketCurrentV5,
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
    let binding = current.binding();
    let runtime = current.runtime();
    let base = binding.base();
    let instance = current.market_instance();
    require(
        current.binding_account() == *accounts[FEE_IX_BINDING].key
            && current.runtime_account() == *accounts[FEE_IX_RUNTIME].key
            && id(accounts[FEE_IX_BINDING].key) == root.root().market_binding()
            && base.market == root.root().market()
            && base.market_instance_v2_id == root.root().market_instance_v2_id()
            && base.batch_policy_id() == root.root().batch_policy_id()
            && runtime.market_instance_v2_id == base.market_instance_v2_id
            && instance
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
                == base.market_instance_v2_id.bytes()
            && instance.market_genesis_profile_id.content_id().bytes()
                == base.market_genesis_profile_v2_id.bytes()
            && current.revenue().realm().bytes() == realm.realm().realm.bytes()
            && current.collateral_profile_id().bytes() == realm.realm().profile.bytes(),
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
    binding: &MarketBindingV5,
) -> Outcome<()> {
    let value = root.root();
    require(
        value.market_binding() == id(binding_account.key)
            && value.market() == binding.base().market
            && value.market_instance_v2_id() == binding.base().market_instance_v2_id
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
    let current = authenticate_action50_current_market_v5(program_id, accounts)?;
    require_root_binding(&root, &accounts[FEE_IX_BINDING], current.binding())?;
    let bound = authenticate_fee_distribution_collateral(program_id, &current, &root, accounts)?;
    let position_data = borrow_data(&accounts[FEE_IX_POSITION])?;
    let position = PositionAccountV3::decode(&position_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_owner = position.owner().bytes();
    drop(position_data);
    let position_replay = authenticate_current_general_position_replay_v5(
        program_id,
        &current,
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

/// Execute only the current two-frame forms of General action 50.
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
        action == GeneralV2Action::AdvanceFeeRetirement
            && capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    match decode_fee_retirement_payload_v1(action.tag(), payload)? {
        FeeRetirementPayloadV1::MakerDistribution(value) => {
            distribute_maker_fee(program_id, accounts, value)
        }
        FeeRetirementPayloadV1::FinalizeTreasuryAndGlobals(value) => {
            finalize_treasury_and_fee_globals(program_id, accounts, value)
        }
    }
}

#[cfg(test)]
mod current_action50_source_tests {
    use super::*;

    #[test]
    fn action50_frames_are_current_and_below_the_deployed_account_ceiling() {
        assert_eq!(FEE_MAKER_DISTRIBUTION_ACCOUNT_COUNT_V1, 34);
        assert_eq!(FEE_FINALIZE_GLOBALS_ACCOUNT_COUNT_V1, 45);
        assert!(FEE_MAKER_DISTRIBUTION_ACCOUNT_COUNT_V1 < 64);
        assert!(FEE_FINALIZE_GLOBALS_ACCOUNT_COUNT_V1 < 64);
        assert_eq!(CURRENT_IX_ARTIFACTS_END - CURRENT_IX_ARTIFACTS_START, 9);
        let source = include_str!("general_v2_fee_retirement_v2.rs");
        assert!(source.contains("authenticate_action50_current_market_v5"));
        assert!(source.contains("authenticate_current_general_position_replay_v5"));
        assert!(!source.contains("authenticate_current_general_position_replay_v2("));
        assert!(!source.contains("FINAL_IX_REVENUE_RECORD"));
    }
}
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
    let current_market = authenticate_action50_current_market_v5(program_id, accounts)?;
    let binding = current_market.binding();
    require_root_binding(&root, &accounts[FEE_IX_BINDING], binding)?;
    require(
        binding.base().neutral_sink == id(accounts[FINAL_IX_NEUTRAL_SINK].key),
        ClutchError::MismatchedState,
    )?;
    let bound = authenticate_fee_distribution_collateral(
        program_id,
        &current_market,
        &root,
        accounts,
    )?;

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
            && selected.semantic.price_scale() == binding.base().price_scale
            && selected.semantic.outcome_count() == binding.base().outcome_count
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
    let revenue_authority = current_market.revenue();
    selected
        .semantic
        .binds_revenue_policy(&revenue_authority.policy())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        selector.revenue_policy == revenue_authority.policy(),
        ClutchError::MismatchedState,
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
                == accounts[FINAL_IX_SERVICE].key.to_bytes()
            && revenue_authority.record_account() == *accounts[CURRENT_IX_REVENUE_RECORD].key,
        ClutchError::MismatchedState,
    )?;
    let treasury_derivation = current_market.treasury();
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
    let position_replay = authenticate_current_general_position_replay_v5(
        program_id,
        &current_market,
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
