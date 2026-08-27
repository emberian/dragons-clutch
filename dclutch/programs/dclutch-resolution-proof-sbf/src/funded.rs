//! The funded liveness walk, as Runtime V2 accounting.
//!
//! This is the module that makes `MAINNET_STATE_RELAY.md` §4.8's headline
//! property executable: **a silent provider cannot make a market unresolvable,
//! only drive it to a pre-disclosed outcome, along a bounded, prepaid,
//! permissionless path that pays whoever walks it.** Every clause of that
//! sentence is a check below, and the last one is why this module exists at all
//! — `ResolutionCertificateV2::validate_shape` refuses a `ResolutionFailure`
//! whose `funding_allocation` or `work_paid` is zero, so the Lean-owned terminal
//! schema encodes prepayment as a *decode-time* invariant. There is no unfunded
//! failure certificate to emit, and therefore no route that could emit one
//! without first debiting an escrow that exists.
//!
//! # What replaced what
//!
//! The V1 generation of this file authored the same accounting against
//! `SourceResolutionStateV1`, `ResolutionCertificateV1` and a three-action
//! `FundedTransitionRequestV3` direct ABI, and it was orphaned dead code: no
//! `mod funded;` named it and its only call site sat under `#[cfg(any())]`. It
//! is gone rather than kept beside this, per `AGENTS.md` — a superseded
//! authority path is deleted in the same convergence cycle as its successor.
//!
//! # Why the walk is one transition rather than three
//!
//! The V1 walk was `FailNext` per recovery leg, then `Exhaust`, then
//! `CommitFailure` — six funded transitions in the worst case, each debiting its
//! own allocation. That shape belongs to a market that *bought* named
//! alternative sources. `SourceResolutionStateV2::exhaust_after_primary_deadline`
//! refuses any material carrying a recovery policy precisely because skipping
//! paid-for legs would take an outcome away from the holders who paid for them.
//!
//! So the walk this module plans is the whole walk for a market with no
//! recovery policy: `Primary → Exhausted → FailureCommitted`, one debit from the
//! explicit-failure compartment, one `ResolutionFailure` certificate. There is
//! no intermediate `Exhausted` certificate because there is no intermediate
//! moment a third party could act on — nothing can be observed between the two
//! transitions, and minting a certificate for a state no route can leave would
//! be recording a bounty for work nobody can do.
//!
//! # Nothing here mutates and nothing here reads an account
//!
//! Account ownership, Registry finality, PDA derivation and custody stay in the
//! physical outer (`crate::relay_transport_v1`), exactly as they do for
//! [`crate::relay_v1`]. This module takes values the outer authenticated and
//! returns a plan.

use dclutch_capability_contract::{
    CapabilityManifestV1, ContentId as CapabilityContentId, FundingAssetClassV1, FundingCompartment,
    FundingCustodyObservationV1, FundingStateV1, FundingStatus,
};
use dclutch_product_runtime_v2::ResultDomainV2;
use dclutch_product_runtime_v2_svm_reader::AuthenticatedProductRuntimeV2;
use dclutch_resolution_codec::{
    RESOLUTION_CONTROLLER_RELEASE_ID_V4, ResolutionCertificateKindV2, ResolutionCertificateV2,
};
use dclutch_source_contract::{
    ContentId as SourceContentId, SourceMaterialV2, SourceResolutionStateV2, WindowSpecV1,
};

/// Stable refusal from the pure funded walk.
///
/// Each variant names the question that failed, not the field: a caller that
/// gets `Funding` back learns the escrow did not hold what the market promised,
/// and a caller that gets `Window` back learns the deadline has not passed. The
/// physical outer maps these onto the Resolution role's own discriminants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FundedWalkErrorV1 {
    /// The request's coordinates did not match the authenticated Source state.
    Request,
    /// The authenticated Source records did not form the graph the state names.
    Source,
    /// Product Runtime V2 record identity or outcome width differed.
    Product,
    /// The primary deadline has not passed, or the transition refused.
    Transition,
    /// The escrowed compartment was missing, misbound, or empty.
    Funding,
}

/// The exact coordinates the physical outer authenticated before calling.
#[derive(Clone, Copy)]
pub struct DeadlineFailureRequestV1 {
    /// Core Market account.
    pub market: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact positive terminal sequence naming the certificate.
    pub terminal_sequence: u64,
    /// The certificate account this plan will be written into.
    pub certificate_account: [u8; 32],
    /// Devnet `Clock` at execution.
    pub current_unix_seconds: i64,
}

/// The escrowed compartment, as the outer authenticated it.
///
/// `custody` is a physical observation — the account's lamports against the
/// exact Rent minimum for its width — and it is what makes the debit below an
/// accounting operation rather than a wish: a `FundingState` claiming to hold a
/// bounty in an account that does not hold the lamports refuses here.
#[derive(Clone, Copy)]
pub struct AuthenticatedFailureFundingV1<'a> {
    /// Capability-manifest content identity, from the authenticated Market.
    pub manifest_id: CapabilityContentId,
    /// The authenticated capability manifest.
    pub manifest: CapabilityManifestV1<'a>,
    /// The manifest entry this compartment was created against.
    pub entry_index: u16,
    /// The decoded persisted funding state.
    pub funding: FundingStateV1,
    /// Physical custody of the funding-state account.
    pub custody: FundingCustodyObservationV1,
}

/// The independently authenticated Source values the walk reads.
#[derive(Clone, Copy)]
pub struct AuthenticatedWalkSourceV1 {
    /// `SourceMaterialV2` content identity, from the Market's own policy.
    pub material_id: SourceContentId,
    /// The authenticated material.
    pub material: SourceMaterialV2,
    /// `WindowSpecV1` content identity, from the material.
    pub window_spec_id: SourceContentId,
    /// The authenticated window whose deadline the walk is past.
    pub window: WindowSpecV1,
}

/// Failure-atomic plan returned to the physical SBF outer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeadlineFailurePlanV1 {
    /// The Source state after `Primary → Exhausted → FailureCommitted`.
    pub next_source: SourceResolutionStateV2,
    /// The terminal `ResolutionFailure` certificate.
    pub certificate: ResolutionCertificateV2,
    /// The funding state after the bounty debit.
    pub next_funding: FundingStateV1,
    /// Exact lamports credited to whoever walked it.
    pub work_paid: u64,
    /// Bounty principal still escrowed after this debit.
    pub funding_remaining: u64,
    /// Exact funding-account lamports after the debit.
    pub funding_lamports_after: u64,
}

/// Debit the escrowed explicit-failure compartment and credit one walker.
///
/// The allocation identity is not a caller's choice and not a parameter: the
/// explicit-failure compartment is the one whose manifest entry names *this
/// market's own Source material* as its configuration, which is exactly the
/// binding `core_effect`'s `authenticate_funding_entries` established when the
/// three compartments were created. A caller presenting the recovery or
/// exhaustion compartment instead is refused by that comparison rather than by
/// an account-position convention.
fn plan_funding_release(
    escrow: &AuthenticatedFailureFundingV1<'_>,
    material_id: SourceContentId,
) -> Result<(FundingStateV1, u64, u64, u64), FundedWalkErrorV1> {
    let entry = escrow
        .manifest
        .entry(escrow.entry_index)
        .map_err(|_| FundedWalkErrorV1::Funding)?;
    if entry.config_id().to_bytes() != material_id.to_bytes()
        || entry.release_id().to_bytes() != RESOLUTION_CONTROLLER_RELEASE_ID_V4
        || escrow.funding.entry_index() != escrow.entry_index
        || escrow.funding.manifest_content_id() != escrow.manifest_id
        || escrow.funding.status() != FundingStatus::Active
    {
        return Err(FundedWalkErrorV1::Funding);
    }
    escrow
        .funding
        .validate_against(escrow.manifest_id, escrow.manifest, escrow.custody)
        .map_err(|_| FundedWalkErrorV1::Funding)?;

    // The bounty is the capability's own quote. Nobody chooses what the walk
    // pays at walk time; the market disclosed it before it opened.
    let quote = entry.funding_quote().amounts().bounty();
    if quote.asset_class() != FundingAssetClassV1::NativeLamports || quote.amount() == 0 {
        return Err(FundedWalkErrorV1::Funding);
    }
    let mut next_funding = escrow.funding;
    let released = next_funding
        .release(
            escrow.manifest_id,
            escrow.manifest,
            escrow.custody,
            FundingCompartment::Bounty,
            quote.amount(),
        )
        .map_err(|_| FundedWalkErrorV1::Funding)?;
    if released.asset_class() != FundingAssetClassV1::NativeLamports
        || released.amount() != quote.amount()
    {
        return Err(FundedWalkErrorV1::Funding);
    }

    let work_paid = quote.amount();
    let funding_lamports_after = escrow
        .custody
        .state_account_lamports()
        .checked_sub(work_paid)
        .ok_or(FundedWalkErrorV1::Funding)?;

    // The post-state has to be a canonical funding state against the custody
    // the payout will actually leave behind, not against the one it started
    // with. Without this the route could pay out of an account it was about to
    // leave below its own Rent reserve, and the refusal would arrive one
    // instruction later as an unrelated rent failure.
    let post_custody = FundingCustodyObservationV1::native_only(
        funding_lamports_after,
        escrow.custody.exact_state_rent_lamports(),
    )
    .map_err(|_| FundedWalkErrorV1::Funding)?;
    next_funding
        .validate_against(escrow.manifest_id, escrow.manifest, post_custody)
        .map_err(|_| FundedWalkErrorV1::Funding)?;

    Ok((
        next_funding,
        work_paid,
        next_funding.remaining().bounty().amount(),
        funding_lamports_after,
    ))
}

/// Plan the whole deadline-driven failure walk.
///
/// The order is deliberate and it is the order a reviewer should check: the
/// Source graph and the Product graph are joined first, then the escrow is
/// debited, then the two Source transitions run, then the certificate is built
/// and *encoded* — so a certificate the Lean-owned schema would refuse cannot
/// reach an account, and a debit cannot be planned against a market whose
/// deadline has not passed.
pub fn plan_deadline_failure_v1(
    request: &DeadlineFailureRequestV1,
    source_state: &SourceResolutionStateV2,
    source: &AuthenticatedWalkSourceV1,
    product_runtime: &AuthenticatedProductRuntimeV2,
    result_domain: ResultDomainV2<'_>,
    escrow: &AuthenticatedFailureFundingV1<'_>,
) -> Result<DeadlineFailurePlanV1, FundedWalkErrorV1> {
    if source_state.market() != request.market
        || source_state.generation() != request.generation
        || source_state.material_id() != source.material_id
        || request.terminal_sequence == 0
    {
        return Err(FundedWalkErrorV1::Request);
    }
    if source.material.window_spec() != source.window_spec_id {
        return Err(FundedWalkErrorV1::Source);
    }

    let product_record_digest = source.material.product_record_digest();
    let outcome_count = result_domain
        .outcome_count()
        .map_err(|_| FundedWalkErrorV1::Product)?;
    if product_runtime.product_record.content_digest.to_bytes() != product_record_digest.to_bytes()
        || product_runtime.coordinate_domain_id.to_bytes()
            != result_domain.coordinate_domain_id().to_bytes()
        || product_runtime.result_unit_id.to_bytes() != result_domain.result_unit_id().to_bytes()
        || product_runtime.outcome_count != outcome_count
    {
        return Err(FundedWalkErrorV1::Product);
    }

    let (next_funding, work_paid, funding_remaining, funding_lamports_after) =
        plan_funding_release(escrow, source.material_id)?;

    let mut next_source = *source_state;
    next_source
        .exhaust_after_primary_deadline(
            source.material_id,
            source.material,
            source.window_spec_id,
            source.window,
            request.generation,
            request.current_unix_seconds,
        )
        .map_err(|_| FundedWalkErrorV1::Transition)?;
    let decision = next_source
        .commit_failure_from_authenticated_domain(
            source.material_id,
            source.material,
            product_record_digest,
            result_domain,
            request.generation,
            request.current_unix_seconds,
            request.terminal_sequence,
        )
        .map_err(|_| FundedWalkErrorV1::Transition)?;
    if decision.outcome_count() != outcome_count
        || decision.selector() != result_domain.failure_selector()
    {
        return Err(FundedWalkErrorV1::Product);
    }

    let certificate = ResolutionCertificateV2 {
        kind: ResolutionCertificateKindV2::ResolutionFailure,
        market: request.market,
        // No route: this terminal is not attributable to any provider, which is
        // the whole content of the claim "the relayer went silent".
        route: [0; 32],
        source_material: source.material_id.to_bytes(),
        product_record_digest: product_record_digest.to_bytes(),
        provider_evidence: [0; 32],
        // The allocation identity is the market's own Source material, as the
        // manifest entry checked above already pinned.
        funding_allocation: source.material_id.to_bytes(),
        receipt_account: request.certificate_account,
        generation: request.generation,
        // Zero legs were skipped: this material bought none.
        attempt_index: 0,
        schedule_index: 0,
        selector: decision.selector(),
        work_paid,
        funding_remaining,
        result_numerator: 0,
        result_denominator: 0,
        observed_at: 0,
    };
    certificate
        .validate_terminal_product(product_record_digest.to_bytes(), outcome_count)
        .and_then(|()| certificate.to_bytes().map(|_| ()))
        .map_err(|_| FundedWalkErrorV1::Transition)?;

    Ok(DeadlineFailurePlanV1 {
        next_source,
        certificate,
        next_funding,
        work_paid,
        funding_remaining,
        funding_lamports_after,
    })
}
