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
use crate::seeds;
use clutch_direct_market_runtime::lifecycle_v2::DirectFamilyTerminalPlanV2;
use clutch_liveness::runtime_v1::{
    PresentFundingSourceV1, PresentFundingV1, RuntimeCompartmentAdmissionV1,
    RuntimeCompartmentIdentityV1, RuntimeCompartmentKindV1, RuntimeCompartmentV1,
    RuntimeLivenessPolicyV1, RUNTIME_COMPARTMENT_COUNT_V1, RUNTIME_COMPARTMENT_ORDER_V1,
    RUNTIME_LIVENESS_ACCOUNT_BYTES_V1, RUNTIME_LIVENESS_POLICY_BYTES_V1,
};
use clutch_liveness::Id as LivenessId;
use clutch_product_series::{
    ContentId, DirectGlobalLivenessCapitalizationV2, DirectGlobalLivenessPhaseV2,
    DirectGlobalLivenessV2, DirectWorkQuoteV1, FixedCodec,
    MarketInstanceV2Id, ProductDirectGlobalLivenessAuthorityV2,
    DIRECT_GLOBAL_LIVENESS_ALLOCATION_CALL_WIDTH_V2,
    DIRECT_GLOBAL_LIVENESS_BINDING_DOMAIN_V2,
    DIRECT_GLOBAL_LIVENESS_CAPITALIZATION_DOMAIN_V2,
};
use clutch_solana_layout::product_series::{
    ProductDirectGlobalLivenessAccountV2, PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_BYTES_V2,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::product_market_foundation_current::
    AuthenticatedProductMarketFounderFoundationPreauthorizationV3;
use super::product_series_current::{
    AuthenticatedMarketLifecycleRootV2, AuthenticatedProductSeriesActivationCompletionV4,
};
use super::product_series::physical_v4::AuthenticatedSeriesPhysicalFounderV4;

const PRODUCT_DIRECT_GLOBAL_LIFECYCLE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-direct-global-lifecycle/v2";
const PRODUCT_DIRECT_ROW_CAPITALIZATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-direct-global-row-capitalization/v2";
const PRODUCT_DIRECT_ACCOUNT_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-direct-global-account-authentication/v2";
const PRODUCT_DIRECT_FOUNDER_ACTIVATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-direct-global-founder-activation/v2";
const PRODUCT_DIRECT_CANDIDATE_RETIREMENT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-direct-candidate-retirement/v2\0";

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
    candidate_quote_schedule_id: ContentId,
}

/// Private non-detachable postwrite minted by the raw capitalization half.
///
/// The Product founder must hostile-reopen the account, compare this receipt,
/// create the root/link, and activate `0xba` before the outer call returns.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductDirectGlobalLivenessCapitalizationV2 {
    state_semantic_id: ContentId,
    global_bundle_binding_id: ContentId,
    global_capitalization_receipt_id: ContentId,
    work_quote: DirectWorkQuoteV1,
    account_data_id: ContentId,
    account_authentication_id: ContentId,
    expected_manifest_balance: u64,
    payer_balance_before: u64,
    payer_balance_after: u64,
    total_payer_debit_lamports: u64,
}

impl AuthenticatedProductDirectGlobalLivenessCapitalizationV2 {
    pub(crate) const fn state_semantic_id(&self) -> ContentId { self.state_semantic_id }
    pub(crate) const fn account_data_id(&self) -> ContentId { self.account_data_id }
    pub(crate) const fn account_authentication_id(&self) -> ContentId {
        self.account_authentication_id
    }
    pub(crate) const fn payer_balance_before(&self) -> u64 { self.payer_balance_before }
    pub(crate) const fn payer_balance_after(&self) -> u64 { self.payer_balance_after }
    pub(crate) const fn total_payer_debit_lamports(&self) -> u64 {
        self.total_payer_debit_lamports
    }
    pub(crate) const fn global_bundle_binding_id(&self) -> ContentId {
        self.global_bundle_binding_id
    }
    pub(crate) const fn global_capitalization_receipt_id(&self) -> ContentId {
        self.global_capitalization_receipt_id
    }
    pub(crate) const fn work_quote(&self) -> DirectWorkQuoteV1 {
        self.work_quote
    }
    pub(crate) const fn expected_manifest_balance(&self) -> u64 {
        self.expected_manifest_balance
    }
}

/// Hostile-reopened current `0xba/v2` account authentication.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductDirectGlobalLivenessAccountV2 {
    state: DirectGlobalLivenessV2,
    data_id: ContentId,
    authentication_id: ContentId,
    observed_lamports: u64,
    stored_bump: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedProductDirectGlobalLivenessPostwriteV2 {
    data_id: ContentId,
    authentication_id: ContentId,
}

impl AuthenticatedProductDirectGlobalLivenessAccountV2 {
    pub(crate) const fn state(&self) -> &DirectGlobalLivenessV2 { &self.state }
    pub(crate) const fn data_id(&self) -> ContentId { self.data_id }
    pub(crate) const fn authentication_id(&self) -> ContentId { self.authentication_id }
    pub(crate) const fn observed_lamports(&self) -> u64 { self.observed_lamports }
    pub(crate) const fn global_bundle_binding_id(&self) -> ContentId {
        self.state.global_bundle_binding_id()
    }
    pub(crate) const fn activated_market_binding_id(&self) -> ContentId {
        self.state.activated_market_binding_id()
    }
    pub(crate) const fn work_quote(&self) -> DirectWorkQuoteV1 {
        self.state.work_quote()
    }
    pub(crate) fn into_state(self) -> DirectGlobalLivenessV2 { self.state }
}

/// Move-only proof that the sole live Direct allocation was retired by the
/// sealed eighth Candidate call. Product RootV3 consumes this before it may
/// mark the Direct family terminal.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductDirectCandidateRetirementV2 {
    id: ContentId,
    account: Pubkey,
    data_before: ContentId,
    data_after: ContentId,
    authentication_before: ContentId,
    authentication_after: ContentId,
    state_before: ContentId,
    state_after: ContentId,
    direct_terminal_receipt_id: ContentId,
    family_terminal_sequence: u32,
    lifecycle_root_account: ContentId,
    activated_market_binding_id: ContentId,
}

impl AuthenticatedProductDirectCandidateRetirementV2 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn account(&self) -> Pubkey { self.account }
    pub(crate) const fn state_before(&self) -> ContentId { self.state_before }
    pub(crate) const fn state_after(&self) -> ContentId { self.state_after }
    pub(crate) const fn data_before(&self) -> ContentId { self.data_before }
    pub(crate) const fn data_after(&self) -> ContentId { self.data_after }
    pub(crate) const fn authentication_before(&self) -> ContentId {
        self.authentication_before
    }
    pub(crate) const fn authentication_after(&self) -> ContentId {
        self.authentication_after
    }
    pub(crate) const fn direct_terminal_receipt_id(&self) -> ContentId {
        self.direct_terminal_receipt_id
    }
    pub(crate) const fn family_terminal_sequence(&self) -> u32 {
        self.family_terminal_sequence
    }
    pub(crate) const fn lifecycle_root_account(&self) -> ContentId {
        self.lifecycle_root_account
    }
    pub(crate) const fn activated_market_binding_id(&self) -> ContentId {
        self.activated_market_binding_id
    }
}

struct ExactDirectCandidateRetirementAuthorityV2 {
    state_before: ContentId,
    terminal_receipt_id: ContentId,
    family_terminal_sequence: u32,
}

/// Default-refusing physical action-13 postwrite boundary. Direct implements
/// this only after it has written and hostile-reopened final b3 and Candidate;
/// Product therefore cannot retire `0xba/v2` from the pure sealed plan alone.
pub(crate) trait AuthenticatedDirectCandidateTerminalPostwriteV2 {
    #[allow(clippy::too_many_arguments)]
    fn authenticate_direct_candidate_terminal_postwrite_v2(
        &self,
        _direct_root_account: ContentId,
        _replay_account: ContentId,
        _candidate_account: ContentId,
        _terminal_receipt_id: ContentId,
        _completed_calls: u32,
        _last_work_receipt_id: ContentId,
        _batch_receipt_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

impl ProductDirectGlobalLivenessAuthorityV2
    for ExactDirectCandidateRetirementAuthorityV2
{
    fn authenticate_candidate_retirement(
        &self,
        state: &DirectGlobalLivenessV2,
        direct_terminal_receipt_id: ContentId,
        family_terminal_sequence: u32,
    ) -> clutch_product_series::Result<()> {
        if state.semantic_id()? != self.state_before
            || direct_terminal_receipt_id != self.terminal_receipt_id
            || family_terminal_sequence != self.family_terminal_sequence
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

/// Retire the exact live Candidate allocation only after Direct has sealed its
/// final b3 transcript. Raw terminal IDs and caller-shaped Product states are
/// not accepted at this boundary.
#[inline(never)]
pub(crate) fn retire_product_direct_candidate_allocation_v2<
    A: AuthenticatedDirectCandidateTerminalPostwriteV2 + ?Sized,
>(
    program_id: &Pubkey,
    manifest_account: &AccountInfo<'_>,
    sealed: &DirectFamilyTerminalPlanV2,
    postwrite: &A,
) -> Outcome<AuthenticatedProductDirectCandidateRetirementV2> {
    let authenticated = authenticate_product_direct_global_liveness_v2(
        program_id,
        manifest_account,
        true,
    )?;
    let state_before = authenticated
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let terminal_receipt_id = ContentId::from_bytes(sealed.terminal_receipt_id);
    let family_terminal_sequence = sealed.family_terminal_sequence;
    let lifecycle_root_account = authenticated.state().lifecycle_root_account();
    let activated_market_binding_id = authenticated.state().activated_market_binding_id();
    require(
        authenticated.state().phase() == DirectGlobalLivenessPhaseV2::Active
            && authenticated.state().live_allocations() == 1
            && authenticated.state().retired_allocations() == family_terminal_sequence
            && authenticated.state().account_id().bytes() == manifest_account.key.to_bytes()
            && sealed.replay_post.candidate_liveness_completed_calls() == 8
            && !sealed.replay_post.candidate_liveness_pending()
            && sealed.replay_post.family_terminal_receipt_id()
                == sealed.terminal_receipt_id,
        ClutchError::MismatchedState,
    )?;
    postwrite.authenticate_direct_candidate_terminal_postwrite_v2(
        ContentId::from_bytes(sealed.replay_post.direct_root_account()),
        ContentId::from_bytes(sealed.replay_post.replay_account()),
        authenticated.state().compartment_account(
            RuntimeCompartmentKindV1::Candidate.index(),
        ).ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?,
        terminal_receipt_id,
        sealed.replay_post.candidate_liveness_completed_calls(),
        ContentId::from_bytes(sealed.replay_post.candidate_liveness_last_receipt_id()),
        ContentId::from_bytes(sealed.replay_post.candidate_liveness_batch_receipt_id()),
    )?;
    let data_before = authenticated.data_id();
    let authentication_before = authenticated.authentication_id();
    let observed_lamports = authenticated.observed_lamports();
    let stored_bump = authenticated.stored_bump;
    let next = authenticated.into_state().retire_candidate(
        &ExactDirectCandidateRetirementAuthorityV2 {
            state_before,
            terminal_receipt_id,
            family_terminal_sequence,
        },
        terminal_receipt_id,
        family_terminal_sequence,
    ).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_manifest_state_v2(
        manifest_account,
        &next,
        next.manifest_rent_principal_lamports(),
        stored_bump,
    )?;
    let state_after = next.semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let reopened = authenticate_expected_product_direct_global_liveness_postwrite_v2(
        program_id,
        manifest_account,
        state_after,
        observed_lamports,
    )?;
    let id = ContentId::from_bytes(solana_sha256_hasher::hashv(&[
        PRODUCT_DIRECT_CANDIDATE_RETIREMENT_DOMAIN_V2,
        program_id.as_ref(),
        manifest_account.key.as_ref(),
        &data_before.bytes(),
        &reopened.data_id.bytes(),
        &authentication_before.bytes(),
        &reopened.authentication_id.bytes(),
        &state_before.bytes(),
        &state_after.bytes(),
        &terminal_receipt_id.bytes(),
        &family_terminal_sequence.to_le_bytes(),
        &lifecycle_root_account.bytes(),
        &activated_market_binding_id.bytes(),
    ]).to_bytes());
    require(!id.is_zero() && state_before != state_after, ClutchError::MismatchedState)?;
    Ok(AuthenticatedProductDirectCandidateRetirementV2 {
        id,
        account: *manifest_account.key,
        data_before,
        data_after: reopened.data_id,
        authentication_before,
        authentication_after: reopened.authentication_id,
        state_before,
        state_after,
        direct_terminal_receipt_id: terminal_receipt_id,
        family_terminal_sequence,
        lifecycle_root_account,
        activated_market_binding_id,
    })
}

/// Private postwrite proving that the exact newly-created Product root consumed
/// the full-payer `0xba` capitalization before Direct allocation became live.
///
/// This value is intentionally non-`Copy` and non-`Clone`. The Product founder
/// outer must move it into the current root successor; neither the raw
/// capitalization receipt nor a caller-supplied Market binding can activate
/// the account.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductDirectGlobalLivenessActivationV2 {
    id: ContentId,
    state: DirectGlobalLivenessV2,
    account_data_id: ContentId,
    account_authentication_id: ContentId,
    founder_creation_receipt_id: ContentId,
    root_semantic_id: ContentId,
    root_authentication_id: ContentId,
}

impl AuthenticatedProductDirectGlobalLivenessActivationV2 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn state(&self) -> &DirectGlobalLivenessV2 { &self.state }
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
    pub(crate) const fn account_id(&self) -> ContentId { self.state.account_id() }
    pub(crate) const fn global_bundle_binding_id(&self) -> ContentId {
        self.state.global_bundle_binding_id()
    }
    pub(crate) const fn activated_market_binding_id(&self) -> ContentId {
        self.state.activated_market_binding_id()
    }
    pub(crate) const fn work_quote(&self) -> DirectWorkQuoteV1 {
        self.state.work_quote()
    }
    pub(crate) fn work_quote_id(&self) -> Outcome<ContentId> {
        self.state
            .work_quote_id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
    }
    pub(crate) const fn transition_sequence(&self) -> u64 {
        self.state.transition_sequence()
    }
}

#[derive(Clone, Copy, Debug)]
struct ExactCapitalizationAuthorityV2;

impl ProductDirectGlobalLivenessAuthorityV2 for ExactCapitalizationAuthorityV2 {
    fn authenticate_capitalization(
        &self,
        _capitalization: &DirectGlobalLivenessCapitalizationV2,
    ) -> clutch_product_series::Result<()> {
        // Private construction is the authority: this module derives every
        // field from live account poststates immediately before initialization.
        Ok(())
    }
}

struct ExactFounderActivationAuthorityV2 {
    expected_state_semantic_id: ContentId,
    expected_founder_receipt_id: ContentId,
    expected_market_binding_id: ContentId,
}

impl ProductDirectGlobalLivenessAuthorityV2 for ExactFounderActivationAuthorityV2 {
    fn authenticate_founder_activation(
        &self,
        state: &DirectGlobalLivenessV2,
        founder_receipt_id: ContentId,
        activated_market_binding_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if state.semantic_id()? == self.expected_state_semantic_id
            && founder_receipt_id == self.expected_founder_receipt_id
            && activated_market_binding_id == self.expected_market_binding_id
        {
            Ok(())
        } else {
            Err(clutch_product_series::Error::UnauthenticatedAuthority)
        }
    }
}

fn require_direct_work_quote_authority_v1(
    work_quote: DirectWorkQuoteV1,
    candidate_lifecycle_policy_id: ContentId,
    candidate_liveness_policy_id: ContentId,
    failure_liveness_policy_id: ContentId,
    runtime_policy_id: ContentId,
    candidate_quote_schedule_id: ContentId,
) -> Outcome<()> {
    let quote_id = work_quote
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        work_quote.candidate_lifecycle_policy_id == candidate_lifecycle_policy_id
            && work_quote.candidate_liveness_policy_id == candidate_liveness_policy_id
            && runtime_policy_id == failure_liveness_policy_id
            && quote_id == candidate_quote_schedule_id,
        ClutchError::MismatchedState,
    )
}

/// Capitalize all seven generic runtime accounts plus the separate Product
/// manifest. This is a raw half: it is safe only because it is crate-private
/// and no route calls it except the eventual atomic Product founder outer.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(super) fn capitalize_product_direct_global_liveness_v2<'a>(
    program_id: &Pubkey,
    founder: &AuthenticatedProductMarketFounderFoundationPreauthorizationV3,
    work_quote_bytes: &[u8],
    policy_account: &AccountInfo<'a>,
    manifest_account: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    compartments: &[AccountInfo<'a>],
    system_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
) -> Outcome<AuthenticatedProductDirectGlobalLivenessCapitalizationV2> {
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
    let work_quote = DirectWorkQuoteV1::decode(work_quote_bytes)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let founder_preauthorization_id = founder.id();

    let market_instance_id = founder.market_instance_id();
    let generation = founder.generation();
    let global_lifecycle_id = global_lifecycle_id_v2(
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
    require_direct_work_quote_authority_v1(
        work_quote,
        founder.candidate_lifecycle_policy_id(),
        founder.candidate_liveness_policy_id(),
        founder.failure_liveness_policy_id(),
        policy.policy_id,
        policy.candidate_quote_schedule_id,
    )?;
    require(
        candidate_maximum_calls >= DIRECT_GLOBAL_LIVENESS_ALLOCATION_CALL_WIDTH_V2
            && candidate_work_principal_lamports
                >= work_quote
                    .reserved_work_lamports()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ClutchError::MismatchedState,
    )?;

    let manifest_rent_principal_lamports =
        rent.minimum_balance(PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_BYTES_V2)?;
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
    let global_bundle_binding_id = global_bundle_binding_id_v2(
        manifest_id,
        market_instance_id,
        lifecycle_root_account,
        founder_preauthorization_id,
        realm_id,
        ContentId::from_bytes(policy_account.key.to_bytes()),
        policy_id,
        policy_data_id,
        work_quote.candidate_lifecycle_policy_id,
        work_quote.id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
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
    let global_capitalization_receipt_id = global_capitalization_receipt_id_v2(
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
    let capitalization = DirectGlobalLivenessCapitalizationV2 {
        account_id: manifest_id,
        market_instance_id,
        lifecycle_root_account,
        founder_preauthorization_id,
        work_quote,
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
        allocation_call_width: DIRECT_GLOBAL_LIVENESS_ALLOCATION_CALL_WIDTH_V2,
    };
    let state = DirectGlobalLivenessV2::initialize(&ExactCapitalizationAuthorityV2, capitalization)
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
        PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_BYTES_V2,
        &[
            seeds::SEED_PRODUCT_DIRECT_GLOBAL_LIVENESS,
            &market_instance_id.bytes(),
            &generation.to_le_bytes(),
            &manifest_bump_seed,
        ],
    )?;
    let state_semantic_id = state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_manifest_balance = manifest_initial_donation_lamports
        .checked_add(manifest_rent_principal_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    write_manifest_state_v2(
        manifest_account,
        &state,
        manifest_rent_principal_lamports,
        manifest_bump,
    )?;
    require(
        payer.lamports() == payer_balance_after
            && manifest_account.lamports()
                == expected_manifest_balance,
        ClutchError::AccountCreationFailed,
    )?;
    drop(state);
    let reopened = authenticate_expected_product_direct_global_liveness_postwrite_v2(
        program_id,
        manifest_account,
        state_semantic_id,
        expected_manifest_balance,
    )?;
    Ok(AuthenticatedProductDirectGlobalLivenessCapitalizationV2 {
        state_semantic_id,
        global_bundle_binding_id,
        global_capitalization_receipt_id,
        work_quote,
        account_data_id: reopened.data_id,
        account_authentication_id: reopened.authentication_id,
        expected_manifest_balance,
        payer_balance_before,
        payer_balance_after,
        total_payer_debit_lamports,
    })
}

/// Hostile-reopen the exact current `0xba/v2` account and its complete data.
#[inline(never)]
pub(crate) fn authenticate_product_direct_global_liveness_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
) -> Outcome<AuthenticatedProductDirectGlobalLivenessAccountV2> {
    require(
        account.owner == program_id
            && !account.is_signer
            && !account.executable
            && account.is_writable == writable
            && account.data_len() == PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_BYTES_V2,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let frame = ProductDirectGlobalLivenessAccountV2::decode(&data)
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
            PRODUCT_DIRECT_ACCOUNT_AUTHENTICATION_DOMAIN_V2,
            account.key.as_ref(),
            program_id.as_ref(),
            &data_id.bytes(),
            &semantic_id.bytes(),
            &account.lamports().to_le_bytes(),
        ])
        .to_bytes(),
    );
    Ok(AuthenticatedProductDirectGlobalLivenessAccountV2 {
        state: frame.state,
        data_id,
        authentication_id,
        observed_lamports: account.lamports(),
        stored_bump: frame.stored_bump,
    })
}

#[inline(never)]
fn authenticate_expected_product_direct_global_liveness_postwrite_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_semantic_id: ContentId,
    expected_lamports: u64,
) -> Outcome<AuthenticatedProductDirectGlobalLivenessPostwriteV2> {
    let reopened = authenticate_product_direct_global_liveness_v2(program_id, account, true)?;
    let semantic_id = reopened
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        semantic_id == expected_semantic_id && reopened.observed_lamports() == expected_lamports,
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedProductDirectGlobalLivenessPostwriteV2 {
        data_id: reopened.data_id(),
        authentication_id: reopened.authentication_id(),
    })
}

/// Final current Founding-to-Active write. The caller must first consume the
/// unique creation authority through Product's concrete RootV2/LinkV2/
/// replayV2/FundingV4 tail and pass only that tail's move-only receipt.
#[inline(never)]
pub(super) fn activate_product_direct_global_liveness_from_current_founder_v2(
    program_id: &Pubkey,
    completion: AuthenticatedProductSeriesActivationCompletionV4,
    manifest_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV2<'_>,
) -> Outcome<(
    AuthenticatedProductDirectGlobalLivenessActivationV2,
    AuthenticatedSeriesPhysicalFounderV4,
)> {
    let (
        founder_creation_receipt_id,
        expected_root_account,
        expected_root_binding_id,
        expected_root_authentication_id,
        expected_root_semantic_id,
        expected_preauthorization_id,
        capitalization,
        physical,
    ) = completion.into_direct_activation_parts();
    let root_binding_id = root.state().binding_ref().id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_semantic_id = root.state().semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_authentication_id = root.authentication_id();
    require(
        root.is_writable()
            && root.state().phase() == clutch_product_series::MarketLifecyclePhaseV2::Active
            && root.account() == expected_root_account
            && root_binding_id == expected_root_binding_id
            && root.authentication_id() == expected_root_authentication_id
            && root_semantic_id == expected_root_semantic_id
            && root.state().binding().direct_global_liveness_binding_id
                == capitalization.global_bundle_binding_id(),
        ClutchError::MismatchedState,
    )?;
    require(
        root.account() == expected_root_account
            && manifest_account.key != &root.account(),
        ClutchError::AccountAlias,
    )?;
    let current = authenticate_product_direct_global_liveness_v2(
        program_id, manifest_account, true)?;
    let current_semantic_id = current.state().semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_manifest_balance = capitalization.expected_manifest_balance();
    require(
        current.state().phase() == DirectGlobalLivenessPhaseV2::Founding
            && current_semantic_id == capitalization.state_semantic_id()
            && current.global_bundle_binding_id() == capitalization.global_bundle_binding_id()
            && current.work_quote() == capitalization.work_quote()
            && current.data_id() == capitalization.account_data_id()
            && current.authentication_id() == capitalization.account_authentication_id()
            && current.observed_lamports() == expected_manifest_balance
            && current.state().founder_preauthorization_id()
                == expected_preauthorization_id
            && current.state().lifecycle_root_account().bytes()
                == root.account().to_bytes()
            && current.activated_market_binding_id() == ContentId::ZERO,
        ClutchError::MismatchedState,
    )?;
    let capitalization_authentication_id = capitalization.account_authentication_id();
    let stored_bump = current.stored_bump;
    let next = current.into_state().activate_founder(
        &ExactFounderActivationAuthorityV2 {
            expected_state_semantic_id: current_semantic_id,
            expected_founder_receipt_id: founder_creation_receipt_id,
            expected_market_binding_id: root_binding_id,
        },
        founder_creation_receipt_id,
        root_binding_id,
    ).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_manifest_state_v2(
        manifest_account, &next, next.manifest_rent_principal_lamports(), stored_bump)?;
    drop(capitalization);
    require(manifest_account.lamports() == expected_manifest_balance,
        ClutchError::MismatchedState)?;
    let next_semantic_id = next.semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let reopened = authenticate_expected_product_direct_global_liveness_postwrite_v2(
        program_id, manifest_account, next_semantic_id, expected_manifest_balance)?;
    let id = ContentId::from_bytes(solana_sha256_hasher::hashv(&[
        PRODUCT_DIRECT_FOUNDER_ACTIVATION_DOMAIN_V2,
        program_id.as_ref(), manifest_account.key.as_ref(),
        &capitalization_authentication_id.bytes(),
        &founder_creation_receipt_id.bytes(), &root_semantic_id.bytes(),
        &root_authentication_id.bytes(), &root_binding_id.bytes(),
        &reopened.data_id.bytes(), &reopened.authentication_id.bytes(),
    ]).to_bytes());
    id.validate().map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok((AuthenticatedProductDirectGlobalLivenessActivationV2 {
        id, state: next, account_data_id: reopened.data_id,
        account_authentication_id: reopened.authentication_id,
        founder_creation_receipt_id, root_semantic_id, root_authentication_id,
    }, physical))
}

#[inline(never)]
fn authenticate_policy_v1(
    program_id: &Pubkey,
    founder: &AuthenticatedProductMarketFounderFoundationPreauthorizationV3,
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
        policy_account.key == &founder.failure_liveness_policy_account()
            && policy.policy_id.bytes() == founder.failure_liveness_policy_id().bytes()
            && policy.realm_id.bytes() == founder.liveness_realm_id().bytes()
            && policy.neutral_sink.bytes() == founder.neutral_lamport_sink().to_bytes()
            && policy.compartments[RuntimeCompartmentKindV1::Recovery.index()]
                .quote_schedule_id
                .bytes()
                == founder.failure_recovery_quote_schedule_id().bytes(),
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
        candidate_quote_schedule_id: ContentId::from_bytes(
            policy.compartments[RuntimeCompartmentKindV1::Candidate.index()]
                .quote_schedule_id
                .bytes(),
        ),
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
            PRODUCT_DIRECT_ROW_CAPITALIZATION_DOMAIN_V2,
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
    founder: &AuthenticatedProductMarketFounderFoundationPreauthorizationV3,
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
fn write_manifest_state_v2(
    account: &AccountInfo<'_>,
    state: &DirectGlobalLivenessV2,
    rent_principal_lamports: u64,
    stored_bump: u8,
) -> Outcome<()> {
    let mut postimage = [0u8; PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_BYTES_V2];
    ProductDirectGlobalLivenessAccountV2::encode_parts(
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

fn global_lifecycle_id_v2(
    program_id: &Pubkey,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    lifecycle_root_account: ContentId,
    policy_id: ContentId,
) -> ContentId {
    ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_DIRECT_GLOBAL_LIFECYCLE_DOMAIN_V2,
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
fn global_bundle_binding_id_v2(
    account_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    lifecycle_root_account: ContentId,
    founder_preauthorization_id: ContentId,
    realm_id: ContentId,
    policy_account: ContentId,
    policy_id: ContentId,
    policy_data_id: ContentId,
    candidate_lifecycle_policy_id: ContentId,
    work_quote_id: ContentId,
    global_lifecycle_id: ContentId,
    principal_refund_owner: ContentId,
    neutral_lamport_sink: ContentId,
    generation: u64,
    compartment_accounts: &[ContentId; RUNTIME_COMPARTMENT_COUNT_V1],
    compartment_receipts: &[ContentId; RUNTIME_COMPARTMENT_COUNT_V1],
) -> ContentId {
    let mut body = [0u8; 13 * 32 + 8 + 2 * RUNTIME_COMPARTMENT_COUNT_V1 * 32];
    let mut at = 0usize;
    for id in [
        account_id,
        market_instance_id.content_id(),
        lifecycle_root_account,
        founder_preauthorization_id,
        realm_id,
        policy_account,
        policy_id,
        policy_data_id,
        candidate_lifecycle_policy_id,
        work_quote_id,
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
        solana_sha256_hasher::hashv(&[DIRECT_GLOBAL_LIVENESS_BINDING_DOMAIN_V2, &body]).to_bytes(),
    )
}

#[allow(clippy::too_many_arguments)]
fn global_capitalization_receipt_id_v2(
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
            DIRECT_GLOBAL_LIVENESS_CAPITALIZATION_DOMAIN_V2,
            &body,
        ])
        .to_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quote() -> DirectWorkQuoteV1 {
        DirectWorkQuoteV1 {
            candidate_lifecycle_policy_id: ContentId::from_bytes([1; 32]),
            candidate_liveness_policy_id: ContentId::from_bytes([2; 32]),
            freeze_book_lamports: 3,
            begin_verification_lamports: 5,
            verify_candidate_lamports: 7,
            finalize_selection_lamports: 11,
            economic_terminal_lamports: 13,
            retire_terminal_lamports: 17,
            retained_candidate_bond_lamports: 19,
        }
    }

    #[test]
    fn direct_work_quote_requires_candidate_anchor_and_both_genesis_owners() {
        let value = quote();
        let quote_id = value.id().expect("valid quote");
        assert!(require_direct_work_quote_authority_v1(
            value,
            value.candidate_lifecycle_policy_id,
            value.candidate_liveness_policy_id,
            ContentId::from_bytes([3; 32]),
            ContentId::from_bytes([3; 32]),
            quote_id,
        )
        .is_ok());

        for substitution in 0..5 {
            let wrong = ContentId::from_bytes([u8::try_from(40 + substitution).unwrap(); 32]);
            let result = require_direct_work_quote_authority_v1(
                value,
                if substitution == 0 { wrong } else { value.candidate_lifecycle_policy_id },
                if substitution == 1 { wrong } else { value.candidate_liveness_policy_id },
                if substitution == 2 { wrong } else { ContentId::from_bytes([3; 32]) },
                if substitution == 3 { wrong } else { ContentId::from_bytes([3; 32]) },
                if substitution == 4 { wrong } else { quote_id },
            );
            assert!(result.is_err());
        }
    }
}
