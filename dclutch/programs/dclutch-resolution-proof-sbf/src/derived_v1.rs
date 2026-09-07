//! The derived-family analogue of [`crate::relay_v1`]: the failure-atomic
//! semantic seam between two parent certificates and one terminal Source
//! result for a child market.
//!
//! Account ownership, PDA derivation, the parents' Market and Source-state
//! custody, and the certificate accounts' own authentication stay in the
//! physical outer (`derived_transport_v1`), which reads each parent
//! certificate exactly as Core's `AdmitTerminal` reads one. This module is
//! what the outer calls once those hold: it admits each certificate against
//! the reference (`ParentRef.admit`'s six conjuncts, in
//! `dclutch_source::parent_reference_v1`), computes the child's selector as
//! the theorems say (`productSelector` / `conditionalSelector`), binds the
//! reference to the child's own window, and returns a plan. Nothing here
//! mutates and nothing here hashes but the evidence identity.
//!
//! # Where this differs from the relay seam, and why
//!
//! The relay interprets attested bytes under a decoding-rules row. This
//! interprets nothing: a parent's certificate already carries a selector its
//! own Product bound, and the child's atom is a pure function of two such
//! selectors (`settleProduct_ok_is_the_selector`). So there is no window
//! proposition to satisfy and no scale to admit -- the joint index is an
//! integer over denominator one at exponent zero, which the statistic record
//! must declare and the route checks.
//!
//! # The one arm this route does not take
//!
//! A parent that resolved to ITS failure coordinate makes the child's
//! selector the child's failure coordinate (`productSelector_failure_of_parent_failure`).
//! The Source machine reaches a failure selector only through `exhaust`, so
//! this route refuses that case by name (`ParentFailed`) and the funded
//! deadline walk lands the child on its failure coordinate and refunds
//! (design §4.3, fallback; `AttestedUnobservable` is not built this
//! generation -- `BUILD_PRODUCT-SHAPES.md` §3 ruling 6).

use dclutch_product::ResultDomainV2;
use dclutch_product::svm_reader::AuthenticatedProductRuntimeV2;
use dclutch_source::parent_reference_v1::{
    ChildShapeV1, DERIVED_DECODING_RULES_ID_V1, DERIVED_EVIDENCE_DOMAIN_V1,
    DERIVED_FAMILY_RELEASE_ID_V1, DERIVED_TRANSPORT_PROFILE_ID_V1, DerivedSettleRequestV1,
    ParentCertificateV1, ParentReferenceV1, ParentRefusalV1, ParentTerminalV1,
    admit_parent_certificate_v1,
};
use dclutch_source::resolution::{ResolutionCertificateKindV2, ResolutionCertificateV2};
use dclutch_source::{
    ContentId as SourceContentId, ProviderReleaseV1, SourceAccessProfile, SourceMaterialV3,
    SourceResolutionStateV2, SourceSpecV1, StatisticSpecV1, WindowKind, WindowSpecV1,
};
use solana_program::hash::hashv;

/// Stable refusal from the pure derived join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivedJoinErrorV1 {
    /// The request's optimistic identities did not match the authenticated ones.
    Request,
    /// Independently authenticated Source records did not form one graph.
    Source,
    /// Product Runtime V2 record identity or coordinate semantics differed.
    Product,
    /// The reference does not describe the child: its `settle_by` is not the
    /// window's end, or the child's width is not the reference's.
    Reference,
    /// A parent the branch depends on has no terminal certificate yet.
    ParentNotTerminal,
    /// The certificate names a market the reference does not.
    WrongParent,
    /// The certificate's generation is not the referenced one.
    ParentGeneration,
    /// The certificate binds a Product record the reference does not.
    ParentRecord,
    /// The parent's ordinary count is not the referenced one.
    ParentWidth,
    /// The certificate's selector exceeds the parent's width.
    SelectorOutOfRange,
    /// A parent resolved to its failure coordinate: the child's answer is its
    /// own failure coordinate, which only the deadline walk may select.
    ParentFailed,
    /// The child's primary deadline has passed; the deadline walk owns it.
    WindowClosed,
    /// Source terminal transition or certificate construction refused.
    Transition,
    /// A checked conversion overflowed.
    Arithmetic,
}

const fn map_refusal(refusal: ParentRefusalV1) -> DerivedJoinErrorV1 {
    match refusal {
        ParentRefusalV1::ParentNotTerminal => DerivedJoinErrorV1::ParentNotTerminal,
        ParentRefusalV1::WrongParent => DerivedJoinErrorV1::WrongParent,
        ParentRefusalV1::ParentGenerationMismatch => DerivedJoinErrorV1::ParentGeneration,
        ParentRefusalV1::ParentRecordMismatch => DerivedJoinErrorV1::ParentRecord,
        ParentRefusalV1::ParentWidthMismatch => DerivedJoinErrorV1::ParentWidth,
        ParentRefusalV1::SelectorOutOfRange => DerivedJoinErrorV1::SelectorOutOfRange,
        // Founding conjuncts; a founded child cannot fail them, and a reference
        // that does is a reference the founding never admitted.
        ParentRefusalV1::ConditionOutOfRange
        | ParentRefusalV1::ConditionOnFailure
        | ParentRefusalV1::SameParent
        | ParentRefusalV1::EmptyParent
        | ParentRefusalV1::WidthOverflow => DerivedJoinErrorV1::Reference,
    }
}

/// Independently authenticated Source record values for the derived family.
#[derive(Clone, Copy)]
pub struct AuthenticatedDerivedSourceRecordsV1 {
    /// `SourceMaterialV3` content identity, from the Market's own policy.
    pub material_id: SourceContentId,
    /// The authenticated material.
    pub material: SourceMaterialV3,
    /// `SourceSpecV1` content identity, from the material.
    pub source_spec_id: SourceContentId,
    /// The authenticated specification; its access profile is
    /// `DerivedFromParents` and its `adapter_config_id` is the reference.
    pub source: SourceSpecV1,
    /// `ProviderReleaseV1` content identity, from the specification.
    pub provider_release_id: SourceContentId,
    /// The derived family's constant release.
    pub provider_release: ProviderReleaseV1,
    /// `WindowSpecV1` content identity, from the material.
    pub window_spec_id: SourceContentId,
    /// The child's window: terminal, ending at `settle_by`.
    pub window: WindowSpecV1,
    /// `StatisticSpecV1` content identity, from the material.
    pub statistic_spec_id: SourceContentId,
    /// The child's statistic: the joint index at exponent zero.
    pub statistic: StatisticSpecV1,
    /// `ParentReferenceV1` content identity, from the specification.
    pub reference_id: SourceContentId,
    /// The authenticated reference.
    pub reference: ParentReferenceV1,
}

/// One parent's certificate as the outer authenticated it: the fields the
/// child reads, its observation time, and its 312 bytes for the evidence
/// identity.
#[derive(Clone, Copy)]
pub struct AuthenticatedParentCertificateV1 {
    /// The certificate's fields the reference is admitted against.
    pub certificate: ParentCertificateV1,
    /// `certificate.observed_at`.
    pub observed_at: u64,
    /// The exact account bytes.
    pub bytes: [u8; dclutch_source::resolution::RESOLUTION_CERTIFICATE_BYTES_V2],
}

/// The exact coordinates the outer authenticated before calling.
#[derive(Clone, Copy)]
pub struct DerivedResolutionRequestV1 {
    /// Core Market account.
    pub market: [u8; 32],
    /// The certificate account this plan will be written into.
    pub certificate_account: [u8; 32],
    /// Devnet `Clock` at execution.
    pub current_unix_seconds: i64,
}

/// Failure-atomic plan returned to the physical SBF outer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivedResolutionPlanV1 {
    /// The Source state after the terminal transition.
    pub next_source: SourceResolutionStateV2,
    /// The terminal certificate.
    pub certificate: ResolutionCertificateV2,
    /// The child's selector: the joint index.
    pub selector: u32,
    /// The domain-separated evidence identity `H(domain ∥ cert_A ∥ cert_B)`.
    pub evidence_id: [u8; 32],
}

/// Re-check that the independently authenticated records form one graph.
fn authenticate_graph(
    records: &AuthenticatedDerivedSourceRecordsV1,
) -> Result<(), DerivedJoinErrorV1> {
    if records.material.primary_source_spec() != records.source_spec_id
        || records.material.window_spec() != records.window_spec_id
        || records.material.statistic_spec() != records.statistic_spec_id
        || records.source.provider_release_id() != records.provider_release_id
        || records.source.adapter_config_id() != records.reference_id
        || records.source.access_profile() != SourceAccessProfile::DerivedFromParents
        || records.provider_release.provider_family_id().to_bytes() != DERIVED_FAMILY_RELEASE_ID_V1
        || records.provider_release.transport_profile_id().to_bytes()
            != DERIVED_TRANSPORT_PROFILE_ID_V1
        || records.provider_release.decoding_rules_id().to_bytes() != DERIVED_DECODING_RULES_ID_V1
    {
        return Err(DerivedJoinErrorV1::Source);
    }
    records
        .window
        .validate_source(records.source_spec_id)
        .map_err(|_| DerivedJoinErrorV1::Source)?;
    if records.window.kind() != WindowKind::Terminal {
        return Err(DerivedJoinErrorV1::Source);
    }
    // A child buys no alternative source (design §2.2): its parents' ladders
    // are what guarantee a certificate arrives.
    if records.material.recovery_policy().is_some() {
        return Err(DerivedJoinErrorV1::Source);
    }
    Ok(())
}

/// The window is the reference's `settle_by`, and the route admits only
/// before the primary deadline (`end + max_age`, the exhaust boundary), so the
/// honest settle and the failure walk never both admit one second.
fn require_window_admits(
    window: WindowSpecV1,
    reference: ParentReferenceV1,
    current_unix_seconds: i64,
) -> Result<(), DerivedJoinErrorV1> {
    let end =
        u64::try_from(window.end_unix_seconds()).map_err(|_| DerivedJoinErrorV1::Reference)?;
    if end != reference.settle_by {
        return Err(DerivedJoinErrorV1::Reference);
    }
    let deadline = window
        .end_unix_seconds()
        .checked_add(i64::from(window.max_age_seconds()))
        .ok_or(DerivedJoinErrorV1::Arithmetic)?;
    if current_unix_seconds <= 0 || current_unix_seconds > deadline {
        return Err(DerivedJoinErrorV1::WindowClosed);
    }
    Ok(())
}

/// Join two parent certificates to the Source and Product graphs.
///
/// `parent_b` is `None` exactly when the outer presented the off-condition
/// frame; on a product child, or a conditional child on its branch, that is
/// `ParentNotTerminal` (`settleProduct_needs_B`, `condition_branch_needs_B`).
#[allow(clippy::too_many_arguments)]
pub fn plan_derived_resolution_v1(
    request: DerivedSettleRequestV1,
    coordinates: &DerivedResolutionRequestV1,
    source_state: &SourceResolutionStateV2,
    records: &AuthenticatedDerivedSourceRecordsV1,
    product_runtime: &AuthenticatedProductRuntimeV2,
    result_domain: ResultDomainV2<'_>,
    parent_a: &AuthenticatedParentCertificateV1,
    parent_b: Option<&AuthenticatedParentCertificateV1>,
) -> Result<DerivedResolutionPlanV1, DerivedJoinErrorV1> {
    if source_state.market() != coordinates.market
        || source_state.generation() != request.generation
        || source_state.material_id() != records.material_id
        || request.source_material != records.material_id.to_bytes()
        || request.source_spec != records.source_spec_id.to_bytes()
        || request.parent_reference != records.reference_id.to_bytes()
        || request.terminal_sequence == 0
    {
        return Err(DerivedJoinErrorV1::Request);
    }
    authenticate_graph(records)?;

    let product_record_digest = records.material.product_record_digest();
    let outcome_count = result_domain
        .outcome_count()
        .map_err(|_| DerivedJoinErrorV1::Product)?;
    if product_runtime.product_record.content_digest.to_bytes() != product_record_digest.to_bytes()
        || product_runtime.coordinate_domain_id.to_bytes()
            != result_domain.coordinate_domain_id().to_bytes()
        || product_runtime.result_unit_id.to_bytes() != result_domain.result_unit_id().to_bytes()
        || records.source.domain_id().to_bytes() != result_domain.coordinate_domain_id().to_bytes()
        || records.statistic.source_unit_id().to_bytes() != records.source.unit_id().to_bytes()
        || records.statistic.result_unit_id().to_bytes()
            != result_domain.result_unit_id().to_bytes()
        || product_runtime.outcome_count != outcome_count
    {
        return Err(DerivedJoinErrorV1::Product);
    }
    // The joint index is an integer at exponent zero; a statistic declaring
    // any shift would move cells under a selector that is not a measurement.
    records
        .statistic
        .require_admitted_scale(0)
        .map_err(|_| DerivedJoinErrorV1::Product)?;

    let reference = records.reference;
    let shape = reference.admit_founding().map_err(map_refusal)?;
    let width = shape.width().map_err(map_refusal)?;
    if width != outcome_count {
        return Err(DerivedJoinErrorV1::Reference);
    }
    require_window_admits(records.window, reference, coordinates.current_unix_seconds)?;

    let a = admit_parent_certificate_v1(reference.parent_a, parent_a.certificate)
        .map_err(map_refusal)?;
    let (selector, observed_at) = match shape {
        ChildShapeV1::Product { .. } => {
            let b = parent_b.ok_or(DerivedJoinErrorV1::ParentNotTerminal)?;
            let tb = admit_parent_certificate_v1(reference.parent_b, b.certificate)
                .map_err(map_refusal)?;
            (
                shape.product_selector(a, tb).map_err(map_refusal)?,
                parent_a.observed_at.max(b.observed_at),
            )
        }
        ChildShapeV1::Conditional { .. } => {
            let tb: Option<ParentTerminalV1> = match parent_b {
                Some(b) => Some(
                    admit_parent_certificate_v1(reference.parent_b, b.certificate)
                        .map_err(map_refusal)?,
                ),
                None => None,
            };
            let selector = shape.conditional_selector(a, tb).map_err(map_refusal)?;
            let observed_at = match parent_b {
                Some(b) if shape.needs_parent_b(a) => parent_a.observed_at.max(b.observed_at),
                _ => parent_a.observed_at,
            };
            (selector, observed_at)
        }
    };
    let failure_selector = shape.failure_selector().map_err(map_refusal)?;
    if selector >= failure_selector || selector >= result_domain.failure_selector() {
        // A parent's outage is the child's outage, and the child reaches its
        // failure coordinate through the deadline walk, never through an
        // observation route.
        return Err(DerivedJoinErrorV1::ParentFailed);
    }

    let b_bytes: &[u8] = match parent_b {
        Some(b) if shape.needs_parent_b(a) => &b.bytes,
        _ => &[],
    };
    let evidence_id = hashv(&[
        DERIVED_EVIDENCE_DOMAIN_V1,
        &[0],
        &records.reference_id.to_bytes(),
        &parent_a.bytes,
        b_bytes,
    ])
    .to_bytes();
    let evidence = SourceContentId::new(evidence_id).map_err(|_| DerivedJoinErrorV1::Transition)?;

    let mut next_source = *source_state;
    let decision = next_source
        .resolve_primary_from_authenticated_domain(
            records.material_id,
            records.material,
            product_record_digest,
            result_domain,
            evidence,
            i128::from(selector),
            1,
            0,
            request.generation,
            coordinates.current_unix_seconds,
            request.terminal_sequence,
        )
        .map_err(|_| DerivedJoinErrorV1::Transition)?;
    if decision.selector() != selector || decision.outcome_count() != outcome_count {
        // `childCuts` says the joint index selects its own region; a domain
        // that maps it elsewhere is not the child's domain.
        return Err(DerivedJoinErrorV1::Reference);
    }

    let certificate = ResolutionCertificateV2 {
        kind: ResolutionCertificateKindV2::ResolutionSuccess,
        market: coordinates.market,
        route: records.provider_release_id.to_bytes(),
        source_material: records.material_id.to_bytes(),
        product_record_digest: product_record_digest.to_bytes(),
        provider_evidence: evidence_id,
        funding_allocation: [0; 32],
        receipt_account: coordinates.certificate_account,
        generation: request.generation,
        attempt_index: 0,
        schedule_index: 0,
        selector,
        work_paid: 0,
        funding_remaining: 0,
        result_numerator: i128::from(selector),
        result_denominator: 1,
        observed_at,
    };
    certificate
        .validate_terminal_product(product_record_digest.to_bytes(), outcome_count)
        .and_then(|_| certificate.to_bytes().map(|_| ()))
        .map_err(|_| DerivedJoinErrorV1::Transition)?;
    Ok(DerivedResolutionPlanV1 {
        next_source,
        certificate,
        selector,
        evidence_id,
    })
}
