//! Chain-observation join and unsigned source-build material.
//!
//! Callers first authenticate finalized records with the canonical Record V1
//! adapter. This module consumes those move-only authorities, requires one
//! observation identity across the complete source, rejoins Product and Series
//! semantics, and invokes the deterministic manifest compiler. It performs no
//! RPC, signing, submission, deployment, or release selection.

use core::fmt::Write as _;

use dclutch_account_profile_contract::lifecycle_v3::{
    CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5, StateLifecyclePolicyV5,
};
use dclutch_capability_program_contract::{
    set_v2::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityProgramSetV2},
    v4::{CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4},
};
use dclutch_claims_svm::founding_v5::ClaimsFoundingRequestV5;
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{ProjectedCustodyOperationV1, ProjectedCustodyRequestV1};
use dclutch_market_core_codec::SeriesCoreRequestV1;
use dclutch_product_runtime_v2::{PortfolioV2, ResultDomainV2, join_product_v2};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_record_contract::AuthenticatedRawRecordV1;
use dclutch_series_v3_kernel::{
    AccountKeyV3, AuthenticatedProductProjectionV2,
    generated::{
        SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3, SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
    },
    request::{SeriesActionV3, admit_series_action_v3},
    series_core_consume_request,
};
use dclutch_trading_sbf::series::consume_artifacts_v4::SeriesConsumeChildRequestsV4;
use sha2::{Digest, Sha256};

use super::{
    SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4, SeriesShadowBundleCompileErrorV4,
    SeriesShadowBundleSourceV4, SeriesShadowDescriptorSemanticsV4, SeriesShadowRebuildSourcesV1,
    SeriesShadowReleaseSourcesV4, SeriesShadowSourceManifestV1,
    compile_series_shadow_source_manifest_v1, require_deterministic_series_shadow_rebuild_v1,
};

/// One finalized record authority paired with its chain-observation identity.
#[derive(Debug, Eq, PartialEq)]
pub struct ObservedSeriesShadowRecordV1<'content> {
    /// Same-finalized-observation identity, such as a blockhash/slot snapshot digest.
    pub observation: ContentId,
    /// Move-only canonical Record V1 authentication authority.
    pub record: AuthenticatedRawRecordV1<'content>,
}

/// Exact finalized records needed to derive one occurrence-specific source.
#[derive(Debug, Eq, PartialEq)]
pub struct SeriesShadowFinalizedRecordsV1<'content> {
    /// Finalized Series Template V3.
    pub template: ObservedSeriesShadowRecordV1<'content>,
    /// Finalized Series Occurrence V3.
    pub occurrence: ObservedSeriesShadowRecordV1<'content>,
    /// Finalized Series Ticket V3.
    pub ticket: ObservedSeriesShadowRecordV1<'content>,
    /// Finalized Product Runtime V2 graph root.
    pub product: ObservedSeriesShadowRecordV1<'content>,
    /// Finalized Product-selected result domain.
    pub result_domain: ObservedSeriesShadowRecordV1<'content>,
    /// Finalized Product-selected rational portfolio.
    pub portfolio: ObservedSeriesShadowRecordV1<'content>,
    /// Finalized CapabilityProgramSetV2 selected by the Series root.
    pub program_set: ObservedSeriesShadowRecordV1<'content>,
    /// Finalized action-selected CapabilityProgramV4.
    pub descriptor: ObservedSeriesShadowRecordV1<'content>,
    /// Finalized selected LifecycleV5.
    pub lifecycle: ObservedSeriesShadowRecordV1<'content>,
}

/// Independently selected release/source identities that exact bytes must match.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedSeriesShadowReleaseV1 {
    /// Exact same-finalized-observation identity.
    pub observation: ContentId,
    /// Finalized selected ProgramSet content identity.
    pub program_set: ContentId,
    /// Action-selected descriptor schema identity.
    pub descriptor_schema: ContentId,
    /// Action-selected descriptor content identity.
    pub descriptor_program: ContentId,
    /// Selected LifecycleV5 content identity.
    pub lifecycle: ContentId,
    /// Reviewed semantic source identity.
    pub semantic_source: ContentId,
    /// Reviewed generator-source-manifest identity.
    pub compiler_source: ContentId,
    /// Pinned toolchain/build-manifest identity.
    pub toolchain: ContentId,
    /// Translation-validation certificate selected for this AOT specialization.
    pub certificate: ContentId,
}

/// Mutable chain facts that occurrence-specific Core request bytes must derive from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesShadowReplaySourceV1 {
    /// Observation identity shared with finalized and physical inputs.
    pub observation: ContentId,
    /// Trading-owned Ticket replay account identity.
    pub ticket_state_account: AccountKeyV3,
    /// Exact current Series optimistic revision.
    pub expected_series_revision: u64,
    /// Exact current Ticket optimistic revision.
    pub expected_ticket_revision: u64,
}

/// Same-observation physical widths used by the canonical Profile13 compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesShadowAccountWidthsV1<'a> {
    /// Observation identity shared with all finalized and mutable inputs.
    pub observation: ContentId,
    /// Exact fixed pre-execution widths before the FundingState span.
    pub fixed_data_lengths: &'a [u32; SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4],
}

/// Complete unsigned input to the chain-derived source builder.
pub struct SeriesShadowObservedSourceV1<'a> {
    /// Finalized immutable source records.
    pub records: SeriesShadowFinalizedRecordsV1<'a>,
    /// Independently selected source/release identities.
    pub checked_release: CheckedSeriesShadowReleaseV1,
    /// Exact Series header plus occurrence proof.
    pub family_request: &'a [u8],
    /// Same-observation mutable replay coordinates.
    pub replay: SeriesShadowReplaySourceV1,
    /// Same-observation exact Profile13 widths.
    pub account_widths: SeriesShadowAccountWidthsV1<'a>,
    /// Exact canonical child request sources.
    pub child_requests: SeriesConsumeChildRequestsV4<'a>,
    /// Exact reviewed semantic source bytes.
    pub semantic_source: &'a [u8],
    /// Exact reviewed generator-source manifest bytes.
    pub compiler_source_manifest: &'a [u8],
    /// Exact pinned compiler/toolchain manifest bytes.
    pub toolchain_manifest: &'a [u8],
}

/// Exact digests handed to the later reproducible ELF build step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesShadowBuildInputsV1 {
    /// Exact source-manifest digest.
    pub source_manifest: ContentId,
    /// Complete generated bundle digest.
    pub bundle: ContentId,
    /// Generated include-payload digest.
    pub generated_include: ContentId,
    /// Reviewed semantic source identity.
    pub semantic_source: ContentId,
    /// Generator-source-manifest identity.
    pub compiler_source: ContentId,
    /// Pinned toolchain/build-manifest identity.
    pub toolchain: ContentId,
    /// Translation certificate identity.
    pub certificate: ContentId,
}

/// Unsigned exact output of one chain-derived source build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltSeriesShadowSourceV1 {
    /// Complete exact source manifest.
    pub manifest: Vec<u8>,
    /// Generated Rust include payload containing only checked bundle constants.
    pub generated_include: Vec<u8>,
    /// Exact immutable build inputs for a later reproducible ELF build.
    pub build_inputs: SeriesShadowBuildInputsV1,
}

/// Stable refusal from chain-observation and source construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesShadowSourceOperatorErrorV1 {
    /// Inputs did not belong to one exact finalized observation.
    Observation,
    /// A finalized schema/content pair or exact raw body differed.
    Record,
    /// Product graph hostile decoding or composition refused.
    Product,
    /// Series immutable content, action, or proof refused.
    Series,
    /// ProgramSet selection or CapabilityProgramV4 bytes differed.
    Descriptor,
    /// Lifecycle selection differed.
    Lifecycle,
    /// A child request was noncanonical or did not join the selected occurrence.
    ChildRequest,
    /// Reviewed source, compiler, toolchain, or certificate selection differed.
    Source,
    /// Canonical manifest generation or deterministic rebuilding refused.
    Compile(SeriesShadowBundleCompileErrorV4),
    /// Generated include construction or digesting refused.
    Include,
}

impl From<SeriesShadowBundleCompileErrorV4> for SeriesShadowSourceOperatorErrorV1 {
    fn from(value: SeriesShadowBundleCompileErrorV4) -> Self {
        Self::Compile(value)
    }
}

/// Result alias for the unsigned source operator.
pub type SourceOperatorResult<T> = core::result::Result<T, SeriesShadowSourceOperatorErrorV1>;

/// Build one exact occurrence-specific source manifest and include payload.
pub fn build_series_shadow_source_v1(
    input: SeriesShadowObservedSourceV1<'_>,
) -> SourceOperatorResult<BuiltSeriesShadowSourceV1> {
    require_observation(&input)?;
    require_source_identities(&input)?;
    let descriptor = authenticate_descriptor(&input)?;
    let product = authenticate_product(&input.records)?;
    require_record(
        &input.records.template.record,
        SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        None,
    )?;
    require_record(
        &input.records.occurrence.record,
        SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
        None,
    )?;
    require_record(
        &input.records.ticket.record,
        SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
        None,
    )?;
    let admitted = admit_series_action_v3(
        input.family_request,
        input.records.template.record.exact_content(),
        Some(input.records.occurrence.record.exact_content()),
        Some(input.records.ticket.record.exact_content()),
    )
    .map_err(|_| SeriesShadowSourceOperatorErrorV1::Series)?;
    if admitted.request().action() != SeriesActionV3::Consume {
        return Err(SeriesShadowSourceOperatorErrorV1::Series);
    }
    let occurrence = admitted
        .required_occurrence()
        .map_err(|_| SeriesShadowSourceOperatorErrorV1::Series)?;
    let ticket = admitted
        .required_ticket()
        .map_err(|_| SeriesShadowSourceOperatorErrorV1::Series)?;
    if occurrence.occurrence().product_record() != product.product_record() {
        return Err(SeriesShadowSourceOperatorErrorV1::Product);
    }
    let expected_core = series_core_consume_request(
        occurrence,
        ticket,
        product,
        input.replay.ticket_state_account,
        input.replay.expected_series_revision,
        input.replay.expected_ticket_revision,
    )
    .map_err(|_| SeriesShadowSourceOperatorErrorV1::ChildRequest)?
    .encode()
    .map_err(|_| SeriesShadowSourceOperatorErrorV1::ChildRequest)?;
    if expected_core.as_slice() != input.child_requests.core.as_slice() {
        return Err(SeriesShadowSourceOperatorErrorV1::ChildRequest);
    }
    require_child_requests(input.child_requests, occurrence, ticket, product)?;

    let source = SeriesShadowBundleSourceV4 {
        descriptor: SeriesShadowDescriptorSemanticsV4 {
            kind: descriptor.kind(),
            config_schema: descriptor.config_schema(),
            request_schema: descriptor.request_schema(),
            root_schema: descriptor.root_schema(),
            derivation_policy: descriptor.derivation_policy(),
            capacity_profile: descriptor.capacity_profile(),
            root_state_bytes: descriptor.root_state_bytes(),
        },
        release_sources: SeriesShadowReleaseSourcesV4 {
            semantic_source: input.semantic_source,
            compiler_source: input.compiler_source_manifest,
            toolchain_manifest: input.toolchain_manifest,
            certificate: input.checked_release.certificate,
        },
        lifecycle: input.records.lifecycle.record.exact_content(),
        fixed_data_lengths: input.account_widths.fixed_data_lengths,
        child_requests: input.child_requests,
    };
    let manifest = compile_series_shadow_source_manifest_v1(source)?;
    require_deterministic_series_shadow_rebuild_v1(
        &manifest,
        SeriesShadowRebuildSourcesV1 {
            semantic_source: input.semantic_source,
            compiler_source: input.compiler_source_manifest,
            toolchain_manifest: input.toolchain_manifest,
        },
    )?;
    let decoded = SeriesShadowSourceManifestV1::decode(&manifest)?;
    if decoded.generated_bundle().capability_program
        != input.records.descriptor.record.exact_content()
    {
        return Err(SeriesShadowSourceOperatorErrorV1::Descriptor);
    }
    let generated_include = emit_generated_include(decoded)?;
    let source_manifest = digest(&manifest)?;
    let generated_include_digest = digest(&generated_include)?;
    let bundle = decoded.bundle_digest();
    let semantic_source = decoded.semantic_source();
    let compiler_source = decoded.compiler_source();
    let toolchain = decoded.toolchain();
    Ok(BuiltSeriesShadowSourceV1 {
        manifest,
        generated_include,
        build_inputs: SeriesShadowBuildInputsV1 {
            source_manifest,
            bundle,
            generated_include: generated_include_digest,
            semantic_source,
            compiler_source,
            toolchain,
            certificate: input.checked_release.certificate,
        },
    })
}

fn require_observation(input: &SeriesShadowObservedSourceV1<'_>) -> SourceOperatorResult<()> {
    let expected = input.checked_release.observation;
    if input.replay.observation != expected
        || input.account_widths.observation != expected
        || [
            &input.records.template,
            &input.records.occurrence,
            &input.records.ticket,
            &input.records.product,
            &input.records.result_domain,
            &input.records.portfolio,
            &input.records.program_set,
            &input.records.descriptor,
            &input.records.lifecycle,
        ]
        .into_iter()
        .any(|record| record.observation != expected)
    {
        return Err(SeriesShadowSourceOperatorErrorV1::Observation);
    }
    Ok(())
}

fn require_source_identities(input: &SeriesShadowObservedSourceV1<'_>) -> SourceOperatorResult<()> {
    if digest(input.semantic_source)? != input.checked_release.semantic_source
        || digest(input.compiler_source_manifest)? != input.checked_release.compiler_source
        || digest(input.toolchain_manifest)? != input.checked_release.toolchain
    {
        return Err(SeriesShadowSourceOperatorErrorV1::Source);
    }
    Ok(())
}

fn authenticate_descriptor(
    input: &SeriesShadowObservedSourceV1<'_>,
) -> SourceOperatorResult<CapabilityProgramV4> {
    if input.checked_release.descriptor_schema.to_bytes() != CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4
    {
        return Err(SeriesShadowSourceOperatorErrorV1::Descriptor);
    }
    require_record(
        &input.records.program_set.record,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        Some(input.checked_release.program_set),
    )?;
    require_record(
        &input.records.descriptor.record,
        input.checked_release.descriptor_schema.to_bytes(),
        Some(input.checked_release.descriptor_program),
    )?;
    let set = CapabilityProgramSetV2::decode(input.records.program_set.record.exact_content())
        .map_err(|_| SeriesShadowSourceOperatorErrorV1::Descriptor)?;
    set.require_descriptor(
        input.family_request,
        input.checked_release.descriptor_schema,
        input.checked_release.descriptor_program,
    )
    .map_err(|_| SeriesShadowSourceOperatorErrorV1::Descriptor)?;
    let descriptor = CapabilityProgramV4::decode(input.records.descriptor.record.exact_content())
        .map_err(|_| SeriesShadowSourceOperatorErrorV1::Descriptor)?;
    require_record(
        &input.records.lifecycle.record,
        CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5,
        Some(input.checked_release.lifecycle),
    )?;
    let lifecycle = input.records.lifecycle.record.exact_content();
    StateLifecyclePolicyV5::decode_selected(
        input.checked_release.lifecycle.to_bytes(),
        input.checked_release.lifecycle.to_bytes(),
        lifecycle,
    )
    .map_err(|_| SeriesShadowSourceOperatorErrorV1::Lifecycle)?;
    if descriptor.lifecycle().program() != input.checked_release.lifecycle {
        return Err(SeriesShadowSourceOperatorErrorV1::Lifecycle);
    }
    Ok(descriptor)
}

fn authenticate_product(
    records: &SeriesShadowFinalizedRecordsV1<'_>,
) -> SourceOperatorResult<AuthenticatedProductProjectionV2> {
    let product_digest =
        require_record(&records.product.record, PRODUCT_RECORD_SCHEMA_ID_V2, None)?;
    let domain_digest = require_record(
        &records.result_domain.record,
        RESULT_DOMAIN_SCHEMA_ID_V2,
        None,
    )?;
    let portfolio_digest = require_record(&records.portfolio.record, PORTFOLIO_SCHEMA_ID_V2, None)?;
    let product = ProductRecordV2::decode(records.product.record.exact_content())
        .map_err(|_| SeriesShadowSourceOperatorErrorV1::Product)?;
    let domain = ResultDomainV2::decode(records.result_domain.record.exact_content())
        .map_err(|_| SeriesShadowSourceOperatorErrorV1::Product)?;
    let portfolio = PortfolioV2::decode(records.portfolio.record.exact_content())
        .map_err(|_| SeriesShadowSourceOperatorErrorV1::Product)?;
    if product.result_domain_digest().to_bytes() != domain_digest.to_bytes()
        || product.portfolio_digest().to_bytes() != portfolio_digest.to_bytes()
    {
        return Err(SeriesShadowSourceOperatorErrorV1::Product);
    }
    let join = join_product_v2(
        product_id(domain_digest)?,
        product_id(portfolio_digest)?,
        domain,
        portfolio,
    )
    .map_err(|_| SeriesShadowSourceOperatorErrorV1::Product)?;
    if join.product_id.to_bytes() != product.product_id().to_bytes() {
        return Err(SeriesShadowSourceOperatorErrorV1::Product);
    }
    Ok(AuthenticatedProductProjectionV2::new(
        product_digest,
        core_id(product.product_id().to_bytes())?,
        domain_digest,
    ))
}

fn require_child_requests(
    requests: SeriesConsumeChildRequestsV4<'_>,
    occurrence: dclutch_series_v3_kernel::AdmittedOccurrenceV3,
    ticket: dclutch_series_v3_kernel::AdmittedTicketV3,
    product: AuthenticatedProductProjectionV2,
) -> SourceOperatorResult<()> {
    let lock = ProjectedCustodyRequestV1::decode(requests.lock)
        .map_err(|_| SeriesShadowSourceOperatorErrorV1::ChildRequest)?;
    let realize = ProjectedCustodyRequestV1::decode(requests.realize)
        .map_err(|_| SeriesShadowSourceOperatorErrorV1::ChildRequest)?;
    let claims = ClaimsFoundingRequestV5::decode(requests.claims)
        .map_err(|_| SeriesShadowSourceOperatorErrorV1::ChildRequest)?;
    let core = SeriesCoreRequestV1::decode(requests.core)
        .map_err(|_| SeriesShadowSourceOperatorErrorV1::ChildRequest)?;
    let release = occurrence.template().release_set().to_bytes();
    let market = occurrence.occurrence().market().to_bytes();
    let founder = ticket.ticket().founder().to_bytes();
    let amount = occurrence.occurrence().funds().hoard_principal();
    let lock_request_digest: [u8; 32] = Sha256::digest(requests.lock).into();
    if lock.operation != ProjectedCustodyOperationV1::LockHoardAndCloseSource
        || realize.operation != ProjectedCustodyOperationV1::RealizeAndClose
        || lock.release_set != release
        || realize.release_set != release
        || claims.release_set() != release
        || lock.market != market
        || realize.market != market
        || claims.market() != market
        || lock.product_record != product.product_record().to_bytes()
        || realize.product_record != product.product_record().to_bytes()
        || claims.product_record_digest() != product.product_record().to_bytes()
        || lock.product != product.stable_product_id().to_bytes()
        || realize.product != product.stable_product_id().to_bytes()
        || claims.product_instance_id() != product.stable_product_id().to_bytes()
        || claims.semantic_basis_id() != occurrence.occurrence().liability_basis().to_bytes()
        || lock.realm != occurrence.template().realm().to_bytes()
        || realize.realm != occurrence.template().realm().to_bytes()
        || lock.refund_owner != ticket.ticket().refund_owner().to_bytes()
        || realize.refund_owner != ticket.ticket().refund_owner().to_bytes()
        || claims.founder() != founder
        || claims.trading_program() != lock.caller_program
        || claims.funding_source() != lock.funding_source_vault
        || claims.hoard() != lock.hoard_vault
        || claims.custody_request_digest() != lock_request_digest
        || lock.amount != amount
        || realize.amount != amount
        || claims.collateral_transferred() != amount
        || lock.context_digest != realize.context_digest
        || core.release_set().to_bytes() != release
        || core.market().map(|value| value.to_bytes()) != Some(market)
        || core.product().map(|value| value.to_bytes()) != Some(product.product_record().to_bytes())
    {
        return Err(SeriesShadowSourceOperatorErrorV1::ChildRequest);
    }
    Ok(())
}

fn require_record(
    record: &AuthenticatedRawRecordV1<'_>,
    schema: [u8; 32],
    expected: Option<ContentId>,
) -> SourceOperatorResult<ContentId> {
    let record_digest = digest(record.exact_content())?;
    if record.key().schema_release_id().to_bytes() != schema
        || record.key().expected_digest().to_bytes() != record_digest.to_bytes()
        || expected.is_some_and(|identity| identity != record_digest)
    {
        return Err(SeriesShadowSourceOperatorErrorV1::Record);
    }
    Ok(record_digest)
}

fn emit_generated_include(
    manifest: SeriesShadowSourceManifestV1<'_>,
) -> SourceOperatorResult<Vec<u8>> {
    let bundle = manifest.generated_bundle();
    let mut output = String::new();
    writeln!(
        output,
        "// @generated by dclutch-series-shadow-bundle-generator; do not edit."
    )
    .map_err(|_| SeriesShadowSourceOperatorErrorV1::Include)?;
    emit_array(
        &mut output,
        "SERIES_SHADOW_SOURCE_MANIFEST_DIGEST_V1",
        &digest(manifest.bytes())?.to_bytes(),
    )?;
    emit_array(
        &mut output,
        "SERIES_SHADOW_BUNDLE_DIGEST_V4",
        &manifest.bundle_digest().to_bytes(),
    )?;
    emit_array(
        &mut output,
        "SERIES_SHADOW_SEMANTIC_SOURCE_ID_V1",
        &manifest.semantic_source().to_bytes(),
    )?;
    emit_array(
        &mut output,
        "SERIES_SHADOW_COMPILER_SOURCE_ID_V1",
        &manifest.compiler_source().to_bytes(),
    )?;
    emit_array(
        &mut output,
        "SERIES_SHADOW_TOOLCHAIN_ID_V1",
        &manifest.toolchain().to_bytes(),
    )?;
    emit_array(
        &mut output,
        "SERIES_SHADOW_CERTIFICATE_ID_V1",
        &bundle.certificate.to_bytes(),
    )?;
    emit_slice(
        &mut output,
        "SERIES_SHADOW_CAPABILITY_PROGRAM_V4",
        bundle.capability_program,
    )?;
    emit_slice(
        &mut output,
        "SERIES_SHADOW_ACCOUNT_PROFILE_V4",
        bundle.account_profile,
    )?;
    emit_slice(
        &mut output,
        "SERIES_SHADOW_REQUEST_PROFILE_V4",
        bundle.request_profile,
    )?;
    emit_slice(&mut output, "SERIES_SHADOW_LIFECYCLE_V5", bundle.lifecycle)?;
    emit_slice(
        &mut output,
        "SERIES_SHADOW_TRANSITION_V4",
        bundle.transition,
    )?;
    emit_slice(&mut output, "SERIES_SHADOW_EFFECT_V4", bundle.effect)?;
    emit_slice(&mut output, "SERIES_SHADOW_STRATEGY_V4", bundle.strategy)?;
    Ok(output.into_bytes())
}

fn emit_array(output: &mut String, name: &str, bytes: &[u8; 32]) -> SourceOperatorResult<()> {
    write!(output, "pub const {name}: [u8; 32] = [")
        .map_err(|_| SeriesShadowSourceOperatorErrorV1::Include)?;
    emit_bytes(output, bytes)?;
    writeln!(output, "];\n").map_err(|_| SeriesShadowSourceOperatorErrorV1::Include)
}

fn emit_slice(output: &mut String, name: &str, bytes: &[u8]) -> SourceOperatorResult<()> {
    write!(output, "pub const {name}: &[u8] = &[")
        .map_err(|_| SeriesShadowSourceOperatorErrorV1::Include)?;
    emit_bytes(output, bytes)?;
    writeln!(output, "];\n").map_err(|_| SeriesShadowSourceOperatorErrorV1::Include)
}

fn emit_bytes(output: &mut String, bytes: &[u8]) -> SourceOperatorResult<()> {
    for byte in bytes {
        write!(output, "0x{byte:02x},").map_err(|_| SeriesShadowSourceOperatorErrorV1::Include)?;
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> SourceOperatorResult<ContentId> {
    core_id(Sha256::digest(bytes).into())
}

fn core_id(bytes: [u8; 32]) -> SourceOperatorResult<ContentId> {
    ContentId::new(bytes).map_err(|_| SeriesShadowSourceOperatorErrorV1::Source)
}

fn product_id(value: ContentId) -> SourceOperatorResult<dclutch_product_runtime_v2::ContentId> {
    dclutch_product_runtime_v2::ContentId::new(value.to_bytes())
        .map_err(|_| SeriesShadowSourceOperatorErrorV1::Product)
}

#[cfg(test)]
mod tests;
