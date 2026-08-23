//! Authenticated SBF boundary for the shared Product Market lifecycle.
//!
//! The pure Product crate owns deterministic state. This module owns hostile
//! account decoding, exact `0xaa/1` and `0xad/1` PDA/owner/full-body checks,
//! atomic state writes, and private non-decodable terminal authority. Merely
//! compiling these helpers does not enable an instruction route.

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::product_artifact::{
    authenticate_product_artifact_v1, AuthenticatedRegistryCapabilityV2,
};
use crate::seeds;
use clutch_liveness::runtime_adapter_v1::{
    decode_runtime_policy_account_v1, RuntimePersistedAccountViewV1,
};
use clutch_liveness::runtime_v1::RuntimeCompartmentKindV1;
use clutch_liveness::Id as LivenessId;
use clutch_product_series::{
    CompiledProductSeriesBundleV2, ContentId, MarketInstanceTerminalProjectionV1,
    MarketInstanceV2Id, MarketLifecyclePhaseV1, MarketLifecycleRootV1, SeriesAttachmentPlanV2,
    SeriesFundingComponentV2, SeriesFundingQuoteV2, SeriesLinkObligationAdmissionProjectionV1,
    SeriesLinkObligationStatusV1, SeriesLinkObligationV1, SeriesMarketLinkPhaseV1,
    SeriesMarketLinkV1, SeriesMarketLinkV1Id, SeriesPlanV5Id,
};
use clutch_solana_layout::failure_recovery::{
    decode_failure_account_body_v1, FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1,
    FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, SeriesMarketLinkAccountV1,
    MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V1, SERIES_MARKET_LINK_ACCOUNT_BYTES_V1,
};
use clutch_solana_layout::registry;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const MARKET_LIFECYCLE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-lifecycle-account-authentication/v1";
const SERIES_MARKET_LINK_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/series-market-link-account-authentication/v1";
const MARKET_INSTANCE_TERMINAL_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-instance-terminal-authentication/v1";
const SERIES_WRAPPER_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/series-wrapper-authentication/v1";
const MARKET_RECOVERY_SCHEDULE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-recovery-schedule-authentication/v1";

/// Exact authenticated shared `0xaa/1` account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedMarketLifecycleRootV1 {
    account: Pubkey,
    owner_program: Pubkey,
    value: MarketLifecycleRootAccountV1,
    observed_lamports: u64,
    writable: bool,
    data_id: ContentId,
    authentication_id: ContentId,
}

impl AuthenticatedMarketLifecycleRootV1 {
    /// Physical root account.
    pub const fn account(self) -> Pubkey {
        self.account
    }

    /// Program which authenticated and owns the account.
    pub const fn owner_program(self) -> Pubkey {
        self.owner_program
    }

    /// Complete hostile-decoded account value.
    pub const fn value(self) -> MarketLifecycleRootAccountV1 {
        self.value
    }

    /// Complete pure Market lifecycle state.
    pub const fn state(self) -> MarketLifecycleRootV1 {
        self.value.state
    }

    /// Exact lamports observed with the authenticated bytes.
    pub const fn observed_lamports(self) -> u64 {
        self.observed_lamports
    }

    /// Whether the outer message granted writable privilege.
    pub const fn is_writable(self) -> bool {
        self.writable
    }

    /// SHA-256 of the exact framed account bytes.
    pub const fn data_id(self) -> ContentId {
        self.data_id
    }

    /// Account/PDA/body/rent authentication identity.
    pub const fn authentication_id(self) -> ContentId {
        self.authentication_id
    }

    /// Exact persisted refundable root rent principal.
    pub const fn rent_principal_lamports(self) -> u64 {
        self.value.rent_principal_lamports
    }
}

/// Exact authenticated per-Series `0xad/1` link account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSeriesMarketLinkV1 {
    account: Pubkey,
    owner_program: Pubkey,
    value: SeriesMarketLinkAccountV1,
    observed_lamports: u64,
    writable: bool,
    data_id: ContentId,
    authentication_id: ContentId,
}

impl AuthenticatedSeriesMarketLinkV1 {
    /// Physical link account.
    pub const fn account(self) -> Pubkey {
        self.account
    }

    /// Program which authenticated and owns the account.
    pub const fn owner_program(self) -> Pubkey {
        self.owner_program
    }

    /// Complete hostile-decoded account value.
    pub const fn value(self) -> SeriesMarketLinkAccountV1 {
        self.value
    }

    /// Complete pure link state.
    pub const fn state(self) -> SeriesMarketLinkV1 {
        self.value.state
    }

    /// Exact lamports observed with the authenticated bytes.
    pub const fn observed_lamports(self) -> u64 {
        self.observed_lamports
    }

    /// Whether the outer message granted writable privilege.
    pub const fn is_writable(self) -> bool {
        self.writable
    }

    /// SHA-256 of the exact framed account bytes.
    pub const fn data_id(self) -> ContentId {
        self.data_id
    }

    /// Account/PDA/body/rent authentication identity.
    pub const fn authentication_id(self) -> ContentId {
        self.authentication_id
    }
}

/// Private full-body authority for the market-scoped Recovery reward schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedMarketRecoveryScheduleV1 {
    id: ContentId,
    market_root_account: Pubkey,
    market_root_authentication_id: ContentId,
    series_link_account: Pubkey,
    series_link_authentication_id: ContentId,
    funding_quote_id: ContentId,
    liveness_policy_account: Pubkey,
    liveness_policy_id: ContentId,
    recovery_quote_schedule_id: ContentId,
    maximum_calls: u32,
    maximum_lamports_per_call: u64,
    work_capital_lamports: u64,
    account_rent_principal_lamports: u64,
    receipt_program_id: ContentId,
    capability_profile_id: ContentId,
    maximum_progress_units_per_call: u64,
}

impl AuthenticatedMarketRecoveryScheduleV1 {
    /// Exact private join identity.
    pub const fn id(self) -> ContentId {
        self.id
    }
    /// Shared Market root account.
    pub const fn market_root_account(self) -> Pubkey {
        self.market_root_account
    }
    /// Full hostile root-account authentication.
    pub const fn market_root_authentication_id(self) -> ContentId {
        self.market_root_authentication_id
    }
    /// Exact subordinate Series link.
    pub const fn series_link_account(self) -> Pubkey {
        self.series_link_account
    }
    /// Full hostile link-account authentication.
    pub const fn series_link_authentication_id(self) -> ContentId {
        self.series_link_authentication_id
    }
    /// Exact per-Series quote used only for funding provenance/local allocations.
    pub const fn funding_quote_id(self) -> ContentId {
        self.funding_quote_id
    }
    /// Physical immutable liveness-policy account.
    pub const fn liveness_policy_account(self) -> Pubkey {
        self.liveness_policy_account
    }
    /// Market-scoped liveness-policy identity.
    pub const fn liveness_policy_id(self) -> ContentId {
        self.liveness_policy_id
    }
    /// Sole Recovery-compartment reward schedule.
    pub const fn recovery_quote_schedule_id(self) -> ContentId {
        self.recovery_quote_schedule_id
    }
    /// Maximum bounded calls authorized by the policy.
    pub const fn maximum_calls(self) -> u32 {
        self.maximum_calls
    }
    /// Maximum lamports paid by one call.
    pub const fn maximum_lamports_per_call(self) -> u64 {
        self.maximum_lamports_per_call
    }
    /// Exact present work capital.
    pub const fn work_capital_lamports(self) -> u64 {
        self.work_capital_lamports
    }
    /// Exact present Recovery-account rent principal.
    pub const fn account_rent_principal_lamports(self) -> u64 {
        self.account_rent_principal_lamports
    }
    /// Program permitted to mint paid-work/terminal receipts.
    pub const fn receipt_program_id(self) -> ContentId {
        self.receipt_program_id
    }
    /// Current loader-authenticated central capability profile.
    pub const fn capability_profile_id(self) -> ContentId {
        self.capability_profile_id
    }
    /// Central maximum Recovery progress delta per paid call.
    pub const fn maximum_progress_units_per_call(self) -> u64 {
        self.maximum_progress_units_per_call
    }
}

/// Authenticate the existing liveness semantic owner against one exact shared
/// Market and subordinate Series quote. Later convergers may carry different
/// local allocations, but they cannot replace these shared policy/schedule
/// terms or debit the shared Recovery compartment again.
pub fn authenticate_market_recovery_schedule_v1(
    program_id: &Pubkey,
    root: AuthenticatedMarketLifecycleRootV1,
    link: AuthenticatedSeriesMarketLinkV1,
    capability: AuthenticatedRegistryCapabilityV2,
    funding_quote_account: &AccountInfo<'_>,
    liveness_policy_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedMarketRecoveryScheduleV1> {
    let root_binding = root.state().binding();
    let link_binding = link.state().binding();
    require(
        link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation
            && link_binding.market_root_account_id.bytes() == root.account().to_bytes(),
        ClutchError::MismatchedState,
    )?;
    require(
        capability.series_plan_id() == link_binding.series_plan_id
            && capability.capability_profile_id() == root_binding.capability_profile_id
            && capability.registry_release_id() == root_binding.registry_release_id
            && capability.program_account() == *program_id
            && capability
                .profile()
                .maximum_recovery_progress_units_per_call
                != 0,
        ClutchError::MismatchedState,
    )?;
    let quote = authenticate_product_artifact_v1::<SeriesFundingQuoteV2>(
        program_id,
        funding_quote_account,
        link_binding.funding_quote_id.content_id(),
    )?;
    require(
        !liveness_policy_account.is_signer
            && !liveness_policy_account.is_writable
            && !liveness_policy_account.executable
            && liveness_policy_account.owner == program_id
            && liveness_policy_account.data_len() == FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    let data = liveness_policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let frame = decode_failure_account_body_v1(
        &data,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
        FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
    )?;
    let policy = decode_runtime_policy_account_v1(
        liveness_id(program_id),
        liveness_id(liveness_policy_account.key),
        RuntimePersistedAccountViewV1 {
            account_id: liveness_id(liveness_policy_account.key),
            owner_program_id: liveness_id(liveness_policy_account.owner),
            lamports: liveness_policy_account.lamports(),
            data: frame.body,
            writable: false,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let stored_bump = frame.stored_bump;
    let policy_data_id =
        ContentId::from_bytes(solana_sha256_hasher::hashv(&[&data[..]]).to_bytes());
    drop(data);
    expect_pda(
        liveness_policy_account.key,
        seeds::failure_liveness_policy_pda(program_id, &policy.policy_id.bytes()),
        Some(stored_bump),
    )?;
    let recovery = policy.compartments[RuntimeCompartmentKindV1::Recovery.index()];
    let quoted_recovery =
        quote.value().components[SeriesFundingComponentV2::RecoveryReserve.index()];
    let capability_programdata_account = capability.programdata_account();
    let capability_profile_account = capability.profile_artifact_account();
    let maximum_progress_units_per_call = capability
        .profile()
        .maximum_recovery_progress_units_per_call;
    require(
        ContentId::from_bytes(policy.policy_id.bytes()) == root_binding.failure_liveness_policy_id
            && ContentId::from_bytes(policy.realm_id.bytes()) == root_binding.realm_id
            && ContentId::from_bytes(policy.neutral_sink.bytes())
                == root.state().capital().neutral_lamport_sink
            && quote.value().evidence_only_recovery_policy_id == root_binding.recovery_policy_id
            && quote.value().failure_liveness_policy_id == root_binding.failure_liveness_policy_id
            && quote.value().failure_recovery_quote_schedule_id
                == root_binding.failure_liveness_quote_schedule_id
            && ContentId::from_bytes(recovery.quote_schedule_id.bytes())
                == root_binding.failure_liveness_quote_schedule_id
            && ContentId::from_bytes(recovery.receipt_program_id.bytes())
                == ContentId::from_bytes(program_id.to_bytes())
            && quoted_recovery.collateral_atoms == 0
            && quoted_recovery.lamports
                == recovery
                    .total_payer_debit_lamports()
                    .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?
            && quote.value().recovery_rent_principal_lamports
                == recovery.account_rent_principal_lamports
            && root.state().capital().recovery_work_principal_lamports
                == recovery.work_capital_lamports
            && root.state().capital().recovery_rent_principal_lamports
                == recovery.account_rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_RECOVERY_SCHEDULE_AUTHENTICATION_DOMAIN_V1,
            root.account().as_ref(),
            &root.authentication_id().bytes(),
            link.account().as_ref(),
            &link.authentication_id().bytes(),
            funding_quote_account.key.as_ref(),
            &quote.semantic_id().bytes(),
            liveness_policy_account.key.as_ref(),
            &policy_data_id.bytes(),
            &policy.policy_id.bytes(),
            &recovery.quote_schedule_id.bytes(),
            capability_programdata_account.as_ref(),
            capability_profile_account.as_ref(),
            &capability.capability_profile_id().bytes(),
            &maximum_progress_units_per_call.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedMarketRecoveryScheduleV1 {
        id,
        market_root_account: root.account(),
        market_root_authentication_id: root.authentication_id(),
        series_link_account: link.account(),
        series_link_authentication_id: link.authentication_id(),
        funding_quote_id: quote.semantic_id(),
        liveness_policy_account: *liveness_policy_account.key,
        liveness_policy_id: ContentId::from_bytes(policy.policy_id.bytes()),
        recovery_quote_schedule_id: ContentId::from_bytes(recovery.quote_schedule_id.bytes()),
        maximum_calls: recovery.maximum_calls,
        maximum_lamports_per_call: recovery.maximum_lamports_per_call,
        work_capital_lamports: recovery.work_capital_lamports,
        account_rent_principal_lamports: recovery.account_rent_principal_lamports,
        receipt_program_id: ContentId::from_bytes(recovery.receipt_program_id.bytes()),
        capability_profile_id: capability.capability_profile_id(),
        maximum_progress_units_per_call,
    })
}

/// Private Product authorization for one exact Structured/wrapper admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSeriesWrapperAuthorizationV1 {
    id: ContentId,
    link_account: Pubkey,
    link_authentication_id: ContentId,
    link_semantic_id: SeriesMarketLinkV1Id,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    attachment_plan_id: ContentId,
    compiler_bundle_id: ContentId,
    capability_profile_id: ContentId,
    wrapper_recipe_set_id: ContentId,
    rent_refund_owner: ContentId,
    neutral_lamport_sink: ContentId,
    wrapper_status: SeriesLinkObligationStatusV1,
    wrapper_admission_receipt_id: ContentId,
    link_transition_sequence: u64,
}

impl AuthenticatedSeriesWrapperAuthorizationV1 {
    /// Authorization identity.
    pub const fn id(self) -> ContentId {
        self.id
    }
    /// Exact SeriesMarketLink account (writable only for first admission).
    pub const fn link_account(self) -> Pubkey {
        self.link_account
    }
    /// Full link account-authentication identity.
    pub const fn link_authentication_id(self) -> ContentId {
        self.link_authentication_id
    }
    /// Exact pre-transition link semantic state.
    pub const fn link_semantic_id(self) -> SeriesMarketLinkV1Id {
        self.link_semantic_id
    }
    /// Exact Series.
    pub const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }
    /// Exact ordinal.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }
    /// Shared Market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    /// Shared generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Exact V2 attachment identity.
    pub const fn attachment_plan_id(self) -> ContentId {
        self.attachment_plan_id
    }
    /// Exact V2 compiler bundle identity.
    pub const fn compiler_bundle_id(self) -> ContentId {
        self.compiler_bundle_id
    }
    /// Exact central capability profile.
    pub const fn capability_profile_id(self) -> ContentId {
        self.capability_profile_id
    }
    /// Exact Structured-owned wrapper recipe-set identity pinned by AttachmentV2.
    pub const fn wrapper_recipe_set_id(self) -> ContentId {
        self.wrapper_recipe_set_id
    }
    /// Exact Product-owned refundable rent recipient.
    pub const fn rent_refund_owner(self) -> ContentId {
        self.rent_refund_owner
    }
    /// Exact Product-owned System lamport donation sink.
    pub const fn neutral_lamport_sink(self) -> ContentId {
        self.neutral_lamport_sink
    }
    /// Current exhaustive Product obligation state.
    pub const fn wrapper_status(self) -> SeriesLinkObligationStatusV1 {
        self.wrapper_status
    }
    /// Exact first Structured admission transcript; zero only before creation.
    pub const fn wrapper_admission_receipt_id(self) -> ContentId {
        self.wrapper_admission_receipt_id
    }
    /// Current link transition sequence bound by this authorization.
    pub const fn link_transition_sequence(self) -> u64 {
        self.link_transition_sequence
    }
    /// Whether the same instruction must persist the first Product admission.
    pub const fn requires_product_admission(self) -> bool {
        matches!(
            self.wrapper_status,
            SeriesLinkObligationStatusV1::EnabledNeverFounded
        )
    }
}

/// Join an authenticated active link to its exact BundleV2 and AttachmentV2.
///
/// The returned receipt distinguishes first admission from later live-child
/// additions. Structured remains the sole owner of recipe-set membership.
pub fn authenticate_series_wrapper_authorization_v1(
    program_id: &Pubkey,
    link: AuthenticatedSeriesMarketLinkV1,
    compiler_bundle_account: &AccountInfo<'_>,
    attachment_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSeriesWrapperAuthorizationV1> {
    let binding = link.state().binding();
    let wrapper_status = link
        .state()
        .obligation_status(SeriesLinkObligationV1::Wrapper);
    require(
        link.state().phase() == SeriesMarketLinkPhaseV1::Active
            && matches!(
                wrapper_status,
                SeriesLinkObligationStatusV1::EnabledNeverFounded
                    | SeriesLinkObligationStatusV1::Live
            )
            && (wrapper_status == SeriesLinkObligationStatusV1::Live || link.is_writable()),
        ClutchError::MismatchedState,
    )?;
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV2>(
        program_id,
        compiler_bundle_account,
        binding.compiler_output_id,
    )?;
    let attachment = authenticate_product_artifact_v1::<SeriesAttachmentPlanV2>(
        program_id,
        attachment_account,
        binding.attachment_plan_id,
    )?;
    let attachment_id = attachment
        .value()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        bundle.value().series_plan_id == binding.series_plan_id
            && bundle.value().funding_quote_id == binding.funding_quote_id
            && bundle.value().attachment_plan_id.content_id() == binding.attachment_plan_id
            && bundle.value().capability_profile_id == binding.capability_profile_id
            && attachment_id.content_id() == binding.attachment_plan_id
            && attachment.value().funding_quote_id == bundle.value().funding_quote_id
            && attachment.value().funding_quote_id == binding.funding_quote_id,
        ClutchError::MismatchedState,
    )?;
    let link_semantic_id = link
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let wrapper_admission_receipt_id = link
        .state()
        .obligation_admission_receipt_id(SeriesLinkObligationV1::Wrapper);
    let link_transition_sequence = link.state().transition_sequence();
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_WRAPPER_AUTHENTICATION_DOMAIN_V1,
            link.account().as_ref(),
            &link.authentication_id().bytes(),
            &link_semantic_id.bytes(),
            compiler_bundle_account.key.as_ref(),
            &bundle.semantic_id().bytes(),
            attachment_account.key.as_ref(),
            &attachment.semantic_id().bytes(),
            &attachment.value().wrapper_recipe_set_id.bytes(),
            &[series_link_status_byte(wrapper_status)],
            &wrapper_admission_receipt_id.bytes(),
            &link_transition_sequence.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedSeriesWrapperAuthorizationV1 {
        id,
        link_account: link.account(),
        link_authentication_id: link.authentication_id(),
        link_semantic_id,
        series_plan_id: binding.series_plan_id,
        ordinal: binding.ordinal,
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        attachment_plan_id: binding.attachment_plan_id,
        compiler_bundle_id: binding.compiler_output_id,
        capability_profile_id: binding.capability_profile_id,
        wrapper_recipe_set_id: attachment.value().wrapper_recipe_set_id,
        rent_refund_owner: binding.rent_refund_owner,
        neutral_lamport_sink: binding.neutral_lamport_sink,
        wrapper_status,
        wrapper_admission_receipt_id,
        link_transition_sequence,
    })
}

/// Private whole-Market terminal receipt re-derived only from authenticated `0xaa`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedMarketInstanceTerminalV1 {
    id: ContentId,
    root_account: Pubkey,
    owner_program: Pubkey,
    root_semantic_id: ContentId,
    root_data_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    projection: MarketInstanceTerminalProjectionV1,
}

impl AuthenticatedMarketInstanceTerminalV1 {
    /// Authentication receipt identity.
    pub const fn id(self) -> ContentId {
        self.id
    }

    /// Exact physical terminal root.
    pub const fn root_account(self) -> Pubkey {
        self.root_account
    }

    /// Program which owns the exact root.
    pub const fn owner_program(self) -> Pubkey {
        self.owner_program
    }

    /// Exact semantic identity of the terminal pure state.
    pub const fn root_semantic_id(self) -> ContentId {
        self.root_semantic_id
    }

    /// SHA-256 of the exact terminal framed bytes.
    pub const fn root_data_id(self) -> ContentId {
        self.root_data_id
    }

    /// Full-width shared Market identity.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact Market/Failure generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Exact exhaustive Failure-family receipt consumed by `0xaa`.
    pub const fn failure_terminal_receipt_id(self) -> ContentId {
        self.projection.failure_terminal_receipt_id()
    }

    /// Private structural projection consumed only inside this program.
    pub(crate) const fn projection(self) -> MarketInstanceTerminalProjectionV1 {
        self.projection
    }
}

/// Authenticate the exact shared Market root without trusting a caller DTO.
pub fn authenticate_market_lifecycle_root_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_generation: u64,
    require_writable: bool,
) -> Outcome<AuthenticatedMarketLifecycleRootV1> {
    require(
        !account.is_signer
            && !account.executable
            && (!require_writable || account.is_writable)
            && account.owner == program_id
            && account.data_len() == MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = MarketLifecycleRootAccountV1::decode(&data)?;
    let binding = value.state.binding();
    let observed_lamports = account.lamports();
    require(
        binding.market_instance_id == expected_market_instance_id
            && binding.generation == expected_generation
            && observed_lamports >= value.rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let (expected, bump) = seeds::product_market_lifecycle_root_pda(
        program_id,
        &expected_market_instance_id.bytes(),
        expected_generation,
    );
    expect_pda(account.key, (expected, bump), Some(value.stored_bump))?;
    let data_id = ContentId::from_bytes(solana_sha256_hasher::hashv(&[&data[..]]).to_bytes());
    drop(data);
    let semantic_id = value
        .state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_LIFECYCLE_AUTHENTICATION_DOMAIN_V1,
            account.key.as_ref(),
            program_id.as_ref(),
            &data_id.bytes(),
            &semantic_id.bytes(),
            &value.rent_principal_lamports.to_le_bytes(),
            &observed_lamports.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(authentication_id)?;
    Ok(AuthenticatedMarketLifecycleRootV1 {
        account: *account.key,
        owner_program: *program_id,
        value,
        observed_lamports,
        writable: account.is_writable,
        data_id,
        authentication_id,
    })
}

/// Authenticate an exact per-Series link and its shared-root association.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_series_market_link_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series_plan_id: SeriesPlanV5Id,
    expected_ordinal: u32,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_generation: u64,
    expected_market_root: Pubkey,
    require_writable: bool,
) -> Outcome<AuthenticatedSeriesMarketLinkV1> {
    require(
        !account.is_signer
            && !account.executable
            && (!require_writable || account.is_writable)
            && account.owner == program_id
            && account.data_len() == SERIES_MARKET_LINK_ACCOUNT_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesMarketLinkAccountV1::decode(&data)?;
    let binding = value.state.binding();
    let accounted_lamports = value
        .state
        .rent_principal_lamports()
        .checked_add(value.state.current_donation_lamports())
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let observed_lamports = account.lamports();
    require(
        binding.series_plan_id == expected_series_plan_id
            && binding.ordinal == expected_ordinal
            && binding.market_instance_id == expected_market_instance_id
            && binding.generation == expected_generation
            && binding.market_root_account_id.bytes() == expected_market_root.to_bytes()
            && observed_lamports >= accounted_lamports,
        ClutchError::MismatchedState,
    )?;
    let (expected, bump) = seeds::product_series_market_link_pda(
        program_id,
        &expected_series_plan_id.bytes(),
        expected_ordinal,
    );
    expect_pda(account.key, (expected, bump), Some(value.stored_bump))?;
    let data_id = ContentId::from_bytes(solana_sha256_hasher::hashv(&[&data[..]]).to_bytes());
    drop(data);
    let semantic_id = value
        .state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_MARKET_LINK_AUTHENTICATION_DOMAIN_V1,
            account.key.as_ref(),
            program_id.as_ref(),
            &data_id.bytes(),
            &semantic_id.bytes(),
            expected_market_root.as_ref(),
            &observed_lamports.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(authentication_id)?;
    Ok(AuthenticatedSeriesMarketLinkV1 {
        account: *account.key,
        owner_program: *program_id,
        value,
        observed_lamports,
        writable: account.is_writable,
        data_id,
        authentication_id,
    })
}

/// Re-open a terminal root and mint the private whole-Market receipt.
pub fn authenticate_market_instance_terminal_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_generation: u64,
) -> Outcome<AuthenticatedMarketInstanceTerminalV1> {
    let root = authenticate_market_lifecycle_root_v1(
        program_id,
        account,
        expected_market_instance_id,
        expected_generation,
        false,
    )?;
    require(
        root.state().phase() == MarketLifecyclePhaseV1::Terminal,
        ClutchError::MismatchedState,
    )?;
    let projection = root
        .state()
        .terminal_projection()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        projection.market_instance_id() == expected_market_instance_id
            && projection.generation() == expected_generation,
        ClutchError::MismatchedState,
    )?;
    let root_semantic_id = root
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            MARKET_INSTANCE_TERMINAL_AUTHENTICATION_DOMAIN_V1,
            account.key.as_ref(),
            program_id.as_ref(),
            &root.data_id().bytes(),
            &root_semantic_id.bytes(),
            &projection.id().bytes(),
            &root.observed_lamports().to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedMarketInstanceTerminalV1 {
        id,
        root_account: root.account(),
        owner_program: root.owner_program(),
        root_semantic_id,
        root_data_id: root.data_id(),
        market_instance_id: expected_market_instance_id,
        generation: expected_generation,
        projection,
    })
}

/// Atomically finalize a fully retired root and return its private terminal receipt.
pub fn finalize_market_lifecycle_terminal_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedMarketLifecycleRootV1,
) -> Outcome<AuthenticatedMarketInstanceTerminalV1> {
    let binding = authenticated.state().binding();
    let (successor, _) = authenticated
        .state()
        .finalize_terminal()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_market_lifecycle_root_v1(program_id, account, authenticated, successor)?;
    authenticate_market_instance_terminal_v1(
        program_id,
        account,
        binding.market_instance_id,
        binding.generation,
    )
}

/// Persist a pure successor and immediately reauthenticate the full root bytes.
pub fn write_market_lifecycle_root_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedMarketLifecycleRootV1,
    successor: MarketLifecycleRootV1,
) -> Outcome<AuthenticatedMarketLifecycleRootV1> {
    require(
        account.is_writable
            && *account.key == authenticated.account
            && account.owner == program_id
            && successor.binding() == authenticated.state().binding(),
        ClutchError::MismatchedState,
    )?;
    let value = MarketLifecycleRootAccountV1 {
        state: successor,
        ..authenticated.value
    };
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    value.encode(&mut data)?;
    drop(data);
    let rebound = authenticate_market_lifecycle_root_v1(
        program_id,
        account,
        successor.binding().market_instance_id,
        successor.binding().generation,
        true,
    )?;
    require(rebound.value == value, ClutchError::MismatchedState)?;
    Ok(rebound)
}

/// Persist a pure per-Series link successor and reauthenticate exact bytes.
pub fn write_series_market_link_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesMarketLinkV1,
    successor: SeriesMarketLinkV1,
) -> Outcome<AuthenticatedSeriesMarketLinkV1> {
    require(
        account.is_writable
            && *account.key == authenticated.account
            && account.owner == program_id
            && successor.binding() == authenticated.state().binding(),
        ClutchError::MismatchedState,
    )?;
    let value = SeriesMarketLinkAccountV1 {
        state: successor,
        ..authenticated.value
    };
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    value.encode(&mut data)?;
    drop(data);
    let binding = successor.binding();
    let rebound = authenticate_series_market_link_v1(
        program_id,
        account,
        binding.series_plan_id,
        binding.ordinal,
        binding.market_instance_id,
        binding.generation,
        Pubkey::new_from_array(binding.market_root_account_id.bytes()),
        true,
    )?;
    require(rebound.value == value, ClutchError::MismatchedState)?;
    Ok(rebound)
}

/// Persist the first Product-side Wrapper admission in the same instruction
/// that an authenticated Structured owner accepts its root/descriptor.
pub(crate) fn admit_series_wrapper_obligation_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesMarketLinkV1,
    authorization: AuthenticatedSeriesWrapperAuthorizationV1,
    structured_admission_receipt_id: ContentId,
) -> Outcome<AuthenticatedSeriesMarketLinkV1> {
    require_live_content_id(structured_admission_receipt_id)?;
    let semantic_id = authenticated
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        authenticated.is_writable()
            && authorization.requires_product_admission()
            && authorization.link_account == authenticated.account()
            && authorization.link_authentication_id == authenticated.authentication_id()
            && authorization.link_semantic_id == semantic_id
            && authorization.wrapper_admission_receipt_id == ContentId::ZERO
            && authorization.link_transition_sequence
                == authenticated.state().transition_sequence(),
        ClutchError::MismatchedState,
    )?;
    let next_sequence = authorization
        .link_transition_sequence
        .checked_add(1)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let successor = authenticated
        .state()
        .admit_obligation(SeriesLinkObligationAdmissionProjectionV1 {
            link_semantic_id: semantic_id,
            obligation: SeriesLinkObligationV1::Wrapper,
            link_transition_sequence: next_sequence,
            owner_admission_receipt_id: structured_admission_receipt_id,
        })
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_series_market_link_v1(program_id, account, authenticated, successor)
}

/// Promote an authenticated active link into a private Failure pin successor.
pub(crate) fn pin_series_market_link_failure_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesMarketLinkV1,
    failure_begin_receipt_id: ContentId,
) -> Outcome<AuthenticatedSeriesMarketLinkV1> {
    require(
        authenticated.state().phase() == SeriesMarketLinkPhaseV1::Active,
        ClutchError::MismatchedState,
    )?;
    let successor = authenticated
        .state()
        .pin_failure_session(failure_begin_receipt_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_series_market_link_v1(program_id, account, authenticated, successor)
}

fn require_live_content_id(id: ContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}

const fn series_link_status_byte(status: SeriesLinkObligationStatusV1) -> u8 {
    match status {
        SeriesLinkObligationStatusV1::CapabilityDisabled => 1,
        SeriesLinkObligationStatusV1::EnabledNeverFounded => 2,
        SeriesLinkObligationStatusV1::Live => 3,
        SeriesLinkObligationStatusV1::Terminal => 4,
    }
}

fn liveness_id(key: &Pubkey) -> LivenessId {
    LivenessId::from_bytes(key.to_bytes())
}
