//! Concrete SBF account and lamport-custody boundary for recurring Series.
//!
//! This module is compiled only by the non-production Product/Series
//! laboratory. It authenticates the account forms that already have frozen
//! semantics: registered-Series and funding-state PDAs, exact stored/current
//! rent coverage, and six physically distinct zero-data System-owned lamport
//! vaults. It also supplies exact-delta System transfers into and out of those
//! vaults.
//!
//! The dispatcher-local entry point is present, but all six capability tuples
//! remain disabled. Typed mutation helpers are kept behind the missing
//! registry, failure, liveness, and occurrence adapters instead of accepting
//! caller-shaped authentication facts.

use crate::accounts::{expect_pda, require, require_count, require_signer, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    allocate_data, assign_data, create_pda_account, read_rent, require_creatable,
    require_system_program, transfer_data, RentParameters, SYSTEM_PROGRAM_ID,
};
use crate::instructions::product_artifact::{
    authenticate_product_artifact_v1, authenticate_registry_capability_for_registration_v3,
    authenticate_registry_capability_v3, authenticate_series_registry_capability_refs_v2,
    authenticate_series_registry_capability_refs_v2_for_mutation,
    AuthenticatedRegistryCapabilityReleaseV3, AuthenticatedRegistryCapabilityV3,
};
use crate::instructions::series_failure_funding::{
    mint_series_market_core_funding_receipt_v1, SeriesMarketCoreFundingReceiptV1,
};
use crate::seeds;
use clutch_collateral_adapter_v2::{
    accept_collateral_transfer_v2, accept_series_collateral_vault_close_v2,
    accept_series_vault_rent_disposition_v2, admit_realm_collateral_account_v2,
    admit_realm_collateral_mint_v2, prepare_custody_creation_v2,
    prepare_realm_collateral_transfer_v2, prepare_series_collateral_vault_close_v2,
    prepare_series_vault_rent_disposition_v2, series_donation_disposition_request_v2,
    series_principal_refund_request_v2, series_segregated_funding_request_v2,
    AcceptedCollateralTransferV2, AcceptedSeriesVaultRentDispositionV2, BoundRealmCollateralV2,
    CpiAccountMetaV2, CustodyBindingV2, CustodyCreationPlanV2, CustodyInitializationStepV2,
    CustodyTransferKindV2, Id as CollateralId, PreparedCollateralTransferV2,
    RuntimeAccountViewV2, RuntimeLamportAccountViewV2, SeriesCollateralFundingJoinV2,
    SeriesCollateralTerminalJoinV2, SeriesCollateralVaultCloseRequestV2, TokenAccountRoleV2,
    TransferAuthorityKindV2, TransferAuthorityV2,
};
use clutch_product_series::{
    assemble_compiled_product_series_bundle_v5, compile_source_occurrence_v3,
    compile_source_occurrence_v4, AuthenticatedSeriesFundingAuthorityV1,
    AuthenticatedSourceSeriesAuthorityV3, CompiledProductSeriesBundleV1,
    CompiledProductSeriesBundleV1Id, CompiledProductSeriesBundleV5,
    CompiledProductSeriesBundleV5Id, CompiledSourceOccurrenceV3, ComponentDebitV1, ContentId,
    EvidenceOnlyRecoveryPolicyV1, MarketGenesisProfileV2, NativeClaimBasisV1, PriceMeasurePolicyV1,
    ProductSeriesBundleInputsV5, ProductTemplateV4, RegistryCapabilityProjectionV2,
    SeriesActivationContextV1, SeriesAttachmentPlanV1, SeriesAttachmentPlanV4,
    AuthenticatedSeriesFundingAuthorityV2, SeriesFundingComponentV1, SeriesFundingComponentV2,
    SeriesFundingQuoteV1, SeriesFundingQuoteV4, SeriesFundingRequirementsV1,
    SeriesFundingStateV1, SeriesFundingStateV2, SeriesFundingTerminalProjectionV1,
    SeriesFundingTerminalProjectionV2,
    SeriesFundingTermsV2, SeriesFundingTermsV2Id, SeriesPlanV5, SeriesPlanV5Id,
    SourceOccurrenceV1Id, SERIES_COLLATERAL_VAULT_COUNT_V2, SERIES_FUNDING_COMPONENT_COUNT,
    SERIES_FUNDING_COMPONENT_COUNT_V2,
};
use clutch_solana_layout::product_series::{
    ActivateSeriesFundingIntentV1, AdvanceSeriesOccurrenceIntentV1, CloseSeriesFundingIntentV1,
    LapseSeriesOccurrenceIntentV1, ObserveSeriesDonationIntentV2, RegisterSeriesIntentV1,
    SeriesFundingAccountV1, SeriesFundingAssetV1, SeriesRegistryAccountV1,
    SeriesFundingAccountV2, SeriesRegistryAccountV2, SERIES_FUNDING_ACCOUNT_BYTES_V1,
    SERIES_FUNDING_ACCOUNT_BYTES_V2, SERIES_REGISTRY_ACCOUNT_BYTES_V1,
    SERIES_REGISTRY_ACCOUNT_BYTES_V2,
};
use clutch_solana_layout::registry::RecurringSeriesAction;
use clutch_source_plane_v3::{SourcePlaneProgramV3, StatisticKindV3, SummaryProgramV3};
use clutch_source_plane_v3_runtime::{
    AuthenticatedClockBucketV1, AuthenticatedSourceReleaseV1, ClockSnapshotV1,
    OccurrenceSourceReceiptV1,
};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Closed number of physical Series funding compartments.
pub const SERIES_CUSTODY_COUNT_V1: usize = SERIES_FUNDING_COMPONENT_COUNT;
/// Canonical generation of the sole funding activation permitted by V1.
pub const SERIES_ACTIVATION_GENERATION_V1: u64 = 1;

/// Exact read-only Product/Series artifact count used by registration and
/// every later transition that reconstructs the immutable join.
pub const SERIES_ARTIFACT_ACCOUNT_COUNT_V1: usize = 9;

/// SeriesPlan V5 artifact account index.
pub const IX_SERIES_ARTIFACT_PLAN: usize = 0;
/// SeriesFundingTerms V2 artifact account index.
pub const IX_SERIES_ARTIFACT_FUNDING_TERMS: usize = 1;
/// ProductTemplate V4 artifact account index.
pub const IX_SERIES_ARTIFACT_TEMPLATE: usize = 2;
/// NativeClaimBasis V1 artifact account index.
pub const IX_SERIES_ARTIFACT_BASIS: usize = 3;
/// EvidenceOnlyRecoveryPolicy V1 artifact account index.
pub const IX_SERIES_ARTIFACT_RECOVERY: usize = 4;
/// PriceMeasurePolicy V1 artifact account index.
pub const IX_SERIES_ARTIFACT_PRICE_POLICY: usize = 5;
/// MarketGenesisProfile V2 artifact account index.
pub const IX_SERIES_ARTIFACT_GENESIS: usize = 6;
/// SeriesFundingQuote artifact account index (QuoteV4 for current registration).
pub const IX_SERIES_ARTIFACT_QUOTE: usize = 7;
/// SeriesAttachmentPlan artifact account index (AttachmentV4 for current registration).
pub const IX_SERIES_ARTIFACT_ATTACHMENT: usize = 8;

const SERIES_TERMINAL_RECEIPT_DOMAIN_V1: &[u8] = b"dragons-clutch/series-terminal-receipt/v1";
const SERIES_COLLATERAL_ACTIVATION_POSTWRITE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-collateral-activation-postwrite/v2\0";
const SERIES_COLLATERAL_ACTIVATION_FUNDING_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-collateral-activation-funding/v2\0";
const SERIES_COLLATERAL_VAULT_POSTSTATE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-collateral-vault-poststate/v2\0";
const SERIES_COLLATERAL_TRANSFER_POSTSTATE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-collateral-transfer-poststate/v2\0";
const SERIES_COLLATERAL_SOURCE_PRESTATE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-collateral-source-prestate/v2\0";
const SERIES_COLLATERAL_SOURCE_POSTSTATE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-collateral-source-poststate/v2\0";

/// Decode one exact Series action payload and enter its local account handler.
///
/// The capability check intentionally precedes payload decoding and account
/// inspection. All six tuples remain absent from the current capability table.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: RecurringSeriesAction,
    payload: &[u8],
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        capabilities::extension_intent_action_enabled(
            clutch_solana_layout::registry::SOURCE_SERIES_FAMILY_TAG,
            clutch_solana_layout::registry::SOURCE_SERIES_FAMILY_VERSION,
            action.tag(),
        ),
        ClutchError::UnsupportedInstruction,
    )?;
    match action {
        RecurringSeriesAction::RegisterSeries => {
            let request = RegisterSeriesIntentV1::decode(payload)?;
            process_register_series(program_id, accounts, request)
        }
        RecurringSeriesAction::ActivateFunding => {
            let request = ActivateSeriesFundingIntentV1::decode(payload)?;
            process_activate_funding(program_id, accounts, request)
        }
        RecurringSeriesAction::AdvanceOccurrence => {
            let request = AdvanceSeriesOccurrenceIntentV1::decode(payload)?;
            process_advance_occurrence(program_id, accounts, request)
        }
        RecurringSeriesAction::LapseOccurrence => {
            let request = LapseSeriesOccurrenceIntentV1::decode(payload)?;
            process_lapse_occurrence(program_id, accounts, request)
        }
        RecurringSeriesAction::ObserveDonation => {
            let request = ObserveSeriesDonationIntentV2::decode(payload)?;
            process_observe_donation(program_id, accounts, request)
        }
        RecurringSeriesAction::CloseFunding => {
            let request = CloseSeriesFundingIntentV1::decode(payload)?;
            process_close_funding(program_id, accounts, request)
        }
    }
}

/// Registration account contract: payer, Series registry PDA, neutral lamport
/// sink, System, Rent, executing program, linked ProgramData, immutable
/// RegistryRelease and CapabilityProfile artifacts, Source release, exact
/// compiler-output bundle, then the nine Series artifacts it references.
const REGISTER_SERIES_ACCOUNT_COUNT_V1: usize = 11 + SERIES_ARTIFACT_ACCOUNT_COUNT_V1;
const IX_REGISTER_PAYER: usize = 0;
const IX_REGISTER_TARGET: usize = 1;
const IX_REGISTER_NEUTRAL_LAMPORT_SINK: usize = 2;
const IX_REGISTER_SYSTEM: usize = 3;
const IX_REGISTER_RENT: usize = 4;
const IX_REGISTER_PROGRAM: usize = 5;
const IX_REGISTER_PROGRAMDATA: usize = 6;
const IX_REGISTER_REGISTRY_RELEASE: usize = 7;
const IX_REGISTER_CAPABILITY_PROFILE: usize = 8;
const IX_REGISTER_SOURCE_RELEASE: usize = 9;
const IX_REGISTER_COMPILER_OUTPUT: usize = 10;
const IX_REGISTER_ARTIFACTS: usize = 11;

fn process_register_series(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: RegisterSeriesIntentV1,
) -> Outcome<()> {
    require_count(accounts, REGISTER_SERIES_ACCOUNT_COUNT_V1)?;
    let rent = read_rent(&accounts[IX_REGISTER_RENT])?;
    let artifacts = authenticate_series_artifact_accounts_v4(
        program_id,
        &accounts[IX_REGISTER_ARTIFACTS..],
        request.series_plan_id,
        request.funding_terms_id,
    )?;
    let registry = authenticate_registry_capability_for_registration_v3(
        program_id,
        &accounts[IX_REGISTER_REGISTRY_RELEASE],
        &accounts[IX_REGISTER_CAPABILITY_PROFILE],
        request.registry_release_id,
        request.capability_profile_id,
        &accounts[IX_REGISTER_PROGRAM],
        &accounts[IX_REGISTER_PROGRAMDATA],
    )?;
    require(
        registry.projection().capability_profile_id == request.capability_profile_id,
        ClutchError::MismatchedState,
    )?;
    let source_release = crate::source_plane_v3::authenticate_release(
        program_id,
        &accounts[IX_REGISTER_SOURCE_RELEASE],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let compiler_bundle = authenticate_compiled_product_series_bundle_v5(
        program_id,
        &accounts[IX_REGISTER_COMPILER_OUTPUT],
        registry,
        source_release,
        &artifacts,
    )?;
    let authority = AuthenticatedProductSourceAuthorityV1::new(registry, source_release)?;
    let registration = authenticate_series_registration_v2(
        &authority,
        &artifacts,
        &registry.projection(),
        compiler_bundle,
        request,
    )?;
    register_series_replay_anchor_v3(
        program_id,
        registration,
        &accounts[IX_REGISTER_PAYER],
        &accounts[IX_REGISTER_TARGET],
        &accounts[IX_REGISTER_NEUTRAL_LAMPORT_SINK],
        &accounts[IX_REGISTER_SYSTEM],
        &rent,
    )?;
    Ok(())
}

/// Minimal current authority stack common to successor Series mutations. The
/// funding owner is now V2; custody, occurrence, and terminal accounts remain
/// deliberately outside this common prefix and are joined by their routes.
const CURRENT_SERIES_AUTHORITY_ACCOUNT_COUNT_V1: usize =
    8 + SERIES_ARTIFACT_ACCOUNT_COUNT_V1;
const IX_CURRENT_REGISTRY: usize = 0;
const IX_CURRENT_RENT: usize = 1;
const IX_CURRENT_PROGRAM: usize = 2;
const IX_CURRENT_PROGRAMDATA: usize = 3;
const IX_CURRENT_REGISTRY_RELEASE: usize = 4;
const IX_CURRENT_CAPABILITY_PROFILE: usize = 5;
const IX_CURRENT_SOURCE_RELEASE: usize = 6;
const IX_CURRENT_COMPILER_BUNDLE: usize = 7;
const IX_CURRENT_ARTIFACTS: usize = 8;

fn authenticate_current_series_authority_stack(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    expected_series: SeriesPlanV5Id,
    registry_writable: bool,
) -> Outcome<()> {
    require_count(accounts, CURRENT_SERIES_AUTHORITY_ACCOUNT_COUNT_V1)?;
    let rent = read_rent(&accounts[IX_CURRENT_RENT])?;
    let registry_account = read_series_registry_account_v2_with_role(
        program_id,
        &accounts[IX_CURRENT_REGISTRY],
        expected_series,
        &rent,
        registry_writable,
    )?;
    let artifacts = authenticate_series_artifact_accounts_v4(
        program_id,
        &accounts[IX_CURRENT_ARTIFACTS..],
        expected_series,
        registry_account.value().funding_terms_id,
    )?;
    let registry_refs = if registry_writable {
        authenticate_series_registry_capability_refs_v2_for_mutation(
            program_id,
            &accounts[IX_CURRENT_REGISTRY],
            expected_series,
        )?
    } else {
        authenticate_series_registry_capability_refs_v2(
            program_id,
            &accounts[IX_CURRENT_REGISTRY],
            expected_series,
        )?
    };
    let registry = authenticate_registry_capability_v3(
        program_id,
        registry_refs,
        &accounts[IX_CURRENT_PROGRAM],
        &accounts[IX_CURRENT_PROGRAMDATA],
        &accounts[IX_CURRENT_REGISTRY_RELEASE],
        &accounts[IX_CURRENT_CAPABILITY_PROFILE],
    )?;
    artifacts.validate_registry_projection(&registry.projection())?;
    let source_release = crate::source_plane_v3::authenticate_release(
        program_id,
        &accounts[IX_CURRENT_SOURCE_RELEASE],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let bundle = authenticate_compiled_product_series_bundle_v5(
        program_id,
        &accounts[IX_CURRENT_COMPILER_BUNDLE],
        registry,
        source_release,
        &artifacts,
    )?;
    require(
        bundle.bundle_id() == registry_account.value().compiler_bundle_id,
        ClutchError::MismatchedState,
    )
}

fn process_activate_funding(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ActivateSeriesFundingIntentV1,
) -> Outcome<()> {
    authenticate_current_series_authority_stack(
        program_id,
        accounts,
        request.series_plan_id,
        true,
    )?;
    Err(ClutchError::AuthorizationUnavailable.into())
}

fn process_advance_occurrence(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: AdvanceSeriesOccurrenceIntentV1,
) -> Outcome<()> {
    authenticate_current_series_authority_stack(
        program_id,
        accounts,
        request.series_plan_id,
        false,
    )?;
    Err(ClutchError::AuthorizationUnavailable.into())
}

/// Free-lapse account contract: Series registry, funding state, Rent,
/// executing program, linked ProgramData, RegistryRelease and CapabilityProfile
/// artifacts, Source release, Clock, exact BundleV5, then the nine immutable
/// current Series artifacts.
const LAPSE_SERIES_ACCOUNT_COUNT_V1: usize = 10 + SERIES_ARTIFACT_ACCOUNT_COUNT_V1;
const IX_LAPSE_REGISTRY: usize = 0;
const IX_LAPSE_FUNDING: usize = 1;
const IX_LAPSE_RENT: usize = 2;
const IX_LAPSE_PROGRAM: usize = 3;
const IX_LAPSE_PROGRAMDATA: usize = 4;
const IX_LAPSE_REGISTRY_RELEASE: usize = 5;
const IX_LAPSE_CAPABILITY_PROFILE: usize = 6;
const IX_LAPSE_SOURCE_RELEASE: usize = 7;
const IX_LAPSE_CLOCK: usize = 8;
const IX_LAPSE_COMPILER_BUNDLE: usize = 9;
const IX_LAPSE_ARTIFACTS: usize = 10;

fn process_lapse_occurrence(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: LapseSeriesOccurrenceIntentV1,
) -> Outcome<()> {
    require_count(accounts, LAPSE_SERIES_ACCOUNT_COUNT_V1)?;
    let rent = read_rent(&accounts[IX_LAPSE_RENT])?;
    let registry_account = read_series_registry_account_v2(
        program_id,
        &accounts[IX_LAPSE_REGISTRY],
        request.series_plan_id,
        &rent,
    )?;
    require(registry_account.activation_consumed(), ClutchError::Replay)?;
    let artifacts = authenticate_series_artifact_accounts_v4(
        program_id,
        &accounts[IX_LAPSE_ARTIFACTS..],
        request.series_plan_id,
        registry_account.value().funding_terms_id,
    )?;
    let registry_refs = authenticate_series_registry_capability_refs_v2(
        program_id,
        &accounts[IX_LAPSE_REGISTRY],
        request.series_plan_id,
    )?;
    let product_registry = authenticate_registry_capability_v3(
        program_id,
        registry_refs,
        &accounts[IX_LAPSE_PROGRAM],
        &accounts[IX_LAPSE_PROGRAMDATA],
        &accounts[IX_LAPSE_REGISTRY_RELEASE],
        &accounts[IX_LAPSE_CAPABILITY_PROFILE],
    )?;
    require(
        product_registry.projection().capability_profile_id
            == registry_account.value().capability_profile_id,
        ClutchError::MismatchedState,
    )?;
    artifacts.validate_registry_projection(&product_registry.projection())?;
    let source_release = crate::source_plane_v3::authenticate_release(
        program_id,
        &accounts[IX_LAPSE_SOURCE_RELEASE],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let compiler_bundle = authenticate_compiled_product_series_bundle_v5(
        program_id,
        &accounts[IX_LAPSE_COMPILER_BUNDLE],
        product_registry,
        source_release,
        &artifacts,
    )?;
    require(
        compiler_bundle.bundle_id() == registry_account.value().compiler_bundle_id,
        ClutchError::MismatchedState,
    )?;
    let _source_authority = AuthenticatedProductSourceAuthorityV1::from_series_registry(
        product_registry,
        source_release,
    )?;
    let funding = read_series_funding_account_v2(
        program_id,
        &accounts[IX_LAPSE_FUNDING],
        registry_account,
        &artifacts,
        &rent,
    )?;
    let clock = authenticate_series_clock_v2(
        &artifacts,
        source_release,
        &accounts[IX_LAPSE_CLOCK],
    )?;
    let (_, ordinal) = lapse_series_occurrence_v2(
        program_id,
        &accounts[IX_LAPSE_FUNDING],
        funding,
        &artifacts,
        &clock,
        &rent,
    )?;
    require(ordinal == request.ordinal, ClutchError::Replay)?;
    Ok(())
}

/// Current local account contract for the donation route: registry, funding,
/// component vault, Rent, current Program/ProgramData and Registry artifacts,
/// Source release, BundleV5, then the nine current immutable Series artifacts.
const OBSERVE_DONATION_ACCOUNT_COUNT_V1: usize = 10 + SERIES_ARTIFACT_ACCOUNT_COUNT_V1;
const IX_OBSERVE_REGISTRY: usize = 0;
const IX_OBSERVE_FUNDING: usize = 1;
const IX_OBSERVE_VAULT: usize = 2;
const IX_OBSERVE_RENT: usize = 3;
const IX_OBSERVE_PROGRAM: usize = 4;
const IX_OBSERVE_PROGRAMDATA: usize = 5;
const IX_OBSERVE_REGISTRY_RELEASE: usize = 6;
const IX_OBSERVE_CAPABILITY_PROFILE: usize = 7;
const IX_OBSERVE_SOURCE_RELEASE: usize = 8;
const IX_OBSERVE_COMPILER_BUNDLE: usize = 9;
const IX_OBSERVE_ARTIFACTS: usize = 10;

fn process_observe_donation(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ObserveSeriesDonationIntentV2,
) -> Outcome<()> {
    require_count(accounts, OBSERVE_DONATION_ACCOUNT_COUNT_V1)?;
    let rent = read_rent(&accounts[IX_OBSERVE_RENT])?;
    let registry = read_series_registry_account_v2(
        program_id,
        &accounts[IX_OBSERVE_REGISTRY],
        request.series_plan_id,
        &rent,
    )?;
    require(registry.activation_consumed(), ClutchError::Replay)?;
    let artifacts = authenticate_series_artifact_accounts_v4(
        program_id,
        &accounts[IX_OBSERVE_ARTIFACTS..],
        request.series_plan_id,
        registry.value().funding_terms_id,
    )?;
    let registry_refs = authenticate_series_registry_capability_refs_v2(
        program_id,
        &accounts[IX_OBSERVE_REGISTRY],
        request.series_plan_id,
    )?;
    let product_registry = authenticate_registry_capability_v3(
        program_id,
        registry_refs,
        &accounts[IX_OBSERVE_PROGRAM],
        &accounts[IX_OBSERVE_PROGRAMDATA],
        &accounts[IX_OBSERVE_REGISTRY_RELEASE],
        &accounts[IX_OBSERVE_CAPABILITY_PROFILE],
    )?;
    artifacts.validate_registry_projection(&product_registry.projection())?;
    let source_release = crate::source_plane_v3::authenticate_release(
        program_id,
        &accounts[IX_OBSERVE_SOURCE_RELEASE],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let compiler_bundle = authenticate_compiled_product_series_bundle_v5(
        program_id,
        &accounts[IX_OBSERVE_COMPILER_BUNDLE],
        product_registry,
        source_release,
        &artifacts,
    )?;
    require(
        compiler_bundle.bundle_id() == registry.value().compiler_bundle_id,
        ClutchError::MismatchedState,
    )?;
    let funding = read_series_funding_account_v2(
        program_id,
        &accounts[IX_OBSERVE_FUNDING],
        registry,
        &artifacts,
        &rent,
    )?;
    if request.asset != SeriesFundingAssetV1::Lamports {
        return Err(ClutchError::AuthorizationUnavailable.into());
    }
    require_lamport_vault_metadata_v2(
        program_id,
        request.series_plan_id,
        request.component,
        &accounts[IX_OBSERVE_VAULT],
    )?;
    let capital = funding.value().state.components[request.component.index()];
    let accounted = add(
        capital.remaining_principal.lamports,
        capital.donations.lamports,
    )?;
    let amount = ComponentDebitV1 {
        lamports: sub(accounts[IX_OBSERVE_VAULT].lamports(), accounted)?,
        collateral_atoms: 0,
    };
    require(amount.lamports != 0, ClutchError::MismatchedState)?;
    let authority = AuthenticatedSeriesDonationAuthorityV2 {
        state_id: funding
            .value()
            .state
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        component: request.component,
        amount,
    };
    observe_series_donation_v2(
        program_id,
        &accounts[IX_OBSERVE_FUNDING],
        funding,
        &artifacts,
        &authority,
        request.component,
        amount,
        &rent,
    )?;
    Ok(())
}

fn process_close_funding(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CloseSeriesFundingIntentV1,
) -> Outcome<()> {
    authenticate_current_series_authority_stack(
        program_id,
        accounts,
        request.series_plan_id,
        true,
    )?;
    Err(ClutchError::AuthorizationUnavailable.into())
}

/// Exact decoded immutable bodies selected by one registered Series.
///
/// Construction is private to [`authenticate_series_artifact_accounts`], so a
/// caller cannot obtain this type from claimed IDs or caller-built projections.
/// It proves account owner, read-only role, exact body length, content-derived
/// PDA, hostile decode, recomputed typed identity, and all body-to-body
/// references. It deliberately does not prove central-registry provenance.
#[derive(Debug)]
#[cfg(test)]
pub struct AuthenticatedSeriesArtifactsV1 {
    /// Finite recurring Series plan.
    pub series: Box<SeriesPlanV5>,
    /// Immutable principal/refund/sink/mint/program ownership.
    pub funding_terms: Box<SeriesFundingTermsV2>,
    /// Reusable relative Product semantics.
    pub template: Box<ProductTemplateV4>,
    /// Exact canonical payout partition.
    pub basis: Box<NativeClaimBasisV1>,
    /// Exact evidence-only recovery schedule.
    pub recovery: Box<EvidenceOnlyRecoveryPolicyV1>,
    /// Exact quantized price-measure semantics.
    pub price_policy: Box<PriceMeasurePolicyV1>,
    /// Immutable Realm/Profile and venue semantics.
    pub genesis: Box<MarketGenesisProfileV2>,
    /// Per-occurrence component funding quote.
    pub quote: Box<SeriesFundingQuoteV1>,
    /// Operational attachment identities excluded from Market identity.
    pub attachment: Box<SeriesAttachmentPlanV1>,
}

/// Exact current Product/Series bodies selected during successor registration.
#[derive(Debug)]
pub struct AuthenticatedSeriesArtifactsV4 {
    /// Finite recurring Series plan.
    series: Box<SeriesPlanV5>,
    /// Immutable principal/refund/sink/mint/program ownership.
    funding_terms: Box<SeriesFundingTermsV2>,
    /// Reusable relative Product semantics.
    template: Box<ProductTemplateV4>,
    /// Exact canonical payout partition.
    basis: Box<NativeClaimBasisV1>,
    /// Exact evidence-only recovery schedule.
    recovery: Box<EvidenceOnlyRecoveryPolicyV1>,
    /// Exact quantized price-measure semantics.
    price_policy: Box<PriceMeasurePolicyV1>,
    /// Immutable Realm/Profile and venue semantics.
    genesis: Box<MarketGenesisProfileV2>,
    /// Current per-occurrence and 46-slot foundation funding quote.
    quote: Box<SeriesFundingQuoteV4>,
    /// Current QuoteV4-bound operational attachment.
    attachment: Box<SeriesAttachmentPlanV4>,
}

/// Authenticated typed compiler output for one exact Product/Series artifact graph.
///
/// Private fields ensure callers cannot turn a proposed manifest into
/// authority. Construction recomputes every identity from the authenticated
/// registry release, Source release, and reopened Product/Series artifacts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(test)]
pub struct AuthenticatedCompiledProductSeriesBundleV1 {
    artifact_account: Pubkey,
    bundle: CompiledProductSeriesBundleV1,
    bundle_id: CompiledProductSeriesBundleV1Id,
}

/// Authenticated current compiler output for one exact ProfileV4 graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedCompiledProductSeriesBundleV5 {
    artifact_account: Pubkey,
    bundle: CompiledProductSeriesBundleV5,
    bundle_id: CompiledProductSeriesBundleV5Id,
}

impl AuthenticatedCompiledProductSeriesBundleV5 {
    /// Exact content-addressed BundleV5 account.
    pub const fn artifact_account(self) -> Pubkey {
        self.artifact_account
    }

    /// Complete ProfileV4/QuoteV4/AttachmentV4 graph.
    pub const fn bundle(self) -> CompiledProductSeriesBundleV5 {
        self.bundle
    }

    /// Content identity of the exact compiler output.
    pub const fn bundle_id(self) -> CompiledProductSeriesBundleV5Id {
        self.bundle_id
    }
}

#[cfg(test)]
impl AuthenticatedCompiledProductSeriesBundleV1 {
    /// Exact content-addressed compiler-output artifact account.
    pub const fn artifact_account(self) -> Pubkey {
        self.artifact_account
    }

    /// Complete typed graph reconstructed from authenticated semantic owners.
    pub const fn bundle(self) -> CompiledProductSeriesBundleV1 {
        self.bundle
    }

    /// Content identity of the exact reconstructed compiler output.
    pub const fn bundle_id(self) -> CompiledProductSeriesBundleV1Id {
        self.bundle_id
    }
}

/// Concrete Product/Source compilation authority over registry and Source release receipts.
#[derive(Clone, Copy, Debug)]
pub struct AuthenticatedProductSourceAuthorityV1 {
    registry: RegistryCapabilityProjectionV2,
    source_release: AuthenticatedSourceReleaseV1,
}

impl AuthenticatedProductSourceAuthorityV1 {
    /// Join one exact Product registry artifact to one exact Source release.
    pub fn new(
        registry: AuthenticatedRegistryCapabilityReleaseV3,
        source_release: AuthenticatedSourceReleaseV1,
    ) -> Outcome<Self> {
        Self::from_projection(registry.projection(), source_release)
    }

    /// Join a registered-Series capability receipt to one exact Source release.
    pub fn from_series_registry(
        registry: AuthenticatedRegistryCapabilityV3,
        source_release: AuthenticatedSourceReleaseV1,
    ) -> Outcome<Self> {
        Self::from_projection(registry.projection(), source_release)
    }

    fn from_projection(
        projection: RegistryCapabilityProjectionV2,
        source_release: AuthenticatedSourceReleaseV1,
    ) -> Outcome<Self> {
        let source_plane_id = source_release
            .source_plane()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            source_plane_id.bytes() == projection.semantic_owners.source_plane_contract_id.bytes()
                && source_release.source_spec_id().bytes()
                    == projection.semantic_owners.source_spec_id.bytes(),
            ClutchError::MismatchedState,
        )?;
        Ok(Self {
            registry: projection,
            source_release,
        })
    }

    /// Exact authenticated Source release used for occurrence compilation.
    pub const fn source_release(self) -> AuthenticatedSourceReleaseV1 {
        self.source_release
    }
}

impl AuthenticatedSourceSeriesAuthorityV3 for AuthenticatedProductSourceAuthorityV1 {
    fn authenticate_registry_projection(
        &self,
        projection: &RegistryCapabilityProjectionV2,
    ) -> clutch_product_series::Result<()> {
        if *projection != self.registry {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }

    fn authenticated_source_plane(
        &self,
        expected_contract_id: ContentId,
    ) -> clutch_product_series::Result<SourcePlaneProgramV3> {
        let source_plane = self.source_release.source_plane();
        if source_plane
            .id()
            .map_err(|_| clutch_product_series::Error::MismatchedArtifact)?
            .bytes()
            != expected_contract_id.bytes()
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(source_plane)
    }

    fn authenticate_source_spec(
        &self,
        expected_source_spec_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if self.source_release.source_spec_id().bytes() != expected_source_spec_id.bytes() {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }

    fn authenticated_summary_program(
        &self,
        expected_summary_program_id: ContentId,
    ) -> clutch_product_series::Result<SummaryProgramV3> {
        let summary = self.registry.registry.summary_program;
        if summary
            .id()
            .map_err(|_| clutch_product_series::Error::MismatchedArtifact)?
            .bytes()
            != expected_summary_program_id.bytes()
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(summary)
    }

    fn resolve_statistic(
        &self,
        registry_release_id: ContentId,
        capability_profile_id: ContentId,
        statistic_registry_value: u16,
    ) -> clutch_product_series::Result<StatisticKindV3> {
        let projection = self.registry.projection;
        if registry_release_id != projection.registry_release_id
            || capability_profile_id != projection.capability_profile_id
            || statistic_registry_value != projection.statistic_registry_value
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(self.registry.registry.resolved_statistic)
    }

    fn resolve_coverage_policy(
        &self,
        registry_release_id: ContentId,
        capability_profile_id: ContentId,
        coverage_policy_registry_value: u16,
    ) -> clutch_product_series::Result<u16> {
        let projection = self.registry.projection;
        if registry_release_id != projection.registry_release_id
            || capability_profile_id != projection.capability_profile_id
            || coverage_policy_registry_value != projection.coverage_policy_registry_value
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(self.registry.registry.resolved_coverage_policy_value)
    }
}

/// Private-field registry authorization minted only after the complete
/// authoritative registry and Source compilation join succeeds.
#[derive(Clone, Copy, Debug)]
#[cfg(test)]
pub struct AuthenticatedSeriesRegistrationV1 {
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: SeriesFundingTermsV2Id,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    neutral_lamport_sink: ContentId,
    first_source_occurrence_id: SourceOccurrenceV1Id,
    requirements: SeriesFundingRequirementsV1,
}

/// Current ProfileV4/BundleV5 registration authorization.
#[derive(Clone, Copy, Debug)]
pub struct AuthenticatedSeriesRegistrationV2 {
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: SeriesFundingTermsV2Id,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    neutral_lamport_sink: ContentId,
    first_source_occurrence_id: SourceOccurrenceV1Id,
    compiler_bundle_id: CompiledProductSeriesBundleV5Id,
}

impl AuthenticatedSeriesRegistrationV2 {
    /// Exact recurring Series authorized for one replay anchor.
    pub const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    /// Exact first compiled Source occurrence.
    pub const fn first_source_occurrence_id(self) -> SourceOccurrenceV1Id {
        self.first_source_occurrence_id
    }

    /// Exact current compiler graph consumed by registration.
    pub const fn compiler_bundle_id(self) -> CompiledProductSeriesBundleV5Id {
        self.compiler_bundle_id
    }
}

#[cfg(test)]
impl AuthenticatedSeriesRegistrationV1 {
    /// Exact recurring Series authorized for one persistent replay anchor.
    pub const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    /// Exact first compiled occurrence, proving Source selectors were joined.
    pub const fn first_source_occurrence_id(self) -> SourceOccurrenceV1Id {
        self.first_source_occurrence_id
    }

    /// Exact whole-Series capitalization derived during registration.
    pub const fn requirements(self) -> SeriesFundingRequirementsV1 {
        self.requirements
    }
}

/// Authenticate one registration against the authoritative registry/Source
/// adapter and compile ordinal zero as a complete selector/source join.
#[cfg(test)]
pub fn authenticate_series_registration<A: AuthenticatedSourceSeriesAuthorityV3 + ?Sized>(
    authority: &A,
    artifacts: &AuthenticatedSeriesArtifactsV1,
    projection: &RegistryCapabilityProjectionV2,
    request: RegisterSeriesIntentV1,
) -> Outcome<AuthenticatedSeriesRegistrationV1> {
    let series_plan_id = artifacts
        .series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terms_id = artifacts
        .funding_terms
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        request.series_plan_id == series_plan_id
            && request.funding_terms_id == funding_terms_id
            && request.registry_release_id == projection.registry_release_id
            && request.capability_profile_id == projection.capability_profile_id,
        ClutchError::MismatchedState,
    )?;
    let requirements = artifacts.validate_registry_projection(projection)?;
    let first = compile_source_occurrence_v3(
        authority,
        &artifacts.series,
        &artifacts.template,
        &artifacts.basis,
        &artifacts.recovery,
        &artifacts.price_policy,
        &artifacts.genesis,
        &artifacts.attachment,
        projection,
        0,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(
        first.series_plan_id == series_plan_id,
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedSeriesRegistrationV1 {
        series_plan_id,
        funding_terms_id,
        registry_release_id: projection.registry_release_id,
        capability_profile_id: projection.capability_profile_id,
        neutral_lamport_sink: artifacts.funding_terms.neutral_lamport_sink,
        first_source_occurrence_id: first
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        requirements,
    })
}

/// Authenticate current Series registration against ProfileV4 and BundleV5.
pub fn authenticate_series_registration_v2<A: AuthenticatedSourceSeriesAuthorityV3 + ?Sized>(
    authority: &A,
    artifacts: &AuthenticatedSeriesArtifactsV4,
    projection: &RegistryCapabilityProjectionV2,
    compiler_bundle: AuthenticatedCompiledProductSeriesBundleV5,
    request: RegisterSeriesIntentV1,
) -> Outcome<AuthenticatedSeriesRegistrationV2> {
    let series_plan_id = artifacts
        .series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terms_id = artifacts
        .funding_terms
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        request.series_plan_id == series_plan_id
            && request.funding_terms_id == funding_terms_id
            && request.registry_release_id == projection.registry_release_id
            && request.capability_profile_id == projection.capability_profile_id
            && compiler_bundle.bundle().series_plan_id == series_plan_id
            && compiler_bundle.bundle().funding_terms_id == funding_terms_id
            && compiler_bundle.bundle().registry_release_id == projection.registry_release_id
            && compiler_bundle.bundle().capability_profile_id.content_id()
                == projection.capability_profile_id,
        ClutchError::MismatchedState,
    )?;
    artifacts.validate_registry_projection(projection)?;
    let first = compile_source_occurrence_v4(
        authority,
        &artifacts.series,
        &artifacts.template,
        &artifacts.basis,
        &artifacts.recovery,
        &artifacts.price_policy,
        &artifacts.genesis,
        &artifacts.attachment,
        projection,
        0,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(
        first.series_plan_id == series_plan_id,
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedSeriesRegistrationV2 {
        series_plan_id,
        funding_terms_id,
        registry_release_id: projection.registry_release_id,
        capability_profile_id: projection.capability_profile_id,
        neutral_lamport_sink: artifacts.funding_terms.neutral_lamport_sink,
        first_source_occurrence_id: first
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        compiler_bundle_id: compiler_bundle.bundle_id(),
    })
}

/// Authenticated persistent registry account.
///
/// Private fields prevent a caller from turning a decoded body into account
/// authority. Instances are minted only after owner/PDA/role/codec/rent checks,
/// or by the canonical one-shot activation write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(test)]
pub struct AuthenticatedSeriesRegistryAccountV1 {
    account: Pubkey,
    value: SeriesRegistryAccountV1,
}

/// Current authenticated 0x7f/version2 Series registry account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSeriesRegistryAccountV2 {
    account: Pubkey,
    value: SeriesRegistryAccountV2,
}

impl AuthenticatedSeriesRegistryAccountV2 {
    /// Exact authenticated registry PDA.
    pub const fn account(self) -> Pubkey {
        self.account
    }

    /// Exact hostile-decoded current registry body.
    pub const fn value(self) -> SeriesRegistryAccountV2 {
        self.value
    }

    /// Whether successor activation was already consumed.
    pub const fn activation_consumed(self) -> bool {
        self.value.activation_consumed
    }
}

#[cfg(test)]
impl AuthenticatedSeriesRegistryAccountV1 {
    /// Exact authenticated registry PDA.
    pub const fn account(&self) -> Pubkey {
        self.account
    }

    /// Exact hostile-decoded registry body.
    pub const fn value(&self) -> SeriesRegistryAccountV1 {
        self.value
    }

    /// Whether its one permitted activation has already been consumed.
    pub const fn activation_consumed(&self) -> bool {
        self.value.activation_consumed
    }
}

/// Authenticated mutable funding account.
///
/// The private account key binds the decoded wrapper to the exact PDA checked
/// by [`read_series_funding_account`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(test)]
pub struct AuthenticatedSeriesFundingAccountV1 {
    account: Pubkey,
    value: SeriesFundingAccountV1,
}

/// Current authenticated 0x80/version2 funding account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSeriesFundingAccountV2 {
    account: Pubkey,
    value: SeriesFundingAccountV2,
}

/// Private physical authority for the five release-selected collateral-vault
/// rent principals persisted by FundingV2.
///
/// There is intentionally no public constructor. The future activation
/// composer must mint this only after exact vault creation/postwrite and live
/// Rent authentication; an instruction array can never become refundable
/// principal merely by reaching the generic state constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedSeriesCollateralVaultRentV2 {
    series_plan_id: SeriesPlanV5Id,
    principal_lamports: [u64; SERIES_COLLATERAL_VAULT_COUNT_V2],
    authentication_id: ContentId,
}

/// Private same-instruction proof of the five current collateral vaults.
///
/// Construction authenticates the exact Realm/Profile/release deployment,
/// creates every release-selected vault at its canonical PDA and live Rent
/// minimum, executes and postchecks every nonzero holder transfer, and binds
/// the exact principal/donation split to the resulting custody bytes. There
/// is intentionally no projection into a caller-shaped rent tuple. The only
/// consumer moves the embedded rent authority directly into FundingV2
/// activation after a separate authority has authenticated the six lamport
/// compartments.
#[derive(Debug)]
struct AuthenticatedSeriesCollateralActivationPostwriteV2 {
    bound: BoundRealmCollateralV2,
    deployment: crate::collateral_release::AuthenticatedCollateralReleaseDeploymentV2,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: SeriesFundingTermsV2Id,
    compiler_bundle_id: CompiledProductSeriesBundleV5Id,
    funding_quote_id: clutch_product_series::SeriesFundingQuoteV4Id,
    attachment_plan_id: clutch_product_series::SeriesAttachmentPlanV4Id,
    funding_account: Pubkey,
    payer: Pubkey,
    payer_token_account: Pubkey,
    payer_token_authority: Pubkey,
    collateral_authority: Pubkey,
    neutral_lamport_sink: Pubkey,
    rent_sysvar: Pubkey,
    rent: RentParameters,
    payer_lamports_before: u64,
    payer_lamports_after: u64,
    neutral_sink_lamports_before: u64,
    neutral_sink_lamports_after: u64,
    principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    donations: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    vault_accounts: [Pubkey; SERIES_COLLATERAL_VAULT_COUNT_V2],
    vault_data_lengths: [u64; SERIES_COLLATERAL_VAULT_COUNT_V2],
    vault_rent_principal_lamports: [u64; SERIES_COLLATERAL_VAULT_COUNT_V2],
    swept_prefund_donation_lamports: [u64; SERIES_COLLATERAL_VAULT_COUNT_V2],
    vault_collateral_principal_atoms: [u64; SERIES_COLLATERAL_VAULT_COUNT_V2],
    vault_collateral_donation_atoms: [u64; SERIES_COLLATERAL_VAULT_COUNT_V2],
    vault_collateral_atoms_after: [u64; SERIES_COLLATERAL_VAULT_COUNT_V2],
    vault_poststate_ids: [ContentId; SERIES_COLLATERAL_VAULT_COUNT_V2],
    transfer_poststate_ids: [ContentId; SERIES_COLLATERAL_VAULT_COUNT_V2],
    source_prestate_id: ContentId,
    source_poststate_id: ContentId,
    vault_rent: AuthenticatedSeriesCollateralVaultRentV2,
    receipt_id: ContentId,
}

impl AuthenticatedSeriesCollateralActivationPostwriteV2 {
    const fn receipt_id(&self) -> ContentId {
        self.receipt_id
    }
}

impl AuthenticatedSeriesFundingAccountV2 {
    /// Exact mutable funding PDA.
    pub const fn account(self) -> Pubkey {
        self.account
    }

    /// Exact hostile-decoded current body.
    pub const fn value(self) -> SeriesFundingAccountV2 {
        self.value
    }
}

/// Exact Realm-selected collateral graph for one authenticated Series funding
/// account.
///
/// Construction authenticates the immutable Product bodies, local registry and
/// funding PDAs, Realm/Profile/mint/program binding, both receive-only token
/// destinations, both System-owned lamport destinations, and the canonical
/// Series collateral authority PDA. It does not substitute for central
/// registry-release provenance, which remains a separately disabled join.
#[derive(Clone, Copy, Debug)]
#[cfg(test)]
pub struct AuthenticatedSeriesCollateralFundingV1 {
    bound: BoundRealmCollateralV2,
    join: SeriesCollateralFundingJoinV2,
    authority: TransferAuthorityV2,
    funding_account: AuthenticatedSeriesFundingAccountV1,
}

#[cfg(test)]
impl AuthenticatedSeriesCollateralFundingV1 {
    /// Exact authenticated Realm collateral binding.
    pub const fn bound(&self) -> BoundRealmCollateralV2 {
        self.bound
    }

    /// Exact immutable Series/collateral funding identity graph.
    pub const fn join(&self) -> SeriesCollateralFundingJoinV2 {
        self.join
    }

    /// Canonical program-derived collateral authority observation.
    pub const fn authority(&self) -> TransferAuthorityV2 {
        self.authority
    }

    /// Exact funding wrapper whose state owns component balances and vault rent.
    pub const fn funding_account(&self) -> AuthenticatedSeriesFundingAccountV1 {
        self.funding_account
    }
}

/// One-shot terminal authorization refined by the exact collateral graph.
#[derive(Clone, Copy, Debug)]
#[cfg(test)]
pub struct AuthenticatedSeriesCollateralTerminalV1 {
    funding: AuthenticatedSeriesCollateralFundingV1,
    join: SeriesCollateralTerminalJoinV2,
    projection: SeriesFundingTerminalProjectionV1,
}

/// Exact accepted terminal movement from one Series lamport compartment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(test)]
pub struct AcceptedSeriesLamportTerminalV1 {
    /// One-shot terminal receipt authorizing this disposition.
    pub terminal_receipt: ContentId,
    /// Ordered Series funding component.
    pub component: SeriesFundingComponentV1,
    /// Exact payer principal returned to the FundingTerms account.
    pub refunded_principal_lamports: u64,
    /// Exact donation residue sent to the neutral lamport sink.
    pub neutral_donation_lamports: u64,
}

#[cfg(test)]
impl AuthenticatedSeriesCollateralTerminalV1 {
    /// Exact authenticated funding graph retained by the terminal receipt.
    pub const fn funding(&self) -> AuthenticatedSeriesCollateralFundingV1 {
        self.funding
    }

    /// Exact collateral terminal join bound to the SBF one-shot receipt.
    pub const fn join(&self) -> SeriesCollateralTerminalJoinV2 {
        self.join
    }

    /// State-derived principal and donation terminal amounts.
    pub const fn projection(&self) -> SeriesFundingTerminalProjectionV1 {
        self.projection
    }
}

#[cfg(test)]
impl AuthenticatedSeriesFundingAccountV1 {
    /// Exact authenticated funding PDA.
    pub const fn account(&self) -> Pubkey {
        self.account
    }

    /// Exact hostile-decoded funding wrapper and pure state.
    pub const fn value(&self) -> SeriesFundingAccountV1 {
        self.value
    }
}

/// Private-field terminal authorization over the exact consumed replay anchor,
/// exact funding PDA/body, and authenticated immutable artifacts.
///
/// This receipt authorizes no transfer by itself. Collateral and lamport
/// adapters must still bind its exact destinations/amounts to runtime accounts
/// and verify post-deltas before the funding account may be closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(test)]
pub struct AuthenticatedSeriesTerminalV1 {
    registry_account: Pubkey,
    funding_account: Pubkey,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: SeriesFundingTermsV2Id,
    funding_quote_id: clutch_product_series::SeriesFundingQuoteId,
    activation_generation: u64,
    projection: SeriesFundingTerminalProjectionV1,
    receipt_id: ContentId,
}

#[derive(Clone, Copy, Debug)]
#[cfg(test)]
struct AuthenticatedSeriesClockAuthorityV1 {
    series_plan_id: SeriesPlanV5Id,
    clock: AuthenticatedClockBucketV1,
}

/// Current Clock-only authority. Every value-bearing method other than the
/// free lapse refuses, so a Clock receipt cannot authorize funding movement.
#[derive(Clone, Copy, Debug)]
struct AuthenticatedSeriesClockAuthorityV2 {
    series_plan_id: SeriesPlanV5Id,
    clock: AuthenticatedClockBucketV1,
}

impl AuthenticatedSeriesFundingAuthorityV2 for AuthenticatedSeriesClockAuthorityV2 {
    fn authenticate_activation(
        &self,
        _series: &SeriesPlanV5,
        _funding_terms_id: SeriesFundingTermsV2Id,
        _compiler_bundle_id: CompiledProductSeriesBundleV5Id,
        _quote: &SeriesFundingQuoteV4,
        _attachment: &SeriesAttachmentPlanV4,
        _principal: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        _donations: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn current_bucket(&self, series: &SeriesPlanV5) -> clutch_product_series::Result<u64> {
        if series.id()? != self.series_plan_id {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(self.clock.bucket())
    }

    fn authenticate_reservation(
        &self,
        _state: &SeriesFundingStateV2,
        _ordinal: u32,
        _market_instance_id: clutch_product_series::MarketInstanceV2Id,
        _source_occurrence_id: SourceOccurrenceV1Id,
        _series_market_link_id: ContentId,
        _disposition: clutch_product_series::SeriesMarketDispositionV1,
        _debits: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        _reservation_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_pending_completion(
        &self,
        _state: &SeriesFundingStateV2,
        _completion_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_pending_abort(
        &self,
        _state: &SeriesFundingStateV2,
        _abort_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_donation(
        &self,
        _state: &SeriesFundingStateV2,
        _component: SeriesFundingComponentV2,
        _amount: ComponentDebitV1,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_close(
        &self,
        _state: &SeriesFundingStateV2,
        _terminal_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }
}

/// Exact physical-surplus authority for one current lamport compartment.
#[derive(Clone, Copy, Debug)]
struct AuthenticatedSeriesDonationAuthorityV2 {
    state_id: clutch_product_series::SeriesFundingStateV2Id,
    component: SeriesFundingComponentV2,
    amount: ComponentDebitV1,
}

impl AuthenticatedSeriesFundingAuthorityV2 for AuthenticatedSeriesDonationAuthorityV2 {
    fn authenticate_activation(
        &self,
        _series: &SeriesPlanV5,
        _funding_terms_id: SeriesFundingTermsV2Id,
        _compiler_bundle_id: CompiledProductSeriesBundleV5Id,
        _quote: &SeriesFundingQuoteV4,
        _attachment: &SeriesAttachmentPlanV4,
        _principal: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        _donations: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn current_bucket(&self, _series: &SeriesPlanV5) -> clutch_product_series::Result<u64> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_reservation(
        &self,
        _state: &SeriesFundingStateV2,
        _ordinal: u32,
        _market_instance_id: clutch_product_series::MarketInstanceV2Id,
        _source_occurrence_id: SourceOccurrenceV1Id,
        _series_market_link_id: ContentId,
        _disposition: clutch_product_series::SeriesMarketDispositionV1,
        _debits: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        _reservation_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_pending_completion(
        &self,
        _state: &SeriesFundingStateV2,
        _completion_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_pending_abort(
        &self,
        _state: &SeriesFundingStateV2,
        _abort_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_donation(
        &self,
        state: &SeriesFundingStateV2,
        component: SeriesFundingComponentV2,
        amount: ComponentDebitV1,
    ) -> clutch_product_series::Result<()> {
        if state.id()? != self.state_id || component != self.component || amount != self.amount {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }

    fn authenticate_close(
        &self,
        _state: &SeriesFundingStateV2,
        _terminal_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }
}

#[cfg(test)]
impl AuthenticatedSeriesFundingAuthorityV1 for AuthenticatedSeriesClockAuthorityV1 {
    fn authenticated_current_bucket(
        &self,
        series: &SeriesPlanV5,
    ) -> clutch_product_series::Result<u64> {
        if series.id()? != self.series_plan_id {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(self.clock.bucket())
    }
}

#[cfg(test)]
impl AuthenticatedSeriesTerminalV1 {
    /// Exact persistent replay-anchor PDA.
    pub const fn registry_account(&self) -> Pubkey {
        self.registry_account
    }

    /// Exact mutable funding PDA whose closed body was authenticated.
    pub const fn funding_account(&self) -> Pubkey {
        self.funding_account
    }

    /// Exact recurring Series identity.
    pub const fn series_plan_id(&self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    /// Exact immutable terminal-ownership terms identity.
    pub const fn funding_terms_id(&self) -> SeriesFundingTermsV2Id {
        self.funding_terms_id
    }

    /// Exact funding quote identity retained by the closed state.
    pub const fn funding_quote_id(&self) -> clutch_product_series::SeriesFundingQuoteId {
        self.funding_quote_id
    }

    /// Canonical one-shot activation generation.
    pub const fn activation_generation(&self) -> u64 {
        self.activation_generation
    }

    /// State-derived terminal amounts and immutable destinations.
    pub const fn projection(&self) -> SeriesFundingTerminalProjectionV1 {
        self.projection
    }

    /// Digest of the exact registry PDA/body and funding PDA/body join.
    pub const fn id(&self) -> ContentId {
        self.receipt_id
    }
}

#[cfg(test)]
impl AuthenticatedSeriesArtifactsV1 {
    /// Apply the pure complete registry join after the adapter has separately
    /// authenticated the authoritative registry release and selector mapping.
    ///
    /// Success here does not authenticate a caller-built projection; it merely
    /// avoids duplicating any artifact equality or capability rule in SBF.
    pub fn validate_registry_projection(
        &self,
        projection: &RegistryCapabilityProjectionV2,
    ) -> Outcome<SeriesFundingRequirementsV1> {
        self.series
            .validate_bindings(
                &self.template,
                &self.basis,
                &self.recovery,
                &self.price_policy,
                &self.genesis,
                &self.attachment,
                projection,
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        self.funding_terms
            .validate_bindings(
                &self.series,
                &self.template,
                &self.basis,
                &self.recovery,
                &self.price_policy,
                &self.genesis,
                projection,
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        self.quote
            .validate_recovery_binding(&self.recovery)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        SeriesFundingRequirementsV1::derive(&self.series, &self.attachment, &self.quote)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
    }
}

impl AuthenticatedSeriesArtifactsV4 {
    /// Apply the complete current registry join without accepting a caller DTO.
    pub fn validate_registry_projection(
        &self,
        projection: &RegistryCapabilityProjectionV2,
    ) -> Outcome<()> {
        self.series
            .validate_bindings_v4(
                &self.template,
                &self.basis,
                &self.recovery,
                &self.price_policy,
                &self.genesis,
                &self.attachment,
                projection,
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        self.funding_terms
            .validate_bindings(
                &self.series,
                &self.template,
                &self.basis,
                &self.recovery,
                &self.price_policy,
                &self.genesis,
                projection,
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        self.quote
            .validate()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            self.quote.evidence_only_recovery_policy_id
                == self
                    .recovery
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    .content_id()
                && self.attachment.funding_quote_id
                    == self
                        .quote
                        .id()
                        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
            ClutchError::MismatchedState,
        )
    }
}

/// Authenticate current BundleV5 against the complete reopened Product graph.
pub fn authenticate_compiled_product_series_bundle_v5(
    program_id: &Pubkey,
    bundle_artifact: &AccountInfo<'_>,
    registry: AuthenticatedRegistryCapabilityReleaseV3,
    source_release: AuthenticatedSourceReleaseV1,
    artifacts: &AuthenticatedSeriesArtifactsV4,
) -> Outcome<AuthenticatedCompiledProductSeriesBundleV5> {
    let expected = assemble_compiled_product_series_bundle_v5(ProductSeriesBundleInputsV5 {
        registry: &registry.projection(),
        source_release_manifest_id: source_release.manifest_id(),
        basis: &artifacts.basis,
        recovery: &artifacts.recovery,
        template: &artifacts.template,
        price_policy: &artifacts.price_policy,
        genesis: &artifacts.genesis,
        funding_quote: &artifacts.quote,
        attachment: &artifacts.attachment,
        series: &artifacts.series,
        funding_terms: &artifacts.funding_terms,
    })
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_id = expected
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let decoded = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV5>(
        program_id,
        bundle_artifact,
        expected_id.content_id(),
    )?;
    require(*decoded.value() == expected, ClutchError::MismatchedState)?;
    Ok(AuthenticatedCompiledProductSeriesBundleV5 {
        artifact_account: *bundle_artifact.key,
        bundle: expected,
        bundle_id: expected_id,
    })
}

#[cfg(test)]
fn reconstruct_compiled_product_series_bundle_v1(
    registry: AuthenticatedRegistryCapabilityReleaseV3,
    source_release: AuthenticatedSourceReleaseV1,
    artifacts: &AuthenticatedSeriesArtifactsV1,
) -> Outcome<CompiledProductSeriesBundleV1> {
    Ok(CompiledProductSeriesBundleV1 {
        registry_release_id: registry.projection().registry_release_id,
        capability_profile_id: registry.projection().capability_profile_id,
        source_release_manifest_id: source_release.manifest_id(),
        source_plane_contract_id: artifacts.template.source_plane_contract_id,
        source_spec_id: artifacts.template.source_spec_id,
        summary_program_id: artifacts.template.summary_program_id,
        product_compiler_release_id: artifacts.template.compiler_release_id,
        native_claim_basis_id: artifacts
            .basis
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        evidence_only_recovery_policy_id: artifacts
            .recovery
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        product_template_id: artifacts
            .template
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        price_measure_policy_id: artifacts
            .price_policy
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        market_genesis_profile_id: artifacts
            .genesis
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        funding_quote_id: artifacts
            .quote
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        attachment_plan_id: artifacts
            .attachment
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        series_plan_id: artifacts
            .series
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        funding_terms_id: artifacts
            .funding_terms
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
    })
}

/// Authenticate an untrusted compiler-output artifact against the exact
/// registry, Source release, and Product/Series bodies admitted in this call.
#[cfg(test)]
pub fn authenticate_compiled_product_series_bundle_v1(
    program_id: &Pubkey,
    bundle_artifact: &AccountInfo<'_>,
    registry: AuthenticatedRegistryCapabilityReleaseV3,
    source_release: AuthenticatedSourceReleaseV1,
    artifacts: &AuthenticatedSeriesArtifactsV1,
) -> Outcome<AuthenticatedCompiledProductSeriesBundleV1> {
    let expected =
        reconstruct_compiled_product_series_bundle_v1(registry, source_release, artifacts)?;
    let expected_id = expected
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let decoded = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV1>(
        program_id,
        bundle_artifact,
        expected_id.content_id(),
    )?;
    require(*decoded.value() == expected, ClutchError::MismatchedState)?;
    Ok(AuthenticatedCompiledProductSeriesBundleV1 {
        artifact_account: *bundle_artifact.key,
        bundle: expected,
        bundle_id: expected_id,
    })
}

/// Authenticate and decode the exact nine immutable Product/Series artifacts.
///
/// The account order is frozen by the `IX_SERIES_ARTIFACT_*` constants. Every
/// dependent expected ID comes from an already-authenticated parent body, not
/// from another instruction field. The two root IDs are the registration
/// payload's Series and FundingTerms claims. Central registry and runtime
/// release accounts are intentionally absent from this mechanical segment.
#[cfg(test)]
pub fn authenticate_series_artifact_accounts(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    expected_series: SeriesPlanV5Id,
    expected_funding_terms: SeriesFundingTermsV2Id,
) -> Outcome<AuthenticatedSeriesArtifactsV1> {
    require_count(accounts, SERIES_ARTIFACT_ACCOUNT_COUNT_V1)?;
    expected_series
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    expected_funding_terms
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let series = authenticate_product_artifact_v1::<SeriesPlanV5>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_PLAN],
        expected_series.content_id(),
    )?
    .into_value();
    require(
        series
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == expected_series,
        ClutchError::MismatchedState,
    )?;
    let funding_terms = authenticate_product_artifact_v1::<SeriesFundingTermsV2>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_FUNDING_TERMS],
        expected_funding_terms.content_id(),
    )?
    .into_value();
    require(
        funding_terms
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == expected_funding_terms
            && funding_terms.series_plan_id == expected_series,
        ClutchError::MismatchedState,
    )?;

    let template = authenticate_product_artifact_v1::<ProductTemplateV4>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_TEMPLATE],
        series.product_template_id.content_id(),
    )?
    .into_value();
    require(
        template
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == series.product_template_id,
        ClutchError::MismatchedState,
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_GENESIS],
        series.market_genesis_profile_id.content_id(),
    )?
    .into_value();
    require(
        genesis
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == series.market_genesis_profile_id,
        ClutchError::MismatchedState,
    )?;
    let attachment = authenticate_product_artifact_v1::<SeriesAttachmentPlanV1>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_ATTACHMENT],
        series.attachment_plan_id.content_id(),
    )?
    .into_value();
    require(
        attachment
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == series.attachment_plan_id,
        ClutchError::MismatchedState,
    )?;

    let basis = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_BASIS],
        template.native_claim_basis_id.content_id(),
    )?
    .into_value();
    let recovery = authenticate_product_artifact_v1::<EvidenceOnlyRecoveryPolicyV1>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_RECOVERY],
        template.evidence_only_recovery_policy_id.content_id(),
    )?
    .into_value();
    require(
        recovery
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == template.evidence_only_recovery_policy_id,
        ClutchError::MismatchedState,
    )?;
    let price_policy = authenticate_product_artifact_v1::<PriceMeasurePolicyV1>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_PRICE_POLICY],
        genesis.price_measure_policy_id.content_id(),
    )?
    .into_value();
    require(
        price_policy
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == genesis.price_measure_policy_id,
        ClutchError::MismatchedState,
    )?;
    let quote = authenticate_product_artifact_v1::<SeriesFundingQuoteV1>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_QUOTE],
        attachment.funding_quote_id.content_id(),
    )?
    .into_value();
    require(
        quote
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == attachment.funding_quote_id,
        ClutchError::MismatchedState,
    )?;

    template
        .validate_bindings(&basis, &recovery)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    genesis
        .validate_bindings(&basis, &price_policy)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    quote
        .validate_recovery_binding(&recovery)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    SeriesFundingRequirementsV1::derive(&series, &attachment, &quote)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    Ok(AuthenticatedSeriesArtifactsV1 {
        series,
        funding_terms,
        template,
        basis,
        recovery,
        price_policy,
        genesis,
        quote,
        attachment,
    })
}

/// Authenticate the current nine-account ProfileV4/QuoteV4 Product graph.
pub fn authenticate_series_artifact_accounts_v4(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    expected_series: SeriesPlanV5Id,
    expected_funding_terms: SeriesFundingTermsV2Id,
) -> Outcome<AuthenticatedSeriesArtifactsV4> {
    require_count(accounts, SERIES_ARTIFACT_ACCOUNT_COUNT_V1)?;
    expected_series
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    expected_funding_terms
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let series = authenticate_product_artifact_v1::<SeriesPlanV5>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_PLAN],
        expected_series.content_id(),
    )?
    .into_value();
    require(
        series
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == expected_series,
        ClutchError::MismatchedState,
    )?;
    let funding_terms = authenticate_product_artifact_v1::<SeriesFundingTermsV2>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_FUNDING_TERMS],
        expected_funding_terms.content_id(),
    )?
    .into_value();
    require(
        funding_terms
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == expected_funding_terms
            && funding_terms.series_plan_id == expected_series,
        ClutchError::MismatchedState,
    )?;
    let template = authenticate_product_artifact_v1::<ProductTemplateV4>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_TEMPLATE],
        series.product_template_id.content_id(),
    )?
    .into_value();
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_GENESIS],
        series.market_genesis_profile_id.content_id(),
    )?
    .into_value();
    let attachment = authenticate_product_artifact_v1::<SeriesAttachmentPlanV4>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_ATTACHMENT],
        series.attachment_plan_id.content_id(),
    )?
    .into_value();
    require(
        attachment
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .content_id()
            == series.attachment_plan_id.content_id(),
        ClutchError::MismatchedState,
    )?;
    let basis = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_BASIS],
        template.native_claim_basis_id.content_id(),
    )?
    .into_value();
    let recovery = authenticate_product_artifact_v1::<EvidenceOnlyRecoveryPolicyV1>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_RECOVERY],
        template.evidence_only_recovery_policy_id.content_id(),
    )?
    .into_value();
    let price_policy = authenticate_product_artifact_v1::<PriceMeasurePolicyV1>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_PRICE_POLICY],
        genesis.price_measure_policy_id.content_id(),
    )?
    .into_value();
    let quote = authenticate_product_artifact_v1::<SeriesFundingQuoteV4>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_QUOTE],
        attachment.funding_quote_id.content_id(),
    )?
    .into_value();

    require(
        template
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == series.product_template_id
            && genesis
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == series.market_genesis_profile_id
            && recovery
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == template.evidence_only_recovery_policy_id
            && price_policy
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == genesis.price_measure_policy_id
            && quote
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == attachment.funding_quote_id
            && quote.evidence_only_recovery_policy_id
                == template.evidence_only_recovery_policy_id.content_id(),
        ClutchError::MismatchedState,
    )?;
    template
        .validate_bindings(&basis, &recovery)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    genesis
        .validate_bindings(&basis, &price_policy)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    quote
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    Ok(AuthenticatedSeriesArtifactsV4 {
        series,
        funding_terms,
        template,
        basis,
        recovery,
        price_policy,
        genesis,
        quote,
        attachment,
    })
}

/// Exact accounted balances for all five physical custody pairs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(test)]
pub struct SeriesCustodyBalancesV1 {
    /// Native balance in each zero-data System-owned PDA.
    pub lamports: [u64; SERIES_CUSTODY_COUNT_V1],
    /// Collateral atoms in each release-selected segregated vault.
    pub collateral_atoms: [u64; SERIES_CUSTODY_COUNT_V1],
}

fn add(left: u64, right: u64) -> Outcome<u64> {
    left.checked_add(right)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))
}

fn sub(left: u64, right: u64) -> Outcome<u64> {
    left.checked_sub(right)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))
}

fn component_from_index(index: usize) -> Outcome<SeriesFundingComponentV1> {
    match index {
        0 => Ok(SeriesFundingComponentV1::MarketCore),
        1 => Ok(SeriesFundingComponentV1::RecoveryReserve),
        2 => Ok(SeriesFundingComponentV1::SourceWork),
        3 => Ok(SeriesFundingComponentV1::LiquidityFacility),
        4 => Ok(SeriesFundingComponentV1::WrapperSet),
        _ => Err(ClutchError::NonCanonical.into()),
    }
}

/// Derive exact physical balances from the state-owned principal/donation
/// facts. Consumed allocation is already absent from `remaining_principal`.
#[cfg(test)]
pub fn accounted_custody_balances(
    state: &SeriesFundingStateV1,
) -> Outcome<SeriesCustodyBalancesV1> {
    state
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut value = SeriesCustodyBalancesV1 {
        lamports: [0; SERIES_CUSTODY_COUNT_V1],
        collateral_atoms: [0; SERIES_CUSTODY_COUNT_V1],
    };
    let mut index = 0usize;
    while index < SERIES_CUSTODY_COUNT_V1 {
        let component = state.components[index];
        value.lamports[index] = add(
            component.remaining_principal.lamports,
            component.donations.lamports,
        )?;
        value.collateral_atoms[index] = add(
            component.remaining_principal.collateral_atoms,
            component.donations.collateral_atoms,
        )?;
        index += 1;
    }
    Ok(value)
}

fn collateral_id(key: &Pubkey) -> CollateralId {
    CollateralId::from_bytes(key.to_bytes())
}

fn collateral_content_id(value: ContentId) -> CollateralId {
    CollateralId::from_bytes(value.bytes())
}

/// Authenticate Series' immutable Product projection against the canonical
/// Realm/Profile V2 policy artifact and this ELF's closed release catalog.
///
/// This is the only Series constructor for [`BoundRealmCollateralV2`]; callers
/// cannot supply catalog rows, deployment digests, or parser identities.
#[cfg(test)]
pub fn authenticate_series_realm_collateral_v2(
    program_id: &Pubkey,
    artifacts: &AuthenticatedSeriesArtifactsV1,
    realm_account: &AccountInfo<'_>,
    profile_account: &AccountInfo<'_>,
    policy_account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
) -> Outcome<BoundRealmCollateralV2> {
    let bound = crate::collateral_release::authenticate_realm_collateral_v2(
        program_id,
        realm_account,
        profile_account,
        policy_account,
        token_program,
    )?;
    let realm = bound.realm();
    let policy = bound.policy();
    require(
        realm.realm == collateral_content_id(artifacts.genesis.realm_id)
            && realm.profile == collateral_content_id(artifacts.genesis.profile_id)
            && policy.mint == collateral_content_id(artifacts.funding_terms.collateral_mint)
            && policy.token_program == collateral_content_id(artifacts.funding_terms.token_program),
        ClutchError::MismatchedState,
    )?;
    Ok(bound)
}

fn require_collateral_program(
    account: &AccountInfo<'_>,
    bound: BoundRealmCollateralV2,
) -> Outcome<()> {
    require(
        collateral_id(account.key) == bound.release().token_program,
        ClutchError::WrongTokenProgram,
    )?;
    require(account.executable, ClutchError::WrongTokenProgram)?;
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    require(!account.is_signer, ClutchError::MismatchedState)
}

fn require_series_collateral_authority(
    program_id: &Pubkey,
    series: SeriesPlanV5Id,
    authority: &AccountInfo<'_>,
) -> Outcome<()> {
    expect_pda(
        authority.key,
        seeds::series_collateral_authority_pda(program_id, &series.bytes()),
        None,
    )?;
    require(!authority.is_writable, ClutchError::UnexpectedWritable)?;
    require(!authority.is_signer, ClutchError::MismatchedState)?;
    require(!authority.executable, ClutchError::ExecutableAccount)?;
    require(authority.data_is_empty(), ClutchError::WrongDataLength)
}

fn series_collateral_binding(
    program_id: &Pubkey,
    bound: BoundRealmCollateralV2,
    series: SeriesPlanV5Id,
    component: SeriesFundingComponentV1,
    vault: &AccountInfo<'_>,
    authority: &AccountInfo<'_>,
) -> Outcome<CustodyBindingV2> {
    series
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_series_collateral_authority(program_id, series, authority)?;
    expect_pda(
        vault.key,
        seeds::series_collateral_vault_pda(program_id, &series.bytes(), component.byte()),
        None,
    )?;
    require(vault.is_writable, ClutchError::NotWritable)?;
    require(!vault.executable, ClutchError::ExecutableAccount)?;
    let binding = CustodyBindingV2 {
        account: collateral_id(vault.key),
        owner_authority: collateral_id(authority.key),
        semantic_owner: CollateralId::from_bytes(series.bytes()),
        compartment: u16::from(component.byte()) + 1,
        owner_guard: bound.release().owner_guard,
        owner_authority_is_program_derived: true,
    };
    binding
        .validate(bound.release())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(binding)
}

fn runtime_account_view<'a>(account: &AccountInfo<'_>, data: &'a [u8]) -> RuntimeAccountViewV2<'a> {
    RuntimeAccountViewV2 {
        key: collateral_id(account.key),
        owner_program: collateral_id(account.owner),
        data,
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        executable: account.executable,
    }
}

fn runtime_lamport_account_view<'a>(
    account: &AccountInfo<'_>,
    data: &'a [u8],
) -> RuntimeLamportAccountViewV2<'a> {
    RuntimeLamportAccountViewV2 {
        account: runtime_account_view(account, data),
        lamports: account.lamports(),
    }
}

fn require_system_lamport_destination(account: &AccountInfo<'_>, exact: ContentId) -> Outcome<()> {
    require(
        account.key.to_bytes() == exact.bytes(),
        ClutchError::MismatchedState,
    )?;
    require(account.is_writable, ClutchError::NotWritable)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(
        *account.owner == SYSTEM_PROGRAM_ID && account.data_is_empty(),
        ClutchError::WrongProgramOwner,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SeriesCollateralVaultCoordinateV2 {
    component: SeriesFundingComponentV2,
    seed: u8,
    compartment: u16,
}

fn series_collateral_vault_coordinate_v2(
    index: usize,
) -> Outcome<SeriesCollateralVaultCoordinateV2> {
    match index {
        0 => Ok(SeriesCollateralVaultCoordinateV2 {
            component: SeriesFundingComponentV2::MarketCore,
            seed: 0,
            compartment: 1,
        }),
        1 => Ok(SeriesCollateralVaultCoordinateV2 {
            component: SeriesFundingComponentV2::RecoveryReserve,
            seed: 1,
            compartment: 2,
        }),
        2 => Ok(SeriesCollateralVaultCoordinateV2 {
            component: SeriesFundingComponentV2::SourceWork,
            seed: 2,
            compartment: 3,
        }),
        3 => Ok(SeriesCollateralVaultCoordinateV2 {
            component: SeriesFundingComponentV2::LiquidityFacility,
            seed: 3,
            compartment: 4,
        }),
        4 => Ok(SeriesCollateralVaultCoordinateV2 {
            component: SeriesFundingComponentV2::WrapperSet,
            seed: 4,
            compartment: 5,
        }),
        _ => Err(ClutchError::NonCanonical.into()),
    }
}

fn checked_data_len_v2(value: usize) -> Outcome<u64> {
    u64::try_from(value).map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))
}

fn series_collateral_account_state_id_v2(
    domain: &[u8],
    account: &AccountInfo<'_>,
    data: &[u8],
) -> Outcome<ContentId> {
    let data_len = checked_data_len_v2(data.len())?;
    let role_flags = [
        u8::from(account.is_signer),
        u8::from(account.is_writable),
        u8::from(account.executable),
    ];
    let value = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            domain,
            account.key.as_ref(),
            account.owner.as_ref(),
            &account.lamports().to_le_bytes(),
            &data_len.to_le_bytes(),
            &role_flags,
            data,
        ])
        .to_bytes(),
    );
    require(!value.is_zero(), ClutchError::MismatchedState)?;
    Ok(value)
}

fn series_collateral_funding_commitment_v2(
    principal: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    donations: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
) -> Outcome<ContentId> {
    const COMPONENT_BYTES: usize = 32;
    const FUNDING_BYTES: usize = COMPONENT_BYTES * SERIES_FUNDING_COMPONENT_COUNT_V2;
    let mut encoded = [0u8; FUNDING_BYTES];
    let mut index = 0usize;
    while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
        let offset = index
            .checked_mul(COMPONENT_BYTES)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let second = offset
            .checked_add(8)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let third = offset
            .checked_add(16)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let fourth = offset
            .checked_add(24)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let end = offset
            .checked_add(COMPONENT_BYTES)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        encoded[offset..second].copy_from_slice(&principal[index].lamports.to_le_bytes());
        encoded[second..third]
            .copy_from_slice(&principal[index].collateral_atoms.to_le_bytes());
        encoded[third..fourth].copy_from_slice(&donations[index].lamports.to_le_bytes());
        encoded[fourth..end].copy_from_slice(&donations[index].collateral_atoms.to_le_bytes());
        index += 1;
    }
    let value = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[SERIES_COLLATERAL_ACTIVATION_FUNDING_DOMAIN_V2, &encoded])
            .to_bytes(),
    );
    require(!value.is_zero(), ClutchError::MismatchedState)?;
    Ok(value)
}

fn series_collateral_transfer_poststate_id_v2(
    accepted: AcceptedCollateralTransferV2,
) -> Outcome<ContentId> {
    require(
        accepted.kind == CustodyTransferKindV2::SegregatedFunding,
        ClutchError::MismatchedState,
    )?;
    let value = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_COLLATERAL_TRANSFER_POSTSTATE_DOMAIN_V2,
            &accepted.amount_atoms.to_le_bytes(),
            &accepted.source_semantic_owner.bytes(),
            &accepted.source_compartment.to_le_bytes(),
            &accepted.destination_semantic_owner.bytes(),
            &accepted.destination_compartment.to_le_bytes(),
            &accepted.source_atoms_after.to_le_bytes(),
            &accepted.destination_atoms_after.to_le_bytes(),
            &accepted.mint_supply_after.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!value.is_zero(), ClutchError::MismatchedState)?;
    Ok(value)
}

fn flatten_series_collateral_ids_v2(
    values: &[ContentId; SERIES_COLLATERAL_VAULT_COUNT_V2],
) -> Outcome<[u8; 32 * SERIES_COLLATERAL_VAULT_COUNT_V2]> {
    let mut encoded = [0u8; 32 * SERIES_COLLATERAL_VAULT_COUNT_V2];
    let mut index = 0usize;
    while index < SERIES_COLLATERAL_VAULT_COUNT_V2 {
        let offset = index
            .checked_mul(32)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let end = offset
            .checked_add(32)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        encoded[offset..end].copy_from_slice(&values[index].bytes());
        index += 1;
    }
    Ok(encoded)
}

fn series_collateral_vault_poststate_id_v2(
    coordinate: SeriesCollateralVaultCoordinateV2,
    account: &AccountInfo<'_>,
    data: &[u8],
    rent_principal_lamports: u64,
    swept_prefund_donation_lamports: u64,
    principal_atoms: u64,
    donation_atoms: u64,
    amount_atoms_after: u64,
    transfer_poststate_id: ContentId,
) -> Outcome<ContentId> {
    let account_state_id = series_collateral_account_state_id_v2(
        SERIES_COLLATERAL_VAULT_POSTSTATE_DOMAIN_V2,
        account,
        data,
    )?;
    let coordinate_bytes = [coordinate.seed];
    let value = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_COLLATERAL_VAULT_POSTSTATE_DOMAIN_V2,
            &coordinate_bytes,
            &coordinate.compartment.to_le_bytes(),
            &account_state_id.bytes(),
            &rent_principal_lamports.to_le_bytes(),
            &swept_prefund_donation_lamports.to_le_bytes(),
            &principal_atoms.to_le_bytes(),
            &donation_atoms.to_le_bytes(),
            &amount_atoms_after.to_le_bytes(),
            &transfer_poststate_id.bytes(),
        ])
        .to_bytes(),
    );
    require(!value.is_zero(), ClutchError::MismatchedState)?;
    Ok(value)
}

fn series_collateral_binding_v2(
    program_id: &Pubkey,
    bound: BoundRealmCollateralV2,
    series: SeriesPlanV5Id,
    coordinate: SeriesCollateralVaultCoordinateV2,
    vault: &AccountInfo<'_>,
    authority: &AccountInfo<'_>,
) -> Outcome<CustodyBindingV2> {
    series
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_series_collateral_authority(program_id, series, authority)?;
    expect_pda(
        vault.key,
        seeds::series_collateral_vault_pda(program_id, &series.bytes(), coordinate.seed),
        None,
    )?;
    require(vault.is_writable, ClutchError::NotWritable)?;
    require(!vault.executable, ClutchError::ExecutableAccount)?;
    let binding = CustodyBindingV2 {
        account: collateral_id(vault.key),
        owner_authority: collateral_id(authority.key),
        semantic_owner: CollateralId::from_bytes(series.bytes()),
        compartment: coordinate.compartment,
        owner_guard: bound.release().owner_guard,
        owner_authority_is_program_derived: true,
    };
    binding
        .validate(bound.release())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(binding)
}

struct CreatedSeriesCollateralVaultV2 {
    binding: CustodyBindingV2,
    data_length: u64,
    rent_principal_lamports: u64,
    swept_prefund_donation_lamports: u64,
}

fn require_series_collateral_vault_rent_poststate_v2(
    rent: &RentParameters,
    expected_data_len: usize,
    actual_data_len: usize,
    observed_lamports: u64,
) -> Outcome<u64> {
    require(
        actual_data_len == expected_data_len,
        ClutchError::WrongDataLength,
    )?;
    let principal = rent.minimum_balance(actual_data_len)?;
    require(
        principal != 0 && observed_lamports == principal,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    Ok(principal)
}

#[allow(clippy::too_many_arguments)]
fn create_series_collateral_vault_v2<'a>(
    program_id: &Pubkey,
    bound: BoundRealmCollateralV2,
    series: SeriesPlanV5Id,
    coordinate: SeriesCollateralVaultCoordinateV2,
    payer: &AccountInfo<'a>,
    vault: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    rent: &RentParameters,
) -> Outcome<CreatedSeriesCollateralVaultV2> {
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require(neutral_sink.is_writable, ClutchError::NotWritable)?;
    require(!neutral_sink.executable, ClutchError::ExecutableAccount)?;
    require_creatable(vault)?;
    require_system_program(system_program)?;
    require_collateral_program(token_program, bound)?;
    let binding =
        series_collateral_binding_v2(program_id, bound, series, coordinate, vault, authority)?;
    let plan = prepare_custody_creation_v2(bound, binding)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_custody_creation_plan(plan, token_program, vault, mint, authority)?;
    require(
        payer.key != vault.key
            && payer.key != neutral_sink.key
            && vault.key != neutral_sink.key
            && vault.key != mint.key
            && vault.key != authority.key
            && vault.key != token_program.key,
        ClutchError::AccountAlias,
    )?;

    let component_seed = [coordinate.seed];
    let series_seed = series.bytes();
    let (_, bump) =
        seeds::series_collateral_vault_pda(program_id, &series_seed, coordinate.seed);
    let bump_seed = [bump];
    let signer_seeds: &[&[u8]] = &[
        seeds::SEED_SERIES_COLLATERAL_VAULT_V1,
        &series_seed,
        &component_seed,
        &bump_seed,
    ];

    let prefund = vault.lamports();
    if prefund != 0 {
        let sink_before = neutral_sink.lamports();
        let sweep = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &transfer_data(prefund),
            vec![
                AccountMeta::new(*vault.key, true),
                AccountMeta::new(*neutral_sink.key, false),
            ],
        );
        invoke_signed(
            &sweep,
            &[vault.clone(), neutral_sink.clone(), system_program.clone()],
            &[signer_seeds],
        )
        .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
        require(
            vault.lamports() == 0 && neutral_sink.lamports() == add(sink_before, prefund)?,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
    }

    let account_bytes = usize::from(plan.account_bytes);
    let rent_principal_lamports = rent.minimum_balance(account_bytes)?;
    require(rent_principal_lamports != 0, ClutchError::MismatchedState)?;
    let payer_before = payer.lamports();
    let fund = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(rent_principal_lamports),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*vault.key, false),
        ],
    );
    invoke(
        &fund,
        &[payer.clone(), vault.clone(), system_program.clone()],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    require(
        payer.lamports() == sub(payer_before, rent_principal_lamports)?
            && vault.lamports() == rent_principal_lamports,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;

    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(account_bytes),
        vec![AccountMeta::new(*vault.key, true)],
    );
    invoke_signed(
        &allocate,
        &[vault.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(token_program.key),
        vec![AccountMeta::new(*vault.key, true)],
    );
    invoke_signed(
        &assign,
        &[vault.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    require(
        vault.owner == token_program.key,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    require_series_collateral_vault_rent_poststate_v2(
        rent,
        account_bytes,
        vault.data_len(),
        vault.lamports(),
    )?;

    invoke_custody_initialization(plan, vault, mint, token_program)?;
    let vault_data = vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let observation = admit_realm_collateral_account_v2(
        bound,
        runtime_account_view(vault, &vault_data),
        TokenAccountRoleV2::SegregatedVault(binding),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        observation.amount_atoms == 0,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    require_series_collateral_vault_rent_poststate_v2(
        rent,
        account_bytes,
        vault_data.len(),
        vault.lamports(),
    )?;
    drop(vault_data);
    Ok(CreatedSeriesCollateralVaultV2 {
        binding,
        data_length: checked_data_len_v2(account_bytes)?,
        rent_principal_lamports,
        swept_prefund_donation_lamports: prefund,
    })
}

#[allow(clippy::too_many_arguments)]
fn series_collateral_activation_postwrite_receipt_id_v2(
    bound: BoundRealmCollateralV2,
    deployment: crate::collateral_release::AuthenticatedCollateralReleaseDeploymentV2,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: SeriesFundingTermsV2Id,
    compiler_bundle_id: CompiledProductSeriesBundleV5Id,
    funding_quote_id: clutch_product_series::SeriesFundingQuoteV4Id,
    attachment_plan_id: clutch_product_series::SeriesAttachmentPlanV4Id,
    funding_account: &Pubkey,
    payer: &Pubkey,
    payer_token_account: &Pubkey,
    payer_token_authority: &Pubkey,
    collateral_authority: &Pubkey,
    neutral_lamport_sink: &Pubkey,
    rent_sysvar: &Pubkey,
    rent: RentParameters,
    payer_lamports_before: u64,
    payer_lamports_after: u64,
    neutral_sink_lamports_before: u64,
    neutral_sink_lamports_after: u64,
    mint: &Pubkey,
    token_program: &Pubkey,
    funding_commitment_id: ContentId,
    source_prestate_id: ContentId,
    source_poststate_id: ContentId,
    vault_poststate_ids: &[ContentId; SERIES_COLLATERAL_VAULT_COUNT_V2],
    transfer_poststate_ids: &[ContentId; SERIES_COLLATERAL_VAULT_COUNT_V2],
) -> Outcome<ContentId> {
    let realm = bound.realm();
    let release_id = deployment.release_id();
    require(
        deployment.release() == bound.release()
            && release_id
                == bound
                    .release()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?,
        ClutchError::AuthorizationUnavailable,
    )?;
    let vault_poststates = flatten_series_collateral_ids_v2(vault_poststate_ids)?;
    let transfer_poststates = flatten_series_collateral_ids_v2(transfer_poststate_ids)?;
    let value = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_COLLATERAL_ACTIVATION_POSTWRITE_DOMAIN_V2,
            &series_plan_id.bytes(),
            &funding_terms_id.bytes(),
            &compiler_bundle_id.bytes(),
            &funding_quote_id.bytes(),
            &attachment_plan_id.bytes(),
            funding_account.as_ref(),
            &realm.realm.bytes(),
            &realm.profile.bytes(),
            &bound.policy_id().bytes(),
            &release_id.bytes(),
            &deployment.programdata_account().bytes(),
            &deployment.deployment_slot().to_le_bytes(),
            &deployment.receipt_id().bytes(),
            payer.as_ref(),
            payer_token_account.as_ref(),
            payer_token_authority.as_ref(),
            collateral_authority.as_ref(),
            neutral_lamport_sink.as_ref(),
            rent_sysvar.as_ref(),
            &rent.lamports_per_byte_year.to_le_bytes(),
            &rent.exemption_threshold.to_bits().to_le_bytes(),
            &payer_lamports_before.to_le_bytes(),
            &payer_lamports_after.to_le_bytes(),
            &neutral_sink_lamports_before.to_le_bytes(),
            &neutral_sink_lamports_after.to_le_bytes(),
            mint.as_ref(),
            token_program.as_ref(),
            &funding_commitment_id.bytes(),
            &source_prestate_id.bytes(),
            &source_poststate_id.bytes(),
            &vault_poststates,
            &transfer_poststates,
        ])
        .to_bytes(),
    );
    require(!value.is_zero(), ClutchError::MismatchedState)?;
    Ok(value)
}

/// Create, fund, and authenticate all five current Series collateral vaults.
///
/// This is deliberately crate-private and returns an opaque non-`Copy`
/// capability. The loader ProgramData and exact ELF digest are authenticated
/// before the first allocation or token CPI. Every nonzero transfer is
/// admitted, executed, reparsed, and delta-checked by Collateral Adapter V2;
/// every zero transfer is represented by an exactly zero destination, not a
/// synthetic success receipt.
#[allow(clippy::too_many_arguments)]
fn deploy_series_collateral_activation_v2<'a>(
    program_id: &Pubkey,
    registry: AuthenticatedSeriesRegistryAccountV2,
    artifacts: &AuthenticatedSeriesArtifactsV4,
    compiler_bundle: AuthenticatedCompiledProductSeriesBundleV5,
    realm_account: &AccountInfo<'a>,
    profile_account: &AccountInfo<'a>,
    policy_account: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    token_programdata: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    payer_token_account: &AccountInfo<'a>,
    payer_token_authority: &AccountInfo<'a>,
    collateral_authority: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    vaults: &[AccountInfo<'a>],
    principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    donations: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
) -> Outcome<AuthenticatedSeriesCollateralActivationPostwriteV2> {
    require(
        vaults.len() == SERIES_COLLATERAL_VAULT_COUNT_V2,
        ClutchError::AccountCount,
    )?;
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require_signer(payer_token_authority)?;
    require(
        payer_token_account.is_writable
            && !payer_token_account.is_signer
            && !payer_token_account.executable
            && !payer_token_authority.executable,
        ClutchError::MismatchedState,
    )?;
    require_system_program(system_program)?;
    let rent = read_rent(rent_sysvar)?;

    let series_plan_id = artifacts
        .series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terms_id = artifacts
        .funding_terms
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_quote_id = artifacts
        .quote
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attachment_plan_id = artifacts
        .attachment
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let registry_value = registry.value();
    require(
        !registry.activation_consumed()
            && registry_value.series_plan_id == series_plan_id
            && registry_value.funding_terms_id == funding_terms_id
            && registry_value.compiler_bundle_id == compiler_bundle.bundle_id()
            && compiler_bundle.bundle().series_plan_id == series_plan_id
            && compiler_bundle.bundle().funding_terms_id == funding_terms_id
            && artifacts.funding_terms.series_plan_id == series_plan_id
            && artifacts.funding_terms.collateral_mint.bytes() == mint.key.to_bytes()
            && artifacts.funding_terms.token_program.bytes() == token_program.key.to_bytes()
            && artifacts.funding_terms.lamport_principal_refund.bytes() == payer.key.to_bytes()
            && artifacts.funding_terms.neutral_lamport_sink.bytes()
                == neutral_lamport_sink.key.to_bytes()
            && principal[SeriesFundingComponentV2::SeriesAdmission.index()].collateral_atoms == 0
            && donations[SeriesFundingComponentV2::SeriesAdmission.index()].collateral_atoms == 0,
        ClutchError::MismatchedState,
    )?;
    require_system_lamport_destination(
        neutral_lamport_sink,
        artifacts.funding_terms.neutral_lamport_sink,
    )?;

    let bound = crate::collateral_release::authenticate_realm_collateral_v2(
        program_id,
        realm_account,
        profile_account,
        policy_account,
        token_program,
    )?;
    let realm = bound.realm();
    let policy = bound.policy();
    require(
        realm.realm == collateral_content_id(artifacts.genesis.realm_id)
            && realm.profile == collateral_content_id(artifacts.genesis.profile_id)
            && policy.mint == collateral_id(mint.key)
            && policy.token_program == collateral_id(token_program.key),
        ClutchError::MismatchedState,
    )?;
    let deployment = crate::collateral_release::authenticate_collateral_release_deployment_v2(
        bound.release(),
        token_program,
        token_programdata,
    )?;
    require(
        deployment.release() == bound.release()
            && deployment.release_id()
                == bound
                    .release()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?,
        ClutchError::AuthorizationUnavailable,
    )?;
    require_collateral_program(token_program, bound)?;
    require_series_collateral_authority(
        program_id,
        series_plan_id,
        collateral_authority,
    )?;

    let registry_account_key = registry.account();
    let (funding_account, _) =
        seeds::series_funding_pda(program_id, &series_plan_id.bytes());
    require(
        registry.account() != funding_account
            && payer_token_account.key != mint.key
            && payer_token_account.key != token_program.key
            && payer_token_account.key != token_programdata.key
            && payer_token_account.key != collateral_authority.key
            && payer_token_account.key != neutral_lamport_sink.key
            && payer_token_authority.key != payer_token_account.key
            && payer_token_authority.key != mint.key
            && payer_token_authority.key != token_program.key
            && payer_token_authority.key != token_programdata.key
            && payer_token_authority.key != collateral_authority.key,
        ClutchError::AccountAlias,
    )?;
    let mut left = 0usize;
    while left < SERIES_COLLATERAL_VAULT_COUNT_V2 {
        require(
            vaults[left].key != payer.key
                && vaults[left].key != payer_token_account.key
                && vaults[left].key != payer_token_authority.key
                && vaults[left].key != collateral_authority.key
                && vaults[left].key != mint.key
                && vaults[left].key != token_program.key
                && vaults[left].key != token_programdata.key
                && vaults[left].key != neutral_lamport_sink.key
                && vaults[left].key != system_program.key
                && vaults[left].key != rent_sysvar.key
                && vaults[left].key != realm_account.key
                && vaults[left].key != profile_account.key
                && vaults[left].key != policy_account.key
                && vaults[left].key != &registry_account_key
                && vaults[left].key != &funding_account,
            ClutchError::AccountAlias,
        )?;
        let mut right = left + 1;
        while right < SERIES_COLLATERAL_VAULT_COUNT_V2 {
            require(vaults[left].key != vaults[right].key, ClutchError::AccountAlias)?;
            right += 1;
        }
        left += 1;
    }

    let funding_join = SeriesCollateralFundingJoinV2 {
        realm: realm.realm,
        profile: realm.profile,
        series_plan: CollateralId::from_bytes(series_plan_id.bytes()),
        funding_terms: CollateralId::from_bytes(funding_terms_id.bytes()),
        funding_state_account: collateral_id(&funding_account),
        quote: CollateralId::from_bytes(funding_quote_id.bytes()),
        funding_authority: collateral_id(collateral_authority.key),
        collateral_principal_refund_token_account: collateral_content_id(
            artifacts
                .funding_terms
                .collateral_principal_refund_token_account,
        ),
        neutral_collateral_disposition_token_account: collateral_content_id(
            artifacts
                .funding_terms
                .neutral_collateral_disposition_token_account,
        ),
        payer_lamport_refund: collateral_content_id(
            artifacts.funding_terms.lamport_principal_refund,
        ),
        neutral_lamport_sink: collateral_id(neutral_lamport_sink.key),
    };
    funding_join
        .validate(bound)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let transfer_authority = TransferAuthorityV2 {
        address: collateral_id(payer_token_authority.key),
        kind: TransferAuthorityKindV2::TransactionSigner,
        is_transaction_signer: payer_token_authority.is_signer,
        program_address_authenticated: false,
        is_writable: payer_token_authority.is_writable,
        executable: payer_token_authority.executable,
        data_is_empty: payer_token_authority.data_is_empty(),
    };
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mint_before = admit_realm_collateral_mint_v2(bound, runtime_account_view(mint, &mint_data))
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(mint_data);
    let source_data = payer_token_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let source_before = admit_realm_collateral_account_v2(
        bound,
        runtime_account_view(payer_token_account, &source_data),
        TokenAccountRoleV2::Holder {
            owner: collateral_id(payer_token_authority.key),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let source_prestate_id = series_collateral_account_state_id_v2(
        SERIES_COLLATERAL_SOURCE_PRESTATE_DOMAIN_V2,
        payer_token_account,
        &source_data,
    )?;
    drop(source_data);

    let mut total_collateral_atoms = 0u64;
    let payer_lamports_before = payer.lamports();
    let neutral_sink_lamports_before = neutral_lamport_sink.lamports();
    let mut index = 0usize;
    while index < SERIES_COLLATERAL_VAULT_COUNT_V2 {
        let coordinate = series_collateral_vault_coordinate_v2(index)?;
        let component_index = coordinate.component.index();
        total_collateral_atoms = add(
            total_collateral_atoms,
            add(
                principal[component_index].collateral_atoms,
                donations[component_index].collateral_atoms,
            )?,
        )?;
        index += 1;
    }
    require(
        source_before.amount_atoms >= total_collateral_atoms,
        ClutchError::MismatchedState,
    )?;

    let mut vault_accounts =
        [Pubkey::new_from_array([0u8; 32]); SERIES_COLLATERAL_VAULT_COUNT_V2];
    let mut vault_data_lengths = [0u64; SERIES_COLLATERAL_VAULT_COUNT_V2];
    let mut vault_rent_principal_lamports = [0u64; SERIES_COLLATERAL_VAULT_COUNT_V2];
    let mut swept_prefund_donation_lamports =
        [0u64; SERIES_COLLATERAL_VAULT_COUNT_V2];
    let mut vault_collateral_principal_atoms = [0u64; SERIES_COLLATERAL_VAULT_COUNT_V2];
    let mut vault_collateral_donation_atoms = [0u64; SERIES_COLLATERAL_VAULT_COUNT_V2];
    let mut vault_collateral_atoms_after = [0u64; SERIES_COLLATERAL_VAULT_COUNT_V2];
    let mut vault_poststate_ids = [ContentId::ZERO; SERIES_COLLATERAL_VAULT_COUNT_V2];
    let mut transfer_poststate_ids = [ContentId::ZERO; SERIES_COLLATERAL_VAULT_COUNT_V2];

    index = 0;
    while index < SERIES_COLLATERAL_VAULT_COUNT_V2 {
        let coordinate = series_collateral_vault_coordinate_v2(index)?;
        let component_index = coordinate.component.index();
        let created = create_series_collateral_vault_v2(
            program_id,
            bound,
            series_plan_id,
            coordinate,
            payer,
            &vaults[index],
            collateral_authority,
            mint,
            neutral_lamport_sink,
            system_program,
            token_program,
            &rent,
        )?;
        let principal_atoms = principal[component_index].collateral_atoms;
        let donation_atoms = donations[component_index].collateral_atoms;
        let amount_atoms = add(principal_atoms, donation_atoms)?;
        if amount_atoms != 0 {
            let request = series_segregated_funding_request_v2(
                bound,
                funding_join,
                coordinate.compartment,
                created.binding,
                collateral_id(payer_token_authority.key),
                transfer_authority,
                amount_atoms,
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            let mint_data = mint
                .try_borrow_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            let source_data = payer_token_account
                .try_borrow_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            let destination_data = vaults[index]
                .try_borrow_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            let prepared = prepare_realm_collateral_transfer_v2(
                bound,
                request,
                runtime_account_view(mint, &mint_data),
                runtime_account_view(payer_token_account, &source_data),
                runtime_account_view(&vaults[index], &destination_data),
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            drop(mint_data);
            drop(source_data);
            drop(destination_data);
            let accepted = invoke_series_collateral_transfer(
                prepared,
                mint,
                payer_token_account,
                &vaults[index],
                payer_token_authority,
                token_program,
                None,
            )?;
            require(
                accepted.kind == CustodyTransferKindV2::SegregatedFunding
                    && accepted.amount_atoms == amount_atoms
                    && accepted.destination_semantic_owner
                        == CollateralId::from_bytes(series_plan_id.bytes())
                    && accepted.destination_compartment == coordinate.compartment
                    && accepted.destination_atoms_after == amount_atoms
                    && accepted.mint_supply_after == mint_before.supply_atoms,
                ClutchError::SeriesCustodyDeltaMismatch,
            )?;
            transfer_poststate_ids[index] =
                series_collateral_transfer_poststate_id_v2(accepted)?;
        }

        let vault_data = vaults[index]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let observation = admit_realm_collateral_account_v2(
            bound,
            runtime_account_view(&vaults[index], &vault_data),
            TokenAccountRoleV2::SegregatedVault(created.binding),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            observation.amount_atoms == amount_atoms
                && vaults[index].lamports() == created.rent_principal_lamports
                && checked_data_len_v2(vault_data.len())? == created.data_length,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
        vault_poststate_ids[index] = series_collateral_vault_poststate_id_v2(
            coordinate,
            &vaults[index],
            &vault_data,
            created.rent_principal_lamports,
            created.swept_prefund_donation_lamports,
            principal_atoms,
            donation_atoms,
            observation.amount_atoms,
            transfer_poststate_ids[index],
        )?;
        drop(vault_data);
        vault_accounts[index] = *vaults[index].key;
        vault_data_lengths[index] = created.data_length;
        vault_rent_principal_lamports[index] = created.rent_principal_lamports;
        swept_prefund_donation_lamports[index] = created.swept_prefund_donation_lamports;
        vault_collateral_principal_atoms[index] = principal_atoms;
        vault_collateral_donation_atoms[index] = donation_atoms;
        vault_collateral_atoms_after[index] = amount_atoms;
        index += 1;
    }

    let source_data = payer_token_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let source_after = admit_realm_collateral_account_v2(
        bound,
        runtime_account_view(payer_token_account, &source_data),
        TokenAccountRoleV2::Holder {
            owner: collateral_id(payer_token_authority.key),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        source_after.amount_atoms
            == sub(source_before.amount_atoms, total_collateral_atoms)?,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let source_poststate_id = series_collateral_account_state_id_v2(
        SERIES_COLLATERAL_SOURCE_POSTSTATE_DOMAIN_V2,
        payer_token_account,
        &source_data,
    )?;
    drop(source_data);
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mint_after = admit_realm_collateral_mint_v2(bound, runtime_account_view(mint, &mint_data))
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        mint_after.supply_atoms == mint_before.supply_atoms,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    drop(mint_data);

    let mut total_vault_rent_principal = 0u64;
    let mut total_swept_prefund_donation = 0u64;
    index = 0;
    while index < SERIES_COLLATERAL_VAULT_COUNT_V2 {
        total_vault_rent_principal = add(
            total_vault_rent_principal,
            vault_rent_principal_lamports[index],
        )?;
        total_swept_prefund_donation = add(
            total_swept_prefund_donation,
            swept_prefund_donation_lamports[index],
        )?;
        index += 1;
    }
    let payer_lamports_after = payer.lamports();
    let neutral_sink_lamports_after = neutral_lamport_sink.lamports();
    require(
        payer_lamports_after == sub(payer_lamports_before, total_vault_rent_principal)?
            && neutral_sink_lamports_after
                == add(
                    neutral_sink_lamports_before,
                    total_swept_prefund_donation,
                )?,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;

    let funding_commitment_id = series_collateral_funding_commitment_v2(&principal, &donations)?;
    let receipt_id = series_collateral_activation_postwrite_receipt_id_v2(
        bound,
        deployment,
        series_plan_id,
        funding_terms_id,
        compiler_bundle.bundle_id(),
        funding_quote_id,
        attachment_plan_id,
        &funding_account,
        payer.key,
        payer_token_account.key,
        payer_token_authority.key,
        collateral_authority.key,
        neutral_lamport_sink.key,
        rent_sysvar.key,
        rent,
        payer_lamports_before,
        payer_lamports_after,
        neutral_sink_lamports_before,
        neutral_sink_lamports_after,
        mint.key,
        token_program.key,
        funding_commitment_id,
        source_prestate_id,
        source_poststate_id,
        &vault_poststate_ids,
        &transfer_poststate_ids,
    )?;
    let vault_rent = AuthenticatedSeriesCollateralVaultRentV2 {
        series_plan_id,
        principal_lamports: vault_rent_principal_lamports,
        authentication_id: receipt_id,
    };
    Ok(AuthenticatedSeriesCollateralActivationPostwriteV2 {
        bound,
        deployment,
        series_plan_id,
        funding_terms_id,
        compiler_bundle_id: compiler_bundle.bundle_id(),
        funding_quote_id,
        attachment_plan_id,
        funding_account,
        payer: *payer.key,
        payer_token_account: *payer_token_account.key,
        payer_token_authority: *payer_token_authority.key,
        collateral_authority: *collateral_authority.key,
        neutral_lamport_sink: *neutral_lamport_sink.key,
        rent_sysvar: *rent_sysvar.key,
        rent,
        payer_lamports_before,
        payer_lamports_after,
        neutral_sink_lamports_before,
        neutral_sink_lamports_after,
        principal,
        donations,
        vault_accounts,
        vault_data_lengths,
        vault_rent_principal_lamports,
        swept_prefund_donation_lamports,
        vault_collateral_principal_atoms,
        vault_collateral_donation_atoms,
        vault_collateral_atoms_after,
        vault_poststate_ids,
        transfer_poststate_ids,
        source_prestate_id,
        source_poststate_id,
        vault_rent,
        receipt_id,
    })
}

fn require_distinct_series_collateral_graph(accounts: &[&AccountInfo<'_>]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
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

/// Authenticate the complete immutable Series collateral funding graph.
///
/// The collateral refund and neutral disposition accounts are admitted only
/// as receive-only release-selected token accounts for the Realm mint. The
/// lamport refund and neutral sink are admitted only as distinct writable,
/// zero-data System accounts. This prevents rent lamports from reaching a
/// token sink and prevents collateral principal from reaching a lamport sink.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn authenticate_series_collateral_funding(
    program_id: &Pubkey,
    bound: BoundRealmCollateralV2,
    artifacts: &AuthenticatedSeriesArtifactsV1,
    registry: AuthenticatedSeriesRegistryAccountV1,
    funding: AuthenticatedSeriesFundingAccountV1,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    authority: &AccountInfo<'_>,
    collateral_principal_refund: &AccountInfo<'_>,
    neutral_collateral_disposition: &AccountInfo<'_>,
    lamport_principal_refund: &AccountInfo<'_>,
    neutral_lamport_sink: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSeriesCollateralFundingV1> {
    let series_plan_id = artifacts
        .series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terms_id = artifacts
        .funding_terms
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_quote_id = artifacts
        .quote
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let registry_value = registry.value();
    let funding_value = funding.value();
    let realm = bound.realm();
    let policy = bound.policy();
    require(
        registry_value.series_plan_id == series_plan_id
            && registry_value.funding_terms_id == funding_terms_id
            && funding_value.state.series_plan_id == series_plan_id
            && funding_value.state.funding_terms_id == funding_terms_id
            && funding_value.state.funding_quote_id == funding_quote_id
            && realm.realm == collateral_content_id(artifacts.genesis.realm_id)
            && realm.profile == collateral_content_id(artifacts.genesis.profile_id)
            && policy.mint == collateral_content_id(artifacts.funding_terms.collateral_mint)
            && policy.token_program == collateral_content_id(artifacts.funding_terms.token_program)
            && collateral_principal_refund.key.to_bytes()
                == artifacts
                    .funding_terms
                    .collateral_principal_refund_token_account
                    .bytes()
            && neutral_collateral_disposition.key.to_bytes()
                == artifacts
                    .funding_terms
                    .neutral_collateral_disposition_token_account
                    .bytes(),
        ClutchError::MismatchedState,
    )?;
    require_collateral_program(token_program, bound)?;
    require_series_collateral_authority(program_id, series_plan_id, authority)?;
    require(
        collateral_id(mint.key) == policy.mint
            && !mint.is_writable
            && !mint.is_signer
            && !mint.executable,
        ClutchError::MismatchedState,
    )?;
    require(
        collateral_principal_refund.is_writable && neutral_collateral_disposition.is_writable,
        ClutchError::NotWritable,
    )?;
    require_system_lamport_destination(
        lamport_principal_refund,
        artifacts.funding_terms.lamport_principal_refund,
    )?;
    require_system_lamport_destination(
        neutral_lamport_sink,
        artifacts.funding_terms.neutral_lamport_sink,
    )?;
    require_distinct_series_collateral_graph(&[
        mint,
        token_program,
        authority,
        collateral_principal_refund,
        neutral_collateral_disposition,
        lamport_principal_refund,
        neutral_lamport_sink,
    ])?;
    require(
        registry.account() != funding.account()
            && registry.account() != *mint.key
            && registry.account() != *token_program.key
            && registry.account() != *authority.key
            && registry.account() != *collateral_principal_refund.key
            && registry.account() != *neutral_collateral_disposition.key
            && registry.account() != *lamport_principal_refund.key
            && registry.account() != *neutral_lamport_sink.key
            && funding.account() != *mint.key
            && funding.account() != *token_program.key
            && funding.account() != *authority.key
            && funding.account() != *collateral_principal_refund.key
            && funding.account() != *neutral_collateral_disposition.key
            && funding.account() != *lamport_principal_refund.key
            && funding.account() != *neutral_lamport_sink.key,
        ClutchError::AccountAlias,
    )?;

    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    admit_realm_collateral_mint_v2(bound, runtime_account_view(mint, &mint_data))
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let collateral_refund_data = collateral_principal_refund
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    admit_realm_collateral_account_v2(
        bound,
        runtime_account_view(collateral_principal_refund, &collateral_refund_data),
        TokenAccountRoleV2::ReceiveOnly {
            account: collateral_id(collateral_principal_refund.key),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let neutral_collateral_data = neutral_collateral_disposition
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    admit_realm_collateral_account_v2(
        bound,
        runtime_account_view(neutral_collateral_disposition, &neutral_collateral_data),
        TokenAccountRoleV2::ReceiveOnly {
            account: collateral_id(neutral_collateral_disposition.key),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let transfer_authority = TransferAuthorityV2 {
        address: collateral_id(authority.key),
        kind: TransferAuthorityKindV2::ProgramDerived,
        is_transaction_signer: authority.is_signer,
        program_address_authenticated: true,
        is_writable: authority.is_writable,
        executable: authority.executable,
        data_is_empty: authority.data_is_empty(),
    };
    let join = SeriesCollateralFundingJoinV2 {
        realm: realm.realm,
        profile: realm.profile,
        series_plan: CollateralId::from_bytes(series_plan_id.bytes()),
        funding_terms: CollateralId::from_bytes(funding_terms_id.bytes()),
        funding_state_account: collateral_id(&funding.account()),
        quote: CollateralId::from_bytes(funding_quote_id.bytes()),
        funding_authority: transfer_authority.address,
        collateral_principal_refund_token_account: collateral_id(collateral_principal_refund.key),
        neutral_collateral_disposition_token_account: collateral_id(
            neutral_collateral_disposition.key,
        ),
        payer_lamport_refund: collateral_id(lamport_principal_refund.key),
        neutral_lamport_sink: collateral_id(neutral_lamport_sink.key),
    };
    join.validate(bound)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(AuthenticatedSeriesCollateralFundingV1 {
        bound,
        join,
        authority: transfer_authority,
        funding_account: funding,
    })
}

/// Refine an authenticated collateral funding graph with the one-shot Series
/// terminal receipt and its exact state-derived destinations.
#[cfg(test)]
pub fn authenticate_series_collateral_terminal(
    funding: AuthenticatedSeriesCollateralFundingV1,
    terminal: AuthenticatedSeriesTerminalV1,
) -> Outcome<AuthenticatedSeriesCollateralTerminalV1> {
    let projection = terminal.projection();
    let funding_join = funding.join();
    require(
        terminal.activation_generation() == SERIES_ACTIVATION_GENERATION_V1
            && CollateralId::from_bytes(terminal.series_plan_id().bytes())
                == funding_join.series_plan
            && CollateralId::from_bytes(terminal.funding_terms_id().bytes())
                == funding_join.funding_terms
            && CollateralId::from_bytes(terminal.funding_quote_id().bytes()) == funding_join.quote
            && collateral_id(&terminal.funding_account()) == funding_join.funding_state_account
            && collateral_content_id(projection.lamport_principal_refund)
                == funding_join.payer_lamport_refund
            && collateral_content_id(projection.collateral_principal_refund_token_account)
                == funding_join.collateral_principal_refund_token_account
            && collateral_content_id(projection.neutral_collateral_disposition_token_account)
                == funding_join.neutral_collateral_disposition_token_account
            && collateral_content_id(projection.neutral_lamport_sink)
                == funding_join.neutral_lamport_sink,
        ClutchError::MismatchedState,
    )?;
    let join = SeriesCollateralTerminalJoinV2 {
        funding: funding_join,
        terminal_receipt: CollateralId::from_bytes(terminal.id().bytes()),
    };
    join.validate(funding.bound())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(AuthenticatedSeriesCollateralTerminalV1 {
        funding,
        join,
        projection,
    })
}

fn cpi_account_meta(value: CpiAccountMetaV2) -> AccountMeta {
    AccountMeta {
        pubkey: Pubkey::new_from_array(value.address.bytes()),
        is_signer: value.signer,
        is_writable: value.writable,
    }
}

#[allow(clippy::too_many_arguments)]
fn invoke_series_collateral_transfer<'a>(
    prepared: PreparedCollateralTransferV2,
    mint: &AccountInfo<'a>,
    source: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    signer_seeds: Option<&[&[u8]]>,
) -> Outcome<AcceptedCollateralTransferV2> {
    let cpi = prepared.cpi();
    require(
        cpi.token_program == collateral_id(token_program.key)
            && cpi.accounts[0].address == collateral_id(source.key)
            && cpi.accounts[1].address == collateral_id(mint.key)
            && cpi.accounts[2].address == collateral_id(destination.key)
            && cpi.accounts[3].address == collateral_id(authority.key)
            && cpi.program_signed == signer_seeds.is_some(),
        ClutchError::MismatchedState,
    )?;
    let instruction = Instruction::new_with_bytes(
        *token_program.key,
        &cpi.data,
        cpi.accounts.into_iter().map(cpi_account_meta).collect(),
    );
    let account_infos = [
        source.clone(),
        mint.clone(),
        destination.clone(),
        authority.clone(),
        token_program.clone(),
    ];
    match signer_seeds {
        Some(seeds) => invoke_signed(&instruction, &account_infos, &[seeds]),
        None => invoke(&instruction, &account_infos),
    }
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;

    let mint_after = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let source_after = source
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let destination_after = destination
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    accept_collateral_transfer_v2(
        prepared,
        runtime_account_view(mint, &mint_after),
        runtime_account_view(source, &source_after),
        runtime_account_view(destination, &destination_after),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))
}

/// Fund one Series collateral compartment by its exact authenticated
/// principal-plus-donation balance and postcheck the raw-atom delta.
///
/// A zero collateral requirement emits no token invocation. The caller must
/// still authenticate the zero-balance vault through
/// [`authenticate_series_collateral_custody`] before activation commits.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn fund_series_collateral_component<'a>(
    program_id: &Pubkey,
    authenticated: AuthenticatedSeriesCollateralFundingV1,
    component: SeriesFundingComponentV1,
    mint: &AccountInfo<'a>,
    payer_token_account: &AccountInfo<'a>,
    vault: &AccountInfo<'a>,
    payer_token_authority: &AccountInfo<'a>,
    canonical_authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
) -> Outcome<Option<AcceptedCollateralTransferV2>> {
    require_collateral_program(token_program, authenticated.bound())?;
    require_series_collateral_authority(
        program_id,
        SeriesPlanV5Id::from_bytes(authenticated.join().series_plan.bytes()),
        canonical_authority,
    )?;
    let state = authenticated.funding_account().value().state;
    require(
        CollateralId::from_bytes(state.series_plan_id.bytes()) == authenticated.join().series_plan
            && collateral_id(canonical_authority.key) == authenticated.join().funding_authority,
        ClutchError::MismatchedState,
    )?;
    let capital = state.components[component.index()];
    let amount_atoms = add(
        capital.remaining_principal.collateral_atoms,
        capital.donations.collateral_atoms,
    )?;
    if amount_atoms == 0 {
        return Ok(None);
    }
    let authority = TransferAuthorityV2 {
        address: collateral_id(payer_token_authority.key),
        kind: TransferAuthorityKindV2::TransactionSigner,
        is_transaction_signer: payer_token_authority.is_signer,
        program_address_authenticated: false,
        is_writable: payer_token_authority.is_writable,
        executable: payer_token_authority.executable,
        data_is_empty: payer_token_authority.data_is_empty(),
    };
    let binding = series_collateral_binding(
        program_id,
        authenticated.bound(),
        state.series_plan_id,
        component,
        vault,
        canonical_authority,
    )?;
    let request = series_segregated_funding_request_v2(
        authenticated.bound(),
        authenticated.join(),
        u16::from(component.byte()) + 1,
        binding,
        collateral_id(payer_token_authority.key),
        authority,
        amount_atoms,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let source_data = payer_token_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let destination_data = vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let prepared = prepare_realm_collateral_transfer_v2(
        authenticated.bound(),
        request,
        runtime_account_view(mint, &mint_data),
        runtime_account_view(payer_token_account, &source_data),
        runtime_account_view(vault, &destination_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(mint_data);
    drop(source_data);
    drop(destination_data);
    invoke_series_collateral_transfer(
        prepared,
        mint,
        payer_token_account,
        vault,
        payer_token_authority,
        token_program,
        None,
    )
    .map(Some)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SeriesCollateralTerminalMovementV1 {
    PrincipalRefund,
    DonationDisposition,
}

#[allow(clippy::too_many_arguments)]
fn transfer_series_terminal_collateral<'a>(
    program_id: &Pubkey,
    authenticated: AuthenticatedSeriesCollateralTerminalV1,
    component: SeriesFundingComponentV1,
    movement: SeriesCollateralTerminalMovementV1,
    mint: &AccountInfo<'a>,
    vault: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
) -> Outcome<Option<AcceptedCollateralTransferV2>> {
    let funding = authenticated.funding();
    require_collateral_program(token_program, funding.bound())?;
    let series = SeriesPlanV5Id::from_bytes(funding.join().series_plan.bytes());
    require_series_collateral_authority(program_id, series, authority)?;
    require(
        collateral_id(authority.key) == funding.join().funding_authority,
        ClutchError::MismatchedState,
    )?;
    let index = component.index();
    let projection = authenticated.projection();
    let amount_atoms = match movement {
        SeriesCollateralTerminalMovementV1::PrincipalRefund => {
            projection.refundable_principal[index].collateral_atoms
        }
        SeriesCollateralTerminalMovementV1::DonationDisposition => {
            projection.donation_residue[index].collateral_atoms
        }
    };
    if amount_atoms == 0 {
        return Ok(None);
    }
    let binding = series_collateral_binding(
        program_id,
        funding.bound(),
        series,
        component,
        vault,
        authority,
    )?;
    let request = match movement {
        SeriesCollateralTerminalMovementV1::PrincipalRefund => series_principal_refund_request_v2(
            funding.bound(),
            authenticated.join(),
            u16::from(component.byte()) + 1,
            binding,
            funding.authority(),
            amount_atoms,
        ),
        SeriesCollateralTerminalMovementV1::DonationDisposition => {
            series_donation_disposition_request_v2(
                funding.bound(),
                authenticated.join(),
                u16::from(component.byte()) + 1,
                binding,
                funding.authority(),
                amount_atoms,
            )
        }
    }
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let source_data = vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let destination_data = destination
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let prepared = prepare_realm_collateral_transfer_v2(
        funding.bound(),
        request,
        runtime_account_view(mint, &mint_data),
        runtime_account_view(vault, &source_data),
        runtime_account_view(destination, &destination_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(mint_data);
    drop(source_data);
    drop(destination_data);

    let series_seed = series.bytes();
    let (_, bump) = seeds::series_collateral_authority_pda(program_id, &series_seed);
    let bump_seed = [bump];
    let signer_seeds: &[&[u8]] = &[
        seeds::SEED_SERIES_COLLATERAL_AUTHORITY_V1,
        &series_seed,
        &bump_seed,
    ];
    invoke_series_collateral_transfer(
        prepared,
        mint,
        vault,
        destination,
        authority,
        token_program,
        Some(signer_seeds),
    )
    .map(Some)
}

/// Refund one component's exact unspent collateral principal to the immutable
/// FundingTerms V2 token account and postcheck both balances and mint supply.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn refund_series_collateral_principal<'a>(
    program_id: &Pubkey,
    authenticated: AuthenticatedSeriesCollateralTerminalV1,
    component: SeriesFundingComponentV1,
    mint: &AccountInfo<'a>,
    vault: &AccountInfo<'a>,
    collateral_principal_refund: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
) -> Outcome<Option<AcceptedCollateralTransferV2>> {
    transfer_series_terminal_collateral(
        program_id,
        authenticated,
        component,
        SeriesCollateralTerminalMovementV1::PrincipalRefund,
        mint,
        vault,
        collateral_principal_refund,
        authority,
        token_program,
    )
}

/// Dispose one component's exact collateral donation residue to the immutable
/// receive-only neutral token account and postcheck the exact raw-atom delta.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn dispose_series_collateral_donation<'a>(
    program_id: &Pubkey,
    authenticated: AuthenticatedSeriesCollateralTerminalV1,
    component: SeriesFundingComponentV1,
    mint: &AccountInfo<'a>,
    vault: &AccountInfo<'a>,
    neutral_collateral_disposition: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
) -> Outcome<Option<AcceptedCollateralTransferV2>> {
    transfer_series_terminal_collateral(
        program_id,
        authenticated,
        component,
        SeriesCollateralTerminalMovementV1::DonationDisposition,
        mint,
        vault,
        neutral_collateral_disposition,
        authority,
        token_program,
    )
}

/// Close one empty Series collateral vault, refund only its stored payer rent
/// principal, send only close surplus to the System-owned neutral lamport sink,
/// and restore the component lamport vault's pre-close funding balance.
///
/// Principal and donation collateral transfers must have completed first; a
/// nonempty token vault refuses before the external close invocation.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn close_series_collateral_vault<'a>(
    program_id: &Pubkey,
    authenticated: AuthenticatedSeriesCollateralTerminalV1,
    component: SeriesFundingComponentV1,
    vault: &AccountInfo<'a>,
    component_lamport_vault: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    payer_lamport_refund: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
) -> Outcome<AcceptedSeriesVaultRentDispositionV2> {
    let funding = authenticated.funding();
    let series = SeriesPlanV5Id::from_bytes(funding.join().series_plan.bytes());
    require_collateral_program(token_program, funding.bound())?;
    require_system_program(system_program)?;
    require_series_collateral_authority(program_id, series, authority)?;
    require(
        collateral_id(authority.key) == funding.join().funding_authority
            && collateral_id(payer_lamport_refund.key) == funding.join().payer_lamport_refund
            && collateral_id(neutral_lamport_sink.key) == funding.join().neutral_lamport_sink,
        ClutchError::MismatchedState,
    )?;
    require_lamport_vault_metadata(
        program_id,
        series,
        component.index(),
        component_lamport_vault,
    )?;
    let binding = series_collateral_binding(
        program_id,
        funding.bound(),
        series,
        component,
        vault,
        authority,
    )?;
    let stored_rent_principal = funding
        .funding_account()
        .value()
        .collateral_vault_rent_principal_lamports[component.index()];
    let request = SeriesCollateralVaultCloseRequestV2 {
        terminal: authenticated.join(),
        component: u16::from(component.byte()) + 1,
        vault: binding,
        component_lamport_vault: collateral_id(component_lamport_vault.key),
        stored_vault_rent_principal_lamports: stored_rent_principal,
        authority: funding.authority(),
    };
    let vault_before_data = vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let component_before_data = component_lamport_vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let prepared_close = prepare_series_collateral_vault_close_v2(
        funding.bound(),
        request,
        runtime_lamport_account_view(vault, &vault_before_data),
        runtime_lamport_account_view(component_lamport_vault, &component_before_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(vault_before_data);
    drop(component_before_data);

    let cpi = prepared_close.cpi();
    require(
        cpi.token_program == collateral_id(token_program.key)
            && cpi.accounts[0].address == collateral_id(vault.key)
            && cpi.accounts[1].address == collateral_id(component_lamport_vault.key)
            && cpi.accounts[2].address == collateral_id(authority.key)
            && cpi.program_signed,
        ClutchError::MismatchedState,
    )?;
    let instruction = Instruction::new_with_bytes(
        *token_program.key,
        &cpi.data,
        cpi.accounts.into_iter().map(cpi_account_meta).collect(),
    );
    let series_seed = series.bytes();
    let (_, authority_bump) = seeds::series_collateral_authority_pda(program_id, &series_seed);
    let authority_bump_seed = [authority_bump];
    invoke_signed(
        &instruction,
        &[
            vault.clone(),
            component_lamport_vault.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        &[&[
            seeds::SEED_SERIES_COLLATERAL_AUTHORITY_V1,
            &series_seed,
            &authority_bump_seed,
        ]],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;

    let vault_after_data = vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let component_after_close_data = component_lamport_vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let closed = accept_series_collateral_vault_close_v2(
        prepared_close,
        runtime_lamport_account_view(vault, &vault_after_data),
        runtime_lamport_account_view(component_lamport_vault, &component_after_close_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    drop(vault_after_data);
    drop(component_after_close_data);

    let component_before_split_data = component_lamport_vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let refund_before_data = payer_lamport_refund
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let sink_before_data = neutral_lamport_sink
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let prepared_disposition = prepare_series_vault_rent_disposition_v2(
        closed,
        runtime_lamport_account_view(component_lamport_vault, &component_before_split_data),
        runtime_lamport_account_view(payer_lamport_refund, &refund_before_data),
        runtime_lamport_account_view(neutral_lamport_sink, &sink_before_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(component_before_split_data);
    drop(refund_before_data);
    drop(sink_before_data);

    let credits = prepared_disposition.credits();
    require(
        credits[0].destination == collateral_id(payer_lamport_refund.key)
            && credits[1].destination == collateral_id(neutral_lamport_sink.key),
        ClutchError::MismatchedState,
    )?;
    transfer_from_lamport_custody(
        program_id,
        series,
        component,
        component_lamport_vault,
        payer_lamport_refund,
        system_program,
        credits[0].lamports,
    )?;
    transfer_from_lamport_custody(
        program_id,
        series,
        component,
        component_lamport_vault,
        neutral_lamport_sink,
        system_program,
        credits[1].lamports,
    )?;

    let component_final_data = component_lamport_vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let refund_final_data = payer_lamport_refund
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let sink_final_data = neutral_lamport_sink
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    accept_series_vault_rent_disposition_v2(
        prepared_disposition,
        runtime_lamport_account_view(component_lamport_vault, &component_final_data),
        runtime_lamport_account_view(payer_lamport_refund, &refund_final_data),
        runtime_lamport_account_view(neutral_lamport_sink, &sink_final_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))
}

/// Settle one component's remaining lamport principal and donation residue to
/// their distinct immutable FundingTerms destinations.
///
/// This must follow collateral-vault closure for the component so the token
/// account's extracted rent has already been split and the component vault has
/// returned to its exact pre-close funding balance.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn settle_series_lamport_component<'a>(
    program_id: &Pubkey,
    authenticated: AuthenticatedSeriesCollateralTerminalV1,
    component: SeriesFundingComponentV1,
    component_lamport_vault: &AccountInfo<'a>,
    payer_lamport_refund: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
) -> Outcome<AcceptedSeriesLamportTerminalV1> {
    let funding = authenticated.funding();
    let series = SeriesPlanV5Id::from_bytes(funding.join().series_plan.bytes());
    require(
        CollateralId::from_bytes(series.bytes()) == funding.join().series_plan
            && collateral_id(payer_lamport_refund.key) == funding.join().payer_lamport_refund
            && collateral_id(neutral_lamport_sink.key) == funding.join().neutral_lamport_sink,
        ClutchError::MismatchedState,
    )?;
    require_system_lamport_destination(
        payer_lamport_refund,
        authenticated.projection().lamport_principal_refund,
    )?;
    require_system_lamport_destination(
        neutral_lamport_sink,
        authenticated.projection().neutral_lamport_sink,
    )?;
    require_lamport_vault_metadata(
        program_id,
        series,
        component.index(),
        component_lamport_vault,
    )?;
    require_system_program(system_program)?;
    let projection = authenticated.projection();
    let principal = projection.refundable_principal[component.index()].lamports;
    let donation = projection.donation_residue[component.index()].lamports;
    require(
        component_lamport_vault.lamports() == add(principal, donation)?,
        ClutchError::MismatchedState,
    )?;
    transfer_from_lamport_custody(
        program_id,
        series,
        component,
        component_lamport_vault,
        payer_lamport_refund,
        system_program,
        principal,
    )?;
    transfer_from_lamport_custody(
        program_id,
        series,
        component,
        component_lamport_vault,
        neutral_lamport_sink,
        system_program,
        donation,
    )?;
    require(
        component_lamport_vault.lamports() == 0,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    Ok(AcceptedSeriesLamportTerminalV1 {
        terminal_receipt: ContentId::from_bytes(authenticated.join().terminal_receipt.bytes()),
        component,
        refunded_principal_lamports: principal,
        neutral_donation_lamports: donation,
    })
}

/// Authenticate the Realm-selected mint and all five release-selected Series
/// collateral vaults against the exact state-owned balances.
///
/// `bound` must itself come from the separately authenticated
/// Realm/Profile/policy/release/loader seam. This helper adds only AccountInfo,
/// PDA, hostile token-byte, semantic-owner, compartment, and exact-balance
/// authentication.
#[cfg(test)]
pub fn authenticate_series_collateral_custody(
    program_id: &Pubkey,
    bound: BoundRealmCollateralV2,
    series: SeriesPlanV5Id,
    mint: &AccountInfo<'_>,
    authority: &AccountInfo<'_>,
    vaults: &[AccountInfo<'_>],
    funding: &SeriesFundingAccountV1,
    rent: &RentParameters,
) -> Outcome<()> {
    require(
        vaults.len() == SERIES_CUSTODY_COUNT_V1,
        ClutchError::AccountCount,
    )?;
    funding.validate()?;
    require(
        funding.state.series_plan_id == series,
        ClutchError::MismatchedState,
    )?;
    let expected = accounted_custody_balances(&funding.state)?;
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    admit_realm_collateral_mint_v2(bound, runtime_account_view(mint, &mint_data))
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut index = 0usize;
    while index < SERIES_CUSTODY_COUNT_V1 {
        let component = component_from_index(index)?;
        let binding = series_collateral_binding(
            program_id,
            bound,
            series,
            component,
            &vaults[index],
            authority,
        )?;
        let data = vaults[index]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let observation = admit_realm_collateral_account_v2(
            bound,
            runtime_account_view(&vaults[index], &data),
            TokenAccountRoleV2::SegregatedVault(binding),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let stored_rent = funding.collateral_vault_rent_principal_lamports[index];
        let current_rent = rent.minimum_balance(vaults[index].data_len())?;
        require(
            observation.amount_atoms == expected.collateral_atoms[index]
                && vaults[index].lamports() >= stored_rent
                && vaults[index].lamports() >= current_rent,
            ClutchError::MismatchedState,
        )?;
        index += 1;
    }
    Ok(())
}

fn require_custody_creation_plan(
    plan: CustodyCreationPlanV2,
    token_program: &AccountInfo<'_>,
    vault: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    authority: &AccountInfo<'_>,
) -> Outcome<()> {
    require(
        plan.token_program == collateral_id(token_program.key)
            && plan.account == collateral_id(vault.key)
            && plan.mint == collateral_id(mint.key)
            && plan.owner_authority == collateral_id(authority.key),
        ClutchError::MismatchedState,
    )?;
    require(
        plan.step_count != 0 && usize::from(plan.step_count) <= plan.steps.len(),
        ClutchError::MismatchedState,
    )
}

fn invoke_custody_initialization<'a>(
    plan: CustodyCreationPlanV2,
    vault: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
) -> Outcome<()> {
    let mut index = 0usize;
    while index < usize::from(plan.step_count) {
        match plan.steps[index] {
            CustodyInitializationStepV2::None => return Err(ClutchError::MismatchedState.into()),
            CustodyInitializationStepV2::InitializeImmutableOwner { account, data } => {
                require(
                    account == collateral_id(vault.key),
                    ClutchError::MismatchedState,
                )?;
                let instruction = Instruction::new_with_bytes(
                    *token_program.key,
                    &data,
                    vec![AccountMeta::new(*vault.key, false)],
                );
                invoke(&instruction, &[vault.clone(), token_program.clone()])
                    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
            }
            CustodyInitializationStepV2::InitializeAccount3 {
                account,
                mint: planned_mint,
                owner_authority: _,
                data,
            } => {
                require(
                    account == collateral_id(vault.key) && planned_mint == collateral_id(mint.key),
                    ClutchError::MismatchedState,
                )?;
                let instruction = Instruction::new_with_bytes(
                    *token_program.key,
                    &data,
                    vec![
                        AccountMeta::new(*vault.key, false),
                        AccountMeta::new_readonly(*mint.key, false),
                    ],
                );
                invoke(
                    &instruction,
                    &[vault.clone(), mint.clone(), token_program.clone()],
                )
                .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
            }
        }
        index += 1;
    }
    Ok(())
}

/// Create and admit one release-selected Series collateral vault while
/// preserving exact payer ownership of its rent principal.
///
/// Any lamports sent to the predictable address before creation are first
/// PDA-signed into the authenticated neutral sink. The payer then supplies the
/// complete current rent principal; prefunding never becomes a discount or a
/// refund claim. The returned principal must be persisted in the matching
/// `SeriesFundingAccountV1` component slot before activation can commit.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn create_series_collateral_vault<'a>(
    program_id: &Pubkey,
    bound: BoundRealmCollateralV2,
    series: SeriesPlanV5Id,
    component: SeriesFundingComponentV1,
    payer: &AccountInfo<'a>,
    vault: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    rent: &RentParameters,
) -> Outcome<u64> {
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require(neutral_sink.is_writable, ClutchError::NotWritable)?;
    require(!neutral_sink.executable, ClutchError::ExecutableAccount)?;
    require_creatable(vault)?;
    require_system_program(system_program)?;
    require_collateral_program(token_program, bound)?;
    let binding =
        series_collateral_binding(program_id, bound, series, component, vault, authority)?;
    let plan = prepare_custody_creation_v2(bound, binding)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_custody_creation_plan(plan, token_program, vault, mint, authority)?;
    require(
        payer.key != vault.key
            && payer.key != neutral_sink.key
            && vault.key != neutral_sink.key
            && vault.key != mint.key
            && vault.key != authority.key
            && vault.key != token_program.key,
        ClutchError::AccountAlias,
    )?;

    let component_seed = [component.byte()];
    let series_seed = series.bytes();
    let (_, bump) = seeds::series_collateral_vault_pda(program_id, &series_seed, component.byte());
    let bump_seed = [bump];
    let signer_seeds: &[&[u8]] = &[
        seeds::SEED_SERIES_COLLATERAL_VAULT_V1,
        &series_seed,
        &component_seed,
        &bump_seed,
    ];

    let prefund = vault.lamports();
    if prefund != 0 {
        let sink_before = neutral_sink.lamports();
        let sweep = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &transfer_data(prefund),
            vec![
                AccountMeta::new(*vault.key, true),
                AccountMeta::new(*neutral_sink.key, false),
            ],
        );
        invoke_signed(
            &sweep,
            &[vault.clone(), neutral_sink.clone(), system_program.clone()],
            &[signer_seeds],
        )
        .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
        require(
            vault.lamports() == 0 && neutral_sink.lamports() == add(sink_before, prefund)?,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
    }

    let account_bytes = usize::from(plan.account_bytes);
    let rent_principal_lamports = rent.minimum_balance(account_bytes)?;
    require(rent_principal_lamports != 0, ClutchError::MismatchedState)?;
    let payer_before = payer.lamports();
    let fund = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(rent_principal_lamports),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*vault.key, false),
        ],
    );
    invoke(
        &fund,
        &[payer.clone(), vault.clone(), system_program.clone()],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    require(
        payer.lamports() == sub(payer_before, rent_principal_lamports)?
            && vault.lamports() == rent_principal_lamports,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;

    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(account_bytes),
        vec![AccountMeta::new(*vault.key, true)],
    );
    invoke_signed(
        &allocate,
        &[vault.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(token_program.key),
        vec![AccountMeta::new(*vault.key, true)],
    );
    invoke_signed(
        &assign,
        &[vault.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    require(
        vault.data_len() == account_bytes
            && vault.owner == token_program.key
            && vault.lamports() == rent_principal_lamports,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;

    invoke_custody_initialization(plan, vault, mint, token_program)?;
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    admit_realm_collateral_mint_v2(bound, runtime_account_view(mint, &mint_data))
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let vault_data = vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let observation = admit_realm_collateral_account_v2(
        bound,
        runtime_account_view(vault, &vault_data),
        TokenAccountRoleV2::SegregatedVault(binding),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        observation.amount_atoms == 0 && vault.lamports() == rent_principal_lamports,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    Ok(rent_principal_lamports)
}

fn require_program_account(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    exact_len: usize,
    writable: bool,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    if writable {
        require(account.is_writable, ClutchError::NotWritable)?;
    } else {
        require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    }
    require(
        account.data_len() == exact_len,
        ClutchError::WrongDataLength,
    )
}

fn require_rent_coverage(
    account: &AccountInfo<'_>,
    stored_principal: u64,
    rent: &RentParameters,
) -> Outcome<()> {
    let current_minimum = rent.minimum_balance(account.data_len())?;
    require(stored_principal != 0, ClutchError::MismatchedState)?;
    require(
        account.lamports() >= stored_principal && account.lamports() >= current_minimum,
        ClutchError::MismatchedState,
    )
}

fn credit_lamports(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    let next = add(account.lamports(), amount)?;
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = next;
    Ok(())
}

fn debit_lamports(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    let next = sub(account.lamports(), amount)?;
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = next;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SeriesProgramAccountFundingPoststateV2 {
    payer_after: u64,
    target_after: u64,
    neutral_sink_after: u64,
}

fn series_program_account_funding_poststate_v2(
    payer_before: u64,
    target_prefund: u64,
    neutral_sink_before: u64,
    rent_principal_lamports: u64,
) -> Outcome<SeriesProgramAccountFundingPoststateV2> {
    require(
        rent_principal_lamports != 0,
        ClutchError::MismatchedState,
    )?;
    Ok(SeriesProgramAccountFundingPoststateV2 {
        payer_after: sub(payer_before, rent_principal_lamports)?,
        target_after: rent_principal_lamports,
        neutral_sink_after: add(neutral_sink_before, target_prefund)?,
    })
}

#[allow(clippy::too_many_arguments)]
fn create_series_program_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    exact_len: usize,
    rent_principal_lamports: u64,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require(neutral_sink.is_writable, ClutchError::NotWritable)?;
    require(!neutral_sink.executable, ClutchError::ExecutableAccount)?;
    require(
        payer.key != target.key && payer.key != neutral_sink.key && target.key != neutral_sink.key,
        ClutchError::AccountAlias,
    )?;
    require_system_program(system_program)?;
    let minimum = rent.minimum_balance(exact_len)?;
    require(
        rent_principal_lamports == minimum,
        ClutchError::MismatchedState,
    )?;
    require_creatable(target)?;
    let payer_before = payer.lamports();
    let target_prefund = target.lamports();
    let sink_before = neutral_sink.lamports();
    let expected = series_program_account_funding_poststate_v2(
        payer_before,
        target_prefund,
        sink_before,
        rent_principal_lamports,
    )?;
    if target_prefund != 0 {
        let sweep = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &transfer_data(target_prefund),
            vec![
                AccountMeta::new(*target.key, true),
                AccountMeta::new(*neutral_sink.key, false),
            ],
        );
        invoke_signed(
            &sweep,
            &[target.clone(), neutral_sink.clone(), system_program.clone()],
            &[signer_seeds],
        )
        .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
        require(
            target.lamports() == 0 && neutral_sink.lamports() == expected.neutral_sink_after,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
    }
    create_pda_account(
        program_id,
        payer,
        target,
        system_program,
        rent,
        exact_len,
        signer_seeds,
    )?;
    require(
        payer.lamports() == expected.payer_after
            && target.lamports() == expected.target_after
            && neutral_sink.lamports() == expected.neutral_sink_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )
}

/// Create and encode one persistent registered-Series PDA after a higher typed
/// adapter has authenticated its full registry/artifact join.
///
/// Predictable-address prefunding is donation, not a rent discount. Every
/// preexisting lamport is first moved to the already-authenticated FundingTerms
/// V2 neutral sink, then the payer supplies the complete typed rent principal.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn create_series_registry_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    value: SeriesRegistryAccountV1,
) -> Outcome<()> {
    value.validate()?;
    require(!value.activation_consumed, ClutchError::Replay)?;
    let (address, bump) = seeds::series_registry_pda(program_id, &value.series_plan_id.bytes());
    expect_pda(target.key, (address, bump), Some(value.stored_bump))?;
    let bump_seed = [bump];
    let series_seed = value.series_plan_id.bytes();
    create_series_program_account(
        program_id,
        payer,
        target,
        neutral_sink,
        system_program,
        rent,
        SERIES_REGISTRY_ACCOUNT_BYTES_V1,
        value.rent_principal_lamports,
        &[seeds::SEED_SERIES_REGISTRY_V1, &series_seed, &bump_seed],
    )?;
    let mut data = target
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    value.encode(&mut data)?;
    Ok(())
}

/// Create the sole persistent registration/replay anchor from a private typed
/// registration receipt. The anchor starts unconsumed and is never recreated
/// by any funding or terminal transition.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn register_series_replay_anchor<'a>(
    program_id: &Pubkey,
    registration: AuthenticatedSeriesRegistrationV1,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
) -> Outcome<AuthenticatedSeriesRegistryAccountV1> {
    require_system_lamport_destination(neutral_lamport_sink, registration.neutral_lamport_sink)?;
    let (address, stored_bump) =
        seeds::series_registry_pda(program_id, &registration.series_plan_id.bytes());
    require(*target.key == address, ClutchError::WrongPda)?;
    let value = SeriesRegistryAccountV1 {
        series_plan_id: registration.series_plan_id,
        funding_terms_id: registration.funding_terms_id,
        registry_release_id: registration.registry_release_id,
        capability_profile_id: registration.capability_profile_id,
        rent_principal_lamports: rent.minimum_balance(SERIES_REGISTRY_ACCOUNT_BYTES_V1)?,
        stored_bump,
        activation_consumed: false,
    };
    create_series_registry_account(
        program_id,
        payer,
        target,
        neutral_lamport_sink,
        system_program,
        rent,
        value,
    )?;
    Ok(AuthenticatedSeriesRegistryAccountV1 {
        account: *target.key,
        value,
    })
}

/// Create the persistent replay anchor from a current BundleV5 authorization.
#[allow(clippy::too_many_arguments)]
pub fn register_series_replay_anchor_v3<'a>(
    program_id: &Pubkey,
    registration: AuthenticatedSeriesRegistrationV2,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
) -> Outcome<AuthenticatedSeriesRegistryAccountV2> {
    require_system_lamport_destination(neutral_lamport_sink, registration.neutral_lamport_sink)?;
    let (address, stored_bump) =
        seeds::series_registry_pda(program_id, &registration.series_plan_id.bytes());
    require(*target.key == address, ClutchError::WrongPda)?;
    let value = SeriesRegistryAccountV2 {
        series_plan_id: registration.series_plan_id,
        funding_terms_id: registration.funding_terms_id,
        registry_release_id: registration.registry_release_id,
        capability_profile_id: registration.capability_profile_id,
        compiler_bundle_id: registration.compiler_bundle_id,
        rent_principal_lamports: rent.minimum_balance(SERIES_REGISTRY_ACCOUNT_BYTES_V2)?,
        stored_bump,
        activation_consumed: false,
    };
    create_series_registry_account_v2(
        program_id,
        payer,
        target,
        neutral_lamport_sink,
        system_program,
        rent,
        value,
    )?;
    Ok(AuthenticatedSeriesRegistryAccountV2 {
        account: *target.key,
        value,
    })
}

/// Create and encode the current BundleV5-retaining Series registry PDA.
#[allow(clippy::too_many_arguments)]
pub fn create_series_registry_account_v2<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    value: SeriesRegistryAccountV2,
) -> Outcome<()> {
    value.validate()?;
    require(!value.activation_consumed, ClutchError::Replay)?;
    let (address, bump) = seeds::series_registry_pda(program_id, &value.series_plan_id.bytes());
    expect_pda(target.key, (address, bump), Some(value.stored_bump))?;
    let bump_seed = [bump];
    let series_seed = value.series_plan_id.bytes();
    create_series_program_account(
        program_id,
        payer,
        target,
        neutral_sink,
        system_program,
        rent,
        SERIES_REGISTRY_ACCOUNT_BYTES_V2,
        value.rent_principal_lamports,
        &[seeds::SEED_SERIES_REGISTRY_V1, &series_seed, &bump_seed],
    )?;
    let mut data = target
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    value.encode(&mut data)?;
    Ok(())
}

/// Create the exact current funding account after a private activation
/// authority has authenticated all six physical principal/donation deposits.
#[allow(clippy::too_many_arguments)]
fn create_series_funding_account_v2<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    value: SeriesFundingAccountV2,
) -> Outcome<()> {
    value.validate()?;
    let (address, bump) =
        seeds::series_funding_pda(program_id, &value.state.series_plan_id.bytes());
    expect_pda(target.key, (address, bump), Some(value.stored_bump))?;
    let bump_seed = [bump];
    let series_seed = value.state.series_plan_id.bytes();
    create_series_program_account(
        program_id,
        payer,
        target,
        neutral_sink,
        system_program,
        rent,
        SERIES_FUNDING_ACCOUNT_BYTES_V2,
        value.rent_principal_lamports,
        &[seeds::SEED_SERIES_FUNDING_V1, &series_seed, &bump_seed],
    )?;
    let mut data = target
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    value.encode(&mut data)?;
    Ok(())
}

/// Consume the opaque collateral postwrite directly into current FundingV2
/// activation.
///
/// The independent `authority` must still authenticate all six physical
/// lamport principal/donation compartments through
/// [`AuthenticatedSeriesFundingAuthorityV2`]. This compositor contributes
/// only the five collateral vaults and their separately refundable rent. It
/// never projects the rent array, loader receipt, or custody observations into
/// a public DTO.
#[allow(clippy::too_many_arguments)]
fn activate_series_funding_with_collateral_v2<'a>(
    program_id: &Pubkey,
    collateral: AuthenticatedSeriesCollateralActivationPostwriteV2,
    state: SeriesFundingStateV2,
    registry_account: &AccountInfo<'a>,
    funding_account: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    registry: AuthenticatedSeriesRegistryAccountV2,
    artifacts: &AuthenticatedSeriesArtifactsV4,
    compiler_bundle: AuthenticatedCompiledProductSeriesBundleV5,
    principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    donations: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
) -> Outcome<(
    AuthenticatedSeriesRegistryAccountV2,
    AuthenticatedSeriesFundingAccountV2,
)> {
    let rent = read_rent(rent_sysvar)?;
    let series_plan_id = artifacts
        .series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terms_id = artifacts
        .funding_terms
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_quote_id = artifacts
        .quote
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attachment_plan_id = artifacts
        .attachment
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_release_id = collateral
        .bound
        .release()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(
        collateral.receipt_id() == collateral.vault_rent.authentication_id
            && !collateral.receipt_id().is_zero()
            && collateral.series_plan_id == series_plan_id
            && collateral.funding_terms_id == funding_terms_id
            && collateral.compiler_bundle_id == compiler_bundle.bundle_id()
            && collateral.funding_quote_id == funding_quote_id
            && collateral.attachment_plan_id == attachment_plan_id
            && collateral.funding_account == *funding_account.key
            && collateral.payer == *payer.key
            && collateral.neutral_lamport_sink == *neutral_lamport_sink.key
            && collateral.rent_sysvar == *rent_sysvar.key
            && collateral.rent == rent
            && collateral.principal == principal
            && collateral.donations == donations
            && collateral.vault_rent.series_plan_id == series_plan_id
            && collateral.vault_rent.principal_lamports
                == collateral.vault_rent_principal_lamports
            && collateral.deployment.release() == collateral.bound.release()
            && collateral.deployment.release_id() == expected_release_id
            && !collateral.deployment.programdata_account().is_zero()
            && !collateral.deployment.receipt_id().is_zero()
            && !collateral.source_prestate_id.is_zero()
            && !collateral.source_poststate_id.is_zero()
            && collateral.payer_token_account != collateral.payer_token_authority
            && collateral.collateral_authority
                == seeds::series_collateral_authority_pda(program_id, &series_plan_id.bytes()).0
            && registry.value().series_plan_id == series_plan_id
            && registry.value().funding_terms_id == funding_terms_id
            && registry.value().compiler_bundle_id == compiler_bundle.bundle_id(),
        ClutchError::MismatchedState,
    )?;
    let mut index = 0usize;
    let mut total_vault_rent_principal = 0u64;
    let mut total_swept_prefund_donation = 0u64;
    while index < SERIES_COLLATERAL_VAULT_COUNT_V2 {
        let coordinate = series_collateral_vault_coordinate_v2(index)?;
        let component_index = coordinate.component.index();
        let amount_atoms = add(
            principal[component_index].collateral_atoms,
            donations[component_index].collateral_atoms,
        )?;
        require(
            collateral.vault_accounts[index]
                == seeds::series_collateral_vault_pda(
                    program_id,
                    &series_plan_id.bytes(),
                    coordinate.seed,
                )
                .0
                && collateral.vault_data_lengths[index] != 0
                && collateral.vault_rent_principal_lamports[index] != 0
                && collateral.vault_collateral_principal_atoms[index]
                    == principal[component_index].collateral_atoms
                && collateral.vault_collateral_donation_atoms[index]
                    == donations[component_index].collateral_atoms
                && collateral.vault_collateral_atoms_after[index] == amount_atoms
                && !collateral.vault_poststate_ids[index].is_zero()
                && collateral.transfer_poststate_ids[index].is_zero() == (amount_atoms == 0),
            ClutchError::MismatchedState,
        )?;
        total_vault_rent_principal = add(
            total_vault_rent_principal,
            collateral.vault_rent_principal_lamports[index],
        )?;
        total_swept_prefund_donation = add(
            total_swept_prefund_donation,
            collateral.swept_prefund_donation_lamports[index],
        )?;
        index += 1;
    }
    require(
        collateral.payer_lamports_after
            == sub(
                collateral.payer_lamports_before,
                total_vault_rent_principal,
            )?
            && collateral.neutral_sink_lamports_after
                == add(
                    collateral.neutral_sink_lamports_before,
                    total_swept_prefund_donation,
                )?,
        ClutchError::MismatchedState,
    )?;
    index = 0;
    while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
        require(
            state.components[index].remaining_principal == principal[index]
                && state.components[index].donations == donations[index]
                && state.components[index].consumed_allocations == 0,
            ClutchError::MismatchedState,
        )?;
        index += 1;
    }
    commit_series_funding_account_v2(
        program_id,
        state,
        registry_account,
        funding_account,
        payer,
        neutral_lamport_sink,
        system_program,
        &rent,
        registry,
        artifacts,
        compiler_bundle,
        collateral.vault_rent,
    )
}

/// Complete current physical activation boundary for Product FundingV2.
///
/// A caller supplies an independently authenticated authority for the six
/// lamport compartments. This function then authenticates the live collateral
/// deployment, creates and funds the five segregated vaults, consumes the
/// resulting private postwrite into the unconstructible rent token, creates
/// FundingV2, and consumes RegistryV2 replay in one instruction. The private
/// collateral receipt cannot be returned, copied, or downgraded between these
/// steps. Dispatch remains capability-disabled until Product supplies the
/// six-compartment authority and exact account contract.
#[allow(clippy::too_many_arguments)]
pub(crate) fn deploy_and_activate_series_funding_v2<
    'a,
    A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized,
>(
    program_id: &Pubkey,
    authority: &A,
    registry_account: &AccountInfo<'a>,
    funding_account: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    payer_token_account: &AccountInfo<'a>,
    payer_token_authority: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    collateral_authority: &AccountInfo<'a>,
    realm_account: &AccountInfo<'a>,
    profile_account: &AccountInfo<'a>,
    policy_account: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    token_programdata: &AccountInfo<'a>,
    collateral_vaults: &[AccountInfo<'a>],
    system_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    registry: AuthenticatedSeriesRegistryAccountV2,
    artifacts: &AuthenticatedSeriesArtifactsV4,
    compiler_bundle: AuthenticatedCompiledProductSeriesBundleV5,
    principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    donations: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
) -> Outcome<(
    AuthenticatedSeriesRegistryAccountV2,
    AuthenticatedSeriesFundingAccountV2,
)> {
    let rent = read_rent(rent_sysvar)?;
    let live_registry = read_series_registry_account_v2_with_role(
        program_id,
        registry_account,
        registry.value().series_plan_id,
        &rent,
        true,
    )?;
    require(live_registry == registry, ClutchError::MismatchedState)?;
    require(!registry.activation_consumed(), ClutchError::Replay)?;
    let preflight_state = SeriesFundingStateV2::activate(
        authority,
        &artifacts.series,
        registry.value().funding_terms_id,
        compiler_bundle.bundle_id(),
        &artifacts.quote,
        &artifacts.attachment,
        principal,
        donations,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(
        preflight_state.series_plan_id == registry.value().series_plan_id
            && preflight_state.funding_terms_id == registry.value().funding_terms_id
            && preflight_state.compiler_bundle_id == registry.value().compiler_bundle_id,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        funding_account.key,
        seeds::series_funding_pda(program_id, &preflight_state.series_plan_id.bytes()),
        None,
    )?;
    require_creatable(funding_account)?;
    let collateral = deploy_series_collateral_activation_v2(
        program_id,
        registry,
        artifacts,
        compiler_bundle,
        realm_account,
        profile_account,
        policy_account,
        mint,
        token_program,
        token_programdata,
        payer,
        payer_token_account,
        payer_token_authority,
        collateral_authority,
        neutral_lamport_sink,
        system_program,
        rent_sysvar,
        collateral_vaults,
        principal,
        donations,
    )?;
    activate_series_funding_with_collateral_v2(
        program_id,
        collateral,
        preflight_state,
        registry_account,
        funding_account,
        payer,
        neutral_lamport_sink,
        system_program,
        rent_sysvar,
        registry,
        artifacts,
        compiler_bundle,
        principal,
        donations,
    )
}

/// Atomically create current funding state and consume RegistryV2 activation.
#[allow(clippy::too_many_arguments)]
fn commit_series_funding_account_v2<'a>(
    program_id: &Pubkey,
    state: SeriesFundingStateV2,
    registry_account: &AccountInfo<'a>,
    funding_account: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    registry: AuthenticatedSeriesRegistryAccountV2,
    artifacts: &AuthenticatedSeriesArtifactsV4,
    compiler_bundle: AuthenticatedCompiledProductSeriesBundleV5,
    collateral_vault_rent: AuthenticatedSeriesCollateralVaultRentV2,
) -> Outcome<(
    AuthenticatedSeriesRegistryAccountV2,
    AuthenticatedSeriesFundingAccountV2,
)> {
    let live_registry = read_series_registry_account_v2_with_role(
        program_id,
        registry_account,
        registry.value().series_plan_id,
        rent,
        true,
    )?;
    require(
        live_registry == registry
            && registry.account() == *registry_account.key
            && !registry.activation_consumed()
            && registry.value().compiler_bundle_id == compiler_bundle.bundle_id()
            && compiler_bundle.bundle().series_plan_id == registry.value().series_plan_id
            && compiler_bundle.bundle().funding_terms_id == registry.value().funding_terms_id
            && collateral_vault_rent.series_plan_id == registry.value().series_plan_id
            && !collateral_vault_rent.authentication_id.is_zero()
            && collateral_vault_rent
                .principal_lamports
                .iter()
                .all(|principal| *principal != 0),
        ClutchError::MismatchedState,
    )?;
    require_system_lamport_destination(
        neutral_lamport_sink,
        artifacts.funding_terms.neutral_lamport_sink,
    )?;
    state
        .validate_against(&artifacts.series, &artifacts.quote, &artifacts.attachment)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        state.series_plan_id == registry.value().series_plan_id
            && state.funding_terms_id == registry.value().funding_terms_id
            && state.compiler_bundle_id == compiler_bundle.bundle_id(),
        ClutchError::MismatchedState,
    )?;
    let (_, stored_bump) = seeds::series_funding_pda(program_id, &state.series_plan_id.bytes());
    let value = SeriesFundingAccountV2 {
        state,
        rent_principal_lamports: rent.minimum_balance(SERIES_FUNDING_ACCOUNT_BYTES_V2)?,
        collateral_vault_rent_principal_lamports: collateral_vault_rent.principal_lamports,
        stored_bump,
    };
    create_series_funding_account_v2(
        program_id,
        payer,
        funding_account,
        neutral_lamport_sink,
        system_program,
        rent,
        value,
    )?;
    let funding = authenticate_live_series_funding_value_v2(
        program_id,
        funding_account,
        value,
        artifacts,
        rent,
    )?;
    let consumed = consume_series_activation_v2(
        program_id,
        registry_account,
        state.series_plan_id,
        rent,
    )?;
    Ok((
        consumed,
        funding,
    ))
}

/// Create and encode one mutable funding PDA after exact activation principal,
/// collateral-vault, liveness, and registry joins have produced its pure state.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn create_series_funding_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    value: SeriesFundingAccountV1,
) -> Outcome<()> {
    value.validate()?;
    let (address, bump) =
        seeds::series_funding_pda(program_id, &value.state.series_plan_id.bytes());
    expect_pda(target.key, (address, bump), Some(value.stored_bump))?;
    let bump_seed = [bump];
    let series_seed = value.state.series_plan_id.bytes();
    create_series_program_account(
        program_id,
        payer,
        target,
        neutral_sink,
        system_program,
        rent,
        SERIES_FUNDING_ACCOUNT_BYTES_V1,
        value.rent_principal_lamports,
        &[seeds::SEED_SERIES_FUNDING_V1, &series_seed, &bump_seed],
    )?;
    let mut data = target
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    value.encode(&mut data)?;
    Ok(())
}

/// Atomically create the one funding root and consume the registration replay
/// bit after a typed authority authenticates registry provenance, payer
/// attribution, liveness funding, and the exact component deposits.
///
/// All ten custody accounts are checked against the resulting pure state
/// before either program-owned root is written. Consuming the replay bit is
/// last, so any failed creation, custody check, or write rolls the transaction
/// back to the unconsumed anchor.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn activate_series_funding_account<'a, A: AuthenticatedSeriesFundingAuthorityV1 + ?Sized>(
    program_id: &Pubkey,
    authority: &A,
    projection: &RegistryCapabilityProjectionV2,
    artifacts: &AuthenticatedSeriesArtifactsV1,
    bound_collateral: BoundRealmCollateralV2,
    payer: &AccountInfo<'a>,
    registry_account: &AccountInfo<'a>,
    funding_account: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    collateral_authority: &AccountInfo<'a>,
    lamport_vaults: &[AccountInfo<'a>],
    collateral_vaults: &[AccountInfo<'a>],
    rent: &RentParameters,
    principal: [ComponentDebitV1; SERIES_CUSTODY_COUNT_V1],
    donations: [ComponentDebitV1; SERIES_CUSTODY_COUNT_V1],
) -> Outcome<(
    AuthenticatedSeriesRegistryAccountV1,
    AuthenticatedSeriesFundingAccountV1,
)> {
    let series_plan_id = artifacts
        .series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terms_id = artifacts
        .funding_terms
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let registry = read_series_registry_account_with_role(
        program_id,
        registry_account,
        series_plan_id,
        rent,
        true,
    )?;
    require(!registry.activation_consumed(), ClutchError::Replay)?;
    require(
        registry.value().funding_terms_id == funding_terms_id
            && registry.value().registry_release_id == projection.registry_release_id
            && registry.value().capability_profile_id == projection.capability_profile_id,
        ClutchError::MismatchedState,
    )?;
    artifacts.validate_registry_projection(projection)?;
    require_system_lamport_destination(
        neutral_lamport_sink,
        artifacts.funding_terms.neutral_lamport_sink,
    )?;
    let realm = bound_collateral.realm();
    let policy = bound_collateral.policy();
    require(
        realm.realm == collateral_content_id(artifacts.genesis.realm_id)
            && realm.profile == collateral_content_id(artifacts.genesis.profile_id)
            && policy.mint == collateral_content_id(artifacts.funding_terms.collateral_mint)
            && policy.token_program == collateral_content_id(artifacts.funding_terms.token_program),
        ClutchError::MismatchedState,
    )?;
    let context = SeriesActivationContextV1 {
        series: &artifacts.series,
        template: &artifacts.template,
        basis: &artifacts.basis,
        recovery: &artifacts.recovery,
        price_policy: &artifacts.price_policy,
        genesis: &artifacts.genesis,
        attachment: &artifacts.attachment,
        quote: &artifacts.quote,
        funding_terms: &artifacts.funding_terms,
        registry: projection,
        principal,
        donations,
    };
    let state = SeriesFundingStateV1::activate(authority, &context)
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let mut collateral_rent = [0; SERIES_CUSTODY_COUNT_V1];
    require(
        collateral_vaults.len() == SERIES_CUSTODY_COUNT_V1,
        ClutchError::AccountCount,
    )?;
    let mut index = 0usize;
    while index < SERIES_CUSTODY_COUNT_V1 {
        collateral_rent[index] = rent.minimum_balance(collateral_vaults[index].data_len())?;
        require(collateral_rent[index] != 0, ClutchError::MismatchedState)?;
        index += 1;
    }
    let (_, stored_bump) = seeds::series_funding_pda(program_id, &series_plan_id.bytes());
    let value = SeriesFundingAccountV1 {
        state,
        rent_principal_lamports: rent.minimum_balance(SERIES_FUNDING_ACCOUNT_BYTES_V1)?,
        collateral_vault_rent_principal_lamports: collateral_rent,
        stored_bump,
        flags: 0,
    };
    let expected = accounted_custody_balances(&state)?;
    authenticate_lamport_custody(program_id, series_plan_id, lamport_vaults, &expected)?;
    authenticate_series_collateral_custody(
        program_id,
        bound_collateral,
        series_plan_id,
        mint,
        collateral_authority,
        collateral_vaults,
        &value,
        rent,
    )?;
    require(
        registry_account.key != funding_account.key,
        ClutchError::AccountAlias,
    )?;
    create_series_funding_account(
        program_id,
        payer,
        funding_account,
        neutral_lamport_sink,
        system_program,
        rent,
        value,
    )?;
    let consumed = consume_series_activation(program_id, registry_account, series_plan_id, rent)?;
    Ok((
        consumed,
        AuthenticatedSeriesFundingAccountV1 {
            account: *funding_account.key,
            value,
        },
    ))
}

/// Authenticate a read-only registered-Series replay anchor, including exact
/// PDA, program owner, codec, and stored/current rent coverage.
#[cfg(test)]
pub fn read_series_registry_account(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series: SeriesPlanV5Id,
    rent: &RentParameters,
) -> Outcome<AuthenticatedSeriesRegistryAccountV1> {
    read_series_registry_account_with_role(program_id, account, expected_series, rent, false)
}

/// Authenticate the exact current read-only BundleV5-retaining registry.
pub fn read_series_registry_account_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series: SeriesPlanV5Id,
    rent: &RentParameters,
) -> Outcome<AuthenticatedSeriesRegistryAccountV2> {
    read_series_registry_account_v2_with_role(program_id, account, expected_series, rent, false)
}

fn read_series_registry_account_v2_with_role(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series: SeriesPlanV5Id,
    rent: &RentParameters,
    writable: bool,
) -> Outcome<AuthenticatedSeriesRegistryAccountV2> {
    require_program_account(
        program_id,
        account,
        SERIES_REGISTRY_ACCOUNT_BYTES_V2,
        writable,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesRegistryAccountV2::decode(&data)?;
    require(
        value.series_plan_id == expected_series,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        account.key,
        seeds::series_registry_pda(program_id, &expected_series.bytes()),
        Some(value.stored_bump),
    )?;
    require_rent_coverage(account, value.rent_principal_lamports, rent)?;
    Ok(AuthenticatedSeriesRegistryAccountV2 {
        account: *account.key,
        value,
    })
}

/// Consume the sole current funding activation without dropping BundleV5.
fn consume_series_activation_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series: SeriesPlanV5Id,
    rent: &RentParameters,
) -> Outcome<AuthenticatedSeriesRegistryAccountV2> {
    let current = read_series_registry_account_v2_with_role(
        program_id,
        account,
        expected_series,
        rent,
        true,
    )?;
    require(!current.activation_consumed(), ClutchError::Replay)?;
    let next = SeriesRegistryAccountV2 {
        activation_consumed: true,
        ..current.value()
    };
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    next.encode(&mut data)?;
    drop(data);
    let rebound = read_series_registry_account_v2_with_role(
        program_id,
        account,
        expected_series,
        rent,
        true,
    )?;
    require(rebound.value() == next, ClutchError::MismatchedState)?;
    Ok(rebound)
}

/// Authenticate a mutable current funding account against RegistryV2 and the
/// exact reopened BundleV5/QuoteV4/AttachmentV4 graph.
pub fn read_series_funding_account_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    registry: AuthenticatedSeriesRegistryAccountV2,
    artifacts: &AuthenticatedSeriesArtifactsV4,
    rent: &RentParameters,
) -> Outcome<AuthenticatedSeriesFundingAccountV2> {
    require_program_account(program_id, account, SERIES_FUNDING_ACCOUNT_BYTES_V2, true)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesFundingAccountV2::decode(&data)?;
    let registry_value = registry.value();
    require(
        registry.activation_consumed()
            && value.state.series_plan_id == registry_value.series_plan_id
            && value.state.funding_terms_id == registry_value.funding_terms_id
            && value.state.compiler_bundle_id == registry_value.compiler_bundle_id,
        ClutchError::MismatchedState,
    )?;
    value
        .state
        .validate_against(&artifacts.series, &artifacts.quote, &artifacts.attachment)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    expect_pda(
        account.key,
        seeds::series_funding_pda(program_id, &registry_value.series_plan_id.bytes()),
        Some(value.stored_bump),
    )?;
    require_rent_coverage(account, value.rent_principal_lamports, rent)?;
    Ok(AuthenticatedSeriesFundingAccountV2 {
        account: *account.key,
        value,
    })
}

/// Write one validated current successor without changing framing/rent facts.
fn write_series_funding_state_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    current: AuthenticatedSeriesFundingAccountV2,
    next: SeriesFundingStateV2,
    artifacts: &AuthenticatedSeriesArtifactsV4,
    rent: &RentParameters,
) -> Outcome<AuthenticatedSeriesFundingAccountV2> {
    let live = authenticate_live_series_funding_value_v2(
        program_id,
        account,
        current.value(),
        artifacts,
        rent,
    )?;
    require(
        live == current
            && current.account() == *account.key
            && current.value().state.series_plan_id == next.series_plan_id
            && current.value().state.funding_terms_id == next.funding_terms_id
            && current.value().state.funding_quote_id == next.funding_quote_id
            && current.value().state.attachment_plan_id == next.attachment_plan_id
            && current.value().state.compiler_bundle_id == next.compiler_bundle_id
            && current.value().state.instance_count == next.instance_count,
        ClutchError::MismatchedState,
    )?;
    next.validate_against(&artifacts.series, &artifacts.quote, &artifacts.attachment)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let updated = SeriesFundingAccountV2 {
        state: next,
        ..current.value()
    };
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    updated.encode(&mut data)?;
    drop(data);
    authenticate_live_series_funding_value_v2(
        program_id,
        account,
        updated,
        artifacts,
        rent,
    )
}

fn authenticate_live_series_funding_value_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected: SeriesFundingAccountV2,
    artifacts: &AuthenticatedSeriesArtifactsV4,
    rent: &RentParameters,
) -> Outcome<AuthenticatedSeriesFundingAccountV2> {
    require_program_account(program_id, account, SERIES_FUNDING_ACCOUNT_BYTES_V2, true)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesFundingAccountV2::decode(&data)?;
    require(value == expected, ClutchError::MismatchedState)?;
    value
        .state
        .validate_against(&artifacts.series, &artifacts.quote, &artifacts.attachment)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    expect_pda(
        account.key,
        seeds::series_funding_pda(program_id, &value.state.series_plan_id.bytes()),
        Some(value.stored_bump),
    )?;
    require_rent_coverage(account, value.rent_principal_lamports, rent)?;
    Ok(AuthenticatedSeriesFundingAccountV2 {
        account: *account.key,
        value,
    })
}

/// Reserve the next current ordinal without advancing its cursor.
#[allow(clippy::too_many_arguments)]
pub fn reserve_series_occurrence_v2<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    current: AuthenticatedSeriesFundingAccountV2,
    artifacts: &AuthenticatedSeriesArtifactsV4,
    authority: &A,
    market_instance_id: clutch_product_series::MarketInstanceV2Id,
    source_occurrence_id: SourceOccurrenceV1Id,
    series_market_link_id: ContentId,
    disposition: clutch_product_series::SeriesMarketDispositionV1,
    debits: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    reservation_receipt_id: ContentId,
    rent: &RentParameters,
) -> Outcome<(AuthenticatedSeriesFundingAccountV2, u32)> {
    let mut next = current.value().state;
    let ordinal = next
        .reserve_created(
            authority,
            &artifacts.series,
            &artifacts.quote,
            &artifacts.attachment,
            market_instance_id,
            source_occurrence_id,
            series_market_link_id,
            disposition,
            debits,
            reservation_receipt_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let written = write_series_funding_state_v2(
        program_id, account, current, next, artifacts, rent,
    )?;
    Ok((written, ordinal))
}

/// Commit the exact pending ordinal after its link/Market transition succeeds.
pub fn complete_series_occurrence_v2<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    current: AuthenticatedSeriesFundingAccountV2,
    artifacts: &AuthenticatedSeriesArtifactsV4,
    authority: &A,
    completion_receipt_id: ContentId,
    rent: &RentParameters,
) -> Outcome<(AuthenticatedSeriesFundingAccountV2, u32)> {
    let mut next = current.value().state;
    let ordinal = next
        .complete_pending(
            authority,
            &artifacts.series,
            &artifacts.quote,
            &artifacts.attachment,
            completion_receipt_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let written = write_series_funding_state_v2(
        program_id, account, current, next, artifacts, rent,
    )?;
    Ok((written, ordinal))
}

/// Restore exact pending principal only after authenticated inert reverse-close.
pub fn abort_series_occurrence_v2<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    current: AuthenticatedSeriesFundingAccountV2,
    artifacts: &AuthenticatedSeriesArtifactsV4,
    authority: &A,
    abort_receipt_id: ContentId,
    rent: &RentParameters,
) -> Outcome<(AuthenticatedSeriesFundingAccountV2, u32)> {
    let mut next = current.value().state;
    let ordinal = next
        .abort_pending(
            authority,
            &artifacts.series,
            &artifacts.quote,
            &artifacts.attachment,
            abort_receipt_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let written = write_series_funding_state_v2(
        program_id, account, current, next, artifacts, rent,
    )?;
    Ok((written, ordinal))
}

/// Apply a free lapse from the authenticated Clock owner.
pub fn lapse_series_occurrence_v2<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    current: AuthenticatedSeriesFundingAccountV2,
    artifacts: &AuthenticatedSeriesArtifactsV4,
    authority: &A,
    rent: &RentParameters,
) -> Outcome<(AuthenticatedSeriesFundingAccountV2, u32)> {
    let mut next = current.value().state;
    let ordinal = next
        .lapse(
            authority,
            &artifacts.series,
            &artifacts.quote,
            &artifacts.attachment,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let written = write_series_funding_state_v2(
        program_id, account, current, next, artifacts, rent,
    )?;
    Ok((written, ordinal))
}

/// Fold physical surplus into exactly one current donation compartment.
pub fn observe_series_donation_v2<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    current: AuthenticatedSeriesFundingAccountV2,
    artifacts: &AuthenticatedSeriesArtifactsV4,
    authority: &A,
    component: SeriesFundingComponentV2,
    amount: ComponentDebitV1,
    rent: &RentParameters,
) -> Outcome<AuthenticatedSeriesFundingAccountV2> {
    let mut next = current.value().state;
    next.add_donation(
        authority,
        &artifacts.series,
        &artifacts.quote,
        &artifacts.attachment,
        component,
        amount,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    write_series_funding_state_v2(program_id, account, current, next, artifacts, rent)
}

/// Derive the exact current terminal principal/donation projection.
pub(crate) fn close_series_funding_v2<A: AuthenticatedSeriesFundingAuthorityV2 + ?Sized>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    current: AuthenticatedSeriesFundingAccountV2,
    artifacts: &AuthenticatedSeriesArtifactsV4,
    authority: &A,
    terminal_receipt_id: ContentId,
    rent: &RentParameters,
) -> Outcome<SeriesFundingTerminalProjectionV2> {
    let live = authenticate_live_series_funding_value_v2(
        program_id,
        account,
        current.value(),
        artifacts,
        rent,
    )?;
    require(live == current, ClutchError::MismatchedState)?;
    live
        .value()
        .state
        .close(
            authority,
            &artifacts.series,
            &artifacts.quote,
            &artifacts.attachment,
            terminal_receipt_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))
}

#[cfg(test)]
fn read_series_registry_account_with_role(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series: SeriesPlanV5Id,
    rent: &RentParameters,
    writable: bool,
) -> Outcome<AuthenticatedSeriesRegistryAccountV1> {
    require_program_account(
        program_id,
        account,
        SERIES_REGISTRY_ACCOUNT_BYTES_V1,
        writable,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesRegistryAccountV1::decode(&data)?;
    require(
        value.series_plan_id == expected_series,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        account.key,
        seeds::series_registry_pda(program_id, &expected_series.bytes()),
        Some(value.stored_bump),
    )?;
    require_rent_coverage(account, value.rent_principal_lamports, rent)?;
    Ok(AuthenticatedSeriesRegistryAccountV1 {
        account: *account.key,
        value,
    })
}

/// Consume the one permitted activation in the persistent registry/replay
/// anchor while preserving every immutable ID, bump, and rent fact.
///
/// Callers must perform this write in the same instruction as funding-account
/// and custody creation. Any later failure rolls the whole transaction back;
/// a committed `true` value can never be reset by this ABI.
#[cfg(test)]
pub fn consume_series_activation(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series: SeriesPlanV5Id,
    rent: &RentParameters,
) -> Outcome<AuthenticatedSeriesRegistryAccountV1> {
    let current =
        read_series_registry_account_with_role(program_id, account, expected_series, rent, true)?;
    require(!current.activation_consumed(), ClutchError::Replay)?;
    let next = SeriesRegistryAccountV1 {
        activation_consumed: true,
        ..current.value()
    };
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    next.encode(&mut data)?;
    Ok(AuthenticatedSeriesRegistryAccountV1 {
        account: *account.key,
        value: next,
    })
}

/// Authenticate a mutable Series-funding account and join it to its exact
/// quote before returning the decoded state.
#[cfg(test)]
pub fn read_series_funding_account(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series: SeriesPlanV5Id,
    quote: &clutch_product_series::SeriesFundingQuoteV1,
    rent: &RentParameters,
) -> Outcome<AuthenticatedSeriesFundingAccountV1> {
    require_program_account(program_id, account, SERIES_FUNDING_ACCOUNT_BYTES_V1, true)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesFundingAccountV1::decode(&data)?;
    require(
        value.state.series_plan_id == expected_series,
        ClutchError::MismatchedState,
    )?;
    value
        .state
        .validate_against_quote(quote)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    expect_pda(
        account.key,
        seeds::series_funding_pda(program_id, &expected_series.bytes()),
        Some(value.stored_bump),
    )?;
    require_rent_coverage(account, value.rent_principal_lamports, rent)?;
    Ok(AuthenticatedSeriesFundingAccountV1 {
        account: *account.key,
        value,
    })
}

/// Write one already-validated atomic successor state back to its authenticated
/// account wrapper without changing rent ownership or framing.
#[cfg(test)]
pub fn write_series_funding_state(
    account: &AccountInfo<'_>,
    current: AuthenticatedSeriesFundingAccountV1,
    next: SeriesFundingStateV1,
) -> Outcome<()> {
    require(
        current.account() == *account.key,
        ClutchError::MismatchedState,
    )?;
    let current = current.value();
    require(
        current.state.series_plan_id == next.series_plan_id
            && current.state.funding_terms_id == next.funding_terms_id
            && current.state.funding_quote_id == next.funding_quote_id
            && current.state.instance_count == next.instance_count,
        ClutchError::MismatchedState,
    )?;
    next.validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let updated = SeriesFundingAccountV1 {
        state: next,
        ..current
    };
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    updated.encode(&mut data)?;
    Ok(())
}

/// Authenticate the exact terminal Series root before any value movement.
///
/// The consumed registry receipt proves that this is the sole V1 activation.
/// The funding receipt proves the exact PDA/body/rent join. The authenticated
/// artifact graph supplies the sole FundingTerms and quote bodies from which
/// the pure terminal projection is derived. This does not itself authorize or
/// execute any lamport or collateral transfer.
#[cfg(test)]
pub fn authenticate_series_terminal(
    registry: AuthenticatedSeriesRegistryAccountV1,
    funding: AuthenticatedSeriesFundingAccountV1,
    artifacts: &AuthenticatedSeriesArtifactsV1,
) -> Outcome<AuthenticatedSeriesTerminalV1> {
    require(registry.activation_consumed(), ClutchError::Replay)?;
    let registry_value = registry.value();
    let funding_value = funding.value();
    let series_plan_id = artifacts
        .series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terms_id = artifacts
        .funding_terms
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_quote_id = artifacts
        .quote
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        registry_value.series_plan_id == series_plan_id
            && registry_value.funding_terms_id == funding_terms_id
            && funding_value.state.series_plan_id == series_plan_id
            && funding_value.state.funding_terms_id == funding_terms_id
            && funding_value.state.funding_quote_id == funding_quote_id,
        ClutchError::MismatchedState,
    )?;
    funding_value
        .state
        .validate_against_quote(&artifacts.quote)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let projection = funding_value
        .state
        .terminal_projection(&artifacts.funding_terms, &artifacts.quote)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let mut registry_body = [0; SERIES_REGISTRY_ACCOUNT_BYTES_V1];
    registry_value.encode(&mut registry_body)?;
    let mut funding_body = [0; SERIES_FUNDING_ACCOUNT_BYTES_V1];
    funding_value.encode(&mut funding_body)?;
    let generation = SERIES_ACTIVATION_GENERATION_V1.to_le_bytes();
    let registry_account = registry.account();
    let funding_account = funding.account();
    let receipt_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_TERMINAL_RECEIPT_DOMAIN_V1,
            registry_account.as_ref(),
            &registry_body,
            funding_account.as_ref(),
            &funding_body,
            &generation,
        ])
        .to_bytes(),
    );
    Ok(AuthenticatedSeriesTerminalV1 {
        registry_account,
        funding_account,
        series_plan_id,
        funding_terms_id,
        funding_quote_id,
        activation_generation: SERIES_ACTIVATION_GENERATION_V1,
        projection,
        receipt_id,
    })
}

#[cfg(test)]
fn authenticate_series_clock(
    artifacts: &AuthenticatedSeriesArtifactsV1,
    source_release: AuthenticatedSourceReleaseV1,
    clock_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSeriesClockAuthorityV1> {
    require(
        *clock_account.key == crate::instructions::artifact::CLOCK_SYSVAR_ID
            && clock_account.owner.to_bytes() == crate::instructions_sysvar::SYSVAR_OWNER_ID,
        ClutchError::WrongClockSysvar,
    )?;
    require(
        !clock_account.is_signer
            && !clock_account.is_writable
            && !clock_account.executable
            && clock_account.data_len() == crate::instructions::artifact::CLOCK_SYSVAR_LEN,
        ClutchError::WrongClockSysvar,
    )?;
    let manifest = source_release.manifest();
    require(
        manifest.source_plane_contract_id == artifacts.template.source_plane_contract_id
            && manifest.source_spec_id == artifacts.template.source_spec_id,
        ClutchError::MismatchedState,
    )?;
    let data = clock_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let slot = u64::from_le_bytes(
        data[..8]
            .try_into()
            .map_err(|_| Refusal::Adapter(ClutchError::WrongClockSysvar))?,
    );
    let unix_timestamp_signed = i64::from_le_bytes(
        data[32..40]
            .try_into()
            .map_err(|_| Refusal::Adapter(ClutchError::WrongClockSysvar))?,
    );
    let unix_timestamp = u64::try_from(unix_timestamp_signed)
        .map_err(|_| Refusal::Adapter(ClutchError::WrongClockSysvar))?;
    let clock = AuthenticatedClockBucketV1::from_snapshot(
        &source_release.clock_policy(),
        ClockSnapshotV1 {
            slot,
            unix_timestamp,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::WrongClockSysvar))?;
    let series_plan_id = artifacts
        .series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(AuthenticatedSeriesClockAuthorityV1 {
        series_plan_id,
        clock,
    })
}

fn authenticate_series_clock_v2(
    artifacts: &AuthenticatedSeriesArtifactsV4,
    source_release: AuthenticatedSourceReleaseV1,
    clock_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSeriesClockAuthorityV2> {
    require(
        *clock_account.key == crate::instructions::artifact::CLOCK_SYSVAR_ID
            && clock_account.owner.to_bytes() == crate::instructions_sysvar::SYSVAR_OWNER_ID
            && !clock_account.is_signer
            && !clock_account.is_writable
            && !clock_account.executable
            && clock_account.data_len() == crate::instructions::artifact::CLOCK_SYSVAR_LEN,
        ClutchError::WrongClockSysvar,
    )?;
    let manifest = source_release.manifest();
    require(
        manifest.source_plane_contract_id == artifacts.template.source_plane_contract_id
            && manifest.source_spec_id == artifacts.template.source_spec_id,
        ClutchError::MismatchedState,
    )?;
    let data = clock_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let slot = u64::from_le_bytes(
        data[..8]
            .try_into()
            .map_err(|_| Refusal::Adapter(ClutchError::WrongClockSysvar))?,
    );
    let unix_timestamp = u64::try_from(i64::from_le_bytes(
        data[32..40]
            .try_into()
            .map_err(|_| Refusal::Adapter(ClutchError::WrongClockSysvar))?,
    ))
    .map_err(|_| Refusal::Adapter(ClutchError::WrongClockSysvar))?;
    let clock = AuthenticatedClockBucketV1::from_snapshot(
        &source_release.clock_policy(),
        ClockSnapshotV1 {
            slot,
            unix_timestamp,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::WrongClockSysvar))?;
    Ok(AuthenticatedSeriesClockAuthorityV2 {
        series_plan_id: artifacts
            .series
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        clock,
    })
}

/// Apply the free elapsed-occurrence transition from the real Solana Clock and
/// the sole ClockPolicy embedded in an authenticated Source release.
///
/// This transition emits no liveness work authorization and consumes no
/// component principal. The caller cannot provide a current bucket or shadow
/// Clock policy. Central registry-release authentication remains a separate
/// missing join, so this helper is not dispatched.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn lapse_series_occurrence(
    funding_account: &AccountInfo<'_>,
    funding: AuthenticatedSeriesFundingAccountV1,
    registry: AuthenticatedSeriesRegistryAccountV1,
    artifacts: &AuthenticatedSeriesArtifactsV1,
    source_release: AuthenticatedSourceReleaseV1,
    clock_account: &AccountInfo<'_>,
    expected_ordinal: u32,
) -> Outcome<u32> {
    require(registry.activation_consumed(), ClutchError::Replay)?;
    let series_plan_id = artifacts
        .series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        registry.value().series_plan_id == series_plan_id
            && registry.value().funding_terms_id
                == artifacts
                    .funding_terms
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && funding.value().state.series_plan_id == series_plan_id
            && funding.value().state.funding_quote_id
                == artifacts
                    .quote
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ClutchError::MismatchedState,
    )?;
    let authority = authenticate_series_clock(artifacts, source_release, clock_account)?;
    let mut next = funding.value().state;
    let ordinal = next
        .lapse(&authority, &artifacts.series, &artifacts.quote)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(ordinal == expected_ordinal, ClutchError::Replay)?;
    write_series_funding_state(funding_account, funding, next)?;
    Ok(ordinal)
}

/// Advance exactly the current ordinal from a private Source runtime receipt
/// plus the typed cross-component authority that authenticated the remaining
/// Market, Recovery, liquidity, wrapper, Clock, and funding transitions.
///
/// The Source receipt is compared to every field of the canonical 184-byte
/// occurrence record. No caller-supplied present/absent bitmap or debit amount
/// crosses this boundary.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn advance_series_occurrence_from_source<A: AuthenticatedSeriesFundingAuthorityV1 + ?Sized>(
    program_id: &Pubkey,
    funding_account: &AccountInfo<'_>,
    market_core_lamport_vault: &AccountInfo<'_>,
    funding: AuthenticatedSeriesFundingAccountV1,
    registry: AuthenticatedSeriesRegistryAccountV1,
    artifacts: &AuthenticatedSeriesArtifactsV1,
    authority: &A,
    occurrence: &CompiledSourceOccurrenceV3,
    source_receipt: OccurrenceSourceReceiptV1,
    expected_source_occurrence_id: SourceOccurrenceV1Id,
    expected_market_instance_id: clutch_product_series::MarketInstanceV2Id,
    expected_ordinal: u32,
) -> Outcome<(
    clutch_product_series::DebitProjectionV1,
    Option<SeriesMarketCoreFundingReceiptV1>,
)> {
    require(registry.activation_consumed(), ClutchError::Replay)?;
    let series_plan_id = artifacts
        .series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let occurrence_id = occurrence
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        registry.value().series_plan_id == series_plan_id
            && registry.value().funding_terms_id
                == artifacts
                    .funding_terms
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && funding.value().state.series_plan_id == series_plan_id
            && funding.value().state.funding_quote_id
                == artifacts
                    .quote
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && funding.value().state.next_ordinal == expected_ordinal
            && occurrence_id == expected_source_occurrence_id
            && occurrence.series_plan_id == series_plan_id
            && occurrence.ordinal == expected_ordinal
            && occurrence.market_instance_id == expected_market_instance_id
            && occurrence.attachment_plan_id == artifacts.series.attachment_plan_id
            && source_receipt.occurrence_record_id().bytes() == occurrence_id.bytes()
            && source_receipt.series_plan_id().bytes() == series_plan_id.bytes()
            && source_receipt.ordinal() == expected_ordinal
            && source_receipt.market_instance_id().bytes() == expected_market_instance_id.bytes()
            && source_receipt.attachment_plan_id().bytes() == occurrence.attachment_plan_id.bytes()
            && source_receipt.source_plane_contract_id().bytes()
                == artifacts.template.source_plane_contract_id.bytes()
            && source_receipt.source_spec_id().bytes() == artifacts.template.source_spec_id.bytes()
            && source_receipt.window_id().bytes() == occurrence.source_window_id.bytes()
            && source_receipt.statistic_key_id().bytes() == occurrence.statistic_key_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let mut next = funding.value().state;
    let (ordinal, debit) = next
        .advance_created(
            authority,
            &artifacts.series,
            &artifacts.recovery,
            &artifacts.attachment,
            &artifacts.quote,
            occurrence,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(ordinal == expected_ordinal, ClutchError::Replay)?;
    let market_core_receipt = authenticate_market_core_funding_receipt(
        program_id,
        funding,
        artifacts,
        expected_ordinal,
        expected_market_instance_id,
        debit,
        market_core_lamport_vault,
    )?;
    write_series_funding_state(funding_account, funding, next)?;
    Ok((debit, market_core_receipt))
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
fn authenticate_market_core_funding_receipt(
    program_id: &Pubkey,
    funding: AuthenticatedSeriesFundingAccountV1,
    artifacts: &AuthenticatedSeriesArtifactsV1,
    ordinal: u32,
    market_instance_id: clutch_product_series::MarketInstanceV2Id,
    debit: clutch_product_series::DebitProjectionV1,
    market_core_lamport_vault: &AccountInfo<'_>,
) -> Outcome<Option<SeriesMarketCoreFundingReceiptV1>> {
    if debit.market_core == ComponentDebitV1::ZERO {
        return Ok(None);
    }
    require(
        debit.market_core == artifacts.quote.market_core,
        ClutchError::MismatchedState,
    )?;
    let series_plan_id = artifacts
        .series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_quote_id = artifacts
        .quote
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_lamport_vault_metadata(
        program_id,
        series_plan_id,
        SeriesFundingComponentV1::MarketCore.index(),
        market_core_lamport_vault,
    )?;
    let vault_balance_before = accounted_custody_balances(&funding.value().state)?.lamports
        [SeriesFundingComponentV1::MarketCore.index()];
    require(
        market_core_lamport_vault.lamports() == vault_balance_before,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let vault_balance_after = sub(vault_balance_before, debit.market_core.lamports)?;
    let failure_account_rent = add(
        artifacts.quote.failure_root_rent_principal_lamports,
        artifacts
            .quote
            .failure_replay_tombstone_rent_principal_lamports,
    )?;
    let vault_balance_after_failure_accounts = sub(vault_balance_before, failure_account_rent)?;
    let generation = SERIES_ACTIVATION_GENERATION_V1;
    Ok(Some(mint_series_market_core_funding_receipt_v1(
        series_plan_id,
        ordinal,
        market_instance_id,
        funding_quote_id,
        funding.account(),
        *market_core_lamport_vault.key,
        artifacts.funding_terms.lamport_principal_refund,
        artifacts.funding_terms.neutral_lamport_sink,
        generation,
        debit.market_core.lamports,
        artifacts.quote.failure_root_rent_principal_lamports,
        artifacts
            .quote
            .failure_replay_tombstone_rent_principal_lamports,
        vault_balance_before,
        vault_balance_after_failure_accounts,
        vault_balance_after,
    )))
}

fn require_lamport_vault_metadata(
    program_id: &Pubkey,
    series: SeriesPlanV5Id,
    index: usize,
    account: &AccountInfo<'_>,
) -> Outcome<()> {
    let component = component_from_index(index)?;
    require(account.is_writable, ClutchError::NotWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(
        *account.owner == SYSTEM_PROGRAM_ID && account.data_is_empty(),
        ClutchError::WrongProgramOwner,
    )?;
    expect_pda(
        account.key,
        seeds::series_lamport_vault_pda(program_id, &series.bytes(), component.byte()),
        None,
    )?;
    Ok(())
}

fn require_lamport_vault_metadata_v2(
    program_id: &Pubkey,
    series: SeriesPlanV5Id,
    component: SeriesFundingComponentV2,
    account: &AccountInfo<'_>,
) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(
        *account.owner == SYSTEM_PROGRAM_ID && account.data_is_empty(),
        ClutchError::WrongProgramOwner,
    )?;
    let byte = match component {
        SeriesFundingComponentV2::MarketCore => 0,
        SeriesFundingComponentV2::SeriesAdmission => 1,
        SeriesFundingComponentV2::RecoveryReserve => 2,
        SeriesFundingComponentV2::SourceWork => 3,
        SeriesFundingComponentV2::LiquidityFacility => 4,
        SeriesFundingComponentV2::WrapperSet => 5,
    };
    expect_pda(
        account.key,
        seeds::series_lamport_vault_pda(program_id, &series.bytes(), byte),
        None,
    )?;
    Ok(())
}

/// Authenticate all five lamport custody PDAs and exact equality with the
/// state-owned balance. A surplus must first be observed as donation; a
/// shortfall always refuses.
#[cfg(test)]
pub fn authenticate_lamport_custody(
    program_id: &Pubkey,
    series: SeriesPlanV5Id,
    vaults: &[AccountInfo<'_>],
    expected: &SeriesCustodyBalancesV1,
) -> Outcome<()> {
    require(
        vaults.len() == SERIES_CUSTODY_COUNT_V1,
        ClutchError::AccountCount,
    )?;
    let mut index = 0usize;
    while index < SERIES_CUSTODY_COUNT_V1 {
        require_lamport_vault_metadata(program_id, series, index, &vaults[index])?;
        require(
            vaults[index].lamports() == expected.lamports[index],
            ClutchError::MismatchedState,
        )?;
        index += 1;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
#[cfg(test)]
struct AuthenticatedCustodyDonationV1 {
    series_plan_id: SeriesPlanV5Id,
    funding_quote_id: clutch_product_series::SeriesFundingQuoteId,
    component: SeriesFundingComponentV1,
    amount: ComponentDebitV1,
}

#[cfg(test)]
impl AuthenticatedSeriesFundingAuthorityV1 for AuthenticatedCustodyDonationV1 {
    fn authenticate_donation(
        &self,
        state: &SeriesFundingStateV1,
        quote: &SeriesFundingQuoteV1,
        component: SeriesFundingComponentV1,
        amount: ComponentDebitV1,
    ) -> clutch_product_series::Result<()> {
        if state.series_plan_id != self.series_plan_id
            || quote.id()? != self.funding_quote_id
            || component != self.component
            || amount != self.amount
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

/// Observe one positive lamport-vault surplus and apply exactly that donation
/// to a copy of the pure funding state.
///
/// The private authority value is constructible only after exact PDA/owner/data
/// authentication and an actual-balance delta. It cannot authorize activation,
/// Clock, or occurrence fulfillment through the trait's default-deny methods.
#[cfg(test)]
pub fn observe_lamport_donation(
    program_id: &Pubkey,
    state: SeriesFundingStateV1,
    quote: &SeriesFundingQuoteV1,
    component: SeriesFundingComponentV1,
    vault: &AccountInfo<'_>,
) -> Outcome<SeriesFundingStateV1> {
    state
        .validate_against_quote(quote)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_lamport_vault_metadata(program_id, state.series_plan_id, component.index(), vault)?;
    let accounted = accounted_custody_balances(&state)?.lamports[component.index()];
    let actual = vault.lamports();
    let delta = sub(actual, accounted)?;
    require(delta != 0, ClutchError::MismatchedState)?;
    let amount = ComponentDebitV1 {
        lamports: delta,
        collateral_atoms: 0,
    };
    let authority = AuthenticatedCustodyDonationV1 {
        series_plan_id: state.series_plan_id,
        funding_quote_id: state.funding_quote_id,
        component,
        amount,
    };
    let mut next = state;
    next.add_donation(&authority, quote, component, amount)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        accounted_custody_balances(&next)?.lamports[component.index()] == actual,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    Ok(next)
}

/// Observe one exact collateral-vault surplus through the Realm-selected token
/// account decoder and add only that raw-atom delta as donation residue.
#[cfg(test)]
pub fn observe_series_collateral_donation(
    program_id: &Pubkey,
    authenticated: AuthenticatedSeriesCollateralFundingV1,
    artifacts: &AuthenticatedSeriesArtifactsV1,
    component: SeriesFundingComponentV1,
    mint: &AccountInfo<'_>,
    vault: &AccountInfo<'_>,
    canonical_authority: &AccountInfo<'_>,
) -> Outcome<SeriesFundingStateV1> {
    let funding = authenticated.funding_account();
    let state = funding.value().state;
    require(
        funding.account()
            == Pubkey::new_from_array(authenticated.join().funding_state_account.bytes())
            && state.series_plan_id.bytes() == authenticated.join().series_plan.bytes()
            && state.funding_quote_id
                == artifacts
                    .quote
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ClutchError::MismatchedState,
    )?;
    state
        .validate_against_quote(&artifacts.quote)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    admit_realm_collateral_mint_v2(
        authenticated.bound(),
        runtime_account_view(mint, &mint_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let binding = series_collateral_binding(
        program_id,
        authenticated.bound(),
        state.series_plan_id,
        component,
        vault,
        canonical_authority,
    )?;
    let vault_data = vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let observation = admit_realm_collateral_account_v2(
        authenticated.bound(),
        runtime_account_view(vault, &vault_data),
        TokenAccountRoleV2::SegregatedVault(binding),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let accounted = accounted_custody_balances(&state)?.collateral_atoms[component.index()];
    let delta = sub(observation.amount_atoms, accounted)?;
    require(delta != 0, ClutchError::MismatchedState)?;
    let amount = ComponentDebitV1 {
        lamports: 0,
        collateral_atoms: delta,
    };
    let authority = AuthenticatedCustodyDonationV1 {
        series_plan_id: state.series_plan_id,
        funding_quote_id: state.funding_quote_id,
        component,
        amount,
    };
    let mut next = state;
    next.add_donation(&authority, &artifacts.quote, component, amount)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        accounted_custody_balances(&next)?.collateral_atoms[component.index()]
            == observation.amount_atoms,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    Ok(next)
}

/// Fund all five lamport custody PDAs from one authenticated payer with exact
/// component post-deltas. Existing balances are preserved as separately
/// observed activation donations; they never reduce the payer debit.
#[cfg(test)]
pub fn fund_lamport_custody<'a>(
    program_id: &Pubkey,
    series: SeriesPlanV5Id,
    payer: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    vaults: &[AccountInfo<'a>],
    principal: [ComponentDebitV1; SERIES_CUSTODY_COUNT_V1],
) -> Outcome<()> {
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require_system_program(system_program)?;
    require(
        vaults.len() == SERIES_CUSTODY_COUNT_V1,
        ClutchError::AccountCount,
    )?;
    let mut index = 0usize;
    while index < SERIES_CUSTODY_COUNT_V1 {
        require_lamport_vault_metadata(program_id, series, index, &vaults[index])?;
        require(payer.key != vaults[index].key, ClutchError::AccountAlias)?;
        index += 1;
    }
    index = 0;
    while index < SERIES_CUSTODY_COUNT_V1 {
        let amount = principal[index].lamports;
        let before = vaults[index].lamports();
        let expected_after = add(before, amount)?;
        if amount != 0 {
            let transfer = Instruction::new_with_bytes(
                SYSTEM_PROGRAM_ID,
                &transfer_data(amount),
                vec![
                    AccountMeta::new(*payer.key, true),
                    AccountMeta::new(*vaults[index].key, false),
                ],
            );
            invoke(
                &transfer,
                &[payer.clone(), vaults[index].clone(), system_program.clone()],
            )
            .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
        }
        require(
            vaults[index].lamports() == expected_after,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
        index += 1;
    }
    Ok(())
}

/// Transfer an exact amount out of one zero-data component vault, signed only
/// by its canonical PDA, and verify both source and destination deltas.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn transfer_from_lamport_custody<'a>(
    program_id: &Pubkey,
    series: SeriesPlanV5Id,
    component: SeriesFundingComponentV1,
    vault: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    amount: u64,
) -> Outcome<()> {
    let index = component.index();
    require_lamport_vault_metadata(program_id, series, index, vault)?;
    require(destination.is_writable, ClutchError::NotWritable)?;
    require(!destination.executable, ClutchError::ExecutableAccount)?;
    require(vault.key != destination.key, ClutchError::AccountAlias)?;
    require_system_program(system_program)?;
    let source_before = vault.lamports();
    let destination_before = destination.lamports();
    let source_after = sub(source_before, amount)?;
    let destination_after = add(destination_before, amount)?;
    if amount != 0 {
        let transfer = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &transfer_data(amount),
            vec![
                AccountMeta::new(*vault.key, true),
                AccountMeta::new(*destination.key, false),
            ],
        );
        let component_seed = [component.byte()];
        let (_, bump) =
            seeds::series_lamport_vault_pda(program_id, &series.bytes(), component.byte());
        let series_seed = series.bytes();
        let bump_seed = [bump];
        invoke_signed(
            &transfer,
            &[vault.clone(), destination.clone(), system_program.clone()],
            &[&[
                seeds::SEED_SERIES_LAMPORT_VAULT_V1,
                &series_seed,
                &component_seed,
                &bump_seed,
            ]],
        )
        .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    }
    require(
        vault.lamports() == source_after && destination.lamports() == destination_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )
}

/// Execute the complete five-component terminal disposition and close only the
/// mutable funding root. The consumed registry account remains as the
/// permanent one-shot replay anchor; this ABI never deletes or resets it.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn close_authenticated_series_funding<'a>(
    program_id: &Pubkey,
    authenticated: AuthenticatedSeriesCollateralTerminalV1,
    funding_account: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    collateral_vaults: &[AccountInfo<'a>],
    lamport_vaults: &[AccountInfo<'a>],
    collateral_principal_refund: &AccountInfo<'a>,
    neutral_collateral_disposition: &AccountInfo<'a>,
    collateral_authority: &AccountInfo<'a>,
    payer_lamport_refund: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
) -> Outcome<ContentId> {
    require(
        collateral_vaults.len() == SERIES_CUSTODY_COUNT_V1
            && lamport_vaults.len() == SERIES_CUSTODY_COUNT_V1,
        ClutchError::AccountCount,
    )?;
    let funding_receipt = authenticated.funding().funding_account();
    require(
        funding_receipt.account() == *funding_account.key,
        ClutchError::MismatchedState,
    )?;
    let terminal_receipt = ContentId::from_bytes(authenticated.join().terminal_receipt.bytes());
    let mut index = 0usize;
    while index < SERIES_CUSTODY_COUNT_V1 {
        let component = component_from_index(index)?;
        refund_series_collateral_principal(
            program_id,
            authenticated,
            component,
            mint,
            &collateral_vaults[index],
            collateral_principal_refund,
            collateral_authority,
            token_program,
        )?;
        dispose_series_collateral_donation(
            program_id,
            authenticated,
            component,
            mint,
            &collateral_vaults[index],
            neutral_collateral_disposition,
            collateral_authority,
            token_program,
        )?;
        let rent_receipt = close_series_collateral_vault(
            program_id,
            authenticated,
            component,
            &collateral_vaults[index],
            &lamport_vaults[index],
            collateral_authority,
            payer_lamport_refund,
            neutral_lamport_sink,
            token_program,
            system_program,
        )?;
        require(
            rent_receipt.terminal_receipt.bytes() == terminal_receipt.bytes()
                && rent_receipt.series_plan.bytes()
                    == authenticated.funding().join().series_plan.bytes()
                && rent_receipt.component == u16::from(component.byte()) + 1,
            ClutchError::MismatchedState,
        )?;
        let lamport_receipt = settle_series_lamport_component(
            program_id,
            authenticated,
            component,
            &lamport_vaults[index],
            payer_lamport_refund,
            neutral_lamport_sink,
            system_program,
        )?;
        require(
            lamport_receipt.terminal_receipt == terminal_receipt
                && lamport_receipt.component == component,
            ClutchError::MismatchedState,
        )?;
        index += 1;
    }
    close_series_program_account(
        program_id,
        funding_account,
        payer_lamport_refund,
        neutral_lamport_sink,
        funding_receipt.value().rent_principal_lamports,
    )?;
    Ok(terminal_receipt)
}

/// Close one program-owned Series account with payer rent and donation surplus
/// kept separate. The exact stored principal goes to the FundingTerms V2
/// lamport refund account; every other lamport goes to its distinct neutral
/// sink. Callers must authenticate the account codec/PDA and closed lifecycle
/// before entering this mechanical primitive.
#[cfg(test)]
pub fn close_series_program_account(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    rent_principal_lamports: u64,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(account.is_writable, ClutchError::NotWritable)?;
    require(principal_refund.is_writable, ClutchError::NotWritable)?;
    require(neutral_sink.is_writable, ClutchError::NotWritable)?;
    require(
        !account.executable && !principal_refund.executable && !neutral_sink.executable,
        ClutchError::ExecutableAccount,
    )?;
    require(
        account.key != principal_refund.key
            && account.key != neutral_sink.key
            && principal_refund.key != neutral_sink.key,
        ClutchError::AccountAlias,
    )?;
    let held = account.lamports();
    let donation = sub(held, rent_principal_lamports)?;
    let refund_before = principal_refund.lamports();
    let sink_before = neutral_sink.lamports();
    let refund_after = add(refund_before, rent_principal_lamports)?;
    let sink_after = add(sink_before, donation)?;
    credit_lamports(principal_refund, rent_principal_lamports)?;
    credit_lamports(neutral_sink, donation)?;
    debit_lamports(account, held)?;
    require(
        account.lamports() == 0
            && principal_refund.lamports() == refund_after
            && neutral_sink.lamports() == sink_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.fill(0);
    Ok(())
}

#[cfg(test)]
mod collateral_activation_v2_adversarial_tests {
    use super::*;

    fn debits() -> [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2] {
        [
            ComponentDebitV1 {
                lamports: 11,
                collateral_atoms: 12,
            },
            ComponentDebitV1 {
                lamports: 21,
                collateral_atoms: 0,
            },
            ComponentDebitV1 {
                lamports: 31,
                collateral_atoms: 32,
            },
            ComponentDebitV1 {
                lamports: 41,
                collateral_atoms: 42,
            },
            ComponentDebitV1 {
                lamports: 51,
                collateral_atoms: 52,
            },
            ComponentDebitV1 {
                lamports: 61,
                collateral_atoms: 62,
            },
        ]
    }

    #[test]
    fn current_vault_mapping_is_exhaustive_and_excludes_admission() {
        let expected = [
            (SeriesFundingComponentV2::MarketCore, 0u8, 1u16),
            (SeriesFundingComponentV2::RecoveryReserve, 1u8, 2u16),
            (SeriesFundingComponentV2::SourceWork, 2u8, 3u16),
            (SeriesFundingComponentV2::LiquidityFacility, 3u8, 4u16),
            (SeriesFundingComponentV2::WrapperSet, 4u8, 5u16),
        ];
        let mut index = 0usize;
        while index < expected.len() {
            let coordinate = series_collateral_vault_coordinate_v2(index).unwrap();
            assert_eq!(
                (coordinate.component, coordinate.seed, coordinate.compartment),
                expected[index]
            );
            assert_eq!(coordinate.component.collateral_vault_index(), Some(index));
            index += 1;
        }
        assert!(series_collateral_vault_coordinate_v2(expected.len()).is_err());
        assert_eq!(
            SeriesFundingComponentV2::SeriesAdmission.collateral_vault_index(),
            None
        );
    }

    #[test]
    fn funding_commitment_binds_every_principal_and_donation_unit() {
        let principal = debits();
        let donations = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        let expected = series_collateral_funding_commitment_v2(&principal, &donations).unwrap();

        let mut changed_principal = principal;
        changed_principal[SeriesFundingComponentV2::LiquidityFacility.index()].collateral_atoms += 1;
        assert_ne!(
            series_collateral_funding_commitment_v2(&changed_principal, &donations).unwrap(),
            expected
        );

        let mut changed_donation = donations;
        changed_donation[SeriesFundingComponentV2::WrapperSet.index()].lamports = 1;
        assert_ne!(
            series_collateral_funding_commitment_v2(&principal, &changed_donation).unwrap(),
            expected
        );

        let mut forbidden_admission_collateral = donations;
        forbidden_admission_collateral[SeriesFundingComponentV2::SeriesAdmission.index()]
            .collateral_atoms = 1;
        assert_ne!(
            series_collateral_funding_commitment_v2(
                &principal,
                &forbidden_admission_collateral
            )
            .unwrap(),
            expected
        );
    }

    #[test]
    fn vault_rent_uses_actual_release_length_and_exact_postbalance() {
        let rent = RentParameters {
            lamports_per_byte_year: 3_480,
            exemption_threshold: 2.0,
        };
        let release_selected_len = 165usize;
        let principal = rent.minimum_balance(release_selected_len).unwrap();
        assert_eq!(
            require_series_collateral_vault_rent_poststate_v2(
                &rent,
                release_selected_len,
                release_selected_len,
                principal,
            )
            .unwrap(),
            principal
        );
        assert!(require_series_collateral_vault_rent_poststate_v2(
            &rent,
            release_selected_len,
            release_selected_len + 1,
            principal,
        )
        .is_err());
        assert!(require_series_collateral_vault_rent_poststate_v2(
            &rent,
            release_selected_len,
            release_selected_len,
            principal - 1,
        )
        .is_err());
        assert!(require_series_collateral_vault_rent_poststate_v2(
            &rent,
            release_selected_len,
            release_selected_len,
            principal + 1,
        )
        .is_err());
    }

    #[test]
    fn vault_and_transfer_poststate_order_cannot_be_substituted() {
        let first = ContentId::from_bytes([1u8; 32]);
        let second = ContentId::from_bytes([2u8; 32]);
        let values = [first, second, ContentId::ZERO, ContentId::ZERO, ContentId::ZERO];
        let encoded = flatten_series_collateral_ids_v2(&values).unwrap();
        let swapped = [second, first, ContentId::ZERO, ContentId::ZERO, ContentId::ZERO];
        let swapped_encoded = flatten_series_collateral_ids_v2(&swapped).unwrap();
        assert_ne!(encoded, swapped_encoded);
        assert_eq!(&encoded[0..32], &first.bytes());
        assert_eq!(&encoded[32..64], &second.bytes());
    }
}
