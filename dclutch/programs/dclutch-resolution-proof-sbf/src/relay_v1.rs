//! The relayed-family analogue of [`crate::provider_v3`]: the failure-atomic
//! semantic seam between one sealed observation record and one terminal Source
//! result.
//!
//! Account ownership, Registry finality, PDA derivation and the record's own
//! program custody stay in the physical outer. This module is what the outer
//! calls once those hold: it applies the release-pinned decoding rules to bytes
//! a relayer quorum certified, bounds the observation by the Product's own
//! window, maps the resulting atom through the Product's sole result domain, and
//! returns a plan. Nothing here mutates.
//!
//! # Where this differs from the Pyth seam, and why
//!
//! Pyth's evidence arrives already interpreted — a `PriceUpdateV2` carries a
//! price, an exponent and a confidence, and the Receiver's own program vouched
//! for them. The relayed family's evidence arrives *un*interpreted on purpose:
//! the relayer signed account bytes and a slot, and the interpretation is this
//! cluster's work. So where `plan_provider_resolution_v3` authenticates a
//! provider release and then reads fields, this plans the reading itself, under
//! rules an immutable adapter release pins.
//!
//! The consequence worth stating plainly: an ordinary observation route may only
//! ever produce an ordinary outcome. Every way this seam can fail is a refusal,
//! including a venue that was upgraded mid-market. The Product's named failure
//! outcome is reachable only along the deadline-driven walk, which is the one
//! path allowed to select a failure selector.

use dclutch_product_runtime_v2::ResultDomainV2;
use dclutch_product_runtime_v2_svm_reader::AuthenticatedProductRuntimeV2;
use dclutch_registry_contract::ArtifactReleaseV1;
use dclutch_relay_contract::{
    Error as RelayContractError,
    decode::{RelayedObservationOutcomeV1, interpret_sealed_record_v1},
    record::RelayedObservationRecordViewV1,
    release::{AccountSetEntryV1, RelayedAdapterConfigV1},
};
use dclutch_resolution_codec::{ResolutionCertificateKindV2, ResolutionCertificateV2};
use dclutch_source_contract::{
    ContentId as SourceContentId, ProviderReleaseV1, SourceMaterialV2, SourceResolutionStateV2,
    SourceSpecV1, WindowKind, WindowSpecV1,
};
use solana_program::hash::hashv;

/// Domain separating relayed provider evidence from every other digest.
pub const RELAYED_EVIDENCE_DOMAIN_V1: &[u8] = b"dclutch/relayed-provider-evidence/v1";

/// Stable refusal from the pure relayed join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayJoinErrorV1 {
    /// The request's optimistic identities did not match the authenticated ones.
    Request,
    /// Independently authenticated Source records did not form one graph.
    Source,
    /// Product Runtime V2 record identity or coordinate semantics differed.
    Product,
    /// The sealed record was not consumable against this Source graph.
    Record,
    /// The certified bytes did not satisfy the release-pinned decoding rules.
    Observation,
    /// The observation was well formed but did not satisfy the window's own
    /// proposition, or fell outside the window's time bounds.
    Window,
    /// Source terminal transition or certificate construction refused.
    Transition,
}

/// Independently authenticated Source record values for the relayed family.
///
/// There is no statistic slot. A terminal window over a terminal sample has one
/// observation and one atom; a scheduled statistic over this family would need a
/// shared-observation child and is a different access profile.
#[derive(Clone, Copy)]
pub struct AuthenticatedRelaySourceRecordsV1 {
    /// `SourceMaterialV2` content identity, from the Market's own policy.
    pub material_id: SourceContentId,
    /// The authenticated material.
    pub material: SourceMaterialV2,
    /// `SourceSpecV1` content identity, from the material.
    pub source_spec_id: SourceContentId,
    /// The authenticated specification.
    pub source: SourceSpecV1,
    /// `ProviderReleaseV1` content identity, from the specification.
    pub provider_release_id: SourceContentId,
    /// The authenticated provider release.
    pub provider_release: ProviderReleaseV1,
    /// `RelayedAdapterConfigV1` content identity, from `decoding_rules_id`.
    pub decoding_rules_id: SourceContentId,
    /// The authenticated adapter configuration.
    pub config: RelayedAdapterConfigV1,
    /// `WindowSpecV1` content identity, from the material.
    pub window_spec_id: SourceContentId,
    /// The authenticated window.
    pub window: WindowSpecV1,
    /// The venue's pinned deployment, named by `SourceSpecV1.adapter_config_id`.
    pub venue_release_id: SourceContentId,
    /// The authenticated venue artifact release.
    pub venue_release: ArtifactReleaseV1,
}

/// The exact coordinates the outer authenticated before calling.
#[derive(Clone, Copy)]
pub struct RelayResolutionRequestV1 {
    /// Core Market account.
    pub market: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact positive terminal sequence naming the certificate.
    pub terminal_sequence: u64,
    /// The certificate account this plan will be written into.
    pub certificate_account: [u8; 32],
    /// The sealed record account being consumed.
    pub record_account: [u8; 32],
    /// The release-pinned observed cluster.
    pub pinned_cluster_id: [u8; 32],
    /// Devnet `Clock` at execution.
    pub current_unix_seconds: i64,
}

/// Failure-atomic plan returned to the physical SBF outer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayResolutionPlanV1 {
    /// The Source state after the terminal transition.
    pub next_source: SourceResolutionStateV2,
    /// The terminal certificate.
    pub certificate: ResolutionCertificateV2,
    /// The interpreted observation the certificate reports.
    pub observation: RelayedObservationOutcomeV1,
    /// The domain-separated evidence identity.
    pub evidence_id: [u8; 32],
}

/// Re-check that the independently authenticated records form one graph.
///
/// Each link is a digest a previous link already committed to, so a caller who
/// swaps one record for another of the same schema is refused by the link rather
/// than by the record's own contents.
fn authenticate_graph(records: &AuthenticatedRelaySourceRecordsV1) -> Result<(), RelayJoinErrorV1> {
    if records.material.primary_source_spec() != records.source_spec_id
        || records.material.window_spec() != records.window_spec_id
        || records.source.provider_release_id() != records.provider_release_id
        || records.source.adapter_config_id() != records.venue_release_id
        || records.provider_release.decoding_rules_id() != records.decoding_rules_id
    {
        return Err(RelayJoinErrorV1::Source);
    }
    records
        .window
        .validate_source(records.source_spec_id)
        .map_err(|_| RelayJoinErrorV1::Source)?;
    if records.window.kind() != WindowKind::Terminal {
        // A scheduled window over this access profile would need more than one
        // observation, and this route consumes exactly one sealed record.
        return Err(RelayJoinErrorV1::Source);
    }
    if records.config.account_set_id() == [0; 32] {
        return Err(RelayJoinErrorV1::Source);
    }
    Ok(())
}

/// Bound the observation by the Product's own window.
///
/// This is the second of the two time bounds and they answer different
/// questions. `RelayedAdapterConfigV1::require_observation_freshness` asks
/// whether the *relayer* was prompt: how far the attested foreign time has
/// fallen behind this cluster's clock. This asks whether the observation is
/// about the period the Product sold. A market can be resolved by a fresh
/// observation of the wrong week, or by a stale observation of the right one,
/// and both must refuse.
///
/// # Both bounds, and why the lower one used to be missing
///
/// This check enforces the whole closed interval `[start, end]`. It used to
/// enforce only the upper bound, and the reason it gave was true at the time:
/// `WindowSpecV1::new` refused `start != end` for `WindowKind::Terminal`, so
/// requiring the attested foreign clock to also be at or after `start` would
/// have required it to equal one exact second. That is not a bound, it is a
/// route nothing can pass — so the lower bound was dropped rather than the
/// degeneracy fixed, and an observation from *before the market opened* could
/// resolve it.
///
/// A terminal window now has real width, so both bounds are ordinary and both
/// are enforced. Together with the two-clock join
/// (`require_observation_freshness`, which refuses anything the relayer sat on
/// for longer than `max_observation_age_seconds`) the admitted span is
/// `[max(start, now - max_age), min(end, now + skew)]`.
///
/// The two edges refuse for different reasons and the distinction is the
/// Product's, not this function's: below `start` the observation is about a
/// period the market had not started selling, and above `end` it is a *late*
/// observation — the exact case a provider cadence straddling the deadline
/// produces, which must refuse rather than resolve the market on a reading from
/// after the question closed.
fn require_window_admits(
    window: WindowSpecV1,
    observed_unix_seconds: i64,
) -> Result<(), RelayJoinErrorV1> {
    if observed_unix_seconds < window.start_unix_seconds()
        || observed_unix_seconds > window.end_unix_seconds()
    {
        return Err(RelayJoinErrorV1::Window);
    }
    Ok(())
}

/// Join one sealed record to the Source and Product graphs.
///
/// `recomputed_account_set_id` is the outer's SHA-256 over the canonical
/// preimage; `entries` are the caller-supplied set the digest authenticates.
#[allow(clippy::too_many_arguments)]
pub fn plan_relayed_resolution_v1(
    request: &RelayResolutionRequestV1,
    source_state: &SourceResolutionStateV2,
    records: &AuthenticatedRelaySourceRecordsV1,
    product_runtime: &AuthenticatedProductRuntimeV2,
    result_domain: ResultDomainV2<'_>,
    record: RelayedObservationRecordViewV1<'_>,
    entries: &[AccountSetEntryV1],
    recomputed_account_set_id: [u8; 32],
) -> Result<RelayResolutionPlanV1, RelayJoinErrorV1> {
    if source_state.market() != request.market
        || source_state.generation() != request.generation
        || source_state.material_id() != records.material_id
        || request.terminal_sequence == 0
    {
        return Err(RelayJoinErrorV1::Request);
    }
    authenticate_graph(records)?;

    let product_record_digest = records.material.product_record_digest();
    if product_runtime.product_record.content_digest.to_bytes() != product_record_digest.to_bytes()
        || product_runtime.coordinate_domain_id.to_bytes()
            != result_domain.coordinate_domain_id().to_bytes()
        || product_runtime.result_unit_id.to_bytes() != result_domain.result_unit_id().to_bytes()
        // The Source and the Product must be about the same thing. Without this
        // a market on one coordinate could be resolved by a well-formed
        // observation of another, and every other check in this function would
        // pass while doing it. There is no statistic record to interpose because
        // a terminal sample is the identity map, so the Source's own unit must
        // equal the Product's result unit rather than map onto it.
        || records.source.domain_id().to_bytes() != result_domain.coordinate_domain_id().to_bytes()
        || records.source.unit_id().to_bytes() != result_domain.result_unit_id().to_bytes()
    {
        return Err(RelayJoinErrorV1::Product);
    }

    // The record's own binding is checked before a byte of it is interpreted:
    // sealed, complete, quorate, and bound to exactly this Market, generation,
    // material, provider release and key set.
    record
        .require_consumable(
            request.market,
            request.generation,
            records.material_id.to_bytes(),
            records.config.account_set_id(),
            records.provider_release_id.to_bytes(),
            records
                .provider_release
                .provider_deployment_release_id()
                .to_bytes(),
            request.pinned_cluster_id,
        )
        .map_err(|_| RelayJoinErrorV1::Record)?;
    if record
        .observed_slot()
        .map_err(|_| RelayJoinErrorV1::Record)?
        == 0
    {
        return Err(RelayJoinErrorV1::Record);
    }

    let observation = interpret_sealed_record_v1(
        record,
        records.config,
        entries,
        recomputed_account_set_id,
        records.venue_release,
        request.pinned_cluster_id,
        request.current_unix_seconds,
    )
    .map_err(|error| match error {
        // A pre-terminal venue state is the one refusal here that is not a
        // complaint about the evidence. The bytes were fine, the quorum was
        // real, the venue was the pinned one -- the market's own question just
        // does not have an answer yet, and flattening that into "bad
        // observation" would tell every caller the wrong thing.
        RelayContractError::WindowNotSatisfied => RelayJoinErrorV1::Window,
        _ => RelayJoinErrorV1::Observation,
    })?;
    require_window_admits(records.window, observation.observed_unix_seconds())?;

    // The evidence identity binds every input that could have moved the result:
    // which Source, which trust root, which rules, which slot, and the exact
    // certified fold over the observed bodies. Two different observations can
    // never share one evidence identity, and the same one can never be replayed
    // under a different rule set.
    let evidence_id = hashv(&[
        RELAYED_EVIDENCE_DOMAIN_V1,
        &[0],
        &records.source_spec_id.to_bytes(),
        &records.provider_release_id.to_bytes(),
        &records.decoding_rules_id.to_bytes(),
        &request.record_account,
        &record.set_digest().map_err(|_| RelayJoinErrorV1::Record)?,
        &observation.observed_slot().to_le_bytes(),
    ])
    .to_bytes();
    let evidence = SourceContentId::new(evidence_id).map_err(|_| RelayJoinErrorV1::Observation)?;

    let mut next_source = *source_state;
    let decision = next_source
        .resolve_primary_from_authenticated_domain(
            records.material_id,
            records.material,
            product_record_digest,
            result_domain,
            evidence,
            observation.atoms(),
            1,
            request.generation,
            request.current_unix_seconds,
            request.terminal_sequence,
        )
        .map_err(|_| RelayJoinErrorV1::Transition)?;
    let outcome_count = result_domain
        .outcome_count()
        .map_err(|_| RelayJoinErrorV1::Product)?;
    if decision.selector() >= result_domain.failure_selector()
        || decision.outcome_count() != outcome_count
        || product_runtime.outcome_count != outcome_count
    {
        // An observation route may not select the failure outcome. That selector
        // belongs to the deadline walk, and reaching it from here would let a
        // relayer choose the outcome it is least trusted to choose.
        return Err(RelayJoinErrorV1::Product);
    }

    let observed_at = u64::try_from(observation.observed_unix_seconds())
        .map_err(|_| RelayJoinErrorV1::Observation)?;
    let certificate = ResolutionCertificateV2 {
        kind: ResolutionCertificateKindV2::ResolutionSuccess,
        market: request.market,
        route: records.provider_release_id.to_bytes(),
        source_material: records.material_id.to_bytes(),
        product_record_digest: product_record_digest.to_bytes(),
        provider_evidence: evidence_id,
        funding_allocation: [0; 32],
        receipt_account: request.certificate_account,
        generation: request.generation,
        attempt_index: 0,
        schedule_index: 0,
        selector: decision.selector(),
        work_paid: 0,
        funding_remaining: 0,
        result_numerator: observation.atoms(),
        result_denominator: 1,
        observed_at,
    };
    certificate
        .validate_terminal_product(product_record_digest.to_bytes(), outcome_count)
        .and_then(|_| certificate.to_bytes().map(|_| ()))
        .map_err(|_| RelayJoinErrorV1::Transition)?;
    Ok(RelayResolutionPlanV1 {
        next_source,
        certificate,
        observation,
        evidence_id,
    })
}

#[cfg(test)]
mod tests {
    use dclutch_source_contract::ContentId as SourceContentId;

    use super::*;

    fn source_id(tag: u8) -> SourceContentId {
        let mut bytes = [0_u8; 32];
        bytes[0] = tag;
        SourceContentId::new(bytes).expect("nonzero Source content ID")
    }

    /// A window with real width, which is the only shape that can tell the two
    /// edges apart. `2_000..=2_600` is ten minutes, about two Solana-mainnet
    /// relayer rounds at the cadence this family is built for.
    fn window() -> WindowSpecV1 {
        WindowSpecV1::new(
            source_id(1),
            WindowKind::Terminal,
            2_000,
            2_600,
            300,
            60,
            source_id(2),
        )
        .expect("terminal window")
    }

    /// Both edges and the interior are admitted, and one second outside either
    /// edge is refused.
    ///
    /// The lower bound is the one this consumer had DELETED, because under a
    /// one-instant terminal window it was a route nothing could pass. Its
    /// absence meant an observation of a period the market had not started
    /// selling could resolve that market, so this case is the regression the
    /// widening was for.
    #[test]
    fn the_relayed_window_admits_its_closed_interval_and_nothing_else() {
        for observed in [2_000, 2_001, 2_300, 2_599, 2_600] {
            assert_eq!(
                require_window_admits(window(), observed),
                Ok(()),
                "{observed} is inside the period the Product sold"
            );
        }
        assert_eq!(
            require_window_admits(window(), 1_999),
            Err(RelayJoinErrorV1::Window),
            "one second before the market opened is the wrong period"
        );
        assert_eq!(
            require_window_admits(window(), 2_601),
            Err(RelayJoinErrorV1::Window),
            "a late observation must not answer a question that already closed"
        );
    }

    /// A degenerate window still means exactly what it says.
    ///
    /// Terminal windows may still be one instant; that is now a market's choice
    /// rather than the constructor's rule, and the bound is enforced the same
    /// way at both edges when a market makes it.
    #[test]
    fn a_degenerate_terminal_window_is_still_one_second() {
        let instant = WindowSpecV1::new(
            source_id(1),
            WindowKind::Terminal,
            2_000,
            2_000,
            300,
            60,
            source_id(2),
        )
        .expect("degenerate terminal window");
        assert_eq!(require_window_admits(instant, 2_000), Ok(()));
        assert_eq!(
            require_window_admits(instant, 1_999),
            Err(RelayJoinErrorV1::Window)
        );
        assert_eq!(
            require_window_admits(instant, 2_001),
            Err(RelayJoinErrorV1::Window)
        );
    }
}
