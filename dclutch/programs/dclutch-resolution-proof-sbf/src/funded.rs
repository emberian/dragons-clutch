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
//! **That `#[cfg(any())]` call site is gone too**, along with the other
//! thirteen in `lib.rs`, so neither half of the sentence above can be looked up
//! any more; it is history, not a pointer.
//!
//! # The two walks this module plans, and why they are two
//!
//! The V1 walk was `FailNext` per recovery leg, then `Exhaust`, then
//! `CommitFailure` — six funded transitions in the worst case, each debiting its
//! own allocation, and every one of them a caller-chosen action byte. Decision
//! 0027 keeps the ladder and drops that shape: a caller who could pick `Exhaust`
//! while an attempt was still funded could skip a leg the holders paid for.
//!
//! So [`process_funded_transition`] is ONE transition with two arms — the
//! current window has closed, and either the policy funds another attempt or it
//! does not — and [`plan_deadline_failure_v1`] is the terminal both ends of the
//! ladder reach. The two are separate because they answer to different
//! materials:
//!
//! - a market that bought no alternative sources arrives at the terminal on
//!   `Primary`, and the failure walk spends its own deadline to reach
//!   `Exhausted`: `Primary → Exhausted → FailureCommitted`, one debit from the
//!   explicit-failure compartment, one `ResolutionFailure` certificate;
//! - a market that bought them arrives already `Exhausted`, because its ladder
//!   walked every leg it sold and each crank was paid from that leg's own
//!   compartment. `exhaust_after_primary_deadline` still refuses that material
//!   by name, and still should: the refusal is the ladder's own correctness
//!   condition, not an obstacle to it.
//!
//! Hence the exhaustion inside the failure walk is conditional and the commit
//! is not. Nothing is weakened by that:
//! `commit_failure_from_authenticated_domain` refuses any phase but
//! `Exhausted`, and a ladder with a funded leg left has not reached it.
//!
//! # Nothing here mutates and nothing here reads an account
//!
//! Account ownership, Registry finality, PDA derivation and custody stay in the
//! physical outer (`crate::relay_transport_v1`), exactly as they do for
//! [`crate::relay_v1`]. This module takes values the outer authenticated and
//! returns a plan.

use dclutch_market::capability_manifest::{
    CapabilityManifestV1, ContentId as CapabilityContentId,
    FUNDING_LEDGER_ACTIVE_ADMISSIBLE_STATES_V2, FUNDING_LEDGER_HEADER_BYTES_V2,
    FUNDING_LEDGER_SLOT_BYTES_V2, FundingAssetClassV1, FundingCompartment, FundingLedgerV2,
};

/// Exact width of the three-row Resolution controller subset ledger.
pub(crate) const RESOLUTION_FUNDING_LEDGER_BYTES_V2: usize =
    FUNDING_LEDGER_HEADER_BYTES_V2 + 3 * FUNDING_LEDGER_SLOT_BYTES_V2;
use dclutch_product::ResultDomainV2;
use dclutch_product::svm_reader::AuthenticatedProductRuntimeV2;
use dclutch_source::resolution::{
    RESOLUTION_CONTROLLER_RELEASE_ID_V7, ResolutionCertificateKindV2, ResolutionCertificateV2,
};
use dclutch_source::{
    ContentId as SourceContentId, RecoveryCrankV2, RecoveryPolicyV2, SourceMaterialV3,
    SourceResolutionPhaseV1, SourceResolutionStateV2, WindowSpecV1,
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
/// The ledger and its physical custody are carried together so the debit below
/// authenticates aggregate native custody before and after changing only the
/// selected Failure row.
#[derive(Clone, Copy)]
pub struct AuthenticatedFailureFundingV2<'manifest> {
    /// Capability-manifest content identity, from the authenticated Market.
    pub manifest_id: CapabilityContentId,
    /// The authenticated capability manifest.
    pub manifest: CapabilityManifestV1<'manifest>,
    /// The manifest entry this compartment was created against.
    pub entry_index: u16,
    /// Exact hostile-decoded persisted subset-ledger bytes.
    pub ledger_bytes: [u8; RESOLUTION_FUNDING_LEDGER_BYTES_V2],
    /// Exact Rent reserve for the full ledger width.
    pub exact_ledger_rent_lamports: u64,
    /// Physical lamports held by the aggregate ledger account.
    pub ledger_account_lamports: u64,
}

/// The independently authenticated Source values the walk reads.
#[derive(Clone, Copy)]
pub struct AuthenticatedWalkSourceV1 {
    /// `SourceMaterialV3` content identity, from the Market's own policy.
    pub material_id: SourceContentId,
    /// The authenticated material.
    pub material: SourceMaterialV3,
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
    /// The complete subset ledger after the Failure-row bounty debit.
    pub next_funding: [u8; RESOLUTION_FUNDING_LEDGER_BYTES_V2],
    /// Exact lamports credited to whoever walked it.
    pub work_paid: u64,
    /// Bounty principal still escrowed after this debit.
    pub funding_remaining: u64,
    /// Exact funding-account lamports after the debit.
    pub funding_lamports_after: u64,
}

/// Debit one escrowed compartment and credit one walker.
///
/// The allocation identity is never a caller's choice. `selecting_config` is
/// the configuration `core_effect`'s `authenticate_funding_entries` pinned to
/// this compartment when the market's three were created, and each of the three
/// walks derives its own from records rather than accepting one: the failure
/// walk from the market's own Source material, an advance from the attempt's
/// funding allocation, an exhaustion from the recovery policy's own digest. A
/// walk presenting another compartment is refused by this comparison rather
/// than by an account-position convention -- there is no position to get
/// right, because all three rows live in one ledger account.
fn plan_funding_release(
    escrow: &AuthenticatedFailureFundingV2<'_>,
    selecting_config: [u8; 32],
) -> Result<([u8; RESOLUTION_FUNDING_LEDGER_BYTES_V2], u64, u64, u64), FundedWalkErrorV1> {
    let entry = escrow
        .manifest
        .entry(escrow.entry_index)
        .map_err(|_| FundedWalkErrorV1::Funding)?;
    if entry.config_id().to_bytes() != selecting_config
        || entry.release_id().to_bytes() != RESOLUTION_CONTROLLER_RELEASE_ID_V7
    {
        return Err(FundedWalkErrorV1::Funding);
    }
    let ledger =
        FundingLedgerV2::decode(&escrow.ledger_bytes).map_err(|_| FundedWalkErrorV1::Funding)?;
    let authenticated = ledger
        .authenticate(escrow.manifest_id, escrow.manifest)
        .map_err(|_| FundedWalkErrorV1::Funding)?;
    let failure_slot = authenticated
        .slot(escrow.entry_index)
        .map_err(|_| FundedWalkErrorV1::Funding)?;
    if !FUNDING_LEDGER_ACTIVE_ADMISSIBLE_STATES_V2.admits(failure_slot.status()) {
        return Err(FundedWalkErrorV1::Funding);
    }
    authenticated
        .validate_native_custody(
            escrow.ledger_account_lamports,
            escrow.exact_ledger_rent_lamports,
            false,
        )
        .map_err(|_| FundedWalkErrorV1::Funding)?;

    // The bounty is the capability's own quote. Nobody chooses what the walk
    // pays at walk time; the market disclosed it before it opened.
    let quote = entry.funding_quote().amounts().bounty();
    if quote.asset_class() != FundingAssetClassV1::NativeLamports || quote.amount() == 0 {
        return Err(FundedWalkErrorV1::Funding);
    }
    let mut next_funding = escrow.ledger_bytes;
    let released = FundingLedgerV2::release_in_place(
        &mut next_funding,
        escrow.manifest_id,
        escrow.manifest,
        escrow.entry_index,
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
        .ledger_account_lamports
        .checked_sub(work_paid)
        .ok_or(FundedWalkErrorV1::Funding)?;
    let post = FundingLedgerV2::decode(&next_funding)
        .and_then(|ledger| ledger.authenticate(escrow.manifest_id, escrow.manifest))
        .map_err(|_| FundedWalkErrorV1::Funding)?;

    // The mutation authority is exactly one logical row. The header and every
    // other selected row must remain byte-identical even when two rows carry
    // equal totals.
    if escrow.ledger_bytes[..FUNDING_LEDGER_HEADER_BYTES_V2]
        != next_funding[..FUNDING_LEDGER_HEADER_BYTES_V2]
    {
        return Err(FundedWalkErrorV1::Funding);
    }
    let selected_mask = ledger.selected_mask();
    let mut entry_index = 0_u16;
    while entry_index < 16 {
        let selected = selected_mask & (1_u16 << u32::from(entry_index)) != 0;
        if selected
            && entry_index != escrow.entry_index
            && authenticated
                .slot_bytes(entry_index)
                .map_err(|_| FundedWalkErrorV1::Funding)?
                != post
                    .slot_bytes(entry_index)
                    .map_err(|_| FundedWalkErrorV1::Funding)?
        {
            return Err(FundedWalkErrorV1::Funding);
        }
        entry_index = entry_index
            .checked_add(1)
            .ok_or(FundedWalkErrorV1::Funding)?;
    }
    post.validate_native_custody(
        funding_lamports_after,
        escrow.exact_ledger_rent_lamports,
        false,
    )
    .map_err(|_| FundedWalkErrorV1::Funding)?;

    Ok((
        next_funding,
        work_paid,
        post.slot(escrow.entry_index)
            .map_err(|_| FundedWalkErrorV1::Funding)?
            .remaining()
            .bounty()
            .amount(),
        funding_lamports_after,
    ))
}

/// The recovery policy the ladder walks, as the outer authenticated it.
///
/// It rides beside [`AuthenticatedWalkSourceV1`] rather than inside it because
/// the deadline-failure walk reads no policy at all -- a material carrying one
/// is precisely the material that walk refuses -- and widening its input to
/// carry a field it must never use would be the wrong shape for the route whose
/// whole property is that it needs nothing from anybody.
#[derive(Clone, Copy)]
pub struct AuthenticatedRecoveryPolicyV1 {
    /// `RecoveryPolicyV2` content identity, from the material's own selection.
    pub policy_id: SourceContentId,
    /// The authenticated finalized policy.
    pub policy: RecoveryPolicyV2,
}

/// Failure-atomic plan for one crank of the funded ordered-recovery ladder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundedTransitionPlanV1 {
    /// The Source state after exactly one rung.
    pub next_source: SourceResolutionStateV2,
    /// The receipt this crank writes.
    pub certificate: ResolutionCertificateV2,
    /// The complete subset ledger after this crank's bounty debit.
    pub next_funding: [u8; RESOLUTION_FUNDING_LEDGER_BYTES_V2],
    /// Exact lamports credited to whoever cranked it.
    pub work_paid: u64,
    /// Bounty principal still escrowed in this compartment after the debit.
    pub funding_remaining: u64,
    /// Exact funding-account lamports after the debit.
    pub funding_lamports_after: u64,
    /// The receipt's kind, which is also the seat's PDA seed.
    pub certificate_kind: ResolutionCertificateKindV2,
}

/// Plan exactly one funded transition of the ordered-recovery ladder.
///
/// # What this name is
///
/// Six live comments and the completion contract's own recovery row cite
/// `funded::process_funded_transition` in the present tense, as "the ladder that
/// consumes the paid-for legs". Until now the citation dangled: the V1 function
/// of that name was deleted with the rest of the orphaned generation, and the
/// `#[cfg(any())]` call site the comments describe went with it. The symbol is
/// back, and it is back as ONE function rather than the three-action ABI the V1
/// walk had, because there is only one decision to make.
///
/// # The one decision
///
/// The current window has closed, and either the policy funds another attempt
/// or it does not. `SourceResolutionStateV2::crank_recovery_ladder` makes that
/// decision; this function turns it into a payment and a receipt. There is no
/// caller-chosen action byte, and that is load-bearing: a caller who could pick
/// `Exhaust` while an attempt was still funded could skip a leg the holders paid
/// for, which is the whole thing a *funded* ladder forbids.
///
/// # Why the order is transition-then-debit, and the failure walk's is not
///
/// [`plan_deadline_failure_v1`] debits first, because it always spends the same
/// compartment and refusing an unpayable walk before it moves the market is the
/// stronger order. Here the compartment is not known until the crank has been
/// decided -- an advance spends the entered attempt's own allocation, an
/// exhaustion spends the one the policy itself configures -- so the transition
/// runs first. Nothing is at risk in either order: the plan is pure, and a
/// refusal anywhere in it writes no byte and moves no lamport.
///
/// # Nothing here mutates and nothing here reads an account
pub fn process_funded_transition(
    request: &DeadlineFailureRequestV1,
    source_state: &SourceResolutionStateV2,
    source: &AuthenticatedWalkSourceV1,
    ladder: &AuthenticatedRecoveryPolicyV1,
    escrow: &AuthenticatedFailureFundingV2<'_>,
) -> Result<FundedTransitionPlanV1, FundedWalkErrorV1> {
    if source_state.market() != request.market
        || source_state.generation() != request.generation
        || source_state.material_id() != source.material_id
        || request.terminal_sequence == 0
    {
        return Err(FundedWalkErrorV1::Request);
    }
    if source.material.window_spec() != source.window_spec_id
        || source.material.recovery_policy() != Some(ladder.policy_id)
    {
        return Err(FundedWalkErrorV1::Source);
    }

    let mut next_source = *source_state;
    let crank = next_source
        .crank_recovery_ladder(
            source.material_id,
            source.material,
            source.window_spec_id,
            source.window,
            ladder.policy_id,
            ladder.policy,
            request.generation,
            request.current_unix_seconds,
        )
        .map_err(|_| FundedWalkErrorV1::Transition)?;

    // Which compartment pays is a function of which rung was taken, and both
    // identities come out of records the market finalized before it opened.
    let (certificate_kind, attempt_index, route, selecting_config) = match crank {
        RecoveryCrankV2::Advanced {
            attempt_index,
            attempt,
        } => (
            ResolutionCertificateKindV2::RecoveryAdvanced,
            u32::from(attempt_index),
            attempt.provider_release_id().to_bytes(),
            attempt.funding_allocation_id().to_bytes(),
        ),
        RecoveryCrankV2::Exhausted {
            final_attempt_index,
            final_attempt,
        } => (
            ResolutionCertificateKindV2::Exhausted,
            // The count of legs the market has now spent, which for the final
            // rung is one past its index. A reader of the receipt learns how
            // much of the paid-for ladder was actually walked.
            u32::from(final_attempt_index).saturating_add(1),
            final_attempt.provider_release_id().to_bytes(),
            ladder.policy_id.to_bytes(),
        ),
    };

    let (next_funding, work_paid, funding_remaining, funding_lamports_after) =
        plan_funding_release(escrow, selecting_config)?;

    let observed_at =
        u64::try_from(request.current_unix_seconds).map_err(|_| FundedWalkErrorV1::Request)?;
    let certificate = ResolutionCertificateV2 {
        kind: certificate_kind,
        market: request.market,
        // Unlike the failure terminal, a rung IS attributable: the route is the
        // provider release the attempt named, so the receipt records which feed
        // was asked and did not answer.
        route,
        source_material: source.material_id.to_bytes(),
        product_record_digest: source.material.product_record_digest().to_bytes(),
        provider_evidence: [0; 32],
        funding_allocation: selecting_config,
        receipt_account: request.certificate_account,
        generation: request.generation,
        attempt_index,
        schedule_index: 0,
        // A crank selects nothing. `validate_terminal_product` refuses to be
        // asked about these two kinds at all, and `validate_shape` pins the
        // selector to zero, so a rung cannot smuggle an outcome.
        selector: 0,
        work_paid,
        funding_remaining,
        result_numerator: 0,
        result_denominator: 0,
        observed_at,
    };
    certificate
        .to_bytes()
        .map_err(|_| FundedWalkErrorV1::Transition)?;

    Ok(FundedTransitionPlanV1 {
        next_source,
        certificate,
        next_funding,
        work_paid,
        funding_remaining,
        funding_lamports_after,
        certificate_kind,
    })
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
    escrow: &AuthenticatedFailureFundingV2<'_>,
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
        plan_funding_release(escrow, source.material_id.to_bytes())?;

    let mut next_source = *source_state;
    // Two ways into `Exhausted`, one way out of it.
    //
    // A market that bought no alternative sources arrives here on `Primary` and
    // this walk spends its own deadline to get there -- that transition refuses
    // a recovery-bearing material by name, and still should: skipping paid-for
    // legs would take an outcome away from the holders who paid for them. A
    // market that DID buy them arrives here already `Exhausted`, because its
    // funded ladder walked every leg it sold and the last crank left it there.
    //
    // So the exhaustion is conditional and the commit is not. Nothing is
    // weakened by that: `commit_failure_from_authenticated_domain` refuses any
    // phase but `Exhausted`, so a market that reached neither end is refused by
    // the same conjunct it always was, and a ladder that still has a funded leg
    // has not reached `Exhausted` to begin with.
    if next_source.phase() == SourceResolutionPhaseV1::Primary {
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
    }
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
        // Zero legs were SKIPPED. A market with no policy bought none; a
        // market with one arrived here having walked every leg it sold, and
        // the count of legs it actually walked is on the ladder's own
        // `Exhausted` receipt rather than smuggled into the terminal.
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
