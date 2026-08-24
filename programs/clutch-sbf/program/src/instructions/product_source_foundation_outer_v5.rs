//! Callable current Product-bound Source foundation/publication outer.
//!
//! Product action14 passes its move-only FundingV5 reservation and private
//! founder preauthorization into this module. The outer hostile-authenticates
//! the complete ReleaseV2/receiver/BundleV7 route, the Source schedule, and a
//! separately sealed content-addressed runtime-policy proposal before moving
//! any lamports. It then capitalizes and publishes the complete Source graph
//! in the same instruction and returns the non-Copy founder receipt directly.

use crate::accounts::{expect_pda, require, require_count, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::product_series_current::AuthenticatedRegistryCapabilityV5;
use crate::instructions::product_series_funding_v5_current::
    AuthenticatedProductSeriesFundingReservationV5;
use crate::instructions::product_source_current::{
    authenticate_compiled_product_series_bundle_v7, authenticate_source_product_route_v4,
    authenticate_source_semantic_publication_v2, AuthenticatedSeriesSourceArtifactsV6,
    AuthenticatedSourceProductRouteV4,
};
use crate::instructions::source_occurrence_foundation_v1::{
    capitalize_source_work_v3, publish_pre_root_source_occurrence_v3,
    AuthenticatedPreRootSourceOccurrenceV3, AuthenticatedSourceOccurrenceFoundationAuthorityV3,
};
use crate::seeds;
use crate::source_plane_v3::{
    authenticate_receiver_route, authenticate_release, authenticate_route,
};
use crate::source_plane_v3_actions::authenticate_source_work_schedule_artifact;
use clutch_liveness::runtime_v1::{
    RuntimeCompartmentKindV1, RuntimeLivenessPolicyV1, RUNTIME_LIVENESS_POLICY_BYTES_V1,
};
use clutch_product_series::ContentId;
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_source_plane_v3_runtime::{
    source_runtime_liveness_policy_id_v1, AuthenticatedSourceRouteV1, RuntimeKey,
    SourceWorkScheduleBindingV1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

pub(crate) const PRODUCT_SOURCE_FOUNDATION_ACCOUNT_COUNT_V5: usize = 26;

pub(crate) const IX_SOURCE_FOUNDATION_BUNDLE_V7: usize = 0;
pub(crate) const IX_SOURCE_FOUNDATION_FUNDING_V5: usize = 1;
pub(crate) const IX_SOURCE_FOUNDATION_RELEASE: usize = 2;
pub(crate) const IX_SOURCE_FOUNDATION_ADAPTER_PROGRAM: usize = 3;
pub(crate) const IX_SOURCE_FOUNDATION_ADAPTER_PROGRAMDATA: usize = 4;
pub(crate) const IX_SOURCE_FOUNDATION_PARSER_PROGRAM: usize = 5;
pub(crate) const IX_SOURCE_FOUNDATION_PARSER_PROGRAMDATA: usize = 6;
pub(crate) const IX_SOURCE_FOUNDATION_PARSER_CONFIG: usize = 7;
pub(crate) const IX_SOURCE_FOUNDATION_SPEC: usize = 8;
pub(crate) const IX_SOURCE_FOUNDATION_RECEIVER_PROGRAM: usize = 9;
pub(crate) const IX_SOURCE_FOUNDATION_RECEIVER_PROGRAMDATA: usize = 10;
pub(crate) const IX_SOURCE_FOUNDATION_RECEIVER_CONFIG: usize = 11;
pub(crate) const IX_SOURCE_FOUNDATION_WORK_SCHEDULE: usize = 12;
pub(crate) const IX_SOURCE_FOUNDATION_LIVENESS_POLICY_ARTIFACT: usize = 13;
pub(crate) const IX_SOURCE_FOUNDATION_WORK_VAULT: usize = 14;
pub(crate) const IX_SOURCE_FOUNDATION_CUSTODY: usize = 15;
pub(crate) const IX_SOURCE_FOUNDATION_LIVENESS_POLICY_TARGET: usize = 16;
pub(crate) const IX_SOURCE_FOUNDATION_COMPARTMENT_TARGET: usize = 17;
pub(crate) const IX_SOURCE_FOUNDATION_OCCURRENCE_TARGET: usize = 18;
pub(crate) const IX_SOURCE_FOUNDATION_WINDOW_TARGET: usize = 19;
pub(crate) const IX_SOURCE_FOUNDATION_SUMMARY_TARGET: usize = 20;
pub(crate) const IX_SOURCE_FOUNDATION_STATISTIC_KEY_TARGET: usize = 21;
pub(crate) const IX_SOURCE_FOUNDATION_RESULT_LINEAGE_TARGET: usize = 22;
pub(crate) const IX_SOURCE_FOUNDATION_GENERATION_REQUEST_TARGET: usize = 23;
pub(crate) const IX_SOURCE_FOUNDATION_SYSTEM_PROGRAM: usize = 24;
pub(crate) const IX_SOURCE_FOUNDATION_RENT_SYSVAR: usize = 25;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductSourceFoundationAccountRoleV5 {
    BundleV7Artifact,
    FundingV5,
    SourceRelease,
    AdapterProgram,
    AdapterProgramData,
    ParserProgram,
    ParserProgramData,
    ParserConfig,
    SourceSpec,
    ReceiverProgram,
    ReceiverProgramData,
    ReceiverConfig,
    SourceWorkSchedule,
    RuntimeLivenessPolicyArtifact,
    SourceWorkVault,
    SourceFundingCustody,
    RuntimeLivenessPolicyTarget,
    SourceCompartmentTarget,
    SourceOccurrenceTarget,
    SourceWindowTarget,
    SourceSummaryTarget,
    SourceStatisticKeyTarget,
    SourceResultLineageTarget,
    SourceGenerationRequestTarget,
    SystemProgram,
    RentSysvar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductSourceFoundationAccountMetaV5 {
    pub(crate) role: ProductSourceFoundationAccountRoleV5,
    pub(crate) writable: bool,
    pub(crate) signer: bool,
}

const fn meta(
    role: ProductSourceFoundationAccountRoleV5,
    writable: bool,
) -> ProductSourceFoundationAccountMetaV5 {
    ProductSourceFoundationAccountMetaV5 {
        role,
        writable,
        signer: false,
    }
}

pub(crate) const PRODUCT_SOURCE_FOUNDATION_ACCOUNT_METAS_V5:
    [ProductSourceFoundationAccountMetaV5; PRODUCT_SOURCE_FOUNDATION_ACCOUNT_COUNT_V5] = [
        meta(ProductSourceFoundationAccountRoleV5::BundleV7Artifact, false),
        meta(ProductSourceFoundationAccountRoleV5::FundingV5, true),
        meta(ProductSourceFoundationAccountRoleV5::SourceRelease, false),
        meta(ProductSourceFoundationAccountRoleV5::AdapterProgram, false),
        meta(ProductSourceFoundationAccountRoleV5::AdapterProgramData, false),
        meta(ProductSourceFoundationAccountRoleV5::ParserProgram, false),
        meta(ProductSourceFoundationAccountRoleV5::ParserProgramData, false),
        meta(ProductSourceFoundationAccountRoleV5::ParserConfig, false),
        meta(ProductSourceFoundationAccountRoleV5::SourceSpec, false),
        meta(ProductSourceFoundationAccountRoleV5::ReceiverProgram, false),
        meta(ProductSourceFoundationAccountRoleV5::ReceiverProgramData, false),
        meta(ProductSourceFoundationAccountRoleV5::ReceiverConfig, false),
        meta(ProductSourceFoundationAccountRoleV5::SourceWorkSchedule, false),
        meta(
            ProductSourceFoundationAccountRoleV5::RuntimeLivenessPolicyArtifact,
            false,
        ),
        meta(ProductSourceFoundationAccountRoleV5::SourceWorkVault, true),
        meta(ProductSourceFoundationAccountRoleV5::SourceFundingCustody, true),
        meta(
            ProductSourceFoundationAccountRoleV5::RuntimeLivenessPolicyTarget,
            true,
        ),
        meta(ProductSourceFoundationAccountRoleV5::SourceCompartmentTarget, true),
        meta(ProductSourceFoundationAccountRoleV5::SourceOccurrenceTarget, true),
        meta(ProductSourceFoundationAccountRoleV5::SourceWindowTarget, true),
        meta(ProductSourceFoundationAccountRoleV5::SourceSummaryTarget, true),
        meta(ProductSourceFoundationAccountRoleV5::SourceStatisticKeyTarget, true),
        meta(ProductSourceFoundationAccountRoleV5::SourceResultLineageTarget, true),
        meta(
            ProductSourceFoundationAccountRoleV5::SourceGenerationRequestTarget,
            true,
        ),
        meta(ProductSourceFoundationAccountRoleV5::SystemProgram, false),
        meta(ProductSourceFoundationAccountRoleV5::RentSysvar, false),
    ];

fn require_product_source_foundation_account_contract_v5(
    accounts: &[AccountInfo<'_>],
) -> Outcome<()> {
    require_count(accounts, PRODUCT_SOURCE_FOUNDATION_ACCOUNT_COUNT_V5)?;
    require_distinct(accounts)?;
    let mut index = 0usize;
    while index < PRODUCT_SOURCE_FOUNDATION_ACCOUNT_COUNT_V5 {
        let observed = &accounts[index];
        let expected = PRODUCT_SOURCE_FOUNDATION_ACCOUNT_METAS_V5[index];
        require(
            observed.key != &Pubkey::default()
                && observed.is_writable == expected.writable
                && observed.is_signer == expected.signer,
            ClutchError::MismatchedState,
        )?;
        index += 1;
    }
    Ok(())
}

/// Move-only authentication of the sealed policy proposal artifact. This is
/// distinct from the initially empty writable Source policy PDA; it is the
/// immutable semantic owner from which that target is created.
#[derive(Debug)]
pub(crate) struct AuthenticatedSourceRuntimeLivenessPolicyArtifactV1 {
    account: Pubkey,
    data_id: ContentId,
    policy_id: ContentId,
    policy: RuntimeLivenessPolicyV1,
}

impl AuthenticatedSourceRuntimeLivenessPolicyArtifactV1 {
    pub(crate) const fn account(&self) -> Pubkey { self.account }
    pub(crate) const fn data_id(&self) -> ContentId { self.data_id }
    pub(crate) const fn policy_id(&self) -> ContentId { self.policy_id }

    fn into_policy(self) -> RuntimeLivenessPolicyV1 { self.policy }
}

fn authenticate_source_runtime_liveness_policy_artifact_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    route: AuthenticatedSourceRouteV1,
    product_route: AuthenticatedSourceProductRouteV4,
    schedule: SourceWorkScheduleBindingV1,
) -> Outcome<AuthenticatedSourceRuntimeLivenessPolicyArtifactV1> {
    require(
        account.owner == program_id
            && !account.is_signer
            && !account.is_writable
            && !account.executable
            && account.data_len() == RUNTIME_LIVENESS_POLICY_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        account.key,
        seeds::product_artifact_pda(
            program_id,
            ArtifactKind::RuntimeLivenessPolicyV1.byte(),
            &route.liveness_policy_id().bytes(),
        ),
        None,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let policy = RuntimeLivenessPolicyV1::decode(&data)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let data_id = ContentId::from_bytes(solana_sha256_hasher::hashv(&[data.as_ref()]).to_bytes());
    drop(data);
    let policy_id = source_runtime_liveness_policy_id_v1(policy)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let source = policy.compartment(RuntimeCompartmentKindV1::Source);
    schedule
        .validate_against(route)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        policy_id == route.liveness_policy_id()
            && policy.policy_id.bytes() == policy_id.bytes()
            && policy.realm_id.bytes() == product_route.realm_id().bytes()
            && policy.neutral_sink.bytes() == route.neutral_sink().bytes()
            && source.quote_schedule_id.bytes()
                == schedule.source_work_schedule_id().bytes()
            && source.receipt_program_id.bytes() == program_id.to_bytes()
            && source.maximum_calls == schedule.maximum_calls()
            && source.maximum_lamports_per_call == schedule.maximum_lamports_per_call()
            && source.work_capital_lamports == schedule.work_capital_lamports()
            && source.account_rent_principal_lamports == schedule.rent_principal_lamports(),
        ClutchError::MismatchedState,
    )?;
    let terminal_calls = schedule.terminal_path_calls();
    let terminal_work = schedule.terminal_path_work_lamports();
    let mut path = 0usize;
    while path < policy.terminal_paths.len() {
        require(
            policy.terminal_paths[path].calls_for(RuntimeCompartmentKindV1::Source)
                == terminal_calls[path]
                && policy.terminal_paths[path]
                    .work_lamports_for(RuntimeCompartmentKindV1::Source)
                    == terminal_work[path],
            ClutchError::MismatchedState,
        )?;
        path += 1;
    }
    Ok(AuthenticatedSourceRuntimeLivenessPolicyArtifactV1 {
        account: *account.key,
        data_id,
        policy_id: ContentId::from_bytes(policy_id.bytes()),
        policy,
    })
}

/// Perform the sole current Product-bound Source publication cut. Every
/// semantic body is hostile-derived from accounts; the instruction payload
/// contributes only the Series ordinal already constrained by Product's
/// reservation and compiler graph.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn compose_product_source_occurrence_foundation_v5<
    A: AuthenticatedSourceOccurrenceFoundationAuthorityV3 + ?Sized,
>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    registry: &AuthenticatedRegistryCapabilityV5,
    artifacts: &AuthenticatedSeriesSourceArtifactsV6,
    funding_reservation: AuthenticatedProductSeriesFundingReservationV5,
    authority: &A,
    ordinal: u32,
) -> Outcome<AuthenticatedPreRootSourceOccurrenceV3> {
    require_product_source_foundation_account_contract_v5(accounts)?;
    let source_release = authenticate_release(
        program_id,
        &accounts[IX_SOURCE_FOUNDATION_RELEASE],
    )
    .map_err(Refusal::from)?;
    let route = authenticate_route(
        program_id,
        &accounts[IX_SOURCE_FOUNDATION_RELEASE],
        &accounts[IX_SOURCE_FOUNDATION_ADAPTER_PROGRAM],
        &accounts[IX_SOURCE_FOUNDATION_ADAPTER_PROGRAMDATA],
        &accounts[IX_SOURCE_FOUNDATION_PARSER_PROGRAM],
        &accounts[IX_SOURCE_FOUNDATION_PARSER_PROGRAMDATA],
        &accounts[IX_SOURCE_FOUNDATION_PARSER_CONFIG],
        &accounts[IX_SOURCE_FOUNDATION_SPEC],
    )
    .map_err(Refusal::from)?;
    let receiver = authenticate_receiver_route(
        route,
        &accounts[IX_SOURCE_FOUNDATION_RECEIVER_PROGRAM],
        &accounts[IX_SOURCE_FOUNDATION_RECEIVER_PROGRAMDATA],
        &accounts[IX_SOURCE_FOUNDATION_RECEIVER_CONFIG],
    )
    .map_err(Refusal::from)?;
    let route_bundle = authenticate_compiled_product_series_bundle_v7(
        program_id,
        &accounts[IX_SOURCE_FOUNDATION_BUNDLE_V7],
        registry,
        source_release,
        artifacts,
    )?;
    let product_route = authenticate_source_product_route_v4(
        route,
        receiver,
        registry,
        route_bundle,
        artifacts,
    )?;
    let publication_bundle = authenticate_compiled_product_series_bundle_v7(
        program_id,
        &accounts[IX_SOURCE_FOUNDATION_BUNDLE_V7],
        registry,
        source_release,
        artifacts,
    )?;
    let publication = authenticate_source_semantic_publication_v2(
        product_route,
        source_release,
        artifacts,
        registry,
        publication_bundle,
        ordinal,
    )?;
    let schedule = authenticate_source_work_schedule_artifact(
        program_id,
        route,
        &accounts[IX_SOURCE_FOUNDATION_WORK_SCHEDULE],
    )?;
    let policy = authenticate_source_runtime_liveness_policy_artifact_v1(
        program_id,
        &accounts[IX_SOURCE_FOUNDATION_LIVENESS_POLICY_ARTIFACT],
        route,
        product_route,
        schedule,
    )?;
    let source_principal_refund = RuntimeKey::from_bytes(
        artifacts.funding_terms().lamport_principal_refund.bytes(),
    );
    let capitalization = capitalize_source_work_v3(
        program_id,
        authority,
        route,
        publication,
        schedule,
        funding_reservation,
        &accounts[IX_SOURCE_FOUNDATION_FUNDING_V5],
        &accounts[IX_SOURCE_FOUNDATION_WORK_VAULT],
        &accounts[IX_SOURCE_FOUNDATION_CUSTODY],
        source_principal_refund,
        &accounts[IX_SOURCE_FOUNDATION_SYSTEM_PROGRAM],
        &accounts[IX_SOURCE_FOUNDATION_RENT_SYSVAR],
    )?;
    publish_pre_root_source_occurrence_v3(
        program_id,
        route,
        publication,
        schedule,
        policy.into_policy(),
        capitalization,
        &accounts[IX_SOURCE_FOUNDATION_CUSTODY],
        &accounts[IX_SOURCE_FOUNDATION_LIVENESS_POLICY_TARGET],
        &accounts[IX_SOURCE_FOUNDATION_COMPARTMENT_TARGET],
        &accounts[IX_SOURCE_FOUNDATION_OCCURRENCE_TARGET],
        &accounts[IX_SOURCE_FOUNDATION_WINDOW_TARGET],
        &accounts[IX_SOURCE_FOUNDATION_SUMMARY_TARGET],
        &accounts[IX_SOURCE_FOUNDATION_STATISTIC_KEY_TARGET],
        &accounts[IX_SOURCE_FOUNDATION_RESULT_LINEAGE_TARGET],
        &accounts[IX_SOURCE_FOUNDATION_GENERATION_REQUEST_TARGET],
        &accounts[IX_SOURCE_FOUNDATION_SYSTEM_PROGRAM],
        &accounts[IX_SOURCE_FOUNDATION_RENT_SYSVAR],
    )
}
