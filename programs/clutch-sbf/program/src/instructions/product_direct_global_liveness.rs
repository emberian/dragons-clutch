//! Product-owned physical capitalization for Direct's global liveness bundle.
//!
//! This module has no instruction route. Its raw creator is crate-private and
//! returns a non-`Copy`, non-`Clone` receipt which the sole Product founder
//! composer must consume in the same instruction after creating and hostile-
//! reopening the exact `0xaa` root. Every target prebalance is retained as
//! neutral-sink donation;
//! the immutable principal owner still supplies the complete work and rent
//! debit, so prefunding never discounts protocol liveness.

use crate::accounts::{expect_pda, require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_creatable, require_system_program,
    transfer_data, RentParameters, SYSTEM_PROGRAM_ID,
};
use crate::instructions::product_market::{
    authenticate_market_lifecycle_root_v1, AuthenticatedMarketLifecycleRootV1,
};
use crate::instructions::product_market_foundation_init::{
    AuthenticatedProductMarketFounderCreationV1,
    AuthenticatedProductMarketFounderFoundationPreauthorizationV1,
};
use crate::seeds;
use clutch_liveness::runtime_v1::{
    PresentFundingSourceV1, PresentFundingV1, RuntimeCompartmentAdmissionV1,
    RuntimeCompartmentIdentityV1, RuntimeCompartmentKindV1, RuntimeCompartmentV1,
    RuntimeLivenessPolicyV1, RUNTIME_COMPARTMENT_COUNT_V1, RUNTIME_COMPARTMENT_ORDER_V1,
    RUNTIME_LIVENESS_ACCOUNT_BYTES_V1, RUNTIME_LIVENESS_POLICY_BYTES_V1,
};
use clutch_liveness::Id as LivenessId;
use clutch_direct_market_runtime::{DirectFamilyTerminalPlanV1, DirectReplayPhaseV1};
use clutch_product_series::{
    ContentId, DirectGlobalLivenessCapitalizationV1, DirectGlobalLivenessPhaseV1,
    DirectGlobalLivenessV1, FixedCodec, MarketFamilyV1, MarketInstanceV2Id,
    MarketLifecyclePhaseV1, ProductDirectGlobalLivenessAuthorityV1,
    DIRECT_GLOBAL_LIVENESS_ALLOCATION_CALL_WIDTH_V1,
    DIRECT_GLOBAL_LIVENESS_BINDING_DOMAIN_V1,
    DIRECT_GLOBAL_LIVENESS_CAPITALIZATION_DOMAIN_V1,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, ProductDirectGlobalLivenessAccountV1,
    PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_BYTES_V1,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const PRODUCT_DIRECT_GLOBAL_LIFECYCLE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product-direct-global-lifecycle/v1";
const PRODUCT_DIRECT_ROW_CAPITALIZATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product-direct-global-row-capitalization/v1";
const PRODUCT_DIRECT_ACCOUNT_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product-direct-global-account-authentication/v1";
const PRODUCT_DIRECT_FOUNDER_ACTIVATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product-direct-global-founder-activation/v1";
const PRODUCT_DIRECT_CANDIDATE_RETIREMENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product-direct-global-candidate-retirement/v1";

/// One small row result retained while seven accounts are created serially.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RowCapitalizationFactsV1 {
    account_id: ContentId,
    capitalization_receipt_id: ContentId,
    work_principal_lamports: u64,
    rent_principal_lamports: u64,
    initial_donation_lamports: u64,
    maximum_calls: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedRuntimeLivenessPolicyFactsV1 {
    policy_id: ContentId,
    realm_id: ContentId,
    neutral_sink: ContentId,
    data_id: ContentId,
}

/// Private non-detachable postwrite minted by the raw capitalization half.
///
/// The Product founder must hostile-reopen the account, compare this receipt,
/// create the root/link, and activate `0xba` before the outer call returns.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductDirectGlobalLivenessCapitalizationV1 {
    state: DirectGlobalLivenessV1,
    account_data_id: ContentId,
    account_authentication_id: ContentId,
    payer_balance_before: u64,
    payer_balance_after: u64,
    total_payer_debit_lamports: u64,
}

impl AuthenticatedProductDirectGlobalLivenessCapitalizationV1 {
    pub(crate) const fn state(&self) -> &DirectGlobalLivenessV1 { &self.state }
    pub(crate) const fn account_data_id(&self) -> ContentId { self.account_data_id }
    pub(crate) const fn account_authentication_id(&self) -> ContentId {
        self.account_authentication_id
    }
    pub(crate) const fn payer_balance_before(&self) -> u64 { self.payer_balance_before }
    pub(crate) const fn payer_balance_after(&self) -> u64 { self.payer_balance_after }
    pub(crate) const fn total_payer_debit_lamports(&self) -> u64 {
        self.total_payer_debit_lamports
    }
}

/// Hostile-reopened current `0xba/v1` account authentication.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductDirectGlobalLivenessAccountV1 {
    state: DirectGlobalLivenessV1,
    data_id: ContentId,
    authentication_id: ContentId,
    observed_lamports: u64,
    stored_bump: u8,
}

impl AuthenticatedProductDirectGlobalLivenessAccountV1 {
    pub(crate) const fn state(&self) -> &DirectGlobalLivenessV1 { &self.state }
    pub(crate) const fn data_id(&self) -> ContentId { self.data_id }
    pub(crate) const fn authentication_id(&self) -> ContentId { self.authentication_id }
    pub(crate) const fn observed_lamports(&self) -> u64 { self.observed_lamports }
}

/// Private postwrite proving that the exact newly-created Product root consumed
/// the full-payer `0xba` capitalization before Direct allocation became live.
///
/// This value is intentionally non-`Copy` and non-`Clone`. The Product founder
/// outer must move it into the current root successor; neither the raw
/// capitalization receipt nor a caller-supplied Market binding can activate
/// the account.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductDirectGlobalLivenessActivationV1 {
    id: ContentId,
    state: DirectGlobalLivenessV1,
    account_data_id: ContentId,
    account_authentication_id: ContentId,
    founder_creation_receipt_id: ContentId,
    root_semantic_id: ContentId,
    root_authentication_id: ContentId,
}

impl AuthenticatedProductDirectGlobalLivenessActivationV1 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn state(&self) -> &DirectGlobalLivenessV1 { &self.state }
    pub(crate) const fn account_data_id(&self) -> ContentId { self.account_data_id }
    pub(crate) const fn account_authentication_id(&self) -> ContentId {
        self.account_authentication_id
    }
    pub(crate) const fn founder_creation_receipt_id(&self) -> ContentId {
        self.founder_creation_receipt_id
    }
    pub(crate) const fn root_semantic_id(&self) -> ContentId { self.root_semantic_id }
    pub(crate) const fn root_authentication_id(&self) -> ContentId {
        self.root_authentication_id
    }
}

/// Private Product postwrite proving one final sealed Direct plan retired the
/// matching live allocation and no provisional action-13 preparation did.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductDirectCandidateRetirementV1 {
    id: ContentId,
    state: DirectGlobalLivenessV1,
    account_data_id: ContentId,
    account_authentication_id: ContentId,
    direct_terminal_receipt_id: ContentId,
    family_terminal_sequence: u32,
    product_family_poststate_id: ContentId,
}

impl AuthenticatedProductDirectCandidateRetirementV1 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn state(&self) -> &DirectGlobalLivenessV1 { &self.state }
    pub(crate) const fn account_data_id(&self) -> ContentId { self.account_data_id }
    pub(crate) const fn account_authentication_id(&self) -> ContentId {
        self.account_authentication_id
    }
    pub(crate) const fn direct_terminal_receipt_id(&self) -> ContentId {
        self.direct_terminal_receipt_id
    }
    pub(crate) const fn family_terminal_sequence(&self) -> u32 {
        self.family_terminal_sequence
    }
    pub(crate) const fn product_family_poststate_id(&self) -> ContentId {
        self.product_family_poststate_id
    }
}

#[derive(Clone, Copy, Debug)]
struct ExactCapitalizationAuthorityV1;

impl ProductDirectGlobalLivenessAuthorityV1 for ExactCapitalizationAuthorityV1 {
    fn authenticate_capitalization(
        &self,
        _capitalization: &DirectGlobalLivenessCapitalizationV1,
    ) -> clutch_product_series::Result<()> {
        // Private construction is the authority: this module derives every
        // field from live account poststates immediately before initialization.
        Ok(())
    }
}

struct ExactFounderActivationAuthorityV1<'state> {
    expected_state: &'state DirectGlobalLivenessV1,
    expected_founder_receipt_id: ContentId,
}

impl ProductDirectGlobalLivenessAuthorityV1 for ExactFounderActivationAuthorityV1<'_> {
    fn authenticate_founder_activation(
        &self,
        state: &DirectGlobalLivenessV1,
        founder_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if state == self.expected_state
            && founder_receipt_id == self.expected_founder_receipt_id
        {
            Ok(())
        } else {
            Err(clutch_product_series::Error::UnauthenticatedAuthority)
        }
    }
}

struct ExactDirectCandidateRetirementAuthorityV1<'state> {
    expected_state: &'state DirectGlobalLivenessV1,
    expected_terminal_receipt_id: ContentId,
    expected_family_terminal_sequence: u32,
}

impl ProductDirectGlobalLivenessAuthorityV1
    for ExactDirectCandidateRetirementAuthorityV1<'_>
{
    fn authenticate_candidate_retirement(
        &self,
        state: &DirectGlobalLivenessV1,
        direct_terminal_receipt_id: ContentId,
        family_terminal_sequence: u32,
    ) -> clutch_product_series::Result<()> {
        if state == self.expected_state
            && direct_terminal_receipt_id == self.expected_terminal_receipt_id
            && family_terminal_sequence == self.expected_family_terminal_sequence
        {
            Ok(())
        } else {
            Err(clutch_product_series::Error::UnauthenticatedAuthority)
        }
    }
}

/// Capitalize all seven generic runtime accounts plus the separate Product
/// manifest. This is a raw half: it is safe only because it is crate-private
/// and no route calls it except the eventual atomic Product founder outer.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn capitalize_product_direct_global_liveness_v1<'a>(
    program_id: &Pubkey,
    founder: &AuthenticatedProductMarketFounderFoundationPreauthorizationV1,
    market_binding_id: ContentId,
    policy_account: &AccountInfo<'a>,
    manifest_account: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    compartments: &[AccountInfo<'a>],
    system_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
) -> Outcome<AuthenticatedProductDirectGlobalLivenessCapitalizationV1> {
    require(
        compartments.len() == RUNTIME_COMPARTMENT_COUNT_V1,
        ClutchError::AccountCount,
    )?;
    require_system_program(system_program)?;
    let rent = read_rent(rent_sysvar)?;
    authenticate_fixed_roles_v1(
        program_id,
        founder,
        policy_account,
        manifest_account,
        payer,
        neutral_lamport_sink,
        compartments,
        system_program,
        rent_sysvar,
    )?;
    let policy = authenticate_policy_v1(program_id, founder, policy_account, &rent)?;
    market_binding_id
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let market_instance_id = founder.market_instance_id();
    let generation = founder.generation();
    let global_lifecycle_id = global_lifecycle_id_v1(
        program_id,
        market_instance_id,
        generation,
        ContentId::from_bytes(founder.lifecycle_root_account().to_bytes()),
        policy.policy_id,
    );
    require(!global_lifecycle_id.is_zero(), ClutchError::MismatchedState)?;

    let payer_balance_before = payer.lamports();
    let mut compartment_accounts = [ContentId::ZERO; RUNTIME_COMPARTMENT_COUNT_V1];
    let mut compartment_receipts = [ContentId::ZERO; RUNTIME_COMPARTMENT_COUNT_V1];
    let mut total_work_principal_lamports = 0u64;
    let mut total_rent_principal_lamports = 0u64;
    let mut initial_bundle_donation_lamports = 0u64;
    let mut candidate_maximum_calls = 0u32;
    let mut candidate_work_principal_lamports = 0u64;
    let mut index = 0usize;
    while index < RUNTIME_COMPARTMENT_COUNT_V1 {
        let facts = capitalize_row_v1(
            program_id,
            policy_account,
            &policy,
            market_instance_id,
            generation,
            global_lifecycle_id,
            manifest_account,
            payer,
            &compartments[index],
            system_program,
            &rent,
            RUNTIME_COMPARTMENT_ORDER_V1[index],
        )?;
        compartment_accounts[index] = facts.account_id;
        compartment_receipts[index] = facts.capitalization_receipt_id;
        total_work_principal_lamports = total_work_principal_lamports
            .checked_add(facts.work_principal_lamports)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        total_rent_principal_lamports = total_rent_principal_lamports
            .checked_add(facts.rent_principal_lamports)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        initial_bundle_donation_lamports = initial_bundle_donation_lamports
            .checked_add(facts.initial_donation_lamports)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        if facts.account_id
            == compartment_accounts[RuntimeCompartmentKindV1::Candidate.index()]
        {
            candidate_maximum_calls = facts.maximum_calls;
            candidate_work_principal_lamports = facts.work_principal_lamports;
        }
        index += 1;
    }
    require(
        candidate_maximum_calls != 0 && candidate_work_principal_lamports != 0,
        ClutchError::MismatchedState,
    )?;

    let manifest_rent_principal_lamports =
        rent.minimum_balance(PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_BYTES_V1)?;
    require(
        manifest_rent_principal_lamports != 0,
        ClutchError::MismatchedState,
    )?;
    let manifest_initial_donation_lamports = manifest_account.lamports();
    let manifest_id = ContentId::from_bytes(manifest_account.key.to_bytes());
    let lifecycle_root_account = ContentId::from_bytes(founder.lifecycle_root_account().to_bytes());
    let policy_id = policy.policy_id;
    let policy_data_id = account_data_id_v1(policy_account)?;
    require(policy_data_id == policy.data_id, ClutchError::MismatchedState)?;
    let realm_id = policy.realm_id;
    let principal_refund_owner = ContentId::from_bytes(payer.key.to_bytes());
    let neutral_sink_id = ContentId::from_bytes(neutral_lamport_sink.key.to_bytes());
    let global_bundle_binding_id = global_bundle_binding_id_v1(
        manifest_id,
        market_instance_id,
        lifecycle_root_account,
        market_binding_id,
        realm_id,
        ContentId::from_bytes(policy_account.key.to_bytes()),
        policy_id,
        policy_data_id,
        global_lifecycle_id,
        principal_refund_owner,
        neutral_sink_id,
        generation,
        &compartment_accounts,
        &compartment_receipts,
    );
    let total_bundle_payer_debit = total_work_principal_lamports
        .checked_add(total_rent_principal_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let total_payer_debit_lamports = total_bundle_payer_debit
        .checked_add(manifest_rent_principal_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let payer_balance_after = payer_balance_before
        .checked_sub(total_payer_debit_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let global_capitalization_receipt_id = global_capitalization_receipt_id_v1(
        global_bundle_binding_id,
        &compartment_receipts,
        total_work_principal_lamports,
        total_rent_principal_lamports,
        initial_bundle_donation_lamports,
        manifest_rent_principal_lamports,
        manifest_initial_donation_lamports,
        payer_balance_before,
        payer_balance_after,
    );
    let capitalization = DirectGlobalLivenessCapitalizationV1 {
        account_id: manifest_id,
        market_instance_id,
        lifecycle_root_account,
        market_binding_id,
        realm_id,
        policy_account: ContentId::from_bytes(policy_account.key.to_bytes()),
        policy_id,
        policy_data_id,
        global_lifecycle_id,
        global_bundle_binding_id,
        global_capitalization_receipt_id,
        principal_refund_owner,
        neutral_lamport_sink: neutral_sink_id,
        compartment_accounts,
        compartment_capitalization_receipt_ids: compartment_receipts,
        generation,
        total_work_principal_lamports,
        total_rent_principal_lamports,
        initial_bundle_donation_lamports,
        manifest_rent_principal_lamports,
        manifest_initial_donation_lamports,
        candidate_maximum_calls,
        candidate_work_principal_lamports,
        allocation_call_width: DIRECT_GLOBAL_LIVENESS_ALLOCATION_CALL_WIDTH_V1,
    };
    let state = DirectGlobalLivenessV1::initialize(&ExactCapitalizationAuthorityV1, capitalization)
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let (_, manifest_bump) = seeds::product_direct_global_liveness_pda(
        program_id,
        &market_instance_id.bytes(),
        generation,
    );
    let manifest_bump_seed = [manifest_bump];
    full_payer_fund_allocate_assign_v1(
        program_id,
        payer,
        manifest_account,
        system_program,
        manifest_rent_principal_lamports,
        PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_BYTES_V1,
        &[
            seeds::SEED_PRODUCT_DIRECT_GLOBAL_LIVENESS,
            &market_instance_id.bytes(),
            &generation.to_le_bytes(),
            &manifest_bump_seed,
        ],
    )?;
    write_manifest_state_v1(
        manifest_account,
        &state,
        manifest_rent_principal_lamports,
        manifest_bump,
    )?;
    require(
        payer.lamports() == payer_balance_after
            && manifest_account.lamports()
                == manifest_initial_donation_lamports
                    .checked_add(manifest_rent_principal_lamports)
                    .ok_or(ClutchError::Arithmetic)?,
        ClutchError::AccountCreationFailed,
    )?;
    let reopened = authenticate_product_direct_global_liveness_v1(
        program_id,
        manifest_account,
        true,
    )?;
    require(
        reopened.state() == &state
            && reopened.observed_lamports() == manifest_account.lamports(),
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedProductDirectGlobalLivenessCapitalizationV1 {
        state,
        account_data_id: reopened.data_id(),
        account_authentication_id: reopened.authentication_id(),
        payer_balance_before,
        payer_balance_after,
        total_payer_debit_lamports,
    })
}

/// Hostile-reopen the exact current `0xba/v1` account and its complete data.
#[inline(never)]
pub(crate) fn authenticate_product_direct_global_liveness_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
) -> Outcome<AuthenticatedProductDirectGlobalLivenessAccountV1> {
    require(
        account.owner == program_id
            && !account.is_signer
            && !account.executable
            && account.is_writable == writable
            && account.data_len() == PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let frame = ProductDirectGlobalLivenessAccountV1::decode(&data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let data_id = ContentId::from_bytes(solana_sha256_hasher::hashv(&[&data[..]]).to_bytes());
    drop(data);
    expect_pda(
        account.key,
        seeds::product_direct_global_liveness_pda(
            program_id,
            &frame.state.market_instance_id().bytes(),
            frame.state.generation(),
        ),
        Some(frame.stored_bump),
    )?;
    let minimum_accounted = frame
        .rent_principal_lamports
        .checked_add(frame.state.manifest_initial_donation_lamports())
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        frame.state.account_id().bytes() == account.key.to_bytes()
            && account.lamports() >= minimum_accounted,
        ClutchError::MismatchedState,
    )?;
    let semantic_id = frame
        .state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_DIRECT_ACCOUNT_AUTHENTICATION_DOMAIN_V1,
            account.key.as_ref(),
            program_id.as_ref(),
            &data_id.bytes(),
            &semantic_id.bytes(),
            &account.lamports().to_le_bytes(),
        ])
        .to_bytes(),
    );
    Ok(AuthenticatedProductDirectGlobalLivenessAccountV1 {
        state: frame.state,
        data_id,
        authentication_id,
        observed_lamports: account.lamports(),
        stored_bump: frame.stored_bump,
    })
}

/// Consume the capitalization postwrite only after hostile-reopening the exact
/// Product founder root and its immutable binding. This is the sole raw
/// Founding-to-Active writer and remains crate-private for composition into the
/// one Product founder instruction.
#[inline(never)]
pub(crate) fn activate_product_direct_global_liveness_from_founder_v1<'state>(
    program_id: &Pubkey,
    capitalization: AuthenticatedProductDirectGlobalLivenessCapitalizationV1,
    founder_creation: AuthenticatedProductMarketFounderCreationV1,
    manifest_account: &AccountInfo<'_>,
    root_account: &AccountInfo<'_>,
    root_output: &'state mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedProductDirectGlobalLivenessActivationV1> {
    require(
        manifest_account.key != root_account.key,
        ClutchError::AccountAlias,
    )?;
    let current = authenticate_product_direct_global_liveness_v1(
        program_id,
        manifest_account,
        true,
    )?;
    let expected_manifest_balance = capitalization
        .state()
        .manifest_rent_principal_lamports()
        .checked_add(
            capitalization
                .state()
                .manifest_initial_donation_lamports(),
        )
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        capitalization.state().phase() == DirectGlobalLivenessPhaseV1::Founding
            && current.state() == capitalization.state()
            && current.data_id() == capitalization.account_data_id()
            && current.authentication_id() == capitalization.account_authentication_id()
            && current.observed_lamports() == expected_manifest_balance,
        ClutchError::MismatchedState,
    )?;

    let facts = founder_creation.facts();
    require(
        facts.root_account == *root_account.key
            && capitalization.state().lifecycle_root_account().bytes()
                == root_account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let root = authenticate_market_lifecycle_root_v1(
        program_id,
        root_account,
        capitalization.state().market_instance_id(),
        capitalization.state().generation(),
        true,
        root_output,
    )?;
    let root_semantic_id = root
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_binding_id = root
        .state()
        .binding()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        root.state().phase() == MarketLifecyclePhaseV1::Founding
            && root_semantic_id == founder_creation.root_semantic_id()
            && root.authentication_id() == founder_creation.root_authentication_id()
            && root_binding_id == capitalization.state().market_binding_id(),
        ClutchError::MismatchedState,
    )?;

    let founder_creation_receipt_id = founder_creation.id();
    let next = capitalization
        .state()
        .activate_founder(
            &ExactFounderActivationAuthorityV1 {
                expected_state: capitalization.state(),
                expected_founder_receipt_id: founder_creation_receipt_id,
            },
            founder_creation_receipt_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let capitalization_authentication_id = capitalization.account_authentication_id();
    let stored_bump = current.stored_bump;
    drop(current);
    write_manifest_state_v1(
        manifest_account,
        &next,
        next.manifest_rent_principal_lamports(),
        stored_bump,
    )?;
    drop(capitalization);
    require(
        manifest_account.lamports() == expected_manifest_balance,
        ClutchError::MismatchedState,
    )?;
    let reopened = authenticate_product_direct_global_liveness_v1(
        program_id,
        manifest_account,
        true,
    )?;
    require(
        reopened.state() == &next
            && reopened.observed_lamports() == expected_manifest_balance,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_DIRECT_FOUNDER_ACTIVATION_DOMAIN_V1,
            program_id.as_ref(),
            manifest_account.key.as_ref(),
            &capitalization_authentication_id.bytes(),
            &founder_creation_receipt_id.bytes(),
            &root_semantic_id.bytes(),
            &root.authentication_id().bytes(),
            &reopened.data_id().bytes(),
            &reopened.authentication_id().bytes(),
        ])
        .to_bytes(),
    );
    id.validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(AuthenticatedProductDirectGlobalLivenessActivationV1 {
        id,
        state: next,
        account_data_id: reopened.data_id(),
        account_authentication_id: reopened.authentication_id(),
        founder_creation_receipt_id,
        root_semantic_id,
        root_authentication_id: root.authentication_id(),
    })
}

/// Retire exactly one Product allocation from Direct's final sealed action-13
/// plan. The private plan's Product authority must independently accept the
/// same live Product-family prestate; a provisional plan or a replay before
/// the eighth Candidate call cannot reach the `0xba` writer.
#[inline(never)]
pub(crate) fn retire_product_direct_candidate_liveness_v1(
    program_id: &Pubkey,
    product_root: AuthenticatedMarketLifecycleRootV1<'_>,
    plan: &DirectFamilyTerminalPlanV1,
    family_terminal_sequence: u32,
    manifest_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedProductDirectCandidateRetirementV1> {
    require(
        product_root.is_writable()
            && product_root.account() != *manifest_account.key
            && matches!(
                product_root.state().phase(),
                MarketLifecyclePhaseV1::Active | MarketLifecyclePhaseV1::Retiring
            )
            && plan.replay_post.phase() == DirectReplayPhaseV1::Terminal
            && !plan.replay_post.candidate_liveness_pending()
            && plan.replay_post.candidate_liveness_completed_calls()
                == DIRECT_GLOBAL_LIVENESS_ALLOCATION_CALL_WIDTH_V1
            && plan.replay_post.candidate_liveness_last_receipt_id() != [0; 32]
            && plan.replay_post.candidate_liveness_batch_receipt_id() != [0; 32]
            && plan.replay_post.family_terminal_receipt_id()
                == plan.terminal_receipt_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let current = authenticate_product_direct_global_liveness_v1(
        program_id,
        manifest_account,
        true,
    )?;
    let binding = product_root.state().binding();
    let binding_id = binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let direct_family = product_root
        .state()
        .product_families()
        .family(MarketFamilyV1::Direct);
    let counts = direct_family.counts();
    require(
        current.state().phase() == DirectGlobalLivenessPhaseV1::Active
            && current.state().market_instance_id() == binding.market_instance_id
            && current.state().generation() == binding.generation
            && current.state().lifecycle_root_account().bytes()
                == product_root.account().to_bytes()
            && current.state().market_binding_id() == binding_id
            && current.state().realm_id() == binding.realm_id
            && current.state().neutral_lamport_sink()
                == product_root.state().capital().neutral_lamport_sink
            && current.state().admitted_allocations() == counts.admitted
            && current.state().live_allocations() == counts.live
            && current.state().retired_allocations() == counts.terminal
            && family_terminal_sequence == counts.terminal,
        ClutchError::MismatchedState,
    )?;

    let product_post = product_root
        .state()
        .terminalize_product_family_child(
            &plan.product_authority,
            MarketFamilyV1::Direct,
            family_terminal_sequence,
            plan.terminal_receipt_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let product_family_poststate_id = product_post
        .product_families()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let next = current
        .state()
        .retire_candidate(
            &ExactDirectCandidateRetirementAuthorityV1 {
                expected_state: current.state(),
                expected_terminal_receipt_id: plan.terminal_receipt_id,
                expected_family_terminal_sequence: family_terminal_sequence,
            },
            plan.terminal_receipt_id,
            family_terminal_sequence,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let observed_lamports = current.observed_lamports();
    let stored_bump = current.stored_bump;
    drop(current);
    write_manifest_state_v1(
        manifest_account,
        &next,
        next.manifest_rent_principal_lamports(),
        stored_bump,
    )?;
    require(
        manifest_account.lamports() == observed_lamports,
        ClutchError::MismatchedState,
    )?;
    let reopened = authenticate_product_direct_global_liveness_v1(
        program_id,
        manifest_account,
        true,
    )?;
    require(
        reopened.state() == &next && reopened.observed_lamports() == observed_lamports,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_DIRECT_CANDIDATE_RETIREMENT_DOMAIN_V1,
            program_id.as_ref(),
            manifest_account.key.as_ref(),
            product_root.account().as_ref(),
            &product_root.authentication_id().bytes(),
            &plan.terminal_receipt_id.bytes(),
            &family_terminal_sequence.to_le_bytes(),
            &product_family_poststate_id.bytes(),
            &reopened.data_id().bytes(),
            &reopened.authentication_id().bytes(),
        ])
        .to_bytes(),
    );
    id.validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(AuthenticatedProductDirectCandidateRetirementV1 {
        id,
        state: next,
        account_data_id: reopened.data_id(),
        account_authentication_id: reopened.authentication_id(),
        direct_terminal_receipt_id: plan.terminal_receipt_id,
        family_terminal_sequence,
        product_family_poststate_id,
    })
}

#[inline(never)]
fn authenticate_policy_v1(
    program_id: &Pubkey,
    founder: &AuthenticatedProductMarketFounderFoundationPreauthorizationV1,
    policy_account: &AccountInfo<'_>,
    rent: &RentParameters,
) -> Outcome<AuthenticatedRuntimeLivenessPolicyFactsV1> {
    require(
        policy_account.owner == program_id
            && !policy_account.is_writable
            && !policy_account.is_signer
            && !policy_account.executable
            && policy_account.data_len() == RUNTIME_LIVENESS_POLICY_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    let data = policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let data_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[&data[..]]).to_bytes(),
    );
    let policy = RuntimeLivenessPolicyV1::decode(&data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(data);
    require(
        policy.realm_id.bytes() == founder.liveness_realm_id().bytes()
            && policy.neutral_sink.bytes() == founder.neutral_lamport_sink().to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let minimum = rent.minimum_balance(RUNTIME_LIVENESS_ACCOUNT_BYTES_V1)?;
    let mut index = 0usize;
    while index < RUNTIME_COMPARTMENT_COUNT_V1 {
        require(
            policy.compartments[index].kind == RUNTIME_COMPARTMENT_ORDER_V1[index]
                && policy.compartments[index].account_rent_principal_lamports == minimum
                && policy.compartments[index].receipt_program_id.bytes() == program_id.to_bytes(),
            ClutchError::MismatchedState,
        )?;
        index += 1;
    }
    Ok(AuthenticatedRuntimeLivenessPolicyFactsV1 {
        policy_id: ContentId::from_bytes(policy.policy_id.bytes()),
        realm_id: ContentId::from_bytes(policy.realm_id.bytes()),
        neutral_sink: ContentId::from_bytes(policy.neutral_sink.bytes()),
        data_id,
    })
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn capitalize_row_v1<'a>(
    program_id: &Pubkey,
    policy_account: &AccountInfo<'a>,
    policy_facts: &AuthenticatedRuntimeLivenessPolicyFactsV1,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    lifecycle_id: ContentId,
    semantic_owner: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    kind: RuntimeCompartmentKindV1,
) -> Outcome<RowCapitalizationFactsV1> {
    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let policy_data_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[&policy_data[..]]).to_bytes(),
    );
    let policy = RuntimeLivenessPolicyV1::decode(&policy_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(policy_data);
    require(
        policy_data_id == policy_facts.data_id
            && policy.policy_id.bytes() == policy_facts.policy_id.bytes()
            && policy.realm_id.bytes() == policy_facts.realm_id.bytes()
            && policy.neutral_sink.bytes() == policy_facts.neutral_sink.bytes(),
        ClutchError::MismatchedState,
    )?;
    let index = kind.index();
    let kind_byte = u8::try_from(index).map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    let (expected_account, bump) = seeds::product_direct_global_liveness_compartment_pda(
        program_id,
        &market_instance_id.bytes(),
        generation,
        kind_byte,
    );
    expect_pda(account.key, (expected_account, bump), None)?;
    let compartment_policy = policy.compartments[index];
    let work_principal_lamports = compartment_policy.work_capital_lamports;
    let rent_principal_lamports = compartment_policy.account_rent_principal_lamports;
    let payer_debit_lamports = work_principal_lamports
        .checked_add(rent_principal_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let initial_donation_lamports = account.lamports();
    let bump_seed = [bump];
    full_payer_fund_allocate_assign_v1(
        program_id,
        payer,
        account,
        system_program,
        payer_debit_lamports,
        RUNTIME_LIVENESS_ACCOUNT_BYTES_V1,
        &[
            seeds::SEED_PRODUCT_DIRECT_GLOBAL_LIVENESS_COMPARTMENT,
            &market_instance_id.bytes(),
            &generation.to_le_bytes(),
            &[kind_byte],
            &bump_seed,
        ],
    )?;
    let state = RuntimeCompartmentV1::admit(
        policy,
        RuntimeCompartmentAdmissionV1 {
            kind,
            identity: RuntimeCompartmentIdentityV1 {
                policy_id: policy.policy_id,
                lifecycle_id: LivenessId::from_bytes(lifecycle_id.bytes()),
                account_id: LivenessId::from_bytes(account.key.to_bytes()),
                owner: LivenessId::from_bytes(semantic_owner.key.to_bytes()),
                payer: LivenessId::from_bytes(payer.key.to_bytes()),
                neutral_sink: policy.neutral_sink,
                generation,
            },
            funding: PresentFundingV1 {
                payer: LivenessId::from_bytes(payer.key.to_bytes()),
                source: PresentFundingSourceV1::ExternalSignerNativeLamports,
                payer_debit_lamports,
                account_balance_before: initial_donation_lamports,
                account_balance_after: account.lamports(),
            },
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut postimage = [0u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1];
    state
        .encode(&mut postimage)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_exact_v1(account, &postimage)?;
    let reopened_data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let reopened = RuntimeCompartmentV1::decode(&reopened_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let data_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[&reopened_data[..]]).to_bytes(),
    );
    drop(reopened_data);
    require(
        reopened == state
            && account.lamports()
                == initial_donation_lamports
                    .checked_add(payer_debit_lamports)
                    .ok_or(ClutchError::Arithmetic)?,
        ClutchError::MismatchedState,
    )?;
    let capitalization_receipt_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_DIRECT_ROW_CAPITALIZATION_DOMAIN_V1,
            program_id.as_ref(),
            &market_instance_id.bytes(),
            &generation.to_le_bytes(),
            &lifecycle_id.bytes(),
            &[kind_byte],
            account.key.as_ref(),
            &data_id.bytes(),
            semantic_owner.key.as_ref(),
            payer.key.as_ref(),
            &compartment_policy.quote_schedule_id.bytes(),
            &compartment_policy.receipt_program_id.bytes(),
            &work_principal_lamports.to_le_bytes(),
            &rent_principal_lamports.to_le_bytes(),
            &initial_donation_lamports.to_le_bytes(),
            &account.lamports().to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!capitalization_receipt_id.is_zero(), ClutchError::MismatchedState)?;
    Ok(RowCapitalizationFactsV1 {
        account_id: ContentId::from_bytes(account.key.to_bytes()),
        capitalization_receipt_id,
        work_principal_lamports,
        rent_principal_lamports,
        initial_donation_lamports,
        maximum_calls: compartment_policy.maximum_calls,
    })
}

#[allow(clippy::too_many_arguments)]
fn authenticate_fixed_roles_v1(
    program_id: &Pubkey,
    founder: &AuthenticatedProductMarketFounderFoundationPreauthorizationV1,
    policy_account: &AccountInfo<'_>,
    manifest_account: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    neutral_lamport_sink: &AccountInfo<'_>,
    compartments: &[AccountInfo<'_>],
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<()> {
    require_distinct(compartments)?;
    require(
        payer.key.to_bytes() == founder.principal_refund_owner().to_bytes()
            && payer.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && payer.data_is_empty()
            && payer.is_writable
            && payer.is_signer
            && !payer.executable
            && neutral_lamport_sink.key.to_bytes()
                == founder.neutral_lamport_sink().to_bytes()
            && neutral_lamport_sink.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && neutral_lamport_sink.data_is_empty()
            && neutral_lamport_sink.is_writable
            && !neutral_lamport_sink.is_signer
            && !neutral_lamport_sink.executable,
        ClutchError::MismatchedState,
    )?;
    let expected_roles = [
        *policy_account.key,
        *manifest_account.key,
        *payer.key,
        *neutral_lamport_sink.key,
        *system_program.key,
        *rent_sysvar.key,
        founder.lifecycle_root_account(),
        founder.founder_link_account(),
        founder.lifecycle_replay_account(),
    ];
    let mut left = 0usize;
    while left < expected_roles.len() {
        let mut right = left + 1;
        while right < expected_roles.len() {
            require(expected_roles[left] != expected_roles[right], ClutchError::AccountAlias)?;
            right += 1;
        }
        left += 1;
    }
    for account in compartments {
        for role in expected_roles {
            require(*account.key != role, ClutchError::AccountAlias)?;
        }
        require(
            account.is_writable && !account.is_signer && !account.executable,
            ClutchError::MismatchedState,
        )?;
    }
    let market = founder.market_instance_id().bytes();
    let (expected_manifest, _) = seeds::product_direct_global_liveness_pda(
        program_id,
        &market,
        founder.generation(),
    );
    require(
        manifest_account.key == &expected_manifest
            && manifest_account.is_writable
            && !manifest_account.is_signer
            && !manifest_account.executable,
        ClutchError::WrongPda,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn full_payer_fund_allocate_assign_v1<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    payer_debit_lamports: u64,
    space: usize,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_creatable(target)?;
    require(
        payer_debit_lamports != 0 && payer.key != target.key,
        ClutchError::MismatchedState,
    )?;
    let payer_before = payer.lamports();
    let target_before = target.lamports();
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(payer_debit_lamports),
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
    require(
        payer.lamports()
            == payer_before
                .checked_sub(payer_debit_lamports)
                .ok_or(ClutchError::Arithmetic)?
            && target.lamports()
                == target_before
                    .checked_add(payer_debit_lamports)
                    .ok_or(ClutchError::Arithmetic)?,
        ClutchError::AccountCreationFailed,
    )?;
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(space),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(&allocate, &[target.clone(), system_program.clone()], &[signer_seeds])
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(&assign, &[target.clone(), system_program.clone()], &[signer_seeds])
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.owner == program_id
            && target.data_len() == space
            && target.lamports()
                == target_before
                    .checked_add(payer_debit_lamports)
                    .ok_or(ClutchError::Arithmetic)?,
        ClutchError::AccountCreationFailed,
    )
}

#[inline(never)]
fn write_manifest_state_v1(
    account: &AccountInfo<'_>,
    state: &DirectGlobalLivenessV1,
    rent_principal_lamports: u64,
    stored_bump: u8,
) -> Outcome<()> {
    let mut postimage = [0u8; PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_BYTES_V1];
    ProductDirectGlobalLivenessAccountV1::encode_parts(
        state,
        rent_principal_lamports,
        stored_bump,
        &mut postimage,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_exact_v1(account, &postimage)
}

fn write_exact_v1(account: &AccountInfo<'_>, bytes: &[u8]) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(data.len() == bytes.len(), ClutchError::WrongDataLength)?;
    data.copy_from_slice(bytes);
    Ok(())
}

fn account_data_id_v1(account: &AccountInfo<'_>) -> Outcome<ContentId> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[&data[..]]).to_bytes(),
    ))
}

fn global_lifecycle_id_v1(
    program_id: &Pubkey,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    lifecycle_root_account: ContentId,
    policy_id: ContentId,
) -> ContentId {
    ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_DIRECT_GLOBAL_LIFECYCLE_DOMAIN_V1,
            program_id.as_ref(),
            &market_instance_id.bytes(),
            &generation.to_le_bytes(),
            &lifecycle_root_account.bytes(),
            &policy_id.bytes(),
        ])
        .to_bytes(),
    )
}

#[allow(clippy::too_many_arguments)]
fn global_bundle_binding_id_v1(
    account_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    lifecycle_root_account: ContentId,
    market_binding_id: ContentId,
    realm_id: ContentId,
    policy_account: ContentId,
    policy_id: ContentId,
    policy_data_id: ContentId,
    global_lifecycle_id: ContentId,
    principal_refund_owner: ContentId,
    neutral_lamport_sink: ContentId,
    generation: u64,
    compartment_accounts: &[ContentId; RUNTIME_COMPARTMENT_COUNT_V1],
    compartment_receipts: &[ContentId; RUNTIME_COMPARTMENT_COUNT_V1],
) -> ContentId {
    let mut body = [0u8; 11 * 32 + 8 + 2 * RUNTIME_COMPARTMENT_COUNT_V1 * 32];
    let mut at = 0usize;
    for id in [
        account_id,
        market_instance_id.content_id(),
        lifecycle_root_account,
        market_binding_id,
        realm_id,
        policy_account,
        policy_id,
        policy_data_id,
        global_lifecycle_id,
        principal_refund_owner,
        neutral_lamport_sink,
    ] {
        body[at..at + 32].copy_from_slice(&id.bytes());
        at += 32;
    }
    body[at..at + 8].copy_from_slice(&generation.to_le_bytes());
    at += 8;
    for account in compartment_accounts {
        body[at..at + 32].copy_from_slice(&account.bytes());
        at += 32;
    }
    for receipt in compartment_receipts {
        body[at..at + 32].copy_from_slice(&receipt.bytes());
        at += 32;
    }
    ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[DIRECT_GLOBAL_LIVENESS_BINDING_DOMAIN_V1, &body]).to_bytes(),
    )
}

#[allow(clippy::too_many_arguments)]
fn global_capitalization_receipt_id_v1(
    global_bundle_binding_id: ContentId,
    compartment_receipts: &[ContentId; RUNTIME_COMPARTMENT_COUNT_V1],
    total_work_principal_lamports: u64,
    total_rent_principal_lamports: u64,
    initial_bundle_donation_lamports: u64,
    manifest_rent_principal_lamports: u64,
    manifest_initial_donation_lamports: u64,
    payer_balance_before: u64,
    payer_balance_after: u64,
) -> ContentId {
    let mut body = [0u8; 32 + RUNTIME_COMPARTMENT_COUNT_V1 * 32 + 7 * 8];
    let mut at = 0usize;
    body[..32].copy_from_slice(&global_bundle_binding_id.bytes());
    at += 32;
    for receipt in compartment_receipts {
        body[at..at + 32].copy_from_slice(&receipt.bytes());
        at += 32;
    }
    for amount in [
        total_work_principal_lamports,
        total_rent_principal_lamports,
        initial_bundle_donation_lamports,
        manifest_rent_principal_lamports,
        manifest_initial_donation_lamports,
        payer_balance_before,
        payer_balance_after,
    ] {
        body[at..at + 8].copy_from_slice(&amount.to_le_bytes());
        at += 8;
    }
    ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            DIRECT_GLOBAL_LIVENESS_CAPITALIZATION_DOMAIN_V1,
            &body,
        ])
        .to_bytes(),
    )
}
