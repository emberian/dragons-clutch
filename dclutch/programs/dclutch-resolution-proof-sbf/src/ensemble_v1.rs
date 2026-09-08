//! The ensemble fold, as one failure-atomic plan.
//!
//! An ensemble market's `k` sources are captured independently inside the
//! window, each capture a FRAGMENT: a kind-1 certificate written by that
//! source's own provider route into the member's seat instead of the market's
//! terminal seat, with the Source state left on `Primary`. After the window's
//! closed deadline this module folds them: it decodes each Resolution-owned
//! seat as a fragment and checks it against the market, refuses fewer than the
//! quorum by name, takes the median of the readings through the one scan the
//! contract has, commits the cell through the primary transition, writes the
//! market's own terminal certificate (today's shape, `attempt_index` zero, the
//! median as its reading and `provider_evidence` folded over the consumed
//! fragments), the fold receipt, and releases each consumed member's bounty to
//! the captor its fragment named.
//!
//! Every authority here is a value the physical outer authenticated:
//! `crate::relay_transport_v1::process_ensemble_fold` owns the accounts, the
//! seat derivations and the writes. Nothing here mutates.

use dclutch_product::ResultDomainV2;
use dclutch_product::svm_reader::AuthenticatedProductRuntimeV2;
use dclutch_source::resolution::{ResolutionCertificateKindV2, ResolutionCertificateV2};
use dclutch_source::{
    ContentId as SourceContentId, ENSEMBLE_EVIDENCE_DOMAIN_V1, ENSEMBLE_MAX_MEMBERS_V1,
    EnsembleFoldReceiptV1, RecoveryPolicyV2, SourceMaterialV3, SourceResolutionStateV2,
    WindowSpecV1,
};
use solana_program::hash::hashv;

use alloc::boxed::Box;

use crate::funded::{
    AuthenticatedFailureFundingV2, FundedWalkErrorV1, MemberBountyPlanV1,
    plan_member_bounty_releases_v1,
};

/// The greatest `k`, as a frame width.
pub const ENSEMBLE_MAX_MEMBERS: usize = ENSEMBLE_MAX_MEMBERS_V1 as usize;

/// Stable refusal from the pure fold.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnsembleFoldErrorV1 {
    /// The request's coordinates did not match the authenticated Source state.
    Request,
    /// The material declares no ensemble, or its records did not form one graph.
    Source,
    /// Product Runtime V2 record identity or outcome width differed.
    Product,
    /// A member seat held something that is not this market's fragment.
    Fragment,
    /// Fewer fragments than the quorum.
    Quorum,
    /// The window's deadline has not passed, or the transition refused.
    Transition,
    /// A member's bounty row was missing, misbound or already spent.
    Funding,
    /// A checked integer conversion overflowed.
    Arithmetic,
}

/// The exact coordinates the physical outer authenticated before calling.
#[derive(Clone, Copy)]
pub struct EnsembleFoldRequestV1 {
    /// Core Market account.
    pub market: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact positive terminal sequence naming the certificate, the receipt
    /// and every member seat.
    pub terminal_sequence: u64,
    /// The terminal certificate account this plan writes.
    pub certificate_account: [u8; 32],
    /// The Clock at execution.
    pub current_unix_seconds: i64,
}

/// One member seat as the outer read it: a fragment, or nothing.
#[derive(Clone, Copy)]
pub enum MemberSeatV1 {
    /// A System-owned, empty seat: this member did not answer.
    Vacant,
    /// A Resolution-owned seat holding a decoded certificate.
    Written(ResolutionCertificateV2),
}

/// The Source records the fold reads: the material, its window, and the
/// policy whose leading slots are the members.
#[derive(Clone, Copy)]
pub struct AuthenticatedEnsembleSourceV1 {
    /// `SourceMaterialV3` content identity, from the Market's own policy.
    pub material_id: SourceContentId,
    /// The authenticated material.
    pub material: SourceMaterialV3,
    /// `WindowSpecV1` content identity, from the material.
    pub window_spec_id: SourceContentId,
    /// The authenticated window.
    pub window: WindowSpecV1,
    /// `RecoveryPolicyV2` content identity, from the material.
    pub policy_id: SourceContentId,
    /// The authenticated policy.
    pub policy: RecoveryPolicyV2,
    /// The primary source's provider release, member zero's `route`.
    pub primary_provider_release_id: SourceContentId,
    /// The statistic's declared source-to-result shift.
    pub source_scale_exponent: i32,
}

/// Failure-atomic plan returned to the physical outer.
pub struct EnsembleFoldPlanV1 {
    /// The Source state after the primary transition with the median.
    pub next_source: SourceResolutionStateV2,
    /// The market's terminal certificate.
    pub certificate: ResolutionCertificateV2,
    /// The fold's receipt.
    pub receipt: EnsembleFoldReceiptV1,
    /// The ledger after every consumed member's bounty was released.
    pub next_funding: Box<[u8]>,
    /// Exact ledger lamports after the releases.
    pub funding_lamports_after: u64,
    /// Which member is paid what, in member order; `None` for a member not
    /// consumed or for the primary.
    pub bounties: [Option<crate::funded::MemberBountyReleaseV1>; ENSEMBLE_MAX_MEMBERS],
    /// The captor each consumed member named, in member order.
    pub captors: [Option<[u8; 32]>; ENSEMBLE_MAX_MEMBERS],
}

/// Which member's fragment a seat holds, checked against the market.
///
/// A fragment is a kind-1 certificate for THIS market, generation and
/// material, at THIS terminal sequence's seat, carrying `attempt_index ==
/// member` and the member's own provider release as its `route`, observed
/// inside `[window.start, window.end + max_age]`, on one scale
/// (`result_denominator == 1`). Anything else in a Resolution-owned seat is
/// refused rather than skipped: a seat that does not hold this market's
/// fragment is a hostile, not an absence.
fn admit_fragment(
    request: &EnsembleFoldRequestV1,
    source: &AuthenticatedEnsembleSourceV1,
    member: u8,
    certificate: &ResolutionCertificateV2,
) -> Result<(), EnsembleFoldErrorV1> {
    let expected_route = if member == 0 {
        source.primary_provider_release_id
    } else {
        source
            .policy
            .member_attempt(source.material.ensemble(), member)
            .map_err(|_| EnsembleFoldErrorV1::Fragment)?
            .provider_release_id()
    };
    let deadline = source
        .window
        .end_unix_seconds()
        .checked_add(i64::from(source.window.max_age_seconds()))
        .ok_or(EnsembleFoldErrorV1::Arithmetic)?;
    let observed =
        i64::try_from(certificate.observed_at).map_err(|_| EnsembleFoldErrorV1::Fragment)?;
    if certificate.kind != ResolutionCertificateKindV2::ResolutionSuccess
        || certificate.market != request.market
        || certificate.generation != request.generation
        || certificate.source_material != source.material_id.to_bytes()
        || certificate.product_record_digest != source.material.product_record_digest().to_bytes()
        || certificate.attempt_index != u32::from(member)
        || certificate.route != expected_route.to_bytes()
        || certificate.result_denominator != 1
        || certificate.provider_evidence == [0; 32]
        || certificate.receipt_account == [0; 32]
        || observed < source.window.start_unix_seconds()
        || observed > deadline
    {
        return Err(EnsembleFoldErrorV1::Fragment);
    }
    Ok(())
}

/// Plan the whole fold.
///
/// The order is the order a reviewer should check: the request against the
/// state, the material's ensemble, every seat against the market, the
/// quorum, the median and the transition, the certificate and receipt
/// encoded (so a shape the Lean-owned schemas refuse never reaches an
/// account), and only then the bounties.
#[allow(clippy::too_many_arguments)]
pub fn plan_ensemble_fold_v1(
    request: &EnsembleFoldRequestV1,
    source_state: &SourceResolutionStateV2,
    source: &AuthenticatedEnsembleSourceV1,
    product_runtime: &AuthenticatedProductRuntimeV2,
    result_domain: ResultDomainV2<'_>,
    seats: &[MemberSeatV1],
    escrow: &AuthenticatedFailureFundingV2<'_>,
) -> Result<EnsembleFoldPlanV1, EnsembleFoldErrorV1> {
    if source_state.market() != request.market
        || source_state.generation() != request.generation
        || source_state.material_id() != source.material_id
        || request.terminal_sequence == 0
    {
        return Err(EnsembleFoldErrorV1::Request);
    }
    let ensemble = source.material.ensemble();
    if ensemble.is_single()
        || source.material.window_spec() != source.window_spec_id
        || source.material.recovery_policy() != Some(source.policy_id)
        || seats.len() != usize::from(ensemble.members())
    {
        return Err(EnsembleFoldErrorV1::Source);
    }
    let product_record_digest = source.material.product_record_digest();
    let outcome_count = result_domain
        .outcome_count()
        .map_err(|_| EnsembleFoldErrorV1::Product)?;
    if product_runtime.product_record.content_digest.to_bytes() != product_record_digest.to_bytes()
        || product_runtime.coordinate_domain_id.to_bytes()
            != result_domain.coordinate_domain_id().to_bytes()
        || product_runtime.result_unit_id.to_bytes() != result_domain.result_unit_id().to_bytes()
        || product_runtime.outcome_count != outcome_count
    {
        return Err(EnsembleFoldErrorV1::Product);
    }

    // Every seat, in member order. A vacant seat is a member that did not
    // answer; a written seat must be this market's fragment for exactly this
    // member, or the fold refuses -- it never skips a hostile seat.
    let mut readings = [0_i128; ENSEMBLE_MAX_MEMBERS];
    let mut evidences = [[0_u8; 32]; ENSEMBLE_MAX_MEMBERS];
    let mut captors = [None; ENSEMBLE_MAX_MEMBERS];
    let mut digests = [[0_u8; 32]; ENSEMBLE_MAX_MEMBERS];
    let mut consumed_bitmap = 0_u8;
    let mut consumed = 0_usize;
    for (index, seat) in seats.iter().enumerate() {
        let member = u8::try_from(index).map_err(|_| EnsembleFoldErrorV1::Arithmetic)?;
        if let MemberSeatV1::Written(certificate) = seat {
            admit_fragment(request, source, member, certificate)?;
            readings[consumed] = certificate.result_numerator;
            evidences[consumed] = certificate.provider_evidence;
            captors[index] = Some(certificate.receipt_account);
            let bytes = certificate
                .to_bytes()
                .map_err(|_| EnsembleFoldErrorV1::Fragment)?;
            digests[index] = solana_program::hash::hash(&bytes).to_bytes();
            consumed_bitmap |= 1 << member;
            consumed = consumed
                .checked_add(1)
                .ok_or(EnsembleFoldErrorV1::Arithmetic)?;
        }
    }
    let consumed_count = u8::try_from(consumed).map_err(|_| EnsembleFoldErrorV1::Arithmetic)?;
    if consumed_count < ensemble.quorum() {
        return Err(EnsembleFoldErrorV1::Quorum);
    }

    // The evidence binds which fragments were folded and in what order, so
    // two folds over different member sets can never share an identity and a
    // reader holding the receipt's digests recomputes it.
    let count = [consumed_count];
    let mut parts: [&[u8]; 2 + ENSEMBLE_MAX_MEMBERS] = [&[]; 2 + ENSEMBLE_MAX_MEMBERS];
    parts[0] = ENSEMBLE_EVIDENCE_DOMAIN_V1;
    parts[1] = &count;
    for (slot, evidence) in parts
        .iter_mut()
        .skip(2)
        .zip(evidences.iter().take(consumed))
    {
        *slot = evidence;
    }
    let evidence_id = hashv(&parts[..2 + consumed]).to_bytes();
    let evidence =
        SourceContentId::new(evidence_id).map_err(|_| EnsembleFoldErrorV1::Transition)?;

    let mut next_source = *source_state;
    let fold = next_source
        .fold_ensemble_from_authenticated_domain(
            source.material_id,
            source.material,
            source.window_spec_id,
            source.window,
            product_record_digest,
            result_domain,
            evidence,
            &readings[..consumed],
            source.source_scale_exponent,
            request.generation,
            request.current_unix_seconds,
            request.terminal_sequence,
        )
        .map_err(|error| match error {
            dclutch_source::Error::EnsembleQuorumNotMet => EnsembleFoldErrorV1::Quorum,
            _ => EnsembleFoldErrorV1::Transition,
        })?;
    if fold.decision.selector() >= result_domain.failure_selector()
        || fold.decision.outcome_count() != outcome_count
    {
        return Err(EnsembleFoldErrorV1::Product);
    }

    let observed_at =
        u64::try_from(request.current_unix_seconds).map_err(|_| EnsembleFoldErrorV1::Arithmetic)?;
    let certificate = ResolutionCertificateV2 {
        kind: ResolutionCertificateKindV2::ResolutionSuccess,
        market: request.market,
        // The fold answered on the market's declared ensemble, whose identity
        // is the primary source's release: one route for one market, as every
        // primary terminal records it.
        route: source.primary_provider_release_id.to_bytes(),
        source_material: source.material_id.to_bytes(),
        product_record_digest: product_record_digest.to_bytes(),
        provider_evidence: evidence_id,
        funding_allocation: [0; 32],
        receipt_account: request.certificate_account,
        generation: request.generation,
        attempt_index: 0,
        schedule_index: 0,
        selector: fold.decision.selector(),
        work_paid: 0,
        funding_remaining: 0,
        result_numerator: fold.median,
        result_denominator: 1,
        observed_at,
    };
    certificate
        .validate_terminal_product(product_record_digest.to_bytes(), outcome_count)
        .and_then(|()| certificate.to_bytes().map(|_| ()))
        .map_err(|_| EnsembleFoldErrorV1::Transition)?;
    let receipt = EnsembleFoldReceiptV1 {
        member_count: ensemble.members(),
        quorum: ensemble.quorum(),
        consumed_count,
        consumed_bitmap,
        market: request.market,
        generation: request.generation,
        terminal_sequence: request.terminal_sequence,
        source_material: source.material_id.to_bytes(),
        median_numerator: fold.median,
        selector: fold.decision.selector(),
        fragment_digests: digests,
    };
    receipt
        .to_bytes()
        .map_err(|_| EnsembleFoldErrorV1::Transition)?;

    let MemberBountyPlanV1 {
        next_funding,
        releases,
        funding_lamports_after,
    } = plan_member_bounty_releases_v1(escrow, source.policy, ensemble, consumed_bitmap).map_err(
        |error| match error {
            FundedWalkErrorV1::Source => EnsembleFoldErrorV1::Source,
            _ => EnsembleFoldErrorV1::Funding,
        },
    )?;

    Ok(EnsembleFoldPlanV1 {
        next_source,
        certificate,
        receipt,
        next_funding,
        funding_lamports_after,
        bounties: releases,
        captors,
    })
}
