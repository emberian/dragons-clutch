//! Current-ABI real-Pyth evidence to Product-domain composition.
//!
//! Account ownership, Registry finality, Loader V3 linkage, caller CPI
//! authentication, and provider execution remain in the physical outer. This
//! module is the failure-atomic semantic seam the outer calls only after those
//! checks: it joins the real provider update and submitter to Source Runtime
//! V2, keeps the permissionless resolver distinct, maps through Product's sole
//! result domain, and emits both the terminal certificate and a typed receipt.

use dclutch_product_runtime_v2::ResultDomainV2;
use dclutch_product_runtime_v2_svm_reader::AuthenticatedProductRuntimeV2;
use dclutch_pyth_svm::{FullPriceUpdateV2, PythReleaseV1};
use dclutch_resolution_codec::{
    ProviderExecutionReceiptV3, ProviderExecutionRequestV3, ResolutionCertificateKindV2,
    ResolutionCertificateV2,
};
use dclutch_source_contract::{
    ContentId as SourceContentId, ProviderReleaseV1, PythAdapterConfigV1,
    PythProviderAdapterObligationV2, SourceMaterialV2, SourceResolutionStateV2, SourceSpecV1,
    StatisticSpecV1, WindowSpecV1,
};
use solana_program::hash::{hash, hashv};

/// Domain separating provider evidence from raw update and request digests.
pub const PROVIDER_EVIDENCE_DOMAIN_V3: &[u8] = b"dclutch/pyth-provider-evidence/v3";

/// Stable refusal from the pure current provider join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderJoinErrorV3 {
    /// Request bytes or their optimistic identities were invalid.
    Request,
    /// Independently authenticated Source records did not form one graph.
    Source,
    /// Product Runtime V2 record identity or coordinate semantics differed.
    Product,
    /// Provider release, adapter release, update, or role identity differed.
    Provider,
    /// Source terminal transition or certificate construction refused.
    Transition,
    /// A checked integer conversion overflowed.
    Arithmetic,
}

/// Independently authenticated current Source record values and identities.
#[derive(Clone, Copy)]
pub struct AuthenticatedSourceRecordsV3 {
    pub(crate) material_id: SourceContentId,
    pub(crate) material: SourceMaterialV2,
    pub(crate) source_spec_id: SourceContentId,
    pub(crate) source: SourceSpecV1,
    pub(crate) provider_release_id: SourceContentId,
    pub(crate) provider_release: ProviderReleaseV1,
    pub(crate) adapter_config_id: SourceContentId,
    pub(crate) adapter_config: PythAdapterConfigV1,
    pub(crate) window_spec_id: SourceContentId,
    pub(crate) window: WindowSpecV1,
    pub(crate) statistic_spec_id: SourceContentId,
    pub(crate) statistic: StatisticSpecV1,
    pub(crate) failure_policy_release: SourceContentId,
}

/// Independently authenticated provider/Product observations.
#[derive(Clone, Copy)]
pub struct AuthenticatedProviderObservationV3<'a> {
    pub(crate) pyth_release_id: [u8; 32],
    pub(crate) pyth_release: PythReleaseV1,
    pub(crate) product_runtime: AuthenticatedProductRuntimeV2,
    pub(crate) result_domain_bytes: &'a [u8],
    pub(crate) result_domain: ResultDomainV2<'a>,
    pub(crate) update_account: [u8; 32],
    pub(crate) provider_submitter: [u8; 32],
    pub(crate) expected_update_authority: [u8; 32],
    pub(crate) update_bytes: &'a [u8],
    pub(crate) post_params_body: &'a [u8],
    pub(crate) current_slot: u64,
    pub(crate) current_unix_seconds: i64,
}

/// Failure-atomic plan returned to the physical SBF outer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderResolutionPlanV3 {
    pub(crate) next_source: SourceResolutionStateV2,
    pub(crate) certificate: ResolutionCertificateV2,
    pub(crate) receipt: ProviderExecutionReceiptV3,
}

/// Join one exact real-provider update to Source and Product Runtime V2.
///
/// `request_bytes` remain authoritative for the parent digest. The outer must
/// authenticate that `caller_program`, release-set coordinates, and (for
/// Trading) CapabilityProgramSet selection are current before applying this
/// plan. No mutation occurs in this function.
pub fn plan_provider_resolution_v3(
    request_bytes: &[u8],
    source_state: &SourceResolutionStateV2,
    source_records: &AuthenticatedSourceRecordsV3,
    observation: &AuthenticatedProviderObservationV3<'_>,
) -> Result<ProviderResolutionPlanV3, ProviderJoinErrorV3> {
    let request = ProviderExecutionRequestV3::decode(request_bytes)
        .map_err(|_| ProviderJoinErrorV3::Request)?;
    let product_record_digest = source_records.material.product_record_digest().to_bytes();
    if request.market != source_state.market()
        || request.generation != source_state.generation()
        || request.source_material != source_records.material_id.to_bytes()
        || source_state.material_id() != source_records.material_id
        || request.source_spec != source_records.source_spec_id.to_bytes()
        || request.product_record != product_record_digest
        || request.product_record
            != observation
                .product_runtime
                .product_record
                .content_digest
                .to_bytes()
        || request.result_domain
            != observation
                .product_runtime
                .result_domain_record
                .content_digest
                .to_bytes()
        || request.provider_release != observation.pyth_release_id
        || request.update_account != observation.update_account
    {
        return Err(ProviderJoinErrorV3::Request);
    }

    let obligation = PythProviderAdapterObligationV2::from_authenticated_records(
        source_records.material,
        source_records.material.product_record_digest(),
        source_records.source_spec_id,
        source_records.source,
        source_records.provider_release_id,
        source_records.provider_release,
        source_records.adapter_config_id,
        source_records.adapter_config,
        source_records.window_spec_id,
        source_records.window,
        source_records.statistic_spec_id,
        source_records.statistic,
        source_records.failure_policy_release,
    )
    .map_err(|_| ProviderJoinErrorV3::Source)?;
    authenticate_provider_release(obligation, source_records.provider_release, observation)?;

    let result_domain_digest = hash(observation.result_domain_bytes).to_bytes();
    if product_record_digest
        != observation
            .product_runtime
            .product_record
            .content_digest
            .to_bytes()
        || result_domain_digest
            != observation
                .product_runtime
                .result_domain_record
                .content_digest
                .to_bytes()
        || obligation.source_domain_id().to_bytes()
            != observation.result_domain.coordinate_domain_id().to_bytes()
        || obligation.result_unit_id().to_bytes()
            != observation.result_domain.result_unit_id().to_bytes()
        || observation.product_runtime.coordinate_domain_id.to_bytes()
            != observation.result_domain.coordinate_domain_id().to_bytes()
        || observation.product_runtime.result_unit_id.to_bytes()
            != observation.result_domain.result_unit_id().to_bytes()
    {
        return Err(ProviderJoinErrorV3::Product);
    }

    let update_digest = hash(observation.update_bytes).to_bytes();
    let post_params_body_digest = hash(observation.post_params_body).to_bytes();
    if update_digest != request.expected_update_digest
        || post_params_body_digest != request.post_params_body_digest
    {
        return Err(ProviderJoinErrorV3::Provider);
    }
    let update = FullPriceUpdateV2::parse(observation.update_bytes)
        .map_err(|_| ProviderJoinErrorV3::Provider)?;
    if request.provider_submitter != observation.provider_submitter
        || update.write_authority() != observation.expected_update_authority
        || update.posted_slot() > observation.current_slot
        || update.publish_time() <= 0
    {
        return Err(ProviderJoinErrorV3::Provider);
    }
    let provider_evidence = hashv(&[
        PROVIDER_EVIDENCE_DOMAIN_V3,
        &[0],
        &request.source_spec,
        &request.provider_release,
        &request.update_account,
        &update_digest,
        &post_params_body_digest,
    ])
    .to_bytes();
    let normalized = obligation
        .normalize_authenticated_update(
            SourceContentId::new(provider_evidence).map_err(|_| ProviderJoinErrorV3::Provider)?,
            update.feed_id(),
            update.price(),
            update.confidence(),
            update.exponent(),
            update.publish_time(),
            observation.current_unix_seconds,
        )
        .map_err(|_| ProviderJoinErrorV3::Provider)?;

    let mut next_source = *source_state;
    let decision = next_source
        .resolve_primary_from_authenticated_domain(
            source_records.material_id,
            source_records.material,
            source_records.material.product_record_digest(),
            observation.result_domain,
            SourceContentId::new(provider_evidence).map_err(|_| ProviderJoinErrorV3::Provider)?,
            normalized.atoms(),
            1,
            request.generation,
            observation.current_unix_seconds,
            request.terminal_sequence,
        )
        .map_err(|_| ProviderJoinErrorV3::Transition)?;
    let outcome_count = observation
        .result_domain
        .outcome_count()
        .map_err(|_| ProviderJoinErrorV3::Product)?;
    if decision.selector() >= observation.result_domain.failure_selector()
        || decision.outcome_count() != outcome_count
        || observation.product_runtime.outcome_count != outcome_count
    {
        return Err(ProviderJoinErrorV3::Product);
    }
    finish_plan(
        request_bytes,
        &request,
        next_source,
        provider_evidence,
        update_digest,
        post_params_body_digest,
        decision.selector(),
        outcome_count,
        normalized.atoms(),
        update.publish_time(),
        update.posted_slot(),
        observation.current_slot,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn finish_plan(
    request_bytes: &[u8],
    request: &ProviderExecutionRequestV3,
    next_source: SourceResolutionStateV2,
    provider_evidence: [u8; 32],
    update_digest: [u8; 32],
    post_params_body_digest: [u8; 32],
    selector: u32,
    outcome_count: u32,
    result_atoms: i128,
    publish_time: i64,
    posted_slot: u64,
    consumed_slot: u64,
) -> Result<ProviderResolutionPlanV3, ProviderJoinErrorV3> {
    let observed_at = u64::try_from(publish_time).map_err(|_| ProviderJoinErrorV3::Arithmetic)?;
    let product_record_digest = request.product_record;
    let certificate = ResolutionCertificateV2 {
        kind: ResolutionCertificateKindV2::ResolutionSuccess,
        market: request.market,
        route: request.provider_release,
        source_material: request.source_material,
        product_record_digest,
        provider_evidence,
        funding_allocation: [0; 32],
        receipt_account: request.certificate_account,
        generation: request.generation,
        attempt_index: 0,
        schedule_index: 0,
        selector,
        work_paid: 0,
        funding_remaining: 0,
        result_numerator: result_atoms,
        result_denominator: 1,
        observed_at,
    };
    certificate
        .validate_terminal_product(product_record_digest, outcome_count)
        .and_then(|_| certificate.to_bytes().map(|_| ()))
        .map_err(|_| ProviderJoinErrorV3::Transition)?;
    let receipt = ProviderExecutionReceiptV3 {
        caller: request.caller,
        generation: request.generation,
        terminal_sequence: request.terminal_sequence,
        request_digest: hash(request_bytes).to_bytes(),
        provider_evidence,
        update_digest,
        post_params_body_digest,
        market: request.market,
        source_state: request.source_state,
        certificate_account: request.certificate_account,
        source_material: request.source_material,
        product_record: request.product_record,
        result_domain: request.result_domain,
        provider_release: request.provider_release,
        update_account: request.update_account,
        provider_submitter: request.provider_submitter,
        resolver: request.resolver,
        caller_program: request.caller_program,
        release_set: request.release_set,
        capability_program_set: request.capability_program_set,
        selected_capability_program: request.selected_capability_program,
        selector,
        outcome_count,
        result_numerator: result_atoms,
        result_denominator: 1,
        publish_time,
        posted_slot,
        consumed_slot,
    };
    receipt
        .to_bytes()
        .map_err(|_| ProviderJoinErrorV3::Transition)?;
    Ok(ProviderResolutionPlanV3 {
        next_source,
        certificate,
        receipt,
    })
}

fn authenticate_provider_release(
    obligation: PythProviderAdapterObligationV2,
    source_release: ProviderReleaseV1,
    observation: &AuthenticatedProviderObservationV3<'_>,
) -> Result<(), ProviderJoinErrorV3> {
    let pyth = observation.pyth_release;
    if obligation.provider_deployment_release_id().to_bytes() != observation.pyth_release_id
        || source_release.provider_deployment_release_id().to_bytes() != observation.pyth_release_id
        || source_release.adapter_release_id().to_bytes() != pyth.adapter_id()
        || source_release.decoding_rules_id().to_bytes() != pyth.price_update_codec_id()
        || source_release.transport_profile_id().to_bytes() != pyth.router_abi_id()
        || pyth.activation_time() > observation.current_unix_seconds
    {
        Err(ProviderJoinErrorV3::Provider)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use dclutch_product_runtime_v2::{
        ContentId as ProductContentId, ResultDomainInputV2, compile_result_domain_v2,
        result_domain_record_bytes,
    };
    use dclutch_product_runtime_v2_svm_reader::AuthenticatedRecordV2;
    use dclutch_pyth_svm::PythReleaseV1Input;
    use dclutch_resolution_codec::ProviderCallerV3;
    use dclutch_source_contract::{
        CapacityEnvelope, PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1, RoundingBoundary,
        SOURCE_FAILURE_POLICY_RELEASE_ID_V2, SourceAccessProfile, SourceCapacityProfileV1,
        SourceResolutionPhaseV1, StatisticKind, WindowKind,
    };
    use solana_program::pubkey::Pubkey;

    use super::*;

    const UPDATE: &[u8] =
        include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/price-update.account");
    const POST_DATA: &[u8] = include_bytes!(
        "../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-post-update.data"
    );

    #[derive(Clone, Copy)]
    enum Case {
        Success,
        WrongSubmitter,
        WrongProductDomain,
        ParallelProviderRelease,
    }

    fn source_id(tag: u8) -> SourceContentId {
        let mut bytes = [0_u8; 32];
        bytes[0] = tag;
        SourceContentId::new(bytes).expect("nonzero Source content ID")
    }

    fn product_id(tag: u8) -> ProductContentId {
        let mut bytes = [0_u8; 32];
        bytes[0] = tag;
        ProductContentId::new(bytes).expect("nonzero Product content ID")
    }

    fn runtime_record(digest: [u8; 32], tag: u8) -> AuthenticatedRecordV2 {
        AuthenticatedRecordV2 {
            schema_id: product_id(tag),
            content_digest: ProductContentId::new(digest).expect("nonzero digest"),
            raw_account: Pubkey::new_from_array([tag.wrapping_add(1); 32]),
            staging_account: Pubkey::new_from_array([tag.wrapping_add(2); 32]),
        }
    }

    fn plan(case: Case) -> Result<ProviderResolutionPlanV3, ProviderJoinErrorV3> {
        let post_body = POST_DATA.get(8..).ok_or(ProviderJoinErrorV3::Provider)?;
        let update = FullPriceUpdateV2::parse(UPDATE).expect("captured full Pyth update");
        let coordinate_domain = source_id(1);
        let source_unit = source_id(2);
        let result_unit = source_id(3);
        let product_record_id = source_id(4);
        let capacity_id = source_id(5);

        let domain_input = ResultDomainInputV2 {
            product_id: product_id(6),
            coordinate_domain_id: ProductContentId::new(coordinate_domain.to_bytes())
                .expect("coordinate ID"),
            result_unit_id: ProductContentId::new(result_unit.to_bytes()).expect("result unit"),
            liability_basis_id: product_id(7),
            representation_release_id: product_id(8),
            mapping_release_id: product_id(9),
            cut_denominator: 1,
            cuts: &[0],
        };
        let mut domain_bytes = vec![0_u8; result_domain_record_bytes(1).expect("domain width")];
        compile_result_domain_v2(domain_input, &mut domain_bytes).expect("runtime domain");
        let domain = ResultDomainV2::decode(&domain_bytes).expect("decoded runtime domain");
        let actual_domain_digest = hash(&domain_bytes).to_bytes();
        let selected_domain_digest = if matches!(case, Case::WrongProductDomain) {
            [0x91; 32]
        } else {
            actual_domain_digest
        };
        let product_runtime = AuthenticatedProductRuntimeV2 {
            product_record: runtime_record(product_record_id.to_bytes(), 20),
            result_domain_record: runtime_record(selected_domain_digest, 23),
            portfolio_record: runtime_record([0x24; 32], 26),
            product_id: domain.product_id(),
            coordinate_domain_id: domain.coordinate_domain_id(),
            result_unit_id: domain.result_unit_id(),
            claim_basis_id: product_id(29),
            liability_basis_id: domain.liability_basis_id(),
            representation_release_id: domain.representation_release_id(),
            mapping_release_id: domain.mapping_release_id(),
            outcome_count: domain.outcome_count().expect("outcome count"),
        };

        let pyth_release = PythReleaseV1::new(PythReleaseV1Input {
            cluster_id: [31; 32],
            receiver_program: [32; 32],
            receiver_programdata: [33; 32],
            receiver_config: [34; 32],
            router_program: [35; 32],
            router_programdata: [36; 32],
            config_digest: [37; 32],
            receiver_abi_id: [38; 32],
            router_abi_id: [39; 32],
            price_update_codec_id: [40; 32],
            adapter_id: PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1,
            receiver_deployment_slot: 1,
            router_deployment_slot: 2,
            guardian_set_count: 19,
            required_guardian_count: 10,
            upstream_commit: [41; 20],
            sdk_crate_digest: [42; 32],
            activation_time: update.publish_time() - 1,
        })
        .expect("Pyth release");
        let pyth_release_id = hash(&pyth_release.to_bytes()).to_bytes();
        let provider_release = ProviderReleaseV1::new(
            source_id(43),
            SourceContentId::new(pyth_release.adapter_id()).expect("adapter release"),
            SourceContentId::new(pyth_release_id).expect("Pyth release ID"),
            SourceContentId::new(pyth_release.price_update_codec_id()).expect("codec ID"),
            SourceContentId::new(pyth_release.router_abi_id()).expect("transport ID"),
        );
        let actual_provider_release_id =
            SourceContentId::new(hash(&provider_release.to_bytes()).to_bytes())
                .expect("provider release digest");
        let provider_release_id = if matches!(case, Case::ParallelProviderRelease) {
            source_id(90)
        } else {
            actual_provider_release_id
        };
        let adapter_config = PythAdapterConfigV1::new(update.feed_id(), update.exponent(), 10_000)
            .expect("Pyth adapter configuration");
        let adapter_config_id = SourceContentId::new(hash(&adapter_config.to_bytes()).to_bytes())
            .expect("adapter digest");
        let source = SourceSpecV1::new(
            coordinate_domain,
            source_unit,
            actual_provider_release_id,
            SourceAccessProfile::PythTerminalOneTransaction,
            adapter_config_id,
            capacity_id,
        );
        let source_spec_id =
            SourceContentId::new(hash(&source.to_bytes()).to_bytes()).expect("source digest");
        let window = WindowSpecV1::new(
            source_spec_id,
            WindowKind::Terminal,
            update.publish_time(),
            update.publish_time(),
            10,
            1,
            source_id(44),
        )
        .expect("terminal window");
        let window_spec_id =
            SourceContentId::new(hash(&window.to_bytes()).to_bytes()).expect("window digest");
        let capacity = SourceCapacityProfileV1::new(
            CapacityEnvelope::Measured,
            1,
            0,
            source_id(45),
            source_id(46),
            208,
            0,
        )
        .expect("capacity");
        let statistic = StatisticSpecV1::new(
            source_unit,
            result_unit,
            StatisticKind::TerminalSample,
            RoundingBoundary::ExactRational,
            1,
            0,
            capacity_id,
            source_id(47),
            capacity,
        )
        .expect("terminal statistic");
        let statistic_spec_id =
            SourceContentId::new(hash(&statistic.to_bytes()).to_bytes()).expect("statistic digest");
        let failure_policy_release =
            SourceContentId::new(SOURCE_FAILURE_POLICY_RELEASE_ID_V2).expect("failure release");
        let material = SourceMaterialV2::new(
            product_record_id,
            source_spec_id,
            window_spec_id,
            statistic_spec_id,
            None,
            failure_policy_release,
        );
        let material_id =
            SourceContentId::new(hash(&material.to_bytes()).to_bytes()).expect("material digest");
        let market = [48; 32];
        let state = SourceResolutionStateV2::fresh(market, 7, material_id, [49; 32], 1, 0, 0)
            .expect("fresh Source")
            .state();
        let provider_submitter = if matches!(case, Case::WrongSubmitter) {
            [50; 32]
        } else {
            [58; 32]
        };
        let request = ProviderExecutionRequestV3 {
            caller: ProviderCallerV3::Core,
            generation: 7,
            terminal_sequence: 1,
            market,
            source_state: [51; 32],
            certificate_account: [52; 32],
            source_material: material_id.to_bytes(),
            source_spec: source_spec_id.to_bytes(),
            product_record: product_record_id.to_bytes(),
            result_domain: selected_domain_digest,
            provider_release: pyth_release_id,
            update_account: [53; 32],
            expected_update_digest: hash(UPDATE).to_bytes(),
            provider_submitter,
            resolver: [54; 32],
            caller_program: [55; 32],
            release_set: [56; 32],
            capability_program_set: [0; 32],
            selected_capability_program: [0; 32],
            parent_request_digest: [57; 32],
            post_params_body_digest: hash(post_body).to_bytes(),
        };
        let request_bytes = request
            .to_bytes()
            .map_err(|_| ProviderJoinErrorV3::Request)?;
        let records = AuthenticatedSourceRecordsV3 {
            material_id,
            material,
            source_spec_id,
            source,
            provider_release_id,
            provider_release,
            adapter_config_id,
            adapter_config,
            window_spec_id,
            window,
            statistic_spec_id,
            statistic,
            failure_policy_release,
        };
        let observation = AuthenticatedProviderObservationV3 {
            pyth_release_id,
            pyth_release,
            product_runtime,
            result_domain_bytes: &domain_bytes,
            result_domain: domain,
            update_account: request.update_account,
            provider_submitter: [58; 32],
            expected_update_authority: update.write_authority(),
            update_bytes: UPDATE,
            post_params_body: post_body,
            current_slot: update.posted_slot(),
            current_unix_seconds: update.publish_time(),
        };
        plan_provider_resolution_v3(&request_bytes, &state, &records, &observation)
    }

    #[test]
    fn captured_real_update_joins_runtime_product_without_failure_alias() {
        let result = plan(Case::Success).expect("current provider plan");
        assert_eq!(
            result.next_source.phase(),
            SourceResolutionPhaseV1::Resolved
        );
        assert!(result.receipt.selector < result.receipt.outcome_count - 1);
        assert_eq!(
            ProviderExecutionReceiptV3::decode(
                &result.receipt.to_bytes().expect("typed provider receipt")
            ),
            Ok(result.receipt)
        );
        assert_eq!(
            result.certificate.provider_evidence,
            result.receipt.provider_evidence
        );
    }

    #[test]
    fn submitter_product_and_provider_substitutions_refuse() {
        assert_eq!(
            plan(Case::WrongSubmitter),
            Err(ProviderJoinErrorV3::Provider)
        );
        assert_eq!(
            plan(Case::WrongProductDomain),
            Err(ProviderJoinErrorV3::Product)
        );
        assert_eq!(
            plan(Case::ParallelProviderRelease),
            Err(ProviderJoinErrorV3::Source)
        );
    }
}
